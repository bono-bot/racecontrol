---
phase: 448-per-target-probe-scripts
plan: 06
type: execute
wave: 3
depends_on: ["448-01", "448-02"]
files_modified:
  - scripts/fleet-probe/probe-cloud-admin.sh
  - scripts/fleet-probe/probe-cloud-rc.sh
  - tests/fleet-probe/probe-cloud-admin.test.mjs
  - tests/fleet-probe/probe-cloud-rc.test.mjs
  - tests/fleet-probe/fixtures/cloud-admin-health-ok.json
  - tests/fleet-probe/fixtures/cloud-admin-health-gated.json
  - tests/fleet-probe/fixtures/cloud-rc-health-ok.json
autonomous: true
requirements: [PROBE-06, PROBE-07]
gap_closure: false

must_haves:
  truths:
    - "probe-cloud-admin.sh captures build_id + git_commit + pages_missing from https://admin.racingpoint.cloud/api/health (public)"
    - "probe-cloud-admin.sh detects ADMIN_COMING_SOON_GATE active state via HEAD / redirect check (307) and flags it as scheduled_tasks entry (not an error)"
    - "probe-cloud-rc.sh captures build_id from https://racingpoint.cloud/api/v1/health"
    - "Missing STAFF_JWT degrades cloud-admin gate-state probe to partial (auth_gap: staff_jwt_expired); public /api/health still probed"
    - "Unit tests mock HTTPS via startMockHttpServer (HTTP in test; host/TLS verification gated by PROBE_SKIP_TLS=1 in probes)"
  artifacts:
    - path: "scripts/fleet-probe/probe-cloud-admin.sh"
      provides: "Cloud admin probe: /api/health (build_id, git_commit, pages_missing) + HEAD / gate detection"
      min_lines: 130
    - path: "scripts/fleet-probe/probe-cloud-rc.sh"
      provides: "Cloud racecontrol probe: /api/v1/health build_id"
      min_lines: 100
    - path: "tests/fleet-probe/probe-cloud-admin.test.mjs"
      provides: "Unit tests: ok (build_id captured), gated (307 detected as scheduled_tasks entry), pages_missing -> probe_errors[] pages_probe"
      min_lines: 60
    - path: "tests/fleet-probe/probe-cloud-rc.test.mjs"
      provides: "Unit tests: ok (build_id captured), 500 (probe_failed), malformed JSON (partial + health_parse)"
      min_lines: 40
  key_links:
    - from: "scripts/fleet-probe/probe-cloud-admin.sh"
      to: "https://admin.racingpoint.cloud/api/health"
      via: "curl + jq extraction of build_id + git_commit + pages_missing"
      pattern: "admin\\.racingpoint\\.cloud"
    - from: "scripts/fleet-probe/probe-cloud-admin.sh"
      to: "ADMIN_COMING_SOON_GATE detection"
      via: "curl -sI / - detects 307 -> /coming-soon"
      pattern: "ADMIN_COMING_SOON_GATE|coming-soon"
    - from: "scripts/fleet-probe/probe-cloud-rc.sh"
      to: "https://racingpoint.cloud/api/v1/health"
      via: "curl + jq .build_id"
      pattern: "racingpoint\\.cloud/api/v1/health"

deploy:
  rust_binary: [none]
  frontend_rebuild: [none]
  config_change: none
  db_migration: none
  infrastructure: none
  data_files: none
  bat_file: none
  cloud_parity: [none]
  targets: [cloud]
---

