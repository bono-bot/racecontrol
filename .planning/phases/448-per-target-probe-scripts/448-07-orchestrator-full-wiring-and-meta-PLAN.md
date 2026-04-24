---
phase: 448-per-target-probe-scripts
plan: 07
type: execute
wave: 4
depends_on: ["448-02", "448-03", "448-04", "448-05", "448-06"]
files_modified:
  - scripts/fleet-probe/probe-all.sh
  - scripts/fleet-probe/build-meta-index.py
  - tests/fleet-probe/smoke-orchestrator.sh
  - tests/fleet-probe/orchestrator-dry-run.test.mjs
autonomous: true
requirements: [PROBE-09]
gap_closure: false

must_haves:
  truths:
    - "bash scripts/fleet-probe/probe-all.sh emits exactly 15 per-target manifests + _meta.json in one state/fleet-manifest/<ts>/ directory"
    - "Pod probes 1-8 run in parallel via & + wait; other 7 probes run sequential to avoid auth rate-limits"
    - "Orchestrator completes even when individual probes return probe_failed — exit 0"
    - "--canary runs only server_23 + pod_8 probes (2 manifests + _meta.json)"
    - "--dry-run still enumerates 15 targets without making any network calls (Plan 02 behavior preserved)"
    - "_meta.json shape matches schemas/examples/_meta.json exactly — status_summary counts by ok/partial/probe_failed; targets[] ordered per RESEARCH section 3"
  artifacts:
    - path: "scripts/fleet-probe/probe-all.sh"
      provides: "FULL wiring: invokes each probe with MANIFEST_TS export, parallel pod fanout, sequential cluster, _meta.json assembly, --canary, --dry-run"
      min_lines: 130
    - path: "scripts/fleet-probe/build-meta-index.py"
      provides: "Reads manifest dir and writes SCHEMA-03 _meta.json with ordered targets[] + status_summary"
      min_lines: 60
    - path: "tests/fleet-probe/smoke-orchestrator.sh"
      provides: "Bash smoke: runs --dry-run (asserts 15 lines) + runs --canary with mocks (asserts 2 manifests + _meta.json)"
      min_lines: 40
    - path: "tests/fleet-probe/orchestrator-dry-run.test.mjs"
      provides: "Node test: --dry-run enumerates 15 targets, --help works, default no-flag produces _meta.json with status_summary"
      min_lines: 30
  key_links:
    - from: "scripts/fleet-probe/probe-all.sh"
      to: "each probe-*.sh via MANIFEST_TS export"
      via: "export MANIFEST_TS + bash invocation"
      pattern: "export MANIFEST_TS"
    - from: "scripts/fleet-probe/probe-all.sh"
      to: "build-meta-index.py"
      via: "python3 invocation after all probes return"
      pattern: "build-meta-index\\.py"
    - from: "scripts/fleet-probe/build-meta-index.py"
      to: "schemas/examples/_meta.json"
      via: "shape replication (probe_run_id, targets[], status_summary)"
      pattern: "status_summary"

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
Wave 4 wiring: Replace the Plan 02 orchestrator skeleton's stub with full probe invocation. Pod probes 1-8 fanned out in parallel; 7 sequential probes for auth-rate-limit safety. Add build-meta-index.py that assembles the SCHEMA-03 _meta.json summary index. Ship an integration smoke test that runs --canary end-to-end.

Purpose: This is the single invocation staff run every day (Phase 454 scheduled) and the execution gate for Phase 449. Must be idempotent (same MANIFEST_TS overwrites cleanly) and must always produce a complete manifest set.

Output: Full-wiring orchestrator + _meta builder + integration smoke.
</objective>

<execution_context>
@$HOME/.claude/get-shit-done/workflows/execute-plan.md
@$HOME/.claude/get-shit-done/templates/summary.md
</execution_context>

<context>
@.planning/phases/448-per-target-probe-scripts/448-CONTEXT.md
@.planning/phases/448-per-target-probe-scripts/448-RESEARCH.md
@.planning/phases/448-per-target-probe-scripts/448-VALIDATION.md

