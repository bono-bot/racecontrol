// tests/fleet-probe/probe-relay.test.mjs -- Phase 448 Plan 05
// Unit tests for probe-relay.sh:
//   - /relay/health 200 + connected:true -> probe_status ok, schema-valid, build_id null, last_deploy_ts null
//   - /relay/health 200 + connected:false -> probe_status partial + sub_probe vps_relay
//   - /relay/health 500 -> probe_status probe_failed + access_gap RELAY_LOCAL_DOWN
// HTTP mocking via startMockHttpServer + async spawn (event loop stays free for mock server).
// No real relay access is made.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { startMockHttpServer, validateAgainstSchema, loadFixture } from "./helpers.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");

// Helper: run probe-relay.sh with an in-process HTTP mock server.
// healthFixtureName: fixture name for /relay/health body, or null to return healthStatus error
// healthStatus: HTTP status code for /relay/health (default 200)
async function runProbeRelay({ healthFixtureName, healthStatus = 200 }) {
  const healthBody = healthFixtureName ? JSON.stringify(loadFixture("responses/" + healthFixtureName)) : "";
  const effectiveStatus = healthFixtureName ? healthStatus : 500;

  const routes = {
    "/relay/health": { status: effectiveStatus, body: healthBody },
  };
  const server = await startMockHttpServer(routes);

  const ts = "test-relay-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);

  return new Promise((res_p, rej) => {
    const child = spawn("bash", ["scripts/fleet-probe/probe-relay.sh"], {
      cwd: REPO_ROOT,
      env: { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_RELAY_URL: server.url },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });

    child.on("close", async (exitCode) => {
      await server.close();
      const mpath = join(REPO_ROOT, "state/fleet-manifest", ts, "relay_james.json");
      const m = existsSync(mpath) ? JSON.parse(readFileSync(mpath, "utf8")) : null;
      try { rmSync(join(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true }); } catch (_) {}
      res_p({ stdout, stderr, exitCode, m });
    });

    child.on("error", async (err) => {
      await server.close();
      rej(err);
    });

    setTimeout(() => {
      child.kill();
      rej(new Error("probe-relay.sh timed out after 60s"));
    }, 60_000);
  });
}

test("probe-relay.sh both connected -> probe_status ok, schema-valid, null build_id + null last_deploy_ts", async () => {
  const { m } = await runProbeRelay({ healthFixtureName: "relay-health-ok" });
  assert.ok(m, "manifest must be written");
  assert.equal(m.target_id, "relay_james");
  assert.equal(m.role, "relay");
  assert.equal(m.probe_status, "ok", `expected ok, got ${m.probe_status}`);
  assert.equal(m.build_id, null, "build_id must be null for relay");
  assert.equal(m.last_deploy_ts, null, "last_deploy_ts must be null for relay");
  assert.deepEqual(m.binary_sha256, {}, "binary_sha256 must be empty object");
  assert.deepEqual(m.config_hash, {}, "config_hash must be empty object");
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

test("probe-relay.sh VPS disconnected -> partial + sub_probe vps_relay", async () => {
  const { m } = await runProbeRelay({ healthFixtureName: "relay-health-disconnected" });
  assert.ok(m, "manifest must be written");
  assert.equal(m.probe_status, "partial", `expected partial, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "vps_relay");
  assert.ok(err, `expected vps_relay error; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

test("probe-relay.sh local down -> probe_failed + RELAY_LOCAL_DOWN", async () => {
  const { m } = await runProbeRelay({ healthFixtureName: null, healthStatus: 500 });
  assert.ok(m, "manifest must be written");
  assert.equal(m.probe_status, "probe_failed", `expected probe_failed, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.access_gap === "RELAY_LOCAL_DOWN");
  assert.ok(err, `expected RELAY_LOCAL_DOWN; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});
