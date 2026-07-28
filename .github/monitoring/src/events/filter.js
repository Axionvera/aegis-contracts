/**
 * Declarative event filtering + routing.
 *
 * A *filter* is a plain object; every specified clause must match (logical AND),
 * and array values inside a clause behave as "any of" (logical OR).
 *
 *   {
 *     action: ['mint', 'transfer'],       // any of
 *     contractId: 'C...',                 // exact
 *     address: 'G...',                    // matches any subject/from/to/admin
 *     minAmount: 1000n,                   // BigInt-safe comparisons
 *     maxAmount: 5_000n,
 *     ledgerFrom: 100, ledgerTo: 200,
 *     since: 1690000000000,               // ms epoch
 *     successOnly: true,
 *     protocol: 'aegis',
 *     topicMatch: ['aegis', '*', 'G...'], // positional, '*' = wildcard
 *     predicate: (event) => boolean       // escape hatch
 *   }
 */

import { amountOf } from './normalize.js';

function toBigInt(value) {
  if (value == null) return null;
  if (typeof value === 'bigint') return value;
  if (typeof value === 'number') return BigInt(Math.trunc(value));
  if (typeof value === 'string' && /^-?\d+$/.test(value)) return BigInt(value);
  return null;
}

function anyOf(spec, value) {
  if (Array.isArray(spec)) return spec.includes(value);
  return spec === value;
}

/** Positional topic matcher supporting '*' (single) and '**' (rest). */
export function matchTopics(pattern, topics) {
  if (!Array.isArray(pattern)) return true;
  for (let i = 0; i < pattern.length; i++) {
    const p = pattern[i];
    if (p === '**') return true;
    if (p === '*') {
      if (i >= topics.length) return false;
      continue;
    }
    if (topics[i] !== p) return false;
  }
  return true;
}

/**
 * Test a normalized event against a filter spec.
 * An empty/absent filter matches everything.
 */
export function matchesFilter(event, filter) {
  if (!filter || Object.keys(filter).length === 0) return true;

  if (filter.protocol !== undefined && !anyOf(filter.protocol, event.protocol)) return false;
  if (filter.action !== undefined && !anyOf(filter.action, event.action)) return false;
  if (filter.type !== undefined && !anyOf(filter.type, event.type)) return false;
  if (filter.contractId !== undefined && !anyOf(filter.contractId, event.contractId)) return false;
  if (filter.txHash !== undefined && !anyOf(filter.txHash, event.txHash)) return false;

  if (filter.successOnly && event.inSuccessfulContractCall === false) return false;

  if (filter.address !== undefined) {
    const wanted = Array.isArray(filter.address) ? filter.address : [filter.address];
    const pool = new Set([
      ...event.subjects,
      ...Object.values(event.fields || {}).filter((v) => typeof v === 'string'),
    ]);
    if (!wanted.some((a) => pool.has(a))) return false;
  }

  if (filter.from !== undefined && !anyOf(filter.from, event.fields?.from)) return false;
  if (filter.to !== undefined && !anyOf(filter.to, event.fields?.to)) return false;

  const amount = amountOf(event);
  const min = toBigInt(filter.minAmount);
  const max = toBigInt(filter.maxAmount);
  if (min != null) {
    if (amount == null || amount < min) return false;
  }
  if (max != null) {
    if (amount == null || amount > max) return false;
  }

  if (filter.ledgerFrom != null && event.ledger < filter.ledgerFrom) return false;
  if (filter.ledgerTo != null && event.ledger > filter.ledgerTo) return false;

  if (filter.since != null && event.ts < filter.since) return false;
  if (filter.until != null && event.ts > filter.until) return false;

  if (filter.topicMatch && !matchTopics(filter.topicMatch, event.topics)) return false;

  if (typeof filter.predicate === 'function' && !filter.predicate(event)) return false;

  return true;
}

/**
 * EventRouter - registers named routes (filter + handler) and dispatches each
 * event to every matching route. Handler errors are captured per-route so one
 * bad consumer can never stall the stream.
 */
export class EventRouter {
  constructor({ logger = () => {} } = {}) {
    this.routes = new Map();
    this.logger = logger;
    this.stats = { dispatched: 0, matched: 0, handlerErrors: 0 };
  }

  /**
   * @param {string} name unique route name
   * @param {object} filter filter spec (see matchesFilter)
   * @param {Function} handler (event, context) => void | Promise
   * @param {object} [opts] { priority = 0 }
   */
  addRoute(name, filter, handler, opts = {}) {
    if (typeof handler !== 'function') throw new TypeError('handler must be a function');
    this.routes.set(name, {
      name,
      filter: filter ?? {},
      handler,
      priority: opts.priority ?? 0,
      matched: 0,
      errors: 0,
    });
    return this;
  }

  removeRoute(name) {
    return this.routes.delete(name);
  }

  listRoutes() {
    return [...this.routes.values()]
      .sort((a, b) => b.priority - a.priority)
      .map(({ name, filter, priority, matched, errors }) => ({
        name,
        filter: sanitizeFilter(filter),
        priority,
        matched,
        errors,
      }));
  }

  /** Which route names match this event (no side effects). */
  match(event) {
    return [...this.routes.values()]
      .filter((r) => matchesFilter(event, r.filter))
      .sort((a, b) => b.priority - a.priority)
      .map((r) => r.name);
  }

  /** Dispatch an event to all matching routes. Returns matched route names. */
  async dispatch(event, context = {}) {
    this.stats.dispatched += 1;
    const ordered = [...this.routes.values()].sort((a, b) => b.priority - a.priority);
    const matched = [];

    for (const route of ordered) {
      let isMatch = false;
      try {
        isMatch = matchesFilter(event, route.filter);
      } catch (error) {
        route.errors += 1;
        this.stats.handlerErrors += 1;
        this.logger('error', `Route ${route.name} filter threw: ${error.message}`);
        continue;
      }
      if (!isMatch) continue;

      matched.push(route.name);
      route.matched += 1;
      this.stats.matched += 1;

      try {
        await route.handler(event, { ...context, route: route.name });
      } catch (error) {
        route.errors += 1;
        this.stats.handlerErrors += 1;
        this.logger('error', `Route ${route.name} handler failed: ${error.message}`);
      }
    }

    return matched;
  }

  getStats() {
    return { ...this.stats, routeCount: this.routes.size };
  }
}

/** Strip non-serializable members (predicate fns) for API output. */
export function sanitizeFilter(filter) {
  const out = {};
  for (const [k, v] of Object.entries(filter || {})) {
    if (typeof v === 'function') out[k] = '[predicate]';
    else if (typeof v === 'bigint') out[k] = v.toString();
    else out[k] = v;
  }
  return out;
}
