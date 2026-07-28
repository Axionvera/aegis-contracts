# Aegis Monitoring — Real-Time Soroban Event Streaming

Live contract event monitoring, filtering, alerting, persistence, replay,
analytics and automated triggers for the Aegis RWA Protocol.

```
Soroban RPC ──► SorobanEventStream ──► normalize (ScVal → envelope)
 (WS or poll)          │
                       ├──► EventStore      persistence + replay + checkpoints
                       ├──► Analytics       rolling metrics for the dashboard
                       ├──► EventRouter     filtering + routing
                       ├──► AlertEngine     pattern-based alerting
                       ├──► TriggerEngine   automated actions
                       └──► Dashboard       HTTP API + WebSocket fan-out + UI
```

## Quick start

```bash
cd monitoring
npm install

# Self-contained demo: mock RPC, no network, full pipeline + dashboard
npm run dev            # → http://127.0.0.1:4500

# Against real infrastructure
AEGIS_NETWORK=testnet \
AEGIS_CONTRACT_IDS=CXXXX... \
npm start

npm test               # 106 tests
```

## A note on transports (important)

Soroban RPC exposes contract events via the **HTTP JSON-RPC `getEvents`**
method. A native WebSocket subscription API has been on the roadmap since the
original "Events by Contract ID" epic but is **not available on public
testnet/mainnet endpoints today**. A monitor that only spoke WebSocket would
never receive an event in practice.

`SorobanEventStream` therefore presents one streaming interface over two
interchangeable transports:

| Transport | When it is used | Behaviour |
|---|---|---|
| `websocket` | `wsUrl` is set and reachable | Real `ws` connection, JSON-RPC `subscribeEvents` framing, ping heartbeats, exponential-backoff reconnect |
| `poll` | Default, or whenever the socket is down | Cursor-driven `getEvents` long-poll producing identical envelopes |

The client **auto-selects and self-heals**: it prefers WebSocket, falls back to
polling so data never stops flowing, and keeps retrying the socket in the
background to upgrade when it recovers. Both paths converge on the same
de-duplicated, normalized envelope.

## Event envelope

Raw ScVal XDR is decoded into a stable shape used by every stage:

```jsonc
{
  "id": "...", "cursor": "...", "ledger": 42, "ts": 1769817600000,
  "contractId": "C...", "txHash": "...",
  "protocol": "aegis", "action": "transfer",
  "topics": ["aegis", "transfer", "G...", "G..."],
  "fields": { "from": "G...", "to": "G...", "amount": 250n },
  "subjects": ["G...", "G..."],
  "raw": { "topic": ["AAAA..."], "value": "AAAA..." }
}
```

`i128` amounts decode to **BigInt**, so token values are exact at every stage.

## Filtering

Filters are plain objects; clauses AND together, arrays inside a clause OR.

```js
{ action: ['mint','transfer'], address: 'G...', minAmount: 1_000_000n,
  ledgerFrom: 100, successOnly: true,
  topicMatch: ['aegis','*','G...'],        // '*' single, '**' rest
  predicate: (e) => e.fields.totalSupply > 0n }
```

## Alert patterns

| Pattern | Fires when |
|---|---|
| `match` | any event matches the filter |
| `threshold` | a numeric field crosses `gt`/`gte`/`lt`/`lte` |
| `rate` | N matching events within a rolling window |
| `sequence` | an ordered chain occurs, optionally correlated by address |
| `absence` | no matching event within `withinMs` |

All rules support `severity`, `cooldownMs` and custom `message`. Sinks: console,
webhook (`AEGIS_ALERT_WEBHOOK`), or any async function.

Defaults ship for the protocol's real risks: `whale-transfer`, `large-mint`,
`mint-burst`, `whitelist-velocity`, `instant-drain` (whitelist → mint →
immediate transfer out), `failed-contract-call`, `stream-stalled`.

## Persistence & replay

Append-only JSONL with buffered writes, an in-memory ring buffer, and cursor
checkpointing so a restart resumes without gaps or duplicates.

```js
await store.replay(handler, { filter: { action: 'transfer' }, speed: 4 });
```

`speed: 0` replays instantly; `> 0` replays using original inter-event timing
divided by the multiplier. Corrupt lines are skipped, never fatal.

## Triggers

Automated actions with execution guards: `once`, `debounceMs`, `throttleMs`,
`maxRuns`, `retries`, and runtime `enabled` toggling via the API.

## Dashboard

`GET /` serves a zero-build UI (live table, KPIs, throughput chart, alerts).

| Endpoint | Purpose |
|---|---|
| `GET /api/health` | service + transport health |
| `GET /api/stats` | counters for every stage |
| `GET /api/analytics` | rolling analytics snapshot |
| `GET /api/events` | recent events (`?action=`,`?address=`,`?minAmount=`,`?source=disk`) |
| `GET /api/alerts` | alert history (`?severity=`) |
| `GET /api/rules` · `/api/routes` · `/api/triggers` | configuration |
| `POST /api/triggers/:name/toggle` | enable/disable a trigger |
| `POST /api/replay` | replay persisted events |
| `WS /ws` | live `event` / `alert` / `analytics` frames |

## Configuration

Every value is env-overridable — see `config.example.json`.

`AEGIS_NETWORK`, `AEGIS_RPC_URL`, `AEGIS_RPC_WS_URL`, `AEGIS_CONTRACT_IDS`,
`AEGIS_POLL_INTERVAL_MS`, `AEGIS_STORE_PATH`, `AEGIS_DASHBOARD_PORT`,
`AEGIS_ALERT_WEBHOOK`, `AEGIS_VERBOSE`, …

## Testing

```bash
npm test    # 106 tests
```

Coverage includes real `ws` client/server streaming, reconnection, HTTP
fallback, cursor advancement, de-duplication, all five alert patterns, replay,
crash recovery, and the full dashboard API.

`tests/onchain-compat.test.js` is the contract↔monitor seam: its payloads are
the **exact XDR emitted by the deployed Rust contract**, captured via

```bash
cargo test dump_event_xdr -- --ignored --nocapture
```

If a contract event's topics or field order change without a matching decoder
update, those tests fail — catching silent drift before production.
