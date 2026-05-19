import { HandlerError } from "./errors";

/*
 * BONO-AI proactive seed per R-42 Registration Pass #2:
 *   sliding-window per-key rate limiter with short+long parallel windows (REG-3 throttle pattern).
 *
 * Production SEAM:
 *   - Redis with `hitWindows(key, now_ms, short_window_ms, long_window_ms)` interface
 *   - Sorted-set per key, ZADD timestamp, ZREMRANGEBYSCORE for window slide, ZCARD for count
 *
 * REG-3 default budget:
 *   - OTP request: ≤1 per 30s per phone + ≤5 per hour per phone
 *   - OTP verify:  ≤5 wrong codes per otp_request_id → invalidate + ≤10 per hour per phone
 *
 * Dev stub: in-memory Map of timestamps per key. Throws on production.
 */

if (process.env.NODE_ENV === "production") {
  throw new Error(
    "rateLimit dev-stub reached in production — wire Redis sorted-set backend before deploy"
  );
}

export interface RateLimitWindow {
  durationMs: number;
  maxHits: number;
}

export interface RateLimitResult {
  allowed: boolean;
  retryAfterMs: number;
  shortHits: number;
  longHits: number;
  shortMax: number;
  longMax: number;
}

const store = new Map<string, number[]>();

export async function hitWindows(
  key: string,
  nowMs: number,
  short: RateLimitWindow,
  long: RateLimitWindow
): Promise<RateLimitResult> {
  const cutoff = nowMs - Math.max(short.durationMs, long.durationMs);
  const existing = (store.get(key) ?? []).filter((ts) => ts >= cutoff);

  const shortHits = existing.filter((ts) => ts >= nowMs - short.durationMs).length;
  const longHits = existing.filter((ts) => ts >= nowMs - long.durationMs).length;

  const shortBlocked = shortHits >= short.maxHits;
  const longBlocked = longHits >= long.maxHits;
  const allowed = !shortBlocked && !longBlocked;

  let retryAfterMs = 0;
  if (shortBlocked) {
    const oldestInShort = existing.filter((ts) => ts >= nowMs - short.durationMs).sort()[0];
    retryAfterMs = Math.max(retryAfterMs, short.durationMs - (nowMs - oldestInShort));
  }
  if (longBlocked) {
    const oldestInLong = existing.filter((ts) => ts >= nowMs - long.durationMs).sort()[0];
    retryAfterMs = Math.max(retryAfterMs, long.durationMs - (nowMs - oldestInLong));
  }

  if (allowed) {
    existing.push(nowMs);
    store.set(key, existing);
  } else {
    store.set(key, existing);
  }

  return {
    allowed,
    retryAfterMs,
    shortHits: allowed ? shortHits + 1 : shortHits,
    longHits: allowed ? longHits + 1 : longHits,
    shortMax: short.maxHits,
    longMax: long.maxHits,
  };
}

export function rateLimitedError(retryAfterMs: number, quietPosture = false): HandlerError {
  if (quietPosture) {
    return new HandlerError(429, {
      error: "throttled",
      message: "Please wait a moment before trying again.",
    });
  }
  const seconds = Math.ceil(retryAfterMs / 1000);
  return new HandlerError(429, {
    error: "rate_limited",
    message: `Too many attempts. Please try again in ${seconds} second${seconds === 1 ? "" : "s"}.`,
    staff_detail: `retry_after_ms=${retryAfterMs}`,
  });
}
