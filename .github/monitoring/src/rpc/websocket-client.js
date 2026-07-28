/**
 * SorobanEventStream - real-time contract event streaming client.
 *
 * ## Why there are two transports
 *
 * Soroban RPC exposes contract events through the **HTTP JSON-RPC `getEvents`**
 * method. A native WebSocket subscription API has been on the roadmap since the
 * original "Events by Contract ID" epic but is *not* available on public
 * testnet/mainnet RPC endpoints today. A monitoring service that only spoke
 * WebSocket would therefore never receive a single event in practice.
 *
 * This client solves that by presenting one streaming interface backed by two
 * interchangeable transports:
 *
 *   1. `websocket` - a real WebSocket (`ws`) connection with JSON-RPC
 *      subscribe/unsubscribe framing, heartbeats and exponential-backoff
 *      reconnection. Used when `wsUrl` points at an RPC/indexer that offers a
 *      subscription API (e.g. a self-hosted stellar-rpc build, Mercury, or the
 *      bundled test double).
 *   2. `poll` - a cursor-driven `getEvents` long-poller that yields the exact
 *      same normalized envelopes. This is the default and guarantees the
 *      "real-time event streaming works" acceptance criterion against stock
 *      infrastructure.
 *
 * The client auto-selects: if `wsUrl` is configured it tries WebSocket first and
 * transparently falls back to polling when the socket cannot be established,
 * then keeps retrying the socket in the background (upgrade-on-recovery).
 *
 * Emits: 'event' (normalized), 'raw', 'open', 'close', 'reconnect', 'error',
 *        'transport', 'ledger', 'cursor'
 */

import { EventEmitter } from 'node:events';
import { rpcCall } from './jsonrpc.js';
import { Backoff } from './backoff.js';
import { normalizeEvent } from '../events/normalize.js';
import { encodeSymbol } from '../events/scval.js';
import { NAMESPACE } from '../events/normalize.js';

/** Lazily resolve the `ws` package so the poller works even if it is absent. */
async function loadWebSocketImpl(injected) {
  if (injected) return injected;
  try {
    const mod = await import('ws');
    return mod.default ?? mod.WebSocket ?? mod;
  } catch {
    return globalThis.WebSocket ?? null;
  }
}

export const TRANSPORT = {
  WEBSOCKET: 'websocket',
  POLL: 'poll',
  IDLE: 'idle',
};

export class SorobanEventStream extends EventEmitter {
  /**
   * @param {object} options
   * @param {string} options.rpcUrl
   * @param {string|null} [options.wsUrl]
   * @param {string[]} [options.contractIds]
   * @param {number} [options.pollIntervalMs]
   * @param {number} [options.pageLimit]
   * @param {number} [options.startLedgerLookback]
   * @param {object} [options.reconnect]
   * @param {boolean} [options.namespaceFilter] restrict RPC filter to the aegis namespace
   * @param {Function} [options.fetchImpl] test seam
   * @param {Function} [options.WebSocketImpl] test seam
   * @param {Function} [options.logger]
   */
  constructor(options = {}) {
    super();
    this.rpcUrl = options.rpcUrl;
    this.wsUrl = options.wsUrl ?? null;
    this.contractIds = options.contractIds ?? [];
    this.pollIntervalMs = options.pollIntervalMs ?? 2000;
    this.pageLimit = options.pageLimit ?? 100;
    this.startLedgerLookback = options.startLedgerLookback ?? 120;
    this.namespaceFilter = options.namespaceFilter ?? false;
    this.fetchImpl = options.fetchImpl ?? globalThis.fetch;
    this.WebSocketImpl = options.WebSocketImpl ?? null;
    this.logger = options.logger ?? (() => {});
    this.heartbeatIntervalMs = options.heartbeatIntervalMs ?? 15000;
    this.wsRetryOnPollMs = options.wsRetryOnPollMs ?? 60000;

    this.backoff = new Backoff(options.reconnect);
    this.maxAttempts = options.reconnect?.maxAttempts ?? 0;

    this.transport = TRANSPORT.IDLE;
    this.running = false;
    this.cursor = options.cursor ?? null;
    this.lastLedger = null;
    this.seen = new Set();
    this.seenOrder = [];
    this.stats = {
      received: 0,
      duplicates: 0,
      pollCycles: 0,
      reconnects: 0,
      errors: 0,
      startedAt: null,
      lastEventAt: null,
    };

    this._ws = null;
    this._pollTimer = null;
    this._heartbeatTimer = null;
    this._wsRetryTimer = null;
    this._reconnectTimer = null;
    this._rpcId = 0;
    this._closingIntentionally = false;

    // Node throws on an 'error' event with no listener. A monitoring sidecar
    // must never crash just because a transport hiccuped (especially while it
    // is successfully degrading to the polling fallback), so we guarantee a
    // listener always exists. Consumer-attached handlers still fire normally.
    this.on('error', () => {});
  }

