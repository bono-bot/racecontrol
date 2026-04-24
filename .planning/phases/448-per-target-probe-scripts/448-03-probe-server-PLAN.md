---
phase: 448-per-target-probe-scripts
plan: 03
type: execute
wave: 2
depends_on: ["448-01", "448-02"]
files_modified:
  - scripts/fleet-probe/probe-server.sh
  - tests/fleet-probe/probe-server.test.mjs
  - tests/fleet-probe/fixtures/server-ssh-ok.txt
  - tests/fleet-probe/fixtures/server-ssh-timeout.txt
autonomous: true
requirements: [PROBE-01]
gap_closure: false

must_haves:
  truths:
    - "Staff can run probe-server.sh against the live Server .23 and get a schema-valid manifest with racecontrol.exe SHA256 + build_id + 3-way config_hash (Q5 drift captured)"
    - "When SSH to ADMIN@100.125.108.37 times out, probe emits probe_status: probe_failed with access_gap: SSH_23 and writes a manifest (not a missing file)"
    - "config_hash has 3 keys: racecontrol.toml.server_live, racecontrol.toml.james_proxy, racecontrol.toml.git_head (Q5 three-way drift surfacing)"
    - "last_deploy_ts is pulled from SWAPLOG.md last row (parsed, IST format)"
    - "Unit test uses PROBE_SSH override to mock SSH responses; never touches real network during tests"
  artifacts:
    - path: "scripts/fleet-probe/probe-server.sh"
      provides: "Server .23 probe via Tailscale SSH + SWAPLOG.md read + local/git config SHA256"
      min_lines: 180
    - path: "tests/fleet-probe/probe-server.test.mjs"
      provides: "Node test: mocks SSH via PROBE_SSH + mock-ssh-responder.sh; asserts schema-valid manifest in both ok + probe_failed paths"
      min_lines: 60
    - path: "tests/fleet-probe/fixtures/server-ssh-ok.txt"
      provides: "Mock SSH scenario for ok path (hostname/tasklist/schtasks/reg/certutil output)"
    - path: "tests/fleet-probe/fixtures/server-ssh-timeout.txt"
      provides: "Mock SSH scenario that exits non-zero to trigger probe_failed path"
  key_links:
    - from: "scripts/fleet-probe/probe-server.sh"
      to: "SWAPLOG.md (repo root)"
      via: "tail -n 20 | grep | awk to parse last row's timestamp column"
      pattern: "SWAPLOG.md"
    - from: "scripts/fleet-probe/probe-server.sh"
      to: "ADMIN@100.125.108.37 (Tailscale)"
      via: "ssh -o ConnectTimeout=15 ... (default key auth)"
      pattern: "ADMIN@100\\.125\\.108\\.37"
    - from: "scripts/fleet-probe/probe-server.sh"
      to: "config_hash.racecontrol.toml.git_head"
      via: "git show HEAD:crates/racecontrol/racecontrol.toml | sha256sum OR fallback to local repo file"
      pattern: "config_hash"
    - from: "scripts/fleet-probe/probe-server.sh"
      to: "$PROBE_SSH override"
      via: "SSH_CMD=\"${PROBE_SSH:-ssh}\" pattern for test injection"
      pattern: "PROBE_SSH"

deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: none
  data_files: none
  bat_file: none
  cloud_parity: [none]
  targets: [server]
---

<objective>
Wave 2 probe 1 of 3: Ship probe-server.sh — the Server .23 Tailscale SSH probe that captures binary hash, build_id from /api/v1/health, Q5 three-way config_hash (D:\ vs live vs git), SWAPLOG-derived last_deploy_ts, and tasklist/schtasks/reg query state. Uses PROBE_SSH env var override pattern so unit tests mock SSH without real network.

Purpose: Server .23 is the central hub of the v53.0 drift-detection story (Q5 D: drift is THE motivating use-case). This probe must reliably capture the three-way config divergence so Phase 452's diff tool can surface it.

