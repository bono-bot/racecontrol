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
