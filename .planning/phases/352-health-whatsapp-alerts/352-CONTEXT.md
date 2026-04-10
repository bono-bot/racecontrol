---
phase: 352
name: Health + WhatsApp Alerts
slug: health-whatsapp-alerts
type: context
created: "2026-04-10T12:35:00+05:30"
mode: auto
---

<objective>
`/api/health` reports true ground truth per subsystem. Degradation triggers WhatsApp alert via comms-link relay within 30 seconds. Structured JSON logs rotate daily and rsync to Bono VPS.
</objective>

<prior_decisions>
## From Phase 343 (Staff PIN Hardening)
- Plan 02 added `post_write_verify_staff_pin()` and `spawn_delayed_sync_verify()` with `alert_incidents` table TODO — Phase 352 must wire this up
- `whatsapp_alerter.rs` already dispatches via Evolution API directly (no relay)

## From Phase 345 (Backend Resilience)
- admin.db lazy-load with `AdminDbError` — health probe must detect admin.db accessibility failures
- rc proxy env-in-handler — `/api/health` must verify RC_URL env is set (cloud mode)

## From Phase 344 (Unbreakable Deploys)
- `admin-deploy.sh` + `verify-deploy.js` — deploy verification exists but no runtime health integration

## From Standing Rules
- "Probes that lie" — per CLAUDE.md, health probes must not falsely report "ok" when subsystem is broken
- "Audit the MONITOR, not just the MONITORED" — health system itself needs monitoring
- "Fix all systems" — any alert infrastructure must work on both venue and cloud
</prior_decisions>

<codebase_context>
## Existing Alerting Infrastructure (MASSIVE — 10 modules, 4,470 lines)

| Module | File | Lines | Purpose | Dedup |
|--------|------|-------|---------|-------|
| P0 Alerter | whatsapp_alerter.rs | 375 | Core WhatsApp dispatch via Evolution API | Per-alert + 5m security |
| Business KPIs | alert_engine.rs | 134 | Revenue/occupancy/maintenance thresholds | 30-min interval |
| App Health | app_health_monitor.rs | 887 | Next.js probe (6 apps, 30s interval) | Per-app 5m + transition |
| Fleet Health | fleet_health.rs | 1,652 | Pod status aggregator + crash loop detection | N/A |
| Metric Alerts | metric_alerts.rs | 269 | Configurable metric threshold rules | Per-rule 30m |
| Fleet Escalation | fleet_alert.rs | 114 | POST /fleet/alert from rc-sentry | Global 60s |
| Cafe Stock | cafe_alerts.rs | 522 | Inventory low-stock alerts | Per-item 4h |
| Notification Queue | notification_outbox.rs | 276 | Durable queue with retry + fallback | Exponential backoff |
| Synthetic Probes | synthetic_monitor.rs | 241 | Golden-path API self-checks | Per-probe consecutive 2 |
| Comms-Link Alert | alert-manager.js | 225 | James heartbeat down/recovery only | 5m cooldown |

## Current `/api/v1/health` Response (routes.rs:744-756)
Returns: `{ status, service, version, build_id, whatsapp, deploy_context }` — **flat, no per-subsystem probes**.

## Existing Health Endpoints
- `GET /api/v1/health` — basic service health (public)
- `GET /api/v1/fleet/health` — per-pod status array (public)
- `GET /api/v1/app-health` — Next.js app probe results (from app_health_monitor)
- `GET /api/v1/sync/health` — cloud sync health

## WhatsApp Dispatch Path
Racecontrol calls Evolution API DIRECTLY via `whatsapp_alerter::send_whatsapp()`. No comms-link relay intermediary.

## Comms-Link Relay
NO `/relay/alert` endpoint exists. AlertManager in comms-link only handles James heartbeat events.

## Logging
Racecontrol uses `tracing` with JSONL rolling appender (`racecontrol-*.jsonl`). No structured admin API request logging. No rsync to Bono VPS.
</codebase_context>

<decisions>

## D1: Per-Subsystem Health Probe Architecture
[auto] Selected: **Extend existing `/api/v1/health` endpoint** with a `subsystems` object alongside the current flat fields (backward-compatible). Each subsystem returns `{ ok: bool, latency_ms: u64, error_code: Option<String>, detail: Option<String> }`.

**Subsystems to probe (per OPS-01):**
- `admin_db` — SQLite file readable + writable (INSERT/DELETE test row)
- `rc_backend` — self-check (always ok if responding)
- `db_writable` — racecontrol.db WAL mode writable (INSERT/DELETE)
- `disk_free` — `statvfs` on data directory, warn if <1GB
- `cloud_sync` — last successful sync timestamp < 2 * sync_interval
- `whatsapp` — Evolution API reachable (existing `check_evolution_health`)
- `fleet_connectivity` — count of ws_connected pods vs expected

**Why:** The existing health endpoint is a lie detector (returns "ok" always). Per-subsystem probes give the admin dashboard (`/settings/health` page per OPS-02/UI-05) real data.

## D2: Alert Dispatch Path — Direct vs Relay
[auto] Selected: **Keep direct Evolution API dispatch from racecontrol** (existing pattern). Add comms-link `/relay/alert` as a FALLBACK path only (when Evolution API is unreachable from venue).

**Why:** 
- Direct dispatch = lower latency (~200ms vs ~500ms through relay)
- 10 modules already use `send_whatsapp()` directly — refactoring all to relay is scope creep
- Relay as fallback adds resilience without breaking existing patterns
- OPS-03 says "via POST to comms-link relay" — satisfy by adding the relay path, but primary remains direct

**Fallback chain:** racecontrol tries Evolution API direct → if fails, POST to `http://192.168.31.27:8766/relay/alert` → comms-link forwards to Evolution API from Bono VPS.

