#!/usr/bin/env bash
# scripts/detectors/detect-flag-desync.sh — DET-05
#
# Detects feature flag desync between the server canonical set and all 8 pods.
# Queries GET /api/v1/flags on the server and each pod, compares enabled flag
# NAME sets only (not full objects).
#
# Pitfall 3: rc-agent :8090/api/v1/flags may not exist — tracks empty responses
# and emits fleet-level finding if all pods return empty.
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   RESULT_DIR, DETECTOR_FINDINGS, SERVER_URL, (log function from core.sh)
# Functions expected in scope: _emit_finding

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_flag_desync() {
  # Get canonical flag set from server
  local server_flags
  server_flags=$(curl -s --max-time 10 "${SERVER_URL}/api/v1/flags" 2>/dev/null || echo "[]")

  local canonical_enabled
  canonical_enabled=$(printf '%s' "$server_flags" | \
    jq -r '[.[] | select(.enabled==true) | .name] | sort | join(",")' 2>/dev/null || echo "")

  # If server is unreachable or returns empty, no baseline to compare against
  if [[ -z "$server_flags" ]] || [[ "$server_flags" == "[]" ]] || [[ -z "$canonical_enabled" ]]; then
    if [[ $(type -t log) == "function" ]]; then
      log INFO "DET-05: server /api/v1/flags unreachable or no enabled flags — skipping flag desync check"
    fi
    return 0
  fi

  local empty_count=0

  for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do

    # Query rc-agent flags cache on pod (Pitfall 3: endpoint may not exist)
    local pod_response
    pod_response=$(curl -s --max-time 10 "http://${pod_ip}:8090/api/v1/flags" 2>/dev/null || echo "")

    # If empty response, pod is offline or endpoint doesn't exist
    if [[ -z "$pod_response" ]] || [[ "$pod_response" == "[]" ]] || [[ "$pod_response" == "null" ]]; then
      empty_count=$((empty_count + 1))
      continue
    fi

    # Extract enabled flag names sorted and joined with comma
    local pod_enabled
    pod_enabled=$(printf '%s' "$pod_response" | \
      jq -r '[.[] | select(.enabled==true) | .name] | sort | join(",")' 2>/dev/null || echo "")

    # Compare with canonical enabled set
    if [[ "$pod_enabled" != "$canonical_enabled" ]]; then
      # Compute specific missing and extra flags for diagnostic detail
      local missing extra
      missing=$(comm -23 \
        <(printf '%s\n' "${canonical_enabled//,/$'\n'}" | sort) \
        <(printf '%s\n' "${pod_enabled//,/$'\n'}" | sort) \
        2>/dev/null | tr '\n' ',' | sed 's/,$//' || echo "")
      extra=$(comm -13 \
        <(printf '%s\n' "${canonical_enabled//,/$'\n'}" | sort) \
        <(printf '%s\n' "${pod_enabled//,/$'\n'}" | sort) \
        2>/dev/null | tr '\n' ',' | sed 's/,$//' || echo "")

      _emit_finding "flag_desync" "P2" "$pod_ip" \
        "flag desync on ${pod_ip}: missing=[${missing}] extra=[${extra}] vs server canonical"
      # HEAL-07: live-sync -- attempt heal immediately after detection
      if [[ $(type -t attempt_heal) == "function" ]]; then
        attempt_heal "$pod_ip" "flag_desync"
      fi
    fi

  done

  # Pitfall 3 awareness: if ALL pods returned empty but server has flags, the endpoint may not exist
  if [[ "$empty_count" -eq 8 ]] && [[ -n "$canonical_enabled" ]]; then
    _emit_finding "flag_desync" "P2" "fleet" \
      "all pods returned empty for /api/v1/flags -- endpoint may not exist on rc-agent (server has enabled flags: ${canonical_enabled})"
    # HEAL-07: live-sync -- fleet-level desync: no specific pod IP to heal, skip attempt_heal
  fi
}
export -f detect_flag_desync
