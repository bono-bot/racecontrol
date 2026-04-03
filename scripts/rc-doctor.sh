#!/bin/bash
# RC-Doctor v4.0: Infrastructure health + Venue systems + Unified Protocol diagnostics
# Called by: Monit (on escalation), systemd timer (routine), manual
# Philosophy: Cloud is nothing without local. Check venue FIRST.

set -euo pipefail

# === SERIALIZATION (prevent concurrent runs) ===
exec 200>/var/lock/rc-doctor.lock
flock -n 200 || { echo "Another rc-doctor instance running, exiting"; exit 0; }

LOG="/var/log/rc-doctor.log"
AUDIT="/var/lib/rc-doctor/audit.log"
STATE_DIR="/var/lib/rc-doctor"
DISABLED_FILE="$STATE_DIR/DISABLED"
MAX_ACTIONS_PER_HOUR=10

log() { echo "[$(date -Is)] $*" | tee -a "$LOG"; }
audit() { echo "[$(date -Is)] ACTION=$1 TARGET=$2 RESULT=$3" >> "$AUDIT"; }

# === ENABLE / DISABLE TOGGLE ===
cmd_enable() {
  if [ -f "$DISABLED_FILE" ]; then
    rm -f "$DISABLED_FILE"
    log "RC-Doctor ENABLED (removed $DISABLED_FILE)"
    audit "toggle" "rc-doctor" "enabled"
    echo "RC-Doctor is now ENABLED"
  else
    echo "RC-Doctor is already ENABLED"
  fi
}

cmd_disable() {
  local reason="${1:-manual}"
  mkdir -p "$STATE_DIR"
  echo "disabled_at=$(date -Is) reason=$reason" > "$DISABLED_FILE"
  log "RC-Doctor DISABLED (reason: $reason)"
  audit "toggle" "rc-doctor" "disabled:$reason"
  alert_whatsapp "RC-Doctor DISABLED by operator (reason: $reason). Automated remediation paused." || true
  echo "RC-Doctor is now DISABLED (reason: $reason)"
  echo "Run 'rc-doctor.sh enable' to re-enable."
}

is_enabled() {
  [ ! -f "$DISABLED_FILE" ]
}

check_enabled_or_exit() {
  if [ -f "$DISABLED_FILE" ] && [ "${FORCE:-}" != "1" ]; then
    local disabled_info
    disabled_info=$(cat "$DISABLED_FILE" 2>/dev/null || echo "unknown")
    log "SKIP: RC-Doctor is DISABLED ($disabled_info). Use --force or 'rc-doctor.sh enable' to resume."
    exit 0
  fi
}

