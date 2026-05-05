#!/bin/bash
# scripts/git-hooks/pre-commit-state-divergence.test.sh — 10-case test harness.
#
# Phase 1 substrate of PACT-20260505-004 P6. Validates pre-commit-state-divergence.sh
# behaviour across the spec'd cases (§3 #2 + bono §AMEND-1 CAVEAT-A/B/C cases).
#
# Cases (10 total; mapping to bono §AMEND-1 absorption + spec §3 #2):
#   1. clean-on-latest commit PASSES
#   2. local behind origin/main by 5 commits BLOCKS with recovery message
#   3. commit message §S-99 when local highest is §S-44 BLOCKS
#   4. detached HEAD silent (no-op)
#   5. non-git working dir silent (no-op)
#   6. BYPASS env-var override PASSES with audit-log entry
#   7. allowlist path-only commit (.tmp/foo.txt) PASSES
#   8. CAVEAT-A: rotation fires when ledger >10 MB; archive landed; ledger truncated
#   9. CAVEAT-B: anchored regex ignores body-text "§S-99 cited below" when only §S-50 in headers
#  10. CAVEAT-C: missing comms-link sibling + VERBOSE=1 -> stderr has skip msg; without -> empty
#
# Usage: bash scripts/git-hooks/pre-commit-state-divergence.test.sh
# Exit codes: 0 = all pass; non-zero = N failures (count printed at end).
#
# Cross-platform: POSIX bash + git + standard coreutils. Tested on Linux + Git Bash.

set -u

HOOK_SCRIPT="$(cd "$(dirname "$0")" && pwd)/pre-commit-state-divergence.sh"
[ -f "$HOOK_SCRIPT" ] || { echo "[test] hook script not found at $HOOK_SCRIPT" >&2; exit 2; }

PASS_COUNT=0
FAIL_COUNT=0
FAILED_CASES=()

# -- Test plumbing ----------------------------------------------------------

# Spin up an isolated git repo with a fake remote tracking origin/main.
# Optionally also creates a sibling comms-link with a V2-MASTER-STATE.md.
# Stdout: absolute path to the created repo (caller cd's into it).
make_test_repo() {
  local tmp
  tmp="$(mktemp -d)"
  (
    cd "$tmp" || exit 1

    # Create the "remote" first.
    git init -q --bare remote.git
    # Create the local clone-equivalent.
    mkdir -p local
    cd local
    git init -q
    git config user.email "test@example"
    git config user.name "test"
    git config commit.gpgsign false
    git remote add origin "$tmp/remote.git"

    # Initial commit so HEAD exists.
    echo "init" > README
    git add README
    git commit -q -m "init"
    git push -q origin HEAD:main
    git checkout -q -b main 2>/dev/null || true
    git branch --set-upstream-to=origin/main main 2>/dev/null || true
  ) >&2
  echo "$tmp/local"
}

make_comms_link_sibling() {
  local repo_root="$1"
  local highest_n="${2:-44}"
  local sibling="$(dirname "$repo_root")/comms-link"
  mkdir -p "$sibling"
  {
    echo "# V2-MASTER-STATE.md (test fixture)"
    echo
    local i
    for i in $(seq 1 "$highest_n"); do
      echo "## §S-$i — Snapshot test fixture entry $i"
      echo "(body)"
      echo
    done
  } > "$sibling/V2-MASTER-STATE.md"
  echo "$sibling"
}

run_hook() {
  # Wrapper: run hook in current PWD, capture stdout/stderr/exit.
  local stderr_file
  stderr_file="$(mktemp)"
  local out
  out="$(bash "$HOOK_SCRIPT" 2>"$stderr_file")"
  local rc=$?
  HOOK_STDOUT="$out"
  HOOK_STDERR="$(cat "$stderr_file")"
  HOOK_EXIT="$rc"
  rm -f "$stderr_file"
}

assert_eq() {
  local label="$1" expected="$2" actual="$3"
  if [ "$expected" = "$actual" ]; then
    : # silent pass
  else
    echo "  ASSERT-FAIL [$label]: expected='$expected' actual='$actual'" >&2
    return 1
  fi
}

assert_match() {
  local label="$1" pattern="$2" haystack="$3"
  if printf '%s' "$haystack" | grep -qE -- "$pattern"; then
    :
  else
    echo "  ASSERT-FAIL [$label]: pattern='$pattern' not found in haystack" >&2
    echo "  haystack: ${haystack:0:200}" >&2
    return 1
  fi
}

assert_no_match() {
  local label="$1" pattern="$2" haystack="$3"
  if ! printf '%s' "$haystack" | grep -qE -- "$pattern"; then
    :
  else
    echo "  ASSERT-FAIL [$label]: pattern='$pattern' UNEXPECTEDLY found in haystack" >&2
    return 1
  fi
}

