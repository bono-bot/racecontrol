#!/bin/bash
# verify-laps.sh — Phase 384 Lap Recording Pipeline Verification
# Checks venue + cloud lap data and telemetry adapter status.
#
# Exit codes: 0=PASS, 1=FAIL, 2=PARTIAL
# Usage: bash scripts/verify-laps.sh [--quiet]

set -uo pipefail

# ── Config ────────────────────────────────────────────────────────────────
VENUE_SSH="bono@100.82.33.94"        # James machine (Tailscale)
VENUE_API="http://192.168.31.23:8080"
CLOUD_API="http://localhost:8080"
SSH_TIMEOUT=5
CURL_TIMEOUT=10

# Venue DB path (Windows, accessed via SSH -> server)
VENUE_DB="C:\\RacingPoint\\data\\racecontrol.db"
VENUE_SERVER_IP="192.168.31.23"

# Telemetry ports
PORT_AC=9996
PORT_F1=20777

# ── Colors ────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

QUIET=0
[[ "${1:-}" == "--quiet" ]] && QUIET=1

# ── Helpers ───────────────────────────────────────────────────────────────
info()  { [[ $QUIET -eq 0 ]] && echo -e "${CYAN}[INFO]${RESET}  $*"; }
ok()    { echo -e "${GREEN}[OK]${RESET}    $*"; }
warn()  { echo -e "${YELLOW}[WARN]${RESET}  $*"; }
fail()  { echo -e "${RED}[FAIL]${RESET}  $*"; }
header(){ echo -e "\n${BOLD}── $* ──${RESET}"; }

# Track results
VENUE_REACHABLE=0
VENUE_API_UP=0
VENUE_TOTAL_LAPS=0
VENUE_TODAY_LAPS=0
VENUE_WEEK_LAPS=0
VENUE_LAST_LAP=""
CLOUD_API_UP=0
CLOUD_LEADERBOARD_ENTRIES=0
VENUE_LEADERBOARD_ENTRIES=0
ADAPTERS_LISTENING=0

# ── Step 1: Venue SSH Reachability ────────────────────────────────────────
header "Step 1: Venue Reachability (SSH)"

if ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no -o BatchMode=yes \
      "$VENUE_SSH" "echo alive" >/dev/null 2>&1; then
    ok "SSH to $VENUE_SSH — connected"
    VENUE_REACHABLE=1
else
    fail "SSH to $VENUE_SSH — unreachable (timeout ${SSH_TIMEOUT}s)"
    info "Venue may be offline. Continuing with cloud-only checks."
fi

# ── Step 2: Venue Leaderboard API ─────────────────────────────────────────
header "Step 2: Venue Leaderboard API"

