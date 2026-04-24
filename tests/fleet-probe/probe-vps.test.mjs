// tests/fleet-probe/probe-vps.test.mjs -- Phase 448 Plan 05
// Unit tests for probe-vps.sh:
//   - missing COMMS_PSK -> probe_failed + auth_gap no_comms_psk
//   - relay connected=false -> probe_failed + access_gap RELAY_DOWN
//   - happy path (mock relay returns exec output) -> ok or partial, running_procs populated
//   - exec non-zero exitCode -> partial + probe_errors sub_probe relay_exec
// HTTP mocking via startMockHttpServer + async spawn (event loop stays free for mock server).
// No real relay or VPS access is made.
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

// Resolve real Python interpreter path (avoids Windows Store python3 stub that hangs).
const PYTHON_CMD = process.platform === "win32" ? "python" : "python3";

// Helper: run probe-vps.sh with an in-process HTTP mock server.
// healthResponse: object to JSON-encode for /relay/health, or null to return 500
// execFixtureName: fixture name for /relay/exec/run body (null = 500)
// psk: string or "" (empty string disables COMMS_PSK from env)
async function runProbeVps({ healthResponse, execFixtureName, psk = "mock-psk" }) {
  const healthBody = healthResponse ? JSON.stringify(healthResponse) : "";
  const healthStatus = healthResponse ? 200 : 500;
  const execBody = execFixtureName ? JSON.stringify(loadFixture("responses/" + execFixtureName)) : "";
  const execStatus = execFixtureName ? 200 : 500;

  const routes = {
    "/relay/health": { status: healthStatus, body: healthBody },
    "/relay/exec/run": { status: execStatus, body: execBody },
  };
  const server = await startMockHttpServer(routes);

  const ts = "test-vps-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);

  return new Promise((res_p, rej) => {
    const env = { ...process.env, MANIFEST_TS: ts, PROBE_OVERRIDE_RELAY_URL: server.url, PROBE_PYTHON: PYTHON_CMD };
    if (psk) {
      env.COMMS_PSK = psk;
    } else {
      delete env.COMMS_PSK;
    }

    const child = spawn("bash", ["scripts/fleet-probe/probe-vps.sh"], {
      cwd: REPO_ROOT,
      env,
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });

    child.on("close", async (exitCode) => {
      await server.close();
      const mpath = join(REPO_ROOT, "state/fleet-manifest", ts, "bono_vps.json");
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
      rej(new Error("probe-vps.sh timed out after 60s"));
    }, 60_000);
  });
}

test("probe-vps.sh missing COMMS_PSK -> probe_failed + auth_gap no_comms_psk", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixtureName: "vps-relay-exec-ok", psk: "" });
  assert.ok(m, "manifest must be written on auth failure");
  assert.equal(m.probe_status, "probe_failed", `expected probe_failed, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "auth");
  assert.ok(err, `expected auth sub_probe; got: ${JSON.stringify(m.probe_errors)}`);
  assert.equal(err.auth_gap, "no_comms_psk", `expected auth_gap=no_comms_psk; got: ${JSON.stringify(err)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

test("probe-vps.sh relay connected=false -> probe_failed + RELAY_DOWN", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: false }, execFixtureName: null });
  assert.ok(m, "manifest must be written on RELAY_DOWN");
  assert.equal(m.probe_status, "probe_failed", `expected probe_failed, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.access_gap === "RELAY_DOWN");
  assert.ok(err, `expected RELAY_DOWN access_gap; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

test("probe-vps.sh happy path -> ok or partial, schema-valid, target_id=bono_vps", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixtureName: "vps-relay-exec-ok" });
  assert.ok(m, "manifest must be written on happy path");
  assert.ok(["ok", "partial"].includes(m.probe_status), `expected ok or partial, got ${m.probe_status}`);
  assert.equal(m.target_id, "bono_vps");
  assert.equal(m.role, "vps");
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

test("probe-vps.sh exec non-zero exitCode -> partial + relay_exec probe error", async () => {
  const { m } = await runProbeVps({ healthResponse: { connected: true }, execFixtureName: "vps-relay-exec-err" });
  assert.ok(m, "manifest must be written on exec error");
  assert.equal(m.probe_status, "partial", `expected partial, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "relay_exec");
  assert.ok(err, `expected relay_exec error; got: ${JSON.stringify(m.probe_errors)}`);
});
