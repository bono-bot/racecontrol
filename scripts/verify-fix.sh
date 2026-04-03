#!/bin/bash
# =============================================================================
# verify-fix.sh — Unified post-fix verification (CGP G1/G4 proof generator)
#
# Runs ALL behavioral checks after a fix/deploy and produces a structured
# verification report. Catches the "declared fixed but actually broken" pattern.
#
# Philosophy:
#   - BEHAVIORS over proxies (actual screen state > health 200)
#   - BEFORE vs AFTER comparison when possible
#   - TESTED vs NOT-TESTED with risk levels (G4 mandate)
#   - Persistent report for accountability
#
# Usage:
#   bash scripts/verify-fix.sh                          # full verification
#   bash scripts/verify-fix.sh --scope server           # server only
#   bash scripts/verify-fix.sh --scope pods             # pods only
#   bash scripts/verify-fix.sh --scope cloud            # cloud/VPS only
#   bash scripts/verify-fix.sh --scope frontend         # frontend apps only
#   bash scripts/verify-fix.sh --scope comms            # comms-link only
#   bash scripts/verify-fix.sh --scope all              # everything (default)
#   bash scripts/verify-fix.sh --fix "description"      # tag the report
#   bash scripts/verify-fix.sh --save                   # save report to file
#   bash scripts/verify-fix.sh --baseline               # capture baseline (before fix)
#   bash scripts/verify-fix.sh --compare                # compare against baseline
#
# Exit code: number of FAIL results (0 = all pass)
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
REPO_DIR=$(cd "${SCRIPT_DIR}/.." && pwd)
RP_DIR=$(cd "${REPO_DIR}/.." && pwd)
REPORT_DIR="${REPO_DIR}/verification-reports"
BASELINE_FILE="/tmp/verify-fix-baseline.json"

# Colors
GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; DIM='\033[2m'; NC='\033[0m'

# Counters
PASS=0; FAIL=0; WARN=0; SKIP=0
TESTED=()
NOT_TESTED=()
RESULTS_JSON="[]"

# Args
SCOPE="all"
FIX_DESC=""
SAVE_REPORT=0
BASELINE_MODE=0
COMPARE_MODE=0

while [[ $# -gt 0 ]]; do
    case $1 in
        --scope) SCOPE="$2"; shift 2 ;;
        --fix) FIX_DESC="$2"; shift 2 ;;
        --save) SAVE_REPORT=1; shift ;;
        --baseline) BASELINE_MODE=1; shift ;;
        --compare) COMPARE_MODE=1; shift ;;
        *) shift ;;
    esac
done

# ─── Network config ──────────────────────────────────────────────────

SERVER_IP="192.168.31.23"
SERVER_TS="100.125.108.37"
BONO_VPS="100.70.177.44"
POS_IP="192.168.31.20"
SENTRY_PORT=8091

pod_ip() {
    case "$1" in
        1) echo "192.168.31.89" ;; 2) echo "192.168.31.33" ;;
        3) echo "192.168.31.28" ;; 4) echo "192.168.31.88" ;;
        5) echo "192.168.31.86" ;; 6) echo "192.168.31.87" ;;
        7) echo "192.168.31.38" ;; 8) echo "192.168.31.91" ;;
        *) echo "" ;;
    esac
}

pod_ssh() {
    case "$1" in
        1) echo "pod1" ;; 2) echo "pod2" ;; 3) echo "pod3" ;; 4) echo "pod4" ;;
        5) echo "pod5" ;; 6) echo "User@192.168.31.87" ;; 7) echo "pod7" ;; 8) echo "pod8" ;;
        *) echo "" ;;
    esac
}

timestamp_ist() {
    python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M IST'))" 2>/dev/null || date -u "+%Y-%m-%d %H:%M UTC"
}

# ─── Result recording ────────────────────────────────────────────────

record() {
    local STATUS="$1"  # PASS, FAIL, WARN, SKIP
    local CHECK="$2"   # check name
    local DETAIL="$3"  # evidence/detail
    local RISK="${4:-}" # for NOT_TESTED: HIGH, MED, LOW

