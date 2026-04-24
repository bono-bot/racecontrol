#!/bin/bash
# scripts/fleet-probe/probe-pos.sh -- Phase 448 Plan 04
# Probes POS .130 via Tailscale SSH (pos1 / 100.95.211.1) + kiosk :3300/api/health.
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-pos.sh
# Optional env:
#   PROBE_SSH             -- overrides ssh binary (tests set to mock-ssh-responder.sh)
#   PROBE_SSH_SCENARIO    -- scenario file read by mock-ssh-responder.sh
#   PROBE_SSH_TARGET      -- overrides default SSH target User@100.95.211.1
#   PROBE_OVERRIDE_POS_URL-- overrides http://192.168.31.130:3300 for HTTP/kiosk calls
#   PROBE_SKIP_HTTP       -- set to 1 to skip kiosk HTTP probe (used in unit tests)
# Exit codes:
#   0 -- probe ran (any probe_status including probe_failed)
#   2 -- script precondition violated (missing MANIFEST_TS)
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-pos: MANIFEST_TS not set" >&2
  exit 2
fi

# --- POS target constants ---
TARGET_ID="pos_130"
HOST_VAL="POS1"
IP_VAL="192.168.31.130"
ROLE_VAL="pos"

SSH_CMD="${PROBE_SSH:-ssh}"
SSH_TARGET="${PROBE_SSH_TARGET:-User@100.95.211.1}"
POS_URL="${PROBE_OVERRIDE_POS_URL:-http://192.168.31.130:3300}"

START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")

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
    "$_PROBE_PYTHON" -c '
import os, json
a = json.loads(os.environ["PE"])
e = {"sub_probe": os.environ["SP"], "error": os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]] = os.environ["XV"]
a.append(e)
print(json.dumps(a))
')
}

# --- SSH: run a batch of commands via single session ---
# POS uses the same MARK-delimited output pattern as probe-server.sh (448-03)
# Commands are executed on the remote Windows host and output is demarcated by ===MARK:<name>=== lines.
SSH_SCRIPT='
echo "===MARK:hostname==="
hostname
echo "===MARK:certutil_pos_cfg==="
certutil -hashfile "C:\POS\config.json" SHA256 2>nul
echo "===MARK:tasklist==="
tasklist /V /FO CSV 2>nul
echo "===MARK:schtasks==="
schtasks /Query /V /FO LIST 2>nul
echo "===MARK:reg_hklm==="
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul
echo "===MARK:reg_hkcu==="
reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul
echo "===MARK:env==="
set 2>nul
echo "===MARK:end==="
'

SSH_OUT_FILE="$WORK_DIR/ssh-out.txt"
SSH_EXIT=0
"$SSH_CMD" -o ConnectTimeout=15 -o ServerAliveInterval=5 -o BatchMode=yes \
  "$SSH_TARGET" "$SSH_SCRIPT" > "$SSH_OUT_FILE" 2>/dev/null || SSH_EXIT=$?

# Check if SSH succeeded and produced expected output
if [ "$SSH_EXIT" -ne 0 ] || ! grep -q "===MARK:end===" "$SSH_OUT_FILE" 2>/dev/null; then
  CONNECT_ERR=1
  append_error "ssh_connect" "SSH to ${SSH_TARGET} failed or output truncated (exit=${SSH_EXIT})" "access_gap" "POS_SSH_DOWN"
fi

# --- Section extractor ---
# extract_section MARKER_NAME NEXT_MARKER_NAME
# Reads from SSH_OUT_FILE and prints lines between the two markers.
extract_section() {
  local marker="$1" next="$2"
  awk -v s="===MARK:${marker}===" -v e="===MARK:${next}===" \
    '$0==s { cap=1; next } $0==e { cap=0; exit } cap' \
    "$SSH_OUT_FILE"
}

# Initialise section data variables
CFG_SHA=""
TASKLIST_OUT=""
SCHTASKS_OUT=""
REG_HKLM_OUT=""
REG_HKCU_OUT=""
ENV_OUT=""

