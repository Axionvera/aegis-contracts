# Compliance registry reads and indexing strategy

## Decision

The compliance registry is **not an on-chain enumerable list** in the current
contract version. It is an address-keyed set:

- `is_whitelisted(address)` is the supported on-chain read for a known address.
- `get_investor_eligibility(address)` is the supported composed read for a
  known address (whitelist state, balance, cap and pause state).
- There is no `count`, `at(index)`, page endpoint, iterator, or total-size
  endpoint. A dashboard must not attempt to discover the registry by scanning
  contract storage or by guessing Soroban storage keys.

A list of investors is therefore an **off-chain, event-indexed projection**.
The contract remains the source of truth for authorization at transaction
execution time; an index is a discovery and presentation aid, not a permission
system.

## Why there is no on-chain enumeration

An on-chain array or iterator would add storage and write costs to every
whitelist change, create deletion/compaction and pagination semantics, and
make large registries expensive to read. It would also expose a complete
registry to every chain reader, which may be undesirable even though the
contract only stores addresses and no KYC documents. The current key/value
layout is cheap and has a stable O(1) membership check, but intentionally does
not provide ordered enumeration.

An on-chain list could be considered in a future version when a bounded
registry, a documented ordering rule, and a migration/versioning plan are
required. It must not be inferred from the current storage layout.

## Canonical event projection

Index committed events from the contract and network identified by the
application configuration. The event schema is documented in
[`events.md`](events.md):

| Event | Projection operation |
| --- | --- |
| `user_whitelisted(caller, user)` | add `user` (idempotently) |
| `whitelist_revoked(caller, user)` | remove or mark `user` inactive |

For the full compliance **status transition model** behind these events —
approved, revoked, blocked, pending, and unknown statuses, the deterministic
transition matrix under authorised and unauthorised callers, and the
invariant transition tests that guard them — see
[`compliance-status-transitions.md`](compliance-status-transitions.md).

Only events from **successful, committed transactions** are valid. Soroban
rolls back events from failed invocations. Store the ledger/transaction/event
identifier and contract ID with every record so ingestion is idempotent and
reorg/replay handling is possible. Preserve the event ledger sequence (or an
explicit ingestion cursor) and use deterministic ordering for API results,
for example `(first_seen_ledger, user_address)`.

### Reconciliation and consistency

Event indexing is eventually consistent. On startup, an indexer should replay
from a configured deployment/genesis ledger, then follow new ledgers and
persist its cursor only after the batch is durably written. It should support
replay from a prior cursor and deduplicate by the network's event identity.
Operators should periodically reconcile a sample (or every known address) by
calling `is_whitelisted`; a mismatch means the index is stale or incomplete,
not that the contract has accepted an invalid address. A new deployment or
network requires a new index namespace and a fresh replay.

Revocation is authoritative: after a committed `whitelist_revoked` event, the
projection must not present the address as currently whitelisted. Historical
records may retain the event for audit purposes. If an indexer missed an event,
it cannot safely reconstruct the full set from `is_whitelisted` without already
having candidate addresses; it must backfill the event range or mark the result
incomplete.

## Pagination contract for off-chain APIs

The SDK or indexer service should expose cursor pagination, not page numbers:

- sort by a stable key (ledger/event sequence, then normalized address);
- return an opaque cursor containing the last sort key and index version;
- accept `limit` with a server-enforced maximum;
- return `items`, `next_cursor`, and an `as_of_ledger` (or equivalent);
- make the snapshot/cutoff explicit so pages do not silently mix registry
  states while new events arrive.

A `whitelisted=true` result means “active in this index snapshot.” Clients that
need an execution-time answer must still call the contract read immediately
before submission (and handle a subsequent transaction failure).

## Responsibilities

### Contract

- Enforce whitelist status during mint and transfer.
- Emit the documented events only after the state change can commit.
- Keep `is_whitelisted` stable for point lookups.

### Indexer / SDK

- Discover members from committed `user_whitelisted` and
  `whitelist_revoked` events.
- Filter by contract ID and network; deduplicate and handle replay.
- Expose bounded cursor pagination and freshness/snapshot metadata.
- Provide `isWhitelisted(address)` and eligibility helpers as direct contract
  reads, rather than treating the index as authoritative.
- Surface an incomplete/stale index instead of returning a falsely complete
  list.

### Dashboard

- Use the paginated index only for browsing/search and clearly label its
  freshness.
- Do not assume a returned page is the complete registry or that an empty page
  proves there are no investors.
- Refresh point reads for an individual investor and handle revocation, pause,
  and compliance errors at submission time.
- Do not expose KYC claims or invent investor metadata from an address/event.

## Unsupported cases and future options

The current contracts do **not** support on-chain “all whitelisted investors,”
`offset`/`limit` reads, a registry count, arbitrary address search, historical
membership snapshots from contract calls, or guaranteed real-time dashboard
lists. An indexer cannot find addresses whose whitelist events predate its
configured start ledger, were emitted by another deployment, or were missed
and not backfilled.

Possible future designs are (1) a versioned on-chain bounded registry with
explicit pagination, (2) a first-class registry/index contract, or (3) a
standardized indexer API. Any option must specify ordering, revocation,
snapshot/finality behavior, migration, and privacy expectations; none changes
the requirement to perform execution-time checks.
