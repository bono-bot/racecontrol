#!/bin/bash
# scripts/fleet-probe/probe-pod.sh -- Phase 448 Plan 04
# Probes one pod (positional N, range 1..8) via rc-sentry :8091/exec + rc-agent :8090/health.
# Usage: MANIFEST_TS=<iso> SENTRY_KEY=<key> bash scripts/fleet-probe/probe-pod.sh <pod_N>
# Optional env:
#   PROBE_OVERRIDE_URL             -- overrides base URL http://{pod_ip} (for tests)
#   PROBE_OVERRIDE_PORT_SENTRY     -- overrides 8091 (for tests; set same as mock server port)
#   PROBE_OVERRIDE_PORT_AGENT      -- overrides 8090 (for tests; set same as mock server port)
# Exit codes:
#   0 -- probe ran (any probe_status including probe_failed)
#   2 -- script precondition violated (missing MANIFEST_TS or invalid N)
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

# --- Preconditions ---
if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-pod: MANIFEST_TS not set" >&2
  exit 2
fi

POD_N="${1:-}"
if ! echo "$POD_N" | grep -qE '^[1-8]$'; then
  echo "probe-pod: invalid pod number '${POD_N}' (expected 1..8)" >&2
  exit 2
fi

# --- Pod IP table (LOCKED from CLAUDE.md network map) ---
case "$POD_N" in
  1) IP_VAL="192.168.31.89";  HOST_VAL="RCPOD-1" ;;
  2) IP_VAL="192.168.31.33";  HOST_VAL="RCPOD-2" ;;
  3) IP_VAL="192.168.31.28";  HOST_VAL="RCPOD-3" ;;
  4) IP_VAL="192.168.31.88";  HOST_VAL="RCPOD-4" ;;
  5) IP_VAL="192.168.31.86";  HOST_VAL="RCPOD-5" ;;
  6) IP_VAL="192.168.31.87";  HOST_VAL="RCPOD-6" ;;
  7) IP_VAL="192.168.31.38";  HOST_VAL="RCPOD-7" ;;
  8) IP_VAL="192.168.31.91";  HOST_VAL="RCPOD-8" ;;
esac

TARGET_ID="pod_${POD_N}"

# --- URL construction (PROBE_OVERRIDE_URL replaces base for tests) ---
# Tests set PROBE_OVERRIDE_URL=http://127.0.0.1:<port> and
# PROBE_OVERRIDE_PORT_SENTRY + PROBE_OVERRIDE_PORT_AGENT to the same port.
# When PORT vars equal each other and equal the URL port, both sentry and agent
# calls land on the single mock server.
BASE_URL="${PROBE_OVERRIDE_URL:-http://${IP_VAL}}"
PORT_SENTRY="${PROBE_OVERRIDE_PORT_SENTRY:-8091}"
PORT_AGENT="${PROBE_OVERRIDE_PORT_AGENT:-8090}"

# Build sentry and agent URLs:
# If PROBE_OVERRIDE_URL is set, strip any trailing port from it and re-append the override port.
if [ -n "${PROBE_OVERRIDE_URL:-}" ]; then
  # For test mocks: use PROBE_OVERRIDE_URL directly (it already has the port), then
  # point both sentry and agent endpoints at the same mock server by using PORT vars.
  # Extract the host:port from the override URL (drop trailing slash).
  BASE_NOPORT=$(echo "$BASE_URL" | sed 's|:[0-9]*$||')
  SENTRY_URL="${BASE_NOPORT}:${PORT_SENTRY}/exec"
  AGENT_HEALTH_URL="${BASE_NOPORT}:${PORT_AGENT}/health"
else
  SENTRY_URL="${BASE_URL}:${PORT_SENTRY}/exec"
  AGENT_HEALTH_URL="${BASE_URL}:${PORT_AGENT}/health"
fi