# === SAFETY RAILS ===
check_billing_active() {
  local active
  active=$(curl -sf http://localhost:8080/api/v1/billing/active 2>/dev/null | jq '.count // 0')
  [ "${active:-0}" -gt 0 ]
}

check_peak_load() {
  local pods
  pods=$(curl -sf http://localhost:8080/api/v1/fleet/health 2>/dev/null | jq '[.pods[] | select(.ws_connected==true)] | length')
  [ "${pods:-0}" -gt 4 ]
}

check_action_budget() {
  local count
  count=$(grep -c "$(date +%Y-%m-%dT%H)" "$AUDIT" 2>/dev/null || echo 0)
  [ "$count" -lt "$MAX_ACTIONS_PER_HOUR" ]
}

alert_whatsapp() {
  local payload
  payload=$(jq -n --arg txt "RC-Doctor: $1" '{number: "917075778180", text: $txt}') || return 0
  curl -sf -X POST "http://localhost:53622/message/sendText/Racing%20Point%20Reception" \
    -H "apikey: ${EVOLUTION_API_KEY:-}" \
    -H "Content-Type: application/json" \
    -d "$payload" >/dev/null 2>&1 || \
  node /root/racingpoint-google/send-email.js \
    --to "uday@racingpoint.in" \
    --subject "RC-Doctor Alert" \
    --body "$1" 2>/dev/null || true
}

# === VENUE INFRASTRUCTURE MAP (hardcoded — cloud is nothing without local) ===
# Tailscale IPs — work even when venue LAN is flaky
declare -A VENUE_PODS=(
  [pod1]="100.92.122.89"
  [pod2]="100.105.93.108"
  [pod3]="100.69.231.26"
  [pod4]="100.75.45.10"
  [pod5]="100.110.133.87"
  [pod6]="100.127.149.17"
  [pod7]="100.82.196.28"
  [pod8]="100.98.67.67"
)
SERVER_TS="100.125.108.37"    # Server (.23) — Tailscale
SERVER_LAN="192.168.31.23"    # Server (.23) — LAN (fallback if Tailscale down)
POS_TS="100.95.211.1"         # POS terminal — Tailscale
POS_LAN="192.168.31.20"       # POS terminal — LAN (fallback)

# Quick HTTP probe: returns "up:CODE" or "down"
# Only 2xx is considered healthy (P1 fix: 500/502/404 must NOT be "up")
probe() {
  local url="$1"
  local code
  code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 3 "$url" 2>/dev/null) || code="000"
  if [ "$code" -ge 200 ] 2>/dev/null && [ "$code" -lt 300 ] 2>/dev/null; then
    echo "up:$code"
  else
    echo "down:$code"
  fi
}

# Probe with fallback: try primary IP, then fallback IP
# Usage: probe_fallback <primary_ip> <fallback_ip> <port> <path>
# Returns: "up:CODE:primary" or "up:CODE:fallback" or "down"
probe_fallback() {
  local primary="$1" fallback="$2" port="$3" path="$4"
  local result
  result=$(probe "http://$primary:$port$path")
  if [[ "$result" == up* ]]; then
    echo "$result:primary"
    return
  fi
  # Tailscale failed — try LAN
  result=$(probe "http://$fallback:$port$path")
  if [[ "$result" == up* ]]; then
    echo "$result:fallback"
    return
  fi
  echo "down"
}

# Extract field from JSON health response
probe_json() {
  local url="$1" field="$2"
  curl -sf --max-time 3 "$url" 2>/dev/null | jq -r ".$field // \"unknown\"" 2>/dev/null || echo "unknown"
}

# === VENUE HEALTH CHECK ===
cmd_venue() {
  log "VENUE CHECK: scanning all on-site systems"
  local total=0 up=0 down=0 warn_list=""

  echo ""
  echo "╔══════════════════════════════════════════════════════════════╗"
  echo "║  RC-Doctor v4.0 — Venue Health Check                       ║"
  echo "║  Cloud is nothing without local.                            ║"
  echo "╚══════════════════════════════════════════════════════════════╝"
  echo ""

  # ── 1. Server (.23) — the brain ──────────────────────────────────────
  echo "━━━ Server (.23 / TS:$SERVER_TS / LAN:$SERVER_LAN) ━━━"
  local svc_result svc_build server_ip=""

  # racecontrol binary (try Tailscale first, then LAN)
  svc_result=$(probe_fallback "$SERVER_TS" "$SERVER_LAN" 8080 "/api/v1/health")
  total=$((total + 1))
  if [[ "$svc_result" == up* ]]; then
    # Remember which IP worked for subsequent checks
    if [[ "$svc_result" == *":fallback" ]]; then
      server_ip="$SERVER_LAN"
      echo "  ⚠ Tailscale DOWN on server — using LAN fallback"
      warn_list+="server-tailscale "
    else
      server_ip="$SERVER_TS"
    fi
    svc_build=$(probe_json "http://$server_ip:8080/api/v1/health" "build_id")
    echo "  ✓ racecontrol :8080  (build: $svc_build)"
    up=$((up + 1))
  else
    echo "  ✗ racecontrol :8080  DOWN (both Tailscale + LAN unreachable)"
    warn_list+="server-racecontrol "
    down=$((down + 1))
  fi

  # Next.js apps on server (use whichever IP worked, or try both)
  local check_ip="${server_ip:-$SERVER_TS}"
  # Next.js apps: kiosk has basePath=/kiosk, others use /api/health directly
  for app_entry in "admin:3201:/api/health" "web:3200:/api/health" "kiosk:3300:/kiosk/api/health"; do
    local app_name="${app_entry%%:*}"
    local app_rest="${app_entry#*:}"
    local app_port="${app_rest%%:*}"
    local app_path="${app_rest#*:}"
    if [ -n "$server_ip" ]; then
      svc_result=$(probe "http://$server_ip:$app_port$app_path")
    else
      svc_result=$(probe_fallback "$SERVER_TS" "$SERVER_LAN" "$app_port" "$app_path")
    fi
    total=$((total + 1))
    if [[ "$svc_result" == up* ]]; then
      echo "  ✓ $app_name :$app_port"
      up=$((up + 1))
    else
      echo "  ✗ $app_name :$app_port  DOWN"
      warn_list+="server-$app_name "
      down=$((down + 1))
    fi
  done
  echo ""

  # ── 2. POS Terminal ──────────────────────────────────────────────────
  echo "━━━ POS Terminal (TS:$POS_TS / LAN:$POS_LAN) ━━━"
  local pos_ip=""
  svc_result=$(probe_fallback "$POS_TS" "$POS_LAN" 8090 "/health")
  total=$((total + 1))
  if [[ "$svc_result" == up* ]]; then
    [[ "$svc_result" == *":fallback" ]] && pos_ip="$POS_LAN" || pos_ip="$POS_TS"
    [[ "$svc_result" == *":fallback" ]] && echo "  ⚠ Tailscale DOWN on POS — using LAN fallback"
    svc_build=$(probe_json "http://$pos_ip:8090/health" "build_id")
    echo "  ✓ rc-pos-agent :8090  (build: $svc_build)"
    up=$((up + 1))
  else
    # Check if machine is reachable at all (try both networks)
    local pos_alive="000"
    pos_alive=$(curl -s -o /dev/null -w "%{http_code}" --max-time 2 "http://$POS_TS:3200/" 2>/dev/null) || pos_alive="000"
    [ "$pos_alive" = "000" ] && pos_alive=$(curl -s -o /dev/null -w "%{http_code}" --max-time 2 "http://$POS_LAN:3200/" 2>/dev/null) || true
    if [ "$pos_alive" != "000" ]; then
      echo "  ✗ rc-pos-agent :8090  DOWN (machine reachable — agent not running)"
    else
      echo "  ✗ rc-pos-agent :8090  DOWN (machine unreachable on both TS + LAN)"
    fi
    warn_list+="pos-agent "
    down=$((down + 1))
  fi

  # POS kiosk: Edge browser loads from server :3300 (no local web server).
  # Check if msedge.exe is running via rc-agent exec API (no SSH keys needed).
  local pos_exec="${pos_ip:-$POS_TS}"
  local edge_payload edge_check
  edge_payload=$(jq -n '{cmd: "tasklist /NH", reason: "rc-doctor"}')
  edge_check=$(curl -sf --max-time 5 -X POST "http://$pos_exec:8090/exec" \
    -H "Content-Type: application/json" \
    -d "$edge_payload" 2>/dev/null) || edge_check=""
  total=$((total + 1))
  if echo "$edge_check" | jq -r '.stdout // ""' 2>/dev/null | grep -qi "msedge" 2>/dev/null; then
    echo "  ✓ kiosk-display (Edge running, renders server :3300)"
    up=$((up + 1))
  else
    echo "  ✗ kiosk-display  DOWN (Edge not running on POS)"
    warn_list+="pos-kiosk-display "
    down=$((down + 1))
  fi
  echo ""

  # ── 3. Pods (1-8) — the fleet ───────────────────────────────────────
  echo "━━━ Pods (Fleet) ━━━"
  local pod_up=0 pod_down=0 pod_details=""

  for pod_name in pod1 pod2 pod3 pod4 pod5 pod6 pod7 pod8; do
    local pod_ip="${VENUE_PODS[$pod_name]}"
    svc_result=$(probe "http://$pod_ip:8090/health")
    total=$((total + 1))
    if [[ "$svc_result" == up* ]]; then
      local pod_build pod_ws
      pod_build=$(probe_json "http://$pod_ip:8090/health" "build_id")
      pod_ws=$(probe_json "http://$pod_ip:8090/health" "ws_connected")
      local ws_icon="↗"
      [ "$pod_ws" = "true" ] && ws_icon="⚡" || ws_icon="○"
      echo "  ✓ $pod_name ($pod_ip)  build:$pod_build  ws:$ws_icon"
      up=$((up + 1))
      pod_up=$((pod_up + 1))
    else
      echo "  ✗ $pod_name ($pod_ip)  DOWN"
      warn_list+="$pod_name "
      down=$((down + 1))
      pod_down=$((pod_down + 1))
    fi
  done
  echo "  Fleet: $pod_up/8 pods responding"
  echo ""

  # ── 4. Cross-check: server fleet health vs actual ────────────────────
  echo "━━━ Cross-Check ━━━"
  local fleet_api="${server_ip:-$SERVER_TS}"
  local fleet_json fleet_count ws_count
  fleet_json=$(curl -sf --max-time 3 "http://$fleet_api:8080/api/v1/fleet/health" 2>/dev/null) || fleet_json=""
  if [ -n "$fleet_json" ]; then
    fleet_count=$(echo "$fleet_json" | jq '.pods | length' 2>/dev/null) || fleet_count="?"
    ws_count=$(echo "$fleet_json" | jq '[.pods[] | select(.ws_connected==true)] | length' 2>/dev/null) || ws_count="?"
  else
    fleet_count="?"
    ws_count="?"
  fi
  echo "  Server sees: $fleet_count pods registered, $ws_count WebSocket connected"
  echo "  Doctor sees: $pod_up/8 pods with healthy :8090"
  if [ "$ws_count" != "?" ] && [ "$pod_up" -gt 0 ] && [ "$ws_count" != "$pod_up" ]; then
    echo "  ⚠ MISMATCH: server WS count ($ws_count) ≠ doctor probe count ($pod_up)"
  fi
  echo ""

  # ── 4b. WebSocket Health (from fleet/health API) ─────────────────────
  echo "━━━ WebSocket Connections ━━━"
  if [ -n "$fleet_json" ]; then
    # Dashboard WS churn metrics
    local ws_connects ws_disconnects ws_healthy ws_clients
    ws_connects=$(echo "$fleet_json" | jq '.dashboard_ws_churn.connects_per_min // "?"' 2>/dev/null)
    ws_disconnects=$(echo "$fleet_json" | jq '.dashboard_ws_churn.disconnects_per_min // "?"' 2>/dev/null)
    ws_healthy=$(echo "$fleet_json" | jq '.dashboard_ws_churn.healthy // false' 2>/dev/null)
    ws_clients=$(echo "$fleet_json" | jq '.dashboard_clients // 0' 2>/dev/null)

    if [ "$ws_healthy" = "true" ]; then
      echo "  ✓ Dashboard WS churn: ${ws_connects}/min connects, ${ws_disconnects}/min disconnects (healthy)"
    else
      echo "  ✗ Dashboard WS churn: ${ws_connects}/min connects, ${ws_disconnects}/min disconnects (UNHEALTHY — stale frontend?)"
    fi
    echo "  ℹ Dashboard clients: $ws_clients"

    # Per-pod WS reconnects
    echo "  ── Per-Pod WS Stability ──"
    local pod_ws_issues=0
    echo "$fleet_json" | jq -r '.pods[] | select(.pod_number <= 8) | "\(.pod_number)|\(.ws_connected)|\(.ws_reconnects_5m // 0)|\(.ws_reconnect_count // 0)"' 2>/dev/null | \
    while IFS='|' read -r pnum pws precon5 precontotal; do
      local ws_status="✓"
      local detail=""
      if [ "$pws" != "true" ]; then
        ws_status="✗"
        detail=" WS_DISCONNECTED"
        pod_ws_issues=$((pod_ws_issues + 1))
      fi
      if [ "${precon5:-0}" -ge 3 ] 2>/dev/null; then
        ws_status="⚠"
        detail=" UNSTABLE(${precon5} reconnects/5min)"
        pod_ws_issues=$((pod_ws_issues + 1))
      fi
      printf "    %s Pod %-2s  ws:%s  reconnects_5m:%-3s  lifetime:%-3s%s\n" \
        "$ws_status" "$pnum" "$pws" "${precon5:-0}" "${precontotal:-0}" "$detail"
    done
  else
    echo "  ⚠ Fleet health API unreachable — cannot check WS metrics"
  fi
  echo ""

  # ── 5. Cloud services (Bono VPS — self) ─────────────────────────────
  echo "━━━ Cloud (Bono VPS — self) ━━━"
  local cloud_ok=0 cloud_total=0
  for svc_pair in "racecontrol-cloud:8080" "comms-link:8765" "evolution-api:53622"; do
    local svc_name="${svc_pair%%:*}" svc_port="${svc_pair##*:}"
    cloud_total=$((cloud_total + 1))
    total=$((total + 1))
    if curl -sf --max-time 2 "http://localhost:$svc_port" >/dev/null 2>&1 || \
       curl -sf --max-time 2 "http://localhost:$svc_port/health" >/dev/null 2>&1; then
      echo "  ✓ $svc_name :$svc_port"
      cloud_ok=$((cloud_ok + 1))
      up=$((up + 1))
    else
      echo "  ✗ $svc_name :$svc_port  DOWN"
      warn_list+="cloud-$svc_name "
      down=$((down + 1))
    fi
  done
  echo "  Cloud: $cloud_ok/$cloud_total services"
  echo ""

  # ── Summary ──────────────────────────────────────────────────────────
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  if [ "$down" -eq 0 ]; then
    echo "  ALL SYSTEMS OPERATIONAL  ($up/$total checks passed)"
    audit "venue" "all" "ALL_PASS_${up}_of_${total}"
  else
    echo "  ⚠ $down SYSTEM(S) DOWN  ($up/$total up, $down/$total down)"
    echo "  Down: $warn_list"
    audit "venue" "fleet" "FAILURES_${down}_of_${total}:${warn_list}"
    # Alert if critical systems are down (with 15min cooldown to prevent fatigue)
    if echo "$warn_list" | grep -qE "server-racecontrol|pos-agent"; then
      local venue_cooldown="$STATE_DIR/venue_alert_cooldown"
      local should_alert=1
      if [ -f "$venue_cooldown" ]; then
        local last_age
        last_age=$(( $(date +%s) - $(stat -c%Y "$venue_cooldown") ))
        [ "$last_age" -lt 900 ] && should_alert=0
      fi
      if [ "$should_alert" -eq 1 ]; then
        alert_whatsapp "VENUE ALERT: Critical system(s) down — $warn_list ($up/$total healthy)" || true
        touch "$venue_cooldown"
      fi
    else
      # Clear cooldown when critical systems recover
      rm -f "$STATE_DIR/venue_alert_cooldown" 2>/dev/null || true
    fi
  fi
  echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
  echo ""

  log "VENUE CHECK: $up/$total up, $down down. ${warn_list:-all clear}"
  # Store result for callers (return 0/1 only — safe with set -e)
  VENUE_DOWN="$down"
  VENUE_UP="$up"
  VENUE_TOTAL="$total"
  [ "$down" -eq 0 ] && return 0 || return 1
}

# === PLAYBOOKS ===

cmd_disk_pressure() {
  check_enabled_or_exit
  log "PLAYBOOK: disk-pressure"
  local before after freed
  before=$(df / --output=pcent | tail -1 | tr -d '% ')

  pm2 flush >/dev/null 2>&1 || true

  if [ -d /root/racecontrol/target ] && [ "$(du -sm /root/racecontrol/target | cut -f1)" -gt 500 ]; then
    cargo clean --manifest-path /root/racecontrol/Cargo.toml 2>/dev/null || true
  fi

  find /root/racingpoint-*/ -name ".next" -path "*/cache/*" -type d -exec rm -rf {} + 2>/dev/null || true
  find /root -maxdepth 1 -name "*.log" -size +50M -exec truncate -s 10M {} \; 2>/dev/null || true

  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db; do
    [ -f "$db" ] && sqlite3 "$db" "PRAGMA wal_checkpoint(TRUNCATE); VACUUM;" 2>/dev/null || true
  done

  after=$(df / --output=pcent | tail -1 | tr -d '% ')
  freed=$((before - after))
  log "Disk: ${before}% -> ${after}% (freed ${freed}%)"
  audit "disk-pressure" "rootfs" "freed_${freed}_pct"

  [ "$freed" -lt 5 ] && alert_whatsapp "Disk pressure: cleaned ${freed}% but still at ${after}%. Manual intervention needed."
}

cmd_memory_pressure() {
  check_enabled_or_exit
  log "PLAYBOOK: memory-pressure"

  local top_proc top_mem
  top_proc=$(pm2 jlist 2>/dev/null | jq -r 'sort_by(.monit.memory) | last | .name // "none"')
  top_mem=$(pm2 jlist 2>/dev/null | jq -r 'sort_by(.monit.memory) | last | .monit.memory / 1048576 | floor')

  if [ "${top_mem:-0}" -gt 300 ]; then
    log "Restarting $top_proc (${top_mem}MB - likely memory leak)"
    pm2 restart "$top_proc" 2>/dev/null || true
    audit "memory-pressure" "$top_proc" "restarted_${top_mem}MB"
  fi

  # MMA-C10: Removed system-wide cache drop — affects ALL processes on VPS
  # sync && echo 3 > /proc/sys/vm/drop_caches — too aggressive for production
  # Instead, only restart the offending service (already done above)
  audit "memory-pressure" "system" "service_restart_only"
}

cmd_crash_loop() {
  check_enabled_or_exit
  local service="${1:-unknown}"
  log "PLAYBOOK: crash-loop for $service"

  if ! check_action_budget; then
    log "SKIP: Action budget exhausted for this hour"
    alert_whatsapp "$service crash loop but action budget exhausted. Manual check needed."
    return 0
  fi

  if check_billing_active; then
    log "SKIP: Active billing sessions - deferring restart of $service"
    alert_whatsapp "$service in crash loop but billing is active. Deferring."
    return 0
  fi

  case "$service" in
    racingpoint-hiring|racingpoint-website|racingpoint-website-api)
      if check_peak_load; then
        log "SKIP: Peak load - deferring non-critical $service"
        return 0
      fi
      ;;
  esac

  local port=""
  case "$service" in
    racecontrol)             port=8080 ;;
    racecontrol-pwa)         port=3500 ;;
    racingpoint-admin)       port=3201 ;;
    racingpoint-dashboard)   port=3400 ;;
    racingpoint-api-gateway) port=3100 ;;
    racingpoint-bot)         port=3000 ;;
    *) port="" ;;
  esac

  if [ -n "$port" ]; then
    local stale_pid
    stale_pid=$(lsof -ti ":$port" 2>/dev/null | head -1)
    if [ -n "$stale_pid" ]; then
      local stale_name
      stale_name=$(ps -p "$stale_pid" -o comm= 2>/dev/null || echo "unknown")
      log "Port $port held by PID $stale_pid ($stale_name) - killing"
      kill "$stale_pid" 2>/dev/null; sleep 2; kill -9 "$stale_pid" 2>/dev/null || true
      audit "crash-loop" "$service" "killed_stale_port_${port}"
    fi
  fi

  local svc_dir="/root/${service}"
  if [ -d "$svc_dir" ] && [ -f "$svc_dir/package.json" ] && [ ! -d "$svc_dir/node_modules" ]; then
    log "Missing node_modules for $service - reinstalling"
    (cd "$svc_dir" && npm install --production) 2>&1 | tail -5 >> "$LOG"
    audit "crash-loop" "$service" "npm_install"
  fi

  if [ "$service" = "racecontrol" ]; then
    cmd_stale_binary
    return
  fi

  log "Restarting $service with 10s delay"
  pm2 stop "$service" 2>/dev/null || true
  sleep 10
  pm2 start "$service" 2>/dev/null || true
  audit "crash-loop" "$service" "delayed_restart"

  sleep 15
  if ! pm2 show "$service" 2>/dev/null | grep -q "online"; then
    alert_whatsapp "$service crash loop - auto-fix failed. Manual intervention needed."
    audit "crash-loop" "$service" "ESCALATED"
  fi
}

