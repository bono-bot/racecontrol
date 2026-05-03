#!/usr/bin/env bash
# Standalone test harness for conflict-marker-pre-git-add.js
# Runs 5 cases: bare git-add of marker file BLOCKS / clean file passes /
# git-commit with staged marker dirt BLOCKS / non-git Bash silent / bypass override.
#
# Usage:  bash hooks/conflict-marker-pre-git-add.test.sh

set -u

HOOK="$(cd "$(dirname "$0")" && pwd)/conflict-marker-pre-git-add.js"
TMPDIR=$(mktemp -d)
PASS=0
FAIL=0

trap "rm -rf $TMPDIR" EXIT

# Initialise a throwaway git repo for the trigger surface.
git -C "$TMPDIR" init --quiet
git -C "$TMPDIR" config user.email "test@test"
git -C "$TMPDIR" config user.name "test"
echo "ok" > "$TMPDIR/clean.txt"
git -C "$TMPDIR" add clean.txt
git -C "$TMPDIR" commit --quiet -m "init"

assert_exit() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    echo "  PASS: $label (exit=$actual)"
    PASS=$((PASS+1))
  else
    echo "  FAIL: $label (expected $expected, got $actual)"
    FAIL=$((FAIL+1))
  fi
}

# Test 1 — bare `git add <file>` with conflict markers BLOCKS
echo "[test 1] bare git-add of file with conflict markers"
cat > "$TMPDIR/dirty.txt" <<'EOF'
clean line
<<<<<<< HEAD
mine
=======
theirs
>>>>>>> branch
clean line
EOF
PAYLOAD=$(jq -Rs --arg c "git add dirty.txt" --arg cwd "$TMPDIR" '{tool_name:"Bash",tool_input:{command:$c,cwd:$cwd}}' < /dev/null)
echo "$PAYLOAD" | node "$HOOK" 2>/dev/null
assert_exit "blocks add of marker file" 2 "$?"

# Test 2 — bare `git add` of clean file PASSES
echo "[test 2] bare git-add of clean file"
echo "all clean" > "$TMPDIR/ok.txt"
PAYLOAD=$(jq -Rs --arg c "git add ok.txt" --arg cwd "$TMPDIR" '{tool_name:"Bash",tool_input:{command:$c,cwd:$cwd}}' < /dev/null)
echo "$PAYLOAD" | node "$HOOK" 2>/dev/null
assert_exit "passes clean add" 0 "$?"

# Test 3 — `git commit` with already-staged marker content BLOCKS
echo "[test 3] git-commit with marker content already staged"
git -C "$TMPDIR" reset --quiet HEAD .
# Stage the dirty file directly (bypassing the hook for setup)
BYPASS_CONFLICT_MARKER_CHECK=1 git -C "$TMPDIR" add dirty.txt
PAYLOAD=$(jq -Rs --arg c "git commit -m setup" --arg cwd "$TMPDIR" '{tool_name:"Bash",tool_input:{command:$c,cwd:$cwd}}' < /dev/null)
echo "$PAYLOAD" | node "$HOOK" 2>/dev/null
assert_exit "blocks commit of staged marker file" 2 "$?"

# Test 4 — non-git Bash command PASSES silently
echo "[test 4] non-git Bash command pass-through"
PAYLOAD=$(jq -Rs --arg c "ls -la" --arg cwd "$TMPDIR" '{tool_name:"Bash",tool_input:{command:$c,cwd:$cwd}}' < /dev/null)
echo "$PAYLOAD" | node "$HOOK" 2>/dev/null
assert_exit "non-git command passes" 0 "$?"

# Test 5 — BYPASS env-var override
echo "[test 5] BYPASS_CONFLICT_MARKER_CHECK=1 override"
PAYLOAD=$(jq -Rs --arg c "git add dirty.txt" --arg cwd "$TMPDIR" '{tool_name:"Bash",tool_input:{command:$c,cwd:$cwd}}' < /dev/null)
echo "$PAYLOAD" | BYPASS_CONFLICT_MARKER_CHECK=1 node "$HOOK" 2>/dev/null
assert_exit "bypass overrides block" 0 "$?"

echo ""
echo "Results: $PASS passed / $FAIL failed"
[ "$FAIL" -eq 0 ]
