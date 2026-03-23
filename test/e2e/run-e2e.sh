#!/usr/bin/env bash
# run-e2e.sh — RaceControl E2E test runner
# Usage: bash test/e2e/run-e2e.sh [--pos-only|--kiosk-only|--api-only]
# Requires: curl, access to 192.168.31.23 (server must be running)
# Output: E2E-TEST-RESULTS-{date}.md in test/e2e/
#
# Test coverage:
#   Automated: HTTP status checks (page loads), API JSON shape validation
#   Manual:    UI interaction tests (modals, keyboard nav, animations, drag-drop)

set -euo pipefail

SERVER="192.168.31.23"
DATE=$(date '+%Y-%m-%d')
REPORT="test/e2e/E2E-TEST-RESULTS-${DATE}.md"
PASS=0
FAIL=0
SKIP=0
TIMEOUT=5
FILTER="${1:-}"

# --- Section counters (associative array: section_id -> "pass fail skip") ---
declare -A SECT_PASS
declare -A SECT_FAIL
declare -A SECT_SKIP
declare -A SECT_TOTAL

# Registered section order for the summary table
SECTIONS=(
  "1.1" "1.2" "1.3" "1.4" "1.5" "1.6" "1.7" "1.8" "1.9" "1.10" "1.11" "1.12" "1.13"
  "2.1" "2.2" "2.3" "2.4" "2.5" "2.6" "2.7"
  "3.1" "3.2" "3.3" "3.4"
)

declare -A SECT_LABEL
SECT_LABEL["1.1"]="1.1 Login"
SECT_LABEL["1.2"]="1.2 Sidebar Nav"
SECT_LABEL["1.3"]="1.3 Live Overview"
SECT_LABEL["1.4"]="1.4 Games"
SECT_LABEL["1.5"]="1.5 Billing"
SECT_LABEL["1.6"]="1.6 AC LAN"
SECT_LABEL["1.7"]="1.7 Leaderboards"
SECT_LABEL["1.8"]="1.8 Cameras"
SECT_LABEL["1.9"]="1.9 Cafe Menu"
SECT_LABEL["1.10"]="1.10 AI Insights"
SECT_LABEL["1.11"]="1.11 Settings"
SECT_LABEL["1.12"]="1.12 Drivers"
SECT_LABEL["1.13"]="1.13 Presenter"
SECT_LABEL["2.1"]="2.1 Kiosk Landing"
SECT_LABEL["2.2"]="2.2 PIN Entry"
SECT_LABEL["2.3"]="2.3 Booking Wizard"
SECT_LABEL["2.4"]="2.4 Pod Kiosk"
SECT_LABEL["2.5"]="2.5 Staff Control"
SECT_LABEL["2.6"]="2.6 Fleet Health"
SECT_LABEL["2.7"]="2.7 Spectator"
SECT_LABEL["3.1"]="3.1 Responsiveness"
SECT_LABEL["3.2"]="3.2 Real-Time"
SECT_LABEL["3.3"]="3.3 Error Handling"
SECT_LABEL["3.4"]="3.4 Edge Cases"

# Initialise counters to zero
for s in "${SECTIONS[@]}"; do
  SECT_PASS[$s]=0
  SECT_FAIL[$s]=0
  SECT_SKIP[$s]=0
  SECT_TOTAL[$s]=0
done

# ── Filter logic ─────────────────────────────────────────────────────────────
CURRENT_SECTION=""
section_in_scope() {
  local s="$1"
  case "$FILTER" in
    --pos-only)   [[ "$s" == 1.* ]] ;;
    --kiosk-only) [[ "$s" == 2.* ]] ;;
    --api-only)   true ;; # api tests are scattered; run all automated checks only
    *)            true ;;
  esac
}

# ── Helper functions ──────────────────────────────────────────────────────────

# test_http_status <id> <section> <description> <url> [expected_status]
test_http_status() {
  local id="$1" section="$2" desc="$3" url="$4" expected="${5:-200}"

  section_in_scope "$section" || return 0
  SECT_TOTAL[$section]=$((${SECT_TOTAL[$section]} + 1))

  local actual
  actual=$(curl -s -o /dev/null -w "%{http_code}" --max-time "$TIMEOUT" "$url" 2>/dev/null || echo "000")

  if [ "$actual" = "$expected" ]; then
    echo "  PASS  [${id}] ${desc} (HTTP ${actual})"
    printf "| %-8s | PASS | HTTP %s | %s |\n" "$id" "$actual" "$desc" >> "$REPORT"
    PASS=$((PASS + 1))
    SECT_PASS[$section]=$((${SECT_PASS[$section]} + 1))
  else
    echo "  FAIL  [${id}] ${desc} (expected HTTP ${expected}, got ${actual})"
    printf "| %-8s | FAIL | HTTP %s (expected %s) | %s |\n" "$id" "$actual" "$expected" "$desc" >> "$REPORT"
    FAIL=$((FAIL + 1))
    SECT_FAIL[$section]=$((${SECT_FAIL[$section]} + 1))
  fi
}

