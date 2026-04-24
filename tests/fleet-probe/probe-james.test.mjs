// tests/fleet-probe/probe-james.test.mjs -- Phase 448 Plan 02
// Unit tests for probe-james.sh -- pure localhost probe, no SSH/HTTP required.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { validateAgainstSchema } from "./helpers.mjs";

// Resolve real Python interpreter path (avoids Windows Store python3 stub that hangs).
// On Windows/Git Bash, 'python3' resolves to the App Execution Alias in WindowsApps which
// hangs (pops up Microsoft Store UI) when called without an interactive session.
// The real Python 3.12 installation on Windows is accessible as 'python'.
// On Linux (production VPS), 'python3' is correct.
const PYTHON_CMD = process.platform === "win32" ? "python" : "python3";

test("probe-james.sh writes schema-valid manifest and exits 0", () => {
  const ts = "test-" + Date.now();
  const env = { ...process.env, MANIFEST_TS: ts, PROBE_PYTHON: PYTHON_CMD };
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-james.sh"], {
    env,
    encoding: "utf8",
    timeout: 60_000,
  });
  assert.equal(
    res.status,
    0,
    `exit=${res.status} stderr=${res.stderr} stdout=${res.stdout}`
  );

  const manifestPath = resolve("state/fleet-manifest", ts, "james_27.json");
  assert.ok(existsSync(manifestPath), `manifest not written: ${manifestPath}`);

  const obj = JSON.parse(readFileSync(manifestPath, "utf8"));

  // Target identity assertions
  assert.equal(obj.target_id, "james_27", "target_id mismatch");
  assert.equal(obj.host, "JAMES-PC", "host mismatch");
  assert.equal(obj.ip, "192.168.31.27", "ip mismatch");
  assert.equal(obj.role, "james", "role mismatch");
  assert.equal(obj.schema_version, "1.0", "schema_version mismatch");

  // james is always-available class -- must always be ok on localhost
  assert.equal(obj.probe_status, "ok", "probe_status must be ok for localhost probe");

  // Null fields expected for james_27 (no deploy pipeline)
  assert.equal(obj.build_id, null, "build_id must be null for james");
  assert.equal(obj.last_deploy_ts, null, "last_deploy_ts must be null for james");
  assert.deepEqual(obj.binary_sha256, {}, "binary_sha256 must be empty {} for james");

  // running_procs must be non-empty (we're running on a live system)
  assert.ok(Array.isArray(obj.running_procs), "running_procs must be array");
  assert.ok(
    obj.running_procs.length > 0,
    "running_procs must be non-empty on a running Windows system"
  );

  // Each proc entry shape
  for (const proc of obj.running_procs) {
    assert.ok(typeof proc.name === "string" && proc.name.length > 0, `proc.name must be non-empty string: ${JSON.stringify(proc)}`);
    assert.ok(Number.isInteger(proc.pid) && proc.pid >= 0, `proc.pid must be non-negative integer: ${JSON.stringify(proc)}`);
    assert.ok(
      /^[0-9a-f]{64}$/.test(proc.cmdline_hash),
      `proc.cmdline_hash must be 64-char hex: ${JSON.stringify(proc)}`
    );
  }

  // env_vars_hash must be 64-char hex
  assert.ok(
    /^[0-9a-f]{64}$/.test(obj.env_vars_hash),
    `env_vars_hash must be 64-char hex, got: ${obj.env_vars_hash}`
  );

  // config_hash must be an object (may be empty if comms-link config not present)
  assert.ok(typeof obj.config_hash === "object" && obj.config_hash !== null, "config_hash must be object");

  // scheduled_tasks must be array (may be empty or have entries)
  assert.ok(Array.isArray(obj.scheduled_tasks), "scheduled_tasks must be array");

  // autostart_entries must be array
  assert.ok(Array.isArray(obj.autostart_entries), "autostart_entries must be array");

  // Schema validation via ajv
  const { valid, errors } = validateAgainstSchema(obj);
  assert.ok(valid, `schema validation failed: ${JSON.stringify(errors, null, 2)}`);

  // stdout: single-line status JSON (last line of stdout)
  const lines = res.stdout.trim().split(/\r?\n/);
  const lastLine = lines[lines.length - 1];
  const status = JSON.parse(lastLine);
  assert.equal(status.target_id, "james_27", "stdout target_id mismatch");
  assert.equal(status.probe_status, "ok", "stdout probe_status must be ok");
  assert.equal(
    typeof status.duration_ms,
    "number",
    "stdout duration_ms must be a number"
  );
  assert.equal(status.errors_count, 0, "stdout errors_count must be 0 for ok probe");

  // Cleanup
  rmSync(resolve("state/fleet-manifest", ts), { recursive: true, force: true });
});

test("probe-james.sh exits 2 when MANIFEST_TS is unset", () => {
  const env = { ...process.env, PROBE_PYTHON: PYTHON_CMD };
  delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-james.sh"], {
    env,
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(
    res.status,
    2,
    `Expected exit 2 for missing MANIFEST_TS, got exit=${res.status}`
  );
  assert.match(
    res.stderr,
    /MANIFEST_TS/,
    "stderr must mention MANIFEST_TS"
  );
});
