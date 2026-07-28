/**
 * Default routes, alert rules and triggers for the Aegis protocol.
 *
 * These encode the protocol's real operational risks: unauthorized-looking
 * compliance activity, whale movements, mint bursts and stream stalls.
 * Everything here is data, so an operator can override it without touching the
 * engine code (see `monitoring/config.example.json`).
 */

import { SEVERITY } from './alerts/index.js';

/** Named routes: filter + a light handler that tags the event for consumers. */
export function defaultRoutes({ logger = () => {} } = {}) {
  return [
    {
      name: 'compliance',
      filter: { action: ['wl_add', 'init'] },
      priority: 100,
      handler: (event) => logger('debug', `compliance route: ${event.action}`),
    },
    {
      name: 'treasury',
      filter: { action: ['mint', 'yield'] },
      priority: 90,
      handler: (event) => logger('debug', `treasury route: ${event.action}`),
    },
    {
      name: 'transfers',
      filter: { action: 'transfer' },
      priority: 80,
      handler: (event) => logger('debug', `transfer route: ${event.fields?.amount}`),
    },
    {
      name: 'failed-calls',
      filter: { predicate: (event) => event.inSuccessfulContractCall === false },
      priority: 120,
      handler: (event) => logger('warn', `event from failed call: ${event.id}`),
    },
  ];
}

/** Alert rules covering all five supported patterns. */
export function defaultAlertRules({ whaleThreshold = 1_000_000n, mintBurst = 5 } = {}) {
  return [
    {
      name: 'whale-transfer',
      pattern: 'threshold',
      description: 'Transfer at or above the whale threshold',
      filter: { action: 'transfer' },
      field: 'amount',
      gte: whaleThreshold,
      severity: SEVERITY.WARNING,
      cooldownMs: 5_000,
    },
    {
      name: 'large-mint',
      pattern: 'threshold',
      description: 'Single mint exceeding the supply-shock threshold',
      filter: { action: 'mint' },
      field: 'amount',
      gte: whaleThreshold,
      severity: SEVERITY.CRITICAL,
      cooldownMs: 5_000,
    },
    {
      name: 'mint-burst',
      pattern: 'rate',
      description: 'Unusual number of mints in a short window',
      filter: { action: 'mint' },
      count: mintBurst,
      windowMs: 60_000,
      severity: SEVERITY.WARNING,
      cooldownMs: 30_000,
    },
    {
      name: 'whitelist-velocity',
      pattern: 'rate',
      description: 'Rapid compliance whitelist expansion',
      filter: { action: 'wl_add' },
      count: 10,
      windowMs: 60_000,
      severity: SEVERITY.WARNING,
      cooldownMs: 60_000,
    },
    {
      name: 'instant-drain',
      pattern: 'sequence',
      description: 'Address whitelisted, minted to, then immediately transfers out',
      steps: [{ action: 'wl_add' }, { action: 'mint' }, { action: 'transfer' }],
      correlateBy: 'address',
      windowMs: 120_000,
      severity: SEVERITY.CRITICAL,
      cooldownMs: 10_000,
    },
    {
      name: 'failed-contract-call',
      pattern: 'match',
      description: 'Event emitted from an unsuccessful contract call',
      filter: { predicate: (event) => event.inSuccessfulContractCall === false },
      severity: SEVERITY.CRITICAL,
      cooldownMs: 5_000,
    },
    {
      name: 'stream-stalled',
      pattern: 'absence',
      description: 'No protocol activity observed within the idle window',
      filter: { protocol: 'aegis' },
      withinMs: 15 * 60_000,
      severity: SEVERITY.WARNING,
      cooldownMs: 15 * 60_000,
    },
  ];
}

/** Event-based triggers with sane execution guards. */
export function defaultTriggers({ logger = () => {}, whaleThreshold = 1_000_000n } = {}) {
  return [
    {
      name: 'audit-log-compliance',
      description: 'Record every whitelist addition to the audit log',
      filter: { action: 'wl_add' },
      action: (event) =>
        logger('info', `AUDIT whitelist user=${event.fields?.user} admin=${event.fields?.admin} ledger=${event.ledger}`),
    },
    {
      name: 'flag-whale-transfer',
      description: 'Flag very large transfers for manual review (throttled)',
      filter: { action: 'transfer', minAmount: whaleThreshold },
      throttleMs: 10_000,
      action: (event) =>
        logger('warn', `REVIEW whale transfer ${event.fields?.amount} from=${event.fields?.from} to=${event.fields?.to}`),
    },
    {
      name: 'supply-checkpoint',
      description: 'Checkpoint total supply after mints (debounced to batch bursts)',
      filter: { action: 'mint' },
      debounceMs: 1_000,
      action: (event) =>
        logger('info', `SUPPLY checkpoint totalSupply=${event.fields?.totalSupply} ledger=${event.ledger}`),
    },
    {
      name: 'first-deployment',
      description: 'Fires once when a contract initialization is observed',
      filter: { action: 'init' },
      once: true,
      action: (event) => logger('info', `DEPLOY detected admin=${event.fields?.admin} contract=${event.contractId}`),
    },
  ];
}