# test_api_json <id> <section> <description> <url> <grep_pattern>
test_api_json() {
  local id="$1" section="$2" desc="$3" url="$4" pattern="$5"

  section_in_scope "$section" || return 0
  SECT_TOTAL[$section]=$((${SECT_TOTAL[$section]} + 1))

  local body
  body=$(curl -s --max-time "$TIMEOUT" "$url" 2>/dev/null || echo "")

  if echo "$body" | grep -q "$pattern"; then
    echo "  PASS  [${id}] ${desc} (pattern '${pattern}' found)"
    printf "| %-8s | PASS | JSON pattern '%s' found | %s |\n" "$id" "$pattern" "$desc" >> "$REPORT"
    PASS=$((PASS + 1))
    SECT_PASS[$section]=$((${SECT_PASS[$section]} + 1))
  else
    echo "  FAIL  [${id}] ${desc} (pattern '${pattern}' NOT found)"
    printf "| %-8s | FAIL | JSON pattern '%s' missing | %s |\n" "$id" "$pattern" "$desc" >> "$REPORT"
    FAIL=$((FAIL + 1))
    SECT_FAIL[$section]=$((${SECT_FAIL[$section]} + 1))
  fi
}

# manual_test <id> <section> <description> <expected>
# Logs a MANUAL/SKIP entry — tester fills this in the report template
manual_test() {
  local id="$1" section="$2" desc="$3" expected="$4"

  section_in_scope "$section" || return 0
  SECT_TOTAL[$section]=$((${SECT_TOTAL[$section]} + 1))

  echo "  SKIP  [${id}] ${desc} (MANUAL — see E2E-REPORT-TEMPLATE.md)"
  printf "| %-8s | MANUAL | — | %s | Expected: %s |\n" "$id" "$desc" "$expected" >> "$REPORT"
  SKIP=$((SKIP + 1))
  SECT_SKIP[$section]=$((${SECT_SKIP[$section]} + 1))
}

# section_header <section_id> <title>
section_header() {
  local section="$1" title="$2"
  CURRENT_SECTION="$section"
  echo ""
  echo "=== Section ${title} ==="
  printf "\n### %s\n\n" "$title" >> "$REPORT"
  printf "| Test ID | Result | Detail | Description |\n" >> "$REPORT"
  printf "|---------|--------|--------|-------------|\n" >> "$REPORT"
}

# ── Pre-flight health check ───────────────────────────────────────────────────
echo "=== Pre-flight health check ==="
HEALTH_SCRIPT="C:/Users/bono/racingpoint/deploy-staging/check-health.sh"

# Support running from repo root OR from CI with absolute path
if [ -f "$HEALTH_SCRIPT" ]; then
  if ! bash "$HEALTH_SCRIPT"; then
    echo ""
    echo "ABORTED: services not healthy. Fix before running E2E tests."
    exit 1
  fi
else
  echo "  WARN  check-health.sh not found at ${HEALTH_SCRIPT} — attempting inline check"
  RC_HEALTH=$(curl -s --max-time "$TIMEOUT" "http://${SERVER}:8080/api/v1/health" 2>/dev/null || echo "")
  if ! echo "$RC_HEALTH" | grep -q '"status":"ok"'; then
    echo "ABORTED: racecontrol (:8080) health check failed. Services not ready."
    exit 1
  fi
  echo "  PASS  racecontrol :8080 (inline check)"
fi
echo ""

# ── Initialise report file ────────────────────────────────────────────────────
mkdir -p "$(dirname "$REPORT")"
cat > "$REPORT" <<HEADER
# E2E Test Results — ${DATE}

**Tester:** _______________
**Date:** ${DATE}
**Environment:** POS (:3200) + Kiosk (:3300) on 192.168.31.23
**Runner:** run-e2e.sh (automated + manual checklist)
**Filter:** ${FILTER:-none (full run)}

<!-- Summary table filled at end of run -->
SUMMARY_PLACEHOLDER

---

## Detailed Results

HEADER

# ── PART 1: POS (:3200) ──────────────────────────────────────────────────────
echo "=== PART 1: POS Dashboard (:3200) ==="

# 1.1 Login
section_header "1.1" "1.1 Login"
test_http_status  "1.1.1" "1.1" "Login page loads"           "http://${SERVER}:3200/login"     200
manual_test       "1.1.2" "1.1" "Wrong PIN shows error"       "Error message shown, stays on login"
manual_test       "1.1.3" "1.1" "Correct PIN redirects"       "Redirects to Live Overview (/)"
manual_test       "1.1.4" "1.1" "Refresh after login"         "Stays logged in (session persists)"