START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files (avoids ARG_MAX and heredoc-stdin conflicts per 448-02 pattern)
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- Error append helper ---
# append_error SUB_PROBE ERROR_MSG [EXTRA_KEY EXTRA_VAL]
append_error() {
  local sp="$1" err_msg="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err_msg" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" \
    python3 -c '
import os, json
a = json.loads(os.environ["PE"])
e = {"sub_probe": os.environ["SP"], "error": os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]] = os.environ["XV"]
a.append(e)
print(json.dumps(a))
')
}

# --- Auth pre-check (before any network call) ---
EFFECTIVE_KEY="${SENTRY_KEY:-${RCAGENT_SERVICE_KEY:-}}"
if [ -z "$EFFECTIVE_KEY" ]; then
  CONNECT_ERR=1
  append_error "auth" "SENTRY_KEY not set in invoking shell" "auth_gap" "no_sentry_key"
fi

# --- rc-sentry /exec helper ---
# pod_exec CMD
# Writes payload to temp file and curls rc-sentry /exec.
# stdout: "<http_code>|<response_body>"
pod_exec() {
  local cmd_str="$1"
  local payload_file="$WORK_DIR/exec-payload.json"
  local resp_file="$WORK_DIR/exec-resp.json"
  # Write JSON payload to file (Git Bash JSON rule: never inline JSON in curl -d)
  python3 -c 'import json,sys; print(json.dumps({"cmd": sys.argv[1]}))' "$cmd_str" > "$payload_file"
  local http_code
  http_code=$(curl -s --connect-timeout 5 --max-time 15 \
    -o "$resp_file" -w "%{http_code}" \
    -H "X-Service-Key: ${EFFECTIVE_KEY}" \
    -H "Content-Type: application/json" \
    -d "@${payload_file}" \
    "$SENTRY_URL" 2>/dev/null) || http_code="000"
  local body
  body=$(cat "$resp_file" 2>/dev/null || echo "")
  printf '%s|%s' "$http_code" "$body"
}

# --- Connectivity + auth probe (tasklist first) ---
TASKLIST_OUT=""
SCHTASKS_OUT=""
REG_HKLM_OUT=""
REG_HKCU_OUT=""
BIN_SHA=""
CFG_SHA=""
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

