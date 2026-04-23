#!/usr/bin/env bash
# game-03-active-failed-launch-no-steam-respawn
#
# BEHAVIOR: Actively trigger a deliberately-failing AC launch via
#   /api/v1/games/launch with an invalid track. Then assert that:
#     - no NEW steam.exe PID appears within 30s (proves game_doctor
#       Check 12 is NOT auto-spawning Steam on retries)
#     - server returns 200 (launch accepted/queued) OR 4xx (validation
#       rejected) — both outcomes are fine; what's NOT fine is retries
#       repeatedly spawning Steam
#
# This is the ACTIVE verification of the PR #18 Steam fix. game-01
# passively observes; this one actively triggers the failure path.
#
# SAFE when rc-agent on TARGET_POD is PR #18 or later. Do NOT run
# against a pod on older builds unless you're OK seeing Steam respawn
# (that's the bug we're confirming is fixed).
#
# Requires env: TARGET_POD, ADMIN_PIN (for JWT)
# Optional: SKIP_LAUNCH=1 — just record baseline, skip launch

set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT}/lib/common.sh"

TEST_ID="game-03-active-failed-launch-no-steam-respawn"
test_start "$TEST_ID" "deliberately-failing launch → no steam.exe respawn within 30s"

if ! pod_preflight "$TARGET_POD"; then
    report "$TEST_ID" "preflight" "skip" "${TARGET_POD} not reachable"
    test_end "$TEST_ID" "skip"
    exit 2
fi

# Snapshot steam.exe PIDs before the launch attempt.
pids_before=$(pod_exec "$TARGET_POD" "tasklist /FI \\\"IMAGENAME eq steam.exe\\\" /FO CSV /NH" | \
    python3 -c "
import sys, json, re
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    pids = sorted(re.findall(r'^\"[^\"]+\",\"(\d+)\"', stdout, re.M))
    print(','.join(pids))
except Exception:
    pass
" 2>/dev/null)
count_before=$(echo "$pids_before" | tr ',' '\n' | grep -c . || true)
report "$TEST_ID" "snapshot-before" "observe" "count=${count_before} pids=${pids_before}"

if [ "${SKIP_LAUNCH:-0}" = "1" ]; then
    report "$TEST_ID" "launch" "skip" "SKIP_LAUNCH=1 — baseline only"
    test_end "$TEST_ID" "skip"
    exit 2
fi

# Trigger a deliberately-failing AC launch.
# Invalid track id "___nonexistent_track___" will fail pre_launch_validate
# with "Track '___nonexistent_track___' not found" without ever reaching
# Steam. This exercises the retry+doctor path without starting a real race.
token=$(get_jwt)
if [ -z "$token" ]; then
    report "$TEST_ID" "auth" "skip" "JWT acquisition failed"
    test_end "$TEST_ID" "skip"
    exit 2
fi

payload=$(mktemp)
cat > "$payload" <<JSON
{
  "pod_id": "${TARGET_POD}",
  "game": "assetto_corsa",
  "car": "bmw_m3_e30",
  "track": "___nonexistent_track___",
  "track_config": "",
  "force_supersede": true
}
JSON

launch_http=$(curl -s --max-time 8 -o /dev/null -w "%{http_code}" \
    -X POST -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${token}" \
    --data @"$payload" \
    "${SERVER_URL}/api/v1/games/launch" 2>/dev/null)
rm -f "$payload"
report "$TEST_ID" "launch-sent" "observe" "http=${launch_http} (any response OK; we care about Steam behaviour after)"

# Wait 30 seconds — long enough for 2-3 diagnostic ticks on event_loop
# if the pre-PR #18 bug were still present it would have spawned one
# or more steam.exe by now.
sleep 30

pids_after=$(pod_exec "$TARGET_POD" "tasklist /FI \\\"IMAGENAME eq steam.exe\\\" /FO CSV /NH" | \
    python3 -c "
import sys, json, re
try:
    d = json.load(sys.stdin)
    stdout = d.get('stdout', '')
    pids = sorted(re.findall(r'^\"[^\"]+\",\"(\d+)\"', stdout, re.M))
    print(','.join(pids))
except Exception:
    pass
" 2>/dev/null)
count_after=$(echo "$pids_after" | tr ',' '\n' | grep -c . || true)
report "$TEST_ID" "snapshot-after" "observe" "count=${count_after} pids=${pids_after}"

# Compute NEW PIDs (in after, not in before).
new_pids=$(python3 -c "
before = set('${pids_before}'.split(',')) - {''}
after = set('${pids_after}'.split(',')) - {''}
new = sorted(after - before)
print(','.join(new))
")

if [ -z "$new_pids" ]; then
    report "$TEST_ID" "assert" "pass" "no new steam.exe PIDs during 30s window after failed launch"
    test_end "$TEST_ID" "pass"
    exit 0
fi
report "$TEST_ID" "assert" "fail" "NEW steam.exe PIDs appeared: ${new_pids}" "before=${pids_before} after=${pids_after} new=${new_pids}"
test_end "$TEST_ID" "fail"
exit 1
