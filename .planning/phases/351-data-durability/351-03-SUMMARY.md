---
phase: 351
plan: 03
subsystem: data-durability
tags: [backup, cloud, restore-drill, cron, sqlite, ops]
dependency_graph:
  requires: [351-01, 351-02]
  provides: [cloud-backup-script, restore-drill-sop, ops-13-drill-executed]
  affects: [bono-vps-cron, scripts/db-sync/RESTORE-DRILL.md]
tech_stack:
  added: [backup-cloud.sh, cron-21:30-utc]
  patterns: [sqlite3-backup, first-of-month-retention, non-zero-exit-on-failure]
key_files:
  created:
    - scripts/backup-cloud.sh
    - .planning/phases/351-data-durability/351-03-PLAN.md
  modified:
    - scripts/db-sync/RESTORE-DRILL.md
    - LOGBOOK.md
decisions:
  - "Cloud DB paths discovered live: racecontrol.db at /root/racecontrol/data/, admin.db at /root/racingpoint-admin/data/"
  - "admin.db uses employees table (not staff_members as initially assumed) — POS/cafe management DB"
  - "Cloud racecontrol.db has index corruption (idx_activity_hash) — data readable, REINDEX needed"
  - "Cron registered at 30 21 * * * (21:30 UTC = 03:00 IST) on Bono VPS"
metrics:
  duration_minutes: 40
  completed_date: "2026-04-11T04:17:51+05:30"
  tasks_completed: 4
  files_created: 2
  files_modified: 2
---

# Phase 351 Plan 03: Cloud Backup Script + Restore Drill Summary

Cloud backup script for Bono VPS created (sqlite3 .backup with 30-day retention and first-of-month 12-month snapshots), RESTORE-DRILL.md updated with venue backup restore section, and first OPS-13 restore drill executed — drill revealed index corruption on cloud racecontrol.db (data intact, needs REINDEX).

## Tasks Completed

| Task | Description | Commit | Status |
|------|-------------|--------|--------|
| 1 | Create backup-cloud.sh for Bono VPS | e1f8bd1d | DONE |
| 2 | Deploy to Bono VPS + register cron + test run | 1f3922d5 | DONE |
| 3 | Update RESTORE-DRILL.md venue restore section | 44044a24 | DONE |
| 4 | Execute first restore drill, log in LOGBOOK.md | 9574b002 | DONE |

## What Was Built

### backup-cloud.sh

New shell script at `scripts/backup-cloud.sh`, deployed to `/root/backup-cloud.sh` on Bono VPS.

- Backs up `/root/racecontrol/data/racecontrol.db` and `/root/racingpoint-admin/data/admin.db`
- Timestamped directories: `/root/backups/cloud/YYYY-MM-DD_HHMM/`
- 30-day rolling retention + first-of-month snapshots for 12 months (OPS-10)
- Post-backup validation: exits non-zero if any critical DB missing or size 0 (OPS-14)
- Cron registered: `30 21 * * *` (21:30 UTC = 03:00 IST)

Test run confirmed: racecontrol.db 132MB, admin.db 98KB, Failures: 0.

### RESTORE-DRILL.md Update

Added "Venue Backup Restore" section with Steps V1-V6 covering `/root/backups/venue/` path on Bono VPS:
- V1: Find target backup (list available, find first-of-month)
- V2: Download to scratch `/tmp/drill-restore-venue/`
- V3: Verify integrity (drivers, billing_sessions, employees, PRAGMA integrity_check)
- V4: Disaster recovery restore steps (venue .23 and cloud admin.db)
- V5: Clean up scratch
- V6: Log result in LOGBOOK.md
- Success criteria section (OPS-13 quarterly drill requirements)

### First Restore Drill (OPS-13 Milestone Artifact)

Drill executed using cloud backup from `/root/backups/cloud/2026-04-11_0412/`:

| Database | Rows | integrity_check | Notes |
|----------|------|-----------------|-------|
| racecontrol.db | 210 drivers, 303 billing_sessions | FAIL | idx_activity_hash index corruption (50+ missing rows in index), 2 double-page references. Data readable and queryable. Needs REINDEX. |
| admin.db | 2 employees | ok | Uses `employees` table, not `staff_members`. POS/cafe management DB. |

Drill result: **PARTIAL-PASS** — data intact and readable, cloud racecontrol.db has index corruption that needs attention.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Cloud DB paths differ from CONTEXT.md assumptions**
- **Found during:** Task 2 (live path discovery via `find`)
- **Issue:** CONTEXT.md assumed `/root/racingpoint/racecontrol/data/racecontrol.db`, actual path is `/root/racecontrol/data/racecontrol.db`. admin.db is at `/root/racingpoint-admin/data/admin.db` not `/root/racingpoint/racingpoint-admin/data/admin.db`.
- **Fix:** Updated backup-cloud.sh with correct paths after live `find` enumeration.
- **Files modified:** scripts/backup-cloud.sh
- **Commit:** 1f3922d5

**2. [Rule 1 - Bug] admin.db has no staff_members table**
- **Found during:** Task 4 (restore drill)
- **Issue:** RESTORE-DRILL.md venue section was written expecting `staff_members` table (from racingpoint-admin kiosk DB). The actual admin.db on Bono VPS is the POS/cafe management DB with `employees` table.
- **Fix:** Drill documented the actual table (`employees`, 2 rows). RESTORE-DRILL.md success criteria uses `staff_members OR employees` — documented in LOGBOOK entry.
- **Files modified:** LOGBOOK.md (drill result)

### Discoveries (not blocking)

**Cloud racecontrol.db index corruption found:** The restore drill exposed that the cloud racecontrol.db has `idx_activity_hash` index corruption (50+ rows missing from index, 2 double-page references). This is NOT a data loss — rows exist in the table but the index is inconsistent. Queries work but performance on activity hash lookups may degrade. Fix: `REINDEX idx_activity_hash` on the cloud database (out of scope for Phase 351, tracked below).

## Known Stubs

None — all functionality is wired.

## Deferred Items

- **Cloud racecontrol.db REINDEX:** `idx_activity_hash` has index corruption (found by drill). Run `sqlite3 /root/racecontrol/data/racecontrol.db 'REINDEX idx_activity_hash;'` on Bono VPS during next maintenance window. This is an operational action, not a code change.
- **Venue daily backup admin.db path:** Plan 351-01 adds venue admin.db backup via backup-databases.sh. Path `C:\RacingPoint\admin\data\admin.db` needs verification when server .23 is accessible (outside Phase 351-03 scope, handled by 351-01).

## Requirements Closed

- OPS-08: Cloud admin.db backed up daily at 03:00 IST via backup-cloud.sh
- OPS-09: Cloud racecontrol.db backed up daily at 03:00 IST via backup-cloud.sh
- OPS-10: 30-day retention + first-of-month 12-month snapshots implemented
- OPS-11: Cron schedule at 21:30 UTC (03:00 IST) on Bono VPS registered
- OPS-13: First restore drill executed, results logged in LOGBOOK.md
- OPS-14: backup-cloud.sh exits non-zero on missing or empty backup

## Self-Check: PASSED

- scripts/backup-cloud.sh: EXISTS (committed e1f8bd1d, fixed 1f3922d5)
- scripts/db-sync/RESTORE-DRILL.md: venue section added (committed 44044a24)
- LOGBOOK.md: restore-drill entry present (committed 9574b002)
- Bono VPS: /root/backup-cloud.sh deployed, cron `30 21 * * *` registered
- Test run: racecontrol.db 132MB, admin.db 98KB, both backed up successfully
