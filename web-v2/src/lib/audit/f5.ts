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

if (process.env.NODE_ENV === "production") {
  throw new Error(
    "audit/f5 dev-stub reached in production — wire Postgres `audit_log` append-only table before deploy"
  );
}

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
  console.log("[f5 audit]", JSON.stringify(row));
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
