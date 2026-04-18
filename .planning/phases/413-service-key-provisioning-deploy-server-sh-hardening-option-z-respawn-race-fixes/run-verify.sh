#!/usr/bin/env bash
# Phase 413 MMA VERIFY — 3 fresh models (different vendor families from DIAGNOSE)
# DIAGNOSE used: deepseek, xiaomi, qwen, google. VERIFY uses: moonshot, nvidia, openai.

set -u
cd "$(dirname "$0")"

export OPENROUTER_KEY="$(cat ../../../data/openrouter-mma-key.txt)"
PROMPT_FILE="verify-prompt.md"
[ -f "$PROMPT_FILE" ] || { echo "missing $PROMPT_FILE"; exit 1; }

PROMPT_JSON=$(node -e 'console.log(JSON.stringify(require("fs").readFileSync(process.argv[1],"utf8")))' "$PROMPT_FILE")

run_one() {
    local model="$1"
    local out="verify-$(echo "$model" | tr '/' '_' | tr '.' '_').json"
    local payload="{\"model\":\"$model\",\"messages\":[{\"role\":\"user\",\"content\":$PROMPT_JSON}],\"max_tokens\":4000,\"temperature\":0.3}"
    echo "[start] $model"
    local start=$(date +%s)
    curl -s -m 300 https://openrouter.ai/api/v1/chat/completions \
        -H "Authorization: Bearer $OPENROUTER_KEY" \
        -H "Content-Type: application/json" \
        -d "$payload" > "$out"
    local end=$(date +%s)
    local dur=$((end - start))
    if grep -q '"choices"' "$out" 2>/dev/null; then
        echo "[done] $model — ${dur}s — $(wc -c < "$out") bytes"
    else
        echo "[FAIL] $model — ${dur}s — $(cat "$out" | head -c 300)"
    fi
}

# Fresh vendor families: moonshot, nvidia, openai (NONE in DIAGNOSE)
for MODEL in \
    "moonshotai/kimi-k2.5" \
    "nvidia/nemotron-3-super-120b-a12b" \
    "openai/gpt-5.4-nano" ; do
    run_one "$MODEL" &
done

wait
echo "=== VERIFY complete ==="
ls -la verify-*.json
