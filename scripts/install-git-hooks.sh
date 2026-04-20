#!/usr/bin/env bash
# install-git-hooks.sh — Install tracked git hooks into .git/hooks/.
#
# Copies scripts/hooks/pre-commit into .git/hooks/pre-commit and appends the
# smart-pipes security scan call (if present) so the full pre-commit chain is:
#
#   scripts/hooks/pre-commit              # SEC-GATE + Day 3 workflow cascade
#     ↓ (if above passed)
#   scripts/smart-pipes/pre-commit-security.sh   # gitleaks / semgrep / cargo audit
#
# Idempotent: safe to re-run. Preserves whatever .git/hooks/pre-commit had
# before (backs it up to .git/hooks/pre-commit.bak-<timestamp>).
#
# Usage:
#   bash scripts/install-git-hooks.sh
#
# Also symlink-able via: git config core.hooksPath scripts/hooks/
# (but that bypasses the smart-pipes append, which is why we prefer copy.)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$REPO_ROOT/scripts/hooks/pre-commit"
DST="$REPO_ROOT/.git/hooks/pre-commit"
SMART_PIPES="$REPO_ROOT/scripts/smart-pipes/pre-commit-security.sh"

if [ ! -f "$SRC" ]; then
  echo "ERROR: tracked hook source missing: $SRC" >&2
  exit 1
fi

if [ -f "$DST" ]; then
  backup="$DST.bak-$(date +%s)"
  cp "$DST" "$backup"
  echo "backed up existing hook to $backup"
fi

cp "$SRC" "$DST"

if [ -f "$SMART_PIPES" ]; then
  cat >> "$DST" <<'APPENDIX'

# ---------------------------------------------------------------------------
# Smart Pipe: Security Scan (gitleaks, semgrep, cargo audit) — appended by
# scripts/install-git-hooks.sh. Runs only if earlier checks passed (set -e).
# ---------------------------------------------------------------------------
REPO_ROOT="$(git rev-parse --show-toplevel)"
if [ -f "$REPO_ROOT/scripts/smart-pipes/pre-commit-security.sh" ]; then
  bash "$REPO_ROOT/scripts/smart-pipes/pre-commit-security.sh"
fi
APPENDIX
  echo "appended smart-pipes security-scan call to $DST"
fi

chmod +x "$DST"
echo "installed: $DST"
echo
echo "Verify:  bash $DST"
echo "Test:    touch /tmp/hook-test && git add /tmp/hook-test && bash $DST; git rm --cached /tmp/hook-test 2>/dev/null"
