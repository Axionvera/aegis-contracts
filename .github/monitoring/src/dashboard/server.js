/**
 * Analytics dashboard: HTTP JSON API + live WebSocket fan-out + static UI.
 *
 * Endpoints
 *   GET  /                       dashboard UI (single-file, no build step)
 *   GET  /api/health             service + RPC transport health
 *   GET  /api/stats              stream/router/alert/trigger/store counters
 *   GET  /api/analytics          rolling analytics snapshot
 *   GET  /api/events?limit&action&address&contractId&minAmount
 *   GET  /api/alerts?limit&severity
 *   GET  /api/rules              configured alert rules
 *   GET  /api/routes             configured routes
 *   GET  /api/triggers           configured triggers
 *   POST /api/triggers/:name/toggle   enable/disable a trigger at runtime
 *   POST /api/replay             replay persisted events {filter, limit, speed}
 *
 * WebSocket (same port): pushes {type:'event'|'alert'|'analytics'|'hello'}
 * frames to every connected browser, giving the UI true real-time updates.
 */

import http from 'node:http';
import { WebSocketServer } from 'ws';
import { serializeEvent } from '../events/normalize.js';
import { sanitizeFilter } from '../events/filter.js';
import { DASHBOARD_HTML } from './ui.js';

/**
 * JSON responder.
 *
 * Uses a BigInt-aware replacer as a safety net: normalized Aegis events carry
 * i128 amounts as BigInt, and a stray one must degrade to a decimal string
 * rather than throwing a 500 out of the monitoring dashboard.
 */
function bigIntReplacer(_key, value) {
  return typeof value === 'bigint' ? value.toString() : value;
}

function json(res, status, payload) {
  const body = JSON.stringify(payload, bigIntReplacer, 2);
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(body),
    'Cache-Control': 'no-store',
    'Access-Control-Allow-Origin': '*',
  });
  res.end(body);
}

async function readBody(req, limitBytes = 1_000_000) {
  const chunks = [];
  let size = 0;
  for await (const chunk of req) {
    size += chunk.length;
    if (size > limitBytes) throw new Error('Request body too large');
    chunks.push(chunk);
  }
  if (!chunks.length) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    throw new Error('Invalid JSON body');
  }
}

export class DashboardServer {
  /**
   * @param {object} deps { stream, router, alerts, store, triggers, analytics, config }
   */
  constructor(deps = {}) {
    this.deps = deps;
    this.host = deps.config?.dashboard?.host ?? '127.0.0.1';
    this.port = deps.config?.dashboard?.port ?? 4500;
    this.logger = deps.logger ?? (() => {});
    this.server = null;
    this.wss = null;
    this.clients = new Set();
    this._analyticsTimer = null;
  }

  async start() {
    this.server = http.createServer((req, res) => {
      this._handle(req, res).catch((error) => {
        this.logger('error', `dashboard request failed: ${error.message}`);
        if (!res.headersSent) json(res, 500, { error: error.message });
      });
    });

    this.wss = new WebSocketServer({ server: this.server, path: '/ws' });
    this.wss.on('connection', (socket) => {
      this.clients.add(socket);
      socket.on('close', () => this.clients.delete(socket));
      socket.on('error', () => this.clients.delete(socket));
      this._send(socket, {
        type: 'hello',
        payload: {
          network: this.deps.config?.network,
          transport: this.deps.stream?.transport,
          recent: this.deps.store?.recent({ limit: 25 }) ?? [],
          analytics: this.deps.analytics?.snapshot() ?? null,
        },
      });
    });

    await new Promise((resolve, reject) => {
      this.server.once('error', reject);
      this.server.listen(this.port, this.host, () => {
        this.server.removeListener('error', reject);
        resolve();
      });
    });

    // Push an analytics refresh to all clients on a cadence.
    this._analyticsTimer = setInterval(() => {
      this.broadcast('analytics', this.deps.analytics?.snapshot() ?? null);
    }, 2000);
    if (typeof this._analyticsTimer.unref === 'function') this._analyticsTimer.unref();

    const addr = this.server.address();
    this.port = typeof addr === 'object' && addr ? addr.port : this.port;
    this.logger('info', `Dashboard listening on http://${this.host}:${this.port}`);
    return this;
  }

  _send(socket, message) {
    if (socket.readyState !== 1) return;
    try {
      socket.send(JSON.stringify(message, bigIntReplacer));
    } catch {
      /* client vanished or payload not serializable */
    }
  }

  /** Fan a message out to every connected dashboard client. */
  broadcast(type, payload) {
    if (!this.clients.size) return 0;
    let message;
    try {
      message = JSON.stringify({ type, payload, ts: Date.now() }, bigIntReplacer);
    } catch {
      return 0; // never let a bad payload take down the fan-out loop
    }
    let sent = 0;
    for (const socket of this.clients) {
      if (socket.readyState !== 1) continue;
      try {
        socket.send(message);
        sent += 1;
      } catch {
        this.clients.delete(socket);
      }
    }
    return sent;
  }

