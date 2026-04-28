#!/usr/bin/env bash
# scripts/graphify-helpers/debug-with-context.sh — P4.3 wiring.
#
# Drop-in "check structural context before escalating" helper for the 4-tier
# debug order (see racecontrol/CLAUDE.md > Debugging Methodology):
#   Tier 1: Deterministic  (before this helper)
#   Tier 2: Memory          (before this helper)
#   Tier 2.5: Structural    ← this helper
#   Tier 3: Local Ollama
#   Tier 4: Cloud Claude
#
# Call this helper BEFORE escalating to Ollama/Claude — it emits the
# graph.json-backed caller/callee + community-neighbour view of a symbol or
# file, so the LLM (human or model) has structural facts instead of guessing.
#
# Usage:
#   scripts/graphify-helpers/debug-with-context.sh <symbol>
#   scripts/graphify-helpers/debug-with-context.sh --file <file.rs>   # emit context for every top-level fn
#   scripts/graphify-helpers/debug-with-context.sh --neighbours 12 <symbol>
#
# Exit: 0 if context found, 2 if symbol not in graph, 3 if unified graph
# missing (run `node scripts/graphify-post/unify.mjs` first).

set -euo pipefail

NEIGHBOURS=6
MODE="symbol"
TARGET=""

while [ "${1:-}" != "" ]; do
  case "$1" in
    --file)       MODE="file"; TARGET="$2"; shift 2 ;;
    --neighbours) NEIGHBOURS="$2"; shift 2 ;;
    --help|-h)    grep '^#' "$0" | head -30; exit 0 ;;
    *)            TARGET="$1"; shift ;;
  esac
done

if [ -z "$TARGET" ]; then
  echo "usage: $0 <symbol> | --file <path>" >&2
  exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
HELPER="$REPO_ROOT/scripts/graphify-helpers/context-for-symbol.sh"
UNIFIED_GRAPH="$REPO_ROOT/graphify-out-unified/graph.json"

if [ ! -f "$UNIFIED_GRAPH" ]; then
  echo "error: $UNIFIED_GRAPH missing. Run: node scripts/graphify-post/unify.mjs" >&2
  exit 3
fi

emit_symbol_context() {
  local sym="$1"
  echo "# Structural context for \`$sym\`"
  echo ""
  bash "$HELPER" "$sym" "$NEIGHBOURS" 2>/dev/null || {
    echo "_No graphify match for \`$sym\`._"
    return 2
  }
  echo ""
}

if [ "$MODE" = "file" ]; then
  [ -f "$TARGET" ] || { echo "error: file not found: $TARGET" >&2; exit 2; }
  # Extract top-level fn/function names (Rust + JS/TS only for now)
  SYMBOLS=$(grep -oE '(fn [a-z_][a-z0-9_]*|pub fn [a-z_][a-z0-9_]*|function [a-zA-Z_][a-zA-Z0-9_]*)' "$TARGET" \
            | awk '{print $NF}' | sort -u | head -8)
  if [ -z "$SYMBOLS" ]; then
    echo "_No fn/function symbols detected in $TARGET._"
    exit 0
  fi
  echo "# Structural context for symbols in $TARGET"
  echo ""
  for sym in $SYMBOLS; do
    emit_symbol_context "$sym" || true
  done
  exit 0
fi

emit_symbol_context "$TARGET"