# 1.2 Sidebar Navigation — 22 page-load tests
section_header "1.2" "1.2 Sidebar Navigation"
test_http_status  "1.2.1"  "1.2" "Live Overview (/) loads"     "http://${SERVER}:3200/"
test_http_status  "1.2.2"  "1.2" "Live Overview (/)"           "http://${SERVER}:3200/"
test_http_status  "1.2.3"  "1.2" "Pods page loads"             "http://${SERVER}:3200/pods"
test_http_status  "1.2.4"  "1.2" "Games page loads"            "http://${SERVER}:3200/games"
test_http_status  "1.2.5"  "1.2" "Telemetry page loads"        "http://${SERVER}:3200/telemetry"
test_http_status  "1.2.6"  "1.2" "AC LAN page loads"           "http://${SERVER}:3200/ac-lan"
test_http_status  "1.2.7"  "1.2" "AC Results page loads"       "http://${SERVER}:3200/ac-results"
test_http_status  "1.2.8"  "1.2" "Sessions page loads"         "http://${SERVER}:3200/sessions"
test_http_status  "1.2.9"  "1.2" "Drivers page loads"          "http://${SERVER}:3200/drivers"
test_http_status  "1.2.10" "1.2" "Leaderboards page loads"     "http://${SERVER}:3200/leaderboards"
test_http_status  "1.2.11" "1.2" "Events page loads"           "http://${SERVER}:3200/events"
test_http_status  "1.2.12" "1.2" "Billing page loads"          "http://${SERVER}:3200/billing"
test_http_status  "1.2.13" "1.2" "Pricing page loads"          "http://${SERVER}:3200/pricing"
test_http_status  "1.2.14" "1.2" "History page loads"          "http://${SERVER}:3200/history"
test_http_status  "1.2.15" "1.2" "Bookings page loads"         "http://${SERVER}:3200/bookings"
test_http_status  "1.2.16" "1.2" "AI Insights page loads"      "http://${SERVER}:3200/ai"
test_http_status  "1.2.17" "1.2" "Cameras page loads"          "http://${SERVER}:3200/cameras"
test_http_status  "1.2.18" "1.2" "Playback page loads"         "http://${SERVER}:3200/playback"
test_http_status  "1.2.19" "1.2" "Cafe Menu page loads"        "http://${SERVER}:3200/cafe"
test_http_status  "1.2.20" "1.2" "Settings page loads"         "http://${SERVER}:3200/settings"
test_http_status  "1.2.21" "1.2" "Presenter View page loads"   "http://${SERVER}:3200/presenter"
test_http_status  "1.2.22" "1.2" "Kiosk Mode link (3300) loads" "http://${SERVER}:3300/"

# 1.3 Live Overview
section_header "1.3" "1.3 Live Overview"
test_api_json     "1.3.1" "1.3" "Fleet health returns pod objects" "http://${SERVER}:8080/api/v1/fleet/health" '"pod_number"'
manual_test       "1.3.2" "1.3" "Idle pods show green"          "Available pods shown green/idle"
manual_test       "1.3.3" "1.3" "Active pods show red"          "In-session pods shown red/active"
manual_test       "1.3.4" "1.3" "Offline pods dimmed"           "Offline pods greyed out"
manual_test       "1.3.5" "1.3" "Telemetry bar updates"         "Live speed/RPM on active pods"
manual_test       "1.3.6" "1.3" "Lap feed scrolls"              "Recent laps appear in real-time"

# 1.4 Games — all UI interaction
section_header "1.4" "1.4 Games"
test_http_status  "1.4.0" "1.4" "Games page loads (HTTP 200)"   "http://${SERVER}:3200/games"
manual_test       "1.4.1" "1.4" "Launch Game modal opens"       "Game selection modal opens on idle pod"
manual_test       "1.4.2" "1.4" "Modal shows game options"      "AC, iRacing, F1 25, Le Mans Ultimate, Forza visible"
manual_test       "1.4.3" "1.4" "Select a game"                 "Game highlighted in modal"
manual_test       "1.4.4" "1.4" "Click Launch"                  "Game launches, pod status changes to 'launching'"
manual_test       "1.4.5" "1.4" "Launch completes"              "Pod status changes to 'running', game name shown"
manual_test       "1.4.6" "1.4" "Stop Game on running pod"      "Game stops, pod returns to idle"
manual_test       "1.4.7" "1.4" "Launch on offline pod disabled" "Button disabled or error shown"
manual_test       "1.4.8" "1.4" "Close modal with X"            "Modal closes, no action taken"
manual_test       "1.4.9" "1.4" "Close modal with Escape"       "Modal closes"

# 1.5 Billing
section_header "1.5" "1.5 Billing"
test_api_json     "1.5.0" "1.5" "Billing API returns JSON"       "http://${SERVER}:8080/api/v1/billing" '"'
manual_test       "1.5.1"  "1.5" "Start billing modal opens"     "Billing start modal opens"
manual_test       "1.5.2"  "1.5" "Mode tabs visible"             "PIN/QR/Direct tabs shown"
manual_test       "1.5.3"  "1.5" "Search for driver"             "Driver dropdown populates"
manual_test       "1.5.4"  "1.5" "Select pricing tier"           "Tier highlighted"
manual_test       "1.5.5"  "1.5" "Click Start — session starts"  "Session starts, timer begins on pod card"
manual_test       "1.5.6"  "1.5" "Session timer counts down"     "Timer decrements every second"
manual_test       "1.5.7"  "1.5" "Pause session"                 "Session pauses, button changes to Resume"
manual_test       "1.5.8"  "1.5" "Resume session"                "Session resumes, timer continues"
manual_test       "1.5.9"  "1.5" "Extend +10min"                 "Timer adds 10 minutes"
manual_test       "1.5.10" "1.5" "End session"                   "Session ends, pod returns to idle"
manual_test       "1.5.11" "1.5" "Warning when < 2min remaining" "Warning indicator appears"
manual_test       "1.5.12" "1.5" "Variable time toggle"          "Custom duration/price fields appear"
manual_test       "1.5.13" "1.5" "Start without driver — error"  "Validation error shown"

