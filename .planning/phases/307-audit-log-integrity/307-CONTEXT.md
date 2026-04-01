# Phase 307: Audit Log Integrity - Context

**Gathered:** 2026-04-01
**Status:** Ready for planning
**Mode:** Security phase — discuss skipped

<domain>
## Phase Boundary

Add SHA-256 hash chain to activity_log entries (append-only integrity). Expand logged actions to cover config changes, deploys, billing, and admin CRUD. Add tamper detection via `GET /api/v1/audit/verify` endpoint. Does NOT retroactively chain existing entries.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices at Claude's discretion. Key constraints: existing `pod_activity_log` table in SQLite needs ALTER for hash columns, fire-and-forget async write pattern should be preserved for performance but hash computation must be sequential (chain requires previous hash).

</decisions>

<code_context>
## Existing Code Insights

### Current Activity Log
- `crates/racecontrol/src/activity_log.rs` (72 lines) — `log_pod_activity()` function
- Fire-and-forget async pattern: tokio::spawn with sqlx INSERT, errors silently ignored
- Broadcasts `DashboardEvent::PodActivity` to WS clients immediately
- UUID v4 for each entry, chrono UTC timestamps

### Current Schema (pod_activity_log)
```sql
CREATE TABLE IF NOT EXISTS pod_activity_log (
    id TEXT PRIMARY KEY,           -- UUID
    pod_id TEXT NOT NULL,
    pod_number INTEGER DEFAULT 0,
    timestamp TEXT DEFAULT (datetime('now')),
    category TEXT NOT NULL,        -- "system", "content", "race_engineer"
    action TEXT NOT NULL,
    details TEXT DEFAULT '',
    source TEXT NOT NULL           -- "staff", "core", "race_engineer"
)
```
- Indexes: idx_activity_pod, idx_activity_ts, idx_activity_cat
- **NO hash/chain columns** — Phase 307 adds these

### Currently Logged Actions (LIMITED)
- system: Maintenance Cleared
- content: Launch Rejected
- race_engineer: Quick Fix Applied, AI Diagnosis, AI Diagnosis Failed
- **Missing**: config changes, deploys, billing events, admin CRUD, auth events

### Existing Endpoints
- `GET /activity` — global activity log (last N entries, default 100)
- `GET /pods/{pod_id}/activity` — per-pod history
- `GET /debug/activity` — debug dashboard with contention detection
- **NO integrity verification endpoint** — Phase 307 adds this

### Security Framework
- `comms-link/test/security-check.js` — 31 static assertions (SEC-GATE-01)
- `test/gate-check.sh` — 6+ suites deploy gate (988 lines)
- Phase 307 should add a SEC assertion for hash chain integrity

### Integration Points
- `activity_log.rs` — needs hash computation + previous_hash column
- `db/mod.rs` (line ~2009) — needs ALTER TABLE migration for hash columns
- `routes.rs` — needs new `GET /api/v1/audit/verify` endpoint
- Various routes.rs call sites — need additional logging for config/billing/deploy actions
- Must handle race conditions: sequential hash chain vs concurrent async writes

</code_context>

<specifics>
## Requirements
- AUDIT-01: Every activity_log entry includes SHA-256 hash linking to previous entry (append-only chain)
- AUDIT-02: Tamper detection — mismatched previous_hash triggers alert
- AUDIT-03: Hash chain covers config changes, deploys, billing events, admin actions
- AUDIT-04: `GET /api/v1/audit/verify` returns chain integrity status
</specifics>

<deferred>
None.
</deferred>