cmd_stale_binary() {
  check_enabled_or_exit
  log "PLAYBOOK: stale-binary"

  local build_id git_head
  build_id=$(curl -sf http://localhost:8080/api/v1/health | jq -r '.build_id // "unknown"')
  git_head=$(git -C /root/racecontrol rev-parse --short HEAD 2>/dev/null || echo "unknown")

  if [ "$build_id" = "$git_head" ]; then
    log "Binary is current ($build_id)"
    return 0
  fi

  local free_gb
  free_gb=$(df / --output=avail | tail -1 | awk '{print int($1/1048576)}')
  if [ "$free_gb" -lt 3 ]; then
    log "SKIP: Only ${free_gb}GB free - not enough for cargo build"
    alert_whatsapp "Stale binary detected but only ${free_gb}GB disk free. Clean up first."
    return 1
  fi

  local bin_path="/root/racecontrol/target/release/racecontrol"
  if [ -f "$bin_path" ]; then
    cp "$bin_path" "${bin_path}.bak"
    log "Backed up current binary to ${bin_path}.bak"
  fi

  # MMA-C11: Guard against building during peak hours or active billing
  if check_billing_active; then
    log "SKIP: Active billing sessions — deferring stale-binary rebuild"
    alert_whatsapp "Stale binary detected (running=$build_id, git=$git_head) but billing active. Deferring rebuild."
    audit "stale-binary" "racecontrol" "DEFERRED_BILLING_ACTIVE"
    return 0
  fi
  if check_peak_load; then
    log "SKIP: Peak load — deferring stale-binary rebuild"
    audit "stale-binary" "racecontrol" "DEFERRED_PEAK_LOAD"
    return 0
  fi

  log "Rebuilding: binary=$build_id, git=$git_head (nice -n 19 to limit CPU impact)"
  if ! (cd /root/racecontrol && nice -n 19 cargo build --release 2>&1 | tail -10) >> "$LOG"; then
    log "BUILD FAILED - restoring backup binary"
    [ -f "${bin_path}.bak" ] && mv "${bin_path}.bak" "$bin_path"
    alert_whatsapp "Cargo build failed for racecontrol. Restored backup. Manual fix needed."
    audit "stale-binary" "racecontrol" "BUILD_FAILED_ROLLED_BACK"
    return 1
  fi

  pm2 restart racecontrol 2>/dev/null || true
  sleep 10

  local new_build
  new_build=$(curl -sf http://localhost:8080/api/v1/health | jq -r '.build_id // "unknown"')
  if [ "$new_build" = "unknown" ] || [ "$new_build" = "$build_id" ]; then
    log "REBUILD UNHEALTHY - restoring backup"
    pm2 stop racecontrol 2>/dev/null || true
    [ -f "${bin_path}.bak" ] && mv "${bin_path}.bak" "$bin_path"
    pm2 start racecontrol 2>/dev/null || true
    alert_whatsapp "Rebuilt racecontrol but health check failed. Rolled back to previous binary."
    audit "stale-binary" "racecontrol" "HEALTH_FAILED_ROLLED_BACK"
  else
    rm -f "${bin_path}.bak"
    log "Rebuild complete: $build_id -> $new_build"
    audit "stale-binary" "racecontrol" "rebuilt_${build_id}_to_${new_build}"
  fi
}

cmd_wal_bloat() {
  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db /root/comms-link/data/*.db; do
    [ -f "$db" ] || continue
    local wal="${db}-wal"
    if [ -f "$wal" ]; then
      local size_mb
      size_mb=$(du -m "$wal" 2>/dev/null | cut -f1)
      if [ "${size_mb:-0}" -gt 50 ]; then
        log "WAL bloat: $wal is ${size_mb}MB - checkpointing"
        sqlite3 "$db" "PRAGMA wal_checkpoint(TRUNCATE);" 2>/dev/null || true
        audit "wal-bloat" "$db" "checkpointed_${size_mb}MB"
      fi
    fi
  done
}

cmd_ssl_check() {
  log "PLAYBOOK: ssl-check"
  local days
  days=$(echo | openssl s_client -connect app.racingpoint.cloud:443 -servername app.racingpoint.cloud 2>/dev/null \
    | openssl x509 -noout -enddate 2>/dev/null \
    | cut -d= -f2 \
    | xargs -I{} bash -c 'echo $(( ($(date -d "{}" +%s) - $(date +%s)) / 86400 ))' 2>/dev/null) || days=999

  if [ "${days:-999}" -lt 14 ]; then
    log "SSL cert expires in ${days} days - renewing"
    certbot renew --quiet 2>&1 | tail -3 >> "$LOG"
    nginx -s reload 2>/dev/null || true
    audit "ssl-check" "certs" "renewed_${days}_days_left"
  fi
}

cmd_db_integrity() {
  log "PLAYBOOK: db-integrity"
  for db in /root/racecontrol/racecontrol.db /root/racingpoint-*/data/*.db; do
    [ -f "$db" ] || continue
    local result
    result=$(sqlite3 "$db" "PRAGMA integrity_check;" 2>/dev/null | head -1)
    if [ "$result" != "ok" ]; then
      log "DB INTEGRITY FAIL: $db - $result"
      alert_whatsapp "Database integrity check FAILED for $db: $result"
      audit "db-integrity" "$db" "FAILED_${result}"
    fi
  done
}

cmd_backup_verify() {
  log "PLAYBOOK: backup-verify"
  local backup_dir="/var/backups/racingpoint"
  local today
  today=$(date +%Y%m%d)

  if [ ! -d "$backup_dir" ]; then
    alert_whatsapp "Backup directory $backup_dir does not exist!"
    return 1
  fi

  local found=0
  for f in "$backup_dir"/*"$today"*; do
    [ -f "$f" ] && found=1 && break
  done

  if [ "$found" -eq 0 ]; then
    log "No backup found for today ($today)"
    alert_whatsapp "Daily backup missing for $today. Check backup-databases.sh."
    audit "backup-verify" "daily" "MISSING"
  fi
}

cmd_routine() {
  check_enabled_or_exit

  # Venue first — cloud is nothing without local
  cmd_venue || true

  cmd_wal_bloat

  if ! curl -sf http://localhost:53622 >/dev/null 2>&1; then
    log "Evolution API unreachable - restarting container"
    docker restart evolution-api 2>/dev/null || true
    audit "routine" "evolution-api" "docker_restart"
  fi

  local minute
  minute=$(date +%M)
  if [ "$minute" = "00" ]; then
    cmd_db_integrity
    cmd_backup_verify
    cmd_ssl_check
  fi
}

cmd_healthcheck() {
  log "PLAYBOOK: healthcheck"
  if [ -x /root/bin/rp-healthcheck ]; then
    /root/bin/rp-healthcheck "${2:---quick}" 2>&1 | tee -a "$LOG"
    local exit_code=${PIPESTATUS[0]}
    if [ "$exit_code" -ne 0 ]; then
      audit "healthcheck" "infrastructure" "FAILURES_DETECTED"
    else
      audit "healthcheck" "infrastructure" "ALL_PASS"
    fi
    return "$exit_code"
  else
    echo "rp-healthcheck not installed — running venue check instead"
    cmd_venue
  fi
}

cmd_logs() {
  tail -f /var/log/rc-doctor.log /var/log/monit.log /root/.pm2/logs/*.log 2>/dev/null
}

cmd_status() {
  echo "=== RC-Doctor v4.0 Status ==="
  if is_enabled; then
    echo "State: ENABLED"
  else
    echo "State: DISABLED ($(cat "$DISABLED_FILE" 2>/dev/null || echo 'unknown'))"
  fi
  echo "Last run: $(stat -c %y /var/lock/rc-doctor.lock 2>/dev/null || echo 'never')"
  echo "Actions this hour: $(grep -c "$(date +%Y-%m-%dT%H)" "$AUDIT" 2>/dev/null || echo 0)/$MAX_ACTIONS_PER_HOUR"
  echo ""

  # Venue health — the most important section
  echo "=== Venue Systems (Local + On-Site) ==="
  cmd_venue || true
  echo ""

  echo "=== PM2 Status (Cloud) ==="
  pm2 list 2>/dev/null || echo "PM2 not available"
  echo ""
  echo "=== Recent Actions ==="
  tail -20 "$AUDIT" 2>/dev/null || echo "No actions yet"
}

# === DISPATCH ===
mkdir -p /var/lib/rc-doctor

[[ "${*}" == *"--force"* ]] && export FORCE=1

case "${1:-routine}" in
  enable)           cmd_enable ;;
  disable)          cmd_disable "${2:-manual}" ;;
  venue|local)      cmd_venue ;;
  healthcheck|hc)   cmd_healthcheck "$@" ;;
  disk-pressure)    cmd_disk_pressure ;;
  memory-pressure)  cmd_memory_pressure ;;
  crash-loop)       cmd_crash_loop "${2:-}" ;;
  stale-binary)     cmd_stale_binary ;;
  wal-bloat)        cmd_wal_bloat ;;
  ssl-check)        cmd_ssl_check ;;
  db-integrity)     cmd_db_integrity ;;
  backup-verify)    cmd_backup_verify ;;
  routine)          cmd_routine ;;
  logs)             cmd_logs ;;
  status)           cmd_status ;;
  *)                echo "RC-Doctor v4.0 — Cloud is nothing without local."
                    echo ""
                    echo "Usage: rc-doctor.sh <command> [args]"
                    echo ""
                    echo "  Venue (LOCAL — check these first):"
                    echo "    venue / local              Check all on-site systems (server, 8 pods, POS)"
                    echo ""
                    echo "  Toggle:"
                    echo "    enable / disable [reason]"
                    echo ""
                    echo "  Cloud Playbooks:"
                    echo "    disk-pressure              Clean disk (logs, caches, WAL)"
                    echo "    memory-pressure            Restart leaky process + drop caches"
                    echo "    crash-loop <service>       Fix crash-looping PM2 service"
                    echo "    stale-binary               Rebuild + deploy racecontrol if stale"
                    echo "    wal-bloat                  Checkpoint bloated SQLite WALs"
                    echo "    ssl-check                  Renew SSL if < 14 days"
                    echo "    db-integrity               PRAGMA integrity_check on all DBs"
                    echo "    backup-verify              Verify today's backup exists"
                    echo ""
                    echo "  Info:"
                    echo "    status                     Venue + cloud state + PM2 + recent actions"
                    echo "    logs                       Tail all log files"
                    echo "    routine                    Run scheduled maintenance (includes venue check)"
                    echo ""
                    echo "  Flags:"
                    echo "    --force                    Bypass disabled state"
                    ;;
esac
