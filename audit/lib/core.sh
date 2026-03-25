#!/usr/bin/env bash
# audit/lib/core.sh — Shared primitives for Racing Point audit framework
#
# Standing rule mitigations embedded at primitive layer:
#   - curl output stripping: health endpoints return "200" with quotes — always strip with tr -d or jq -r
#   - cmd.exe quoting: JSON payloads written to temp file, use curl -d @file (never inline JSON)
#   - SSH banner corruption: 2>/dev/null + first-line banner validation
#   - AUDIT_PIN: always from env var, never hardcoded
#   - All timestamps: IST (UTC+5:30) via TZ=Asia/Kolkata
#
# Usage: source this file in audit.sh or any phase script
# All 8 functions exported for use in subshells and background jobs

# ---------------------------------------------------------------------------
# FUNCTION 1 — ist_now
# Returns current time as ISO 8601 string in IST (UTC+5:30)
# ---------------------------------------------------------------------------
ist_now() {
  TZ=Asia/Kolkata date '+%Y-%m-%dT%H:%M:%S+05:30'
}
export -f ist_now

# ---------------------------------------------------------------------------
# FUNCTION 2 — http_get (url [timeout_secs])
# Fetch URL via curl; strips surrounding double-quotes from response.
# Standing rule: health endpoints return "200" (with quotes) — tr -d removes them.
# ---------------------------------------------------------------------------
http_get() {
  local url=$1
  local timeout=${2:-${DEFAULT_TIMEOUT:-10}}
  curl -s -m "$timeout" "$url" 2>/dev/null | tr -d '"\r'
}
export -f http_get

# ---------------------------------------------------------------------------
# FUNCTION 3 — emit_result (phase tier host status severity message mode venue_state)
# Write 9-field JSON result to ${RESULT_DIR}/phase-${phase}-${host}.json
# Fields: phase, tier, host, status, severity, message, mode, venue_state, timestamp
# ---------------------------------------------------------------------------
emit_result() {
  local phase=$1 tier=$2 host=$3 status=$4 severity=$5 message=$6 mode=$7 venue_state=$8
  local ts; ts=$(ist_now)
  mkdir -p "${RESULT_DIR:-/tmp/audit-fallback}"
  local outfile="${RESULT_DIR:-/tmp/audit-fallback}/phase-${phase}-${host}.json"
  jq -n \
    --arg phase       "$phase"       \
    --arg tier        "$tier"        \
    --arg host        "$host"        \
    --arg status      "$status"      \
    --arg severity    "$severity"    \
    --arg message     "$message"     \
    --arg mode        "$mode"        \
    --arg venue_state "$venue_state" \
    --arg timestamp   "$ts"          \
    '{phase:$phase,tier:$tier,host:$host,status:$status,severity:$severity,message:$message,mode:$mode,venue_state:$venue_state,timestamp:$timestamp}' \
    > "$outfile"
}
export -f emit_result

# ---------------------------------------------------------------------------
# FUNCTION 4 — emit_fix (phase host action before_state after_state)
# Append fix record to ${RESULT_DIR}/fixes.jsonl for audit trail.
# ---------------------------------------------------------------------------
emit_fix() {
  local phase=$1 host=$2 action=$3 before_state=$4 after_state=$5
  local ts; ts=$(ist_now)
  mkdir -p "${RESULT_DIR:-/tmp/audit-fallback}"
  local fixfile="${RESULT_DIR:-/tmp/audit-fallback}/fixes.jsonl"
  jq -n \
    --arg phase       "$phase"       \
    --arg host        "$host"        \
    --arg action      "$action"      \
    --arg before      "$before_state" \
    --arg after       "$after_state"  \
    --arg timestamp   "$ts"          \
    '{phase:$phase,host:$host,action:$action,before:$before,after:$after,timestamp:$timestamp}' \
    >> "$fixfile"
}
export -f emit_fix

