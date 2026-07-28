/**
 * Event-based triggers - execute automated actions when events match a pattern.
 *
 * A trigger couples a filter to an action with execution guarantees the raw
 * router does not provide:
 *
 *  - `once`        : fire at most one time
 *  - `debounceMs`  : collapse bursts, firing after quiet time
 *  - `throttleMs`  : fire at most once per interval
 *  - `maxRuns`     : hard cap on executions
 *  - `retries`     : retry a failing action with backoff
 *  - `enabled`     : toggle at runtime without unregistering
 *
 * Built-in action factories cover the common cases (webhook, log, collect,
 * chain-to-another-trigger); custom actions are just async functions.
 */

import { EventEmitter } from 'node:events';
import { matchesFilter } from '../events/filter.js';
import { serializeEvent } from '../events/normalize.js';

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export class TriggerEngine extends EventEmitter {
  constructor({ logger = () => {}, now = () => Date.now() } = {}) {
    super();
    this.triggers = new Map();
    this.logger = logger;
    this.now = now;
    this.stats = { evaluated: 0, fired: 0, skipped: 0, failed: 0 };
    this.history = [];
    this.historyLimit = 200;
  }

  /**
   * @param {object} spec
   * @param {string} spec.name
   * @param {object} spec.filter
   * @param {Function} spec.action async (event, ctx) => any
   * @param {boolean} [spec.once]
   * @param {number} [spec.debounceMs]
   * @param {number} [spec.throttleMs]
   * @param {number} [spec.maxRuns]
   * @param {number} [spec.retries]
   * @param {number} [spec.retryDelayMs]
   * @param {boolean} [spec.enabled]
   */
  register(spec) {
    if (!spec?.name) throw new TypeError('trigger.name is required');
    if (typeof spec.action !== 'function') throw new TypeError('trigger.action must be a function');

    this.triggers.set(spec.name, {
      enabled: true,
      once: false,
      debounceMs: 0,
      throttleMs: 0,
      maxRuns: Infinity,
      retries: 0,
      retryDelayMs: 250,
      filter: {},
      description: null,
      ...spec,
      _state: { runs: 0, lastRunAt: 0, debounceTimer: null, lastError: null },
    });
    return this;
  }

  unregister(name) {
    const trigger = this.triggers.get(name);
    if (trigger?._state.debounceTimer) clearTimeout(trigger._state.debounceTimer);
    return this.triggers.delete(name);
  }

  enable(name, enabled = true) {
    const trigger = this.triggers.get(name);
    if (!trigger) return false;
    trigger.enabled = enabled;
    return true;
  }

  list() {
    return [...this.triggers.values()].map((t) => ({
      name: t.name,
      enabled: t.enabled,
      runs: t._state.runs,
      lastRunAt: t._state.lastRunAt || null,
      once: t.once,
      debounceMs: t.debounceMs,
      throttleMs: t.throttleMs,
      maxRuns: t.maxRuns === Infinity ? null : t.maxRuns,
      description: t.description,
      lastError: t._state.lastError,
    }));
  }

  /** Evaluate an event against all triggers; returns names that fired. */
  async process(event, context = {}) {
    this.stats.evaluated += 1;
    const fired = [];

    for (const trigger of this.triggers.values()) {
      if (!trigger.enabled) continue;
      if (trigger._state.runs >= trigger.maxRuns) continue;
      if (trigger.once && trigger._state.runs >= 1) continue;

      let isMatch = false;
      try {
        isMatch = matchesFilter(event, trigger.filter);
      } catch (error) {
        this.logger('error', `Trigger ${trigger.name} filter threw: ${error.message}`);
        continue;
      }
      if (!isMatch) continue;

      const now = this.now();
      if (trigger.throttleMs && now - trigger._state.lastRunAt < trigger.throttleMs) {
        this.stats.skipped += 1;
        this.emit('skipped', { trigger: trigger.name, reason: 'throttled' });
        continue;
      }

      if (trigger.debounceMs) {
        if (trigger._state.debounceTimer) clearTimeout(trigger._state.debounceTimer);
        trigger._state.debounceTimer = setTimeout(() => {
          trigger._state.debounceTimer = null;
          this._execute(trigger, event, context).catch(() => {});
        }, trigger.debounceMs);
        if (typeof trigger._state.debounceTimer.unref === 'function') {
          trigger._state.debounceTimer.unref();
        }
        this.emit('debounced', { trigger: trigger.name });
        continue;
      }

      const ok = await this._execute(trigger, event, context);
      if (ok) fired.push(trigger.name);
    }

    return fired;
  }

  async _execute(trigger, event, context) {
    const attempts = trigger.retries + 1;
    let lastError = null;

    for (let attempt = 1; attempt <= attempts; attempt++) {
      try {
        const result = await trigger.action(event, { ...context, trigger: trigger.name, attempt });
        trigger._state.runs += 1;
        trigger._state.lastRunAt = this.now();
        trigger._state.lastError = null;
        this.stats.fired += 1;

        const record = {
          trigger: trigger.name,
          ts: trigger._state.lastRunAt,
          eventId: event?.id ?? null,
          attempt,
          ok: true,
          result: typeof result === 'object' ? undefined : result,
        };
        this._pushHistory(record);
        this.emit('fired', { ...record, event: event ? serializeEvent(event) : null });
        return true;
      } catch (error) {
        lastError = error;
        if (attempt < attempts) await sleep(trigger.retryDelayMs * attempt);
      }
    }

    trigger._state.lastError = lastError?.message ?? String(lastError);
    this.stats.failed += 1;
    this._pushHistory({
      trigger: trigger.name,
      ts: this.now(),
      eventId: event?.id ?? null,
      ok: false,
      error: trigger._state.lastError,
    });
    this.logger('error', `Trigger ${trigger.name} failed: ${trigger._state.lastError}`);
    this.emit('failed', { trigger: trigger.name, error: trigger._state.lastError });
    return false;
  }

  _pushHistory(record) {
    this.history.push(record);
    if (this.history.length > this.historyLimit) this.history.shift();
  }

  getHistory(limit = 50) {
    return this.history.slice(-limit).reverse();
  }

  getStats() {
    return { ...this.stats, triggerCount: this.triggers.size };
  }

  /** Clear pending debounce timers (call on shutdown). */
  dispose() {
    for (const trigger of this.triggers.values()) {
      if (trigger._state.debounceTimer) {
        clearTimeout(trigger._state.debounceTimer);
        trigger._state.debounceTimer = null;
      }
    }
  }
}

/** Ready-made trigger actions. */
export const actions = {
  log(logger = console) {
    return (event) => {
      logger.log(
        `[TRIGGER] ${event.action ?? event.type} ledger=${event.ledger} contract=${event.contractId ?? 'n/a'}`,
      );
    };
  },

  webhook(url, { fetchImpl = globalThis.fetch, timeoutMs = 5000, headers = {} } = {}) {
    return async (event, ctx) => {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const response = await fetchImpl(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', ...headers },
          body: JSON.stringify({ trigger: ctx.trigger, event: serializeEvent(event) }),
          signal: controller.signal,
        });
        if (response && response.ok === false) {
          throw new Error(`Webhook responded ${response.status}`);
        }
        return true;
      } finally {
        clearTimeout(timer);
      }
    };
  },

  collect(target = []) {
    return (event) => {
      target.push(event);
      return target.length;
    };
  },
};
