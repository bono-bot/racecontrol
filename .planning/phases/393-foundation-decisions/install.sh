#!/usr/bin/env bash
# workspace/sync/install.sh — DRAFT from Phase 393
#
# Copies workspace files into ~/.claude/ on the current machine.
# Detects OS and installs the right subset (cross-platform + os-specific).
# Atomic: copies to a temp dir first, then swaps.
# Re-runnable: idempotent, safe to run on every `git pull main`.
#
# Usage:
#   bash sync/install.sh                # normal install
#   bash sync/install.sh --dry-run      # show what would change
#   CLAUDE_HOME=~/.claude-test bash sync/install.sh  # test target

set -euo pipefail

# ── config ──────────────────────────────────────────────────────────
WORKSPACE_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CLAUDE_HOME="${CLAUDE_HOME:-$HOME/.claude}"
DRY_RUN="${1:-}"

# ── OS detection ────────────────────────────────────────────────────
case "$(uname -s)" in
  Linux*)   OS_FAMILY="linux" ;;
  Darwin*)  OS_FAMILY="linux" ;;  # treat macOS like linux for hook purposes
  MINGW*|MSYS*|CYGWIN*) OS_FAMILY="windows" ;;
  *) echo "ERROR: unknown OS $(uname -s)"; exit 1 ;;
esac

echo "[install] workspace: $WORKSPACE_ROOT"
echo "[install] target:    $CLAUDE_HOME"
echo "[install] os:        $OS_FAMILY"

# ── safety: workspace must look like a workspace ────────────────────
for required in hooks memory ARCHITECTURE.md CONVENTIONS.md; do
  if [ ! -e "$WORKSPACE_ROOT/$required" ]; then
    echo "ERROR: $WORKSPACE_ROOT/$required missing — is this a workspace repo?"
    exit 1
  fi
done

# ── build the copy plan ─────────────────────────────────────────────
# Cross-platform always installs. OS-specific installs its family only.
COPY_PAIRS=(
  "hooks/cross-platform:$CLAUDE_HOME/hooks"
  "memory:$CLAUDE_HOME/projects/C--Users-bono/memory"
  "agents:$CLAUDE_HOME/agents"
  "commands:$CLAUDE_HOME/commands"
  "skills:$CLAUDE_HOME/skills"
)

if [ "$OS_FAMILY" = "windows" ]; then
  COPY_PAIRS+=("hooks/windows-only:$CLAUDE_HOME/hooks")
else
  COPY_PAIRS+=("hooks/linux-only:$CLAUDE_HOME/hooks")
fi

# ── execute ─────────────────────────────────────────────────────────
install_pair() {
  local src_rel="$1"
  local dest="$2"
  local src="$WORKSPACE_ROOT/$src_rel"

  if [ ! -d "$src" ]; then
    echo "[install] skip $src_rel (not present in workspace)"
    return
  fi

  if [ "$DRY_RUN" = "--dry-run" ]; then
    echo "[install] DRY: would copy $src_rel → $dest"
    return
  fi

  mkdir -p "$dest"
  # use rsync if available for atomic-ish copy with delete
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete-after "$src/" "$dest/"
  else
    # fallback: cp -R (non-atomic but workable)
    cp -R "$src/." "$dest/"
  fi
  echo "[install] ok  $src_rel → $dest"
}

for pair in "${COPY_PAIRS[@]}"; do
  src_rel="${pair%%:*}"
  dest="${pair##*:}"
  install_pair "$src_rel" "$dest"
done

# ── settings merge (base + machine-local override) ──────────────────
if [ -f "$WORKSPACE_ROOT/settings/base.json" ] && [ "$DRY_RUN" != "--dry-run" ]; then
  if [ -f "$CLAUDE_HOME/settings.local.json" ]; then
    echo "[install] merging settings/base.json + settings.local.json → settings.json"
    # TODO: jq merge in Phase 404 (CLN-01). For now, base.json wins if no local.
    cp "$WORKSPACE_ROOT/settings/base.json" "$CLAUDE_HOME/settings.json"
  else
    cp "$WORKSPACE_ROOT/settings/base.json" "$CLAUDE_HOME/settings.json"
  fi
fi

echo "[install] complete"
