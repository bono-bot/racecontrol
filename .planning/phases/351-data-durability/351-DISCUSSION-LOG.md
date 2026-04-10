# Phase 351: Data Durability - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-11
**Phase:** 351-data-durability
**Mode:** Auto (--auto flag — all decisions auto-selected)
**Areas discussed:** Venue backup approach, Admin.db backup scope, Retention policy, Remote transfer protocol, WAL verification, Alert mechanism, Restore drill scope, Schedule timing

---

## Venue Backup Script — Extend Rust vs New PS1

| Option | Description | Selected |
|--------|-------------|----------|
| Extend `backup_pipeline.rs` | Add admin.db + monthly tier + rsync to existing Rust pipeline | ✓ |
| New `backup-sqlite.ps1` | ROADMAP names this file, but creates a parallel backup system | |
| New `backup-sqlite.sh` (Linux cloud only) | Shell script for cloud side only, Rust handles venue | |

**Auto-selected:** Extend `backup_pipeline.rs` (recommended) — eliminates duplicate backup systems, leverages existing VACUUM INTO + alerting + rotation infrastructure.
**Notes:** ROADMAP names `backup-sqlite.ps1` for venue, but a separate PS1 script would create two competing backup systems. The Rust pipeline already handles everything Phase 351 needs; extension is safer and more maintainable. A new `backup-cloud.sh` is created for the cloud-only side (Bono VPS has no Rust binary running backup logic).

---

## Admin.db Backup Coverage

| Option | Description | Selected |
|--------|-------------|----------|
| Venue admin.db via Rust pipeline | Add `backup.admin_db_path` config, VACUUM INTO admin.db in backup_tick | ✓ |
| Cloud admin.db via backup-cloud.sh | Shell script on Bono VPS backs up cloud admin.db | ✓ |
| Skip admin.db (racecontrol.db only) | OPS-08 explicitly requires admin.db backup | |

**Auto-selected:** Both venue and cloud admin.db backed up (OPS-08 mandatory).
**Notes:** OPS-08 explicitly requires daily `sqlite3 .backup` of admin.db on both venue and cloud. Venue admin.db path needs researcher verification (likely `C:\RacingPoint\admin\data\admin.db`).

---

## Retention Policy (OPS-10)

| Option | Description | Selected |
|--------|-------------|----------|
| 30d daily + 4 weekly + 12 monthly | Full OPS-10 compliance — first-of-month snapshots for 12 months | ✓ |
| 30d daily only | Simpler but misses 12-month annual snapshot requirement | |
| Keep current (7d daily + 4 weekly) | Does not meet OPS-10 | |

**Auto-selected:** Three-tier retention: `daily_retain=30`, `weekly_retain=4`, `monthly_retain=12` (recommended — exact OPS-10 match).
**Notes:** Monthly snapshot triggered by `now_ist.day() == 1 && now_ist.hour() == 3`. File naming: `{prefix}-monthly-YYYY-MM.db`. Adds `monthly_retain` field to `BackupConfig`.

---

## Remote Transfer Protocol (OPS-11)

| Option | Description | Selected |
|--------|-------------|----------|
| Rsync (replace SCP) | ROADMAP-specified, built-in checksum, graceful partial transfers | ✓ |
| Keep existing SCP | Already works, SHA256 verified separately | |
| Both (rsync primary, SCP fallback) | Complex, two transfer paths to maintain | |

**Auto-selected:** Rsync (recommended — matches ROADMAP OPS-11 wording).
**Notes:** Rsync via Git Bash `rsync.exe` on Windows server. Fallback config flag `backup.use_rsync = false` for environments where rsync is unavailable. Researcher must verify `C:\Program Files\Git\usr\bin\rsync.exe` exists on server .23.

---

## WAL Mode Verification (OPS-12)

| Option | Description | Selected |
|--------|-------------|----------|
| Already implemented — document only | `db/mod.rs:22-37` + `db.ts:60` both enforce WAL | ✓ |
| Add explicit startup check | Already exists in both places | |
| Add WAL verification to backup_pipeline.rs | Redundant — db init already verifies | |

**Auto-selected:** Document as satisfied — no new code needed (recommended).
**Notes:** `racecontrol.db` uses `bail!` (hard fail at startup) if WAL not active (RESIL-01). `admin.db` sets WAL pragma on first lazy-load. OPS-12 is fully covered by Phase 345 code.

---

## Backup Alert Mechanism (OPS-14)

| Option | Description | Selected |
|--------|-------------|----------|
| Extend check_staleness() with size-check | Reuses existing WhatsApp alerter, immediate alert on zero-byte | ✓ |
| New dedicated OPS-14 check function | Separate function, more explicit but duplicates alerter logic | |
| Subsystem health probe | Belongs in Phase 352 (Health + Alerts), not Phase 351 | |

**Auto-selected:** Extend `check_staleness()` with size-check (recommended — minimal code, existing alerter).
**Notes:** Zero-byte backup bypasses normal debounce — always fires immediately. Cloud script exits non-zero on zero-byte file; cron mail handles alerting on cloud side.

---

## Restore Drill Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Extend existing RESTORE-DRILL.md | Add venue backup section to Phase 349's cloud restore doc | ✓ |
| New separate restore-drill-venue.md | Two drill docs to maintain | |
| Inline in PLAN 351-03 | No persistent SOP document | |

**Auto-selected:** Extend `scripts/db-sync/RESTORE-DRILL.md` (recommended — one doc for all restore paths).
**Notes:** Phase 349 already created this file for cloud Google Drive restore. Adding venue backup restore section (from Bono VPS `/root/backups/venue/`) creates a unified restore reference. First execution is the Plan 351-03 milestone artifact.

---

## Schedule Timing

| Option | Description | Selected |
|--------|-------------|----------|
| 03:00 IST daily | OPS-08 + OPS-09 requirement — low-traffic window | ✓ |
| 02:00 IST | Overlaps with existing nightly SCP window (02:00-04:00) — fine but 03:00 cleaner | |
| Multiple times per day | Over-engineering for daily backup requirement | |

**Auto-selected:** 03:00 IST daily (recommended — exact REQUIREMENTS match).
**Notes:** Venue: IST-hour gate `now_ist.hour() == 3` in backup_tick. Cloud cron: `30 21 * * *` (21:30 UTC = 03:00 IST).

---

## Claude's Discretion

- Exact venue admin.db path (researcher must verify)
- Telemetry.db retention tier upgrade (researcher should check size growth rate)
- Rsync availability on server .23 (researcher must verify)

## Deferred Ideas

- Admin dashboard backup panel — deferred to Phase 354
- Automated restore drill — manual drill is sufficient
- Telemetry.db 30-day retention — deferred pending size analysis
- End-of-day rsync verification — rsync --checksum covers in-transfer integrity
