#!/usr/bin/env bash
# Test harness for post-merge-memory-autolog.sh v2 (JSONL-only)
#
# Creates an isolated sandbox under /tmp with:
#   - a fake racecontrol-like repo with the hook installed
#   - a fake comms-link sibling with data/ dir
#   - mock git commits that simulate PACT merges
# Verifies hook behavior across 6 cases.
#
# Run from the racecontrol repo root:
#   bash scripts/git-hooks/post-merge-memory-autolog.test.sh
#
# Exit code 0 = all passed. Non-zero = failure (with description).

set -euo pipefail

HOOK="$(cd "$(dirname "$0")" && pwd)/post-merge-memory-autolog.sh"
if [ ! -x "$HOOK" ]; then
  chmod +x "$HOOK" 2>/dev/null || true
fi
[ -f "$HOOK" ] || { echo "FAIL: hook not found at $HOOK"; exit 2; }

SANDBOX="$(mktemp -d -t mma-risk2-test-XXXXXX)"
trap 'rm -rf "$SANDBOX"' EXIT

mkdir -p "$SANDBOX/parent/racecontrol"
mkdir -p "$SANDBOX/parent/comms-link/data"

# init the fake racecontrol repo
cd "$SANDBOX/parent/racecontrol"
git init -q -b main
git config user.email "test@example.com"
git config user.name "Test"
echo "init" > README.md
git add README.md
git commit -q -m "init"

LEDGER="$SANDBOX/parent/comms-link/data/memory-autolog.jsonl"
PASS=0
FAIL=0
FAIL_NAMES=""

assert() {
  # assert <name> <condition-cmd>
  local name="$1"; shift
  if eval "$@"; then
    PASS=$((PASS+1))
    echo "  [PASS] $name"
  else
    FAIL=$((FAIL+1))
    FAIL_NAMES="$FAIL_NAMES\n    $name"
    echo "  [FAIL] $name"
  fi
}

# ---- Case 1: PACT-NNN in merge commit message → entry appended -------------
echo "Case 1: PACT-NNN in merge commit message"
git checkout -q -b case1-pact-branch
echo "case1" > case1.txt
git add case1.txt
git commit -q -m "feat: PACT-20260505-004 sample work"
git checkout -q main
git merge -q --no-ff case1-pact-branch -m "Merge PACT-20260505-004 into main"
bash "$HOOK"
assert "Case 1: ledger file created" "[ -f \"$LEDGER\" ]"
assert "Case 1: ledger has 1 line" "[ \"\$(wc -l < \"$LEDGER\")\" -eq 1 ]"
assert "Case 1: ledger entry has pact_id PACT-20260505-004" "grep -q '\"pact_id\":\"PACT-20260505-004\"' \"$LEDGER\""
assert "Case 1: entry is valid JSON" "head -1 \"$LEDGER\" | grep -qE '^\\{.*\\}$'"

# ---- Case 2: idempotency — running hook twice does not duplicate ----------
echo "Case 2: idempotency check"
bash "$HOOK"
assert "Case 2: ledger still has 1 line after re-run" "[ \"\$(wc -l < \"$LEDGER\")\" -eq 1 ]"

# ---- Case 3: pact-* branch but no PACT-NNN in message → entry appended ----
echo "Case 3: pact-* branch detection"
git checkout -q -b pact-20260505-005-style-only-branch
echo "case3" > case3.txt
git add case3.txt
git commit -q -m "wip without explicit PACT id"
git checkout -q main
git merge -q --no-ff pact-20260505-005-style-only-branch -m "Merge pact-20260505-005-style-only-branch"
bash "$HOOK"
LINES=$(wc -l < "$LEDGER")
assert "Case 3: ledger grew to 2 lines" "[ \"$LINES\" -eq 2 ]"
assert "Case 3: tail entry has source_branch with pact-" "tail -1 \"$LEDGER\" | grep -q '\"source_branch\":\"pact-20260505-005'"

# ---- Case 4: non-PACT merge → no entry ------------------------------------
echo "Case 4: non-PACT merge → silent no-op"
git checkout -q -b ordinary-feature-branch
echo "case4" > case4.txt
git add case4.txt
git commit -q -m "regular work, no PACT here"
git checkout -q main
git merge -q --no-ff ordinary-feature-branch -m "Merge ordinary-feature-branch"
bash "$HOOK"
LINES=$(wc -l < "$LEDGER")
assert "Case 4: ledger still 2 lines (no new entry)" "[ \"$LINES\" -eq 2 ]"

# ---- Case 5: BYPASS env var → no entry even on PACT merge -----------------
echo "Case 5: BYPASS_MEMORY_AUTOLOG=1"
git checkout -q -b case5-pact-branch
echo "case5" > case5.txt
git add case5.txt
git commit -q -m "feat: PACT-20260505-006 should not be logged"
git checkout -q main
git merge -q --no-ff case5-pact-branch -m "Merge PACT-20260505-006 into main"
BYPASS_MEMORY_AUTOLOG=1 bash "$HOOK"
LINES=$(wc -l < "$LEDGER")
assert "Case 5: ledger still 2 lines (BYPASS suppressed)" "[ \"$LINES\" -eq 2 ]"
# Now run without bypass — should append (catch-up after bypass)
bash "$HOOK"
LINES=$(wc -l < "$LEDGER")
assert "Case 5b: post-bypass run appends (now 3 lines)" "[ \"$LINES\" -eq 3 ]"

# ---- Case 6: missing comms-link sibling → silent no-op ---------------------
echo "Case 6: missing comms-link sibling"
mv "$SANDBOX/parent/comms-link" "$SANDBOX/parent/comms-link.tmp"
git checkout -q -b case6-pact-branch
echo "case6" > case6.txt
git add case6.txt
git commit -q -m "feat: PACT-20260505-007 with comms-link absent"
git checkout -q main
git merge -q --no-ff case6-pact-branch -m "Merge PACT-20260505-007 into main"
RC=0
bash "$HOOK" || RC=$?
assert "Case 6: hook exits 0 (no error)" "[ \"$RC\" -eq 0 ]"
mv "$SANDBOX/parent/comms-link.tmp" "$SANDBOX/parent/comms-link"
# After comms-link returns, re-run hook — should append the now-merged commit
bash "$HOOK"
LINES=$(wc -l < "$LEDGER")
assert "Case 6b: post-restore run appends (now 4 lines)" "[ \"$LINES\" -eq 4 ]"

# ---- Final report ---------------------------------------------------------
echo ""
echo "============================================"
echo "Total: $((PASS+FAIL)) | PASS: $PASS | FAIL: $FAIL"
if [ "$FAIL" -gt 0 ]; then
  printf "Failed cases:%b\n" "$FAIL_NAMES"
  exit 1
fi
echo "============================================"
echo "Ledger snapshot:"
cat "$LEDGER"
echo "============================================"
exit 0
