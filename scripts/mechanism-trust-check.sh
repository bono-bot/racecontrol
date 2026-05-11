#!/usr/bin/env bash
# mechanism-trust-check.sh — 5-question audit on V2-foundational-boundary delivery surfaces
# Spec: racecontrol/.planning/hooks-bilateral/pre-v2-edit-rca-check.spec.md
# Doctrine: §S-146 V1↔V2 RCA + §S-174 mechanism-trust-check upstream extension
# Phase 2 of §S-174 4-phase enforcement plan
# Output: JSON consumed by Phase 1 hook (pre-v2-edit-rca-check.js) as PASS condition (b)

set -u

VERSION="0.1.0-bono"
SCRIPT_NAME="mechanism-trust-check.sh"
DEFAULT_OUT_DIR="/root/racecontrol/.planning/specs/v2/MECHANISM-TRUST"

usage() {
  cat <<EOF
${SCRIPT_NAME} ${VERSION}

Usage:
  $0 --surface <name> --mode interactive [--out <path>]
  $0 --surface <name> --mode template [--out <path>]
  $0 --surface <name> --mode non-interactive \\
       --atomic-primitives YES|NO|N/A \\
       --atomic-primitives-evidence "<text>" \\
       --ttl-sentinels YES|NO|N/A \\
       --ttl-sentinels-evidence "<text>" \\
       --behavioral-verify YES|NO|N/A \\
       --behavioral-verify-evidence "<text>" \\
       --single-target-dry-run YES|NO|N/A \\
       --single-target-dry-run-evidence "<text>" \\
       --guard-contracts YES|NO|N/A \\
       --guard-contracts-evidence "<text>" \\
       [--notes "<text>"] [--out <path>]

Modes:
  interactive       Prompt for each answer (default)
  template          Output blank JSON template
  non-interactive   All 5 answers + evidence required via flags

Output:
  Default path: ${DEFAULT_OUT_DIR}/<surface>-<YYYY-MM-DD>.json
  Override:     --out <path>

Exit codes:
  0 on success; 1 on bad args; 2 on JSON write failure.

Doctrine:
  Cache valid 30 days from mtime; consumed by pre-v2-edit-rca-check.js as
  PASS condition (b). FAIL verdict permits the trust-check to exist (the
  artifact records the V1-shaped state) but the Phase 1 hook's PASS gate
  treats any cache file within 30 days as PASS — interpretation: caller
  has acknowledged the V1-shape by running this audit. Override discipline
  via V2_RCA_BYPASS=1 still needed for FAIL surfaces in true emergency.
EOF
}

# --- Defaults ---
SURFACE=""
MODE="interactive"
OUT=""
NOTES=""
ATOMIC=""
ATOMIC_EV=""
TTL=""
TTL_EV=""
BEHAV=""
BEHAV_EV=""
DRY=""
DRY_EV=""
GUARD=""
GUARD_EV=""

# --- Parse args ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --surface) SURFACE="$2"; shift 2 ;;
    --mode) MODE="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    --notes) NOTES="$2"; shift 2 ;;
    --atomic-primitives) ATOMIC="$2"; shift 2 ;;
    --atomic-primitives-evidence) ATOMIC_EV="$2"; shift 2 ;;
    --ttl-sentinels) TTL="$2"; shift 2 ;;
    --ttl-sentinels-evidence) TTL_EV="$2"; shift 2 ;;
    --behavioral-verify) BEHAV="$2"; shift 2 ;;
    --behavioral-verify-evidence) BEHAV_EV="$2"; shift 2 ;;
    --single-target-dry-run) DRY="$2"; shift 2 ;;
    --single-target-dry-run-evidence) DRY_EV="$2"; shift 2 ;;
    --guard-contracts) GUARD="$2"; shift 2 ;;
    --guard-contracts-evidence) GUARD_EV="$2"; shift 2 ;;
    --help|-h) usage; exit 0 ;;
    *) echo "ERROR: unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if [ -z "$SURFACE" ]; then
  echo "ERROR: --surface required" >&2
  usage >&2
  exit 1
fi

# --- Helpers ---
TODAY="$(date -u +%Y-%m-%d)"
NOW_ISO="$(date -u +%Y-%m-%dT%H:%M:%S.000Z)"
[ -z "$OUT" ] && OUT="${DEFAULT_OUT_DIR}/${SURFACE}-${TODAY}.json"

prompt_q() {
  local key="$1"
  local q="$2"
  local ans evidence
  while true; do
    read -p "Q $key — $q (YES/NO/N/A): " ans
    case "$ans" in
      YES|NO|N/A) break ;;
      *) echo "  please enter YES, NO, or N/A" >&2 ;;
    esac
  done
  read -p "  evidence (path:line / commit / one-liner): " evidence
  echo "$ans|$evidence"
}

