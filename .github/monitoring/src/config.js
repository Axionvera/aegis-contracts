/**
 * Central configuration for the Aegis monitoring service.
 *
 * Every value can be overridden with an environment variable so the service can
 * be pointed at local / testnet / mainnet RPC without code changes.
 */

const NETWORKS = {
  local: {
    rpcUrl: 'http://localhost:8000/soroban/rpc',
    networkPassphrase: 'Standalone Network ; February 2017',
  },
  testnet: {
    rpcUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
  },
  futurenet: {
    rpcUrl: 'https://rpc-futurenet.stellar.org',
    networkPassphrase: 'Test SDF Future Network ; October 2022',
  },
  mainnet: {
    rpcUrl: 'https://mainnet.sorobanrpc.com',
    networkPassphrase: 'Public Global Stellar Network ; September 2015',
  },
};

function envInt(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw === '') return fallback;
  const parsed = Number.parseInt(raw, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function envBool(name, fallback) {
  const raw = process.env[name];
  if (raw === undefined || raw === '') return fallback;
  return ['1', 'true', 'yes', 'on'].includes(raw.toLowerCase());
}

function envList(name, fallback = []) {
  const raw = process.env[name];
  if (!raw) return fallback;
  return raw
    .split(',')
    .map((s) => s.trim())
    .filter(Boolean);
}

export function loadConfig(overrides = {}) {
  const networkName = overrides.network || process.env.AEGIS_NETWORK || 'testnet';
  const network = NETWORKS[networkName] || NETWORKS.testnet;

  const config = {
    /** Network preset name: local | testnet | futurenet | mainnet */
    network: networkName,

    /** Soroban RPC endpoint (HTTP JSON-RPC, used for getEvents polling fallback). */
    rpcUrl: process.env.AEGIS_RPC_URL || network.rpcUrl,

    /**
     * Optional native WebSocket endpoint. Soroban RPC does not ship a public
     * subscription API yet, so this is only used when an operator runs an
     * RPC/indexer that exposes one. When unset (or when the socket cannot be
     * reached) the client transparently degrades to HTTP long-polling.
     */
    wsUrl: process.env.AEGIS_RPC_WS_URL || null,

    networkPassphrase: process.env.AEGIS_NETWORK_PASSPHRASE || network.networkPassphrase,

    /** Contract IDs to monitor. Empty = monitor every contract event. */
    contractIds: envList('AEGIS_CONTRACT_IDS', overrides.contractIds || []),

    /** Polling cadence for the HTTP fallback, in milliseconds. */
    pollIntervalMs: envInt('AEGIS_POLL_INTERVAL_MS', 2000),

    /** Max events requested per getEvents page. */
    pageLimit: envInt('AEGIS_PAGE_LIMIT', 100),

    /** How many ledgers to look back on a cold start. */
    startLedgerLookback: envInt('AEGIS_START_LEDGER_LOOKBACK', 120),

    /** Reconnect backoff (exponential, capped). */
    reconnect: {
      initialDelayMs: envInt('AEGIS_RECONNECT_INITIAL_MS', 500),
      maxDelayMs: envInt('AEGIS_RECONNECT_MAX_MS', 30000),
      factor: 2,
      jitter: 0.2,
      maxAttempts: envInt('AEGIS_RECONNECT_MAX_ATTEMPTS', 0), // 0 = unlimited
    },

    /** Event persistence. */
    store: {
      /** Path to the append-only JSONL event log. */
      path: process.env.AEGIS_STORE_PATH || './data/events.jsonl',
      /** Ring-buffer size held in memory for fast replay/analytics. */
      memoryLimit: envInt('AEGIS_STORE_MEMORY_LIMIT', 10000),
      /** Flush to disk after this many events or this many ms, whichever first. */
      flushEvery: envInt('AEGIS_STORE_FLUSH_EVERY', 25),
      flushIntervalMs: envInt('AEGIS_STORE_FLUSH_INTERVAL_MS', 1000),
      enabled: envBool('AEGIS_STORE_ENABLED', true),
    },

    /** Analytics dashboard + WebSocket fan-out server. */
    dashboard: {
      enabled: envBool('AEGIS_DASHBOARD_ENABLED', true),
      host: process.env.AEGIS_DASHBOARD_HOST || '127.0.0.1',
      port: envInt('AEGIS_DASHBOARD_PORT', 4500),
    },

    /** Analytics rollup window in milliseconds. */
    analytics: {
      windowMs: envInt('AEGIS_ANALYTICS_WINDOW_MS', 5 * 60 * 1000),
      bucketMs: envInt('AEGIS_ANALYTICS_BUCKET_MS', 10 * 1000),
    },

    /** Emit verbose logs. */
    verbose: envBool('AEGIS_VERBOSE', false),
  };

  return { ...config, ...overrides, store: { ...config.store, ...(overrides.store || {}) } };
}

export { NETWORKS };
