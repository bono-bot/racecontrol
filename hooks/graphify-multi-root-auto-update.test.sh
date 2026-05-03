#!/usr/bin/env bash
# graphify-multi-root-auto-update.test.sh
# Standalone harness for the multi-root auto-update hook (PACT-20260503-006).
# Builds isolated temp git repos to exercise each branch deterministically.
# Uses GRAPHIFY_MULTI_DRY_RUN=1 to skip the actual `graphify` CLI spawn.

set -u

HOOK="$(dirname "$0")/graphify-multi-root-auto-update.js"
# Use a tmp dir under $HOME. On Git Bash + Windows, Node.js rejects POSIX-form
# paths like /c/Users/... but accepts mixed form C:/Users/... — convert via
# cygpath -m for any path embedded in JSON passed to Node. mkdir works either way.
TMP_BASE_POSIX="$HOME/.tmp-graphify-multi-test-$$-$RANDOM"
mkdir -p "$TMP_BASE_POSIX"
if command -v cygpath >/dev/null 2>&1; then
  TMP_BASE="$(cygpath -m "$TMP_BASE_POSIX")"
else
  TMP_BASE="$TMP_BASE_POSIX"
fi
PASS=0
FAIL=0
RESULTS=()

cleanup() { rm -rf "$TMP_BASE_POSIX" 2>/dev/null || true; }
trap cleanup EXIT

mk_repo() {
  local id="$1"
  local mode="$2"  # fresh | stale | nograph | norepo
  local repo_posix="$TMP_BASE_POSIX/$id"

  if [[ "$mode" == "norepo" ]]; then
    return 0
  fi

  mkdir -p "$repo_posix/graphify-out"
  ( cd "$repo_posix" && git init -q && git -c user.email=t@t.t -c user.name=t commit --allow-empty -q -m "init" 2>/dev/null )

  if [[ "$mode" == "nograph" ]]; then
    return 0
  fi

  echo '{"nodes":[],"edges":[]}' > "$repo_posix/graphify-out/graph.json"

  if [[ "$mode" == "fresh" ]]; then
    touch "$repo_posix/graphify-out/graph.json"
  elif [[ "$mode" == "stale" ]]; then
    local past
    past="$(date -d '1 hour ago' '+%Y%m%d%H%M.%S' 2>/dev/null || date -v-1H '+%Y%m%d%H%M.%S')"
    touch -t "$past" "$repo_posix/graphify-out/graph.json"
  fi
}

assert() {
  local name="$1" expected_pattern="$2" actual="$3"
  if grep -qE "$expected_pattern" <<< "$actual"; then
    echo "  PASS: $name"
    PASS=$((PASS+1))
    RESULTS+=("PASS  $name")
  else
    echo "  FAIL: $name"
    echo "    expected pattern: $expected_pattern"
    echo "    actual: $actual"
    FAIL=$((FAIL+1))
    RESULTS+=("FAIL  $name")
  fi
}

# ---------------------------------------------------------------------
# Test 1 — root not found → "skipped (root not found)"
echo "[test 1] root path missing"
ROOTS_JSON='[{"id":"missing","path":"'"$TMP_BASE"'/does-not-exist"}]'
OUT="$(GRAPHIFY_MULTI_ROOTS_OVERRIDE="$ROOTS_JSON" GRAPHIFY_MULTI_DRY_RUN=1 node "$HOOK" 2>&1)"
assert "missing root → skipped" "missing: skipped \(root not found\)" "$OUT"

# ---------------------------------------------------------------------
# Test 2 — graph.json missing → "skipped (no graph.json …)"
echo "[test 2] graph.json missing"
mk_repo "nograph" "nograph"
ROOTS_JSON='[{"id":"nograph","path":"'"$TMP_BASE"'/nograph"}]'
OUT="$(GRAPHIFY_MULTI_ROOTS_OVERRIDE="$ROOTS_JSON" GRAPHIFY_MULTI_DRY_RUN=1 node "$HOOK" 2>&1)"
assert "no graph.json → skipped" "nograph: skipped \(no graph\.json" "$OUT"

# ---------------------------------------------------------------------
# Test 3 — graph fresh (mtime ≥ HEAD) → "fresh"
echo "[test 3] graph fresher than HEAD"
mk_repo "fresh" "fresh"
ROOTS_JSON='[{"id":"fresh","path":"'"$TMP_BASE"'/fresh"}]'
OUT="$(GRAPHIFY_MULTI_ROOTS_OVERRIDE="$ROOTS_JSON" GRAPHIFY_MULTI_DRY_RUN=1 node "$HOOK" 2>&1)"
assert "graph fresh → fresh" "fresh: fresh \(graph\.json" "$OUT"

# ---------------------------------------------------------------------
# Test 4 — graph stale (HEAD > mtime) → DRY_RUN spawn message + lock file
echo "[test 4] graph stale → would spawn"
mk_repo "stale" "stale"
ROOTS_JSON='[{"id":"stale","path":"'"$TMP_BASE"'/stale"}]'
LOCK="$HOME/.claude/state/graphify-bg-stale.lock"
rm -f "$LOCK" 2>/dev/null
OUT="$(GRAPHIFY_MULTI_ROOTS_OVERRIDE="$ROOTS_JSON" GRAPHIFY_MULTI_DRY_RUN=1 node "$HOOK" 2>&1)"
assert "stale graph → DRY_RUN spawn message" "stale: DRY_RUN would spawn" "$OUT"
if [[ -f "$LOCK" ]]; then
  echo "  PASS: lock file written for stale root"
  PASS=$((PASS+1))
  RESULTS+=("PASS  lock file written for stale root")
else
  echo "  FAIL: lock file NOT written for stale root"
  FAIL=$((FAIL+1))
  RESULTS+=("FAIL  lock file NOT written for stale root")
fi

# ---------------------------------------------------------------------
# Test 5 — lock-fresh (<2 min) → second invocation skips
echo "[test 5] lock-fresh skip"
OUT2="$(GRAPHIFY_MULTI_ROOTS_OVERRIDE="$ROOTS_JSON" GRAPHIFY_MULTI_DRY_RUN=1 node "$HOOK" 2>&1)"
assert "second invocation under LOCK_FRESH → skipped" "stale: skipped \(fired [0-9]+s ago\)" "$OUT2"
rm -f "$LOCK" 2>/dev/null

# ---------------------------------------------------------------------
echo
echo "Results: $PASS passed / $FAIL failed"
for line in "${RESULTS[@]}"; do echo "  $line"; done

[[ $FAIL -eq 0 ]] && exit 0 || exit 1
