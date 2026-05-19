/*
 * audit/queries.ts — F5 audit_log read helpers for Captain console.
 *
 * Companion to audit/f5.ts (writer). Provides the read-side for
 * incident pane + admin queries on the racingpoint_v2.audit_log table.
 *
 * Schema reference (per migrations/0001_v2_skeleton.sql):
 *   audit_log (id BIGSERIAL, ts TIMESTAMPTZ, actor_id TEXT, actor_tier TEXT,
 *              action TEXT, target_type TEXT, target_id TEXT,
 *              outcome TEXT, request_id TEXT, operation_id TEXT,
 *              body_hash TEXT, detail JSONB)
 *
 * Dev stub (this implementation):
 *   - Returns deterministic mock incident rows for UI development
 *   - Throws on production via NODE_ENV check at module load (mirrors f5.ts + requireF6.ts)
 *   - Mock data shape exactly matches audit_log table for clean swap to real pg queries in B-3
 *
 * Production wire-in (B-3 per ARCH-FOLLOWUP-1 scope-map):
 *   - Real pg Pool connection (pg driver to install)
 *   - SELECT against audit_log with parameterized WHERE clauses
 *   - Index usage: audit_log_ts_idx (ts DESC), audit_log_target_idx (target_type, target_id)
 *
 * Privacy posture (per §S-404 + §B-16):
 *   - actor_id is household_id UUID for customers, staff_id for staff (never phone)
 *   - body_hash hides raw customer payloads; detail JSONB is staff-payload only
 *   - PII never returned from this layer
 *
 * Trust model (per §S-404 Axis .3):
 *   - Bono edge READ; James canonical writes
 *   - This module ONLY reads; no writeAudit/INSERT semantics
 */

if (process.env.NODE_ENV === "production") {
  throw new Error(
    "audit/queries dev-stub reached in production — wire pg Pool against racingpoint_v2.audit_log before deploy"
  );
}

export type AuditOutcome = "success" | "failure" | "throttled" | "conflict";
export type AuditTier = "customer" | "staff" | "shift_lead" | "captain" | "system";

export interface AuditLogRow {
  id: string;
  ts: string;
  actor_id: string;
  actor_tier: AuditTier;
  action: string;
  target_type: string;
  target_id: string;
  outcome: AuditOutcome;
  request_id?: string | null;
  operation_id?: string | null;
  body_hash?: string | null;
  detail?: Record<string, unknown> | null;
}

export interface ListIncidentsParams {
  status?: "active" | "resolved" | "all";
  since?: string;
  classPattern?: string;
  limit?: number;
}

export interface ListIncidentsResponse {
  incidents: AuditLogRow[];
  active_count: number;
  resolved_24h: number;
}

const MOCK_INCIDENTS: AuditLogRow[] = [
  {
    id: "1001",
    ts: "2026-05-19T15:00:00.000Z",
    actor_id: "system",
    actor_tier: "system",
    action: "incident_bono_egress_down",
    target_type: "venue",
    target_id: "venue-1",
    outcome: "failure",
    request_id: "req-dev-001",
    operation_id: "incident_open",
    body_hash: null,
    detail: {
      reason: "whatsapp-bot pm2 status stopped",
      since_ts: "2026-05-19T14:46:00Z",
      affected_households: 0,
    },
  },
  {
    id: "1002",
    ts: "2026-05-19T15:13:00.000Z",
    actor_id: "system",
    actor_tier: "system",
    action: "incident_wallet_write_throttle",
    target_type: "household",
    target_id: "hh-dev-abc",
    outcome: "throttled",
    request_id: "req-dev-002",
    operation_id: "incident_open",
    body_hash: null,
    detail: {
      reason: "rate-limit exceeded on wallet_topup",
      retry_after_s: 30,
    },
  },
  {
    id: "1003",
    ts: "2026-05-19T14:30:00.000Z",
    actor_id: "system",
    actor_tier: "system",
    action: "incident_pricing_cache_stale",
    target_type: "venue",
    target_id: "venue-1",
    outcome: "success",
    request_id: "req-dev-003",
    operation_id: "incident_resolve",
    body_hash: null,
    detail: {
      reason: "cache refreshed; stale_seconds=180",
      resolved_at: "2026-05-19T14:30:00Z",
    },
  },
];

function isIncidentAction(action: string): boolean {
  return action.startsWith("incident_");
}

function isResolved(row: AuditLogRow): boolean {
  return row.outcome === "success" && row.operation_id === "incident_resolve";
}

export async function listIncidents(
  params: ListIncidentsParams = {},
): Promise<ListIncidentsResponse> {
  const status = params.status ?? "active";
  const limit = Math.min(params.limit ?? 100, 1000);
  const since = params.since ? new Date(params.since) : null;

  const all = MOCK_INCIDENTS.filter((r) => isIncidentAction(r.action));

  let filtered = all;
  if (since) {
    filtered = filtered.filter((r) => new Date(r.ts) >= since);
  }
  if (params.classPattern) {
    const pat = params.classPattern;
    filtered = filtered.filter((r) => r.action.includes(pat));
  }
  if (status === "active") {
    filtered = filtered.filter((r) => !isResolved(r));
  } else if (status === "resolved") {
    filtered = filtered.filter(isResolved);
  }

  const sorted = filtered.sort((a, b) => (b.ts > a.ts ? 1 : -1));
  const sliced = sorted.slice(0, limit);

  const active_count = all.filter((r) => !isResolved(r)).length;
  const twentyFourHoursAgo = new Date(Date.now() - 24 * 60 * 60 * 1000);
  const resolved_24h = all.filter(
    (r) => isResolved(r) && new Date(r.ts) >= twentyFourHoursAgo,
  ).length;

  return { incidents: sliced, active_count, resolved_24h };
}

export async function getIncidentById(id: string): Promise<AuditLogRow | null> {
  return MOCK_INCIDENTS.find((r) => r.id === id) ?? null;
}
