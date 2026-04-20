#!/usr/bin/env bash
# Install a post-commit hook that auto-runs graphify gap validation after commits.
# Appends to any existing post-commit hook rather than overwriting.
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
HOOK="$REPO_ROOT/.git/hooks/post-commit"

BLOCK_START="# BEGIN graphify-post (installed $(date -u +%Y-%m-%d))"
BLOCK_END="# END graphify-post"
PAYLOAD='if [ -f graphify-out/graph.json ]; then
  node scripts/graphify-post/validate.mjs graphify-out >/dev/null 2>&1 && \
    echo "graphify-post: GAP_REPORT.md updated ($(jq -r ".hits" graphify-out/GAP_REPORT.json 2>/dev/null || echo ?) hits)"
fi'

# Second block: auto-rebuild graphify-meta when backend graph or tooling changes.
# Session-3 addition — meta-map no longer goes stale between deliberate regens.
META_BLOCK_START="# BEGIN graphify-meta-rebuild (installed $(date -u +%Y-%m-%d))"
META_BLOCK_END="# END graphify-meta-rebuild"
META_PAYLOAD='if [ -f graphify-meta/rebuild-all.mjs ]; then
  _META_TRIGGER=""
  if [ -f graphify-out/graph.json ] && [ graphify-out/graph.json -nt graphify-meta/meta-graph.json ] 2>/dev/null; then
    _META_TRIGGER="backend-rebuilt"
  elif echo "$CHANGED" | grep -qE "^graphify-meta/.*\.mjs$|^graphify-meta/scan-module\.py$"; then
    _META_TRIGGER="tooling-changed"
  fi
  if [ -n "$_META_TRIGGER" ]; then
    (cd graphify-meta && node rebuild-all.mjs >/dev/null 2>&1) && \
      echo "graphify-meta: rebuilt ($_META_TRIGGER)" || \
      echo "graphify-meta: rebuild failed (non-blocking)"
  fi
fi'

mkdir -p "$(dirname "$HOOK")"
if [ ! -f "$HOOK" ]; then
  echo '#!/usr/bin/env bash' > "$HOOK"
fi

# Append graphify-post block if not already present
if ! grep -q 'BEGIN graphify-post' "$HOOK"; then
  {
    echo ""
    echo "$BLOCK_START"
    echo "$PAYLOAD"
    echo "$BLOCK_END"
  } >> "$HOOK"
  echo "Installed graphify-post block"
else
  echo "graphify-post block already present"
fi

# Append graphify-meta-rebuild block if not already present
if ! grep -q 'BEGIN graphify-meta-rebuild' "$HOOK"; then
  {
    echo ""
    echo "$META_BLOCK_START"
    echo "$META_PAYLOAD"
    echo "$META_BLOCK_END"
  } >> "$HOOK"
  echo "Installed graphify-meta-rebuild block"
else
  echo "graphify-meta-rebuild block already present"
fi

chmod +x "$HOOK"
echo "Hook path: $HOOK"
echo "Uninstall: sed -i '/BEGIN graphify-post/,/END graphify-post/d; /BEGIN graphify-meta-rebuild/,/END graphify-meta-rebuild/d' $HOOK"
