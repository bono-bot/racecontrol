---
phase: 300-sqlite-backup-pipeline
verified: 2026-04-01T15:15:00+05:30
status: passed
score: 5/5 must-haves verified
re_verification: false
gaps: []
human_verification:
  - test: "Trigger a backup tick and verify racecontrol.db file appears in ./data/backups/"
    expected: "File named racecontrol-YYYY-MM-DDTHH-MM-SS.db created within seconds"
    why_human: "VACUUM INTO requires a live SQLite connection — cannot exercise without running server"
  - test: "Open admin settings page as staff user and confirm Backup Status card renders with data"
    expected: "Card shows Last Backup, Size, Local Backups count, Remote reachability status, Last Transfer, Checksum Match fields — all populated or showing --- correctly"
    why_human: "Visual UI rendering + JWT-authenticated API call requires a running server and browser"
  - test: "Set clock to 02:30 IST on server (or wait for nightly window) and verify SCP fires"
    expected: "backup_pipeline log shows 'Nightly remote transfer complete', racecontrol-*.db appears on Bono VPS at /root/racecontrol-backups/"
    why_human: "Nightly SCP requires live SSH access to Bono VPS and the 02:00-04:00 IST window cannot be simulated in static analysis"
---

# Phase 300: SQLite Backup Pipeline — Verification Report

**Phase Goal:** Operational databases are continuously backed up and staff can see backup health at a glance
**Verified:** 2026-04-01T15:15:00 IST
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | Server creates a WAL-safe backup of racecontrol.db and telemetry.db every hour | VERIFIED | `VACUUM INTO` SQL used in `backup_tick()` for both `state.db` and `state.telemetry_db`. `tokio::time::interval(Duration::from_secs(interval_secs))` with default 3600. `backup_pipeline.rs` lines 119–196. |
| 2 | Backup directory never grows beyond 7 daily + 4 weekly files per database | VERIFIED | `rotate_backups()` at `backup_pipeline.rs:434–505` — sorts daily/weekly files by ISO timestamp name, deletes oldest beyond `daily_retain`/`weekly_retain`. 5 unit tests pass (retain, oldest-deleted, weekly separate). |
| 3 | WhatsApp alert fires if no successful backup exists within 2 hours, with debounce | VERIFIED | `check_staleness()` at line 568 reads `staleness_hours` from `BackupStatus`, compares against `staleness_alert_hours` (default 2h), fires `send_whatsapp()` with debounce = `2 * staleness_alert_hours * 3600s`. Debounce test passes. |
| 4 | Nightly backup file appears on Bono VPS with matching SHA256 checksum | VERIFIED (logic) | `transfer_to_remote()` at lines 278–432: SSH mkdir, local `sha2::Sha256::digest`, SCP with 120s timeout, remote `sha256sum`, parse + compare, mismatch fires WhatsApp. No hardcoded IPs. Remote reachability checked every tick via `check_remote_reachable()`. Cannot verify live transfer programmatically — flagged for human verification. |
| 5 | Admin settings page shows last backup time, size, and remote reachability | VERIFIED | `GET /backup/status` registered in `staff_routes` (line 553). Handler reads `state.backup_status.read().await.clone()`. `BackupStatus` TypeScript interface at `api.ts:306` with 8 explicitly typed fields. `settings/page.tsx` has full Backup Status card with all required fields rendered via `useEffect` → `api.backupStatus()`. |

