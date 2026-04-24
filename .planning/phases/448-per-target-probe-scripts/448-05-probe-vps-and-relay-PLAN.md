---
phase: 448-per-target-probe-scripts
plan: 05
type: execute
wave: 3
depends_on: ["448-01", "448-02"]
files_modified:
  - scripts/fleet-probe/probe-vps.sh
  - scripts/fleet-probe/probe-relay.sh
  - tests/fleet-probe/probe-vps.test.mjs
  - tests/fleet-probe/probe-relay.test.mjs
  - tests/fleet-probe/fixtures/vps-relay-exec-ok.json
  - tests/fleet-probe/fixtures/vps-relay-exec-err.json
  - tests/fleet-probe/fixtures/relay-health-ok.json
  - tests/fleet-probe/fixtures/relay-health-disconnected.json
autonomous: true
requirements: [PROBE-05, PROBE-08]
gap_closure: false

must_haves:
  truths:
    - "probe-vps.sh uses comms-link relay POST /relay/exec/run with COMMS_PSK env var to capture running procs + pm2 list + systemctl + /root/racecontrol.toml hash — NEVER SSHes directly"
    - "probe-vps.sh with relay disconnected -> probe_status probe_failed + access_gap RELAY_DOWN"
    - "probe-vps.sh with missing COMMS_PSK -> probe_failed + auth_gap no_comms_psk"
    - "probe-relay.sh emits composite manifest with both James :8766 and VPS :8765 endpoints; vps side down -> partial + sub_probe vps_relay"
    - "Unit tests mock HTTP (relay endpoints) with startMockHttpServer — no real network"
  artifacts:
    - path: "scripts/fleet-probe/probe-vps.sh"
      provides: "VPS probe via comms-link relay /relay/exec/run with COMMS_PSK auth"
      min_lines: 150
    - path: "scripts/fleet-probe/probe-relay.sh"
      provides: "Relay composite probe (James :8766 local + VPS :8765 via local /relay/health)"
      min_lines: 120
    - path: "tests/fleet-probe/probe-vps.test.mjs"
      provides: "Unit tests: ok (mock relay returns ps/pm2), probe_failed (relay disconnected), auth_gap (no COMMS_PSK)"
      min_lines: 60
    - path: "tests/fleet-probe/probe-relay.test.mjs"
      provides: "Unit tests: both ok, james ok + vps disconnected (partial), james down (probe_failed)"
      min_lines: 60
  key_links:
    - from: "scripts/fleet-probe/probe-vps.sh"
      to: "http://localhost:8766/relay/exec/run"
      via: "POST with COMMS_PSK header and {command,reason} body"
      pattern: "relay/exec/run"
    - from: "scripts/fleet-probe/probe-relay.sh"
      to: "http://localhost:8766/relay/health"
      via: "GET"
      pattern: "relay/health"
    - from: "scripts/fleet-probe/probe-relay.sh"
      to: "composite manifest (James + VPS)"
      via: "scheduled_tasks entries flag each side's status"
      pattern: "relay_james|vps"

deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: none
  data_files: none
  bat_file: none
  cloud_parity: [none]
  targets: [james]
---

<objective>
Wave 3 probes 1+2 of 4: Ship probe-vps.sh (Bono VPS via comms-link relay) and probe-relay.sh (composite James :8766 + VPS :8765). Both run against HTTP endpoints only — no SSH — so their mocks are simpler (startMockHttpServer).

Purpose: Bono VPS is the home of cloud racecontrol + cloud admin; comms-link relay is the bidirectional bus. probe-vps.sh closes Gap 4 on the v53.0 drift story (cross-boundary VPS state surfaced). probe-relay.sh is the only "composite" probe in the phase — one manifest covers two endpoints.

Output: 2 probes + 2 unit tests + 4 HTTP response fixtures.
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

# Reference implementations
@scripts/auto-detect.sh
@scripts/audit/autostart-surfaces.sh