<objective>
Wave 3 probes 3+4 of 4: Ship probe-cloud-admin.sh (https://admin.racingpoint.cloud/api/health + gate detection) and probe-cloud-rc.sh (https://racingpoint.cloud/api/v1/health). Both are pure-HTTP probes using startMockHttpServer for unit tests.

Purpose: Cloud admin + cloud racecontrol are the two logical cloud services that ran from Bono VPS. The admin panel's Coming Soon gate (PR #13) is an intentional state that probe-cloud-admin.sh must surface as a scheduled_tasks entry (not an error) so Phase 452's diff tool can flag it appropriately.

Output: 2 probes + 2 unit tests + 3 HTTP response fixtures.
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

# Shape references
@schemas/examples/cloud_admin.json
@schemas/examples/cloud_racecontrol.json

<interfaces>
**probe-cloud-admin.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-cloud-admin.sh
Optional env:
  PROBE_OVERRIDE_CLOUD_ADMIN_URL  -- overrides https://admin.racingpoint.cloud (tests: http://127.0.0.1:PORT)
  STAFF_JWT                        -- optional; enables gated-page probe
Stdout: {"target_id":"cloud_admin","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: state/fleet-manifest/$MANIFEST_TS/cloud_admin.json
Sources:
  - GET $URL/api/health -> build_id, git_commit, pages_missing[]
  - HEAD $URL/ -> 307 redirect to /coming-soon means gate active
Failure classes:
  - DNS/TLS/connect failure -> probe_failed + sub_probe: dns|tls|connectivity
  - /api/health 200 but malformed -> partial + sub_probe: health_parse
  - pages_missing[] non-empty -> partial + sub_probe: pages_probe + error listing missing pages
  - Gate active (307 to /coming-soon) -> probe_status stays OK; ADMIN_COMING_SOON_GATE appended to scheduled_tasks as {name: ADMIN_COMING_SOON_GATE, state: active}
  - STAFF_JWT expired (only attempted if set): sub_probe: authed_page_check + auth_gap: staff_jwt_expired -> partial
```

**probe-cloud-rc.sh contract**
```
Usage: MANIFEST_TS=<iso> bash scripts/fleet-probe/probe-cloud-rc.sh
Optional env:
  PROBE_OVERRIDE_CLOUD_RC_URL  -- overrides https://racingpoint.cloud
Stdout: {"target_id":"cloud_racecontrol","probe_status":"ok|probe_failed|partial","duration_ms":N,"errors_count":N}
Side effect: state/fleet-manifest/$MANIFEST_TS/cloud_racecontrol.json
Sources:
  - GET $URL/api/v1/health -> build_id
Failure classes:
  - connect failure -> probe_failed
  - non-200 -> probe_failed + health error
  - 200 but missing build_id -> partial + build_id null
  - 200 but malformed JSON -> partial + health_parse
```

**Example responses (from Phase 445 STATE.md):**
```
https://admin.racingpoint.cloud/api/health -> {"healthy":true, "build_id":"cDyHRUgWTiqZTchmlEPgz", "git_commit":"dfaabe6", "pages_missing":[]}
https://racingpoint.cloud/api/v1/health    -> {"build_id":"129a24f2", ...}
```
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create probe-cloud-admin.sh + 2 fixtures + unit test</name>
  <files>scripts/fleet-probe/probe-cloud-admin.sh, tests/fleet-probe/probe-cloud-admin.test.mjs, tests/fleet-probe/fixtures/cloud-admin-health-ok.json, tests/fleet-probe/fixtures/cloud-admin-health-gated.json</files>
  <read_first>
    - scripts/auto-detect.sh lines around bono_health/admin_health (pattern for cloud health curl)
    - schemas/examples/cloud_admin.json (shape reference)
    - tests/fleet-probe/helpers.mjs (use startMockHttpServer with /api/health + / routes)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §7 (cloud_admin failure matrix)
  </read_first>
  <behavior>
    - Mock server returning /api/health 200 with {build_id, git_commit, pages_missing:[]} and / 200 -> probe_status ok, build_id captured, no probe_errors
    - Mock server returning /api/health 200 and / 307 (Location: /coming-soon) -> probe_status ok, scheduled_tasks contains {name:ADMIN_COMING_SOON_GATE, state:active}
    - Mock /api/health with pages_missing:["/staff","/billing"] -> partial + sub_probe pages_probe + error string listing pages
    - Mock /api/health returns 500 -> probe_failed + sub_probe health
    - STAFF_JWT unset -> gate-state probe still attempted (no auth needed for HEAD /); only authed_page_check is gated on STAFF_JWT
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/cloud-admin-health-ok.json`:

```json
{"healthy":true,"build_id":"cDyHRUgWTiqZTchmlEPgz","git_commit":"dfaabe6","pages_missing":[],"uptime_secs":3600}
```

Create `tests/fleet-probe/fixtures/cloud-admin-health-gated.json`:

```json
{"healthy":true,"build_id":"0a78db9","git_commit":"0a78db9","pages_missing":["/staff","/billing"],"gated":true}
```

Create `scripts/fleet-probe/probe-cloud-admin.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-cloud-admin.sh — Phase 448 Plan 06
# Probes admin.racingpoint.cloud via HTTPS /api/health + HEAD / gate detection.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-cloud-admin: MANIFEST_TS not set" >&2
  exit 2
fi

CLOUD_URL="${PROBE_OVERRIDE_CLOUD_ADMIN_URL:-https://admin.racingpoint.cloud}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="cloud_admin"
HOSTNAME_VAL="admin.racingpoint.cloud"
IP_VAL="45.11.110.250"
ROLE_VAL="cloud_admin"

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

# --- /api/health probe ---
HEALTH_RAW=$(curl -s --max-time 10 -w "\n%{http_code}" "$CLOUD_URL/api/health" 2>/dev/null || echo "")
HEALTH_CODE=$(printf '%s' "$HEALTH_RAW" | tail -1)
HEALTH_BODY=$(printf '%s' "$HEALTH_RAW" | head -n -1)

BUILD_ID="null"
GIT_COMMIT=""
PAGES_MISSING=""

if [ "$HEALTH_CODE" = "000" ] || [ -z "$HEALTH_CODE" ]; then
  CONNECT_ERR=1
  append_error "connectivity" "cannot reach $CLOUD_URL/api/health (DNS/TLS/connect failure)"
elif [ "$HEALTH_CODE" != "200" ]; then
  CONNECT_ERR=1
  append_error "health" "HTTP $HEALTH_CODE from $CLOUD_URL/api/health"
else
  if ! printf '%s' "$HEALTH_BODY" | jq . >/dev/null 2>&1; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "health_parse" "cloud admin /api/health returned non-JSON"
  else
    BID=$(printf '%s' "$HEALTH_BODY" | jq -r '.build_id // empty' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    fi
    GIT_COMMIT=$(printf '%s' "$HEALTH_BODY" | jq -r '.git_commit // empty' 2>/dev/null || true)
    PAGES_MISSING=$(printf '%s' "$HEALTH_BODY" | jq -r '.pages_missing // [] | @csv' 2>/dev/null || echo "")
    if [ -n "$PAGES_MISSING" ] && [ "$PAGES_MISSING" != "" ]; then
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "pages_probe" "pages_missing: $PAGES_MISSING"
    fi
  fi
fi

# --- Gate detection via HEAD / ---
GATE_ACTIVE=0
if [ "$CONNECT_ERR" -eq 0 ]; then
  HEAD_STATUS=$(curl -s --max-time 10 -o /dev/null -w "%{http_code}" -I "$CLOUD_URL/" 2>/dev/null || echo "000")
  HEAD_LOC=$(curl -s --max-time 10 -I "$CLOUD_URL/" 2>/dev/null | tr -d '\r' | awk -F': ' 'tolower($1)=="location" {print $2}' | tr -d ' ')
  if [ "$HEAD_STATUS" = "307" ] || echo "$HEAD_LOC" | grep -qi "coming-soon"; then
    GATE_ACTIVE=1
  fi
fi

# --- Compose scheduled_tasks (gate state) ---
SCHTASKS_JSON="[]"
if [ "$GATE_ACTIVE" -eq 1 ]; then
  SCHTASKS_JSON='[{"name":"ADMIN_COMING_SOON_GATE","state":"active"}]'
fi

# running_procs empty (cloud service, no process snapshot)
RUNNING_PROCS_JSON="[]"
AUTOSTART_JSON="[]"
CONFIG_HASH_JSON="{}"
BINARY_SHA_JSON="{}"

# Compose a lightweight config_hash derived from git_commit if available (stable per-deploy fingerprint)
if [ -n "$GIT_COMMIT" ]; then
  # Convert git_commit short hash into a pseudo-config_hash by SHA256 of the commit string.
  # This keeps the schema contract (64-char hex) while giving Phase 452 a stable per-deploy key.
  CMT_HASH=$(printf '%s' "$GIT_COMMIT" | sha256sum | awk '{print $1}')
  CONFIG_HASH_JSON=$(python3 -c 'import json,sys; print(json.dumps({"admin.git_commit": sys.argv[1]}))' "$CMT_HASH")
fi

ENV_HASH="e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

if [ "$PROBE_STATUS" = "probe_failed" ]; then
  CONFIG_HASH_JSON="{}"
  SCHTASKS_JSON="[]"
  BUILD_ID="null"
fi

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version":"1.0","target_id":"cloud_admin","host":"admin.racingpoint.cloud","ip":os.environ["IP_VAL"],"role":"cloud_admin",
  "probed_at_ist":os.environ["PROBED_AT"],"probe_status":os.environ["PROBE_STATUS"],
  "binary_sha256":{},"build_id":json.loads(os.environ["BUILD_ID"]),
  "config_hash":json.loads(os.environ["CONFIG_HASH_JSON"]),
  "running_procs":[],
  "scheduled_tasks":json.loads(os.environ["SCHTASKS_JSON"]),
  "autostart_entries":[],
  "env_vars_hash":os.environ["ENV_HASH"],"last_deploy_ts":None,
}
err=json.loads(os.environ["PROBE_ERRORS_JSON"])
if err: m["probe_errors"]=err
print(json.dumps(m))
' IP_VAL="$IP_VAL" PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" BUILD_ID="$BUILD_ID" \
   CONFIG_HASH_JSON="$CONFIG_HASH_JSON" SCHTASKS_JSON="$SCHTASKS_JSON" ENV_HASH="$ENV_HASH" \
   PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-cloud-admin.test.mjs`:

```js
// tests/fleet-probe/probe-cloud-admin.test.mjs — Phase 448 Plan 06
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema, loadFixture } from "./helpers.mjs";
import { createServer } from "node:http";

// Need a mock server that supports HEAD too — helpers.mjs only wires GET by default.
// We write a small custom server here for the gate case.
function startGatedMockServer({ apiHealth, apiHealthStatus = 200, rootRedirect = null }) {
  return new Promise((ok) => {
    const server = createServer((req, res) => {
      if (req.url === "/api/health") {
        res.writeHead(apiHealthStatus, { "content-type": "application/json" });
        res.end(JSON.stringify(apiHealth));
      } else if (req.url === "/" && req.method === "HEAD" && rootRedirect) {
        res.writeHead(307, { location: rootRedirect });
        res.end();
      } else if (req.url === "/") {
        res.writeHead(200, { "content-type": "text/html" });
        res.end("<html>ok</html>");
      } else {
        res.writeHead(404); res.end();
      }
    });
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      ok({ url: `http://127.0.0.1:${port}`, close: () => new Promise((r) => server.close(r)) });
    });
  });
}

