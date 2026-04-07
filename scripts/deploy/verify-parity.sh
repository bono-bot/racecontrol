#!/usr/bin/env bash
# verify-parity.sh — Cross-target build_id parity check for Racing Point fleet
# Compares build_id across server, all pods, POS, and cloud.
# Usage: bash verify-parity.sh [expected_build_id]
# Run from James (.27).

set -uo pipefail

EXPECTED="${1:-}"
TIMEOUT=5

# Collect results: "target|binary|build_id|match"
declare -a ROWS=()
MISMATCH=0
UNREACHABLE=0

query_build_id() {
  local target="$1"
  local binary="$2"
  local url="$3"

  local resp
  resp=$(curl -sf --max-time "$TIMEOUT" "$url" 2>/dev/null || echo "")
  if [ -z "$resp" ]; then
    ROWS+=("$target|$binary|UNREACHABLE|--")
    UNREACHABLE=$((UNREACHABLE + 1))
    return
  fi

  local bid
  bid=$(echo "$resp" | grep -o '"build_id":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -z "$bid" ]; then
    bid="(not in response)"
  fi
  ROWS+=("$target|$binary|$bid|pending")
}

echo "=============================================="
echo "  Racing Point Fleet Parity Check"
echo "  $(date '+%Y-%m-%d %H:%M') IST"
echo "=============================================="
echo ""

# ── Query all targets ──

# Server .23 — racecontrol on :8080
query_build_id "Server .23" "racecontrol" "http://192.168.31.23:8080/api/v1/health"

# Pods 1-8 — rc-agent on :8090
declare -A POD_IPS=(
  ["Pod 1"]="192.168.31.89"
  ["Pod 2"]="192.168.31.33"
  ["Pod 3"]="192.168.31.28"
  ["Pod 4"]="192.168.31.88"
  ["Pod 5"]="192.168.31.86"
  ["Pod 6"]="192.168.31.87"
  ["Pod 7"]="192.168.31.38"
  ["Pod 8"]="192.168.31.91"
)

for name in "Pod 1" "Pod 2" "Pod 3" "Pod 4" "Pod 5" "Pod 6" "Pod 7" "Pod 8"; do
  ip="${POD_IPS[$name]}"
  query_build_id "$name" "rc-agent" "http://${ip}:8090/health"
done

# POS .20 — rc-agent on :8090
query_build_id "POS .20" "rc-agent" "http://192.168.31.20:8090/health"

# Cloud (Bono VPS) — racecontrol on :8080 via Tailscale
query_build_id "Cloud VPS" "racecontrol" "http://100.70.177.44:8080/api/v1/health"

# ── Determine expected build_id ──
# If not provided, use the server's build_id as the reference
if [ -z "$EXPECTED" ]; then
  for row in "${ROWS[@]}"; do
    local_target=$(echo "$row" | cut -d'|' -f1)
    local_bid=$(echo "$row" | cut -d'|' -f3)
    if [ "$local_target" = "Server .23" ] && [ "$local_bid" != "UNREACHABLE" ] && [ "$local_bid" != "(not in response)" ]; then
      EXPECTED="$local_bid"
      break
    fi
  done
  if [ -z "$EXPECTED" ]; then
    echo "WARNING: Could not determine expected build_id (server unreachable)"
    echo "Pass expected build_id as argument: bash verify-parity.sh <build_id>"
  fi
fi

echo "Expected build_id: ${EXPECTED:-"(auto-detect failed)"}"
echo ""

# ── Print table ──
printf "| %-12s | %-12s | %-14s | %-14s | %-5s |\n" \
  "Target" "Binary" "Build ID" "Expected" "Match"
printf "| %-12s | %-12s | %-14s | %-14s | %-5s |\n" \
  "------------" "------------" "--------------" "--------------" "-----"

for row in "${ROWS[@]}"; do
  target=$(echo "$row" | cut -d'|' -f1)
  binary=$(echo "$row" | cut -d'|' -f2)
  bid=$(echo "$row" | cut -d'|' -f3)

  if [ "$bid" = "UNREACHABLE" ]; then
    match="N/A"
    UNREACHABLE=$((UNREACHABLE)) # already counted
  elif [ -z "$EXPECTED" ]; then
    match="?"
  elif [ "$bid" = "$EXPECTED" ]; then
    match="YES"
  else
    match="NO"
    MISMATCH=$((MISMATCH + 1))
  fi

  printf "| %-12s | %-12s | %-14s | %-14s | %-5s |\n" \
    "$target" "$binary" "$bid" "${EXPECTED:-"?"}" "$match"
done

echo ""

# ── Summary ──
TOTAL=${#ROWS[@]}
MATCHED=$((TOTAL - MISMATCH - UNREACHABLE))

echo "Fleet: $TOTAL targets checked"
echo "  Matched:     $MATCHED"
echo "  Mismatched:  $MISMATCH"
echo "  Unreachable: $UNREACHABLE"
echo ""

if [ "$MISMATCH" -gt 0 ]; then
  echo "PARITY CHECK: FAILED — $MISMATCH target(s) have different build_id"
  echo ""
  echo "Mismatched targets need redeployment:"
  for row in "${ROWS[@]}"; do
    target=$(echo "$row" | cut -d'|' -f1)
    bid=$(echo "$row" | cut -d'|' -f3)
    if [ -n "$EXPECTED" ] && [ "$bid" != "$EXPECTED" ] && [ "$bid" != "UNREACHABLE" ]; then
      echo "  - $target: has $bid, expected $EXPECTED"
    fi
  done
  exit 1
elif [ "$UNREACHABLE" -gt 0 ]; then
  echo "PARITY CHECK: WARNING — $UNREACHABLE target(s) unreachable"
  exit 2
else
  echo "PARITY CHECK: PASSED — all targets at build_id $EXPECTED"
  exit 0
fi
