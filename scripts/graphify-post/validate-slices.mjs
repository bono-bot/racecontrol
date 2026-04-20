#!/usr/bin/env node
// racecontrol/scripts/graphify-post/validate-slices.mjs
// Runs validate.mjs against every racecontrol/graphify-out-<slice>/ directory.
// Produces one GAP_REPORT.md per slice and prints a summary table.
//
// Not all 29 rules are meaningful at slice granularity — some rules (e.g. G1
// cross-community hubs, G27 cross-repo fetch edges) are inherently parent-scoped.
// Per-slice results are still useful for surfacing slice-local issues like
// disconnected tests (G4), missing source_location (G21), or community modularity.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
import { spawnSync } from 'child_process';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..', '..');  // …/racecontrol

const sliceDirs = fs.readdirSync(REPO_ROOT)
  .filter(n => n.startsWith('graphify-out-'))
  .map(n => path.join(REPO_ROOT, n))
  .filter(p => fs.existsSync(path.join(p, 'graph.json')));

if (sliceDirs.length === 0) {
  console.error('No graphify-out-* slice directories found.');
  console.error('Run: node graphify-meta/slice-submodules.mjs first.');
  process.exit(1);
}

console.log(`Found ${sliceDirs.length} slice(s). Running validate.mjs against each…\n`);

const summary = [];
for (const dir of sliceDirs) {
  const name = path.basename(dir).replace(/^graphify-out-/, '');
  const validatePath = path.join(__dirname, 'validate.mjs');
  const r = spawnSync(process.execPath, [validatePath, dir], { encoding: 'utf8' });
  if (r.status !== 0) {
    console.log(`  [${name}] FAILED — ${r.stderr.slice(0, 200)}`);
    summary.push({ name, status: 'FAIL', hits: '?', reason: r.stderr.split('\n')[0] });
    continue;
  }
  // Parse GAP_REPORT.json for numbers
  const reportJson = path.join(dir, 'GAP_REPORT.json');
  if (!fs.existsSync(reportJson)) {
    summary.push({ name, status: 'FAIL', hits: '?', reason: 'GAP_REPORT.json missing' });
    continue;
  }
  const rep = JSON.parse(fs.readFileSync(reportJson, 'utf8'));
  summary.push({
    name,
    status: 'OK',
    nodes: rep.graph_nodes,
    edges: rep.graph_edges,
    hits: rep.hits,
    p0: rep.by_severity.P0 || 0,
    p1: rep.by_severity.P1 || 0,
    p2: rep.by_severity.P2 || 0,
    p3: rep.by_severity.P3 || 0,
  });
  console.log(`  [${name.padEnd(14)}] ${rep.hits}/${rep.total_rules} rules hit (P0=${rep.by_severity.P0||0} P1=${rep.by_severity.P1||0} P2=${rep.by_severity.P2||0} P3=${rep.by_severity.P3||0})`);
}

// Print table summary
console.log('\nSummary:');
console.log('  Slice              Nodes  Edges  Hits  P0/P1/P2/P3');
for (const s of summary) {
  if (s.status !== 'OK') {
    console.log(`  ${s.name.padEnd(18)} FAIL: ${s.reason}`);
    continue;
  }
  console.log(
    `  ${s.name.padEnd(18)} ${String(s.nodes).padStart(5)}  ${String(s.edges).padStart(5)}  ${String(s.hits).padStart(4)}  ${s.p0}/${s.p1}/${s.p2}/${s.p3}`
  );
}

// Emit combined JSON summary
const combinedPath = path.join(REPO_ROOT, 'graphify-meta', 'SLICE_GAP_SUMMARY.json');
fs.mkdirSync(path.dirname(combinedPath), { recursive: true });
fs.writeFileSync(combinedPath, JSON.stringify({
  generated_at: new Date().toISOString(),
  slices: summary,
}, null, 2));
console.log(`\nCombined summary: ${path.relative(REPO_ROOT, combinedPath)}`);