test("probe-cloud-admin.sh ok path -> build_id captured, no probe_errors", async () => {
  const server = await startGatedMockServer({ apiHealth: loadFixture("cloud-admin-health-ok") });
  try {
    const ts = "test-ca-" + Date.now();
    spawnSync("bash", ["scripts/fleet-probe/probe-cloud-admin.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_CLOUD_ADMIN_URL: server.url },
      encoding: "utf8", timeout: 30_000,
    });
    const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "cloud_admin.json"), "utf8"));
    assert.equal(m.probe_status, "ok");
    assert.equal(m.build_id, "cDyHRUgWTiqZTchmlEPgz");
    assert.equal(m.target_id, "cloud_admin");
    const { valid, errors } = validateAgainstSchema(m);
    assert.ok(valid, `schema: ${JSON.stringify(errors)}`);
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
  } finally { await server.close(); }
});

test("probe-cloud-admin.sh gate active -> scheduled_tasks entry + status still ok (or partial if pages_missing)", async () => {
  const server = await startGatedMockServer({
    apiHealth: loadFixture("cloud-admin-health-ok"),
    rootRedirect: "/coming-soon",
  });
  try {
    const ts = "test-ca-gate-" + Date.now();
    spawnSync("bash", ["scripts/fleet-probe/probe-cloud-admin.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_CLOUD_ADMIN_URL: server.url },
      encoding: "utf8", timeout: 30_000,
    });
    const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "cloud_admin.json"), "utf8"));
    const gateEntry = (m.scheduled_tasks || []).find((s) => s.name === "ADMIN_COMING_SOON_GATE");
    assert.ok(gateEntry, `expected ADMIN_COMING_SOON_GATE entry; got: ${JSON.stringify(m.scheduled_tasks)}`);
    assert.equal(gateEntry.state, "active");
    const { valid } = validateAgainstSchema(m);
    assert.ok(valid);
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
  } finally { await server.close(); }
});

