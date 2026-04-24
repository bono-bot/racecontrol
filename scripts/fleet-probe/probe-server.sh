#!/bin/bash
# scripts/fleet-probe/probe-server.sh -- Phase 448 Plan 03
# Probes Server .23 via Tailscale SSH (ADMIN@100.125.108.37).
# Captures: binary_sha256 (racecontrol.exe via certutil), build_id (HTTP /api/v1/health),
#           config_hash 3-way Q5 drift (server_live via SSH certutil, james_proxy from /d/,
#           git_head from repo), running_procs, scheduled_tasks, autostart_entries,
#           env_vars_hash, last_deploy_ts (SWAPLOG.md).
# PROBE_SSH env var overrides the ssh binary for offline unit testing.
# PROBE_SSH_SCENARIO env var is passed through to the mock-ssh-responder.sh.
# PROBE_SKIP_HTTP=1 skips the /api/v1/health HTTP probe (for unit tests).
# Exit: 0 always except exit 2 when MANIFEST_TS is unset.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-server: MANIFEST_TS not set" >&2
  exit 2
fi

SSH_CMD="${PROBE_SSH:-ssh}"
SSH_TARGET="${PROBE_SSH_TARGET:-ADMIN@100.125.108.37}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")

TARGET_ID="server_23"
HOSTNAME_VAL="Racing-Point-Server"
IP_VAL="192.168.31.23"
ROLE_VAL="server"

CONNECT_ERR=0
SUBPROBE_ERR=0

