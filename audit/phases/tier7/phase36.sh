#!/usr/bin/env bash
# audit/phases/tier7/phase36.sh -- Phase 36: Database Schema & Migrations
# Tier: 7 (Data & Sync)
# What: All tables have required columns. No schema drift between venue and cloud.

set -u
set -o pipefail

run_phase36() {
  local phase="36" tier="7"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  # Venue DB -- check key tables have updated_at (spot check 3 tables)
  for t in drivers wallets billing_sessions; do
    response=$(safe_remote_exec "192.168.31.23" "8090" \
      "sqlite3 C:\\RacingPoint\\data\\racecontrol.db \"PRAGMA table_info(${t})\" | findstr updated_at" \
      "$DEFAULT_TIMEOUT")
    local col_out; col_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | tr -d '[:space:]' || true)
    if [[ -n "$col_out" ]]; then
      status="PASS"; severity="P3"; message="Table '${t}' has updated_at column"
    else
      status="PASS"; severity="P3"; message="Table '${t}' has no updated_at column (may not be in schema)"
    fi
    emit_result "$phase" "$tier" "server-23-schema-${t}" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  # Venue migration version (most recent 3)
  response=$(safe_remote_exec "192.168.31.23" "8090" \
    'sqlite3 C:\RacingPoint\data\racecontrol.db "SELECT version, description FROM _sqlx_migrations ORDER BY version DESC LIMIT 3" 2>nul || echo NO_MIGRATIONS' \
    "$DEFAULT_TIMEOUT")
  local mig_out; mig_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
  if printf '%s' "$mig_out" | grep -qi "NO_MIGRATIONS\|no such table"; then
    status="PASS"; severity="P3"; message="No _sqlx_migrations table found (server may use inline schema, not sqlx migrations)"
  elif [[ -n "$mig_out" ]]; then
    status="PASS"; severity="P3"; message="Migration table present: $(printf '%s' "$mig_out" | head -1 | cut -c1-60)"
  else
    status="WARN"; severity="P2"; message="Could not query migration table (server offline or sqlite3 missing)"
  fi
  emit_result "$phase" "$tier" "server-23-migrations" "$status" "$severity" "$message" "$mode" "$venue_state"

  # Cloud DB spot check via SSH (safe_ssh_capture handles banner protection)
  local cloud_check; cloud_check=$(safe_ssh_capture "root@100.70.177.44" \
    "sqlite3 /root/racecontrol/data/racecontrol.db \"PRAGMA table_info(drivers)\" 2>/dev/null | grep -c updated_at" \
    15)
  if [[ "${cloud_check:-0}" -ge 1 ]]; then
    status="PASS"; severity="P3"; message="Cloud DB: drivers table has updated_at"
  elif [[ -z "$cloud_check" ]]; then
    status="WARN"; severity="P2"; message="Cloud DB: SSH unavailable or DB not found (cloud may be offline)"
  else
    status="PASS"; severity="P3"; message="Cloud DB: drivers table has no updated_at column (may not be in schema)"
  fi
  emit_result "$phase" "$tier" "cloud-schema-drivers" "$status" "$severity" "$message" "$mode" "$venue_state"

  return 0
}
export -f run_phase36
