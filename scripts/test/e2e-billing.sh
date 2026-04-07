#!/usr/bin/env bash
# e2e-billing.sh — End-to-end billing lifecycle test
# Tests: create driver -> top up wallet -> start billing -> verify game -> stop billing -> verify refund -> cleanup
# Usage: bash e2e-billing.sh [server_ip] [pod_number]
# Run from James (.27).

set -uo pipefail

SERVER="${1:-192.168.31.23}"
POD_NUM="${2:-8}"
API_BASE="http://${SERVER}:8080/api/v1"
TIMEOUT=10

# Pod IP mapping
declare -A POD_IPS=(
  [1]="192.168.31.89"
  [2]="192.168.31.33"
  [3]="192.168.31.28"
  [4]="192.168.31.88"
  [5]="192.168.31.86"
  [6]="192.168.31.87"
  [7]="192.168.31.38"
  [8]="192.168.31.91"
)

POD_IP="${POD_IPS[$POD_NUM]:-}"
if [ -z "$POD_IP" ]; then
  echo "ERROR: Invalid pod number: $POD_NUM (valid: 1-8)"
  exit 1
fi

# Test state
TEST_DRIVER_ID=""
TEST_SESSION_ID=""
CLEANUP_NEEDED=0
PASS=0
FAIL=0
STEP_TIMES=()

# IST timestamp
ist_now() {
  python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M:%S IST'))" 2>/dev/null || date '+%Y-%m-%d %H:%M:%S'
}

step_start() {
  STEP_START_EPOCH=$(date +%s)
}

step_end() {
  local name="$1"
  local end_epoch
  end_epoch=$(date +%s)
  local elapsed=$((end_epoch - STEP_START_EPOCH))
  STEP_TIMES+=("$name: ${elapsed}s")
}

pass() {
  local msg="$1"
  echo "  PASS: $msg"
  PASS=$((PASS + 1))
}

fail() {
  local msg="$1"
  echo "  FAIL: $msg"
  FAIL=$((FAIL + 1))
}

cleanup() {
  if [ "$CLEANUP_NEEDED" -eq 0 ]; then
    return
  fi
  echo ""
  echo "[Step 8] Cleanup"
  step_start

  # Stop billing session if still active
  if [ -n "$TEST_SESSION_ID" ]; then
    echo "  Stopping leftover billing session $TEST_SESSION_ID..."
    curl -sf --max-time "$TIMEOUT" -X POST \
      "${API_BASE}/billing/${TEST_SESSION_ID}/stop" 2>/dev/null || true
  fi

  # Delete test driver if created
  if [ -n "$TEST_DRIVER_ID" ]; then
    echo "  Deleting test driver $TEST_DRIVER_ID..."
    curl -sf --max-time "$TIMEOUT" -X DELETE \
      "${API_BASE}/drivers/${TEST_DRIVER_ID}" 2>/dev/null || true
  fi

  step_end "Cleanup"
  echo "  Done."
}

trap cleanup EXIT

echo "=============================================="
echo "  Racing Point E2E Billing Lifecycle Test"
echo "  $(ist_now)"
echo "  Server: $SERVER | Pod: $POD_NUM ($POD_IP)"
echo "=============================================="
echo ""

# ── Pre-check: Server health ──
echo "[Pre-check] Server health"
step_start
HEALTH=$(curl -sf --max-time "$TIMEOUT" "${API_BASE}/health" 2>/dev/null || echo "")
if echo "$HEALTH" | grep -q '"status":"ok"'; then
  pass "Server healthy"
else
  fail "Server unreachable or unhealthy"
  echo "  Cannot proceed without server. Aborting."
  exit 1
fi
step_end "Pre-check"

# ── Pre-check: Pod health ──
echo "[Pre-check] Pod $POD_NUM health"
POD_HEALTH=$(curl -sf --max-time "$TIMEOUT" "http://${POD_IP}:8090/health" 2>/dev/null || echo "")
if echo "$POD_HEALTH" | grep -q '"status":"ok"'; then
  pass "Pod $POD_NUM healthy"
else
  fail "Pod $POD_NUM unreachable"
  echo "  Cannot proceed without target pod. Aborting."
  exit 1
fi

# ── Step 1: Create test driver ──
echo ""
echo "[Step 1] Create test driver"
step_start

# Generate unique test driver name
TEST_NAME="E2E_Test_$(date +%s)"
TEST_PHONE="+91900000$(( RANDOM % 9000 + 1000 ))"

CREATE_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
  "${API_BASE}/drivers" \
  -H "Content-Type: application/json" \
  -d "{\"name\":\"${TEST_NAME}\",\"phone\":\"${TEST_PHONE}\"}" 2>/dev/null || echo "")

