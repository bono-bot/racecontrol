#!/usr/bin/env bash
# debug-poe.sh — Gate 2 sequencer: pre-fix alternative-cause enumeration.
#
# Wave 2 scaffold — plan_deploy_poe_gates_20260423.md.
# Sibling gates: scripts/deploy-poe.sh (Gate 1, shipped), scripts/claim-poe.sh (Gate 3, Wave 4).
#
# Class of failure this blocks: "correlation mistaken for causation" — a
# candidate fix is applied to the first plausible cause without eliminating
# alternatives. Pod 6 Variable_dump.exe incident (2026-03-26) is the archetype:
# crash dump blamed, but RAM pressure + FFB driver + USB hub were never tested.
#
# Behavior (Wave 2):
#   1. Take a symptom description + optional affected-target.
#   2. Require at least 3 distinct alternative-cause hypotheses.
#   3. Each hypothesis must have an eliminated|confirmed|untestable verdict
#      with evidence text. Block if fewer than 3 or any missing verdict.
#   4. Exactly one cause may be marked "confirmed" to proceed; two or more
#      confirmed causes block (class of failure is multiple root causes
#      pretending to be one — needs separate fixes, not a single patch).
#
# Verdict vocabulary (PER hypothesis):
#   eliminated:<evidence>   — tested and disproven (e.g. "ran mdsched, RAM clean")
#   confirmed:<evidence>    — reproduces; this is the cause
#   untestable:<reason>     — cannot currently test (e.g. "pods offline"; counts toward 3-minimum but flagged)
#
# Verdict input modes:
#   A. --hypotheses-file PATH  JSON: {"cause1":{"label":"...","verdict":"eliminated:..."}, ...}
#   B. --cause "LABEL|VERDICT" — repeatable flag (2+ required before CLI-mode passes)
#
# Usage: bash scripts/debug-poe.sh <symptom> [flags]
# Flags:
#   --target NAME          affected target (pod_1, server_23, cloud, kiosk, ...)
#   --hypotheses-file PATH JSON file with cause entries
#   --cause "LABEL|VERDICT" per-cause flag (repeatable)
#   --search-memory        grep memory directory for symptom keywords (informational)
#   --memory-dir PATH      override memory root (default ~/.claude/projects/C--Users-bono/memory)
#   --no-log               do not append to .debug-poe/log.jsonl
#   --log-dir PATH         override log directory (default .debug-poe/)
#   --no-color             disable ANSI colors
#   -h | --help            print usage
# Exit codes:
#   0 passed (≥3 verdicts, exactly ≤1 confirmed)   1 insufficient hypotheses or >1 confirmed
#   2 usage error                                  3 log write failure

set -euo pipefail

# --- Arg parsing ---
SYMPTOM=""
TARGET=""
HYPO_FILE=""
SEARCH_MEMORY=0
MEMORY_DIR="${HOME}/.claude/projects/C--Users-bono/memory"
NO_LOG=0
LOG_DIR=".debug-poe"
USE_COLOR=1
CAUSES=()

print_usage() { sed -n '3,38p' "$0" | sed 's/^# \{0,1\}//'; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGET="$2"; shift 2 ;;
    --hypotheses-file) HYPO_FILE="$2"; shift 2 ;;
    --cause) CAUSES+=("$2"); shift 2 ;;
    --search-memory) SEARCH_MEMORY=1; shift ;;
    --memory-dir) MEMORY_DIR="$2"; shift 2 ;;
    --no-log) NO_LOG=1; shift ;;
    --log-dir) LOG_DIR="$2"; shift 2 ;;
    --no-color) USE_COLOR=0; shift ;;
    -h|--help) print_usage; exit 0 ;;
    -*) echo "unknown flag: $1" >&2; print_usage >&2; exit 2 ;;
    *) [[ -z "$SYMPTOM" ]] && SYMPTOM="$1" && shift || { echo "multiple symptoms given (quote multi-word symptoms)" >&2; exit 2; } ;;
  esac
