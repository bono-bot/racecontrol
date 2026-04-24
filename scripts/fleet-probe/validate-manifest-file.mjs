#!/usr/bin/env node
// scripts/fleet-probe/validate-manifest-file.mjs -- Phase 448 Plan 01
// CLI wrapper around Phase 447 ajv validator.
// Usage: node scripts/fleet-probe/validate-manifest-file.mjs <manifest.json>
// Exits: 0 valid | 1 invalid (errors printed to stderr) | 2 usage/IO error
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import Ajv2020 from "ajv/dist/2020.js";
import addFormats from "ajv-formats";

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const REPO_ROOT = resolve(__dirname, "..", "..");
const SCHEMA_PATH = resolve(REPO_ROOT, "schemas", "fleet-manifest.schema.json");

const manifestPath = process.argv[2];
if (!manifestPath) {
  console.error("Usage: validate-manifest-file.mjs <manifest.json>");
  process.exit(2);
}

let schema, manifest;
try {
  schema = JSON.parse(readFileSync(SCHEMA_PATH, "utf8"));
  manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
} catch (e) {
  console.error(`IO/parse error: ${e.message}`);
  process.exit(2);
}

const ajv = new Ajv2020({ allErrors: true, strict: false });
addFormats(ajv);
const validate = ajv.compile(schema);
if (validate(manifest)) {
  process.exit(0);
}
for (const err of validate.errors) {
  console.error(`  ${err.instancePath || "/"}: ${err.message}`);
}
process.exit(1);