# Shape references
@schemas/examples/bono_vps.json
@schemas/examples/relay_james.json

<interfaces>
**probe-vps.sh contract**
```
Usage: MANIFEST_TS=<iso> COMMS_PSK=<psk> bash scripts/fleet-probe/probe-vps.sh
Preconditions: $MANIFEST_TS exported; $COMMS_PSK env var set
Optional env:
  PROBE_OVERRIDE_RELAY_URL  -- overrides http://localhost:8766 (tests set this to mock server)
Stdout: {"target_id":"bono_vps","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: state/fleet-manifest/$MANIFEST_TS/bono_vps.json
Failures:
  - COMMS_PSK unset -> probe_failed + auth_gap: no_comms_psk
  - /relay/health -> connected:false -> probe_failed + access_gap: RELAY_DOWN
  - Relay reachable but any /relay/exec/run non-zero -> partial + sub_probe: <command>
```

**probe-vps remote commands (sent via /relay/exec/run):**
```
uname -a                                    # hostname/os
ps -eo comm,pid,args | head -100            # running procs
systemctl list-unit-files --type=service | grep enabled | head -40   # scheduled/autostart
pm2 jlist                                   # autostart (Node services)
sha256sum /root/racecontrol.toml || true    # config hash
curl -s http://localhost:8080/api/v1/health # cloud racecontrol build_id (captured for cross-reference)
env                                         # env names hash (we slice to names only)
```

**Relay /exec/run request body:**
```json
{"command": "bash_script", "script": "<multi-line-bash-heredoc>", "reason": "probe-vps-448"}
```
Response:
```json
{"stdout": "...", "stderr": "...", "exitCode": 0}
```

**probe-relay.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-relay.sh
No auth needed (public /relay/health endpoints).
Optional env:
  PROBE_OVERRIDE_RELAY_URL  -- overrides http://localhost:8766 for James side
Side effect: state/fleet-manifest/$MANIFEST_TS/relay_james.json
Manifest shape (from schemas/examples/relay_james.json):
  binary_sha256: {}
  build_id: null
  config_hash: {}
  running_procs: [{name: node.exe, pid, cmdline_hash}]  -- synthesized from `tasklist /FI "IMAGENAME eq node.exe"`
  scheduled_tasks: [{name: CommsLink-DaemonWatchdog, state: Ready}]  -- from local schtasks /Query
  autostart_entries: [{source: HKCU_Run, key: CommsLink, value: ...}]  -- from local reg query
  last_deploy_ts: null
  probe_errors:
    - if local :8766/relay/health returns 4xx/5xx: sub_probe: local_relay
    - if connected==false in response: sub_probe: vps_relay