    case "$STATUS" in
        PASS) echo -e "  ${GREEN}[PASS]${NC} ${CHECK}: ${DETAIL}"; PASS=$((PASS+1)); TESTED+=("$CHECK") ;;
        FAIL) echo -e "  ${RED}[FAIL]${NC} ${CHECK}: ${DETAIL}"; FAIL=$((FAIL+1)); TESTED+=("$CHECK") ;;
        WARN) echo -e "  ${YELLOW}[WARN]${NC} ${CHECK}: ${DETAIL}"; WARN=$((WARN+1)); TESTED+=("$CHECK") ;;
        SKIP) echo -e "  ${DIM}[SKIP]${NC} ${CHECK}: ${DETAIL}"; SKIP=$((SKIP+1)); NOT_TESTED+=("${CHECK} [${RISK:-LOW}]") ;;
    esac

    # Accumulate for JSON report
    RESULTS_JSON=$(echo "$RESULTS_JSON" | python3 -c "
import sys,json
data=json.loads(sys.stdin.read())
data.append({'status':'$STATUS','check':'$CHECK','detail':'''$DETAIL''','risk':'${RISK:-}'})
print(json.dumps(data))
" 2>/dev/null || echo "$RESULTS_JSON")
}

section() { echo ""; echo -e "${BOLD}═══ $1 ═══${NC}"; }

# =======================================================================
# HEADER
# =======================================================================

echo "=========================================="
echo "  Verification Report"
echo "  $(timestamp_ist)"
[ -n "$FIX_DESC" ] && echo "  Fix: ${FIX_DESC}"
echo "  Scope: ${SCOPE}"
echo "=========================================="

# =======================================================================
# SECTION 1: GIT STATE (is the fix actually committed & pushed?)
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "server" ] || [ "$SCOPE" = "pods" ]; then
    section "GIT STATE"

    RC_HEAD=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "?")
    RC_DIRTY=$(git -C "$REPO_DIR" diff --quiet 2>/dev/null && echo "clean" || echo "DIRTY")
    RC_UNPUSHED=$(git -C "$REPO_DIR" rev-list --count '@{upstream}..HEAD' 2>/dev/null || echo "?")

    echo "  HEAD: ${RC_HEAD} | Tree: ${RC_DIRTY} | Unpushed: ${RC_UNPUSHED}"

    if [ "$RC_DIRTY" = "DIRTY" ]; then
        record "WARN" "git-dirty" "Uncommitted changes — fix may not be in the binary"
    else
        record "PASS" "git-clean" "Working tree clean"
    fi

    if [ "$RC_UNPUSHED" != "0" ] && [ "$RC_UNPUSHED" != "?" ]; then
        record "WARN" "git-unpushed" "${RC_UNPUSHED} commits not pushed — Bono/cloud can't see this fix"
    else
        record "PASS" "git-synced" "All commits pushed to remote"
    fi
fi

