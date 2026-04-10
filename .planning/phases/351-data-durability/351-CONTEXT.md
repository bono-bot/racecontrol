---
phase: 351
name: Data Durability
slug: data-durability
type: context
created: "2026-04-11T12:00:00+05:30"
mode: auto
---

<domain>
## Phase Boundary

Daily point-in-time SQLite backups of both `racecontrol.db` (venue + cloud) and `admin.db` (venue + cloud), with 30-day rolling retention + first-of-month snapshots for 12 months, nightly rsync to Bono VPS, WAL mode verification at startup, and a quarterly restore drill SOP with first execution + LOGBOOK entry. Alert fires if backup is missing or size 0 after the scheduled window.

This is DISTINCT from Phase 349 (real-time 5-minute Google Drive sync). Phase 349 = continuous sync for disaster recovery of the live DB. Phase 351 = daily point-in-time snapshots for long-term retention, rollback to a specific day, and formal restore drill.

</domain>

<decisions>
## Implementation Decisions

### D1: Venue Backup Script — Extend Rust Pipeline vs New PS1 Script

[auto] Selected: **Extend the existing Rust `backup_pipeline.rs`** — do NOT create a separate `backup-sqlite.ps1`.

The existing `backup_pipeline.rs` (Phase 300) already handles `racecontrol.db` with VACUUM INTO (WAL-safe), hourly runs, daily/weekly rotation, SCP to Bono VPS, SHA256 verification, and WhatsApp staleness alerting. Creating a parallel PS1 script would duplicate logic and create two competing backup systems.

Phase 351 extends `backup_pipeline.rs` with:
- Add admin.db to the backup loop (alongside racecontrol.db and telemetry.db)
- Add daily-at-03:00 IST gate using the same IST window pattern as the nightly SCP transfer
- Add monthly retention tier (first-of-month snapshots retained 12 months)
- Bump `daily_retain` default from 7 to 30
- Switch remote transfer from SCP to rsync

**For cloud (Bono VPS):** Add a new `backup-cloud.sh` shell script (since cloud runs Linux with no Rust binary managing cloud-specific backup config). The cloud script backs up the cloud racecontrol.db and the cloud admin.db using `sqlite3 .backup`, runs at 03:00 IST (21:30 UTC) via cron, and stores locally in `/root/backups/cloud/`.

**Why:** No new processes, no parallel backup systems. One Rust backup pipeline covers venue; one shell script covers cloud.

### D2: Admin.db Backup — Where to Run

[auto] Selected: **Two separate backup operations:**
1. **Venue admin.db** — backed up by the extended Rust `backup_pipeline.rs` on the server (.23). Path configured via `backup.admin_db_path` in racecontrol.toml. Researcher should verify actual deploy path from `admin-deploy.sh` (likely `C:\RacingPoint\admin\data\admin.db`).
2. **Cloud admin.db** — backed up by the new `backup-cloud.sh` script on Bono VPS. Path: `/root/racingpoint/racingpoint-admin/data/admin.db`.

**Why:** Venue backup is server-side (where racecontrol.db already lives). Cloud backup is Bono-VPS-side.

### D3: Retention Policy

[auto] Selected: **Three-tier retention in `backup_pipeline.rs`:**
- **Daily:** Last 30 files per database prefix (up from current 7) — OPS-10
- **Weekly:** 4 files retained (unchanged — Sunday snapshots)
- **Monthly:** First backup of each calendar month, retained for 12 months. File naming: `<prefix>-monthly-YYYY-MM.db`

Config additions to `BackupConfig`:
```toml
[backup]
daily_retain = 30      # was 7
weekly_retain = 4      # unchanged
monthly_retain = 12    # new field
```

Monthly detection: `now_ist.day() == 1 && now_ist.hour() == 3` — copy daily backup to monthly filename on 1st of month at 03:00 IST.

**Why:** OPS-10 requires exactly this. The existing `rotate_backups()` pattern is established for daily/weekly; monthly is a third tier with the same logic.

### D4: Remote Transfer — Rsync vs SCP

[auto] Selected: **Rsync for the daily backup transfer.** ROADMAP explicitly says "rsync to Bono VPS." Replace the existing nightly SCP transfer in `backup_pipeline.rs` with rsync.

