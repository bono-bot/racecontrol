---
phase: 347-admin-staff-management
plan: "01"
subsystem: racecontrol-rust
tags: [staff-management, cloud-sync, auth, backend, api]
dependency_graph:
  requires: [343-staff-pin-hardening]
  provides: [change-staff-pin-safe-endpoint, sync-pull-now-endpoint, pull-tables-now]
  affects: [admin-ui-plan-02, cloud-sync, staff-auth]
tech_stack:
  added: []
  patterns: [cloud-forward-pattern, dual-verify-pattern, filtered-pull]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/cloud_sync.rs
    - crates/racecontrol/src/api/routes.rs
decisions:
  - "Venue instance forwards PIN change to cloud rather than writing locally — keeps cloud authoritative"
  - "pull_tables_now uses since=epoch so it always fetches current state (not incremental delta)"
  - "validate_pin_format extracted to standalone fn for testability"
  - "ALLOWED_SYNC_TABLES const is a hardcoded allowlist (not derived from SYNC_TABLES string) to enable compile-time array contains check"
metrics:
  duration: "~25 minutes"
  completed: "2026-04-10"
  tasks_completed: 2
  files_modified: 2
---

# Phase 347 Plan 01: Staff PIN Safe Change + Sync Pull Now — Summary

Add two new Rust backend endpoints: `change_staff_pin_safe` (orchestrated cloud-write + pull + dual-verify for staff PIN changes) and `sync_pull_now` (on-demand filtered cloud→venue table pull), plus the underlying `pull_tables_now` function in `cloud_sync.rs`.

## What Was Built

### `pull_tables_now` — `cloud_sync.rs`

New `pub(crate) async fn pull_tables_now(state, tables: &[&str])` added after `sync_once_http`. Performs a filtered HTTP GET to `{cloud_url}/sync/changes` with only the requested tables, upserts matching rows into local DB, does NOT update `last_synced` timestamp (out-of-band pull). Supports HMAC signing and terminal secret, follows same auth pattern as `sync_once_http`.

### `change_staff_pin_safe` — `routes.rs`

Orchestrated handler registered at `POST /api/v1/admin/staff/{id}/change-pin` (manager+ only).

Flow:
1. Validate PIN (4+ digits, numeric only) — 400 on failure
2. If venue instance: forward to cloud via HTTP POST with same Bearer token — 502 on cloud failure
3. If cloud instance: `UPDATE staff_members SET pin = ?` directly + `cloud_authority_guard` check
4. Call `pull_tables_now(&state, &["staff_members"])` — log warning on failure, continue
5. Post-write verify venue PIN — `post_write_verify_staff_pin`
6. Spawn delayed sync verify — `spawn_delayed_sync_verify`
7. Return `ChangePinResponse { status, cloud_verified, venue_verified, latency_ms, correlation_id }`

### `sync_pull_now_handler` — `routes.rs`

On-demand handler at `POST /api/v1/admin/sync/pull-now` (manager+ only).

Flow:
1. Validate tables list is non-empty
2. Validate each table name against `ALLOWED_SYNC_TABLES` — 400 on unknown table
3. Call `pull_tables_now` — 502 on failure
4. Return `SyncPullNowResponse { status, tables_synced, latency_ms }`

## Deviations from Plan

None — plan executed exactly as written.

## Unit Tests

6 tests added in `post_write_verify_tests` module:

| Test | Validates |
|------|-----------|
| `change_staff_pin_safe_rejects_short_pin` | PIN len < 4 is rejected |
| `change_staff_pin_safe_rejects_non_numeric` | Non-digit PIN is rejected |
| `change_staff_pin_safe_accepts_valid_pin` | "1234", "99999", "0000" all pass |
| `change_staff_pin_safe_response_shape` | All 5 `ChangePinResponse` fields serialize to JSON |
| `sync_pull_now_rejects_unknown_table` | "users", "secrets" etc. not in ALLOWED_SYNC_TABLES |
| `sync_pull_now_accepts_valid_table` | "staff_members", "drivers", "billing_rates" accepted |

All 6 pass. `cargo check --bin racecontrol` clean (existing warnings only).

## Known Stubs

None. All data paths are wired. The admin UI (Plan 02) will call these endpoints; the endpoints themselves are fully functional.

## Commits

| Commit | Message |
|--------|---------|
| `4ef17f82` | feat(347-01): add change_staff_pin_safe + sync_pull_now handlers + pull_tables_now |

## Self-Check: PASSED

- `async fn change_staff_pin_safe` — 1 match in routes.rs ✅
- `async fn sync_pull_now_handler` — 1 match in routes.rs ✅
- `pub(crate) async fn pull_tables_now` — 1 match in cloud_sync.rs ✅
- `/admin/staff/{id}/change-pin` route — 1 match in manager sub-router ✅
- `/admin/sync/pull-now` route — 1 match in manager sub-router ✅
- `struct ChangePinResponse` with 5 fields — confirmed ✅
- `struct SyncPullNowResponse` with 3 fields — confirmed ✅
- 6 unit tests: 4 change_staff_pin_safe + 2 sync_pull_now — all pass ✅
- `cargo check --bin racecontrol` — clean ✅
- No `.unwrap()` in new handler code — confirmed ✅
- Commit `4ef17f82` exists in git log ✅
