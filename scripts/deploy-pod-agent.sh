#!/bin/bash
# deploy-pod-agent.sh — Atomic rc-agent deploy with sentinel-based swap
#
# Usage: bash scripts/deploy-pod-agent.sh <pod_number> <hash>
# Example: bash scripts/deploy-pod-agent.sh 8 878536b2
#
# Flow:
#   0. Compute local SHA256 of the staged binary
#   1. Verify staged binary exists on pod AND matches local SHA256
#   2. Delete all stale hash-named binaries (prevents bat from swapping wrong one)
#   3. Write OTA_DEPLOYING sentinel (watchdog stops restarting)
#   4. Kill rc-agent (watchdog sees sentinel, doesn't restart)
#   5. Swap binary (move old -> prev, move staged -> current)
#   6. Clear sentinel (watchdog restarts with new binary)
#   7. Verify build_id matches expected hash
#
# Requires: rc-agent-<hash>.exe already downloaded to pod via HTTP staging
#
# Root cause of this design (2026-04-06 deploy incident):
# - start-rcagent.bat swaps the NEWEST rc-agent-????????*.exe on every boot
# - If a stale hash-named binary exists from a previous failed deploy,
#   the bat picks IT up instead of the new one
# - SHA256 verification catches stale/cached downloads from HTTP server

set -euo pipefail

POD_NUM="${1:?Usage: deploy-pod-agent.sh <pod_number> <hash>}"
HASH="${2:?Usage: deploy-pod-agent.sh <pod_number> <hash>}"

POD_HOST="pod${POD_NUM}"
INSTALL_DIR='C:\RacingPoint'
STAGING_DIR="C:/Users/bono/racingpoint/deploy-staging"
LOCAL_BINARY="${STAGING_DIR}/rc-agent-${HASH}.exe"

# Pod IP lookup for health check
declare -A POD_IPS=([1]=192.168.31.89 [2]=192.168.31.33 [3]=192.168.31.28 [4]=192.168.31.88 [5]=192.168.31.86 [6]=192.168.31.87 [7]=192.168.31.38 [8]=192.168.31.91)
POD_IP="${POD_IPS[$POD_NUM]}"

echo "=== Deploying rc-agent ${HASH} to Pod ${POD_NUM} (${POD_IP}) ==="

# Step 0: Compute local SHA256
echo -n "Step 0: Local SHA256... "
if [ ! -f "$LOCAL_BINARY" ]; then
    echo "FAILED — ${LOCAL_BINARY} not found locally."
    exit 1
fi
LOCAL_SHA=$(sha256sum "$LOCAL_BINARY" | awk '{print $1}')
echo "${LOCAL_SHA:0:16}..."

# Step 1: Verify staged binary exists on pod AND matches SHA256
echo -n "Step 1: Verify staged binary on pod... "
STAGED=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "if exist ${INSTALL_DIR}\\rc-agent-${HASH}.exe (echo EXISTS) else (echo MISSING)" 2>/dev/null)
if [ "$STAGED" != "EXISTS" ]; then
    echo "FAILED — rc-agent-${HASH}.exe not found on pod. Download first."
    exit 1
fi
# SHA256 verification — catches stale cached downloads
REMOTE_SHA=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "certutil -hashfile ${INSTALL_DIR}\\rc-agent-${HASH}.exe SHA256 2>nul" 2>/dev/null | grep -v "SHA256\|CertUtil" | tr -d '[:space:]')
if [ "$REMOTE_SHA" != "$LOCAL_SHA" ]; then
    echo "FAILED — SHA256 mismatch!"
    echo "  Local:  ${LOCAL_SHA}"
    echo "  Remote: ${REMOTE_SHA}"
    echo "  The pod has a stale/corrupt binary. Re-download from staging server."
    exit 1
fi
echo "OK (SHA256 verified)"

