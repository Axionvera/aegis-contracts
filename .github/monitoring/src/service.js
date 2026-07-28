/**
 * AegisMonitor - the composition root.
 *
 * Wires the streaming client into the full pipeline:
 *
 *   Soroban RPC (WebSocket or getEvents poll)
 *        -> normalize (ScVal decode -> canonical envelope)
 *        -> EventStore.append            (persistence + replay source)
 *        -> AnalyticsEngine.record       (dashboard metrics)
 *        -> EventRouter.dispatch         (filtering + routing)
 *        -> AlertEngine.process          (pattern alerting)
 *        -> TriggerEngine.process        (automated actions)
 *        -> DashboardServer.broadcast    (live UI fan-out)
 */

import { EventEmitter } from 'node:events';
import { loadConfig } from './config.js';
import { SorobanEventStream, TRANSPORT } from './rpc/websocket-client.js';
import { EventRouter } from './events/filter.js';
import { AlertEngine, sinks } from './alerts/index.js';
import { EventStore } from './store/event-store.js';
import { TriggerEngine } from './triggers/index.js';
import { AnalyticsEngine } from './analytics/index.js';
import { DashboardServer } from './dashboard/server.js';
import { serializeEvent } from './events/normalize.js';
import { defaultRoutes, defaultAlertRules, defaultTriggers } from './defaults.js';

const LEVELS = { debug: 10, info: 20, warn: 30, error: 40 };

export function createLogger({ verbose = false, sink = console } = {}) {
  const min = verbose ? LEVELS.debug : LEVELS.info;
  return (level, message) => {
    if ((LEVELS[level] ?? 20) < min) return;
    const stamp = new Date().toISOString();
    const line = `${stamp} [${level.toUpperCase().padEnd(5)}] ${message}`;
    if (level === 'error') sink.error(line);
    else if (level === 'warn') sink.warn(line);
    else sink.log(line);
  };
}

export class AegisMonitor extends EventEmitter {
  constructor(options = {}) {
    super();
    this.config = loadConfig(options.config ?? {});
    this.logger = options.logger ?? createLogger({ verbose: this.config.verbose });

    this.stream = new SorobanEventStream({
      rpcUrl: this.config.rpcUrl,
      wsUrl: this.config.wsUrl,
      contractIds: this.config.contractIds,
      pollIntervalMs: this.config.pollIntervalMs,
      pageLimit: this.config.pageLimit,
      startLedgerLookback: this.config.startLedgerLookback,
      reconnect: this.config.reconnect,
      namespaceFilter: options.namespaceFilter ?? false,
      fetchImpl: options.fetchImpl,
      WebSocketImpl: options.WebSocketImpl,
      logger: this.logger,
    });

    this.router = new EventRouter({ logger: this.logger });
    this.alerts = new AlertEngine({ logger: this.logger });
    this.triggers = new TriggerEngine({ logger: this.logger });
    this.analytics = new AnalyticsEngine(this.config.analytics);
    this.store = new EventStore({ ...this.config.store, logger: this.logger });

    this.dashboard = null;
    this._absenceTimer = null;
    this._checkpointTimer = null;
    this.processed = 0;
    this.started = false;

    this._installDefaults(options);
    this._wire();
  }

  _installDefaults(options) {
    if (options.useDefaults === false) return;

    for (const route of defaultRoutes({ logger: this.logger })) {
      this.router.addRoute(route.name, route.filter, route.handler, { priority: route.priority });
    }
    this.alerts.addRules(defaultAlertRules(options.thresholds ?? {}));
    this.alerts.addSink(sinks.console({ log: (m) => this.logger('warn', m) }));
    if (process.env.AEGIS_ALERT_WEBHOOK) {
      this.alerts.addSink(sinks.webhook(process.env.AEGIS_ALERT_WEBHOOK));
    }
    for (const trigger of defaultTriggers({ logger: this.logger, ...(options.thresholds ?? {}) })) {
      this.triggers.register(trigger);
    }
  }