done

[[ -z "$SYMPTOM" ]] && { echo "missing <symptom> argument" >&2; print_usage >&2; exit 2; }

if [[ "$USE_COLOR" == 1 ]]; then
  C_HEAD=$'\033[1;36m'; C_WARN=$'\033[1;33m'; C_DIM=$'\033[2m'; C_OK=$'\033[1;32m'; C_ERR=$'\033[1;31m'; C_END=$'\033[0m'
else
  C_HEAD=""; C_WARN=""; C_DIM=""; C_OK=""; C_ERR=""; C_END=""
fi

repo_root="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
cd "$repo_root"

# --- Parse hypotheses from file (if provided) ---
declare -A HYPO_LABELS=()
declare -A HYPO_VERDICTS=()
idx=0

if [[ -n "$HYPO_FILE" ]]; then
  if [[ ! -f "$HYPO_FILE" ]]; then
    echo "hypotheses file not found: $HYPO_FILE" >&2; exit 2
  fi
  while IFS=$'\t' read -r key label verdict; do
    [[ -z "$key" ]] && continue
    HYPO_LABELS[$key]="$label"
    HYPO_VERDICTS[$key]="$verdict"
    idx=$((idx+1))
  done < <(jq -r 'to_entries | .[] | "\(.key)\t\(.value.label)\t\(.value.verdict)"' "$HYPO_FILE" 2>/dev/null || true)
fi

# --- Parse CLI --cause flags ---
for c in "${CAUSES[@]+"${CAUSES[@]}"}"; do
  # split on first | only (verdict may contain :)
  label="${c%%|*}"
  verdict="${c#*|}"
  if [[ "$label" == "$c" || -z "$verdict" ]]; then
    echo "invalid --cause: '$c' (expected 'LABEL|VERDICT')" >&2; exit 2
  fi
  idx=$((idx+1))
  key="cause${idx}"
  HYPO_LABELS[$key]="$label"
  HYPO_VERDICTS[$key]="$verdict"
done

# --- Header ---
echo ""
echo "${C_HEAD}═══ debug-poe Gate 2 (Wave 2 scaffold) ═══${C_END}"
printf "%-22s %s\n" "symptom:" "$SYMPTOM"
printf "%-22s %s\n" "target:"  "${TARGET:-<unspecified>}"
printf "%-22s %s\n" "hypotheses:" "$idx"
echo ""

# --- Memory search (informational — surfaces prior incidents with same symptom keywords) ---
if (( SEARCH_MEMORY == 1 )); then
  if [[ -d "$MEMORY_DIR" ]]; then
    echo "${C_HEAD}── memory search (informational) ──${C_END}"
    # Split symptom into keywords (alphanumeric tokens of >3 chars)
    kws=$(printf "%s" "$SYMPTOM" | tr -c 'A-Za-z0-9' '\n' | awk 'length($0)>3' | sort -u | head -5)
    if [[ -n "$kws" ]]; then
      for kw in $kws; do
        hits=$(grep -rli --include="*.md" "$kw" "$MEMORY_DIR" 2>/dev/null | head -3 || true)
        if [[ -n "$hits" ]]; then
          printf "  ${C_DIM}keyword '%s'${C_END}\n" "$kw"
          printf "%s\n" "$hits" | sed 's#^#    #'
        fi
      done
    else
      echo "  ${C_DIM}(no keywords >3 chars extracted from symptom)${C_END}"
    fi
    echo ""
  else
    echo "${C_DIM}memory dir not found: $MEMORY_DIR (skipping search)${C_END}"
    echo ""
  fi
fi

