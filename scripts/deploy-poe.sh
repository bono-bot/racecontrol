#!/usr/bin/env bash
# deploy-poe.sh — Gate 1 sequencer: pre-deploy disruption-hypothesis enumeration.
#
# Wave 1 scaffold — plan_deploy_poe_gates_20260423.md.
# Paired gates (future): scripts/debug-poe.sh (W2), scripts/claim-poe.sh (W4).
#
# Class of failure this blocks: "instance fix without enumeration of disruption
# surfaces" — fix landed for one caller but broke three others, or deployed to
# the server but skipped pods, or touched code but forgot the data migration.
#
# Behavior (Wave 1):
#   1. Take a branch + compute diff vs origin/main merge-base.
#   2. For 8 default hypothesis classes (Function, Data, Env, External, State,
#      Multi-owner, Permanence, Fleet-parity), require an explicit verdict:
#         done        = evidence checked; link or one-line summary required
#         skip        = explicit N/A with reason
#         accept-risk = override with reason (reviewer sign-off needed)
#   3. Missing verdict for any hypothesis → block, exit 1.
#   4. All verdicts present → log JSONL row to .deploy-poe/log.jsonl, exit 0.
#
# Verdict input modes (in precedence order):
#   A. --verdicts-file PATH   — JSON: {"h1":"done:<evidence>","h2":"skip:<reason>", ...}
#   B. --h1 --h2 ... --h8     — CLI flags per hypothesis
#   C. --all-skip REASON      — shortcut for trivially-safe PRs (docs, comment fixes, version bumps)
#   D. --interactive          — prompt-per-hypothesis (W1 does not support yet; use A/B/C)
#
# Usage: bash scripts/deploy-poe.sh <branch> [flags]
# Flags:
#   --verdicts-file PATH   JSON file with h1..h8 keys
#   --h1 VERDICT ... --h8  per-hypothesis verdict (verb:reason)
#   --all-skip REASON      mark every hypothesis as skip:REASON (requires reason)
#   --list-hypotheses      print the 8 default hypotheses + exit 0
#   --no-log               do not append to .deploy-poe/log.jsonl (dry run)
#   --log-dir PATH         override log directory (default .deploy-poe/)
#   --no-color             disable ANSI colors
#   -h | --help            print usage
# Exit codes:
#   0 all hypotheses verdicted           1 missing verdict or arg error
#   2 usage error                        3 log write failure

set -euo pipefail

# --- Default hypotheses (Wave 1 initial — Wave 5 grows from regression history) ---
HYPOTHESES=(
  "h1|Function-layer: does any caller of a changed function break?|scripts/graphify-helpers/deploy-probe.sh <branch>"
  "h2|Data-layer: does data shape change? migration / backward-compat required?|grep JSON shape in readers/writers; check packages/shared-types/generated/"
  "h3|Env-layer: does change alter process start/stop/env-vars?|scripts/audit/process-ownership.sh (PR #19)"
  "h4|External-layer: does change call external service with new pattern?|list new HTTP verbs/endpoints/auth headers"
  "h5|State-layer: does runtime state invariant change?|tests/e2e/behavioral/run-all.sh (PR #20)"
  "h6|Multi-owner: does any process now have >1 spawn site?|grep Command::new / spawn_safe; later: scripts/audit/process-registry-check.sh (S8)"
  "h7|Permanence: does change survive redeploy? manual fix elsewhere?|CGP Permanence Gate — fix in git vs server-side?"
  "h8|Fleet-parity: does change land on ALL target hosts?|explicit per-target list (Server .23, Pods 1-8, POS .130, James .27, Bono VPS, Cloud, Comms-link)"
)

# --- Arg parsing ---
BRANCH=""
VERDICTS_FILE=""
ALL_SKIP_REASON=""
NO_LOG=0
LOG_DIR=".deploy-poe"
USE_COLOR=1
declare -A VERDICTS=()

print_usage() { sed -n '3,35p' "$0" | sed 's/^# \{0,1\}//'; }

list_hypotheses() {
  printf "Default hypothesis set (Wave 1):\n\n"
  for row in "${HYPOTHESES[@]}"; do
    key="${row%%|*}"; rest="${row#*|}"
    label="${rest%%|*}"; tool="${rest#*|}"
    printf "  %s  %s\n       evidence: %s\n\n" "$key" "$label" "$tool"
  done
  printf "Verdict vocabulary: done:<evidence>  |  skip:<reason>  |  accept-risk:<reason>\n"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --verdicts-file) VERDICTS_FILE="$2"; shift 2 ;;
    --h1) VERDICTS[h1]="$2"; shift 2 ;;
    --h2) VERDICTS[h2]="$2"; shift 2 ;;
    --h3) VERDICTS[h3]="$2"; shift 2 ;;
    --h4) VERDICTS[h4]="$2"; shift 2 ;;
    --h5) VERDICTS[h5]="$2"; shift 2 ;;
    --h6) VERDICTS[h6]="$2"; shift 2 ;;
    --h7) VERDICTS[h7]="$2"; shift 2 ;;
    --h8) VERDICTS[h8]="$2"; shift 2 ;;
    --all-skip) ALL_SKIP_REASON="$2"; shift 2 ;;
    --list-hypotheses) list_hypotheses; exit 0 ;;
    --no-log) NO_LOG=1; shift ;;
    --log-dir) LOG_DIR="$2"; shift 2 ;;
    --no-color) USE_COLOR=0; shift ;;
    -h|--help) print_usage; exit 0 ;;
    -*) echo "unknown flag: $1" >&2; print_usage >&2; exit 2 ;;
    *) [[ -z "$BRANCH" ]] && BRANCH="$1" && shift || { echo "multiple branches given" >&2; exit 2; } ;;
  esac