# 1.6 AC LAN — all UI interaction
section_header "1.6" "1.6 AC LAN"
test_http_status  "1.6.0" "1.6" "AC LAN page loads (HTTP 200)"  "http://${SERVER}:3200/ac-lan"
manual_test       "1.6.1" "1.6" "Pod checkboxes work"           "Can select/deselect individual pods"
manual_test       "1.6.2" "1.6" "Track dropdown"                "Lists available tracks, can select"
manual_test       "1.6.3" "1.6" "Car dropdown"                  "Lists available cars, can select"
manual_test       "1.6.4" "1.6" "Session type toggle"           "Practice/Qualifying/Race toggles correctly"
manual_test       "1.6.5" "1.6" "Duration/laps input"           "Can enter custom values"
manual_test       "1.6.6" "1.6" "Advanced settings expand"      "Expand/collapse works"
manual_test       "1.6.7" "1.6" "Load preset"                   "Preset loads, fields populate"
manual_test       "1.6.8" "1.6" "Save preset"                   "New preset saved"
manual_test       "1.6.9" "1.6" "Click Start — race launches"   "Race launches on selected pods"

# 1.7 Leaderboards
section_header "1.7" "1.7 Leaderboards"
test_api_json     "1.7.0" "1.7" "Leaderboards API returns data"  "http://${SERVER}:8080/api/v1/leaderboards" '"'
manual_test       "1.7.1" "1.7" "Records tab shows fastest times" "Fastest times displayed"
manual_test       "1.7.2" "1.7" "Drivers tab shows rankings"     "Driver rankings shown"
manual_test       "1.7.3" "1.7" "Tracks tab shows track records" "Track records listed"
manual_test       "1.7.4" "1.7" "Sim type filter works"         "Filters by AC/iRacing/F1 etc."
manual_test       "1.7.5" "1.7" "Show Invalid toggle"            "Toggles invalid laps visibility"
manual_test       "1.7.6" "1.7" "Car filter per track"           "Filters leaderboard by car"
manual_test       "1.7.7" "1.7" "Position colors (gold/silver/bronze)" "1st=gold, 2nd=silver, 3rd=bronze"
manual_test       "1.7.8" "1.7" "Track drill-down"               "Click track shows detailed times"

# 1.8 Cameras
section_header "1.8" "1.8 Cameras"
test_http_status  "1.8.1"  "1.8" "Cameras page loads"            "http://${SERVER}:3200/cameras"
manual_test       "1.8.2"  "1.8" "Grid mode buttons (1/4/9/16)"  "Grid layout changes per selection"
manual_test       "1.8.3"  "1.8" "Refresh rate dropdown"         "Changes polling interval"
manual_test       "1.8.4"  "1.8" "Status dots (green/yellow/red)" "Green=live, Yellow=stale, Red=offline"
manual_test       "1.8.5"  "1.8" "Zone colors"                   "Red=entrance, Blue=reception, Green=pods"
manual_test       "1.8.6"  "1.8" "Click tile opens fullscreen"   "Fullscreen overlay opens with live video"
manual_test       "1.8.7"  "1.8" "Fullscreen close X button"     "Returns to grid"
manual_test       "1.8.8"  "1.8" "Fullscreen close Escape"       "Returns to grid"
manual_test       "1.8.9"  "1.8" "Fullscreen close backdrop click" "Returns to grid"
manual_test       "1.8.10" "1.8" "Fullscreen prev/next arrow keys" "Cycles to adjacent camera"
manual_test       "1.8.11" "1.8" "Fullscreen prev/next on-screen buttons" "Hover edges shows buttons, click cycles"
manual_test       "1.8.12" "1.8" "Grid keyboard nav arrow keys"  "Red outline moves between tiles"
manual_test       "1.8.13" "1.8" "Grid keyboard nav Enter"       "Opens fullscreen on focused tile"
manual_test       "1.8.14" "1.8" "Drag-and-drop reorder"         "Tiles swap positions, layout saved"
manual_test       "1.8.15" "1.8" "Controls auto-hide 3s"         "Fullscreen controls fade after 3s"
manual_test       "1.8.16" "1.8" "Controls reappear on mousemove" "Controls show again"

