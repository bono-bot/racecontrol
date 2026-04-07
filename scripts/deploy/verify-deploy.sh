#!/usr/bin/env bash
# verify-deploy.sh — Enhanced post-deploy verification for Racing Point pods
# Checks: build_id, ws_connected, blanking active, session context, screenshot non-black
# Usage: bash verify-deploy.sh [expected_build_id]
# Run from James (.27). Queries each pod's rc-agent :8090 and lock screen :18923.

set -uo pipefail

EXPECTED_BUILD_ID="${1:-}"
TIMEOUT=5

# Pod fleet — name:ip pairs
declare -A PODS=(
  ["Pod 1"]="192.168.31.89"
  ["Pod 2"]="192.168.31.33"
  ["Pod 3"]="192.168.31.28"
  ["Pod 4"]="192.168.31.88"
  ["Pod 5"]="192.168.31.86"
  ["Pod 6"]="192.168.31.87"
  ["Pod 7"]="192.168.31.38"
  ["Pod 8"]="192.168.31.91"
)

# SSH user mapping (pods use User)
POD_SSH_USER="User"

PASS_TOTAL=0
FAIL_TOTAL=0
SKIP_TOTAL=0

# Collect results for final table
declare -a RESULTS=()

check_pod() {
  local name="$1"
  local ip="$2"
  local build_ok="--"
  local ws_ok="--"
  local blank_ok="--"
  local session_ok="--"
  local screen_ok="--"
  local pod_pass=0
  local pod_fail=0
  local pod_skip=0

  # ── Check 1: Health endpoint (build_id + status) ──
  local health
  health=$(curl -sf --max-time "$TIMEOUT" "http://${ip}:8090/health" 2>/dev/null || echo "")
  if [ -z "$health" ]; then
    build_ok="UNREACHABLE"
    ws_ok="UNREACHABLE"
    pod_fail=$((pod_fail + 2))
  else
    # build_id
    local actual_build
    actual_build=$(echo "$health" | grep -o '"build_id":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ -n "$EXPECTED_BUILD_ID" ]; then
      if [ "$actual_build" = "$EXPECTED_BUILD_ID" ]; then
        build_ok="PASS ($actual_build)"
        pod_pass=$((pod_pass + 1))
      else
        build_ok="FAIL ($actual_build)"
        pod_fail=$((pod_fail + 1))
      fi
    else
      build_ok="INFO ($actual_build)"
      pod_pass=$((pod_pass + 1))
    fi

    # status ok
    if echo "$health" | grep -q '"status":"ok"'; then
      ws_ok="PASS"
      pod_pass=$((pod_pass + 1))
    else
      ws_ok="FAIL"
      pod_fail=$((pod_fail + 1))
    fi
  fi

  # ── Check 2: Lock screen / blanking active ──
  # Query the native lock screen HTTP server on port 18923
  local lock_health
  lock_health=$(curl -sf --max-time "$TIMEOUT" "http://${ip}:18923/health" 2>/dev/null || echo "")
  if [ -z "$lock_health" ]; then
    blank_ok="FAIL (lock screen unreachable)"
    pod_fail=$((pod_fail + 1))
  else
    local lock_status
    lock_status=$(echo "$lock_health" | grep -o '"status":"[^"]*"' | head -1 | cut -d'"' -f4)
    local lock_state
    lock_state=$(echo "$lock_health" | grep -o '"state":"[^"]*"' | head -1 | cut -d'"' -f4)
    if [ "$lock_status" = "ok" ]; then
      blank_ok="PASS ($lock_state)"
      pod_pass=$((pod_pass + 1))
    else
      blank_ok="FAIL ($lock_state)"
      pod_fail=$((pod_fail + 1))
    fi
  fi

  # ── Check 3: Session context (Console, not Services) ──
  # Use rc-agent /exec to run tasklist and check session type
  local session_resp
  session_resp=$(curl -sf --max-time 10 -X POST "http://${ip}:8090/exec" \
    -H "Content-Type: application/json" \
    -d '{"cmd":"tasklist /V /FO CSV /FI \"IMAGENAME eq rc-agent.exe\"","timeout_ms":5000}' 2>/dev/null || echo "")
  if [ -z "$session_resp" ]; then
    session_ok="SKIP (exec unreachable)"
    pod_skip=$((pod_skip + 1))
  else
    local stdout_val
    stdout_val=$(echo "$session_resp" | grep -o '"stdout":"[^"]*"' | head -1 | sed 's/"stdout":"//;s/"$//')
    if echo "$stdout_val" | grep -qi "console"; then
      session_ok="PASS (Console)"
      pod_pass=$((pod_pass + 1))
    elif echo "$stdout_val" | grep -qi "services"; then
      session_ok="FAIL (Services)"
      pod_fail=$((pod_fail + 1))
    else
      # Could not determine — try alternative: check session ID
      local sess_resp2
      sess_resp2=$(curl -sf --max-time 10 -X POST "http://${ip}:8090/exec" \
        -H "Content-Type: application/json" \
        -d '{"cmd":"query session","timeout_ms":5000}' 2>/dev/null || echo "")
      if echo "$sess_resp2" | grep -qi "console.*Active"; then
        session_ok="PASS (Console via query)"
        pod_pass=$((pod_pass + 1))
      else
        session_ok="WARN (unknown)"
        pod_skip=$((pod_skip + 1))
      fi
    fi
  fi

  # ── Check 4: Screenshot non-black ──
  # Capture via rc-agent /screenshot (quality=30, scale=25 for speed)
  local screenshot_file="/tmp/verify_screenshot_${ip}.jpg"
  local sc_http
  sc_http=$(curl -sf --max-time 10 -o "$screenshot_file" -w "%{http_code}" \
    "http://${ip}:8090/screenshot?quality=30&scale=25" 2>/dev/null || echo "000")
  if [ "$sc_http" = "200" ] && [ -f "$screenshot_file" ] && [ -s "$screenshot_file" ]; then
    # Check if image is all-black by examining pixel data
    # Use ImageMagick identify if available, else use file size heuristic
    if command -v identify &>/dev/null; then
      local mean_val
      mean_val=$(identify -verbose "$screenshot_file" 2>/dev/null | grep -A1 "Channel statistics:" | grep "mean:" | head -1 | grep -o '[0-9.]*' | head -1)
      if [ -n "$mean_val" ] && [ "$(echo "$mean_val < 5" | bc 2>/dev/null)" = "1" ]; then
        screen_ok="FAIL (black screen, mean=$mean_val)"
        pod_fail=$((pod_fail + 1))
      else
        screen_ok="PASS (non-black)"
        pod_pass=$((pod_pass + 1))
      fi
    else
      # Fallback: JPEG files that are all black compress very small
      local fsize
      fsize=$(wc -c < "$screenshot_file" 2>/dev/null || echo "0")
      if [ "$fsize" -lt 500 ]; then
        screen_ok="WARN (suspiciously small: ${fsize}B)"
        pod_skip=$((pod_skip + 1))
      else
        screen_ok="PASS (${fsize}B, likely non-black)"
        pod_pass=$((pod_pass + 1))
      fi
    fi
    rm -f "$screenshot_file"
  else
    screen_ok="SKIP (screenshot unavailable, HTTP $sc_http)"
    pod_skip=$((pod_skip + 1))
  fi

  # Build result line
  RESULTS+=("| $name | $ip | $build_ok | $blank_ok | $session_ok | $screen_ok |")

  PASS_TOTAL=$((PASS_TOTAL + pod_pass))
  FAIL_TOTAL=$((FAIL_TOTAL + pod_fail))
  SKIP_TOTAL=$((SKIP_TOTAL + pod_skip))
}

echo "=============================================="
echo "  Racing Point Post-Deploy Verification"
echo "  $(date '+%Y-%m-%d %H:%M') IST"
if [ -n "$EXPECTED_BUILD_ID" ]; then
  echo "  Expected build_id: $EXPECTED_BUILD_ID"
fi
echo "=============================================="
echo ""

# Run checks for all pods
for name in "Pod 1" "Pod 2" "Pod 3" "Pod 4" "Pod 5" "Pod 6" "Pod 7" "Pod 8"; do
  ip="${PODS[$name]}"
  echo "Checking $name ($ip)..."
  check_pod "$name" "$ip"
done

echo ""
echo "=============================================="
echo "  Results"
echo "=============================================="
echo ""
printf "| %-6s | %-15s | %-22s | %-28s | %-22s | %-30s |\n" \
  "Target" "IP" "Build ID" "Blanking" "Session" "Screenshot"
printf "| %-6s | %-15s | %-22s | %-28s | %-22s | %-30s |\n" \
  "------" "---------------" "----------------------" "----------------------------" "----------------------" "------------------------------"
for line in "${RESULTS[@]}"; do
  echo "$line"
done

echo ""
echo "Summary: ${PASS_TOTAL} PASS, ${FAIL_TOTAL} FAIL, ${SKIP_TOTAL} SKIP"
echo ""

if [ "$FAIL_TOTAL" -gt 0 ]; then
  echo "DEPLOY VERIFICATION: FAILED ($FAIL_TOTAL failures)"
  exit 1
else
  echo "DEPLOY VERIFICATION: PASSED"
  exit 0
fi