  _wire() {
    this.stream.on('event', (event) => {
      this._handleEvent(event).catch((error) => {
        this.logger('error', `pipeline error: ${error.message}`);
        this.emit('error', error);
      });
    });

    this.stream.on('error', (error) => {
      this.logger('error', `stream: ${error.message}`);
      this.emit('stream-error', error);
    });

    this.stream.on('transport', (transport) => {
      this.logger('info', `transport is now: ${transport}`);
      this.dashboard?.broadcast('transport', transport);
      this.emit('transport', transport);
    });

    this.stream.on('reconnect', (info) =>
      this.dashboard?.broadcast('reconnect', info));

    this.alerts.on('alert', (alert) => {
      this.dashboard?.broadcast('alert', alert);
      this.emit('alert', alert);
    });

    this.triggers.on('fired', (info) => this.emit('trigger', info));
  }

  /** The full per-event pipeline. Exposed so tests can drive it directly. */
  async _handleEvent(event) {
    this.processed += 1;

    await this.store.append(event);
    this.analytics.record(event);

    const routes = await this.router.dispatch(event, { monitor: this });
    const alerts = await this.alerts.process(event);
    const fired = await this.triggers.process(event, { monitor: this });

    this.dashboard?.broadcast('event', serializeEvent(event));
    this.emit('event', event, { routes, alerts, triggers: fired });

    return { routes, alerts, triggers: fired };
  }

  /** Ingest a raw RPC event object through the whole pipeline (test/simulator). */
  async ingestRaw(rawEvent) {
    this.stream.injectRaw(rawEvent);
  }

  async start({ dashboard = true, stream = true } = {}) {
    if (this.started) return this;
    await this.store.init();

    // Resume exactly where the previous process stopped.
    const checkpoint = await this.store.loadCheckpoint();
    if (checkpoint?.cursor) {
      this.stream.cursor = checkpoint.cursor;
      this.logger('info', `resuming from cursor ${checkpoint.cursor}`);
    }

    if (dashboard && this.config.dashboard.enabled) {
      this.dashboard = new DashboardServer({
        stream: this.stream,
        router: this.router,
        alerts: this.alerts,
        store: this.store,
        triggers: this.triggers,
        analytics: this.analytics,
        config: this.config,
        logger: this.logger,
      });
      await this.dashboard.start();
    }

    if (stream) {
      const transport = await this.stream.start();
      this.logger('info', `event stream started on transport: ${transport}`);
    }

    // Absence rules need a clock, not an event.
    this._absenceTimer = setInterval(() => {
      this.alerts.checkAbsence().catch((error) =>
        this.logger('error', `absence check failed: ${error.message}`));
    }, 30_000);
    if (typeof this._absenceTimer.unref === 'function') this._absenceTimer.unref();

    this._checkpointTimer = setInterval(() => {
      if (this.stream.cursor) {
        this.store
          .saveCheckpoint(this.stream.cursor, this.stream.lastLedger)
          .catch((error) => this.logger('error', `checkpoint failed: ${error.message}`));
      }
    }, 5_000);
    if (typeof this._checkpointTimer.unref === 'function') this._checkpointTimer.unref();

    this.started = true;
    return this;
  }

  async stop() {
    this.started = false;
    if (this._absenceTimer) clearInterval(this._absenceTimer);
    if (this._checkpointTimer) clearInterval(this._checkpointTimer);
    this.triggers.dispose();
    await this.stream.stop();
    if (this.dashboard) await this.dashboard.stop();
    if (this.stream.cursor) {
      await this.store.saveCheckpoint(this.stream.cursor, this.stream.lastLedger).catch(() => {});
    }
    await this.store.close();
  }

  /** Replay persisted history back through the live pipeline. */
  async replay({ filter = null, limit = Infinity, speed = 0, throughPipeline = false } = {}) {
    const handler = throughPipeline
      ? (event) => this._handleEvent(event)
      : (event) => {
          this.dashboard?.broadcast('replay', serializeEvent(event));
        };
    return this.store.replay(handler, { filter, limit, speed });
  }

  getStats() {
    return {
      processed: this.processed,
      stream: this.stream.getStats(),
      router: this.router.getStats(),
      alerts: this.alerts.getStats(),
      triggers: this.triggers.getStats(),
      store: this.store.getStats(),
    };
  }
}

export { TRANSPORT };
