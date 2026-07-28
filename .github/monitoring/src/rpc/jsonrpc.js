/**
 * Tiny JSON-RPC 2.0 client over HTTP(S) using the built-in fetch.
 *
 * Used for `getEvents` / `getLatestLedger` / `getHealth` against Soroban RPC.
 */

export class RpcError extends Error {
  constructor(message, { code, data, method } = {}) {
    super(message);
    this.name = 'RpcError';
    this.code = code;
    this.data = data;
    this.method = method;
  }
}

let requestCounter = 0;

export function nextRequestId() {
  requestCounter += 1;
  return requestCounter;
}

/** Reset the internal id counter (test helper). */
export function __resetRequestId() {
  requestCounter = 0;
}

/**
 * Perform a JSON-RPC call.
 *
 * @param {string} url         RPC endpoint
 * @param {string} method      JSON-RPC method name
 * @param {object} params      Method params
 * @param {object} [options]
 * @param {number} [options.timeoutMs=15000]
 * @param {typeof fetch} [options.fetchImpl] injectable for tests
 * @param {object} [options.headers]
 */
export async function rpcCall(url, method, params = {}, options = {}) {
  const { timeoutMs = 15000, fetchImpl = globalThis.fetch, headers = {} } = options;

  if (typeof fetchImpl !== 'function') {
    throw new RpcError('No fetch implementation available (Node >= 18 required)', { method });
  }

  const body = JSON.stringify({
    jsonrpc: '2.0',
    id: nextRequestId(),
    method,
    params,
  });

  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);

  let response;
  try {
    response = await fetchImpl(url, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', ...headers },
      body,
      signal: controller.signal,
    });
  } catch (error) {
    if (error.name === 'AbortError') {
      throw new RpcError(`RPC ${method} timed out after ${timeoutMs}ms`, { method });
    }
    throw new RpcError(`RPC ${method} transport error: ${error.message}`, { method });
  } finally {
    clearTimeout(timer);
  }

  if (!response.ok) {
    throw new RpcError(`RPC ${method} HTTP ${response.status}`, {
      method,
      code: response.status,
    });
  }

  let payload;
  try {
    payload = await response.json();
  } catch (error) {
    throw new RpcError(`RPC ${method} returned malformed JSON: ${error.message}`, { method });
  }

  if (payload.error) {
    throw new RpcError(payload.error.message || `RPC ${method} failed`, {
      method,
      code: payload.error.code,
      data: payload.error.data,
    });
  }

  return payload.result;
}
