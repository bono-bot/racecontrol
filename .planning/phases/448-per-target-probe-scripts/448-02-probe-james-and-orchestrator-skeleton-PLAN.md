---
phase: 448-per-target-probe-scripts
plan: 02
type: execute
wave: 1
depends_on: ["448-01"]
files_modified:
  - scripts/fleet-probe/probe-james.sh
  - scripts/fleet-probe/probe-all.sh
  - tests/fleet-probe/probe-james.test.mjs
  - tests/fleet-probe/smoke-james.sh
autonomous: true
requirements: [PROBE-04, PROBE-09]
gap_closure: false

must_haves:
  truths:
    - "Staff can run `bash scripts/fleet-probe/probe-james.sh` and get a schema-valid james_27 manifest without network calls"
    - "Staff can run `bash scripts/fleet-probe/probe-all.sh --dry-run` and see the 11-target enumeration without making any real calls"
    - "probe-james.sh uses the Plan 01 shared lib (source lib/probe-common.sh, call write_manifest)"
    - "probe-james.sh never emits probe_status: probe_failed because it is pure localhost (always-available class)"
  artifacts:
    - path: "scripts/fleet-probe/probe-james.sh"
      provides: "James .27 localhost probe — tasklist, schtasks, reg query, startup folder, pm2 list"
      min_lines: 120
    - path: "scripts/fleet-probe/probe-all.sh"
      provides: "Orchestrator SKELETON — --dry-run prints all 11 target_ids, enforces MANIFEST_TS, no real probe calls yet (full wiring comes in Plan 07)"
      min_lines: 80
    - path: "tests/fleet-probe/probe-james.test.mjs"
      provides: "Node test: spawn probe-james.sh with MANIFEST_TS set, assert manifest written + schema-valid"
      min_lines: 40
    - path: "tests/fleet-probe/smoke-james.sh"
      provides: "Bash smoke: runs probe-james.sh in a tempdir, cats manifest, exits 0"
      min_lines: 30
  key_links:
    - from: "scripts/fleet-probe/probe-james.sh"
      to: "scripts/fleet-probe/lib/probe-common.sh"
      via: "source statement at top"
      pattern: "source .*lib/probe-common.sh"
    - from: "scripts/fleet-probe/probe-james.sh"
      to: "scripts/fleet-probe/validate-manifest-file.mjs"
      via: "write_manifest -> FLEET_PROBE_VALIDATE=1 gate"
      pattern: "FLEET_PROBE_VALIDATE"
    - from: "scripts/fleet-probe/probe-all.sh"
      to: "target-enumeration case block"
      via: "11-target enumeration per CONTEXT.md + RESEARCH §3"
      pattern: "server_23|pod_1|pod_2|pod_3|pod_4|pod_5|pod_6|pod_7|pod_8|pos_130|james_27|bono_vps|cloud_admin|cloud_racecontrol|relay_james"

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
Wave 1: Ship the SAFEST probe (probe-james.sh is pure localhost) + an orchestrator skeleton that can enumerate targets in --dry-run mode. This proves the Plan 01 helpers work end-to-end before any SSH/HTTP probe is written.

Purpose: Validate the assembly pattern (source lib, gather data, build JSON, write_manifest) on a zero-risk target. Prove the orchestrator can list all 11 target_ids correctly before Plan 07 wires real probe invocations.

Output: 1 working probe (james_27) + 1 orchestrator skeleton (dry-run mode only) + 2 tests.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md
@.planning/phases/448-per-target-probe-scripts/448-VALIDATION.md
@.planning/phases/448-per-target-probe-scripts/448-01-wave0-scaffolding-PLAN.md

# Reused Plan 01 artifacts
@scripts/fleet-probe/lib/probe-common.sh
@scripts/fleet-probe/validate-manifest-file.mjs
@tests/fleet-probe/helpers.mjs

# Schema + example (shape reference for james_27)
@schemas/fleet-manifest.schema.json
@schemas/examples/james_27.json

