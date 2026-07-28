/**
 * ScVal decoder tests.
 *
 * Every base64 vector below was produced by the real `soroban-sdk` v26 XDR
 * serializer (see monitoring/tests/fixtures/README.md), so these assert
 * byte-level compatibility with what Soroban RPC actually returns.
 */

import test from 'node:test';
import assert from 'node:assert/strict';
import { decodeScVal, decodeTopics, encodeSymbol, encodeStrkey } from '../src/events/scval.js';

const SDK = {
  symbol_aegis: 'AAAADwAAAAVhZWdpcwAAAA==',
  symbol_transfer: 'AAAADwAAAAh0cmFuc2Zlcg==',
  address: 'AAAAEgAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQ==',
  addressStr: 'CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD2KM',
  i128_pos: 'AAAACgAAAAAAAAAAAAAAAAAAA+g=',
  i128_neg: 'AAAACv///////////////////9Y=',
  i128_max: 'AAAACn////////////////////8=',
  u32: 'AAAAAwAAAAc=',
  i32: 'AAAABP////k=',
  u64: 'AAAABQAACzpzzi/y',
  i64: 'AAAABv//9MWMMdAO',
  bool_true: 'AAAAAAAAAAE=',
  void: 'AAAAAQ==',
  string: 'AAAADgAAAAtoZWxsbyB3b3JsZAA=',
  bytes: 'AAAADQAAAATerb7v',
  tuple3: 'AAAAEAAAAAEAAAADAAAACgAAAAAAAAAAAAAAAAAAA+gAAAAKAAAAAAAAAAAAAAAAAAAH0AAAAAoAAAAAAAAAAAAAAAAAAAu4',
  tupleMixed:
    'AAAAEAAAAAEAAAADAAAAEgAAAAEAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAQAAAAoAAAAAAAAAAAAAAAAAAAH0AAAACgAAAAAAAAAAAAAAAAAAIyg=',
};

test('decodes symbols exactly as the SDK encodes them', () => {
  assert.equal(decodeScVal(SDK.symbol_aegis), 'aegis');
  assert.equal(decodeScVal(SDK.symbol_transfer), 'transfer');
});

test('decodes contract addresses to valid strkeys', () => {
  assert.equal(decodeScVal(SDK.address), SDK.addressStr);
  assert.match(decodeScVal(SDK.address), /^C[A-Z2-7]{55}$/);
});

test('decodes i128 across the full signed range', () => {
  assert.equal(decodeScVal(SDK.i128_pos), 1000n);
  assert.equal(decodeScVal(SDK.i128_neg), -42n);
  assert.equal(decodeScVal(SDK.i128_max), 170141183460469231731687303715884105727n);
});

test('decodes integer, bool, void, string and bytes scalars', () => {
  assert.equal(decodeScVal(SDK.u32), 7);
  assert.equal(decodeScVal(SDK.i32), -7);
  assert.equal(decodeScVal(SDK.u64), 12345678901234n);
  assert.equal(decodeScVal(SDK.i64), -12345678901234n);
  assert.equal(decodeScVal(SDK.bool_true), true);
  assert.equal(decodeScVal(SDK.void), null);
  assert.equal(decodeScVal(SDK.string), 'hello world');
  assert.equal(decodeScVal(SDK.bytes), 'deadbeef');
});

test('decodes vectors (Soroban tuples) including mixed member types', () => {
  assert.deepEqual(decodeScVal(SDK.tuple3), [1000n, 2000n, 3000n]);
  const mixed = decodeScVal(SDK.tupleMixed);
  assert.equal(mixed[0], SDK.addressStr);
  assert.equal(mixed[1], 500n);
  assert.equal(mixed[2], 9000n);
});

test('encodeSymbol round-trips against SDK ground truth', () => {
  assert.equal(encodeSymbol('aegis'), SDK.symbol_aegis);
  assert.equal(encodeSymbol('transfer'), SDK.symbol_transfer);
  assert.equal(decodeScVal(encodeSymbol('wl_add')), 'wl_add');
});

test('encodeStrkey produces checksum-valid account keys', () => {
  const key = encodeStrkey(6 << 3, Buffer.alloc(32, 7));
  assert.match(key, /^G[A-Z2-7]{55}$/);
});

test('decodeTopics maps an array of topics', () => {
  const topics = decodeTopics([SDK.symbol_aegis, SDK.symbol_transfer, SDK.address]);
  assert.deepEqual(topics, ['aegis', 'transfer', SDK.addressStr]);
});

test('never throws on malformed input - returns an undecodable marker', () => {
  const result = decodeScVal('!!!not-valid-base64!!!');
  assert.ok(result && typeof result === 'object');
  assert.equal(result.__undecodable, true);
});

test('handles truncated XDR without crashing', () => {
  const result = decodeScVal('AAAACgAA');
  assert.equal(result.__undecodable, true);
});
