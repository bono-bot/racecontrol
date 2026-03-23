#!/usr/bin/env bash
# run-cross-sync.sh — Cross-cutting real-time sync tests for Phase 175 E2E
# Requires: two browser windows open (POS at :3200, Kiosk at :3300)
# Usage: bash test/e2e/run-cross-sync.sh
# This script guides you through the 21 cross-cutting tests in sections 3.1–3.4
#
# Sections covered:
#   3.1  Responsiveness & Display    (5 tests — manual only)
#   3.2  Real-Time Updates           (5 tests — manual + curl state verification)
#   3.3  Error Handling              (5 tests — manual + connectivity simulation)
#   3.4  Edge Cases                  (6 tests — manual + partial automation for timezone)

set -euo pipefail

# ── Config ─────────────────────────────────────────────────────────────────────
SERVER="192.168.31.23"
DATE=$(date '+%Y-%m-%d')
REPORT="test/e2e/E2E-TEST-RESULTS-${DATE}.md"
PASS=0
FAIL=0
SKIP=0

# ── Colours ─────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ── Helpers ──────────────────────────────────────────────────────────────────────
header() { echo -e "\n${BOLD}${CYAN}═══ $* ═══${RESET}"; }
step()   { echo -e "${YELLOW}  ▶ $*${RESET}"; }
info()   { echo -e "    $*"; }
ok()     { echo -e "${GREEN}  ✔ PASS${RESET}"; }
fail()   { echo -e "${RED}  ✘ FAIL${RESET}"; }

record_result() {
  local test_id="$1"
  local test_desc="$2"
  local result="$3"
  local section="${test_id%.*}"    # e.g. "3.2" from "3.2.1"

  if [[ "$result" == "pass" ]]; then
    PASS=$((PASS + 1))
    ok
  elif [[ "$result" == "skip" ]]; then
    SKIP=$((SKIP + 1))
    echo -e "${YELLOW}  → SKIP${RESET}"
  else
    FAIL=$((FAIL + 1))
    fail
  fi

  # Append detail row to report
  if [[ -f "$REPORT" ]]; then
    echo "| ${test_id} | ${test_desc} | ${result^^} | |" >> "$REPORT"
  fi
}

ask_result() {
  local test_id="$1"
  local test_desc="$2"
  local answer
  echo ""
  read -rp "  [${test_id}] Result? [pass/fail/skip]: " answer
  answer="${answer,,}"
  if [[ "$answer" != "pass" && "$answer" != "fail" && "$answer" != "skip" ]]; then
    answer="fail"
  fi
  record_result "$test_id" "$test_desc" "$answer"
}

fetch_fleet_health() {
  echo ""
  echo -e "  ${CYAN}[curl] Current fleet/health state:${RESET}"
  curl -s --max-time 5 "http://${SERVER}:8080/api/v1/fleet/health" \
    | python3 -m json.tool 2>/dev/null \
    || curl -s --max-time 5 "http://${SERVER}:8080/api/v1/fleet/health" \
    || echo "  (could not reach fleet/health — server may be offline)"
}

fetch_sessions() {
  echo ""
  echo -e "  ${CYAN}[curl] Current sessions:${RESET}"
  curl -s --max-time 5 "http://${SERVER}:8080/api/v1/sessions" \
    | python3 -m json.tool 2>/dev/null \
    || curl -s --max-time 5 "http://${SERVER}:8080/api/v1/sessions" \
    || echo "  (could not reach sessions — server may be offline)"
}

# ── Pre-flight health check ─────────────────────────────────────────────────────
echo -e "\n${BOLD}run-cross-sync.sh — Cross-cutting E2E Tests (Sections 3.1–3.4)${RESET}"
echo -e "Date: ${DATE}\n"

# Try check-health.sh first; fall back to inline health probe
HEALTH_SCRIPT="$(dirname "$0")/../../deploy-staging/check-health.sh"
if [[ -f "$HEALTH_SCRIPT" ]]; then
  echo "Running pre-flight health check..."
  bash "$HEALTH_SCRIPT" || {
    echo -e "${RED}Pre-flight FAILED — one or more services are down. Aborting.${RESET}"
    exit 1
  }
