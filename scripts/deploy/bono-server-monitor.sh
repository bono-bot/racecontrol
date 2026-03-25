#!/bin/bash
# bono-server-monitor.sh
# Runs on Bono VPS via cron every 3 minutes.
# Layer 4 redundancy: cloud-based monitoring of server .23 via Tailscale.
# If both James and server are down, Bono is the last line of defense.
#
# Install on Bono VPS:
#   scp bono-server-monitor.sh root@100.70.177.44:/root/bono-server-monitor.sh
#   ssh root@100.70.177.44 "chmod +x /root/bono-server-monitor.sh"
#   ssh root@100.70.177.44 "crontab -l 2>/dev/null; echo '*/3 * * * * /root/bono-server-monitor.sh >> /root/bono-server-monitor.log 2>&1'" | ssh root@100.70.177.44 "crontab -"

# Config
SERVER_TAILSCALE_IP="100.125.108.37"
SERVER_LAN_IP="192.168.31.23"
JAMES_TAILSCALE_IP="100.82.33.94"
HEALTH_URL="http://${SERVER_TAILSCALE_IP}:8080/api/v1/health"
STATE_FILE="/root/bono-server-monitor.state"
LOG_FILE="/root/bono-server-monitor.log"
MAX_LOG_LINES=500

# WhatsApp alerting via Evolution API
EVOLUTION_URL="${EVOLUTION_URL:-http://localhost:8080}"
EVOLUTION_INSTANCE="${EVOLUTION_INSTANCE:-racingpoint}"
EVOLUTION_API_KEY="${EVOLUTION_API_KEY:-}"
UDAY_WHATSAPP="919059833001"
STAFF_WHATSAPP="917075778180"

ts() {
    date '+%Y-%m-%d %H:%M:%S'
}

log() {
    echo "$(ts) | $1"
}

# Rotate log if too large
if [ -f "$LOG_FILE" ] && [ "$(wc -l < "$LOG_FILE")" -gt "$MAX_LOG_LINES" ]; then
    tail -n 200 "$LOG_FILE" > "${LOG_FILE}.tmp"
    mv "${LOG_FILE}.tmp" "$LOG_FILE"
fi

# Read state
consecutive_fails=0
last_restart=""
total_restarts=0
if [ -f "$STATE_FILE" ]; then
    consecutive_fails=$(grep -o 'fails=[0-9]*' "$STATE_FILE" | cut -d= -f2)
    last_restart=$(grep -o 'last_restart=[^|]*' "$STATE_FILE" | cut -d= -f2)
    total_restarts=$(grep -o 'total_restarts=[0-9]*' "$STATE_FILE" | cut -d= -f2)
    consecutive_fails=${consecutive_fails:-0}
    total_restarts=${total_restarts:-0}
fi

save_state() {
    echo "fails=${consecutive_fails}|last_restart=${last_restart}|total_restarts=${total_restarts}" > "$STATE_FILE"
}

send_whatsapp() {
    local message="$1"
    local phone="$2"
    if [ -n "$EVOLUTION_API_KEY" ]; then
        curl -s -X POST "${EVOLUTION_URL}/message/sendText/${EVOLUTION_INSTANCE}" \
            -H "Content-Type: application/json" \
            -H "apikey: ${EVOLUTION_API_KEY}" \
            -d "{\"number\":\"${phone}\",\"text\":\"${message}\"}" > /dev/null 2>&1
    fi
}

send_alert() {
    local message="$1"
    log "ALERT: $message"
    # Alert staff WhatsApp
    send_whatsapp "[BONO-MONITOR] $message" "$STAFF_WHATSAPP"
    # Alert Uday for critical
    if echo "$message" | grep -qi "CRITICAL"; then
        send_whatsapp "[BONO-MONITOR] $message" "$UDAY_WHATSAPP"
    fi
}

# Check if server is reachable via Tailscale
if ! ping -c 1 -W 3 "$SERVER_TAILSCALE_IP" > /dev/null 2>&1; then
    consecutive_fails=$((consecutive_fails + 1))
    log "Server unreachable via Tailscale - ping failed - fails=$consecutive_fails"
    save_state
    if [ "$consecutive_fails" -ge 5 ]; then
        send_alert "Server .23 unreachable via Tailscale for 15+ min. Tailscale may be down on server or server is off."
    fi
    exit 0
fi

# Check racecontrol health
health_response=$(curl -s --connect-timeout 5 --max-time 10 "$HEALTH_URL" 2>/dev/null)
health_status=$(echo "$health_response" | grep -o '"status":"ok"' 2>/dev/null)