if [ "$CONNECT_ERR" -eq 0 ]; then
  # --- config_hash: certutil for C:\POS\config.json ---
  CERT_RAW=$(extract_section "certutil_pos_cfg" "tasklist" | tr -d '\r')
  CFG_SHA=$(printf '%s' "$CERT_RAW" | awk 'NR==2 {gsub(/ /, ""); print tolower($0)}')
  if ! echo "$CFG_SHA" | grep -qE '^[0-9a-f]{64}$'; then
    CFG_SHA=""
  fi

  # --- tasklist section ---
  TASKLIST_OUT=$(extract_section "tasklist" "schtasks")

  # Detect WMI-denied pattern (common on POS via remote SSH context)
  if printf '%s' "$TASKLIST_OUT" | grep -qiE 'WMI|ERROR:|Unable to connect'; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "tasklist" "WMI access denied from remote SSH context; retry via rp-bono-exec relay"
    TASKLIST_OUT=""  # clear so parser emits empty running_procs
  fi

  # --- schtasks section ---
  SCHTASKS_OUT=$(extract_section "schtasks" "reg_hklm")

  # --- reg_hklm section ---
  REG_HKLM_OUT=$(extract_section "reg_hklm" "reg_hkcu")

  # --- reg_hkcu section ---
  REG_HKCU_OUT=$(extract_section "reg_hkcu" "env")

  # --- env section ---
  ENV_OUT=$(extract_section "env" "end")
fi

# --- kiosk :3300/api/health for build_id ---
BUILD_ID="null"
if [ "${PROBE_SKIP_HTTP:-0}" != "1" ] && [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --connect-timeout 5 --max-time 10 "${POS_URL}/api/health" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BID=$(printf '%s' "$HEALTH" | "$_PROBE_PYTHON" -c '
import json, sys
try:
    d = json.load(sys.stdin)
    v = d.get("build_id")
    if v: print(v)
except Exception:
    pass
' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$("$_PROBE_PYTHON" -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "kiosk_health" "POS kiosk /api/health unreachable at ${POS_URL}"
  fi
fi

# --- Parse sections to JSON (temp-file pattern from 448-02 to avoid heredoc-stdin conflict) ---
# Write section data to temp files first, then pass file paths to Python via sys.argv.
# (Using '"$_PROBE_PYTHON" - <<PYEOF' redirects Python's stdin to the heredoc, breaking any pipe input.)
TASKLIST_RAW_FILE="$WORK_DIR/tasklist.txt"
SCHTASKS_RAW_FILE="$WORK_DIR/schtasks.txt"
REG_COMBINED_FILE="$WORK_DIR/reg-combined.txt"
printf '%s' "$TASKLIST_OUT" > "$TASKLIST_RAW_FILE"
printf '%s' "$SCHTASKS_OUT" > "$SCHTASKS_RAW_FILE"
printf '%s\n---HKCU---\n%s' "$REG_HKLM_OUT" "$REG_HKCU_OUT" > "$REG_COMBINED_FILE"

# --- Parse tasklist to running_procs ---
RUNNING_PROCS_FILE="$WORK_DIR/running_procs.json"
"$_PROBE_PYTHON" - "$TASKLIST_RAW_FILE" "$RUNNING_PROCS_FILE" <<'PYEOF'
import csv, hashlib, json, sys
in_file, out_file = sys.argv[1], sys.argv[2]
rows = []
with open(in_file, encoding="utf-8", errors="replace") as f:
    text = f.read()
if text.strip():
    import io
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
"$_PROBE_PYTHON" - "$SCHTASKS_RAW_FILE" "$SCHTASKS_FILE" <<'PYEOF'
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
"$_PROBE_PYTHON" - "$REG_COMBINED_FILE" "$AUTOSTART_FILE" <<'PYEOF'
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

# --- env_vars_hash (names only, never values) ---
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
if [ -n "$ENV_OUT" ]; then
  ENV_HASH=$(printf '%s' "$ENV_OUT" | tr -d '\r' | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
fi

# --- config_hash ---
CONFIG_HASH_JSON="{}"
if [ -n "$CFG_SHA" ]; then
  CONFIG_HASH_JSON=$("$_PROBE_PYTHON" -c 'import json,sys; print(json.dumps({"C:\\\\POS\\\\config.json": sys.argv[1]}))' "$CFG_SHA")
fi

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out all data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  BUILD_ID="null"
  printf '[]' > "$RUNNING_PROCS_FILE"
  printf '[]' > "$SCHTASKS_FILE"
  printf '[]' > "$AUTOSTART_FILE"
  ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
fi

# --- Assemble manifest via "$_PROBE_PYTHON" (file-arg pattern to avoid ARG_MAX) ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
"$_PROBE_PYTHON" - \
  "$TARGET_ID" "$HOST_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_FILE" "$AUTOSTART_FILE" \
  "$BUILD_ID" "$CONFIG_HASH_JSON" \
  "$ENV_HASH" "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys, os

(target_id, host, ip,
 probed_at, probe_status,
 running_procs_file, schtasks_file, autostart_file,
 build_id_json, config_hash_json,
 env_hash, probe_errors_json,
 out_file) = sys.argv[1:14]

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
    "role":              "pos",
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     {},
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