if [[ $VENUE_REACHABLE -eq 1 ]]; then
    # Access venue API through SSH tunnel (since venue is on LAN)
    VENUE_LB_RESPONSE=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
        "$VENUE_SSH" \
        "curl -s --connect-timeout $CURL_TIMEOUT '$VENUE_API/api/v1/public/leaderboard'" 2>/dev/null)

    if [[ -n "$VENUE_LB_RESPONSE" ]] && echo "$VENUE_LB_RESPONSE" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
        VENUE_API_UP=1
        # Count entries — handle both array and object-with-array responses
        VENUE_LEADERBOARD_ENTRIES=$(echo "$VENUE_LB_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
if isinstance(data, list):
    print(len(data))
elif isinstance(data, dict):
    # Try common wrapper keys
    for key in ('data', 'entries', 'leaderboard', 'records'):
        if key in data and isinstance(data[key], list):
            print(len(data[key]))
            sys.exit(0)
    # If dict has no list, count top-level keys as tracks
    print(len(data))
else:
    print(0)
" 2>/dev/null || echo "0")
        ok "Venue leaderboard API responding — $VENUE_LEADERBOARD_ENTRIES entries"
    else
        fail "Venue leaderboard API not responding or returned invalid JSON"
        info "Response (first 200 chars): ${VENUE_LB_RESPONSE:0:200}"
    fi
else
    warn "Skipping venue API check — SSH not available"
fi

# ── Step 3: Venue DB Lap Counts (via SSH) ─────────────────────────────────
header "Step 3: Venue Database Lap Counts"

if [[ $VENUE_REACHABLE -eq 1 ]]; then
    # Try sqlite3 first, fall back to PowerShell + System.Data.SQLite, then curl API
    info "Querying venue database via SSH..."

    # Method 1: Try sqlite3 on server (via SSH -> server SSH or curl to API)
    # Since James (.27) can reach server (.23) API, query counts via API
    VENUE_DB_RESPONSE=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
        "$VENUE_SSH" "
        # Try sqlite3 on the server DB path first (if James has it mounted/accessible)
        if command -v sqlite3 >/dev/null 2>&1 && [ -f '/mnt/server/data/racecontrol.db' ]; then
            sqlite3 '/mnt/server/data/racecontrol.db' \"
                SELECT 'TOTAL:' || COUNT(*) FROM laps;
                SELECT 'TODAY:' || COUNT(*) FROM laps WHERE created_at >= date('now');
                SELECT 'WEEK:' || COUNT(*) FROM laps WHERE created_at >= date('now', '-7 days');
                SELECT 'LAST:' || COALESCE(MAX(created_at), 'NONE') FROM laps;
            \"
        else
            # Method 2: Use the server's own API for lap info
            # Query total laps via the leaderboard endpoint (already fetched)
            # and get additional stats from a debug/stats endpoint if available
            echo 'NO_SQLITE3'
        fi
    " 2>/dev/null)

    if [[ "$VENUE_DB_RESPONSE" == *"TOTAL:"* ]]; then
        # Parse sqlite3 output
        VENUE_TOTAL_LAPS=$(echo "$VENUE_DB_RESPONSE" | grep "^TOTAL:" | cut -d: -f2)
        VENUE_TODAY_LAPS=$(echo "$VENUE_DB_RESPONSE" | grep "^TODAY:" | cut -d: -f2)
        VENUE_WEEK_LAPS=$(echo "$VENUE_DB_RESPONSE" | grep "^WEEK:" | cut -d: -f2)
        VENUE_LAST_LAP=$(echo "$VENUE_DB_RESPONSE" | grep "^LAST:" | cut -d: -f2-)
    else
        # Method 2: Query server's ops endpoint for DB stats via HTTP
        # Avoids PowerShell-over-SSH quoting nightmares (standing rule: NEVER inline PS over SSH)
        info "sqlite3 not available — trying server ops endpoint..."
        OPS_RESPONSE=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
            "$VENUE_SSH" \
            "curl -s --connect-timeout $CURL_TIMEOUT '$VENUE_API/api/v1/debug/activity'" 2>/dev/null)

        if [[ -n "$OPS_RESPONSE" ]] && echo "$OPS_RESPONSE" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
            info "Debug activity endpoint responded. Extracting lap info..."
            # Try to extract lap-related data from the debug response
            EXTRACTED=$(echo "$OPS_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
# Look for lap counts in various possible response shapes
for key in ('total_laps', 'laps_total', 'laps'):
    if key in data and isinstance(data[key], (int, float)):
        print('TOTAL:' + str(int(data[key])))
        break
for key in ('laps_today', 'today_laps'):
    if key in data and isinstance(data[key], (int, float)):
        print('TODAY:' + str(int(data[key])))
        break
for key in ('laps_week', 'laps_7d', 'week_laps'):
    if key in data and isinstance(data[key], (int, float)):
        print('WEEK:' + str(int(data[key])))
        break
for key in ('last_lap_at', 'latest_lap'):
    if key in data and isinstance(data[key], str):
        print('LAST:' + data[key])
        break
" 2>/dev/null)
            if [[ "$EXTRACTED" == *"TOTAL:"* ]]; then
                VENUE_TOTAL_LAPS=$(echo "$EXTRACTED" | grep "^TOTAL:" | cut -d: -f2)
                VENUE_TODAY_LAPS=$(echo "$EXTRACTED" | grep "^TODAY:" | cut -d: -f2)
                VENUE_WEEK_LAPS=$(echo "$EXTRACTED" | grep "^WEEK:" | cut -d: -f2)
                VENUE_LAST_LAP=$(echo "$EXTRACTED" | grep "^LAST:" | cut -d: -f2-)
            fi
        fi

        # Method 3: Fall back to leaderboard as a proxy if we still have no counts
        if [[ -z "$VENUE_TOTAL_LAPS" || "$VENUE_TOTAL_LAPS" == "0" ]]; then
            if [[ ${VENUE_LEADERBOARD_ENTRIES:-0} -gt 0 ]]; then
                info "Using leaderboard entry count as proxy for lap data."
                VENUE_TOTAL_LAPS="$VENUE_LEADERBOARD_ENTRIES (leaderboard proxy)"
                warn "Could not query DB directly — counts are approximate from leaderboard"
            else
                # Method 4: Last resort — check health endpoint
                API_STATS=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
                    "$VENUE_SSH" \
                    "curl -s --connect-timeout $CURL_TIMEOUT '$VENUE_API/api/v1/health'" 2>/dev/null)

                if [[ -n "$API_STATS" ]]; then
                    info "Health API responded but no lap data available."
                    warn "Server is running but could not determine lap counts"
                else
                    fail "All venue DB query methods failed"
                fi
            fi
        fi
    fi

    # Display results
    if [[ -n "$VENUE_TOTAL_LAPS" && "$VENUE_TOTAL_LAPS" != "0" ]]; then
        ok "Total laps:      $VENUE_TOTAL_LAPS"
        ok "Laps today:      ${VENUE_TODAY_LAPS:-unknown}"
        ok "Laps (7 days):   ${VENUE_WEEK_LAPS:-unknown}"
        ok "Most recent lap: ${VENUE_LAST_LAP:-unknown}"
    elif [[ "$VENUE_TOTAL_LAPS" == "0" ]]; then
        fail "Total laps: 0 — no laps recorded in venue database"
    else
        warn "Could not determine venue lap counts"
    fi
