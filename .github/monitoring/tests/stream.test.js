/**
 * Streaming transport tests (acceptance criterion #1: real-time event
 * streaming works). Exercises the real `ws` WebSocket path against an
 * in-process Soroban RPC test double, plus the HTTP getEvents fallback.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { SorobanEventStream, TRANSPORT } from '../src/rpc/websocket-client.js';
import { Backoff } from '../src/rpc/backoff.js';
import { rpcCall, RpcError } from '../src/rpc/jsonrpc.js';
import { MockSorobanWebSocketServer, build, makeAddress } from '../src/simulator.js';

const alice = makeAddress(1);
const bob = makeAddress(2);

/**
 * Wait for a named event.
 *
 * Deliberately NOT `events.once()`: that helper rejects the moment the emitter
 * emits 'error', but this stream emits recoverable transport errors by design
 * (e.g. while degrading from WebSocket to polling). Those must not fail a test
 * that is waiting for a data event.
 */
function waitFor(emitter, name, timeout = 4000) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      emitter.off(name, onEvent);
      reject(new Error(`timeout waiting for ${name}`));
    }, timeout);

    function onEvent(...args) {
      clearTimeout(timer);
      emitter.off(name, onEvent);
      resolve(args);
    }
    emitter.on(name, onEvent);
  });
}

// ------------------------------------------------------------ WebSocket path

