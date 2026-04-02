#!/bin/bash
# Pod Fleet Verification Script — CGP G1/G4 Automated Proof
# Runs behavioral checks, NOT proxy metrics
# Exit code 0 = all pass, non-zero = failures found
#
# Usage:
#   bash scripts/pod-verify.sh              # Behavioral checks only
#   bash scripts/pod-verify.sh --visual     # + screenshot capture & visual analysis
#   bash scripts/pod-verify.sh --visual --compare <dir>  # + compare against previous

PASS=0
FAIL=0
WARN=0
KEY="478a3688339737fb5945f9b89d8bb533f2569fe0b1fea46b504656eee455b9ab"
VISUAL=0
COMPARE_DIR=""

# Parse flags
while [[ $# -gt 0 ]]; do
    case $1 in
        --visual) VISUAL=1; shift ;;
        --compare) COMPARE_DIR="$2"; shift 2 ;;
        *) shift ;;
    esac
done

echo "=== POD FLEET VERIFICATION ($(date '+%Y-%m-%d %H:%M IST')) ==="
echo ""

# Check 1: Pod count validation
POD_COUNT=$(curl -s --connect-timeout 5 http://192.168.31.23:8080/api/v1/fleet/health 2>/dev/null | python3 -c "
import sys,json
try:
    data=json.loads(sys.stdin.read())
    gaming=[p for p in data.get('pods',[]) if p.get('pod_number',99) <= 8]
    print(len(gaming))
except: print('ERROR')
" 2>/dev/null)
if [ "$POD_COUNT" = "8" ]; then
    echo "[PASS] Pod count: 8 gaming pods detected"
    ((PASS++))
else
    echo "[FAIL] Pod count: expected 8 gaming pods, got $POD_COUNT"
    ((FAIL++))
fi

# Check 2-4: Per-pod Session context + edge count + blanking
for pod_info in "1:192.168.31.89:pod1" "2:192.168.31.33:pod2" "3:192.168.31.28:pod3" "4:192.168.31.88:pod4" "5:192.168.31.86:pod5" "6:192.168.31.87:User@192.168.31.87" "7:192.168.31.38:pod7" "8:192.168.31.91:pod8"; do
    IFS=: read -r num ip ssh_target <<< "$pod_info"
    
    # Session check via SSH
    session=$(ssh -o ConnectTimeout=3 -o StrictHostKeyChecking=no $ssh_target "tasklist /V /FO CSV /NH | findstr rc-agent" 2>/dev/null)
    
    if [ -z "$session" ]; then
        echo "[FAIL] Pod $num: rc-agent NOT RUNNING"
        ((FAIL++))
        continue
    fi
    
    if echo "$session" | grep -q "Services"; then
        echo "[FAIL] Pod $num: rc-agent in SESSION 0 (Services) — GUI broken"
        ((FAIL++))
    elif echo "$session" | grep -q "Console"; then
        echo "[PASS] Pod $num: rc-agent in Console Session 1"
        ((PASS++))
    else
        echo "[WARN] Pod $num: session unknown: $session"
        ((WARN++))
    fi
    
    # Debug endpoint check (edge count + blanking)
    debug=$(curl -s --connect-timeout 3 "http://$ip:18924/debug" 2>/dev/null)
    if [ -n "$debug" ]; then
        edge=$(echo "$debug" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('edge_process_count',0))" 2>/dev/null)
        lock=$(echo "$debug" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('lock_screen_state','unknown'))" 2>/dev/null)
        
        if [ "$edge" -gt 0 ] 2>/dev/null; then
            echo "[PASS] Pod $num: edge_process_count=$edge, lock=$lock"
            ((PASS++))
        else
            echo "[FAIL] Pod $num: edge_process_count=$edge (blanking broken)"
            ((FAIL++))
        fi
    else
        echo "[FAIL] Pod $num: debug endpoint unreachable (agent may not be fully started)"
        ((FAIL++))
    fi
done

echo ""
echo "=== SUMMARY: PASS=$PASS FAIL=$FAIL WARN=$WARN ==="

# Visual verification (captures actual screenshots as G1 evidence)
if [ $VISUAL -eq 1 ]; then
    echo ""
    echo "=== VISUAL VERIFICATION ==="
    SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
    VISUAL_ARGS=""
    if [ -n "$COMPARE_DIR" ]; then
        VISUAL_ARGS="--compare $COMPARE_DIR"
    fi
    node "$SCRIPT_DIR/visual-verify.js" $VISUAL_ARGS
    VISUAL_EXIT=$?
    if [ $VISUAL_EXIT -ne 0 ]; then
        ((FAIL++))
        echo "[FAIL] Visual verification found issues"
    else
        echo "[PASS] Visual verification complete — screenshots saved"
    fi
    echo ""
    echo "=== FINAL SUMMARY: PASS=$PASS FAIL=$FAIL WARN=$WARN ==="
fi

if [ $FAIL -gt 0 ]; then
    echo "STATUS: FAIL — DO NOT claim 'done' or 'fixed'"
    exit 1
else
    echo "STATUS: PASS — behavioral verification complete"
    exit 0
fi