# ---------------------------------------------------------------------------
# FUNCTION 5 — safe_remote_exec (host [port] cmd [timeout_secs])
# Run cmd on remote pod via rc-agent POST /exec.
# CRITICAL standing rule: write JSON payload to temp file, use curl -d @file.
# NEVER inline JSON — cmd.exe quoting in /exec will mangle strings with spaces or special chars.
# ---------------------------------------------------------------------------
safe_remote_exec() {
  local host=$1 port=${2:-8090} cmd=$3 timeout=${4:-${DEFAULT_TIMEOUT:-10}}
  local tmpfile; tmpfile=$(mktemp)
  jq -n --arg cmd "$cmd" '{cmd: $cmd}' > "$tmpfile"
  local result
  result=$(curl -s -m "$timeout" -X POST "http://${host}:${port}/exec" \
    -H 'Content-Type: application/json' -d "@${tmpfile}" 2>/dev/null)
  rm -f "$tmpfile"
  # Strip \r from Windows HTTP responses to prevent arithmetic errors downstream
  printf '%s' "${result:-{}}" | tr -d '\r'
}
export -f safe_remote_exec

# ---------------------------------------------------------------------------
# FUNCTION 6 — safe_ssh_capture (user_at_host command [timeout_secs])
# Run command via SSH with protection against SSH banner corruption.
# CRITICAL standing rule: 2>/dev/null + StrictHostKeyChecking=no + validate first line.
# Returns empty string and exit 1 if banner detected in output.
# ---------------------------------------------------------------------------
safe_ssh_capture() {
  local host=$1 cmd=$2 timeout=${3:-${DEFAULT_TIMEOUT:-10}}
  local raw
  raw=$(ssh -o StrictHostKeyChecking=no -o ConnectTimeout="$timeout" -o BatchMode=yes \
    "$host" "$cmd" 2>/dev/null)
  local first_line; first_line=$(printf '%s' "$raw" | head -1)
  if printf '%s' "$first_line" | grep -qiE 'warning|ecdsa|ed25519|post.quantum|motd|welcome|last login'; then
    printf ''; return 1
  fi
  printf '%s' "$raw"
}
export -f safe_ssh_capture

# ---------------------------------------------------------------------------
# FUNCTION 7 — get_session_token
# Obtain auth token from /api/v1/terminal/auth.
# CRITICAL: reads PIN from AUDIT_PIN env var — never hardcoded.
# Returns empty string on any failure; does NOT exit.
# ---------------------------------------------------------------------------
get_session_token() {
  local pin="${AUDIT_PIN:-}"
  if [[ -z "$pin" ]]; then printf ''; return 1; fi
  local tmpfile; tmpfile=$(mktemp)
  jq -n --arg pin "$pin" '{pin: $pin}' > "$tmpfile"
  local response
  response=$(curl -s -m 10 -X POST \
    "${AUTH_ENDPOINT:-http://192.168.31.23:8080/api/v1/terminal/auth}" \
    -H 'Content-Type: application/json' -d "@${tmpfile}" 2>/dev/null)
  rm -f "$tmpfile"
  printf '%s' "$response" | jq -r '.session // empty' 2>/dev/null || printf ''
}
export -f get_session_token

# ---------------------------------------------------------------------------
# FUNCTION 8 — venue_state_detect
# Returns "open" or "closed".
# Check fleet health API first (any active billing session = open),
# then fall back to IST time window (09:00-22:00 = open).
# ---------------------------------------------------------------------------
venue_state_detect() {
  local fleet_response
  fleet_response=$(curl -s -m 8 \
    "${FLEET_HEALTH_ENDPOINT:-http://192.168.31.23:8080/api/v1/fleet/health}" 2>/dev/null)
  if [[ -n "$fleet_response" ]]; then
    local active
    active=$(printf '%s' "$fleet_response" | \
      jq -r '[.[] | select(.active_billing_session==true or .billing_active==true)] | length' \
      2>/dev/null || printf '0')
    if [[ "${active:-0}" -gt 0 ]]; then printf 'open'; return 0; fi
  fi
  local hour; hour=$(TZ=Asia/Kolkata date '+%H' | sed 's/^0*//')
  hour=${hour:-0}
  if [[ "$hour" -ge 9 && "$hour" -lt 22 ]]; then printf 'open'; else printf 'closed'; fi
}
export -f venue_state_detect
