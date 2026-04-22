#!/usr/bin/env bash
# One-shot MMA DIAGNOSE runner for Phase 19 (Plans 01 + 04).
# Reads key from data/openrouter-mma-key.txt (never echoes it).
# Calls 5 models in parallel, writes JSON findings to mma-diagnose-output/.
set -u

REPO="/c/Users/bono/racingpoint/racecontrol"
KEY_FILE="$REPO/data/openrouter-mma-key.txt"
OUT_DIR="$REPO/.planning/phases/19-shared-context-and-history/mma-diagnose-output"
PROMPT_FILE="$OUT_DIR/prompt.txt"

[ -s "$KEY_FILE" ] || { echo "Missing key file: $KEY_FILE"; exit 1; }
[ -s "$PROMPT_FILE" ] || { echo "Missing prompt: $PROMPT_FILE"; exit 1; }

KEY=$(cat "$KEY_FILE")

call_model() {
  local model="$1"
  local short="$2"
  local out="$OUT_DIR/${short}.json"
  local err="$OUT_DIR/${short}.err"

  # Build JSON body via jq (safe escaping)
  local body
  body=$(jq -Rs --arg m "$model" '{
    model: $m,
    messages: [{role:"user", content: .}],
    max_tokens: 4000,
    temperature: 0.3
  }' < "$PROMPT_FILE")

  echo "[${short}] starting..."
  local start_ms
  start_ms=$(date +%s%3N 2>/dev/null || date +%s)
  curl -s -m 180 \
    -H "Authorization: Bearer $KEY" \
    -H "Content-Type: application/json" \
    -d "$body" \
    "https://openrouter.ai/api/v1/chat/completions" \
    > "$out" 2> "$err"
  local rc=$?
  local end_ms
  end_ms=$(date +%s%3N 2>/dev/null || date +%s)
  local elapsed=$(( end_ms - start_ms ))
  echo "[${short}] done rc=$rc elapsed=${elapsed}ms bytes=$(wc -c < "$out")"
}

# 5 models — ≥3 vendor families, max 2 per vendor, ≥1 reasoner + ≥1 code + ≥1 SRE
call_model "deepseek/deepseek-r1-0528"         "deepseek-r1"    &
call_model "deepseek/deepseek-chat-v3-0324"    "deepseek-v3"    &
call_model "x-ai/grok-code-fast-1"             "grok-code"      &
call_model "xiaomi/mimo-v2-pro"                "mimo-v2-pro"    &
call_model "google/gemini-2.5-flash"           "gemini-flash"   &

wait
echo "=== All model calls returned ==="
for f in deepseek-r1 deepseek-v3 grok-code mimo-v2-pro gemini-flash; do
  out="$OUT_DIR/${f}.json"
  err="$OUT_DIR/${f}.err"
  if [ -s "$out" ]; then
    # Extract whether we got an error object or a choices array
    has_choices=$(jq -r 'has("choices")' < "$out" 2>/dev/null || echo "parse-fail")
    if [ "$has_choices" = "true" ]; then
      printf "%-15s OK  content=%s bytes\n" "$f" "$(jq -r '.choices[0].message.content | length' < "$out")"
    else
      err_msg=$(jq -r '.error.message // "unknown"' < "$out" 2>/dev/null)
      printf "%-15s ERR %s\n" "$f" "$err_msg"
    fi
  else
    printf "%-15s NO-OUTPUT (err=%s)\n" "$f" "$([ -s "$err" ] && head -1 "$err" || echo empty)"
  fi
done
