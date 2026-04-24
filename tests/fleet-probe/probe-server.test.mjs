// tests/fleet-probe/probe-server.test.mjs -- Phase 448 Plan 03
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { validateAgainstSchema } from "./helpers.mjs";

const MOCK_SSH = resolve("tests/fleet-probe/mock-ssh-responder.sh");

// Resolve real Python interpreter path (avoids Windows Store python3 stub that hangs).
const PYTHON_CMD = process.platform === "win32" ? "python" : "python3";

function runProbe(scenarioPath, extra = {}) {
  const ts = "test-server-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);
  const env = {
    ...process.env,
    MANIFEST_TS: ts,
    PROBE_SSH: MOCK_SSH,
    PROBE_SSH_SCENARIO: resolve(scenarioPath),
    PROBE_SKIP_HTTP: "1",
    PROBE_PYTHON: PYTHON_CMD,
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
  // 3-way config_hash keys (Q5 drift surfacing) -- at least server_live must be present on ok path
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
