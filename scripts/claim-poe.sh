#!/usr/bin/env bash
# claim-poe.sh — PoE Gate 3: regression-hypothesis enforcement.
#
# Plan: plan_deploy_poe_gates_20260423.md §Wave 4.
# Waves 1+2 shipped in main; Wave 3 (PR template) shipped via PR #23.
# This script is Wave 4. Wave 5 (self-updating defaults) + Wave 6 (CI) are follow-ups.
#
# Purpose: before a claim containing a completion trigger word
# (done|fixed|deployed|PASS|complete|ready|ships) can stand, the claimant must
# have enumerated ≥3 regression hypotheses AND paired each with evidence.
#
# Composes with CGP H3 — does not replace it. H3 requires:
#   - named behavior tested + raw output + where + not-tested list
# Gate 3 adds: "what else could have broken, and did you check?" — a forward-
# looking enumeration missing from the backward-looking H3 block.
#
# Hook integration (Wave 4 deferred to follow-up):
#   ~/.claude/hooks/cgp-enforce.js invokes this from the H3 path on trigger-word
#   detect. Wave 4 ships the standalone script + a self-test mode. Hook wiring
#   needs behavioural review (hook timeouts; escape-hatch for ack-only claims
#   like "ack, will investigate"; user can always --allow-bypass).
#
# Usage:
#   bash scripts/claim-poe.sh [--text "claim body"] [--file path.md]
#   cat claim.md | bash scripts/claim-poe.sh
#   bash scripts/claim-poe.sh --self-test   # run all fixture cases, no I/O
#
# Flags:
#   --text TEXT            claim body inline (otherwise reads stdin)
#   --file PATH            read claim body from file
#   --min-hypotheses N     minimum required (default 3)
#   --min-evidence-chars N minimum chars per evidence (default 20 — prevents "ok")
#   --trigger-regex REGEX  override trigger-word set
#   --no-log               skip writing .claim-poe/log.jsonl
#   --self-test            run built-in fixtures (no stdin read)
#   --no-color             disable ANSI color
#   -h|--help              print usage
#
# Exit codes:
#   0  claim passes (either no trigger word OR trigger word + full evidence)
#   1  claim blocks (trigger word present + missing enumeration or evidence)
#   2  usage / IO error

set -euo pipefail

CLAIM_TEXT=""
CLAIM_FILE=""
MIN_HYPS=3
MIN_EVIDENCE_CHARS=20
TRIGGER_REGEX='\b(done|fixed|deployed|pass|complete|ready|ships)\b'
WRITE_LOG=1
SELF_TEST=0
USE_COLOR=1

print_usage() { sed -n '3,47p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --text) CLAIM_TEXT="$2"; shift 2 ;;
    --file) CLAIM_FILE="$2"; shift 2 ;;
    --min-hypotheses) MIN_HYPS="$2"; shift 2 ;;
    --min-evidence-chars) MIN_EVIDENCE_CHARS="$2"; shift 2 ;;
    --trigger-regex) TRIGGER_REGEX="$2"; shift 2 ;;
    --no-log) WRITE_LOG=0; shift ;;
    --self-test) SELF_TEST=1; shift ;;
    --no-color) USE_COLOR=0; shift ;;
    -h|--help) print_usage; exit 0 ;;
    *) echo "unknown arg: $1" >&2; exit 2 ;;
  esac
done

if [[ "$USE_COLOR" == 1 ]]; then
  C_HEAD=$'\033[1;36m'; C_WARN=$'\033[1;33m'; C_FAIL=$'\033[1;31m'; C_OK=$'\033[1;32m'; C_DIM=$'\033[2m'; C_END=$'\033[0m'
else
  C_HEAD=""; C_WARN=""; C_FAIL=""; C_OK=""; C_DIM=""; C_END=""
fi

