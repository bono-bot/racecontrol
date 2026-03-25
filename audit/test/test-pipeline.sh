#!/usr/bin/env bash
# audit/test/test-pipeline.sh -- Integration test for full audit pipeline wiring
#
# Tests pipeline flag gates, library wiring, and pipeline order.
# DRY-RUN structural test -- does not require live fleet or server.
#
# Usage: bash audit/test/test-pipeline.sh
# Exit: 0 if all tests pass, 1 if any test fails

set -u
set -o pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AUDIT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

PASS_COUNT=0
FAIL_COUNT=0

_pass() {
  local name="$1"
  echo "PASS: $name"
  PASS_COUNT=$((PASS_COUNT + 1))
}

_fail() {
  local name="$1"
  local reason="${2:-}"
  echo "FAIL: $name${reason:+ -- $reason}"
  FAIL_COUNT=$((FAIL_COUNT + 1))
}

echo "=== audit/test/test-pipeline.sh ==="
echo ""

# TEST 1: --auto-fix off by default
# When AUTO_FIX=false, run_auto_fixes returns 0 without creating fixes.jsonl
TEST="TEST 1: --auto-fix off by default"
(
  tmp_dir=$(mktemp -d)
  export AUTO_FIX=false
  export RESULT_DIR="$tmp_dir"
  export PODS=""
  export FLEET_HEALTH_ENDPOINT="http://127.0.0.1:1/never"
  http_get() { return 1; }
  safe_remote_exec() { echo "{}"; }
  emit_fix() { :; }
  export -f http_get safe_remote_exec emit_fix
  source "$AUDIT_DIR/lib/fixes.sh" 2>/dev/null
  run_auto_fixes
  if [ -f "$tmp_dir/fixes.jsonl" ]; then
    rm -rf "$tmp_dir"
    exit 1
  fi
  rm -rf "$tmp_dir"
  exit 0
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "fixes.jsonl was created despite AUTO_FIX=false"; fi

# TEST 2: --notify off by default
# When NOTIFY=false, send_notifications returns 0 without side effects
TEST="TEST 2: --notify off by default"
(
  tmp_dir=$(mktemp -d)
  export NOTIFY=false
  export RESULT_DIR="$tmp_dir"
  export AUDIT_MODE="quick"
  source "$AUDIT_DIR/lib/notify.sh" 2>/dev/null
  send_notifications
  count=$(find "$tmp_dir" -type f 2>/dev/null | wc -l | tr -d " ")
  if [ "${count:-0}" -gt 0 ]; then
    rm -rf "$tmp_dir"
    exit 1
  fi
  rm -rf "$tmp_dir"
  exit 0
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "notification side effects occurred despite NOTIFY=false"; fi

# TEST 3: --commit flag absent means no git activity
# Verify the git commit block is gated on COMMIT=true
TEST="TEST 3: --commit flag absent means no git activity"
block_gate=$(grep -c 'COMMIT:-false' "$AUDIT_DIR/audit.sh" 2>/dev/null || echo "0")
git_add_gate=$(grep -c 'git add.*RESULT_DIR' "$AUDIT_DIR/audit.sh" 2>/dev/null || echo "0")
if [ "${block_gate:-0}" -ge 1 ] && [ "${git_add_gate:-0}" -ge 1 ]; then
  _pass "$TEST"
else
  _fail "$TEST" "COMMIT=true gate not found in audit.sh git commit block"
fi

# TEST 4: pipeline order verification
# suppress < fix < finalize < delta < report < notify < commit
TEST="TEST 4: pipeline order verification"
line_suppress=$(grep -n "apply_suppressions" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_fix=$(grep -n "run_auto_fixes" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_finalize=$(grep -n "finalize_results" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_delta=$(grep -n "compute_delta" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_report=$(grep -n "generate_report" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_notify=$(grep -n "send_notifications" "$AUDIT_DIR/audit.sh" | grep "declare -f" | head -1 | cut -d: -f1)
line_commit=$(grep -n 'COMMIT:-false' "$AUDIT_DIR/audit.sh" | head -1 | cut -d: -f1)
all_found=true
for v in "$line_suppress" "$line_fix" "$line_finalize" "$line_delta" "$line_report" "$line_notify" "$line_commit"; do
  [ -z "$v" ] && all_found=false && break
done
if [ "$all_found" = "true" ] &&
   [ "$line_suppress" -lt "$line_fix" ] &&
   [ "$line_fix" -lt "$line_finalize" ] &&
   [ "$line_finalize" -lt "$line_delta" ] &&
   [ "$line_delta" -lt "$line_report" ] &&
   [ "$line_report" -lt "$line_notify" ] &&
   [ "$line_notify" -lt "$line_commit" ]; then
  _pass "$TEST"
else
  _fail "$TEST" "order: suppress=$line_suppress fix=$line_fix finalize=$line_finalize delta=$line_delta report=$line_report notify=$line_notify commit=$line_commit"
fi

# TEST 5: all libs syntax-check
TEST="TEST 5: all libs syntax-check"
syntax_ok=true
for f in "$AUDIT_DIR/lib/fixes.sh" "$AUDIT_DIR/lib/notify.sh" "$AUDIT_DIR/audit.sh"; do
  if ! bash -n "$f" 2>/dev/null; then
    _fail "$TEST" "syntax error in $f"
    syntax_ok=false
    break
  fi
done
[ "$syntax_ok" = "true" ] && _pass "$TEST"

# TEST 6: APPROVED_FIXES whitelist contains exactly 3 entries
TEST="TEST 6: APPROVED_FIXES whitelist has exactly 3 entries"
(
  export AUTO_FIX=false
  export PODS=""
  http_get() { return 1; }
  safe_remote_exec() { echo "{}"; }
  emit_fix() { :; }
  export -f http_get safe_remote_exec emit_fix
  source "$AUDIT_DIR/lib/fixes.sh" 2>/dev/null
  count="${#APPROVED_FIXES[@]}"
  [ "$count" -eq 3 ] && exit 0 || exit 1
) 2>/dev/null
if [ $? -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "APPROVED_FIXES count is not 3"; fi

# TEST 7: dry-run with all flags exits 0
TEST="TEST 7: dry-run with all flags exits 0"
(
  AUDIT_PIN=000000 bash "$AUDIT_DIR/audit.sh"     --mode quick --auto-fix --notify --commit --dry-run 2>/dev/null
) 2>/dev/null
exit_code=$?
if [ "$exit_code" -eq 0 ]; then _pass "$TEST"; else _fail "$TEST" "dry-run exited $exit_code (expected 0)"; fi

# Summary
echo ""
TOTAL=$((PASS_COUNT + FAIL_COUNT))
echo "${PASS_COUNT}/${TOTAL} tests passed."
[ "$FAIL_COUNT" -gt 0 ] && exit 1
exit 0