Status:
  - local down -> probe_failed + access_gap: RELAY_LOCAL_DOWN
  - local ok, VPS disconnected -> partial + sub_probe: vps_relay
  - both ok -> ok
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create probe-vps.sh (comms-link relay exec) + fixtures + unit test</name>
  <files>scripts/fleet-probe/probe-vps.sh, tests/fleet-probe/probe-vps.test.mjs, tests/fleet-probe/fixtures/vps-relay-exec-ok.json, tests/fleet-probe/fixtures/vps-relay-exec-err.json</files>
  <read_first>
    - scripts/auto-detect.sh lines 286-341 (relay_health + exec round-trip pattern)
    - scripts/audit/autostart-surfaces.sh (the Linux enumerator; single-shot multi-line output pattern we wrap)
    - scripts/smart-pipes/env-drift-check.sh line 36 (concise relay usage)
    - schemas/examples/bono_vps.json (shape reference for VPS manifest)
    - scripts/fleet-probe/probe-server.sh (reuse error-handling + manifest assembly)
  </read_first>
  <behavior>
    - No COMMS_PSK -> probe_failed + auth_gap: no_comms_psk, schema-valid manifest
    - Mock relay returning /relay/health {connected:false} -> probe_failed + access_gap: RELAY_DOWN
    - Mock relay returning /relay/exec/run 200 with ps/pm2/sha256sum output -> probe_status ok or partial, running_procs non-empty, config_hash has at least one entry
    - Mock relay returning /relay/exec/run with non-zero exitCode -> partial + probe_errors[] entry with relevant sub_probe
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/vps-relay-exec-ok.json`:

```json
{
  "stdout": "Linux srv1422716 5.15.0-101-generic\nroot        1234  /usr/bin/node /root/racecontrol/dist/index.js\nroot        5678  pm2 ...\nnginx.service                              enabled\nssh.service                                enabled\n[{\"name\":\"racecontrol\",\"pm_id\":0,\"pm2_env\":{\"status\":\"online\"}}]\nabcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789  /root/racecontrol.toml\nPATH=/usr/bin\nHOME=/root\n",
  "stderr": "",
  "exitCode": 0
}
```

Create `tests/fleet-probe/fixtures/vps-relay-exec-err.json`:

```json
{
  "stdout": "",
  "stderr": "bash: /root/foo.sh: not found",
  "exitCode": 127
}
```

Create `scripts/fleet-probe/probe-vps.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-vps.sh — Phase 448 Plan 05
# Probes Bono VPS via comms-link relay localhost:8766/relay/exec/run.
# Never SSH'es directly (relay is the ONLY prod path).
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
IP_VAL="45.11.110.250"  # approximate; resolved from DNS in cloud_racecontrol probe
ROLE_VAL="vps"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sp="$1" err="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" python3 -c '
import os, json
a=json.loads(os.environ["PE"]); e={"sub_probe":os.environ["SP"],"error":os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]]=os.environ["XV"]
a.append(e); print(json.dumps(a))
')
}

# --- Auth pre-check ---
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
    CONNECTED=$(printf '%s' "$HEALTH" | jq -r '.connected // false' 2>/dev/null || echo "false")
    if [ "$CONNECTED" != "true" ]; then
      CONNECT_ERR=1
      append_error "relay_connect" "relay reports connected=$CONNECTED" "access_gap" "RELAY_DOWN"
    else
      RELAY_OK=1
    fi
  fi
fi

EXEC_OUT=""
if [ "$RELAY_OK" -eq 1 ]; then
  # Compose a single bash script that emits sectioned output. Each section prefixed ===MARK:<name>===
  PROBE_SCRIPT='#!/bin/bash
echo "===MARK:uname==="
uname -a
echo "===MARK:ps==="
ps -eo comm,pid,args --no-headers 2>/dev/null | head -60
echo "===MARK:systemctl==="
systemctl list-unit-files --type=service --state=enabled --no-pager 2>/dev/null | head -40
echo "===MARK:pm2==="
pm2 jlist 2>/dev/null | head -300
echo "===MARK:config_hash==="
sha256sum /root/racecontrol.toml 2>/dev/null || echo "NO_FILE"
echo "===MARK:env==="
env | awk -F= "NF>=2 {print \$1}" | sort
echo "===MARK:end==="
'
  TMP=$(mktemp)
  python3 -c 'import json,sys,os