Output: Working probe + 2 test fixtures + unit test covering ok + probe_failed paths.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md
@.planning/phases/448-per-target-probe-scripts/448-VALIDATION.md
@.planning/phases/448-per-target-probe-scripts/448-01-wave0-scaffolding-PLAN.md
@.planning/phases/448-per-target-probe-scripts/448-02-probe-james-and-orchestrator-skeleton-PLAN.md

# Reused Plan 01 + 02 artifacts
@scripts/fleet-probe/lib/probe-common.sh
@scripts/fleet-probe/probe-james.sh
@tests/fleet-probe/helpers.mjs
@tests/fleet-probe/mock-ssh-responder.sh

# Reference implementations (READ before writing)
@scripts/deploy-server.sh
@scripts/deploy-preflight.sh
@scripts/lib/ssh-helpers.sh
@SWAPLOG.md

# Shape reference
@schemas/examples/server_23.json

<interfaces>
**probe-server.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-server.sh
Preconditions: $MANIFEST_TS exported; SSH key for ADMIN@100.125.108.37 available (or PROBE_SSH override)
Optional env:
  PROBE_SSH      -- overrides `ssh` binary (tests set this to mock-ssh-responder.sh)
  PROBE_SSH_SCENARIO  -- used by mock-ssh-responder.sh
  PROBE_SKIP_HTTP=1   -- skips /api/v1/health HTTP probe (tests use this)
