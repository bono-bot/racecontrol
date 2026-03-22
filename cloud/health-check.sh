#!/usr/bin/env bash
# Racing Point VPS Health Monitor
# Cron: */2 * * * * /root/racingpoint/racecontrol/cloud/health-check.sh >> /var/log/rc-health-check.log 2>&1

set -euo pipefail

# --- Config ---
EVOLUTION_API_KEY="CHANGE_ME"
UDAY_WHATSAPP="91XXXXXXXXXX"
EVOLUTION_INSTANCE="racingpoint"
COOLDOWN_SECS=1800
STATE_DIR="/tmp/rc-health-state"
ALERT_DIR="/tmp/rc-health-alerts"
COMMS_DIR="/root/racingpoint/comms-link"
COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0"
COMMS_URL="ws://localhost:8765"
LOG_TAG="[RC Health]"

# --- Functions ---

log() {
  echo "$(date '+%Y-%m-%d %H:%M:%S %Z') $LOG_TAG $*"
}

get_ist_timestamp() {
  TZ='Asia/Kolkata' date '+%Y-%m-%d %H:%M:%S IST'
}

check_cooldown() {
  local key="$1"
  local cooldown_file="$ALERT_DIR/$key"
  if [[ -f "$cooldown_file" ]]; then
    local last_alert
    last_alert=$(cat "$cooldown_file")
    local now
    now=$(date +%s)
    local elapsed=$(( now - last_alert ))
    if (( elapsed < COOLDOWN_SECS )); then
      return 1  # In cooldown, do not alert
    fi
  fi
  return 0  # Can alert
}

update_cooldown() {
  local key="$1"
  date +%s > "$ALERT_DIR/$key"
}

send_alert() {
  local message="$1"
  local ist_time
  ist_time=$(get_ist_timestamp)
  local full_message="[Racing Point VPS Alert]
${message}
Time: ${ist_time}"

  # WhatsApp via Evolution API
  curl -s -X POST "http://localhost:53622/message/sendText/${EVOLUTION_INSTANCE}" \
    -H "Content-Type: application/json" \
    -H "apikey: ${EVOLUTION_API_KEY}" \
    -d "{\"number\": \"${UDAY_WHATSAPP}\", \"text\": $(echo "$full_message" | jq -Rs .)}" \
    > /dev/null 2>&1 || log "WARNING: WhatsApp alert failed"

  # comms-link to James
  (
    cd "$COMMS_DIR" && \
    COMMS_PSK="$COMMS_PSK" COMMS_URL="$COMMS_URL" \
    node send-message.js "[VPS Health] ${message}" \
    > /dev/null 2>&1
  ) || log "WARNING: comms-link alert failed"

  log "ALERT SENT: $message"
}

# --- Checks ---

check_pm2_status() {
  local pm2_json
  pm2_json=$(pm2 jlist 2>/dev/null) || { log "ERROR: pm2 jlist failed"; return; }

  local count
  count=$(echo "$pm2_json" | jq 'length')

  for (( i=0; i<count; i++ )); do
    local name status
    name=$(echo "$pm2_json" | jq -r ".[$i].name")
    status=$(echo "$pm2_json" | jq -r ".[$i].pm2_env.status")

    if [[ "$status" == "errored" || "$status" == "stopped" ]]; then
      local key="pm2_${name}"
      if check_cooldown "$key"; then
        send_alert "PM2 process '${name}' is ${status}"
        update_cooldown "$key"
      fi
    fi
  done
}

check_pm2_crashloop() {
  local pm2_json
  pm2_json=$(pm2 jlist 2>/dev/null) || { log "ERROR: pm2 jlist failed"; return; }

  local count
  count=$(echo "$pm2_json" | jq 'length')
  local now
  now=$(date +%s)

  for (( i=0; i<count; i++ )); do
    local name restarts
    name=$(echo "$pm2_json" | jq -r ".[$i].name")
    restarts=$(echo "$pm2_json" | jq -r ".[$i].pm2_env.restart_time")

    local state_file="$STATE_DIR/${name}.restarts"

    if [[ -f "$state_file" ]]; then
      local prev_time prev_restarts
      prev_time=$(awk '{print $1}' "$state_file")
      prev_restarts=$(awk '{print $2}' "$state_file")

      local delta_restarts=$(( restarts - prev_restarts ))
      local delta_time=$(( now - prev_time ))

      if (( delta_restarts > 3 && delta_time < 600 )); then
        local key="pm2_crashloop_${name}"
        if check_cooldown "$key"; then
          send_alert "PM2 process '${name}' crash loop detected: ${delta_restarts} restarts in $(( delta_time / 60 ))m"
          update_cooldown "$key"
        fi
      fi
    fi

    # Update stored state
    echo "$now $restarts" > "$state_file"
  done
}

check_disk() {
  local usage
  usage=$(df -h / | awk 'NR==2{print $5}' | tr -d '%')

  if (( usage > 90 )); then
    if check_cooldown "disk"; then
      send_alert "Disk usage critical: ${usage}% (threshold: 90%)"
      update_cooldown "disk"
    fi
  fi
}

check_memory() {
  local usage
  usage=$(free | awk '/Mem:/{printf "%.0f", $3/$2 * 100}')

  if (( usage > 90 )); then
    if check_cooldown "memory"; then
      send_alert "Memory usage critical: ${usage}% (threshold: 90%)"
      update_cooldown "memory"
    fi
  fi
}

# --- Main ---
mkdir -p "$STATE_DIR" "$ALERT_DIR"

log "Health check starting"
check_pm2_status || true
check_pm2_crashloop || true
check_disk || true
check_memory || true
log "Health check complete"
