---
phase: 70-failback-data-reconciliation
plan: 01
subsystem: api
tags: [rust, axum, sqlite, sqlx, billing, failback, sync]

# Dependency graph
requires:
  - phase: 69-failover-orchestration
    provides: failover_broadcast endpoint + terminal_secret auth pattern in service_routes()
provides:
  - POST /api/v1/sync/import-sessions endpoint with INSERT OR IGNORE semantics
  - Failback billing data reconciliation for failover window sessions
affects: [70-failback-data-reconciliation, cloud-sync, billing]

# Tech tracking
tech-stack:
  added: []
  patterns: [INSERT OR IGNORE for idempotent session import, terminal_secret auth gate reused from sync_push]

key-files:
  created: []
  modified:
    - crates/racecontrol/src/api/routes.rs

key-decisions:
  - "INSERT OR IGNORE (not ON CONFLICT DO UPDATE) — failback import must not overwrite locally-confirmed billing data"
  - "Duplicate UUIDs silently skipped (skipped += 1) — idempotent, safe to re-POST"
  - "Omit end_reason column — follows sync_push precedent at line 7281 to avoid schema drift"
  - "terminal_secret auth uses != comparison (not subtle crate) — consistent with all other service routes"
  - "tracing::warn! on DB error + skipped += 1 — log failures without aborting the batch"

patterns-established:
  - "Failback import pattern: INSERT OR IGNORE + imported/skipped/synced_at response"

requirements-completed: [BACK-02]

# Metrics
duration: 15min
completed: 2026-03-21
---

# Phase 70 Plan 01: Failback Data Reconciliation Summary

**POST /api/v1/sync/import-sessions endpoint using INSERT OR IGNORE for lossless billing session failback after cloud-failover window**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-21T07:45:00Z (IST 13:15)
- **Completed:** 2026-03-21T08:00:00Z (IST 13:30)
- **Tasks:** 1/1
- **Files modified:** 1

## Accomplishments
- Added `import_sessions` async handler to `crates/racecontrol/src/api/routes.rs`
- Route registered in `service_routes()` at `/sync/import-sessions`
- 26-column INSERT OR IGNORE matches billing_sessions schema (omitting end_reason per sync_push precedent)
- terminal_secret auth gate is an exact copy of the sync_push pattern
- Response includes `imported`, `skipped`, `synced_at` for caller visibility
- `cargo build --bin racecontrol` compiles cleanly (exit 0, warnings pre-existing only)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add import_sessions endpoint to service_routes** - `c06c6f9` (feat)

**Plan metadata:** (docs commit — see below)

## Files Created/Modified
- `crates/racecontrol/src/api/routes.rs` - Route registration + import_sessions handler (~90 lines added)

## Decisions Made
- INSERT OR IGNORE chosen over ON CONFLICT DO UPDATE: failback import must never silently overwrite a locally-confirmed billing record. If the UUID exists, skip it.
- end_reason omitted: sync_push at line 7281 also omits it; following this precedent prevents schema drift.
- terminal_secret auth uses simple `!=` (not subtle crate): consistent with sync_push, failover_broadcast, and all other service routes.
- tracing::warn! on DB error continues batch rather than aborting: partial import is better than full failure.

## Deviations from Plan

None — plan executed exactly as written.

Note: Edit tool silently failed on Windows paths (returned "updated successfully" but file unchanged). Used Python patch script as Rule 3 (blocking) workaround. No code changes required.

## Issues Encountered
- Edit tool path issue on Windows: reported success but did not write. Resolved by using Python file patching directly — confirmed by `cargo build` success and grep verification.

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness
- `POST /api/v1/sync/import-sessions` is live on next racecontrol deploy
- During failback: James POSTs cloud billing sessions to this endpoint; server imports new UUIDs, skips duplicates
- Bono's cloud API needs to export billing sessions from failover window in the expected `{ "sessions": [...] }` format

---
*Phase: 70-failback-data-reconciliation*
*Completed: 2026-03-21*
