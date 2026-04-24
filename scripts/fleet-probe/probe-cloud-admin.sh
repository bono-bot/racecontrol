#!/bin/bash
# scripts/fleet-probe/probe-cloud-admin.sh -- Phase 448 Plan 06
# Probes admin.racingpoint.cloud via HTTPS /api/health + HEAD / gate detection.
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-cloud-admin.sh
# Optional env:
#   PROBE_OVERRIDE_CLOUD_ADMIN_URL  -- overrides https://admin.racingpoint.cloud (tests: http://127.0.0.1:PORT)
#   STAFF_JWT                        -- optional; enables gated-page probe
# Stdout: {"target_id":"cloud_admin","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
# Side effect: state/fleet-manifest/$MANIFEST_TS/cloud_admin.json
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-cloud-admin: MANIFEST_TS not set" >&2
  exit 2
fi

CLOUD_URL="${PROBE_OVERRIDE_CLOUD_ADMIN_URL:-https://admin.racingpoint.cloud}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="cloud_admin"
HOST_VAL="admin.racingpoint.cloud"
IP_VAL="45.11.110.250"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files (avoids ARG_MAX and heredoc-stdin conflicts per 448-02 pattern)
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- Error append helper ---
# append_error SUB_PROBE ERROR_MSG [EXTRA_KEY EXTRA_VAL]
append_error() {
  local sp="$1" err="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" python3 -c '
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
HEALTH_BODY=$(cat "$HEALTH_RESP_FILE" 2>/dev/null || echo "")

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
  # Validate JSON
  if ! python3 -c 'import json,sys; json.load(sys.stdin)' < "$HEALTH_RESP_FILE" 2>/dev/null; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "health_parse" "cloud admin /api/health returned non-JSON"
  else
    # Extract build_id
    BID=$(python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    v = d.get("build_id")
    if v: print(v)
except Exception:
    pass
' < "$HEALTH_RESP_FILE" 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    fi

    # Extract git_commit
    GIT_COMMIT=$(python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    v = d.get("git_commit", "")
    if v: print(v)
except Exception:
    pass
' < "$HEALTH_RESP_FILE" 2>/dev/null || true)

    # Extract pages_missing and check if non-empty
    PAGES_MISSING_JSON=$(python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    pm = d.get("pages_missing", [])
    print(json.dumps(pm))
except Exception:
    print("[]")
' < "$HEALTH_RESP_FILE" 2>/dev/null || echo "[]")

    # pages_missing non-empty -> partial + pages_probe error
    PM_COUNT=$(python3 -c 'import json,sys; print(len(json.loads(sys.argv[1])))' "$PAGES_MISSING_JSON" 2>/dev/null || echo "0")
    if [ "$PM_COUNT" != "0" ] && [ -n "$PM_COUNT" ]; then
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      PM_LIST=$(python3 -c 'import json,sys; print(", ".join(json.loads(sys.argv[1])))' "$PAGES_MISSING_JSON" 2>/dev/null || echo "unknown")
      append_error "pages_probe" "pages_missing: $PM_LIST"
    fi
  fi
fi

# --- Gate detection via HEAD / ---
# Detect ADMIN_COMING_SOON_GATE: HEAD / returns 307 -> /coming-soon means gate is active.
# We make two calls: one to get the status code, one to get the Location header.
# Done only when connectivity succeeded (no CONNECT_ERR).
GATE_ACTIVE=0
if [ "$CONNECT_ERR" -eq 0 ]; then
  HEAD_STATUS=$(curl -s --max-time 10 -o /dev/null -w "%{http_code}" -I "$CLOUD_URL/" 2>/dev/null || echo "000")
  HEAD_HEADERS_FILE="$WORK_DIR/head-headers.txt"
  curl -s --max-time 10 -I "$CLOUD_URL/" > "$HEAD_HEADERS_FILE" 2>/dev/null || true
  HEAD_LOC=$(tr -d '\r' < "$HEAD_HEADERS_FILE" | awk -F': ' 'tolower($1)=="location" {print $2; exit}' | tr -d ' ' | tr -d '\n')
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
  CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"admin.git_commit": sys.argv[1]}))' "$CMT_HASH")
fi

# Stable empty SHA256 (no binary to hash for cloud admin -- Next.js build)
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  SCHTASKS_JSON="[]"
  BUILD_ID="null"
fi

# --- Assemble manifest ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
python3 - \
  "$TARGET_ID" "$HOST_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$BUILD_ID" "$CONFIG_HASH_JSON" \
  "$SCHTASKS_JSON" "$ENV_HASH" \
  "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys

(target_id, host, ip,
 probed_at, probe_status,
 build_id_json, config_hash_json,
 schtasks_json, env_hash,
 probe_errors_json,
 out_file) = sys.argv[1:12]

m = {
    "schema_version":    "1.0",
    "target_id":         target_id,
    "host":              host,
    "ip":                ip,
    "role":              "cloud_admin",
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     {},
    "build_id":          json.loads(build_id_json),
    "config_hash":       json.loads(config_hash_json),
    "running_procs":     [],
    "scheduled_tasks":   json.loads(schtasks_json),
    "autostart_entries": [],
    "env_vars_hash":     env_hash,
    "last_deploy_ts":    None,
}
errors = json.loads(probe_errors_json)
if errors:
    m["probe_errors"] = errors

with open(out_file, "w") as f:
    json.dump(m, f)
PYEOF

MANIFEST_JSON=$(cat "$MANIFEST_FILE")
write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
