---
phase: 448-per-target-probe-scripts
plan: 01
type: execute
wave: 0
depends_on: []
files_modified:
  - scripts/fleet-probe/lib/probe-common.sh
  - scripts/fleet-probe/validate-manifest-file.mjs
  - tests/fleet-probe/helpers.mjs
  - tests/fleet-probe/mock-ssh-responder.sh
  - tests/fleet-probe/mock-http-server.mjs
  - tests/fleet-probe/fixtures/server_23_ok.json
  - tests/fleet-probe/fixtures/pod_1_partial.json
  - tests/fleet-probe/fixtures/pos_130_probe_failed.json
  - tests/fleet-probe/schema-compat.test.mjs
  - package.json
autonomous: true
requirements: [PROBE-09]
gap_closure: false

must_haves:
  truths:
    - "Shared Bash helpers exist so every probe uses identical JSON/SHA256/IST/error-classification logic"
    - "A Node CLI validator wraps Phase 447 ajv so write_manifest can fail-fast when FLEET_PROBE_VALIDATE=1"
    - "Mock helpers (SSH shell responder + ephemeral HTTP server) exist so Waves 1-3 can unit-test probes offline"
    - "Three schema-valid fixture manifests cover ok/partial/probe_failed status classes"
    - "npm run test:fleet-probe executes node --test tests/fleet-probe/*.test.mjs and exits 0"
  artifacts:
    - path: "scripts/fleet-probe/lib/probe-common.sh"
      provides: "Shared Bash lib with json_escape, write_manifest, sha256_of, sha256_of_remote_file, iso_ist_now, probe_status_from_errors, env_names_hash, env_names_hash_remote, cmdline_hash"
      min_lines: 120
    - path: "scripts/fleet-probe/validate-manifest-file.mjs"
      provides: "CLI wrapper that validates a single manifest file path against schemas/fleet-manifest.schema.json"
      min_lines: 30
    - path: "tests/fleet-probe/helpers.mjs"
      provides: "ESM helpers for unit tests — startMockHttpServer(routes), makeMockSshEnv(scenarioFile), loadFixture(name), validateAgainstSchema(obj)"
      min_lines: 80
    - path: "tests/fleet-probe/mock-ssh-responder.sh"
      provides: "Stand-in for ssh binary; reads scenario file named via env var PROBE_SSH_SCENARIO and emits matching stdout + exit code"
      min_lines: 30
    - path: "tests/fleet-probe/mock-http-server.mjs"
      provides: "Tiny node:http ephemeral server (binds 127.0.0.1:0, returns map of path->{status, body}); exported as startMockHttpServer"
      min_lines: 30
    - path: "tests/fleet-probe/schema-compat.test.mjs"
      provides: "Assertion that every file in tests/fleet-probe/fixtures/ passes schema validation"
      min_lines: 25
    - path: "tests/fleet-probe/fixtures/server_23_ok.json"
      provides: "Baseline schema-valid ok-class fixture"
    - path: "tests/fleet-probe/fixtures/pod_1_partial.json"
      provides: "Baseline schema-valid partial-class fixture with probe_errors[]"
    - path: "tests/fleet-probe/fixtures/pos_130_probe_failed.json"
      provides: "Baseline schema-valid probe_failed-class fixture"
    - path: "package.json"
      provides: "scripts.test:fleet-probe entry"
      contains: "\"test:fleet-probe\""
  key_links:
    - from: "scripts/fleet-probe/lib/probe-common.sh::iso_ist_now"
      to: "scripts/ist-now.sh"
      via: "UTC_EPOCH+19800 pattern (MUST NOT use TZ=Asia/Kolkata)"
      pattern: "UTC_EPOCH \\+ 19800"
    - from: "scripts/fleet-probe/validate-manifest-file.mjs"
      to: "schemas/fleet-manifest.schema.json"
      via: "ajv 2020 import + compile"
      pattern: "ajv/dist/2020"
    - from: "scripts/fleet-probe/lib/probe-common.sh::write_manifest"
      to: "scripts/fleet-probe/validate-manifest-file.mjs"
      via: "FLEET_PROBE_VALIDATE=1 env gate"
      pattern: "FLEET_PROBE_VALIDATE"
    - from: "tests/fleet-probe/schema-compat.test.mjs"
      to: "tests/fleet-drift/validate-manifest.test.mjs"
      via: "reuses ajv 2020 import pattern verbatim"
      pattern: "Ajv2020"

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
Wave 0 scaffolding: ship the shared Bash library, manifest validator CLI, mock test helpers, fixtures, and npm script that ALL subsequent probe plans will build on. This is a pure tooling plan — no probe scripts yet, no live fleet reads.