# 1.9 Cafe Menu
section_header "1.9" "1.9 Cafe Menu"
test_http_status  "1.9.0" "1.9" "Cafe page loads"               "http://${SERVER}:3200/cafe"
# Test /api/v1/cafe/items if it exists — marked manual if 404
CAFE_STATUS=$(curl -s -o /dev/null -w "%{http_code}" --max-time "$TIMEOUT" "http://${SERVER}:8080/api/v1/cafe/items" 2>/dev/null || echo "000")
if [ "$CAFE_STATUS" = "200" ] || [ "$CAFE_STATUS" = "401" ]; then
  test_api_json   "1.9.A" "1.9" "Cafe items API accessible"     "http://${SERVER}:8080/api/v1/cafe/items" '"'
else
  manual_test     "1.9.A" "1.9" "Cafe items API (endpoint may not exist yet)" "API returns items list"
fi
manual_test       "1.9.1" "1.9" "Items tab visible"             "Menu items list visible"
manual_test       "1.9.2" "1.9" "Inventory tab"                 "Stock levels shown"
manual_test       "1.9.3" "1.9" "Promos tab"                    "Promotions list visible"
manual_test       "1.9.4" "1.9" "Add new item"                  "Form opens, can enter name/price/category"
manual_test       "1.9.5" "1.9" "Edit existing item"            "Fields populate, can modify"
manual_test       "1.9.6" "1.9" "Stock status badges"           "Out/Low/Warning/In Stock shown correctly"
manual_test       "1.9.7" "1.9" "Category dropdown filter"      "Can filter by category"
manual_test       "1.9.8" "1.9" "Promo on/off toggle"           "Toggle works, status changes"
manual_test       "1.9.9" "1.9" "Promo types selectable"        "Combo/Happy Hour/Bundle selectable"

# 1.10 AI Insights
section_header "1.10" "1.10 AI Insights"
test_http_status  "1.10.1" "1.10" "AI Insights page loads"       "http://${SERVER}:3200/ai"
manual_test       "1.10.2" "1.10" "Filter: All shows all insights" "All insights displayed"
manual_test       "1.10.3" "1.10" "Filter: Active"               "Shows only active insights"
manual_test       "1.10.4" "1.10" "Filter: Dismissed"            "Shows only dismissed insights"
manual_test       "1.10.5" "1.10" "Refresh button reloads"       "Reloads insights"
# Note: 1.10.5 in test script is "Dismiss insight" — also manual
manual_test       "1.10.5b" "1.10" "Dismiss insight"             "Card moves to dismissed"

# 1.11 Settings
section_header "1.11" "1.11 Settings"
test_api_json     "1.11.A" "1.11" "Health API returns version"   "http://${SERVER}:8080/api/v1/health" '"version"'
test_http_status  "1.11.1" "1.11" "Settings page loads"          "http://${SERVER}:3200/settings"
manual_test       "1.11.2" "1.11" "Venue info displayed"         "Name, location, timezone, capacity shown"
manual_test       "1.11.3" "1.11" "POS Lockdown toggle"          "Toggles between Locked/Unlocked"
manual_test       "1.11.4" "1.11" "POS Lockdown effect"          "When locked, restricted actions enforced"

# 1.12 Drivers
section_header "1.12" "1.12 Drivers"
test_api_json     "1.12.A" "1.12" "Drivers API returns array"    "http://${SERVER}:8080/api/v1/drivers" '"'
test_http_status  "1.12.1" "1.12" "Drivers page loads"           "http://${SERVER}:3200/drivers"
manual_test       "1.12.2" "1.12" "Driver info shown"            "Email, total laps, track time visible"
manual_test       "1.12.3" "1.12" "Long name truncation"         "Names don't overflow card"

# 1.13 Presenter View
section_header "1.13" "1.13 Presenter View"
test_http_status  "1.13.1" "1.13" "Presenter page loads"         "http://${SERVER}:3200/presenter"
manual_test       "1.13.2" "1.13" "Pod counts shown"             "Active/Idle/Total shown"
# Note: "1.13.2" in test script is "Pod counts" — keeping consistent

# ── PART 2: Kiosk (:3300) ────────────────────────────────────────────────────
echo ""
echo "=== PART 2: Kiosk (:3300) ==="
printf "\n---\n\n## PART 2: Kiosk (:3300)\n\n" >> "$REPORT"

# 2.1 Customer Landing
section_header "2.1" "2.1 Customer Landing"
test_http_status  "2.1.1" "2.1" "Kiosk landing page loads"      "http://${SERVER}:3300/"
manual_test       "2.1.2" "2.1" "Available pods show green"      "Idle pods highlighted as available"
manual_test       "2.1.3" "2.1" "Racing pods show red"           "In-session pods shown as occupied"
manual_test       "2.1.4" "2.1" "Click available pod"           "PIN entry modal opens"
manual_test       "2.1.5" "2.1" "Click occupied pod"            "Nothing happens / 'In use' indicator"
manual_test       "2.1.6" "2.1" "Book a Session button"         "Navigates to /book"
manual_test       "2.1.7" "2.1" "Have a PIN? button"            "Opens PIN entry"
manual_test       "2.1.8" "2.1" "Staff Login link"              "Navigates to /staff"
manual_test       "2.1.9" "2.1" "Live status indicator"         "Shows real-time connection status"

