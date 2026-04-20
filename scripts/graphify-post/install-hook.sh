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

if [ -f "$HOOK" ] && grep -q 'BEGIN graphify-post' "$HOOK"; then
  echo "graphify-post hook already installed at $HOOK"
  exit 0
fi

mkdir -p "$(dirname "$HOOK")"
if [ ! -f "$HOOK" ]; then
  echo '#!/usr/bin/env bash' > "$HOOK"
fi

{
  echo ""
  echo "$BLOCK_START"
  echo "$PAYLOAD"
  echo "$BLOCK_END"
} >> "$HOOK"

chmod +x "$HOOK"
echo "Installed graphify-post hook at $HOOK"
echo "Uninstall: sed -i '/BEGIN graphify-post/,/END graphify-post/d' $HOOK"
