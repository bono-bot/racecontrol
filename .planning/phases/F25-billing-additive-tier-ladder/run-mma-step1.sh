#!/usr/bin/env bash
set -uo pipefail

WT="/c/Users/bono/racingpoint/racecontrol-f25-billing"
KEY_FILE="/c/Users/bono/racingpoint/racecontrol/data/openrouter-mma-key.txt"
PHASE="$WT/.planning/phases/F25-billing-additive-tier-ladder"
OUT="$PHASE/diagnose"

mkdir -p "$OUT"

KEY=$(cat "$KEY_FILE")
PROMPT_BODY=$(cat "$PHASE/MMA-PROMPT.md")
PAYLOAD_BASE=$(printf '%s' "$PROMPT_BODY" | jq -Rs '{messages: [{"role":"user","content":.}], max_tokens: 6000, temperature: 0.2}')

call_model() {
  local mid="$1"
  local tag="$2"
  local payload
  payload=$(echo "$PAYLOAD_BASE" | jq --arg m "$mid" '. + {model: $m}')
  curl -s -m 240 \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "$payload" \
    https://openrouter.ai/api/v1/chat/completions \
    > "$OUT/raw-$tag.json" 2> "$OUT/err-$tag.txt"
  local rc=$?
  echo "[$tag] curl rc=$rc model=$mid"
}

echo "=== firing 5 models parallel ==="
date -u

call_model "deepseek/deepseek-r1-0528"          "r1"     &
call_model "deepseek/deepseek-chat-v3-0324"     "v3"     &
call_model "xiaomi/mimo-v2-pro"                  "mimo"   &
call_model "qwen/qwen3-235b-a22b-2507"           "qwen"   &
call_model "google/gemini-2.5-flash"             "gemini" &
wait

echo "=== all jobs returned ==="
date -u

echo "=== sizes ==="
ls -la "$OUT/"

echo "=== model + content lengths ==="
for f in "$OUT"/raw-*.json; do
  tag=$(basename "$f" .json | sed 's/^raw-//')
  model=$(jq -r '.model // .error.message // "no-model-no-error"' "$f" 2>/dev/null | head -c 80)
  err=$(jq -r '.error.message // empty' "$f" 2>/dev/null | head -c 200)
  clen=$(jq -r '.choices[0].message.content | length // 0' "$f" 2>/dev/null)
  ttok=$(jq -r '.usage.total_tokens // 0' "$f" 2>/dev/null)
  echo "[$tag] model=$model content_chars=$clen tokens=$ttok err=$err"
done
