// tests/fleet-probe/helpers.mjs -- Phase 448 Plan 01 test helpers
// ESM module exporting 4 functions for probe unit tests:
//   startMockHttpServer, makeMockSshEnv, loadFixture, validateAgainstSchema
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
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

// startMockHttpServer(routes)
// routes: { "/path": { status: N, body: "string", contentType: "..." } }
// Returns Promise<{ url: string, close: () => Promise<void> }>
// Binds to 127.0.0.1:0 (OS-assigned ephemeral port).
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

// makeMockSshEnv(scenarioFile)
// Returns an env object with PROBE_SSH pointing to mock-ssh-responder.sh
// and PROBE_SSH_SCENARIO pointing to the given scenario file.
export function makeMockSshEnv(scenarioFile) {
  return {
    PROBE_SSH: MOCK_SSH_PATH,
    PROBE_SSH_SCENARIO: scenarioFile,
  };
}

// loadFixture(name)
// Reads tests/fleet-probe/fixtures/<name>.json and returns the parsed object.
export function loadFixture(name) {
  return JSON.parse(readFileSync(join(FIXTURES_DIR, `${name}.json`), "utf8"));
}

// validateAgainstSchema(obj)
// Returns { valid: bool, errors: array|null } using ajv 2020 against fleet-manifest.schema.json.
export function validateAgainstSchema(obj) {
  const validate = getValidator();
  const valid = validate(obj);
  return { valid, errors: valid ? null : validate.errors };
}