Purpose: Give Waves 1-3 a validated, tested, offline-capable foundation. Every downstream probe plan sources `lib/probe-common.sh`, calls `write_manifest`, and uses `tests/fleet-probe/helpers.mjs` for its unit tests. Without this wave, each probe plan would re-invent the same helpers.

Output: 10 new files + 1 package.json edit. `npm run test:fleet-probe` passes (schema-compat test green on 3 fixtures).
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/PROJECT.md
@.planning/ROADMAP.md
@.planning/STATE.md
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md
@.planning/phases/448-per-target-probe-scripts/448-VALIDATION.md

# Phase 447 predecessor — schema + existing validator are the contract this plan wraps
@schemas/fleet-manifest.schema.json
@schemas/examples/server_23.json
@schemas/examples/pos_130.json
@schemas/examples/relay_james.json
@tests/fleet-drift/validate-manifest.test.mjs

# Reused helpers
@scripts/ist-now.sh

<interfaces>
<!-- Shared contract defined here, consumed by every plan 02-08 -->

**probe-common.sh contract (shell functions sourced via `source "$(dirname "$0")/lib/probe-common.sh"`)**
```bash
json_escape <string>                   # stdout: JSON-escaped string (no surrounding quotes)
write_manifest <target_id> <json>      # writes state/fleet-manifest/$MANIFEST_TS/<target_id>.json (pretty-printed, validated if FLEET_PROBE_VALIDATE=1)
sha256_of <filepath>                   # stdout: 64-char lowercase hex
sha256_of_stdin                        # stdin: bytes -> stdout: 64-char hex
sha256_of_remote_file <ssh_target> <path> [windows|linux]   # stdout: 64-char hex (uses certutil on windows, sha256sum on linux)
iso_ist_now                            # stdout: "2026-04-24T17:30:00+05:30"
probe_status_from_errors <connect_err_count> <subprobe_err_count>   # stdout: "ok" | "probe_failed" | "partial"
env_names_hash                         # stdout: sha256 of local sorted env NAMES
env_names_hash_remote <ssh_target>     # stdout: sha256 of remote sorted env NAMES (via ssh)
cmdline_hash <cmdline_string>          # stdout: sha256 of the cmdline
```

**validate-manifest-file.mjs contract (Node CLI)**
```
usage: node scripts/fleet-probe/validate-manifest-file.mjs <manifest.json>
  exit 0 = valid
  exit 1 = invalid (errors printed to stderr via ajv)
  exit 2 = usage error or file read failure
```

**tests/fleet-probe/helpers.mjs exports**
```js
export function startMockHttpServer(routes)   // routes: {"/path": {status, body}}; returns { url, close }
export function makeMockSshEnv(scenarioFile)  // returns env object: PROBE_SSH=<path to mock-ssh-responder.sh>, PROBE_SSH_SCENARIO=<scenarioFile>
export function loadFixture(name)             // reads tests/fleet-probe/fixtures/<name>.json
export function validateAgainstSchema(obj)    // returns { valid: bool, errors: [...] } using ajv2020
```

