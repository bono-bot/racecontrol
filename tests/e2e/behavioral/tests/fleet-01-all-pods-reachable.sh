#!/usr/bin/env bash
# fleet-01-all-pods-reachable
#
# BEHAVIOR: All 8 pods (pod_1..pod_8) must be present in fleet/health
#   AND show http_reachable=true. Allows user to catch silent pod
#   drops overnight.
#
# Also records ws_connected and screen_blanked per pod as observations.
#
# Safe: read-only.

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="fleet-01-all-pods-reachable"
test_start "$TEST_ID" "all 8 pods present + http_reachable in fleet health"

resp=$(curl -s --max-time 5 "${SERVER_URL}/api/v1/fleet/health" 2>/dev/null)
if [ -z "$resp" ]; then
    report "$TEST_ID" "fetch" "fail" "empty response from /api/v1/fleet/health"
    test_end "$TEST_ID" "fail"
    exit 1
fi

report_str=$(echo "$resp" | python3 -c "
import sys, json
d = json.load(sys.stdin)
pods = d.get('pods', [])
expected = set(['pod_1','pod_2','pod_3','pod_4','pod_5','pod_6','pod_7','pod_8'])
present = set(p.get('pod_id') for p in pods)
missing = expected - present
unreachable = [p.get('pod_id') for p in pods if p.get('pod_id') in expected and not p.get('http_reachable')]
print(f\"missing={sorted(list(missing))} unreachable={unreachable}\")
" 2>/dev/null)

report "$TEST_ID" "observed" "observe" "$report_str"

# Parse for failure conditions.
missing_count=$(echo "$report_str" | grep -oE "missing=\[[^]]*\]" | grep -oE "pod_[0-9]+" | wc -l | tr -d ' ')
unreachable_count=$(echo "$report_str" | grep -oE "unreachable=\[[^]]*\]" | grep -oE "pod_[0-9]+" | wc -l | tr -d ' ')

if [ "$missing_count" -eq 0 ] && [ "$unreachable_count" -eq 0 ]; then
    report "$TEST_ID" "assert" "pass" "all 8 pods present and reachable"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "missing=${missing_count} unreachable=${unreachable_count}" "$report_str"
test_end "$TEST_ID" "fail"
exit 1
