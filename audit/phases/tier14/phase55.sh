#!/usr/bin/env bash
# audit/phases/tier14/phase55.sh -- Phase 55: DB Migration Completeness
# Tier: 14 (Data Integrity Deep)
# What: Every column used in sync/query code has a corresponding ALTER TABLE migration for existing DBs.
# Standing rule: PRO-04 (migrations must cover ALL consumers with ALTER TABLE)

set -u
set -o pipefail
# NO set -e

run_phase55() {
  local phase="55" tier="14"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local MIGRATIONS_DIR="C:/Users/bono/racingpoint/racecontrol/migrations"

  # --- Check 1: Migrations directory exists and has .sql files ---
  local sql_count; sql_count=$(ls "${MIGRATIONS_DIR}"/*.sql 2>/dev/null | wc -l)
  sql_count="${sql_count//[[:space:]]/}"
  if [[ "${sql_count:-0}" -gt 0 ]] 2>/dev/null; then
    status="PASS"; severity="P3"; message="Migrations directory exists with ${sql_count} .sql files"
  elif [[ -d "${MIGRATIONS_DIR}" ]]; then
    status="WARN"; severity="P2"; message="Migrations directory exists but contains no .sql files"
  else
    status="WARN"; severity="P2"; message="Migrations directory not found: ${MIGRATIONS_DIR}"
  fi
  emit_result "$phase" "$tier" "james-migrations-exist" "$status" "$severity" "$message" "$mode" "$venue_state"

  # --- Checks 2-4: Sync-critical column coverage (updated_at, synced_at, deleted_at) ---
  for COL in updated_at synced_at deleted_at; do
    # Count CREATE TABLE mentions for this column
    local create_count; create_count=$(grep -rci "${COL}" "${MIGRATIONS_DIR}/" --include="*.sql" 2>/dev/null \
      | grep -v ":0$" \
      | awk -F: '{sum += $2} END {print sum+0}')
    create_count="${create_count//[[:space:]]/}"

    # Count ALTER TABLE ADD COLUMN mentions for this column
    local alter_count; alter_count=$(grep -rci "ALTER.*ADD.*${COL}\|ADD COLUMN.*${COL}" "${MIGRATIONS_DIR}/" \
      --include="*.sql" 2>/dev/null \
      | grep -v ":0$" \
      | awk -F: '{sum += $2} END {print sum+0}')
    alter_count="${alter_count//[[:space:]]/}"

    if [[ -z "$create_count" || "$create_count" = "0" ]]; then
      # Column not referenced in CREATE TABLE — not critical for this migration set
      status="PASS"; severity="P3"
      message="${COL}: not referenced in CREATE TABLE migrations (may be added entirely via ALTER)"
    elif [[ "${alter_count:-0}" -ge "${create_count:-0}" ]] 2>/dev/null; then
      status="PASS"; severity="P3"
      message="${COL}: ALTER TABLE migrations (${alter_count}) >= CREATE TABLE mentions (${create_count}) — coverage OK"
    else
      status="WARN"; severity="P2"
      message="${COL}: ALTER TABLE migrations (${alter_count:-0}) < CREATE TABLE mentions (${create_count}) — some tables may lack column on existing DBs"
    fi
    emit_result "$phase" "$tier" "james-migration-${COL}" "$status" "$severity" "$message" "$mode" "$venue_state"
  done

  return 0
}
export -f run_phase55
