#!/bin/bash
# scripts/fleet-probe/probe-relay.sh -- Phase 448 Plan 05
# Composite probe for comms-link relay: James :8766 local + VPS :8765 side.
# Checks local /relay/health which reports the VPS connection state.
# No COMMS_PSK needed -- /relay/health is a public endpoint.
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-relay.sh
# Optional env:
#   PROBE_OVERRIDE_RELAY_URL  -- overrides http://localhost:8766 for James side (tests use this)
# Exit codes:
#   0 -- probe ran (any probe_status including probe_failed)
#   2 -- script precondition violated (MANIFEST_TS not set)
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-relay: MANIFEST_TS not set" >&2
  exit 2
fi

RELAY_URL="${PROBE_OVERRIDE_RELAY_URL:-http://localhost:8766}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")

TARGET_ID="relay_james"
HOSTNAME_VAL="JAMES-PC"
IP_VAL="192.168.31.27"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# append_error SUB_PROBE ERROR_MSG [EXTRA_KEY EXTRA_VAL]
append_error() {
  local sp="$1" err_msg="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err_msg" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" \
    "$_PROBE_PYTHON" -c '
import os, json
a = json.loads(os.environ["PE"])
e = {"sub_probe": os.environ["SP"], "error": os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]] = os.environ["XV"]
a.append(e)
print(json.dumps(a))
')
}

# --- Local :8766 /relay/health ---
# curl -w outputs: body lines then a final line with the http code
HEALTH_RESP_FILE="$WORK_DIR/relay-health.txt"
HTTP_CODE=$(curl -s --max-time 5 \
  -o "$HEALTH_RESP_FILE" -w "%{http_code}" \
  "$RELAY_URL/relay/health" 2>/dev/null) || HTTP_CODE="000"
LOCAL_BODY=$(cat "$HEALTH_RESP_FILE" 2>/dev/null || echo "")

if [ "$HTTP_CODE" != "200" ]; then
  CONNECT_ERR=1
  append_error "local_relay" "James relay /relay/health HTTP ${HTTP_CODE} on ${RELAY_URL}" "access_gap" "RELAY_LOCAL_DOWN"
else
  VPS_CONNECTED=$(printf '%s' "$LOCAL_BODY" | "$_PROBE_PYTHON" -c '
import json, sys
try:
    d = json.loads(sys.stdin.read())
    print(str(d.get("connected","false")).lower())
except Exception:
    print("false")
' 2>/dev/null || echo "false")
  if [ "$VPS_CONNECTED" != "true" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "vps_relay" "James relay reports VPS connected=${VPS_CONNECTED}"
  fi
fi

# --- Local tasklist (node.exe) ---
# On Windows Git Bash: use cmd //c to prevent Git Bash from mangling /FI /FO flags as Unix paths.
RUNNING_PROCS_FILE="$WORK_DIR/running_procs.json"
printf '[]' > "$RUNNING_PROCS_FILE"
TASKLIST_FILE="$WORK_DIR/tasklist.csv"
if cmd //c "tasklist /FI \"IMAGENAME eq node.exe\" /V /FO CSV" > "$TASKLIST_FILE" 2>/dev/null; then
  "$_PROBE_PYTHON" - "$TASKLIST_FILE" "$RUNNING_PROCS_FILE" <<'PYEOF'
import csv, hashlib, json, sys, io
in_file, out_file = sys.argv[1], sys.argv[2]
rows = []
with open(in_file, encoding="utf-8-sig", errors="replace") as f:
    text = f.read()
if text.strip():
    reader = csv.reader(io.StringIO(text))
    for i, row in enumerate(reader):
        if i == 0:
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
    json.dump(rows[:50], f)
PYEOF
fi

# --- CommsLink-DaemonWatchdog schtask ---
SCHTASKS_FILE="$WORK_DIR/schtasks.json"
printf '[]' > "$SCHTASKS_FILE"
SCHTASKS_RAW="$WORK_DIR/schtasks.txt"
if cmd //c "schtasks /Query /TN \"CommsLink-DaemonWatchdog\" /V /FO LIST" > "$SCHTASKS_RAW" 2>/dev/null; then
  "$_PROBE_PYTHON" - "$SCHTASKS_RAW" "$SCHTASKS_FILE" <<'PYEOF'
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
    json.dump(entries, f)
PYEOF
fi

# --- HKCU Run registry (look for CommsLink entries) ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
printf '[]' > "$AUTOSTART_FILE"
REG_FILE="$WORK_DIR/reg-hkcu.txt"
if cmd //c "reg query \"HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run\"" > "$REG_FILE" 2>/dev/null; then
  "$_PROBE_PYTHON" - "$REG_FILE" "$AUTOSTART_FILE" <<'PYEOF'
import json, sys
in_file, out_file = sys.argv[1], sys.argv[2]
es = []
with open(in_file, encoding="utf-8", errors="replace") as f:
    for ln in f:
        ln = ln.strip()
        if not ln or ln.startswith("HKEY_"):
            continue
        parts = ln.split(None, 2)
        if len(parts) == 3 and "comms" in parts[2].lower():
            es.append({"source": "HKCU_Run", "key": parts[0], "value": parts[2]})
with open(out_file, "w") as f:
    json.dump(es, f)
PYEOF
fi

# --- env_vars_hash ---
ENV_HASH=$(env_names_hash)

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  printf '[]' > "$RUNNING_PROCS_FILE"
  printf '[]' > "$SCHTASKS_FILE"
  printf '[]' > "$AUTOSTART_FILE"
fi

# --- Assemble manifest ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
"$_PROBE_PYTHON" - \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_FILE" "$AUTOSTART_FILE" \
  "$ENV_HASH" "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys

(probed_at, probe_status,
 running_procs_file, schtasks_file, autostart_file,
 env_hash, probe_errors_json,
 out_file) = sys.argv[1:9]

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
    "target_id":         "relay_james",
    "host":              "JAMES-PC",
    "ip":                "192.168.31.27",
    "role":              "relay",
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     {},
    "build_id":          None,
    "config_hash":       {},
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
