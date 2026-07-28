/**
 * End-to-end integration tests.
 *
 * Drives the whole system exactly as production would run it:
 *
 *   real WebSocket server -> SorobanEventStream -> normalize -> store ->
 *   analytics -> router -> alerts -> triggers -> dashboard (HTTP + WS)
 *
 * Nothing here is mocked except the Soroban RPC endpoint itself, which is a
 * real `ws` server emitting real ScVal XDR.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { WebSocket } from 'ws';
import { AegisMonitor } from '../src/service.js';
import { MockSorobanWebSocketServer, build, generateLifecycle, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function harness(configOverrides = {}, monitorOptions = {}) {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'aegis-e2e-'));
  const server = await new MockSorobanWebSocketServer().start();

  const monitor = new AegisMonitor({
    config: {
      network: 'local',
      rpcUrl: 'http://127.0.0.1:1/unused',
      wsUrl: server.url,
      verbose: false,
      store: { path: path.join(dir, 'events.jsonl'), flushEvery: 1, flushIntervalMs: 10_000 },
      dashboard: { enabled: false, host: '127.0.0.1', port: 0 },
      ...configOverrides,
    },
    logger: () => {},
    ...monitorOptions,
  });

  return {
    monitor,
    server,
    dir,
    async cleanup() {
      await monitor.stop();
      await server.stop();
      await fs.rm(dir, { recursive: true, force: true });
    },
  };
}

/** Wait until `predicate()` is true or time runs out. */
async function until(predicate, { timeout = 4000, interval = 20 } = {}) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (await predicate()) return true;
    await sleep(interval);
  }
  throw new Error('condition not met within timeout');
}

test('end-to-end: a streamed event flows through every pipeline stage', async () => {
  const { monitor, server, cleanup } = await harness();
  try {
    await monitor.start({ dashboard: false });
    assert.equal(monitor.stream.transport, 'websocket');

    server.push(build.transfer(alice, bob, 12_345n, { ledger: 100 }));
    await until(() => monitor.processed >= 1);

    // 1. persisted
    const stored = await monitor.store.query({});
    assert.equal(stored.length, 1);
    assert.equal(stored[0].fields.amount, 12_345n);

    // 2. analytics recorded
    assert.equal(monitor.analytics.snapshot().totals.transferred, '12345');

    // 3. routed
    assert.ok(monitor.router.getStats().matched > 0);

    // 4. in the in-memory buffer for the dashboard
    assert.equal(monitor.store.recent().length, 1);
  } finally {
    await cleanup();
  }
});

test('end-to-end: a whale transfer raises the configured alert', async () => {
  const { monitor, server, cleanup } = await harness();
  const alerts = [];
  try {
    monitor.on('alert', (a) => alerts.push(a));
    await monitor.start({ dashboard: false });

    server.push(build.transfer(alice, bob, 5_000_000n, { ledger: 200 }));
    await until(() => alerts.length >= 1);

    const whale = alerts.find((a) => a.rule === 'whale-transfer');
    assert.ok(whale, 'whale-transfer alert fired');
    assert.equal(whale.details.value, '5000000');
  } finally {
    await cleanup();
  }
});

test('end-to-end: the instant-drain sequence alert detects a suspicious chain', async () => {
  const { monitor, server, cleanup } = await harness();
  const alerts = [];
  try {
    monitor.on('alert', (a) => alerts.push(a));
    await monitor.start({ dashboard: false });

    const victim = makeAddress(77);
    server.push(build.whitelist(alice, victim, { ledger: 300 }));
    server.push(build.mint(victim, 100n, 100n, 100n, { ledger: 301 }));
    server.push(build.transfer(victim, bob, 100n, { ledger: 302 }));

    await until(() => alerts.some((a) => a.rule === 'instant-drain'));
    const drain = alerts.find((a) => a.rule === 'instant-drain');
    assert.equal(drain.severity, 'critical');
    assert.equal(drain.details.steps, 3);
  } finally {
    await cleanup();
  }
});

test('end-to-end: triggers execute off streamed events', async () => {
  const { monitor, server, cleanup } = await harness();
  const fired = [];
  try {
    monitor.triggers.register({
      name: 'test-collector',
      filter: { action: 'mint' },
      action: (e) => fired.push(e.fields.amount),
    });
    await monitor.start({ dashboard: false });

    server.push(build.mint(alice, 900n, 900n, 900n, { ledger: 400 }));
    await until(() => fired.length >= 1);
    assert.deepEqual(fired, [900n]);
  } finally {
    await cleanup();
  }
});

