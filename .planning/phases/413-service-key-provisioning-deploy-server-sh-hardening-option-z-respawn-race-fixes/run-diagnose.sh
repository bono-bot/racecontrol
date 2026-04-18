#!/usr/bin/env bash
# Phase 413 MMA DIAGNOSE — 5 models in parallel
# Reads prompt from audit-prompt.md, writes outputs to diagnose-<model>.json

set -u
cd "$(dirname "$0")"

export OPENROUTER_KEY="$(cat ../../../data/openrouter-mma-key.txt)"
PROMPT_FILE="audit-prompt.md"
[ -f "$PROMPT_FILE" ] || { echo "missing $PROMPT_FILE"; exit 1; }

# Build the JSON payload once (escaping the prompt file), reuse per model
PROMPT_JSON=$(node -e 'console.log(JSON.stringify(require("fs").readFileSync(process.argv[1],"utf8")))' "$PROMPT_FILE")

run_one() {
    local model="$1"
    local out="diagnose-$(echo "$model" | tr '/' '_' | tr '.' '_').json"
    local payload="{\"model\":\"$model\",\"messages\":[{\"role\":\"user\",\"content\":$PROMPT_JSON}],\"max_tokens\":8000,\"temperature\":0.3}"
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

for MODEL in \
    "deepseek/deepseek-r1-0528" \
    "deepseek/deepseek-chat-v3-0324" \
    "xiaomi/mimo-v2-pro" \
    "qwen/qwen3-235b-a22b-2507" \
    "google/gemini-2.5-flash" ; do
    run_one "$MODEL" &
done

wait
echo "=== DIAGNOSE complete ==="
ls -la diagnose-*.json