else
  echo "Checking racecontrol API (inline fallback)..."
  if ! curl -sf --max-time 5 "http://${SERVER}:8080/api/v1/fleet/health" > /dev/null; then
    echo -e "${RED}Cannot reach racecontrol at ${SERVER}:8080. Is the server running?${RESET}"
    echo -e "${RED}Aborting cross-sync tests.${RESET}"
    exit 1
  fi
  if ! curl -sf --max-time 5 "http://${SERVER}:3200" > /dev/null; then
    echo -e "${YELLOW}Warning: POS (:3200) not reachable. Tests may fail.${RESET}"
  fi
  if ! curl -sf --max-time 5 "http://${SERVER}:3300" > /dev/null; then
    echo -e "${YELLOW}Warning: Kiosk (:3300) not reachable. Tests may fail.${RESET}"
  fi
fi

# ── Setup instructions ─────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}SETUP REQUIRED BEFORE STARTING${RESET}"
echo "───────────────────────────────────────────────────────────────"
echo "  Browser A (POS):   http://${SERVER}:3200"
echo "  Browser B (Kiosk): http://${SERVER}:3300"
echo ""
echo "  1. Open POS in Browser A"
echo "  2. Log into POS with your staff PIN"
echo "  3. Open Kiosk in Browser B (separate window or second monitor)"
echo "  4. Ensure at least one pod is idle and not in use"
echo "  5. Note the pod number you will use for tests (e.g. Pod 1)"
echo ""
read -rp "Press Enter when both browsers are open and you are ready..."

# ── Initialise report section ───────────────────────────────────────────────────
if [[ -f "$REPORT" ]]; then
  echo "" >> "$REPORT"
  echo "---" >> "$REPORT"
  echo "" >> "$REPORT"
  echo "## Cross-Sync Tests (Sections 3.1–3.4) — ${DATE}" >> "$REPORT"
  echo "" >> "$REPORT"
  echo "| Test ID | Description | Result | Comments |" >> "$REPORT"
  echo "|---------|-------------|--------|----------|" >> "$REPORT"
else
  cat > "$REPORT" <<EOF
# E2E Test Results — ${DATE}

Run by: run-cross-sync.sh (cross-cutting sections only)

## Cross-Sync Tests (Sections 3.1–3.4) — ${DATE}

| Test ID | Description | Result | Comments |
|---------|-------------|--------|----------|
EOF
fi

# ══════════════════════════════════════════════════════════════════════════════
# SECTION 3.1 — Responsiveness & Display
# ══════════════════════════════════════════════════════════════════════════════
header "SECTION 3.1 — Responsiveness & Display (5 tests)"

echo -e "${BOLD}
These tests verify layout at full resolution. Use Browser A (POS) and Browser B (Kiosk).
Set both browsers to full-screen (F11) at 1920×1080 before proceeding.${RESET}
"

# 3.1.1
step "TEST 3.1.1: Kiosk at 1920×1080"
info "In Browser B (Kiosk), check that:"
info "  - All 8 pod slots are visible without vertical scrolling"
info "  - The pod grid fits on one screen with no cutoff"
ask_result "3.1.1" "Kiosk at 1920x1080 — all 8 pods visible no scroll"

# 3.1.2
step "TEST 3.1.2: POS at full screen"
info "In Browser A (POS), check that:"
info "  - Sidebar and main content area fit side-by-side"
info "  - No sidebar item is cut off or hidden"
info "  - No horizontal scroll bar appears"
ask_result "3.1.2" "POS at full screen — sidebar + content fit without overlap"

# 3.1.3
step "TEST 3.1.3: Modal sizing"
info "In Browser A (POS):"
info "  1. Go to /billing"
info "  2. Click Start on an idle pod"
info "  3. Verify the billing start modal does not overflow the viewport"
info "  4. Close the modal (press Escape)"
info "  5. Go to /games, click Launch Game, verify that modal is also within screen bounds"
ask_result "3.1.3" "Modals don't overflow screen at 1920x1080"

