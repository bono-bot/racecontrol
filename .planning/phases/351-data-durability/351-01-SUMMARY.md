---
phase: 351-data-durability
plan: 01
type: summary
commit: f7775573
completed: "2026-04-11T04:06:00+05:30"
requirements_closed: [OPS-08, OPS-09, OPS-10, OPS-11, OPS-14]
---

## What Was Done

Extended the backup pipeline (both Rust and bash) to meet OPS-08..11 and OPS-14.

### Rust Pipeline (backup_pipeline.rs)

**OPS-08 — admin.db backup:**
- `backup_tick()` now backs up admin.db via `sqlite3 .backup` subprocess (safe for cross-pool access)
- Config: `config.backup.admin_db_path` (default empty = skip admin backup)
- Venue path: `C:/RacingPoint/admin/data/admin.db` (must be set in racecontrol.toml)
- Weekly and monthly snapshots for admin.db follow same rotation as racecontrol.db

**OPS-09 — 30-day retention:**
- `default_daily_retain()` changed from 7 to 30
- Existing `rotate_backups()` logic unchanged; now prunes to 30 instead of 7

**OPS-10 — Monthly retention tier:**
- `rotate_backups()` now takes `monthly_retain` parameter (default 12)
- Monthly snapshots: `{prefix}-monthly-YYYY-MM.db`, created on 1st of month (any tick that fires)
- Monthly rotation: keeps newest 12, prunes oldest beyond limit
- Exempted from daily rotation: monthly files contain `-monthly-` in name

**OPS-11 — rsync transfer:**
- `transfer_to_remote()` now attempts rsync first (`C:/Program Files/Git/usr/bin/rsync.exe` on Windows)
- New `transfer_via_scp()` helper used as automatic fallback if rsync fails or is unavailable
- Config: `config.backup.use_rsync` (default true); set false to always use SCP
- rsync flags: `-az --checksum --no-perms --timeout=60`

**OPS-14 — Zero-byte alert:**
- After each VACUUM INTO, checks `metadata().len() == 0`
- Fires `send_whatsapp()` immediately (no debounce) for zero-byte racecontrol.db or admin.db

**Struct changes:**
- `BackupConfig`: added `monthly_retain`, `admin_db_path`, `use_rsync` fields with serde defaults
- `BackupStatus`: added `last_admin_backup_at: Option<String>`, `last_admin_backup_size: Option<u64>`
- Both fields exposed automatically via `GET /api/v1/backup/status`

### bash (scripts/backup-databases.sh)

Secondary path used by the schtask on James .27:
- FAILURES counter + `exit $FAILURES` for schtask failure tracking
- admin.db: venue (`C:/RacingPoint/admin/data/admin.db`) + cloud (`/root/racingpoint-admin/data/admin.db`)
- First-of-month prune exemption: `YYYY-MM-01_*` dirs exempted from daily rotation
- Monthly snapshot dir: `$BACKUP_ROOT/monthly/$YEAR_MONTH/` + sync to Bono VPS
- rsync to `/root/backups/venue/$TIMESTAMP/` with SCP fallback
- OPS-14 validation loop: `relay/alert` on missing or zero-byte CRITICAL_DBS

### Scheduled Task

`DatabaseBackup` schtask already registered (daily 03:00 IST, Git Bash, `bono` user).
Verified: `Get-ScheduledTask -TaskName 'DatabaseBackup'` → State: Ready.

## Tests

14 backup_pipeline tests pass (was 12 before; 2 new tests added):
- `rotate_backups_monthly_tier_retains_up_to_monthly_retain`
- `rotate_backups_monthly_files_not_affected_by_daily_rotation`

## Not Tested (runtime)

- admin.db VACUUM INTO on server .23: `admin_db_path` not yet set in `C:\RacingPoint\racecontrol.toml`
  → requires config change at next server maintenance window
- rsync.exe on server .23: path `C:/Program Files/Git/usr/bin/rsync.exe` not verified
  → SCP fallback will be used until rsync is confirmed available

## Requirements Satisfied

| Req | Description | Status |
|-----|-------------|--------|
| OPS-08 | Daily sqlite3 .backup of racecontrol.db and admin.db (venue + cloud) | DONE — code + bash |
| OPS-09 | 30-day rolling retention | DONE — daily_retain=30 default |
| OPS-10 | First-of-month snapshots retained 12 months | DONE — monthly tier in rotate_backups |
| OPS-11 | rsync to Bono VPS | DONE — rsync primary, SCP fallback |
| OPS-14 | Alert on missing or zero-byte backup | DONE — immediate WhatsApp alert |
