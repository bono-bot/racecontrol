---
phase: 300-sqlite-backup-pipeline
plan: "02"
subsystem: backup
tags: [sqlite, backup, scp, sha256, offsite-transfer, backup-status-api, admin-dashboard]
dependency_graph:
  requires: [300-01]
  provides: [remote_transfer, backup_status_endpoint, backup_dashboard_card]
  affects: [backup_pipeline.rs, api/routes.rs, web/src/lib/api.ts, web/src/app/settings/page.tsx]
tech_stack:
  added: []
  patterns: [tokio::process::Command scp/ssh, sha2::Sha256::digest + sha256sum remote verification, NaiveDate transfer tracking, tokio::time::timeout 120s, staff-JWT-gated REST endpoint, Next.js useState+useEffect data card]
key_files:
  modified:
    - crates/racecontrol/src/backup_pipeline.rs
    - crates/racecontrol/src/api/routes.rs
    - web/src/lib/api.ts
    - web/src/app/settings/page.tsx
decisions:
  - Nightly window 02:00-04:00 IST (hours 2 and 3) — low-traffic window per RESEARCH.md
  - Once-per-day transfer tracked via NaiveDate (survives server restart within window)
  - Remote reachability checked every tick (not just nightly) so dashboard reflects current state
  - backup/status placed in staff_routes (not public) — backup health is internal operational data
  - SCP timeout 120s per RESEARCH.md recommendation for 50-500MB SQLite files
  - Only racecontrol.db transferred (telemetry.db deferred — noted in plan as secondary)
  - No hardcoded IPs — all host/path from config.backup.remote_host / remote_path
metrics:
  duration: "~6 minutes"
  completed_date: "2026-04-01"
  tasks_completed: 2
  files_modified: 4
  files_created: 0
---

# Phase 300 Plan 02: Offsite Transfer + Backup Status API + Dashboard Card Summary

Nightly SCP offsite transfer to Bono VPS with SHA256 integrity verification, REST status endpoint, and admin settings page Backup Status card.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Nightly SCP + SHA256 verification + remote reachability check | 021d2565 | backup_pipeline.rs |
| 2 | GET /api/v1/backup/status + BackupStatus interface + admin card | 81a3fd9f | routes.rs, api.ts, settings/page.tsx |

## What Was Built

**Task 1: Nightly SCP to Bono VPS with SHA256 Verification**

- `check_remote_reachable(state)` — async fn, called every backup tick
  - Runs `ssh -o StrictHostKeyChecking=no -o BatchMode=yes -o ConnectTimeout=10 <remote_host> echo ok`
  - Updates `BackupStatus.remote_reachable` every tick (dashboard always current)
  - Returns silently on failure (non-fatal — remote may be unreachable due to maintenance)

- `transfer_to_remote(state, backup_path, filename, last_remote_transfer)` — async fn
  - Guards: `remote_enabled` check, IST hour 2 or 3, NaiveDate dedup
  - Step A: `ssh mkdir -p <remote_path>` to ensure directory exists
  - Step B: `tokio::fs::read` + `sha2::Sha256::digest` → local checksum (hex)
  - Step C: `scp` with 120s `tokio::time::timeout` + StrictHostKeyChecking=no
  - Step D: `ssh sha256sum <remote_path>/<filename>` → parse first 64 chars → compare
  - Step E: Update `BackupStatus.remote_reachable`, `last_remote_transfer_at`, `last_checksum_match`
  - Checksum mismatch fires WhatsApp alert: `[BACKUP] Remote checksum MISMATCH for {filename}`
  - All config values cloned before async IO (no lock across .await)
  - No .unwrap() — all errors via ? or if let Err

- `spawn()` updated: adds `last_remote_transfer: Option<NaiveDate>` alongside `last_alert_fired`
- `backup_tick()` updated: calls `check_remote_reachable` + `transfer_to_remote` at end of each tick

**Task 2: REST Endpoint + Frontend**

- `get_backup_status` handler in `routes.rs`:
  - Reads `state.backup_status.read().await.clone()` (snapshot, no lock held)
  - Returns `Json(status)` — serializes all 8 BackupStatus fields
  - Registered in `staff_routes` as `GET /backup/status` (staff JWT required)
  - Route uniqueness: exactly 1 `.route("/backup/status", ...)` registration

- `BackupStatus` TypeScript interface in `api.ts`:
  - All 8 fields explicitly typed (no `any`): `last_backup_at: string | null`, `last_backup_size_bytes: number | null`, `last_backup_file: string | null`, `remote_reachable: boolean`, `last_remote_transfer_at: string | null`, `last_checksum_match: boolean | null`, `backup_count_local: number`, `staleness_hours: number | null`
  - `api.backupStatus()` method added

- Backup Status card in `settings/page.tsx`:
  - State: `const [backup, setBackup] = useState<BackupStatus | null>(null)`
  - useEffect: `api.backupStatus().then(setBackup).catch(() => {})`
  - Card rows: Last Backup, Size (MB formatted), Local Backups count, Remote (Bono VPS) reachability (emerald/red), Last Transfer, Checksum Match (OK/MISMATCH/---)
  - Staleness warning: amber `AlertTriangle` if `staleness_hours > 2`
  - `AlertTriangle` already imported (no duplicate import added)
  - No `any` type anywhere in the card

## Verification

```
cargo check -p racecontrol-crate: PASS (Finished dev profile)
cargo test -p racecontrol-crate --lib backup: 12 passed; 0 failed
grep "scp" crates/racecontrol/src/backup_pipeline.rs: 6 matches
grep "sha256sum" crates/racecontrol/src/backup_pipeline.rs: 4 matches
grep "Sha256::digest" crates/racecontrol/src/backup_pipeline.rs: 1 match
grep "StrictHostKeyChecking=no" crates/racecontrol/src/backup_pipeline.rs: 5 matches
grep "remote_reachable" crates/racecontrol/src/backup_pipeline.rs: 8 matches
grep "last_remote_transfer_at" crates/racecontrol/src/backup_pipeline.rs: 1 match
grep "100.70.177.44" crates/racecontrol/src/backup_pipeline.rs: 0 matches (no hardcoded IPs)
grep ".route(\"/backup/status\"" crates/racecontrol/src/api/routes.rs: 1 match (unique)
grep "backupStatus" web/src/lib/api.ts: 1 match
grep "BackupStatus" web/src/lib/api.ts: 2 matches (interface + method return type)
grep "Backup Status" web/src/app/settings/page.tsx: 2 matches (heading + card title)
grep "MISMATCH" web/src/app/settings/page.tsx: 1 match
```

## Deviations from Plan

None — plan executed exactly as written.

Pre-existing integration test failure in `crates/racecontrol/tests/integration.rs` (BillingTimer missing `nonce` field) is out of scope per deviation rules scope boundary (pre-dates this plan).

## Known Stubs

None. All functionality is wired end-to-end:
- `check_remote_reachable` updates `BackupStatus.remote_reachable` every tick
- `transfer_to_remote` transfers racecontrol daily backup and updates all remote fields
- `get_backup_status` endpoint returns live state
- Dashboard card displays all 8 fields with proper null handling

## Self-Check: PASSED
