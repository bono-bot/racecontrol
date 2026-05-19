/**
 * idempotency.ts — hit/miss/conflict/replay state-machine tests.
 *
 * R-41 invariant #3 + R-42 §4 SCHEMA-WALLET.md idempotency contract:
 *   - exact match → replay (200 + cached response)
 *   - same (op, actor) different request_id → conflict (409 concurrent retry)
 *   - same op different actor_scope → conflict (409 cross-actor IDOR sentinel)
 *   - 24h TTL
 */

import { describe, it, expect } from "vitest";
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
