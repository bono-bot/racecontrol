#!/usr/bin/env bash
# run-all-checkers.sh — workflow-plan Day 2 orchestrator.
#
# Runs every board-state generator + coverage checker in sequence, aggregates
# results into .planning/board-state/status.json, and prints a one-line
# summary suitable for comms-link WS dispatch.
#
# Exit codes:
#   0 — all checkers green
#   1 — at least one checker red (the human/AI must read status.json)
#   2 — generator failure (source code is in a state that can't be parsed)
#
# Designed to run from:
#   - James pre-commit hook (fast path, touched surfaces only — future work)
#   - Bono pm2 workflow-monitor (every 5 min, always full battery)
#   - CI on every PR
#
# Dependencies: python3, jq. No sqlite3, no live probes — safe anywhere.

set -u

# Resolve repo root from script location so the script is cwd-independent.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

cd "$REPO_ROOT" || { echo "ERROR: cannot cd to $REPO_ROOT" >&2; exit 2; }

BOARD_DIR="$REPO_ROOT/.planning/board-state"
mkdir -p "$BOARD_DIR"

STATUS_JSON="$BOARD_DIR/status.json"
TIMESTAMP_IST="$(python3 -c "from datetime import datetime,timedelta,timezone; print((datetime.now(timezone.utc)+timedelta(hours=5,minutes=30)).strftime('%Y-%m-%d %H:%M IST'))")"
COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"

# Each entry: (name, generator_cmd, checker_cmd)
# Empty checker_cmd = generator-only (stub surfaces).
declare -a CHECKS=(
  "schema-dpdp|python3 $SCRIPT_DIR/board-state-generate.py|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/dpdp-coverage-check.py"
  "fleet-swaplog-parity|true|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/fleet-swaplog-parity-check.py"
  "deploy-staging-parity|true|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/deploy-staging-parity-check.py"
  "workflow-centrality|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/workflow-graph-generate.py|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/workflow-centrality-check.py"
  "instant-arithmetic|true|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/instant-arithmetic-check.py"
  "process-ownership|true|bash $SCRIPT_DIR/process-ownership.sh"
  "schema-types|true|PYTHONIOENCODING=utf-8 python3 $SCRIPT_DIR/schema-types-check.py"
)
# NOTE: data-content-check.py is shipped to origin/main (commit f0d0ffb1) but its
# rules YAML (.planning/functional-layer/data-content-rules.yaml, commit 8810ca6c)
# is still only on origin/feat/f4-data-content-check-20260421. Without the YAML the
# checker exits 2 (internal error). Leaving it OUT of the CHECKS array until the
# YAML lands on main — wiring it now would produce false RED in status.json.
# Tracked as gap for PACT-20260424-007.

# Future surfaces (stubs — registered but generators not yet written):
#   types     — extract rc-common serde structs, cross-check vs kiosk TS types
#   protocol  — extract CoreToAgentMessage/AgentToCoreMessage variants, check handler coverage
#   http      — extract api/routes.rs route table, check auth-tier coverage

results_json="["
sep=""
overall_exit=0
summary_lines=()

for entry in "${CHECKS[@]}"; do
  IFS="|" read -r name gen_cmd check_cmd <<< "$entry"
  gen_out="$(eval "$gen_cmd" 2>&1)"
  gen_status=$?
  if [ "$gen_status" -ne 0 ]; then
    summary_lines+=("$name: GENERATOR_ERROR")
    overall_exit=2
    status="generator_error"
    check_exit=-1
    check_out="$gen_out"
  elif [ -z "$check_cmd" ]; then
    summary_lines+=("$name: generator_only_ok")
    status="generator_only_ok"
    check_exit=0
    check_out=""
  else
    check_out="$(eval "$check_cmd" 2>&1)"
    check_exit=$?
    if [ "$check_exit" -eq 0 ]; then
      summary_lines+=("$name: green")
      status="green"
    else
      summary_lines+=("$name: RED ($(echo "$check_out" | grep -c '^  -' || echo 0) gaps)")
      status="red"
      overall_exit=1
    fi
  fi

  # Escape for JSON (naive; good enough for logged output).
  safe_out="$(printf '%s' "$check_out" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
  results_json+="${sep}{\"name\":\"$name\",\"status\":\"$status\",\"exit\":$check_exit,\"output\":$safe_out}"
  sep=","
done
results_json+="]"

# Write aggregate status file.
python3 -c "
import json, sys
doc = {
    'timestamp_ist': '$TIMESTAMP_IST',
    'commit': '$COMMIT',
    'overall_exit': $overall_exit,
    'summary': [${summary_lines_python:-}],
    'checks': $results_json,
}
print(json.dumps(doc, indent=2))
" <<< "" > "$STATUS_JSON" 2>/dev/null || {
  # Fallback: the $results_json + summary interpolation can be brittle in
  # shells. Write a minimal JSON that still lets the WS dispatcher parse.
  printf '{\n  "timestamp_ist": "%s",\n  "commit": "%s",\n  "overall_exit": %d,\n  "checks": %s\n}\n' \
    "$TIMESTAMP_IST" "$COMMIT" "$overall_exit" "$results_json" > "$STATUS_JSON"
}

# One-line summary for callers (pm2 logs, pre-commit hook, CI).
echo "workflow-check $TIMESTAMP_IST commit=$COMMIT overall=$overall_exit $(printf '%s; ' "${summary_lines[@]}")"

exit "$overall_exit"
