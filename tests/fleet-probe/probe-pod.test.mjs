// tests/fleet-probe/probe-pod.test.mjs -- Phase 448 Plan 04
// Unit tests for probe-pod.sh:
//   - positional N arg validation (1..8)
//   - missing SENTRY_KEY -> probe_failed + auth_gap no_sentry_key
//   - pod number to host/IP mapping verification
//   - 401 auth response -> probe_failed + stale_sentry_key
// HTTP mocking via spawn (async child process) to avoid blocking the Node event loop
// that the mock HTTP server needs to serve connections.
// No real fleet access is made.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawnSync, spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { startMockHttpServer, validateAgainstSchema } from "./helpers.mjs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");

// Resolve real Python interpreter path (avoids Windows Store python3 stub that hangs).
// On Windows/Git Bash, 'python3' resolves to the App Execution Alias in WindowsApps which
// hangs (pops up Microsoft Store UI) when called without an interactive session.
// The real Python 3.12 installation on Windows is accessible as 'python'.
// On Linux (production VPS), 'python3' is correct.
const PYTHON_CMD = process.platform === "win32" ? "python" : "python3";

// Load fixture files (rc-sentry response bodies for mock HTTP server)
const EXEC_OK = readFileSync(join(REPO_ROOT, "tests/fleet-probe/fixtures/responses/pod-exec-ok.json"), "utf8");
const EXEC_401 = readFileSync(join(REPO_ROOT, "tests/fleet-probe/fixtures/responses/pod-exec-401.json"), "utf8");

// Helper: run probe-pod.sh with an async mock HTTP server.
// Uses spawn (not spawnSync) so the Node event loop remains free to serve HTTP.
// Returns a Promise that resolves to { stdout, stderr, exitCode, manifest }.
async function runProbeWithMock({ podN, execBody, execStatus = 200, healthBody = null, healthStatus = 404 }) {
  const server = await startMockHttpServer({
    "/exec": { status: execStatus, body: execBody },
    "/health": { status: healthStatus, body: healthBody || "" },
  });

  const { port } = new URL(server.url);
  const ts = "test-pod-mock-" + Date.now() + "-" + Math.random().toString(36).slice(2, 7);

  return new Promise((resolve_p, reject) => {
    const child = spawn("bash", ["scripts/fleet-probe/probe-pod.sh", String(podN)], {
      cwd: REPO_ROOT,
      env: {
        ...process.env,
        MANIFEST_TS: ts,
        SENTRY_KEY: "mock-sentry-key-ok",
        PROBE_OVERRIDE_URL: server.url,
        PROBE_OVERRIDE_PORT_SENTRY: port,
        PROBE_OVERRIDE_PORT_AGENT: port,
        PROBE_PYTHON: PYTHON_CMD,
      },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });

    child.on("close", async (exitCode) => {
      await server.close();
      const manifestPath = join(REPO_ROOT, "state/fleet-manifest", ts, `pod_${podN}.json`);
      const manifest = existsSync(manifestPath)
        ? JSON.parse(readFileSync(manifestPath, "utf8"))
        : null;
      // Cleanup
      try { rmSync(join(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true }); } catch (_) {}
      resolve_p({ stdout, stderr, exitCode, manifest });
    });

    child.on("error", async (err) => {
      await server.close();
      reject(err);
    });

    // Timeout guard
    setTimeout(() => {
      child.kill();
      reject(new Error("probe-pod.sh timed out after 60s"));
    }, 60_000);
  });
}

// ---- Validation Tests ----

test("probe-pod.sh exits 2 for invalid pod number 9", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "9"], {
    cwd: REPO_ROOT,
    env: { ...process.env, MANIFEST_TS: "test-invalid", SENTRY_KEY: "k" },
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2, `expected exit 2, got ${res.status}; stderr=${res.stderr}`);
  assert.match(res.stderr, /invalid pod number/, "stderr must mention 'invalid pod number'");
});

test("probe-pod.sh exits 2 for pod number 0", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "0"], {
    cwd: REPO_ROOT,
    env: { ...process.env, MANIFEST_TS: "test-invalid-0", SENTRY_KEY: "k" },
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2, `expected exit 2 for pod 0, got ${res.status}`);
});

test("probe-pod.sh exits 2 when no pod number given", () => {
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh"], {
    cwd: REPO_ROOT,
    env: { ...process.env, MANIFEST_TS: "test-noarg", SENTRY_KEY: "k" },
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2, `expected exit 2 for no arg, got ${res.status}`);
});

test("probe-pod.sh exits 2 when MANIFEST_TS is not set", () => {
  const env = { ...process.env };
  delete env.MANIFEST_TS;
  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "1"], {
    cwd: REPO_ROOT,
    env,
    encoding: "utf8",
    timeout: 10_000,
  });
  assert.equal(res.status, 2, `expected exit 2 for missing MANIFEST_TS, got ${res.status}`);
  assert.match(res.stderr, /MANIFEST_TS/, "stderr must mention MANIFEST_TS");
});

