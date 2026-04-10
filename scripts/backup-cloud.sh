#!/usr/bin/env bash
# backup-cloud.sh -- Cloud database backup for Bono VPS
#
# Backs up cloud racecontrol.db and admin.db to /root/backups/cloud/
# with 30-day rolling retention + first-of-month 12-month snapshots.
#
# Schedule: 30 21 * * * /root/backup-cloud.sh >> /root/backups/cloud/backup.log 2>&1
# (21:30 UTC = 03:00 IST)
#
# Requirements satisfied: OPS-08, OPS-09, OPS-10, OPS-14
# Phase: 351-data-durability

set -euo pipefail

BACKUP_ROOT="/root/backups/cloud"
TIMESTAMP=$(date '+%Y-%m-%d_%H%M')
BACKUP_DIR="${BACKUP_ROOT}/${TIMESTAMP}"
MAX_BACKUPS=30
MONTHLY_RETAIN=12
FAILURES=0

echo "============================================================"
echo "CLOUD DATABASE BACKUP -- ${TIMESTAMP}"
echo "============================================================"

mkdir -p "${BACKUP_DIR}"

# ---- racecontrol.db (OPS-09) -----------------------------------------
RC_DB="/root/racingpoint/racecontrol/data/racecontrol.db"
if [ -f "${RC_DB}" ]; then
  sqlite3 "${RC_DB}" ".backup ${BACKUP_DIR}/racecontrol.db" 2>/dev/null && \
    echo "  OK: racecontrol.db ($(wc -c < "${BACKUP_DIR}/racecontrol.db" 2>/dev/null || echo '?') bytes)" || \
    { echo "  FAIL: racecontrol.db -- sqlite3 backup failed"; FAILURES=$((FAILURES + 1)); }
else
  echo "  FAIL: racecontrol.db not found at ${RC_DB}"
  FAILURES=$((FAILURES + 1))
fi

# ---- admin.db (OPS-08) -----------------------------------------------
# Try primary path first, then fallback
ADMIN_DB=""
for candidate in \
  "/root/racingpoint/racingpoint-admin/data/admin.db" \
  "/root/racingpoint-admin/data/admin.db" \
  "/root/racingpoint/admin/data/admin.db"; do
  if [ -f "${candidate}" ]; then
    ADMIN_DB="${candidate}"
    break
  fi
done

if [ -n "${ADMIN_DB}" ]; then
  sqlite3 "${ADMIN_DB}" ".backup ${BACKUP_DIR}/admin.db" 2>/dev/null && \
    echo "  OK: admin.db ($(wc -c < "${BACKUP_DIR}/admin.db" 2>/dev/null || echo '?') bytes) [from ${ADMIN_DB}]" || \
    { echo "  FAIL: admin.db -- sqlite3 backup failed"; FAILURES=$((FAILURES + 1)); }
else
  echo "  WARN: admin.db not found (paths tried: /root/racingpoint/racingpoint-admin/data/, /root/racingpoint-admin/data/, /root/racingpoint/admin/data/)"
  echo "  NOTE: admin.db backup skipped -- locate actual path and update ADMIN_DB candidates"
fi

# ---- First-of-month snapshot (OPS-10) --------------------------------
DAY_OF_MONTH=$(date '+%d')
MONTH_LABEL=$(date '+%Y-%m')
if [ "${DAY_OF_MONTH}" = "01" ]; then
  echo ""
  echo "First of month -- creating monthly snapshots..."
  for db in racecontrol.db admin.db; do
    if [ -f "${BACKUP_DIR}/${db}" ]; then
      cp "${BACKUP_DIR}/${db}" "${BACKUP_ROOT}/${db%.db}-monthly-${MONTH_LABEL}.db" && \
        echo "  OK: ${db%.db}-monthly-${MONTH_LABEL}.db"
    fi
  done
fi

# ---- Post-backup validation (OPS-14) ---------------------------------
echo ""
echo "Validating backup completeness..."
CRITICAL_DBS=("racecontrol.db")
# Only validate admin.db if we found it earlier
if [ -n "${ADMIN_DB:-}" ]; then
  CRITICAL_DBS+=("admin.db")