done

[[ -z "$BRANCH" ]] && { echo "missing <branch> argument" >&2; print_usage >&2; exit 2; }

if [[ "$USE_COLOR" == 1 ]]; then
  C_HEAD=$'\033[1;36m'; C_WARN=$'\033[1;33m'; C_DIM=$'\033[2m'; C_OK=$'\033[1;32m'; C_ERR=$'\033[1;31m'; C_END=$'\033[0m'
else
  C_HEAD=""; C_WARN=""; C_DIM=""; C_OK=""; C_ERR=""; C_END=""
fi

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

# --- Apply verdict precedence: file > CLI flags > --all-skip ---
if [[ -n "$VERDICTS_FILE" ]]; then
  if [[ ! -f "$VERDICTS_FILE" ]]; then
    echo "verdicts file not found: $VERDICTS_FILE" >&2; exit 2
  fi
  while IFS="=" read -r k v; do
    [[ -z "$k" ]] && continue
    VERDICTS[$k]="$v"
  done < <(jq -r 'to_entries | .[] | "\(.key)=\(.value)"' "$VERDICTS_FILE" 2>/dev/null || true)
fi

if [[ -n "$ALL_SKIP_REASON" ]]; then
  for row in "${HYPOTHESES[@]}"; do
    key="${row%%|*}"
    if [[ -z "${VERDICTS[$key]:-}" ]]; then
      VERDICTS[$key]="skip:${ALL_SKIP_REASON}"
    fi
  done
fi

# --- Diff summary ---
base=$(git merge-base origin/main "$BRANCH" 2>/dev/null || git merge-base HEAD "$BRANCH" 2>/dev/null || true)
changed_files=$(git diff --name-only "${base:-HEAD}" "$BRANCH" 2>/dev/null || echo "")
file_count=$(printf "%s\n" "$changed_files" | grep -c . || true)

# --- Header ---
echo ""
echo "${C_HEAD}═══ deploy-poe Gate 1 (Wave 1 scaffold) ═══${C_END}"
printf "%-22s %s\n" "branch:"       "$BRANCH"
printf "%-22s %s\n" "merge-base:"   "${base:-unknown}"
printf "%-22s %s\n" "changed files:" "$file_count"
echo ""

# --- Verdict audit ---
missing=0
echo "${C_HEAD}── hypothesis verdicts ──${C_END}"
for row in "${HYPOTHESES[@]}"; do
  key="${row%%|*}"; rest="${row#*|}"
  label="${rest%%|*}"
  verdict="${VERDICTS[$key]:-}"
  if [[ -z "$verdict" ]]; then
    printf "  %-3s ${C_ERR}MISSING${C_END}  %s\n" "$key" "$label"
    missing=$((missing+1))
    continue
  fi
  verb="${verdict%%:*}"
  detail="${verdict#*:}"
  case "$verb" in
    done)        color="$C_OK"   ;;
    skip)        color="$C_DIM"  ;;
    accept-risk) color="$C_WARN" ;;
    *) echo "  ${C_ERR}invalid verdict for $key: '$verdict' (expect done:|skip:|accept-risk:)${C_END}" >&2
       missing=$((missing+1))
       continue ;;
  esac
  [[ "$detail" == "$verdict" ]] && detail="(no detail — WARNING)"
  printf "  %-3s ${color}%-12s${C_END}  %s\n       ${C_DIM}%s${C_END}\n" "$key" "$verb" "$label" "$detail"
done
echo ""

if (( missing > 0 )); then
  echo "${C_ERR}BLOCK: $missing of 8 hypotheses missing verdict.${C_END}" >&2
  echo "Pass verdicts via --h1..--h8, --verdicts-file PATH, or --all-skip REASON (docs-only / trivially-safe PRs)." >&2
  echo "List the hypothesis set + evidence tools: bash $0 --list-hypotheses" >&2
  exit 1
fi

# --- JSONL log (append) ---
if (( NO_LOG == 0 )); then
  mkdir -p "$LOG_DIR"
  log_file="$LOG_DIR/log.jsonl"
  now_iso=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  # Build verdicts JSON
  verdicts_json="{"
  first=1
  for row in "${HYPOTHESES[@]}"; do
    key="${row%%|*}"
    v="${VERDICTS[$key]}"
    # jq-safe escape via printf + jq -Rs
    v_esc=$(printf '%s' "$v" | jq -Rs . 2>/dev/null || printf '"<unencodable>"')
    (( first == 1 )) || verdicts_json+=","
    verdicts_json+="\"$key\":$v_esc"
    first=0
  done
  verdicts_json+="}"
  branch_esc=$(printf '%s' "$BRANCH" | jq -Rs . 2>/dev/null || printf '""')
  base_esc=$(printf '%s' "${base:-}" | jq -Rs . 2>/dev/null || printf '""')
  line="{\"ts\":\"$now_iso\",\"branch\":$branch_esc,\"merge_base\":$base_esc,\"changed_files\":$file_count,\"verdicts\":$verdicts_json}"
  if ! printf '%s\n' "$line" >> "$log_file"; then
    echo "${C_ERR}failed to append to $log_file${C_END}" >&2
    exit 3
  fi
  printf "${C_DIM}logged: %s${C_END}\n" "$log_file"
fi

echo "${C_OK}PASS: all 8 hypotheses verdicted. Proceed with deploy.${C_END}"
exit 0
