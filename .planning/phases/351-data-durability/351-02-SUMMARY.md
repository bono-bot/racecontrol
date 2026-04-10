---
phase: 351-data-durability
plan: 02
type: summary
commit: ead172a4
completed: "2026-04-11T04:25:00+05:30"
requirements_closed: [OPS-12, OPS-13]
---

## What Was Done

### OPS-12: WAL Mode Verification (Confirmed Already Implemented)

Verified that WAL mode is already implemented with fail-fast at startup:

- `crates/racecontrol/src/db/mod.rs:26-36`: `PRAGMA journal_mode=WAL` executed, then verified via
  `PRAGMA journal_mode` query. If result != "wal", `anyhow::bail!` fires at startup (RESIL-01, Phase 345).
- `crates/racecontrol/src/telemetry_store.rs`: `PRAGMA journal_mode=WAL` set (no bail!, but WAL is set).
- `racingpoint-admin/src/lib/db.ts:60`: `db.pragma('journal_mode = WAL')` on lazy-load.

No code changes required for OPS-12. Already complete from Phase 345.

### OPS-13: RESTORE-DRILL.md Updated + Drill Executed

**RESTORE-DRILL.md changes** (`scripts/db-sync/RESTORE-DRILL.md`):
- Purpose section updated to mention admin.db alongside racecontrol.db
- Step 2: admin.db download section (from `/root/racecontrol-backups/admin-*.db`)
- Step 3: admin.db integrity check (`SELECT COUNT(*) FROM employees; PRAGMA integrity_check;`)
  - NOTE: admin.db uses `employees` table (not `staff_members`) — verified 2026-04-11
- Step 4: admin.db disaster recovery restore commands
- Step 6: log format updated with Admin-Staff field
- Added "Venue Backup Restore" section (Step V1-V6) for restoring from daily backups
- Added "Success Criteria (OPS-13)" section

**Restore drill executed** 2026-04-11 04:21 IST on Bono VPS scratch path:
- racecontrol.db (from Google Drive): Drivers=210, Sessions=303, PRAGMA integrity_check=ok — PASS
- admin.db: NOT YET AVAILABLE — admin_db_path not yet configured in racecontrol.toml on server .23
  (pipeline installed, config update needed)
- Result: PASS (racecontrol.db clean; admin.db documented as pending)

LOGBOOK.md updated with drill result.

## Requirements Satisfied

| Req | Description | Status |
|-----|-------------|--------|
| OPS-12 | WAL mode verified at startup with fail-fast for racecontrol.db | CONFIRMED — Phase 345 code |
| OPS-13 | Restore drill SOP covers admin.db; drill executed; results logged | DONE |

## Follow-up Actions

- Server .23 racecontrol.toml needs `[backup] admin_db_path = "C:/RacingPoint/admin/data/admin.db"`
  for the Rust pipeline to start producing admin.db backups
- Next drill (first Monday of month) should show Admin-Staff count > 0 once configured
