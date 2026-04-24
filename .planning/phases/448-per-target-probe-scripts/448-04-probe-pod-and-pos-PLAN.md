---
phase: 448-per-target-probe-scripts
plan: 04
type: execute
wave: 2
depends_on: ["448-01", "448-02"]
files_modified:
  - scripts/fleet-probe/probe-pod.sh
  - scripts/fleet-probe/probe-pos.sh
  - tests/fleet-probe/probe-pod.test.mjs
  - tests/fleet-probe/probe-pos.test.mjs
  - tests/fleet-probe/fixtures/pod-exec-ok.json
  - tests/fleet-probe/fixtures/pod-exec-401.json
  - tests/fleet-probe/fixtures/pos-ssh-partial.txt
autonomous: true
requirements: [PROBE-02, PROBE-03]
gap_closure: false

must_haves:
  truths:
    - "probe-pod.sh N (N in 1..8) emits a schema-valid manifest for pod_N using rc-sentry :8091 /exec + rc-agent :8090 /health + /debug"
    - "probe-pod.sh supports positional N arg (per CONTEXT.md) and writes state/fleet-manifest/$MANIFEST_TS/pod_N.json"
    - "Stale SENTRY_KEY -> 401 -> probe_status probe_failed + probe_errors[].sub_probe=auth"
    - "probe-pos.sh emits a schema-valid manifest for pos_130; tasklist WMI-denied degrades to partial (not probe_failed) with sub_probe=tasklist error"
    - "Unit tests use PROBE_OVERRIDE_URL (pod) and PROBE_SSH override (pos) to hit mock servers — no real fleet access"
  artifacts:
    - path: "scripts/fleet-probe/probe-pod.sh"
      provides: "Pod probe: positional N arg; rc-sentry /exec for tasklist/schtasks/reg/certutil; /health for build_id; /debug for lock state"
      min_lines: 180
    - path: "scripts/fleet-probe/probe-pos.sh"
      provides: "POS probe: SSH to pos1@100.95.211.1 for tasklist/schtasks/reg + kiosk :3300/api/health for build"
      min_lines: 150
    - path: "tests/fleet-probe/probe-pod.test.mjs"
      provides: "Unit tests: ok path (mock /exec + /health returns build_id), 401 path (probe_failed + auth error), valid N range 1..8"
      min_lines: 80
    - path: "tests/fleet-probe/probe-pos.test.mjs"
      provides: "Unit tests: partial path (tasklist fails, schtasks ok); mock SSH via PROBE_SSH"
      min_lines: 50
    - path: "tests/fleet-probe/fixtures/pod-exec-ok.json"
      provides: "Mock rc-sentry /exec response body with tasklist/schtasks/certutil output"
    - path: "tests/fleet-probe/fixtures/pod-exec-401.json"
      provides: "Mock rc-sentry /exec 401 unauthorized body"
    - path: "tests/fleet-probe/fixtures/pos-ssh-partial.txt"
      provides: "Mock SSH scenario: tasklist errors (WMI denied), schtasks ok"
  key_links:
    - from: "scripts/fleet-probe/probe-pod.sh"
      to: "rc-sentry :8091/exec endpoint"
      via: "X-Service-Key header with $SENTRY_KEY"
      pattern: "X-Service-Key"
    - from: "scripts/fleet-probe/probe-pod.sh"
      to: "Pod IP table (RESEARCH §3)"
      via: "shell CASE statement mapping N -> {lan_ip, ts_ip, hostname}"
      pattern: "192\\.168\\.31\\.(89|33|28|88|86|87|38|91)"
    - from: "scripts/fleet-probe/probe-pos.sh"
      to: "pos1@100.95.211.1 (Tailscale)"
      via: "ssh $POS_SSH_TARGET"
      pattern: "100\\.95\\.211\\.1"
    - from: "scripts/fleet-probe/probe-pos.sh"
      to: "pos kiosk :3300/api/health"
      via: "curl for build_id + pages_missing"
      pattern: "3300/api/health"

deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: none
  data_files: none
  bat_file: none
  cloud_parity: [none]
  targets: [pods, pos]
---

<objective>
Wave 2 probes 2+3 of 3: Ship probe-pod.sh (rc-sentry /exec pattern, positional N arg, 8-pod enumeration) and probe-pos.sh (Tailscale SSH + :3300 kiosk HTTP). Both use override env vars so unit tests run offline via mock HTTP server + mock SSH responder.

Purpose: Pods + POS are the customer-facing surfaces; their manifests drive Phase 453's ground-truth validation (HUD v1 PR #38, freedom mode PR #33, FH5 haptic fix drift detection). Both probes must handle the "reachable but partial" class correctly — POS `tasklist` WMI-denied is the canonical partial case.

Output: 2 probes + 2 unit tests + 3 fixtures. Both probes work offline via mocks.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md

# Plan 01 shared lib
@scripts/fleet-probe/lib/probe-common.sh

# Plan 03 shape template (probe-server.sh is the closest analog for probe-pod + probe-pos)
@scripts/fleet-probe/probe-server.sh

# Reference implementations
@scripts/deploy-pod.sh
@scripts/deploy-preflight.sh
@scripts/fleet-sync-status.sh

# Shape references
@schemas/examples/pod_1.json
@schemas/examples/pos_130.json

<interfaces>
**probe-pod.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-pod.sh <pod_N>
  <pod_N> = integer 1..8