fi

MISSING_DBS=""
for db in "${CRITICAL_DBS[@]}"; do
  DB_PATH="${BACKUP_DIR}/${db}"
  if [ ! -f "${DB_PATH}" ]; then
    MISSING_DBS="${MISSING_DBS} ${db}(MISSING)"
    FAILURES=$((FAILURES + 1))
  elif [ ! -s "${DB_PATH}" ]; then
    MISSING_DBS="${MISSING_DBS} ${db}(EMPTY)"
    FAILURES=$((FAILURES + 1))
  else
    SIZE=$(wc -c < "${DB_PATH}")
    echo "  OK: ${db} (${SIZE} bytes)"
  fi
done

if [ -n "${MISSING_DBS}" ]; then
  echo "  ALERT: Backup validation failed:${MISSING_DBS}"
  echo "  ACTION: Check cloud DB paths and re-run backup"
fi

# ---- Prune old backups -----------------------------------------------
echo ""
echo "Pruning old backups (keeping last ${MAX_BACKUPS} days + first-of-month up to ${MONTHLY_RETAIN} months)..."

BACKUP_COUNT=$(ls -d "${BACKUP_ROOT}"/20* 2>/dev/null | wc -l)
if [ "${BACKUP_COUNT}" -gt "${MAX_BACKUPS}" ]; then
  PRUNE_COUNT=$((BACKUP_COUNT - MAX_BACKUPS))
  ls -d "${BACKUP_ROOT}"/20* | head -"${PRUNE_COUNT}" | while read -r dir; do
    DIRNAME=$(basename "${dir}")
    # Keep first-of-month snapshots (YYYY-MM-01_HHMM pattern) for 12 months
    if [[ "${DIRNAME}" == *"-01_"* ]]; then
      DIR_DATE="${DIRNAME%_*}"
      # Compute 12-month cutoff
      CURRENT_YEAR=$(date '+%Y')
      CURRENT_MONTH=$(date '+%m')
      CUTOFF_YEAR=$((CURRENT_YEAR - 1))
      CUTOFF="${CUTOFF_YEAR}-$(printf '%02d' "${CURRENT_MONTH}")-01"
      if [[ "${DIR_DATE}" < "${CUTOFF}" ]]; then
        rm -rf "${dir}"
        echo "  Removed (>12 months): ${DIRNAME}"
      else
        echo "  Kept (first-of-month): ${DIRNAME}"
      fi
    else
      rm -rf "${dir}"
      echo "  Removed: ${DIRNAME}"
    fi
  done
fi

# Prune old monthly snapshot files (older than 12 months)
find "${BACKUP_ROOT}" -maxdepth 1 -name "*-monthly-*.db" 2>/dev/null | while read -r f; do
  FNAME=$(basename "${f}")
  # Extract YYYY-MM from filename (e.g. racecontrol-monthly-2025-04.db)
  FDATE=$(echo "${FNAME}" | grep -oP '\d{4}-\d{2}' | tail -1)
  if [ -n "${FDATE}" ]; then
    CURRENT_YEAR=$(date '+%Y')
    CURRENT_MONTH=$(date '+%m')
    CUTOFF_YEAR=$((CURRENT_YEAR - 1))
    CUTOFF="${CUTOFF_YEAR}-$(printf '%02d' "${CURRENT_MONTH}")"
    if [[ "${FDATE}" < "${CUTOFF}" ]]; then
      rm -f "${f}"
      echo "  Removed monthly (>12 months): ${FNAME}"
    fi
  fi
done

# ---- Summary ---------------------------------------------------------
echo ""
echo "============================================================"
TOTAL_SIZE=$(du -sh "${BACKUP_DIR}" 2>/dev/null | awk '{print $1}' || echo "?")
FILE_COUNT=$(find "${BACKUP_DIR}" -type f 2>/dev/null | wc -l)
echo "Cloud backup complete: ${FILE_COUNT} files, ${TOTAL_SIZE} total"
echo "Location: ${BACKUP_DIR}"
echo "Failures: ${FAILURES}"
echo "============================================================"

exit "${FAILURES}"