**Score:** 5/5 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/backup_pipeline.rs` | Hourly backup loop, rotation, staleness alert | VERIFIED | 871 lines (150 min). Contains `VACUUM INTO`, `rotate_backups`, `send_whatsapp`, `staleness_alert_hours`, `spawn`. |
| `crates/racecontrol/src/config.rs` | BackupConfig struct with serde defaults | VERIFIED | `pub struct BackupConfig` at line 929, `pub backup: BackupConfig` at line 70 of Config. |
| `crates/racecontrol/src/backup_pipeline.rs` (Plan 02) | Nightly SCP transfer with SHA256 verification | VERIFIED | Contains `scp`, `sha256sum`, `Sha256::digest`, `StrictHostKeyChecking=no`, `remote_reachable`. Zero matches for hardcoded `100.70.177.44`. |
| `crates/racecontrol/src/api/routes.rs` | GET /api/v1/backup/status endpoint | VERIFIED | `get_backup_status` handler registered at `.route("/backup/status", get(get_backup_status))` in `staff_routes` (line 553, function begins line 301). |
| `web/src/app/settings/page.tsx` | Backup Status card in admin dashboard | VERIFIED | "Backup Status" heading at line 136, all 6 display fields including `remote_reachable`, `MISMATCH`, `staleness_hours` warning. |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `main.rs` | `backup_pipeline::spawn` | tokio::spawn in startup sequence | WIRED | `backup_pipeline::spawn(state.clone())` at main.rs line 957, after `scheduler::spawn`. Module in use block at line 25. |
| `backup_pipeline.rs` | `whatsapp_alerter::send_whatsapp` | staleness alert call | WIRED | Called at lines 412 (checksum mismatch) and 603 (staleness alert). |
| `backup_pipeline.rs` | `state.db` | VACUUM INTO SQL command | WIRED | `sqlx::query(&vacuum_sql).execute(&state.db).await` at line 121–123. Telemetry: `execute(telemetry_db).await` at line 167–169. |
| `backup_pipeline.rs` | Bono VPS root@100.70.177.44 | tokio::process::Command scp + ssh sha256sum | WIRED | SCP at line 337, sha256sum at line 377, all using `config.backup.remote_host` (no hardcoded IPs). |
| `routes.rs` | `state.backup_status` | RwLock read in handler | WIRED | `state.backup_status.read().await.clone()` at routes.rs line 679. |
| `settings/page.tsx` | `/api/v1/backup/status` | fetch in useEffect | WIRED | `api.backupStatus().then(setBackup)` at page.tsx line 27 inside `useEffect`. `fetchApi<BackupStatus>("/backup/status")` in api.ts line 614. |

---

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `settings/page.tsx` | `backup` (BackupStatus) | `api.backupStatus()` → `GET /backup/status` → `state.backup_status.read()` | Yes — RwLock populated by `backup_pipeline.rs` on every tick | FLOWING |
| `routes.rs` `get_backup_status` | `state.backup_status` | Written by `backup_tick()` and `check_remote_reachable()` on every interval tick | Yes — last_backup_at, size, file, count, staleness, remote_reachable all populated from live operations | FLOWING |

---

### Behavioral Spot-Checks

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| `cargo check -p racecontrol-crate` compiles without errors | `cargo check -p racecontrol-crate` | `Finished dev profile [unoptimized + debuginfo] target(s) in 9.63s` (1 warning: irrefutable_let_patterns — non-blocking) | PASS |
| All 12 backup unit tests pass | `cargo test -p racecontrol-crate --lib backup` | `test result: ok. 12 passed; 0 failed; 0 ignored` | PASS |
| No hardcoded IPs in backup_pipeline.rs | `grep "100\.70\.177\.44" backup_pipeline.rs` | 0 matches | PASS |
| Route uniqueness: exactly 1 backup/status registration | `grep "backup/status" routes.rs` | Exactly 1 `.route` call at line 553 | PASS |
| No TypeScript `any` in backup-related code | `grep ": any\|as any" settings/page.tsx` | 0 matches | PASS |
| No `.unwrap()` in production Rust (outside tests) | `grep -n "\.unwrap()" backup_pipeline.rs` | All matches in `#[cfg(test)]` block (line 609+). Zero in production code. | PASS |
| 4 commits verified in git history | `git show --stat <hash>` for each | All 4 commits exist with correct author `James Vowles <james@racingpoint.in>` and matching file changes | PASS |
| Nightly SCP behavior | Runtime only — requires live SSH + nightly window | Cannot exercise statically | SKIP (human verification) |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| BACKUP-01 | 300-01 | Server performs hourly SQLite .backup (WAL-safe) of all operational databases | SATISFIED | `VACUUM INTO` in `backup_tick()` for both racecontrol.db and telemetry.db; `interval_secs` default 3600. `backup_pipeline.rs` lines 119–196. |
| BACKUP-02 | 300-01 | Local backup rotation retains 7 daily + 4 weekly snapshots, auto-purging older files | SATISFIED | `rotate_backups()` function with `daily_retain=7`, `weekly_retain=4` defaults. 5 rotation unit tests pass verifying exact counts. |
| BACKUP-03 | 300-02 | Nightly backup is SCP'd to Bono VPS with integrity verification (SHA256 match) | SATISFIED (logic) | `transfer_to_remote()` implements full A-E flow: mkdir, local SHA256, SCP 120s timeout, remote sha256sum, compare + WhatsApp on mismatch. Cannot live-verify without running server in 02:00 IST window. |
| BACKUP-04 | 300-01 | WhatsApp alert fires if newest backup is older than 2 hours (staleness detection) | SATISFIED | `check_staleness()` computes `staleness_hours` via `compute_staleness()`, compares to `staleness_alert_hours` (default 2), calls `send_whatsapp()` with 2x debounce. Debounce test passes. |
| BACKUP-05 | 300-02 | Backup status visible in admin dashboard (last backup time, size, destination health) | SATISFIED | `GET /backup/status` returns 8-field `BackupStatus` JSON. Admin settings card at `settings/page.tsx:134–180` shows all fields: last backup, size MB, local count, remote reachability (colour-coded), last transfer, checksum match status, staleness warning. |

