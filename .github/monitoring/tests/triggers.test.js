/**
 * Event-based trigger tests (acceptance criterion #6).
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { TriggerEngine, actions } from '../src/triggers/index.js';
import { AnalyticsEngine } from '../src/analytics/index.js';
import { normalizeEvent } from '../src/events/normalize.js';
import { build, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);
const ev = (raw) => normalizeEvent(raw);

function clock(start = 1_000_000) {
  let t = start;
  return { now: () => t, advance: (ms) => (t += ms) };
}

test('a trigger fires when its filter matches', async () => {
  const engine = new TriggerEngine();
  const hits = [];
  engine.register({ name: 'on-mint', filter: { action: 'mint' }, action: (e) => hits.push(e.ledger) });

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 5 })));
  await engine.process(ev(build.transfer(alice, bob, 1n, { ledger: 6 })));

  assert.deepEqual(hits, [5]);
  assert.equal(engine.getStats().fired, 1);
});

test('once triggers fire exactly one time', async () => {
  const engine = new TriggerEngine();
  let count = 0;
  engine.register({ name: 'deploy', filter: { action: 'init' }, once: true, action: () => (count += 1) });

  await engine.process(ev(build.init(alice, { ledger: 1 })));
  await engine.process(ev(build.init(alice, { ledger: 2 })));
  await engine.process(ev(build.init(alice, { ledger: 3 })));
  assert.equal(count, 1);
});

test('maxRuns caps total executions', async () => {
  const engine = new TriggerEngine();
  let count = 0;
  engine.register({ name: 'capped', filter: {}, maxRuns: 2, action: () => (count += 1) });

  for (let i = 0; i < 5; i++) await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: i })));
  assert.equal(count, 2);
});

test('throttle limits execution frequency', async () => {
  const c = clock();
  const engine = new TriggerEngine({ now: c.now });
  let count = 0;
  engine.register({ name: 'throttled', filter: {}, throttleMs: 1000, action: () => (count += 1) });

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })));
  assert.equal(count, 1);

  c.advance(1500);
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 3 })));
  assert.equal(count, 2);
  assert.equal(engine.getStats().skipped, 1);
});

test('debounce collapses a burst into a single execution', async () => {
  const engine = new TriggerEngine();
  let count = 0;
  engine.register({ name: 'debounced', filter: { action: 'mint' }, debounceMs: 40, action: () => (count += 1) });

  for (let i = 0; i < 5; i++) await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: i })));
  assert.equal(count, 0, 'nothing fires during the burst');

  await new Promise((resolve) => setTimeout(resolve, 120));
  assert.equal(count, 1, 'exactly one execution after quiet time');
  engine.dispose();
});

test('failing actions are retried then recorded as failed', async () => {
  const engine = new TriggerEngine();
  let attempts = 0;
  engine.register({
    name: 'flaky',
    filter: {},
    retries: 2,
    retryDelayMs: 1,
    action: () => {
      attempts += 1;
      throw new Error('nope');
    },
  });

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.equal(attempts, 3, 'initial attempt + 2 retries');
  assert.equal(engine.getStats().failed, 1);
  assert.match(engine.list()[0].lastError, /nope/);
});

test('a retried action that eventually succeeds is counted as fired', async () => {
  const engine = new TriggerEngine();
  let attempts = 0;
  engine.register({
    name: 'recovers',
    filter: {},
    retries: 3,
    retryDelayMs: 1,
    action: () => {
      attempts += 1;
      if (attempts < 3) throw new Error('transient');
      return 'ok';
    },
  });

  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.deepEqual(fired, ['recovers']);
  assert.equal(engine.getStats().fired, 1);
  assert.equal(engine.getStats().failed, 0);
});

test('triggers can be disabled and re-enabled at runtime', async () => {
  const engine = new TriggerEngine();
  let count = 0;
  engine.register({ name: 'toggle', filter: {}, action: () => (count += 1) });

  engine.enable('toggle', false);
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.equal(count, 0);

  engine.enable('toggle', true);
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })));
  assert.equal(count, 1);
});

test('one failing trigger does not stop the others', async () => {
  const engine = new TriggerEngine();
  const ok = [];
  engine.register({
    name: 'bad',
    filter: {},
    action: () => {
      throw new Error('boom');
    },
  });
  engine.register({ name: 'good', filter: {}, action: () => ok.push(1) });

  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.deepEqual(fired, ['good']);
  assert.equal(ok.length, 1);
});

test('amount-filtered triggers use exact BigInt bounds', async () => {
  const engine = new TriggerEngine();
  const whales = [];
  engine.register({
    name: 'whale',
    filter: { action: 'transfer', minAmount: 1_000_000n },
    action: (e) => whales.push(e.fields.amount),
  });

  await engine.process(ev(build.transfer(alice, bob, 999_999n, { ledger: 1 })));
  await engine.process(ev(build.transfer(alice, bob, 1_000_000n, { ledger: 2 })));
  assert.deepEqual(whales, [1_000_000n]);
});

test('collect action gathers matching events', async () => {
  const engine = new TriggerEngine();
  const target = [];
  engine.register({ name: 'collector', filter: { action: 'transfer' }, action: actions.collect(target) });

  await engine.process(ev(build.transfer(alice, bob, 1n, { ledger: 1 })));
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })));
  assert.equal(target.length, 1);
});

test('webhook action posts the serialized event', async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, body: JSON.parse(init.body) });
    return { ok: true, status: 200 };
  };
  const engine = new TriggerEngine();
  engine.register({
    name: 'hook',
    filter: { action: 'transfer' },
    action: actions.webhook('http://hook.local', { fetchImpl }),
  });

  await engine.process(ev(build.transfer(alice, bob, 4242n, { ledger: 1 })));
  assert.equal(calls.length, 1);
  assert.equal(calls[0].body.event.fields.amount, '4242');
  assert.equal(calls[0].body.trigger, 'hook');
});

test('trigger history records executions', async () => {
  const engine = new TriggerEngine();
  engine.register({ name: 't', filter: {}, action: () => 'done' });
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));

  const history = engine.getHistory();
  assert.equal(history.length, 1);
  assert.equal(history[0].trigger, 't');
  assert.equal(history[0].ok, true);
});

// ------------------------------------------------------------------ Analytics

test('analytics aggregates totals with exact BigInt math', () => {
  const analytics = new AnalyticsEngine();
  analytics.record(ev(build.mint(alice, 1000n, 1000n, 1000n, { ledger: 1 })));
  analytics.record(ev(build.mint(bob, 500n, 500n, 1500n, { ledger: 2 })));
  analytics.record(ev(build.transfer(alice, bob, 250n, { ledger: 3 })));
  analytics.record(ev(build.whitelist(alice, bob, { ledger: 4 })));

  const snap = analytics.snapshot();
  assert.equal(snap.totals.events, 4);
  assert.equal(snap.totals.minted, '1500');
  assert.equal(snap.totals.transferred, '250');
  assert.equal(snap.totals.whitelisted, 1);
  assert.equal(snap.totals.byAction.mint, 2);
  assert.equal(snap.lastLedger, 4);
});

test('analytics tracks the largest transfer and unique addresses', () => {
  const analytics = new AnalyticsEngine();
  analytics.record(ev(build.transfer(alice, bob, 100n, { ledger: 1 })));
  analytics.record(ev(build.transfer(bob, alice, 9999n, { ledger: 2 })));

  const snap = analytics.snapshot();
  assert.equal(snap.totals.largestTransfer.amount, '9999');
  assert.equal(snap.totals.uniqueAddresses, 2);
});

test('analytics snapshot is JSON serializable (no BigInt leaks)', () => {
  const analytics = new AnalyticsEngine();
  analytics.record(ev(build.mint(alice, 170141183460469231731687303715884105727n, 1n, 1n, { ledger: 1 })));
  assert.doesNotThrow(() => JSON.stringify(analytics.snapshot()));
});

test('analytics builds a time-bucketed series for the dashboard chart', () => {
  const analytics = new AnalyticsEngine({ bucketMs: 1000, windowMs: 60_000 });
  const raw = build.transfer(alice, bob, 1n, { ledger: 1 });
  raw.ledgerClosedAt = new Date(1_700_000_000_000).toISOString();
  analytics.record(normalizeEvent(raw));

  const series = analytics.snapshot().series;
  assert.equal(series.length, 1);
  assert.equal(series[0].count, 1);
  assert.equal(series[0].volume, '1');
});

test('analytics counts events from failed contract calls separately', () => {
  const analytics = new AnalyticsEngine();
  const raw = build.transfer(alice, bob, 1n, { ledger: 1 });
  raw.inSuccessfulContractCall = false;
  analytics.record(normalizeEvent(raw));
  assert.equal(analytics.snapshot().totals.failed, 1);
});