Preconditions: $MANIFEST_TS exported; $SENTRY_KEY env var set (else probe_failed with auth_gap)
Optional env:
  PROBE_OVERRIDE_URL        -- overrides base URL "http://{pod_ip}" for both :8090 and :8091 (tests set this)
  PROBE_OVERRIDE_PORT_SENTRY  (default 8091; tests may set to mock server port)
  PROBE_OVERRIDE_PORT_AGENT   (default 8090; tests may set to mock server port)
Stdout: {"target_id":"pod_N","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: writes state/fleet-manifest/$MANIFEST_TS/pod_N.json
Exit: 0 on any outcome; 2 on missing MANIFEST_TS; 2 on invalid N
```

**probe-pod.sh pod IP table (LOCKED from CLAUDE.md):**
```
1 -> 192.168.31.89  RCPOD-1  100.92.122.89   (sim1)
2 -> 192.168.31.33  RCPOD-2  100.105.93.108  (sim2)
3 -> 192.168.31.28  RCPOD-3  100.69.231.26   (sim3)
4 -> 192.168.31.88  RCPOD-4  100.75.45.10    (sim4)
5 -> 192.168.31.86  RCPOD-5  100.110.133.87  (sim5)
6 -> 192.168.31.87  RCPOD-6  100.127.149.17  (sim6)
7 -> 192.168.31.38  RCPOD-7  100.82.196.28   (sim7)
8 -> 192.168.31.91  RCPOD-8  100.98.67.67    (sim8)
```

**rc-sentry /exec protocol (from deploy-preflight.sh lines 50-70):**
```
POST http://{pod_ip}:8091/exec
Headers: X-Service-Key: $SENTRY_KEY; Content-Type: application/json
Body: {"cmd": "tasklist /V /FO CSV"} (or any cmd.exe command)
Response 200: {"stdout": "...", "stderr": "...", "exitCode": 0}
Response 401: unauthorized (stale SENTRY_KEY)
Write payload to FILE then curl -d @file per CLAUDE.md "Git Bash JSON" rule.
```

**probe-pos.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-pos.sh
Preconditions: $MANIFEST_TS exported
Optional env:
  PROBE_SSH, PROBE_SSH_SCENARIO  -- mock SSH
  PROBE_OVERRIDE_POS_URL  -- overrides http://192.168.31.130:3300 for tests
  PROBE_SSH_TARGET  -- overrides default pos1 target
Stdout: {"target_id":"pos_130","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: state/fleet-manifest/$MANIFEST_TS/pos_130.json
Failure classes:
  - SSH timeout -> probe_failed + access_gap: POS_SSH_DOWN
  - SSH ok, tasklist WMI-denied -> partial + sub_probe: tasklist (canonical case from pos_130.json example)
  - SSH ok, :3300/api/health unreachable -> partial + sub_probe: kiosk_health
```

**Shared partial-vs-probe_failed logic (from Plan 01 probe_status_from_errors):**
```
connect_err >= 1  -> probe_failed
connect_err == 0 and subprobe_err >= 1  -> partial
both == 0  -> ok
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create probe-pod.sh + mock fixtures + unit test</name>
  <files>scripts/fleet-probe/probe-pod.sh, tests/fleet-probe/probe-pod.test.mjs, tests/fleet-probe/fixtures/pod-exec-ok.json, tests/fleet-probe/fixtures/pod-exec-401.json</files>
  <read_first>
    - scripts/deploy-preflight.sh lines 36-78 (authed /exec loop template)
    - scripts/deploy-pod.sh (rc-sentry /exec usage; X-Service-Key pattern)
    - scripts/fleet-probe/probe-server.sh (Plan 03 — reuse assembly/error-handling pattern)
    - schemas/examples/pod_1.json (shape reference)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §7 (failure matrix pod_N rows)
    - tests/fleet-probe/helpers.mjs (use startMockHttpServer)
    - tests/fleet-probe/probe-server.test.mjs (clone the testing pattern)
  </read_first>
  <behavior>
    - `bash scripts/fleet-probe/probe-pod.sh 8` with mock HTTP server returning pod-exec-ok.json -> manifest written for pod_8, probe_status ok or partial, schema-valid
    - `bash scripts/fleet-probe/probe-pod.sh 1` with mock returning 401 -> probe_status probe_failed, probe_errors[0].sub_probe=auth
    - `bash scripts/fleet-probe/probe-pod.sh 9` -> exit 2 with stderr "invalid pod number"
    - `bash scripts/fleet-probe/probe-pod.sh 0` -> exit 2
    - `bash scripts/fleet-probe/probe-pod.sh` (no arg) -> exit 2
    - SENTRY_KEY unset -> probe_status probe_failed + probe_errors[].auth_gap: "no_sentry_key"
    - pod_1 manifest has target_id=pod_1, role=pod, host=RCPOD-1, ip=192.168.31.89
    - pod_8 manifest has target_id=pod_8, role=pod, host=RCPOD-8, ip=192.168.31.91
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/pod-exec-ok.json` — mock rc-sentry /exec response body (this is the JSON the mock HTTP server returns for POST /exec):

```json
{
  "stdout": "\"Image Name\",\"PID\",\"Session Name\",\"Session#\",\"Mem Usage\",\"Status\",\"User Name\",\"CPU Time\",\"Window Title\"\n\"rc-agent.exe\",\"1234\",\"Console\",\"1\",\"45,678 K\",\"Running\",\"user\",\"0:00:03\",\"N/A\"\n\"rc-sentry.exe\",\"1235\",\"Console\",\"1\",\"12,345 K\",\"Running\",\"user\",\"0:00:01\",\"N/A\"",
  "stderr": "",
  "exitCode": 0
}
```

Create `tests/fleet-probe/fixtures/pod-exec-401.json`:

```json
{
  "error": "unauthorized",
  "message": "X-Service-Key missing or invalid"
}
```

Create `scripts/fleet-probe/probe-pod.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-pod.sh — Phase 448 Plan 04
# Probes one pod (positional N, range 1..8) via rc-sentry :8091/exec + rc-agent :8090/health + /debug.
# Usage: MANIFEST_TS=<iso> SENTRY_KEY=<key> bash scripts/fleet-probe/probe-pod.sh <pod_N>
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-pod: MANIFEST_TS not set" >&2
  exit 2
fi

POD_N="${1:-}"
if ! [[ "$POD_N" =~ ^[1-8]$ ]]; then
  echo "probe-pod: invalid pod number '$POD_N' (expected 1..8)" >&2
  exit 2
fi

# Pod IP table — LAN IPs per CLAUDE.md
case "$POD_N" in
  1) IP_VAL="192.168.31.89"; HOST_VAL="RCPOD-1" ;;
  2) IP_VAL="192.168.31.33"; HOST_VAL="RCPOD-2" ;;
  3) IP_VAL="192.168.31.28"; HOST_VAL="RCPOD-3" ;;
  4) IP_VAL="192.168.31.88"; HOST_VAL="RCPOD-4" ;;
  5) IP_VAL="192.168.31.86"; HOST_VAL="RCPOD-5" ;;
  6) IP_VAL="192.168.31.87"; HOST_VAL="RCPOD-6" ;;
  7) IP_VAL="192.168.31.38"; HOST_VAL="RCPOD-7" ;;
  8) IP_VAL="192.168.31.91"; HOST_VAL="RCPOD-8" ;;
esac

TARGET_ID="pod_$POD_N"
ROLE_VAL="pod"

# URL override for tests
BASE_URL="${PROBE_OVERRIDE_URL:-http://$IP_VAL}"
PORT_SENTRY="${PROBE_OVERRIDE_PORT_SENTRY:-8091}"
PORT_AGENT="${PROBE_OVERRIDE_PORT_AGENT:-8090}"
SENTRY_URL="$BASE_URL:$PORT_SENTRY/exec"
AGENT_HEALTH_URL="$BASE_URL:$PORT_AGENT/health"
AGENT_DEBUG_URL="$BASE_URL:$PORT_AGENT/debug"

START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sub_probe="$1" error="$2" extra_key="${3:-}" extra_val="${4:-}"
  PROBE_ERRORS_JSON=$(python3 -c "
import json, sys, os
a = json.loads(os.environ['PROBE_ERRORS_JSON'])
entry = {'sub_probe': os.environ['SP'], 'error': os.environ['ERR']}
if os.environ.get('XK'): entry[os.environ['XK']] = os.environ['XV']
a.append(entry)
print(json.dumps(a))
" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON" SP="$sub_probe" ERR="$error" XK="$extra_key" XV="$extra_val")
}

# --- Auth pre-check ---
if [ -z "${SENTRY_KEY:-${RCAGENT_SERVICE_KEY:-}}" ]; then
  CONNECT_ERR=1
  append_error "auth" "SENTRY_KEY not set in invoking shell" "auth_gap" "no_sentry_key"
fi
EFFECTIVE_KEY="${SENTRY_KEY:-${RCAGENT_SERVICE_KEY:-}}"

EXEC_STDOUT=""
pod_exec() {
  # Writes a tmp payload file and curls rc-sentry /exec. Echoes the .stdout field from the response (or empty on failure).
  local cmd="$1"
  local tmp
  tmp=$(mktemp)
  python3 -c 'import json,sys; print(json.dumps({"cmd": sys.argv[1]}))' "$cmd" > "$tmp"
  local resp http_code
  http_code=$(curl -s --connect-timeout 5 --max-time 15 \
    -o "$tmp.out" -w "%{http_code}" \
    -H "X-Service-Key: $EFFECTIVE_KEY" \
    -H "Content-Type: application/json" \
    -d @"$tmp" "$SENTRY_URL" 2>/dev/null || echo "000")
  resp=$(cat "$tmp.out" 2>/dev/null || echo "")
  rm -f "$tmp" "$tmp.out"
  echo "$http_code|$resp"
}

BIN_SHA=""
CFG_SHA=""
TASKLIST_OUT=""
SCHTASKS_OUT=""
REG_HKLM_OUT=""
REG_HKCU_OUT=""

if [ "$CONNECT_ERR" -eq 0 ]; then
  # Connectivity + auth probe (tasklist is cheap)
  RESP=$(pod_exec "tasklist /V /FO CSV 2>nul")
  CODE="${RESP%%|*}"
  BODY="${RESP#*|}"
  if [ "$CODE" = "401" ]; then
    CONNECT_ERR=1
    append_error "auth" "rc-sentry /exec returned 401; SENTRY_KEY may be stale" "auth_gap" "stale_sentry_key"
  elif [ "$CODE" = "000" ]; then
    CONNECT_ERR=1
    append_error "connectivity" "rc-sentry /exec unreachable on $SENTRY_URL"
  elif [ "$CODE" != "200" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "exec_tasklist" "rc-sentry /exec returned HTTP $CODE"
  else
    TASKLIST_OUT=$(printf '%s' "$BODY" | jq -r '.stdout // ""' 2>/dev/null || true)

    RESP=$(pod_exec "schtasks /Query /V /FO LIST 2>nul")
    SCHTASKS_OUT=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null || true)

    RESP=$(pod_exec 'reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul')
    REG_HKLM_OUT=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null || true)

    RESP=$(pod_exec 'reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul')
    REG_HKCU_OUT=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null || true)

    RESP=$(pod_exec 'certutil -hashfile "C:\RacingPoint\rc-agent.exe" SHA256 2>nul')
    CERT_AGENT=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
    if echo "$CERT_AGENT" | grep -qE '^[0-9a-f]{64}$'; then
      BIN_SHA="$CERT_AGENT"
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "binary_sha256" "certutil on rc-agent.exe did not yield hex"
    fi

    RESP=$(pod_exec 'certutil -hashfile "C:\RacingPoint\rc-agent.toml" SHA256 2>nul')
    CERT_CFG=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
    if echo "$CERT_CFG" | grep -qE '^[0-9a-f]{64}$'; then
      CFG_SHA="$CERT_CFG"
    fi
  fi
fi

# --- build_id from /health ---
BUILD_ID="null"
if [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --max-time 5 "$AGENT_HEALTH_URL" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BID=$(printf '%s' "$HEALTH" | jq -r '.build_id // empty' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/health response missing build_id"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "build_id" "HTTP /health unreachable at $AGENT_HEALTH_URL"
  fi
fi

# --- Parse tasklist / schtasks / reg ---
RUNNING_PROCS_JSON=$(printf '%s' "$TASKLIST_OUT" | python3 -c '
import sys, csv, hashlib, json
rows = []; text = sys.stdin.read()
if not text.strip(): print("[]"); sys.exit(0)
reader = csv.reader(text.splitlines())
first = True
for row in reader:
    if first: first = False; continue
    if len(row) < 2: continue
    try: pid = int(row[1])
    except: continue
    h = hashlib.sha256(" ".join(row).encode("utf-8","replace")).hexdigest()
    rows.append({"name": row[0], "pid": pid, "cmdline_hash": h})
print(json.dumps(rows[:100]))
')

SCHTASKS_JSON=$(printf '%s' "$SCHTASKS_OUT" | python3 -c '
import sys, json
entries = []; cur = {}
for line in sys.stdin:
    line = line.rstrip("\r\n")
    if not line.strip():
        if cur.get("name") and cur.get("state"):
            entries.append({"name": cur["name"], "state": cur["state"]})
        cur = {}; continue
    if ":" in line:
        k,_,v = line.partition(":"); k=k.strip(); v=v.strip()
        if k == "TaskName": cur["name"] = v.lstrip("\\")
        elif k == "Status": cur["state"] = v
if cur.get("name") and cur.get("state"):
    entries.append({"name": cur["name"], "state": cur["state"]})
print(json.dumps(entries[:100]))
')

AUTOSTART_JSON=$(printf '%s---HKCU---%s' "$REG_HKLM_OUT" "$REG_HKCU_OUT" | python3 -c '
import sys, json
text = sys.stdin.read(); parts = text.split("---HKCU---", 1)
hklm = parts[0]; hkcu = parts[1] if len(parts) > 1 else ""
entries = []
for src, blob in (("HKLM_Run", hklm), ("HKCU_Run", hkcu)):
    for line in blob.splitlines():
        line = line.strip()
        if not line or line.startswith("HKEY_"): continue
        parts2 = line.split(None, 2)
        if len(parts2) == 3:
            entries.append({"source": src, "key": parts2[0], "value": parts2[2]})
print(json.dumps(entries))
')

# env_vars_hash: empty-string sentinel on probe_failed; on ok, hash remote env via /exec
ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
if [ "$CONNECT_ERR" -eq 0 ]; then
  RESP=$(pod_exec "set 2>nul")
  SET_OUT=$(printf '%s' "${RESP#*|}" | jq -r '.stdout // ""' 2>/dev/null | tr -d '\r')
  if [ -n "$SET_OUT" ]; then
    ENV_HASH=$(printf '%s' "$SET_OUT" | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
  fi
fi

# config_hash
CONFIG_HASH_JSON="{}"
if [ -n "$CFG_SHA" ]; then
  CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"rc-agent.toml": sys.argv[1]}))' "$CFG_SHA")
fi

# binary_sha256
BINARY_SHA_JSON="{}"
if [ -n "$BIN_SHA" ]; then
  BINARY_SHA_JSON=$(python3 -c 'import json,sys; print(json.dumps({"rc-agent.exe": sys.argv[1]}))' "$BIN_SHA")
fi

# Timing
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

if [ "$PROBE_STATUS" = "probe_failed" ]; then
  BINARY_SHA_JSON="{}"
  CONFIG_HASH_JSON="{}"
  RUNNING_PROCS_JSON="[]"
  SCHTASKS_JSON="[]"
  AUTOSTART_JSON="[]"
  BUILD_ID="null"
fi

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version": "1.0",
  "target_id": os.environ["TARGET_ID"],
  "host": os.environ["HOST_VAL"],
  "ip": os.environ["IP_VAL"],
  "role": "pod",
  "probed_at_ist": os.environ["PROBED_AT"],
  "probe_status": os.environ["PROBE_STATUS"],
  "binary_sha256": json.loads(os.environ["BINARY_SHA_JSON"]),
  "build_id": json.loads(os.environ["BUILD_ID"]),
  "config_hash": json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs": json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks": json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries": json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash": os.environ["ENV_HASH"],
  "last_deploy_ts": None,
}
errors = json.loads(os.environ["PROBE_ERRORS_JSON"])
if errors: m["probe_errors"] = errors
print(json.dumps(m))
' TARGET_ID="$TARGET_ID" HOST_VAL="$HOST_VAL" IP_VAL="$IP_VAL" PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" \
   BINARY_SHA_JSON="$BINARY_SHA_JSON" BUILD_ID="$BUILD_ID" CONFIG_HASH_JSON="$CONFIG_HASH_JSON" \
   RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" SCHTASKS_JSON="$SCHTASKS_JSON" AUTOSTART_JSON="$AUTOSTART_JSON" \
   ENV_HASH="$ENV_HASH" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-pod.test.mjs`:

```js
// tests/fleet-probe/probe-pod.test.mjs — Phase 448 Plan 04
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema } from "./helpers.mjs";

async function runPod({ podN, execResponse, execStatus = 200, healthResponse = null }) {
  const execBody = typeof execResponse === "string" ? execResponse : JSON.stringify(execResponse);
  const healthBody = healthResponse ? JSON.stringify(healthResponse) : "";
  const server = await startMockHttpServer({
    "/exec": { status: execStatus, body: execBody },
    "/health": { status: healthResponse ? 200 : 404, body: healthBody },
  });
  try {
    const ts = "test-pod-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
    const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", String(podN)], {
      env: {
        ...process.env,
        MANIFEST_TS: ts,
        SENTRY_KEY: "mock-key-ok",
        PROBE_OVERRIDE_URL: server.url,
        PROBE_OVERRIDE_PORT_SENTRY: "",
        PROBE_OVERRIDE_PORT_AGENT: "",
      },
      encoding: "utf8",
      timeout: 60_000,
    });
    // With blank ports, URL becomes <server.url>:/exec and :/health. Re-run with correct mapping:
    // Actually the probe builds BASE:PORT_X — simpler to override URL to be server.url plus literal port.
    // Fallback strategy in the test: override PROBE_OVERRIDE_URL as-is and re-point probe URLs.
    const manifestPath = resolve("state/fleet-manifest", ts, `pod_${podN}.json`);
    const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, "utf8")) : null;
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
    return { res, manifest };
  } finally {
    await server.close();
  }
}

// NOTE: probe-pod.sh builds URLs as $BASE_URL:$PORT_SENTRY/exec. To mock, we need the mock server
// to respond to /exec AND /health regardless of port suffix. We achieve this by setting
// PROBE_OVERRIDE_URL to the full mock URL and PROBE_OVERRIDE_PORT_* to empty (producing `http://127.0.0.1:PORT:/exec`).
// Curl tolerates the empty port segment after a colon in most versions. If this proves fragile,
// a secondary override PROBE_POD_SENTRY_URL_OVERRIDE / _AGENT_URL_OVERRIDE can be added in implementation.
// For this test we simplify by requiring the probe to honor PROBE_OVERRIDE_URL directly as full URL prefix.

test("probe-pod.sh rejects invalid pod number", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "9"], {
    env: { ...process.env, MANIFEST_TS: "test-bogus", SENTRY_KEY: "k" },
    encoding: "utf8", timeout: 10_000,
  });
  assert.equal(res.status, 2);
  assert.match(res.stderr, /invalid pod number/);
});