# =======================================================================
# SECTION 2: BUILD MATCH (is the deployed binary the one with the fix?)
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "server" ]; then
    section "SERVER VERIFICATION"

    # 2a: Server reachable?
    SERVER_HEALTH=$(curl -s --max-time 5 "http://${SERVER_IP}:8080/api/v1/health" 2>/dev/null || echo "")

    if [ -z "$SERVER_HEALTH" ]; then
        record "FAIL" "server-reachable" "Server .23:8080 not responding"
        # Skip downstream server checks
    else
        SERVER_BUILD=$(echo "$SERVER_HEALTH" | sed -n 's/.*"build_id":"\([^"]*\)".*/\1/p')
        SERVER_STATUS=$(echo "$SERVER_HEALTH" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p')
        record "PASS" "server-reachable" "status=${SERVER_STATUS}, build_id=${SERVER_BUILD}"

        # 2b: Build matches HEAD?
        RC_HEAD=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "?")
        if [ "$SERVER_BUILD" = "$RC_HEAD" ]; then
            record "PASS" "server-build-match" "Deployed build ${SERVER_BUILD} matches HEAD"
        else
            # Check if the diff is code or docs
            CODE_DIFF=$(git -C "$REPO_DIR" log --oneline "${SERVER_BUILD}..${RC_HEAD}" -- 'crates/racecontrol/' 2>/dev/null | wc -l || echo "?")
            if [ "$CODE_DIFF" -gt 0 ] 2>/dev/null; then
                record "FAIL" "server-build-match" "Server on ${SERVER_BUILD}, HEAD is ${RC_HEAD} (${CODE_DIFF} code commits behind)"
            else
                record "WARN" "server-build-match" "Server on ${SERVER_BUILD}, HEAD is ${RC_HEAD} (docs-only diff)"
            fi
        fi

        # 2c: Fleet health (does server see pods?)
        FLEET=$(curl -s --max-time 8 "http://${SERVER_IP}:8080/api/v1/fleet/health" 2>/dev/null || echo "")
        if [ -n "$FLEET" ]; then
            WS_COUNT=$(echo "$FLEET" | python3 -c "import sys,json; d=json.loads(sys.stdin.read()); print(sum(1 for p in d.get('pods',[]) if p.get('ws_connected')))" 2>/dev/null || echo "0")
            record "PASS" "fleet-visibility" "Server sees ${WS_COUNT}/8 pods via WebSocket"
        else
            record "SKIP" "fleet-visibility" "Fleet endpoint not responding" "MED"
        fi

        # 2d: Smoke test critical endpoints
        for ENDPOINT in "/api/v1/health" "/api/v1/fleet/health" "/api/v1/billing/rates"; do
            CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:8080${ENDPOINT}" 2>/dev/null || echo "000")
            if [ "$CODE" = "200" ]; then
                record "PASS" "endpoint:${ENDPOINT}" "HTTP ${CODE}"
            elif [ "$CODE" = "401" ]; then
                record "PASS" "endpoint:${ENDPOINT}" "HTTP ${CODE} (auth required — expected)"
            else
                record "FAIL" "endpoint:${ENDPOINT}" "HTTP ${CODE}"
            fi
        done
    fi
fi