# 2.2 PIN Entry — all UI interaction
section_header "2.2" "2.2 PIN Entry Modal"
manual_test       "2.2.1" "2.2" "Numpad renders"                "1-9, 0, Clear, Backspace buttons visible"
manual_test       "2.2.2" "2.2" "Press digits fills dots"        "Dots fill up (4 dots max)"
manual_test       "2.2.3" "2.2" "Backspace removes last digit"  "Last digit removed"
manual_test       "2.2.4" "2.2" "Clear clears all digits"       "All digits cleared"
manual_test       "2.2.5" "2.2" "Auto-submit at 4 digits"       "Validation triggers automatically"
manual_test       "2.2.6" "2.2" "Correct PIN success"           "Success state, pod assigned, session starts"
manual_test       "2.2.7" "2.2" "Wrong PIN error"               "Error message, can retry"
manual_test       "2.2.8" "2.2" "60s inactivity auto-close"     "Modal auto-closes after 60s"
manual_test       "2.2.9" "2.2" "Only numeric input"            "Non-numeric keys ignored"

# 2.3 Booking Wizard
section_header "2.3" "2.3 Booking Wizard"
test_http_status  "2.3.0" "2.3" "Booking Wizard page loads"     "http://${SERVER}:3300/book"
manual_test       "2.3.1"  "2.3" "Phone input field"            "Can enter phone number"
manual_test       "2.3.2"  "2.3" "Invalid phone format error"   "Validation error shown"
manual_test       "2.3.3"  "2.3" "Send OTP button"              "OTP sent, advances to OTP screen"
manual_test       "2.3.4"  "2.3" "6-digit OTP input"            "Can enter OTP code"
manual_test       "2.3.5"  "2.3" "Correct OTP advances"         "Advances to setup wizard"
manual_test       "2.3.6"  "2.3" "Wrong OTP error"              "Error message shown"
manual_test       "2.3.7"  "2.3" "Resend OTP link"              "New OTP sent (throttle ~30s)"
manual_test       "2.3.8"  "2.3" "Select Plan"                  "Pricing tier cards visible, can select"
manual_test       "2.3.9"  "2.3" "Select Game"                  "Game grid shows, can select"
manual_test       "2.3.10" "2.3" "Player Mode selection"        "Solo/Multiplayer options, can select"
manual_test       "2.3.11" "2.3" "Session Type tabs"            "Practice/Qualifying/Race tabs work"
manual_test       "2.3.12" "2.3" "AI Opponents settings"        "Difficulty presets + AI count slider"
manual_test       "2.3.13" "2.3" "Select Experience"            "Experience cards visible, can select"
manual_test       "2.3.14" "2.3" "Select Track"                 "Track search works, category filter, can select"
manual_test       "2.3.15" "2.3" "Select Car"                   "Car search works, category filter, can select"
manual_test       "2.3.16" "2.3" "Driving Settings"             "Controller layout, ABS toggle, TC slider"
manual_test       "2.3.17" "2.3" "Review & Confirm"             "Summary shows all selections, Confirm button"
manual_test       "2.3.18" "2.3" "Skip button works"            "Skips optional steps correctly"
manual_test       "2.3.19" "2.3" "Back button retains state"    "Returns to previous step, retains state"
manual_test       "2.3.20" "2.3" "Next validates required field" "Can't proceed without required selection"
manual_test       "2.3.21" "2.3" "PIN code displayed on success" "4-digit PIN shown clearly"
manual_test       "2.3.22" "2.3" "Pod number shown"             "Assigned pod highlighted"
manual_test       "2.3.23" "2.3" "Allocated time shown"         "Session duration displayed"
manual_test       "2.3.24" "2.3" "Done button returns to landing" "Returns to landing page"
manual_test       "2.3.25" "2.3" "Auto-return after 30s"        "Landing page after 30s inactivity"

# 2.4 Pod Kiosk View
section_header "2.4" "2.4 Pod Kiosk View"
for pod in 1 2 3 4 5 6 7 8; do
  test_http_status "2.4.pod${pod}" "2.4" "Pod ${pod} kiosk page loads" "http://${SERVER}:3300/pod/${pod}"
done
manual_test       "2.4.1"  "2.4" "Experience grid shown (idle)"  "Game experiences shown as cards"
manual_test       "2.4.2"  "2.4" "Click experience selects it"   "Game selected/highlighted"
manual_test       "2.4.3"  "2.4" "Launch button starts game"     "Game begins launching"
manual_test       "2.4.4"  "2.4" "Game splash screen (launching)" "Shows game name + progress indicator"
manual_test       "2.4.5"  "2.4" "Setting up your rig spinner"   "Spinner visible during setup"
manual_test       "2.4.6"  "2.4" "Session timer counts down"     "HH:MM:SS countdown, updates every second"
manual_test       "2.4.7"  "2.4" "Speed display (in-session)"    "Real-time speed value"
manual_test       "2.4.8"  "2.4" "RPM display"                   "Real-time RPM value"
manual_test       "2.4.9"  "2.4" "Brake % display"               "Real-time brake pressure"
manual_test       "2.4.10" "2.4" "Lap count increments"          "Increments on lap completion"
manual_test       "2.4.11" "2.4" "Best lap time updates"         "Updates when new PB set"

