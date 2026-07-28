/**
 * Exponential backoff with jitter, used by the reconnecting WebSocket client.
 */
export class Backoff {
  constructor({ initialDelayMs = 500, maxDelayMs = 30000, factor = 2, jitter = 0.2 } = {}) {
    this.initialDelayMs = initialDelayMs;
    this.maxDelayMs = maxDelayMs;
    this.factor = factor;
    this.jitter = jitter;
    this.attempt = 0;
  }

  /** Next delay in ms; advances the attempt counter. */
  next(random = Math.random) {
    const raw = this.initialDelayMs * this.factor ** this.attempt;
    const capped = Math.min(raw, this.maxDelayMs);
    this.attempt += 1;
    if (!this.jitter) return Math.round(capped);
    // full-spectrum +/- jitter
    const delta = capped * this.jitter;
    const jittered = capped - delta + random() * delta * 2;
    return Math.max(0, Math.round(Math.min(jittered, this.maxDelayMs)));
  }

  reset() {
    this.attempt = 0;
  }
}