<interfaces>
From Plan 01 shared lib (already sourced):
```
json_escape, write_manifest, sha256_of, sha256_of_stdin, sha256_of_remote_file,
iso_ist_now, probe_status_from_errors, env_names_hash, env_names_hash_remote, cmdline_hash
```

**probe-james.sh contract**
```
Usage: bash scripts/fleet-probe/probe-james.sh
Preconditions: $MANIFEST_TS exported (orchestrator does this; direct callers must export manually)
Stdout (single line JSON): {"target_id":"james_27","probe_status":"ok","duration_ms":N,"errors_count":0}
Side effect: writes state/fleet-manifest/$MANIFEST_TS/james_27.json (schema-valid)
Exit: 0 (never probe_failed — localhost always reachable; script crash -> exit 2)
```

**probe-all.sh --dry-run contract (Plan 02 skeleton only)**
```
Usage: bash scripts/fleet-probe/probe-all.sh --dry-run
Behavior: prints one "target=<id> role=<role>" line per target, 11 lines total, in enumeration order. No network calls, no manifest writes.
Exit: 0
```

**11-target enumeration (LOCKED from CONTEXT.md + RESEARCH §3):**
```
target=server_23          role=server
target=pod_1              role=pod
target=pod_2              role=pod
target=pod_3              role=pod
target=pod_4              role=pod
target=pod_5              role=pod
target=pod_6              role=pod
target=pod_7              role=pod
target=pod_8              role=pod
target=pos_130            role=pos
target=james_27           role=james
target=bono_vps           role=vps
target=cloud_admin        role=cloud_admin
target=cloud_racecontrol  role=cloud_racecontrol
target=relay_james        role=relay
```
(15 lines for 11 targets? No — 11 distinct target_ids. Re-count: server_23 (1) + 8 pods (9) + pos_130 (10) + james_27 (11) + bono_vps (12) + cloud_admin (13) + cloud_racecontrol (14) + relay_james (15) = 15 targets.)

**IMPORTANT CORRECTION — target count is 15, not 11.**
Count per RESEARCH §3 table: 15 rows. CONTEXT.md says "11-host fleet" but the schema role enum has 8 values (server, pod, pos, james, vps, cloud_admin, cloud_racecontrol, relay) and the target count across those roles is 15 (server=1 + pod=8 + pos=1 + james=1 + vps=1 + cloud_admin=1 + cloud_racecontrol=1 + relay=1 = 15). Orchestrator prints 15 lines in --dry-run.

(The "11-host fleet" phrasing in planning docs counts physical hosts; server + 8 pods + pos + james = 11 local hosts + 1 remote VPS + 3 cloud logical services + 1 relay composite = 15 probe manifests. Use 15 throughout.)
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create probe-james.sh (pure localhost probe)</name>
  <files>scripts/fleet-probe/probe-james.sh, tests/fleet-probe/probe-james.test.mjs, tests/fleet-probe/smoke-james.sh</files>
  <read_first>
    - scripts/fleet-probe/lib/probe-common.sh (see which helpers exist; use them verbatim)
    - schemas/examples/james_27.json (shape reference — note last_deploy_ts: null for james; comms-link has no deploy pipeline)
    - schemas/fleet-manifest.schema.json (confirm 15 required fields)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §7 (failure-mode matrix — james_27 row says "practically never fails, ok")
    - tests/fleet-probe/helpers.mjs (use loadFixture + validateAgainstSchema)
  </read_first>
  <behavior>
    - Running `MANIFEST_TS=test-$$; export MANIFEST_TS; bash scripts/fleet-probe/probe-james.sh` creates `state/fleet-manifest/test-$$/james_27.json` and exits 0
    - The written manifest validates against schemas/fleet-manifest.schema.json (all 15 required fields present)
    - probe_status = "ok" (james is the always-available class)
    - target_id = "james_27", host = "JAMES-PC", ip = "192.168.31.27", role = "james"
    - build_id = null (no Rust binary on James; comms-link is Node)
    - last_deploy_ts = null (no deploy pipeline for comms-link in v53.0 scope)
    - binary_sha256 = {} (James has no primary Rust binary)
    - config_hash includes at least one entry if `C:\Users\bono\racingpoint\comms-link\config.toml` or similar exists; else {}
    - running_procs array is non-empty (tasklist will always have processes on a running system)
    - env_vars_hash is a 64-char hex string (from env_names_hash)
    - probe_errors is omitted when probe_status=ok (schema allows its absence)
    - If MANIFEST_TS is unset: exit 2 with stderr message "MANIFEST_TS not set"
  </behavior>
  <action>
