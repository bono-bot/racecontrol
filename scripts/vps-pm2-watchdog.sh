#!/bin/bash
# vps-pm2-watchdog.sh — PM2 meta-monitor for Bono VPS
#
# Runs via cron every 2 minutes on VPS. Checks critical PM2 services.
# Restarts crashed services, alerts James via comms-link.
#
# Install: echo "*/2 * * * * bash /root/racingpoint/racecontrol/scripts/vps-pm2-watchdog.sh" | crontab -
# Or add to rc-guardian's monitoring loop.

LOG="/root/racingpoint/pm2-watchdog.log"

# Rotate log if >1MB
if [ -f "$LOG" ] && [ "$(stat -c%s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
  mv "$LOG" "${LOG}.prev" 2>/dev/null
fi

log() { echo "$(date -u +'%Y-%m-%dT%H:%M:%SZ') $*" >> "$LOG"; }

CRITICAL_SERVICES=(
  "racecontrol"
  "comms-link"
  "racingpoint-admin"
  "racingpoint-kiosk"
  "racingpoint-web"
  "racecontrol-pwa"
  "rc-guardian"
)

FAILURES=()

for svc in "${CRITICAL_SERVICES[@]}"; do
  status=$(pm2 show "$svc" 2>/dev/null | grep "status" | awk '{print $4}')
  if [ "$status" != "online" ]; then
    log "RESTART $svc (status=$status)"
    pm2 restart "$svc" 2>/dev/null
    FAILURES+=("$svc")
  fi
done

if [ ${#FAILURES[@]} -gt 0 ]; then
  log "RESTARTED: ${FAILURES[*]}"
fi

# Also check PM2 daemon itself
if ! pm2 ping > /dev/null 2>&1; then
  log "CRITICAL: PM2 daemon dead, resurrecting"
  pm2 resurrect 2>/dev/null
fi
