#!/usr/bin/env bash
# tests/e2e/netdata-fleet.sh
# Verify Netdata agent is running and API is responding on all 9 hosts:
#   - Server .23 (racecontrol server)
#   - Pods 1-8 (gaming rigs)
#
# Usage:
#   bash tests/e2e/netdata-fleet.sh
#
# Exit code: number of hosts that failed (0 = all pass)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Load common lib for color output if available (standalone-safe)
if [ -f "$SCRIPT_DIR/lib/common.sh" ]; then
    source "$SCRIPT_DIR/lib/common.sh"
else
    # Standalone fallback — minimal color support
    if [ -t 1 ]; then
        GREEN="\033[0;32m"; RED="\033[0;31m"; RESET="\033[0m"
    else
        GREEN=""; RED=""; RESET=""
    fi
    pass() { printf "${GREEN}PASS${RESET} %s\n" "$*"; }
    fail() { printf "${RED}FAIL${RESET} %s\n" "$*"; }
fi

SERVER_IP="192.168.31.23"
POD_IPS=(
    "192.168.31.89"   # Pod 1
    "192.168.31.33"   # Pod 2
    "192.168.31.28"   # Pod 3
    "192.168.31.88"   # Pod 4
    "192.168.31.86"   # Pod 5
    "192.168.31.87"   # Pod 6
    "192.168.31.38"   # Pod 7
    "192.168.31.91"   # Pod 8
)

ALL_IPS=("$SERVER_IP" "${POD_IPS[@]}")
HOST_LABELS=("Server .23" "Pod 1 .89" "Pod 2 .33" "Pod 3 .28" "Pod 4 .88" "Pod 5 .86" "Pod 6 .87" "Pod 7 .38" "Pod 8 .91")

pass_count=0
fail_count=0

echo "=== Netdata Fleet Verification ==="
echo "Checking ${#ALL_IPS[@]} hosts for Netdata API response at :19999..."
echo ""

for i in "${!ALL_IPS[@]}"; do
    ip="${ALL_IPS[$i]}"
    label="${HOST_LABELS[$i]}"

    response=$(curl -sf --max-time 5 "http://$ip:19999/api/v1/info" 2>/dev/null || true)

    if echo "$response" | grep -q '"version"'; then
        version=$(echo "$response" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('version','?'))" 2>/dev/null || echo "?")
        hostname=$(echo "$response" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('hostname','?'))" 2>/dev/null || echo "?")
        pass "$label ($ip) — Netdata v$version on $hostname"
        ((pass_count++)) || true
    else
        fail "$label ($ip) — :19999/api/v1/info not responding"
        ((fail_count++)) || true
    fi
done

echo ""
echo "--- $pass_count passed, $fail_count failed ---"

if [ "$fail_count" -eq 0 ]; then
    echo "All ${#ALL_IPS[@]} hosts: Netdata running."
else
    echo "$fail_count host(s) not responding. Run Plan 01/02 to install Netdata."
fi

exit "$fail_count"