Stdout: {"target_id":"server_23","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: writes state/fleet-manifest/$MANIFEST_TS/server_23.json (schema-valid)
Exit: 0 on any outcome (including probe_failed); 2 only on missing MANIFEST_TS
```

**SSH target (LOCKED from CLAUDE.md network map)**
```
Host: ADMIN@100.125.108.37  (Tailscale IP)
Fallback: ssh -J root@100.70.177.44 ADMIN@192.168.31.23  (LAN via Bono jump — not implemented this plan)
```

**Remote commands (all run in one multi-command SSH call to amortize the handshake):**
```
hostname
whoami
ver
certutil -hashfile "C:\RacingPoint\racecontrol.exe" SHA256 2>nul
certutil -hashfile "C:\RacingPoint\racecontrol.toml" SHA256 2>nul
tasklist /V /FO CSV 2>nul
schtasks /Query /V /FO LIST 2>nul
reg query "HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul
reg query "HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run" 2>nul
set 2>nul
```

**Q5 three-way config_hash KEY NAMES (LOCKED):**
```
racecontrol.toml.server_live  -- sha256 of live file on server .23 (via SSH certutil)
racecontrol.toml.james_proxy  -- sha256 of /d/racecontrol.toml on James if present; else skip with partial error
racecontrol.toml.git_head     -- sha256 of `git show HEAD:crates/racecontrol/racecontrol.toml` OR repo file at crates/racecontrol/racecontrol.toml if tracked; else skip
```

**SWAPLOG.md parse contract:**
```
Format: | timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |
Last row (not header) -> extract column 2 (timestamp_ist); format is "YYYY-MM-DD HH:MM IST" or similar.
Convert to ISO-8601 "+05:30" suffix before writing last_deploy_ts.
If SWAPLOG.md missing or empty -> last_deploy_ts: null + probe_errors[].sub_probe: "last_deploy_ts" + partial class.
```

**build_id source:**
```
HTTP GET http://192.168.31.23:8080/api/v1/health (LAN) -- 5s timeout
OR
curl via relay-localhost if LAN unreachable -- NOT implemented this plan (document as future hardening)
On success: jq -r .build_id
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create mock SSH fixtures + unit test (TDD — write before implementation)</name>
  <files>tests/fleet-probe/fixtures/server-ssh-ok.txt, tests/fleet-probe/fixtures/server-ssh-timeout.txt, tests/fleet-probe/probe-server.test.mjs</files>
  <read_first>
    - tests/fleet-probe/mock-ssh-responder.sh (understand the "line 1 stdout, ---EXIT--- separator, next line exit code" format)
    - tests/fleet-probe/probe-james.test.mjs (reuse the spawnSync + env + schema-validate pattern)
    - schemas/examples/server_23.json (copy shape for "what the manifest should look like on ok")
    - schemas/fleet-manifest.schema.json (confirm probe_failed class allows empty binary_sha256 {} and null build_id)
  </read_first>
  <behavior>
    - Mock SSH ok scenario: stdout contains simulated hostname/tasklist/schtasks/reg output in a parseable shape; exit 0
    - Mock SSH timeout scenario: empty stdout; exit 255 (SSH timeout code)
    - Unit test with ok scenario -> manifest probe_status=ok, binary_sha256 non-empty, 3 config_hash keys
    - Unit test with timeout scenario -> manifest probe_status=probe_failed, probe_errors[0].sub_probe=ssh_connect, access_gap=SSH_23, binary_sha256={}, config_hash={}
    - Both paths produce schema-valid manifests
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/server-ssh-ok.txt` — mock SSH responder reads this and emits stdout up to `---EXIT---`, then exit code on next line:

```
Racing-Point-Server
ADMIN
Microsoft Windows [Version 10.0.26100.2161]
SHA256 hash of C:\RacingPoint\racecontrol.exe:
aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
CertUtil: -hashfile command completed successfully.
SHA256 hash of C:\RacingPoint\racecontrol.toml:
bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
CertUtil: -hashfile command completed successfully.
"Image Name","PID","Session Name","Session#","Mem Usage","Status","User Name","CPU Time","Window Title"
"racecontrol.exe","4567","Services","0","123,456 K","Running","NT AUTHORITY\SYSTEM","0:00:12","N/A"
"powershell.exe","4568","Console","1","45,678 K","Running","ADMIN","0:00:05","RaceControl Watchdog"
HostName:                             Racing-Point-Server
TaskName:                             \StartRCDirect
Status:                               Ready
Logon Mode:                           Interactive/Background

TaskName:                             \StartRCOnBoot
Status:                               Ready
Logon Mode:                           Interactive/Background

HKEY_LOCAL_MACHINE\SOFTWARE\Microsoft\Windows\CurrentVersion\Run
    RaceControl    REG_SZ    C:\RacingPoint\start-racecontrol.bat

HKEY_CURRENT_USER\SOFTWARE\Microsoft\Windows\CurrentVersion\Run

ALLUSERSPROFILE=C:\ProgramData
APPDATA=C:\Users\ADMIN\AppData\Roaming
COMPUTERNAME=RACING-POINT-SERVER
PATH=C:\Windows;C:\Windows\System32
USERNAME=ADMIN
---EXIT---
0
```

Create `tests/fleet-probe/fixtures/server-ssh-timeout.txt`:

```
---EXIT---
255
```

Create `tests/fleet-probe/probe-server.test.mjs`:

```js
// tests/fleet-probe/probe-server.test.mjs — Phase 448 Plan 03
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { validateAgainstSchema } from "./helpers.mjs";

const MOCK_SSH = resolve("tests/fleet-probe/mock-ssh-responder.sh");

function runProbe(scenarioPath, extra = {}) {
  const ts = "test-server-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
  const env = {
    ...process.env,
    MANIFEST_TS: ts,
    PROBE_SSH: MOCK_SSH,
    PROBE_SSH_SCENARIO: resolve(scenarioPath),
    PROBE_SKIP_HTTP: "1",
    ...extra,
  };
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-server.sh"], {
    env,
    encoding: "utf8",
    timeout: 30_000,
  });
  const manifestPath = resolve("state/fleet-manifest", ts, "server_23.json");
  const manifest = existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, "utf8")) : null;
  return { res, manifestPath, manifest, ts };
}