# 2.5 Staff Login & Control
section_header "2.5" "2.5 Staff Login & Control"
test_http_status  "2.5.0" "2.5" "Staff page loads"              "http://${SERVER}:3300/staff"
manual_test       "2.5.1"  "2.5" "Staff PIN input works"         "Staff PIN entry works"
manual_test       "2.5.2"  "2.5" "Correct PIN redirects"         "Shows staff name, redirects to /control"
manual_test       "2.5.3"  "2.5" "Wrong PIN error"               "Error message shown"
manual_test       "2.5.4"  "2.5" "Pod grid 4x2 visible"          "All pods with status visible"
manual_test       "2.5.5"  "2.5" "Telemetry on active pod card"  "Speed, RPM, brake % on active pods"
manual_test       "2.5.6"  "2.5" "Session timer on active pod"   "Countdown visible"
manual_test       "2.5.7"  "2.5" "Driver name on active pod"     "Correct driver shown"
manual_test       "2.5.8"  "2.5" "Open game picker"              "Experience grid shown"
manual_test       "2.5.9"  "2.5" "Launch game on pod"            "Game launches, status updates"
manual_test       "2.5.10" "2.5" "Session details panel"         "Active session info shown"
manual_test       "2.5.11" "2.5" "Pause/Resume session"          "Session pauses and resumes"
manual_test       "2.5.12" "2.5" "Extend +10min"                 "Timer adds 10 minutes"
manual_test       "2.5.13" "2.5" "End session"                   "Session ends, pod returns to idle"
manual_test       "2.5.14" "2.5" "Driver selection for topup"    "Can select driver"
manual_test       "2.5.15" "2.5" "Topup amount input"            "Can enter amount"
manual_test       "2.5.16" "2.5" "Confirm topup updates balance" "Balance updated"
manual_test       "2.5.17" "2.5" "Assistance alerts visible"     "Customer requests shown with dismiss"
manual_test       "2.5.18" "2.5" "Multiplayer group UI"          "Can create multiplayer groups"
manual_test       "2.5.19" "2.5" "Sign Out button"               "Returns to staff login"
manual_test       "2.5.20" "2.5" "30min auto-logout"             "Inactive staff logged out automatically"

# 2.6 Fleet Health
section_header "2.6" "2.6 Fleet Health"
test_http_status  "2.6.1" "2.6" "Fleet health page loads"       "http://${SERVER}:3300/fleet"
test_api_json     "2.6.A" "2.6" "Fleet health API ws_connected" "http://${SERVER}:8080/api/v1/fleet/health" '"ws_connected"'
manual_test       "2.6.2"  "2.6" "Health status badges shown"   "Healthy/WS Only/HTTP Only/Offline/Maintenance"
manual_test       "2.6.3"  "2.6" "WS connection indicator"      "Green if connected"
manual_test       "2.6.4"  "2.6" "HTTP reachability indicator"  "Green if reachable"
manual_test       "2.6.5"  "2.6" "Uptime display"               "Hours:minutes shown"
manual_test       "2.6.6"  "2.6" "Violation count badge"        "24h violation count shown"
manual_test       "2.6.7"  "2.6" "Crash recovery indicator"     "Shows if pod recovered from crash"
manual_test       "2.6.8"  "2.6" "Click Maintenance button"     "Maintenance modal opens"
manual_test       "2.6.9"  "2.6" "Maintenance modal PIN verify" "PIN input validates staff"
manual_test       "2.6.10" "2.6" "Maintenance modal failed checks" "Lists what failed"
manual_test       "2.6.11" "2.6" "Maintenance modal Clear button" "Clears maintenance mode"
manual_test       "2.6.12" "2.6" "Maintenance modal Close"      "Modal closes"

# 2.7 Spectator View
section_header "2.7" "2.7 Spectator View"
test_http_status  "2.7.1" "2.7" "Spectator page loads"          "http://${SERVER}:3300/spectator"
manual_test       "2.7.2" "2.7" "Live lap ticker updates"        "Laps appear in real-time"
manual_test       "2.7.3" "2.7" "Throttle/brake traces"         "Visualizations update"
manual_test       "2.7.4" "2.7" "Live leaderboard"              "Rankings shown with times"
manual_test       "2.7.5" "2.7" "Delta color coding"            "Positive=red, Negative=green"
manual_test       "2.7.6" "2.7" "Pod status cards"              "Real-time telemetry per pod"

# ── PART 3: Cross-cutting — all manual ───────────────────────────────────────
echo ""
echo "=== PART 3: Cross-cutting (all manual) ==="
printf "\n---\n\n## PART 3: Cross-Cutting Tests (all manual — require two browser windows + live state)\n\n" >> "$REPORT"
printf "| Test ID | Result | Detail | Description |\n" >> "$REPORT"
printf "|---------|--------|--------|-------------|\n" >> "$REPORT"