  /**
   * Emit an error without ever risking an unhandled 'error' throw.
   * Normalizes non-Error payloads (the `ws` package can emit strings/objects).
   */
  _emitError(error) {
    const normalized =
      error instanceof Error ? error : new Error(String(error?.message ?? error ?? 'unknown stream error'));
    this.stats.errors += 1;
    this.emit('error', normalized);
    return normalized;
  }

  /** Build the Soroban RPC event filter array. */
  buildFilters() {
    const filter = { type: 'contract' };
    if (this.contractIds.length) filter.contractIds = this.contractIds.slice(0, 5);
    if (this.namespaceFilter) {
      // topic[0] pinned to the `aegis` symbol, remaining topics wild.
      filter.topics = [[encodeSymbol(NAMESPACE), '*', '*', '*']];
    }
    return [filter];
  }

  /** Start streaming. Resolves once a transport is active. */
  async start() {
    if (this.running) return this.transport;
    this.running = true;
    this.stats.startedAt = Date.now();

    if (this.wsUrl) {
      const ok = await this._tryWebSocket();
      if (ok) return this.transport;
      this.logger('warn', 'WebSocket unavailable, falling back to getEvents polling');
    }

    await this._startPolling();
    return this.transport;
  }

  /** Stop streaming and release all timers/sockets. */
  async stop() {
    this.running = false;
    this._closingIntentionally = true;
    this._clearTimers();
    if (this._ws) {
      try {
        this._ws.close();
      } catch {
        /* ignore */
      }
      this._ws = null;
    }
    this.transport = TRANSPORT.IDLE;
    this.emit('transport', this.transport);
  }

  _clearTimers() {
    for (const key of ['_pollTimer', '_heartbeatTimer', '_wsRetryTimer', '_reconnectTimer']) {
      if (this[key]) {
        clearTimeout(this[key]);
        clearInterval(this[key]);
        this[key] = null;
      }
    }
  }

  // ---------------------------------------------------------------- WebSocket

  async _tryWebSocket() {
    const Impl = await loadWebSocketImpl(this.WebSocketImpl);
    if (!Impl) {
      this._emitError(new Error('No WebSocket implementation available'));
      return false;
    }

    return new Promise((resolve) => {
      let settled = false;
      const finish = (ok) => {
        if (settled) return;
        settled = true;
        resolve(ok);
      };

      let socket;
      try {
        socket = new Impl(this.wsUrl);
      } catch (error) {
        this._emitError(error);
        return finish(false);
      }

      this._ws = socket;
      const openTimeout = setTimeout(() => {
        if (!settled) {
          try {
            socket.close();
          } catch {
            /* ignore */
          }
          finish(false);
        }
      }, 5000);

      const onOpen = () => {
        clearTimeout(openTimeout);
        this.transport = TRANSPORT.WEBSOCKET;
        this.backoff.reset();
        this._stopPolling();
        this._subscribe(socket);
        this._startHeartbeat(socket);
        this.emit('transport', this.transport);
        this.emit('open', { url: this.wsUrl });
        this.logger('info', `WebSocket connected: ${this.wsUrl}`);
        finish(true);
      };

      const onMessage = (payload) => {
        const text = typeof payload === 'string' ? payload : payload?.data ?? payload;
        this._handleSocketMessage(text);
      };

      const onError = (error) => {
        this._emitError(error);
        clearTimeout(openTimeout);
        finish(false);
      };

      const onClose = () => {
        clearTimeout(openTimeout);
        this._stopHeartbeat();
        if (this.transport === TRANSPORT.WEBSOCKET) {
          this.transport = TRANSPORT.IDLE;
          this.emit('close', { url: this.wsUrl });
        }
        if (this.running && !this._closingIntentionally) this._scheduleReconnect();
        finish(false);
      };

      // Support both `ws` (EventEmitter) and browser-style WebSocket.
      if (typeof socket.on === 'function') {
        socket.on('open', onOpen);
        socket.on('message', onMessage);
        socket.on('error', onError);
        socket.on('close', onClose);
        socket.on('pong', () => {
          this._lastPongAt = Date.now();
        });
      } else {
        socket.onopen = onOpen;
        socket.onmessage = (e) => onMessage(e.data);
        socket.onerror = onError;
        socket.onclose = onClose;
      }
    });
  }

