/**
 * Deterministic Aegis event generator + in-process WebSocket RPC test double.
 *
 * Purpose:
 *  - `--simulate` mode: demo/verify the whole pipeline with no network or
 *    deployed contract, producing byte-accurate ScVal XDR identical to what the
 *    real contract emits.
 *  - Tests: a real `ws` server that speaks the subscribeEvents protocol, so the
 *    WebSocket transport is exercised end to end rather than mocked away.
 */

import { WebSocketServer } from 'ws';
import { SCV, encodeStrkey } from './events/scval.js';

// ---------------------------------------------------------------- XDR writers

function writeType(type) {
  const buf = Buffer.alloc(4);
  buf.writeInt32BE(type, 0);
  return buf;
}

function padTo4(buf) {
  const pad = (4 - (buf.length % 4)) % 4;
  return pad ? Buffer.concat([buf, Buffer.alloc(pad)]) : buf;
}

export function scSymbol(value) {
  const utf8 = Buffer.from(value, 'utf8');
  const len = Buffer.alloc(4);
  len.writeUInt32BE(utf8.length, 0);
  return Buffer.concat([writeType(SCV.SYMBOL), len, padTo4(utf8)]);
}

export function scI128(value) {
  const v = BigInt(value);
  const hi = BigInt.asIntN(64, v >> 64n);
  const lo = BigInt.asUintN(64, v & 0xffffffffffffffffn);
  const buf = Buffer.alloc(16);
  buf.writeBigInt64BE(hi, 0);
  buf.writeBigUInt64BE(lo, 8);
  return Buffer.concat([writeType(SCV.I128), buf]);
}

