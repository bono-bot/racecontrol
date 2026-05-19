/**
 * rateLimit.ts — sliding-window short+long parallel windows tests.
 *
 * REG-3 throttle pattern: ≤1/30s + ≤5/hour for OTP request.
 */

import { describe, it, expect } from "vitest";
import { hitWindows, rateLimitedError } from "./rateLimit";

const SHORT = { durationMs: 30_000, maxHits: 1 };
const LONG = { durationMs: 3_600_000, maxHits: 5 };

function uniqueKey(suffix: string): string {
  return `test:${suffix}:${Math.random().toString(36).slice(2, 8)}`;
}

describe("rateLimit · short-window enforcement", () => {
  it("first hit is allowed", async () => {
    const k = uniqueKey("short-1");
    const r = await hitWindows(k, Date.now(), SHORT, LONG);
    expect(r.allowed).toBe(true);
    expect(r.shortHits).toBe(1);
    expect(r.retryAfterMs).toBe(0);
  });

  it("second hit within short window is blocked with retryAfter > 0", async () => {
    const k = uniqueKey("short-2");
    const t0 = Date.now();
    await hitWindows(k, t0, SHORT, LONG);
    const r = await hitWindows(k, t0 + 1000, SHORT, LONG);
    expect(r.allowed).toBe(false);
    expect(r.retryAfterMs).toBeGreaterThan(0);
    expect(r.retryAfterMs).toBeLessThanOrEqual(SHORT.durationMs);
  });

  it("hit after short window expires is allowed again", async () => {
    const k = uniqueKey("short-3");
    const t0 = Date.now();
    await hitWindows(k, t0, SHORT, LONG);
    const r = await hitWindows(k, t0 + SHORT.durationMs + 1, SHORT, LONG);
    expect(r.allowed).toBe(true);
  });
});

describe("rateLimit · long-window enforcement", () => {
  it("blocks the 6th hit within long window even if spaced past short window", async () => {
    const k = uniqueKey("long-6");
    const t0 = Date.now();
    // 5 hits, each spaced just past short window — all should be allowed
    for (let i = 0; i < 5; i++) {
      const r = await hitWindows(k, t0 + i * (SHORT.durationMs + 1), SHORT, LONG);
      expect(r.allowed).toBe(true);
    }
    // 6th hit — within long window — should be blocked
    const r6 = await hitWindows(k, t0 + 5 * (SHORT.durationMs + 1), SHORT, LONG);
    expect(r6.allowed).toBe(false);
    expect(r6.longHits).toBe(5);
    expect(r6.longMax).toBe(5);
  });

  it("hit count reflects only un-expired entries", async () => {
    const k = uniqueKey("decay");
    const t0 = Date.now();
    await hitWindows(k, t0, SHORT, LONG);
    // Way past long window
    const r = await hitWindows(k, t0 + LONG.durationMs + 1000, SHORT, LONG);
    expect(r.allowed).toBe(true);
    expect(r.longHits).toBe(1); // The new hit; the old one has expired
  });
});

describe("rateLimit · error envelopes", () => {
  it("rateLimitedError default posture exposes retry hint to customer", () => {
    const err = rateLimitedError(45_000);
    expect(err.status).toBe(429);
    expect(err.body.error).toBe("rate_limited");
    expect(err.body.message).toContain("45 second");
    expect(err.body.staff_detail).toContain("retry_after_ms=45000");
  });

  it("rateLimitedError quiet posture suppresses retry hint (privacy class)", () => {
    const err = rateLimitedError(45_000, true);
    expect(err.status).toBe(429);
    expect(err.body.error).toBe("throttled");
    expect(err.body.message).not.toContain("45");
    expect(err.body.message).not.toContain("second");
  });
});
