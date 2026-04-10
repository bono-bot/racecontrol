---
phase: 349
name: DB Sync via Google Drive
slug: db-sync-google-drive
type: context
created: "2026-04-11T10:00:00+05:30"
mode: auto
---

<objective>
Venue `racecontrol.db` syncs to cloud via shared Google Drive folder. Plans 01+02 (upload/download scripts) already shipped (`b8c7726b`). Plan 03 remains: cloud read-replica guard + sync lag health probe.
</objective>

<prior_decisions>
## From Phase 343 (Staff PIN Hardening)
- `cloud_authority_guard()` in routes.rs:13014-13024 — blocks VENUE writes to cloud-authoritative tables (currently only `staff_members`)
- `this_instance_is_cloud()` in config.rs:303-315 — detects cloud instance via `RC_IS_CLOUD=1` env or loopback heuristic
- `CloudConfig.authoritative_tables` — configurable list of cloud-authoritative tables (default: `["staff_members"]`)

## From Phase 352 (Health + WhatsApp Alerts)
- `subsystem_health.rs` with 7 probes — existing probe infrastructure for adding `db_sync` probe
- D7: Admin dashboard health page deferred to Phase 354

## From Phase 345 (Backend Resilience)
- admin.db lazy-load with `AdminDbError` — pattern for graceful degradation on DB access issues
</prior_decisions>

<codebase_context>
## Existing Sync Infrastructure

| Component | File | Status |
|-----------|------|--------|
| Upload script | scripts/db-sync/upload-db.ps1 | SHIPPED (349-01) — runs on James .27 every 5 min |
| Download script | scripts/db-sync/download-db.sh | SHIPPED (349-02) — runs on Bono VPS every 5 min |
| Env credentials | scripts/db-sync/db-sync.env | SHIPPED — Google OAuth client_id, client_secret, refresh_token, folder_id |
| Cloud authority guard | crates/racecontrol/src/api/routes.rs:13014 | SHIPPED (343) — `cloud_authority_guard()` blocks venue writes to cloud-authoritative tables |
| Instance detection | crates/racecontrol/src/config.rs:303 | SHIPPED (343) — `this_instance_is_cloud()` |
| Sync health endpoint | crates/racecontrol/src/api/routes.rs:12338 | EXISTING — `sync_health()` shows per-table sync state |
| Subsystem health | crates/racecontrol/src/subsystem_health.rs | SHIPPED (352) — 7 probes, extensible with new probe functions |

## Key Patterns
- `cloud_authority_guard(state, table)` returns `Option<(StatusCode, Json<Value>)>` — Some(409, ...) to reject, None to allow
- All venue-authoritative endpoints are POST/PUT/DELETE in routes.rs — need to identify which to guard
- `sync-status.json` written by both upload (James) and download (Bono VPS) scripts with timestamps
</codebase_context>

<decisions>

## D1: Venue-Authoritative Table List
[auto] Selected: **Inverse of cloud-authoritative** — ALL tables are venue-authoritative EXCEPT those in `cloud.authoritative_tables` (currently only `staff_members`). This means the cloud instance should reject writes to billing, sessions, drivers, laps, game state, etc.

**Why:** The venue racecontrol is the source of truth for operational data. Cloud is a read replica for monitoring/admin. The existing `authoritative_tables` config already defines the boundary — 349-03 just needs to enforce the inverse on the cloud side.

## D2: Read-Replica Guard Pattern
[auto] Selected: **Symmetric `venue_authority_guard()` function** — same pattern as `cloud_authority_guard()` but inverted logic:
- Runs on CLOUD instance (checks `this_instance_is_cloud()`)
- Rejects writes to tables NOT in `authoritative_tables` (venue-owned data)
- Returns 409 with `{"error": "venue_authoritative", "table": "...", "hint": "This table is managed by the venue instance"}`

**Implementation:** Add `venue_authority_guard(state, table)` next to `cloud_authority_guard()`. Apply to write endpoints for: billing_sessions, drivers, laps, game_launches, presets, pricing, etc. Skip read endpoints (GET) — cloud must be able to read everything.

**Why:** Mirrors the existing pattern exactly. No new abstractions needed. Same error format for consistency.

