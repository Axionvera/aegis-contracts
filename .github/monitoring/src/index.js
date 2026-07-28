/**
 * @aegis/monitoring - public API surface.
 */

export { AegisMonitor, createLogger, TRANSPORT } from './service.js';
export { SorobanEventStream } from './rpc/websocket-client.js';
export { rpcCall, RpcError } from './rpc/jsonrpc.js';
export { Backoff } from './rpc/backoff.js';
export { EventRouter, matchesFilter, matchTopics, sanitizeFilter } from './events/filter.js';
export { normalizeEvent, serializeEvent, amountOf, ACTION_SCHEMA, NAMESPACE } from './events/normalize.js';
export { decodeScVal, decodeTopics, encodeSymbol, encodeStrkey, jsonSafe } from './events/scval.js';
export { AlertEngine, SEVERITY, sinks } from './alerts/index.js';
export { EventStore } from './store/event-store.js';
export { TriggerEngine, actions } from './triggers/index.js';
export { AnalyticsEngine } from './analytics/index.js';
export { DashboardServer } from './dashboard/server.js';
export { loadConfig, NETWORKS } from './config.js';
export { defaultRoutes, defaultAlertRules, defaultTriggers } from './defaults.js';
export {
  build as buildEvent,
  generateLifecycle,
  makeAddress,
  MockSorobanWebSocketServer,
} from './simulator.js';
