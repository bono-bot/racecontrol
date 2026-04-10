#!/usr/bin/env bash
# backup-databases.sh — Daily SQLite backup for disaster recovery
#
# Backs up racecontrol.db + admin.db (venue + cloud) to a timestamped directory.
# Run daily at 03:00 IST via scheduled task or cron.
# OPS-08: daily sqlite3 .backup of racecontrol.db and admin.db
# OPS-09: 30-day rolling retention
# OPS-10: first-of-month backups retained 12 months
# OPS-11: rsync to Bono VPS /root/backups/venue/ with SCP fallback
# OPS-14: WhatsApp alert if backup is missing or size 0 after scheduled window
#
# Usage:
#   bash scripts/backup-databases.sh              # backup to default dir
#   bash scripts/backup-databases.sh /path/to/dir # backup to custom dir
#
# Register as scheduled task:
#   schtasks /Create /SC DAILY /ST 03:00 /TN "DatabaseBackup" /TR "bash C:\Users\bono\racingpoint\racecontrol\scripts\backup-databases.sh" /RU bono

set -e

BACKUP_ROOT="${1:-C:/Users/bono/racingpoint/backups}"
TIMESTAMP=$(date '+%Y-%m-%d_%H%M')
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
MAX_BACKUPS=30  # OPS-09: Keep last 30 days
MONTHLY_RETAIN=12  # OPS-10: Keep first-of-month snapshots for 12 months
FAILURES=0

# Detect if today is the 1st of the month (for OPS-10 monthly snapshot)
DAY_OF_MONTH=$(date '+%d')
YEAR_MONTH=$(date '+%Y-%m')
IS_FIRST_OF_MONTH=false
if [ "$DAY_OF_MONTH" = "01" ]; then
  IS_FIRST_OF_MONTH=true
fi

echo "============================================================"
echo "DATABASE BACKUP — $TIMESTAMP"
echo "============================================================"

mkdir -p "$BACKUP_DIR"

# ─── Venue databases (server .23 via SSH) ─────────────────────────────
echo "[1/5] Backing up venue databases from server .23..."
SERVER_SSH="ssh -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no ADMIN@100.125.108.37"

# racecontrol.db — main venue database (OPS-08)
$SERVER_SSH "sqlite3 C:\\RacingPoint\\racecontrol.db '.backup C:\\RacingPoint\\racecontrol-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
    ADMIN@100.125.108.37:C:/RacingPoint/racecontrol-backup.db \
    "$BACKUP_DIR/racecontrol.db" 2>/dev/null && \
  echo "  OK: racecontrol.db ($(wc -c < "$BACKUP_DIR/racecontrol.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  FAIL: racecontrol.db — server unreachable or backup failed"; FAILURES=$((FAILURES + 1)); }

# admin.db — venue admin database (OPS-08)
$SERVER_SSH "sqlite3 C:\\RacingPoint\\admin\\data\\admin.db '.backup C:\\RacingPoint\\admin-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
    ADMIN@100.125.108.37:C:/RacingPoint/admin-backup.db \
    "$BACKUP_DIR/admin.db" 2>/dev/null && \
  echo "  OK: admin.db ($(wc -c < "$BACKUP_DIR/admin.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  WARN: admin.db — venue admin db not found or backup failed (non-fatal)"; }

# ─── James databases ──────────────────────────────────────────────────
echo "[2/5] Backing up James local databases..."

# faces.db (rc-sentry-ai face gallery)
FACES_DB="C:/RacingPoint/data/faces.db"
if [ -f "$FACES_DB" ]; then
  sqlite3 "$FACES_DB" ".backup $BACKUP_DIR/faces.db" 2>/dev/null && \
    echo "  OK: faces.db" || echo "  FAIL: faces.db"
else
  echo "  SKIP: faces.db not found"
fi

# people_tracker.db
PT_DB="C:/Users/bono/racingpoint/people-tracker/data/people_tracker.db"
if [ -f "$PT_DB" ]; then
  sqlite3 "$PT_DB" ".backup $BACKUP_DIR/people_tracker.db" 2>/dev/null && \
    echo "  OK: people_tracker.db" || echo "  FAIL: people_tracker.db"
else
  echo "  SKIP: people_tracker.db not found"
fi

# ─── VPS databases (Bono) ────────────────────────────────────────────
echo "[3/5] Backing up VPS databases from Bono..."
BONO_SSH="ssh -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no root@100.70.177.44"