## D3: Alert Dedup Strategy
[auto] Selected: **Leverage existing per-module dedup** (already implemented in 8 modules). Add a CENTRAL dedup layer in the new `subsystem_health` module: same `(subsystem, error_code)` pair within 10 minutes = suppress. This satisfies OPS-04 without touching existing alert modules.

**Implementation:** `HashMap<(String, String), Instant>` in the health monitor task. On transition to degraded: check dedup map → if not suppressed, fire alert → update map.

## D4: Phase 343-02 alert_incidents Wiring (OPS-05)
[auto] Selected: **Wire `alert_incidents` table** that Phase 343-02 referenced. The table tracks: `id, alert_type, subsystem, severity, message, fired_at, resolved_at, correlation_id`. `spawn_delayed_sync_verify()` already fires — add INSERT to `alert_incidents` on every WhatsApp alert dispatch.

**Why:** This closes the 343-02 TODO and provides alert history for the admin dashboard.

## D5: Structured JSON Logging (OPS-06)
[auto] Selected: **Add `tracing` middleware for admin API requests** using tower-http's `TraceLayer`. Each request logs: `{ ts, level, method, route, status, latency_ms, staff_id, err, correlation_id }`. Uses the existing JSONL rolling appender — no new log infrastructure.

**Why:** Admin API requests currently have no structured logging. This gives audit trail for admin actions (PIN changes, staff CRUD, config changes).

## D6: Log Rotation + Rsync (OPS-07)
[auto] Selected: **Daily rotation via existing tracing rolling appender** (already rotates by date). Add a `cron` or background task that rsyncs `data/logs/*.jsonl` to Bono VPS `/root/backups/venue-logs/` via SSH. Use comms-link relay `exec` command to trigger `rsync` from James .27.

**Why:** Logs are already rotated. The gap is getting them off-venue for disaster recovery. rsync via relay is the existing pattern for cross-machine ops.

## D7: Admin Dashboard Health Page (OPS-02 / UI-05)
[auto] Selected: **DEFERRED to Phase 354** (UI Hardening) which already has `/settings/health` in scope. Phase 352 delivers the backend API; Phase 354 renders the tiles.

**Note:** Phase 354 is already shipped per STATE.md (354-01 + 354-02). If `/settings/health` page already exists, this is just wiring it to the new subsystem health API.

</decisions>

<specifics>

## Alert Message Format
Use existing `[RP ALERT]` prefix pattern from `whatsapp_alerter.rs`. Subsystem alerts:
```
[RP ALERT] Subsystem Degraded: admin_db
Error: SQLITE_READONLY — database is locked
Server: venue (.23)
```

Recovery:
```
[RP RESOLVED] admin_db recovered (was down 3m 12s)
```

## Subsystem Health Response Shape
```json
{
  "status": "ok",
  "service": "racecontrol",
  "version": "0.1.0",
  "build_id": "4074bb0d",
  "whatsapp": "ok",
  "subsystems": {
    "admin_db": { "ok": true, "latency_ms": 2, "error_code": null, "detail": null },
    "db_writable": { "ok": true, "latency_ms": 1, "error_code": null, "detail": null },
    "disk_free": { "ok": true, "latency_ms": 0, "error_code": null, "detail": "42.1 GB free" },
    "cloud_sync": { "ok": false, "latency_ms": 0, "error_code": "SYNC_STALE", "detail": "Last sync 185s ago (threshold: 60s)" },
    "fleet_connectivity": { "ok": true, "latency_ms": 0, "error_code": null, "detail": "8/8 pods connected" },
    "whatsapp_api": { "ok": true, "latency_ms": 312, "error_code": null, "detail": null }
  }
}
```

## Comms-Link `/relay/alert` Endpoint Shape
```json
POST /relay/alert
{
  "source": "venue-racecontrol",
  "subsystem": "admin_db",
  "severity": "critical",
  "message": "[RP ALERT] Subsystem Degraded: admin_db\nError: SQLITE_READONLY",
  "timestamp": "2026-04-10T12:00:00+05:30"
}
```
Response: `{ "ok": true, "dispatched": true }`

</specifics>

<deferred>
- **Alert analytics dashboard** — trending, SLA tracking, MTTR per subsystem → separate phase
- **Centralized dedup across racecontrol + comms-link** — current architecture has independent dedup; centralizing requires rearchitecting 10 modules → not worth the risk
- **SMS fallback for WhatsApp alerts** — notification_outbox.rs has SMS stub, but SMS provider not configured
- **Per-pod alert routing** — currently all alerts go to Uday; per-staff routing is a separate feature
- **Alert severity semantics standardization** — currently "critical"/"warning"/"info" used inconsistently across modules
</deferred>

<canonical_refs>
- `crates/racecontrol/src/whatsapp_alerter.rs` — Core WhatsApp dispatch (Evolution API)
- `crates/racecontrol/src/alert_engine.rs` — Business KPI alert logic
- `crates/racecontrol/src/app_health_monitor.rs` — Next.js app probing (6 apps)
- `crates/racecontrol/src/fleet_health.rs` — Pod status aggregator
- `crates/racecontrol/src/api/routes.rs:744-816` — Current `/health` endpoint
- `crates/racecontrol/src/metric_alerts.rs` — Threshold-based metric alerts
- `crates/racecontrol/src/fleet_alert.rs` — rc-sentry escalation endpoint
- `crates/racecontrol/src/synthetic_monitor.rs` — Golden-path self-checks
- `crates/racecontrol/src/notification_outbox.rs` — Durable notification queue
- `comms-link/bono/alert-manager.js` — Bono-side AlertManager
- `.planning/REQUIREMENTS.md:52-58` — OPS-01..07 requirements
</canonical_refs>