# 3.1.4
step "TEST 3.1.4: Touch targets (kiosk button sizes)"
info "In Browser B (Kiosk), open browser DevTools:"
info "  1. Right-click any pod button → Inspect"
info "  2. Check computed height — should be >= 60px for main interactive buttons"
info "  3. Check the PIN numpad buttons (booking wizard) — should be >= 60px"
info "  Alternatively: try tapping buttons with a finger — they should be easy to press"
ask_result "3.1.4" "Touch targets >= 60px height on kiosk"

# 3.1.5
step "TEST 3.1.5: Long text truncation"
info "Look for any text values that might be long in the live UI:"
info "  - Driver names on /drivers (POS) — check 3-column grid"
info "  - Track names on /leaderboards"
info "  - Pod status text on kiosk landing"
info "Verify that long text truncates with '...' and does not break the card layout"
ask_result "3.1.5" "Long text truncates without breaking layout"

# ══════════════════════════════════════════════════════════════════════════════
# SECTION 3.2 — Real-Time Updates
# ══════════════════════════════════════════════════════════════════════════════
header "SECTION 3.2 — Real-Time Updates (5 tests)"

echo -e "${BOLD}
IMPORTANT: For every test in this section, perform the action in ONE browser,
then look at the OTHER browser to verify the update appears within ~3 seconds.
The script will curl the API after your action to confirm server-side state.${RESET}
"

read -rp "Enter the pod number you will use for 3.2 tests (e.g. 1): " TEST_POD_NUM
TEST_POD_NUM="${TEST_POD_NUM:-1}"

# 3.2.1
header "TEST 3.2.1: Start session on POS → Kiosk shows pod occupied"
step "Step 1: In Browser A (POS), go to /billing"
step "Step 2: Click 'Start' on Pod ${TEST_POD_NUM} (it should be idle)"
step "Step 3: Complete the start session modal (select driver + pricing tier, click Start)"
info "         Note: even a Direct/quick start with any driver works for this test"
step "Step 4: Wait 2–3 seconds after starting"
echo ""
fetch_fleet_health
echo ""
step "Step 5: In Browser B (Kiosk), verify that Pod ${TEST_POD_NUM} shows as occupied (red/in-use)"
info "         Look for a red tile or 'In Use' badge on the landing page"
ask_result "3.2.1" "Start session on POS — Kiosk shows pod as occupied immediately"

# 3.2.2
header "TEST 3.2.2: Launch game on POS → Pod kiosk view shows launching state"
step "Step 1: In Browser A (POS), go to /games"
step "Step 2: Click 'Launch Game' on Pod ${TEST_POD_NUM} (which should now be in session)"
step "Step 3: Select a game and click Launch"
step "Step 4: Wait 2–3 seconds"
echo ""
fetch_fleet_health
echo ""
step "Step 5: In Browser B (Kiosk), navigate to /pod/${TEST_POD_NUM}"
info "         Verify the pod view shows a 'launching' or 'setting up' state"
info "         (spinner, progress indicator, or game name with loading state)"
ask_result "3.2.2" "Launch game on POS — pod kiosk view shows launching state"

# 3.2.3
header "TEST 3.2.3: End session on POS → Kiosk returns to idle"
step "Step 1: In Browser A (POS), go to /billing"
step "Step 2: Click 'End' on Pod ${TEST_POD_NUM}'s active session"
step "Step 3: Confirm ending the session in the modal"
step "Step 4: Wait 2–3 seconds"
echo ""
fetch_fleet_health
echo ""
step "Step 5: In Browser B (Kiosk), verify Pod ${TEST_POD_NUM} is back to idle/available (green)"
ask_result "3.2.3" "End session on POS — Kiosk pod returns to idle"

