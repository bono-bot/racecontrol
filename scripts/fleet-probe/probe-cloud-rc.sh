#!/bin/bash
# scripts/fleet-probe/probe-cloud-rc.sh -- Phase 448 Plan 06
# Probes racingpoint.cloud (Bono VPS racecontrol :8080 via pm2) via HTTPS /api/v1/health.
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-cloud-rc.sh
# Optional env:
#   PROBE_OVERRIDE_CLOUD_RC_URL  -- overrides https://racingpoint.cloud (tests: http://127.0.0.1:PORT)
#   PROBE_PYTHON                 -- override python3 interpreter (tests pass 'python' on Windows)
# Stdout: {"target_id":"cloud_racecontrol","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
# Side effect: state/fleet-manifest/$MANIFEST_TS/cloud_racecontrol.json
#
# NOTE ON PYTHON INVOCATION:
# All python3 JSON helpers delegate to lib/cloud_rc_helpers.py via sys.argv.
# This avoids heredoc-stdin conflicts on Windows/Git Bash where any heredoc ('<<PYEOF')
# hangs when bash's stdin is a pipe (e.g. under Node.js spawn in unit tests).
# Default target: https://racingpoint.cloud/api/v1/health
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

PY_HELPERS="$SCRIPT_DIR/lib/cloud_rc_helpers.py"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-cloud-rc: MANIFEST_TS not set" >&2
  exit 2
fi

# PROBE_PYTHON: allows tests on Windows to override 'python3' with real interpreter path
PYTHON="${PROBE_PYTHON:-python3}"
# Default target: https://racingpoint.cloud/api/v1/health
CLOUD_URL="${PROBE_OVERRIDE_CLOUD_RC_URL:-https://racingpoint.cloud}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$PYTHON" -c "import time; print(int(time.time()*1000))")

TARGET_ID="cloud_racecontrol"
HOST_VAL="racingpoint.cloud"
IP_VAL="45.11.110.250"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- Error append helper ---
# append_error SUB_PROBE ERROR_MSG
append_error() {
  local sp="$1" err="$2"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" PE="$PROBE_ERRORS_JSON" "$PYTHON" -c '
import os, json
a = json.loads(os.environ["PE"])
a.append({"sub_probe": os.environ["SP"], "error": os.environ["ERR"]})
print(json.dumps(a))
')
}

# --- GET /api/v1/health ---
HEALTH_RESP_FILE="$WORK_DIR/health-resp.txt"
HEALTH_CODE=$(curl -s --max-time 10 \
  -o "$HEALTH_RESP_FILE" \
  -w "%{http_code}" \
  "$CLOUD_URL/api/v1/health" 2>/dev/null) || HEALTH_CODE="000"

BUILD_ID="null"

if [ -z "$HEALTH_CODE" ] || [ "$HEALTH_CODE" = "000" ]; then
  CONNECT_ERR=1
  append_error "connectivity" "cannot reach $CLOUD_URL/api/v1/health (DNS/TLS/connect failure)"
elif [ "$HEALTH_CODE" != "200" ]; then
  CONNECT_ERR=1
  append_error "health" "HTTP $HEALTH_CODE from $CLOUD_URL/api/v1/health"
else
  # Validate JSON -- reads file path via sys.argv, no stdin
  if ! "$PYTHON" "$PY_HELPERS" validate_json "$HEALTH_RESP_FILE" 2>/dev/null; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "health_parse" "cloud_racecontrol /api/v1/health returned non-JSON"
  else
    # Extract build_id -- reads file path via sys.argv, no stdin
    BID=$("$PYTHON" "$PY_HELPERS" extract_bid "$HEALTH_RESP_FILE" 2>/dev/null || true)

    if [ -n "$BID" ]; then
      BUILD_ID=$("$PYTHON" -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/api/v1/health response missing build_id field"
    fi
  fi
fi

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$PYTHON" -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# Stable empty SHA256 (no local binary to hash for cloud racecontrol)
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

# On probe_failed: zero out data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  BUILD_ID="null"
fi

# --- Assemble manifest -- no stdin/heredoc, all args via sys.argv ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
"$PYTHON" "$PY_HELPERS" build_manifest \
  "$TARGET_ID" "$HOST_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$BUILD_ID" "$ENV_HASH" \
  "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE"

MANIFEST_JSON=$(cat "$MANIFEST_FILE")
write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
