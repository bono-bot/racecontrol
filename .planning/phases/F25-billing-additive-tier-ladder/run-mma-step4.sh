#!/usr/bin/env bash
set -uo pipefail

WT="/c/Users/bono/racingpoint/racecontrol-f25-billing"
KEY_FILE="/c/Users/bono/racingpoint/racecontrol/data/openrouter-mma-key.txt"
PHASE="$WT/.planning/phases/F25-billing-additive-tier-ladder"
OUT="$PHASE/verify"

mkdir -p "$OUT"

KEY=$(cat "$KEY_FILE")
PROMPT_BODY=$(cat "$PHASE/MMA-VERIFY-FULL.md")
# Build payloads as files (cmdline-arg limit is ~32KB on Windows; payload is ~37KB).
PAYLOAD_BASE=$(printf '%s' "$PROMPT_BODY" | jq -Rs '{messages: [{"role":"user","content":.}], max_tokens: 6000, temperature: 0.2}')

echo "$PAYLOAD_BASE" | jq --arg m "mistralai/mistral-medium-3.1" '. + {model: $m}' > "$OUT/payload-mistral.json"
echo "$PAYLOAD_BASE" | jq --arg m "x-ai/grok-code-fast-1"       '. + {model: $m}' > "$OUT/payload-grok.json"
echo "$PAYLOAD_BASE" | jq --arg m "nvidia/nemotron-nano-9b-v2"  '. + {model: $m}' > "$OUT/payload-nemo.json"

echo "=== payload sizes ==="
ls -la "$OUT/payload-"*.json

echo "=== firing 3 verifier models in parallel ==="
date -u

(
  curl -s -m 240 \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d @"$OUT/payload-mistral.json" \
    https://openrouter.ai/api/v1/chat/completions \
    > "$OUT/raw-mistral.json" 2> "$OUT/err-mistral.txt"
  echo "[mistral] rc=$?"
) &

(
  curl -s -m 240 \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d @"$OUT/payload-grok.json" \
    https://openrouter.ai/api/v1/chat/completions \
    > "$OUT/raw-grok.json" 2> "$OUT/err-grok.txt"
  echo "[grok] rc=$?"
) &

(
  curl -s -m 240 \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d @"$OUT/payload-nemo.json" \
    https://openrouter.ai/api/v1/chat/completions \
    > "$OUT/raw-nemo.json" 2> "$OUT/err-nemo.txt"
  echo "[nemo] rc=$?"
) &

wait
echo "=== all 3 returned ==="
date -u

echo "=== response sizes ==="
ls -la "$OUT/raw-"*.json

echo "=== model + content + reasoning lengths + finish_reason ==="
for f in "$OUT"/raw-*.json; do
  tag=$(basename "$f" .json | sed 's/^raw-//')
  model=$(jq -r '.model // .error.message // "no-model"' "$f" 2>/dev/null | head -c 80)
  err=$(jq -r '.error.message // empty' "$f" 2>/dev/null | head -c 200)
  fin=$(jq -r '.choices[0].finish_reason // "no-finish"' "$f" 2>/dev/null)
  clen=$(jq -r '.choices[0].message.content | length // 0' "$f" 2>/dev/null)
  rlen=$(jq -r '.choices[0].message.reasoning | length // 0' "$f" 2>/dev/null)
  echo "[$tag] model=$model finish=$fin content_chars=$clen reasoning_chars=$rlen err=$err"
done