test("probe-cloud-admin.sh pages_missing non-empty -> partial + pages_probe", async () => {
  const server = await startGatedMockServer({ apiHealth: loadFixture("cloud-admin-health-gated") });
  try {
    const ts = "test-ca-pm-" + Date.now();
    spawnSync("bash", ["scripts/fleet-probe/probe-cloud-admin.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_CLOUD_ADMIN_URL: server.url },
      encoding: "utf8", timeout: 30_000,
    });
    const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "cloud_admin.json"), "utf8"));
    assert.equal(m.probe_status, "partial");
    const err = (m.probe_errors || []).find((e) => e.sub_probe === "pages_probe");
    assert.ok(err);
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
  } finally { await server.close(); }
});

test("probe-cloud-admin.sh /api/health 500 -> probe_failed", async () => {
  const server = await startGatedMockServer({ apiHealth: {}, apiHealthStatus: 500 });
  try {
    const ts = "test-ca-500-" + Date.now();
    spawnSync("bash", ["scripts/fleet-probe/probe-cloud-admin.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_CLOUD_ADMIN_URL: server.url },
      encoding: "utf8", timeout: 30_000,
    });
    const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "cloud_admin.json"), "utf8"));
    assert.equal(m.probe_status, "probe_failed");
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
  } finally { await server.close(); }
});
```

`chmod +x scripts/fleet-probe/probe-cloud-admin.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-cloud-admin.sh &amp;&amp; node --check tests/fleet-probe/probe-cloud-admin.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-cloud-admin.sh` exits 0
    - `grep -c "admin\\.racingpoint\\.cloud" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "/api/health" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "ADMIN_COMING_SOON_GATE" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "coming-soon" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "pages_missing" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "pages_probe" scripts/fleet-probe/probe-cloud-admin.sh` >= 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-cloud-admin.sh` == 1
    - `npm run test:fleet-probe` exits 0 (all 4 probe-cloud-admin tests green: ok, gate, pages_missing partial, 500 probe_failed)
    - ASCII-only check passes
  </acceptance_criteria>
  <done>probe-cloud-admin.sh surfaces build_id, gate state (as scheduled_tasks entry, not error), pages_missing (as partial); 4 test cases cover all branches.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Create probe-cloud-rc.sh + fixture + unit test</name>
  <files>scripts/fleet-probe/probe-cloud-rc.sh, tests/fleet-probe/probe-cloud-rc.test.mjs, tests/fleet-probe/fixtures/cloud-rc-health-ok.json</files>
  <read_first>
    - scripts/fleet-probe/probe-cloud-admin.sh (just written; clone the structure, simpler since no gate detection)
    - schemas/examples/cloud_racecontrol.json (shape reference)
    - scripts/auto-detect.sh lines 286-301 (bono_health extraction pattern)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §7 (cloud_racecontrol failure matrix)
  </read_first>
  <behavior>
    - Mock /api/v1/health 200 with build_id -> probe_status ok, build_id captured, schema-valid
    - Mock /api/v1/health 500 -> probe_status probe_failed
    - Mock /api/v1/health 200 with malformed JSON -> probe_status partial + sub_probe health_parse
    - Mock /api/v1/health 200 without build_id field -> probe_status partial + sub_probe build_id
  </behavior>
  <action>