validate_yes_no_na() {
  case "$1" in
    YES|NO|N/A) return 0 ;;
    *) echo "ERROR: invalid value '$1' for $2 (must be YES|NO|N/A)" >&2; return 1 ;;
  esac
}

# --- Mode dispatch ---
if [ "$MODE" = "interactive" ]; then
  echo "=== mechanism-trust-check.sh v${VERSION} — surface: ${SURFACE} ==="
  echo "Per §S-174 mechanism-trust-check upstream rule. ALL 5 must answer YES for V2-aligned."
  echo ""
  IFS='|' read -r ATOMIC ATOMIC_EV  < <(prompt_q "1" "Atomic primitives? (single /exec atomic chain, not multi-step pipeline)")
  IFS='|' read -r TTL TTL_EV        < <(prompt_q "2" "TTL-bounded sentinels integrated with atomic primitive?")
  IFS='|' read -r BEHAV BEHAV_EV    < <(prompt_q "3" "Behavioral-verify success? (binary hash / mtime / ws_uptime, not echo-string)")
  IFS='|' read -r DRY DRY_EV        < <(prompt_q "4" "Single-target dry-run path? (--canary / staging / behavioral test harness)")
  IFS='|' read -r GUARD GUARD_EV    < <(prompt_q "5" "Guard contracts? (parser-not-regex with explicit allowlist)")
  read -p "Notes (optional): " NOTES
elif [ "$MODE" = "template" ]; then
  ATOMIC="<YES|NO|N/A>"; ATOMIC_EV="<path:line / commit / one-liner>"
  TTL="<YES|NO|N/A>";    TTL_EV="<path:line / commit / one-liner>"
  BEHAV="<YES|NO|N/A>";  BEHAV_EV="<path:line / commit / one-liner>"
  DRY="<YES|NO|N/A>";    DRY_EV="<path:line / commit / one-liner>"
  GUARD="<YES|NO|N/A>";  GUARD_EV="<path:line / commit / one-liner>"
  NOTES="<free-text>"
elif [ "$MODE" = "non-interactive" ]; then
  for v in ATOMIC TTL BEHAV DRY GUARD; do
    val="${!v}"
    if [ -z "$val" ]; then
      echo "ERROR: --${v,,} required in non-interactive mode" >&2; exit 1
    fi
    validate_yes_no_na "$val" "$v" || exit 1
  done
else
  echo "ERROR: unknown mode '$MODE'" >&2; usage >&2; exit 1
fi

# --- Compute overall verdict ---
verdict() {
  local n_yes=0 n_no=0 n_na=0
  for v in "$ATOMIC" "$TTL" "$BEHAV" "$DRY" "$GUARD"; do
    case "$v" in
      YES) n_yes=$((n_yes+1)) ;;
      NO)  n_no=$((n_no+1))   ;;
      N/A) n_na=$((n_na+1))   ;;
    esac
  done
  if [ "$MODE" = "template" ]; then echo "<PASS|FAIL|PARTIAL>"; return; fi
  if [ "$n_no" -eq 0 ] && [ "$n_yes" -ge 4 ]; then echo "PASS"; return; fi
  if [ "$n_no" -ge 3 ]; then echo "FAIL"; return; fi
  echo "PARTIAL"
}
OVERALL="$(verdict)"

# --- JSON output (manual encoding to avoid jq dependency) ---
json_escape() {
  # Minimal JSON string escape: \, ", and control chars 0x00-0x1F
  python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "$1" 2>/dev/null \
    || node -e "console.log(JSON.stringify(process.argv[1]))" "$1" 2>/dev/null \
    || printf '"%s"' "$(printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\n/\\n/g; s/\r/\\r/g; s/\t/\\t/g')"
}

mkdir -p "$(dirname "$OUT")"

JSON=$(cat <<EOF
{
  "version": "${VERSION}",
  "surface": "${SURFACE}",
  "date": "${TODAY}",
  "audited_at": "${NOW_ISO}",
  "auditor": "bono",
  "questions": {
    "atomic_primitives": {
      "answer": "${ATOMIC}",
      "evidence": $(json_escape "${ATOMIC_EV}")
    },
    "ttl_sentinels": {
      "answer": "${TTL}",
      "evidence": $(json_escape "${TTL_EV}")
    },
    "behavioral_verify": {
      "answer": "${BEHAV}",
      "evidence": $(json_escape "${BEHAV_EV}")
    },
    "single_target_dry_run": {
      "answer": "${DRY}",
      "evidence": $(json_escape "${DRY_EV}")
    },
    "guard_contracts": {
      "answer": "${GUARD}",
      "evidence": $(json_escape "${GUARD_EV}")
    }
  },
  "overall": "${OVERALL}",
  "notes": $(json_escape "${NOTES}")
}
EOF
)

echo "$JSON" > "$OUT" || { echo "ERROR: write to $OUT failed" >&2; exit 2; }

echo "Wrote: $OUT (verdict: $OVERALL)" >&2
echo "$JSON"
exit 0
