---
phase: 351-data-durability
plan: "01"
subsystem: backup-scripts
tags: [backup, data-durability, sqlite, schtask, whatsapp-alert, rsync]
requirements: [OPS-08, OPS-09, OPS-10, OPS-11, OPS-14]

dependency_graph:
  requires: []
  provides: [backup-databases-sh-patched, database-backup-schtask-registered]
  affects: [data-recovery, backup-monitoring]

tech_stack:
  added: []
  patterns: [bash-failure-tracking, first-of-month-retention, rsync-remote-sync, comms-link-alert]

key_files:
  created: []
  modified:
    - scripts/backup-databases.sh
    - scripts/register-backup-task.bat

decisions:
  - "Use C:\\PROGRA~1\\Git 8.3 short path in schtask /TR to avoid space-in-path quoting failure"
  - "Cloud admin.db path verified via SSH as /root/racingpoint-admin/data/admin.db (not the CONTEXT.md assumed path /root/racingpoint/racingpoint-admin/admin.db)"
  - "Venue admin.db path defaulted to C:\\RacingPoint\\racingpoint-admin\\data\\admin.db (server unreachable during edit — path based on admin-deploy.sh convention)"

metrics:
  duration_minutes: 4
  completed_date: "2026-04-11"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 2
---

# Phase 351 Plan 01: Backup Script Hardening Summary

Patched `scripts/backup-databases.sh` to close 5 gaps found in the 2026-04-11 live audit — silent failures, missing admin.db backup, no first-of-month retention, no post-backup validation, no rsync offsite sync — and registered the `DatabaseBackup` schtask on James .27 to run daily at 03:00 IST.

## Tasks Completed

| Task | Commit | Files |
|------|--------|-------|
| Task 1: Patch backup-databases.sh | `2cdd71ed` | scripts/backup-databases.sh |
| Task 2: Register DatabaseBackup schtask | `abf00a22` | scripts/register-backup-task.bat |

## What Was Built

### Task 1: backup-databases.sh (6 changes)

1. **Failure tracking (OPS-09):** Added `FAILURES=0` at top. All FAIL paths now increment `FAILURES=$((FAILURES + 1))`. Script exits with `exit $FAILURES` so the schtask LastTaskResult reflects actual success/failure (was: always exit 0).

2. **Venue admin.db backup (OPS-08):** Added SSH+SCP block after racecontrol.db for `C:\RacingPoint\racingpoint-admin\data\admin.db`. Path defaulted to admin-deploy.sh convention (server unreachable at edit time — documented assumption).

3. **Cloud admin.db backup (OPS-08):** Added SSH+SCP block for Bono VPS `/root/racingpoint-admin/data/admin.db`. Path was **verified via SSH** — differed from CONTEXT.md assumption (`/root/racingpoint/racingpoint-admin/admin.db`), actual path is `/root/racingpoint-admin/data/admin.db`.

4. **First-of-month retention (OPS-10):** Replaced simple prune loop with logic that skips `*-01_*` directories. Directories matching `-01_` pattern are kept unless older than 12 months (checked via `date -d "-12 months"`).

5. **Rsync to Bono VPS (OPS-11):** Added `rsync -az --timeout=30` step after summary block, syncing `$BACKUP_DIR/` to `root@100.70.177.44:/root/backups/venue/$TIMESTAMP/`.

6. **Post-backup validation with WhatsApp alert (OPS-14):** Added loop checking 4 critical DBs (`racecontrol.db`, `admin.db`, `racecontrol-cloud.db`, `admin-cloud.db`) for existence and non-zero size. On any missing/empty DB, fires `curl POST http://localhost:8766/relay/alert` with alert message, then increments FAILURES.

### Task 2: register-backup-task.bat + schtask

- **Fixed bat file:** Replaced parentheses in if/else with `goto` labels (CLAUDE.md .bat rule — parentheses cause silent failures).
- **Fixed path quoting:** Used `C:\PROGRA~1\Git` (8.3 short path) in `/TR` argument — quoted paths with spaces in schtasks /TR fail silently.
- **Registered task:** `DatabaseBackup` task created on James .27 — Status: Ready, Next Run Time: 12-04-2026 03:00:00 IST.