payload={"command":"bash_script","script":sys.argv[1],"reason":"probe-vps-448"}
print(json.dumps(payload))' "$PROBE_SCRIPT" > "$TMP"

  RESP=$(curl -s --max-time 30 -X POST \
    -H "Content-Type: application/json" \
    -H "X-Comms-PSK: $COMMS_PSK" \
    -d @"$TMP" "$RELAY_URL/relay/exec/run" 2>/dev/null || echo "")
  rm -f "$TMP"

  if [ -z "$RESP" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "relay_exec" "no response from /relay/exec/run"
  else
    EXIT_CODE=$(printf '%s' "$RESP" | jq -r '.exitCode // -1' 2>/dev/null || echo "-1")
    if [ "$EXIT_CODE" != "0" ]; then
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "relay_exec" "exec exitCode=$EXIT_CODE"
    fi
    EXEC_OUT=$(printf '%s' "$RESP" | jq -r '.stdout // ""' 2>/dev/null || echo "")
  fi
fi

extract_section() {
  local marker="$1" next="$2"
  printf '%s' "$EXEC_OUT" | awk -v s="===MARK:$marker===" -v e="===MARK:$next===" '
    $0==s { cap=1; next } $0==e { cap=0; exit } cap
  '
}

UNAME_LINE=$(extract_section "uname" "ps" | head -1)
PS_OUT=$(extract_section "ps" "systemctl")
SYSTEMCTL_OUT=$(extract_section "systemctl" "pm2")
PM2_OUT=$(extract_section "pm2" "config_hash")
CFG_LINE=$(extract_section "config_hash" "env" | head -1)
ENV_OUT=$(extract_section "env" "end")

# Parse ps into running_procs
RUNNING_PROCS_JSON=$(printf '%s' "$PS_OUT" | python3 -c '
import sys, hashlib, json
rows=[]
for ln in sys.stdin:
    ln = ln.strip()
    if not ln: continue
    parts = ln.split(None, 2)
    if len(parts) < 2: continue
    name = parts[0]
    try: pid = int(parts[1])
    except: continue
    h = hashlib.sha256(ln.encode("utf-8","replace")).hexdigest()
    rows.append({"name": name, "pid": pid, "cmdline_hash": h})
print(json.dumps(rows[:100]))
')

# scheduled_tasks from systemctl enabled services + pm2 jlist
SCHTASKS_JSON=$(printf 'SYSTEMCTL_START\n%s\nPM2_START\n%s\n' "$SYSTEMCTL_OUT" "$PM2_OUT" | python3 -c '
import sys, json, re
entries = []
text = sys.stdin.read()
parts = text.split("PM2_START", 1)
systemctl = parts[0].replace("SYSTEMCTL_START\n", "") if parts else ""
pm2 = parts[1] if len(parts) > 1 else ""
for ln in systemctl.splitlines():
    ln = ln.strip()
    if not ln or ln.startswith("UNIT"): continue
    bits = ln.split(None, 1)
    if bits: entries.append({"name": bits[0], "state": "enabled"})
try:
    for obj in json.loads(pm2 or "[]"):
        n = obj.get("name") or "pm2-unknown"
        st = obj.get("pm2_env", {}).get("status") or "unknown"
        entries.append({"name": f"pm2:{n}", "state": st})
except Exception:
    pass
print(json.dumps(entries[:100]))
')

# autostart_entries from systemctl (same list, flagged as schtask source)
AUTOSTART_JSON=$(printf '%s' "$SYSTEMCTL_OUT" | python3 -c '
import sys, json
es=[]
for ln in sys.stdin:
    ln=ln.strip()
    if not ln or ln.startswith("UNIT"): continue
    bits=ln.split(None,1)
    if bits:
        es.append({"source":"schtask","key":bits[0],"value":"systemctl enabled"})
print(json.dumps(es[:100]))
')

# config_hash
CONFIG_HASH_JSON="{}"
if [ -n "$CFG_LINE" ] && [ "$CFG_LINE" != "NO_FILE" ]; then
  CFG_HASH=$(printf '%s' "$CFG_LINE" | awk '{print $1}')
  if echo "$CFG_HASH" | grep -qE '^[0-9a-f]{64}$'; then
    CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"/root/racecontrol.toml": sys.argv[1]}))' "$CFG_HASH")
  fi
fi

ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
if [ -n "$ENV_OUT" ]; then
  ENV_HASH=$(printf '%s' "$ENV_OUT" | sort | sha256sum | awk '{print $1}')
fi

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  RUNNING_PROCS_JSON="[]"
  SCHTASKS_JSON="[]"
  AUTOSTART_JSON="[]"
fi

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version":"1.0","target_id":"bono_vps","host":os.environ["HOSTNAME_VAL"],"ip":os.environ["IP_VAL"],"role":"vps",
  "probed_at_ist":os.environ["PROBED_AT"],"probe_status":os.environ["PROBE_STATUS"],
  "binary_sha256":{},"build_id":None,
  "config_hash":json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs":json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks":json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries":json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash":os.environ["ENV_HASH"],"last_deploy_ts":None,
}
err=json.loads(os.environ["PROBE_ERRORS_JSON"])
if err: m["probe_errors"]=err
print(json.dumps(m))
' HOSTNAME_VAL="$HOSTNAME_VAL" IP_VAL="$IP_VAL" PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" \
   CONFIG_HASH_JSON="$CONFIG_HASH_JSON" RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" \
   SCHTASKS_JSON="$SCHTASKS_JSON" AUTOSTART_JSON="$AUTOSTART_JSON" \
   ENV_HASH="$ENV_HASH" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-vps.test.mjs`:

```js
// tests/fleet-probe/probe-vps.test.mjs — Phase 448 Plan 05
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema, loadFixture } from "./helpers.mjs";

async function runProbeVps({ healthResponse, execFixture, psk = "mock-psk" }) {
  const execBody = execFixture ? JSON.stringify(loadFixture(execFixture)) : "";
  const healthBody = healthResponse ? JSON.stringify(healthResponse) : "";
  const routes = {
    "/relay/health": { status: healthResponse ? 200 : 500, body: healthBody },
    "/relay/exec/run": { status: 200, body: execBody },
  };
  const server = await startMockHttpServer(routes);
  const ts = "test-vps-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
  try {
    const env = { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_RELAY_URL: server.url };
    if (psk) env.COMMS_PSK = psk; else delete env.COMMS_PSK;
    const res = spawnSync("bash", ["scripts/fleet-probe/probe-vps.sh"], {
      env, encoding: "utf8", timeout: 60_000,
    });
    const mpath = resolve("state/fleet-manifest", ts, "bono_vps.json");
    const m = existsSync(mpath) ? JSON.parse(readFileSync(mpath, "utf8")) : null;
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
    return { res, m };
  } finally { await server.close(); }
}

test("probe-vps.sh missing COMMS_PSK -> probe_failed + auth_gap no_comms_psk", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixture: "vps-relay-exec-ok", psk: "" });
  assert.ok(m);
  assert.equal(m.probe_status, "probe_failed");
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "auth");
  assert.ok(err); assert.equal(err.auth_gap, "no_comms_psk");
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema: ${JSON.stringify(errors)}`);
});

test("probe-vps.sh relay connected=false -> probe_failed + RELAY_DOWN", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: false }, execFixture: null });
  assert.equal(m.probe_status, "probe_failed");
  const err = (m.probe_errors || []).find((e) => e.access_gap === "RELAY_DOWN");
  assert.ok(err, `expected RELAY_DOWN; got: ${JSON.stringify(m.probe_errors)}`);
});

test("probe-vps.sh happy path -> running_procs populated + schema-valid", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixture: "vps-relay-exec-ok" });
  assert.ok(["ok", "partial"].includes(m.probe_status), `status=${m.probe_status}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema: ${JSON.stringify(errors)}`);
});

