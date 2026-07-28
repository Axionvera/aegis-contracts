/**
 * Minimal, dependency-free ScVal XDR decoder.
 *
 * Soroban RPC returns event topics and values as base64-encoded `ScVal` XDR.
 * The full `@stellar/stellar-sdk` is a heavy dependency for a monitoring
 * sidecar, and the Aegis event surface only uses a small, well-defined subset of
 * the ScVal type space:
 *
 *   symbol, address (account/contract), i128, u128, i64, u64, i32, u32,
 *   bool, void, string, bytes, vec, map
 *
 * This decoder covers exactly that subset and degrades gracefully (returning a
 * `{ __raw }` marker) for anything it does not understand, so an unexpected
 * type can never crash the stream.
 *
 * XDR reference: stellar-core `Stellar-contract.x`
 */

// ScValType discriminants
export const SCV = {
  BOOL: 0,
  VOID: 1,
  ERROR: 2,
  U32: 3,
  I32: 4,
  U64: 5,
  I64: 6,
  TIMEPOINT: 7,
  DURATION: 8,
  U128: 9,
  I128: 10,
  U256: 11,
  I256: 12,
  BYTES: 13,
  STRING: 14,
  SYMBOL: 15,
  VEC: 16,
  MAP: 17,
  ADDRESS: 18,
};

// Strkey version bytes
const STRKEY_ED25519_PUBLIC = 6 << 3; // 'G' => 48
const STRKEY_CONTRACT = 2 << 3; // 'C' => 16
const STRKEY_MUXED_ACCOUNT = 12 << 3; // 'M' => 96
const B32_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ234567';

