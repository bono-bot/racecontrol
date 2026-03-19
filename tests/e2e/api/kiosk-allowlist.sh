#!/usr/bin/env bash
# tests/e2e/api/kiosk-allowlist.sh — Kiosk Allowlist API CRUD E2E test (Phase 48)
# Tests: GET list, POST add, verify add, duplicate handling, baseline guard, DELETE, verify delete
# Usage: bash tests/e2e/api/kiosk-allowlist.sh
#   RC_BASE_URL=http://192.168.31.23:8080 bash tests/e2e/api/kiosk-allowlist.sh
set -uo pipefail

SERVER="${RC_BASE_URL:-http://192.168.31.23:8080}"
API="$SERVER/api/v1"
TEST_PROCESS="test_phase48_dummy.exe"

SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=../lib/common.sh
source "$SCRIPT_DIR/../lib/common.sh"
# shellcheck source=../lib/pod-map.sh
source "$SCRIPT_DIR/../lib/pod-map.sh"

echo "========================================"
echo "Kiosk Allowlist API CRUD Test (Phase 48)"
echo "Server : ${SERVER}"
echo "API    : ${API}"
echo "========================================"
echo ""

# Cleanup function — always delete test entry to avoid polluting DB
cleanup() {
    curl -sf -X DELETE "${API}/config/kiosk-allowlist/${TEST_PROCESS}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# ─── Gate 1: API reachable ─────────────────────────────────────────────────
echo "--- Gate 1: API Reachable ---"
if curl -sf --max-time 5 "${API}/health" >/dev/null 2>&1; then
    pass "Gate 1: Server reachable at ${SERVER}"
else
    fail "Gate 1: racecontrol not reachable at ${SERVER}"
    echo ""
    echo "Cannot proceed — server is down."
    summary_exit
fi

# ─── Gate 2: GET returns list with hardcoded_count ─────────────────────────
echo ""
echo "--- Gate 2: GET Allowlist ---"
GET_RESP=$(curl -sf --max-time 10 "${API}/config/kiosk-allowlist" 2>/dev/null)
if [ -z "$GET_RESP" ]; then
    fail "Gate 2: GET /config/kiosk-allowlist returned empty response"
else
    HAS_ALLOWLIST=$(echo "$GET_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok' if 'allowlist' in d else 'missing')" 2>/dev/null || echo "parse_error")
    HAS_COUNT=$(echo "$GET_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok' if d.get('hardcoded_count',0)>0 else 'missing')" 2>/dev/null || echo "parse_error")

    if [ "$HAS_ALLOWLIST" = "ok" ] && [ "$HAS_COUNT" = "ok" ]; then
        COUNT=$(echo "$GET_RESP" | python3 -c "import sys,json; print(json.load(sys.stdin).get('hardcoded_count',0))" 2>/dev/null || echo "?")
        pass "Gate 2: allowlist array present, hardcoded_count=${COUNT}"
    elif [ "$HAS_ALLOWLIST" = "ok" ]; then
        fail "Gate 2: allowlist present but hardcoded_count missing or zero"
    else
        fail "Gate 2: response missing allowlist field — got: ${GET_RESP}"
    fi
fi

# ─── Gate 3: POST add entry ─────────────────────────────────────────────────
echo ""
echo "--- Gate 3: POST Add Entry ---"
POST_RESP=$(curl -sf --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"process_name\":\"${TEST_PROCESS}\",\"notes\":\"e2e test\"}" \
    -w "\n%{http_code}" \
    "${API}/config/kiosk-allowlist" 2>/dev/null)

POST_BODY=$(echo "$POST_RESP" | python3 -c "import sys; lines=sys.stdin.read().strip().split('\n'); print('\n'.join(lines[:-1]))" 2>/dev/null || echo "")
POST_CODE=$(echo "$POST_RESP" | python3 -c "import sys; lines=sys.stdin.read().strip().split('\n'); print(lines[-1])" 2>/dev/null || echo "0")

HAS_ID=$(echo "$POST_BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok' if d.get('id') else 'missing')" 2>/dev/null || echo "parse_error")
HAS_NAME=$(echo "$POST_BODY" | python3 -c "import sys,json; d=json.load(sys.stdin); print('ok' if d.get('process_name') else 'missing')" 2>/dev/null || echo "parse_error")

if [ "$POST_CODE" = "201" ] && [ "$HAS_ID" = "ok" ] && [ "$HAS_NAME" = "ok" ]; then
    ENTRY_ID=$(echo "$POST_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('id',''))" 2>/dev/null || echo "")
    pass "Gate 3: Entry added (HTTP 201), id=${ENTRY_ID}"
elif [ "$POST_CODE" = "200" ] || [ "$POST_CODE" = "201" ]; then
    pass "Gate 3: Entry added (HTTP ${POST_CODE})"
else
    fail "Gate 3: POST failed (HTTP ${POST_CODE}): ${POST_BODY}"
fi

# ─── Gate 4: GET verify added ───────────────────────────────────────────────
echo ""
echo "--- Gate 4: GET Verify Entry Added ---"
GET_AFTER=$(curl -sf --max-time 10 "${API}/config/kiosk-allowlist" 2>/dev/null)
FOUND_IN_LIST=$(echo "$GET_AFTER" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    names = [e.get('process_name','').lower() for e in d.get('allowlist', [])]
    print('found' if '${TEST_PROCESS}'.lower() in names else 'not_found')
except Exception as e:
    print('parse_error')
" 2>/dev/null || echo "parse_error")

if [ "$FOUND_IN_LIST" = "found" ]; then
    pass "Gate 4: ${TEST_PROCESS} appears in allowlist after POST"
elif [ "$FOUND_IN_LIST" = "not_found" ]; then
    fail "Gate 4: ${TEST_PROCESS} not found in allowlist after POST"
else
    fail "Gate 4: Could not parse GET response after POST"
fi

# ─── Gate 5: POST duplicate — should not error ──────────────────────────────
echo ""
echo "--- Gate 5: POST Duplicate (idempotent) ---"
DUP_CODE=$(curl -sf --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d "{\"process_name\":\"${TEST_PROCESS}\",\"notes\":\"e2e test duplicate\"}" \
    -o /dev/null \
    -w "%{http_code}" \
    "${API}/config/kiosk-allowlist" 2>/dev/null || echo "000")

if [ "$DUP_CODE" = "200" ] || [ "$DUP_CODE" = "201" ]; then
    pass "Gate 5: Duplicate POST returns HTTP ${DUP_CODE} (no error)"
else
    fail "Gate 5: Duplicate POST returned HTTP ${DUP_CODE} (expected 200 or 201)"
fi

# ─── Gate 6: POST baseline process — should get already_in_baseline ─────────
echo ""
echo "--- Gate 6: POST Baseline Process (guard check) ---"
BASELINE_RESP=$(curl -sf --max-time 10 -X POST \
    -H "Content-Type: application/json" \
    -d '{"process_name":"svchost.exe","notes":"baseline guard test"}' \
    "${API}/config/kiosk-allowlist" 2>/dev/null)

if echo "$BASELINE_RESP" | python3 -c "import sys,json; d=json.load(sys.stdin); print('yes' if 'already_in_baseline' in str(d) else 'no')" 2>/dev/null | grep -q "yes"; then
    pass "Gate 6: svchost.exe returns already_in_baseline response"
else
    # Some implementations might return 200 OK without the marker — soft fail
    info "Gate 6 info: baseline guard response: ${BASELINE_RESP}"
    if echo "$BASELINE_RESP" | python3 -c "import sys; sys.exit(0 if 'baseline' in sys.stdin.read().lower() else 1)" 2>/dev/null; then
        pass "Gate 6: svchost.exe response contains 'baseline' keyword"
    else
        fail "Gate 6: svchost.exe not identified as baseline entry — response: ${BASELINE_RESP}"
    fi
fi

# ─── Gate 7: DELETE entry ───────────────────────────────────────────────────
echo ""
echo "--- Gate 7: DELETE Entry ---"
DEL_CODE=$(curl -sf --max-time 10 -X DELETE \
    -o /dev/null \
    -w "%{http_code}" \
    "${API}/config/kiosk-allowlist/${TEST_PROCESS}" 2>/dev/null || echo "000")

if [ "$DEL_CODE" = "204" ] || [ "$DEL_CODE" = "200" ]; then
    pass "Gate 7: DELETE ${TEST_PROCESS} returned HTTP ${DEL_CODE}"
else
    fail "Gate 7: DELETE returned HTTP ${DEL_CODE} (expected 204)"
fi

# ─── Gate 8: GET verify deleted ─────────────────────────────────────────────
echo ""
echo "--- Gate 8: GET Verify Entry Deleted ---"
GET_FINAL=$(curl -sf --max-time 10 "${API}/config/kiosk-allowlist" 2>/dev/null)
STILL_IN_LIST=$(echo "$GET_FINAL" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    names = [e.get('process_name','').lower() for e in d.get('allowlist', [])]
    print('still_present' if '${TEST_PROCESS}'.lower() in names else 'gone')
except Exception as e:
    print('parse_error')
" 2>/dev/null || echo "parse_error")

if [ "$STILL_IN_LIST" = "gone" ]; then
    pass "Gate 8: ${TEST_PROCESS} no longer in allowlist after DELETE"
elif [ "$STILL_IN_LIST" = "still_present" ]; then
    fail "Gate 8: ${TEST_PROCESS} still appears in allowlist after DELETE"
else
    fail "Gate 8: Could not parse GET response after DELETE"
fi

# ─── Summary ────────────────────────────────────────────────────────────────
echo ""
summary_exit