if [ "$CONNECT_ERR" -eq 0 ]; then
  RESP=$(pod_exec "tasklist /V /FO CSV 2>nul")
  CODE="${RESP%%|*}"
  BODY="${RESP#*|}"

  if [ "$CODE" = "401" ]; then
    CONNECT_ERR=1
    append_error "auth" "rc-sentry /exec returned 401; SENTRY_KEY may be stale" "auth_gap" "stale_sentry_key"
  elif [ "$CODE" = "000" ]; then
    CONNECT_ERR=1
    append_error "connectivity" "rc-sentry /exec unreachable at ${SENTRY_URL}"
  elif [ "$CODE" != "200" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "exec_tasklist" "rc-sentry /exec returned HTTP ${CODE}"
  else
    # Extract stdout from JSON response
    TASKLIST_OUT=$(printf '%s' "$BODY" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    print(d.get("stdout", ""))
except Exception:
    pass
' 2>/dev/null || true)

    # schtasks
    RESP=$(pod_exec "schtasks /Query /V /FO LIST 2>nul")
    SCHTASKS_OUT=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    print(d.get("stdout", ""))
except Exception:
    pass
' 2>/dev/null || true)

    # HKLM Run registry
    RESP=$(pod_exec 'reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul')
    REG_HKLM_OUT=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    print(d.get("stdout", ""))
except Exception:
    pass
' 2>/dev/null || true)

    # HKCU Run registry
    RESP=$(pod_exec 'reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul')
    REG_HKCU_OUT=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    print(d.get("stdout", ""))
except Exception:
    pass
' 2>/dev/null || true)

    # binary SHA256 via certutil
    RESP=$(pod_exec 'certutil -hashfile "C:\RacingPoint\rc-agent.exe" SHA256 2>nul')
    CERT_AGENT=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    lines = [l.strip() for l in d.get("stdout","").replace("\r","").splitlines()]
    if len(lines) >= 2:
        h = lines[1].replace(" ","").lower()
        if len(h) == 64 and all(c in "0123456789abcdef" for c in h):
            print(h)
except Exception:
    pass
' 2>/dev/null || true)
    if [ -n "$CERT_AGENT" ]; then
      BIN_SHA="$CERT_AGENT"
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "binary_sha256" "certutil on rc-agent.exe did not yield valid hex"
    fi

    # config SHA256 via certutil
    RESP=$(pod_exec 'certutil -hashfile "C:\RacingPoint\rc-agent.toml" SHA256 2>nul')
    CFG_SHA=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    lines = [l.strip() for l in d.get("stdout","").replace("\r","").splitlines()]
    if len(lines) >= 2:
        h = lines[1].replace(" ","").lower()
        if len(h) == 64 and all(c in "0123456789abcdef" for c in h):
            print(h)
except Exception:
    pass
' 2>/dev/null || true)

    # env_vars_hash via set command
    RESP=$(pod_exec "set 2>nul")
    SET_OUT=$(printf '%s' "${RESP#*|}" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    print(d.get("stdout", ""))
except Exception:
    pass
' 2>/dev/null | tr -d '\r')
    if [ -n "$SET_OUT" ]; then
      ENV_HASH=$(printf '%s' "$SET_OUT" | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
    fi
  fi
fi

# --- build_id from rc-agent /health ---
BUILD_ID="null"
if [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --connect-timeout 5 --max-time 10 "$AGENT_HEALTH_URL" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BID=$(printf '%s' "$HEALTH" | python3 -c '
import json, sys
try:
    d = json.load(sys.stdin)
    v = d.get("build_id")
    if v: print(v)
except Exception:
    pass
' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/health response missing build_id field"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "build_id" "rc-agent /health unreachable at ${AGENT_HEALTH_URL}"
  fi
fi

# --- Parse sections to JSON (temp-file pattern from 448-02: no heredoc+pipe combination) ---
# Save raw data to temp files, pass file paths as sys.argv to avoid heredoc-stdin conflict.
TASKLIST_RAW_FILE="$WORK_DIR/tasklist.txt"
SCHTASKS_RAW_FILE="$WORK_DIR/schtasks.txt"
REG_COMBINED_FILE="$WORK_DIR/reg-combined.txt"
printf '%s' "$TASKLIST_OUT" > "$TASKLIST_RAW_FILE"
printf '%s' "$SCHTASKS_OUT" > "$SCHTASKS_RAW_FILE"
printf '%s\n---HKCU---\n%s' "$REG_HKLM_OUT" "$REG_HKCU_OUT" > "$REG_COMBINED_FILE"

# --- Parse tasklist to running_procs ---
RUNNING_PROCS_FILE="$WORK_DIR/running_procs.json"
python3 - "$TASKLIST_RAW_FILE" "$RUNNING_PROCS_FILE" <<'PYEOF'
import csv, hashlib, json, sys, io
in_file, out_file = sys.argv[1], sys.argv[2]
rows = []
with open(in_file, encoding="utf-8", errors="replace") as f:
    text = f.read()
if text.strip():
    reader = csv.reader(io.StringIO(text))
    first = True
    for row in reader:
        if first:
            first = False
            continue
        if len(row) < 2:
            continue
        try:
            pid = int(row[1])
        except Exception:
            continue
        h = hashlib.sha256(" ".join(row).encode("utf-8", "replace")).hexdigest()
        rows.append({"name": row[0], "pid": pid, "cmdline_hash": h})
with open(out_file, "w") as f:
    json.dump(rows[:100], f)
PYEOF

# --- Parse schtasks to scheduled_tasks ---
SCHTASKS_FILE="$WORK_DIR/schtasks.json"
python3 - "$SCHTASKS_RAW_FILE" "$SCHTASKS_FILE" <<'PYEOF'
import json, sys
in_file, out_file = sys.argv[1], sys.argv[2]
entries = []
cur = {}
with open(in_file, encoding="utf-8", errors="replace") as f:
    for line in f:
        line = line.rstrip("\r\n")
        if not line.strip():
            if cur.get("name") and cur.get("state"):
                entries.append({"name": cur["name"], "state": cur["state"]})
            cur = {}
            continue
        if ":" in line:
            k, _, v = line.partition(":")
            k = k.strip()
            v = v.strip()
            if k == "TaskName":
                cur["name"] = v.lstrip("\\")
            elif k == "Status":
                cur["state"] = v
if cur.get("name") and cur.get("state"):
    entries.append({"name": cur["name"], "state": cur["state"]})
with open(out_file, "w") as f:
    json.dump(entries[:100], f)
PYEOF

# --- Parse reg to autostart_entries ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
python3 - "$REG_COMBINED_FILE" "$AUTOSTART_FILE" <<'PYEOF'
import json, sys
in_file, out_file = sys.argv[1], sys.argv[2]
with open(in_file, encoding="utf-8", errors="replace") as f:
    text = f.read()
parts = text.split("\n---HKCU---\n", 1)
hklm = parts[0]
hkcu = parts[1] if len(parts) > 1 else ""
entries = []
for src, blob in (("HKLM_Run", hklm), ("HKCU_Run", hkcu)):
    for line in blob.splitlines():
        line = line.strip()
        if not line or line.startswith("HKEY_"):
            continue
        pp = line.split(None, 2)
        if len(pp) == 3:
            entries.append({"source": src, "key": pp[0], "value": pp[2]})
with open(out_file, "w") as f:
    json.dump(entries, f)
PYEOF

# --- config_hash and binary_sha256 ---
CONFIG_HASH_JSON="{}"
if [ -n "$CFG_SHA" ]; then
  CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"rc-agent.toml": sys.argv[1]}))' "$CFG_SHA")
fi

BINARY_SHA_JSON="{}"
if [ -n "$BIN_SHA" ]; then
  BINARY_SHA_JSON=$(python3 -c 'import json,sys; print(json.dumps({"rc-agent.exe": sys.argv[1]}))' "$BIN_SHA")
fi

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out data fields (no partial data)
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  BINARY_SHA_JSON="{}"
  CONFIG_HASH_JSON="{}"
  printf '[]' > "$RUNNING_PROCS_FILE"
  printf '[]' > "$SCHTASKS_FILE"
  printf '[]' > "$AUTOSTART_FILE"
  BUILD_ID="null"
  ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
fi

# --- Assemble manifest via python3 (file-arg pattern from 448-02 to avoid ARG_MAX) ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
python3 - \
  "$TARGET_ID" "$HOST_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_FILE" "$AUTOSTART_FILE" \
  "$BUILD_ID" "$BINARY_SHA_JSON" "$CONFIG_HASH_JSON" \
  "$ENV_HASH" "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys, os

(target_id, host, ip,
 probed_at, probe_status,
 running_procs_file, schtasks_file, autostart_file,
 build_id_json, binary_sha_json, config_hash_json,
 env_hash, probe_errors_json,
 out_file) = sys.argv[1:15]

def load_json_file(path):
    try:
        with open(path) as f:
            return json.load(f)
    except Exception:
        return []

running_procs = load_json_file(running_procs_file)
scheduled_tasks = load_json_file(schtasks_file)
autostart_entries = load_json_file(autostart_file)

m = {
    "schema_version":    "1.0",
    "target_id":         target_id,
    "host":              host,
    "ip":                ip,
    "role":              "pod",
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     json.loads(binary_sha_json),
    "build_id":          json.loads(build_id_json),
    "config_hash":       json.loads(config_hash_json),
    "running_procs":     running_procs,
    "scheduled_tasks":   scheduled_tasks,
    "autostart_entries": autostart_entries,
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

TOTAL_ERRS=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERRS"
