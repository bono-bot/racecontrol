#!/usr/bin/env bash
# workspace/sync/verify-parity.sh — DRAFT from Phase 393
#
# Verifies that this machine's ~/.claude/ state matches the workspace repo.
# Non-zero exit = drift detected.
# Used by CI on every wip branch push AND by SessionStart hook as a sanity check.

set -euo pipefail

WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLAUDE_HOME="${CLAUDE_HOME:-$HOME/.claude}"

FAILS=0

fail() { echo "FAIL: $1"; FAILS=$((FAILS + 1)); }
pass() { echo "ok:   $1"; }

# ── check 1: cross-platform hook syntax ─────────────────────────────
if [ -d "$WORKSPACE_ROOT/hooks/cross-platform" ]; then
  for js in "$WORKSPACE_ROOT"/hooks/cross-platform/*.js; do
    [ -e "$js" ] || continue
    if node --check "$js" 2>/dev/null; then
      pass "syntax $(basename "$js")"
    else
      fail "syntax $(basename "$js") — node --check rejected"
    fi
  done
  for sh in "$WORKSPACE_ROOT"/hooks/cross-platform/*.sh; do
    [ -e "$sh" ] || continue
    if bash -n "$sh" 2>/dev/null; then
      pass "syntax $(basename "$sh")"
    else
      fail "syntax $(basename "$sh") — bash -n rejected"
    fi
  done
fi

# ── check 2: secret scan (tracked files only) ───────────────────────
if command -v git >/dev/null 2>&1 && [ -d "$WORKSPACE_ROOT/.git" ]; then
  cd "$WORKSPACE_ROOT"
  PATTERNS='sk-[A-Za-z0-9]|Bearer [A-Za-z0-9]|password=[^ ]|secret=[^ ]|BEGIN.*PRIVATE KEY'
  HITS=$(git grep -I -E "$PATTERNS" -- ':!*.md' ':!tests/fixtures/*' 2>/dev/null || true)
  if [ -n "$HITS" ]; then
    fail "secret scan — tracked files contain secret-like strings:"
    echo "$HITS" | head -20
  else
    pass "secret scan"
  fi
fi

# ── check 3: file size (≤1000 lines hard) ───────────────────────────
OVERSIZED=$(find "$WORKSPACE_ROOT" -type f \( -name '*.md' -o -name '*.js' -o -name '*.sh' \) \
  -not -path '*/.git/*' -not -path '*/node_modules/*' \
  | while read -r f; do
    n=$(wc -l < "$f")
    if [ "$n" -gt 1000 ]; then
      echo "$f:$n"
    fi
  done)
if [ -n "$OVERSIZED" ]; then
  fail "size check — files over 1000 lines:"
  echo "$OVERSIZED"
else
  pass "size check"
fi

# ── check 4: memory orphan (every memory/*.md in INDEX.md) ──────────
if [ -d "$WORKSPACE_ROOT/memory" ] && [ -f "$WORKSPACE_ROOT/memory/INDEX.md" ]; then
  ORPHANS=$(find "$WORKSPACE_ROOT/memory" -maxdepth 1 -name '*.md' -not -name 'INDEX.md' \
    | while read -r f; do
      base=$(basename "$f")
      if ! grep -q "$base" "$WORKSPACE_ROOT/memory/INDEX.md"; then
        echo "$base"
      fi
    done)
  if [ -n "$ORPHANS" ]; then
    fail "orphan check — memory files not in INDEX.md:"
    echo "$ORPHANS"
  else
    pass "orphan check"
  fi
fi

# ── check 5: cgp-distribution-probe (when it exists in workspace) ───
if [ -f "$WORKSPACE_ROOT/scripts/cgp-distribution-probe.js" ]; then
  if node "$WORKSPACE_ROOT/scripts/cgp-distribution-probe.js" --verify 2>/dev/null; then
    pass "cgp-distribution-probe"
  else
    fail "cgp-distribution-probe — probe reported drift"
  fi
fi

echo
if [ "$FAILS" -eq 0 ]; then
  echo "verify-parity: PASS"
  exit 0
else
  echo "verify-parity: $FAILS check(s) failed"
  exit 1
fi