## Verification Results

```
bash -n scripts/backup-databases.sh          → PASS (no syntax errors)
grep -c "admin.db" scripts/backup-databases.sh → 9 (venue + cloud + validation references)
grep "FAILURES=" scripts/backup-databases.sh  → FAILURES=0 at top
grep "exit.*FAILURES" scripts/backup-databases.sh → exit $FAILURES at bottom
grep "\-01_" scripts/backup-databases.sh      → first-of-month retention present
grep "relay/alert" scripts/backup-databases.sh → WhatsApp alert present
grep "rsync.*100.70.177.44" scripts/backup-databases.sh → rsync step present
grep -c 'FAILURES=\$((FAILURES + 1))' → 10 failure increment points
schtasks /Query /TN "DatabaseBackup"          → Status: Ready, Next Run: 12-04-2026 03:00:00
```

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] register-backup-task.bat used parentheses in if/else**
- **Found during:** Task 2 — reading the existing bat file
- **Issue:** `if %ERRORLEVEL% EQU 0 (echo ...) else (echo ...)` — parentheses in .bat if/else cause silent failures per CLAUDE.md standing rule
- **Fix:** Replaced with `if %ERRORLEVEL% EQU 0 goto :success` + `goto :done` + `:success` / `:done` labels
- **Files modified:** scripts/register-backup-task.bat
- **Commit:** `abf00a22`

**2. [Rule 1 - Bug] register-backup-task.bat space-in-path quoting failure**
- **Found during:** Task 2 — schtask registration test
- **Issue:** `"C:\Program Files\Git\bin\bash.exe"` in /TR fails because schtasks truncates at the space even with quoting
- **Fix:** Changed to `C:\PROGRA~1\Git\bin\bash.exe` (8.3 short path, no quoting needed)
- **Files modified:** scripts/register-backup-task.bat
- **Commit:** `abf00a22`

**3. [Rule 2 - Discovery] Cloud admin.db path differs from CONTEXT.md assumption**
- **Found during:** Task 1 — verified via `ssh root@100.70.177.44 "find /root -name 'admin.db'"`
- **Issue:** CONTEXT.md D2 assumed path `/root/racingpoint/racingpoint-admin/admin.db` — actual path is `/root/racingpoint-admin/data/admin.db`
- **Fix:** Used verified path in script
- **Files modified:** scripts/backup-databases.sh
- **Commit:** `2cdd71ed`

## Known Stubs

**Venue admin.db path (unverified):** `C:\RacingPoint\racingpoint-admin\data\admin.db` — server .23 was unreachable via SSH during edit (outside venue hours). Path is based on admin-deploy.sh convention. First backup run at 03:00 IST will confirm or fail. If it fails, check the actual path via `ssh ADMIN@100.125.108.37 "dir /s /b C:\\RacingPoint\\*admin.db"` and update line 36 of backup-databases.sh.

## Requirements Coverage

| Requirement | Status | Evidence |
|-------------|--------|---------|
| OPS-08 admin.db backup | DONE | Venue + cloud admin.db backup blocks added |
| OPS-09 failure tracking + non-zero exit | DONE | FAILURES counter + exit $FAILURES |
| OPS-10 first-of-month 12-month retention | DONE | -01_ pattern exemption in prune loop |
| OPS-11 rsync to Bono VPS | DONE | rsync -az step after summary |
| OPS-14 post-backup validation + WhatsApp alert | DONE | CRITICAL_DBS loop + /relay/alert curl |

## Self-Check: PASSED

- `scripts/backup-databases.sh` modified and committed at `2cdd71ed` ✓
- `scripts/register-backup-task.bat` modified and committed at `abf00a22` ✓
- `schtasks /Query /TN "DatabaseBackup"` → Status: Ready ✓
- `bash -n scripts/backup-databases.sh` → exit 0 ✓