# --- Verdict audit ---
if (( idx < 3 )); then
  echo "${C_ERR}BLOCK: need ≥3 alternative-cause hypotheses (got $idx).${C_END}" >&2
  echo "Pass via --cause \"LABEL|eliminated:evidence\" (repeatable) or --hypotheses-file PATH." >&2
  echo "Verdict vocabulary: eliminated:<evidence> | confirmed:<evidence> | untestable:<reason>" >&2
  exit 1
fi

echo "${C_HEAD}── hypothesis verdicts ──${C_END}"
confirmed=0
invalid=0
for key in "${!HYPO_LABELS[@]}"; do
  label="${HYPO_LABELS[$key]}"
  verdict="${HYPO_VERDICTS[$key]}"
  verb="${verdict%%:*}"
  detail="${verdict#*:}"
  case "$verb" in
    eliminated) color="$C_DIM"  ;;
    confirmed)  color="$C_OK"   ; confirmed=$((confirmed+1)) ;;
    untestable) color="$C_WARN" ;;
    *)
      echo "  ${C_ERR}$key: invalid verdict '$verdict' (expect eliminated:|confirmed:|untestable:)${C_END}" >&2
      invalid=$((invalid+1))
      continue ;;
  esac
  [[ "$detail" == "$verdict" ]] && detail="(no detail — WARNING)"
  printf "  ${color}%-11s${C_END}  %s\n       ${C_DIM}%s${C_END}\n" "$verb" "$label" "$detail"
done
echo ""

if (( invalid > 0 )); then
  echo "${C_ERR}BLOCK: $invalid invalid verdict(s). Use eliminated:|confirmed:|untestable:.${C_END}" >&2
  exit 1
fi

if (( confirmed > 1 )); then
  echo "${C_ERR}BLOCK: $confirmed hypotheses marked 'confirmed' — multiple-root-cause scenario.${C_END}" >&2
  echo "Separate fixes per cause. Do NOT single-patch multiple confirmed roots." >&2
  exit 1
fi

if (( confirmed == 0 )); then
  echo "${C_WARN}WARN: zero hypotheses marked 'confirmed'. You cannot fix what you haven't identified.${C_END}"
  echo "Hypotheses enumerated but cause still unknown — continue investigation before applying a fix."
fi

# --- JSONL log ---
if (( NO_LOG == 0 )); then
  mkdir -p "$LOG_DIR"
  log_file="$LOG_DIR/log.jsonl"
  now_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  # Build hypotheses JSON
  h_json="{"
  first=1
  for key in "${!HYPO_LABELS[@]}"; do
    label_esc=$(printf '%s' "${HYPO_LABELS[$key]}" | jq -Rs . 2>/dev/null || printf '""')
    verdict_esc=$(printf '%s' "${HYPO_VERDICTS[$key]}" | jq -Rs . 2>/dev/null || printf '""')
    (( first == 1 )) || h_json+=","
    h_json+="\"$key\":{\"label\":$label_esc,\"verdict\":$verdict_esc}"
    first=0
  done
  h_json+="}"
  s_esc=$(printf '%s' "$SYMPTOM" | jq -Rs . 2>/dev/null || printf '""')
  t_esc=$(printf '%s' "${TARGET:-}" | jq -Rs . 2>/dev/null || printf '""')
  line="{\"ts\":\"$now_iso\",\"symptom\":$s_esc,\"target\":$t_esc,\"hypotheses\":$h_json,\"confirmed_count\":$confirmed}"
  if ! printf '%s\n' "$line" >> "$log_file"; then
    echo "${C_ERR}failed to append to $log_file${C_END}" >&2
    exit 3
  fi
  printf "${C_DIM}logged: %s${C_END}\n" "$log_file"
fi

if (( confirmed == 1 )); then
  echo "${C_OK}PASS: ≥3 hypotheses enumerated, exactly 1 confirmed. Proceed with targeted fix.${C_END}"
else
  echo "${C_WARN}SOFT-PASS: hypotheses enumerated but no confirmed cause yet.${C_END}"
fi
exit 0
