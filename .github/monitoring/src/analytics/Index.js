/**
 * Rolling analytics over the event stream.
 *
 * Maintains counters, per-action/per-contract breakdowns, value totals,
 * top-address leaderboards and a time-bucketed series for the dashboard chart.
 * All amount math is BigInt to stay exact for i128 token values.
 */

import { amountOf, VALUE_ACTIONS } from '../events/normalize.js';

function bigMax(a, b) {
  if (a == null) return b;
  if (b == null) return a;
  return a > b ? a : b;
}

export class AnalyticsEngine {
  constructor({ windowMs = 5 * 60 * 1000, bucketMs = 10 * 1000, topN = 10, now = () => Date.now() } = {}) {
    this.windowMs = windowMs;
    this.bucketMs = bucketMs;
    this.topN = topN;
    this.now = now;

    this.totals = {
      events: 0,
      byAction: {},
      byContract: {},
      byType: {},
      failed: 0,
      minted: 0n,
      transferred: 0n,
      yielded: 0n,
      largestTransfer: null,
      uniqueAddresses: new Set(),
      whitelisted: 0,
    };

    this.window = []; // [{ ts, action, amount, contractId }]
    this.buckets = new Map(); // bucketStart -> { count, byAction, volume }
    this.firstEventTs = null;
    this.lastEventTs = null;
    this.lastLedger = null;
  }

  record(event) {
    const ts = event.ts ?? this.now();
    const action = event.action ?? event.type ?? 'unknown';
    const amount = amountOf(event);

    this.totals.events += 1;
    this.totals.byAction[action] = (this.totals.byAction[action] ?? 0) + 1;
    if (event.contractId) {
      this.totals.byContract[event.contractId] = (this.totals.byContract[event.contractId] ?? 0) + 1;
    }
    this.totals.byType[event.type ?? 'contract'] =
      (this.totals.byType[event.type ?? 'contract'] ?? 0) + 1;
    if (event.inSuccessfulContractCall === false) this.totals.failed += 1;

    for (const subject of event.subjects ?? []) this.totals.uniqueAddresses.add(subject);

    if (action === 'wl_add') this.totals.whitelisted += 1;
    if (amount != null) {
      if (action === 'mint') this.totals.minted += amount;
      if (action === 'transfer') {
        this.totals.transferred += amount;
        if (this.totals.largestTransfer == null || amount > BigInt(this.totals.largestTransfer.amount)) {
          this.totals.largestTransfer = {
            amount: amount.toString(),
            from: event.fields?.from ?? null,
            to: event.fields?.to ?? null,
            ledger: event.ledger,
            id: event.id,
          };
        }
      }
      if (action === 'yield') this.totals.yielded += amount;
    }

    this.window.push({ ts, action, amount, contractId: event.contractId, subjects: event.subjects ?? [] });
    this._trimWindow(ts);

    const bucketStart = Math.floor(ts / this.bucketMs) * this.bucketMs;
    let bucket = this.buckets.get(bucketStart);
    if (!bucket) {
      bucket = { ts: bucketStart, count: 0, byAction: {}, volume: 0n };
      this.buckets.set(bucketStart, bucket);
    }
    bucket.count += 1;
    bucket.byAction[action] = (bucket.byAction[action] ?? 0) + 1;
    if (amount != null && VALUE_ACTIONS.has(action)) bucket.volume += amount;
    this._trimBuckets(ts);

    this.firstEventTs = this.firstEventTs ?? ts;
    this.lastEventTs = bigMax(this.lastEventTs, ts) ?? ts;
    if (event.ledger) this.lastLedger = Math.max(this.lastLedger ?? 0, event.ledger);
  }

  _trimWindow(now) {
    const cutoff = now - this.windowMs;
    while (this.window.length && this.window[0].ts < cutoff) this.window.shift();
  }

  _trimBuckets(now) {
    const cutoff = now - this.windowMs;
    for (const key of this.buckets.keys()) {
      if (key < cutoff) this.buckets.delete(key);
    }
  }

  /** Events per second over the rolling window. */
  get eventsPerSecond() {
    if (!this.window.length) return 0;
    const span = Math.max(1, (this.window[this.window.length - 1].ts - this.window[0].ts) / 1000);
    return Number((this.window.length / span).toFixed(3));
  }

  topAddresses() {
    const counts = new Map();
    for (const entry of this.window) {
      for (const address of entry.subjects) {
        counts.set(address, (counts.get(address) ?? 0) + 1);
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1])
      .slice(0, this.topN)
      .map(([address, count]) => ({ address, count }));
  }

  series() {
    return [...this.buckets.values()]
      .sort((a, b) => a.ts - b.ts)
      .map((b) => ({ ts: b.ts, count: b.count, byAction: b.byAction, volume: b.volume.toString() }));
  }

  snapshot() {
    const now = this.now();
    this._trimWindow(now);
    return {
      generatedAt: now,
      totals: {
        events: this.totals.events,
        failed: this.totals.failed,
        whitelisted: this.totals.whitelisted,
        byAction: { ...this.totals.byAction },
        byContract: { ...this.totals.byContract },
        byType: { ...this.totals.byType },
        minted: this.totals.minted.toString(),
        transferred: this.totals.transferred.toString(),
        yielded: this.totals.yielded.toString(),
        largestTransfer: this.totals.largestTransfer,
        uniqueAddresses: this.totals.uniqueAddresses.size,
      },
      window: {
        windowMs: this.windowMs,
        events: this.window.length,
        eventsPerSecond: this.eventsPerSecond,
        topAddresses: this.topAddresses(),
      },
      series: this.series(),
      lastEventTs: this.lastEventTs,
      lastLedger: this.lastLedger,
    };
  }

  reset() {
    this.totals = {
      events: 0,
      byAction: {},
      byContract: {},
      byType: {},
      failed: 0,
      minted: 0n,
      transferred: 0n,
      yielded: 0n,
      largestTransfer: null,
      uniqueAddresses: new Set(),
      whitelisted: 0,
    };
    this.window = [];
    this.buckets.clear();
    this.firstEventTs = null;
    this.lastEventTs = null;
  }
}
