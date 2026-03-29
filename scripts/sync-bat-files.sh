#!/usr/bin/env bash
# sync-bat-files.sh — Automated bat file sync to all pods (Layer 8 fix)
#
# Ensures all pods have the latest start-rcagent.bat and start-rcsentry.bat
# from the repo. Prevents settings regression from stale bat files.
#
# Usage: bash scripts/sync-bat-files.sh [--dry-run]

set -e

STAGING="C:/Users/bono/racingpoint/deploy-staging"
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SENTRY_PORT=8091
DRY_RUN="${1:-}"

POD_IPS=(192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91)
POD_NAMES=(Pod-1 Pod-2 Pod-3 Pod-4 Pod-5 Pod-6 Pod-7 Pod-8)

BAT_FILES=("start-rcagent.bat" "start-rcsentry.bat")

echo "============================================================"
echo "BAT FILE SYNC — All Pods"
echo "============================================================"

PASS=0; FAIL=0; SKIP=0

for i in "${!POD_IPS[@]}"; do
  pod_ip="${POD_IPS[$i]}"
  pod_name="${POD_NAMES[$i]}"
  echo ""
  echo "--- ${pod_name} (${pod_ip}) ---"

  for bat in "${BAT_FILES[@]}"; do
    SRC="${STAGING}/${bat}"
    if [ ! -f "$SRC" ]; then
      echo "  SKIP ${bat}: not in staging"
      ((SKIP++))
      continue
    fi

    if [ "$DRY_RUN" = "--dry-run" ]; then
      echo "  DRY-RUN: would sync ${bat}"
      continue
    fi

    # MMA iter1: atomic write — download to .tmp, then rename (prevents partial file corruption)
    RESULT=$(curl -s --connect-timeout 3 -X POST "http://${pod_ip}:${SENTRY_PORT}/exec" \
      -H "Content-Type: application/json" \
      -d "{\"cmd\":\"curl.exe -s -o C:\\\\RacingPoint\\\\${bat}.tmp http://192.168.31.27:18889/${bat} && move /Y C:\\\\RacingPoint\\\\${bat}.tmp C:\\\\RacingPoint\\\\${bat}\"}" \
      2>/dev/null || echo '{"error":"unreachable"}')

    if echo "$RESULT" | grep -q '"success":true\|"exit_code":0'; then
      echo "  OK: ${bat} synced"
      ((PASS++))
    else
      echo "  FAIL: ${bat} sync failed"
      ((FAIL++))
    fi
  done
done

echo ""
echo "============================================================"
echo "RESULTS: ${PASS} synced | ${FAIL} failed | ${SKIP} skipped"
echo "============================================================"
