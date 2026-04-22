#!/usr/bin/env bash
# run-all.sh — execute every test under tests/e2e/behavioral/tests/
# in alphanumeric order, aggregate results, emit summary to stdout
# + append summary line to logs/summary.jsonl.
#
# Does NOT exit on first failure. All tests run; final exit code
# reflects aggregate pass/fail.
#
# Environment:
#   TARGET_POD   default pod_8
#   SERVER_URL   default http://192.168.31.23:8080
#   ADMIN_PIN    default 261121
#   LOG_DIR      default tests/e2e/behavioral/logs

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=lib/common.sh
source "${ROOT}/lib/common.sh"

echo ""
echo "╔══════════════════════════════════════════════════════════╗"
echo "║  Behavioral E2E Suite — canary target: ${TARGET_POD:-?}               ║"
echo "║  Started: $(ist_now)                       ║"
echo "╚══════════════════════════════════════════════════════════╝"

# Preflight: target pod must be reachable.
if ! pod_preflight "$TARGET_POD"; then
    echo "[run-all] ABORT: ${TARGET_POD} is not http_reachable per fleet health."
    log_emit "run-all" "preflight" "fail" "${TARGET_POD} unreachable" ""
    exit 2
fi
log_emit "run-all" "preflight" "pass" "${TARGET_POD} reachable" ""

# JWT warm-up.
if ! get_jwt >/dev/null 2>&1; then
    echo "[run-all] ABORT: admin JWT acquisition failed."
    log_emit "run-all" "preflight" "fail" "admin JWT failed" ""
    exit 2
fi

TESTS_DIR="${ROOT}/tests"
PASS=0
FAIL=0
SKIP=0
TOTAL=0
FAILED_IDS=()

for t in "$TESTS_DIR"/*.sh; do
    [ -f "$t" ] || continue
    TOTAL=$((TOTAL + 1))
    test_name=$(basename "$t" .sh)

    # Run in a subshell so a test can't poison the harness state.
    (bash "$t"); rc=$?

    case $rc in
        0) PASS=$((PASS + 1)); echo "    → $test_name: PASS" ;;
        2) SKIP=$((SKIP + 1)); echo "    → $test_name: SKIP" ;;
        *) FAIL=$((FAIL + 1)); FAILED_IDS+=("$test_name"); echo "    → $test_name: FAIL (rc=$rc)" ;;
    esac
done

echo ""
echo "══════ Summary ══════"
echo "  Total:   $TOTAL"
echo "  Passed:  $PASS"
echo "  Failed:  $FAIL"
echo "  Skipped: $SKIP"
if [ $FAIL -gt 0 ]; then
    echo "  Failed tests:"
    for id in "${FAILED_IDS[@]}"; do
        echo "    - $id"
    done
fi
echo "  Log:     $LOG_FILE"
echo ""

# Append summary line to logs/summary.jsonl (single-line run record).
SUMMARY_JSONL="${LOG_DIR}/summary.jsonl"
python3 -c "
import json
d = {
    'ts': '$(ist_now)',
    'total': $TOTAL,
    'pass': $PASS,
    'fail': $FAIL,
    'skip': $SKIP,
    'failed': [$(for id in "${FAILED_IDS[@]}"; do printf '\"%s\",' "$id"; done | sed 's/,$//')],
    'target_pod': '${TARGET_POD}',
    'log': '$(basename "$LOG_FILE")',
}
print(json.dumps(d))
" >> "$SUMMARY_JSONL"

if [ $FAIL -gt 0 ]; then
    exit 1
fi
exit 0
