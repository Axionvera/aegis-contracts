/**
 * Append-only event persistence with replay.
 *
 * Storage format is newline-delimited JSON (JSONL): crash-safe by construction,
 * trivially greppable, and streamable without loading the whole file. BigInt
 * values are serialized as decimal strings and rehydrated on read.
 *
 * Features:
 *  - buffered async appends (flush by count or interval)
 *  - in-memory ring buffer for instant queries/replay of recent history
 *  - filtered replay from disk with speed control (instant, or time-scaled)
 *  - cursor checkpointing so a restart resumes exactly where it left off
 */

import { EventEmitter } from 'node:events';
import fs from 'node:fs';
import fsp from 'node:fs/promises';
import path from 'node:path';
import readline from 'node:readline';
import { matchesFilter } from '../events/filter.js';
import { serializeEvent } from '../events/normalize.js';

/** Restore BigInt-ish numeric strings on known amount fields. */
function rehydrate(event) {
  if (!event || typeof event !== 'object') return event;
  const out = { ...event };
  if (out.fields && typeof out.fields === 'object') {
    const fields = { ...out.fields };
    for (const key of ['amount', 'newBalance', 'totalSupply']) {
      const v = fields[key];
      if (typeof v === 'string' && /^-?\d+$/.test(v)) fields[key] = BigInt(v);
    }
    out.fields = fields;
  }
  return out;
}

export class EventStore extends EventEmitter {
  constructor({
    path: storePath = './data/events.jsonl',
    memoryLimit = 10000,
    flushEvery = 25,
    flushIntervalMs = 1000,
    enabled = true,
    logger = () => {},
  } = {}) {
    super();
    this.path = storePath;
    this.memoryLimit = memoryLimit;
    this.flushEvery = flushEvery;
    this.flushIntervalMs = flushIntervalMs;
    this.enabled = enabled;
    this.logger = logger;

    this.buffer = [];
    this.memory = [];
    this.checkpointPath = `${storePath}.checkpoint`;
    this.stats = { appended: 0, flushed: 0, replayed: 0, flushErrors: 0 };
    this._flushTimer = null;
    this._writing = null;
    this._ready = false;
  }

  async init() {
    if (!this.enabled) {
      this._ready = true;
      return this;
    }
    await fsp.mkdir(path.dirname(path.resolve(this.path)), { recursive: true });
    // Touch the file so readers never race a missing path.
    await fsp.appendFile(this.path, '');
    this._flushTimer = setInterval(() => {
      this.flush().catch((error) => this.logger('error', `flush failed: ${error.message}`));
    }, this.flushIntervalMs);
    if (typeof this._flushTimer.unref === 'function') this._flushTimer.unref();
    this._ready = true;
    return this;
  }

  /** Append one normalized event. */
  async append(event) {
    const serialized = serializeEvent(event);

    this.memory.push(serialized);
    if (this.memory.length > this.memoryLimit) this.memory.shift();

    this.stats.appended += 1;
    this.emit('appended', serialized);

    if (!this.enabled) return serialized;

    this.buffer.push(serialized);
    if (this.buffer.length >= this.flushEvery) await this.flush();
    return serialized;
  }

  /** Write buffered events to disk. Safe to call concurrently. */
  async flush() {
    if (!this.enabled || !this.buffer.length) return 0;
    // Serialize writes so lines never interleave.
    while (this._writing) await this._writing;

    const batch = this.buffer;
    this.buffer = [];
    const payload = batch.map((e) => JSON.stringify(e)).join('\n') + '\n';

    this._writing = fsp
      .appendFile(this.path, payload, 'utf8')
      .then(() => {
        this.stats.flushed += batch.length;
        this.emit('flushed', batch.length);
      })
      .catch((error) => {
        this.stats.flushErrors += 1;
        // Put the batch back so data is not silently lost.
        this.buffer = batch.concat(this.buffer);
        this.logger('error', `EventStore flush error: ${error.message}`);
        throw error;
      })
      .finally(() => {
        this._writing = null;
      });

    try {
      await this._writing;
    } catch {
      return 0;
    }
    return batch.length;
  }

