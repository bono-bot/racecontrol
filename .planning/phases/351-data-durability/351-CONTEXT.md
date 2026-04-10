# Phase 351: Data Durability - Context

**Gathered:** 2026-04-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Close the remaining gaps between the existing backup infrastructure (Phase 300 backup_pipeline.rs + backup-databases.sh) and the v47.0 OPS-08..14 requirements. Most requirements are already implemented — this phase is a hardening/gap-close, not a greenfield build.

</domain>

<decisions>
## Implementation Decisions

### D-01: This is a gap-close phase, not a new build

The codebase already has:
- **Rust backup pipeline** (`backup_pipeline.rs`): hourly VACUUM INTO for racecontrol.db + telemetry.db, 7d+4w rotation, staleness WhatsApp alert (2h), nightly SCP to Bono VPS with SHA256 verification
- **Shell backup script** (`scripts/backup-databases.sh`): daily backup of venue racecontrol.db (via SSH to .23), James local DBs (faces.db, people_tracker.db), VPS DBs (bot.sqlite, racecontrol-cloud.db), configs (racecontrol.toml, comms-link.env), 30-day pruning
- **Schtask registration** (`scripts/register-backup-task.bat`): registers daily 03:00 task
- **WAL mode** (`db/mod.rs:26-37`): set at startup with fail-fast verification
- **Restore drill SOP** (`scripts/db-sync/RESTORE-DRILL.md`): 6-step procedure with sentinel-based pause

### D-02: Known gaps to close (from live audit 2026-04-11)

1. **Venue racecontrol.db backup FAILING silently** — `backup-databases.sh` step [1/4] produces "FAIL: racecontrol.db — server unreachable or backup failed" but the script continues with exit 0. Today's backup has cloud DBs but NO venue DB. This is the highest-priority gap.
2. **No scheduled task registered** — `schtasks.exe /Query` shows no "DatabaseBackup" task on James. Backups ARE running daily at 03:00 (12 days of evidence), suggesting some other mechanism (possibly Task Scheduler GUI entry with different name, or rc-watchdog cron). Need to identify the actual trigger and ensure it's durable.
3. **Missing first-of-month 12-month retention** — OPS-10 requires "first-of-month snapshots retained for 12 months." Current `backup-databases.sh` only does 30-day rolling prune. Need to exempt `*-01_*` directories from pruning.
4. **admin.db not backed up** — OPS-08 specifies "admin.db on venue and cloud." The shell script only backs up racecontrol.db from venue. admin.db (better-sqlite3, Next.js admin portal) is not included.
5. **No automated post-backup validation** — OPS-14 says "alert fires if backup missing or size 0." The Rust pipeline has staleness alerts but `backup-databases.sh` has no post-run check or alert. A backup that silently fails (like venue racecontrol.db) generates no notification.

### D-03: Fix strategy — patch existing scripts, don't rewrite

- Fix `backup-databases.sh` to handle the venue SSH failure (diagnose root cause: SSH key? sqlite3 not in PATH on server? Tailscale down at 03:00?)
- Add admin.db backup step to the shell script
- Add first-of-month retention logic to the pruning section
- Add post-backup validation with alert (reuse comms-link relay `POST /relay/alert`)
- Register the schtask properly (or verify the existing trigger mechanism)

### D-04: Restore drill — already documented, needs first v47.0 execution

The `RESTORE-DRILL.md` exists and covers the Google Drive sync path. The v47.0 success criterion says "Restore drill on a scratch machine recovers admin.db with matching row counts." This means the drill needs to also cover admin.db (not just racecontrol.db) and should be executed once with results logged.

### Claude's Discretion

- Exact alert message format for post-backup validation
- Whether to add admin.db backup to the Rust pipeline or just the shell script (shell script is simpler for a Next.js SQLite DB)
- Diagnostic approach for the venue racecontrol.db SSH failure

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Backup Infrastructure (existing)
- `crates/racecontrol/src/backup_pipeline.rs` — Rust hourly backup pipeline (VACUUM INTO, SCP, staleness alerts)
- `crates/racecontrol/src/config.rs:1090-1145` — BackupConfig struct with all defaults
- `scripts/backup-databases.sh` — Shell daily backup script (venue + James + VPS + configs)
- `scripts/register-backup-task.bat` — Schtask registration for daily 03:00

### Database initialization
- `crates/racecontrol/src/db/mod.rs:22-37` — WAL mode setup + verification at startup

### Restore drill
- `scripts/db-sync/RESTORE-DRILL.md` — 6-step restore procedure (Phase 349 SYNC-07)
- `scripts/db-sync/download-db.sh` — Google Drive download script used in drill

### Requirements
- `.planning/REQUIREMENTS.md:81-87` — OPS-08 through OPS-14

### Alerting (for post-backup validation)
- `crates/racecontrol/src/backup_pipeline.rs` — existing WhatsApp alert via staleness check pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- **backup_pipeline.rs**: Full Rust backup pipeline with VACUUM INTO, rotation, SCP, SHA256 verification, WhatsApp staleness alerts. Can be extended for admin.db.
- **backup-databases.sh**: Working daily script with multi-target SSH/SCP pattern. Needs patching, not rewriting.
- **register-backup-task.bat**: Ready to register schtask, just needs to be run as admin.
- **comms-link /relay/alert**: Existing alert endpoint for WhatsApp notifications.

### Established Patterns
- **VACUUM INTO** (not file copy) for WAL-safe SQLite backups — locked decision from Phase 300
- **SCP with `StrictHostKeyChecking=no BatchMode=yes`** for remote transfers — standing rule
- **Staleness-based alerting** with debounce — pattern in backup_pipeline.rs

### Integration Points
- `backup-databases.sh` runs from James .27, SSHs into server .23 and Bono VPS
- Rust backup pipeline runs inside racecontrol binary on server .23
- Post-backup alerts should use comms-link relay on James .27 (port 8766)

</code_context>

<specifics>
## Specific Ideas

- Venue racecontrol.db SSH backup failure is the P0 — a backup system that doesn't back up the primary database is worse than useless (false confidence)
- admin.db path on server .23: likely `C:\RacingPoint\racingpoint-admin\admin.db` or similar — needs verification
- The 12-month first-of-month retention is a simple `if [[ $(basename $dir) == *"-01_"* ]]` guard in the prune loop

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 351-data-durability*
*Context gathered: 2026-04-11*