# Plans 01-06 artifacts wired in this plan
@scripts/fleet-probe/lib/probe-common.sh
@scripts/fleet-probe/probe-all.sh
@scripts/fleet-probe/probe-james.sh
@scripts/fleet-probe/probe-server.sh
@scripts/fleet-probe/probe-pod.sh
@scripts/fleet-probe/probe-pos.sh
@scripts/fleet-probe/probe-vps.sh
@scripts/fleet-probe/probe-relay.sh
@scripts/fleet-probe/probe-cloud-admin.sh
@scripts/fleet-probe/probe-cloud-rc.sh

# Shape reference for _meta.json
@schemas/examples/_meta.json

<interfaces>
**probe-all.sh full contract (Plan 07 replaces the skeleton)**

Usage: `bash scripts/fleet-probe/probe-all.sh [--dry-run|--canary]`

Flags:
- `--dry-run`   enumerate 15 targets, no network (Plan 02 preserved)
- `--canary`    run ONLY server_23 + pod_8 (+ _meta.json covering just those 2)
- (no flag)     run all 15 probes

Exit: 0 always (probe_failed is a row in _meta.json, not an orchestrator error)

Env propagated to children:
- MANIFEST_TS (generated here; exported)
- FLEET_PROBE_VALIDATE (optional)
- SENTRY_KEY, COMMS_PSK, STAFF_JWT (passed through from invoking shell)

Side effect: state/fleet-manifest/$MANIFEST_TS/ populated with 15 JSON files + _meta.json

**Sequential order (LOCKED per CONTEXT.md):**
1. server_23
2. pos_130
3. james_27
4. bono_vps
5. cloud_admin
6. cloud_racecontrol
7. relay_james
8-15. pods 1-8 (parallel via & + wait)

**build-meta-index.py contract:**
- Reads each target json in a manifest dir (skipping _meta.json itself)
- Writes _meta.json with shape from schemas/examples/_meta.json
- Ordered targets[] per RESEARCH section 3
- status_summary.{ok,partial,probe_failed}
- Exit 0 success / 2 missing dir / 1 parse errors

**--canary subset:** `["server_23", "pod_8"]` only — _meta.json still written with target_count=2.
</interfaces>
</context>

<tasks>

<task type="auto" tdd="true">
  <name>Task 1: Create build-meta-index.py + full probe-all.sh wiring</name>
  <files>scripts/fleet-probe/build-meta-index.py, scripts/fleet-probe/probe-all.sh</files>
  <read_first>
    - scripts/fleet-probe/probe-all.sh (current Plan 02 skeleton — PRESERVE --dry-run verbatim)
    - schemas/examples/_meta.json (exact shape; field names; ordering)
    - .planning/phases/448-per-target-probe-scripts/448-RESEARCH.md section 3 (target ordering table — 15 entries)
  </read_first>
  <behavior>
    - `bash scripts/fleet-probe/probe-all.sh --dry-run` still emits exactly 15 `target=...` lines (Plan 02 contract preserved)
    - `bash scripts/fleet-probe/probe-all.sh --canary` creates state/fleet-manifest/$MANIFEST_TS/server_23.json + pod_8.json + _meta.json (target_count=2)
    - `bash scripts/fleet-probe/probe-all.sh` (full) creates 15 per-target manifests + _meta.json
    - _meta.json status_summary is computed from the manifests actually written
    - Orchestrator always exits 0 (even if every probe returns probe_failed)
    - --help prints usage and exits 0
    - Full run invokes probes in the documented order (server, pos, james, vps, cloud_admin, cloud_racecontrol, relay_james, pods-parallel)
  </behavior>
  <action>
Create `scripts/fleet-probe/build-meta-index.py` (ASCII only, no emojis):

