// tests/fleet-probe/probe-cloud-rc.test.mjs -- Phase 448 Plan 06
// Unit tests for probe-cloud-rc.sh:
//   - ok path: build_id captured, schema-valid
//   - 500: probe_failed
//   - malformed JSON 200: partial + health_parse error
//   - 200 but no build_id field: partial + build_id error
// Uses async spawn (not spawnSync) so the in-process mock HTTP server
// can serve connections while the bash subprocess runs.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { spawn } from "node:child_process";
import { existsSync, readFileSync, rmSync } from "node:fs";
import { resolve, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { startMockHttpServer, validateAgainstSchema } from "./helpers.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");

// Load response fixtures from responses/ subdir (not top-level fixtures/ to avoid
// schema-compat.test.mjs validating non-manifest JSON -- same pattern as 448-04).
function loadResponseFixture(name) {
  const p = join(__dirname, "fixtures", "responses", `${name}.json`);
  return JSON.parse(readFileSync(p, "utf8"));
}

// Run probe-cloud-rc.sh against a mock server.
// body: string or object (serialized to JSON if object)
// status: HTTP status code for /api/v1/health
// Returns Promise<manifest object>
async function runProbeRC({ body, status = 200 }) {
  const bodyStr = typeof body === "string" ? body : JSON.stringify(body);
  const server = await startMockHttpServer({
    "/api/v1/health": { status, body: bodyStr },
  });

  const ts = "test-crc-" + Date.now() + "-" + Math.random().toString(36).slice(2, 8);

  return new Promise((res_p, rej) => {
    const child = spawn("bash", ["scripts/fleet-probe/probe-cloud-rc.sh"], {
      cwd: REPO_ROOT,
      env: {
        ...process.env,
        MANIFEST_TS: ts,
        PROBE_OVERRIDE_CLOUD_RC_URL: server.url,
      },
    });

    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => { stdout += d.toString(); });
    child.stderr.on("data", (d) => { stderr += d.toString(); });

    child.on("close", async (exitCode) => {
      await server.close();
      const mpath = resolve(REPO_ROOT, "state/fleet-manifest", ts, "cloud_racecontrol.json");
      const m = existsSync(mpath) ? JSON.parse(readFileSync(mpath, "utf8")) : null;
      try { rmSync(resolve(REPO_ROOT, "state/fleet-manifest", ts), { recursive: true, force: true }); } catch (_) {}
      res_p({ stdout, stderr, exitCode, m });
    });

    child.on("error", async (err) => {
      await server.close();
      rej(err);
    });

    setTimeout(() => {
      child.kill();
      rej(new Error("probe-cloud-rc.sh timed out after 60s"));
    }, 60_000);
  });
}

// --- Test 1: ok path ---
test("probe-cloud-rc.sh ok path -> build_id captured, schema-valid", async () => {
  const { m } = await runProbeRC({ body: loadResponseFixture("cloud-rc-health-ok") });
  assert.ok(m, "manifest must be written");
  assert.equal(m.probe_status, "ok", `expected ok, got ${m.probe_status}`);
  assert.equal(m.build_id, "129a24f2", `build_id mismatch; got ${m.build_id}`);
  assert.equal(m.target_id, "cloud_racecontrol");
  assert.equal(m.role, "cloud_racecontrol");
  assert.ok(!m.probe_errors || m.probe_errors.length === 0, `unexpected probe_errors: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 2: 500 -> probe_failed ---
test("probe-cloud-rc.sh 500 -> probe_failed", async () => {
  const { m } = await runProbeRC({ body: "server error", status: 500 });
  assert.ok(m, "manifest must be written on probe_failed");
  assert.equal(m.probe_status, "probe_failed", `expected probe_failed, got ${m.probe_status}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 3: malformed JSON 200 -> partial + health_parse ---
test("probe-cloud-rc.sh malformed JSON 200 -> partial + health_parse error", async () => {
  const { m } = await runProbeRC({ body: "not json at all", status: 200 });
  assert.ok(m, "manifest must be written on partial");
  assert.equal(m.probe_status, "partial", `expected partial, got ${m.probe_status}`);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "health_parse");
  assert.ok(err, `expected health_parse sub_probe; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});

// --- Test 4: 200 but no build_id -> partial + build_id error ---
test("probe-cloud-rc.sh 200 but missing build_id -> partial + build_id error + null build_id", async () => {
  const { m } = await runProbeRC({ body: { healthy: true } });
  assert.ok(m, "manifest must be written on partial");
  assert.equal(m.probe_status, "partial", `expected partial, got ${m.probe_status}`);
  assert.strictEqual(m.build_id, null, `expected null build_id, got ${m.build_id}`);
  const err = (m.probe_errors || []).find((e) => e.sub_probe === "build_id");
  assert.ok(err, `expected build_id sub_probe; got: ${JSON.stringify(m.probe_errors)}`);
  const { valid, errors } = validateAgainstSchema(m);
  assert.ok(valid, `schema invalid: ${JSON.stringify(errors)}`);
});