# 3.2.4
header "TEST 3.2.4: Book on Kiosk → POS billing shows new session"
step "Step 1: Snapshot current sessions BEFORE booking:"
fetch_sessions
echo ""
step "Step 2: In Browser B (Kiosk), go to the landing page"
step "Step 3: Click 'Book a Session' (or 'Have a PIN?')"
step "Step 4: Complete the booking wizard:"
info "         - Enter phone → get OTP → enter OTP"
info "         - Select plan, game, and confirm"
info "         OR use 'Have a PIN?' if you have a pre-issued PIN"
step "Step 5: Wait 2–3 seconds after booking completes"
echo ""
info "Sessions AFTER booking:"
fetch_sessions
echo ""
step "Step 6: In Browser A (POS), go to /billing"
info "         Verify Pod ${TEST_POD_NUM} shows the new session created via Kiosk"
ask_result "3.2.4" "Book on Kiosk — POS billing shows new session"

# 3.2.5
header "TEST 3.2.5: Telemetry during game — both POS and Kiosk show live data"
info "This test requires a pod actively running a game with a driver seated and driving."
info "If no active game session is running right now, start one via POS /games first."
echo ""
step "Step 1: Ensure a game is running on Pod ${TEST_POD_NUM} (driver in seat, AC/iRacing active)"
step "Step 2: In Browser A (POS), go to / (Live Overview)"
info "         Verify speed, RPM, brake % are updating live on Pod ${TEST_POD_NUM}"
step "Step 3: In Browser B (Kiosk), go to /pod/${TEST_POD_NUM}"
info "         Verify speed, RPM, lap count are updating live"
step "Step 4: Also check /telemetry on POS — verify speed/RPM bars are moving"
echo ""
fetch_fleet_health
echo ""
ask_result "3.2.5" "Telemetry during game — both POS Live Overview and Kiosk pod view show live data"

# ══════════════════════════════════════════════════════════════════════════════
# SECTION 3.3 — Error Handling
# ══════════════════════════════════════════════════════════════════════════════
header "SECTION 3.3 — Error Handling (5 tests)"

echo -e "${BOLD}
These tests simulate network and error conditions. You can use either:
  Option A — Browser DevTools: F12 → Network tab → Toggle 'Offline' mode
  Option B — Physical disconnect: Disable/enable WiFi or unplug ethernet briefly
${RESET}"

# 3.3.1
step "TEST 3.3.1: Network disconnect → UI shows offline indicator"
info "Steps:"
info "  1. While both browsers are showing live data, simulate a network disconnect:"
info "     - Option A: In Browser A DevTools → Network → Offline"
info "     - Option B: Disable WiFi for 10 seconds"
info "  2. Wait 5–10 seconds"
info "  3. Verify the UI shows an offline or reconnecting indicator:"
info "     - POS: banner/toast like 'Disconnected' or 'Reconnecting...'"
info "     - Kiosk: status indicator changes to red/offline"
ask_result "3.3.1" "Network disconnect — UI shows offline/reconnecting indicator"

# 3.3.2
step "TEST 3.3.2: Network reconnect → data refreshes"
info "Steps (continue from 3.3.1):"
info "  1. Re-enable the network (DevTools → go back online, or re-enable WiFi)"
info "  2. Wait 5–10 seconds"
info "  3. Verify:"
info "     - Status indicators return to green/connected"
info "     - Pod grid, sessions, and telemetry resume updating"
info "     - No stale data remains (pod states are current)"
ask_result "3.3.2" "Network reconnect — data refreshes, status returns to normal"

# 3.3.3
step "TEST 3.3.3: API error on page load → error message with Retry button"
info "Steps:"
info "  1. Go to /drivers in POS"
info "  2. In DevTools → Network, block the URL pattern 'api/v1/drivers'"
info "     (right-click any drivers request → Block request URL)"
info "  3. Refresh the page (F5)"
info "  4. Verify an error message is shown (e.g. 'Failed to load' or 'Error loading drivers')"
info "  5. Verify a Retry button or Reload option is visible"
info "  6. Remove the block, click Retry — verify data loads"
ask_result "3.3.3" "API error on page load — error message with Retry button"

