#!/usr/bin/env node
// scripts/graphify-benchmark/bench.mjs — P5.4 MVP.
//
// Runs graphify on scripts/graphify-benchmark/fixture/ and scores the emitted
// graph.json against reference-graph.json. Produces:
//   - stdout: per-category coverage + overall
//   - audit/results/graphify-benchmark-YYYY-MM-DD.md: dated markdown report
//   - audit/results/graphify-benchmark-YYYY-MM-DD.json: machine-readable
//
// Coverage per category = (matched edges / expected edges).
// "Matched" = an emitted edge whose source+target+relation triple matches the
// reference. Tolerant: relation-name synonyms (e.g. `calls` / `invokes`) count.
//
// Exit 0 always — this is a measurement tool, not a gate.

import fs from 'node:fs';
import path from 'node:path';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const BENCH_ROOT = __dirname;
const FIXTURE_DIR = path.join(BENCH_ROOT, 'fixture');
const REPO_ROOT = path.resolve(__dirname, '..', '..');
const REFERENCE = JSON.parse(fs.readFileSync(path.join(BENCH_ROOT, 'reference-graph.json'), 'utf-8'));

// Relation name synonyms — different graphify versions / extractors name things
// differently. Treat these as interchangeable when matching.
const RELATION_SYNONYMS = {
  calls: new Set(['calls', 'invokes', 'fn_call', 'function_call']),
  reads_field: new Set(['reads_field', 'field_read', 'reads', 'accesses_field']),
  writes_field: new Set(['writes_field', 'field_write', 'writes', 'assigns_field']),
  spawns_process: new Set(['spawns_process', 'spawn', 'exec', 'subprocess']),
  reads_file: new Set(['reads_file', 'file_read', 'fs_read', 'reads']),
  dispatched_to: new Set(['dispatched_to', 'dyn_dispatch', 'trait_impl']),
  calls_macro: new Set(['calls_macro', 'macro_call', 'calls']),
};

function relationMatch(expectedRel, actualRel) {
  if (expectedRel === actualRel) return true;
  const syns = RELATION_SYNONYMS[expectedRel];
  return syns ? syns.has(actualRel) : false;
}

function labelMatch(expected, actualNode) {
  if (!actualNode) return false;
  const label = (actualNode.label || actualNode.id || '').toLowerCase();
  const norm = (actualNode.norm_label || '').toLowerCase();
  const want = expected.toLowerCase();
  // Accept paren-suffixed labels ("foo()" matches "foo"), field-notation ("Config.name" matches "name")
  const wantBase = want.replace(/\(\)$/, '').replace(/^.*\./, '');
  return label === want || norm === want
      || label === want + '()' || label === '.' + wantBase
      || label === wantBase || norm === wantBase
      || label.endsWith('::' + wantBase) || label.includes(want);
}

function findNode(graph, wantLabel) {
  return graph.nodes.find(n => labelMatch(wantLabel, n));
}

// Returns {matched: bool, directionReversed: bool}. G16 gap (caller/callee
// direction lost) is measured separately: we count an edge as "matched content"
// when either direction matches the reference, and flag direction-reversed
// matches so the report can show the G16 gap size.
function edgeMatches(expected, actualEdges, nodesById) {
  for (const e of actualEdges) {
    const src = nodesById[e.source];
    const tgt = nodesById[e.target];
    if (!src || !tgt) continue;
    if (!relationMatch(expected.relation, e.relation || 'calls')) continue;
    const forward = labelMatch(expected.source, src) && labelMatch(expected.target, tgt);
    const reverse = labelMatch(expected.source, tgt) && labelMatch(expected.target, src);
    if (forward) return { matched: true, directionReversed: false, source: 'graphify' };
    if (reverse) return { matched: true, directionReversed: true, source: 'graphify' };
  }
  return { matched: false, directionReversed: false };
}

