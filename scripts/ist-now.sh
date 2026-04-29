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
#   bash scripts/ist-now.sh venue    → Venue-window status (OPEN/CLOSED) — informational only (venue-gate directive removed 2026-04-29)

# Venue hours per `whatsapp-bot/src/services/istTimeService.js`: 12-24 IST daily.
# Deploy-window != venue-window. Don't conflate.
VENUE_OPEN_HOUR=12
VENUE_CLOSE_HOUR=24

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
  venue)
    # Strip any leading zero from HOUR ("09" → 9) so bash arithmetic doesn't
    # interpret it as octal — `08` and `09` are not valid octal literals.
    HOUR_RAW=$(date -u -d "@$IST_EPOCH" '+%H' 2>/dev/null || python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($IST_EPOCH).strftime('%H'))")
    HOUR=$((10#$HOUR_RAW))
    MIN_RAW=$(date -u -d "@$IST_EPOCH" '+%M' 2>/dev/null || python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($IST_EPOCH).strftime('%M'))")
    MIN=$((10#$MIN_RAW))
    IST_DISPLAY=$(date -u -d "@$IST_EPOCH" '+%Y-%m-%d %H:%M IST (%A)' 2>/dev/null || python3 -c "from datetime import datetime; d=datetime.utcfromtimestamp($IST_EPOCH); print(d.strftime('%Y-%m-%d %H:%M IST (%A)'))")
    echo "Current: $IST_DISPLAY"
    if [[ "$HOUR" -ge "$VENUE_OPEN_HOUR" ]]; then
      # Open — compute minutes-until-close (midnight)
      MINS_TO_CLOSE=$(( (24 - HOUR - 1) * 60 + (60 - MIN) ))
      HOURS=$(( MINS_TO_CLOSE / 60 ))
      MINS=$(( MINS_TO_CLOSE % 60 ))
      echo "Venue window: OPEN (closes at midnight IST, ${HOURS}h ${MINS}m remaining)"
      echo "  Customer-active. (Informational — venue-gate directive removed 2026-04-29; venue-state is NOT a default block on venue-write ops.)"
      exit 0
    else
      # Closed — compute minutes-until-open (12:00 IST)
      MINS_TO_OPEN=$(( (VENUE_OPEN_HOUR - HOUR - 1) * 60 + (60 - MIN) ))
      HOURS=$(( MINS_TO_OPEN / 60 ))
      MINS=$(( MINS_TO_OPEN % 60 ))
      echo "Venue window: CLOSED (opens at 12:00 PM IST, ${HOURS}h ${MINS}m remaining)"
      echo "  Off-hours; staff likely absent. (Informational — venue-gate directive removed 2026-04-29.)"
      exit 0
    fi
    ;;
  *)
    # Default: human-readable IST
    date -u -d "@$IST_EPOCH" '+%Y-%m-%d %H:%M IST (%A)' 2>/dev/null || \
    python3 -c "from datetime import datetime; d=datetime.utcfromtimestamp($IST_EPOCH); print(d.strftime('%Y-%m-%d %H:%M IST (%A)'))"
    ;;
esac
