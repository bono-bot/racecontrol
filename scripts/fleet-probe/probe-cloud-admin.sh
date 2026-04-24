#!/bin/bash
# scripts/fleet-probe/probe-cloud-admin.sh -- Phase 448 Plan 06
# Probes admin.racingpoint.cloud via HTTPS /api/health + GET / gate detection.
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-cloud-admin.sh
# Optional env:
#   PROBE_OVERRIDE_CLOUD_ADMIN_URL  -- overrides https://admin.racingpoint.cloud (tests: http://127.0.0.1:PORT)
#   STAFF_JWT                        -- optional; enables gated-page probe
# Stdout: {"target_id":"cloud_admin","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
# Side effect: state/fleet-manifest/$MANIFEST_TS/cloud_admin.json
#
# NOTE ON PYTHON INVOCATION:
# All python3 JSON helpers delegate to lib/cloud_admin_helpers.py via sys.argv.
# This avoids heredoc-stdin conflicts on Windows/Git Bash where ANY heredoc ('<<PYEOF')
# hangs when bash's stdin is a pipe (e.g. under Node.js spawn in unit tests).
# Default target: https://admin.racingpoint.cloud/api/health
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

PY_HELPERS="$SCRIPT_DIR/lib/cloud_admin_helpers.py"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-cloud-admin: MANIFEST_TS not set" >&2
  exit 2
fi

CLOUD_URL="${PROBE_OVERRIDE_CLOUD_ADMIN_URL:-https://admin.racingpoint.cloud}"
# PROBE_PYTHON: allows tests on Windows to override 'python3' with real interpreter path
PYTHON="${PROBE_PYTHON:-python3}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$PYTHON" -c "import time; print(int(time.time()*1000))")

TARGET_ID="cloud_admin"
HOST_VAL="admin.racingpoint.cloud"
IP_VAL="45.11.110.250"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- Error append helper ---
# append_error SUB_PROBE ERROR_MSG [EXTRA_KEY EXTRA_VAL]
append_error() {
  local sp="$1" err="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" "$PYTHON" -c '
import os, json
a = json.loads(os.environ["PE"])
e = {"sub_probe": os.environ["SP"], "error": os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]] = os.environ["XV"]
a.append(e)
print(json.dumps(a))
')
}

# --- /api/health probe ---
HEALTH_RESP_FILE="$WORK_DIR/health-resp.txt"
HEALTH_CODE=$(curl -s --max-time 10 \
  -o "$HEALTH_RESP_FILE" \
  -w "%{http_code}" \
  "$CLOUD_URL/api/health" 2>/dev/null) || HEALTH_CODE="000"

BUILD_ID="null"
GIT_COMMIT=""
PAGES_MISSING_JSON="[]"

if [ -z "$HEALTH_CODE" ] || [ "$HEALTH_CODE" = "000" ]; then
  CONNECT_ERR=1
  append_error "connectivity" "cannot reach $CLOUD_URL/api/health (DNS/TLS/connect failure)"
elif [ "$HEALTH_CODE" != "200" ]; then
  CONNECT_ERR=1
  append_error "health" "HTTP $HEALTH_CODE from $CLOUD_URL/api/health"