// Match expected edge against a static-extras / cross-process edge list.
// Those edges carry raw source/target strings (not node IDs) so we label-match
// directly. Returns {matched, source: tag} for reporting attribution.
function edgeMatchesExtras(expected, extraEdges, tag) {
  // extras carry raw strings (not node objects) — adapt to a fake-node shape
  // labelMatch can consume.
  const toNode = (s) => ({ label: s || '', norm_label: s || '' });
  for (const e of extraEdges) {
    if (!relationMatch(expected.relation, e.relation || 'calls')) continue;
    const srcStr = e.source_label || e.source || '';
    const tgtStr = e.target_label || e.target || '';
    if (labelMatch(expected.source, toNode(srcStr)) &&
        labelMatch(expected.target, toNode(tgtStr))) {
      return { matched: true, directionReversed: false, source: tag };
    }
  }
  return { matched: false, directionReversed: false };
}

// ─── 1. Run graphify on the fixture ───────────────────────────────────────
console.log('[bench] running graphify on fixture/...');
try {
  execSync('graphify update .', { cwd: FIXTURE_DIR, stdio: ['ignore', 'pipe', 'pipe'] });
} catch (err) {
  // graphify can exit non-zero when viz build fails but still writes graph.json.
  // Only bail if graph.json wasn't produced.
  console.error(`[bench] graphify exited with error: ${err.message.slice(0, 200)}`);
}

const graphPath = path.join(FIXTURE_DIR, 'graphify-out', 'graph.json');
if (!fs.existsSync(graphPath)) {
  console.error(`[bench] fatal: ${graphPath} not produced. Is graphifyy installed? Check: graphify --help`);
  process.exit(1);
}

const actual = JSON.parse(fs.readFileSync(graphPath, 'utf-8'));
const actualEdges = actual.links || actual.edges || [];
const nodesById = {};
for (const n of actual.nodes) nodesById[n.id] = n;

console.log(`[bench] graphify output: ${actual.nodes.length} nodes, ${actualEdges.length} edges`);

// Load sibling extras (static-extras + cross-process) if present. These are
// optional inputs — the benchmark runs without them and reports graphify-only
// coverage; present-but-empty files just add zero.
function loadExtras(p, tag) {
  try {
    const j = JSON.parse(fs.readFileSync(p, 'utf-8'));
    const edges = j.edges || [];
    console.log(`[bench] ${tag} loaded: ${edges.length} edges from ${p}`);
    return edges;
  } catch {
    console.log(`[bench] ${tag} not present at ${p} (skipping)`);
    return [];
  }
}
const extrasEdges = loadExtras(path.join(REPO_ROOT, 'graphify-out-static-extras', 'edges.json'), 'static-extras');
const crossProcessEdges = loadExtras(path.join(REPO_ROOT, 'graphify-out-cross-process', 'edges.json'), 'cross-process');

// ─── 2. Score each category ───────────────────────────────────────────────
const results = {};
let totalExpected = 0;
let totalMatched = 0;
let totalDirectionReversed = 0;
let totalByGraphify = 0;
let totalByExtras = 0;

for (const [cat, def] of Object.entries(REFERENCE.edge_categories)) {
  const expected = def.expected_edges;
  const edgeResults = expected.map(e => {
    // Try graphify native first, then extras, then cross-process.
    let r = edgeMatches(e, actualEdges, nodesById);
    if (!r.matched) r = edgeMatchesExtras(e, extrasEdges, 'static-extras');
    if (!r.matched) r = edgeMatchesExtras(e, crossProcessEdges, 'cross-process');
    return { expected: e, ...r };
  });
  const matched = edgeResults.filter(r => r.matched);
  const reversed = matched.filter(r => r.directionReversed);
  const byGraphify = matched.filter(r => r.source === 'graphify').length;
  const byExtras = matched.filter(r => r.source === 'static-extras' || r.source === 'cross-process').length;
  const coverage = expected.length ? matched.length / expected.length : 1;
  results[cat] = {
    description: def.description,
    expected: expected.length,
    matched: matched.length,
    direction_reversed: reversed.length,
    by_graphify: byGraphify,
    by_extras: byExtras,
    coverage,
    missing: edgeResults.filter(r => !r.matched).map(r => r.expected),
  };
  totalExpected += expected.length;
  totalMatched += matched.length;
  totalDirectionReversed += reversed.length;
  totalByGraphify += byGraphify;
  totalByExtras += byExtras;
}

const overall = totalExpected ? totalMatched / totalExpected : 0;
const g16GapSize = totalExpected ? totalDirectionReversed / totalMatched : 0;

