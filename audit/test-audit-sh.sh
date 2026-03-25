#!/usr/bin/env bash
# audit/test-audit-sh.sh — TDD behavioral tests for audit/audit.sh
# Run from repo root: bash audit/test-audit-sh.sh
# Exit 0 = all pass, Exit 1 = failures

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUDIT_SH="$SCRIPT_DIR/audit.sh"
PASS=0
FAIL=0

assert_exit() {
  local desc="$1"
  local expected="$2"
  local actual="$3"
  if [ "$actual" -eq "$expected" ]; then
    echo "PASS: $desc"
    PASS=$((PASS+1))
  else
    echo "FAIL: $desc (expected exit $expected, got $actual)"
    FAIL=$((FAIL+1))
  fi
}

assert_contains() {
  local desc="$1"
  local needle="$2"
  local haystack="$3"
  if echo "$haystack" | grep -q "$needle"; then
    echo "PASS: $desc"
    PASS=$((PASS+1))
  else
    echo "FAIL: $desc (expected to find '$needle' in output)"
    FAIL=$((FAIL+1))
  fi
}

assert_zero() {
  local desc="$1"
  local count="$2"
  if [ "$count" -eq 0 ]; then
    echo "PASS: $desc"
    PASS=$((PASS+1))
  else
    echo "FAIL: $desc (expected 0, got $count)"
    FAIL=$((FAIL+1))
  fi
}

assert_nonzero() {
  local desc="$1"
  local count="$2"
  if [ "$count" -gt 0 ]; then
    echo "PASS: $desc"
    PASS=$((PASS+1))
  else
    echo "FAIL: $desc (expected > 0, got $count)"
    FAIL=$((FAIL+1))
  fi
}

echo "=== audit/audit.sh behavioral tests ==="
echo ""

# Test 1: bash -n syntax check
bash -n "$AUDIT_SH" 2>/dev/null
assert_exit "T1: bash -n syntax check exits 0" 0 $?

# Test 2: no AUDIT_PIN → exit 2, stderr contains AUDIT_PIN
STDERR_T2=$(bash "$AUDIT_SH" --mode quick 2>&1 1>/dev/null)
T2_EXIT=$?
assert_exit "T2: missing AUDIT_PIN exits 2" 2 $T2_EXIT
assert_contains "T2: stderr contains AUDIT_PIN" "AUDIT_PIN" "$STDERR_T2"

# Test 3: no --mode → exit 2, stderr contains --mode
STDERR_T3=$(bash "$AUDIT_SH" 2>&1 1>/dev/null)
T3_EXIT=$?
assert_exit "T3: missing --mode exits 2" 2 $T3_EXIT
assert_contains "T3: stderr contains --mode" "\-\-mode" "$STDERR_T3"

# Test 4: AUDIT_PIN=test --mode quick → exits 0 or 1, creates result dir
# Use DRY_RUN to avoid real network calls
RESULT=$(AUDIT_PIN=test bash "$AUDIT_SH" --mode quick --dry-run 2>&1)
T4_EXIT=$?
# Exit should be 0 (dry run) or 1 (failures)
if [ "$T4_EXIT" -eq 0 ] || [ "$T4_EXIT" -eq 1 ]; then
  echo "PASS: T4: AUDIT_PIN=test --mode quick exits 0 or 1 (got $T4_EXIT)"
  PASS=$((PASS+1))
else
  echo "FAIL: T4: expected exit 0 or 1, got $T4_EXIT"
  FAIL=$((FAIL+1))
fi

# Test 5: no set -e
SET_E_COUNT=$(grep -c "set -e" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_zero "T5: set -e is absent (grep -c returns 0)" "$SET_E_COUNT"

# Test 6: set -u present
SET_U_COUNT=$(grep -c "set -u" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T6: set -u is present" "$SET_U_COUNT"

# Test 7: set -o pipefail present
PIPEFAIL_COUNT=$(grep -c "set -o pipefail" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T7: set -o pipefail is present" "$PIPEFAIL_COUNT"

# Test 8: AUDIT_PIN check with exit 2 present
AUDIT_PIN_CHECK=$(grep -c "AUDIT_PIN" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T8: AUDIT_PIN env var check present" "$AUDIT_PIN_CHECK"

# Test 9: jq prereq check present
JQ_CHECK=$(grep -c "command -v jq" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T9: jq prereq check present" "$JQ_CHECK"

# Test 10: get_session_token auth call present
AUTH_CALL=$(grep -c "get_session_token" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T10: get_session_token call present" "$AUTH_CALL"

# Test 11: RESULT_DIR variable present
RESULT_DIR_VAR=$(grep -c "RESULT_DIR" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T11: RESULT_DIR variable present" "$RESULT_DIR_VAR"

# Test 12: source lib/core present
SOURCE_CORE=$(grep -c "source.*lib/core" "$AUDIT_SH" 2>/dev/null || echo 0)
assert_nonzero "T12: source lib/core line present" "$SOURCE_CORE"

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] && exit 0 || exit 1
