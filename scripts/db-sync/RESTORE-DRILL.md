# RESTORE-DRILL — Monthly DB Restore Verification Procedure

Phase 349, SYNC-07. Run this drill on the first Monday of each month (or during disaster recovery).

---

## Purpose

Verify that the Google Drive backup can restore a working `racecontrol.db` on the cloud server.
This drill exercises the full backup-restore path without touching the live production database.

---

## When to Run

- **Routine drill:** First Monday of each month
- **Disaster recovery:** Any time the cloud `racecontrol.db` is corrupt, missing, or stale

---

## Pre-requisites

- SSH access to Bono VPS (`ssh bono-vps` or `ssh root@100.70.177.44`)
- `db-sync.env` credentials in place at `/root/racingpoint/racecontrol/scripts/db-sync/db-sync.env`
- `jq` and `sqlite3` installed on Bono VPS

---

## Step 1: Pause Replication

Prevent the 5-minute cron from overwriting files during the drill.

```bash
ssh bono-vps "touch /tmp/DB_SYNC_PAUSED"
```

Verify it works (wait up to 5 minutes for the next cron tick, or trigger manually):

```bash
ssh bono-vps "bash /root/racingpoint/racecontrol/scripts/db-sync/download-db.sh 2>&1 | tail -5"
# Expected output: "=== SYNC PAUSED (rm /tmp/DB_SYNC_PAUSED to resume) ==="
```

---

## Step 2: Download to Scratch Path

Download the Google Drive backup to an isolated temp directory (does NOT touch the live database):

```bash
ssh bono-vps "RC_DATA_DIR=/tmp/drill-restore mkdir -p /tmp/drill-restore && bash /root/racingpoint/racecontrol/scripts/db-sync/download-db.sh"
```

Check the sync-status:

```bash
ssh bono-vps "cat /tmp/drill-restore/sync-status.json"
# Expected: {"status":"ok","message":"Download complete",...}
```

---

## Step 3: Verify Integrity

Run SQLite integrity checks on the downloaded file:

```bash
ssh bono-vps "sqlite3 /tmp/drill-restore/racecontrol.db 'SELECT COUNT(*) FROM drivers; SELECT COUNT(*) FROM billing_sessions; PRAGMA integrity_check;'"
```

Expected output:
- First line: driver count (should be > 0 for an active venue)
- Second line: session count (may be 0 for a fresh or quiet day)
- Third line: `ok` (integrity check must pass)

If `integrity_check` returns anything other than `ok`, the backup is corrupt — escalate to Uday immediately.

---

## Step 4: Production Restore (Disaster Recovery Only)

**Skip this step during routine drills.** Only run this if the live database is corrupt or missing.

```bash
# 1. Stop racecontrol on cloud
ssh bono-vps "pm2 stop racecontrol"

# 2. Backup the current (possibly corrupt) database
ssh bono-vps "cp /root/racingpoint/racecontrol/data/racecontrol.db /root/racingpoint/racecontrol/data/racecontrol.db.corrupt-$(date +%Y%m%d%H%M%S)"

# 3. Copy the verified drill restore to production
ssh bono-vps "cp /tmp/drill-restore/racecontrol.db /root/racingpoint/racecontrol/data/racecontrol.db"

# 4. Restart racecontrol
ssh bono-vps "pm2 start racecontrol"

# 5. Verify health
sleep 5 && curl -s https://admin.racingpoint.cloud/api/v1/health | jq '.status'
# Expected: "ok"
```

---

## Step 5: Resume Replication

Remove the sentinel file so the 5-minute cron resumes:

```bash
ssh bono-vps "rm /tmp/DB_SYNC_PAUSED"
```

Verify the next download cycle succeeds (wait up to 5 minutes):

```bash
ssh bono-vps "cat /root/racingpoint/racecontrol/data/sync-status.json"
# Expected: {"status":"ok",...} with a fresh timestamp
```

---

## Step 6: Log Result

Append the drill outcome to LOGBOOK.md in this repo:

```
| YYYY-MM-DD HH:MM IST | James | restore-drill | [PASS/FAIL] Drivers: N, Sessions: M, integrity_check: ok/FAIL | notes |
```

If any step failed, document what failed and open a task to fix it before the next drill.

---

## Sentinel Reference

| File | Effect |
|------|--------|
| `/tmp/DB_SYNC_PAUSED` | Pauses `download-db.sh` before downloading. Created by `touch`, removed by `rm`. |
| `/tmp/drill-restore/` | Scratch directory for drill downloads. Safe to delete after drill. |

The `DB_SYNC_PAUSED` sentinel is checked by `download-db.sh` on every run (SYNC-08).
The `db_sync_lag` health probe in `/api/v1/health` will show `DB_SYNC_LAG_WARN` after 5 minutes
and `DB_SYNC_LAG_CRITICAL` after 15 minutes of paused replication — this is expected during the drill.
Resume replication (Step 5) before the 15-minute window to avoid a WhatsApp alert.

---

## Venue Backup Restore (from daily backup)

Phase 351, OPS-13. This section covers restoring venue databases from the daily backup stored on
Bono VPS at `/root/backups/venue/`. This is DISTINCT from the Google Drive restore above (Phase 349).

Use this path when:
- You need to roll back to a specific day's snapshot (not just "most recent")
- The Google Drive sync is unavailable or stale
- Restoring a specific DB version from up to 30 days ago (or first-of-month for up to 12 months)