```python
#!/usr/bin/env python3
# scripts/fleet-probe/build-meta-index.py -- Phase 448 Plan 07
# Reads per-target manifests in a directory, writes _meta.json (SCHEMA-03 shape).
# Usage: python3 scripts/fleet-probe/build-meta-index.py <manifest_dir> [--orchestrator-start-epoch N]
import sys
import json
import time
import datetime
from pathlib import Path

TARGET_ORDER = [
    "server_23",
    "pod_1", "pod_2", "pod_3", "pod_4", "pod_5", "pod_6", "pod_7", "pod_8",
    "pos_130", "james_27", "bono_vps",
    "cloud_admin", "cloud_racecontrol", "relay_james",
]


def iso_ist_now():
    utc_epoch = time.time()
    ist_epoch = utc_epoch + 19800
    return datetime.datetime.utcfromtimestamp(ist_epoch).strftime("%Y-%m-%dT%H:%M:%S+05:30")


def main():
    if len(sys.argv) < 2:
        print("Usage: build-meta-index.py <manifest_dir> [--orchestrator-start-epoch N]", file=sys.stderr)
        sys.exit(2)
    mdir = Path(sys.argv[1])
    if not mdir.is_dir():
        print("manifest dir not found: {}".format(mdir), file=sys.stderr)
        sys.exit(2)

    start_epoch = None
    for i, arg in enumerate(sys.argv[2:], 2):
        if arg == "--orchestrator-start-epoch" and i + 1 < len(sys.argv):
            try:
                start_epoch = float(sys.argv[i + 1])
            except Exception:
                pass

    per_target = {}
    for f in sorted(mdir.glob("*.json")):
        if f.name == "_meta.json":
            continue
        try:
            with open(f, "r", encoding="utf-8") as fh:
                m = json.load(fh)
        except Exception as e:
            print("skip unreadable manifest {}: {}".format(f, e), file=sys.stderr)
            continue
        tid = m.get("target_id") or f.stem
        per_target[tid] = {
            "target_id": tid,
            "role": m.get("role", "unknown"),
            "probe_status": m.get("probe_status", "unknown"),
            "manifest_file": f.name,
        }

    ordered = [per_target[t] for t in TARGET_ORDER if t in per_target]
    for tid in sorted(per_target.keys()):
        if tid not in TARGET_ORDER:
            ordered.append(per_target[tid])

    summary = {"ok": 0, "partial": 0, "probe_failed": 0}
    for entry in ordered:
        st = entry["probe_status"]
        if st in summary:
            summary[st] += 1

    now_ist = iso_ist_now()
    duration = round(time.time() - start_epoch, 2) if start_epoch else 0.0

    meta = {
        "schema_version": "1.0",
        "probe_run_id": now_ist,
        "probed_at_ist": now_ist,
        "probe_duration_sec": duration,
        "orchestrator": "scripts/fleet-probe/probe-all.sh",
        "orchestrator_version": "phase-448-v1",
        "target_count": len(ordered),
        "targets": ordered,
        "status_summary": summary,
    }

    out = mdir / "_meta.json"
    with open(out, "w", encoding="utf-8") as fh:
        json.dump(meta, fh, indent=2)
    print("wrote {} target_count={} summary={}".format(out, len(ordered), summary))
    sys.exit(0)


if __name__ == "__main__":
    main()
```

Overwrite `scripts/fleet-probe/probe-all.sh` — preserve the --dry-run block from Plan 02, add full wiring + --canary:

```bash
#!/bin/bash
# scripts/fleet-probe/probe-all.sh -- Phase 448 Plan 07 -- FULL wiring.
# Usage:
#   bash scripts/fleet-probe/probe-all.sh --dry-run
#   bash scripts/fleet-probe/probe-all.sh --canary
#   bash scripts/fleet-probe/probe-all.sh
set -eo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$REPO_ROOT"

# shellcheck source=lib/probe-common.sh
source "$SCRIPT_DIR/lib/probe-common.sh"

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
    --canary)  MODE="canary"  ;;
    --help|-h)
      echo "Usage: probe-all.sh [--dry-run|--canary]"
      echo "  --dry-run  enumerate targets, no network calls"
      echo "  --canary   run server_23 + pod_8 only"
      echo "  (no flag)  run all 15 probes"
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

export MANIFEST_TS="${MANIFEST_TS:-$(date -u +%Y-%m-%dT%H%M%SZ)}"
MANIFEST_DIR="state/fleet-manifest/$MANIFEST_TS"
mkdir -p "$MANIFEST_DIR"
START_EPOCH=$(date +%s)

echo "probe-all: MANIFEST_TS=$MANIFEST_TS mode=$MODE" >&2

run_probe() {
  local script="$1"; shift
  echo "  -> $script $*" >&2
  bash "$script" "$@" || true
}

if [ "$MODE" = "canary" ]; then
  run_probe "$SCRIPT_DIR/probe-server.sh"
  run_probe "$SCRIPT_DIR/probe-pod.sh" 8
else
  run_probe "$SCRIPT_DIR/probe-server.sh"
  run_probe "$SCRIPT_DIR/probe-pos.sh"
  run_probe "$SCRIPT_DIR/probe-james.sh"
  run_probe "$SCRIPT_DIR/probe-vps.sh"
  run_probe "$SCRIPT_DIR/probe-cloud-admin.sh"
  run_probe "$SCRIPT_DIR/probe-cloud-rc.sh"
  run_probe "$SCRIPT_DIR/probe-relay.sh"

  PIDS=()
  for N in 1 2 3 4 5 6 7 8; do
    bash "$SCRIPT_DIR/probe-pod.sh" "$N" &
    PIDS+=($!)
  done
  for pid in "${PIDS[@]}"; do
    wait "$pid" || true
  done
fi

python3 "$SCRIPT_DIR/build-meta-index.py" "$MANIFEST_DIR" --orchestrator-start-epoch "$START_EPOCH" || true

ELAPSED=$(( $(date +%s) - START_EPOCH ))
echo "probe-all: done in ${ELAPSED}s  dir=$MANIFEST_DIR" >&2
exit 0
```

`chmod +x scripts/fleet-probe/build-meta-index.py scripts/fleet-probe/probe-all.sh`.
  </action>
  <verify>
    <automated>bash -n scripts/fleet-probe/probe-all.sh &amp;&amp; python3 -c "import ast; ast.parse(open('scripts/fleet-probe/build-meta-index.py').read())" &amp;&amp; bash scripts/fleet-probe/probe-all.sh --help &amp;&amp; [ "$(bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l)" = "15" ]</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n scripts/fleet-probe/probe-all.sh` exits 0
    - `python3 -c "import ast; ast.parse(open('scripts/fleet-probe/build-meta-index.py').read())"` exits 0
    - `bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l` == 15
    - `bash scripts/fleet-probe/probe-all.sh --help | grep -c "Usage:"` >= 1
    - `grep -c "export MANIFEST_TS" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "build-meta-index.py" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-pod.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-server.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-pos.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-james.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-vps.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-cloud-admin.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-cloud-rc.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "probe-relay.sh" scripts/fleet-probe/probe-all.sh` >= 1
    - `grep -c "TARGET_ORDER" scripts/fleet-probe/build-meta-index.py` >= 1
    - `grep -c "status_summary" scripts/fleet-probe/build-meta-index.py` >= 1
    - `grep -c "probe_run_id" scripts/fleet-probe/build-meta-index.py` >= 1
    - `grep -c "wait" scripts/fleet-probe/probe-all.sh` >= 1 (parallel pod fanout uses `wait`)
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/probe-all.sh','rb').read().decode('ascii')"` does not raise
    - ASCII-only: `python3 -c "open('scripts/fleet-probe/build-meta-index.py','rb').read().decode('ascii')"` does not raise
  </acceptance_criteria>
  <done>Full orchestrator wires all 8 probe scripts; build-meta-index.py produces SCHEMA-03-shaped _meta.json; --dry-run still emits 15 target lines; --canary runs only server_23 + pod_8; orchestrator always exits 0.</done>
</task>

