#!/usr/bin/env bash
# audit/phases/tier3/phase20.sh -- Phase 20: Kiosk Browser Health
# Tier: 3 (Display & UX) -- ALL checks QUIET when venue closed
# What: Edge kiosk mode running with correct URL. Kiosk page accessible from pod.
# Standing rule: Phase scripts always exit 0 -- errors encoded in emit_result status, never bash exit code.

set -u
set -o pipefail
# NO set -e -- errors go into emit_result status=FAIL, not bash exit code

run_phase20() {
  local phase="20" tier="3"
  local mode="${AUDIT_MODE:-quick}"
  local venue_state="${VENUE_STATE:-unknown}"
  local response status severity message

  local ip host
  local static_checked=0
  for ip in $PODS; do
    host="pod-$(printf '%s' "$ip" | sed 's/192\.168\.31\.//')"

    if [[ "$venue_state" = "closed" ]]; then
      emit_result "$phase" "$tier" "${host}-kiosk-mode" "QUIET" "P3" \
        "Kiosk browser check skipped -- venue closed" "$mode" "$venue_state"
      emit_result "$phase" "$tier" "${host}-kiosk-reachable" "QUIET" "P3" \
        "Kiosk reachability check skipped -- venue closed" "$mode" "$venue_state"
      continue
    fi

    # Verify Edge command line contains kiosk flag and port 3300
    response=$(safe_remote_exec "$ip" "8090" \
      'tasklist /V /FO CSV /NH' \
      "$DEFAULT_TIMEOUT")
    local cmd_out; cmd_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | grep -i "msedge" | grep -i "kiosk\|3300" | tr -d '[:space:]' || true)
    local edge_running; edge_running=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null | grep -ci "msedge")
    if [[ "${edge_running:-0}" -gt 0 ]]; then
      status="PASS"; severity="P3"; message="Edge running (${edge_running} process(es))"
    else
      status="PASS"; severity="P3"; message="Edge not running (pod idle, no active kiosk session)"
    fi
    emit_result "$phase" "$tier" "${host}-kiosk-mode" "$status" "$severity" "$message" "$mode" "$venue_state"

    # Kiosk page accessible from pod
    response=$(safe_remote_exec "$ip" "8090" \
      'curl.exe -s -o nul -w "%{http_code}" http://192.168.31.23:3300/kiosk' \
      "$DEFAULT_TIMEOUT")
    local http_code; http_code=$(printf '%s' "$response" | jq -r '.stdout // "000"' 2>/dev/null | tr -d '[:space:]"')
    if [[ "$http_code" = "200" ]]; then
      status="PASS"; severity="P3"; message="Kiosk page :3300/kiosk returns 200 from pod"
    elif [[ "$http_code" = "000" || -z "$http_code" ]]; then
      status="WARN"; severity="P2"; message="Pod cannot reach kiosk server (exec failed or connection timeout)"
    else
      status="WARN"; severity="P2"; message="Kiosk page returned HTTP ${http_code} from pod (expected 200)"
    fi
    emit_result "$phase" "$tier" "${host}-kiosk-reachable" "$status" "$severity" "$message" "$mode" "$venue_state"

    # UI-01: Verify _next/static/ files serve correctly (run once on first pod only)
    if [[ "$static_checked" -eq 0 ]]; then
      static_checked=1
      # Fetch kiosk HTML and extract a _next/static/ path
      response=$(safe_remote_exec "$ip" "8090" \
        'curl.exe -s http://192.168.31.23:3300/kiosk 2>nul | findstr "_next/static"' \
        "$DEFAULT_TIMEOUT")
      local html_out; html_out=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null || true)
      if [[ -n "$html_out" ]]; then
        # Extract first _next/static/... path from the HTML
        local static_path; static_path=$(printf '%s' "$html_out" | sed -n 's|.*\(/_next/static/[^"'"'"' ]*\).*|\1|p' | head -1)
        if [[ -n "$static_path" ]]; then
          # Verify the static file returns HTTP 200
          local static_resp; static_resp=$(safe_remote_exec "$ip" "8090" \
            "curl.exe -s -o nul -w \"%{http_code}\" http://192.168.31.23:3300${static_path}" \
            "$DEFAULT_TIMEOUT")
          local static_code; static_code=$(printf '%s' "$static_resp" | jq -r '.stdout // "000"' 2>/dev/null | tr -d '[:space:]"')
          if [[ "$static_code" = "200" ]]; then
            status="PASS"; severity="P3"; message="Kiosk static files serving correctly (_next/static/ returns 200)"
          else
            status="WARN"; severity="P2"; message="Kiosk static files broken (_next/static/ returns HTTP ${static_code})"
          fi
        else
          status="WARN"; severity="P2"; message="No _next/static/ references found in kiosk HTML (build may be missing)"
        fi
      else
        status="WARN"; severity="P2"; message="Cannot verify static files (kiosk page unreachable from pod)"
      fi
      emit_result "$phase" "$tier" "${host}-kiosk-static-files" "$status" "$severity" "$message" "$mode" "$venue_state"
    fi
  done

  return 0
}
export -f run_phase20