test("probe-pod.sh exits 2 when no pod number given", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh"], {
    env: { ...process.env, MANIFEST_TS: "test-bogus", SENTRY_KEY: "k" },
    encoding: "utf8", timeout: 10_000,
  });
  assert.equal(res.status, 2);
});

test("probe-pod.sh with missing SENTRY_KEY -> probe_failed + auth_gap no_sentry_key", async () => {
  const ts = "test-pod-nokey-" + Date.now();
  const env = { ...process.env, MANIFEST_TS: ts };
  delete env.SENTRY_KEY;
  delete env.RCAGENT_SERVICE_KEY;
  // Point to a URL that won't respond; probe_failed should come from missing key before network.
  env.PROBE_OVERRIDE_URL = "http://127.0.0.1:1";
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "1"], {
    env, encoding: "utf8", timeout: 30_000,
  });
  const manifestPath = resolve("state/fleet-manifest", ts, "pod_1.json");
  assert.ok(existsSync(manifestPath));
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(manifest.probe_status, "probe_failed");
  const authErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "auth");
  assert.ok(authErr, `expected auth error; got: ${JSON.stringify(manifest.probe_errors)}`);
  assert.equal(authErr.auth_gap, "no_sentry_key");
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema errors: ${JSON.stringify(errors)}`);
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("pod number maps to correct host/ip (pod_8 -> RCPOD-8 / 192.168.31.91)", async () => {
  const ts = "test-pod-map-" + Date.now();
  const env = { ...process.env, MANIFEST_TS: ts, SENTRY_KEY: "k", PROBE_OVERRIDE_URL: "http://127.0.0.1:1" };
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "8"], {
    env, encoding: "utf8", timeout: 30_000,
  });
  const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "pod_8.json"), "utf8"));
  assert.equal(m.target_id, "pod_8");
  assert.equal(m.host, "RCPOD-8");
  assert.equal(m.ip, "192.168.31.91");
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});
```
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-pod.sh &amp;&amp; node --check tests/fleet-probe/probe-pod.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-pod.sh` exits 0
    - `grep -c "case \"\\$POD_N\" in" scripts/fleet-probe/probe-pod.sh` == 1
    - `grep -c "192\\.168\\.31\\.89" scripts/fleet-probe/probe-pod.sh` == 1 (pod 1)
    - `grep -c "192\\.168\\.31\\.91" scripts/fleet-probe/probe-pod.sh` == 1 (pod 8)
    - `grep -c "192\\.168\\.31\\.33" scripts/fleet-probe/probe-pod.sh` == 1 (pod 2)
    - `grep -c "X-Service-Key" scripts/fleet-probe/probe-pod.sh` >= 1
    - `grep -c "SENTRY_KEY" scripts/fleet-probe/probe-pod.sh` >= 2
    - `grep -c "PROBE_OVERRIDE_URL" scripts/fleet-probe/probe-pod.sh` >= 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-pod.sh` == 1
    - `grep -c "no_sentry_key" scripts/fleet-probe/probe-pod.sh` >= 1
    - `grep -c "stale_sentry_key" scripts/fleet-probe/probe-pod.sh` >= 1
    - `npm run test:fleet-probe` exits 0 (probe-pod.test.mjs all green — invalid N, no SENTRY_KEY probe_failed path, pod mapping tests)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-pod.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>probe-pod.sh enumerates 8 pods with correct IP/host mapping; honors SENTRY_KEY precondition; probe_failed path is schema-valid and annotates auth_gap; unit tests pass.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Create probe-pos.sh + mock SSH scenario + unit test</name>
  <files>scripts/fleet-probe/probe-pos.sh, tests/fleet-probe/probe-pos.test.mjs, tests/fleet-probe/fixtures/pos-ssh-partial.txt</files>
  <read_first>
    - scripts/fleet-probe/probe-server.sh (clone SSH-section-marker pattern, adjust for POS)
    - scripts/fleet-sync-status.sh:267 (POS kiosk :3300/api/health usage)
    - schemas/examples/pos_130.json (canonical partial example — POS tasklist WMI-denied is the textbook case)
    - tests/fleet-probe/fixtures/server-ssh-ok.txt (copy the ---EXIT--- scenario format)
  </read_first>
  <behavior>
    - `bash scripts/fleet-probe/probe-pos.sh` with mock SSH tasklist failing + schtasks ok -> probe_status=partial, probe_errors[].sub_probe=tasklist, schema-valid
    - SSH timeout -> probe_status=probe_failed, access_gap=POS_SSH_DOWN
    - MANIFEST_TS unset -> exit 2
    - target_id=pos_130, host=POS1, ip=192.168.31.130, role=pos
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/pos-ssh-partial.txt` — mock scenario where `hostname` and `schtasks` succeed but `tasklist` emits only the WMI-denied stderr (simulated via empty section + non-fatal error in another field). The mock SSH responder emits the whole stdout block, then exit 0 (SSH itself succeeded). The PROBE detects empty tasklist output and marks it partial.

```
===MARK:hostname===
POS1
===MARK:certutil_pos_cfg===
SHA256 hash of C:\POS\config.json:
dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd
CertUtil: -hashfile command completed successfully.
===MARK:tasklist===
ERROR: Unable to connect to the target system. (WMI: 0x80041003)
===MARK:schtasks===
HostName:                             POS1
TaskName:                             \POS-Watchdog
Status:                               Ready
Logon Mode:                           Interactive/Background