  _filterFromQuery(url) {
    const q = url.searchParams;
    const filter = {};
    if (q.get('action')) filter.action = q.get('action').split(',');
    if (q.get('address')) filter.address = q.get('address');
    if (q.get('contractId')) filter.contractId = q.get('contractId');
    if (q.get('minAmount')) filter.minAmount = q.get('minAmount');
    if (q.get('maxAmount')) filter.maxAmount = q.get('maxAmount');
    if (q.get('ledgerFrom')) filter.ledgerFrom = Number(q.get('ledgerFrom'));
    if (q.get('ledgerTo')) filter.ledgerTo = Number(q.get('ledgerTo'));
    return Object.keys(filter).length ? filter : null;
  }

  async _handle(req, res) {
    const url = new URL(req.url, `http://${req.headers.host ?? 'localhost'}`);
    const { pathname } = url;
    const { stream, router, alerts, store, triggers, analytics, config } = this.deps;

    if (req.method === 'OPTIONS') {
      res.writeHead(204, {
        'Access-Control-Allow-Origin': '*',
        'Access-Control-Allow-Methods': 'GET,POST,OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type',
      });
      return res.end();
    }

    if (pathname === '/' || pathname === '/index.html') {
      const body = DASHBOARD_HTML;
      res.writeHead(200, {
        'Content-Type': 'text/html; charset=utf-8',
        'Content-Length': Buffer.byteLength(body),
      });
      return res.end(body);
    }

    if (pathname === '/api/health') {
      return json(res, 200, {
        status: 'ok',
        network: config?.network,
        rpcUrl: config?.rpcUrl,
        wsUrl: config?.wsUrl,
        transport: stream?.transport ?? 'idle',
        uptimeMs: stream?.getStats?.().uptimeMs ?? 0,
        clients: this.clients.size,
      });
    }

    if (pathname === '/api/stats') {
      return json(res, 200, {
        stream: stream?.getStats?.() ?? {},
        router: router?.getStats?.() ?? {},
        alerts: alerts?.getStats?.() ?? {},
        triggers: triggers?.getStats?.() ?? {},
        store: store?.getStats?.() ?? {},
        dashboardClients: this.clients.size,
      });
    }

    if (pathname === '/api/analytics') {
      return json(res, 200, analytics?.snapshot() ?? {});
    }

    if (pathname === '/api/events') {
      const limit = Math.min(Number(url.searchParams.get('limit') ?? 100), 1000);
      const filter = this._filterFromQuery(url);
      const source = url.searchParams.get('source');
      if (source === 'disk' && store) {
        const events = await store.query({ filter, limit });
        return json(res, 200, { count: events.length, source: 'disk', events: events.map(serializeEvent) });
      }
      const events = store?.recent({ limit, filter }) ?? [];
      return json(res, 200, { count: events.length, source: 'memory', events });
    }

    if (pathname === '/api/alerts') {
      const limit = Math.min(Number(url.searchParams.get('limit') ?? 50), 500);
      const severity = url.searchParams.get('severity');
      return json(res, 200, { alerts: alerts?.getHistory({ limit, severity }) ?? [] });
    }

    if (pathname === '/api/rules') {
      return json(res, 200, { rules: alerts?.listRules() ?? [] });
    }

    if (pathname === '/api/routes') {
      return json(res, 200, { routes: router?.listRoutes() ?? [] });
    }

    if (pathname === '/api/triggers' && req.method === 'GET') {
      return json(res, 200, { triggers: triggers?.list() ?? [] });
    }

    const toggleMatch = pathname.match(/^\/api\/triggers\/([^/]+)\/toggle$/);
    if (toggleMatch && req.method === 'POST') {
      const name = decodeURIComponent(toggleMatch[1]);
      const body = await readBody(req);
      const ok = triggers?.enable(name, body.enabled !== false);
      if (!ok) return json(res, 404, { error: `Unknown trigger: ${name}` });
      return json(res, 200, { trigger: name, enabled: body.enabled !== false });
    }

    if (pathname === '/api/replay' && req.method === 'POST') {
      const body = await readBody(req);
      const collected = [];
      const count = await store.replay(
        (event) => {
          collected.push(serializeEvent(event));
          this.broadcast('replay', serializeEvent(event));
        },
        {
          filter: body.filter ?? null,
          limit: body.limit ?? 500,
          speed: body.speed ?? 0,
        },
      );
      return json(res, 200, {
        replayed: count,
        filter: sanitizeFilter(body.filter ?? {}),
        events: body.includeEvents === false ? undefined : collected.slice(0, 200),
      });
    }

    return json(res, 404, { error: 'Not found', path: pathname });
  }

  async stop() {
    if (this._analyticsTimer) clearInterval(this._analyticsTimer);
    for (const socket of this.clients) {
      try {
        socket.close();
      } catch {
        /* ignore */
      }
    }
    this.clients.clear();
    if (this.wss) await new Promise((resolve) => this.wss.close(resolve));
    if (this.server) await new Promise((resolve) => this.server.close(resolve));
    this.server = null;
    this.wss = null;
  }
}