<task type="auto" tdd="true">
  <name>Task 2: Create orchestrator smoke test + node unit test</name>
  <files>tests/fleet-probe/smoke-orchestrator.sh, tests/fleet-probe/orchestrator-dry-run.test.mjs</files>
  <read_first>
    - scripts/fleet-probe/probe-all.sh (just written)
    - scripts/fleet-probe/build-meta-index.py (just written)
    - tests/fleet-probe/smoke-james.sh (pattern to clone)
    - tests/fleet-probe/probe-james.test.mjs (spawnSync pattern)
  </read_first>
  <behavior>
    - `bash tests/fleet-probe/smoke-orchestrator.sh` exits 0
    - Smoke asserts: --dry-run produces 15 lines; a simulated minimal canary run that uses override URLs pointing at `http://127.0.0.1:1` produces a manifest directory with at least 2 JSON files + _meta.json (all probe_failed but schema-valid)
    - Node test asserts: --dry-run exit 0, 15 lines present, all 15 target_ids visible
    - Node test asserts: --help exits 0 and prints "Usage:"
  </behavior>
  <action>
Create `tests/fleet-probe/smoke-orchestrator.sh`:

```bash
#!/bin/bash
# tests/fleet-probe/smoke-orchestrator.sh -- Phase 448 Plan 07
# Verifies probe-all.sh --dry-run enumeration + --canary end-to-end with URLs pointed at :1 (probe_failed path).
set -eo pipefail

echo "-- smoke 1: --dry-run enumerates 15 targets --"
LINES=$(bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l)
if [ "$LINES" != "15" ]; then
  echo "FAIL: dry-run emitted $LINES lines (expected 15)" >&2
  exit 1
fi
echo "  ok: 15 targets"

echo "-- smoke 2: --help --"
bash scripts/fleet-probe/probe-all.sh --help | grep -q "Usage:" || { echo "FAIL: --help output"; exit 1; }
echo "  ok: --help prints usage"

echo "-- smoke 3: --canary produces manifest set with _meta.json --"
export MANIFEST_TS="smoke-canary-$(date +%s)"
# Force probe_failed for all probes by pointing overrides at :1 (unreachable) + using dummy SENTRY_KEY so pod probe reaches network layer.
export SENTRY_KEY="smoke-dummy"
export COMMS_PSK="smoke-dummy"
export PROBE_OVERRIDE_URL="http://127.0.0.1:1"
export PROBE_OVERRIDE_CLOUD_ADMIN_URL="http://127.0.0.1:1"
export PROBE_OVERRIDE_CLOUD_RC_URL="http://127.0.0.1:1"
export PROBE_OVERRIDE_RELAY_URL="http://127.0.0.1:1"
# Use mock SSH that always times out to avoid real SSH attempts
export PROBE_SSH="$(pwd)/tests/fleet-probe/mock-ssh-responder.sh"
export PROBE_SSH_SCENARIO="$(pwd)/tests/fleet-probe/fixtures/server-ssh-timeout.txt"
export PROBE_SKIP_HTTP=1

bash scripts/fleet-probe/probe-all.sh --canary
DIR="state/fleet-manifest/$MANIFEST_TS"
test -d "$DIR" || { echo "FAIL: manifest dir $DIR not created" >&2; exit 1; }
test -f "$DIR/_meta.json" || { echo "FAIL: _meta.json missing" >&2; exit 1; }
test -f "$DIR/server_23.json" || { echo "FAIL: server_23.json missing" >&2; exit 1; }
test -f "$DIR/pod_8.json" || { echo "FAIL: pod_8.json missing" >&2; exit 1; }

TARGET_COUNT=$(jq -r '.target_count' "$DIR/_meta.json")
if [ "$TARGET_COUNT" != "2" ]; then
  echo "FAIL: canary _meta.target_count=$TARGET_COUNT (expected 2)" >&2
  exit 1
fi

# Validate all produced manifests against the schema
for f in "$DIR"/*.json; do
  base=$(basename "$f")
  if [ "$base" = "_meta.json" ]; then continue; fi
  node scripts/fleet-probe/validate-manifest-file.mjs "$f" || { echo "FAIL: $f failed schema validation" >&2; exit 1; }
done

echo "  ok: canary dir has 2 manifests + _meta.json, all schema-valid"

rm -rf "$DIR"
echo "smoke-orchestrator OK"
```

