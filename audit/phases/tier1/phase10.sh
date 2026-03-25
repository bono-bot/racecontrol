#!/usr/bin/env bash
# audit/phases/tier1/phase10.sh -- Phase 10: AI Healer / Watchdog
# Tier: 1 (Infrastructure Foundation)
# What: rc-watchdog monitoring all 10 services, Ollama responsive with expected models.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase10() {
  local phase="10" tier="1"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Watchdog state file
  local watchdog_state="C:/Users/bono/.claude/watchdog-state.json"
  if [[ -f "$watchdog_state" ]]; then
    local failure_count; failure_count=$(jq '[.[] | .failure_count // 0] | max' "$watchdog_state" 2>/dev/null || echo "0")
    if [[ "${failure_count:-0}" -eq 0 ]]; then
      status="PASS"; severity="P3"; message="AI healer: no services in failure state"
    elif [[ "${failure_count:-0}" -le 3 ]]; then
      status="WARN"; severity="P2"; message="AI healer: max failure_count=${failure_count} (monitoring, not critical)"
    else
      status="FAIL"; severity="P2"; message="AI healer: max failure_count=${failure_count} (service repeatedly failing)"
    fi
  else
    status="WARN"; severity="P2"; message="AI healer watchdog-state.json not found at expected path"
  fi
  emit_result "$phase" "$tier" "james-watchdog-state" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Ollama responding
  response=$(http_get "http://localhost:11434/api/tags" 5)
  if [[ -n "$response" ]]; then
    local model_count; model_count=$(printf '%s' "$response" | jq '.models | length' 2>/dev/null || echo "0")
    if [[ "${model_count:-0}" -ge 2 ]]; then
      status="PASS"; severity="P3"; message="Ollama responding with ${model_count} models"
    elif [[ "${model_count:-0}" -ge 1 ]]; then
      status="WARN"; severity="P2"; message="Ollama responding but only ${model_count} model(s) -- expected qwen2.5:3b + llama3.1:8b"
    else
      status="WARN"; severity="P2"; message="Ollama responding but no models loaded"
    fi
  else
    status="WARN"; severity="P2"; message="Ollama not responding at localhost:11434 (AI healer diagnosis disabled)"
  fi
  emit_result "$phase" "$tier" "james-ollama" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase10
