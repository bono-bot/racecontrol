#!/usr/bin/env bash
# cleanup-pos-edge-backups.sh - One-shot cleanup of POS Edge-backup files.
#
# The 2026-04-20 Edge->Chrome kiosk swap left two backup files on POS:
#   - C:\Windows\System32\Tasks\RestartKiosk-Edge-backup-2026-04-20.xml
#     (OR wherever the backup was placed; actual path is known at ship time)
#   - C:\RacingPoint\BillingDashboard-Edge-backup-2026-04-20.txt
#
# The 72h rollback window ends 2026-04-23 00:55 IST, after which the backups
# are safe to delete. This script is date-guarded: it refuses to run before
# that window expires unless --force is passed.
#
# Usage (any time on or after 2026-04-23 00:55 IST):
#   ./cleanup-pos-edge-backups.sh
#
# Dry-run (lists what would be deleted, no changes):
#   ./cleanup-pos-edge-backups.sh --dry-run
#
# Bypass the date guard (expert-only, e.g. for early cleanup after
# confirmation that Chrome swap is stable):
#   ./cleanup-pos-edge-backups.sh --force

set -u

POS_HOST="POS@192.168.31.20"
DRY_RUN=0
FORCE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --force)   FORCE=1; shift ;;
        -h|--help) sed -n '2,/^set -u/p' "$0" | sed 's/^#//'; exit 0 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

# IST-aware date check: backup retention window is 72h from 2026-04-20 00:55 IST
# => expires 2026-04-23 00:55 IST. Use python for robust tz handling (git bash
# TZ=Asia/Kolkata silently no-ops on Windows per CLAUDE.md).
NOW_IST_EPOCH=$(python3 -c "
from datetime import datetime, timedelta
ist_now = datetime.utcnow() + timedelta(hours=5, minutes=30)
print(int(ist_now.timestamp()))
")
EXPIRES_EPOCH=$(python3 -c "
from datetime import datetime
# 2026-04-23 00:55 IST == 2026-04-22 19:25 UTC
expires = datetime(2026, 4, 22, 19, 25, 0)
print(int(expires.timestamp()))
")

NOW_IST=$(python3 -c "
from datetime import datetime, timedelta
ist_now = datetime.utcnow() + timedelta(hours=5, minutes=30)
print(ist_now.strftime('%Y-%m-%d %H:%M IST'))
")

echo "Now (IST):          $NOW_IST"
echo "Window expires:     2026-04-23 00:55 IST"

if [ "$NOW_IST_EPOCH" -lt "$EXPIRES_EPOCH" ] && [ "$FORCE" -eq 0 ]; then
    echo "ERROR: 72h rollback window has NOT expired. Refusing to delete."
    echo "       Re-run on/after 2026-04-23 00:55 IST, or pass --force to bypass."
    exit 1
fi

echo ""
echo "--- Locating backup files on $POS_HOST ---"

# Find backup files created by the 2026-04-20 swap. Pattern is
# *-Edge-backup-2026-04-20.* across known autostart surfaces.
LIST=$(ssh -o ConnectTimeout=5 "$POS_HOST" 'powershell -NoProfile -Command "@( Get-ChildItem -Path C:\RacingPoint -Filter *-Edge-backup-2026-04-20.* -ErrorAction SilentlyContinue; Get-ChildItem -Path C:\Windows\System32\Tasks -Filter *-Edge-backup-2026-04-20.* -ErrorAction SilentlyContinue ) | ForEach-Object { $_.FullName }"' 2>&1)

if [ -z "$LIST" ] || echo "$LIST" | grep -qi 'error\|cannot'; then
    echo "No backup files found (or SSH failed). Output:"
    echo "$LIST"
    exit 0
fi

echo "Found:"
echo "$LIST" | sed 's/^/  /'
echo ""

if [ "$DRY_RUN" -eq 1 ]; then
    echo "--dry-run: no changes made."
    exit 0
fi

echo "--- Deleting ---"
while IFS= read -r path; do
    [ -z "$path" ] && continue
    echo "  rm $path"
    ssh "$POS_HOST" "powershell -NoProfile -Command \"Remove-Item -LiteralPath '$path' -Force -ErrorAction SilentlyContinue\""
done <<EOF
$LIST
EOF

echo ""
echo "--- Verifying ---"
POST=$(ssh "$POS_HOST" 'powershell -NoProfile -Command "@( Get-ChildItem -Path C:\RacingPoint -Filter *-Edge-backup-2026-04-20.* -ErrorAction SilentlyContinue; Get-ChildItem -Path C:\Windows\System32\Tasks -Filter *-Edge-backup-2026-04-20.* -ErrorAction SilentlyContinue ) | ForEach-Object { $_.FullName }"')

if [ -z "$POST" ]; then
    echo "OK: all 2026-04-20 Edge-backup files removed."
else
    echo "WARN: some files remain:"
    echo "$POST" | sed 's/^/  /'
    exit 1
fi
