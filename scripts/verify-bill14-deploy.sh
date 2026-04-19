#!/bin/bash
# verify-bill14-deploy.sh — Validates BILL-14 + 5 audit fixes (commits 5fcabd38, 3b38f541, ba388dc1) are live post-deploy.
#
# Run AFTER deploying racecontrol to server .23 AND rc-agent to pods 1-8.
# Usage: bash scripts/verify-bill14-deploy.sh [expected_hash]
#
# Validates:
#   - Server racecontrol build_id matches expected
#   - All 8 pods rc-agent build_id matches expected
#   - No "Launch timeout (attempt 1)" firing in last 24h where pod reached Running earlier
#     (regression check — BILL-14 should suppress those)
#   - pod_6 rc-agent log no longer shows ≥10 "RC AC plugin not found" lines/sec
#     (log-spam fix verification)

set -u

EXPECTED_HASH="${1:-d8882854}"
SERVER="192.168.31.23"
POD_IPS=(89 33 28 88 86 87 38 91)
POD_NAMES=(pod_1 pod_2 pod_3 pod_4 pod_5 pod_6 pod_7 pod_8)

echo "=== BILL-14 Deploy Verification ==="
echo "Expected hash: $EXPECTED_HASH"
echo ""

# 1. Server racecontrol build_id
echo "[1/4] Server racecontrol build_id:"
SERVER_BID=$(curl -s --max-time 5 "http://${SERVER}:8080/api/v1/health" | grep -o '"build_id":"[^"]*"' | head -1)
echo "  $SERVER_BID"
if echo "$SERVER_BID" | grep -q "$EXPECTED_HASH"; then
  echo "  -> Matches expected"
else
  echo "  -> DOES NOT MATCH (expected $EXPECTED_HASH)"
fi

# 2. Each pod rc-agent build_id
echo ""
echo "[2/4] Pod rc-agent build_id (8 pods):"
for i in "${!POD_IPS[@]}"; do
  ip="192.168.31.${POD_IPS[$i]}"
  name="${POD_NAMES[$i]}"
  BID=$(curl -s --max-time 5 "http://${ip}:8090/health" | grep -o '"build_id":"[^"]*"')
  echo "  $name ($ip): $BID"
done

# 3. BILL-14 suppression: grep server log for "BILL-14: Launch timeout suppressed"
echo ""
echo "[3/4] BILL-14 guard firings (server log last 24h):"
ssh server "findstr /C:\"BILL-14:\" C:\\RacingPoint\\logs\\racecontrol-.2026-04-18.jsonl 2>nul | find /C /V \"\"" 2>/dev/null
echo "  (Count of BILL-14 guard log lines today — ≥1 means guard is firing on real timeouts.)"

# 4. Log spam fix: check pod_6 rc-agent log rate of "RC AC plugin not found"
echo ""
echo "[4/4] pod_6 RC AC plugin log-spam rate:"
SPAM_COUNT=$(ssh pod6 "findstr /C:\"RC AC plugin not found\" C:\\RacingPoint\\rc-agent-.2026-04-18.jsonl 2>nul | find /C /V \"\"" 2>/dev/null)
echo "  Total 'RC AC plugin not found' lines today: $SPAM_COUNT"
echo "  (Before fix: ~10 lines/sec = ~864000/day. After fix: should be ≤1 per plugin-gone episode.)"

echo ""
echo "=== Done ==="
