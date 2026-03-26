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
  local findings_count=0

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

    # Use findstr to count ERROR and PANIC lines in today's JSONL log
    # findstr /C:"string" counts lines containing the literal string
    local response
    response=$(safe_remote_exec "$pod_ip" 8090 \
      "findstr /C:\"ERROR\" /C:\"PANIC\" \"${today_log}\"" 15)

    local stdout
    stdout=$(printf '%s' "$response" | jq -r '.stdout // ""' 2>/dev/null)

    # Count matching lines
    local error_count
    if [[ -z "$stdout" ]]; then
      error_count=0
    else
      error_count=$(printf '%s' "$stdout" | grep -c . 2>/dev/null || echo "0")
    fi

    # Severity classification
    if [[ "$error_count" -gt 50 ]]; then
      _emit_finding "log_anomaly" "P1" "$pod_ip" \
        "log anomaly: ${error_count} ERROR/PANIC lines in today's log on ${pod_ip} (log=${today_log_name}, threshold=${threshold}, severity=critical)"
      findings_count=$((findings_count + 1))
    elif [[ "$error_count" -gt "$threshold" ]]; then
      _emit_finding "log_anomaly" "P2" "$pod_ip" \
        "log anomaly: ${error_count} ERROR/PANIC lines in today's log on ${pod_ip} (log=${today_log_name}, threshold=${threshold})"
      findings_count=$((findings_count + 1))
    fi

  done

  DETECTOR_FINDINGS=$((DETECTOR_FINDINGS + findings_count))
}
export -f detect_log_anomaly