# 3.3.4
step "TEST 3.3.4: Rapid button clicks → no duplicate actions (debounce)"
info "Steps:"
info "  1. Go to /billing in POS"
info "  2. Click 'Start' on an idle pod to open the billing modal"
info "  3. Fill in the required fields"
info "  4. Click 'Start' RAPIDLY multiple times (3–5 quick clicks)"
info "  5. Go to /sessions or /billing — verify only ONE session was created for that pod"
info "     (no duplicate sessions with the same pod/driver/start time)"
ask_result "3.3.4" "Rapid button clicks — no duplicate actions (debounce works)"

# 3.3.5
step "TEST 3.3.5: Pod goes offline mid-game-launch → error with Retry or Clear"
info "This test is hard to simulate without pulling a pod's power. If possible:"
info "  1. Start a game launch on a non-critical pod (e.g. Pod 8 if available)"
info "  2. While the game is launching, disconnect the pod's network (or power it off)"
info "  3. In POS /games, verify an error message appears for that pod"
info "  4. Verify a 'Retry' or 'Clear' option is shown (not just a frozen spinner)"
info "  NOTE: If you cannot safely power-cycle a pod, SKIP this test."
ask_result "3.3.5" "Pod goes offline mid-game-launch — error message with Retry or Clear option"

# ══════════════════════════════════════════════════════════════════════════════
# SECTION 3.4 — Edge Cases
# ══════════════════════════════════════════════════════════════════════════════
header "SECTION 3.4 — Edge Cases (6 tests)"

# 3.4.1
step "TEST 3.4.1: Special characters in search → no crash"
info "Steps:"
info "  1. Go to /drivers in POS"
info "  2. In any search/filter field, type: !@#\$%^&*()"
info "  3. Verify no crash, white screen, or JavaScript error"
info "  4. Verify results either show 'No results' or filter correctly"
info "  5. Also try in /leaderboards track search"
ask_result "3.4.1" "Special characters in search — no crash, results filter correctly"

# 3.4.2
step "TEST 3.4.2: Empty state — no drivers"
info "Steps:"
info "  1. Go to /drivers in POS"
info "  2. Apply a filter or search that returns zero results"
info "     (e.g. search for 'ZZZZZ' or a phone number that doesn't exist)"
info "  3. Verify a 'No drivers found' or similar placeholder is shown"
info "  4. Verify no white screen or JavaScript error"
ask_result "3.4.2" "Empty state (no drivers) — 'No data' placeholder shown"

# 3.4.3
step "TEST 3.4.3: Empty state — no sessions"
info "Steps:"
info "  1. Go to /sessions in POS"
info "  2. Apply a date filter to a day with no sessions (e.g. far in the past)"
info "  3. Verify an appropriate empty state message is shown"
ask_result "3.4.3" "Empty state (no sessions) — 'No data' placeholder shown"

# 3.4.4
step "TEST 3.4.4: Empty state — no bookings"
info "Steps:"
info "  1. Go to /bookings in POS"
info "  2. Filter or search to get zero results"
info "  3. Verify empty state placeholder"
ask_result "3.4.4" "Empty state (no bookings) — 'No data' placeholder shown"

# 3.4.5 — PARTIALLY AUTOMATED: timezone check via curl
step "TEST 3.4.5: Timezone shows IST"
info "Automated check: curling /api/v1/sessions and looking for IST/+05:30 in timestamps..."
echo ""
SESSION_DATA=$(curl -s --max-time 5 "http://${SERVER}:8080/api/v1/sessions" 2>/dev/null || echo "")
if [[ -n "$SESSION_DATA" ]]; then
  if echo "$SESSION_DATA" | python3 -c "
import sys, json
data = json.load(sys.stdin)
timestamps = []
if isinstance(data, list):
    for item in data:
        for field in ['created_at', 'started_at', 'ended_at', 'updated_at']:
            if field in item and item[field]:
                timestamps.append(item[field])
if not timestamps:
    print('No timestamp fields found — check manually')
    sys.exit(0)
ist_ok = sum(1 for t in timestamps if '+05:30' in t or 'IST' in t)
non_ist = [t for t in timestamps if '+05:30' not in t and 'IST' not in t]
print(f'Checked {len(timestamps)} timestamp(s). IST format: {ist_ok}')
if non_ist:
    print(f'Non-IST timestamps found: {non_ist[:3]}')
    sys.exit(2)
