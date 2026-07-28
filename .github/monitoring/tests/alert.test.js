/**
 * Alert engine tests (acceptance criterion #3: alert system with patterns).
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { AlertEngine, SEVERITY, sinks } from '../src/alerts/index.js';
import { normalizeEvent } from '../src/events/normalize.js';
import { build, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);
const ev = (raw) => normalizeEvent(raw);

/** Deterministic clock so time-based patterns are testable without sleeping. */
function clock(start = 1_000_000) {
  let t = start;
  return { now: () => t, advance: (ms) => (t += ms) };
}

test('match pattern fires on every matching event', async () => {
  const engine = new AlertEngine();
  engine.addRule({ name: 'any-mint', pattern: 'match', filter: { action: 'mint' } });

  const fired = await engine.process(ev(build.mint(alice, 10n, 10n, 10n, { ledger: 1 })));
  assert.equal(fired.length, 1);
  assert.equal(fired[0].rule, 'any-mint');

  const none = await engine.process(ev(build.transfer(alice, bob, 1n, { ledger: 2 })));
  assert.equal(none.length, 0);
});

test('threshold pattern compares exact i128 values', async () => {
  const engine = new AlertEngine();
  engine.addRule({
    name: 'whale',
    pattern: 'threshold',
    filter: { action: 'transfer' },
    field: 'amount',
    gte: 1_000_000n,
    severity: SEVERITY.CRITICAL,
  });

  assert.equal((await engine.process(ev(build.transfer(alice, bob, 999_999n, { ledger: 1 })))).length, 0);
  const fired = await engine.process(ev(build.transfer(alice, bob, 1_000_000n, { ledger: 2 })));
  assert.equal(fired.length, 1);
  assert.equal(fired[0].severity, 'critical');
  assert.equal(fired[0].details.value, '1000000');
});

test('threshold supports lt/lte bounds', async () => {
  const engine = new AlertEngine();
  engine.addRule({ name: 'dust', pattern: 'threshold', filter: { action: 'transfer' }, lte: 10n });
  assert.equal((await engine.process(ev(build.transfer(alice, bob, 5n, { ledger: 1 })))).length, 1);
  assert.equal((await engine.process(ev(build.transfer(alice, bob, 500n, { ledger: 2 })))).length, 0);
});

test('rate pattern fires only after N events inside the window', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({ name: 'burst', pattern: 'rate', filter: { action: 'mint' }, count: 3, windowMs: 1000 });

  assert.equal((await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })))).length, 0);
  assert.equal((await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })))).length, 0);
  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 3 })));
  assert.equal(fired.length, 1);
  assert.equal(fired[0].details.observed, 3);
});

test('rate pattern forgets events that age out of the window', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({ name: 'burst', pattern: 'rate', filter: { action: 'mint' }, count: 3, windowMs: 1000 });

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })));
  c.advance(5000); // both fall out of the window
  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 3 })));
  assert.equal(fired.length, 0);
});

test('sequence pattern detects an ordered chain correlated by address', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({
    name: 'instant-drain',
    pattern: 'sequence',
    steps: [{ action: 'wl_add' }, { action: 'mint' }, { action: 'transfer' }],
    correlateBy: 'address',
    windowMs: 60_000,
    severity: SEVERITY.CRITICAL,
  });

  assert.equal((await engine.process(ev(build.whitelist(bob, alice, { ledger: 1 })))).length, 0);
  assert.equal((await engine.process(ev(build.mint(alice, 100n, 100n, 100n, { ledger: 2 })))).length, 0);
  const fired = await engine.process(ev(build.transfer(alice, bob, 100n, { ledger: 3 })));
  assert.equal(fired.length, 1);
  assert.equal(fired[0].pattern, 'sequence');
  assert.equal(fired[0].details.steps, 3);
});

test('sequence resets when the window expires', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({
    name: 'chain',
    pattern: 'sequence',
    steps: [{ action: 'wl_add' }, { action: 'mint' }],
    correlateBy: 'address',
    windowMs: 1000,
  });

  await engine.process(ev(build.whitelist(bob, alice, { ledger: 1 })));
  c.advance(5000);
  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })));
  assert.equal(fired.length, 0);
});

test('absence pattern fires when the stream goes quiet', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({ name: 'stalled', pattern: 'absence', filter: { protocol: 'aegis' }, withinMs: 1000 });

  assert.equal((await engine.checkAbsence()).length, 0);
  c.advance(1500);
  const fired = await engine.checkAbsence();
  assert.equal(fired.length, 1);
  assert.equal(fired[0].reason, 'absence');
});

test('absence clock resets when matching activity resumes', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({ name: 'stalled', pattern: 'absence', filter: { protocol: 'aegis' }, withinMs: 1000 });

  c.advance(900);
  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  c.advance(500);
  assert.equal((await engine.checkAbsence()).length, 0);
});

test('cooldown suppresses alert spam', async () => {
  const c = clock();
  const engine = new AlertEngine({ now: c.now });
  engine.addRule({ name: 'noisy', pattern: 'match', filter: { action: 'mint' }, cooldownMs: 5000 });

  assert.equal((await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })))).length, 1);
  assert.equal((await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 2 })))).length, 0);
  c.advance(6000);
  assert.equal((await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 3 })))).length, 1);
  assert.equal(engine.getStats().suppressed, 1);
});

test('sinks receive every alert and a failing sink is isolated', async () => {
  const collected = [];
  const engine = new AlertEngine();
  engine.addRule({ name: 'r', pattern: 'match', filter: {} });
  engine.addSink(() => {
    throw new Error('sink down');
  });
  engine.addSink(sinks.collect(collected));

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.equal(collected.length, 1);
});

test('alert history is queryable and severity-filterable', async () => {
  const engine = new AlertEngine();
  engine.addRule({ name: 'info-rule', pattern: 'match', filter: { action: 'mint' }, severity: SEVERITY.INFO });
  engine.addRule({ name: 'crit-rule', pattern: 'match', filter: { action: 'transfer' }, severity: SEVERITY.CRITICAL });

  await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  await engine.process(ev(build.transfer(alice, bob, 1n, { ledger: 2 })));

  assert.equal(engine.getHistory().length, 2);
  assert.equal(engine.getHistory({ severity: 'critical' }).length, 1);
  assert.equal(engine.getHistory({ severity: 'critical' })[0].rule, 'crit-rule');
});

test('alerts carry the serialized triggering event (BigInt-safe)', async () => {
  const engine = new AlertEngine();
  engine.addRule({ name: 'r', pattern: 'match', filter: { action: 'transfer' } });
  const [alert] = await engine.process(ev(build.transfer(alice, bob, 12345n, { ledger: 7 })));

  assert.equal(alert.event.fields.amount, '12345');
  assert.doesNotThrow(() => JSON.stringify(alert));
});

test('a rule whose filter throws does not break evaluation of other rules', async () => {
  const engine = new AlertEngine();
  engine.addRule({
    name: 'bad',
    pattern: 'match',
    filter: {
      predicate: () => {
        throw new Error('bad predicate');
      },
    },
  });
  engine.addRule({ name: 'good', pattern: 'match', filter: { action: 'mint' } });

  const fired = await engine.process(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.equal(fired.length, 1);
  assert.equal(fired[0].rule, 'good');
});
