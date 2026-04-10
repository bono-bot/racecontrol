#!/usr/bin/env bash
# backup-databases.sh — P0-SEC: Database backup for disaster recovery
#
# Backs up all SQLite databases to a timestamped directory.
# Run daily via scheduled task or cron.
#
# Usage:
#   bash scripts/backup-databases.sh              # backup to default dir
#   bash scripts/backup-databases.sh /path/to/dir # backup to custom dir
#
# Register as scheduled task:
#   schtasks /Create /SC DAILY /ST 03:00 /TN "DatabaseBackup" /TR "\"C:\Program Files\Git\bin\bash.exe\" \"C:\Users\bono\racingpoint\racecontrol\scripts\backup-databases.sh\"" /RU bono /F

# Track failures — non-zero exit lets schtask report failure (OPS-09)
FAILURES=0

BACKUP_ROOT="${1:-C:/Users/bono/racingpoint/backups}"
TIMESTAMP=$(date '+%Y-%m-%d_%H%M')
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
MAX_BACKUPS=30  # Keep last 30 days

echo "============================================================"
echo "DATABASE BACKUP — $TIMESTAMP"
echo "============================================================"

mkdir -p "$BACKUP_DIR"

# ─── Venue databases (server .23 via SSH) ─────────────────────────────
echo "[1/4] Backing up venue databases from server .23..."
SERVER_SSH="ssh -o ConnectTimeout=5 ADMIN@100.125.108.37"

