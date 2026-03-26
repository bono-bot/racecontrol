#!/usr/bin/env bash
# scripts/detectors/detect-log-anomaly.sh — DET-03
#
# Detects ERROR/PANIC log anomalies in today's rc-agent JSONL log on each pod.
#
# Thresholds (venue-state-aware, per requirements):
#   - Venue OPEN:   > 10 ERROR/PANIC lines in today's log = P2; > 50 = P1
#   - Venue CLOSED: >  2 ERROR/PANIC lines in today's log = P2; > 50 = P1
#
# Log filename: rc-agent-YYYY-MM-DD.jsonl (UTC date — Pitfall 4: logs are UTC)
#   This runs during nightly audit (~02:30 IST = ~21:00 UTC prior day).
#   Using UTC date ensures correct filename match.
#
# NOTE: Rate-based threshold (errors per hour) deferred — requires 7-day calibration.
#   Full-day count is used as conservative approximation.
#
# Env vars inherited from auto-detect.sh / cascade.sh:
#   RESULT_DIR, DETECTOR_FINDINGS, DETECTED_VENUE_STATE
# Functions expected in scope: _emit_finding, safe_remote_exec

set -u
set -o pipefail
# NO set -e — errors are encoded in findings, not exit codes

detect_log_anomaly() {
  # Venue-state-aware threshold selection
  # DETECTED_VENUE_STATE is set by auto-detect.sh before step 4
  local threshold
  if [[ "${DETECTED_VENUE_STATE:-closed}" == "open" ]]; then
    threshold=10
  else
    threshold=2
  fi

  # Pitfall 4: log filenames use UTC date (not IST)
  # rc-agent rolls logs at UTC midnight, not IST midnight
  local today_log_name
  today_log_name="rc-agent-$(date -u '+%Y-%m-%d').jsonl"
  local today_log="C:\\RacingPoint\\${today_log_name}"

  for pod_ip in 192.168.31.89 192.168.31.33 192.168.31.28 192.168.31.88 192.168.31.86 192.168.31.87 192.168.31.38 192.168.31.91; do

    # Use findstr to get ERROR and PANIC lines in today's JSONL log
    # findstr /C:"string" returns lines containing the literal string
    local response
    response=$(safe_remote_exec "$pod_ip" 8090 \
      "findstr /C:\"ERROR\" /C:\"PANIC\" \"${today_log}\"" 15)

    local stdout
    stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)

    # Filter to last 60 minutes only (SC-3: "in the last hour", not full day)
    # JSONL entries have ISO timestamps like "2026-03-26T21:15:00Z"
    # Compare against UTC cutoff = now - 3600 seconds
    local cutoff_ts
    cutoff_ts=$(date -u -d '1 hour ago' '+%Y-%m-%dT%H:%M:%S' 2>/dev/null || date -u '+%Y-%m-%dT%H:%M:%S')

    local error_count=0
    if [[ -n "$stdout" ]]; then
      # Each JSONL line has a timestamp field — filter lines with timestamp >= cutoff
      while IFS= read -r line; do
        # Extract timestamp from JSONL line (format: "timestamp":"2026-03-26T21:15:00Z" or similar)
        local line_ts
        line_ts=$(printf '%s' "$line" | grep -oP '"timestamp"\s*:\s*"\K[^"]+' 2>/dev/null | head -1)
        if [[ -z "$line_ts" ]]; then
          # If no parseable timestamp, count it conservatively
          error_count=$((error_count + 1))
          continue
        fi
        # Strip trailing Z and compare lexicographically (ISO 8601 sorts correctly)
        line_ts="${line_ts%Z}"
        if [[ "$line_ts" > "$cutoff_ts" || "$line_ts" == "$cutoff_ts" ]]; then
          error_count=$((error_count + 1))
        fi
      done <<< "$stdout"
    fi

    # Severity classification
    if [[ "$error_count" -gt 50 ]]; then
      _emit_finding "log_anomaly" "P1" "$pod_ip" \
        "log anomaly: ${error_count} ERROR/PANIC lines in last hour on ${pod_ip} (log=${today_log_name}, threshold=${threshold}, severity=critical)"
      # HEAL-07: live-sync -- attempt heal immediately after detection
      if [[ $(type -t attempt_heal) == "function" ]]; then
        attempt_heal "$pod_ip" "log_anomaly"
      fi
    elif [[ "$error_count" -gt "$threshold" ]]; then
      _emit_finding "log_anomaly" "P2" "$pod_ip" \
        "log anomaly: ${error_count} ERROR/PANIC lines in last hour on ${pod_ip} (log=${today_log_name}, threshold=${threshold})"
      # HEAL-07: live-sync -- attempt heal immediately after detection
      if [[ $(type -t attempt_heal) == "function" ]]; then
        attempt_heal "$pod_ip" "log_anomaly"
      fi
    fi

  done

  # Note: DETECTOR_FINDINGS already incremented by _emit_finding() in cascade.sh
}
export -f detect_log_anomaly