Create `tests/fleet-probe/fixtures/cloud-rc-health-ok.json`:

```json
{"build_id":"129a24f2","healthy":true,"uptime_secs":86400,"git_commit":"129a24f2"}
```

Create `scripts/fleet-probe/probe-cloud-rc.sh`:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-cloud-rc.sh — Phase 448 Plan 06
# Probes racingpoint.cloud (Bono VPS racecontrol :8080 via pm2) via HTTPS /api/v1/health.
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

if [ -z "${MANIFEST_TS:-}" ]; then
  echo "probe-cloud-rc: MANIFEST_TS not set" >&2
  exit 2
fi

CLOUD_URL="${PROBE_OVERRIDE_CLOUD_RC_URL:-https://racingpoint.cloud}"
START_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")

TARGET_ID="cloud_racecontrol"
HOSTNAME_VAL="racingpoint.cloud"
IP_VAL="45.11.110.250"
ROLE_VAL="cloud_racecontrol"

PROBE_ERRORS_JSON="[]"
CONNECT_ERR=0
SUBPROBE_ERR=0

append_error() {
  local sp="$1" err="$2"
  PROBE_ERRORS_JSON=$(SP="$sp" ERR="$err" PE="$PROBE_ERRORS_JSON" python3 -c '
import os,json
a=json.loads(os.environ["PE"]); a.append({"sub_probe":os.environ["SP"],"error":os.environ["ERR"]})
print(json.dumps(a))
')
}

HEALTH_RAW=$(curl -s --max-time 10 -w "\n%{http_code}" "$CLOUD_URL/api/v1/health" 2>/dev/null || echo "")
HEALTH_CODE=$(printf '%s' "$HEALTH_RAW" | tail -1)
HEALTH_BODY=$(printf '%s' "$HEALTH_RAW" | head -n -1)

BUILD_ID="null"

if [ -z "$HEALTH_CODE" ] || [ "$HEALTH_CODE" = "000" ]; then
  CONNECT_ERR=1
  append_error "connectivity" "cannot reach $CLOUD_URL/api/v1/health"
elif [ "$HEALTH_CODE" != "200" ]; then
  CONNECT_ERR=1
  append_error "health" "HTTP $HEALTH_CODE from /api/v1/health"
else
  if ! printf '%s' "$HEALTH_BODY" | jq . >/dev/null 2>&1; then
    SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
    append_error "health_parse" "cloud_racecontrol /api/v1/health returned non-JSON"
  else
    BID=$(printf '%s' "$HEALTH_BODY" | jq -r '.build_id // empty' 2>/dev/null || true)
    if [ -n "$BID" ]; then
      BUILD_ID=$(python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$BID")
    else
      SUBPROBE_ERR=$((SUBPROBE_ERR + 1))
      append_error "build_id" "/health response missing build_id"
    fi
  fi
fi

PROBED_AT=$(iso_ist_now)
END_EPOCH_MS=$(date +%s%3N 2>/dev/null || python3 -c "import time; print(int(time.time()*1000))")
DURATION_MS=$((END_EPOCH_MS - START_EPOCH_MS))
PROBE_STATUS=$(probe_status_from_errors "$CONNECT_ERR" "$SUBPROBE_ERR")

MANIFEST_JSON=$(python3 -c '
import json, os
m = {
  "schema_version":"1.0","target_id":"cloud_racecontrol","host":"racingpoint.cloud","ip":os.environ["IP_VAL"],"role":"cloud_racecontrol",
  "probed_at_ist":os.environ["PROBED_AT"],"probe_status":os.environ["PROBE_STATUS"],
  "binary_sha256":{},"build_id":json.loads(os.environ["BUILD_ID"]),
  "config_hash":{},"running_procs":[],"scheduled_tasks":[],"autostart_entries":[],
  "env_vars_hash":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "last_deploy_ts":None,
}
err=json.loads(os.environ["PROBE_ERRORS_JSON"])
if err: m["probe_errors"]=err
print(json.dumps(m))
' IP_VAL="$IP_VAL" PROBED_AT="$PROBED_AT" PROBE_STATUS="$PROBE_STATUS" BUILD_ID="$BUILD_ID" \
   PROBE_ERRORS_JSON="$PROBE_ERRORS_JSON")

write_manifest "$TARGET_ID" "$MANIFEST_JSON"

TOTAL_ERR=$((CONNECT_ERR + SUBPROBE_ERR))
printf '{"target_id":"%s","probe_status":"%s","duration_ms":%d,"errors_count":%d}\n' \
  "$TARGET_ID" "$PROBE_STATUS" "$DURATION_MS" "$TOTAL_ERR"
```

Create `tests/fleet-probe/probe-cloud-rc.test.mjs`:

```js
// tests/fleet-probe/probe-cloud-rc.test.mjs — Phase 448 Plan 06
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema, loadFixture } from "./helpers.mjs";

async function runProbeRC({ body, status = 200 }) {
  const routes = { "/api/v1/health": { status, body: typeof body === "string" ? body : JSON.stringify(body) } };
  const server = await startMockHttpServer(routes);
  try {
    const ts = "test-crc-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
    spawnSync("bash", ["scripts/fleet-probe/probe-cloud-rc.sh"], {
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_CLOUD_RC_URL: server.url },
      encoding: "utf8", timeout: 20_000,
    });
    const m = JSON.parse(readFileSync(resolve("state/fleet-manifest", ts, "cloud_racecontrol.json"), "utf8"));
    rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
    return m;
  } finally { await server.close(); }
}

test("probe-cloud-rc.sh ok -> build_id captured", async () => {
  const m = await runProbeRC({ body: loadFixture("cloud-rc-health-ok") });
  assert.equal(m.probe_status, "ok");
  assert.equal(m.build_id, "129a24f2");
  const { valid } = validateAgainstSchema(m);
  assert.ok(valid);
});

test("probe-cloud-rc.sh 500 -> probe_failed", async () => {
  const m = await runProbeRC({ body: "server error", status: 500 });
  assert.equal(m.probe_status, "probe_failed");
});

test("probe-cloud-rc.sh malformed JSON -> partial + health_parse", async () => {
  const m = await runProbeRC({ body: "not json at all", status: 200 });
  assert.equal(m.probe_status, "partial");
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "health_parse");
  assert.ok(err);
});