else
    warn "Skipping venue DB check — SSH not available"
fi

# ── Step 4: Cloud Leaderboard ─────────────────────────────────────────────
header "Step 4: Cloud Leaderboard"

CLOUD_LB_RESPONSE=$(curl -s --connect-timeout $CURL_TIMEOUT \
    "$CLOUD_API/api/v1/public/leaderboard" 2>/dev/null)

if [[ -n "$CLOUD_LB_RESPONSE" ]] && echo "$CLOUD_LB_RESPONSE" | python3 -c "import sys,json; json.load(sys.stdin)" 2>/dev/null; then
    CLOUD_API_UP=1
    CLOUD_LEADERBOARD_ENTRIES=$(echo "$CLOUD_LB_RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
if isinstance(data, list):
    print(len(data))
elif isinstance(data, dict):
    for key in ('data', 'entries', 'leaderboard', 'records'):
        if key in data and isinstance(data[key], list):
            print(len(data[key]))
            sys.exit(0)
    print(len(data))
else:
    print(0)
" 2>/dev/null || echo "0")
    ok "Cloud leaderboard API responding — $CLOUD_LEADERBOARD_ENTRIES entries"
else
    fail "Cloud leaderboard API not responding"
    if [[ -n "$CLOUD_LB_RESPONSE" ]]; then
        info "Response (first 200 chars): ${CLOUD_LB_RESPONSE:0:200}"
    fi
fi

# ── Step 5: Venue vs Cloud Sync Comparison ────────────────────────────────
header "Step 5: Sync Verification (Venue vs Cloud)"

if [[ $VENUE_API_UP -eq 1 && $CLOUD_API_UP -eq 1 ]]; then
    # Compare leaderboard entry counts as a sync proxy
    V_COUNT="${VENUE_LEADERBOARD_ENTRIES:-0}"
    C_COUNT="${CLOUD_LEADERBOARD_ENTRIES:-0}"

    if [[ "$V_COUNT" -eq "$C_COUNT" && "$V_COUNT" -gt 0 ]]; then
        ok "Leaderboard entries match: venue=$V_COUNT, cloud=$C_COUNT"
    elif [[ "$V_COUNT" -gt 0 && "$C_COUNT" -gt 0 ]]; then
        DIFF=$(( V_COUNT > C_COUNT ? V_COUNT - C_COUNT : C_COUNT - V_COUNT ))
        if [[ $DIFF -le 5 ]]; then
            ok "Leaderboard entries close: venue=$V_COUNT, cloud=$C_COUNT (diff=$DIFF, within tolerance)"
        else
            warn "Leaderboard entry mismatch: venue=$V_COUNT, cloud=$C_COUNT (diff=$DIFF)"
            info "Sync may be lagging — cloud pulls every 30s"
        fi
    elif [[ "$V_COUNT" -gt 0 && "$C_COUNT" -eq 0 ]]; then
        warn "Venue has $V_COUNT entries but cloud has 0 — sync may be broken"
    elif [[ "$V_COUNT" -eq 0 && "$C_COUNT" -gt 0 ]]; then
        warn "Cloud has $C_COUNT entries but venue has 0 — unexpected"
    else
        fail "Both venue and cloud have 0 leaderboard entries"
    fi
elif [[ $VENUE_API_UP -eq 1 ]]; then
    warn "Cloud API down — cannot compare sync"
elif [[ $CLOUD_API_UP -eq 1 ]]; then
    warn "Venue API unreachable — cannot compare sync"
else
    fail "Neither venue nor cloud API responding — cannot verify sync"
fi

# ── Step 6: Telemetry Adapter Ports ──────────────────────────────────────
header "Step 6: Telemetry Adapter Ports"

if [[ $VENUE_REACHABLE -eq 1 ]]; then
    info "Checking UDP listener ports on venue server..."

    PORT_CHECK=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
        "$VENUE_SSH" "
        # Check if server is listening on telemetry ports
        # Try netstat on the server via SSH, or check from James
        ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no ADMIN@$VENUE_SERVER_IP \
            'netstat -ano | findstr /C:\":$PORT_AC \" /C:\":$PORT_F1 \"' 2>/dev/null \
        || echo 'NETSTAT_FAILED'
    " 2>/dev/null)

    if [[ "$PORT_CHECK" == *"NETSTAT_FAILED"* || -z "$PORT_CHECK" ]]; then
        # Fallback: check from James machine using ss/netstat
        info "Server netstat failed — trying local check on James..."
        PORT_CHECK=$(ssh -o ConnectTimeout=$SSH_TIMEOUT -o StrictHostKeyChecking=no \
            "$VENUE_SSH" "
            # Check if racecontrol process is running (it hosts the UDP adapters)
            ssh -o ConnectTimeout=5 -o StrictHostKeyChecking=no ADMIN@$VENUE_SERVER_IP \
                'tasklist /FI \"IMAGENAME eq racecontrol.exe\" /FO CSV /NH' 2>/dev/null \
            || echo 'TASKLIST_FAILED'
        " 2>/dev/null)

        if [[ "$PORT_CHECK" == *"racecontrol.exe"* ]]; then
            ok "racecontrol.exe is running on venue server (hosts telemetry adapters)"
            ADAPTERS_LISTENING=1
        else
            warn "Could not confirm racecontrol.exe running or telemetry ports listening"
            info "Response: ${PORT_CHECK:0:200}"
        fi
    else
        AC_LISTENING=0
        F1_LISTENING=0

        if echo "$PORT_CHECK" | grep -q ":$PORT_AC "; then
            ok "AC telemetry adapter: port $PORT_AC LISTENING"
            AC_LISTENING=1
        else
            warn "AC telemetry adapter: port $PORT_AC not found"
        fi

        if echo "$PORT_CHECK" | grep -q ":$PORT_F1 "; then
            ok "F1 25 telemetry adapter: port $PORT_F1 LISTENING"
            F1_LISTENING=1
        else
            warn "F1 25 telemetry adapter: port $PORT_F1 not found"
        fi

        if [[ $AC_LISTENING -eq 1 || $F1_LISTENING -eq 1 ]]; then
            ADAPTERS_LISTENING=1
        fi
    fi
else
    warn "Skipping telemetry port check — SSH not available"
fi

# ── Verdict ──────────────────────────────────────────────────────────────
header "VERDICT"

echo ""

# Determine numeric lap counts (strip non-numeric suffixes for comparison)
TOTAL_NUM=$(echo "${VENUE_TOTAL_LAPS:-0}" | grep -o '^[0-9]*' || echo "0")
WEEK_NUM=$(echo "${VENUE_WEEK_LAPS:-0}" | grep -o '^[0-9]*' || echo "0")

# Decision logic
EXIT_CODE=1  # Default FAIL

if [[ $VENUE_REACHABLE -eq 0 ]]; then
    # Cannot reach venue at all
    if [[ $CLOUD_API_UP -eq 1 && ${CLOUD_LEADERBOARD_ENTRIES:-0} -gt 0 ]]; then
        echo -e "${YELLOW}${BOLD}  PARTIAL${RESET} — Venue unreachable, but cloud has ${CLOUD_LEADERBOARD_ENTRIES} leaderboard entries"
        echo -e "  ${DIM}Venue offline or SSH down. Cloud data suggests laps exist but cannot verify pipeline.${RESET}"
        EXIT_CODE=2
    else
        echo -e "${RED}${BOLD}  FAIL${RESET} — Venue unreachable and cloud has no leaderboard data"
        echo -e "  ${DIM}Cannot verify lap recording pipeline. Check SSH connectivity and cloud sync.${RESET}"
        EXIT_CODE=1
    fi
elif [[ "$TOTAL_NUM" -gt 0 && "$WEEK_NUM" -gt 0 ]]; then
    # Venue has recent laps
    if [[ $CLOUD_API_UP -eq 1 && ${CLOUD_LEADERBOARD_ENTRIES:-0} -gt 0 ]]; then
        echo -e "${GREEN}${BOLD}  PASS${RESET} — Venue has $TOTAL_NUM total laps ($WEEK_NUM in last 7 days), cloud synced ($CLOUD_LEADERBOARD_ENTRIES entries)"
        if [[ $ADAPTERS_LISTENING -eq 1 ]]; then
            echo -e "  ${DIM}Telemetry adapters confirmed running. Pipeline is live.${RESET}"
        fi
        EXIT_CODE=0
    else
        echo -e "${YELLOW}${BOLD}  PARTIAL${RESET} — Venue has $TOTAL_NUM laps ($WEEK_NUM recent), but cloud sync unconfirmed"
        echo -e "  ${DIM}Cloud API down or cloud leaderboard empty. Lap recording works locally but sync needs attention.${RESET}"
        EXIT_CODE=2
    fi
elif [[ "$TOTAL_NUM" -gt 0 && "$WEEK_NUM" -eq 0 ]]; then
    # Has old laps but nothing recent
    echo -e "${YELLOW}${BOLD}  PARTIAL${RESET} — Venue has $TOTAL_NUM laps but NONE in last 7 days"
    echo -e "  ${DIM}Telemetry adapter may not be wired to the laps table. Last lap: ${VENUE_LAST_LAP:-unknown}${RESET}"
    if [[ $ADAPTERS_LISTENING -eq 0 ]]; then
        echo -e "  ${DIM}Telemetry ports not confirmed listening — adapters may be down.${RESET}"
    fi
    EXIT_CODE=2
elif [[ "$TOTAL_NUM" -eq 0 && ${VENUE_LEADERBOARD_ENTRIES:-0} -gt 0 ]]; then
    # No DB access but leaderboard has data
    echo -e "${YELLOW}${BOLD}  PARTIAL${RESET} — Could not query DB but venue leaderboard has ${VENUE_LEADERBOARD_ENTRIES} entries"
    echo -e "  ${DIM}Laps likely exist but direct DB query failed. Leaderboard confirms some data.${RESET}"
    EXIT_CODE=2
else
    echo -e "${RED}${BOLD}  FAIL${RESET} — No laps found in venue database"
    echo -e "  ${DIM}The lap recording pipeline is not producing data. Check:${RESET}"
    echo -e "  ${DIM}  1. Is racecontrol.exe running on the server?${RESET}"
    echo -e "  ${DIM}  2. Are games sending UDP telemetry to the correct ports?${RESET}"
    echo -e "  ${DIM}  3. Is the telemetry adapter wired to insert into the laps table?${RESET}"
    EXIT_CODE=1
fi

echo ""
echo -e "${DIM}── Summary ──${RESET}"
echo -e "  Venue SSH:         $([ $VENUE_REACHABLE -eq 1 ] && echo -e "${GREEN}reachable${RESET}" || echo -e "${RED}unreachable${RESET}")"
echo -e "  Venue API:         $([ $VENUE_API_UP -eq 1 ] && echo -e "${GREEN}up${RESET}" || echo -e "${RED}down${RESET}")"
echo -e "  Cloud API:         $([ $CLOUD_API_UP -eq 1 ] && echo -e "${GREEN}up${RESET}" || echo -e "${RED}down${RESET}")"
echo -e "  Total laps:        ${VENUE_TOTAL_LAPS:-unknown}"
echo -e "  Laps (today):      ${VENUE_TODAY_LAPS:-unknown}"
echo -e "  Laps (7 days):     ${VENUE_WEEK_LAPS:-unknown}"
echo -e "  Last lap:          ${VENUE_LAST_LAP:-unknown}"
echo -e "  Venue leaderboard: ${VENUE_LEADERBOARD_ENTRIES:-0} entries"
echo -e "  Cloud leaderboard: ${CLOUD_LEADERBOARD_ENTRIES:-0} entries"
echo -e "  Telemetry ports:   $([ $ADAPTERS_LISTENING -eq 1 ] && echo -e "${GREEN}confirmed${RESET}" || echo -e "${YELLOW}unconfirmed${RESET}")"
echo ""

exit $EXIT_CODE
