#!/bin/bash
# =============================================================================
# Deploy Verification Master Script
#
# Validates that a deploy succeeded:
#   - Binary was swapped and is non-empty (DEPL-01)
#   - Ports are free and services are serving (DEPL-01)
#   - All 8 pods reconnected via WebSocket (DEPL-02)
#   - build_id is consistent across fleet (DEPL-02)
#   - installed_games is non-empty on canary pod (DEPL-02)
#   - Failures are routed to AI debugger log (DEPL-04)
#
# Usage:
#   bash tests/e2e/deploy/verify.sh
#   RC_BASE_URL=http://192.168.31.23:8080/api/v1 TEST_POD_ID=pod-8 bash tests/e2e/deploy/verify.sh
#
# Environment:
#   RC_BASE_URL   — racecontrol API base (default: http://192.168.31.23:8080/api/v1)
#   TEST_POD_ID   — canary pod for binary/sentry checks (default: pod-8)
#   RESULTS_DIR   — where to write AI debugger log (default: tests/e2e/results/)
# =============================================================================

set -uo pipefail

BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod-8}"
SERVER_IP="192.168.31.23"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)

# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"
# shellcheck source=../lib/pod-map.sh
source "$SCRIPT_DIR/../lib/pod-map.sh"

POD_IP=$(pod_ip "${POD_ID}")
RESULTS_DIR="${RESULTS_DIR:-$(dirname "$SCRIPT_DIR")/results}"
AI_LOG="${RESULTS_DIR}/ai-debugger-input.log"

# Ensure results directory exists
mkdir -p "${RESULTS_DIR}"

# ─── Helper: log_to_ai_debugger ────────────────────────────────────────────
# Args: gate_name failure_message
# Appends a timestamped failure line to $AI_LOG for AI debugger consumption.
log_to_ai_debugger() {
    local gate_name="$1"
    local failure_msg="$2"
    local ts
    ts=$(date -u +"%Y-%m-%d %H:%M:%S")
    echo "[${ts}] GATE: ${gate_name} | FAILURE: ${failure_msg}" >> "${AI_LOG}"
}

echo "========================================"
echo "Deploy Verification (DEPL-01 / DEPL-02 / DEPL-04)"
echo "Base URL  : ${BASE_URL}"
echo "Canary pod: ${POD_ID} (${POD_IP})"
echo "AI log    : ${AI_LOG}"
echo "========================================"
echo ""

# ─── Gate 0: Server Health (pre-check) ────────────────────────────────────
echo "--- Gate 0: Server Health (pre-check) ---"

HEALTH=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH" = "UNREACHABLE" ] || [ -z "$HEALTH" ]; then
    fail "racecontrol unreachable at ${BASE_URL}"
    log_to_ai_debugger "server_health" "racecontrol unreachable at ${BASE_URL}"
    echo ""
    echo "Cannot proceed — server is down. Check racecontrol.exe on ${SERVER_IP}."
    summary_exit
fi
pass "racecontrol responding at ${BASE_URL}"

# Cache fleet/health response — used by Gates 5, 6, 7 to avoid duplicate calls
FLEET_RESP=""

# ─── Gate 1: rc-sentry reachable on canary pod (DEPL-01) ──────────────────
echo ""
echo "--- Gate 1: rc-sentry reachable on canary pod (DEPL-01) ---"

SENTRY_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${POD_IP}:8091/ping" 2>/dev/null || echo "000")
if [ "$SENTRY_CODE" = "200" ]; then
    pass "rc-sentry :8091 reachable on ${POD_ID} (${POD_IP})"
else
    fail "rc-sentry :8091 not reachable on ${POD_ID} (HTTP ${SENTRY_CODE}) — deploy verification channel unavailable"
    log_to_ai_debugger "sentry_reachable" "rc-sentry :8091 on ${POD_IP} returned HTTP ${SENTRY_CODE} — pod may not have received new binary or sentry crashed"
fi