# Work dir for intermediate data files; cleaned up on EXIT.
WORK_DIR=$(mktemp -d /tmp/probe-server.XXXXXX 2>/dev/null || mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

PROBE_ERRORS_FILE="$WORK_DIR/probe_errors.json"
printf '[]' > "$PROBE_ERRORS_FILE"

# append_error sub_probe error_msg [access_gap]
# Appends a JSON error object to PROBE_ERRORS_FILE.
append_error() {
  local sp="$1" msg="$2" gap="${3:-}"
  "$_PROBE_PYTHON" - "$PROBE_ERRORS_FILE" "$sp" "$msg" "$gap" <<'PYEOF'
import json, sys
fname, sp, msg, gap = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
with open(fname) as f:
    a = json.load(f)
entry = {"sub_probe": sp, "error": msg}
if gap:
    entry["access_gap"] = gap
a.append(entry)
with open(fname, "w") as f:
    json.dump(a, f)
PYEOF
}

# --- SSH round-trip: one session, section markers separate sub-probe outputs ---
# All remote commands are concatenated with section markers so we can parse them
# from a single SSH call (amortize handshake cost).
# Remote host runs Windows cmd.exe; use 2>nul not 2>/dev/null.
SSH_SCRIPT='echo ===MARK:hostname=== & hostname & echo ===MARK:certutil_exe=== & certutil -hashfile "C:\RacingPoint\racecontrol.exe" SHA256 2>nul & echo ===MARK:certutil_toml=== & certutil -hashfile "C:\RacingPoint\racecontrol.toml" SHA256 2>nul & echo ===MARK:tasklist=== & tasklist /V /FO CSV 2>nul & echo ===MARK:schtasks=== & schtasks /Query /V /FO LIST 2>nul & echo ===MARK:reg_hklm=== & reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul & echo ===MARK:reg_hkcu=== & reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul & echo ===MARK:env=== & set 2>nul & echo ===MARK:end==='

SSH_OUT_FILE="$WORK_DIR/ssh_out.txt"
SSH_EXIT=0
"$SSH_CMD" -o ConnectTimeout=15 -o ServerAliveInterval=5 -o BatchMode=yes \
  "$SSH_TARGET" "$SSH_SCRIPT" > "$SSH_OUT_FILE" 2>/dev/null || SSH_EXIT=$?

# Validate SSH succeeded and output contains the end marker.
if [ "$SSH_EXIT" -ne 0 ] || ! grep -q "===MARK:end===" "$SSH_OUT_FILE" 2>/dev/null; then
  CONNECT_ERR=1
  append_error "ssh_connect" "SSH to $SSH_TARGET failed or truncated (exit=$SSH_EXIT)" "SSH_23"
fi

# extract_section MARKER_NAME NEXT_MARKER_NAME
# Reads SSH_OUT_FILE and prints lines between the two named markers.
extract_section() {
  local marker="$1" next="$2"
  awk -v start="===MARK:$marker===" -v end_mark="===MARK:$next===" '
    $0 == start  { capture=1; next }
    $0 == end_mark { capture=0; exit }
    capture       { print }
  ' "$SSH_OUT_FILE"
}

BIN_SHA=""
CFG_SERVER_SHA=""
TASKLIST_FILE="$WORK_DIR/tasklist.csv"
SCHTASKS_FILE="$WORK_DIR/schtasks.txt"
REG_HKLM_FILE="$WORK_DIR/reg_hklm.txt"
REG_HKCU_FILE="$WORK_DIR/reg_hkcu.txt"
ENV_FILE="$WORK_DIR/env.txt"

if [ "$CONNECT_ERR" -eq 0 ]; then
  # certutil output: 3 lines (header / hex / trailer). Hex line is all lowercase hex.
  # certutil on Windows outputs SHA256 as space-separated pairs; strip spaces and lowercase.
  CERT_EXE_OUT=$(extract_section "certutil_exe" "certutil_toml" | tr -d '\r')
  CERT_TOML_OUT=$(extract_section "certutil_toml" "tasklist" | tr -d '\r')

  BIN_SHA=$(printf '%s' "$CERT_EXE_OUT" | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
  CFG_SERVER_SHA=$(printf '%s' "$CERT_TOML_OUT" | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')

  # Validate 64-char hex
  if ! printf '%s' "$BIN_SHA" | grep -qE '^[0-9a-f]{64}$'; then
    BIN_SHA=""
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "binary_sha256" "certutil on racecontrol.exe did not yield 64-char hex"
  fi
  if ! printf '%s' "$CFG_SERVER_SHA" | grep -qE '^[0-9a-f]{64}$'; then
    CFG_SERVER_SHA=""
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "config_hash_server_live" "certutil on racecontrol.toml did not yield 64-char hex"
  fi

  extract_section "tasklist" "schtasks" > "$TASKLIST_FILE"
  extract_section "schtasks" "reg_hklm" > "$SCHTASKS_FILE"
  extract_section "reg_hklm" "reg_hkcu" > "$REG_HKLM_FILE"
  extract_section "reg_hkcu" "env" > "$REG_HKCU_FILE"
  extract_section "env" "end" > "$ENV_FILE"
fi

# --- Q5 james_proxy config hash: /d/racecontrol.toml on James ---
CFG_JAMES_SHA=""
if [ -f "/d/racecontrol.toml" ]; then
  CFG_JAMES_SHA=$(sha256_of "/d/racecontrol.toml")
else
  SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
  append_error "config_hash_james_proxy" "D:\\racecontrol.toml not found on James (/d/racecontrol.toml)"
fi

# --- Q5 git_head config hash: racecontrol.toml in repo or git ---
CFG_GIT_SHA=""
for candidate in "crates/racecontrol/racecontrol.toml" "crates/racecontrol/config/racecontrol.toml" "racecontrol.toml"; do
  if [ -f "$candidate" ]; then
    CFG_GIT_SHA=$(sha256_of "$candidate")
    break
  fi
done
if [ -z "$CFG_GIT_SHA" ]; then
  GIT_CONTENT=$(git show HEAD:crates/racecontrol/racecontrol.toml 2>/dev/null || true)
  if [ -n "$GIT_CONTENT" ]; then
    CFG_GIT_SHA=$(printf '%s' "$GIT_CONTENT" | sha256_of_stdin)
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "config_hash_git_head" "no racecontrol.toml in working tree or git HEAD"
  fi
fi

# --- Assemble config_hash JSON from three Q5 keys ---
CONFIG_HASH_FILE="$WORK_DIR/config_hash.json"
"$_PROBE_PYTHON" - "$CONFIG_HASH_FILE" "$CFG_SERVER_SHA" "$CFG_JAMES_SHA" "$CFG_GIT_SHA" <<'PYEOF'
import json, sys
fname = sys.argv[1]
server_sha = sys.argv[2]
james_sha  = sys.argv[3]
git_sha    = sys.argv[4]
d = {}
if server_sha: d["racecontrol.toml.server_live"]  = server_sha
if james_sha:  d["racecontrol.toml.james_proxy"]  = james_sha
if git_sha:    d["racecontrol.toml.git_head"]      = git_sha
with open(fname, "w") as f:
    json.dump(d, f)
PYEOF

# --- build_id via HTTP /api/v1/health (LAN 192.168.31.23:8080) ---
BUILD_ID_VAL="null"
if [ "${PROBE_SKIP_HTTP:-0}" != "1" ] && [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --max-time 5 "http://$IP_VAL:8080/api/v1/health" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BID=$(printf '%s' "$HEALTH" | "$_PROBE_PYTHON" -c "import json,sys; d=json.load(sys.stdin); print(d.get('build_id',''))" 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID_VAL=$("$_PROBE_PYTHON" -c "import json,sys; print(json.dumps(sys.argv[1]))" "$BID")
    else
      BUILD_ID_VAL="null"
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/api/v1/health response missing build_id field"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "build_id" "HTTP GET http://$IP_VAL:8080/api/v1/health unreachable or timed out"
  fi
fi

# --- SWAPLOG.md: extract last_deploy_ts from the last data row ---
LAST_DEPLOY_VAL="null"
SWAPLOG_PATH="SWAPLOG.md"
if [ -f "$SWAPLOG_PATH" ]; then
  LAST_ROW=$(grep -E '^\| [0-9]' "$SWAPLOG_PATH" | tail -1)
  if [ -n "$LAST_ROW" ]; then
    # Column 2 format: "YYYY-MM-DD HH:MM IST" or "YYYY-MM-DD HH:MM:SS IST"
    TS=$(printf '%s' "$LAST_ROW" | awk -F'|' '{gsub(/^ +| +$/, "", $2); print $2}')
    ISO=$(printf '%s' "$TS" | "$_PROBE_PYTHON" -c "
import sys, re
s = sys.stdin.read().strip()
m = re.match(r'(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})(?::(\d{2}))?', s)
if m:
    secs = m.group(3) or '00'
    print('{0}T{1}:{2}+05:30'.format(m.group(1), m.group(2), secs))
" 2>/dev/null || true)
    if [ -n "$ISO" ]; then
      LAST_DEPLOY_VAL="\"$ISO\""
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "last_deploy_ts" "SWAPLOG.md last row timestamp unparseable: $TS"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "last_deploy_ts" "SWAPLOG.md has no data rows matching timestamp pattern"
  fi
else
  SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
  append_error "last_deploy_ts" "SWAPLOG.md not found in repo root"
fi

# --- Parse tasklist CSV into running_procs ---
RUNNING_PROCS_FILE="$WORK_DIR/running_procs.json"
"$_PROBE_PYTHON" - "$TASKLIST_FILE" "$RUNNING_PROCS_FILE" <<'PYEOF'
import csv, hashlib, json, sys, os
rows = []
fname = sys.argv[1]
if os.path.exists(fname) and os.path.getsize(fname) > 0:
    with open(fname, encoding="utf-8-sig", errors="replace") as f:
        reader = csv.reader(f)
        first = True
        for row in reader:
            if first:
                first = False
                continue
            if len(row) < 2:
                continue
            name = row[0]
            try:
                pid = int(row[1])
            except Exception:
                continue
            h = hashlib.sha256(" ".join(row).encode("utf-8", "replace")).hexdigest()
            rows.append({"name": name, "pid": pid, "cmdline_hash": h})
            if len(rows) >= 100:
                break
with open(sys.argv[2], "w") as f:
    json.dump(rows, f)
PYEOF

# --- Parse schtasks LIST output into scheduled_tasks ---
SCHTASKS_JSON_FILE="$WORK_DIR/scheduled_tasks.json"
"$_PROBE_PYTHON" - "$SCHTASKS_FILE" "$SCHTASKS_JSON_FILE" <<'PYEOF'
import json, sys, os
entries = []
fname = sys.argv[1]
if os.path.exists(fname):
    cur = {}
    with open(fname, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.rstrip("\r\n")
            if not line.strip():
                if cur.get("name") and cur.get("state"):
                    entries.append({"name": cur["name"], "state": cur["state"]})
                cur = {}
                continue
            if ":" in line:
                k, _, v = line.partition(":")
                k = k.strip(); v = v.strip()
                if k == "TaskName":
                    cur["name"] = v.lstrip("\\")
                elif k == "Status":
                    cur["state"] = v
    if cur.get("name") and cur.get("state"):
        entries.append({"name": cur["name"], "state": cur["state"]})
with open(sys.argv[2], "w") as f:
    json.dump(entries[:100], f)
PYEOF

# --- Parse reg HKLM/HKCU Run into autostart_entries ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
"$_PROBE_PYTHON" - "$REG_HKLM_FILE" "$REG_HKCU_FILE" "$AUTOSTART_FILE" <<'PYEOF'
import json, sys, os
entries = []
for source, fname in (("HKLM_Run", sys.argv[1]), ("HKCU_Run", sys.argv[2])):
    if not os.path.exists(fname):
        continue
    with open(fname, encoding="utf-8", errors="replace") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("HKEY_"):
                continue
            parts = line.split(None, 2)
            if len(parts) == 3:
                entries.append({"source": source, "key": parts[0], "value": parts[2]})
with open(sys.argv[3], "w") as f:
    json.dump(entries, f)
PYEOF

# --- env_vars_hash from remote set output (NAMES only, never values) ---
ENV_HASH=""
if [ -f "$ENV_FILE" ] && [ -s "$ENV_FILE" ]; then
  ENV_HASH=$(tr -d '\r' < "$ENV_FILE" | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
fi
if [ -z "$ENV_HASH" ]; then
  # Empty-hash sentinel (sha256 of empty string) when no env data available.
  ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
fi

# --- Determine probe status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed: zero out binary and config data.
BINARY_SHA_FILE="$WORK_DIR/binary_sha.json"
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  BIN_SHA=""
  printf '{}' > "$CONFIG_HASH_FILE"
  printf '[]' > "$RUNNING_PROCS_FILE"
  printf '[]' > "$SCHTASKS_JSON_FILE"
  printf '[]' > "$AUTOSTART_FILE"
  BUILD_ID_VAL="null"
fi

if [ -n "$BIN_SHA" ]; then
  "$_PROBE_PYTHON" -c "import json,sys; print(json.dumps({'racecontrol.exe': sys.argv[1]}))" "$BIN_SHA" > "$BINARY_SHA_FILE"
else
  printf '{}' > "$BINARY_SHA_FILE"
fi

# --- Assemble final manifest JSON ---
MANIFEST_FILE="$WORK_DIR/manifest.json"
"$_PROBE_PYTHON" - \
  "$TARGET_ID" "$HOSTNAME_VAL" "$IP_VAL" "$ROLE_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$BINARY_SHA_FILE" "$BUILD_ID_VAL" "$CONFIG_HASH_FILE" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_JSON_FILE" "$AUTOSTART_FILE" \
  "$ENV_HASH" "$LAST_DEPLOY_VAL" \
  "$PROBE_ERRORS_FILE" \
  "$MANIFEST_FILE" <<'PYEOF'
import json, sys, os

(target_id, host, ip, role, probed_at, probe_status,
 binary_sha_file, build_id_json, config_hash_file,
 running_procs_file, schtasks_file, autostart_file,
 env_hash, last_deploy_json,
 probe_errors_file,
 out_file) = sys.argv[1:17]

def load_json(path, default):
    if os.path.exists(path):
        try:
            with open(path) as f:
                return json.load(f)
        except Exception:
            return default
    return default

m = {
    "schema_version":    "1.0",
    "target_id":         target_id,
    "host":              host,
    "ip":                ip,
    "role":              role,
    "probed_at_ist":     probed_at,
    "probe_status":      probe_status,
    "binary_sha256":     load_json(binary_sha_file, {}),
    "build_id":          json.loads(build_id_json),
    "config_hash":       load_json(config_hash_file, {}),
    "running_procs":     load_json(running_procs_file, []),
    "scheduled_tasks":   load_json(schtasks_file, []),
    "autostart_entries": load_json(autostart_file, []),
    "env_vars_hash":     env_hash,
    "last_deploy_ts":    json.loads(last_deploy_json),
}

errors = load_json(probe_errors_file, [])
if errors:
    m["probe_errors"] = errors

with open(out_file, "w") as f:
    json.dump(m, f)
PYEOF

# Read manifest JSON and hand off to write_manifest.
MANIFEST_JSON=$(cat "$MANIFEST_FILE")
write_manifest "$TARGET_ID" "$MANIFEST_JSON"

# Single-line stdout status summary.
TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
