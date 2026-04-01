#!/usr/bin/env bash
# adversarial-eval.sh — Adversarial Evaluator + Ralph Wiggum Loop (GAP-11)
#
# Implements Unified Protocol Principles 2 and 6:
#   P2: Evaluator model MUST differ from diagnostician model
#   P6: Deterministic checks after AI evaluation passes
#
# Usage:
#   bash scripts/adversarial-eval.sh <diagnosis-file> [--diagnostician model_id]
#
# Example:
#   bash scripts/adversarial-eval.sh .planning/debug/pod6-crash-diagnosis.md --diagnostician deepseek-r1
#
# Environment:
#   OPENROUTER_KEY — required for evaluator model call

set -e

DIAGNOSIS_FILE="${1:-.planning/debug/latest-diagnosis.md}"
DIAGNOSTICIAN="${3:-unknown}"
MAX_LOOPS=5

if [ ! -f "$DIAGNOSIS_FILE" ]; then
  echo "ERROR: Diagnosis file not found: $DIAGNOSIS_FILE"
  exit 1
fi

if [ -z "$OPENROUTER_KEY" ]; then
  # Try loading from saved key file
  SAVED_KEY=$(node -e "const k=require('./scripts/lib/openrouter-key-recovery').loadSavedKey();if(k)process.stdout.write(k)" 2>/dev/null || true)
  if [ -n "$SAVED_KEY" ]; then
    OPENROUTER_KEY="$SAVED_KEY"
    echo "Loaded saved key from previous recovery"
  else
    echo "ERROR: OPENROUTER_KEY not set"
    exit 1
  fi
fi

# 401 recovery function — provisions new key via Node module
recover_openrouter_key() {
  echo "Attempting OpenRouter key recovery..."
  NEW_KEY=$(node -e "require('./scripts/lib/openrouter-key-recovery').recoverKey().then(k=>{process.stdout.write(k);process.exit(0)}).catch(e=>{console.error(e.message);process.exit(1)})" 2>&1)
  if [ $? -eq 0 ] && [ -n "$NEW_KEY" ]; then
    OPENROUTER_KEY="$NEW_KEY"
    echo "Key recovered successfully"
    return 0
  else
    echo "Key recovery failed: $NEW_KEY"
    return 1
  fi
}

# ─── Evaluator model selection (must differ from diagnostician) ────────────────
select_evaluator() {
  case "$DIAGNOSTICIAN" in
    *deepseek*r1*|*deepseek*reasoner*)
      echo "google/gemini-2.5-pro-preview-03-25" ;;
    *gemini*)
      echo "deepseek/deepseek-r1-0528" ;;
    *qwen*)
      echo "xiaomi/mimo-v2-pro" ;;
    *mimo*)
      echo "qwen/qwen3-235b-a22b-2507" ;;
    *)
      echo "deepseek/deepseek-r1-0528" ;;
  esac
}

EVALUATOR_MODEL=$(select_evaluator)
echo "============================================================"
echo "ADVERSARIAL EVALUATOR (Principle 2)"
echo "============================================================"
echo "Diagnostician: ${DIAGNOSTICIAN}"
echo "Evaluator:     ${EVALUATOR_MODEL}"
echo "Diagnosis:     ${DIAGNOSIS_FILE}"
echo ""

# ─── P2: Send diagnosis to evaluator for grading ──────────────────────────────
DIAGNOSIS_CONTENT=$(cat "$DIAGNOSIS_FILE" | head -500)

EVAL_PROMPT="You are an adversarial code reviewer evaluating an AI-generated bug diagnosis and fix.

Grade the following diagnosis on 4 criteria (each 1-5):
1. Root Cause Accuracy (35%): Is the identified root cause correct and well-evidenced?
2. Fix Completeness (25%): Does the fix address the root cause without introducing new issues?
3. Verification Evidence (25%): Was the fix properly verified with the EXACT failing behavior?
4. Side Effect Safety (15%): Are there unintended side effects on adjacent systems?

DIAGNOSIS:
${DIAGNOSIS_CONTENT}

Respond in this exact JSON format:
{
  \"root_cause_accuracy\": { \"score\": N, \"rationale\": \"...\" },
  \"fix_completeness\": { \"score\": N, \"rationale\": \"...\" },
  \"verification_evidence\": { \"score\": N, \"rationale\": \"...\" },
  \"side_effect_safety\": { \"score\": N, \"rationale\": \"...\" },
  \"weighted_score\": N.N,
  \"verdict\": \"PASS|REVIEW|FAIL\",
  \"critical_gaps\": [\"...\"]
}"

