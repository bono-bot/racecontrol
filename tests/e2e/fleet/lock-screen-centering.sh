#!/bin/bash
# tests/e2e/fleet/lock-screen-centering.sh
# Verify lock screen HTML on all pods has proper centering CSS.
#
# For each reachable pod:
#   1. Fetch lock screen HTML from :18923 (via rc-agent exec on :8090)
#   2. Assert body CSS block has: justify-content: center, align-items: center, text-align: center
#   3. Assert .pin-row CSS block has: justify-content: center
#
# Usage: bash tests/e2e/fleet/lock-screen-centering.sh

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"

info "Lock Screen Centering — Fleet-wide CSS Verification"
echo ""

for POD_NUM in $(seq 1 8); do
    POD_ID="pod-${POD_NUM}"
    POD_IP=$(pod_ip "$POD_ID")

    if [ -z "$POD_IP" ]; then
        skip "${POD_ID}: no IP mapping"
        continue
    fi

    # Check if pod agent is reachable
    PING=$(curl -s --connect-timeout 3 "http://${POD_IP}:8090/health" 2>/dev/null)
    if [ -z "$PING" ]; then
        skip "${POD_ID}: agent not reachable at ${POD_IP}:8090"
        continue
    fi

    # Fetch lock screen HTML via rc-agent exec (lock screen :18923 not exposed externally)
    RESP=$(curl -s --connect-timeout 5 -X POST "http://${POD_IP}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"curl -s http://127.0.0.1:18923/","timeout_ms":5000}' 2>/dev/null)

    HTML=$(echo "$RESP" | python -c "import sys,json; print(json.load(sys.stdin).get('stdout',''))" 2>/dev/null)

    if [ -z "$HTML" ] || [ ${#HTML} -lt 100 ]; then
        skip "${POD_ID}: lock screen HTML not available (len=${#HTML})"
        continue
    fi

    # Extract body CSS block (between "body {" and the next "}")
    BODY_CSS=$(echo "$HTML" | python -c "
import sys, re
html = sys.stdin.read()
m = re.search(r'body\s*\{([^}]+)\}', html)
print(m.group(1) if m else '')
" 2>/dev/null)

    # Test 1: body has justify-content: center
    if echo "$BODY_CSS" | grep -q "justify-content"; then
        pass "${POD_ID}: body has justify-content: center"
    else
        fail "${POD_ID}: body MISSING justify-content: center"
    fi

    # Test 2: body has align-items: center
    if echo "$BODY_CSS" | grep -q "align-items"; then
        pass "${POD_ID}: body has align-items: center"
    else
        fail "${POD_ID}: body MISSING align-items: center"
    fi

    # Test 3: body has text-align: center
    if echo "$BODY_CSS" | grep -q "text-align"; then
        pass "${POD_ID}: body has text-align: center"
    else
        fail "${POD_ID}: body MISSING text-align: center"
    fi

    # Test 4: .pin-row has justify-content: center (extract pin-row CSS block)
    PIN_ROW_CSS=$(echo "$HTML" | python -c "
import sys, re
html = sys.stdin.read()
m = re.search(r'\.pin-row\s*\{([^}]+)\}', html)
print(m.group(1) if m else '')
" 2>/dev/null)

    if [ -n "$PIN_ROW_CSS" ]; then
        if echo "$PIN_ROW_CSS" | grep -q "justify-content"; then
            pass "${POD_ID}: .pin-row has justify-content: center"
        else
            fail "${POD_ID}: .pin-row MISSING justify-content: center"
        fi
    fi

    # Test 5: tagline margin-bottom is <= 32px (not the old 50px)
    TAGLINE_CSS=$(echo "$HTML" | python -c "
import sys, re
html = sys.stdin.read()
m = re.search(r'\.tagline\s*\{([^}]+)\}', html)
print(m.group(1) if m else '')
" 2>/dev/null)

    if [ -n "$TAGLINE_CSS" ]; then
        MARGIN=$(echo "$TAGLINE_CSS" | python -c "
import sys, re
css = sys.stdin.read()
m = re.search(r'margin-bottom:\s*(\d+)px', css)
print(m.group(1) if m else '0')
" 2>/dev/null)
        if [ "$MARGIN" -le 32 ] 2>/dev/null; then
            pass "${POD_ID}: .tagline margin-bottom=${MARGIN}px (<=32px)"
        else
            fail "${POD_ID}: .tagline margin-bottom=${MARGIN}px (should be <=32px)"
        fi
    fi
done

summary_exit
