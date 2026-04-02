---
phase: "307"
plan: "01"
subsystem: "audit-log"
tags: [security, integrity, hash-chain, tamper-detection, audit]
dependency_graph:
  requires: []
  provides: [audit-chain-integrity, tamper-detection-endpoint]
  affects: [activity-log, routes, state, db-schema]
tech_stack:
  added: [sha2 (already in workspace)]
  patterns: [mutex-serialized-hash-chain, fire-and-forget-with-hash, idempotent-alter-table]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/activity_log.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/db/mod.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/flags.rs
decisions:
  - "Mutex-serialized hash chain: std::sync::Mutex held only for hash computation (no .await inside), released before async DB insert — preserves fire-and-forget non-blocking caller behavior while serializing chain writes"
  - "compute_activity_hash is pub so audit_verify endpoint can recompute hashes without duplicating the formula"
  - "Pre-migration rows retain NULL entry_hash/previous_hash — chain starts from first post-migration entry using GENESIS as previous_hash"
  - "pod_id = 'server' used for non-pod events (billing, config, deploy) since pod_number=0 is valid"
metrics:
  duration: "~70 minutes"
  completed: "2026-04-01T16:52:00Z"
  tasks_completed: 6
  files_changed: 6
---

# Phase 307 Plan 01: Audit Log Integrity Summary

## One-Liner

SHA-256 append-only hash chain on pod_activity_log with mutex-serialized writes, tamper detection via GET /api/v1/audit/verify, and expanded coverage for billing/admin/config/deploy events.

## What Was Built

### AUDIT-01: Hash Chain on Every Entry

Every new `pod_activity_log` entry carries two new columns:
- `entry_hash`: `SHA-256(id|timestamp|category|action|details|source|previous_hash)` as hex
- `previous_hash`: hash of the preceding entry, or `"GENESIS"` for the first hashed entry

The chain is serialized via `AppState.audit_last_hash: std::sync::Mutex<String>`. The mutex is held only for the hash computation (no `.await` inside) and released before the async DB INSERT. Callers of `log_pod_activity()` remain non-blocking.

On server startup, `main.rs` loads the most recent `entry_hash` from the DB to resume the chain after restart.

Pre-migration rows have `entry_hash = NULL` and `previous_hash = NULL` — they are outside the chain and not included in verification.

### AUDIT-02: Tamper Detection

`GET /api/v1/audit/verify` (staff-only):
- Fetches all hashed entries in chronological order
- Recomputes `SHA-256` for each using stored field values and `previous_hash`
- Compares computed hash to stored `entry_hash`
- Returns `chain_valid`, `total_entries`, `verified_entries`, `tampered_entries`, `tampered_at`
- Logs `WARN` on first detected tamper with entry ID and timestamp

### AUDIT-03: Expanded Coverage

New `log_pod_activity()` call sites (8 total new):

| Category | Action | Handler |
|----------|--------|---------|
| `billing` | Session Started | `start_billing` |
| `billing` | Session Ended | `stop_billing` |
| `admin` | Pod Lockdown / Pod Lockdown Released | `lockdown_pod` |
| `admin` | Pod Enabled | `enable_pod` |
| `admin` | Pod Disabled | `disable_pod` |
| `config` | Pricing Rule Created | `create_pricing_rule` |
| `config` | Feature Flag Updated | `flags::update_flag` |
| `deploy` | OTA Deploy Initiated | `ota_deploy_handler` |

### AUDIT-04: Verify Endpoint

`GET /api/v1/audit/verify` registered in `staff_routes()`. Response example:
```json
{
  "chain_valid": true,
  "total_entries": 142,
  "verified_entries": 142,
  "tampered_entries": 0,
  "first_genesis": "2026-04-01T12:00:00Z",
  "latest_hash": "a3f8b...",
  "tampered_at": null
}
```

### DB Migration

Two idempotent `ALTER TABLE` statements added to `init_db()`:
```sql
ALTER TABLE pod_activity_log ADD COLUMN entry_hash TEXT
ALTER TABLE pod_activity_log ADD COLUMN previous_hash TEXT
```
Both use `let _ = sqlx::query(...)` pattern (errors ignored for idempotency — SQLite returns error if column already exists).

Index added: `idx_activity_hash ON pod_activity_log (entry_hash)`.

### Activity Endpoint Updates

`global_activity` and `pod_activity` handlers updated to 10-tuple SELECT including `entry_hash` and `previous_hash`. Consumers can now verify individual entry hashes client-side.

## Tests

4 new unit tests in `activity_log::tests`:
- `test_hash_is_deterministic` — same inputs always produce same 64-char hex hash
- `test_hash_chain_different_entries` — different entry data produces different hashes
- `test_tamper_detection` — changing any field changes the hash
- `test_genesis_chain_start` — GENESIS produces valid SHA-256 hex

All 4 pass. Total: 710 tests pass (up from 706 baseline). 4 pre-existing failures unrelated to Phase 307.

## Commits

| Hash | Description |
|------|-------------|
| `d5f9b387` | feat(307-01): audit log SHA-256 hash chain + tamper detection |

## Deviations from Plan

None — plan executed exactly as written. All 6 tasks completed in one commit.

Minor note: The `route_registration` was verified as non-duplicate via grep before adding.

## Known Stubs

None. All data flows are wired: hash computed from real field values, stored in DB, and recomputed by verify endpoint.

## Self-Check: PASSED

- `d5f9b387` — commit exists in git log
- `crates/racecontrol/src/activity_log.rs` — contains `compute_activity_hash` and hash chain logic
- `crates/racecontrol/src/state.rs` — contains `audit_last_hash: std::sync::Mutex<String>`
- `crates/racecontrol/src/db/mod.rs` — contains `ALTER TABLE pod_activity_log ADD COLUMN entry_hash`
- `crates/racecontrol/src/api/routes.rs` — contains `audit_verify` handler and `/audit/verify` route
- `cargo check` — 0 errors, 1 pre-existing warning
- `cargo test` — 710 pass, 4 pre-existing failures
- `activity_log::tests::test_hash_is_deterministic` — ok
- `activity_log::tests::test_hash_chain_different_entries` — ok
- `activity_log::tests::test_tamper_detection` — ok
- `activity_log::tests::test_genesis_chain_start` — ok
