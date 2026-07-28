/**
 * Pattern-based alerting engine.
 *
 * Supported rule patterns:
 *
 *  - `match`      : fire whenever an event matches the filter
 *  - `threshold`  : fire when a numeric field crosses a bound
 *  - `rate`       : fire when N matching events occur within a rolling window
 *  - `sequence`   : fire when an ordered chain of filters is observed within a
 *                   window (optionally correlated by a shared key, e.g. address)
 *  - `absence`    : fire when NO matching event is seen for `withinMs`
 *
 * Each rule can declare `severity`, `cooldownMs` (anti-spam) and arbitrary
 * `metadata`. Alerts are emitted on the 'alert' event and pushed to any number
 * of registered sinks (console, webhook, custom).
 */

import { EventEmitter } from 'node:events';
import { matchesFilter } from '../events/filter.js';
import { amountOf, serializeEvent } from '../events/normalize.js';

export const SEVERITY = {
  INFO: 'info',
  WARNING: 'warning',
  CRITICAL: 'critical',
};

const SEVERITY_RANK = { info: 10, warning: 20, critical: 30 };

function toBigInt(value) {
  if (value == null) return null;
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number') return BigInt(Math.trunc(value));
  if (typeof value === 'string' && /^-?\d+$/.test(value)) return BigInt(value);
  return null;
}

/** Pull a comparable numeric out of an event for threshold rules. */
function numericField(event, field) {
  if (!field || field === 'amount') return amountOf(event);
  const direct = event.fields?.[field] ?? event[field];
  return toBigInt(direct);
}

let alertSeq = 0;

export class AlertEngine extends EventEmitter {
  constructor({ logger = () => {}, now = () => Date.now(), historyLimit = 500 } = {}) {
    super();
    this.rules = new Map();
    this.sinks = [];
    this.logger = logger;
    this.now = now;
    this.historyLimit = historyLimit;
    this.history = [];
    this.stats = { evaluated: 0, fired: 0, suppressed: 0 };
    this._absenceTimer = null;
  }

  /**
   * @param {object} rule
   * @param {string} rule.name
   * @param {'match'|'threshold'|'rate'|'sequence'|'absence'} rule.pattern
   * @param {object} [rule.filter]
   * @param {string} [rule.severity]
   * @param {number} [rule.cooldownMs]
   * @param {string} [rule.message]
   */
  addRule(rule) {
    if (!rule?.name) throw new TypeError('rule.name is required');
    const pattern = rule.pattern ?? 'match';
    const normalized = {
      severity: SEVERITY.WARNING,
      cooldownMs: 0,
      filter: {},
      metadata: {},
      ...rule,
      pattern,
      _state: {
        lastFiredAt: 0,
        window: [],
        sequences: new Map(),
        lastSeenAt: this.now(),
        fired: 0,
      },
    };

    if (pattern === 'rate') {
      normalized.count = rule.count ?? 5;
      normalized.windowMs = rule.windowMs ?? 60_000;
    }
    if (pattern === 'threshold') {
      normalized.field = rule.field ?? 'amount';
      normalized.gt = toBigInt(rule.gt);
      normalized.gte = toBigInt(rule.gte);
      normalized.lt = toBigInt(rule.lt);
      normalized.lte = toBigInt(rule.lte);
    }
    if (pattern === 'sequence') {
      normalized.steps = rule.steps ?? [];
      normalized.windowMs = rule.windowMs ?? 300_000;
      normalized.correlateBy = rule.correlateBy ?? null;
      if (!normalized.steps.length) throw new TypeError('sequence rule requires steps');
    }
    if (pattern === 'absence') {
      normalized.withinMs = rule.withinMs ?? 300_000;
    }

    this.rules.set(rule.name, normalized);
    return this;
  }

  addRules(rules = []) {
    for (const rule of rules) this.addRule(rule);
    return this;
  }

  removeRule(name) {
    return this.rules.delete(name);
  }

  listRules() {
    return [...this.rules.values()].map((r) => ({
      name: r.name,
      pattern: r.pattern,
      severity: r.severity,
      fired: r._state.fired,
      cooldownMs: r.cooldownMs,
      description: r.description ?? null,
    }));
  }

  /** Register an alert sink: async (alert) => void */
  addSink(sink) {
    if (typeof sink !== 'function') throw new TypeError('sink must be a function');
    this.sinks.push(sink);
    return this;
  }

  _correlationKey(rule, event) {
    if (!rule.correlateBy) return '__global__';
    if (rule.correlateBy === 'address') return event.subjects[0] ?? '__none__';
    return String(event.fields?.[rule.correlateBy] ?? event[rule.correlateBy] ?? '__none__');
  }

  /** Evaluate one event against every rule. Returns fired alerts. */
  async process(event) {
    this.stats.evaluated += 1;
    const fired = [];

    for (const rule of this.rules.values()) {
      let alert = null;
      try {
        switch (rule.pattern) {
          case 'match':
            alert = this._evalMatch(rule, event);
            break;
          case 'threshold':
            alert = this._evalThreshold(rule, event);
            break;
          case 'rate':
            alert = this._evalRate(rule, event);
            break;
          case 'sequence':
            alert = this._evalSequence(rule, event);
            break;
          case 'absence':
            if (matchesFilter(event, rule.filter)) rule._state.lastSeenAt = this.now();
            break;
          default:
            this.logger('warn', `Unknown alert pattern: ${rule.pattern}`);
        }
      } catch (error) {
        this.logger('error', `Alert rule ${rule.name} threw: ${error.message}`);
      }

      if (alert) {
        const emitted = await this._fire(rule, alert, event);
        if (emitted) fired.push(emitted);
      }
    }

    return fired;
  }

