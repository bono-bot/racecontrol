#!/bin/bash
# scripts/install-hooks.sh — Phase 445 + future client-side git hook installer.
# One-command install for contributors. Idempotent.
#
# Usage:
#   bash scripts/install-hooks.sh            # install all hooks in scripts/git-hooks/
#   bash scripts/install-hooks.sh --dry-run  # print what would be installed (no file changes)
#
# Semantics:
#   - Source of truth: scripts/git-hooks/* (TRACKED in git)
#   - Install target:  .git/hooks/*        (LOCAL, not tracked)
#   - Non-destructive: if .git/hooks/<name> already exists AND its content
#     differs from the source, this script installs the source hook as
#     .git/hooks/<name>.445 instead of overwriting, and prints a warning
#     telling the user to integrate manually. This avoids stomping on
#     existing hooks like SEC-GATE-02 installed via scripts/install-git-hooks.sh.
#   - If --dry-run is passed, no files are written; the planned actions are
#     printed (and the word "pre-commit" appears in the output so callers can
#     verify the hook would be installed).
#
# Invariant: fresh clone + `bash scripts/install-hooks.sh` → drift guard armed
# locally (plus any other hooks added to scripts/git-hooks/ in future).

set -euo pipefail

DRY_RUN=false
if [ "${1:-}" = "--dry-run" ]; then
  DRY_RUN=true
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)"
if [ -z "$REPO_ROOT" ]; then
  echo "ERROR: not inside a git repository"
  exit 1
fi

SRC_DIR="$REPO_ROOT/scripts/git-hooks"
DST_DIR="$REPO_ROOT/.git/hooks"

if [ ! -d "$SRC_DIR" ]; then
  echo "ERROR: $SRC_DIR does not exist (tracked hook source)"
  exit 1
fi

if [ "$DRY_RUN" = "false" ]; then
  mkdir -p "$DST_DIR"
fi

INSTALLED_COUNT=0
SIDECAR_COUNT=0
SKIPPED_COUNT=0

for HOOK_SRC in "$SRC_DIR"/*; do
  [ -f "$HOOK_SRC" ] || continue
  HOOK_NAME=$(basename "$HOOK_SRC")
  HOOK_DST="$DST_DIR/$HOOK_NAME"

  if [ "$DRY_RUN" = "true" ]; then
    if [ -f "$HOOK_DST" ] && ! cmp -s "$HOOK_SRC" "$HOOK_DST"; then
      echo "[dry-run] would install sidecar: $HOOK_NAME -> $HOOK_DST.445 (existing hook differs)"
      SIDECAR_COUNT=$((SIDECAR_COUNT + 1))
    elif [ -f "$HOOK_DST" ] && cmp -s "$HOOK_SRC" "$HOOK_DST"; then
      echo "[dry-run] would skip: $HOOK_NAME (already installed, content identical)"
      SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
    else
      echo "[dry-run] would install $HOOK_NAME -> $HOOK_DST"
      INSTALLED_COUNT=$((INSTALLED_COUNT + 1))
    fi
    continue
  fi

  if [ -f "$HOOK_DST" ] && ! cmp -s "$HOOK_SRC" "$HOOK_DST"; then
    # Existing hook content differs — install as sidecar to avoid stomping
    # on hooks like SEC-GATE-02 installed by scripts/install-git-hooks.sh.
    SIDECAR="$HOOK_DST.445"
    cp "$HOOK_SRC" "$SIDECAR"
    chmod +x "$SIDECAR"
    echo "WARN  $HOOK_NAME: existing hook differs; installed as sidecar $SIDECAR"
    SIDECAR_COUNT=$((SIDECAR_COUNT + 1))

    # Auto-chain: inject sidecar invocation into the existing hook so the
    # sidecar actually runs. Idempotent via the SIDECAR-CHAIN-445 sentinel.
    # Why: requiring contributors to manually patch .git/hooks/<name> after
    # every install caused the Phase 445 drift guard + classifier-test gate
    # to silently no-op for everyone except whoever wired it once. Mirrors
    # the pattern graphify uses for post-checkout (.git/hooks/post-checkout
    # auto-invokes its sidecar via a top-of-file `if [ -x ... ]` block).
    if ! grep -q "SIDECAR-CHAIN-445" "$HOOK_DST" 2>/dev/null; then
      # Pre-commit + pre-push: sidecar failure must BLOCK (exit $?)
      # post-* hooks: sidecar failure must NOT block (|| true)
      case "$HOOK_NAME" in
        pre-*) FAIL_MODE='|| exit $?' ;;
        *)     FAIL_MODE='|| true' ;;
      esac
      TMP=$(mktemp)
      # Preserve shebang (line 1) at top
      head -1 "$HOOK_DST" > "$TMP"
      cat >> "$TMP" <<EOF
# SIDECAR-CHAIN-445 (auto-injected by scripts/install-hooks.sh — do not remove)
# Chains the version-controlled sidecar so .git/hooks/$HOOK_NAME and
# scripts/git-hooks/$HOOK_NAME stay in sync. Sidecar source-of-truth lives
# under scripts/git-hooks/. To remove this block, also remove the sidecar.
if [ -x "\$(git rev-parse --git-dir)/hooks/$HOOK_NAME.445" ]; then
  bash "\$(git rev-parse --git-dir)/hooks/$HOOK_NAME.445" "\$@" $FAIL_MODE
fi
EOF
      tail -n +2 "$HOOK_DST" >> "$TMP"
      mv "$TMP" "$HOOK_DST"
      chmod +x "$HOOK_DST"
      echo "      auto-chained: $HOOK_DST now invokes $SIDECAR ($FAIL_MODE)"
    else
      echo "      sidecar chain already present in $HOOK_DST (sentinel found)"
    fi
  elif [ -f "$HOOK_DST" ] && cmp -s "$HOOK_SRC" "$HOOK_DST"; then
    echo "OK    $HOOK_NAME already installed (identical content)"
    SKIPPED_COUNT=$((SKIPPED_COUNT + 1))
  else
    cp "$HOOK_SRC" "$HOOK_DST"
    chmod +x "$HOOK_DST"
    echo "OK    installed $HOOK_NAME"
    INSTALLED_COUNT=$((INSTALLED_COUNT + 1))
  fi
done

if [ "$DRY_RUN" = "true" ]; then
  echo ""
  echo "[dry-run] summary: install=$INSTALLED_COUNT sidecar=$SIDECAR_COUNT skip=$SKIPPED_COUNT"
  echo "[dry-run] complete — no files changed"
else
  echo ""
  echo "hooks installed to $DST_DIR"
  echo "summary: installed=$INSTALLED_COUNT sidecar=$SIDECAR_COUNT skipped=$SKIPPED_COUNT"
  if [ $SIDECAR_COUNT -gt 0 ]; then
    echo ""
    echo "NOTE: $SIDECAR_COUNT hook(s) installed as *.445 sidecars."
    echo "      The chain block was auto-injected into the existing hook on first"
    echo "      install (idempotent via SIDECAR-CHAIN-445 sentinel)."
  fi
fi