# =======================================================================
# SECTION 3: POD BEHAVIORAL VERIFICATION
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "pods" ]; then
    section "POD VERIFICATION"

    for POD_NUM in $(seq 1 8); do
        POD_LAN=$(pod_ip $POD_NUM)
        POD_SSH=$(pod_ssh $POD_NUM)

        echo -e "  ${DIM}--- Pod ${POD_NUM} (${POD_LAN}) ---${NC}"

        # 3a: Ping reachable?
        if ! ping -c 1 -W 2 "$POD_LAN" > /dev/null 2>&1; then
            record "SKIP" "pod${POD_NUM}-reachable" "Pod ${POD_NUM} not pingable (venue may be closed)" "MED"
            continue
        fi

        # 3b: rc-agent health
        AGENT_HEALTH=$(curl -s --max-time 3 "http://${POD_LAN}:8090/health" 2>/dev/null || echo "")
        if [ -z "$AGENT_HEALTH" ]; then
            # Multi-probe: try sentry
            SENTRY_PING=$(curl -s --max-time 3 "http://${POD_LAN}:${SENTRY_PORT}/ping" 2>/dev/null || echo "")
            if [ "$SENTRY_PING" = "pong" ]; then
                record "FAIL" "pod${POD_NUM}-agent" "rc-agent DOWN but rc-sentry alive — agent crashed or stuck"
            else
                record "FAIL" "pod${POD_NUM}-agent" "Both rc-agent and rc-sentry unresponsive"
            fi
            continue
        fi

        AGENT_BUILD=$(echo "$AGENT_HEALTH" | sed -n 's/.*"build_id":"\([^"]*\)".*/\1/p')
        record "PASS" "pod${POD_NUM}-agent" "rc-agent UP, build_id=${AGENT_BUILD}"

        # 3c: Session context (the #1 silent killer)
        SESSION=$(ssh -o ConnectTimeout=3 -o StrictHostKeyChecking=no "$POD_SSH" \
            'tasklist /V /FO CSV /NH | findstr rc-agent' 2>/dev/null || echo "")

        if echo "$SESSION" | grep -qi "Services"; then
            record "FAIL" "pod${POD_NUM}-session" "rc-agent in SESSION 0 — ALL GUI broken (blanking, games, Edge)"
        elif echo "$SESSION" | grep -qi "Console"; then
            record "PASS" "pod${POD_NUM}-session" "rc-agent in Session 1 (Console)"
        elif [ -z "$SESSION" ]; then
            record "SKIP" "pod${POD_NUM}-session" "SSH failed — can't verify session context" "HIGH"
        else
            record "WARN" "pod${POD_NUM}-session" "Session unclear: ${SESSION:0:80}"
        fi

        # 3d: Edge browser / blanking state
        DEBUG=$(curl -s --max-time 3 "http://${POD_LAN}:18924/debug" 2>/dev/null || echo "")
        if [ -n "$DEBUG" ]; then
            EDGE_COUNT=$(echo "$DEBUG" | python3 -c "import sys,json; print(json.load(sys.stdin).get('edge_process_count',0))" 2>/dev/null || echo "?")
            LOCK_STATE=$(echo "$DEBUG" | python3 -c "import sys,json; print(json.load(sys.stdin).get('lock_screen_state','unknown'))" 2>/dev/null || echo "?")

            if [ "$LOCK_STATE" = "screen_blanked" ] && [ "$EDGE_COUNT" = "0" ]; then
                record "FAIL" "pod${POD_NUM}-blanking" "Blanking state=${LOCK_STATE} but edge_count=0 — screen shows nothing"
            elif [ "$EDGE_COUNT" != "?" ] && [ "$EDGE_COUNT" -gt 0 ] 2>/dev/null; then
                record "PASS" "pod${POD_NUM}-blanking" "edge=${EDGE_COUNT}, lock=${LOCK_STATE}"
            else
                record "WARN" "pod${POD_NUM}-blanking" "edge=${EDGE_COUNT}, lock=${LOCK_STATE}"
            fi
        else
            record "SKIP" "pod${POD_NUM}-blanking" "Debug endpoint :18924 not responding" "MED"
        fi

        # 3e: MAINTENANCE_MODE sentinel (silent pod killer)
        MAINT=$(curl -s --max-time 3 -X POST "http://${POD_LAN}:${SENTRY_PORT}/exec" \
            -H "Content-Type: application/json" \
            -d '{"cmd":"if exist C:\\RacingPoint\\MAINTENANCE_MODE (type C:\\RacingPoint\\MAINTENANCE_MODE) else (echo CLEAR)"}' \
            2>/dev/null || echo "")
        if echo "$MAINT" | grep -qi "CLEAR"; then
            record "PASS" "pod${POD_NUM}-maintenance" "No MAINTENANCE_MODE sentinel"
        elif [ -z "$MAINT" ]; then
            record "SKIP" "pod${POD_NUM}-maintenance" "rc-sentry exec failed" "LOW"
        else
            record "FAIL" "pod${POD_NUM}-maintenance" "MAINTENANCE_MODE active — all restarts blocked!"
        fi
    done
fi