export function scAddressFromStrkey(strkey) {
  // Decode base32 strkey -> 32-byte payload, then re-encode as ScAddress XDR.
  const B32 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';
  let bits = 0;
  let value = 0;
  const bytes = [];
  for (const ch of strkey.replace(/=+$/, '')) {
    const idx = B32.indexOf(ch);
    if (idx < 0) throw new Error(`Invalid strkey char: ${ch}`);
    value = (value << 5) | idx;
    bits += 5;
    if (bits >= 8) {
      bytes.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  const raw = Buffer.from(bytes);
  const version = raw[0];
  const payload = raw.subarray(1, raw.length - 2);
  const isContract = version === 2 << 3;

  if (isContract) {
    return Buffer.concat([writeType(SCV.ADDRESS), writeType(1), payload]);
  }
  // account: ScAddress(0) -> PublicKey union discriminant 0 -> 32 bytes
  return Buffer.concat([writeType(SCV.ADDRESS), writeType(0), writeType(0), payload]);
}

export function scVec(items) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(items.length, 0);
  return Buffer.concat([writeType(SCV.VEC), writeType(1), len, ...items]);
}

const b64 = (buf) => buf.toString('base64');

// ------------------------------------------------------------- Address minting

/** Deterministic pseudo-address generator (valid strkey checksums). */
export function makeAddress(seed, kind = 'account') {
  const payload = Buffer.alloc(32);
  let x = seed >>> 0;
  for (let i = 0; i < 32; i++) {
    x = (x * 1664525 + 1013904223) >>> 0;
    payload[i] = x & 0xff;
  }
  return encodeStrkey(kind === 'contract' ? 2 << 3 : 6 << 3, payload);
}

export const DEMO_CONTRACT = makeAddress(999, 'contract');

// -------------------------------------------------------------- Event builders

let idCounter = 0;

function envelope(contractId, ledger, topics, value) {
  idCounter += 1;
  const paging = `${String(ledger).padStart(10, '0')}-${String(idCounter).padStart(10, '0')}`;
  return {
    type: 'contract',
    ledger: String(ledger),
    ledgerClosedAt: new Date(Date.now()).toISOString(),
    contractId,
    id: paging,
    pagingToken: paging,
    inSuccessfulContractCall: true,
    txHash: Buffer.from(`tx-${paging}`).toString('hex').padEnd(64, '0').slice(0, 64),
    topic: topics.map(b64),
    value: b64(value),
  };
}

export const build = {
  init: (admin, { contractId = DEMO_CONTRACT, ledger = 1 } = {}) =>
    envelope(contractId, ledger, [scSymbol('aegis'), scSymbol('init')], scAddressFromStrkey(admin)),

  whitelist: (admin, user, { contractId = DEMO_CONTRACT, ledger = 1 } = {}) =>
    envelope(
      contractId,
      ledger,
      [scSymbol('aegis'), scSymbol('wl_add'), scAddressFromStrkey(user)],
      scAddressFromStrkey(admin),
    ),

  mint: (to, amount, balance, supply, { contractId = DEMO_CONTRACT, ledger = 1 } = {}) =>
    envelope(
      contractId,
      ledger,
      [scSymbol('aegis'), scSymbol('mint'), scAddressFromStrkey(to)],
      scVec([scI128(amount), scI128(balance), scI128(supply)]),
    ),

  transfer: (from, to, amount, { contractId = DEMO_CONTRACT, ledger = 1 } = {}) =>
    envelope(
      contractId,
      ledger,
      [scSymbol('aegis'), scSymbol('transfer'), scAddressFromStrkey(from), scAddressFromStrkey(to)],
      scI128(amount),
    ),

  yield: (admin, amount, supply, { contractId = DEMO_CONTRACT, ledger = 1 } = {}) =>
    envelope(
      contractId,
      ledger,
      [scSymbol('aegis'), scSymbol('yield')],
      scVec([scAddressFromStrkey(admin), scI128(amount), scI128(supply)]),
    ),
};

/**
 * Produce a realistic protocol lifecycle: deploy -> whitelist -> mint ->
 * transfers -> yield, including one whale transfer that trips alerts.
 */
export function generateLifecycle({ users = 4, startLedger = 1000 } = {}) {
  const admin = makeAddress(1);
  const holders = Array.from({ length: users }, (_, i) => makeAddress(100 + i));
  const events = [];
  let ledger = startLedger;
  let supply = 0n;

  events.push(build.init(admin, { ledger: ledger++ }));
  for (const user of holders) events.push(build.whitelist(admin, user, { ledger: ledger++ }));

  const balances = new Map(holders.map((h) => [h, 0n]));
  for (const user of holders) {
    const amount = 250_000n;
    supply += amount;
    balances.set(user, balances.get(user) + amount);
    events.push(build.mint(user, amount, balances.get(user), supply, { ledger: ledger++ }));
  }

  for (let i = 0; i < holders.length - 1; i++) {
    events.push(build.transfer(holders[i], holders[i + 1], 10_000n * BigInt(i + 1), { ledger: ledger++ }));
  }

  // Whale transfer -> triggers `whale-transfer` alert.
  events.push(build.transfer(holders[0], holders[2], 1_500_000n, { ledger: ledger++ }));
  events.push(build.yield(admin, 42_000n, supply, { ledger: ledger++ }));

  return { admin, holders, events };
}

/**
 * A minimal WebSocket server that speaks the `subscribeEvents` JSON-RPC shape
 * this client expects. Used by `--simulate` and by the integration test.
 */
export class MockSorobanWebSocketServer {
  constructor({ port = 0, host = '127.0.0.1' } = {}) {
    this.host = host;
    this.port = port;
    this.wss = null;
    this.sockets = new Set();
    this.subscriptions = new Map();
  }

  async start() {
    this.wss = new WebSocketServer({ port: this.port, host: this.host });
    await new Promise((resolve) => this.wss.once('listening', resolve));
    this.port = this.wss.address().port;

    this.wss.on('connection', (socket) => {
      this.sockets.add(socket);
      socket.on('close', () => this.sockets.delete(socket));
      socket.on('message', (data) => {
        let msg;
        try {
          msg = JSON.parse(data.toString());
        } catch {
          return;
        }
        if (msg.method === 'subscribeEvents') {
          this.subscriptions.set(socket, msg.params ?? {});
          socket.send(JSON.stringify({ jsonrpc: '2.0', id: msg.id, result: { subscriptionId: `sub-${msg.id}` } }));
        }
      });
    });
    return this;
  }

  get url() {
    return `ws://${this.host}:${this.port}`;
  }

  /** Push one raw RPC event to every subscriber. */
  push(rawEvent) {
    const frame = JSON.stringify({
      jsonrpc: '2.0',
      method: 'events',
      params: { events: [rawEvent] },
    });
    let sent = 0;
    for (const socket of this.sockets) {
      if (socket.readyState === 1) {
        socket.send(frame);
        sent += 1;
      }
    }
    return sent;
  }

  /** Force-close all client sockets (used to test reconnection). */
  dropConnections() {
    for (const socket of this.sockets) {
      try {
        socket.terminate();
      } catch {
        /* ignore */
      }
    }
    this.sockets.clear();
  }

  async stop() {
    this.dropConnections();
    if (this.wss) await new Promise((resolve) => this.wss.close(resolve));
    this.wss = null;
  }
}