test('streams events in real time over a real WebSocket connection', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({ rpcUrl: 'http://unused', wsUrl: server.url });

  try {
    const transport = await stream.start();
    assert.equal(transport, TRANSPORT.WEBSOCKET);

    const received = waitFor(stream, 'event');
    server.push(build.transfer(alice, bob, 777n, { ledger: 42 }));
    const [event] = await received;

    assert.equal(event.action, 'transfer');
    assert.equal(event.fields.amount, 777n);
    assert.equal(event.fields.from, alice);
    assert.equal(event.fields.to, bob);
    assert.equal(event.ledger, 42);
    assert.equal(event.protocol, 'aegis');
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('sends a subscribeEvents request with the configured filters', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const contractId = makeAddress(500, 'contract');
  const stream = new SorobanEventStream({
    rpcUrl: 'http://unused',
    wsUrl: server.url,
    contractIds: [contractId],
    namespaceFilter: true,
  });

  try {
    await stream.start();
    // Give the subscribe frame a tick to arrive.
    await new Promise((resolve) => setTimeout(resolve, 100));
    const [, params] = [...server.subscriptions.entries()][0] ?? [];
    assert.ok(params, 'server recorded a subscription');
    assert.equal(params.filters[0].contractIds[0], contractId);
    assert.equal(params.filters[0].topics[0][0], 'AAAADwAAAAVhZWdpcwAAAA==');
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('streams a full protocol lifecycle in order', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({ rpcUrl: 'http://unused', wsUrl: server.url });
  const seen = [];

  try {
    await stream.start();
    stream.on('event', (e) => seen.push(e.action));

    server.push(build.init(alice, { ledger: 1 }));
    server.push(build.whitelist(alice, bob, { ledger: 2 }));
    server.push(build.mint(bob, 1000n, 1000n, 1000n, { ledger: 3 }));
    server.push(build.transfer(bob, alice, 250n, { ledger: 4 }));
    server.push(build.yield(alice, 99n, 1000n, { ledger: 5 }));

    await new Promise((resolve) => setTimeout(resolve, 250));
    assert.deepEqual(seen, ['init', 'wl_add', 'mint', 'transfer', 'yield']);
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('de-duplicates events redelivered by the transport', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({ rpcUrl: 'http://unused', wsUrl: server.url });
  let count = 0;

  try {
    await stream.start();
    stream.on('event', () => (count += 1));

    const duplicate = build.transfer(alice, bob, 1n, { ledger: 9 });
    server.push(duplicate);
    server.push(duplicate);
    server.push(duplicate);

    await new Promise((resolve) => setTimeout(resolve, 200));
    assert.equal(count, 1);
    assert.equal(stream.getStats().duplicates, 2);
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('falls back to HTTP polling when the WebSocket cannot connect', async () => {
  const page = {
    events: [build.mint(alice, 500n, 500n, 500n, { ledger: 77 })],
    latestLedger: 77,
    cursor: 'cursor-77',
  };
  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({ jsonrpc: '2.0', id: 1, result: page }),
  });

  const stream = new SorobanEventStream({
    rpcUrl: 'http://rpc.local',
    wsUrl: 'ws://127.0.0.1:9', // nothing listening
    fetchImpl,
    pollIntervalMs: 50,
  });

  try {
    const received = waitFor(stream, 'event', 6000);
    const transport = await stream.start();
    assert.equal(transport, TRANSPORT.POLL);

    const [event] = await received;
    assert.equal(event.action, 'mint');
    assert.equal(event.fields.amount, 500n);
    assert.equal(stream.cursor, 'cursor-77');
  } finally {
    await stream.stop();
  }
});

test('polling advances the cursor and never replays the same event', async () => {
  let call = 0;
  const fetchImpl = async () => {
    call += 1;
    const events =
      call === 1
        ? [build.transfer(alice, bob, 1n, { ledger: 1 }), build.transfer(alice, bob, 2n, { ledger: 2 })]
        : [];
    return {
      ok: true,
      status: 200,
      json: async () => ({ result: { events, latestLedger: 2, cursor: `c-${call}` } }),
    };
  };

  const stream = new SorobanEventStream({ rpcUrl: 'http://rpc.local', fetchImpl });
  const seen = [];
  stream.on('event', (e) => seen.push(e.ledger));

  await stream.pollOnce();
  assert.deepEqual(seen, [1, 2]);
  assert.equal(stream.cursor, 'c-1');

  await stream.pollOnce();
  assert.deepEqual(seen, [1, 2]);
  assert.equal(stream.cursor, 'c-2');
});

test('reconnects with backoff after the socket drops', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({
    rpcUrl: 'http://unused',
    wsUrl: server.url,
    reconnect: { initialDelayMs: 30, maxDelayMs: 100, factor: 2, jitter: 0 },
    fetchImpl: async () => ({ ok: true, status: 200, json: async () => ({ result: { events: [] } }) }),
  });

  try {
    await stream.start();
    assert.equal(stream.transport, TRANSPORT.WEBSOCKET);

    const reconnecting = waitFor(stream, 'reconnect', 4000);
    server.dropConnections();
    const [info] = await reconnecting;
    assert.ok(info.attempt >= 1);
    assert.ok(stream.getStats().reconnects >= 1);
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('keeps data flowing via polling while the socket is down', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  let polled = false;
  const stream = new SorobanEventStream({
    rpcUrl: 'http://rpc.local',
    wsUrl: server.url,
    pollIntervalMs: 30,
    reconnect: { initialDelayMs: 5000, maxDelayMs: 5000, factor: 1, jitter: 0 },
    fetchImpl: async () => {
      polled = true;
      return { ok: true, status: 200, json: async () => ({ result: { events: [], latestLedger: 5 } }) };
    },
  });

  try {
    await stream.start();
    server.dropConnections();
    await new Promise((resolve) => setTimeout(resolve, 300));
    assert.equal(stream.transport, TRANSPORT.POLL);
    assert.ok(polled, 'poller took over while the socket was down');
  } finally {
    await stream.stop();
    await server.stop();
  }
});

test('malformed WebSocket frames raise an error but do not kill the stream', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({ rpcUrl: 'http://unused', wsUrl: server.url });

  try {
    await stream.start();
    const errored = waitFor(stream, 'error');
    for (const socket of server.sockets) socket.send('this is not json');
    const [error] = await errored;
    assert.match(error.message, /Malformed WebSocket frame/);

    // Stream still delivers real events afterwards.
    const received = waitFor(stream, 'event');
    server.push(build.mint(alice, 1n, 1n, 1n, { ledger: 3 }));
    const [event] = await received;
    assert.equal(event.action, 'mint');
  } finally {
    await stream.stop();
    await server.stop();
  }
});

// ------------------------------------------------------------------ Internals

test('backoff grows exponentially and respects the cap', () => {
  const backoff = new Backoff({ initialDelayMs: 100, maxDelayMs: 1000, factor: 2, jitter: 0 });
  assert.equal(backoff.next(), 100);
  assert.equal(backoff.next(), 200);
  assert.equal(backoff.next(), 400);
  assert.equal(backoff.next(), 800);
  assert.equal(backoff.next(), 1000);
  assert.equal(backoff.next(), 1000);
  backoff.reset();
  assert.equal(backoff.next(), 100);
});

test('backoff jitter stays inside the configured band', () => {
  const backoff = new Backoff({ initialDelayMs: 1000, maxDelayMs: 10000, factor: 1, jitter: 0.2 });
  for (let i = 0; i < 50; i++) {
    const delay = backoff.next();
    assert.ok(delay >= 800 && delay <= 1200, `delay ${delay} outside band`);
  }
});

test('rpcCall surfaces JSON-RPC errors as RpcError', async () => {
  const fetchImpl = async () => ({
    ok: true,
    status: 200,
    json: async () => ({ jsonrpc: '2.0', id: 1, error: { code: -32602, message: 'bad params' } }),
  });
  await assert.rejects(() => rpcCall('http://x', 'getEvents', {}, { fetchImpl }), (error) => {
    assert.ok(error instanceof RpcError);
    assert.equal(error.code, -32602);
    return true;
  });
});

test('rpcCall surfaces HTTP failures', async () => {
  const fetchImpl = async () => ({ ok: false, status: 503, json: async () => ({}) });
  await assert.rejects(() => rpcCall('http://x', 'getEvents', {}, { fetchImpl }), /HTTP 503/);
});

test('stream stats expose transport, counts and cursor', async () => {
  const server = await new MockSorobanWebSocketServer().start();
  const stream = new SorobanEventStream({ rpcUrl: 'http://unused', wsUrl: server.url });
  try {
    await stream.start();
    server.push(build.mint(alice, 1n, 1n, 1n, { ledger: 11 }));
    await new Promise((resolve) => setTimeout(resolve, 150));

    const stats = stream.getStats();
    assert.equal(stats.transport, TRANSPORT.WEBSOCKET);
    assert.equal(stats.received, 1);
    assert.equal(stats.lastLedger, 11);
    assert.ok(stats.uptimeMs >= 0);
  } finally {
    await stream.stop();
    await server.stop();
  }
});