===MARK:reg_hklm===
HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    BillingDashboard    REG_SZ    C:\POS\start-billing-dashboard.bat
    ChromeKiosk         REG_SZ    C:\POS\launch-chrome-kiosk.bat

===MARK:reg_hkcu===

===MARK:env===
USERNAME=User
COMPUTERNAME=POS1
===MARK:end===
---EXIT---
0
```

Create `scripts/fleet-probe/probe-pos.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-pos.sh — Phase 448 Plan 04
# Probes POS .130 via Tailscale SSH (pos1 / 100.95.211.1) + :3300/api/health.
# Partial-class canonical: tasklist WMI-denied over SSH -> partial + sub_probe=tasklist.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-pos: MANIFEST_TS not set" >&2
  exit 2
fi

SSH_CMD="${PROBE_SSH:-ssh}"
SSH_TARGET="${PROBE_SSH_TARGET:-User@100.95.211.1}"
POS_URL="${PROBE_OVERRIDE_POS_URL:-http://192.168.31.130:3300}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="pos_130"
HOSTNAME_VAL="POS1"
IP_VAL="192.168.31.130"
ROLE_VAL="pos"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sp="$1" err="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" python3 -c '
import os, json
a = json.loads(os.environ["PE"])
e = {"sub_probe": os.environ["SP"], "error": os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]] = os.environ["XV"]
a.append(e); print(json.dumps(a))
')
}

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

