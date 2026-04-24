// tests/fleet-probe/probe-pos.test.mjs -- Phase 448 Plan 04
// Unit tests for probe-pos.sh:
//   - partial path: tasklist WMI denied -> probe_status=partial + sub_probe=tasklist
//   - probe_failed path: SSH unreachable -> probe_status=probe_failed + access_gap=POS_SSH_DOWN
//   - precondition: MANIFEST_TS unset -> exit 2
// Mock SSH via PROBE_SSH env var pointing to mock-ssh-responder.sh.
// No real fleet access is made.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { validateAgainstSchema } from "./helpers.mjs";
import { fileURLToPath } from "node:url";
import { dirname } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");

const MOCK_SSH = resolve(REPO_ROOT, "tests/fleet-probe/mock-ssh-responder.sh");

// ---- Partial path: tasklist WMI denied -> probe_status=partial ----

test("probe-pos.sh partial path: tasklist WMI denied -> partial + sub_probe tasklist", () => {
  const ts = "test-pos-partial-" + Date.now();
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      MANIFEST_TS: ts,
      PROBE_SSH: MOCK_SSH,
      PROBE_SSH_SCENARIO: resolve(REPO_ROOT, "tests/fleet-probe/fixtures/pos-ssh-partial.txt"),
      PROBE_SKIP_HTTP: "1",
    },
    encoding: "utf8",
    timeout: 30_000,
  });

  const manifestPath = resolve(REPO_ROOT, "state/fleet-manifest", ts, "pos_130.json");
  assert.ok(existsSync(manifestPath), `manifest not written; exit=${res.status} stderr=${res.stderr}`);

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

  // Identity fields
  assert.equal(manifest.target_id, "pos_130");
  assert.equal(manifest.role, "pos");
  assert.equal(manifest.host, "POS1");
  assert.equal(manifest.ip, "192.168.31.130");

  // WMI-denied tasklist -> partial (not probe_failed)
  assert.equal(
    manifest.probe_status,
    "partial",
    `expected probe_status=partial; got ${manifest.probe_status}; errors=${JSON.stringify(manifest.probe_errors)}`
  );

  // tasklist sub_probe error must be present
  const tasklistErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "tasklist");
  assert.ok(
    tasklistErr,
    `expected tasklist sub_probe error; got: ${JSON.stringify(manifest.probe_errors)}`
  );

  // running_procs should be empty (WMI-denied means no process data)
  assert.deepEqual(manifest.running_procs, [], "running_procs must be [] on WMI-denied tasklist");

  // schtasks should have data (it succeeded in the fixture)
  assert.ok(Array.isArray(manifest.scheduled_tasks), "scheduled_tasks must be array");
  assert.ok(
    manifest.scheduled_tasks.length >= 1,
    `expected at least 1 scheduled task from fixture; got ${manifest.scheduled_tasks.length}`
  );

  // autostart_entries should have data from reg_hklm
  assert.ok(Array.isArray(manifest.autostart_entries), "autostart_entries must be array");
  assert.ok(
    manifest.autostart_entries.length >= 1,
    `expected at least 1 autostart entry from fixture; got ${manifest.autostart_entries.length}`
  );

  // Schema validation
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema validation failed: ${JSON.stringify(errors)}`);

  rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true });
});

// ---- probe_failed path: SSH unreachable -> probe_failed + access_gap POS_SSH_DOWN ----

test("probe-pos.sh SSH timeout -> probe_failed + access_gap POS_SSH_DOWN", () => {
  const ts = "test-pos-down-" + Date.now();
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    cwd: REPO_ROOT,
    env: {
      ...process.env,
      MANIFEST_TS: ts,
      PROBE_SSH: MOCK_SSH,
      PROBE_SSH_SCENARIO: resolve(REPO_ROOT, "tests/fleet-probe/fixtures/server-ssh-timeout.txt"),
      PROBE_SKIP_HTTP: "1",
    },
    encoding: "utf8",
    timeout: 30_000,
  });

  const manifestPath = resolve(REPO_ROOT, "state/fleet-manifest", ts, "pos_130.json");
  assert.ok(existsSync(manifestPath), `manifest must be written even on probe_failed; exit=${res.status} stderr=${res.stderr}`);

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(manifest.probe_status, "probe_failed", `expected probe_failed on SSH timeout`);

  const connErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "ssh_connect");
  assert.ok(connErr, `expected ssh_connect error; got: ${JSON.stringify(manifest.probe_errors)}`);
  assert.equal(connErr.access_gap, "POS_SSH_DOWN", `expected access_gap=POS_SSH_DOWN`);

  // probe_failed: all data fields must be zeroed
  assert.deepEqual(manifest.binary_sha256, {});
  assert.deepEqual(manifest.running_procs, []);
  assert.deepEqual(manifest.scheduled_tasks, []);
  assert.equal(manifest.build_id, null);

  // Schema validation
  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema validation failed: ${JSON.stringify(errors)}`);

  rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true });
});

// ---- Precondition: MANIFEST_TS unset -> exit 2 ----

test("probe-pos.sh exits 2 when MANIFEST_TS is unset", () => {
  const env = { ...process.env };
  delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pos.sh"], {
    cwd: REPO_ROOT,
    env,
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2, `expected exit 2 for missing MANIFEST_TS; got ${res.status}`);
  assert.match(res.stderr, /MANIFEST_TS/, "stderr must mention MANIFEST_TS");
});