# WhatsApp bot database
$BONO_SSH "sqlite3 /root/racingpoint/whatsapp-bot/data/bot.sqlite '.backup /tmp/bot-backup.sqlite'" 2>/dev/null && \
  scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
    root@100.70.177.44:/tmp/bot-backup.sqlite \
    "$BACKUP_DIR/bot.sqlite" 2>/dev/null && \
  echo "  OK: bot.sqlite" || echo "  FAIL: bot.sqlite"

# Cloud racecontrol.db (OPS-08 cloud coverage)
$BONO_SSH "sqlite3 /root/racecontrol/data/racecontrol.db '.backup /tmp/rc-cloud-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
    root@100.70.177.44:/tmp/rc-cloud-backup.db \
    "$BACKUP_DIR/racecontrol-cloud.db" 2>/dev/null && \
  echo "  OK: racecontrol-cloud.db" || \
  { echo "  FAIL: racecontrol-cloud.db"; FAILURES=$((FAILURES + 1)); }

# Cloud admin.db (OPS-08 cloud coverage)
$BONO_SSH "sqlite3 /root/racingpoint-admin/data/admin.db '.backup /tmp/admin-cloud-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
    root@100.70.177.44:/tmp/admin-cloud-backup.db \
    "$BACKUP_DIR/admin-cloud.db" 2>/dev/null && \
  echo "  OK: admin-cloud.db ($(wc -c < "$BACKUP_DIR/admin-cloud.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  FAIL: admin-cloud.db"; FAILURES=$((FAILURES + 1)); }

# ─── Config backups ──────────────────────────────────────────────────
echo "[4/5] Backing up critical configs..."
mkdir -p "$BACKUP_DIR/configs"

scp -o ConnectTimeout=5 -o BatchMode=yes -o StrictHostKeyChecking=no \
  ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.toml \
  "$BACKUP_DIR/configs/racecontrol-server.toml" 2>/dev/null && \
  echo "  OK: server racecontrol.toml" || echo "  FAIL: server config"

cp C:/Users/bono/racingpoint/comms-link/.env "$BACKUP_DIR/configs/comms-link.env" 2>/dev/null && \
  echo "  OK: comms-link .env" || echo "  SKIP: comms-link .env"

# ─── OPS-14: Post-backup validation — alert on missing or zero-byte ───
echo "[5/5] Validating critical backups (OPS-14)..."
CRITICAL_DBS=("racecontrol.db" "racecontrol-cloud.db" "admin-cloud.db")
ALERT_MSG=""
for db in "${CRITICAL_DBS[@]}"; do
  db_path="$BACKUP_DIR/$db"
  if [ ! -f "$db_path" ]; then
    ALERT_MSG="${ALERT_MSG}MISSING: $db. "
    echo "  WARN: $db not found"
  elif [ ! -s "$db_path" ]; then
    ALERT_MSG="${ALERT_MSG}ZERO-BYTE: $db. "
    echo "  WARN: $db is zero bytes"
    FAILURES=$((FAILURES + 1))
  else
    echo "  OK: $db present and non-empty"
  fi
done
if [ -n "$ALERT_MSG" ]; then
  echo "  Sending OPS-14 alert: $ALERT_MSG"
  curl -s -X POST http://localhost:8766/relay/alert \
    -H "Content-Type: application/json" \
    -d "{\"message\": \"Backup alert ($TIMESTAMP): $ALERT_MSG\", \"channel\": \"ops\"}" \
    2>/dev/null || echo "  WARN: relay alert failed (relay may be down)"
fi

# ─── OPS-11: Rsync to Bono VPS ───────────────────────────────────────
echo "Transferring backups to Bono VPS via rsync (OPS-11)..."
RSYNC_BIN="C:/Program Files/Git/usr/bin/rsync.exe"
REMOTE_DIR="root@100.70.177.44:/root/backups/venue/${TIMESTAMP}/"

# Create the remote target directory first
$BONO_SSH "mkdir -p /root/backups/venue/${TIMESTAMP}" 2>/dev/null