test("probe-cloud-rc.sh 200 but no build_id -> partial + build_id error", async () => {
  const m = await runProbeRC({ body: { healthy: true } });
  assert.equal(m.probe_status, "partial");
  assert.equal(m.build_id, null);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "build_id");
  assert.ok(err);
});
```

`chmod +x scripts/fleet-probe/probe-cloud-rc.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-cloud-rc.sh &amp;&amp; node --check tests/fleet-probe/probe-cloud-rc.test.mjs &amp;&amp; npm run test:fleet-probe</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-cloud-rc.sh` exits 0
    - `grep -c "racingpoint\\.cloud/api/v1/health" scripts/fleet-probe/probe-cloud-rc.sh` >= 1
    - `grep -c "source .*lib/probe-common.sh" scripts/fleet-probe/probe-cloud-rc.sh` == 1
    - `grep -c "health_parse" scripts/fleet-probe/probe-cloud-rc.sh` >= 1
    - `npm run test:fleet-probe` exits 0 (all 4 probe-cloud-rc tests green: ok, 500, malformed, missing build_id)
    - `npm run test:fleet-drift` still exits 0
    - ASCII-only check passes
  </acceptance_criteria>
  <done>probe-cloud-rc.sh covers all 4 failure modes; all 8 target probe scripts now exist in scripts/fleet-probe/; Wave 3 complete.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0
- `npm run test:fleet-drift` still exits 0
- `ls scripts/fleet-probe/*.sh | wc -l` == 9 (probe-all + probe-james/server/pod/pos/vps/relay/cloud-admin/cloud-rc + lib dir isn't counted)
- All 8 per-target probes produce schema-valid manifests on their ok paths
</verification>

<success_criteria>
- Cloud admin Coming Soon gate is properly encoded as a scheduled_tasks entry (intentional state, not error)
- Cloud racecontrol health probe supports 4 distinct result classes
- All 8 probe scripts exist after Plan 06; Plan 07 only needs to wire them into the orchestrator
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-06-SUMMARY.md` with:
- Files created
- Test results
- Count of probe scripts on disk (should be 8 per-target + 1 orchestrator skeleton + 1 lib file)
- Handoff to Plan 07 (orchestrator full wiring)
</output>
