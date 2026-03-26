#!/usr/bin/env bash
# scripts/detectors/detect-crash-loop.sh — DET-04
#
# Detects crash loops on all 8 pods by counting rc-agent startup events in
# the JSONL log within the last 30 minutes. Uses UTC-dated JSONL file (Pitfall 4).
#
# Threshold: >3 restarts in 30 minutes = crash loop.
#
# NOTE: Does NOT use rc-agent-startup.log — it is truncated on each startup
# (Pitfall 1). Uses rc-agent-YYYY-MM-DD.jsonl which is append-only.
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   RESULT_DIR, DETECTOR_FINDINGS, (log function from core.sh)
# Functions expected in scope: _emit_finding, safe_remote_exec

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_crash_loop() {
  local max_restarts=3
  local window_minutes=30

  # Compute UTC cutoff timestamp (ISO 8601 prefix for string comparison)
  # Supports both GNU date (Git Bash) and BSD date (macOS)
  local cutoff_ts
  cutoff_ts=$(date -u -d "30 minutes ago" '+%Y-%m-%dT%H:%M' 2>/dev/null \
    || date -u -v-30M '+%Y-%m-%dT%H:%M' 2>/dev/null \
    || echo "")

  # UTC-dated JSONL log filename (Pitfall 4: logs roll at UTC midnight, not IST)
  local today_log
  today_log="C:\\RacingPoint\\rc-agent-$(date -u '+%Y-%m-%d').jsonl"

  for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do

    # Search JSONL log for startup indicator lines
    # rc-agent logs "config_loaded", "started", or "listening on" once per startup cycle
    local response
    response=$(safe_remote_exec "$pod_ip" 8090 \
      "findstr /C:\"config_loaded\" /C:\"listening on\" /C:\"started\" \"${today_log}\"" 15)

    local stdout
    stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)

    # If empty stdout, pod is offline or log doesn't exist — skip
    if [[ -z "$stdout" ]]; then
      continue
    fi

    # Count startup events within the time window
    local restart_count=0

    if [[ -n "$cutoff_ts" ]]; then
      # Extract timestamp from each JSONL line and compare with cutoff (ISO 8601 sorts lexicographically)
      while IFS= read -r line; do
        if [[ -z "$line" ]]; then continue; fi
        # Extract "timestamp":"YYYY-MM-DDTHH:MM..." field (first 16 chars for HH:MM comparison)
        local ts
        ts=$(printf '%s' "$line" | grep -oE '"timestamp"[[:space:]]*:[[:space:]]*"[^"]+"' 2>/dev/null | head -1 | grep -oE '[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}' | head -1)
        if [[ -n "$ts" ]] && [[ "$ts" > "$cutoff_ts" || "$ts" == "$cutoff_ts" ]]; then
          restart_count=$((restart_count + 1))
        fi
      done <<< "$stdout"
    else
      # Cutoff computation failed — conservatively count all matching lines
      restart_count=$(printf '%s' "$stdout" | grep -c '.' 2>/dev/null || echo "0")
    fi

    if [[ "$restart_count" -gt "$max_restarts" ]]; then
      _emit_finding "crash_loop" "P1" "$pod_ip" \
        "crash loop: ${restart_count} restarts in last ${window_minutes}min on ${pod_ip} (threshold=${max_restarts})"
      # HEAL-07: live-sync -- attempt heal immediately after detection
      if [[ $(type -t attempt_heal) == "function" ]]; then
        attempt_heal "$pod_ip" "crash_loop"
      fi
    fi

  done
}
export -f detect_crash_loop
