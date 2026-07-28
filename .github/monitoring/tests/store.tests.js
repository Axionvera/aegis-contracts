/**
 * Event persistence and replay tests (acceptance criterion #4).
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { EventStore } from '../src/store/event-store.js';
import { normalizeEvent } from '../src/events/normalize.js';
import { build, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);
const ev = (raw) => normalizeEvent(raw);

async function tempStore(overrides = {}) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'aegis-store-'));
  const store = new EventStore({
    path: path.join(dir, 'events.jsonl'),
    flushEvery: 1,
    flushIntervalMs: 10_000,
    ...overrides,
  });
  await store.init();
  return { store, dir, cleanup: () => fs.rm(dir, { recursive: true, force: true }) };
}

test('appends events and persists them as JSONL', async () => {
  const { store, cleanup } = await tempStore();
  try {
    await store.append(ev(build.mint(alice, 100n, 100n, 100n, { ledger: 1 })));
    await store.append(ev(build.transfer(alice, bob, 50n, { ledger: 2 })));
    await store.flush();

    const raw = await fs.readFile(store.path, 'utf8');
    const lines = raw.trim().split('\n');
    assert.equal(lines.length, 2);
    assert.equal(JSON.parse(lines[0]).action, 'mint');
    assert.equal(JSON.parse(lines[1]).action, 'transfer');
  } finally {
    await store.close();
    await cleanup();
  }
});

test('BigInt amounts survive a persist -> read round trip', async () => {
  const { store, cleanup } = await tempStore();
  try {
    const huge = 170141183460469231731687303715884105727n;
    await store.append(ev(build.transfer(alice, bob, huge, { ledger: 1 })));
    await store.flush();

    const [restored] = await store.query({});
    assert.equal(typeof restored.fields.amount, 'bigint');
    assert.equal(restored.fields.amount, huge);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('query filters persisted events from disk', async () => {
  const { store, cleanup } = await tempStore();
  try {
    await store.append(ev(build.mint(alice, 10n, 10n, 10n, { ledger: 1 })));
    await store.append(ev(build.transfer(alice, bob, 20n, { ledger: 2 })));
    await store.append(ev(build.transfer(bob, alice, 30n, { ledger: 3 })));
    await store.flush();

    assert.equal((await store.query({ filter: { action: 'transfer' } })).length, 2);
    assert.equal((await store.query({ filter: { action: 'mint' } })).length, 1);
    assert.equal((await store.query({ filter: { minAmount: 25n } })).length, 1);
    assert.equal((await store.query({ filter: { address: bob } })).length, 2);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('replay streams every persisted event through a handler in order', async () => {
  const { store, cleanup } = await tempStore();
  try {
    for (let i = 1; i <= 5; i++) {
      await store.append(ev(build.transfer(alice, bob, BigInt(i * 10), { ledger: i })));
    }
    await store.flush();

    const seen = [];
    const count = await store.replay((event) => seen.push(event.ledger));
    assert.equal(count, 5);
    assert.deepEqual(seen, [1, 2, 3, 4, 5]);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('replay honours filter and limit', async () => {
  const { store, cleanup } = await tempStore();
  try {
    await store.append(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
    await store.append(ev(build.transfer(alice, bob, 2n, { ledger: 2 })));
    await store.append(ev(build.transfer(bob, alice, 3n, { ledger: 3 })));
    await store.flush();

    const filtered = [];
    await store.replay((e) => filtered.push(e.action), { filter: { action: 'transfer' } });
    assert.deepEqual(filtered, ['transfer', 'transfer']);

    const limited = [];
    await store.replay((e) => limited.push(e.ledger), { limit: 2 });
    assert.equal(limited.length, 2);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('replay supports an abort signal', async () => {
  const { store, cleanup } = await tempStore();
  try {
    for (let i = 1; i <= 10; i++) {
      await store.append(ev(build.transfer(alice, bob, 1n, { ledger: i })));
    }
    await store.flush();

    const controller = new AbortController();
    let count = 0;
    await store.replay(
      () => {
        count += 1;
        if (count === 3) controller.abort();
      },
      { signal: controller.signal },
    );
    assert.equal(count, 3);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('in-memory ring buffer is capped and returns most recent events', async () => {
  const { store, cleanup } = await tempStore({ memoryLimit: 3 });
  try {
    for (let i = 1; i <= 6; i++) {
      await store.append(ev(build.transfer(alice, bob, 1n, { ledger: i })));
    }
    assert.equal(store.size, 3);
    assert.deepEqual(store.recent().map((e) => e.ledger), [4, 5, 6]);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('recent() applies filters to the memory buffer', async () => {
  const { store, cleanup } = await tempStore();
  try {
    await store.append(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
    await store.append(ev(build.transfer(alice, bob, 2n, { ledger: 2 })));
    assert.equal(store.recent({ filter: { action: 'mint' } }).length, 1);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('checkpoints let a restart resume from the last cursor', async () => {
  const { store, dir, cleanup } = await tempStore();
  try {
    await store.saveCheckpoint('cursor-abc-123', 4242);
    const restored = await store.loadCheckpoint();
    assert.equal(restored.cursor, 'cursor-abc-123');
    assert.equal(restored.ledger, 4242);

    // A brand-new store instance on the same path sees the checkpoint.
    const second = new EventStore({ path: path.join(dir, 'events.jsonl') });
    await second.init();
    assert.equal((await second.loadCheckpoint()).cursor, 'cursor-abc-123');
    await second.close();
  } finally {
    await store.close();
    await cleanup();
  }
});

test('a torn/corrupt line is skipped rather than aborting replay', async () => {
  const { store, cleanup } = await tempStore();
  try {
    await store.append(ev(build.mint(alice, 1n, 1n, 1n, { ledger: 1 })));
    await store.flush();
    await fs.appendFile(store.path, '{not valid json\n');
    await store.append(ev(build.transfer(alice, bob, 2n, { ledger: 2 })));
    await store.flush();

    const seen = [];
    await store.replay((e) => seen.push(e.ledger));
    assert.deepEqual(seen, [1, 2]);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('disabled store still serves in-memory replay', async () => {
  const store = new EventStore({ enabled: false, path: '/nonexistent/never-written.jsonl' });
  await store.init();
  await store.append(ev(build.mint(alice, 5n, 5n, 5n, { ledger: 1 })));

  const seen = [];
  await store.replay((e) => seen.push(e.action));
  assert.deepEqual(seen, ['mint']);
  await store.close();
});

test('buffered writes flush on the configured batch size', async () => {
  const { store, cleanup } = await tempStore({ flushEvery: 3 });
  try {
    await store.append(ev(build.transfer(alice, bob, 1n, { ledger: 1 })));
    await store.append(ev(build.transfer(alice, bob, 1n, { ledger: 2 })));
    assert.equal(store.getStats().buffered, 2);
    await store.append(ev(build.transfer(alice, bob, 1n, { ledger: 3 })));
    assert.equal(store.getStats().buffered, 0);
    assert.equal(store.getStats().flushed, 3);
  } finally {
    await store.close();
    await cleanup();
  }
});

test('recent() returns JSON-safe events by default (BigInt regression guard)', async () => {
  const store = new EventStore({ enabled: false });
  await store.init();
  await store.append(ev(build.mint(alice, 777n, 777n, 777n, { ledger: 1 })));

  // The dashboard serializes this directly; a BigInt here would throw a 500.
  assert.doesNotThrow(() => JSON.stringify({ events: store.recent() }));
  assert.equal(store.recent()[0].fields.amount, '777');

  // Opt-in hydration still yields BigInt for arithmetic.
  assert.equal(store.recent({ hydrate: true })[0].fields.amount, 777n);
  await store.close();
});

test('recent() filtering still works on hydrated values', async () => {
  const store = new EventStore({ enabled: false });
  await store.init();
  await store.append(ev(build.transfer(alice, bob, 10n, { ledger: 1 })));
  await store.append(ev(build.transfer(alice, bob, 5_000_000n, { ledger: 2 })));

  const whales = store.recent({ filter: { minAmount: 1_000_000n } });
  assert.equal(whales.length, 1);
  assert.equal(whales[0].ledger, 2);
  await store.close();
});
