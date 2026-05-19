import { HandlerError } from "./errors";

/*
 * BONO-AI proactive seed per R-41 §"The four SEAMs Bono-AI must wire" #3:
 *   "Idempotency store → Redis or Postgres `idempotency_keys` table (24h TTL)"
 *
 * Contract (R-41 invariant #3):
 *   "Idempotency check BEFORE mutation — replay returns 200+cached, conflict returns 409"
 *
 * Key triple (R-42 SCHEMA-WALLET.md §4):
 *   (operation_id, household_id|staff_id, request_id)
 *   - operation_id: ULID minted client-side per logical operation
 *   - actor scope (household for customer mutations, staff for staff-tier)
 *   - request_id: per-attempt id for retry observability
 *
 * Replay semantics:
 *   - Same (operation_id, actor_scope, request_id) within 24h → return cached response (200)
 *   - Same operation_id+actor_scope but different request_id → conflict (409 — concurrent retry)
 *   - Same operation_id but different actor_scope → 409 (cross-actor reuse — sentinel for IDOR)
 *
 * Dev stub: in-memory Map with manual TTL sweep on hit. Throws on production.
 */

if (process.env.NODE_ENV === "production") {
  throw new Error(
    "idempotency dev-stub reached in production — wire Postgres `idempotency_keys` table with unique index before deploy"
  );
}

export interface IdempotencyKey {
  operation_id: string;
  actor_scope: string;
  request_id: string;
}

export interface IdempotencyEntry {
  key: IdempotencyKey;
  response_body: unknown;
  response_status: number;
  created_at: number;
}

const TTL_MS = 24 * 60 * 60 * 1000;
const store = new Map<string, IdempotencyEntry>();

function compositeKey(k: IdempotencyKey): string {
  return `${k.operation_id}::${k.actor_scope}::${k.request_id}`;
}

function sweepExpired(): void {
  const now = Date.now();
  for (const [k, v] of store) {
    if (now - v.created_at > TTL_MS) store.delete(k);
  }
}

export async function checkIdempotency(
  k: IdempotencyKey
): Promise<{ hit: "replay"; entry: IdempotencyEntry } | { hit: "conflict"; existing: IdempotencyEntry } | { hit: "miss" }> {
  sweepExpired();

  const exact = store.get(compositeKey(k));
  if (exact) return { hit: "replay", entry: exact };

  for (const entry of store.values()) {
    if (
      entry.key.operation_id === k.operation_id &&
      entry.key.actor_scope === k.actor_scope &&
      entry.key.request_id !== k.request_id
    ) {
      return { hit: "conflict", existing: entry };
    }
    if (entry.key.operation_id === k.operation_id && entry.key.actor_scope !== k.actor_scope) {
      return { hit: "conflict", existing: entry };
    }
  }
  return { hit: "miss" };
}

export async function storeIdempotency(
  k: IdempotencyKey,
  response_body: unknown,
  response_status: number
): Promise<void> {
  store.set(compositeKey(k), {
    key: k,
    response_body,
    response_status,
    created_at: Date.now(),
  });
}

export function idempotencyConflictError(existing: IdempotencyEntry): HandlerError {
  return new HandlerError(409, {
    error: "idempotency_conflict",
    message: "This action is already in progress or has changed. Please refresh and try again.",
    staff_detail: `existing op=${existing.key.operation_id} actor=${existing.key.actor_scope} at ${new Date(existing.created_at).toISOString()}`,
  });
}