test('end-to-end: a full protocol lifecycle produces correct analytics', async () => {
  const { monitor, server, cleanup } = await harness();
  try {
    await monitor.start({ dashboard: false });
    const { events } = generateLifecycle({ users: 4, startLedger: 1000 });
    for (const event of events) server.push(event);

    await until(() => monitor.processed >= events.length);

    const snap = monitor.analytics.snapshot();
    assert.equal(snap.totals.events, events.length);
    assert.equal(snap.totals.byAction.init, 1);
    assert.equal(snap.totals.byAction.wl_add, 4);
    assert.equal(snap.totals.byAction.mint, 4);
    assert.equal(snap.totals.minted, '1000000'); // 4 x 250_000
    assert.equal(snap.totals.byAction.yield, 1);
    assert.equal(snap.totals.largestTransfer.amount, '1500000');
  } finally {
    await cleanup();
  }
});

test('end-to-end: persisted events replay through the pipeline after a restart', async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'aegis-replay-'));
  const storePath = path.join(dir, 'events.jsonl');
  const baseConfig = {
    network: 'local',
    rpcUrl: 'http://127.0.0.1:1/unused',
    store: { path: storePath, flushEvery: 1, flushIntervalMs: 10_000 },
    dashboard: { enabled: false, host: '127.0.0.1', port: 0 },
  };

  // --- session 1: capture events ---
  const server = await new MockSorobanWebSocketServer().start();
  const first = new AegisMonitor({ config: { ...baseConfig, wsUrl: server.url }, logger: () => {} });
  await first.start({ dashboard: false });
  server.push(build.mint(alice, 10n, 10n, 10n, { ledger: 1 }));
  server.push(build.transfer(alice, bob, 20n, { ledger: 2 }));
  await until(() => first.processed >= 2);
  first.stream.cursor = 'cursor-xyz';
  await first.stop();
  await server.stop();

  // --- session 2: fresh process, replays history ---
  const second = new AegisMonitor({ config: { ...baseConfig }, logger: () => {} });
  await second.store.init();

  const checkpoint = await second.store.loadCheckpoint();
  assert.equal(checkpoint.cursor, 'cursor-xyz', 'cursor checkpoint survived restart');

  const replayed = [];
  const count = await second.store.replay((e) => replayed.push(e.action));
  assert.equal(count, 2);
  assert.deepEqual(replayed, ['mint', 'transfer']);

  // Replaying through the live pipeline re-populates analytics.
  await second.replay({ throughPipeline: true });
  assert.equal(second.analytics.snapshot().totals.events, 2);

  await second.stop();
  await fs.rm(dir, { recursive: true, force: true });
});

test('end-to-end: dashboard API exposes events, analytics, alerts and stats', async () => {
  const { monitor, server, cleanup } = await harness({
    dashboard: { enabled: true, host: '127.0.0.1', port: 0 },
  });
  try {
    await monitor.start();
    const base = `http://127.0.0.1:${monitor.dashboard.port}`;

    server.push(build.mint(alice, 777n, 777n, 777n, { ledger: 500 }));
    await until(() => monitor.processed >= 1);

    const health = await (await fetch(`${base}/api/health`)).json();
    assert.equal(health.status, 'ok');
    assert.equal(health.transport, 'websocket');

    const events = await (await fetch(`${base}/api/events`)).json();
    assert.equal(events.count, 1);
    assert.equal(events.events[0].action, 'mint');
    assert.equal(events.events[0].fields.amount, '777');

    const filtered = await (await fetch(`${base}/api/events?action=transfer`)).json();
    assert.equal(filtered.count, 0);

    const analytics = await (await fetch(`${base}/api/analytics`)).json();
    assert.equal(analytics.totals.minted, '777');

    const stats = await (await fetch(`${base}/api/stats`)).json();
    assert.equal(stats.stream.received, 1);

    const rules = await (await fetch(`${base}/api/rules`)).json();
    assert.ok(rules.rules.length >= 5, 'default alert rules installed');

    const routes = await (await fetch(`${base}/api/routes`)).json();
    assert.ok(routes.routes.length >= 3);

    const triggers = await (await fetch(`${base}/api/triggers`)).json();
    assert.ok(triggers.triggers.length >= 3);

    const ui = await fetch(`${base}/`);
    assert.equal(ui.status, 200);
    assert.match(ui.headers.get('content-type'), /text\/html/);
  } finally {
    await cleanup();
  }
});