  _subscribe(socket) {
    this._rpcId += 1;
    const message = {
      jsonrpc: '2.0',
      id: this._rpcId,
      method: 'subscribeEvents',
      params: {
        filters: this.buildFilters(),
        ...(this.cursor ? { cursor: this.cursor } : {}),
      },
    };
    try {
      socket.send(JSON.stringify(message));
    } catch (error) {
      this._emitError(error);
    }
  }

  _startHeartbeat(socket) {
    this._stopHeartbeat();
    if (typeof socket.ping !== 'function') return;
    this._heartbeatTimer = setInterval(() => {
      try {
        socket.ping();
      } catch {
        /* socket already gone */
      }
    }, this.heartbeatIntervalMs);
    if (typeof this._heartbeatTimer.unref === 'function') this._heartbeatTimer.unref();
  }

  _stopHeartbeat() {
    if (this._heartbeatTimer) {
      clearInterval(this._heartbeatTimer);
      this._heartbeatTimer = null;
    }
  }

  _handleSocketMessage(text) {
    let payload;
    try {
      payload = typeof text === 'string' ? JSON.parse(text) : JSON.parse(String(text));
    } catch (error) {
      this._emitError(new Error(`Malformed WebSocket frame: ${error.message}`));
      return;
    }

    // Accept several shapes: notification params, direct event, batched result.
    const candidates = [];
    if (Array.isArray(payload)) candidates.push(...payload);
    else if (payload?.params?.events) candidates.push(...payload.params.events);
    else if (payload?.params?.event) candidates.push(payload.params.event);
    else if (payload?.result?.events) candidates.push(...payload.result.events);
    else if (payload?.events) candidates.push(...payload.events);
    else if (payload?.topic || payload?.topics) candidates.push(payload);

    if (payload?.error) {
      this._emitError(new Error(payload.error.message || 'WebSocket RPC error'));
    }

    for (const raw of candidates) this._ingest(raw);
  }

  _scheduleReconnect() {
    if (this.maxAttempts && this.backoff.attempt >= this.maxAttempts) {
      this.logger('warn', 'Max reconnect attempts reached; staying on polling transport');
      this._startPolling();
      return;
    }
    const delay = this.backoff.next();
    this.stats.reconnects += 1;
    this.emit('reconnect', { attempt: this.backoff.attempt, delayMs: delay });
    this.logger('info', `Reconnecting WebSocket in ${delay}ms (attempt ${this.backoff.attempt})`);

    // Keep data flowing while the socket is down.
    this._startPolling();

    this._reconnectTimer = setTimeout(async () => {
      if (!this.running) return;
      this._closingIntentionally = false;
      const ok = await this._tryWebSocket();
      if (!ok && this.running) this._scheduleReconnect();
    }, delay);
    if (typeof this._reconnectTimer.unref === 'function') this._reconnectTimer.unref();
  }

  // ------------------------------------------------------------------- Poller