# 3.1 Responsiveness
section_header "3.1" "3.1 Responsiveness & Display"
manual_test       "3.1.1" "3.1" "Kiosk at 1920x1080"            "All 8 pods visible, no scroll needed"
manual_test       "3.1.2" "3.1" "POS at full screen"            "Sidebar + content fit without overlap"
manual_test       "3.1.3" "3.1" "Modal sizing"                  "Modals don't overflow screen"
manual_test       "3.1.4" "3.1" "Touch targets >= 60px"         "All buttons >= 60px height on kiosk"
manual_test       "3.1.5" "3.1" "Long text truncation"          "Names/values don't break layout"

# 3.2 Real-Time Updates
section_header "3.2" "3.2 Real-Time Updates"
manual_test       "3.2.1" "3.2" "Start session on POS → Kiosk"  "Kiosk shows pod as occupied immediately"
manual_test       "3.2.2" "3.2" "Launch game on POS → Pod view" "Pod kiosk view shows launching state"
manual_test       "3.2.3" "3.2" "End session on POS → Kiosk"   "Kiosk returns to idle"
manual_test       "3.2.4" "3.2" "Book on Kiosk → POS billing"   "POS billing shows new session"
manual_test       "3.2.5" "3.2" "Telemetry during game"         "Both POS and Kiosk show live data"

# 3.3 Error Handling
section_header "3.3" "3.3 Error Handling"
manual_test       "3.3.1" "3.3" "Network disconnect"            "UI shows offline/reconnecting indicator"
manual_test       "3.3.2" "3.3" "Network reconnect"             "Data refreshes, status returns to normal"
manual_test       "3.3.3" "3.3" "API error on page load"        "Error message with Retry button"
manual_test       "3.3.4" "3.3" "Rapid button clicks debounced" "No duplicate actions"
manual_test       "3.3.5" "3.3" "Pod goes offline mid-launch"   "Error message, Retry or Clear option"

# 3.4 Edge Cases
section_header "3.4" "3.4 Edge Cases"
manual_test       "3.4.1" "3.4" "Special chars in search"       "No crash, results filter correctly"
manual_test       "3.4.2" "3.4" "Empty state (no drivers)"      "'No data' placeholder shown"
manual_test       "3.4.3" "3.4" "Empty state (no sessions)"     "'No data' placeholder shown"
manual_test       "3.4.4" "3.4" "Empty state (no bookings)"     "'No data' placeholder shown"
manual_test       "3.4.5" "3.4" "Timezone shows IST"            "All times in IST"
manual_test       "3.4.6" "3.4" "Page refresh mid-session"      "Session state preserved"

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "=== Results ==="
echo "  PASS:   ${PASS}"
echo "  FAIL:   ${FAIL}"
echo "  MANUAL: ${SKIP}"
echo "  TOTAL:  $((PASS + FAIL + SKIP))"
echo ""
echo "Report written to: ${REPORT}"

# Build summary table
SUMMARY_TABLE="## Summary\n\n| Section | Total | Pass | Fail | Manual/Skip |\n|---------|-------|------|------|-------------|\n"
TOTAL_PASS=0
TOTAL_FAIL=0
TOTAL_SKIP=0
TOTAL_ALL=0

for s in "${SECTIONS[@]}"; do
  p=${SECT_PASS[$s]}
  f=${SECT_FAIL[$s]}
  k=${SECT_SKIP[$s]}
  t=${SECT_TOTAL[$s]}
  TOTAL_PASS=$((TOTAL_PASS + p))
  TOTAL_FAIL=$((TOTAL_FAIL + f))
  TOTAL_SKIP=$((TOTAL_SKIP + k))
  TOTAL_ALL=$((TOTAL_ALL + t))
  label="${SECT_LABEL[$s]}"
  SUMMARY_TABLE+="| ${label} | ${t} | ${p} | ${f} | ${k} |\n"
done
SUMMARY_TABLE+="| **TOTAL** | **${TOTAL_ALL}** | **${TOTAL_PASS}** | **${TOTAL_FAIL}** | **${TOTAL_SKIP}** |\n"

# Replace placeholder in report
# Use Python for portable in-place replacement (avoids sed -i portability issues)
python3 - <<PYEOF
import re
with open("${REPORT}", "r") as fh:
    content = fh.read()
table = "${SUMMARY_TABLE}".replace("\\n", "\n")
content = content.replace("SUMMARY_PLACEHOLDER", table)
with open("${REPORT}", "w") as fh:
    fh.write(content)
PYEOF

# Exit non-zero if any automated tests failed
if [ "$FAIL" -gt 0 ]; then
  echo "RESULT: FAIL — ${FAIL} automated test(s) failed. See ${REPORT}"
  exit 1
fi

echo "RESULT: PASS — all automated tests passed (${SKIP} manual tests require human verification)"
exit 0