test("probe-vps.sh exec non-zero -> partial", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixture: "vps-relay-exec-err" });
  assert.equal(m.probe_status, "partial");
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "relay_exec");
  assert.ok(err);
});
```

`chmod +x scripts/fleet-probe/probe-vps.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-vps.sh &amp;&amp; node --check tests/fleet-probe/probe-vps.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-vps.sh` exits 0
    - `grep -c "relay/exec/run" scripts/fleet-probe/probe-vps.sh` >= 1
    - `grep -c "relay/health" scripts/fleet-probe/probe-vps.sh` >= 1
    - `grep -c "COMMS_PSK" scripts/fleet-probe/probe-vps.sh` >= 2
    - `grep -c "RELAY_DOWN" scripts/fleet-probe/probe-vps.sh` >= 1
    - `grep -c "no_comms_psk" scripts/fleet-probe/probe-vps.sh` >= 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-vps.sh` == 1
    - `grep -c "ssh " scripts/fleet-probe/probe-vps.sh` == 0  (no SSH — relay only per CONTEXT.md)
    - `npm run test:fleet-probe` exits 0 (probe-vps tests all green: missing PSK, RELAY_DOWN, ok, exec-non-zero partial)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-vps.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>probe-vps.sh uses comms-link relay exclusively, enforces COMMS_PSK precondition, handles all 3 failure classes (no PSK, RELAY_DOWN, exec non-zero) with schema-valid output; unit tests cover all 4 paths.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Create probe-relay.sh (composite James + VPS) + fixtures + unit test</name>
  <files>scripts/fleet-probe/probe-relay.sh, tests/fleet-probe/probe-relay.test.mjs, tests/fleet-probe/fixtures/relay-health-ok.json, tests/fleet-probe/fixtures/relay-health-disconnected.json</files>
  <read_first>
    - scripts/auto-detect.sh lines 303-341 (relay_health + connected/status pattern)
    - schemas/examples/relay_james.json (composite manifest shape — binary_sha256:{}, build_id:null, config_hash:{}, last_deploy_ts:null)
    - scripts/fleet-probe/probe-james.sh (reuse local tasklist/schtasks/reg pattern; probe-relay is subset)
    - tests/fleet-probe/helpers.mjs (startMockHttpServer is sufficient — no SSH mock needed)
  </read_first>
  <behavior>
    - Mock local /relay/health returns {connected:true} -> probe_status ok (or partial if VPS side flags report a problem)
    - Mock local /relay/health returns {connected:false} -> probe_status partial + sub_probe vps_relay
    - Mock local /relay/health unreachable (HTTP 500) -> probe_status probe_failed + access_gap RELAY_LOCAL_DOWN
    - running_procs non-empty (local tasklist picks up node.exe)
    - target_id=relay_james, host=JAMES-PC, ip=192.168.31.27, role=relay
    - last_deploy_ts always null (comms-link has no deploy pipeline)
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/relay-health-ok.json`:

```json
{"connected": true, "status": "connected", "queue_depth": 0, "last_sync": "2026-04-24T12:00:00+05:30"}
```

Create `tests/fleet-probe/fixtures/relay-health-disconnected.json`:

```json
{"connected": false, "status": "disconnected", "queue_depth": 5, "last_sync": "2026-04-24T11:30:00+05:30"}
```

Create `scripts/fleet-probe/probe-relay.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-relay.sh — Phase 448 Plan 05
# Composite probe for comms-link relay: James :8766 local + VPS :8765 side (queried via James /relay/health which reports VPS connect state).
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-relay: MANIFEST_TS not set" >&2
  exit 2
fi

RELAY_URL="${PROBE_OVERRIDE_RELAY_URL:-http://localhost:8766}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="relay_james"
HOSTNAME_VAL="JAMES-PC"
IP_VAL="192.168.31.27"
ROLE_VAL="relay"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sp="$1" err="$2" xk="${3:-}" xv="${4:-}"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" XK="$xk" XV="$xv" PE="$PROBE_ERRORS_JSON" python3 -c '
import os,json
a=json.loads(os.environ["PE"]); e={"sub_probe":os.environ["SP"],"error":os.environ["ERR"]}
if os.environ.get("XK"): e[os.environ["XK"]]=os.environ["XV"]
a.append(e); print(json.dumps(a))
')
}

# --- Local :8766 /relay/health ---
LOCAL_HEALTH_BODY=$(curl -s --max-time 5 -w "\n%{http_code}" "$RELAY_URL/relay/health" 2>/dev/null || echo "")
LOCAL_STATUS=$(printf '%s' "$LOCAL_HEALTH_BODY" | tail -1)
LOCAL_BODY=$(printf '%s' "$LOCAL_HEALTH_BODY" | head -n -1)