## D3: Sync Lag Health Probe
[auto] Selected: **File-mtime based probe** — check the age of `racecontrol.db` on the cloud instance (last modified by download-db.sh). Add `db_sync` probe to subsystem_health.rs:
- `ok: true` if DB mtime < 600s (10 min, allows 2 missed 5-min cycles)
- WARN threshold: 300s (5 min — 1 missed cycle)
- CRITICAL threshold: 900s (15 min — 3 missed cycles)
- `detail` field shows: `"Last sync 4m 32s ago"`

**Why:** The download script already updates the file. Checking mtime is simpler and more reliable than parsing sync-status.json (which could be stale if the script crashes mid-write). SYNC-06 requires this probe.

## D4: Admin UI Badge (SYNC-04)
[auto] Selected: **DEFERRED to Phase 354** (UI Hardening). Backend delivers sync status via existing `/sync/health` endpoint. Cloud admin header badge belongs in the frontend phase — consistent with Phase 352 D7 pattern.

**Why:** No frontend changes in Phase 349. The existing sync_health endpoint already returns per-table sync state — the frontend just needs to consume it.

## D5: Monthly Restore Drill (SYNC-07)
[auto] Selected: **Document the procedure** in Phase 349-03 as a runbook section. The drill itself is operational — not code. Create `scripts/db-sync/RESTORE-DRILL.md` with step-by-step instructions for restoring from Google Drive to a scratch path.

**Why:** SYNC-07 requires documentation and execution, not code. The download script already handles the download — the drill just needs a fresh-path variant.

## D6: Pause Replication Command (SYNC-08)
[auto] Selected: **Claude's discretion** — simple approach: `touch /tmp/DB_SYNC_PAUSED` sentinel file checked by download-db.sh before downloading. Upload continues (data doesn't get lost). Document in RESTORE-DRILL.md.

**Why:** Minimal code change. Upload continues to preserve data. Only download pauses.

### Claude's Discretion
- Guard placement: which specific write endpoints to guard (researcher should enumerate from routes.rs)
- Sync probe interval alignment with existing health probe cycle
- sync-status.json format (already defined by shipped scripts — just consume it)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Sync Scripts (shipped)
- `scripts/db-sync/upload-db.ps1` — Upload script (James .27, every 5 min)
- `scripts/db-sync/download-db.sh` — Download script (Bono VPS, every 5 min)
- `scripts/db-sync/db-sync.env` — Google OAuth credentials (template, not committed with secrets)

### Authority Guard Pattern (Phase 343)
- `crates/racecontrol/src/api/routes.rs:13014-13024` — `cloud_authority_guard()` — template for venue_authority_guard
- `crates/racecontrol/src/config.rs:293-315` — `CloudConfig`, `is_cloud_authoritative_for()`, `this_instance_is_cloud()`

### Health Probes (Phase 352)
- `crates/racecontrol/src/subsystem_health.rs` — Probe infrastructure for adding db_sync probe

### Existing Sync Health
- `crates/racecontrol/src/api/routes.rs:12338` — `sync_health()` endpoint

### Requirements
- `.planning/REQUIREMENTS.md` — SYNC-01..08

</canonical_refs>

<specifics>
## Specific Ideas

- The 409 response from `venue_authority_guard` should match the format from `cloud_authority_guard` for consistency
- Sync lag probe should be CLOUD-ONLY — venue instance doesn't download from Drive, so mtime check is irrelevant there
- Break-glass override: `RC_ALLOW_CLOUD_VENUE_WRITE=1` env var for emergency scenarios (mirrors `RC_ALLOW_VENUE_STAFF_WRITE` from Phase 343)

</specifics>

<deferred>
## Deferred Ideas

- **Admin dashboard sync status badge** — SYNC-04 deferred to Phase 354 (UI Hardening)
- **Bi-directional conflict resolution for venue-authoritative tables** — current design is one-way (venue→cloud). If cloud ever needs to write venue data, that's a separate architecture decision
- **Automated restore drill** — SYNC-07 currently manual. Could be automated with a schtask but adds complexity for minimal value

</deferred>

---

*Phase: 349-db-sync-google-drive*
*Context gathered: 2026-04-11*
