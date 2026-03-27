#!/bin/bash
# bono-racecontrol-monitor.sh
# Runs on Bono VPS via cron every 5 minutes.
# Monitors the LOCAL pm2 racecontrol process (cloud failover).
# If pm2 reports stopped/errored, restart it and alert.
#
# Install on Bono VPS:
#   scp bono-racecontrol-monitor.sh root@100.70.177.44:/root/bono-racecontrol-monitor.sh
#   chmod +x /root/bono-racecontrol-monitor.sh
#   (crontab -l 2>/dev/null; echo '*/5 * * * * /root/bono-racecontrol-monitor.sh >> /root/bono-racecontrol-monitor.log 2>&1') | crontab -

STATE_FILE="/root/bono-racecontrol-monitor.state"
LOG_FILE="/root/bono-racecontrol-monitor.log"
MAX_LOG_LINES=300

ts() { date '+%Y-%m-%d %H:%M:%S'; }
log() { echo "$(ts) | $1"; }

# Rotate log
if [ -f "$LOG_FILE" ] && [ "$(wc -l < "$LOG_FILE")" -gt "$MAX_LOG_LINES" ]; then
    tail -n 100 "$LOG_FILE" > "${LOG_FILE}.tmp"
    mv "${LOG_FILE}.tmp" "$LOG_FILE"
fi

# Read state
consecutive_fails=0
total_restarts=0
check_count=0
last_restart=""
if [ -f "$STATE_FILE" ]; then
    consecutive_fails=$(grep -o 'fails=[0-9]*' "$STATE_FILE" | cut -d= -f2)
    total_restarts=$(grep -o 'total_restarts=[0-9]*' "$STATE_FILE" | cut -d= -f2)
    check_count=$(grep -o 'checks=[0-9]*' "$STATE_FILE" | cut -d= -f2)
    last_restart=$(grep -o 'last_restart=[^|]*' "$STATE_FILE" | cut -d= -f2)
    consecutive_fails=${consecutive_fails:-0}
    total_restarts=${total_restarts:-0}
    check_count=${check_count:-0}
fi

save_state() {
    echo "fails=${consecutive_fails}|total_restarts=${total_restarts}|checks=${check_count}|last_restart=${last_restart}" > "$STATE_FILE"
}

send_whatsapp() {
    local message="$1"
    local phone="$2"
    local evo_url="${EVOLUTION_URL:-http://localhost:53622}"
    local evo_instance="${EVOLUTION_INSTANCE:-Racing Point Reception}"
    local evo_key="${EVOLUTION_API_KEY:-}"
    if [ -z "$evo_key" ]; then
        log "ERROR: EVOLUTION_API_KEY env var is not set — cannot send WhatsApp"
        return 1
    fi
    curl -s -X POST "${evo_url}/message/sendText/${evo_instance}" \
        -H "Content-Type: application/json" \
        -H "apikey: ${evo_key}" \
        -d "{\"number\":\"${phone}\",\"text\":\"${message}\"}" > /dev/null 2>&1
}

send_alert() {
    local message="$1"
    log "ALERT: $message"
    send_whatsapp "[BONO-RC-MONITOR] $message" "917075778180"
}

check_count=$((check_count + 1))

# Check pm2 racecontrol status
rc_status=$(pm2 jlist 2>/dev/null | python3 -c "
import sys, json
try:
    procs = json.load(sys.stdin)
    for p in procs:
        if p.get('name') == 'racecontrol':
            print(p.get('pm2_env', {}).get('status', 'unknown'))
            break
    else:
        print('not_found')
except:
    print('error')
" 2>/dev/null)

if [ "$rc_status" = "online" ]; then
    # Also verify health endpoint responds
    health_code=$(curl -s -o /dev/null -w '%{http_code}' --connect-timeout 3 http://localhost:8080/api/v1/health 2>/dev/null)
    if [ "$health_code" = "200" ]; then
        if [ "$consecutive_fails" -gt 0 ]; then
            log "RECOVERED: racecontrol online after $consecutive_fails failed checks (restarted $total_restarts times)"
            send_alert "Bono racecontrol recovered after $consecutive_fails failed checks."
        fi
        consecutive_fails=0
        # Heartbeat every 12th check (~1 hour)
        if [ $((check_count % 12)) -eq 1 ]; then
            log "OK - racecontrol online, health 200 (check #$check_count, restarts: $total_restarts)"
        fi
        save_state
        exit 0
    else
        log "WARNING: pm2 says online but health returns $health_code"
    fi
fi

# Failed
consecutive_fails=$((consecutive_fails + 1))
log "FAIL: racecontrol status=$rc_status, fails=$consecutive_fails"

# Hysteresis: 2 consecutive fails (10 min) before restart
if [ "$consecutive_fails" -lt 2 ]; then
    log "Waiting for hysteresis (need 2 consecutive failures)"
    save_state
    exit 0
fi

# Rate limit: max 1 restart per 10 min
if [ -n "$last_restart" ]; then
    last_epoch=$(date -d "$last_restart" +%s 2>/dev/null || echo 0)
    now_epoch=$(date +%s)
    elapsed=$(( now_epoch - last_epoch ))
    if [ "$elapsed" -lt 600 ]; then
        log "Rate limited: last restart ${elapsed}s ago (min 600s)"
        save_state
        exit 0
    fi
fi

# Restart
log "=== RESTARTING racecontrol via pm2 ==="
last_restart=$(date '+%Y-%m-%d %H:%M:%S')
total_restarts=$((total_restarts + 1))
save_state

pm2 restart racecontrol 2>&1 | while read -r line; do log "pm2: $line"; done

# Verify after 5s
sleep 5
verify_code=$(curl -s -o /dev/null -w '%{http_code}' --connect-timeout 3 http://localhost:8080/api/v1/health 2>/dev/null)
if [ "$verify_code" = "200" ]; then
    log "RESTART SUCCESS: health returns 200"
    consecutive_fails=0
    save_state
    send_alert "Bono racecontrol restarted successfully (attempt #$total_restarts)."
else
    log "RESTART FAILED: health returns $verify_code after pm2 restart"
    save_state
    send_alert "CRITICAL: Bono racecontrol restart failed (health=$verify_code). Manual intervention needed."
fi