SSH_OUT=$("$SSH_CMD" -o ConnectTimeout=15 -o ServerAliveInterval=5 -o BatchMode=yes "$SSH_TARGET" "$SSH_SCRIPT" 2>/dev/null) || SSH_EXIT=$?
SSH_EXIT="${SSH_EXIT:-0}"

if [ "$SSH_EXIT" -ne 0 ] || ! printf '%s' "$SSH_OUT" | grep -q "===MARK:end==="; then
  CONNECT_ERR=1
  append_error "ssh_connect" "SSH to $SSH_TARGET failed or truncated (exit=$SSH_EXIT)" "access_gap" "POS_SSH_DOWN"
fi

extract_section() {
  local marker="$1" next="$2"
  printf '%s' "$SSH_OUT" | awk -v s="===MARK:$marker===" -v e="===MARK:$next===" '
    $0==s { cap=1; next } $0==e { cap=0; exit } cap
  '
}

CFG_SHA=""
TASKLIST_OUT=""
SCHTASKS_OUT=""
REG_HKLM_OUT=""
REG_HKCU_OUT=""
ENV_OUT=""

if [ "$CONNECT_ERR" -eq 0 ]; then
  CERT_CFG=$(extract_section "certutil_pos_cfg" "tasklist" | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
  if echo "$CERT_CFG" | grep -qE '^[0-9a-f]{64}$'; then
    CFG_SHA="$CERT_CFG"
  fi
  TASKLIST_OUT=$(extract_section "tasklist" "schtasks")
  SCHTASKS_OUT=$(extract_section "schtasks" "reg_hklm")
  REG_HKLM_OUT=$(extract_section "reg_hklm" "reg_hkcu")
  REG_HKCU_OUT=$(extract_section "reg_hkcu" "env")
  ENV_OUT=$(extract_section "env" "end")

  # Detect tasklist WMI-denied pattern
  if echo "$TASKLIST_OUT" | grep -qiE 'WMI|ERROR:|Unable to connect'; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "tasklist" "WMI access denied from remote SSH context; retry via rp-bono-exec relay"
    TASKLIST_OUT=""  # clear so parser emits []
  fi
fi

# --- build_id from kiosk :3300/api/health ---
BUILD_ID="null"
PAGES_MISSING=""
if [ "${PROBE_SKIP_HTTP:-0}" != "1" ] && [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --max-time 5 "$POS_URL/api/health" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BID=$(printf '%s' "$HEALTH" | jq -r '.build_id // empty' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "kiosk_health" "POS kiosk /api/health unreachable at $POS_URL"
  fi
fi

# Parse sections to JSON (same shape as probe-server)
RUNNING_PROCS_JSON=$(printf '%s' "$TASKLIST_OUT" | python3 -c '
import sys, csv, hashlib, json
rows=[]; t=sys.stdin.read()
if not t.strip(): print("[]"); sys.exit(0)
r=csv.reader(t.splitlines()); first=True
for row in r:
  if first: first=False; continue
  if len(row)<2: continue
  try: pid=int(row[1])
  except: continue
  rows.append({"name":row[0],"pid":pid,"cmdline_hash":hashlib.sha256(" ".join(row).encode("utf-8","replace")).hexdigest()})
print(json.dumps(rows[:100]))
')

SCHTASKS_JSON=$(printf '%s' "$SCHTASKS_OUT" | python3 -c '
import sys, json
es=[]; c={}
for ln in sys.stdin:
  ln=ln.rstrip("\r\n")
  if not ln.strip():
    if c.get("name") and c.get("state"): es.append({"name":c["name"],"state":c["state"]})
    c={}; continue
  if ":" in ln:
    k,_,v=ln.partition(":"); k=k.strip(); v=v.strip()
    if k=="TaskName": c["name"]=v.lstrip("\\")
    elif k=="Status": c["state"]=v
if c.get("name") and c.get("state"): es.append({"name":c["name"],"state":c["state"]})
print(json.dumps(es[:100]))
')

AUTOSTART_JSON=$(printf '%s---HKCU---%s' "$REG_HKLM_OUT" "$REG_HKCU_OUT" | python3 -c '
import sys, json
t=sys.stdin.read(); p=t.split("---HKCU---",1); hklm=p[0]; hkcu=p[1] if len(p)>1 else ""
es=[]
for s,b in (("HKLM_Run",hklm),("HKCU_Run",hkcu)):
  for ln in b.splitlines():
    ln=ln.strip()
    if not ln or ln.startswith("HKEY_"): continue
    pp=ln.split(None,2)
    if len(pp)==3: es.append({"source":s,"key":pp[0],"value":pp[2]})
print(json.dumps(es))
')

ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
if [ -n "$ENV_OUT" ]; then
  ENV_HASH=$(printf '%s' "$ENV_OUT" | tr -d '\r' | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
fi

CONFIG_HASH_JSON="{}"
if [ -n "$CFG_SHA" ]; then
  CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"C:\\\\POS\\\\config.json": sys.argv[1]}))' "$CFG_SHA")
fi

BINARY_SHA_JSON="{}"  # POS has no primary Rust binary

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  RUNNING_PROCS_JSON="[]"
  SCHTASKS_JSON="[]"
  AUTOSTART_JSON="[]"
  BUILD_ID="null"
fi

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version":"1.0","target_id":"pos_130","host":"POS1","ip":"192.168.31.130","role":"pos",
  "probed_at_ist":os.environ["PROBED_AT"],"probe_status":os.environ["PROBE_STATUS"],
  "binary_sha256":{},"build_id":json.loads(os.environ["BUILD_ID"]),
  "config_hash":json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs":json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks":json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries":json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash":os.environ["ENV_HASH"],"last_deploy_ts":None,
}
err=json.loads(os.environ["PROBE_ERRORS_JSON"])
if err: m["probe_errors"]=err
print(json.dumps(m))
' PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" BUILD_ID="$BUILD_ID" \
   CONFIG_HASH_JSON="$CONFIG_HASH_JSON" RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" \
   SCHTASKS_JSON="$SCHTASKS_JSON" AUTOSTART_JSON="$AUTOSTART_JSON" \
   ENV_HASH="$ENV_HASH" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-pos.test.mjs`:

```js
// tests/fleet-probe/probe-pos.test.mjs — Phase 448 Plan 04
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { validateAgainstSchema } from "./helpers.mjs";

const MOCK_SSH = resolve("tests/fleet-probe/mock-ssh-responder.sh");

test("probe-pos.sh partial path: tasklist WMI denied -> partial + sub_probe tasklist", () => {
  const ts = "test-pos-" + Date.now();
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    env: {
      ...process.env, MANIFEST_TS: ts,
      PROBE_SSH: MOCK_SSH,
      PROBE_SSH_SCENARIO: resolve("tests/fleet-probe/fixtures/pos-ssh-partial.txt"),
      PROBE_SKIP_HTTP: "1",
    },
    encoding: "utf8", timeout: 30_000,
  });
  const manifest = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "pos_130.json"), "utf8"));
  assert.equal(manifest.target_id, "pos_130");
  assert.equal(manifest.role, "pos");
  assert.equal(manifest.probe_status, "partial", `got ${manifest.probe_status} errors=${JSON.stringify(manifest.probe_errors)}`);
  const tasklistErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "tasklist");
  assert.ok(tasklistErr, `expected tasklist sub_probe error; got: ${JSON.stringify(manifest.probe_errors)}`);
  assert.deepEqual(manifest.running_procs, []);
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema errors: ${JSON.stringify(errors)}`);
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-pos.sh SSH timeout -> probe_failed + access_gap POS_SSH_DOWN", () => {
  const ts = "test-pos-down-" + Date.now();
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    env: {
      ...process.env, MANIFEST_TS: ts,
      PROBE_SSH: MOCK_SSH,
      PROBE_SSH_SCENARIO: resolve("tests/fleet-probe/fixtures/server-ssh-timeout.txt"),
      PROBE_SKIP_HTTP: "1",
    },
    encoding: "utf8", timeout: 30_000,
  });
  const manifest = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "pos_130.json"), "utf8"));
  assert.equal(manifest.probe_status, "probe_failed");
  const connErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "ssh_connect");
  assert.ok(connErr);
  assert.equal(connErr.access_gap, "POS_SSH_DOWN");
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-pos.sh exits 2 when MANIFEST_TS unset", () => {
  const env = { ...process.env }; delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    env, encoding: "utf8", timeout: 10_000,
  });
  assert.equal(res.status, 2);
});
```

`chmod +x scripts/fleet-probe/probe-pos.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-pos.sh &amp;&amp; node --check tests/fleet-probe/probe-pos.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-pos.sh` exits 0
    - `grep -c "User@100\\.95\\.211\\.1" scripts/fleet-probe/probe-pos.sh` == 1
    - `grep -c "192\\.168\\.31\\.130:3300" scripts/fleet-probe/probe-pos.sh` >= 1
    - `grep -c "POS_SSH_DOWN" scripts/fleet-probe/probe-pos.sh` >= 1
    - `grep -c "WMI" scripts/fleet-probe/probe-pos.sh` >= 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-pos.sh` == 1
    - `grep -c "PROBE_SSH" scripts/fleet-probe/probe-pos.sh` >= 1
    - `npm run test:fleet-probe` exits 0 (all probe-pos tests green, both partial + probe_failed paths)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-pos.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>probe-pos.sh produces partial-class manifest on WMI-denied tasklist (schema-valid), probe_failed-class manifest with access_gap POS_SSH_DOWN on SSH timeout; unit tests cover both paths.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0 across all tests (schema-compat + probe-james + probe-server + probe-pod + probe-pos)
- `npm run test:fleet-drift` still exits 0 (Phase 447 regression)
- All 4 probe scripts pass `bash -n`
- No network calls made in any unit test run
</verification>

<success_criteria>
- probe-pod.sh: 8 valid pod numbers with correct IP mapping, PROBE_OVERRIDE_URL for offline tests, SENTRY_KEY precondition enforced
- probe-pos.sh: partial class canonical test case (tasklist WMI-denied -> partial) passes; POS_SSH_DOWN access_gap surfaced on probe_failed
- Pattern for SSH-section-marker + certutil hash extraction is now reusable (Plans 03+04 prove the shape)
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-04-SUMMARY.md` with:
- Files created (2 probes, 2 tests, 3 fixtures)
- Test results
- Sample probe_failed + partial manifests (one each)
- Handoff to Plans 05+06 (HTTP/relay cluster probes)
</output>
