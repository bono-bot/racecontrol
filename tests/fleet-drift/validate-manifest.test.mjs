// tests/fleet-drift/validate-manifest.test.mjs
// Phase 447 Plan 03 — validates every schemas/examples/<target>.json against schemas/fleet-manifest.schema.json,
// plus positive forward-compat fixtures (SCHEMA-02) and negative fixtures.
// Runner: node --test (Node 18+).

import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync, readdirSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");
const SCHEMA_PATH = join(REPO_ROOT, "schemas", "fleet-manifest.schema.json");
const EXAMPLES_DIR = join(REPO_ROOT, "schemas", "examples");
const FIXTURES_DIR = join(__dirname, "fixtures");

const PER_TARGET_EXAMPLE_IDS = [
  "server_23",
  "pod_1",
  "pos_130",
  "james_27",
  "bono_vps",
  "cloud_admin",
  "cloud_racecontrol",
  "relay_james",
];

function loadJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function makeAjv() {
  // strict:false permits `format: date-time` with custom pattern constraint without warnings.
  // Draft 2020-12 is the entry point from ajv/dist/2020.js.
  const ajv = new Ajv2020({ allErrors: true, strict: false });
  addFormats(ajv);
  return ajv;
}

test("schema compiles cleanly under ajv 2020", () => {
  const schema = loadJson(SCHEMA_PATH);
  const ajv = makeAjv();
  const validate = ajv.compile(schema);
  assert.equal(typeof validate, "function", "ajv.compile must return a validator function");
});

test("schema declares additionalProperties:true at root (SCHEMA-02 forward-compat)", () => {
  const schema = loadJson(SCHEMA_PATH);
  assert.equal(schema.additionalProperties, true, "root additionalProperties must be true");
});

test("schema has no additionalProperties:false anywhere (forbidden everywhere)", () => {
  const txt = readFileSync(SCHEMA_PATH, "utf8");
  assert.equal(
    txt.includes('"additionalProperties": false'),
    false,
    "`additionalProperties: false` is forbidden in fleet-manifest.schema.json (breaks SCHEMA-02)"
  );
});

test("per-target examples folder has exactly 9 files (8 targets + _meta.json)", () => {
  const files = readdirSync(EXAMPLES_DIR).filter((f) => f.endsWith(".json"));
  assert.equal(files.length, 9, `expected 9 JSON files in schemas/examples/, got ${files.length}: ${files.join(",")}`);
});

for (const targetId of PER_TARGET_EXAMPLE_IDS) {
  test(`example validates: ${targetId}`, () => {
    const schema = loadJson(SCHEMA_PATH);
    const validate = makeAjv().compile(schema);
    const manifest = loadJson(join(EXAMPLES_DIR, `${targetId}.json`));
    const ok = validate(manifest);
    if (!ok) {
      const errs = (validate.errors || []).map((e) => `${e.instancePath} ${e.message}`).join("; ");
      assert.fail(`${targetId}.json failed validation: ${errs}`);
    }
    assert.equal(manifest.target_id, targetId, "target_id must match filename");
  });
}

test("forward-compat fixture valid-with-unknown-fields.json passes (SCHEMA-02 root + nested additionalProperties)", () => {
  const schema = loadJson(SCHEMA_PATH);
  const validate = makeAjv().compile(schema);
  const data = loadJson(join(FIXTURES_DIR, "valid-with-unknown-fields.json"));
  const ok = validate(data);
  if (!ok) {
    const errs = (validate.errors || []).map((e) => `${e.instancePath} ${e.message}`).join("; ");
    assert.fail(`forward-compat root+nested fixture failed: ${errs}`);
  }
  assert.equal(data.future_field_xyz.nested[2], 3, "fixture structure intact");
  assert.equal(data.running_procs[0].cpu_percent, 12.5, "nested unknown field present");
});

test("forward-compat fixture valid-schema-version-2.json passes (future schema_version accepted by v1 validator)", () => {
  const schema = loadJson(SCHEMA_PATH);
  const validate = makeAjv().compile(schema);
  const data = loadJson(join(FIXTURES_DIR, "valid-schema-version-2.json"));
  const ok = validate(data);
  if (!ok) {
    const errs = (validate.errors || []).map((e) => `${e.instancePath} ${e.message}`).join("; ");
    assert.fail(`schema_version=2.0 fixture failed — SCHEMA-02 broken: ${errs}`);
  }
  assert.equal(data.schema_version, "2.0");
});

test("negative fixture invalid-missing-required.json FAILS validation (missing env_vars_hash)", () => {
  const schema = loadJson(SCHEMA_PATH);
  const validate = makeAjv().compile(schema);
  const data = loadJson(join(FIXTURES_DIR, "invalid-missing-required.json"));
  const ok = validate(data);
  assert.equal(ok, false, "missing-required fixture must fail validation");
  const errorText = JSON.stringify(validate.errors || []);
  assert.ok(
    errorText.includes("env_vars_hash"),
    `errors must mention env_vars_hash; got: ${errorText}`
  );
});

test("negative fixture invalid-bad-enum.json FAILS validation (role not in enum)", () => {
  const schema = loadJson(SCHEMA_PATH);
  const validate = makeAjv().compile(schema);
  const data = loadJson(join(FIXTURES_DIR, "invalid-bad-enum.json"));
  const ok = validate(data);
  assert.equal(ok, false, "bad-enum fixture must fail validation");
  const errorText = JSON.stringify(validate.errors || []);
  assert.ok(
    errorText.includes("role") || errorText.includes("enum"),
    `errors must mention role or enum; got: ${errorText}`
  );
});

test("_meta.json cross-references resolve (every manifest_file entry is a real file)", () => {
  const meta = loadJson(join(EXAMPLES_DIR, "_meta.json"));
  assert.equal(meta.targets.length, 8, "_meta.json must list 8 targets");
  for (const t of meta.targets) {
    const path = join(EXAMPLES_DIR, t.manifest_file);
    assert.doesNotThrow(() => readFileSync(path), `missing referenced file: ${t.manifest_file}`);
  }
});