run_case() {
  local name="$1"
  local fn="$2"
  echo "[test] CASE $name"
  if ( "$fn" ); then
    PASS_COUNT=$((PASS_COUNT + 1))
    echo "  -> PASS"
  else
    FAIL_COUNT=$((FAIL_COUNT + 1))
    FAILED_CASES+=("$name")
    echo "  -> FAIL"
  fi
}

# -- Test cases -------------------------------------------------------------

case_1_clean_on_latest() {
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 50 >/dev/null
  cd "$repo" || return 1

  echo "edit" >> README
  git add README
  printf 'normal commit body\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1
  return 0
}

case_2_behind_blocks() {
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 50 >/dev/null
  cd "$repo" || return 1

  # Push 5 commits to remote that local doesn't have.
  local clone2
  clone2="$(dirname "$repo")/clone2"
  git clone -q "$(git config --get remote.origin.url)" "$clone2"
  (
    cd "$clone2" || exit 1
    git config user.email "other@example"
    git config user.name "other"
    git config commit.gpgsign false
    local i
    for i in 1 2 3 4 5; do
      echo "remote-$i" >> README
      git add README
      git commit -q -m "remote commit $i"
    done
    git push -q origin main
  )

  # Local is now 5 behind. Fetch happens inside the hook.
  echo "edit" >> README
  git add README
  printf 'normal commit body\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-1" "1" "$HOOK_EXIT" || return 1
  assert_match "block-msg" "BLOCK: local branch is" "$HOOK_STDERR" || return 1
  return 0
}

case_3_forward_ref_blocks() {
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 44 >/dev/null
  cd "$repo" || return 1

  echo "edit" >> README
  git add README
  printf 'commit referencing §S-99 cited inline\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-1" "1" "$HOOK_EXIT" || return 1
  assert_match "forward-ref-block-msg" "BLOCK: commit message references" "$HOOK_STDERR" || return 1
  return 0
}

case_4_detached_head() {
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 50 >/dev/null
  cd "$repo" || return 1

  # Detach HEAD by checking out the commit hash.
  local sha
  sha="$(git rev-parse HEAD)"
  git checkout -q "$sha"

  echo "edit" >> README
  git add README
  printf 'detached commit\n' > .git/COMMIT_EDITMSG

  run_hook
  # Edge case: hook should silent-skip (exit 0).
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1
  return 0
}

case_5_non_git_dir() {
  local tmp
  tmp="$(mktemp -d)"
  cd "$tmp" || return 1
  printf 'msg\n' > /tmp/state-divergence-not-a-msg-file.tmp
  run_hook
  # Outside git: hook tries `git rev-parse --abbrev-ref HEAD` which fails -> silent skip.
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1
  return 0
}

case_6_bypass_env() {
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 44 >/dev/null
  cd "$repo" || return 1

  # Make state stale (remote-ahead) so hook would normally block.
  local clone2
  clone2="$(dirname "$repo")/clone2"
  git clone -q "$(git config --get remote.origin.url)" "$clone2"
  (
    cd "$clone2" || exit 1
    git config user.email o@e
    git config user.name o
    git config commit.gpgsign false
    for i in 1 2 3; do
      echo "x$i" >> README
      git add README
      git commit -q -m "x$i"
    done
    git push -q origin main
  )

  echo "edit" >> README
  git add README
  printf 'msg\n' > .git/COMMIT_EDITMSG

  BYPASS_STATE_DIVERGENCE_CHECK=1 run_hook
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1
  # Audit row should record bypass.
  assert_match "audit-bypass" "bypass-env" "$(cat data/state-divergence-audit.jsonl 2>/dev/null)" || return 1
  return 0
}

case_7_allowlist_path() {
  local repo
  repo="$(make_test_repo)"
  cd "$repo" || return 1

  mkdir -p .tmp
  echo "tmp content" > .tmp/foo.txt
  git add .tmp/foo.txt
  printf 'msg\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1
  return 0
}