Create `scripts/fleet-probe/probe-james.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-james.sh — Phase 448 Plan 02
# Probes James .27 localhost: tasklist, schtasks, HKLM/HKCU Run, startup folder, pm2.
# Pure localhost — never emits probe_failed.
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

# --- sub_probe: running_procs (tasklist /V /FO CSV) ---
RUNNING_PROCS_JSON="[]"
if tasklist_raw=$(tasklist /V /FO CSV 2>/dev/null); then
  # Parse first 50 rows for brevity; hash cmdline. CSV: "Image Name","PID","Session Name","Session#","Mem","Status","User","CPU","Window Title"
  RUNNING_PROCS_JSON=$(printf '%s\n' "$tasklist_raw" | tail -n +2 | head -n 50 | python3 -c '
import sys, csv, hashlib, json
rows = []
reader = csv.reader(sys.stdin)
for row in reader:
    if len(row) < 2: continue
    name = row[0]
    try:
        pid = int(row[1])
    except Exception:
        continue
    cmdline = " ".join(row)  # approximation; tasklist has no cmdline column
    h = hashlib.sha256(cmdline.encode("utf-8", "replace")).hexdigest()
    rows.append({"name": name, "pid": pid, "cmdline_hash": h})
print(json.dumps(rows))
')
else
  ERR_COUNT_SUBPROBE=$((ERR_COUNT_SUBPROBE + 1))
  PROBE_ERRORS_JSON=$(printf '%s' "$PROBE_ERRORS_JSON" | python3 -c 'import sys,json; a=json.load(sys.stdin); a.append({"sub_probe":"tasklist","error":"tasklist failed on james localhost"}); print(json.dumps(a))')
fi

# --- sub_probe: scheduled_tasks (schtasks /Query /V /FO LIST) ---
SCHTASKS_JSON="[]"
if schtasks_raw=$(schtasks /Query /V /FO LIST 2>/dev/null); then
  SCHTASKS_JSON=$(printf '%s' "$schtasks_raw" | python3 -c '
import sys, json
entries = []
cur = {}
for line in sys.stdin:
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
print(json.dumps(entries[:100]))
')
else
  ERR_COUNT_SUBPROBE=$((ERR_COUNT_SUBPROBE + 1))
  PROBE_ERRORS_JSON=$(printf '%s' "$PROBE_ERRORS_JSON" | python3 -c 'import sys,json; a=json.load(sys.stdin); a.append({"sub_probe":"schtasks_query","error":"schtasks /Query failed"}); print(json.dumps(a))')
fi

# --- sub_probe: autostart_entries (reg query HKLM/HKCU Run + startup folder) ---
AUTOSTART_JSON=$(python3 -c '
import subprocess, os, json
entries = []
for hive, source in (("HKLM", "HKLM_Run"), ("HKCU", "HKCU_Run")):
    try:
        out = subprocess.check_output(["reg", "query", f"{hive}\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run"], text=True, stderr=subprocess.DEVNULL)
        for line in out.splitlines():
            line = line.strip()
            if not line or line.startswith("HKEY_"):
                continue
            parts = line.split(None, 2)
            if len(parts) == 3:
                entries.append({"source": source, "key": parts[0], "value": parts[2]})
    except Exception:
        pass
startup = os.path.expandvars(r"%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup")
if os.path.isdir(startup):
    for f in os.listdir(startup):
        entries.append({"source": "startup_folder", "key": f, "value": os.path.join(startup, f)})
print(json.dumps(entries))
')

# --- sub_probe: env_vars_hash ---
ENV_HASH=$(env_names_hash)

# --- sub_probe: config_hash (best-effort — comms-link config if present) ---
CONFIG_HASH_JSON="{}"
COMMS_CFG="$HOME/racingpoint/comms-link/config.toml"
if [ -f "$COMMS_CFG" ]; then
  CONFIG_HASH_JSON=$(printf '{"comms-link/config.toml":"%s"}' "$(sha256_of "$COMMS_CFG")")
fi

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors 0 "$ERR_COUNT_SUBPROBE")

# Assemble manifest JSON via python3 (safe escaping).
MANIFEST_JSON=$(python3 -c '
import json, sys, os
m = {
  "schema_version": "1.0",
  "target_id": os.environ["TARGET_ID"],
  "host": os.environ["HOSTNAME_VAL"],
  "ip": os.environ["IP_VAL"],
  "role": os.environ["ROLE_VAL"],
  "probed_at_ist": os.environ["PROBED_AT"],
  "probe_status": os.environ["PROBE_STATUS"],
  "binary_sha256": {},
  "build_id": None,
  "config_hash": json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs": json.loads(os.environ["RUNNING_PROCS_JSON"]),
  "scheduled_tasks": json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries": json.loads(os.environ["AUTOSTART_JSON"]),
  "env_vars_hash": os.environ["ENV_HASH"],
  "last_deploy_ts": None,
}
errors = json.loads(os.environ["PROBE_ERRORS_JSON"])
if errors:
  m["probe_errors"] = errors
print(json.dumps(m))
' TARGET_ID="$TARGET_ID" HOSTNAME_VAL="$HOSTNAME_VAL" IP_VAL="$IP_VAL" ROLE_VAL="$ROLE_VAL" \
   PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" CONFIG_HASH_JSON="$CONFIG_HASH_JSON" \
   RUNNING_PROCS_JSON="$RUNNING_PROCS_JSON" SCHTASKS_JSON="$SCHTASKS_JSON" AUTOSTART_JSON="$AUTOSTART_JSON" \
   ENV_HASH="$ENV_HASH" PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

# Single-line stdout status
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$ERR_COUNT_SUBPROBE"
```

