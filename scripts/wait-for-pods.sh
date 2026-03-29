#!/bin/bash
# wait-for-pods.sh — Retry-based pod readiness checker
# MMA consensus fix: 9/9 models agreed single-shot probes cause false "off" conclusions
#
# Usage:
#   bash scripts/wait-for-pods.sh                    # Wait for all 8 pods
#   bash scripts/wait-for-pods.sh 192.168.31.89      # Wait for specific IP
#   bash scripts/wait-for-pods.sh --timeout 60       # Custom timeout (default 150s)
#
# Exit codes:
#   0 = all pods ready
#   1 = one or more pods failed to become ready
#
# NEVER concludes "powered off" — only "READY" or "NOT READY after Ns"

set -euo pipefail

# All 8 pod IPs + POS
ALL_PODS=(
  "pod1:192.168.31.89"
  "pod2:192.168.31.33"
  "pod3:192.168.31.28"
  "pod4:192.168.31.88"
  "pod5:192.168.31.86"
  "pod6:192.168.31.87"
  "pod7:192.168.31.38"
  "pod8:192.168.31.91"
  "pos:192.168.31.20"
)

MAX_WAIT=150       # 150s > worst-case 120s boot
PROBE_INTERVAL=10  # seconds between attempts
CONNECT_TIMEOUT=3  # per-attempt TCP timeout

# Parse args
TARGETS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout) MAX_WAIT="$2"; shift 2 ;;
    --interval) PROBE_INTERVAL="$2"; shift 2 ;;
    --pods-only)
      # Exclude POS
      for p in "${ALL_PODS[@]}"; do
        [[ "$p" != pos:* ]] && TARGETS+=("$p")
      done
      shift ;;
    *)
      # Specific IP passed — wrap it
      TARGETS+=("custom:$1")
      shift ;;
  esac
done

# Default: all pods
[[ ${#TARGETS[@]} -eq 0 ]] && TARGETS=("${ALL_PODS[@]}")

# Probe a single pod — returns 0 if ready, 1 if not
probe_pod() {
  local ip="$1"
  # Layer 1: ICMP ping (host alive?)
  ping -c 1 -W "$CONNECT_TIMEOUT" "$ip" >/dev/null 2>&1 || return 1
  # Layer 2: rc-sentry HTTP health (service ready?)
  curl -sf --connect-timeout "$CONNECT_TIMEOUT" --max-time 5 \
    "http://${ip}:8091/ping" >/dev/null 2>&1 || return 1
  return 0
}

# Wait for a single pod with retry loop
wait_for_pod() {
  local name="$1"
  local ip="$2"
  local elapsed=0
  local attempt=0

  while [ "$elapsed" -lt "$MAX_WAIT" ]; do
    attempt=$((attempt + 1))

    if probe_pod "$ip"; then
      # Also try rc-agent for build info
      local build
      build=$(curl -sf --connect-timeout "$CONNECT_TIMEOUT" --max-time 5 \
        "http://${ip}:8090/health" 2>/dev/null | \
        python3 -c "import sys,json; print(json.load(sys.stdin).get('build_id','?'))" 2>/dev/null || echo "sentry-only")
      echo "READY  ${name} (${ip}) — ${elapsed}s, build=${build}"
      return 0
    fi

    echo "WAIT   ${name} (${ip}) — attempt ${attempt}, ${elapsed}s elapsed"
    sleep "$PROBE_INTERVAL"
    elapsed=$((elapsed + PROBE_INTERVAL))
  done

  echo "TIMEOUT ${name} (${ip}) — NOT READY after ${MAX_WAIT}s (NOT assuming powered off)"
  return 1
}

echo "=== Pod Readiness Check ==="
echo "Targets: ${#TARGETS[@]} | Max wait: ${MAX_WAIT}s | Interval: ${PROBE_INTERVAL}s"
echo ""

FAILED=0
READY=0

for target in "${TARGETS[@]}"; do
  name="${target%%:*}"
  ip="${target##*:}"

  if wait_for_pod "$name" "$ip"; then
    READY=$((READY + 1))
  else
    FAILED=$((FAILED + 1))
  fi
done

echo ""
echo "=== Results: ${READY} ready, ${FAILED} not ready ==="

if [ "$FAILED" -gt 0 ]; then
  echo "WARNING: ${FAILED} target(s) did not become ready."
  echo "Investigate manually — do NOT assume they are powered off."
  exit 1
fi

exit 0