Rsync command pattern (via Git Bash rsync on Windows):
```bash
rsync -az --checksum --no-perms \
  -e "ssh -o StrictHostKeyChecking=no -o BatchMode=yes" \
  {local_backup_file} \
  root@{remote_host}:/root/backups/venue/
```

**Fallback:** If `rsync.exe` is not available on the server (`C:\Program Files\Git\usr\bin\rsync.exe`), use the existing SCP path with a config flag `backup.use_rsync = false`. Researcher must verify rsync availability on server .23.

**Why:** OPS-11 specifies rsync. Rsync also handles partial file transfers more gracefully than SCP.

### D5: WAL Mode Verification at Startup (OPS-12)

[auto] Selected: **Already implemented — no new code needed.**

- `racecontrol.db`: `crates/racecontrol/src/db/mod.rs:22-37` — verifies WAL with `bail!` if mode != "wal". Shipped in Phase 345 as RESIL-01.
- `admin.db`: `racingpoint-admin/src/lib/db.ts:60` — `db.pragma('journal_mode = WAL')` in lazy-load `getDb()`. Shipped in Phase 345.

Phase 351 documents OPS-12 as satisfied by citing these two files. No code changes required for WAL verification.

### D6: Backup Alert for Missing/Zero-Size Files (OPS-14)

[auto] Selected: **Extend existing `check_staleness()` in `backup_pipeline.rs`** to also check backup file size.

Currently `check_staleness()` fires when `staleness_hours > threshold`. Add a size-check: after each VACUUM INTO, verify file size > 0 bytes. If size is 0 or the file doesn't exist after the VACUUM INTO call returns Ok, fire an immediate WhatsApp alert (bypass normal debounce — zero-byte backup is always an emergency).

Cloud script (`backup-cloud.sh`): exits non-zero and logs to stderr if any backup file is size 0, letting cron mail the failure.

**Why:** OPS-14 specifically requires the alert for "missing or size 0." The staleness check alone misses the case where a backup runs but produces an empty file.

### D7: Restore Drill Scope

[auto] Selected: **Extend the existing `scripts/db-sync/RESTORE-DRILL.md`** to add a venue backup restore section alongside the existing cloud Google Drive restore procedure.

New section covers:
1. Find target daily backup on Bono VPS (`/root/backups/venue/`)
2. Download to scratch path on Bono VPS or James (`/tmp/drill-restore/`)
3. Run integrity check (`sqlite3 .backup` + `PRAGMA integrity_check`)
4. Verify row counts match expected (drivers, billing_sessions)
5. Log result in LOGBOOK.md with format: `| YYYY-MM-DD HH:MM IST | James | restore-drill-venue | [PASS/FAIL] Drivers: N, Sessions: M | notes |`

The **first execution** of this drill is the milestone artifact for Plan 351-03.

**Why:** Phase 349 already created this file for the cloud path. Extending is cleaner than two separate docs.

### D8: Backup Schedule Timing

[auto] Selected: **03:00 IST daily** for both venue and cloud.

- **Venue (Rust pipeline):** Add IST-hour gate `now_ist.hour() == 3` to the daily backup block. Follows the existing nightly SCP window check pattern.
- **Cloud (cron):** `30 21 * * *` in Bono VPS crontab (03:00 IST = 21:30 UTC).

### Claude's Discretion

- Exact path for venue admin.db (researcher must verify from `admin-deploy.sh` or server file system — likely `C:\RacingPoint\admin\data\admin.db`)
- Whether telemetry.db also gets the 30-day retention upgrade (currently 7 daily). Researcher can check telemetry.db size to advise — if too large, keep at 7.
- Rsync availability on server .23 (`C:\Program Files\Git\usr\bin\rsync.exe`) — researcher must verify

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing Backup Pipeline (Phase 300)
- `crates/racecontrol/src/backup_pipeline.rs` — Core backup implementation (VACUUM INTO, rotation, SCP transfer, staleness alert). PRIMARY file to extend.
- `crates/racecontrol/src/config.rs` §BackupConfig (lines ~1095-1145) — `daily_retain`, `weekly_retain`, `remote_host`, `remote_enabled`, `staleness_alert_hours`. Add `monthly_retain`, `admin_db_path`, `use_rsync`.

### WAL Mode (OPS-12 — already satisfied)
- `crates/racecontrol/src/db/mod.rs:22-37` — WAL verify + fail-fast for racecontrol.db
- `racingpoint-admin/src/lib/db.ts:60` — WAL pragma for admin.db