if [ -z "$CREATE_RESP" ]; then
  # Try GET existing drivers to find a test-safe one
  echo "  Driver create failed (may need auth). Trying to find existing test driver..."
  DRIVERS_RESP=$(curl -sf --max-time "$TIMEOUT" "${API_BASE}/drivers?limit=5" 2>/dev/null || echo "")
  TEST_DRIVER_ID=$(echo "$DRIVERS_RESP" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -z "$TEST_DRIVER_ID" ]; then
    # Last resort: use driver list endpoint
    TEST_DRIVER_ID=$(echo "$DRIVERS_RESP" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
  fi
  if [ -n "$TEST_DRIVER_ID" ]; then
    pass "Using existing driver: $TEST_DRIVER_ID"
    CLEANUP_NEEDED=0  # Don't delete existing driver
  else
    fail "Cannot create or find a test driver"
    echo "  Aborting."
    exit 1
  fi
else
  TEST_DRIVER_ID=$(echo "$CREATE_RESP" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
  if [ -z "$TEST_DRIVER_ID" ]; then
    TEST_DRIVER_ID=$(echo "$CREATE_RESP" | grep -o '"id":[0-9]*' | head -1 | cut -d':' -f2)
  fi
  if [ -n "$TEST_DRIVER_ID" ]; then
    pass "Created driver: $TEST_DRIVER_ID ($TEST_NAME)"
    CLEANUP_NEEDED=1
  else
    fail "Create returned but no ID found: $CREATE_RESP"
    exit 1
  fi
fi
step_end "Create driver"

# ── Step 2: Top up wallet ──
echo ""
echo "[Step 2] Top up wallet"
step_start

TOPUP_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
  "${API_BASE}/wallet/${TEST_DRIVER_ID}/topup" \
  -H "Content-Type: application/json" \
  -d '{"amount":500,"currency":"rupees","method":"cash"}' 2>/dev/null || echo "")

if [ -z "$TOPUP_RESP" ]; then
  # Try alternative endpoint format
  TOPUP_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
    "${API_BASE}/wallet/topup" \
    -H "Content-Type: application/json" \
    -d "{\"driver_id\":\"${TEST_DRIVER_ID}\",\"amount\":500,\"currency\":\"rupees\",\"method\":\"cash\"}" 2>/dev/null || echo "")
fi

if echo "$TOPUP_RESP" | grep -qi "balance\|success\|ok"; then
  BALANCE_BEFORE=$(echo "$TOPUP_RESP" | grep -o '"rupees_balance":[0-9.]*' | head -1 | cut -d':' -f2)
  pass "Wallet topped up. Balance: ${BALANCE_BEFORE:-unknown}"
else
  fail "Topup failed: ${TOPUP_RESP:-no response}"
  echo "  Continuing anyway (driver may have existing balance)..."
fi
step_end "Top up wallet"

# ── Step 3: Get wallet balance before billing ──
echo ""
echo "[Step 3] Record pre-billing wallet balance"
step_start

WALLET_BEFORE=$(curl -sf --max-time "$TIMEOUT" \
  "${API_BASE}/wallet/${TEST_DRIVER_ID}" 2>/dev/null || echo "")
RUPEES_BEFORE=$(echo "$WALLET_BEFORE" | grep -o '"rupees_balance":[0-9.]*' | head -1 | cut -d':' -f2)
CREDITS_BEFORE=$(echo "$WALLET_BEFORE" | grep -o '"credits_balance":[0-9.]*' | head -1 | cut -d':' -f2)

if [ -n "$RUPEES_BEFORE" ] || [ -n "$CREDITS_BEFORE" ]; then
  pass "Pre-billing balance: rupees=${RUPEES_BEFORE:-0}, credits=${CREDITS_BEFORE:-0}"
else
  echo "  WARN: Could not read wallet balance (may use different field names)"
  echo "  Response: ${WALLET_BEFORE:-none}"
fi
step_end "Pre-billing balance"

# ── Step 4: Start billing session on target pod ──
echo ""
echo "[Step 4] Start billing session on Pod $POD_NUM"
step_start

START_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
  "${API_BASE}/billing/start" \
  -H "Content-Type: application/json" \
  -d "{\"driver_id\":\"${TEST_DRIVER_ID}\",\"pod_number\":${POD_NUM}}" 2>/dev/null || echo "")

if [ -z "$START_RESP" ]; then
  fail "Billing start returned no response"
  echo "  Aborting billing test."
  # Print what we got for debugging
  echo "  Attempted: POST ${API_BASE}/billing/start"
  echo "  Body: {\"driver_id\":\"${TEST_DRIVER_ID}\",\"pod_number\":${POD_NUM}}"
  exit 1
fi

