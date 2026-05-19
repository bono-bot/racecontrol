/**
 * idempotency.ts — hit/miss/conflict/replay state-machine tests.
 *
 * R-41 invariant #3 + R-42 §4 SCHEMA-WALLET.md idempotency contract:
 *   - exact match → replay (200 + cached response)
 *   - same (op, actor) different request_id → conflict (409 concurrent retry)
 *   - same op different actor_scope → conflict (409 cross-actor IDOR sentinel)
 *   - 24h TTL
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  checkIdempotency,
  storeIdempotency,
  idempotencyConflictError,
  type IdempotencyKey,
} from "./idempotency";

function uniqueKey(suffix: string): IdempotencyKey {
  // dev stub uses module-level Map; use unique op_id per test to avoid bleed
  return {
    operation_id: `op-${suffix}-${Math.random().toString(36).slice(2, 8)}`,
    actor_scope: "actor-A",
    request_id: "req-1",
  };
}

describe("idempotency · miss → store → replay", () => {
  it("miss on fresh key", async () => {
    const k = uniqueKey("miss");
    const result = await checkIdempotency(k);
    expect(result.hit).toBe("miss");
  });

  it("replay on exact (op, actor, request_id) match", async () => {
    const k = uniqueKey("replay");
    await storeIdempotency(k, { ok: true, balance: 100 }, 200);
    const result = await checkIdempotency(k);
    expect(result.hit).toBe("replay");
    if (result.hit === "replay") {
      expect(result.entry.response_status).toBe(200);
      expect((result.entry.response_body as { balance: number }).balance).toBe(100);
    }
  });
});

describe("idempotency · conflict detection", () => {
  it("conflict on same (op, actor) but different request_id (concurrent retry sentinel)", async () => {
    const k1 = uniqueKey("conflict-retry");
    await storeIdempotency(k1, { v: 1 }, 200);
    const k2: IdempotencyKey = { ...k1, request_id: "req-2" };
    const result = await checkIdempotency(k2);
    expect(result.hit).toBe("conflict");
  });

  it("conflict on same op_id but different actor_scope (cross-actor IDOR sentinel)", async () => {
    const k1 = uniqueKey("conflict-actor");
    await storeIdempotency(k1, { v: 1 }, 200);
    const k2: IdempotencyKey = { ...k1, actor_scope: "actor-B" };
    const result = await checkIdempotency(k2);
    expect(result.hit).toBe("conflict");
  });
});

describe("idempotency · error envelope", () => {
  it("idempotencyConflictError produces HandlerError(409)", () => {
    const existing = {
      key: uniqueKey("err"),
      response_body: {},
      response_status: 200,
      created_at: Date.now(),
    };
    const err = idempotencyConflictError(existing);
    expect(err.status).toBe(409);
    expect(err.body.error).toBe("idempotency_conflict");
    expect(err.body.staff_detail).toContain("existing op=");
  });
});

describe("idempotency · TTL expiry (24h sweep)", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("sweeps entries older than 24h on next checkIdempotency call", async () => {
    vi.setSystemTime(new Date("2026-05-19T00:00:00Z"));
    const k = uniqueKey("ttl-sweep");
    await storeIdempotency(k, { v: 1 }, 200);

    const before = await checkIdempotency(k);
    expect(before.hit).toBe("replay");

    // Advance just past 24h boundary
    vi.setSystemTime(new Date("2026-05-20T00:00:01Z"));
    const after = await checkIdempotency(k);
    expect(after.hit).toBe("miss");
  });

  it("does NOT sweep entries within 24h window", async () => {
    vi.setSystemTime(new Date("2026-05-19T00:00:00Z"));
    const k = uniqueKey("ttl-within");
    await storeIdempotency(k, { v: 1 }, 200);

    // Advance 23h (within TTL)
    vi.setSystemTime(new Date("2026-05-19T23:00:00Z"));
    const result = await checkIdempotency(k);
    expect(result.hit).toBe("replay");
  });

  it("sweep operates per-entry (mixed-age store: keeps fresh, removes stale)", async () => {
    vi.setSystemTime(new Date("2026-05-19T00:00:00Z"));
    const stale = uniqueKey("ttl-stale");
    await storeIdempotency(stale, { age: "stale" }, 200);

    // 12h later, store a fresh entry
    vi.setSystemTime(new Date("2026-05-19T12:00:00Z"));
    const fresh = uniqueKey("ttl-fresh");
    await storeIdempotency(fresh, { age: "fresh" }, 200);

    // 25h after t0: stale is past TTL, fresh is still within
    vi.setSystemTime(new Date("2026-05-20T01:00:00Z"));
    const staleResult = await checkIdempotency(stale);
    const freshResult = await checkIdempotency(fresh);
    expect(staleResult.hit).toBe("miss");
    expect(freshResult.hit).toBe("replay");
  });
});
