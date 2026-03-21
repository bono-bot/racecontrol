#!/bin/bash
# =============================================================================
# deploy-pod.sh — Safe rc-agent deploy to a single pod via rc-sentry
#
# FAILSAFE: Uses rc-sentry (:8091) for all deploy operations.
# rc-sentry is independent of both racecontrol and rc-agent — it never
# loses connectivity during deploys.
#
# Usage:
#   ./deploy-pod.sh pod-8                 # deploy to Pod 8 only
#   ./deploy-pod.sh all                   # deploy to all 8 pods
#   BINARY_DIR=/path ./deploy-pod.sh pod-3
# =============================================================================

set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
JAMES_IP="${JAMES_IP:-192.168.31.27}"
SENTRY_PORT=8091
SERVE_PORT=18889
BINARY_DIR="${BINARY_DIR:-$(cd "${SCRIPT_DIR}/../racingpoint/deploy-staging" 2>/dev/null && pwd || echo "$HOME/racingpoint/deploy-staging")}"
BINARY_DIR="${BINARY_DIR:-$HOME/racingpoint/deploy-staging}"

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[0;33m'; NC='\033[0m'
pass() { echo -e "  ${GREEN}OK${NC}    $1"; }
fail() { echo -e "  ${RED}FAIL${NC}  $1"; FAILURES=$((FAILURES+1)); }
info() { echo -e "  ${YELLOW}...${NC}   $1"; }
FAILURES=0

# Source pod map if available
POD_MAP="${SCRIPT_DIR}/../tests/e2e/lib/pod-map.sh"
if [ -f "$POD_MAP" ]; then
    source "$POD_MAP"
else
    pod_ip() {
        case "$1" in
            pod-1) echo "192.168.31.89" ;; pod-2) echo "192.168.31.33" ;;
            pod-3) echo "192.168.31.28" ;; pod-4) echo "192.168.31.88" ;;
            pod-5) echo "192.168.31.86" ;; pod-6) echo "192.168.31.87" ;;
            pod-7) echo "192.168.31.38" ;; pod-8) echo "192.168.31.91" ;;
            *) echo "" ;;
        esac
    }
fi

deploy_pod() {
    local POD_NAME="$1"
    local POD_IP=$(pod_ip "$POD_NAME")
    if [ -z "$POD_IP" ]; then
        fail "Unknown pod: $POD_NAME"
        return
    fi

    echo ""
    echo "─── Deploying to $POD_NAME ($POD_IP) ───"

    # Gate: rc-sentry reachable
    SENTRY=$(curl -s --max-time 5 "http://${POD_IP}:${SENTRY_PORT}/ping" 2>/dev/null || echo "")
    if [ "$SENTRY" != "pong" ]; then
        fail "$POD_NAME: rc-sentry unreachable"
        return
    fi

    # Download
    info "$POD_NAME: Downloading rc-agent.exe..."
    DL=$(curl -s --max-time 120 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d "{\"cmd\":\"curl -s -o C:/RacingPoint/rc-agent-new.exe http://${JAMES_IP}:${SERVE_PORT}/rc-agent.exe && for %f in (C:/RacingPoint/rc-agent-new.exe) do echo SIZE=%~zf\"}" 2>/dev/null || echo "")
    DL_SIZE=$(echo "$DL" | grep -oP 'SIZE=\K[0-9]+' || echo "0")
    if [ "$DL_SIZE" -lt 500000 ]; then
        fail "$POD_NAME: Download failed (${DL_SIZE} bytes)"
        return
    fi
    pass "$POD_NAME: Downloaded (${DL_SIZE} bytes)"

    # Stop rc-agent (kill bat wrapper first to prevent auto-relaunch, then kill process)
    info "$POD_NAME: Stopping rc-agent..."
    curl -s --max-time 15 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"taskkill /F /FI \"WINDOWTITLE eq RC*Agent*\" 2>nul & taskkill /F /IM rc-agent.exe 2>nul & echo KILLED"}' > /dev/null 2>&1
    sleep 3

    # Swap
    SWAP=$(curl -s --max-time 15 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"move /Y C:/RacingPoint/rc-agent-new.exe C:/RacingPoint/rc-agent.exe & echo SWAPPED"}' 2>/dev/null || echo "")
    if ! echo "$SWAP" | grep -q "SWAPPED"; then
        fail "$POD_NAME: Swap failed"
        return
    fi

    # Start
    curl -s --max-time 10 "http://${POD_IP}:${SENTRY_PORT}/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"start \"rc-agent\" C:/RacingPoint/start-rcagent.bat & echo STARTED"}' > /dev/null 2>&1

    # Verify
    sleep 5
    PING=$(curl -s --max-time 5 "http://${POD_IP}:8090/ping" 2>/dev/null || echo "")
    if [ "$PING" = "pong" ]; then
        pass "$POD_NAME: rc-agent is UP"
    else
        info "$POD_NAME: rc-agent not yet responding (may need more time)"
    fi
}

# ─── Main ─────────────────────────────────────────────────────────────

TARGET="${1:-}"
if [ -z "$TARGET" ]; then
    echo "Usage: $0 <pod-N|all>"
    exit 1
fi

# Gate: Binary exists
if [ ! -f "${BINARY_DIR}/rc-agent.exe" ]; then
    fail "Binary not found: ${BINARY_DIR}/rc-agent.exe"
    exit 1
fi
BINARY_SIZE=$(stat -c%s "${BINARY_DIR}/rc-agent.exe" 2>/dev/null || wc -c < "${BINARY_DIR}/rc-agent.exe" 2>/dev/null || echo "0")
pass "rc-agent.exe binary valid (${BINARY_SIZE} bytes)"

# Start HTTP server
info "Starting HTTP file server on :${SERVE_PORT}..."
cd "${BINARY_DIR}"
python3 -m http.server ${SERVE_PORT} --bind 0.0.0.0 > /dev/null 2>&1 &
HTTP_PID=$!
trap "kill $HTTP_PID 2>/dev/null" EXIT
sleep 1

if [ "$TARGET" = "all" ]; then
    for i in $(seq 1 8); do
        deploy_pod "pod-$i"
    done
else
    deploy_pod "$TARGET"
fi

echo ""
echo "=========================================="
if [ "$FAILURES" -eq 0 ]; then
    echo -e "${GREEN}Deploy complete — 0 failures${NC}"
else
    echo -e "${RED}Deploy complete — ${FAILURES} failure(s)${NC}"
fi
echo "=========================================="
exit $FAILURES