Create `tests/fleet-probe/probe-james.test.mjs`:

```js
// tests/fleet-probe/probe-james.test.mjs — Phase 448 Plan 02
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, mkdtempSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { validateAgainstSchema } from "./helpers.mjs";

test("probe-james.sh writes schema-valid manifest and exits 0", () => {
  const ts = "test-" + Date.now();
  const env = { ...process.env, MANIFEST_TS: ts };
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-james.sh"], {
    env,
    encoding: "utf8",
    timeout: 30_000,
  });
  assert.equal(res.status, 0, `exit=${res.status} stderr=${res.stderr}`);

  const manifestPath = resolve("state/fleet-manifest", ts, "james_27.json");
  assert.ok(existsSync(manifestPath), `manifest not written: ${manifestPath}`);

  const obj = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(obj.target_id, "james_27");
  assert.equal(obj.role, "james");
  assert.equal(obj.probe_status, "ok"); // james is always-available class
  assert.equal(obj.build_id, null);
  assert.equal(obj.last_deploy_ts, null);
  assert.ok(Array.isArray(obj.running_procs));
  assert.ok(obj.running_procs.length > 0, "running_procs should be non-empty on localhost");
  assert.ok(/^[0-9a-f]{64}$/.test(obj.env_vars_hash));

  const { valid, errors } = validateAgainstSchema(obj);
  assert.ok(valid, `schema errors: ${JSON.stringify(errors)}`);

  // stdout status line is one-line JSON
  const lines = res.stdout.trim().split(/\r?\n/);
  const status = JSON.parse(lines[lines.length - 1]);
  assert.equal(status.target_id, "james_27");
  assert.equal(status.probe_status, "ok");
  assert.equal(status.errors_count, 0);

  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-james.sh exits 2 when MANIFEST_TS unset", () => {
  const env = { ...process.env };
  delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-james.sh"], {
    env,
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2);
  assert.match(res.stderr, /MANIFEST_TS/);
});
```

