#!/usr/bin/env bash
# scripts/detectors/detect-schema-gap.sh — DET-06
#
# Detects schema drift between the venue DB (server .23) and the cloud DB (Bono VPS).
# Checks a set of critical table:column pairs known to have been added via ALTER TABLE
# in db/mod.rs migrations.
#
# Method: SELECT <column> FROM <table> LIMIT 1 — if stderr contains "no such column",
# the migration was not applied to that DB.
#
# Venue DB: checked via safe_remote_exec to server :8090 (sqlite3.exe on server .23)
# Cloud DB: checked via SSH to root@100.70.177.44 (relay custom_command not supported)
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   RESULT_DIR, DETECTOR_FINDINGS, (log function from core.sh)
# Functions expected in scope: _emit_finding, safe_remote_exec

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_schema_gap() {
  # Critical table:column pairs from known ALTER TABLE migrations in db/mod.rs
  local -a SCHEMA_CHECKS=(
    "drivers:updated_at"
    "drivers:membership_type"
    "billing_sessions:payment_method"
    "billing_sessions:staff_discount"
    "feature_flags:description"
    "game_catalog:category"
  )

  for check in "${SCHEMA_CHECKS[@]}"; do
    local table column
    table="${check%%:*}"
    column="${check##*:}"

    # ── Venue DB check (server .23 via safe_remote_exec :8090) ──────────────
    local venue_result venue_stderr venue_exit venue_has_col
    venue_result=$(safe_remote_exec "192.168.31.23" 8090 \
      "sqlite3.exe C:\\RacingPoint\\racecontrol.db \"SELECT ${column} FROM ${table} LIMIT 1\"" 10)

    venue_stderr=$(printf '%s' "$venue_result" | jq -r '.stderr // ""' 2>/dev/null)
    venue_exit=$(printf '%s' "$venue_result" | jq -r '.exitCode // 0' 2>/dev/null)
    venue_has_col="true"

    if printf '%s' "$venue_stderr" | grep -qi "no such column"; then
      venue_has_col="false"
    fi
    # If safe_remote_exec itself failed (empty result), treat as unknown
    if [[ -z "$venue_result" ]]; then
      venue_has_col="unknown"
    fi

    # ── Cloud DB check (Bono VPS via SSH — relay custom_command not supported) ──
    local cloud_stderr cloud_has_col
    cloud_has_col="true"
    cloud_stderr=$(ssh -o StrictHostKeyChecking=no -o ConnectTimeout=10 -o BatchMode=yes \
      root@100.70.177.44 \
      "sqlite3 /root/racecontrol/racecontrol.db 'SELECT ${column} FROM ${table} LIMIT 1'" \
      2>&1 || echo "SSH_ERROR")

    if printf '%s' "$cloud_stderr" | grep -qi "no such column"; then
      cloud_has_col="false"
    fi
    # Connection errors or SSH failure — treat as unknown (not a false schema gap)
    if printf '%s' "$cloud_stderr" | grep -qi "unable to open\|Connection refused\|Connection timed out\|No route to host\|SSH_ERROR\|ssh: connect"; then
      cloud_has_col="unknown"
    fi

    # ── Compare results ──────────────────────────────────────────────────────
    # Skip if either side is unknown (unreachable, not a schema gap)
    if [[ "$venue_has_col" == "unknown" ]] || [[ "$cloud_has_col" == "unknown" ]]; then
      continue
    fi

    if [[ "$venue_has_col" == "true" ]] && [[ "$cloud_has_col" == "false" ]]; then
      _emit_finding "schema_gap" "P2" "cloud" \
        "schema gap: cloud DB missing ${table}.${column} (venue has it)"
    elif [[ "$venue_has_col" == "false" ]] && [[ "$cloud_has_col" == "true" ]]; then
      _emit_finding "schema_gap" "P2" "venue" \
        "schema gap: venue DB missing ${table}.${column} (cloud has it)"
    elif [[ "$venue_has_col" == "false" ]] && [[ "$cloud_has_col" == "false" ]]; then
      _emit_finding "schema_gap" "P2" "fleet" \
        "schema gap: both DBs missing ${table}.${column} -- migration not applied anywhere"
    fi

  done
}
export -f detect_schema_gap