# ─── Core evaluator ────────────────────────────────────────────────────────
# evaluate_claim <claim_text> → exit code 0 (pass) or 1 (block); prints verdict
#
# Rules:
# 1. If trigger regex does NOT match → PASS (no enforcement needed)
# 2. Otherwise, the claim MUST contain a `## Regression hypotheses` section.
# 3. Under that heading, at least MIN_HYPS bullet lines (`- ` or `* `) must appear.
# 4. Each bullet MUST be followed on the same or a later line (before the next
#    bullet/heading) by `evidence:` followed by ≥ MIN_EVIDENCE_CHARS non-space chars.
# Ordering: a PASS on #1 returns 0 with exit-code-2 semantics — no trigger word
# means no enforcement; we PASS the claim through.
evaluate_claim() {
  local text="$1"
  local lower
  lower="$(printf '%s' "$text" | tr '[:upper:]' '[:lower:]')"

  # 1. Trigger-word check.
  if ! echo "$lower" | grep -Eq "$TRIGGER_REGEX"; then
    echo "${C_OK}PASS${C_END} — no trigger word; Gate 3 not engaged"
    return 0
  fi

  # 2. Heading present?
  if ! echo "$text" | grep -Eqi '^##+[[:space:]]+regression[[:space:]]+hypotheses'; then
    echo "${C_FAIL}BLOCK${C_END} — trigger word present, no '## Regression hypotheses' heading"
    echo "  required: add a section titled '## Regression hypotheses' with ≥${MIN_HYPS} bullets."
    return 1
  fi

  # 3. Extract the block under the heading (until next heading of same/lower
  # level or EOF).
  local block
  block="$(printf '%s\n' "$text" | awk '
    /^##+[[:space:]]+[Rr]egression[[:space:]]+[Hh]ypotheses/ { in_block=1; next }
    in_block && /^##+[[:space:]]+/ { in_block=0 }
    in_block { print }
  ')"

  # 4. Count bullet lines.
  local bullets
  bullets="$(printf '%s\n' "$block" | grep -Ec '^[[:space:]]*[-*][[:space:]]+[^[:space:]]' || true)"
  bullets=${bullets:-0}
  if (( bullets < MIN_HYPS )); then
    echo "${C_FAIL}BLOCK${C_END} — only ${bullets} hypothesis bullet(s) under heading; need ≥ ${MIN_HYPS}"
    return 1
  fi

  # 5. For every bullet, ensure `evidence:` appears with sufficient chars.
  # Walk the block line-by-line grouping each bullet with subsequent non-bullet
  # non-heading lines, then require `evidence:` with non-trivial text.
  local missing_evidence=0
  local bullet_idx=0
  local current=""
  local finalize
  finalize() {
    local b="$1"
    [[ -z "$b" ]] && return 0
    # Does the bullet group contain `evidence: ...` with ≥ MIN_EVIDENCE_CHARS
    # non-space chars after?
    local ev_line
    ev_line="$(printf '%s\n' "$b" | grep -Ei 'evidence[:：]' | head -1 || true)"
    if [[ -z "$ev_line" ]]; then
      missing_evidence=$((missing_evidence+1))
      echo "${C_WARN}  hypothesis #${bullet_idx} missing 'evidence:' line${C_END}"
      return 0
    fi
    local ev_text
    ev_text="$(printf '%s' "$ev_line" | sed -E 's/.*evidence[:：][[:space:]]*//i')"
    local ev_nonspace
    ev_nonspace="$(printf '%s' "$ev_text" | tr -d '[:space:]')"
    if (( ${#ev_nonspace} < MIN_EVIDENCE_CHARS )); then
      missing_evidence=$((missing_evidence+1))
      echo "${C_WARN}  hypothesis #${bullet_idx} evidence too short (${#ev_nonspace} chars; need ≥${MIN_EVIDENCE_CHARS}): ${ev_text:0:40}...${C_END}"
      return 0
    fi
  }

  while IFS= read -r line; do
    if echo "$line" | grep -Eq '^[[:space:]]*[-*][[:space:]]+[^[:space:]]'; then
      # new bullet: finalize previous
      finalize "$current"
      bullet_idx=$((bullet_idx+1))
      current="$line"
    else
      current+=$'\n'"$line"
    fi
  done <<< "$block"
  finalize "$current"

  if (( missing_evidence > 0 )); then
    echo "${C_FAIL}BLOCK${C_END} — ${missing_evidence}/${bullets} hypotheses missing or short-evidence"
    return 1
  fi

  echo "${C_OK}PASS${C_END} — ${bullets} hypotheses, all with evidence ≥${MIN_EVIDENCE_CHARS} chars"
  return 0
}

# ─── Log writer ────────────────────────────────────────────────────────────
write_log() {
  local verdict="$1"
  local claim_preview="$2"
  [[ "$WRITE_LOG" == 0 ]] && return 0
  local repo_root
  repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
  local log_dir="$repo_root/.claim-poe"
  mkdir -p "$log_dir"
  local ts
  ts="$(date -Iseconds 2>/dev/null || date -u +%Y-%m-%dT%H:%M:%SZ)"
  local preview_escaped
  preview_escaped="$(printf '%s' "$claim_preview" | head -c 400 | tr '\n' ' ' | sed 's/"/\\"/g')"
  printf '{"ts":"%s","verdict":"%s","preview":"%s"}\n' "$ts" "$verdict" "$preview_escaped" \
    >> "$log_dir/log.jsonl"
}

# ─── Self-test (fixture-driven) ────────────────────────────────────────────
run_self_test() {
  local pass=0
  local fail=0
  run_case() {
    local name="$1" expected="$2" text="$3"
    local result
    set +e
    result="$(evaluate_claim "$text" 2>&1)"
    local ec=$?
    set -e
    local verdict="pass"; (( ec != 0 )) && verdict="block"
    local name_exp="$(echo "$name" | tr '[:upper:]' '[:lower:]')"
    if [[ "$verdict" == "$expected" ]]; then
      echo "${C_OK}  ok${C_END}  ${name}"
      pass=$((pass+1))
    else
      echo "${C_FAIL}  FAIL${C_END} ${name}  expected=${expected} got=${verdict}"
      echo "    output: $result"
      fail=$((fail+1))
    fi
  }

  echo "${C_HEAD}── claim-poe self-test ──${C_END}"
  run_case "no-trigger-word" "pass" \
    "discussing options, still investigating root cause."
  run_case "trigger-no-section" "block" \
    "Fix deployed — all green. No further action required."
  run_case "trigger-1-bullet-no-ev" "block" "$(cat <<'EOF'
fix deployed.

## Regression hypotheses
- Something could have broken.
EOF
)"
  run_case "trigger-3-bullets-no-ev" "block" "$(cat <<'EOF'
fix complete.

## Regression hypotheses
- Touching the auth middleware could break the login page.
- The rebuild might have invalidated the static-files cache.
- The new env var may have been missed on one host.
EOF
)"
  run_case "trigger-3-bullets-short-ev" "block" "$(cat <<'EOF'
fixed.

## Regression hypotheses
- Auth middleware — evidence: ok.
- Static cache — evidence: ok.
- Env var — evidence: ok.
EOF
)"
  run_case "trigger-3-bullets-full-ev" "pass" "$(cat <<'EOF'
fixed — all green.

## Regression hypotheses
- Touching the auth middleware could break the login page. evidence: curl -s http://localhost:3000/login returned 200 with the login form HTML intact; manually navigated in browser to confirm submit still routes to /api/auth.
- The rebuild might have invalidated the static-files cache. evidence: GET /_next/static/chunks/main.js returned 200 from the deployed server; cache headers include etag.
- The new env var may have been missed on one host. evidence: ran scripts/env-parity-check.sh across all 8 pods + server; every host reports FLAG_NEW_FEATURE=true from /api/v1/flags.
EOF
)"

  echo ""
  if (( fail > 0 )); then
    echo "${C_FAIL}self-test: ${fail} failed, ${pass} passed${C_END}"
    return 1
  fi
  echo "${C_OK}self-test: ${pass} passed, 0 failed${C_END}"
  return 0
}

# ─── Entry ─────────────────────────────────────────────────────────────────
if (( SELF_TEST )); then
  run_self_test
  exit $?
fi

# Source selection order: --text > --file > stdin.
if [[ -z "$CLAIM_TEXT" ]]; then
  if [[ -n "$CLAIM_FILE" ]]; then
    [[ -f "$CLAIM_FILE" ]] || { echo "file not found: $CLAIM_FILE" >&2; exit 2; }
    CLAIM_TEXT="$(cat "$CLAIM_FILE")"
  else
    # Read stdin. If TTY (no piped input), refuse with a hint.
    if [[ -t 0 ]]; then
      echo "no claim body — pipe text on stdin, or use --text/--file" >&2
      echo "  e.g. echo 'fix deployed' | bash scripts/claim-poe.sh" >&2
      exit 2
    fi
    CLAIM_TEXT="$(cat)"
  fi
fi

[[ -z "$CLAIM_TEXT" ]] && { echo "empty claim body" >&2; exit 2; }

set +e
verdict_output="$(evaluate_claim "$CLAIM_TEXT")"
ec=$?
set -e
echo "$verdict_output"

verdict="pass"; (( ec != 0 )) && verdict="block"
write_log "$verdict" "$CLAIM_TEXT"
exit $ec