Create `tests/fleet-probe/smoke-james.sh`:

```bash
#!/bin/bash
# tests/fleet-probe/smoke-james.sh — Phase 448 Plan 02 — live-runs probe-james.sh
set -eo pipefail
export MANIFEST_TS="smoke-$(date +%s)"
mkdir -p "state/fleet-manifest/$MANIFEST_TS"
STATUS=$(bash scripts/fleet-probe/probe-james.sh | tail -1)
echo "status: $STATUS"
test -f "state/fleet-manifest/$MANIFEST_TS/james_27.json" || { echo "FAIL: manifest not written" >&2; exit 1; }
node scripts/fleet-probe/validate-manifest-file.mjs "state/fleet-manifest/$MANIFEST_TS/james_27.json"
echo "smoke-james OK"
rm -rf "state/fleet-manifest/$MANIFEST_TS"
```

`chmod +x scripts/fleet-probe/probe-james.sh tests/fleet-probe/smoke-james.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-james.sh &amp;&amp; bash -n tests/fleet-probe/smoke-james.sh &amp;&amp; node --check tests/fleet-probe/probe-james.test.mjs &amp;&amp; npm run test:fleet-probe &amp;&amp; bash tests/fleet-probe/smoke-james.sh</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-james.sh` exits 0
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-james.sh` == 1
    - `grep -c "write_manifest" scripts/fleet-probe/probe-james.sh` >= 1
    - `grep -c "TARGET_ID=\"james_27\"" scripts/fleet-probe/probe-james.sh` == 1
    - `grep -c "ROLE_VAL=\"james\"" scripts/fleet-probe/probe-james.sh` == 1
    - `npm run test:fleet-probe` exits 0 (includes probe-james.test.mjs + schema-compat.test.mjs)
    - `bash tests/fleet-probe/smoke-james.sh` exits 0 and validates the written manifest
    - `node scripts/fleet-probe/validate-manifest-file.mjs state/fleet-manifest/smoke-*/james_27.json` (via the smoke test) exits 0
    - probe-james.sh is ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-james.sh','rb').read().decode('ascii')"` does not raise
    - Running with unset MANIFEST_TS: `env -u MANIFEST_TS bash scripts/fleet-probe/probe-james.sh; echo exit=$?` prints `exit=2`
  </acceptance_criteria>
  <done>probe-james.sh produces a schema-valid manifest on James localhost, smoke test passes, unit test passes, MANIFEST_TS precondition enforced with exit 2.</done>
</task>

<task type="auto">
  <name>Task 2: Create probe-all.sh orchestrator SKELETON with --dry-run target enumeration</name>
  <files>scripts/fleet-probe/probe-all.sh</files>
  <read_first>
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §5 (orchestrator shape) and §3 (15-target table)
    - .planning/phases/448-per-target-probe-scripts/448-CONTEXT.md &lt;decisions&gt; section (orchestrator contract)
    - schemas/examples/_meta.json (shape for later _meta.json writing — NOT yet built in this plan)
  </read_first>
  <action>
Create `scripts/fleet-probe/probe-all.sh` — Plan 02 SKELETON. Full wiring of real probes is Plan 07. For now, only --dry-run mode must work.