  /** Persist the RPC cursor so a restart resumes without gaps. */
  async saveCheckpoint(cursor, ledger = null) {
    if (!this.enabled || !cursor) return;
    const payload = JSON.stringify({ cursor, ledger, savedAt: Date.now() });
    await fsp.writeFile(this.checkpointPath, payload, 'utf8');
  }

  async loadCheckpoint() {
    if (!this.enabled) return null;
    try {
      const raw = await fsp.readFile(this.checkpointPath, 'utf8');
      return JSON.parse(raw);
    } catch {
      return null;
    }
  }

  /**
   * Recent events from the in-memory ring buffer (newest last).
   *
   * Events are rehydrated (BigInt amounts) *for filtering*, then returned in
   * their stored, JSON-safe form by default so callers such as the dashboard
   * HTTP/WebSocket layer can serialize the result directly. Pass
   * `{ hydrate: true }` when you need BigInt values for arithmetic.
   */
  recent({ limit = 100, filter = null, hydrate = false } = {}) {
    let out = this.memory;
    if (filter) {
      out = out.filter((stored) => matchesFilter(rehydrate(stored), filter));
    }
    out = out.slice(-limit);
    return hydrate ? out.map(rehydrate) : out;
  }

  /** Total events retained in memory. */
  get size() {
    return this.memory.length;
  }

  /**
   * Stream persisted events from disk, oldest first.
   * @param {object} [opts]
   * @param {object} [opts.filter]
   * @param {number} [opts.limit]
   * @yields normalized (rehydrated) events
   */
  async *read({ filter = null, limit = Infinity } = {}) {
    if (!this.enabled) {
      let count = 0;
      for (const event of this.memory) {
        const hydrated = rehydrate(event);
        if (filter && !matchesFilter(hydrated, filter)) continue;
        if (count >= limit) return;
        count += 1;
        yield hydrated;
      }
      return;
    }

    await this.flush().catch(() => {});
    if (!fs.existsSync(this.path)) return;

    const stream = fs.createReadStream(this.path, { encoding: 'utf8' });
    const rl = readline.createInterface({ input: stream, crlfDelay: Infinity });
    let count = 0;
    try {
      for await (const line of rl) {
        if (!line.trim()) continue;
        let parsed;
        try {
          parsed = JSON.parse(line);
        } catch {
          continue; // skip a torn line rather than abort the replay
        }
        const hydrated = rehydrate(parsed);
        if (filter && !matchesFilter(hydrated, filter)) continue;
        count += 1;
        yield hydrated;
        if (count >= limit) break;
      }
    } finally {
      rl.close();
      stream.destroy();
    }
  }

  /** Materialize a filtered query into an array. */
  async query({ filter = null, limit = 1000 } = {}) {
    const out = [];
    for await (const event of this.read({ filter, limit })) out.push(event);
    return out;
  }

  /**
   * Replay persisted events through a handler.
   *
   * @param {Function} handler async (event, index) => void
   * @param {object} [opts]
   * @param {object} [opts.filter]
   * @param {number} [opts.limit]
   * @param {number} [opts.speed] 0 = instant (default). >0 replays using the
   *        original inter-event ledger timing divided by `speed`.
   * @param {number} [opts.maxDelayMs] clamp for time-scaled replay
   * @param {AbortSignal} [opts.signal]
   */
  async replay(handler, { filter = null, limit = Infinity, speed = 0, maxDelayMs = 2000, signal = null } = {}) {
    if (typeof handler !== 'function') throw new TypeError('replay handler must be a function');
    let index = 0;
    let previousTs = null;

    for await (const event of this.read({ filter, limit })) {
      if (signal?.aborted) break;

      if (speed > 0 && previousTs != null) {
        const gap = Math.max(0, (event.ts ?? previousTs) - previousTs) / speed;
        const delay = Math.min(gap, maxDelayMs);
        if (delay > 0) await new Promise((resolve) => setTimeout(resolve, delay));
      }
      previousTs = event.ts ?? previousTs;

      await handler(event, index);
      index += 1;
      this.stats.replayed += 1;
      this.emit('replayed', event);
    }

    return index;
  }

  async close() {
    if (this._flushTimer) {
      clearInterval(this._flushTimer);
      this._flushTimer = null;
    }
    await this.flush().catch(() => {});
  }

  getStats() {
    return { ...this.stats, buffered: this.buffer.length, inMemory: this.memory.length, path: this.path };
  }
}
