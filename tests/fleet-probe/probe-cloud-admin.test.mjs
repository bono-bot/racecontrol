// tests/fleet-probe/probe-cloud-admin.test.mjs -- Phase 448 Plan 06
// Unit tests for probe-cloud-admin.sh:
//   - ok path: build_id captured, no probe_errors, schema-valid
//   - gate active: 307 redirect -> scheduled_tasks has ADMIN_COMING_SOON_GATE active
//   - pages_missing non-empty -> partial + pages_probe error
//   - /api/health 500 -> probe_failed
// Uses async spawn (not spawnSync) so the in-process mock HTTP server
// can serve connections while the bash subprocess runs.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { validateAgainstSchema } from "./helpers.mjs";

// Load response fixtures from responses/ subdir (not top-level fixtures/ to avoid
// schema-compat.test.mjs validating non-manifest JSON -- same pattern as 448-04).
function loadResponseFixture(name) {
  const p = join(__dirname, "fixtures", "responses", `${name}.json`);
  return JSON.parse(readFileSync(p, "utf8"));
}

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");

// Start a mock HTTP server that handles both GET /api/health and HEAD /
// rootRedirect: if set (e.g. "/coming-soon"), HEAD / returns 307 with that Location
// apiHealthStatus: HTTP status for GET /api/health (default 200)
// apiHealth: response body object for GET /api/health
function startAdminMockServer({ apiHealth, apiHealthStatus = 200, rootRedirect = null }) {
  return new Promise((ok) => {
    const server = createServer((req, res) => {
      if (req.url === "/api/health") {
        res.writeHead(apiHealthStatus, { "content-type": "application/json" });
        res.end(JSON.stringify(apiHealth));
      } else if (req.url === "/" && req.method === "HEAD") {
        if (rootRedirect) {
          res.writeHead(307, { location: rootRedirect });
          res.end();
        } else {
          res.writeHead(200, { "content-type": "text/html" });
          res.end();
        }
      } else if (req.url === "/") {
        res.writeHead(200, { "content-type": "text/html" });
        res.end("<html>ok</html>");
      } else {
        res.writeHead(404);
        res.end();
      }
    });
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      ok({
        url: `http://127.0.0.1:${port}`,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}

// Run probe-cloud-admin.sh against a mock server; returns Promise<manifest>
function runProbeAdmin({ server, ts }) {
  return new Promise((resolve_p, reject) => {
    const child = spawn("bash", ["scripts/fleet-probe/probe-cloud-admin.sh"], {
      cwd: REPO_ROOT,
      env: {
        ...process.env,
        MANIFEST_TS: ts,
        PROBE_OVERRIDE_CLOUD_ADMIN_URL: server.url,
      },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });

    child.on("close", async (exitCode) => {
      const manifestPath = resolve(REPO_ROOT, "state/fleet-manifest", ts, "cloud_admin.json");
      const manifest = existsSync(manifestPath)
        ? JSON.parse(readFileSync(manifestPath, "utf8"))
        : null;
      try { rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true }); } catch (_) {}
      resolve_p({ stdout, stderr, exitCode, manifest });
    });

    child.on("error", (err) => reject(err));

    setTimeout(() => {
      child.kill();
      reject(new Error("probe-cloud-admin.sh timed out after 60s"));
    }, 60_000);
  });
}

// --- Test 1: ok path ---
test("probe-cloud-admin.sh ok path -> build_id captured, no probe_errors, schema-valid", async () => {
  const server = await startAdminMockServer({ apiHealth: loadResponseFixture("cloud-admin-health-ok") });
  const ts = "test-ca-ok-" + Date.now();
  let result;
  try {
    result = await runProbeAdmin({ server, ts });
  } finally {
    await server.close();
  }
  const m = result.manifest;
  assert.ok(m, "manifest must be written");
  assert.equal(m.probe_status, "ok", `expected ok, got ${m.probe_status}; stderr=${result.stderr}`);
  assert.equal(m.build_id, "cDyHRUgWTiqZTchmlEPgz", `build_id mismatch`);
  assert.equal(m.target_id, "cloud_admin");
  assert.equal(m.role, "cloud_admin");
  assert.ok(!m.probe_errors || m.probe_errors.length === 0, `unexpected probe_errors: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 2: gate active (307 redirect to /coming-soon) ---
test("probe-cloud-admin.sh gate active -> scheduled_tasks has ADMIN_COMING_SOON_GATE active + schema-valid", async () => {
  const server = await startAdminMockServer({
    apiHealth: loadResponseFixture("cloud-admin-health-ok"),
    rootRedirect: "/coming-soon",
  });
  const ts = "test-ca-gate-" + Date.now();
  let result;
  try {
    result = await runProbeAdmin({ server, ts });
  } finally {
    await server.close();
  }
  const m = result.manifest;
  assert.ok(m, "manifest must be written");
  const gateEntry = (m.scheduled_tasks || []).find((s) => s.name === "ADMIN_COMING_SOON_GATE");
  assert.ok(gateEntry, `expected ADMIN_COMING_SOON_GATE in scheduled_tasks; got: ${JSON.stringify(m.scheduled_tasks)}`);
  assert.equal(gateEntry.state, "active", `expected state=active, got: ${gateEntry.state}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 3: pages_missing non-empty -> partial + pages_probe error ---
test("probe-cloud-admin.sh pages_missing non-empty -> partial + pages_probe error", async () => {
  const server = await startAdminMockServer({ apiHealth: loadResponseFixture("cloud-admin-health-gated") });
  const ts = "test-ca-pm-" + Date.now();
  let result;
  try {
    result = await runProbeAdmin({ server, ts });
  } finally {
    await server.close();
  }
  const m = result.manifest;
  assert.ok(m, "manifest must be written");
  assert.equal(m.probe_status, "partial", `expected partial, got ${m.probe_status}`);
  const pageErr = (m.probe_errors || []).find((e) => e.sub_probe === "pages_probe");
  assert.ok(pageErr, `expected pages_probe error; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 4: /api/health 500 -> probe_failed ---
test("probe-cloud-admin.sh /api/health 500 -> probe_failed", async () => {
  const server = await startAdminMockServer({ apiHealth: {}, apiHealthStatus: 500 });
  const ts = "test-ca-500-" + Date.now();
  let result;
  try {
    result = await runProbeAdmin({ server, ts });
  } finally {
    await server.close();
  }
  const m = result.manifest;
  assert.ok(m, "manifest must be written even on probe_failed");
  assert.equal(m.probe_status, "probe_failed", `expected probe_failed, got ${m.probe_status}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});
