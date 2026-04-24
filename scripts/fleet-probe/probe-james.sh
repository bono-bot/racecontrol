#!/bin/bash
# scripts/fleet-probe/probe-james.sh -- Phase 448 Plan 02
# Probes James .27 localhost: tasklist, schtasks, HKLM/HKCU Run, startup folder.
# Pure localhost -- never emits probe_failed (always-available class).
# Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-james.sh
#
# PROBE_PYTHON: allows tests on Windows to override 'python3' with the real interpreter path.
# On Windows, 'python3' may resolve to the App Execution Alias stub (Microsoft Store) which
# hangs silently in non-interactive contexts. Pass PROBE_PYTHON=python to use real Python 3.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

# _PROBE_PYTHON is set by probe-common.sh sourcing (uses PROBE_PYTHON env or defaults to python3)
PY_HELPERS="$SCRIPT_DIR/lib/james_helpers.py"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-james: MANIFEST_TS not set" >&2
  exit 2
fi

START_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" "$PY_HELPERS" elapsed_ms)
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
  PROBE_ERRORS_JSON=$("$_PROBE_PYTHON" "$PY_HELPERS" append_error "$PROBE_ERRORS_JSON" "$sp" "$msg")
}

# Work in a temp dir for intermediate data files.
WORK_DIR=$(mktemp -d /tmp/probe-james.XXXXXX)
trap 'rm -rf "$WORK_DIR"' EXIT

# --- sub_probe: running_procs (tasklist /V /FO CSV via cmd) ---
# Git Bash converts /V and /FO as Unix paths -- must use cmd //c to invoke tasklist.
# Output is saved to a temp file; Python reads from file (no stdin/heredoc).
RUNNING_PROCS_FILE="$WORK_DIR/tasklist.csv"
RUNNING_PROCS_JSON_FILE="$WORK_DIR/running_procs.json"
if cmd //c "tasklist /V /FO CSV" > "$RUNNING_PROCS_FILE" 2>/dev/null; then
  "$_PROBE_PYTHON" "$PY_HELPERS" parse_tasklist "$RUNNING_PROCS_FILE" "$RUNNING_PROCS_JSON_FILE"
else
  echo "[]" > "$RUNNING_PROCS_JSON_FILE"
  _add_probe_error "tasklist" "tasklist /V /FO CSV failed on james localhost"
fi
RUNNING_PROCS_JSON=$(cat "$RUNNING_PROCS_JSON_FILE")

# --- sub_probe: scheduled_tasks (schtasks /Query /V /FO LIST via cmd) ---
# Git Bash converts /Query as Unix path -- must use cmd //c.
SCHTASKS_FILE="$WORK_DIR/schtasks.txt"
SCHTASKS_JSON_FILE="$WORK_DIR/schtasks.json"
if cmd //c "schtasks /Query /V /FO LIST" > "$SCHTASKS_FILE" 2>/dev/null; then
  "$_PROBE_PYTHON" "$PY_HELPERS" parse_schtasks "$SCHTASKS_FILE" "$SCHTASKS_JSON_FILE"
else
  echo "[]" > "$SCHTASKS_JSON_FILE"
  _add_probe_error "schtasks_query" "schtasks /Query /V /FO LIST failed"
fi
SCHTASKS_JSON=$(cat "$SCHTASKS_JSON_FILE")

# --- sub_probe: autostart_entries (reg query HKLM/HKCU Run + startup folder) ---
AUTOSTART_FILE="$WORK_DIR/autostart.json"
"$_PROBE_PYTHON" "$PY_HELPERS" parse_autostart "$AUTOSTART_FILE"
AUTOSTART_JSON=$(cat "$AUTOSTART_FILE")

# --- sub_probe: env_vars_hash ---
ENV_HASH=$(env_names_hash)

# --- sub_probe: config_hash (best-effort -- comms-link config if present) ---
CONFIG_HASH_JSON="{}"
COMMS_CFG="$HOME/racingpoint/comms-link/config.toml"
if [ -f "$COMMS_CFG" ]; then
  COMMS_CFG_HASH=$(sha256_of "$COMMS_CFG")
  CONFIG_HASH_FILE="$WORK_DIR/config_hash.json"
  "$_PROBE_PYTHON" "$PY_HELPERS" make_config_hash "$COMMS_CFG_HASH" "$CONFIG_HASH_FILE"
  CONFIG_HASH_JSON=$(cat "$CONFIG_HASH_FILE")
fi

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || "$_PROBE_PYTHON" "$PY_HELPERS" elapsed_ms)
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors 0 "$ERR_COUNT_SUBPROBE")

# Assemble and write manifest JSON using write_manifest (which uses _PROBE_PYTHON via probe-common).
# Build the manifest as a JSON string using python (reads pre-computed JSON values from env vars).
MANIFEST_JSON=$("$_PROBE_PYTHON" - \
  "$TARGET_ID" "$HOSTNAME_VAL" "$IP_VAL" "$ROLE_VAL" \
  "$PROBED_AT" "$PROBE_STATUS" \
  "$ENV_HASH" \
  "$RUNNING_PROCS_JSON_FILE" "$SCHTASKS_JSON_FILE" "$AUTOSTART_FILE" \
  "$PROBE_ERRORS_JSON" "$CONFIG_HASH_JSON" << 'PYEOF'
import json, sys, os

(target_id, host, ip, role, probed_at, probe_status,
 env_hash,
 running_procs_file, schtasks_file, autostart_file,
 probe_errors_json, config_hash_json) = sys.argv[1:13]

running_procs = json.load(open(running_procs_file)) if os.path.exists(running_procs_file) else []
scheduled_tasks = json.load(open(schtasks_file)) if os.path.exists(schtasks_file) else []
autostart_entries = json.load(open(autostart_file)) if os.path.exists(autostart_file) else []

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

print(json.dumps(m))
PYEOF
)

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

# Single-line stdout status
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$ERR_COUNT_SUBPROBE"