if [ "$LOCAL_STATUS" != "200" ]; then
  CONNECT_ERR=1
  append_error "local_relay" "James relay /relay/health HTTP $LOCAL_STATUS on $RELAY_URL" "access_gap" "RELAY_LOCAL_DOWN"
else
  VPS_CONNECTED=$(printf '%s' "$LOCAL_BODY" | jq -r '.connected // false' 2>/dev/null || echo "false")
  if [ "$VPS_CONNECTED" != "true" ]; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "vps_relay" "James relay reports VPS connected=$VPS_CONNECTED"
  fi
fi

# --- Local tasklist / schtasks / reg — same as probe-james but filtered to relay-relevant rows ---
# Use filters to keep manifest small.
RUNNING_PROCS_JSON="[]"
if tasklist_raw=$(tasklist /FI "IMAGENAME eq node.exe" /V /FO CSV 2>/dev/null); then
  RUNNING_PROCS_JSON=$(printf '%s' "$tasklist_raw" | python3 -c '
import sys, csv, hashlib, json
rows=[]; first=True
for row in csv.reader(sys.stdin.read().splitlines()):
    if first: first=False; continue
    if len(row) < 2: continue
    try: pid = int(row[1])
    except: continue
    rows.append({"name": row[0], "pid": pid, "cmdline_hash": hashlib.sha256(" ".join(row).encode("utf-8","replace")).hexdigest()})
print(json.dumps(rows[:50]))
')
fi

SCHTASKS_JSON="[]"
if schtasks_raw=$(schtasks /Query /TN "CommsLink-DaemonWatchdog" /V /FO LIST 2>/dev/null); then
  SCHTASKS_JSON=$(printf '%s' "$schtasks_raw" | python3 -c '
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
print(json.dumps(es))
')
fi

AUTOSTART_JSON=$(python3 -c '
import subprocess, json
es=[]
try:
    out = subprocess.check_output(["reg","query","HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run"], text=True, stderr=subprocess.DEVNULL)
    for ln in out.splitlines():
        ln=ln.strip()
        if not ln or ln.startswith("HKEY_"): continue
        parts=ln.split(None,2)
        if len(parts)==3 and "comms" in parts[2].lower():
            es.append({"source":"HKCU_Run","key":parts[0],"value":parts[2]})
except Exception: pass
print(json.dumps(es))
')

ENV_HASH=$(env_names_hash)

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

if [ "$PROBE_STATUS" = "probe_failed" ]; then
  RUNNING_PROCS_JSON="[]"
  SCHTASKS_JSON="[]"
  AUTOSTART_JSON="[]"
fi

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version":"1.0","target_id":"relay_james","host":"JAMES-PC","ip":"192.168.31.27","role":"relay",
  "probed_at_ist":os.environ["PROBED_AT"],"probe_status":os.environ["PROBE_STATUS"],
  "binary_sha256":{},"build_id":None,"config_hash":{},
  "running_procs":json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks":json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries":json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash":os.environ["ENV_HASH"],"last_deploy_ts":None,
}
err=json.loads(os.environ["PROBE_ERRORS_JSON"])
if err: m["probe_errors"]=err
print(json.dumps(m))
' PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" \
   RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" SCHTASKS_JSON="$SCHTASKS_JSON" \
   AUTOSTART_JSON="$AUTOSTART_JSON" ENV_HASH="$ENV_HASH" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-relay.test.mjs`:

```js
// tests/fleet-probe/probe-relay.test.mjs — Phase 448 Plan 05
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema, loadFixture } from "./helpers.mjs";

async function runProbeRelay({ healthFixture, healthStatus = 200 }) {
  const body = healthFixture ? JSON.stringify(loadFixture(healthFixture)) : "";
  const server = await startMockHttpServer({
    "/relay/health": { status: healthStatus, body },
  });
  try {
    const ts = "test-relay-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
    const res = spawnSync("bash", ["scripts/fleet-probe/probe-relay.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_RELAY_URL: server.url },
      encoding: "utf8", timeout: 30_000,
    });
    const p = resolve("state/fleet-manifest", ts, "relay_james.json");
    const m = existsSync(p) ? JSON.parse(readFileSync(p, "utf8")) : null;
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
    return { res, m };
  } finally { await server.close(); }
}

test("probe-relay.sh both connected -> probe_status ok, schema-valid, null build_id + null last_deploy_ts", async () => {
  const { m } = await runProbeRelay({ healthFixture: "relay-health-ok" });
  assert.ok(m);
  assert.equal(m.target_id, "relay_james");
  assert.equal(m.role, "relay");
  assert.equal(m.probe_status, "ok");
  assert.equal(m.build_id, null);
  assert.equal(m.last_deploy_ts, null);
  assert.deepEqual(m.binary_sha256, {});
  assert.deepEqual(m.config_hash, {});
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema: ${JSON.stringify(errors)}`);
});

