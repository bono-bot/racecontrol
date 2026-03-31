#!/bin/bash
# MMA Plan Review — 4 models via OpenRouter
# Usage: OPENROUTER_KEY="..." bash audit/mma-plan-review.sh

set -euo pipefail

OPENROUTER_KEY="${OPENROUTER_KEY:?Set OPENROUTER_KEY}"
PLAN_FILE=".planning/phases/LEADERBOARD-TELEMETRY-PLAN.md"
PLAN_CONTENT=$(cat "$PLAN_FILE" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | sed ':a;N;$!ba;s/\n/\\n/g')
OUTPUT_DIR="audit/results/mma-plan-review-$(date +%Y-%m-%d)"
mkdir -p "$OUTPUT_DIR"

MODELS=(
  "qwen/qwen3-235b-a22b-2507"
  "deepseek/deepseek-chat-v3-0324"
  "deepseek/deepseek-r1-0528"
  "google/gemini-2.5-pro-preview-03-25"
)

SHORTS=("qwen3" "deepseek-v3" "deepseek-r1" "gemini-2.5")

SYSTEM_PROMPT="You are a senior software architect reviewing a milestone plan for a racing esports venue management system. The system: Rust/Axum server + SQLite + Next.js frontend, 8 gaming pods with rc-agent, 1 server (racecontrol), 3 leaderboard display machines on Tailscale. Each pod runs games (Assetto Corsa, F1 25, iRacing, LMU, Forza) and sends UDP telemetry to rc-agent which forwards via WebSocket to the server.\n\nREVIEW THE PLAN FOR:\n1. Architecture bugs — missing data flows, dead ends, incorrect assumptions\n2. Performance traps — SQLite bottlenecks, memory issues, disk I/O\n3. Security gaps — auth, PII leaks, injection\n4. Correctness bugs — edge cases, formula errors, race conditions\n5. Missing requirements — what a customer or operator would expect but isn't listed\n6. Deployment risks — what could go wrong in production\n7. Integration gaps — how these phases interact with existing code\n\nReturn ONLY a JSON array of findings. Each finding: {\"id\": \"F-XX\", \"severity\": \"P1|P2|P3\", \"category\": \"architecture|performance|security|correctness|missing|deployment|integration\", \"phase\": \"251|252|253|254|255|general\", \"description\": \"...\", \"recommendation\": \"...\"}\n\nBe specific. Reference actual technical details. P1 = will cause data loss, security breach, or system failure. P2 = will cause degraded experience or operational burden. P3 = improvement opportunity."

for i in "${!MODELS[@]}"; do
  MODEL="${MODELS[$i]}"
  SHORT="${SHORTS[$i]}"
  OUTPUT_FILE="$OUTPUT_DIR/${SHORT}-findings.json"

  echo ">>> Sending to $SHORT ($MODEL)..."

  # Build request body
  REQUEST=$(cat <<ENDJSON
{
  "model": "$MODEL",
  "max_tokens": 16000,
  "temperature": 0.3,
  "messages": [
    {"role": "system", "content": "$SYSTEM_PROMPT"},
    {"role": "user", "content": "Review this plan:\\n\\n$PLAN_CONTENT"}
  ]
}
ENDJSON
)

  # Send request
  curl -s -X POST "https://openrouter.ai/api/v1/chat/completions" \
    -H "Authorization: Bearer $OPENROUTER_KEY" \
    -H "Content-Type: application/json" \
    -H "HTTP-Referer: https://racingpoint.cloud" \
    -d "$REQUEST" \
    -o "$OUTPUT_DIR/${SHORT}-raw.json" &

  echo ">>> $SHORT launched in background (PID $!)"
done

echo ">>> Waiting for all 4 models to complete..."
wait
echo ">>> All 4 models done. Extracting findings..."

for SHORT in "${SHORTS[@]}"; do
  RAW="$OUTPUT_DIR/${SHORT}-raw.json"
  if [ -f "$RAW" ]; then
    # Extract content from OpenRouter response
    node -e "
      const r = JSON.parse(require('fs').readFileSync('$RAW','utf8'));
      const content = r.choices?.[0]?.message?.content || 'ERROR: No content';
      console.log(content);
    " > "$OUTPUT_DIR/${SHORT}-findings.md" 2>/dev/null || echo "ERROR parsing $SHORT" > "$OUTPUT_DIR/${SHORT}-findings.md"
    echo ">>> $SHORT: saved to $OUTPUT_DIR/${SHORT}-findings.md"
  else
    echo ">>> $SHORT: MISSING raw response"
  fi
done

echo ""
echo "=== MMA Plan Review Complete ==="
echo "Results in: $OUTPUT_DIR/"
echo "Next: Review findings, create cross-model consensus, iterate plan"
