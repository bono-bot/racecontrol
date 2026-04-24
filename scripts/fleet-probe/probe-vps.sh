#!/bin/bash
# scripts/fleet-probe/probe-vps.sh -- Phase 448 Plan 05
# Probes Bono VPS via comms-link relay POST localhost:8766/relay/exec/run.
# NEVER SSHes directly to VPS -- the relay is the only prod path.
# Usage: MANIFEST_TS=<iso> COMMS_PSK=<psk> bash scripts/fleet-probe/probe-vps.sh
# Optional env:
#   PROBE_OVERRIDE_RELAY_URL  -- overrides http://localhost:8766 (tests set this to mock server URL)
# Exit codes:
#   0 -- probe ran (any probe_status including probe_failed)
#   2 -- script precondition violated (MANIFEST_TS not set)
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-vps: MANIFEST_TS not set" >&2
  exit 2
fi

RELAY_URL="${PROBE_OVERRIDE_RELAY_URL:-http://localhost:8766}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="bono_vps"
HOSTNAME_VAL="srv1422716.hstgr.cloud"
IP_VAL="45.11.110.250"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for temp files (avoids heredoc+pipe stdin conflicts per 448-02 pattern)
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

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

# --- Auth pre-check: COMMS_PSK must be set ---
if [ -z "${COMMS_PSK:-}" ]; then
  CONNECT_ERR=1
  append_error "auth" "COMMS_PSK not set in invoking shell" "auth_gap" "no_comms_psk"
fi