# racecontrol.db — the main venue database (OPS-09)
$SERVER_SSH "sqlite3 C:\\RacingPoint\\racecontrol.db '.backup C:\\RacingPoint\\racecontrol-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 ADMIN@100.125.108.37:C:/RacingPoint/racecontrol-backup.db "$BACKUP_DIR/racecontrol.db" 2>/dev/null && \
  echo "  OK: racecontrol.db ($(wc -c < "$BACKUP_DIR/racecontrol.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  FAIL: racecontrol.db — server unreachable or backup failed"; FAILURES=$((FAILURES + 1)); }

# admin.db — Next.js admin portal database (OPS-08)
# Verified path: C:\RacingPoint\racingpoint-admin\data\admin.db (from admin-deploy.sh)
# If server unreachable, this increments FAILURES non-fatally
$SERVER_SSH "sqlite3 C:\\RacingPoint\\racingpoint-admin\\data\\admin.db '.backup C:\\RacingPoint\\admin-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 ADMIN@100.125.108.37:C:/RacingPoint/admin-backup.db "$BACKUP_DIR/admin.db" 2>/dev/null && \
  echo "  OK: admin.db ($(wc -c < "$BACKUP_DIR/admin.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  FAIL: admin.db — server unreachable or backup failed"; FAILURES=$((FAILURES + 1)); }

# ─── James databases ──────────────────────────────────────────────────
echo "[2/4] Backing up James local databases..."

# faces.db (rc-sentry-ai face gallery)
FACES_DB="C:/RacingPoint/data/faces.db"
if [ -f "$FACES_DB" ]; then
  sqlite3 "$FACES_DB" ".backup $BACKUP_DIR/faces.db" 2>/dev/null && \
    echo "  OK: faces.db" || { echo "  FAIL: faces.db"; FAILURES=$((FAILURES + 1)); }
else
  echo "  SKIP: faces.db not found"
fi

# people_tracker.db
PT_DB="C:/Users/bono/racingpoint/people-tracker/data/people_tracker.db"
if [ -f "$PT_DB" ]; then
  sqlite3 "$PT_DB" ".backup $BACKUP_DIR/people_tracker.db" 2>/dev/null && \
    echo "  OK: people_tracker.db" || { echo "  FAIL: people_tracker.db"; FAILURES=$((FAILURES + 1)); }
else
  echo "  SKIP: people_tracker.db not found"
fi

# ─── VPS databases (Bono) ────────────────────────────────────────────
echo "[3/4] Backing up VPS databases from Bono..."
BONO_SSH="ssh -o ConnectTimeout=5 root@100.70.177.44"

# WhatsApp bot database
$BONO_SSH "sqlite3 /root/racingpoint/whatsapp-bot/data/bot.sqlite '.backup /tmp/bot-backup.sqlite'" 2>/dev/null && \
  scp -o ConnectTimeout=5 root@100.70.177.44:/tmp/bot-backup.sqlite "$BACKUP_DIR/bot.sqlite" 2>/dev/null && \
  echo "  OK: bot.sqlite" || { echo "  FAIL: bot.sqlite"; FAILURES=$((FAILURES + 1)); }

# Cloud racecontrol.db
$BONO_SSH "sqlite3 /root/racecontrol/data/racecontrol.db '.backup /tmp/rc-cloud-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 root@100.70.177.44:/tmp/rc-cloud-backup.db "$BACKUP_DIR/racecontrol-cloud.db" 2>/dev/null && \
  echo "  OK: racecontrol-cloud.db" || { echo "  FAIL: racecontrol-cloud.db"; FAILURES=$((FAILURES + 1)); }

# Cloud admin.db (OPS-08) — verified path: /root/racingpoint-admin/data/admin.db
$BONO_SSH "sqlite3 /root/racingpoint-admin/data/admin.db '.backup /tmp/admin-cloud-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 root@100.70.177.44:/tmp/admin-cloud-backup.db "$BACKUP_DIR/admin-cloud.db" 2>/dev/null && \
  echo "  OK: admin-cloud.db ($(wc -c < "$BACKUP_DIR/admin-cloud.db" 2>/dev/null || echo '?') bytes)" || \
  { echo "  FAIL: admin-cloud.db"; FAILURES=$((FAILURES + 1)); }

# ─── Config backups ──────────────────────────────────────────────────
echo "[4/4] Backing up critical configs..."
mkdir -p "$BACKUP_DIR/configs"

scp -o ConnectTimeout=5 ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.toml "$BACKUP_DIR/configs/racecontrol-server.toml" 2>/dev/null && \
  echo "  OK: server racecontrol.toml" || { echo "  FAIL: server config"; FAILURES=$((FAILURES + 1)); }

cp C:/Users/bono/racingpoint/comms-link/.env "$BACKUP_DIR/configs/comms-link.env" 2>/dev/null && \
  echo "  OK: comms-link .env" || echo "  SKIP: comms-link .env"

# ─── Prune old backups — keep 30 days + first-of-month forever (OPS-10: 12-month retention)
BACKUP_COUNT=$(ls -d "$BACKUP_ROOT"/20* 2>/dev/null | wc -l)
if [ "$BACKUP_COUNT" -gt "$MAX_BACKUPS" ]; then
  echo ""
  echo "Pruning old backups (keeping last $MAX_BACKUPS + first-of-month)..."
  ls -d "$BACKUP_ROOT"/20* | head -$((BACKUP_COUNT - MAX_BACKUPS)) | while read dir; do
    DIRNAME=$(basename "$dir")
    # Keep first-of-month snapshots (YYYY-MM-01_HHMM pattern) for 12 months
    if [[ "$DIRNAME" == *"-01_"* ]]; then
      # Check if older than 12 months
      DIR_DATE="${DIRNAME%_*}"
      CUTOFF_DATE=$(date -d "-12 months" '+%Y-%m-%d' 2>/dev/null || date -v-12m '+%Y-%m-%d' 2>/dev/null || echo "")
      if [ -n "$CUTOFF_DATE" ] && [[ "$DIR_DATE" < "$CUTOFF_DATE" ]]; then
        rm -rf "$dir"
        echo "  Removed (>12 months): $DIRNAME"
      else
        echo "  Kept (first-of-month): $DIRNAME"
      fi
    else
      rm -rf "$dir"
      echo "  Removed: $DIRNAME"
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
echo "============================================================"

# ─── Rsync to Bono VPS (OPS-11) ──────────────────────────────────────
echo ""
echo "Syncing today's backup to Bono VPS..."
rsync -az --timeout=30 "$BACKUP_DIR/" root@100.70.177.44:/root/backups/venue/"$TIMESTAMP"/ 2>/dev/null && \
  echo "  OK: rsync to /root/backups/venue/$TIMESTAMP/" || \
  { echo "  FAIL: rsync to Bono VPS"; FAILURES=$((FAILURES + 1)); }

# ─── Post-backup validation (OPS-14) ──────────────────────────────────
echo ""
echo "Validating backup completeness..."
CRITICAL_DBS=("racecontrol.db" "admin.db" "racecontrol-cloud.db" "admin-cloud.db")
MISSING_DBS=""
for db in "${CRITICAL_DBS[@]}"; do
  DB_PATH="$BACKUP_DIR/$db"
  if [ ! -f "$DB_PATH" ]; then
    MISSING_DBS="$MISSING_DBS $db(MISSING)"
  elif [ ! -s "$DB_PATH" ]; then
    MISSING_DBS="$MISSING_DBS $db(EMPTY)"
  else
    SIZE=$(wc -c < "$DB_PATH")
    echo "  OK: $db ($SIZE bytes)"
  fi
done

if [ -n "$MISSING_DBS" ]; then
  ALERT_MSG="BACKUP ALERT ($TIMESTAMP): Missing/empty databases:$MISSING_DBS. Check James backup script."
  echo "  ALERT: $ALERT_MSG"
  # Send WhatsApp alert via comms-link relay on James .27
  curl -s -m 10 -X POST http://localhost:8766/relay/alert \
    -H "Content-Type: application/json" \
    -d "{\"message\": \"$ALERT_MSG\"}" 2>/dev/null || \
    echo "  WARN: Could not send WhatsApp alert (comms-link relay down?)"
  FAILURES=$((FAILURES + 1))
else
  echo "  All critical databases present and non-empty"
fi

exit $FAILURES
