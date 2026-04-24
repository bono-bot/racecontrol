// tests/fleet-probe/schema-compat.test.mjs -- Phase 448 Plan 01
// Asserts that every file in tests/fleet-probe/fixtures/ passes schema validation
// against schemas/fleet-manifest.schema.json, and that validateAgainstSchema catches
// objects missing required fields.
import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { loadFixture, validateAgainstSchema } from "./helpers.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const FIXTURES_DIR = resolve(__dirname, "fixtures");

const fixtureNames = readdirSync(FIXTURES_DIR)
  .filter((f) => f.endsWith(".json"))
  .map((f) => f.replace(/\.json$/, ""));

test("all fixtures are schema-valid", () => {
  assert.ok(fixtureNames.length >= 3, `expected >=3 fixtures, got ${fixtureNames.length}`);
  for (const name of fixtureNames) {
    const obj = loadFixture(name);
    const { valid, errors } = validateAgainstSchema(obj);
    assert.ok(valid, `${name} failed: ${JSON.stringify(errors)}`);
  }
});

test("validateAgainstSchema catches missing required fields", () => {
  const { valid, errors } = validateAgainstSchema({ target_id: "bogus" });
  assert.equal(valid, false);
  assert.ok(errors && errors.length > 0, "expected errors array to be non-empty");
});