```bash
#!/bin/bash
# scripts/fleet-probe/probe-all.sh — Phase 448 orchestrator.
# Plan 02: skeleton with --dry-run target enumeration only. Plan 07 wires real invocations.
# Usage:
#   bash scripts/fleet-probe/probe-all.sh --dry-run   # prints target list, no network
#   bash scripts/fleet-probe/probe-all.sh             # (Plan 07) runs all 15 probes
#   bash scripts/fleet-probe/probe-all.sh --canary    # (Plan 07) runs only server_23 + pod_8
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

# 15 targets -- role -- (ordering locked in CONTEXT.md + RESEARCH section 3)
# Format: one line per target, space-separated fields.
# Fields: target_id role
TARGETS=(
  "server_23 server"
  "pod_1 pod"
  "pod_2 pod"
  "pod_3 pod"
  "pod_4 pod"
  "pod_5 pod"
  "pod_6 pod"
  "pod_7 pod"
  "pod_8 pod"
  "pos_130 pos"
  "james_27 james"
  "bono_vps vps"
  "cloud_admin cloud_admin"
  "cloud_racecontrol cloud_racecontrol"
  "relay_james relay"
)

MODE="full"
for arg in "$@"; do
  case "$arg" in
    --dry-run) MODE="dry-run" ;;
    --canary)  MODE="canary" ;;
    --help|-h)
      echo "Usage: probe-all.sh [--dry-run|--canary]"
      echo "  --dry-run  enumerate targets, no network calls (Plan 02)"
      echo "  --canary   run server_23 + pod_8 only (Plan 07)"
      echo "  (no flag)  run all 15 probes (Plan 07)"
      exit 0
      ;;
  esac
done

if [ "$MODE" = "dry-run" ]; then
  for entry in "${TARGETS[@]}"; do
    id="${entry%% *}"
    role="${entry##* }"
    printf "target=%-22s role=%s\n" "$id" "$role"
  done
  exit 0
fi

# Plan 07 wires the rest. Until then, refuse to run full/canary mode.
echo "probe-all.sh: full / canary modes are wired in Plan 448-07. Use --dry-run for now." >&2
exit 3
```

`chmod +x scripts/fleet-probe/probe-all.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-all.sh &amp;&amp; [ "$(bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l)" = "15" ] &amp;&amp; bash scripts/fleet-probe/probe-all.sh --dry-run | grep -q "^target=server_23" &amp;&amp; bash scripts/fleet-probe/probe-all.sh --dry-run | grep -q "^target=pod_8" &amp;&amp; bash scripts/fleet-probe/probe-all.sh --dry-run | grep -q "^target=relay_james"</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-all.sh` exits 0
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l` == 15
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=server_23"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=pod_1"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=pod_8"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=pos_130"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=james_27"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=bono_vps"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=cloud_admin"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=cloud_racecontrol"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | grep -c "^target=relay_james"` == 1
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | awk '{print $1}' | sort -u | wc -l` == 15 (no duplicates)
    - `bash scripts/fleet-probe/probe-all.sh` (no flag) exits 3 with stderr message citing "Plan 448-07"
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-all.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>Orchestrator skeleton enumerates exactly 15 targets in --dry-run mode; default mode refuses to run (clean handoff to Plan 07); all 15 target_ids match the RESEARCH §3 list verbatim.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0 (schema-compat + probe-james tests green)
- `bash tests/fleet-probe/smoke-james.sh` exits 0
- `bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l` == 15
- `npm run test:fleet-drift` (Plan 01 regression check) still exits 0
</verification>

<success_criteria>
- probe-james.sh is the first fully-functional probe; its manifest passes ajv schema validation
- Orchestrator skeleton proves 15-target enumeration is correct (downstream plans can rely on it)
- Zero network calls made in any test run
- Shared lib usage pattern established for Waves 2-3
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-02-SUMMARY.md` with:
- Files added/modified
- Sample probe-james.sh output (one status line + brief manifest peek)
- Test results (`npm run test:fleet-probe`, `bash tests/fleet-probe/smoke-james.sh`)
- Confirmation that orchestrator skeleton enumerates 15 targets
- Handoff note for Plan 03 (probe-server.sh will be the first SSH probe)
</output>