  _evalMatch(rule, event) {
    if (!matchesFilter(event, rule.filter)) return null;
    return { reason: 'match', details: {} };
  }

  _evalThreshold(rule, event) {
    if (!matchesFilter(event, rule.filter)) return null;
    const value = numericField(event, rule.field);
    if (value == null) return null;
    const checks = [
      rule.gt != null && value > rule.gt && `${value} > ${rule.gt}`,
      rule.gte != null && value >= rule.gte && `${value} >= ${rule.gte}`,
      rule.lt != null && value < rule.lt && `${value} < ${rule.lt}`,
      rule.lte != null && value <= rule.lte && `${value} <= ${rule.lte}`,
    ].filter(Boolean);
    if (!checks.length) return null;
    return {
      reason: 'threshold',
      details: { field: rule.field, value: value.toString(), checks },
    };
  }

  _evalRate(rule, event) {
    if (!matchesFilter(event, rule.filter)) return null;
    const now = this.now();
    const state = rule._state;
    state.window.push(now);
    const cutoff = now - rule.windowMs;
    while (state.window.length && state.window[0] < cutoff) state.window.shift();
    if (state.window.length < rule.count) return null;
    return {
      reason: 'rate',
      details: {
        observed: state.window.length,
        threshold: rule.count,
        windowMs: rule.windowMs,
      },
    };
  }

  _evalSequence(rule, event) {
    const now = this.now();
    const key = this._correlationKey(rule, event);
    const state = rule._state;
    let progress = state.sequences.get(key);
    if (!progress || now - progress.startedAt > rule.windowMs) {
      progress = { index: 0, startedAt: now, events: [] };
    }

    const expected = rule.steps[progress.index];
    if (matchesFilter(event, expected)) {
      progress.index += 1;
      progress.events.push(event.id);
      if (progress.index === 1) progress.startedAt = now;

      if (progress.index >= rule.steps.length) {
        state.sequences.delete(key);
        return {
          reason: 'sequence',
          details: {
            correlationKey: key,
            steps: rule.steps.length,
            eventIds: progress.events,
            elapsedMs: now - progress.startedAt,
          },
        };
      }
      state.sequences.set(key, progress);
    } else if (progress.index > 0) {
      state.sequences.set(key, progress);
    }
    return null;
  }

  /**
   * Evaluate 'absence' rules. Call periodically (the service does this on a
   * timer); returns any alerts fired.
   */
  async checkAbsence() {
    const now = this.now();
    const fired = [];
    for (const rule of this.rules.values()) {
      if (rule.pattern !== 'absence') continue;
      const idle = now - rule._state.lastSeenAt;
      if (idle >= rule.withinMs) {
        const alert = await this._fire(
          rule,
          { reason: 'absence', details: { idleMs: idle, withinMs: rule.withinMs } },
          null,
        );
        if (alert) {
          rule._state.lastSeenAt = now; // restart the clock after firing
          fired.push(alert);
        }
      }
    }
    return fired;
  }

  async _fire(rule, payload, event) {
    const now = this.now();
    if (rule.cooldownMs && now - rule._state.lastFiredAt < rule.cooldownMs) {
      this.stats.suppressed += 1;
      return null;
    }
    rule._state.lastFiredAt = now;
    rule._state.fired += 1;
    this.stats.fired += 1;
    alertSeq += 1;

    const alert = {
      id: `alert-${now}-${alertSeq}`,
      rule: rule.name,
      pattern: rule.pattern,
      severity: rule.severity,
      severityRank: SEVERITY_RANK[rule.severity] ?? 0,
      message: typeof rule.message === 'function' ? rule.message(event, payload) : rule.message ?? defaultMessage(rule, payload, event),
      reason: payload.reason,
      details: payload.details,
      metadata: rule.metadata,
      ts: now,
      event: event ? serializeEvent(event) : null,
    };

    this.history.push(alert);
    if (this.history.length > this.historyLimit) this.history.shift();

    this.emit('alert', alert);
    for (const sink of this.sinks) {
      try {
        await sink(alert);
      } catch (error) {
        this.logger('error', `Alert sink failed: ${error.message}`);
      }
    }
    return alert;
  }

  getHistory({ limit = 50, severity = null } = {}) {
    let out = this.history;
    if (severity) out = out.filter((a) => a.severity === severity);
    return out.slice(-limit).reverse();
  }

  getStats() {
    return { ...this.stats, ruleCount: this.rules.size };
  }
}

function defaultMessage(rule, payload, event) {
  const who = event?.action ? `${event.action}` : rule.pattern;
  switch (payload.reason) {
    case 'threshold':
      return `[${rule.name}] ${who} ${payload.details.field}=${payload.details.value} crossed threshold (${payload.details.checks.join(', ')})`;
    case 'rate':
      return `[${rule.name}] ${payload.details.observed} matching events in ${payload.details.windowMs}ms (limit ${payload.details.threshold})`;
    case 'sequence':
      return `[${rule.name}] sequence of ${payload.details.steps} steps completed for ${payload.details.correlationKey}`;
    case 'absence':
      return `[${rule.name}] no matching activity for ${payload.details.idleMs}ms`;
    default:
      return `[${rule.name}] ${who} event matched`;
  }
}

/** Built-in sinks. */
export const sinks = {
  console(logger = console) {
    return (alert) => {
      const tag = alert.severity.toUpperCase();
      logger.log(`[ALERT/${tag}] ${alert.message}`);
    };
  },

  webhook(url, { fetchImpl = globalThis.fetch, timeoutMs = 5000 } = {}) {
    return async (alert) => {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        await fetchImpl(url, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(alert),
          signal: controller.signal,
        });
      } finally {
        clearTimeout(timer);
      }
    };
  },

  collect(target = []) {
    return (alert) => {
      target.push(alert);
    };
  },
};