test("probe-server.sh ok path: schema-valid manifest, 3 config_hash keys", () => {
  const { res, manifest, ts } = runProbe("tests/fleet-probe/fixtures/server-ssh-ok.txt");
  assert.equal(res.status, 0, `exit=${res.status} stderr=${res.stderr}`);
  assert.ok(manifest, "manifest must be written");
  assert.equal(manifest.target_id, "server_23");
  assert.equal(manifest.role, "server");
  assert.ok(["ok", "partial"].includes(manifest.probe_status), `status=${manifest.probe_status}`);
  assert.ok(manifest.binary_sha256 && typeof manifest.binary_sha256 === "object");
  // 3-way config_hash keys (Q5 drift surfacing) — at least server_live must be present on ok path
  assert.ok(
    "racecontrol.toml.server_live" in manifest.config_hash,
    `config_hash keys: ${Object.keys(manifest.config_hash)}`
  );
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema errors: ${JSON.stringify(errors)}`);
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-server.sh probe_failed path: SSH timeout -> probe_failed + access_gap SSH_23", () => {
  const { res, manifest, ts } = runProbe("tests/fleet-probe/fixtures/server-ssh-timeout.txt");
  assert.equal(res.status, 0, `exit should be 0 even on probe_failed; got ${res.status}`);
  assert.ok(manifest, "manifest MUST still be written on probe_failed");
  assert.equal(manifest.probe_status, "probe_failed");
  assert.ok(Array.isArray(manifest.probe_errors) && manifest.probe_errors.length >= 1);
  const connectErr = manifest.probe_errors.find((e) => e.sub_probe === "ssh_connect");
  assert.ok(connectErr, `expected ssh_connect error, got: ${JSON.stringify(manifest.probe_errors)}`);
  assert.equal(connectErr.access_gap, "SSH_23");
  assert.deepEqual(manifest.binary_sha256, {});
  assert.equal(manifest.build_id, null);
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema errors: ${JSON.stringify(errors)}`);
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-server.sh exits 2 when MANIFEST_TS unset", () => {
  const env = { ...process.env };
  delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-server.sh"], {
    env,
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2);
});
```
  </action>
  <verify>
    <automated>node --check tests/fleet-probe/probe-server.test.mjs &amp;&amp; test -f tests/fleet-probe/fixtures/server-ssh-ok.txt &amp;&amp; test -f tests/fleet-probe/fixtures/server-ssh-timeout.txt</automated>
  </verify>
  <acceptance_criteria>
    - `test -f tests/fleet-probe/fixtures/server-ssh-ok.txt` exits 0
    - `test -f tests/fleet-probe/fixtures/server-ssh-timeout.txt` exits 0
    - `grep -c "^---EXIT---$" tests/fleet-probe/fixtures/server-ssh-ok.txt` == 1
    - `grep -c "^---EXIT---$" tests/fleet-probe/fixtures/server-ssh-timeout.txt` == 1
    - `grep -c "access_gap" tests/fleet-probe/probe-server.test.mjs` >= 1
    - `grep -c "SSH_23" tests/fleet-probe/probe-server.test.mjs` >= 1
    - `node --check tests/fleet-probe/probe-server.test.mjs` exits 0
    - The test file references MOCK_SSH responder and uses PROBE_SSH env var pattern
  </acceptance_criteria>
  <done>Fixtures written, test file is syntactically valid, tests describe both ok and probe_failed paths with access_gap assertion. Tests FAIL at this stage (probe-server.sh does not exist yet) — that is RED state for TDD. Task 2 turns it GREEN.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Implement probe-server.sh (Tailscale SSH + Q5 three-way config + SWAPLOG parse)</name>
  <files>scripts/fleet-probe/probe-server.sh</files>
  <read_first>
    - scripts/deploy-preflight.sh lines 36-78 (findstr sentry_service_key + authed-exec loop — pattern for SSH one-shot)
    - scripts/deploy-server.sh (SSH quoting patterns, ConnectTimeout, 2>nul convention)
    - scripts/lib/ssh-helpers.sh (SCP-safe remote read + SSH banner detection — reuse if applicable)
    - scripts/fleet-probe/probe-james.sh (copy the manifest-assembly pattern verbatim; probe-server adds SSH layer)
    - SWAPLOG.md (see actual column format for the timestamp parser)
    - tests/fleet-probe/probe-server.test.mjs (Task 1 tests define the contract to satisfy)
  </read_first>
  <behavior>
    - Running with PROBE_SSH pointing to mock-ssh-responder.sh + ok scenario: manifest written with probe_status ok or partial, 3-way config_hash has at least server_live key, schema-valid
    - Running with PROBE_SSH ok but no /d/racecontrol.toml on James: config_hash has server_live + git_head keys; james_proxy missing; probe_errors[] has sub_probe: "config_hash_james_proxy" -> partial class
    - Running with PROBE_SSH timeout scenario: probe_status probe_failed, probe_errors[0] has sub_probe ssh_connect + access_gap SSH_23
    - SWAPLOG.md present: last_deploy_ts is non-null ISO-8601 with +05:30
    - SWAPLOG.md absent: last_deploy_ts null + probe_errors[] appends sub_probe: last_deploy_ts
    - MANIFEST_TS unset: exit 2
  </behavior>
  <action>
Create `scripts/fleet-probe/probe-server.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-server.sh — Phase 448 Plan 03
# Probes Server .23 via Tailscale SSH (ADMIN@100.125.108.37).
# Captures: binary_sha256 (racecontrol.exe), build_id (HTTP /api/v1/health),
#           config_hash 3-way (server_live via SSH, james_proxy from D:\, git_head from repo),
#           running_procs, scheduled_tasks, autostart_entries, env_vars_hash, last_deploy_ts (SWAPLOG).
# Honors PROBE_SSH override for testing (default: ssh).
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
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="server_23"
HOSTNAME_VAL="Racing-Point-Server"
IP_VAL="192.168.31.23"
ROLE_VAL="server"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sub_probe="$1" error="$2" access_gap="${3:-}"
  local extra=""
  if [ -n "$access_gap" ]; then
    extra=",\"access_gap\":\"$access_gap\""
  fi
  PROBE_ERRORS_JSON=$(printf '%s' "$PROBE_ERRORS_JSON" | python3 -c "
import sys,json
a=json.load(sys.stdin)
entry={'sub_probe':'$sub_probe','error':'''$(printf '%s' "$error" | sed \"s/'/\\\\\\\\''/g\")'''}
$( [ -n "$access_gap" ] && echo "entry['access_gap']='$access_gap'" )
a.append(entry)
print(json.dumps(a))
")
}

