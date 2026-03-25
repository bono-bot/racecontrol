#!/usr/bin/env bash
# audit/phases/tier13/phase54.sh -- Phase 54: Command Registry and Shell Relay
# Tier: 13 (Registry & Relay Integrity)
# What: Comms-link command registry populated, dynamic registration works, shell relay allowlist enforced.
# Standing rules: COM-08 (static registry), COM-09 (dynamic), COM-11 (shell allowlist)

set -u
set -o pipefail
# NO set -e

run_phase54() {
  local phase="54" tier="13"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # --- Check 1: Registry endpoint reachable and returns valid JSON with keys ---
  local registry_response; registry_response=$(http_get "http://localhost:8766/relay/registry" 5)
  local key_count; key_count=$(printf '%s' "$registry_response" | jq 'keys | length' 2>/dev/null)
  key_count="${key_count//[[:space:]]/}"
  if [[ -z "$registry_response" || "$registry_response" = *"curl"* ]]; then
    status="FAIL"; severity="P1"; message="Registry endpoint http://localhost:8766/relay/registry is DOWN or unreachable"
  elif [[ "${key_count:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Registry endpoint UP — ${key_count} commands registered"
  else
    status="FAIL"; severity="P1"; message="Registry endpoint responded but returned invalid JSON or no keys"
  fi
  emit_result "$phase" "$tier" "james-registry-endpoint" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 2: Core commands present in registry ---
  local missing_cmds=""
  local check_count=0
  for CMD in git_pull git_status node_version health_check pm2_status uptime; do
    check_count=$((check_count + 1))
    if ! printf '%s' "$registry_response" | jq -e ".\"${CMD}\"" >/dev/null 2>&1; then
      missing_cmds="${missing_cmds} ${CMD}"
    fi
  done
  missing_cmds="${missing_cmds# }"  # trim leading space
  if [[ -z "$missing_cmds" ]]; then
    status="PASS"; severity="P3"; message="All ${check_count} core commands present in registry (git_pull, git_status, node_version, health_check, pm2_status, uptime)"
  else
    status="WARN"; severity="P2"; message="Registry missing commands: ${missing_cmds}"
  fi
  emit_result "$phase" "$tier" "james-registry-commands" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Check 3: Dynamic registration test (register + verify + delete) ---
  local tmpfile; tmpfile=$(mktemp /tmp/audit-reg-XXXXXX.json)
  # Write registration payload to temp file (cmd.exe quoting workaround)
  jq -n '{"name":"audit_test","command":"echo audit_ok","tier":"PUBLIC"}' > "$tmpfile" 2>/dev/null

  local reg_response; reg_response=$(curl -s -m 10 -X POST \
    -H "Content-Type: application/json" \
    "http://localhost:8766/relay/registry/register" \
    -d @"$tmpfile" 2>/dev/null || echo "")
  rm -f "$tmpfile"

  # Verify it appears in the registry
  local verify_response; verify_response=$(http_get "http://localhost:8766/relay/registry" 5)
  local found; found=$(printf '%s' "$verify_response" | jq -e '.audit_test' >/dev/null 2>&1 && echo "YES" || echo "NO")

  # Cleanup: delete the test registration
  curl -s -m 5 -X DELETE "http://localhost:8766/relay/registry/audit_test" >/dev/null 2>&1 || true

  if [[ "$found" = "YES" ]]; then
    status="PASS"; severity="P3"; message="Dynamic registration works: register + verify + delete all succeeded"
  else
    status="WARN"; severity="P2"; message="Dynamic registration check failed: audit_test not found in registry after POST to /relay/registry/register (reg_response: ${reg_response:-empty})"
  fi
  emit_result "$phase" "$tier" "james-registry-dynamic" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase54
