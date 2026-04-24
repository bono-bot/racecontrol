#!/bin/bash
# scripts/fleet-probe/probe-james.sh -- Phase 448 Plan 02
# Probes James .27 localhost: tasklist, schtasks, HKLM/HKCU Run, startup folder.
# Pure localhost -- never emits probe_failed (always-available class).
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-james.sh
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-james: MANIFEST_TS not set" >&2
  exit 2
fi

START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
TARGET_ID="james_27"
HOSTNAME_VAL="JAMES-PC"
IP_VAL="192.168.31.27"
ROLE_VAL="james"

PROBE_ERRORS_JSON="[]"
ERR_COUNT_SUBPROBE=0

# Helper: append an error entry to PROBE_ERRORS_JSON and increment ERR_COUNT_SUBPROBE.
# Usage: _add_probe_error "sub_probe_name" "error message"
_add_probe_error() {
  local sp="$1" msg="$2"
  ERR_COUNT_SUBPROBE=$((ERR_COUNT_SUBPROBE + 1))
  PROBE_ERRORS_JSON=$(python3 -c "
import json, sys
a = json.loads(sys.argv[1])
a.append({'sub_probe': sys.argv[2], 'error': sys.argv[3]})
print(json.dumps(a))
" "$PROBE_ERRORS_JSON" "$sp" "$msg")
}

# Work in a temp dir for intermediate data files.
WORK_DIR=$(mktemp -d /tmp/probe-james.XXXXXX)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- sub_probe: running_procs (tasklist /V /FO CSV via cmd) ---
# Git Bash converts /V and /FO as Unix paths -- must use cmd //c to invoke tasklist.
# Output is saved to a temp file; Python reads from file to avoid stdin/pipe/heredoc conflicts.
RUNNING_PROCS_FILE="$WORK_DIR/tasklist.csv"
RUNNING_PROCS_JSON="[]"
if cmd //c "tasklist /V /FO CSV" > "$RUNNING_PROCS_FILE" 2>/dev/null; then
  RUNNING_PROCS_JSON=$(python3 - "$RUNNING_PROCS_FILE" <<'PYEOF'
import csv, hashlib, json, sys
rows = []
with open(sys.argv[1], encoding='utf-8-sig', errors='replace') as f:
    reader = csv.reader(f)
    for i, row in enumerate(reader):
        if i == 0:
            continue  # skip header
        if len(row) < 2:
            continue
        name = row[0]
        try:
            pid = int(row[1])
        except Exception:
            continue
        cmdline = ' '.join(row)
        h = hashlib.sha256(cmdline.encode('utf-8', 'replace')).hexdigest()
        rows.append({'name': name, 'pid': pid, 'cmdline_hash': h})
print(json.dumps(rows))
PYEOF
)
else
  _add_probe_error "tasklist" "tasklist /V /FO CSV failed on james localhost"
fi

# --- sub_probe: scheduled_tasks (schtasks /Query /V /FO LIST via cmd) ---
# Git Bash converts /Query as Unix path -- must use cmd //c.
SCHTASKS_FILE="$WORK_DIR/schtasks.txt"
SCHTASKS_JSON="[]"
if cmd //c "schtasks /Query /V /FO LIST" > "$SCHTASKS_FILE" 2>/dev/null; then
  SCHTASKS_JSON=$(python3 - "$SCHTASKS_FILE" <<'PYEOF'
import json, sys
entries = []
cur = {}
with open(sys.argv[1], encoding='utf-8', errors='replace') as f:
    for line in f:
        line = line.rstrip('\r\n')
        if not line.strip():
            if cur.get('name') and cur.get('state'):
                entries.append({'name': cur['name'], 'state': cur['state']})
            cur = {}
            continue
        if ':' in line:
            k, _, v = line.partition(':')
            k = k.strip()
            v = v.strip()
            if k == 'TaskName':
                cur['name'] = v.lstrip('\\')
            elif k == 'Status':
                cur['state'] = v
if cur.get('name') and cur.get('state'):
    entries.append({'name': cur['name'], 'state': cur['state']})
print(json.dumps(entries[:100]))
PYEOF
)
else
  _add_probe_error "schtasks_query" "schtasks /Query /V /FO LIST failed"
fi

# --- sub_probe: autostart_entries (reg query HKLM/HKCU Run + startup folder) ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
python3 - "$AUTOSTART_FILE" <<'PYEOF'
import subprocess, os, json, sys
entries = []
for hive, source in (('HKLM', 'HKLM_Run'), ('HKCU', 'HKCU_Run')):
    try:
        out = subprocess.check_output(
            ['reg', 'query',
             hive + r'\SOFTWARE\Microsoft\Windows\CurrentVersion\Run'],
            text=True, stderr=subprocess.DEVNULL
        )
        for line in out.splitlines():
            line = line.strip()
            if not line or line.startswith('HKEY_'):
                continue
            parts = line.split(None, 2)
            if len(parts) == 3:
                entries.append({'source': source, 'key': parts[0], 'value': parts[2]})
    except Exception:
        pass
startup = os.path.expandvars(
    r'%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup'
)
if os.path.isdir(startup):
    for f in os.listdir(startup):
        entries.append({
            'source': 'startup_folder',
            'key': f,
            'value': os.path.join(startup, f)
        })
with open(sys.argv[1], 'w') as fp:
    json.dump(entries, fp)
PYEOF
AUTOSTART_JSON=$(cat "$AUTOSTART_FILE")

# --- sub_probe: env_vars_hash ---
ENV_HASH=$(env_names_hash)

# --- sub_probe: config_hash (best-effort -- comms-link config if present) ---
CONFIG_HASH_JSON="{}"
COMMS_CFG="$HOME/racingpoint/comms-link/config.toml"
if [ -f "$COMMS_CFG" ]; then
  COMMS_CFG_HASH=$(sha256_of "$COMMS_CFG")
  CONFIG_HASH_JSON=$(python3 -c "
import json, sys
print(json.dumps({'comms-link/config.toml': sys.argv[1]}))
" "$COMMS_CFG_HASH")
fi

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors 0 "$ERR_COUNT_SUBPROBE")

# Assemble manifest JSON via python3 (all sub-probe data from files + small env vars as args).
MANIFEST_FILE="$WORK_DIR/manifest.json"
python3 - \
  "$TARGET_ID" "$HOSTNAME_VAL" "$IP_VAL" "$ROLE_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$RUNNING_PROCS_FILE" "$SCHTASKS_FILE" "$AUTOSTART_FILE" \
  "$ENV_HASH" "$PROBE_ERRORS_JSON" "$CONFIG_HASH_JSON" \
  "$MANIFEST_FILE" <<'PYEOF'
import csv, hashlib, json, sys, os

(target_id, host, ip, role, probed_at, probe_status,
 running_procs_file, schtasks_file, autostart_file,
 env_hash, probe_errors_json, config_hash_json,
 out_file) = sys.argv[1:14]

# running_procs
running_procs = []
if os.path.exists(running_procs_file):
    with open(running_procs_file, encoding='utf-8-sig', errors='replace') as f:
        reader = csv.reader(f)
        for i, row in enumerate(reader):
            if i == 0:
                continue
            if len(row) < 2:
                continue
            name = row[0]
            try:
                pid = int(row[1])
            except Exception:
                continue
            cmdline = ' '.join(row)
            h = hashlib.sha256(cmdline.encode('utf-8', 'replace')).hexdigest()
            running_procs.append({'name': name, 'pid': pid, 'cmdline_hash': h})

# scheduled_tasks
scheduled_tasks = []
if os.path.exists(schtasks_file):
    cur = {}
    with open(schtasks_file, encoding='utf-8', errors='replace') as f:
        for line in f:
            line = line.rstrip('\r\n')
            if not line.strip():
                if cur.get('name') and cur.get('state'):
                    scheduled_tasks.append({'name': cur['name'], 'state': cur['state']})
                cur = {}
                continue
            if ':' in line:
                k, _, v = line.partition(':')
                k = k.strip()
                v = v.strip()
                if k == 'TaskName':
                    cur['name'] = v.lstrip('\\')
                elif k == 'Status':
                    cur['state'] = v
    if cur.get('name') and cur.get('state'):
        scheduled_tasks.append({'name': cur['name'], 'state': cur['state']})
    scheduled_tasks = scheduled_tasks[:100]

# autostart_entries
autostart_entries = []
if os.path.exists(autostart_file):
    with open(autostart_file) as f:
        autostart_entries = json.load(f)

m = {
    'schema_version':    '1.0',
    'target_id':         target_id,
    'host':              host,
    'ip':                ip,
    'role':              role,
    'probed_at_ist':     probed_at,
    'probe_status':      probe_status,
    'binary_sha256':     {},
    'build_id':          None,
    'config_hash':       json.loads(config_hash_json),
    'running_procs':     running_procs,
    'scheduled_tasks':   scheduled_tasks,
    'autostart_entries': autostart_entries,
    'env_vars_hash':     env_hash,
    'last_deploy_ts':    None,
}
errors = json.loads(probe_errors_json)
if errors:
    m['probe_errors'] = errors

with open(out_file, 'w') as f:
    json.dump(m, f)
PYEOF
MANIFEST_JSON=$(cat "$MANIFEST_FILE")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

# Single-line stdout status
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$ERR_COUNT_SUBPROBE"
