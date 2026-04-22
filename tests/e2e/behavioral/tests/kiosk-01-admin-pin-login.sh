#!/usr/bin/env bash
# kiosk-01-admin-pin-login
#
# BEHAVIOR: POST /api/v1/auth/admin-login with the correct PIN returns
#   a JWT with 12h expiry. The PIN is currently 261121 (per session
#   guidance). Server stores argon2id hash — verify_admin_pin validates.
#
# Safe: read-only via login endpoint. No state change.
# On lockout (too many prior fails), this test skips rather than failing.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="kiosk-01-admin-pin-login"
test_start "$TEST_ID" "admin PIN login returns JWT"

resp=$(curl -s --max-time 5 -o /dev/null -w "%{http_code}" \
    -X POST -H "Content-Type: application/json" \
    -d "{\"pin\":\"${ADMIN_PIN}\"}" \
    "${SERVER_URL}/api/v1/auth/admin-login" 2>/dev/null)

if [ "$resp" = "429" ]; then
    report "$TEST_ID" "login" "skip" "rate-limited (429) — lockout active, skipping"
    test_end "$TEST_ID" "skip"
    exit 2
fi

if [ "$resp" != "200" ]; then
    report "$TEST_ID" "login" "fail" "expected 200, got ${resp}" "http=${resp}"
    test_end "$TEST_ID" "fail"
    exit 1
fi

body=$(curl -s --max-time 5 \
    -X POST -H "Content-Type: application/json" \
    -d "{\"pin\":\"${ADMIN_PIN}\"}" \
    "${SERVER_URL}/api/v1/auth/admin-login" 2>/dev/null)

token_present=$(echo "$body" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    t = d.get('token', '')
    print('yes' if t and t.count('.') == 2 else 'no')
except Exception:
    print('no')
" 2>/dev/null)

expires_ok=$(echo "$body" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    e = d.get('expires_in', 0)
    print('yes' if 3600 <= e <= 86400 else 'no')
except Exception:
    print('no')
" 2>/dev/null)

report "$TEST_ID" "response" "observe" "http=${resp} token_present=${token_present} expires_ok=${expires_ok}"

if [ "$token_present" = "yes" ] && [ "$expires_ok" = "yes" ]; then
    report "$TEST_ID" "assert" "pass" "JWT returned with valid expires_in"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "token or expires_in invalid"
test_end "$TEST_ID" "fail"
exit 1