else:
    print('All timestamps have IST offset')
" 2>/dev/null; then
    TIMESTAMP_OK=true
  else
    TIMESTAMP_OK=false
  fi
else
  echo "  (no session data — API unreachable or no sessions exist)"
  TIMESTAMP_OK=false
fi

echo ""
info "Manual check: In POS /billing or /history, verify all dates show times in IST (e.g. '14:30 IST' or '+05:30')"
info "              In Kiosk booking confirmation, verify allocated time shows IST"
ask_result "3.4.5" "Timezone shows IST — all times in IST (+05:30)"

# 3.4.6
step "TEST 3.4.6: Page refresh mid-session → session state preserved"
info "Steps:"
info "  1. Start an active session on a pod via POS /billing"
info "  2. Navigate to /billing in POS — note the timer value (e.g. 58:42 remaining)"
info "  3. Press F5 to refresh the page"
info "  4. Verify after reload:"
info "     - The session is still shown as active"
info "     - The timer continues from approximately where it was (not reset to start)"
info "     - Driver name and pod number are still correct"
info "  5. Repeat on Kiosk: go to /pod/N, refresh, verify session data preserved"
ask_result "3.4.6" "Page refresh mid-session — session state preserved"

# ══════════════════════════════════════════════════════════════════════════════
# FINAL SUMMARY
# ══════════════════════════════════════════════════════════════════════════════
TOTAL=$((PASS + FAIL + SKIP))

echo ""
echo -e "${BOLD}╔══════════════════════════════════════════╗${RESET}"
echo -e "${BOLD}║   CROSS-SYNC TEST RESULTS SUMMARY        ║${RESET}"
echo -e "${BOLD}╠══════════════════════════════════════════╣${RESET}"
echo -e "${BOLD}║  Total:  ${TOTAL}/21                             ║${RESET}"
echo -e "${BOLD}║  ${GREEN}Pass:   ${PASS}${RESET}${BOLD}                               ║${RESET}"
if [[ $FAIL -gt 0 ]]; then
echo -e "${BOLD}║  ${RED}Fail:   ${FAIL}${RESET}${BOLD}                               ║${RESET}"
else
echo -e "${BOLD}║  Fail:   0                               ║${RESET}"
fi
echo -e "${BOLD}║  Skip:   ${SKIP}                               ║${RESET}"
echo -e "${BOLD}╚══════════════════════════════════════════╝${RESET}"
echo ""

if [[ $FAIL -eq 0 && $SKIP -eq 0 ]]; then
  echo -e "${GREEN}${BOLD}All cross-cutting tests PASSED.${RESET}"
elif [[ $FAIL -eq 0 ]]; then
  echo -e "${YELLOW}Cross-cutting tests done. Some skipped — review skips before sign-off.${RESET}"
else
  echo -e "${RED}${BOLD}${FAIL} test(s) FAILED. Add entries to test/e2e/TRIAGE.md for each failure.${RESET}"
fi

# Append summary to report
if [[ -f "$REPORT" ]]; then
  {
    echo ""
    echo "### Cross-Sync Summary"
    echo ""
    echo "| Section | Total | Pass | Fail | Skip |"
    echo "|---------|-------|------|------|------|"
    echo "| 3.1 Responsiveness | 5 | | | |"
    echo "| 3.2 Real-Time | 5 | | | |"
    echo "| 3.3 Error Handling | 5 | | | |"
    echo "| 3.4 Edge Cases | 6 | | | |"
    echo "| **Cross-Sync Total** | **21** | **${PASS}** | **${FAIL}** | **${SKIP}** |"
    echo ""
    echo "_Run completed: $(date '+%Y-%m-%d %H:%M IST')_"
  } >> "$REPORT"
fi

echo ""
echo "Results appended to: ${REPORT}"
echo ""
echo "Next step: For any FAILs, open test/e2e/TRIAGE.md and add a row to the"
echo "           'Fixed Failures' or 'Known Issues' table."
echo ""