// ---- Missing SENTRY_KEY -> probe_failed + auth_gap no_sentry_key ----

test("probe-pod.sh with missing SENTRY_KEY -> probe_failed + auth_gap no_sentry_key", () => {
  const ts = "test-pod-nokey-" + Date.now();
  const env = { ...process.env, MANIFEST_TS: ts, PROBE_PYTHON: PYTHON_CMD };
  delete env.SENTRY_KEY;
  delete env.RCAGENT_SERVICE_KEY;
  // Use a port that won't respond -- probe should fail before trying network (auth pre-check)
  env.PROBE_OVERRIDE_URL = "http://127.0.0.1:1";
  env.PROBE_OVERRIDE_PORT_SENTRY = "1";
  env.PROBE_OVERRIDE_PORT_AGENT = "1";

  const res = spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "1"], {
    cwd: REPO_ROOT,
    env,
    encoding: "utf8",
    timeout: 30_000,
  });

  const manifestPath = resolve(REPO_ROOT, "state/fleet-manifest", ts, "pod_1.json");
  assert.ok(existsSync(manifestPath), `manifest must be written even on probe_failed: ${manifestPath}`);

  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  assert.equal(manifest.probe_status, "probe_failed", `expected probe_failed, got ${manifest.probe_status}`);

  const authErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "auth");
  assert.ok(authErr, `expected auth sub_probe error; got: ${JSON.stringify(manifest.probe_errors)}`);
  assert.equal(authErr.auth_gap, "no_sentry_key", `expected auth_gap=no_sentry_key; got: ${JSON.stringify(authErr)}`);

  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema validation failed: ${JSON.stringify(errors)}`);

  rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true });
});

// ---- Pod IP/host mapping tests ----

test("pod_1 manifest has correct host RCPOD-1 and ip 192.168.31.89", () => {
  const ts = "test-pod-map1-" + Date.now();
  // Use port 1 (unreachable) to get a probe_failed manifest quickly -- mapping is set before network
  const env = {
    ...process.env,
    MANIFEST_TS: ts,
    SENTRY_KEY: "k",
    PROBE_OVERRIDE_URL: "http://127.0.0.1:1",
    PROBE_OVERRIDE_PORT_SENTRY: "1",
    PROBE_OVERRIDE_PORT_AGENT: "1",
    PROBE_PYTHON: PYTHON_CMD,
  };
  spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "1"], {
    cwd: REPO_ROOT, env, encoding: "utf8", timeout: 30_000,
  });
  const m = JSON.parse(readFileSync(resolve(REPO_ROOT, "state/fleet-manifest", ts, "pod_1.json"), "utf8"));
  assert.equal(m.target_id, "pod_1");
  assert.equal(m.host, "RCPOD-1");
  assert.equal(m.ip, "192.168.31.89");
  assert.equal(m.role, "pod");
  rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true });
});

test("pod_8 manifest has correct host RCPOD-8 and ip 192.168.31.91", () => {
  const ts = "test-pod-map8-" + Date.now();
  const env = {
    ...process.env,
    MANIFEST_TS: ts,
    SENTRY_KEY: "k",
    PROBE_OVERRIDE_URL: "http://127.0.0.1:1",
    PROBE_OVERRIDE_PORT_SENTRY: "1",
    PROBE_OVERRIDE_PORT_AGENT: "1",
    PROBE_PYTHON: PYTHON_CMD,
  };
  spawnSync("bash", ["scripts/fleet-probe/probe-pod.sh", "8"], {
    cwd: REPO_ROOT, env, encoding: "utf8", timeout: 30_000,
  });
  const m = JSON.parse(readFileSync(resolve(REPO_ROOT, "state/fleet-manifest", ts, "pod_8.json"), "utf8"));
  assert.equal(m.target_id, "pod_8");
  assert.equal(m.host, "RCPOD-8");
  assert.equal(m.ip, "192.168.31.91");
  rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true });
});

// ---- 401 response -> probe_failed + auth sub_probe ----

test("probe-pod.sh with 401 from rc-sentry -> probe_failed + stale_sentry_key", async () => {
  const { res, manifest, ts } = await runProbeWithMock({
    podN: 1,
    execBody: EXEC_401,
    execStatus: 401,
  });

  assert.ok(manifest, "manifest must be written on 401");
  assert.equal(manifest.probe_status, "probe_failed", `expected probe_failed on 401; got ${manifest.probe_status}`);
  const authErr = (manifest.probe_errors || []).find((e) => e.sub_probe === "auth");
  assert.ok(authErr, `expected auth sub_probe; got ${JSON.stringify(manifest.probe_errors)}`);
  assert.equal(authErr.auth_gap, "stale_sentry_key");

  const { valid, errors } = validateAgainstSchema(manifest);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});