### DB Sync Scripts (Phase 349 — related but distinct)
- `scripts/db-sync/upload-db.ps1` — 5-min Google Drive upload (NOT the backup system)
- `scripts/db-sync/download-db.sh` — 5-min Google Drive download (NOT the backup system)
- `scripts/db-sync/RESTORE-DRILL.md` — Existing restore drill for cloud path. Phase 351 adds venue section here.

### Backup Status API
- `crates/racecontrol/src/api/routes.rs:621-622` — `GET /api/v1/backup/status` route
- `crates/racecontrol/src/api/routes.rs:798-810` — `get_backup_status()` handler

### Existing Bat Backup Registration
- `scripts/register-backup-task.bat` — Registers bash backup task (bash-based, secondary to Rust pipeline)
- `scripts/backup-databases.sh` — Multi-host bash backup (secondary to Rust pipeline)

### Requirements
- `.planning/REQUIREMENTS.md` — OPS-08..14

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `backup_pipeline.rs::backup_tick()` — Main backup loop. Extend by adding admin.db VACUUM INTO block.
- `backup_pipeline.rs::rotate_backups()` — Rotation logic (daily + weekly). Add monthly tier.
- `backup_pipeline.rs::check_staleness()` — WhatsApp alert for stale backups. Extend to also check zero-byte.
- `backup_pipeline.rs::transfer_to_remote()` — Current SCP implementation. Replace with rsync via `std::process::Command`.
- `backup_pipeline.rs::check_remote_reachable()` — SSH pre-flight ping. Reuse unchanged.

### Established Patterns
- **IST time-of-day gate:** Uses `now_ist.hour()` for nightly SCP window (02:00-04:00). Same pattern for daily-at-03:00 gate.
- **VACUUM INTO** (not file copy) — WAL-safe atomic snapshot. Must continue. Forward slashes in SQL path even on Windows.
- **No lock across .await** — backup_tick snapshots state before async work. Maintain.
- **StrictHostKeyChecking=no + BatchMode=yes** — All SSH/SCP/rsync calls.
- **No hardcoded IPs** — `remote_host` always from config.
- **`BackupStatus` in `AppState`** — `RwLock<BackupStatus>` updated after each tick. Add `last_admin_backup_at` and `last_admin_backup_size` fields.

### Integration Points
- `AppState::backup_status` — `RwLock<BackupStatus>` in state.rs. Add admin.db fields.
- `GET /api/v1/backup/status` — Will automatically expose admin.db backup state once `BackupStatus` is extended.
- `subsystem_health.rs::probe_admin_db()` — Currently checks file existence only. May be extended in Phase 352 for backup-freshness; not Phase 351 scope.

</code_context>

<specifics>
## Specific Ideas

- Monthly snapshot naming: `{prefix}-monthly-YYYY-MM.db` (e.g., `racecontrol-monthly-2026-04.db`)
- Venue backup rsync target on Bono VPS: `/root/backups/venue/`
- Cloud-local backup target: `/root/backups/cloud/`
- The Phase 349 Google Drive sync and Phase 351 daily backups serve different purposes and must NOT be merged. Phase 349 is the DR path; Phase 351 is the retention/rollback path.
- For the RESIL-01 WAL verification on racecontrol.db: it is a `bail!` (hard fail at startup). For admin.db, the WAL pragma is set on first lazy-load — no startup fail-fast. This is acceptable per Phase 345 design (admin.db is non-critical for racecontrol startup).

</specifics>

<deferred>
## Deferred Ideas

- **Admin dashboard backup status panel** — `/api/v1/backup/status` exists. Dashboard visualization deferred to Phase 354 (UI Hardening).
- **Automated quarterly restore drill** — OPS-13 requires quarterly drill with LOGBOOK entry. Drill is manual; calendar reminder is sufficient. Automating is deferred.
- **Telemetry.db 30-day retention upgrade** — OPS-09 covers racecontrol.db. Telemetry.db currently retains 7 daily copies. Upgrading is deferred pending size analysis (telemetry.db may be too large for 30 daily copies at 03:00 IST).
- **Bi-directional end-of-day rsync verification** — Automated SHA256 check between local backup and Bono VPS copy. Rsync `--checksum` handles in-transfer integrity; end-of-day remote check deferred.

</deferred>

---

*Phase: 351-data-durability*
*Context gathered: 2026-04-11*