function crc16xmodem(bytes) {
  let crc = 0x0000;
  for (const byte of bytes) {
    crc ^= byte << 8;
    for (let i = 0; i < 8; i++) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  return crc & 0xffff;
}

function base32Encode(bytes) {
  let bits = 0;
  let value = 0;
  let output = '';
  for (const byte of bytes) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      output += B32_ALPHABET[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) output += B32_ALPHABET[(value << (5 - bits)) & 31];
  while (output.length % 8 !== 0) output += '=';
  return output;
}

/** Encode raw key bytes into a Stellar strkey (G.../C...). */
export function encodeStrkey(versionByte, payload) {
  const data = Buffer.concat([Buffer.from([versionByte]), Buffer.from(payload)]);
  const checksum = crc16xmodem(data);
  const withChecksum = Buffer.concat([data, Buffer.from([checksum & 0xff, (checksum >> 8) & 0xff])]);
  return base32Encode(withChecksum);
}

class XdrReader {
  constructor(buffer) {
    this.buf = buffer;
    this.offset = 0;
  }

  get remaining() {
    return this.buf.length - this.offset;
  }

  require(n) {
    if (this.offset + n > this.buf.length) {
      throw new RangeError(`XDR underflow: need ${n} bytes at ${this.offset}, have ${this.remaining}`);
    }
  }

  readInt32() {
    this.require(4);
    const v = this.buf.readInt32BE(this.offset);
    this.offset += 4;
    return v;
  }

  readUint32() {
    this.require(4);
    const v = this.buf.readUInt32BE(this.offset);
    this.offset += 4;
    return v;
  }

  readBigInt64() {
    this.require(8);
    const v = this.buf.readBigInt64BE(this.offset);
    this.offset += 8;
    return v;
  }

  readBigUint64() {
    this.require(8);
    const v = this.buf.readBigUInt64BE(this.offset);
    this.offset += 8;
    return v;
  }

  readBytes(n) {
    this.require(n);
    const v = this.buf.subarray(this.offset, this.offset + n);
    this.offset += n;
    // XDR pads to 4-byte boundaries
    const pad = (4 - (n % 4)) % 4;
    this.offset += pad;
    return v;
  }

  readVarBytes() {
    const len = this.readUint32();
    return this.readBytes(len);
  }

  readString() {
    return this.readVarBytes().toString('utf8');
  }
}

/**
 * Combine hi/lo 64-bit halves into a signed 128-bit BigInt.
 * Soroban encodes i128 as { hi: int64, lo: uint64 }.
 */
function combineI128(hi, lo) {
  return (BigInt.asIntN(64, hi) << 64n) | BigInt.asUintN(64, lo);
}

function combineU128(hi, lo) {
  return (BigInt.asUintN(64, hi) << 64n) | BigInt.asUintN(64, lo);
}

function readScAddress(reader) {
  const type = reader.readInt32();
  switch (type) {
    case 0: {
      // SC_ADDRESS_TYPE_ACCOUNT -> AccountID (PublicKey union, type 0 = ed25519)
      const keyType = reader.readInt32();
      const key = reader.readBytes(32);
      if (keyType !== 0) return { __raw: 'unsupported-account-key-type', keyType };
      return encodeStrkey(STRKEY_ED25519_PUBLIC, key);
    }
    case 1: {
      // SC_ADDRESS_TYPE_CONTRACT -> Hash(32)
      const hash = reader.readBytes(32);
      return encodeStrkey(STRKEY_CONTRACT, hash);
    }
    case 2: {
      // SC_ADDRESS_TYPE_MUXED_ACCOUNT
      const id = reader.readBigUint64();
      const key = reader.readBytes(32);
      const payload = Buffer.concat([Buffer.from(key), Buffer.alloc(8)]);
      payload.writeBigUInt64BE(BigInt.asUintN(64, id), 32);
      return encodeStrkey(STRKEY_MUXED_ACCOUNT, payload);
    }
    default:
      return { __raw: 'unsupported-address-type', type };
  }
}

function readScVal(reader) {
  const type = reader.readInt32();
  switch (type) {
    case SCV.BOOL:
      return reader.readInt32() !== 0;
    case SCV.VOID:
      return null;
    case SCV.ERROR: {
      const errType = reader.readInt32();
      const code = reader.readInt32();
      return { __error: { type: errType, code } };
    }
    case SCV.U32:
      return reader.readUint32();
    case SCV.I32:
      return reader.readInt32();
    case SCV.U64:
    case SCV.TIMEPOINT:
    case SCV.DURATION:
      return reader.readBigUint64();
    case SCV.I64:
      return reader.readBigInt64();
    case SCV.U128: {
      const hi = reader.readBigUint64();
      const lo = reader.readBigUint64();
      return combineU128(hi, lo);
    }
    case SCV.I128: {
      const hi = reader.readBigInt64();
      const lo = reader.readBigUint64();
      return combineI128(hi, lo);
    }
    case SCV.U256:
    case SCV.I256: {
      const parts = [
        reader.readBigUint64(),
        reader.readBigUint64(),
        reader.readBigUint64(),
        reader.readBigUint64(),
      ];
      let value = 0n;
      for (const part of parts) value = (value << 64n) | BigInt.asUintN(64, part);
      return type === SCV.I256 ? BigInt.asIntN(256, value) : value;
    }
    case SCV.BYTES:
      return reader.readVarBytes().toString('hex');
    case SCV.STRING:
      return reader.readString();
    case SCV.SYMBOL:
      return reader.readString();
    case SCV.VEC: {
      const present = reader.readInt32();
      if (!present) return [];
      const len = reader.readUint32();
      const out = [];
      for (let i = 0; i < len; i++) out.push(readScVal(reader));
      return out;
    }
    case SCV.MAP: {
      const present = reader.readInt32();
      if (!present) return {};
      const len = reader.readUint32();
      const out = {};
      for (let i = 0; i < len; i++) {
        const key = readScVal(reader);
        const value = readScVal(reader);
        out[typeof key === 'object' ? JSON.stringify(key) : String(key)] = value;
      }
      return out;
    }
    case SCV.ADDRESS:
      return readScAddress(reader);
    default:
      return { __raw: 'unsupported-scval-type', type };
  }
}

const BASE64_RE = /^[A-Za-z0-9+/]*={0,2}$/;

/**
 * Strict base64 validation.
 *
 * `Buffer.from(str, 'base64')` silently skips characters outside the base64
 * alphabet, so malformed input would otherwise decode into plausible-looking
 * garbage instead of being reported. Validating up front means corrupt data is
 * always surfaced as `__undecodable` rather than a bogus value.
 */
function decodeBase64Strict(value) {
  const compact = value.trim();
  if (!BASE64_RE.test(compact) || compact.length % 4 !== 0) {
    throw new Error('invalid base64 input');
  }
  return Buffer.from(compact, 'base64');
}

/**
 * Decode a base64 ScVal XDR string into a native JS value.
 * Returns `{ __undecodable, value, error }` instead of throwing.
 */
export function decodeScVal(base64) {
  if (base64 == null) return null;
  // The RPC may already return decoded JSON when xdrFormat=json.
  if (typeof base64 !== 'string') return base64;
  try {
    const reader = new XdrReader(decodeBase64Strict(base64));
    const value = readScVal(reader);
    // A well-formed ScVal consumes its entire buffer (padding included).
    if (reader.remaining > 0) {
      throw new Error(`${reader.remaining} trailing byte(s) after ScVal`);
    }
    return value;
  } catch (error) {
    return { __undecodable: true, value: base64, error: error.message };
  }
}

/** Decode an array of base64 topics. */
export function decodeTopics(topics = []) {
  return topics.map((t) => decodeScVal(t));
}

/**
 * Encode a Symbol ScVal to base64 - used to build RPC topic filters.
 * Symbols are limited to 32 chars of [a-zA-Z0-9_].
 */
export function encodeSymbol(symbol) {
  if (typeof symbol !== 'string') throw new TypeError('symbol must be a string');
  if (symbol.length > 32) throw new RangeError('symbol exceeds 32 characters');
  const utf8 = Buffer.from(symbol, 'utf8');
  const pad = (4 - (utf8.length % 4)) % 4;
  const buf = Buffer.alloc(4 + 4 + utf8.length + pad);
  buf.writeInt32BE(SCV.SYMBOL, 0);
  buf.writeUInt32BE(utf8.length, 4);
  utf8.copy(buf, 8);
  return buf.toString('base64');
}

/** JSON-safe replacer that renders BigInt as a decimal string. */
export function jsonSafe(value) {
  if (typeof value === 'bigint') return value.toString();
  if (Array.isArray(value)) return value.map(jsonSafe);
  if (value && typeof value === 'object') {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = jsonSafe(v);
    return out;
  }
  return value;
}