# ─── Gate 2: Binary size check on canary pod (DEPL-01) ────────────────────
echo ""
echo "--- Gate 2: Binary size check on canary pod (DEPL-01) ---"

BINARY_RESP=$(curl -s --max-time 10 -X POST "http://${POD_IP}:8091/exec" \
    -H "Content-Type: application/json" \
    -d "{\"cmd\": \"powershell -Command (Get-Item C:/RacingPoint/rc-agent.exe).Length\"}" 2>/dev/null || echo "")

BINARY_SIZE=$(echo "$BINARY_RESP" | python3 -c "
import sys, json, re
try:
    data = json.load(sys.stdin)
    # rc-sentry /exec returns stdout or output field
    out = data.get('stdout', data.get('output', data.get('result', '')))
    # Extract the first numeric value from output
    m = re.search(r'(\d+)', str(out))
    print(m.group(1) if m else '0')
except:
    # If not JSON, try extracting number from raw response
    m = re.search(r'(\d+)', str(sys.stdin.read()))
    print(m.group(1) if m else '0')
" 2>/dev/null || echo "0")

if [ "${BINARY_SIZE:-0}" -gt 0 ] 2>/dev/null; then
    pass "rc-agent.exe binary present on ${POD_ID}: ${BINARY_SIZE} bytes"
    info "Binary size: ${BINARY_SIZE} bytes"
else
    fail "rc-agent.exe binary size is 0 or unreadable on ${POD_ID} (response: ${BINARY_RESP})"
    log_to_ai_debugger "binary_size" "rc-agent.exe on ${POD_ID} (${POD_IP}) has size 0 or could not be read — binary may not have been deployed. Sentry response: ${BINARY_RESP}"
fi

# ─── Gate 3: Port conflict detection — kiosk :3300 (DEPL-01) ──────────────
echo ""
echo "--- Gate 3: Port conflict detection — kiosk :3300 (DEPL-01) ---"

KIOSK_PORT_OK=false
# Kiosk binds to 127.0.0.1:3300 (loopback only) — check via racecontrol proxy at :8080/kiosk
KIOSK_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:8080/kiosk" 2>/dev/null || echo "000")

if [ "$KIOSK_CODE" = "200" ] || [ "$KIOSK_CODE" = "307" ] || [ "$KIOSK_CODE" = "308" ]; then
    pass "Kiosk serving via :8080/kiosk proxy (HTTP ${KIOSK_CODE})"
    KIOSK_PORT_OK=true
elif [ "$KIOSK_CODE" = "000" ]; then
    info "Kiosk :3300 not responding (HTTP 000) — checking if port is stuck (EADDRINUSE detection)"
    # Poll up to 30s (6 attempts x 5s) for :3300 to either serve or be confirmed free
    POLL_ATTEMPT=0
    POLL_MAX=6
    while [ "$POLL_ATTEMPT" -lt "$POLL_MAX" ]; do
        POLL_ATTEMPT=$((POLL_ATTEMPT + 1))
        info "Poll ${POLL_ATTEMPT}/${POLL_MAX}: waiting 5s for :3300..."
        sleep 5
        POLL_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:3300/" 2>/dev/null || echo "000")
        if [ "$POLL_CODE" = "200" ] || [ "$POLL_CODE" = "307" ] || [ "$POLL_CODE" = "308" ]; then
            pass "Kiosk :3300 recovered after ${POLL_ATTEMPT} poll(s) (HTTP ${POLL_CODE})"
            KIOSK_PORT_OK=true
            break
        elif [ "$POLL_CODE" = "000" ]; then
            info "Poll ${POLL_ATTEMPT}/${POLL_MAX}: :3300 still not serving (EADDRINUSE may be in progress)"
        else
            info "Poll ${POLL_ATTEMPT}/${POLL_MAX}: :3300 returned HTTP ${POLL_CODE}"
        fi
    done
    if [ "$KIOSK_PORT_OK" != "true" ]; then
        fail "Kiosk :3300 not serving after 30s poll — EADDRINUSE or kiosk process crashed"
        log_to_ai_debugger "kiosk_port_3300" "Kiosk :3300 on ${SERVER_IP} did not respond after 30s polling. Possible EADDRINUSE — old Node.js process holding port, or kiosk failed to start after deploy. Run: netstat -ano | findstr :3300"
    fi
else
    fail "Kiosk :3300 returned unexpected HTTP ${KIOSK_CODE}"
    log_to_ai_debugger "kiosk_port_3300" "Kiosk :3300 on ${SERVER_IP} returned unexpected HTTP ${KIOSK_CODE}"
fi

# ─── Gate 4: racecontrol :8080 serving (DEPL-01) ──────────────────────────
echo ""
echo "--- Gate 4: racecontrol :8080 serving after restart (DEPL-01) ---"

# Re-check health to confirm server is still live after any deploy/restart
HEALTH2=$(curl -s --max-time 5 "${BASE_URL}/health" 2>/dev/null || echo "UNREACHABLE")
if [ "$HEALTH2" = "UNREACHABLE" ] || [ -z "$HEALTH2" ]; then
    fail "racecontrol :8080 not serving — may have crashed during deploy restart"
    log_to_ai_debugger "racecontrol_health" "racecontrol :8080 on ${SERVER_IP} is not responding after deploy. Check start-racecontrol.bat and HKLM Run key. Binary may have crashed on startup."
else
    # Validate response contains valid JSON or known health string
    HEALTH_VALID=$(echo "$HEALTH2" | python3 -c "
import sys, json
resp = sys.stdin.read().strip()
try:
    data = json.loads(resp)
    # Accept any JSON response as valid
    print('JSON_OK')
except:
    # If not JSON, check for known health string patterns
    if resp and len(resp) > 0:
        print('TEXT_OK')
    else:
        print('EMPTY')
" 2>/dev/null || echo "PARSE_ERROR")
    if [ "$HEALTH_VALID" = "JSON_OK" ] || [ "$HEALTH_VALID" = "TEXT_OK" ]; then
        pass "racecontrol :8080 serving valid health response"
    else
        fail "racecontrol :8080 returned empty or unparseable health response: ${HEALTH2}"
        log_to_ai_debugger "racecontrol_health" "racecontrol :8080 health endpoint returned invalid response: ${HEALTH2}"
    fi
fi

# ─── Gates 5-7 depend on fleet/health — fetch once ────────────────────────
echo ""
echo "--- Fetching /fleet/health for Gates 5-7 ---"
FLEET_RESP=$(curl -s --max-time 10 "${BASE_URL}/fleet/health" 2>/dev/null || echo "")
if [ -z "$FLEET_RESP" ]; then
    fail "fleet/health endpoint returned empty response — cannot run Gates 5, 6, 7"
    log_to_ai_debugger "fleet_health_fetch" "GET ${BASE_URL}/fleet/health returned empty response — racecontrol may be unresponsive or fleet endpoint missing"
    summary_exit
fi
info "fleet/health response received (${#FLEET_RESP} bytes)"

# ─── Gate 5: Fleet health — all 8 pods WS connected (DEPL-02) ────────────
echo ""
echo "--- Gate 5: Fleet WS connectivity — all 8 pods (DEPL-02) ---"

FLEET_RESULT=$(echo "$FLEET_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    connected = []
    disconnected = []
    for p in pods:
        num = p.get('pod_number', 0)
        pid = p.get('pod_id', 'pod-' + str(num))
        if p.get('ws_connected', False):
            connected.append(pid)
        else:
            disconnected.append(pid)
    total = len(pods)
    print('CONNECTED_COUNT=' + str(len(connected)))
    print('TOTAL_COUNT=' + str(total))
    print('DISCONNECTED=' + ','.join(disconnected) if disconnected else 'DISCONNECTED=')
    print('CONNECTED=' + ','.join(connected) if connected else 'CONNECTED=')
except Exception as e:
    print('PARSE_ERROR=' + str(e))
" 2>/dev/null || echo "PARSE_ERROR=python3 failed")

CONNECTED_COUNT=$(echo "$FLEET_RESULT" | grep '^CONNECTED_COUNT=' | cut -d= -f2)
TOTAL_COUNT=$(echo "$FLEET_RESULT" | grep '^TOTAL_COUNT=' | cut -d= -f2)
DISCONNECTED_PODS=$(echo "$FLEET_RESULT" | grep '^DISCONNECTED=' | cut -d= -f2)
CONNECTED_PODS=$(echo "$FLEET_RESULT" | grep '^CONNECTED=' | cut -d= -f2)

if echo "$FLEET_RESULT" | grep -q "^PARSE_ERROR"; then
    PARSE_ERR=$(echo "$FLEET_RESULT" | grep '^PARSE_ERROR=' | cut -d= -f2-)
    fail "Could not parse fleet/health response: ${PARSE_ERR}"
    log_to_ai_debugger "fleet_ws_connected" "fleet/health JSON parse error: ${PARSE_ERR}. Response snippet: ${FLEET_RESP:0:200}"
elif [ "${CONNECTED_COUNT:-0}" -eq 8 ] 2>/dev/null; then
    pass "All 8 pods ws_connected=true (DEPL-02)"
    info "Connected pods: ${CONNECTED_PODS}"
else
    EXPECTED=8
    fail "Only ${CONNECTED_COUNT:-0}/${EXPECTED} pods ws_connected=true — disconnected: ${DISCONNECTED_PODS} (DEPL-02)"
    log_to_ai_debugger "fleet_ws_connected" "Only ${CONNECTED_COUNT:-0}/8 pods have ws_connected=true after deploy. Disconnected pods: ${DISCONNECTED_PODS}. Check rc-agent.exe on those pods — may need restart via HKLM Run key."
    info "Connected: ${CONNECTED_PODS}"
    info "Disconnected: ${DISCONNECTED_PODS}"
fi

# ─── Gate 6: build_id consistency across fleet (DEPL-02) ──────────────────
echo ""
echo "--- Gate 6: build_id consistency across fleet (DEPL-02) ---"

BUILD_RESULT=$(echo "$FLEET_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    build_ids = {}
    for p in pods:
        num = p.get('pod_number', 0)
        pid = p.get('pod_id', 'pod-' + str(num))
        bid = p.get('build_id', None)
        if bid is not None:
            build_ids[pid] = str(bid)
    unique_builds = set(build_ids.values())
    if len(unique_builds) == 0:
        print('NO_BUILD_ID')
    elif len(unique_builds) == 1:
        print('CONSISTENT=' + list(unique_builds)[0])
    else:
        # Multiple build IDs — report per-pod mapping
        detail = '; '.join(f'{pod}={bid}' for pod, bid in sorted(build_ids.items()))
        print('MISMATCH=' + detail)
except Exception as e:
    print('PARSE_ERROR=' + str(e))
" 2>/dev/null || echo "PARSE_ERROR=python3 failed")

if echo "$BUILD_RESULT" | grep -q "^CONSISTENT="; then
    BUILD_ID=$(echo "$BUILD_RESULT" | cut -d= -f2-)
    pass "All connected pods report consistent build_id: ${BUILD_ID} (DEPL-02)"
elif echo "$BUILD_RESULT" | grep -q "^NO_BUILD_ID"; then
    info "build_id field not present in fleet/health response (older agent version may not report it)"
    skip "build_id not reported by fleet — skipping consistency check (DEPL-02)"
elif echo "$BUILD_RESULT" | grep -q "^MISMATCH="; then
    MISMATCH_DETAIL=$(echo "$BUILD_RESULT" | cut -d= -f2-)
    fail "build_id mismatch across fleet — not all pods have new binary (DEPL-02)"
    log_to_ai_debugger "build_id_consistency" "build_id mismatch detected after deploy. Per-pod build IDs: ${MISMATCH_DETAIL}. Pods with old build_id did not receive the deploy — check pendrive install or rc-sentry /exec on affected pods."
    info "Per-pod build IDs: ${MISMATCH_DETAIL}"
elif echo "$BUILD_RESULT" | grep -q "^PARSE_ERROR"; then
    PARSE_ERR=$(echo "$BUILD_RESULT" | cut -d= -f2-)
    fail "Could not parse build_id from fleet/health: ${PARSE_ERR}"
    log_to_ai_debugger "build_id_consistency" "build_id parse error from fleet/health: ${PARSE_ERR}"
fi

# ─── Gate 7: installed_games validation on canary pod (DEPL-02) ───────────
echo ""
echo "--- Gate 7: installed_games on canary ${POD_ID} (DEPL-02) ---"

GAMES_RESULT=$(echo "$FLEET_RESP" | python3 -c "
import sys, json
try:
    data = json.load(sys.stdin)
    pods = data.get('pods', [])
    canary = '${POD_ID}'
    for p in pods:
        num = p.get('pod_number', 0)
        pid = p.get('pod_id', 'pod-' + str(num))
        pnum2 = 'pod-' + str(num)
        if pid == canary or pnum2 == canary:
            games = p.get('installed_games', None)
            if games is None:
                print('FIELD_MISSING')
            elif len(games) == 0:
                print('EMPTY')
            else:
                print('OK=' + ','.join(str(g) for g in games))
            break
    else:
        print('POD_NOT_FOUND')
except Exception as e:
    print('PARSE_ERROR=' + str(e))
" 2>/dev/null || echo "PARSE_ERROR=python3 failed")

if echo "$GAMES_RESULT" | grep -q "^OK="; then
    GAMES_LIST=$(echo "$GAMES_RESULT" | cut -d= -f2-)
    # Check for baseline game
    if echo "$GAMES_LIST" | grep -qi "assetto_corsa"; then
        pass "installed_games non-empty and contains assetto_corsa on ${POD_ID}: ${GAMES_LIST} (DEPL-02)"
    else
        pass "installed_games non-empty on ${POD_ID}: ${GAMES_LIST} (DEPL-02)"
        info "Note: assetto_corsa not in installed_games list — verify Content Manager is installed"
    fi
elif echo "$GAMES_RESULT" | grep -q "^FIELD_MISSING"; then
    skip "installed_games field not present in fleet/health for ${POD_ID} (older agent version)"
elif echo "$GAMES_RESULT" | grep -q "^EMPTY"; then
    fail "installed_games is empty list for ${POD_ID} — no games detected by rc-agent (DEPL-02)"
    log_to_ai_debugger "installed_games" "installed_games is empty on ${POD_ID} — rc-agent game detection may be broken or games not installed. Check C:/RacingPoint/rc-agent.toml game_paths config."
elif echo "$GAMES_RESULT" | grep -q "^POD_NOT_FOUND"; then
    fail "Canary pod ${POD_ID} not found in fleet/health response"
    log_to_ai_debugger "installed_games" "Pod ${POD_ID} not in fleet/health pods list. Pod may not be registered or pod_id/pod_number mismatch."
elif echo "$GAMES_RESULT" | grep -q "^PARSE_ERROR"; then
    PARSE_ERR=$(echo "$GAMES_RESULT" | cut -d= -f2-)
    fail "Could not parse installed_games from fleet/health: ${PARSE_ERR}"
    log_to_ai_debugger "installed_games" "installed_games parse error: ${PARSE_ERR}"
fi

# ─── DEPL-04: AI debugger log summary ─────────────────────────────────────
echo ""
if [ -f "${AI_LOG}" ] && [ -s "${AI_LOG}" ]; then
    FAILURE_COUNT=$(wc -l < "${AI_LOG}" | tr -d ' ')
    info "AI debugger input written to: ${AI_LOG} (${FAILURE_COUNT} failure(s) logged)"
else
    info "No failures logged — AI debugger input file not created (all gates passed)"
fi

# ─── Summary ───────────────────────────────────────────────────────────────
echo ""
summary_exit