### Backup Location

Daily backups are stored on Bono VPS at:

```
/root/backups/venue/YYYY-MM-DD_HHMM/
  racecontrol.db         -- venue racecontrol database
  admin.db               -- venue admin portal database
  racecontrol-cloud.db   -- cloud racecontrol database
  admin-cloud.db         -- cloud admin database (when available)
  configs/               -- server config backups
```

First-of-month snapshots are retained for 12 months.

### Step V1: Find the Target Backup

```bash
# List available venue backups (most recent first)
ssh bono-vps "ls -td /root/backups/venue/*/ 2>/dev/null | head -20"

# Find the most recent backup containing racecontrol.db
ssh bono-vps "ls -td /root/backups/venue/*/racecontrol.db 2>/dev/null | head -5"

# Find first-of-month snapshots (12-month retention)
ssh bono-vps "ls -d /root/backups/venue/*-01_*/ 2>/dev/null"
```

### Step V2: Download to Scratch Path

```bash
# Set target date (YYYY-MM-DD_HHMM format matching the backup directory name)
TARGET_BACKUP="2026-04-01_0300"  # Replace with actual target

# Create scratch directory
ssh bono-vps "mkdir -p /tmp/drill-restore-venue"

# Copy target backup to scratch
ssh bono-vps "cp /root/backups/venue/${TARGET_BACKUP}/racecontrol.db /tmp/drill-restore-venue/racecontrol.db"
ssh bono-vps "cp /root/backups/venue/${TARGET_BACKUP}/admin.db /tmp/drill-restore-venue/admin.db 2>/dev/null || echo 'admin.db not in this backup'"
```

### Step V3: Verify Integrity

```bash
# Verify racecontrol.db
ssh bono-vps "sqlite3 /tmp/drill-restore-venue/racecontrol.db 'SELECT COUNT(*) FROM drivers; SELECT COUNT(*) FROM billing_sessions; PRAGMA integrity_check;'"

# Verify admin.db (if available)
ssh bono-vps "sqlite3 /tmp/drill-restore-venue/admin.db 'SELECT COUNT(*) FROM staff_members; PRAGMA integrity_check;' 2>/dev/null || echo 'admin.db not available in this backup'"
```

Expected:
- `drivers` count: > 0 for an active venue
- `billing_sessions` count: >= 0
- `staff_members` count: > 0 (for admin.db)
- `integrity_check`: `ok` (must pass — any other output means corrupt backup, escalate to Uday)

### Step V4: Disaster Recovery (Production Restore)

**Skip during routine drills. Only run when live DB is corrupt or missing.**

**For venue racecontrol.db (restore to server .23):**

```bash
# 1. Download from Bono VPS to James .27 staging
scp bono-vps:/tmp/drill-restore-venue/racecontrol.db /tmp/racecontrol-restore.db

# 2. Upload to server .23 (while racecontrol is still running -- safe to stage)
scp /tmp/racecontrol-restore.db ADMIN@100.125.108.37:C:/RacingPoint/racecontrol-restore.db

# 3. On venue server: stop racecontrol, swap DB, restart
ssh ADMIN@100.125.108.37 "taskkill /F /IM racecontrol.exe & ping -n 4 127.0.0.1 >nul & ren C:\\RacingPoint\\racecontrol.db racecontrol.db.corrupt & ren C:\\RacingPoint\\racecontrol-restore.db racecontrol.db"
ssh ADMIN@100.125.108.37 "schtasks /Run /TN StartRCDirect"

# 4. Verify health
sleep 10 && curl -s http://192.168.31.23:8080/api/v1/health | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('status','?'))"
```

**For cloud admin.db (restore on Bono VPS):**

```bash
# 1. Stop cloud services
ssh bono-vps "pm2 stop racecontrol 2>/dev/null; true"

# 2. Backup the current (possibly corrupt) file
ssh bono-vps "cp /root/racingpoint-admin/data/admin.db /root/racingpoint-admin/data/admin.db.corrupt-\$(date +%Y%m%d%H%M%S)"

# 3. Copy verified backup to production path
ssh bono-vps "cp /tmp/drill-restore-venue/admin.db /root/racingpoint-admin/data/admin.db"

# 4. Restart
ssh bono-vps "pm2 start racecontrol"
```

### Step V5: Clean Up

```bash
ssh bono-vps "rm -rf /tmp/drill-restore-venue"
```

### Step V6: Log Result

Append the drill outcome to LOGBOOK.md:

```
| YYYY-MM-DD HH:MM IST | James | restore-drill-venue | [PASS/FAIL] RC-Drivers: N, RC-Sessions: M, Admin-Staff: K or N/A, integrity_check: ok/FAIL | Phase 351 OPS-13 quarterly drill |
```

Use `bash scripts/ist-now.sh` for IST timestamp (never `TZ=Asia/Kolkata date` — silently returns UTC on Windows).

---

## Success Criteria (OPS-13)

A restore drill PASSes when:
1. racecontrol.db: `PRAGMA integrity_check` returns `ok`
2. racecontrol.db: `drivers` table has > 0 rows
3. admin.db: `PRAGMA integrity_check` returns `ok` (or documented as not-yet-available)
4. admin.db: `staff_members` table has > 0 rows (or documented as not-yet-available)
5. Row counts match production within 24h tolerance (backup is daily)
