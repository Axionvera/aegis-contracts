/**
 * Normalizes raw Soroban RPC event payloads into the canonical Aegis event
 * envelope used by every downstream stage (filter, router, alerts, store,
 * triggers, analytics, dashboard).
 *
 * Canonical envelope:
 * {
 *   id, cursor, type, ledger, ledgerClosedAt, ts,
 *   contractId, txHash, inSuccessfulContractCall,
 *   topics: [decoded...], data: <decoded>,
 *   protocol: 'aegis' | null,
 *   action: 'mint' | 'transfer' | ... | null,
 *   subjects: [addresses...],
 *   fields: { ...action specific },
 *   raw: { topic: [...b64], value: b64 }
 * }
 */

import { decodeScVal, decodeTopics, jsonSafe } from './scval.js';

export const NAMESPACE = 'aegis';

/** Known Aegis actions and how to project their decoded payload into fields. */
export const ACTION_SCHEMA = {
  init: {
    subjectsFrom: [],
    project: (topics, data) => ({ admin: data ?? null }),
  },
  wl_add: {
    subjectsFrom: [2],
    project: (topics, data) => ({ user: topics[2] ?? null, admin: data ?? null }),
  },
  mint: {
    subjectsFrom: [2],
    project: (topics, data) => {
      const [amount, newBalance, totalSupply] = Array.isArray(data) ? data : [];
      return {
        to: topics[2] ?? null,
        amount: amount ?? null,
        newBalance: newBalance ?? null,
        totalSupply: totalSupply ?? null,
      };
    },
  },
  transfer: {
    subjectsFrom: [2, 3],
    project: (topics, data) => ({
      from: topics[2] ?? null,
      to: topics[3] ?? null,
      amount: data ?? null,
    }),
  },
  yield: {
    subjectsFrom: [],
    project: (topics, data) => {
      const [admin, amount, totalSupply] = Array.isArray(data) ? data : [];
      return { admin: admin ?? null, amount: amount ?? null, totalSupply: totalSupply ?? null };
    },
  },
};

/** Actions that move value; used by analytics + alert defaults. */
export const VALUE_ACTIONS = new Set(['mint', 'transfer', 'yield']);

function toMillis(ledgerClosedAt) {
  if (!ledgerClosedAt) return Date.now();
  const parsed = Date.parse(ledgerClosedAt);
  return Number.isFinite(parsed) ? parsed : Date.now();
}

function isAddress(value) {
  return typeof value === 'string' && /^[GC][A-Z2-7]{55}$/.test(value);
}

/**
 * @param {object} rawEvent  event object as returned by Soroban RPC getEvents
 * @returns {object} canonical Aegis event envelope
 */
export function normalizeEvent(rawEvent) {
  if (!rawEvent || typeof rawEvent !== 'object') {
    throw new TypeError('normalizeEvent requires an event object');
  }

  // RPC has used both `topic` and `topics` across versions; accept either.
  const rawTopics = rawEvent.topic ?? rawEvent.topics ?? [];
  const rawValue = rawEvent.value?.xdr ?? rawEvent.value ?? null;

  const topics = decodeTopics(rawTopics);
  const data = decodeScVal(rawValue);

  const protocol = topics[0] === NAMESPACE ? NAMESPACE : null;
  const rawAction = typeof topics[1] === 'string' ? topics[1] : null;
  const action = protocol ? rawAction : null;

  const schema = action ? ACTION_SCHEMA[action] : null;
  const fields = schema ? schema.project(topics, data) : {};

  const subjects = [];
  if (schema) {
    for (const idx of schema.subjectsFrom) {
      if (isAddress(topics[idx])) subjects.push(topics[idx]);
    }
  }
  for (const value of Object.values(fields)) {
    if (isAddress(value) && !subjects.includes(value)) subjects.push(value);
  }

  const ledger = Number(rawEvent.ledger ?? 0) || 0;

  return {
    id: rawEvent.id ?? `${ledger}-${rawEvent.pagingToken ?? Math.random().toString(36).slice(2)}`,
    cursor: rawEvent.pagingToken ?? rawEvent.cursor ?? rawEvent.id ?? null,
    type: rawEvent.type ?? 'contract',
    ledger,
    ledgerClosedAt: rawEvent.ledgerClosedAt ?? null,
    ts: toMillis(rawEvent.ledgerClosedAt),
    contractId: rawEvent.contractId ?? null,
    txHash: rawEvent.txHash ?? rawEvent.transactionHash ?? null,
    inSuccessfulContractCall: rawEvent.inSuccessfulContractCall !== false,
    protocol,
    action,
    topics,
    data,
    subjects,
    fields,
    raw: { topic: rawTopics, value: rawValue },
  };
}

/** Convert an envelope into a JSON-serializable object (BigInt -> string). */
export function serializeEvent(event) {
  return jsonSafe(event);
}

/** Best-effort numeric coercion for amount fields (BigInt-safe). */
export function amountOf(event) {
  const amount = event?.fields?.amount;
  if (typeof amount === 'bigint') return amount;
  if (typeof amount === 'number') return BigInt(Math.trunc(amount));
  if (typeof amount === 'string' && /^-?\d+$/.test(amount)) return BigInt(amount);
  return null;
}