else
  # Validate JSON -- no stdin/heredoc, reads file path via sys.argv
  if ! "$PYTHON" "$PY_HELPERS" validate_json "$HEALTH_RESP_FILE" 2>/dev/null; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "health_parse" "cloud admin /api/health returned non-JSON"
  else
    # Extract build_id -- no stdin/heredoc, reads file path via sys.argv
    BID=$("$PYTHON" "$PY_HELPERS" extract_field "$HEALTH_RESP_FILE" "build_id" 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$("$PYTHON" -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    fi

    # Extract git_commit -- no stdin/heredoc, reads file path via sys.argv
    GIT_COMMIT=$("$PYTHON" "$PY_HELPERS" extract_field "$HEALTH_RESP_FILE" "git_commit" 2>/dev/null || true)

    # Extract pages_missing -- no stdin/heredoc, reads file path via sys.argv
    PAGES_MISSING_JSON=$("$PYTHON" "$PY_HELPERS" extract_pages "$HEALTH_RESP_FILE" 2>/dev/null || echo "[]")

    # pages_missing non-empty -> partial + pages_probe error
    PM_COUNT=$("$PYTHON" -c 'import json,sys; print(len(json.loads(sys.argv[1])))' "$PAGES_MISSING_JSON" 2>/dev/null || echo "0")
    if [ "$PM_COUNT" != "0" ] && [ -n "$PM_COUNT" ]; then
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      PM_LIST=$("$PYTHON" -c 'import json,sys; print(", ".join(json.loads(sys.argv[1])))' "$PAGES_MISSING_JSON" 2>/dev/null || echo "unknown")
      append_error "pages_probe" "pages_missing: $PM_LIST"
    fi
  fi
fi

# --- Gate detection via GET / (no redirect follow) ---
# Detect ADMIN_COMING_SOON_GATE: GET / returns 307 -> /coming-soon means gate is active.
# Use -w format to capture http_code and redirect_url (Location header) in one call.
# -o /dev/null discards the response body; no redirect follow (no -L).
# Done only when connectivity succeeded (no CONNECT_ERR).
GATE_ACTIVE=0
if [ "$CONNECT_ERR" -eq 0 ]; then
  GATE_STATUS_FILE="$WORK_DIR/gate-status.txt"
  curl -s --max-time 10 \
    -o /dev/null \
    -w "%{http_code}\n%{redirect_url}" \
    "$CLOUD_URL/" > "$GATE_STATUS_FILE" 2>/dev/null || printf "000\n" > "$GATE_STATUS_FILE"
  HEAD_STATUS=$(sed -n '1p' "$GATE_STATUS_FILE" | tr -d '\r\n')
  HEAD_LOC=$(sed -n '2p' "$GATE_STATUS_FILE" | tr -d '\r\n')
  if [ "$HEAD_STATUS" = "307" ] || echo "$HEAD_LOC" | grep -qi "coming-soon"; then
    GATE_ACTIVE=1
  fi
fi

# --- Compose scheduled_tasks (gate state) ---
# ADMIN_COMING_SOON_GATE is intentional state, not an error -- encode as a scheduled_tasks entry.
SCHTASKS_JSON="[]"
if [ "$GATE_ACTIVE" -eq 1 ]; then
  SCHTASKS_JSON='[{"name":"ADMIN_COMING_SOON_GATE","state":"active"}]'
else
  SCHTASKS_JSON='[{"name":"ADMIN_COMING_SOON_GATE","state":"inactive"}]'
fi

# --- Build config_hash from git_commit (stable per-deploy fingerprint) ---
# sha256 of git_commit short hash -> gives Phase 452 a stable per-deploy config key.
CONFIG_HASH_JSON="{}"
if [ -n "$GIT_COMMIT" ]; then
  CMT_HASH=$(printf '%s' "$GIT_COMMIT" | sha256sum | awk '{print $1}')
  CONFIG_HASH_JSON=$("$PYTHON" -c 'import json,sys; print(json.dumps({"admin.git_commit": sys.argv[1]}))' "$CMT_HASH")
fi

# Stable empty SHA256 (no binary to hash for cloud admin -- Next.js build)
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$PYTHON" -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  SCHTASKS_JSON="[]"
  BUILD_ID="null"
fi

# --- Assemble manifest -- no stdin/heredoc, all args via sys.argv ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
"$PYTHON" "$PY_HELPERS" build_manifest \
  "$TARGET_ID" "$HOST_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$BUILD_ID" "$CONFIG_HASH_JSON" \
  "$SCHTASKS_JSON" "$ENV_HASH" \
  "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE"

MANIFEST_JSON=$(cat "$MANIFEST_FILE")
write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