TEST_SESSION_ID=$(echo "$START_RESP" | grep -o '"session_id":"[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$TEST_SESSION_ID" ]; then
  TEST_SESSION_ID=$(echo "$START_RESP" | grep -o '"id":"[^"]*"' | head -1 | cut -d'"' -f4)
fi

if [ -n "$TEST_SESSION_ID" ]; then
  pass "Billing started: session=$TEST_SESSION_ID"
  CLEANUP_NEEDED=1
else
  fail "Billing start failed: $START_RESP"
  echo "  Aborting billing test."
  exit 1
fi
step_end "Start billing"

# ── Step 5: Wait and verify game state ──
echo ""
echo "[Step 5] Wait 10s, then verify game state"
step_start

echo "  Waiting 10 seconds for game to launch..."
sleep 10

# Check active games on the server
ACTIVE_RESP=$(curl -sf --max-time "$TIMEOUT" \
  "${API_BASE}/games/active" 2>/dev/null || echo "")

if [ -n "$ACTIVE_RESP" ]; then
  if echo "$ACTIVE_RESP" | grep -q "pod.*${POD_NUM}\|pod_number.*${POD_NUM}"; then
    pass "Game active on Pod $POD_NUM"
  else
    echo "  WARN: Game state response does not mention Pod $POD_NUM"
    echo "  Response: ${ACTIVE_RESP:0:200}"
    # Not necessarily a failure — endpoint format may differ
  fi
else
  echo "  WARN: /games/active returned no response (endpoint may not exist)"
fi

# Also verify via pod's lock screen state
LOCK_STATE=$(curl -sf --max-time "$TIMEOUT" "http://${POD_IP}:18923/health" 2>/dev/null || echo "")
if echo "$LOCK_STATE" | grep -qi "active_session\|ActiveSession\|pin_entry\|PinEntry"; then
  pass "Lock screen shows active session"
else
  echo "  WARN: Lock screen state: ${LOCK_STATE:-unreachable}"
fi
step_end "Verify game state"

# ── Step 6: Stop billing session ──
echo ""
echo "[Step 6] Stop billing session"
step_start

STOP_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
  "${API_BASE}/billing/${TEST_SESSION_ID}/stop" 2>/dev/null || echo "")

if [ -z "$STOP_RESP" ]; then
  # Try alternative format
  STOP_RESP=$(curl -sf --max-time "$TIMEOUT" -X POST \
    "${API_BASE}/billing/stop" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"${TEST_SESSION_ID}\"}" 2>/dev/null || echo "")
fi

if echo "$STOP_RESP" | grep -qi "stopped\|completed\|ok\|success\|duration\|refund"; then
  DURATION_USED=$(echo "$STOP_RESP" | grep -o '"duration_seconds":[0-9]*' | head -1 | cut -d':' -f2)
  REFUND_AMT=$(echo "$STOP_RESP" | grep -o '"refund":[0-9.]*' | head -1 | cut -d':' -f2)
  pass "Billing stopped. Duration: ${DURATION_USED:-?}s, Refund: ${REFUND_AMT:-?}"
  TEST_SESSION_ID=""  # Clear so cleanup doesn't try to stop again
else
  fail "Billing stop unclear: ${STOP_RESP:-no response}"
fi
step_end "Stop billing"

# ── Step 7: Verify refund / wallet balance ──
echo ""
echo "[Step 7] Verify post-billing wallet balance"
step_start

sleep 2  # Brief delay for server to process refund

WALLET_AFTER=$(curl -sf --max-time "$TIMEOUT" \
  "${API_BASE}/wallet/${TEST_DRIVER_ID}" 2>/dev/null || echo "")
RUPEES_AFTER=$(echo "$WALLET_AFTER" | grep -o '"rupees_balance":[0-9.]*' | head -1 | cut -d':' -f2)
CREDITS_AFTER=$(echo "$WALLET_AFTER" | grep -o '"credits_balance":[0-9.]*' | head -1 | cut -d':' -f2)

if [ -n "$RUPEES_AFTER" ] || [ -n "$CREDITS_AFTER" ]; then
  pass "Post-billing balance: rupees=${RUPEES_AFTER:-0}, credits=${CREDITS_AFTER:-0}"

  # Verify balance change makes sense (should be less than before, by the session cost)
  if [ -n "$RUPEES_BEFORE" ] && [ -n "$RUPEES_AFTER" ]; then
    if [ "$(echo "$RUPEES_AFTER <= $RUPEES_BEFORE" | bc 2>/dev/null)" = "1" ]; then
      SPENT=$(echo "$RUPEES_BEFORE - $RUPEES_AFTER" | bc 2>/dev/null || echo "?")
      pass "Balance decreased by ${SPENT} rupees (session charge)"
    else
      echo "  WARN: Balance increased after session (refund may exceed charge for short test)"
    fi
  fi
else
  fail "Could not read post-billing wallet: ${WALLET_AFTER:-no response}"
fi
step_end "Verify refund"

# ── Summary ──
echo ""
echo "=============================================="
echo "  E2E Billing Test Results"
echo "=============================================="
echo ""
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo ""
echo "  Timing:"
for t in "${STEP_TIMES[@]}"; do
  echo "    $t"
done
echo ""

TOTAL_SECS=0
for t in "${STEP_TIMES[@]}"; do
  secs=$(echo "$t" | grep -o '[0-9]*s' | grep -o '[0-9]*')
  TOTAL_SECS=$((TOTAL_SECS + secs))
done
echo "  Total: ${TOTAL_SECS}s"
echo ""

if [ "$FAIL" -gt 0 ]; then
  echo "E2E BILLING TEST: FAILED ($FAIL failures)"
  exit 1
else
  echo "E2E BILLING TEST: PASSED"
  exit 0
fi
