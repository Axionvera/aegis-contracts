/**
 * Event filtering and routing tests (acceptance criterion #2).
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { EventRouter, matchesFilter, matchTopics } from '../src/events/filter.js';
import { normalizeEvent } from '../src/events/normalize.js';
import { build, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);
const carol = makeAddress(3);

const ev = (raw) => normalizeEvent(raw);

test('empty filter matches everything', () => {
  const event = ev(build.transfer(alice, bob, 100n, { ledger: 10 }));
  assert.ok(matchesFilter(event, {}));
  assert.ok(matchesFilter(event, null));
});

test('filters by action, including "any of" arrays', () => {
  const transfer = ev(build.transfer(alice, bob, 100n, { ledger: 10 }));
  const mint = ev(build.mint(alice, 50n, 50n, 50n, { ledger: 11 }));

  assert.ok(matchesFilter(transfer, { action: 'transfer' }));
  assert.ok(!matchesFilter(transfer, { action: 'mint' }));
  assert.ok(matchesFilter(mint, { action: ['mint', 'yield'] }));
  assert.ok(!matchesFilter(mint, { action: ['transfer', 'yield'] }));
});

test('filters by address across subjects and fields', () => {
  const event = ev(build.transfer(alice, bob, 100n, { ledger: 10 }));
  assert.ok(matchesFilter(event, { address: alice }));
  assert.ok(matchesFilter(event, { address: bob }));
  assert.ok(!matchesFilter(event, { address: carol }));
  assert.ok(matchesFilter(event, { address: [carol, bob] }));
});

test('filters by from/to direction', () => {
  const event = ev(build.transfer(alice, bob, 100n, { ledger: 10 }));
  assert.ok(matchesFilter(event, { from: alice }));
  assert.ok(matchesFilter(event, { to: bob }));
  assert.ok(!matchesFilter(event, { from: bob }));
});

test('amount bounds use exact BigInt comparison', () => {
  const event = ev(build.transfer(alice, bob, 1000n, { ledger: 10 }));
  assert.ok(matchesFilter(event, { minAmount: 1000n }));
  assert.ok(matchesFilter(event, { minAmount: '999' }));
  assert.ok(!matchesFilter(event, { minAmount: 1001n }));
  assert.ok(matchesFilter(event, { maxAmount: 1000n }));
  assert.ok(!matchesFilter(event, { maxAmount: 999n }));
});

test('huge i128 amounts do not lose precision through the filter', () => {
  const huge = 170141183460469231731687303715884105727n;
  const event = ev(build.transfer(alice, bob, huge, { ledger: 10 }));
  assert.equal(event.fields.amount, huge);
  assert.ok(matchesFilter(event, { minAmount: huge - 1n }));
  assert.ok(!matchesFilter(event, { minAmount: huge + 1n }));
});

test('filters by ledger range and contract id', () => {
  const event = ev(build.transfer(alice, bob, 100n, { ledger: 500, contractId: 'CTEST' }));
  assert.ok(matchesFilter(event, { ledgerFrom: 400, ledgerTo: 600 }));
  assert.ok(!matchesFilter(event, { ledgerFrom: 501 }));
  assert.ok(matchesFilter(event, { contractId: 'CTEST' }));
  assert.ok(!matchesFilter(event, { contractId: 'COTHER' }));
});

test('positional topic matching supports * and ** wildcards', () => {
  const event = ev(build.transfer(alice, bob, 100n, { ledger: 10 }));
  assert.ok(matchTopics(['aegis', 'transfer'], event.topics));
  assert.ok(matchTopics(['aegis', '*', alice], event.topics));
  assert.ok(matchTopics(['aegis', '**'], event.topics));
  assert.ok(!matchTopics(['aegis', 'mint'], event.topics));
  assert.ok(matchesFilter(event, { topicMatch: ['aegis', 'transfer', '*', bob] }));
});

test('custom predicate acts as an escape hatch', () => {
  const event = ev(build.mint(alice, 100n, 100n, 100n, { ledger: 10 }));
  assert.ok(matchesFilter(event, { predicate: (e) => e.fields.totalSupply === 100n }));
  assert.ok(!matchesFilter(event, { predicate: () => false }));
});

test('successOnly excludes events from failed calls', () => {
  const raw = build.transfer(alice, bob, 100n, { ledger: 10 });
  raw.inSuccessfulContractCall = false;
  const event = ev(raw);
  assert.ok(!matchesFilter(event, { successOnly: true }));
  assert.ok(matchesFilter(event, {}));
});

test('router dispatches an event to every matching route', async () => {
  const router = new EventRouter();
  const hits = [];
  router.addRoute('all', {}, (e) => hits.push(['all', e.action]));
  router.addRoute('transfers', { action: 'transfer' }, (e) => hits.push(['transfers', e.action]));
  router.addRoute('mints', { action: 'mint' }, (e) => hits.push(['mints', e.action]));

  const matched = await router.dispatch(ev(build.transfer(alice, bob, 5n, { ledger: 10 })));
  assert.deepEqual(matched.sort(), ['all', 'transfers']);
  assert.equal(hits.length, 2);
});

test('router honours priority ordering', async () => {
  const router = new EventRouter();
  const order = [];
  router.addRoute('low', {}, () => order.push('low'), { priority: 1 });
  router.addRoute('high', {}, () => order.push('high'), { priority: 100 });
  router.addRoute('mid', {}, () => order.push('mid'), { priority: 50 });

  await router.dispatch(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.deepEqual(order, ['high', 'mid', 'low']);
});

test('a throwing route handler cannot break the pipeline', async () => {
  const router = new EventRouter();
  const survived = [];
  router.addRoute('bad', {}, () => {
    throw new Error('boom');
  }, { priority: 10 });
  router.addRoute('good', {}, () => survived.push('ok'), { priority: 1 });

  const matched = await router.dispatch(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
  assert.deepEqual(matched, ['bad', 'good']);
  assert.deepEqual(survived, ['ok']);
  assert.equal(router.getStats().handlerErrors, 1);
});

test('routes can be listed and removed', () => {
  const router = new EventRouter();
  router.addRoute('a', { action: 'mint' }, () => {});
  assert.equal(router.listRoutes().length, 1);
  assert.equal(router.listRoutes()[0].name, 'a');
  assert.ok(router.removeRoute('a'));
  assert.equal(router.listRoutes().length, 0);
});