# =======================================================================
# SECTION 4: FRONTEND VERIFICATION
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "frontend" ]; then
    section "FRONTEND VERIFICATION"

    # 4a: Static files served (the real test, not just "is port open")
    for APP_ENTRY in "web:3200:" "kiosk:3300:/kiosk"; do
        APP_NAME=$(echo "$APP_ENTRY" | cut -d: -f1)
        APP_PORT=$(echo "$APP_ENTRY" | cut -d: -f2)
        APP_BASE=$(echo "$APP_ENTRY" | cut -d: -f3)
        APP_PATH="${REPO_DIR}/${APP_NAME}"

        # Find a real CSS file to test
        CSS_FILE=$(ls "${APP_PATH}/.next/static/css/"*.css 2>/dev/null | head -1 | xargs basename 2>/dev/null || echo "")

        if [ -n "$CSS_FILE" ]; then
            STATIC_URL="http://${SERVER_IP}:${APP_PORT}${APP_BASE}/_next/static/css/${CSS_FILE}"
            STATIC_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$STATIC_URL" 2>/dev/null || echo "000")

            if [ "$STATIC_CODE" = "200" ]; then
                record "PASS" "${APP_NAME}-static" "Static CSS served (HTTP ${STATIC_CODE})"
            else
                record "FAIL" "${APP_NAME}-static" "Static CSS 404 — standalone/.next/static missing (HTTP ${STATIC_CODE})"
            fi
        else
            record "SKIP" "${APP_NAME}-static" "No local .next/static/css/ to test against" "MED"
        fi

        # Page renders (not just port open)
        PAGE_CODE=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "http://${SERVER_IP}:${APP_PORT}${APP_BASE}/" 2>/dev/null || echo "000")
        if [ "$PAGE_CODE" = "200" ] || [ "$PAGE_CODE" = "307" ] || [ "$PAGE_CODE" = "302" ]; then
            record "PASS" "${APP_NAME}-page" "Main page responds (HTTP ${PAGE_CODE})"
        else
            record "FAIL" "${APP_NAME}-page" "Main page broken (HTTP ${PAGE_CODE})"
        fi
    done
fi