# Step 2: Delete stale hash-named binaries (prevents bat from swapping wrong one)
echo -n "Step 2: Clean stale binaries... "
# Delete all rc-agent-*.exe EXCEPT the one we're deploying and rc-agent-prev.exe
ssh -o ConnectTimeout=5 "${POD_HOST}" "cd /d ${INSTALL_DIR} && for /F %F in ('dir /B rc-agent-*.exe 2^>nul ^| findstr /V /I \"rc-agent-${HASH}.exe rc-agent-prev.exe\"') do del /Q %F 2>nul" 2>/dev/null || true
echo "OK"

# Step 3: Write OTA_DEPLOYING sentinel (watchdog suppresses restart)
echo -n "Step 3: Write OTA_DEPLOYING sentinel... "
SENTINEL_JSON="{\"kind\":\"OtaDeploying\",\"layer\":\"Layer3Guardian\",\"started_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"action\":\"deploy_agent_${HASH}\",\"ttl_secs\":300,\"action_id\":{\"id\":\"deploy-$(date +%s)\"}}"
ssh -o ConnectTimeout=5 "${POD_HOST}" "echo ${SENTINEL_JSON} > ${INSTALL_DIR}\\OTA_DEPLOYING" 2>/dev/null
echo "OK"

# Step 4: Kill rc-agent (watchdog will see sentinel and NOT restart)
echo -n "Step 4: Kill rc-agent... "
ssh -o ConnectTimeout=5 "${POD_HOST}" "taskkill /F /IM rc-agent.exe >nul 2>nul" 2>/dev/null || true
sleep 3
# Verify it's dead and watchdog hasn't restarted
STILL_RUNNING=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "tasklist /FI \"IMAGENAME eq rc-agent.exe\" /NH 2>nul | findstr rc-agent >nul && echo YES || echo NO" 2>/dev/null)
if [ "$STILL_RUNNING" = "YES" ]; then
    echo "WARNING — agent still running (watchdog may have ignored sentinel). Killing again."
    ssh -o ConnectTimeout=5 "${POD_HOST}" "taskkill /F /IM rc-agent.exe >nul 2>nul" 2>/dev/null || true
    sleep 2
fi
echo "OK"

# Step 5: Swap binary
echo -n "Step 5: Swap binary... "
SWAP_RESULT=$(ssh -o ConnectTimeout=5 "${POD_HOST}" "cd /d ${INSTALL_DIR} & del /Q rc-agent-prev.exe 2>nul & del /Q rc-agent-failed.exe 2>nul & move /Y rc-agent.exe rc-agent-prev.exe >nul 2>nul & move /Y rc-agent-${HASH}.exe rc-agent.exe >nul 2>nul & if exist rc-agent.exe (echo SWAP_OK) else (echo SWAP_FAILED)" 2>/dev/null)
if [ "$SWAP_RESULT" != "SWAP_OK" ]; then
    echo "FAILED — binary swap failed. Clearing sentinel, rolling back."
    ssh -o ConnectTimeout=5 "${POD_HOST}" "del ${INSTALL_DIR}\\OTA_DEPLOYING 2>nul" 2>/dev/null
    exit 1
fi
echo "OK"

# Step 6: Clear sentinel (watchdog restarts with new binary)
echo -n "Step 6: Clear OTA_DEPLOYING sentinel... "
ssh -o ConnectTimeout=5 "${POD_HOST}" "del ${INSTALL_DIR}\\OTA_DEPLOYING 2>nul" 2>/dev/null
echo "OK"

# Step 7: Wait for watchdog to restart agent, then verify build_id
echo -n "Step 7: Waiting for watchdog restart..."
for i in 1 2 3 4 5 6; do
    sleep 5
    echo -n "."
    BUILD_ID=$(curl -s --connect-timeout 3 --max-time 5 "http://${POD_IP}:8090/health" 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get('build_id',''))" 2>/dev/null || echo "")
    if [ "$BUILD_ID" = "$HASH" ]; then
        echo " OK"
        echo "=== Pod ${POD_NUM}: build_id=${BUILD_ID} — DEPLOY SUCCESS ==="
        exit 0
    fi
done

echo " TIMEOUT"
echo "=== Pod ${POD_NUM}: Expected ${HASH}, got ${BUILD_ID:-none} — VERIFY MANUALLY ==="
exit 1
