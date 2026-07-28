#!/usr/bin/env node
/**
 * aegis-monitor - CLI entry point.
 *
 * Usage:
 *   aegis-monitor                          stream from the configured RPC
 *   aegis-monitor --simulate               run a self-contained demo (no network)
 *   aegis-monitor --network testnet --contract C...
 *   aegis-monitor --replay --filter-action transfer --speed 4
 *   aegis-monitor --no-dashboard --verbose
 */

import process from 'node:process';
import { AegisMonitor, createLogger } from './service.js';
import { generateLifecycle, MockSorobanWebSocketServer } from './simulator.js';

function parseArgs(argv) {
  const args = { flags: new Set(), opts: {} };
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (!token.startsWith('--')) continue;
    const key = token.slice(2);
    const next = argv[i + 1];
    if (next && !next.startsWith('--')) {
      args.opts[key] = next;
      i += 1;
    } else {
      args.flags.add(key);
    }
  }
  return args;
}

function usage() {
  console.log(`
aegis-monitor - real-time Soroban event monitoring for the Aegis RWA Protocol

Options:
  --simulate               Run against a built-in mock RPC WebSocket (no network)
  --network <name>         local | testnet | futurenet | mainnet
  --rpc-url <url>          Override the HTTP JSON-RPC endpoint
  --ws-url <url>           WebSocket endpoint offering subscribeEvents
  --contract <id>          Contract ID to monitor (repeatable via comma list)
  --port <n>               Dashboard port (default 4500)
  --no-dashboard           Disable the dashboard/HTTP server
  --replay                 Replay persisted events instead of streaming
  --filter-action <a>      Filter replay/stream by action (mint,transfer,...)
  --speed <n>              Replay speed multiplier (0 = instant)
  --limit <n>              Replay limit
  --verbose                Debug logging
  --help                   Show this help
`);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.flags.has('help')) return usage();

  const simulate = args.flags.has('simulate');
  const verbose = args.flags.has('verbose');
  const logger = createLogger({ verbose });

  let mockServer = null;
  const config = {
    verbose,
    ...(args.opts.network ? { network: args.opts.network } : {}),
    ...(args.opts['rpc-url'] ? { rpcUrl: args.opts['rpc-url'] } : {}),
    ...(args.opts['ws-url'] ? { wsUrl: args.opts['ws-url'] } : {}),
    ...(args.opts.contract ? { contractIds: args.opts.contract.split(',') } : {}),
    ...(args.flags.has('no-dashboard') ? { dashboard: { enabled: false } } : {}),
  };

  if (args.opts.port) {
    config.dashboard = { ...(config.dashboard ?? {}), enabled: !args.flags.has('no-dashboard'), port: Number(args.opts.port), host: '127.0.0.1' };
  }

  if (simulate) {
    mockServer = await new MockSorobanWebSocketServer().start();
    config.network = config.network ?? 'local';
    config.wsUrl = mockServer.url;
    config.rpcUrl = config.rpcUrl ?? 'http://127.0.0.1:1/unused';
    config.store = { path: './data/simulated-events.jsonl' };
    logger('info', `simulator RPC WebSocket at ${mockServer.url}`);
  }

  const monitor = new AegisMonitor({ config });

  if (args.flags.has('replay')) {
    await monitor.store.init();
    const filter = args.opts['filter-action'] ? { action: args.opts['filter-action'].split(',') } : null;
    logger('info', 'replaying persisted events…');
    let shown = 0;
    const count = await monitor.store.replay(
      (event) => {
        shown += 1;
        console.log(
          `#${String(shown).padStart(4)} ledger=${event.ledger} ${event.action ?? event.type} ` +
            `${JSON.stringify(event.fields, (_, v) => (typeof v === 'bigint' ? v.toString() : v))}`,
        );
      },
      { filter, limit: Number(args.opts.limit ?? 500), speed: Number(args.opts.speed ?? 0) },
    );
    logger('info', `replayed ${count} events`);
    await monitor.store.close();
    return;
  }

  await monitor.start();

  if (config.dashboard?.enabled !== false && monitor.dashboard) {
    logger('info', `dashboard: http://${monitor.dashboard.host}:${monitor.dashboard.port}`);
  }

  if (simulate) {
    const { events } = generateLifecycle({ users: 4, startLedger: 1000 });
    logger('info', `streaming ${events.length} simulated protocol events…`);
    let index = 0;
    const timer = setInterval(() => {
      if (index >= events.length) {
        // Loop with fresh ledgers so the dashboard keeps moving.
        const next = generateLifecycle({ users: 3, startLedger: 2000 + Math.floor(Math.random() * 1000) });
        events.push(...next.events);
      }
      mockServer.push(events[index++]);
    }, 900);
    timer.unref?.();
  }

  const shutdown = async (signal) => {
    logger('info', `received ${signal}, shutting down…`);
    await monitor.stop();
    if (mockServer) await mockServer.stop();
    process.exit(0);
  };
  process.on('SIGINT', () => shutdown('SIGINT'));
  process.on('SIGTERM', () => shutdown('SIGTERM'));

  setInterval(() => {
    const stats = monitor.getStats();
    logger(
      'info',
      `processed=${stats.processed} transport=${stats.stream.transport} ` +
        `alerts=${stats.alerts.fired} triggers=${stats.triggers.fired} stored=${stats.store.appended}`,
    );
  }, 30_000).unref?.();
}

main().catch((error) => {
  console.error(`fatal: ${error.stack ?? error.message}`);
  process.exit(1);
});