All 5 requirement IDs (BACKUP-01 through BACKUP-05) from both plan frontmatter entries are accounted for and satisfied. REQUIREMENTS.md shows all 5 marked `[x]` with Phase 300 mapped to Complete.

No orphaned requirements found.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `backup_pipeline.rs` | 609+ | `.unwrap()` in test code | Info | All `.unwrap()` calls are inside `#[cfg(test)]` block — test-only code, not production. No standing rule violation. |
| `backup_pipeline.rs` | 394 | `split_whitespace().next().unwrap_or("")` | Info | Safe `unwrap_or` default, not a panic risk. Correctly handles empty ssh output. |
| `backup_pipeline.rs` (spawn) | 57 | `chrono::Utc::now().checked_sub_signed(...).unwrap_or(chrono::Utc::now())` | Warning | One `unwrap_or` in production code (startup initialization only, not in tick loop). Fallback is safe (uses now()), so this is not a crash risk. Non-blocking. |

No blockers found. The single `unwrap_or` in production code uses a safe fallback and is in the startup initialization path, not the hot tick path.

---

### Human Verification Required

#### 1. Live VACUUM INTO backup creation

**Test:** Start the server, wait for the first tick (1 hour, or temporarily set `interval_secs = 5` in TOML), then check `./data/backups/` for a `racecontrol-*.db` file.
**Expected:** File appears within `interval_secs` seconds, is a valid SQLite database (`file racecontrol-*.db` returns "SQLite 3.x database").
**Why human:** VACUUM INTO requires a live SQLite pool — cannot exercise without a running server.

#### 2. Admin Backup Status card visual verification

**Test:** Log in as staff to admin dashboard → Settings page.
**Expected:** "Backup Status" card renders with all 6 rows populated. Remote shows "Reachable" (emerald) or "Unreachable" (red). If no backup has run, fields show "Never" / "---" (not a JS error).
**Why human:** Visual rendering + JWT-authenticated fetch requires a running browser session.

#### 3. Nightly SCP transfer to Bono VPS

**Test:** During the 02:00–04:00 IST window, monitor server logs for `[backup_pipeline] Starting nightly remote transfer`.
**Expected:** Log shows transfer start, SCP complete, checksum match. File appears at `root@100.70.177.44:/root/racecontrol-backups/racecontrol-*.db`.
**Why human:** Requires live SSH connectivity to Bono VPS and cannot be simulated in the 2h nightly window without actually running the server at that time.

---

### Gaps Summary

No gaps. All automated checks pass. All 5 requirements satisfied. All 5 must-have truths verified against actual codebase with direct grep, file read, and test execution evidence.

Three items flagged for human verification (live backup creation, UI card render, nightly SCP) are runtime behaviors that require a running server — they are expected to work given the completeness and correctness of the implementation, but cannot be proven without execution.

---

_Verified: 2026-04-01T15:15:00 IST_
_Verifier: Claude (gsd-verifier)_