// ─── 3. Report ────────────────────────────────────────────────────────────
const ts = new Date().toISOString().slice(0, 10);
const OUT_MD = path.join(REPO_ROOT, 'audit', 'results', `graphify-benchmark-${ts}.md`);
const OUT_JSON = path.join(REPO_ROOT, 'audit', 'results', `graphify-benchmark-${ts}.json`);
fs.mkdirSync(path.dirname(OUT_MD), { recursive: true });

const md = [];
md.push(`# Graphify Benchmark — ${ts}`);
md.push('');
md.push(`Fixture: \`scripts/graphify-benchmark/fixture/\` — hand-authored known-answer Rust.`);
md.push(`graphify version: ${(() => { try { return execSync('graphify --help', { encoding: 'utf-8', stdio: ['ignore', 'pipe', 'ignore'] }).split('\n')[0].trim(); } catch { return 'unknown'; } })()}`);
md.push(`Emitted: ${actual.nodes.length} nodes, ${actualEdges.length} edges.`);
md.push('');
md.push(`## Overall coverage: **${(overall * 100).toFixed(1)}%** (${totalMatched} / ${totalExpected})`);
md.push('');
md.push(`Of the ${totalMatched} matched edges, **${totalDirectionReversed}** had reversed source/target direction (G16 gap). Content caught, direction lost.`);
md.push('');
md.push(`| Category | Expected | Matched | By graphify | By extras | Dir-reversed | Coverage |`);
md.push(`|---|---|---|---|---|---|---|`);
for (const [cat, r] of Object.entries(results)) {
  md.push(`| ${cat} | ${r.expected} | ${r.matched} | ${r.by_graphify} | ${r.by_extras} | ${r.direction_reversed} | ${(r.coverage * 100).toFixed(1)}% |`);
}
md.push('');
md.push(`Attribution: ${totalByGraphify} edges from pure-AST graphify, ${totalByExtras} from Phase 5 post-processors (static-extras + cross-process).`);
md.push('');
md.push('## Missing edges (per category)');
md.push('');
for (const [cat, r] of Object.entries(results)) {
  if (r.missing.length === 0) continue;
  md.push(`### ${cat}`);
  md.push('');
  md.push(`> ${r.description}`);
  md.push('');
  for (const e of r.missing) {
    md.push(`- \`${e.source}\` --[${e.relation}]-> \`${e.target}\``);
  }
  md.push('');
}
md.push('## Interpretation');
md.push('');
md.push(`- Overall **${(overall * 100).toFixed(0)}%** is the measured coverage on the fixture. This is NOT the same as coverage on racecontrol — fixture is intentionally shaped to expose gap categories.`);
md.push(`- Category A (function calls) is what pure-AST graphify is built for; expect ≥90%.`);
md.push(`- Categories B/C/D/E are the known gaps (G3/G6/G16/G21). Low coverage here is EXPECTED until upstream PRs / P5.1 / P5.2 land.`);
md.push(`- To measure racecontrol-scale coverage, expand the fixture + reference or author a second reference for a real-code slice.`);
md.push('');
md.push('## Replay');
md.push('');
md.push('```bash');
md.push('node scripts/graphify-benchmark/bench.mjs');
md.push('```');

fs.writeFileSync(OUT_MD, md.join('\n') + '\n');
fs.writeFileSync(OUT_JSON, JSON.stringify({ date: ts, overall, total_expected: totalExpected, total_matched: totalMatched, results }, null, 2));

console.log(`\n=== Graphify Benchmark — ${ts} ===`);
for (const [cat, r] of Object.entries(results)) {
  const bar = '█'.repeat(Math.round(r.coverage * 20)).padEnd(20, '░');
  console.log(`  ${cat.padEnd(25)} ${bar} ${r.matched}/${r.expected} (${(r.coverage * 100).toFixed(1)}%)`);
}
const obar = '█'.repeat(Math.round(overall * 20)).padEnd(20, '░');
console.log(`  ${'OVERALL'.padEnd(25)} ${obar} ${totalMatched}/${totalExpected} (${(overall * 100).toFixed(1)}%)`);
console.log(`\nReports:`);
console.log(`  ${OUT_MD}`);
console.log(`  ${OUT_JSON}`);