if [ -f "$RSYNC_BIN" ]; then
  "$RSYNC_BIN" -az --checksum --no-perms --timeout=60 \
    -e "ssh -o StrictHostKeyChecking=no -o BatchMode=yes" \
    "$BACKUP_DIR/" \
    "$REMOTE_DIR" 2>/dev/null && \
    echo "  OK: rsync to $REMOTE_DIR" || \
    { echo "  WARN: rsync failed, falling back to SCP...";
      scp -r -o ConnectTimeout=10 -o BatchMode=yes -o StrictHostKeyChecking=no \
        "$BACKUP_DIR" root@100.70.177.44:/root/backups/venue/ 2>/dev/null && \
        echo "  OK: SCP fallback succeeded" || \
        { echo "  FAIL: remote transfer failed (rsync + SCP both failed)"; FAILURES=$((FAILURES + 1)); }
    }
else
  echo "  INFO: rsync.exe not found at expected path, using SCP (set backup.use_rsync=false to suppress)"
  scp -r -o ConnectTimeout=10 -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$BACKUP_DIR" root@100.70.177.44:/root/backups/venue/ 2>/dev/null && \
    echo "  OK: SCP to /root/backups/venue/" || \
    { echo "  FAIL: SCP transfer failed"; FAILURES=$((FAILURES + 1)); }
fi

# ─── OPS-10: First-of-month snapshot — copy to monthly dir ──────────
if [ "$IS_FIRST_OF_MONTH" = "true" ]; then
  MONTHLY_DIR="${BACKUP_ROOT}/monthly/${YEAR_MONTH}"
  echo "First of month — creating monthly snapshot: $MONTHLY_DIR (OPS-10)"
  mkdir -p "$MONTHLY_DIR"
  cp -r "$BACKUP_DIR/." "$MONTHLY_DIR/"
  echo "  OK: Monthly snapshot for $YEAR_MONTH"

  # Also copy monthly snapshot to Bono VPS
  $BONO_SSH "mkdir -p /root/backups/venue/monthly/${YEAR_MONTH}" 2>/dev/null
  scp -r -o ConnectTimeout=10 -o BatchMode=yes -o StrictHostKeyChecking=no \
    "$MONTHLY_DIR" root@100.70.177.44:/root/backups/venue/monthly/ 2>/dev/null && \
    echo "  OK: Monthly snapshot synced to VPS" || \
    echo "  WARN: Monthly snapshot VPS sync failed (local copy retained)"

  # Prune monthly snapshots older than 12 months
  MONTHLY_COUNT=$(ls -d "${BACKUP_ROOT}/monthly"/20* 2>/dev/null | wc -l)
  if [ "$MONTHLY_COUNT" -gt "$MONTHLY_RETAIN" ]; then
    PRUNE_MONTHLY=$((MONTHLY_COUNT - MONTHLY_RETAIN))
    echo "Pruning $PRUNE_MONTHLY old monthly snapshot(s) (keeping last $MONTHLY_RETAIN months)..."
    ls -d "${BACKUP_ROOT}/monthly"/20* | head -$PRUNE_MONTHLY | while read dir; do
      rm -rf "$dir"
      echo "  Removed monthly: $(basename $dir)"
    done
  fi
fi

# ─── OPS-09: Prune daily backups — keep last 30, exempt first-of-month ──
BACKUP_COUNT=$(ls -d "$BACKUP_ROOT"/20*_* 2>/dev/null | wc -l)
if [ "$BACKUP_COUNT" -gt "$MAX_BACKUPS" ]; then
  PRUNE_COUNT=$((BACKUP_COUNT - MAX_BACKUPS))
  echo ""
  echo "Pruning $PRUNE_COUNT old daily backup(s) (keeping last $MAX_BACKUPS)..."
  ls -d "$BACKUP_ROOT"/20*_* | head -$PRUNE_COUNT | while read dir; do
    # Exempt first-of-month daily backups (YYYY-MM-01_*) from daily pruning
    dirname=$(basename "$dir")
    if echo "$dirname" | grep -qE '^[0-9]{4}-[0-9]{2}-01_'; then
      echo "  SKIP (monthly exempt): $dirname"
    else
      rm -rf "$dir"
      echo "  Removed: $dirname"
    fi
  done
fi

# ─── Summary ──────────────────────────────────────────────────────────
echo ""
echo "============================================================"
TOTAL_SIZE=$(du -sh "$BACKUP_DIR" 2>/dev/null | awk '{print $1}')
FILE_COUNT=$(find "$BACKUP_DIR" -type f | wc -l)
echo "Backup complete: $FILE_COUNT files, $TOTAL_SIZE total"
echo "Location: $BACKUP_DIR"
if [ "$FAILURES" -gt 0 ]; then
  echo "FAILURES: $FAILURES critical backup(s) failed — check logs"
fi
echo "============================================================"

exit $FAILURES
