#!/bin/bash
# infra-health-monitor.sh — Fleet-wide infrastructure health monitor
#
# Runs on James (.27) every 2 minutes via scheduled task.
# Checks every service across all machines. Alerts on failures.
# Designed to be the meta-monitor that watches everything, including watchdogs.
#
# Usage: bash scripts/infra-health-monitor.sh [--once] [--alert]
#   --once: run single check and exit (for testing)
#   --alert: send comms-link message on failures

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
LOG_FILE="${SCRIPT_DIR}/../infra-health.log"
ALERT_FILE="/tmp/infra-health-alert-sent"
ONCE=false
ALERT=false

for arg in "$@"; do
  case $arg in
    --once) ONCE=true ;;
    --alert) ALERT=true ;;
  esac
done

# Rotate log if >1MB
if [ -f "$LOG_FILE" ] && [ "$(wc -c < "$LOG_FILE" 2>/dev/null)" -gt 1048576 ]; then
  mv "$LOG_FILE" "${LOG_FILE}.prev" 2>/dev/null
fi

log() { echo "$(date -u +'%Y-%m-%dT%H:%M:%SZ') | $*" >> "$LOG_FILE"; }
log_and_echo() { echo "$*"; log "$*"; }

FAILURES=()

check() {
  local label=$1 url=$2 timeout=${3:-5}
  if curl -sf --max-time "$timeout" "$url" > /dev/null 2>&1; then
    echo "  OK   $label"
  else
    echo "  FAIL $label ($url)"
    FAILURES+=("$label")
    log "FAIL: $label ($url)"
  fi
}

check_process() {
  local label=$1 name=$2
  if tasklist 2>/dev/null | grep -qi "$name"; then
    echo "  OK   $label"
  else
    echo "  FAIL $label (process $name not found)"
    FAILURES+=("$label")
    log "FAIL: $label (process $name not found)"
  fi
}

check_ssh() {
  local label=$1 host=$2 cmd=$3
  local result
  result=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$host" "$cmd" 2>/dev/null)
  if [ $? -eq 0 ] && [ -n "$result" ]; then
    echo "  OK   $label"
  else
    echo "  FAIL $label (ssh $host)"
    FAILURES+=("$label")
    log "FAIL: $label (ssh $host)"
  fi
}

run_check() {
  FAILURES=()

  echo ""
  echo "=== Infrastructure Health Check $(date -u +'%H:%M:%S UTC') ==="
  echo ""

  # ---- SERVER .23 ----
  echo "--- Server .23 ---"
  check "racecontrol :8080" "http://192.168.31.23:8080/api/v1/health"
  check "web :3200" "http://192.168.31.23:3200/"
  check "kiosk :3300" "http://192.168.31.23:3300/kiosk/staff"
  check "admin :3201" "http://192.168.31.23:3201/"
  check "fleet health" "http://192.168.31.23:8080/api/v1/fleet/health"

  # ---- PODS 1-8 ----
  echo "--- Pods ---"
  for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do
    pod_num=$(echo "$pod_ip" | awk -F. '{
      if ($4==89) print 1; else if ($4==33) print 2; else if ($4==28) print 3;
      else if ($4==88) print 4; else if ($4==86) print 5; else if ($4==87) print 6;
      else if ($4==38) print 7; else if ($4==91) print 8;
    }')
    check "pod${pod_num} agent" "http://${pod_ip}:8090/health" 3
    check "pod${pod_num} sentry" "http://${pod_ip}:8091/health" 3
  done

  # ---- POS .20 ----
  echo "--- POS ---"
  check "POS agent" "http://192.168.31.130:8090/health" 3

  # ---- JAMES .27 (local) ----
  echo "--- James .27 ---"
  check "comms-link" "http://localhost:8766/health"
  check "go2rtc" "http://localhost:1984/api/streams" 3
  check "Ollama" "http://localhost:11434/api/tags" 3
  check_process "rc-watchdog" "rc-watchdog"
  check_process "rc-sentry-ai" "rc-sentry-ai"

  # ---- BONO VPS ----
  echo "--- Bono VPS ---"
  check "VPS racecontrol" "http://100.70.177.44:8080/api/v1/health" 10
  check "VPS comms-link" "http://100.70.177.44:8765/health" 10
  check "VPS web :3200" "http://100.70.177.44:3200/" 10
  check "VPS admin :3201" "http://100.70.177.44:3201/" 10
  check "VPS kiosk :3300" "http://100.70.177.44:3300/" 10
  check "VPS PWA :3500" "http://100.70.177.44:3500/" 10

  # ---- COMMS-LINK RELAY ----
  echo "--- Comms-link ---"
  local relay_result
  relay_result=$(curl -sf --max-time 10 http://localhost:8766/health 2>/dev/null)
  if echo "$relay_result" | grep -q '"connected":true'; then
    echo "  OK   comms-link WS connected"
  else
    echo "  FAIL comms-link WS disconnected"
    FAILURES+=("comms-link WS")
    log "FAIL: comms-link WS disconnected"
  fi

  # ---- SUMMARY ----
  echo ""
  local total=36
  local fail_count=${#FAILURES[@]}
  local pass_count=$((total - fail_count))

  if [ "$fail_count" -eq 0 ]; then
    echo "RESULT: ALL $total SERVICES HEALTHY"
    log "CHECK OK: $total/$total healthy"
    # Clear alert cooldown
    rm -f "$ALERT_FILE" 2>/dev/null
  else
    echo "RESULT: $fail_count FAILURES / $total services"
    echo "FAILED: ${FAILURES[*]}"
    log "CHECK FAIL: $fail_count failures — ${FAILURES[*]}"

    # Alert via comms-link (with 10min cooldown to avoid spam)
    if [ "$ALERT" = "true" ]; then
      if [ ! -f "$ALERT_FILE" ] || [ "$(find "$ALERT_FILE" -mmin +10 2>/dev/null)" ]; then
        local msg="INFRA ALERT: $fail_count services down — ${FAILURES[*]}"
        cd ~/racingpoint/comms-link 2>/dev/null && \
          COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" \
          COMMS_URL="ws://srv1422716.hstgr.cloud:8765" \
          timeout 10 node send-message.js "$msg" 2>/dev/null
        touch "$ALERT_FILE"
        log "ALERT sent: $msg"
      else
        log "ALERT suppressed (cooldown)"
      fi
    fi
  fi
  echo ""
}

# Main
if [ "$ONCE" = "true" ]; then
  run_check
else
  while true; do
    run_check
    sleep 120
  done
fi
