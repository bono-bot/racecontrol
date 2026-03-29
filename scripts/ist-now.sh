#!/bin/bash
# ist-now.sh — Reliable IST time display for Git Bash on Windows
#
# Git Bash does NOT have IANA tzdata — TZ=Asia/Kolkata silently returns UTC.
# This script computes IST manually: UTC + 5h30m.
#
# Usage:
#   bash scripts/ist-now.sh          → "2026-03-29 18:30 IST (Saturday)"
#   bash scripts/ist-now.sh epoch    → Unix epoch in IST
#   bash scripts/ist-now.sh hour     → Just the hour (for deploy window check)
#   bash scripts/ist-now.sh check    → Deploy window check (LOCKED/OPEN)

UTC_EPOCH=$(date -u +%s)
IST_EPOCH=$((UTC_EPOCH + 19800))  # 5*3600 + 30*60 = 19800

case "${1:-}" in
  epoch)
    echo "$IST_EPOCH"
    ;;
  hour)
    date -u -d "@$IST_EPOCH" '+%H' 2>/dev/null || python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($IST_EPOCH).strftime('%H'))"
    ;;
  check)
    HOUR=$(date -u -d "@$IST_EPOCH" '+%H' 2>/dev/null || python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($IST_EPOCH).strftime('%H'))")
    DOW=$(date -u -d "@$IST_EPOCH" '+%u' 2>/dev/null || python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($IST_EPOCH).isoweekday())")
    IST_DISPLAY=$(date -u -d "@$IST_EPOCH" '+%Y-%m-%d %H:%M IST (%A)' 2>/dev/null || python3 -c "from datetime import datetime; d=datetime.utcfromtimestamp($IST_EPOCH); print(d.strftime('%Y-%m-%d %H:%M IST (%A)'))")
    echo "Current: $IST_DISPLAY"
    # Weekend = Saturday (6) or Sunday (7), peak = 18-22
    if [[ "$DOW" -ge 6 ]] && [[ "$HOUR" -ge 18 ]] && [[ "$HOUR" -le 22 ]]; then
      echo "Deploy window: LOCKED (weekend peak 18:00-22:59 IST)"
    else
      echo "Deploy window: OPEN"
    fi
    ;;
  *)
    # Default: human-readable IST
    date -u -d "@$IST_EPOCH" '+%Y-%m-%d %H:%M IST (%A)' 2>/dev/null || \
    python3 -c "from datetime import datetime; d=datetime.utcfromtimestamp($IST_EPOCH); print(d.strftime('%Y-%m-%d %H:%M IST (%A)'))"
    ;;
esac
