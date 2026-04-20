#!/usr/bin/env node
// graphify-meta/smoke-test.mjs
// Regression-test the meta-map pipeline end-to-end. Runs each stage and asserts that
// expected artifacts exist + meet minimum quality thresholds (non-zero edge counts,
// no absolute paths, no duplicate backend_files, no prose-artifact table names).
//
// Exit 0 = all assertions pass. Exit 1 = something regressed.
//
// Run after any change to api-edges.mjs / db-edges.mjs / ws-edges.mjs / build-meta.mjs /
// slice-submodules.mjs to catch regressions before pushing.

import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

let failures = 0;
function assert(cond, msg) {
  if (cond) {
    console.log(`  PASS  ${msg}`);
  } else {
    console.log(`  FAIL  ${msg}`);
    failures++;
  }
}
function section(name) {
  console.log(`\n[${name}]`);
}

// ---------------------------------------------------------------------------
// Stage 1: run all 4 scripts, capture their exit codes.
// ---------------------------------------------------------------------------

const scripts = ['slice-submodules.mjs', 'api-edges.mjs', 'db-edges.mjs', 'ws-edges.mjs', 'build-meta.mjs'];
section('Run pipeline');
for (const s of scripts) {
  const r = spawnSync(process.execPath, [path.join(__dirname, s)], { cwd: __dirname, encoding: 'utf8' });
  assert(r.status === 0, `${s} exit=${r.status}`);
  if (r.status !== 0) {
    console.log(`      stderr: ${(r.stderr || '').slice(0, 200)}`);
  }
}

// ---------------------------------------------------------------------------
// Stage 2: artifact existence
// ---------------------------------------------------------------------------

section('Artifact existence');
const artifacts = ['api-edges.json', 'db-edges.json', 'ws-edges.json', 'meta-graph.json', 'meta.html', 'META_REPORT.md', 'slices.json'];
for (const a of artifacts) {
  assert(fs.existsSync(path.join(__dirname, a)), `${a} exists`);
}

// ---------------------------------------------------------------------------
// Stage 3: quality assertions
// ---------------------------------------------------------------------------

section('api-edges.json quality');
const api = JSON.parse(fs.readFileSync(path.join(__dirname, 'api-edges.json'), 'utf8'));
assert(api.edges.length >= 15, `>=15 edges (got ${api.edges.length})`);
assert(api.edges.some(e => e.direction === 'back-edge'), 'at least one back-edge');
assert(api.edges.some(e => ['GET', 'POST', 'PUT', 'DELETE'].includes(e.verb)), 'at least one non-UNKNOWN verb');
// No duplicate backend_file within any single edge's matched_routes
const withDupes = api.edges.filter(e => {
  const mrs = e.matched_routes || [];
  const files = mrs.map(mr => mr.backend_file).filter(Boolean);
  return files.length > 0 && new Set(files).size < files.length;
});
assert(withDupes.length === 0, `no duplicate backend_file within matched_routes (got ${withDupes.length})`);
// No absolute Windows paths
const absPaths = api.edges.filter(e =>
  (e.matched_routes || []).some(mr => /^[A-Z]:[\/\\]/.test(mr.backend_file || ''))
);
assert(absPaths.length === 0, `no absolute Windows paths (got ${absPaths.length})`);

section('db-edges.json quality');
const db = JSON.parse(fs.readFileSync(path.join(__dirname, 'db-edges.json'), 'utf8'));
assert(db.edges.length >= 1, `>=1 shared-table edge (got ${db.edges.length})`);
// Must NOT contain prose-artifact tables from Python imports
const allSharedTables = new Set();
db.edges.forEach(e => e.shared_tables.forEach(s => allSharedTables.add(s.table)));
const forbidden = ['the', 'a', 'an', 'datetime', 'pathlib', 'collections', 'user', 'admin', 'active'];
const leaked = forbidden.filter(t => allSharedTables.has(t));
assert(leaked.length === 0, `no stopword tables in shared_tables (got ${leaked.join(',')})`);

section('ws-edges.json quality');
const ws = JSON.parse(fs.readFileSync(path.join(__dirname, 'ws-edges.json'), 'utf8'));
assert(typeof ws.per_module_variant_counts === 'object', 'per_module_variant_counts present');
// After round-4 split: backend WS variants live under `racecontrol.crates` (slice-aware).
const rcVariantCount = ws.per_module_variant_counts['racecontrol.crates'] ?? ws.per_module_variant_counts.racecontrol ?? 0;
assert(rcVariantCount > 100, `racecontrol(.crates) has many variants (got ${rcVariantCount})`);

section('meta-graph.json quality');
const meta = JSON.parse(fs.readFileSync(path.join(__dirname, 'meta-graph.json'), 'utf8'));
assert(meta.modules.length >= 16, `>=16 modules registered (got ${meta.modules.length})`);
assert(meta.edges.length >= 5, `>=5 inter-module edges (got ${meta.edges.length})`);
const edgeTypes = new Set(meta.edges.map(e => e.type));
assert(edgeTypes.has('api-call'), 'api-call edges present');
assert(edgeTypes.has('shared-db-table') || ws.edges.length === 0, 'shared-db-table edges present OR ws has none');

section('per-slice artifacts');
const slices = JSON.parse(fs.readFileSync(path.join(__dirname, 'slices.json'), 'utf8')).slices;
assert(slices.length === 8, `8 slices in manifest (got ${slices.length})`);
const ROOT = path.resolve(__dirname, '..', '..');
for (const s of slices) {
  const gp = path.join(ROOT, s.graph_path);
  const hp = path.join(ROOT, s.html_path);
  assert(fs.existsSync(gp), `slice ${s.id} graph.json`);
  assert(fs.existsSync(hp), `slice ${s.id} graph.html`);
}

// ---------------------------------------------------------------------------
// Summary
// ---------------------------------------------------------------------------

console.log(`\n${failures === 0 ? 'OK' : 'FAIL'} — ${failures} failure(s)`);
process.exit(failures === 0 ? 0 : 1);