# --- Connect-stage probe: single SSH round-trip gathers hostname + multiple outputs in ONE session ---
# Use clear section markers so we can split output back apart.
SSH_SCRIPT='
echo "===MARK:hostname==="
hostname
echo "===MARK:certutil_exe==="
certutil -hashfile "C:\RacingPoint\racecontrol.exe" SHA256 2>nul
echo "===MARK:certutil_toml==="
certutil -hashfile "C:\RacingPoint\racecontrol.toml" SHA256 2>nul
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
  append_error "ssh_connect" "SSH to $SSH_TARGET failed or truncated (exit=$SSH_EXIT)" "SSH_23"
fi

extract_section() {
  local marker="$1" next="$2"
  printf '%s' "$SSH_OUT" | awk -v start="===MARK:$marker===" -v end="===MARK:$next===" '
    $0==start { capture=1; next }
    $0==end   { capture=0; exit }
    capture   { print }
  '
}

BIN_SHA=""
CFG_SERVER_SHA=""
TASKLIST_OUT=""
SCHTASKS_OUT=""
REG_HKLM_OUT=""
REG_HKCU_OUT=""
ENV_OUT=""

if [ "$CONNECT_ERR" -eq 0 ]; then
  # Parse each section. certutil output has 3 lines: header / hex / trailer. Hex line matches ^[0-9a-f ]{40,}$ after lowercase.
  CERT_EXE=$(extract_section "certutil_exe" "certutil_toml" | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
  CERT_TOML=$(extract_section "certutil_toml" "tasklist" | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0)}')
  if [ -n "$CERT_EXE" ] && echo "$CERT_EXE" | grep -qE '^[0-9a-f]{64}$'; then
    BIN_SHA="$CERT_EXE"
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "binary_sha256" "certutil hash did not yield 64-char hex"
  fi
  if [ -n "$CERT_TOML" ] && echo "$CERT_TOML" | grep -qE '^[0-9a-f]{64}$'; then
    CFG_SERVER_SHA="$CERT_TOML"
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "config_hash_server_live" "certutil on racecontrol.toml did not yield hex"
  fi
  TASKLIST_OUT=$(extract_section "tasklist" "schtasks")
  SCHTASKS_OUT=$(extract_section "schtasks" "reg_hklm")
  REG_HKLM_OUT=$(extract_section "reg_hklm" "reg_hkcu")
  REG_HKCU_OUT=$(extract_section "reg_hkcu" "env")
  ENV_OUT=$(extract_section "env" "end")
