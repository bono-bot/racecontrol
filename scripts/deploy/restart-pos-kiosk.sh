#!/bin/bash
# restart-pos-kiosk.sh — Restart Edge kiosk on POS machine
# Usage: bash restart-pos-kiosk.sh
# Can be run from James (.27) or the web terminal (http://192.168.31.27:9999)

POS_HOST="POS@192.168.31.20"

echo "=== POS Kiosk Restart ==="
echo "  Target: $POS_HOST"
echo "  Time:   $(date '+%Y-%m-%d %H:%M:%S IST')"
echo ""

# Trigger the RestartKiosk scheduled task (runs in Session 1)
echo "  Triggering RestartKiosk task..."
RESULT=$(ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no "$POS_HOST" "schtasks /Run /TN RestartKiosk" 2>&1 | grep -v WARNING)

if echo "$RESULT" | grep -qi "success"; then
    echo "  OK — Edge kiosk restarting"
    echo ""
    echo "  Waiting 6s for page load..."
    sleep 6

    # Verify Edge is running
    EDGE_COUNT=$(ssh -o ConnectTimeout=5 "$POS_HOST" "tasklist /FI \"IMAGENAME eq msedge.exe\" /NH" 2>&1 | grep -c msedge)
    if [ "$EDGE_COUNT" -gt 0 ]; then
        echo "  PASS — Edge running ($EDGE_COUNT processes)"
    else
        echo "  WARN — Edge may not have started, check POS screen"
    fi

    # Check billing page is reachable from POS
    BILLING=$(ssh -o ConnectTimeout=5 "$POS_HOST" "curl -s -o nul -w \"%{http_code}\" --connect-timeout 5 http://192.168.31.23:8080/billing" 2>&1 | grep -o '[0-9]*$')
    if [ "$BILLING" = "200" ]; then
        echo "  PASS — Billing page HTTP 200"
    else
        echo "  WARN — Billing page returned HTTP $BILLING"
    fi
else
    echo "  FAIL — Could not trigger task"
    echo "  $RESULT"
fi

echo ""
echo "=== Done ==="