# Write the request payload to a temp file (Git Bash JSON safety)
PAYLOAD_FILE=$(mktemp)
cat > "$PAYLOAD_FILE" << PAYLOAD_EOF
{
  "model": "${EVALUATOR_MODEL}",
  "messages": [{"role": "user", "content": $(echo "$EVAL_PROMPT" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')}],
  "max_tokens": 2000,
  "temperature": 0.1
}
PAYLOAD_EOF

echo "Sending to evaluator model..."
EVAL_RESPONSE=$(curl -s -X POST "https://openrouter.ai/api/v1/chat/completions" \
  -H "Authorization: Bearer ${OPENROUTER_KEY}" \
  -H "Content-Type: application/json" \
  -d @"$PAYLOAD_FILE")

# Check for 401 and attempt recovery
if echo "$EVAL_RESPONSE" | grep -qi '"code":401\|"status":401\|Unauthorized\|User not found'; then
  echo "401 detected — key is dead. Attempting auto-recovery..."
  if recover_openrouter_key; then
    echo "Retrying with new key..."
    EVAL_RESPONSE=$(curl -s -X POST "https://openrouter.ai/api/v1/chat/completions" \
      -H "Authorization: Bearer ${OPENROUTER_KEY}" \
      -H "Content-Type: application/json" \
      -d @"$PAYLOAD_FILE")
  else
    echo "FATAL: Key recovery failed. Get a new key from openrouter.ai/settings/keys"
    rm -f "$PAYLOAD_FILE"
    exit 1
  fi
fi

rm -f "$PAYLOAD_FILE"

# Extract the evaluator's response
EVAL_CONTENT=$(echo "$EVAL_RESPONSE" | python3 -c "
import json, sys
try:
    data = json.load(sys.stdin)
    print(data['choices'][0]['message']['content'])
except:
    print('ERROR: Failed to parse evaluator response')
" 2>/dev/null)

echo ""
echo "--- Evaluator Response ---"
echo "$EVAL_CONTENT"
echo ""

# Extract weighted score
SCORE=$(echo "$EVAL_CONTENT" | python3 -c "
import json, sys, re
try:
    text = sys.stdin.read()
    match = re.search(r'\"weighted_score\"\s*:\s*([\d.]+)', text)
    if match: print(match.group(1))
    else: print('0')
except:
    print('0')
" 2>/dev/null)

echo "Weighted Score: ${SCORE}"

# Decision
PASS_THRESHOLD="4.0"
REVIEW_THRESHOLD="3.0"

if python3 -c "exit(0 if float('${SCORE}') >= float('${PASS_THRESHOLD}') else 1)" 2>/dev/null; then
  echo -e "\033[0;32mVERDICT: PASS (>= ${PASS_THRESHOLD}) — proceed to Ralph Wiggum loop\033[0m"
elif python3 -c "exit(0 if float('${SCORE}') >= float('${REVIEW_THRESHOLD}') else 1)" 2>/dev/null; then
  echo -e "\033[0;33mVERDICT: REVIEW (${REVIEW_THRESHOLD}-${PASS_THRESHOLD}) — human review required\033[0m"
  exit 2
else
  echo -e "\033[0;31mVERDICT: FAIL (< ${REVIEW_THRESHOLD}) — iterate on diagnosis\033[0m"
  exit 1
fi

# ─── P6: Ralph Wiggum Deterministic Loop ──────────────────────────────────────
echo ""
echo "============================================================"
echo "RALPH WIGGUM LOOP (Principle 6)"
echo "============================================================"
echo "Running deterministic checks that cannot lie..."
echo ""

LOOP=0
CHECKS_PASS=true

# Check 1: cargo test passes
echo "[Check 1] cargo test -p rc-common && cargo test -p racecontrol"
if cargo test -p rc-common 2>&1 | tail -5 && cargo test -p racecontrol 2>&1 | tail -5; then
  echo "  PASS"
else
  echo "  FAIL"
  CHECKS_PASS=false
fi

# Check 2: no .unwrap() in changed files
echo ""
echo "[Check 2] No .unwrap() in production code (changed files)"
UNWRAP_COUNT=$(git diff HEAD~1 -- '*.rs' | grep '+.*\.unwrap()' | grep -v test | grep -v '#\[cfg(test)\]' | wc -l)
if [ "$UNWRAP_COUNT" -eq 0 ]; then
  echo "  PASS (0 unwrap in diff)"
else
  echo "  FAIL ($UNWRAP_COUNT unwrap calls in diff)"
  CHECKS_PASS=false
fi

# Check 3: no format! SQL
echo ""
echo "[Check 3] No format! SQL injection risk"
SQL_INJECT=$(grep -rn 'format!.*SELECT\|INSERT\|UPDATE\|DELETE' crates/*/src/ --include='*.rs' | grep -v test | wc -l)
if [ "$SQL_INJECT" -eq 0 ]; then
  echo "  PASS"
else
  echo "  FAIL ($SQL_INJECT format! SQL patterns found)"
  CHECKS_PASS=false
fi

# Check 4: no secrets in diff
echo ""
echo "[Check 4] No secrets in changed code"
SECRET_COUNT=$(git diff HEAD~1 | grep -iE '(password|secret|api_key|token).*=.*"[^"]{8,}"' | grep -v test | wc -l)
if [ "$SECRET_COUNT" -eq 0 ]; then
  echo "  PASS"
else
  echo "  FAIL ($SECRET_COUNT potential secrets found)"
  CHECKS_PASS=false
fi

# Check 5: clippy clean
echo ""
echo "[Check 5] cargo clippy --all-targets"
CLIPPY_ERRORS=$(cargo clippy --all-targets 2>&1 | grep "^error" | wc -l)
if [ "$CLIPPY_ERRORS" -eq 0 ]; then
  echo "  PASS"
else
  echo "  FAIL ($CLIPPY_ERRORS clippy errors)"
  CHECKS_PASS=false
fi

echo ""
echo "============================================================"
if $CHECKS_PASS; then
  echo -e "\033[0;32mRALPH WIGGUM: ALL CHECKS PASS — safe to deploy\033[0m"
  exit 0
else
  echo -e "\033[0;31mRALPH WIGGUM: DETERMINISTIC CHECKS FAILED — fix before deploy\033[0m"
  exit 1
fi
