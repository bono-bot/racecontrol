#!/usr/bin/env bash
# scripts/graphify-helpers/mma-with-context.sh — Tier 4 wiring:
# feed graphify structural context into an MMA single-model query.
#
# Usage:
#   mma-with-context.sh <symbol> <model-id> <prompt-text>
#
# Example:
#   mma-with-context.sh handle_ws_message \
#     deepseek/deepseek-v3.2 \
#     "What happens when handle_ws_message receives an unexpected variant?"
#
# The prompt is prepended with a section like:
#   ## Graphify structural context for `<symbol>`
#   (JSON from context-for-symbol.sh)
#
# …before being sent to the OpenRouter model. This gives the model
# caller/callee and community-neighbour context that textual code
# search alone misses.
#
# Requires:
#   - OPENROUTER_KEY env (or saved at data/openrouter-mma-key.txt
#     per MMA bootstrap rules)
#   - graphify-out-unified/graph.json (run: node
#     scripts/graphify-post/unify.mjs)

set -euo pipefail

SYMBOL="${1:-}"
MODEL="${2:-}"
PROMPT="${3:-}"

if [ -z "$SYMBOL" ] || [ -z "$MODEL" ] || [ -z "$PROMPT" ]; then
  echo "usage: $0 <symbol> <model-id> <prompt-text>" >&2
  exit 2
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
HELPER="$REPO_ROOT/scripts/graphify-helpers/context-for-symbol.sh"

if [ ! -x "$HELPER" ]; then
  echo "error: $HELPER not executable" >&2
  exit 3
fi

# Load OpenRouter key using the canonical fallback chain
if [ -z "${OPENROUTER_KEY:-}" ]; then
  if [ -f "$REPO_ROOT/data/openrouter-mma-key.txt" ]; then
    OPENROUTER_KEY="$(tr -d '\n\r' < "$REPO_ROOT/data/openrouter-mma-key.txt")"
  else
    echo "error: OPENROUTER_KEY not set and data/openrouter-mma-key.txt missing" >&2
    exit 4
  fi
fi

# Extract graphify context for the symbol
CONTEXT="$("$HELPER" "$SYMBOL" 5 2>/dev/null || true)"

if [ -z "$CONTEXT" ] || [ "$(echo "$CONTEXT" | jq -r '.matches' 2>/dev/null)" = "0" ]; then
  # No match — note it in the prompt and continue rather than fail loudly
  CONTEXT_BLOCK="## Graphify structural context for \`$SYMBOL\`
(no matches found in unified graph — either symbol doesn't exist or graph is stale; run \`node scripts/graphify-post/unify.mjs\` to refresh)"
else
  CONTEXT_BLOCK="## Graphify structural context for \`$SYMBOL\`
\`\`\`json
$CONTEXT
\`\`\`

Use the outgoing (called-by-this-symbol) and incoming (calls-to-this-symbol) lists
to answer with awareness of actual call graph, not just textual code."
fi

FULL_PROMPT="$CONTEXT_BLOCK

---

$PROMPT"

# Build the JSON payload safely via jq stdin
PAYLOAD="$(printf '%s' "$FULL_PROMPT" | jq -Rs --arg m "$MODEL" '{
  model: $m,
  messages: [{role: "user", content: .}],
  max_tokens: 2000
}')"

curl -s -m 120 https://openrouter.ai/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENROUTER_KEY" \
  -H "Content-Type: application/json" \
  -d "$PAYLOAD"