# =======================================================================
# SECTION 5: CLOUD / BONO VPS
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "cloud" ]; then
    section "CLOUD VERIFICATION (Bono VPS)"

    # 5a: SSH reachable?
    if ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" "echo ok" > /dev/null 2>&1; then
        record "PASS" "cloud-ssh" "Bono VPS reachable via SSH"

        # 5b: racecontrol running?
        CLOUD_HEALTH=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" \
            "curl -s --max-time 5 http://localhost:8080/api/v1/health 2>/dev/null" 2>/dev/null || echo "")
        if [ -n "$CLOUD_HEALTH" ]; then
            CLOUD_BUILD=$(echo "$CLOUD_HEALTH" | sed -n 's/.*"build_id":"\([^"]*\)".*/\1/p')
            record "PASS" "cloud-racecontrol" "racecontrol running, build_id=${CLOUD_BUILD}"

            # 5c: Git HEAD matches local?
            CLOUD_HEAD=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" \
                "cd /root/racingpoint/racecontrol && git rev-parse --short HEAD 2>/dev/null" 2>/dev/null || echo "?")
            LOCAL_HEAD=$(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo "?")
            if [ "$CLOUD_HEAD" = "$LOCAL_HEAD" ]; then
                record "PASS" "cloud-git-sync" "VPS HEAD ${CLOUD_HEAD} matches local"
            else
                record "WARN" "cloud-git-sync" "VPS HEAD ${CLOUD_HEAD} != local ${LOCAL_HEAD}"
            fi
        else
            record "FAIL" "cloud-racecontrol" "racecontrol not responding on VPS"
        fi

        # 5d: comms-link running?
        COMMS_STATUS=$(ssh -o ConnectTimeout=5 -o BatchMode=yes "root@${BONO_VPS}" \
            "pm2 jlist 2>/dev/null" 2>/dev/null | python3 -c "
import sys,json
try:
    procs=json.loads(sys.stdin.read())
    cl=[p for p in procs if 'comms' in p.get('name','').lower()]
    if cl: print(f\"{cl[0]['name']}:{cl[0].get('pm2_env',{}).get('status','?')}\")
    else: print('NOT_FOUND')
except: print('ERROR')
" 2>/dev/null || echo "ERROR")
        if echo "$COMMS_STATUS" | grep -qi "online"; then
            record "PASS" "cloud-comms-link" "comms-link ${COMMS_STATUS}"
        else
            record "FAIL" "cloud-comms-link" "comms-link status: ${COMMS_STATUS}"
        fi
    else
        record "SKIP" "cloud-ssh" "Bono VPS unreachable via SSH (Tailscale down?)" "HIGH"
        NOT_TESTED+=("cloud-racecontrol [HIGH]" "cloud-git-sync [HIGH]" "cloud-comms-link [HIGH]")
    fi
fi

# =======================================================================
# SECTION 6: COMMS-LINK
# =======================================================================

if [ "$SCOPE" = "all" ] || [ "$SCOPE" = "comms" ]; then
    section "COMMS-LINK VERIFICATION"

    # 6a: James relay
    RELAY_HEALTH=$(curl -s --max-time 3 "http://localhost:8766/relay/health" 2>/dev/null || echo "")
    if [ -n "$RELAY_HEALTH" ]; then
        RELAY_CONNECTED=$(echo "$RELAY_HEALTH" | python3 -c "
import sys,json
try:
    d=json.loads(sys.stdin.read())
    print(d.get('connection_mode','unknown'))
except: print('error')
" 2>/dev/null || echo "?")
        record "PASS" "comms-relay" "James relay :8766 UP, mode=${RELAY_CONNECTED}"

        # 6b: Round-trip test (exec a harmless command on Bono)
        EXEC_RESULT=$(curl -s --max-time 15 -X POST "http://localhost:8766/relay/exec/run" \
            -H "Content-Type: application/json" \
            -d '{"command":"node_version","reason":"verify-fix round-trip test"}' 2>/dev/null || echo "")
        if echo "$EXEC_RESULT" | grep -qi "v[0-9]"; then
            record "PASS" "comms-roundtrip" "Exec round-trip OK: $(echo "$EXEC_RESULT" | sed -n 's/.*\(v[0-9][0-9.]*\).*/\1/p' | head -1)"
        elif [ -n "$EXEC_RESULT" ]; then
            record "WARN" "comms-roundtrip" "Exec returned but unexpected: ${EXEC_RESULT:0:80}"
        else
            record "FAIL" "comms-roundtrip" "Exec round-trip failed — relay may be disconnected"
        fi
    else
        record "FAIL" "comms-relay" "James relay :8766 not responding"
        record "SKIP" "comms-roundtrip" "Relay down, can't test" "MED"
    fi
fi

# =======================================================================
# SECTION 7: BASELINE CAPTURE / COMPARISON
# =======================================================================

if [ "$BASELINE_MODE" -eq 1 ]; then
    section "BASELINE CAPTURED"
    echo "$RESULTS_JSON" > "$BASELINE_FILE"
    echo -e "  Saved to ${BASELINE_FILE}"
    echo -e "  Run the fix, then: ${CYAN}bash scripts/verify-fix.sh --compare${NC}"
fi

if [ "$COMPARE_MODE" -eq 1 ] && [ -f "$BASELINE_FILE" ]; then
    section "BEFORE vs AFTER COMPARISON"

    # Write current results to temp file for safe python comparison
    AFTER_FILE="/tmp/verify-fix-after.json"
    echo "$RESULTS_JSON" > "$AFTER_FILE"

    python3 - "$BASELINE_FILE" "$AFTER_FILE" <<'PYEOF'
import json, sys
before = json.load(open(sys.argv[1]))
after = json.load(open(sys.argv[2]))
before_map = {r['check']: r for r in before}
after_map = {r['check']: r for r in after}
changes = []
for check, after_r in after_map.items():
    before_r = before_map.get(check)
    if before_r and before_r['status'] != after_r['status']:
        changes.append(f"  {before_r['status']} -> {after_r['status']}  {check}")
if changes:
    print('  Changes detected:')
    for c in changes: print(c)
else:
    print('  No status changes between baseline and current run')
    print('  (The fix may not have had the expected effect)')
PYEOF
fi

# =======================================================================
# G1/G4 STRUCTURED REPORT
# =======================================================================

section "G1/G4 VERIFICATION REPORT"

echo ""
echo -e "${BOLD}G1 PROOF:${NC}"
echo "  1. BEHAVIORS tested: ${#TESTED[@]} checks across scope '${SCOPE}'"
echo "  2. METHOD: automated behavioral verification (verify-fix.sh)"
echo "  3. EVIDENCE: see results above"
echo ""

echo -e "${BOLD}G4 PROOF:${NC}"
echo -e "  ${GREEN}TESTED (${#TESTED[@]}):${NC}"
for t in "${TESTED[@]}"; do
    echo "    - $t"
done

if [ ${#NOT_TESTED[@]} -gt 0 ]; then
    echo ""
    echo -e "  ${YELLOW}NOT TESTED (${#NOT_TESTED[@]}):${NC}"
    for nt in "${NOT_TESTED[@]}"; do
        echo "    - $nt"
    done
fi

# =======================================================================
# SAVE REPORT
# =======================================================================

if [ "$SAVE_REPORT" -eq 1 ]; then
    mkdir -p "$REPORT_DIR"
    TIMESTAMP=$(date -u +"%Y%m%d-%H%M%S")
    REPORT_FILE="${REPORT_DIR}/verify-${TIMESTAMP}.md"

    {
        echo "# Verification Report"
        echo ""
        echo "- **Date:** $(timestamp_ist)"
        echo "- **Fix:** ${FIX_DESC:-unspecified}"
        echo "- **Scope:** ${SCOPE}"
        echo "- **HEAD:** $(git -C "$REPO_DIR" rev-parse --short HEAD 2>/dev/null || echo '?')"
        echo ""
        echo "## Results"
        echo ""
        echo "| Status | Count |"
        echo "|--------|-------|"
        echo "| PASS   | ${PASS} |"
        echo "| FAIL   | ${FAIL} |"
        echo "| WARN   | ${WARN} |"
        echo "| SKIP   | ${SKIP} |"
        echo ""
        echo "## Tested"
        echo ""
        for t in "${TESTED[@]}"; do echo "- $t"; done
        echo ""
        echo "## Not Tested"
        echo ""
        if [ ${#NOT_TESTED[@]} -gt 0 ]; then
            for nt in "${NOT_TESTED[@]}"; do echo "- $nt"; done
        else
            echo "- (all checks ran)"
        fi
    } > "$REPORT_FILE"

    echo ""
    echo -e "  Report saved: ${CYAN}${REPORT_FILE}${NC}"
fi

# =======================================================================
# SUMMARY
# =======================================================================

echo ""
echo "=========================================="
echo -e "  ${GREEN}PASS: ${PASS}${NC} | ${RED}FAIL: ${FAIL}${NC} | ${YELLOW}WARN: ${WARN}${NC} | ${DIM}SKIP: ${SKIP}${NC}"

if [ "$FAIL" -eq 0 ] && [ "$WARN" -eq 0 ]; then
    echo -e "  ${GREEN}${BOLD}ALL CHECKS PASSED${NC}"
elif [ "$FAIL" -eq 0 ]; then
    echo -e "  ${YELLOW}${BOLD}PASSED WITH WARNINGS${NC} — review WARN items"
else
    echo -e "  ${RED}${BOLD}VERIFICATION FAILED${NC} — ${FAIL} check(s) need attention"
    echo -e "  ${RED}DO NOT claim 'done' or 'fixed' until FAIL=0${NC}"
fi

HIGH_RISK=0
if [ ${#NOT_TESTED[@]} -gt 0 ]; then
    HIGH_RISK=$(printf '%s\n' "${NOT_TESTED[@]}" | grep -c "HIGH" || true)
fi
if [ "${HIGH_RISK:-0}" -gt 0 ] 2>/dev/null; then
    echo -e "  ${RED}${HIGH_RISK} HIGH-risk items not tested — follow-up required${NC}"
fi

echo "=========================================="
exit $FAIL