if [ -n "$health_status" ]; then
    # Healthy — heartbeat log every 10th check (~30 min) so we know the monitor is alive
    check_count=0
    if [ -f "$STATE_FILE" ]; then
        check_count=$(grep -o 'checks=[0-9]*' "$STATE_FILE" | cut -d= -f2)
        check_count=${check_count:-0}
    fi
    check_count=$((check_count + 1))
    if [ "$consecutive_fails" -gt 0 ]; then
        log "RECOVERED: racecontrol healthy after $consecutive_fails failed checks"
        if [ "$consecutive_fails" -ge 3 ]; then
            send_alert "Server recovered. Racecontrol healthy after $consecutive_fails failed checks."
        fi
    fi
    if [ $((check_count % 10)) -eq 1 ]; then
        log "OK - server .23 healthy (check #$check_count)"
    fi
    consecutive_fails=0
    save_state
    # Append check_count to state
    echo "|checks=${check_count}" >> "$STATE_FILE"
    exit 0
fi

# Health check failed
consecutive_fails=$((consecutive_fails + 1))
log "Health check FAILED - $consecutive_fails consecutive"

# Hysteresis: wait for 3 consecutive failures (9 minutes) before acting
# Bono uses higher threshold because James should handle it first
if [ "$consecutive_fails" -lt 3 ]; then
    log "Waiting for hysteresis - need 3 consecutive failures (James should handle first)"
    save_state
    exit 0
fi

# Rate limit: max 1 restart attempt per 10 minutes
if [ -n "$last_restart" ]; then
    last_epoch=$(date -d "$last_restart" +%s 2>/dev/null || echo 0)
    now_epoch=$(date +%s)
    elapsed=$(( now_epoch - last_epoch ))
    if [ "$elapsed" -lt 600 ]; then
        log "Rate limited: last restart attempt was ${elapsed}s ago - min 10min"
        save_state
        exit 0
    fi
fi

# === ATTEMPT RESTART ===
log "=== BONO RESTART SEQUENCE INITIATED ==="
last_restart=$(date '+%Y-%m-%d %H:%M:%S')
total_restarts=$((total_restarts + 1))
save_state

send_alert "Racecontrol DOWN on server .23 for $((consecutive_fails * 3)) min. James may also be down. Bono attempting restart..."

restarted=false

# Check if James is alive first
james_alive=false
if ping -c 1 -W 3 "$JAMES_TAILSCALE_IP" > /dev/null 2>&1; then
    james_alive=true
    log "James is alive - will try to trigger James monitor"
fi

# Method 1: SSH to server via Tailscale and run schtasks
log "Method 1: SSH to server via Tailscale..."
if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "ADMIN@${SERVER_TAILSCALE_IP}" "schtasks /Run /TN StartRCDirect" 2>/dev/null; then
    log "SSH schtasks restart succeeded"
    restarted=true
fi

# Method 2: SSH to server and start directly
if [ "$restarted" = false ]; then
    log "Method 2: SSH direct start..."
    if ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no -o BatchMode=yes "ADMIN@${SERVER_TAILSCALE_IP}" "cd /d C:\\RacingPoint && start /B racecontrol.exe" 2>/dev/null; then
        log "SSH direct start succeeded"
        restarted=true
    fi
fi

# Method 3: Trigger James to handle it (via comms-link relay)
if [ "$restarted" = false ] && [ "$james_alive" = true ]; then
    log "Method 3: Triggering James health monitor via relay..."
    # Send exec command to James to run the monitor
    curl -s -X POST "http://${JAMES_TAILSCALE_IP}:8766/relay/exec/run" \
        -H "Content-Type: application/json" \
        -d '{"command":"shell","args":["powershell -ExecutionPolicy Bypass -File C:\\RacingPoint\\server-health-monitor.ps1"],"reason":"bono-triggered: racecontrol down on server"}' \
        --connect-timeout 5 --max-time 30 > /dev/null 2>&1
    if [ $? -eq 0 ]; then
        log "James monitor triggered"
        restarted=true
    fi
fi

if [ "$restarted" = true ]; then
    log "Restart command sent. Will verify on next check in 3 min."
    send_alert "Restart command sent to server .23 from Bono VPS. Verifying in 3 min..."
else
    log "ALL RESTART METHODS FAILED - requires physical intervention"
    send_alert "CRITICAL: All restart methods failed for server .23. Both James and server appear unreachable. Physical restart required!"
fi

save_state