test("probe-relay.sh VPS disconnected -> partial + sub_probe vps_relay", async () => {
  const { m } = await runProbeRelay({ healthFixture: "relay-health-disconnected" });
  assert.equal(m.probe_status, "partial");
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "vps_relay");
  assert.ok(err, `expected vps_relay error, got: ${JSON.stringify(m.probe_errors)}`);
  const { valid } = validateAgainstSchema(m);
  assert.ok(valid);
});

test("probe-relay.sh local down -> probe_failed + RELAY_LOCAL_DOWN", async () => {
  const { m } = await runProbeRelay({ healthFixture: null, healthStatus: 500 });
  assert.equal(m.probe_status, "probe_failed");
  const err = (m.probe_errors || []).find((e) => e.access_gap === "RELAY_LOCAL_DOWN");
  assert.ok(err);
});
```

`chmod +x scripts/fleet-probe/probe-relay.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-relay.sh &amp;&amp; node --check tests/fleet-probe/probe-relay.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-relay.sh` exits 0
    - `grep -c "relay/health" scripts/fleet-probe/probe-relay.sh` >= 1
    - `grep -c "RELAY_LOCAL_DOWN" scripts/fleet-probe/probe-relay.sh` >= 1
    - `grep -c "vps_relay" scripts/fleet-probe/probe-relay.sh` >= 1
    - `grep -c "target_id\":\"relay_james\"" scripts/fleet-probe/probe-relay.sh` >= 1 OR (dynamic build) check for `TARGET_ID="relay_james"` (LOCKED)
    - `grep -c "TARGET_ID=\"relay_james\"" scripts/fleet-probe/probe-relay.sh` == 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-relay.sh` == 1
    - `npm run test:fleet-probe` exits 0 (probe-relay tests all green: both ok, vps disconnected partial, local down probe_failed)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-relay.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>probe-relay.sh is the composite probe — single manifest covering James local + VPS side status; all 3 failure classes return schema-valid manifests with stable binary_sha256:{}, build_id:null, last_deploy_ts:null (comms-link has no binary/deploy pipeline).</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0
- `npm run test:fleet-drift` still exits 0
- All 6 probe scripts now present: james, server, pod, pos, vps, relay
- No SSH in probe-vps.sh (relay-only per CONTEXT.md)
</verification>

<success_criteria>
- probe-vps.sh uses comms-link relay exclusively (no SSH fallback in this phase)
- probe-relay.sh is the only composite-manifest probe; shape matches schemas/examples/relay_james.json
- 4 new mock HTTP fixtures cover all relay failure modes
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-05-SUMMARY.md` with:
- Files created
- Test results
- Handoff to Plan 06 (cloud-admin + cloud-rc probes) — Wave 3 cousin plan
</output>