case_8_rotation() {
  # CAVEAT-A: rotation fires when ledger > 10 MB.
  local repo
  repo="$(make_test_repo)"
  make_comms_link_sibling "$repo" 50 >/dev/null
  cd "$repo" || return 1

  mkdir -p data
  # Pre-fill audit ledger with >10 MB of valid JSONL.
  local row='{"ts":"2025-01-01T00:00:00Z","outcome":"pass","local_head":"x","origin_head":"x","branch":"main","detail":""}'
  python3 -c "import sys; row=sys.argv[1]; n=int((10*1024*1024)/(len(row)+1)) + 100; sys.stdout.write((row+chr(10))*n)" "$row" \
    > data/state-divergence-audit.jsonl 2>/dev/null \
    || {
      # Fallback if python3 unavailable: shell loop (slower but works).
      local i n=$((10 * 1024 * 1024 / (${#row} + 1) + 100))
      : > data/state-divergence-audit.jsonl
      for ((i = 0; i < n; i++)); do echo "$row" >> data/state-divergence-audit.jsonl; done
    }

  local size_before
  size_before="$(stat -c '%s' data/state-divergence-audit.jsonl 2>/dev/null \
    || stat -f '%z' data/state-divergence-audit.jsonl 2>/dev/null \
    || wc -c < data/state-divergence-audit.jsonl)"

  echo "edit" >> README
  git add README
  printf 'msg\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-0" "0" "$HOOK_EXIT" || return 1

  # Verify rotation: archive sibling exists + ledger size shrank dramatically.
  local archive
  archive="$(ls data/state-divergence-audit-*.jsonl.gz 2>/dev/null | head -n 1)"
  if [ -z "$archive" ]; then
    echo "  ASSERT-FAIL: no rotation archive landed (size_before=$size_before)" >&2
    return 1
  fi

  local size_after
  size_after="$(stat -c '%s' data/state-divergence-audit.jsonl 2>/dev/null \
    || stat -f '%z' data/state-divergence-audit.jsonl 2>/dev/null \
    || wc -c < data/state-divergence-audit.jsonl)"
  # After rotation: file truncated to 0 then one new "pass" row appended; should be < 1 KB.
  if [ "$size_after" -gt 1024 ]; then
    echo "  ASSERT-FAIL: ledger not truncated (size_before=$size_before size_after=$size_after)" >&2
    return 1
  fi
  return 0
}

case_9_anchored_regex() {
  # CAVEAT-B: body-text §S-99 must NOT raise local_highest above the actual highest header §S-50.
  local repo
  repo="$(make_test_repo)"
  local sibling
  sibling="$(make_comms_link_sibling "$repo" 50)"
  # Append body-text reference WITHOUT a header.
  cat >> "$sibling/V2-MASTER-STATE.md" <<'EOF'

(body text: per §S-99 cited below — this is a body line, NOT a header)
EOF
  cd "$repo" || return 1

  echo "edit" >> README
  git add README
  # Reference §S-50 in commit msg — should pass since local headers go to §S-50.
  printf 'normal commit referencing §S-50\n' > .git/COMMIT_EDITMSG

  run_hook
  assert_eq "exit-0-§S-50-pass" "0" "$HOOK_EXIT" || return 1

  # Now reference §S-99 in commit msg — should BLOCK (regex anchored, body-text §S-99 not extracted).
  printf 'commit referencing §S-99\n' > .git/COMMIT_EDITMSG
  run_hook
  assert_eq "exit-1-§S-99-block" "1" "$HOOK_EXIT" || return 1
  assert_match "anchored-block-msg" "highest header is §S-50" "$HOOK_STDERR" || return 1
  return 0
}

case_10_verbose_skip_msg() {
  # CAVEAT-C: missing comms-link sibling + VERBOSE=1 -> stderr has skip msg.
  local repo
  repo="$(make_test_repo)"
  # Do NOT create comms-link sibling.
  cd "$repo" || return 1

  echo "edit" >> README
  git add README
  printf 'msg\n' > .git/COMMIT_EDITMSG

  # Without VERBOSE: stderr should be silent.
  run_hook
  assert_eq "exit-0-noverbose" "0" "$HOOK_EXIT" || return 1
  if printf '%s' "$HOOK_STDERR" | grep -qE 'comms-link sibling not found'; then
    echo "  ASSERT-FAIL: stderr contained skip msg without VERBOSE" >&2
    return 1
  fi

  # With VERBOSE=1: stderr should include skip msg.
  STATE_DIVERGENCE_VERBOSE=1 run_hook
  assert_eq "exit-0-verbose" "0" "$HOOK_EXIT" || return 1
  assert_match "verbose-skip-msg" "comms-link sibling not found" "$HOOK_STDERR" || return 1
  return 0
}

# -- Run all cases ----------------------------------------------------------

ROTATE_JSONL_DISABLE_ORIG="${ROTATE_JSONL_DISABLE:-}"
# Default: rotation enabled (Case 8 expects it; other cases unaffected since their ledgers are tiny).

run_case "1: clean-on-latest"            case_1_clean_on_latest
run_case "2: behind blocks"              case_2_behind_blocks
run_case "3: forward §S-N ref blocks"    case_3_forward_ref_blocks
run_case "4: detached HEAD silent"       case_4_detached_head
run_case "5: non-git dir silent"         case_5_non_git_dir
run_case "6: BYPASS env"                 case_6_bypass_env
run_case "7: allowlist path-only"        case_7_allowlist_path
run_case "8: rotation (CAVEAT-A)"        case_8_rotation
run_case "9: anchored regex (CAVEAT-B)"  case_9_anchored_regex
run_case "10: verbose skip (CAVEAT-C)"   case_10_verbose_skip_msg

ROTATE_JSONL_DISABLE="$ROTATE_JSONL_DISABLE_ORIG"

echo ""
echo "[test] PASS=$PASS_COUNT  FAIL=$FAIL_COUNT"
if [ "$FAIL_COUNT" -gt 0 ]; then
  echo "[test] failed cases: ${FAILED_CASES[*]}"
  exit 1
fi
exit 0
