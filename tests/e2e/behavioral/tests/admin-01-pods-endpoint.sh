#!/usr/bin/env bash
# admin-01-pods-endpoint
#
# BEHAVIOR: GET /api/v1/pods (staff-JWT-protected) must return 200 and
#   a JSON array/object with >=8 pod entries. This validates admin
#   dashboard's primary data source.
#
# Safe: read-only.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="admin-01-pods-endpoint"
test_start "$TEST_ID" "/api/v1/pods returns 200 with >=8 pods (staff JWT)"

token=$(get_jwt)
if [ -z "$token" ]; then
    report "$TEST_ID" "auth" "skip" "JWT acquisition failed"
    test_end "$TEST_ID" "skip"
    exit 2
fi

http=$(curl -s --max-time 5 -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer ${token}" \
    "${SERVER_URL}/api/v1/pods" 2>/dev/null)

if [ "$http" != "200" ]; then
    report "$TEST_ID" "fetch" "fail" "expected 200, got ${http}" "http=${http}"
    test_end "$TEST_ID" "fail"
    exit 1
fi

count=$(curl -s --max-time 5 \
    -H "Authorization: Bearer ${token}" \
    "${SERVER_URL}/api/v1/pods" 2>/dev/null | \
    python3 -c "
import sys, json
d = json.load(sys.stdin)
if isinstance(d, list):
    print(len(d))
elif isinstance(d, dict):
    # e.g., {'pods': [...]}
    for k in ('pods', 'items', 'data'):
        if k in d and isinstance(d[k], list):
            print(len(d[k]))
            sys.exit(0)
    print(0)
else:
    print(0)
" 2>/dev/null)

report "$TEST_ID" "observed" "observe" "http=${http} pod_count=${count}"

if [ "$count" -ge 8 ]; then
    report "$TEST_ID" "assert" "pass" "${count} pods returned"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "expected >=8 pods, got ${count}"
test_end "$TEST_ID" "fail"
exit 1
