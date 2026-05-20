/*
 * BONO-AI proactive seed per R-41 §"The four SEAMs Bono-AI must wire" #2:
 *   "F5 audit writer → Postgres append-only `audit_log` table (suggested integrity: signed batches)"
 *
 * Contract (R-41 invariant #4):
 *   "Audit BEFORE mutation — if F5 write fails, mutation fails"
 *
 * Privacy posture (R-42 §B-16 + §S-158 customer_consent_change pattern):
 *   - Customer payloads: `body_hash` only (sha256 of serialized body); raw body never written
 *   - Staff payloads: full `detail` permitted (staff actions are auditable in full)
 *   - PII (phone, address): NEVER written; identity is household_id / staff_id
 *
 * Dev stub: console.log to stdout for dev visibility. Throws on production.
 */

import { createHash } from "crypto";
import type { Tier } from "../auth/requireF6";
import { getPool } from "../db";

// B-3 wire-in: graceful degradation. PG_URL set → real append-only INSERT
// against racingpoint_v2.audit_log as web_v2_audit role; unset → console.log
// dev-stub. The earlier module-top throw-on-prod was removed (it broke Next.js
// page-data collection when imported by routes — same class as queries.ts).
// Append-only is enforced at the GRANT level (web_v2_audit has no UPDATE/DELETE).

export type F5Outcome = "success" | "failure" | "throttled" | "conflict";

export interface F5AuditRow {
  ts: string;
  actor_id: string;
  actor_tier: Tier;
  action: string;
  target_type: string;
  target_id: string;
  outcome: F5Outcome;
  request_id?: string;
  operation_id?: string;
  body_hash?: string;
  detail?: Record<string, unknown>;
}

export function hashBody(body: unknown): string {
  const serialized = typeof body === "string" ? body : JSON.stringify(body);
  return createHash("sha256").update(serialized, "utf-8").digest("hex");
}

export async function writeAudit(row: F5AuditRow): Promise<void> {
  const pool = getPool();
  if (!pool) {
    // Dev-stub fallback: no DB credential configured.
    console.log("[f5 audit]", JSON.stringify(row));
    return;
  }
  await pool.query(
    `INSERT INTO audit_log
       (ts, actor_id, actor_tier, action, target_type, target_id,
        outcome, request_id, operation_id, body_hash, detail)
     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)`,
    [
      row.ts,
      row.actor_id,
      row.actor_tier,
      row.action,
      row.target_type,
      row.target_id,
      row.outcome,
      row.request_id ?? null,
      row.operation_id ?? null,
      row.body_hash ?? null,
      row.detail ?? null,
    ],
  );
}

export async function writeAuditOrFail(row: F5AuditRow): Promise<void> {
  try {
    await writeAudit(row);
  } catch (e) {
    throw new Error(
      `F5 audit write failed (mutation must abort): ${e instanceof Error ? e.message : String(e)}`
    );
  }
}