test('end-to-end: dashboard pushes live events over WebSocket', async () => {
  const { monitor, server, cleanup } = await harness({
    dashboard: { enabled: true, host: '127.0.0.1', port: 0 },
  });
  try {
    await monitor.start();
    const client = new WebSocket(`ws://127.0.0.1:${monitor.dashboard.port}/ws`);
    const messages = [];
    client.on('message', (data) => messages.push(JSON.parse(data.toString())));
    await new Promise((resolve) => client.on('open', resolve));

    await until(() => messages.some((m) => m.type === 'hello'));

    server.push(build.transfer(alice, bob, 31337n, { ledger: 600 }));
    await until(() => messages.some((m) => m.type === 'event'));

    const pushed = messages.find((m) => m.type === 'event');
    assert.equal(pushed.payload.action, 'transfer');
    assert.equal(pushed.payload.fields.amount, '31337');

    client.close();
  } finally {
    await cleanup();
  }
});

test('end-to-end: dashboard replay endpoint returns persisted history', async () => {
  const { monitor, server, cleanup } = await harness({
    dashboard: { enabled: true, host: '127.0.0.1', port: 0 },
  });
  try {
    await monitor.start();
    const base = `http://127.0.0.1:${monitor.dashboard.port}`;

    server.push(build.mint(alice, 1n, 1n, 1n, { ledger: 700 }));
    server.push(build.transfer(alice, bob, 2n, { ledger: 701 }));
    await until(() => monitor.processed >= 2);

    const response = await fetch(`${base}/api/replay`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ filter: { action: 'transfer' }, limit: 10 }),
    });
    const body = await response.json();
    assert.equal(body.replayed, 1);
    assert.equal(body.events[0].action, 'transfer');
  } finally {
    await cleanup();
  }
});

test('end-to-end: a trigger can be toggled through the dashboard API', async () => {
  const { monitor, cleanup } = await harness({
    dashboard: { enabled: true, host: '127.0.0.1', port: 0 },
  });
  try {
    await monitor.start();
    const base = `http://127.0.0.1:${monitor.dashboard.port}`;

    const off = await fetch(`${base}/api/triggers/audit-log-compliance/toggle`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: false }),
    });
    assert.equal(off.status, 200);
    assert.equal(monitor.triggers.list().find((t) => t.name === 'audit-log-compliance').enabled, false);

    const missing = await fetch(`${base}/api/triggers/does-not-exist/toggle`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled: true }),
    });
    assert.equal(missing.status, 404);
  } finally {
    await cleanup();
  }
});

test('end-to-end: the monitor survives malformed frames and keeps streaming', async () => {
  const { monitor, server, cleanup } = await harness();
  try {
    await monitor.start({ dashboard: false });
    for (const socket of server.sockets) socket.send('<<<garbage>>>');
    await sleep(50);

    server.push(build.mint(alice, 5n, 5n, 5n, { ledger: 800 }));
    await until(() => monitor.processed >= 1);
    assert.equal(monitor.analytics.snapshot().totals.events, 1);
  } finally {
    await cleanup();
  }
});

test('end-to-end: events from unrelated contracts are still normalized safely', async () => {
  const { monitor, server, cleanup } = await harness();
  try {
    await monitor.start({ dashboard: false });

    // A non-Aegis event: unknown topics, no protocol namespace.
    server.push({
      type: 'contract',
      ledger: '900',
      ledgerClosedAt: new Date().toISOString(),
      contractId: makeAddress(4242, 'contract'),
      id: 'foreign-1',
      pagingToken: 'foreign-1',
      inSuccessfulContractCall: true,
      topic: ['AAAADwAAAAh0cmFuc2Zlcg=='],
      value: 'AAAAAwAAAAc=',
    });

    await until(() => monitor.processed >= 1);
    const [event] = monitor.store.recent();
    assert.equal(event.protocol, null);
    assert.equal(event.action, null);
    assert.deepEqual(event.topics, ['transfer']);
  } finally {
    await cleanup();
  }
});