# --- Connect-stage: /relay/health ---
RELAY_OK=0
if [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --max-time 5 "$RELAY_URL/relay/health" 2>/dev/null || true)
  if [ -z "$HEALTH" ]; then
    CONNECT_ERR=1
    append_error "local_relay" "connection refused on $RELAY_URL/relay/health" "access_gap" "RELAY_LOCAL_DOWN"
  else
    CONNECTED=$(printf '%s' "$HEALTH" | python3 -c '
import json, sys
try:
    d = json.loads(sys.stdin.read())
    print(str(d.get("connected","false")).lower())
except Exception:
    print("false")
' 2>/dev/null || echo "false")
    if [ "$CONNECTED" != "true" ]; then
      CONNECT_ERR=1
      append_error "relay_connect" "relay reports connected=$CONNECTED" "access_gap" "RELAY_DOWN"
    else
      RELAY_OK=1
    fi
  fi
fi

# --- Execute probe script on VPS via /relay/exec/run ---
EXEC_OUT=""
if [ "$RELAY_OK" -eq 1 ]; then
  # Compose a single bash script that emits sectioned output.
  # Each section delimited by ===MARK:<name>=== sentinel lines.
  PROBE_SCRIPT='#!/bin/bash
echo "===MARK:uname==="
uname -a
echo "===MARK:ps==="
ps -eo comm,pid,args --no-headers 2>/dev/null | head -60 || ps -eo comm,pid,args 2>/dev/null | tail -n +2 | head -60
echo "===MARK:systemctl==="
systemctl list-unit-files --type=service --state=enabled --no-pager 2>/dev/null | tail -n +2 | head -40 || true
echo "===MARK:pm2==="
pm2 jlist 2>/dev/null | head -500 || echo "[]"
echo "===MARK:config_hash==="
sha256sum /root/racecontrol.toml 2>/dev/null || echo "NO_FILE"
echo "===MARK:env==="
env | awk -F= "NF>=2 {print \$1}" | sort
echo "===MARK:end==="
'

  # Write JSON payload to temp file (Git Bash JSON rule: never inline JSON in curl -d)
  PAYLOAD_FILE="$WORK_DIR/exec-payload.json"
  python3 - "$PROBE_SCRIPT" "$PAYLOAD_FILE" <<'PYEOF'
import json, sys
script_text, out_file = sys.argv[1], sys.argv[2]
payload = {"command": "bash_script", "script": script_text, "reason": "probe-vps-448"}
with open(out_file, "w") as f:
    json.dump(payload, f)
PYEOF

  RESP_FILE="$WORK_DIR/exec-resp.json"
  HTTP_CODE=$(curl -s --max-time 30 -X POST \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer ${COMMS_PSK}" \
    -o "$RESP_FILE" -w "%{http_code}" \
    -d "@${PAYLOAD_FILE}" \
    "$RELAY_URL/relay/exec/run" 2>/dev/null) || HTTP_CODE="000"

  RESP=$(cat "$RESP_FILE" 2>/dev/null || echo "")

  if [ -z "$RESP" ] || [ "$HTTP_CODE" = "000" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "relay_exec" "no response from /relay/exec/run (http_code=${HTTP_CODE})"
  else
    EXIT_CODE=$(printf '%s' "$RESP" | python3 -c '
import json, sys
try:
    d = json.loads(sys.stdin.read())
    ec = d.get("exitCode")
    if ec is None:
        ec = d.get("exit_code", -1)
    print(int(ec))
except Exception:
    print(-1)
' 2>/dev/null || echo "-1")
    if [ "$EXIT_CODE" != "0" ]; then
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "relay_exec" "exec exitCode=${EXIT_CODE}"
    fi
    EXEC_OUT=$(printf '%s' "$RESP" | python3 -c '
import json, sys
try:
    d = json.loads(sys.stdin.read())
    print(d.get("stdout",""))
except Exception:
    pass
' 2>/dev/null || echo "")
  fi
fi

# --- Parse sectioned output via temp files (avoids heredoc+pipe stdin conflicts) ---
EXEC_FILE="$WORK_DIR/exec-out.txt"
printf '%s' "$EXEC_OUT" > "$EXEC_FILE"

UNAME_FILE="$WORK_DIR/uname.txt"
PS_FILE="$WORK_DIR/ps.txt"
SYSTEMCTL_FILE="$WORK_DIR/systemctl.txt"
PM2_FILE="$WORK_DIR/pm2.txt"
CFG_FILE="$WORK_DIR/config_hash.txt"
ENV_FILE="$WORK_DIR/env.txt"

python3 - "$EXEC_FILE" "$UNAME_FILE" "$PS_FILE" "$SYSTEMCTL_FILE" "$PM2_FILE" "$CFG_FILE" "$ENV_FILE" <<'PYEOF'
import sys

def extract(lines, start_marker, end_marker):
    out = []
    cap = False
    for ln in lines:
        if ln == start_marker:
            cap = True
            continue
        if ln == end_marker:
            break
        if cap:
            out.append(ln)
    return out

with open(sys.argv[1], encoding="utf-8", errors="replace") as f:
    lines = [l.rstrip("\r\n") for l in f]

marks = ["===MARK:uname===", "===MARK:ps===", "===MARK:systemctl===", "===MARK:pm2===",
         "===MARK:config_hash===", "===MARK:env===", "===MARK:end==="]

def extract_between(lines, start, end):
    out = []
    cap = False
    for ln in lines:
        if ln == start:
            cap = True
            continue
        if ln == end:
            break
        if cap:
            out.append(ln)
    return out

pairs = [
    (marks[0], marks[1], sys.argv[2]),   # uname
    (marks[1], marks[2], sys.argv[3]),   # ps
    (marks[2], marks[3], sys.argv[4]),   # systemctl
    (marks[3], marks[4], sys.argv[5]),   # pm2
    (marks[4], marks[5], sys.argv[6]),   # config_hash
    (marks[5], marks[6], sys.argv[7]),   # env
]
for start, end, out_path in pairs:
    chunk = extract_between(lines, start, end)
    with open(out_path, "w") as f:
        f.write("\n".join(chunk))
PYEOF

# --- Parse ps -> running_procs ---
RUNNING_PROCS_FILE="$WORK_DIR/running_procs.json"
python3 - "$PS_FILE" "$RUNNING_PROCS_FILE" <<'PYEOF'
import sys, hashlib, json
in_file, out_file = sys.argv[1], sys.argv[2]
rows = []
with open(in_file, encoding="utf-8", errors="replace") as f:
    for ln in f:
        ln = ln.rstrip("\r\n").strip()
        if not ln:
            continue
        parts = ln.split(None, 2)
        if len(parts) < 2:
            continue
        name = parts[0]
        try:
            pid = int(parts[1])
        except Exception:
            continue
        h = hashlib.sha256(ln.encode("utf-8", "replace")).hexdigest()
        rows.append({"name": name, "pid": pid, "cmdline_hash": h})
with open(out_file, "w") as f:
    json.dump(rows[:100], f)
PYEOF

# --- Parse systemctl + pm2 -> scheduled_tasks ---
SCHTASKS_FILE="$WORK_DIR/schtasks.json"
python3 - "$SYSTEMCTL_FILE" "$PM2_FILE" "$SCHTASKS_FILE" <<'PYEOF'
import sys, json
svc_file, pm2_file, out_file = sys.argv[1], sys.argv[2], sys.argv[3]
entries = []
with open(svc_file, encoding="utf-8", errors="replace") as f:
    for ln in f:
        ln = ln.rstrip("\r\n").strip()
        if not ln or ln.startswith("UNIT") or ln.startswith("Legend"):
            continue
        bits = ln.split(None, 1)
        if bits:
            entries.append({"name": bits[0], "state": "enabled"})
with open(pm2_file, encoding="utf-8", errors="replace") as f:
    pm2_text = f.read().strip()
if pm2_text and pm2_text != "[]":
    try:
        for obj in json.loads(pm2_text):
            n = obj.get("name") or "pm2-unknown"
            st = obj.get("pm2_env", {}).get("status") or "unknown"
            entries.append({"name": "pm2:" + n, "state": st})
    except Exception:
        pass
with open(out_file, "w") as f:
    json.dump(entries[:100], f)
PYEOF

# --- Parse systemctl -> autostart_entries ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
python3 - "$SYSTEMCTL_FILE" "$AUTOSTART_FILE" <<'PYEOF'
import sys, json
in_file, out_file = sys.argv[1], sys.argv[2]
es = []
with open(in_file, encoding="utf-8", errors="replace") as f:
    for ln in f:
        ln = ln.rstrip("\r\n").strip()
        if not ln or ln.startswith("UNIT") or ln.startswith("Legend"):
            continue
        bits = ln.split(None, 1)
        if bits:
            es.append({"source": "schtask", "key": bits[0], "value": "systemctl enabled"})
with open(out_file, "w") as f:
    json.dump(es[:100], f)
PYEOF

# --- config_hash ---
CONFIG_HASH_JSON="{}"
CFG_LINE=$(head -1 "$CFG_FILE" 2>/dev/null || echo "")
if [ -n "$CFG_LINE" ] && [ "$CFG_LINE" != "NO_FILE" ]; then
  CFG_HASH=$(printf '%s' "$CFG_LINE" | awk '{print $1}')
  if echo "$CFG_HASH" | grep -qE '^[0-9a-f]{64}$'; then
    CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"/root/racecontrol.toml": sys.argv[1]}))' "$CFG_HASH")
  fi
fi

# --- env_vars_hash ---
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
ENV_CONTENT=$(cat "$ENV_FILE" 2>/dev/null || echo "")
if [ -n "$ENV_CONTENT" ]; then
  ENV_HASH=$(printf '%s' "$ENV_CONTENT" | sort | sha256sum | awk '{print $1}')
fi

# --- Timing and status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out data fields
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  printf '[]' > "$RUNNING_PROCS_FILE"
  printf '[]' > "$SCHTASKS_FILE"
  printf '[]' > "$AUTOSTART_FILE"
  ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
fi

# --- Assemble manifest (file-arg pattern: no inline JSON in python -c to avoid ARG_MAX) ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
python3 - \
  "$TARGET_ID" "$HOSTNAME_VAL" "$IP_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_FILE" "$AUTOSTART_FILE" \
  "$CONFIG_HASH_JSON" "$ENV_HASH" "$PROBE_ERRORS_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys

(target_id, host, ip,
 probed_at, probe_status,
 running_procs_file, schtasks_file, autostart_file,
 config_hash_json, env_hash, probe_errors_json,
 out_file) = sys.argv[1:13]

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
    "role":              "vps",
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     {},
    "build_id":          None,
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
