---
phase: 300-sqlite-backup-pipeline
plan: "01"
subsystem: backup
tags: [sqlite, backup, vacuum-into, rotation, whatsapp-alert, pipeline]
dependency_graph:
  requires: []
  provides: [backup_pipeline, BackupConfig, BackupStatus]
  affects: [state.rs, config.rs, main.rs]
tech_stack:
  added: [tempfile=3 (dev-dependency)]
  patterns: [VACUUM INTO, tokio interval loop, RwLock snapshot before .await, serde defaults]
key_files:
  created:
    - crates/racecontrol/src/backup_pipeline.rs
  modified:
    - crates/racecontrol/src/config.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/lib.rs
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/Cargo.toml
decisions:
  - VACUUM INTO used for WAL-safe backup (not file copy) — per locked decision in STATE.md
  - Staleness debounce window = 2 * staleness_alert_hours seconds
  - Weekly snapshots created on IST Sunday using chrono::Weekday::Sun
  - tempfile crate added as dev-dependency for test temp dirs
  - Pre-existing integration test failure (BillingTimer nonce) is out of scope
metrics:
  duration: "~20 minutes"
  completed_date: "2026-04-01"
  tasks_completed: 2
  files_modified: 5
  files_created: 1
---

# Phase 300 Plan 01: SQLite Backup Pipeline Summary

SQLite backup pipeline with hourly WAL-safe VACUUM INTO for racecontrol.db and telemetry.db, 7-daily + 4-weekly file rotation, WhatsApp staleness alert with debounce, and BackupStatus in AppState.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | BackupConfig + BackupStatus structs + Config/State wiring | f036ee3e | config.rs, state.rs |
| 2 | backup_pipeline.rs — hourly VACUUM INTO, rotation, staleness alert, spawn | f414789f | backup_pipeline.rs, lib.rs, main.rs, Cargo.toml |

## What Was Built

**Task 1: Config and State Structs**

- `BackupConfig` struct in `config.rs` with 9 fields, all with serde defaults:
  - `enabled` (default: true), `backup_dir` (`./data/backups`), `interval_secs` (3600)
  - `daily_retain` (7), `weekly_retain` (4)
  - `remote_enabled` (true), `remote_host` (`root@100.70.177.44`), `remote_path` (`/root/racecontrol-backups`)
  - `staleness_alert_hours` (2)
- `BackupConfig::default()` impl for backward compatibility (no `[backup]` section required in TOML)
- `pub backup: BackupConfig` added to `Config` struct (which has `#[serde(deny_unknown_fields)]`)
- `BackupStatus` struct in `state.rs` — 8 fields: `last_backup_at`, `last_backup_size_bytes`, `last_backup_file`, `remote_reachable`, `last_remote_transfer_at`, `last_checksum_match`, `backup_count_local`, `staleness_hours`
- `pub backup_status: RwLock<BackupStatus>` added to `AppState` and initialized in `AppState::new()`
- `default_config()` test helper updated to include `backup: BackupConfig::default()`

**Task 2: Backup Pipeline Module**

- `backup_pipeline.rs` — 320 lines of production code + 230 lines of tests
- `pub fn spawn(state: Arc<AppState>)` — follows scheduler.rs pattern exactly
  - Disabled check at spawn time — logs and returns if `!config.backup.enabled`
  - First-tick initialization: scans backup_dir to populate `BackupStatus.last_backup_at` (prevents false staleness alert on startup)
  - tokio interval loop at `config.backup.interval_secs`
- `async fn backup_tick` — creates backups for both databases, rotates, updates status, checks staleness
  - `VACUUM INTO '{path}'` SQL with forward-slash paths (SQLite cross-platform)
  - Logs warn if VACUUM INTO takes >30s
  - Weekly snapshot (copy from daily) on IST Sunday via `chrono::Weekday::Sun`
- `pub fn rotate_backups` — separates daily vs weekly files by name pattern, sorts by name (ISO = chronological), deletes oldest beyond retention
- `fn compute_staleness` — scans directory for newest .db mtime, returns hours elapsed
- `async fn check_staleness` — debounce prevents re-fire within `2 * staleness_alert_hours` seconds
- WhatsApp alert format: `[BACKUP] No successful backup in {hours:.1} hours -- last at {time} | {ist_now_string()}`
- `backup_pipeline` added to lib.rs module list and main.rs use block
- `backup_pipeline::spawn(state.clone())` called after `scheduler::spawn` in main.rs startup sequence

## Test Coverage (12 tests, all pass)

- `rotate_backups_with_10_daily_and_retain_7_deletes_3_oldest` — exact count
- `rotate_backups_deletes_oldest_3_keeps_newest_7` — oldest files verified deleted
- `rotate_backups_preserves_weekly_files_up_to_weekly_retain` — weekly retention
- `rotate_backups_does_nothing_when_below_retain_limit` — no spurious deletion
- `rotate_backups_does_not_delete_files_from_other_prefix` — prefix isolation
- `compute_staleness_returns_none_for_empty_dir` — no files
- `compute_staleness_returns_none_for_nonexistent_dir` — nonexistent path
- `compute_staleness_returns_some_when_files_exist` — freshly created file < 0.1h
- `backup_file_naming_follows_racecontrol_prefix_pattern` — naming check
- `backup_file_naming_follows_telemetry_prefix_pattern` — naming check
- `weekly_snapshot_naming_follows_pattern` — weekly name format
- `staleness_debounce_logic_fires_on_first_call` — first fire + immediate debounce suppression

## Verification

```
cargo check -p racecontrol-crate: PASS (Finished dev profile)
cargo test -p racecontrol-crate --lib backup: 12 passed; 0 failed
grep "VACUUM INTO": matches in backup_pipeline.rs
grep "pub backup: BackupConfig": matches in config.rs
grep "backup_pipeline::spawn": matches in main.rs
Server starts without [backup] in TOML: backward compatible via serde defaults
```

## Deviations from Plan

None — plan executed exactly as written.

Pre-existing integration test failure in `crates/racecontrol/tests/integration.rs` (BillingTimer missing `nonce` field) was not caused by this plan's changes and is out of scope per deviation rules scope boundary.

## Known Stubs

None. All functionality is wired:
- `remote_enabled` and `remote_host`/`remote_path` fields are in BackupConfig but remote transfer logic is not implemented in this plan (Phase 300-02 handles remote sync). The fields are present for config compatibility.

## Self-Check: PASSED