**env contract used by all probes**
- `MANIFEST_TS` (required for any write_manifest call)
- `FLEET_PROBE_VALIDATE` (optional, "1" enables per-write schema validation)
- `PROBE_SSH` (optional, overrides `ssh` binary path — used by tests)
- `PROBE_OVERRIDE_URL` (optional, overrides a probe's base URL — used by tests)
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create probe-common.sh shared library + validate-manifest-file.mjs CLI</name>
  <files>scripts/fleet-probe/lib/probe-common.sh, scripts/fleet-probe/validate-manifest-file.mjs</files>
  <read_first>
    - scripts/ist-now.sh (COPY the UTC_EPOCH+19800 pattern verbatim; TZ=Asia/Kolkata silently fails on Git Bash per CLAUDE.md)
    - tests/fleet-drift/validate-manifest.test.mjs lines 1-50 (COPY the ajv 2020 import + compile pattern verbatim)
    - schemas/fleet-manifest.schema.json (identify additionalProperties:true and the 15 required fields)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md §5 (Manifest Assembly Pattern) — write_manifest sketch
    - C:\Users\bono\racingpoint\racecontrol\CLAUDE.md lines about certutil SHA256 vs Get-FileHash
  </read_first>
  <behavior>
    - json_escape: input `a"b\c<newline>d` -> output `a\"b\\c\nd` (no surrounding quotes)
    - iso_ist_now: returns string matching regex `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+05:30$`
    - probe_status_from_errors 0 0 -> "ok"; 1 0 -> "probe_failed"; 0 1 -> "partial"; 1 1 -> "probe_failed" (connect-stage dominates)
    - sha256_of on a file of known content produces the correct 64-char hex
    - write_manifest with MANIFEST_TS=test-ts creates state/fleet-manifest/test-ts/<id>.json as valid JSON
    - write_manifest with FLEET_PROBE_VALIDATE=1 and malformed JSON returns exit 1 and removes the .tmp file
    - validate-manifest-file.mjs with a schema-valid file exits 0
    - validate-manifest-file.mjs with a schema-invalid file exits 1 and prints ajv errors to stderr
  </behavior>
  <action>
Create `scripts/fleet-probe/lib/probe-common.sh` with shebang `#!/bin/bash` and `set -eo pipefail`. ASCII-only (no em-dashes, no smart quotes — standing rule `feedback_ascii_only_script_constraint.md`). Source-only (no `exit` calls; `return 1` for errors).

Implement EXACTLY these functions with this contract:

```bash
# json_escape STRING
# Escapes backslash, double-quote, control chars. Uses python3 for correctness.
json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.argv[1])[1:-1])' "$1"
}

# iso_ist_now
# Returns ISO-8601 with +05:30 suffix. Uses UTC_EPOCH+19800 pattern (NEVER TZ=Asia/Kolkata).
iso_ist_now() {
  local utc_epoch ist_epoch
  utc_epoch=$(date -u +%s)
  ist_epoch=$((utc_epoch + 19800))
  date -u -d "@$ist_epoch" '+%Y-%m-%dT%H:%M:%S+05:30' 2>/dev/null || \
    python3 -c "from datetime import datetime; print(datetime.utcfromtimestamp($ist_epoch).strftime('%Y-%m-%dT%H:%M:%S+05:30'))"
}

# sha256_of FILEPATH
# Returns lowercase 64-char hex.
sha256_of() {
  if [ -f "$1" ]; then
    sha256sum "$1" | awk '{print $1}'
  else
    echo "sha256_of: file not found: $1" >&2
    return 1
  fi
}

# sha256_of_stdin — reads stdin, emits sha256
sha256_of_stdin() {
  sha256sum | awk '{print $1}'
}

# sha256_of_remote_file SSH_TARGET REMOTE_PATH [OS=windows|linux]
# Returns lowercase 64-char hex; uses certutil on Windows, sha256sum on Linux.
sha256_of_remote_file() {
  local ssh_target="$1" remote_path="$2" os="${3:-windows}"
  if [ "$os" = "windows" ]; then
    ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "certutil -hashfile \"$remote_path\" SHA256 2>nul" 2>/dev/null \
      | tr -d '\r' | awk 'NR==2 {gsub(/ /,""); print tolower($0); exit}'
  else
    ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "sha256sum \"$remote_path\"" 2>/dev/null | awk '{print $1}'
  fi
}

# probe_status_from_errors CONNECT_ERR_COUNT SUBPROBE_ERR_COUNT
# "ok" | "probe_failed" | "partial"; connect-stage dominates.
probe_status_from_errors() {
  local connect_err="${1:-0}" subprobe_err="${2:-0}"
  if [ "$connect_err" -gt 0 ]; then
    echo "probe_failed"
  elif [ "$subprobe_err" -gt 0 ]; then
    echo "partial"
  else
    echo "ok"
  fi
}

# env_names_hash — hash of sorted local env NAMES (security boundary: names only).
env_names_hash() {
  env | awk -F= '{print $1}' | sort | sha256sum | awk '{print $1}'
}

# env_names_hash_remote SSH_TARGET — same over SSH (Windows `set /A` isn't usable; use `set` parsing).
# For Windows over SSH: `set` returns NAME=VALUE lines; awk -F= '{print $1}' strips values.
env_names_hash_remote() {
  local ssh_target="$1"
  ssh -o ConnectTimeout=15 -o BatchMode=yes "$ssh_target" "set" 2>/dev/null \
    | tr -d '\r' | awk -F= 'NF>=2 {print $1}' | sort | sha256sum | awk '{print $1}'
}

# cmdline_hash CMDLINE_STRING — sha256 of full cmdline.
cmdline_hash() {
  printf '%s' "$1" | sha256sum | awk '{print $1}'
}

# write_manifest TARGET_ID MANIFEST_JSON
# MANIFEST_TS must be exported. Writes state/fleet-manifest/$MANIFEST_TS/<target_id>.json.
# If FLEET_PROBE_VALIDATE=1, validates via validate-manifest-file.mjs; returns 1 on schema failure.
write_manifest() {
  local target_id="$1" manifest_json="$2"
  if [ -z "${MANIFEST_TS:-}" ]; then
    echo "write_manifest: MANIFEST_TS env var not set" >&2
    return 2
  fi
  local out_dir="state/fleet-manifest/$MANIFEST_TS"
  mkdir -p "$out_dir"
  local out_file="$out_dir/$target_id.json"
  local tmp_file="$out_file.tmp"

  if ! echo "$manifest_json" | python3 -m json.tool > "$tmp_file" 2>/dev/null; then
    echo "write_manifest: $target_id manifest is not valid JSON" >&2
    rm -f "$tmp_file"
    return 1
  fi

  if [ "${FLEET_PROBE_VALIDATE:-0}" = "1" ]; then
    if ! node scripts/fleet-probe/validate-manifest-file.mjs "$tmp_file" >&2; then
      echo "write_manifest: $target_id manifest failed schema validation" >&2
      rm -f "$tmp_file"
      return 1
    fi
  fi

  mv "$tmp_file" "$out_file"
  return 0
}
```

Create `scripts/fleet-probe/validate-manifest-file.mjs` using the EXACT ajv 2020 import pattern from `tests/fleet-drift/validate-manifest.test.mjs`:

```js
#!/usr/bin/env node
// Phase 448 Plan 01 — CLI wrapper around Phase 447 ajv validator.
// Usage: node scripts/fleet-probe/validate-manifest-file.mjs <manifest.json>
// Exits: 0 valid | 1 invalid | 2 usage/IO error
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");
const SCHEMA_PATH = resolve(REPO_ROOT, "schemas", "fleet-manifest.schema.json");

const manifestPath = process.argv[2];
if (!manifestPath) {
  console.error("Usage: validate-manifest-file.mjs <manifest.json>");
  process.exit(2);
}

let schema, manifest;
try {
  schema = JSON.parse(readFileSync(SCHEMA_PATH, "utf8"));
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  console.error(`IO/parse error: ${e.message}`);
  process.exit(2);
}

const ajv = new Ajv2020({ allErrors: true, strict: false });
addFormats(ajv);
const validate = ajv.compile(schema);
if (validate(manifest)) {
  process.exit(0);
}
for (const err of validate.errors) {
  console.error(`  ${err.instancePath || "/"}: ${err.message}`);
}
process.exit(1);
```

`chmod +x scripts/fleet-probe/lib/probe-common.sh scripts/fleet-probe/validate-manifest-file.mjs`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/lib/probe-common.sh &amp;&amp; node --check scripts/fleet-probe/validate-manifest-file.mjs &amp;&amp; bash -c 'source scripts/fleet-probe/lib/probe-common.sh &amp;&amp; [ "$(probe_status_from_errors 0 0)" = "ok" ] &amp;&amp; [ "$(probe_status_from_errors 1 0)" = "probe_failed" ] &amp;&amp; [ "$(probe_status_from_errors 0 1)" = "partial" ] &amp;&amp; iso_ist_now | grep -qE "^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\+05:30$"' &amp;&amp; node scripts/fleet-probe/validate-manifest-file.mjs schemas/examples/server_23.json</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/lib/probe-common.sh` exits 0
    - `node --check scripts/fleet-probe/validate-manifest-file.mjs` exits 0
    - `grep -c "UTC_EPOCH + 19800" scripts/fleet-probe/lib/probe-common.sh` >= 1
    - `grep -c "TZ=Asia/Kolkata" scripts/fleet-probe/lib/probe-common.sh` == 0 (banned pattern)
    - `grep -cE "^(json_escape|write_manifest|sha256_of|sha256_of_stdin|sha256_of_remote_file|iso_ist_now|probe_status_from_errors|env_names_hash|env_names_hash_remote|cmdline_hash)\(\) \{" scripts/fleet-probe/lib/probe-common.sh` == 10
    - `grep -c "FLEET_PROBE_VALIDATE" scripts/fleet-probe/lib/probe-common.sh` >= 1
    - `grep -c "ajv/dist/2020" scripts/fleet-probe/validate-manifest-file.mjs` == 1
    - `node scripts/fleet-probe/validate-manifest-file.mjs schemas/examples/server_23.json` exits 0
    - `node scripts/fleet-probe/validate-manifest-file.mjs` (no arg) exits 2
    - probe-common.sh is ASCII-only: `python3 -c "open('scripts/fleet-probe/lib/probe-common.sh','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>Shared library + validator CLI exist, bash -n clean, node --check clean, ajv validator round-trips a known-good manifest, all 10 required functions defined with exact signatures above.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Create test helpers (mock SSH/HTTP) + 3 fixtures + schema-compat test + package.json script</name>
  <files>tests/fleet-probe/helpers.mjs, tests/fleet-probe/mock-ssh-responder.sh, tests/fleet-probe/mock-http-server.mjs, tests/fleet-probe/fixtures/server_23_ok.json, tests/fleet-probe/fixtures/pod_1_partial.json, tests/fleet-probe/fixtures/pos_130_probe_failed.json, tests/fleet-probe/schema-compat.test.mjs, package.json</files>
  <read_first>
    - package.json (full file — to locate the `"scripts"` block and insert new entry without regressing existing keys)
    - tests/fleet-drift/validate-manifest.test.mjs (full file — clone the Ajv2020 import + compile idiom)
    - schemas/examples/server_23.json (copy shape for ok-class fixture; change values to distinguish from example)
    - schemas/examples/pos_130.json (copy shape for partial-class fixture reference)
    - schemas/fleet-manifest.schema.json (confirm the 15 required fields + probe_status enum)
  </read_first>
  <behavior>
    - `npm run test:fleet-probe` runs node --test on tests/fleet-probe/*.test.mjs and exits 0
    - schema-compat.test.mjs validates all 3 fixtures against schemas/fleet-manifest.schema.json — all pass
    - mock-ssh-responder.sh with PROBE_SSH_SCENARIO=/path/to/scenario.txt outputs the scenario file's first line to stdout and exits with the code in the scenario file's second line
    - mock-http-server.mjs startMockHttpServer({"/health": {status: 200, body: '{"ok":true}'}}) starts a server, returns a url, and GET url/health returns 200 with that body
    - loadFixture("server_23_ok") returns the parsed JSON object
    - validateAgainstSchema({target_id: "x"}) returns {valid: false, errors: [...]}
    - validateAgainstSchema(loadFixture("server_23_ok")) returns {valid: true, errors: null}
  </behavior>
  <action>
Create `tests/fleet-probe/helpers.mjs` as an ESM module exporting the 4 functions from the interface block above. Pattern:

```js
// tests/fleet-probe/helpers.mjs — Phase 448 Plan 01 test helpers
import { createServer } from "node:http";
import { readFileSync, chmodSync } from "node:fs";
import { dirname, resolve, join } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");
const SCHEMA_PATH = resolve(REPO_ROOT, "schemas", "fleet-manifest.schema.json");
const FIXTURES_DIR = resolve(__dirname, "fixtures");
const MOCK_SSH_PATH = resolve(__dirname, "mock-ssh-responder.sh");

let cachedValidator = null;
function getValidator() {
  if (cachedValidator) return cachedValidator;
  const schema = JSON.parse(readFileSync(SCHEMA_PATH, "utf8"));
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  cachedValidator = ajv.compile(schema);
  return cachedValidator;
}

export function startMockHttpServer(routes) {
  const server = createServer((req, res) => {
    const match = routes[req.url] || routes[req.url.split("?")[0]];
    if (match) {
      res.writeHead(match.status, { "content-type": match.contentType || "application/json" });
      res.end(match.body || "");
    } else {
      res.writeHead(404, { "content-type": "text/plain" });
      res.end("not found");
    }
  });
  return new Promise((ok) => {
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      ok({
        url: `http://127.0.0.1:${port}`,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}

export function makeMockSshEnv(scenarioFile) {
  return {
    PROBE_SSH: MOCK_SSH_PATH,
    PROBE_SSH_SCENARIO: scenarioFile,
  };
}

export function loadFixture(name) {
  return JSON.parse(readFileSync(join(FIXTURES_DIR, `${name}.json`), "utf8"));
}

export function validateAgainstSchema(obj) {
  const validate = getValidator();
  const valid = validate(obj);
  return { valid, errors: valid ? null : validate.errors };
}
```

Create `tests/fleet-probe/mock-ssh-responder.sh` — reads `$PROBE_SSH_SCENARIO` file; line 1 = stdout; line 2 = exit code:
```bash
#!/bin/bash
set -eo pipefail
if [ -z "${PROBE_SSH_SCENARIO:-}" ] || [ ! -f "$PROBE_SSH_SCENARIO" ]; then
  echo "mock-ssh-responder: PROBE_SSH_SCENARIO not set or file missing" >&2
  exit 2
fi
# stdout = all lines up to first "---" separator or all but last line
# exit code = last non-empty line
awk '/^---EXIT---$/ { exit_section=1; next } !exit_section' "$PROBE_SSH_SCENARIO"
EXIT_CODE=$(awk '/^---EXIT---$/ { exit_section=1; next } exit_section && NF' "$PROBE_SSH_SCENARIO" | head -1)
exit "${EXIT_CODE:-0}"
```
Then `chmod +x tests/fleet-probe/mock-ssh-responder.sh`.

Create `tests/fleet-probe/mock-http-server.mjs` — thin re-export of helpers.mjs startMockHttpServer for direct runtime use:
```js
// tests/fleet-probe/mock-http-server.mjs — CLI wrapper for shell-level mocks
export { startMockHttpServer } from "./helpers.mjs";
```

Create 3 fixtures under `tests/fleet-probe/fixtures/`:

**server_23_ok.json** (MUST validate — 15 required fields, probe_status=ok):
```json
{
  "schema_version": "1.0",
  "target_id": "server_23",
  "host": "Racing-Point-Server",
  "ip": "192.168.31.23",
  "role": "server",
  "probed_at_ist": "2026-04-24T18:00:00+05:30",
  "probe_status": "ok",
  "binary_sha256": { "racecontrol.exe": "0000000000000000000000000000000000000000000000000000000000000000" },
  "build_id": "3c2a1b48",
  "config_hash": {
    "racecontrol.toml.server_live": "1111111111111111111111111111111111111111111111111111111111111111",
    "racecontrol.toml.james_proxy": "2222222222222222222222222222222222222222222222222222222222222222",
    "racecontrol.toml.git_head":    "3333333333333333333333333333333333333333333333333333333333333333"
  },
  "running_procs": [{"name": "racecontrol.exe", "pid": 4567, "cmdline_hash": "4444444444444444444444444444444444444444444444444444444444444444"}],
  "scheduled_tasks": [{"name": "StartRCDirect", "state": "Ready"}],
  "autostart_entries": [{"source": "HKLM_Run", "key": "RaceControl", "value": "C:\\RacingPoint\\start-racecontrol.bat"}],
  "env_vars_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "last_deploy_ts": "2026-04-23T18:15:00+05:30"
}
```

**pod_1_partial.json** (MUST validate — probe_status=partial with probe_errors[]):
```json
{
  "schema_version": "1.0",
  "target_id": "pod_1",
  "host": "RCPOD-1",
  "ip": "192.168.31.89",
  "role": "pod",
  "probed_at_ist": "2026-04-24T18:00:00+05:30",
  "probe_status": "partial",
  "probe_errors": [{"sub_probe": "debug_endpoint", "error": "404 -- rc-agent pre-debug version"}],
  "binary_sha256": { "rc-agent.exe": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" },
  "build_id": null,
  "config_hash": { "rc-agent.toml": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" },
  "running_procs": [{"name": "rc-agent.exe", "pid": 1234, "cmdline_hash": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"}],
  "scheduled_tasks": [],
  "autostart_entries": [{"source": "HKLM_Run", "key": "RCAgent", "value": "C:\\RacingPoint\\start-rcagent.bat"}],
  "env_vars_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "last_deploy_ts": "2026-04-20T12:00:00+05:30"
}
```

**pos_130_probe_failed.json** (MUST validate — probe_status=probe_failed, empty data):
```json
{
  "schema_version": "1.0",
  "target_id": "pos_130",
  "host": "POS1",
  "ip": "192.168.31.130",
  "role": "pos",
  "probed_at_ist": "2026-04-24T18:00:00+05:30",
  "probe_status": "probe_failed",
  "probe_errors": [{"sub_probe": "ssh_connect", "error": "timeout after 15s", "access_gap": "POS_SSH_DOWN"}],
  "binary_sha256": {},
  "build_id": null,
  "config_hash": {},
  "running_procs": [],
  "scheduled_tasks": [],
  "autostart_entries": [],
  "env_vars_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "last_deploy_ts": null
}
```

Create `tests/fleet-probe/schema-compat.test.mjs`:
```js
// tests/fleet-probe/schema-compat.test.mjs — Phase 448 Plan 01
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { loadFixture, validateAgainstSchema } from "./helpers.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const FIXTURES_DIR = resolve(__dirname, "fixtures");

const fixtureNames = readdirSync(FIXTURES_DIR)
  .filter((f) => f.endsWith(".json"))
  .map((f) => f.replace(/\.json$/, ""));

test("all fixtures are schema-valid", () => {
  assert.ok(fixtureNames.length >= 3, `expected >=3 fixtures, got ${fixtureNames.length}`);
  for (const name of fixtureNames) {
    const obj = loadFixture(name);
    const { valid, errors } = validateAgainstSchema(obj);
    assert.ok(valid, `${name} failed: ${JSON.stringify(errors)}`);
  }
});

test("validateAgainstSchema catches missing required fields", () => {
  const { valid, errors } = validateAgainstSchema({ target_id: "bogus" });
  assert.equal(valid, false);
  assert.ok(errors && errors.length > 0);
});
```

Edit `package.json` `"scripts"` object: add `"test:fleet-probe": "node --test tests/fleet-probe/*.test.mjs"` after the existing `"test:fleet-drift"` entry. Do NOT reorder or remove any other keys.
  </action>
  <verify>
    <automated>bash -n tests/fleet-probe/mock-ssh-responder.sh &amp;&amp; node --check tests/fleet-probe/helpers.mjs &amp;&amp; node --check tests/fleet-probe/mock-http-server.mjs &amp;&amp; node --check tests/fleet-probe/schema-compat.test.mjs &amp;&amp; npm run test:fleet-probe &amp;&amp; npm run test:fleet-drift</automated>
  </verify>
  <acceptance_criteria>
    - `grep -c "\"test:fleet-probe\"" package.json` == 1
    - `grep -c "\"test:fleet-drift\"" package.json` == 1 (preserved)
    - `bash -n tests/fleet-probe/mock-ssh-responder.sh` exits 0
    - `node --check tests/fleet-probe/helpers.mjs` exits 0
    - `node --check tests/fleet-probe/schema-compat.test.mjs` exits 0
    - `npm run test:fleet-probe` exits 0 (schema-compat test green on >=3 fixtures)
    - `npm run test:fleet-drift` still exits 0 (Phase 447 regression gate)
    - `node scripts/fleet-probe/validate-manifest-file.mjs tests/fleet-probe/fixtures/server_23_ok.json` exits 0
    - `node scripts/fleet-probe/validate-manifest-file.mjs tests/fleet-probe/fixtures/pod_1_partial.json` exits 0
    - `node scripts/fleet-probe/validate-manifest-file.mjs tests/fleet-probe/fixtures/pos_130_probe_failed.json` exits 0
    - `ls tests/fleet-probe/fixtures/*.json | wc -l` >= 3
    - `grep -c "export function startMockHttpServer" tests/fleet-probe/helpers.mjs` == 1
    - `grep -c "export function makeMockSshEnv" tests/fleet-probe/helpers.mjs` == 1
    - `grep -c "export function loadFixture" tests/fleet-probe/helpers.mjs` == 1
    - `grep -c "export function validateAgainstSchema" tests/fleet-probe/helpers.mjs` == 1
  </acceptance_criteria>
  <done>Test harness files + 3 schema-valid fixtures + package.json entry all in place; both npm test scripts exit 0; ajv CLI validates each fixture.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0
- `npm run test:fleet-drift` still exits 0 (no regression)
- `bash -c 'source scripts/fleet-probe/lib/probe-common.sh && iso_ist_now'` emits a valid IST timestamp
- `node scripts/fleet-probe/validate-manifest-file.mjs schemas/examples/server_23.json` exits 0
- ASCII-only check on every new .sh/.mjs: `python3 -c "for f in open('/dev/stdin').read().split(): open(f,'rb').read().decode('ascii')" <<< $(ls scripts/fleet-probe/lib/probe-common.sh tests/fleet-probe/mock-ssh-responder.sh)` does not raise
</verification>

<success_criteria>
- Every Wave 0 file listed in VALIDATION.md §Wave 0 Requirements exists on disk with non-trivial content
- `npm run test:fleet-probe` runs ≥1 test with ≥3 assertions and exits 0
- `write_manifest` helper can be sourced + invoked without errors (smoke test in verify)
- Phase 447 regression gate (`test:fleet-drift`) still green
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-01-SUMMARY.md` with:
- Commits created
- Files added (10 new + 1 modified)
- Test results (`npm run test:fleet-probe`, `npm run test:fleet-drift`)
- Any deviations from plan (with reasons)
- Open handoff items for Plan 02 (probe-james.sh will be first consumer)
</output>