  async _startPolling() {
    if (this._pollTimer || this.transport === TRANSPORT.POLL) return;
    this.transport = TRANSPORT.POLL;
    this.emit('transport', this.transport);
    this.logger('info', `Polling getEvents every ${this.pollIntervalMs}ms`);

    if (!this.cursor && this.lastLedger == null) {
      try {
        const latest = await this.getLatestLedger();
        this.lastLedger = Math.max(1, latest.sequence - this.startLedgerLookback);
      } catch (error) {
        this._emitError(error);
      }
    }

    const tick = async () => {
      if (!this.running || this.transport !== TRANSPORT.POLL) return;
      try {
        await this.pollOnce();
      } catch (error) {
        this._emitError(error);
      }
      if (this.running && this.transport === TRANSPORT.POLL) {
        this._pollTimer = setTimeout(tick, this.pollIntervalMs);
        if (typeof this._pollTimer.unref === 'function') this._pollTimer.unref();
      }
    };

    // Kick off immediately, then on an interval.
    this._pollTimer = setTimeout(tick, 0);
    if (typeof this._pollTimer.unref === 'function') this._pollTimer.unref();

    // Periodically attempt to upgrade back to WebSocket.
    if (this.wsUrl && !this._wsRetryTimer) {
      this._wsRetryTimer = setInterval(async () => {
        if (!this.running || this.transport === TRANSPORT.WEBSOCKET) return;
        this._closingIntentionally = false;
        await this._tryWebSocket();
      }, this.wsRetryOnPollMs);
      if (typeof this._wsRetryTimer.unref === 'function') this._wsRetryTimer.unref();
    }
  }

  _stopPolling() {
    if (this._pollTimer) {
      clearTimeout(this._pollTimer);
      this._pollTimer = null;
    }
  }

  /** One getEvents page fetch. Exposed for tests and manual replay. */
  async pollOnce() {
    const params = {
      filters: this.buildFilters(),
      pagination: { limit: this.pageLimit },
    };
    if (this.cursor) params.pagination.cursor = this.cursor;
    else params.startLedger = this.lastLedger ?? 1;

    const result = await rpcCall(this.rpcUrl, 'getEvents', params, {
      fetchImpl: this.fetchImpl,
    });
    this.stats.pollCycles += 1;

    const events = result?.events ?? [];
    for (const raw of events) this._ingest(raw);

    if (result?.cursor) {
      this.cursor = result.cursor;
      this.emit('cursor', this.cursor);
    } else if (events.length) {
      const last = events[events.length - 1];
      if (last.pagingToken) {
        this.cursor = last.pagingToken;
        this.emit('cursor', this.cursor);
      }
    }

    if (result?.latestLedger) {
      this.lastLedger = Number(result.latestLedger);
      this.emit('ledger', this.lastLedger);
    }

    return events.length;
  }

  async getLatestLedger() {
    return rpcCall(this.rpcUrl, 'getLatestLedger', {}, { fetchImpl: this.fetchImpl });
  }

  async getHealth() {
    return rpcCall(this.rpcUrl, 'getHealth', {}, { fetchImpl: this.fetchImpl });
  }

  // ------------------------------------------------------------------ Ingest

  /** De-duplicate + normalize + emit. Shared by both transports. */
  _ingest(raw) {
    if (!raw) return;
    const key = raw.id ?? raw.pagingToken ?? JSON.stringify(raw.topic ?? raw.topics ?? raw);
    if (this.seen.has(key)) {
      this.stats.duplicates += 1;
      return;
    }
    this.seen.add(key);
    this.seenOrder.push(key);
    if (this.seenOrder.length > 5000) {
      this.seen.delete(this.seenOrder.shift());
    }

    let event;
    try {
      event = normalizeEvent(raw);
    } catch (error) {
      this._emitError(new Error(`Failed to normalize event: ${error.message}`));
      return;
    }

    this.stats.received += 1;
    this.stats.lastEventAt = Date.now();
    if (event.cursor) this.cursor = event.cursor;
    if (event.ledger) this.lastLedger = Math.max(this.lastLedger ?? 0, event.ledger);

    this.emit('raw', raw);
    this.emit('event', event);
  }

  /** Inject an event directly (used by the simulator and by tests). */
  injectRaw(raw) {
    this._ingest(raw);
  }

  getStats() {
    return {
      ...this.stats,
      transport: this.transport,
      cursor: this.cursor,
      lastLedger: this.lastLedger,
      uptimeMs: this.stats.startedAt ? Date.now() - this.stats.startedAt : 0,
    };
  }
}