fi

# --- Q5 james_proxy config hash ---
CFG_JAMES_SHA=""
if [ -f "/d/racecontrol.toml" ]; then
  CFG_JAMES_SHA=$(sha256_of "/d/racecontrol.toml")
else
  SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
  append_error "config_hash_james_proxy" "D:\\racecontrol.toml not found on James"
fi

# --- Q5 git_head config hash ---
CFG_GIT_SHA=""
for candidate in "crates/racecontrol/racecontrol.toml" "crates/racecontrol/config/racecontrol.toml" "racecontrol.toml"; do
  if [ -f "$candidate" ]; then
    CFG_GIT_SHA=$(sha256_of "$candidate")
    break
  fi
done
if [ -z "$CFG_GIT_SHA" ]; then
  # Fallback: try git show HEAD:<path> for any tracked racecontrol.toml
  GIT_CONTENT=$(git show HEAD:crates/racecontrol/racecontrol.toml 2>/dev/null || true)
  if [ -n "$GIT_CONTENT" ]; then
    CFG_GIT_SHA=$(printf '%s' "$GIT_CONTENT" | sha256_of_stdin)
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "config_hash_git_head" "no racecontrol.toml in working tree or git HEAD"
  fi
fi

# --- Assemble config_hash JSON ---
CONFIG_HASH_JSON=$(python3 -c '
import json, os
d = {}
for k, env_var in [("racecontrol.toml.server_live", "CFG_SERVER_SHA"),
                   ("racecontrol.toml.james_proxy", "CFG_JAMES_SHA"),
                   ("racecontrol.toml.git_head", "CFG_GIT_SHA")]:
    v = os.environ.get(env_var, "")
    if v:
        d[k] = v
print(json.dumps(d))
' CFG_SERVER_SHA="$CFG_SERVER_SHA" CFG_JAMES_SHA="$CFG_JAMES_SHA" CFG_GIT_SHA="$CFG_GIT_SHA")

# --- build_id via HTTP /api/v1/health (LAN) ---
BUILD_ID="null"
if [ "${PROBE_SKIP_HTTP:-0}" != "1" ] && [ "$CONNECT_ERR" -eq 0 ]; then
  HEALTH=$(curl -s --max-time 5 "http://$IP_VAL:8080/api/v1/health" 2>/dev/null || true)
  if [ -n "$HEALTH" ]; then
    BUILD_ID=$(printf '%s' "$HEALTH" | jq -r '.build_id // empty' 2>/dev/null || true)
    if [ -n "$BUILD_ID" ]; then
      BUILD_ID_JSON=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BUILD_ID")
      BUILD_ID="$BUILD_ID_JSON"
    else
      BUILD_ID="null"
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/health response missing build_id"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "build_id" "HTTP /api/v1/health unreachable"
  fi
fi

# --- SWAPLOG.md last_deploy_ts ---
LAST_DEPLOY_JSON="null"
if [ -f "SWAPLOG.md" ]; then
  LAST_ROW=$(grep -E '^\| [0-9]' SWAPLOG.md | tail -1)
  if [ -n "$LAST_ROW" ]; then
    # Column 2 format typically "YYYY-MM-DD HH:MM IST"
    TS=$(printf '%s' "$LAST_ROW" | awk -F'|' '{gsub(/^ +| +$/,"",$2); print $2}')
    # Convert "YYYY-MM-DD HH:MM IST" -> "YYYY-MM-DDTHH:MM:00+05:30"
    ISO=$(printf '%s' "$TS" | python3 -c '
import sys, re
s = sys.stdin.read().strip()
m = re.match(r"(\d{4}-\d{2}-\d{2})[ T](\d{2}:\d{2})(?::(\d{2}))?", s)
if m:
    secs = m.group(3) or "00"
    print(f"{m.group(1)}T{m.group(2)}:{secs}+05:30")
' || true)
    if [ -n "$ISO" ]; then
      LAST_DEPLOY_JSON="\"$ISO\""
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "last_deploy_ts" "SWAPLOG last row timestamp unparseable: $TS"
    fi
  else
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "last_deploy_ts" "SWAPLOG.md has no data rows"
  fi
else
  SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
  append_error "last_deploy_ts" "SWAPLOG.md not found in repo root"
fi

# --- Parse tasklist into running_procs ---
RUNNING_PROCS_JSON=$(printf '%s' "$TASKLIST_OUT" | python3 -c '
import sys, csv, hashlib, json
rows = []
text = sys.stdin.read()
if not text.strip():
    print("[]"); sys.exit(0)
reader = csv.reader(text.splitlines())
first = True
for row in reader:
    if first: first = False; continue
    if len(row) < 2: continue
    name = row[0]
    try: pid = int(row[1])
    except: continue
    h = hashlib.sha256(" ".join(row).encode("utf-8","replace")).hexdigest()
    rows.append({"name": name, "pid": pid, "cmdline_hash": h})
print(json.dumps(rows[:100]))
')

# --- Parse schtasks into scheduled_tasks ---
SCHTASKS_JSON=$(printf '%s' "$SCHTASKS_OUT" | python3 -c '
import sys, json
entries = []
cur = {}
for line in sys.stdin:
    line = line.rstrip("\r\n")
    if not line.strip():
        if cur.get("name") and cur.get("state"):
            entries.append({"name": cur["name"], "state": cur["state"]})
        cur = {}; continue
    if ":" in line:
        k, _, v = line.partition(":"); k = k.strip(); v = v.strip()
        if k == "TaskName": cur["name"] = v.lstrip("\\")
        elif k == "Status": cur["state"] = v
if cur.get("name") and cur.get("state"):
    entries.append({"name": cur["name"], "state": cur["state"]})
print(json.dumps(entries[:100]))
')

# --- Parse reg queries into autostart_entries ---
AUTOSTART_JSON=$(printf '%s---HKCU---%s' "$REG_HKLM_OUT" "$REG_HKCU_OUT" | python3 -c '
import sys, json
text = sys.stdin.read()
parts = text.split("---HKCU---", 1)
hklm = parts[0] if parts else ""
hkcu = parts[1] if len(parts) > 1 else ""
entries = []
for source, blob in (("HKLM_Run", hklm), ("HKCU_Run", hkcu)):
    for line in blob.splitlines():
        line = line.strip()
        if not line or line.startswith("HKEY_"): continue
        parts = line.split(None, 2)
        if len(parts) == 3:
            entries.append({"source": source, "key": parts[0], "value": parts[2]})
print(json.dumps(entries))
')

# --- env_vars_hash from remote `set` output ---
ENV_HASH=$(printf '%s' "$ENV_OUT" | tr -d '\r' | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}')
if [ -z "$ENV_HASH" ]; then
  ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
fi

# --- Timing + status ---
PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

# On probe_failed, empty out binary_sha256 + config_hash + running_procs + scheduled_tasks + autostart_entries
if [ "$PROBE_STATUS" = "probe_failed" ]; then
  BIN_SHA=""
  CONFIG_HASH_JSON="{}"
  RUNNING_PROCS_JSON="[]"
  SCHTASKS_JSON="[]"
  AUTOSTART_JSON="[]"
  BUILD_ID="null"
fi

BINARY_SHA_JSON="{}"
if [ -n "$BIN_SHA" ]; then
  BINARY_SHA_JSON=$(python3 -c 'import json,sys; print(json.dumps({"racecontrol.exe": sys.argv[1]}))' "$BIN_SHA")
fi

# --- Assemble manifest ---
MANIFEST_JSON=$(python3 -c '
import json, os, sys
m = {
  "schema_version": "1.0",
  "target_id": "server_23",
  "host": os.environ["HOSTNAME_VAL"],
  "ip": os.environ["IP_VAL"],
  "role": "server",
  "probed_at_ist": os.environ["PROBED_AT"],
  "probe_status": os.environ["PROBE_STATUS"],
  "binary_sha256": json.loads(os.environ["BINARY_SHA_JSON"]),
  "build_id": json.loads(os.environ["BUILD_ID"]),
  "config_hash": json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs": json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks": json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries": json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash": os.environ["ENV_HASH"],
  "last_deploy_ts": json.loads(os.environ["LAST_DEPLOY_JSON"]),
}
errors = json.loads(os.environ["PROBE_ERRORS_JSON"])
if errors: m["probe_errors"] = errors
print(json.dumps(m))
' HOSTNAME_VAL="$HOSTNAME_VAL" IP_VAL="$IP_VAL" PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" \
   BINARY_SHA_JSON="$BINARY_SHA_JSON" BUILD_ID="$BUILD_ID" CONFIG_HASH_JSON="$CONFIG_HASH_JSON" \
   RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" SCHTASKS_JSON="$SCHTASKS_JSON" AUTOSTART_JSON="$AUTOSTART_JSON" \
   ENV_HASH="$ENV_HASH" LAST_DEPLOY_JSON="$LAST_DEPLOY_JSON" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

`chmod +x scripts/fleet-probe/probe-server.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-server.sh &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-server.sh` exits 0
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-server.sh` == 1
    - `grep -c "ADMIN@100\\.125\\.108\\.37" scripts/fleet-probe/probe-server.sh` >= 1
    - `grep -c "PROBE_SSH" scripts/fleet-probe/probe-server.sh` >= 2 (SSH_CMD override + scenario)
    - `grep -c "racecontrol.toml.server_live" scripts/fleet-probe/probe-server.sh` >= 1
    - `grep -c "racecontrol.toml.james_proxy" scripts/fleet-probe/probe-server.sh` >= 1
    - `grep -c "racecontrol.toml.git_head" scripts/fleet-probe/probe-server.sh` >= 1
    - `grep -c "SWAPLOG.md" scripts/fleet-probe/probe-server.sh` >= 1
    - `grep -c "SSH_23" scripts/fleet-probe/probe-server.sh` >= 1 (access_gap identifier)
    - `grep -c "2>nul" scripts/fleet-probe/probe-server.sh` >= 1 (Windows SSH convention)
    - `grep -c "TZ=Asia/Kolkata" scripts/fleet-probe/probe-server.sh` == 0 (banned pattern)
    - `npm run test:fleet-probe` exits 0 (all probe-server.test.mjs tests green, including probe_failed path with access_gap=SSH_23)
    - `npm run test:fleet-drift` still exits 0 (Phase 447 regression)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-server.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>probe-server.sh produces schema-valid manifests on both ok and probe_failed paths; Q5 three-way config_hash keys present when data available; SWAPLOG parsing extracts last_deploy_ts; PROBE_SSH override makes unit tests deterministic and offline.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0 (schema-compat + probe-james + probe-server tests)
- `npm run test:fleet-drift` still exits 0
- Server probe produces a manifest with 3 config_hash keys (when all inputs available)
- probe_failed path produces a schema-valid manifest (not a missing file) and flags access_gap: SSH_23
</verification>

<success_criteria>
- Q5 three-way drift is now captured in the manifest shape (Phase 452 diff tool will use this)
- probe-server.sh can be run offline via PROBE_SSH mock for CI
- PROBE-01 access-gap reporting mechanism proven end-to-end (the unreachable-target -> probe_failed + access_gap pattern works)
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-03-SUMMARY.md` with:
- Files created
- Test results (unit test pass counts)
- Sample ok + probe_failed manifest excerpts
- Confirmation of 3-way config_hash keys
- Handoff to Plan 04 (probe-pod.sh + probe-pos.sh in parallel Wave 2)
</output>
