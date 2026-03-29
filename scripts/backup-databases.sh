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
#   schtasks /Create /SC DAILY /ST 03:00 /TN "DatabaseBackup" /TR "bash C:\Users\bono\racingpoint\racecontrol\scripts\backup-databases.sh" /RU bono

set -e

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

# racecontrol.db — the main venue database
$SERVER_SSH "sqlite3 C:\\RacingPoint\\racecontrol.db '.backup C:\\RacingPoint\\racecontrol-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 ADMIN@100.125.108.37:C:/RacingPoint/racecontrol-backup.db "$BACKUP_DIR/racecontrol.db" 2>/dev/null && \
  echo "  OK: racecontrol.db ($(wc -c < "$BACKUP_DIR/racecontrol.db" 2>/dev/null || echo '?') bytes)" || \
  echo "  FAIL: racecontrol.db — server unreachable or backup failed"

# ─── James databases ──────────────────────────────────────────────────
echo "[2/4] Backing up James local databases..."

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
echo "[3/4] Backing up VPS databases from Bono..."
BONO_SSH="ssh -o ConnectTimeout=5 root@100.70.177.44"

# WhatsApp bot database
$BONO_SSH "sqlite3 /root/racingpoint/whatsapp-bot/data/bot.sqlite '.backup /tmp/bot-backup.sqlite'" 2>/dev/null && \
  scp -o ConnectTimeout=5 root@100.70.177.44:/tmp/bot-backup.sqlite "$BACKUP_DIR/bot.sqlite" 2>/dev/null && \
  echo "  OK: bot.sqlite" || echo "  FAIL: bot.sqlite"

# Cloud racecontrol.db
$BONO_SSH "sqlite3 /root/racecontrol/data/racecontrol.db '.backup /tmp/rc-cloud-backup.db'" 2>/dev/null && \
  scp -o ConnectTimeout=5 root@100.70.177.44:/tmp/rc-cloud-backup.db "$BACKUP_DIR/racecontrol-cloud.db" 2>/dev/null && \
  echo "  OK: racecontrol-cloud.db" || echo "  FAIL: racecontrol-cloud.db"

# ─── Config backups ──────────────────────────────────────────────────
echo "[4/4] Backing up critical configs..."
mkdir -p "$BACKUP_DIR/configs"

scp -o ConnectTimeout=5 ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.toml "$BACKUP_DIR/configs/racecontrol-server.toml" 2>/dev/null && \
  echo "  OK: server racecontrol.toml" || echo "  FAIL: server config"

cp C:/Users/bono/racingpoint/comms-link/.env "$BACKUP_DIR/configs/comms-link.env" 2>/dev/null && \
  echo "  OK: comms-link .env" || echo "  SKIP: comms-link .env"

# ─── Prune old backups ───────────────────────────────────────────────
BACKUP_COUNT=$(ls -d "$BACKUP_ROOT"/20* 2>/dev/null | wc -l)
if [ "$BACKUP_COUNT" -gt "$MAX_BACKUPS" ]; then
  PRUNE_COUNT=$((BACKUP_COUNT - MAX_BACKUPS))
  echo ""
  echo "Pruning $PRUNE_COUNT old backup(s) (keeping last $MAX_BACKUPS)..."
  ls -d "$BACKUP_ROOT"/20* | head -$PRUNE_COUNT | while read dir; do
    rm -rf "$dir"
    echo "  Removed: $(basename $dir)"
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