`chmod +x tests/fleet-probe/smoke-orchestrator.sh`.

Create `tests/fleet-probe/orchestrator-dry-run.test.mjs`:

```js
// tests/fleet-probe/orchestrator-dry-run.test.mjs -- Phase 448 Plan 07
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";

const ALL_TARGETS = [
  "server_23", "pod_1", "pod_2", "pod_3", "pod_4",
  "pod_5", "pod_6", "pod_7", "pod_8",
  "pos_130", "james_27", "bono_vps",
  "cloud_admin", "cloud_racecontrol", "relay_james",
];

test("probe-all.sh --dry-run enumerates 15 targets", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-all.sh", "--dry-run"], {
    encoding: "utf8",
    timeout: 15_000,
  });
  assert.equal(res.status, 0, `exit=${res.status} stderr=${res.stderr}`);
  const lines = res.stdout.trim().split(/\r?\n/);
  assert.equal(lines.length, 15, `expected 15 lines, got ${lines.length}:\n${res.stdout}`);
  for (const t of ALL_TARGETS) {
    assert.ok(
      lines.some((l) => l.includes(`target=${t}`)),
      `missing target ${t} in dry-run output`
    );
  }
});

test("probe-all.sh --help exits 0 and prints Usage:", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-all.sh", "--help"], {
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 0);
  assert.match(res.stdout, /Usage:/);
});
```
  </action>
  <verify>
    <automated>bash -n tests/fleet-probe/smoke-orchestrator.sh &amp;&amp; node --check tests/fleet-probe/orchestrator-dry-run.test.mjs &amp;&amp; npm run test:fleet-probe &amp;&amp; bash tests/fleet-probe/smoke-orchestrator.sh</automated>
  </verify>
  <acceptance_criteria>
    - `bash -n tests/fleet-probe/smoke-orchestrator.sh` exits 0
    - `node --check tests/fleet-probe/orchestrator-dry-run.test.mjs` exits 0
    - `npm run test:fleet-probe` exits 0 (orchestrator-dry-run tests now included)
    - `bash tests/fleet-probe/smoke-orchestrator.sh` exits 0 (full end-to-end canary works)
    - `grep -c "target_count" tests/fleet-probe/smoke-orchestrator.sh` >= 1
    - `grep -c "validate-manifest-file.mjs" tests/fleet-probe/smoke-orchestrator.sh` >= 1
    - `grep -c "server_23.json" tests/fleet-probe/smoke-orchestrator.sh` >= 1
    - `grep -c "pod_8.json" tests/fleet-probe/smoke-orchestrator.sh` >= 1
  </acceptance_criteria>
  <done>Smoke test proves orchestrator end-to-end: --dry-run (15 lines), --help (usage), --canary (2 manifests + _meta.json, all schema-valid); Node unit test covers dry-run and help; `npm run test:fleet-probe` suite total green.</done>
</task>

</tasks>

<verification>
- `npm run test:fleet-probe` exits 0 with all tests green (schema-compat + per-probe unit tests + orchestrator-dry-run)
- `bash tests/fleet-probe/smoke-orchestrator.sh` exits 0
- `bash scripts/fleet-probe/probe-all.sh --dry-run | wc -l` == 15
- `npm run test:fleet-drift` still exits 0
</verification>

<success_criteria>
- One-invocation fleet probe exists: `bash scripts/fleet-probe/probe-all.sh`
- Canary mode works end-to-end (2 manifests + _meta.json)
- _meta.json status_summary counts computed automatically from written manifests
- Every produced manifest (on canary) passes ajv schema validation
- Orchestrator is ready for Phase 449 live-run execution gate
</success_criteria>

<output>
After completion, create `.planning/phases/448-per-target-probe-scripts/448-07-SUMMARY.md` with:
- Files created / modified
- Test results (node test + bash smoke)
- Sample _meta.json excerpt from the canary smoke run
- Wall-clock time for `--canary` (typical < 60s)
- Handoff to Plan 08 (access-gaps.md scaffold + handoff to Phase 449)
</output>
