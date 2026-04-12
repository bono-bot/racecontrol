#!/usr/bin/env bash
# fix-check-ratio.sh — Pre-commit helper: warns when fix commits add too much code
# Usage: bash scripts/fix-check-ratio.sh
# Exit 0 always (warning only, never blocks)
set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Check for staged changes
DIFF_STAT=$(git -C "$REPO_ROOT" diff --cached --stat 2>/dev/null)
if [ -z "$DIFF_STAT" ]; then
  echo "fix-check-ratio: No staged changes."
  exit 0
fi

# Get insertions and deletions
INSERTIONS=$(git -C "$REPO_ROOT" diff --cached --numstat 2>/dev/null | \
  awk '{ ins += $1 } END { print ins+0 }')
DELETIONS=$(git -C "$REPO_ROOT" diff --cached --numstat 2>/dev/null | \
  awk '{ del += $2 } END { print del+0 }')

# Get the last commit message for context (if amending, check current staged msg)
LAST_MSG=$(git -C "$REPO_ROOT" log -1 --format='%s' 2>/dev/null || echo "")

echo "=========================================="
echo "FIX-CHECK-RATIO ANALYSIS"
echo "=========================================="
echo ""
echo "Staged changes:"
echo "  Insertions: +$INSERTIONS"
echo "  Deletions:  -$DELETIONS"
echo ""

# Avoid division by zero
if [ "$DELETIONS" -eq 0 ]; then
  if [ "$INSERTIONS" -gt 20 ]; then
    RATIO="INF"
    echo "  Ratio: $INSERTIONS:0 (all additions, no removals)"
  else
    echo "  Ratio: $INSERTIONS:0 (small addition, OK)"
    exit 0
  fi
else
  # Integer ratio (bash can't do float)
  RATIO=$(( (INSERTIONS * 100) / DELETIONS ))
  RATIO_DISPLAY="$((INSERTIONS / DELETIONS)):1"
  echo "  Ratio: ~${RATIO_DISPLAY} (${INSERTIONS}:${DELETIONS})"
fi

echo ""

# Check if this looks like a fix commit
IS_FIX=0
# Check staged commit message (if using -m flag, check args)
# Also check branch name and last commit for fix patterns
BRANCH=$(git -C "$REPO_ROOT" branch --show-current 2>/dev/null || echo "")
case "$BRANCH" in
  *fix*|*hotfix*|*bugfix*|*patch*) IS_FIX=1 ;;
esac
case "$LAST_MSG" in
  fix*|Fix*|FIX*|*bugfix*|*hotfix*) IS_FIX=1 ;;
esac

# List changed files by type
echo "Changed files:"
RUST_FILES=0
TS_FILES=0
OTHER_FILES=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  case "$line" in
    *.rs)        RUST_FILES=$((RUST_FILES + 1)) ;;
    *.ts|*.tsx)  TS_FILES=$((TS_FILES + 1)) ;;
    *)           OTHER_FILES=$((OTHER_FILES + 1)) ;;
  esac
  echo "  $line"
done < <(git -C "$REPO_ROOT" diff --cached --name-only 2>/dev/null)
echo ""

# Warnings
WARN_COUNT=0

# Warning 1: High insertion ratio on a fix
if [ "$DELETIONS" -gt 0 ]; then
  if [ "$RATIO" -gt 200 ] && [ "$INSERTIONS" -gt 20 ]; then
    WARN_COUNT=$((WARN_COUNT + 1))
    echo "⚠  WARNING: This change adds ${RATIO_DISPLAY} more code than it removes."
    echo "   If this is a fix: are you fixing the root cause, or papering over it?"
    echo "   Consider: could you replace/simplify instead of adding?"
    echo ""
  fi
elif [ "$INSERTIONS" -gt 50 ]; then
  WARN_COUNT=$((WARN_COUNT + 1))
  echo "⚠  WARNING: +$INSERTIONS lines with 0 deletions."
  echo "   If this is a fix: are you adding a workaround instead of fixing the root cause?"
  echo ""
fi

# Warning 2: Too many files changed for a fix
FILE_COUNT=$(git -C "$REPO_ROOT" diff --cached --name-only 2>/dev/null | wc -l)
if [ "$FILE_COUNT" -gt 10 ]; then
  WARN_COUNT=$((WARN_COUNT + 1))
  echo "⚠  WARNING: $FILE_COUNT files changed. Fix commits should be surgical."
  echo "   Consider splitting into smaller, focused commits."
  echo ""
fi

# Warning 3: Mixing Rust and frontend in one fix
if [ "$RUST_FILES" -gt 0 ] && [ "$TS_FILES" -gt 0 ]; then
  WARN_COUNT=$((WARN_COUNT + 1))
  echo "⚠  WARNING: Mixing Rust ($RUST_FILES files) and frontend ($TS_FILES files) in one commit."
  echo "   Cross-boundary fixes have higher blast radius. Consider separate commits."
  echo ""
fi

# Warning 4: Large files with no test changes
HAS_TEST_CHANGES=0
while IFS= read -r f; do
  case "$f" in
    *test*|*spec*|*_test.*) HAS_TEST_CHANGES=1 ;;
  esac
done < <(git -C "$REPO_ROOT" diff --cached --name-only 2>/dev/null)

if [ "$INSERTIONS" -gt 30 ] && [ "$HAS_TEST_CHANGES" -eq 0 ]; then
  WARN_COUNT=$((WARN_COUNT + 1))
  echo "⚠  WARNING: +$INSERTIONS lines but no test file changes."
  echo "   Run: bash scripts/fix-scope.sh <file> <function> to check test coverage."
  echo ""
fi

# Summary
if [ "$WARN_COUNT" -eq 0 ]; then
  echo "✓  No ratio concerns. Looks reasonable."
else
  echo "──────────────────────────────────────────"
  echo "$WARN_COUNT warning(s). Review before committing."
  echo "Tip: bash scripts/fix-scope.sh <file> <func> for blast radius analysis."
fi

# Always exit 0 — warning only, never block
exit 0
