#!/usr/bin/env bash
# scripts/detectors/detect-config-drift.sh — DET-01
#
# Detects configuration drift on all 8 pods by reading rc-agent.toml via
# safe_remote_exec (rc-agent :8090). No SCP or SSH — avoids auth issues on pods.
#
# Checks:
#   - Banner corruption guard: first line must start with '['
#   - ws_connect_timeout: must be >= 600ms (incident: WS timeouts at 200ms caused flicker)
#   - pod_number key: must exist (missing = config loaded with defaults)
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   RESULT_DIR, DETECTOR_FINDINGS, (log function from core.sh)
# Functions expected in scope: _emit_finding, safe_remote_exec

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_config_drift() {
  for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do

    # Fetch rc-agent.toml content via safe_remote_exec
    # Pods run rc-agent (not racecontrol) — config is rc-agent.toml, not racecontrol.toml
    local response
    response=$(safe_remote_exec "$pod_ip" 8090 'type C:\RacingPoint\rc-agent.toml' 10)
    local content
    content=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)

    # If content is empty, pod is offline — skip (not a drift finding)
    if [[ -z "$content" ]]; then
      continue
    fi

    # Banner corruption guard: first line must start with '['
    local first_line
    first_line=$(printf '%s' "$content" | head -1)
    if ! printf '%s' "$first_line" | grep -q '^\['; then
      _emit_finding "config_drift" "P1" "$pod_ip" \
        "rc-agent.toml first line invalid -- likely banner corruption: ${first_line:0:60}"
      # HEAL-07: live-sync -- attempt heal immediately after detection (pod IP only)
      if [[ $(type -t attempt_heal) == "function" ]] && [[ "$pod_ip" =~ ^192\.168\. ]]; then
        attempt_heal "$pod_ip" "config_drift"
      fi
      continue
    fi

    # Check ws_connect_timeout (must be >= 600ms)
    # Incident history: threshold was 200ms, caused WS timeout flicker on all pods
    local ws_val
    ws_val=$(printf '%s' "$content" | grep -oE 'ws_connect_timeout[[:space:]]*=[[:space:]]*[0-9]+' | grep -oE '[0-9]+$' | head -1)
    if [[ -n "$ws_val" ]]; then
      if [[ "$ws_val" -lt 600 ]] 2>/dev/null; then
        _emit_finding "config_drift" "P1" "$pod_ip" \
          "ws_connect_timeout=${ws_val}ms on ${pod_ip} -- expected>=600ms (key=ws_connect_timeout, observed=${ws_val}, expected>=600)"
        # HEAL-07: live-sync -- attempt heal immediately after detection (pod IP only)
        if [[ $(type -t attempt_heal) == "function" ]] && [[ "$pod_ip" =~ ^192\.168\. ]]; then
          attempt_heal "$pod_ip" "config_drift"
        fi
      fi
    fi

    # Check pod_number key exists (missing = pod running with default config)
    if ! printf '%s' "$content" | grep -q 'pod_number'; then
      _emit_finding "config_drift" "P2" "$pod_ip" \
        "pod_number key missing from rc-agent.toml on ${pod_ip} -- pod may be using default config"
      # HEAL-07: live-sync -- attempt heal immediately after detection (pod IP only)
      if [[ $(type -t attempt_heal) == "function" ]] && [[ "$pod_ip" =~ ^192\.168\. ]]; then
        attempt_heal "$pod_ip" "config_drift"
      fi
    fi

    # Check app_health URL ports (SC-1: admin must be :3201, kiosk must use basePath)
    # Incident: kiosk at /api/health returned 404 (needs /kiosk/api/health due to basePath)
    local app_health_urls
    app_health_urls=$(printf '%s' "$content" | grep -i 'app_health' | grep -oE 'https?://[^[:space:]"]+' || true)
    if [[ -n "$app_health_urls" ]]; then
      # Admin must use port 3201 (not 3200)
      if printf '%s' "$app_health_urls" | grep -q ':3200.*admin'; then
        _emit_finding "config_drift" "P1" "$pod_ip" \
          "app_health admin URL uses wrong port :3200 on ${pod_ip} -- expected :3201 (key=app_health, observed=:3200, expected=:3201)"
        # HEAL-07: live-sync -- attempt heal immediately after detection (pod IP only)
        if [[ $(type -t attempt_heal) == "function" ]] && [[ "$pod_ip" =~ ^192\.168\. ]]; then
          attempt_heal "$pod_ip" "config_drift"
        fi
      fi
      # Kiosk must include /kiosk/ basePath
      if printf '%s' "$app_health_urls" | grep -q 'kiosk' && ! printf '%s' "$app_health_urls" | grep -q '/kiosk/api/health'; then
        _emit_finding "config_drift" "P1" "$pod_ip" \
          "app_health kiosk URL missing basePath on ${pod_ip} -- expected /kiosk/api/health (key=app_health)"
        # HEAL-07: live-sync -- attempt heal immediately after detection (pod IP only)
        if [[ $(type -t attempt_heal) == "function" ]] && [[ "$pod_ip" =~ ^192\.168\. ]]; then
          attempt_heal "$pod_ip" "config_drift"
        fi
      fi
    fi

  done

  # Note: DETECTOR_FINDINGS already incremented by _emit_finding() in cascade.sh
}
export -f detect_config_drift
