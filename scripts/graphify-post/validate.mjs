#!/usr/bin/env node
// racecontrol/scripts/graphify-post/validate.mjs
// Runs all gap-rules against graphify-out/graph.json and produces GAP_REPORT.md.
// Never modifies the graph. Exit 0 regardless — meant for observability, not gating.
//
// Usage: node scripts/graphify-post/validate.mjs [graphify-out-dir]

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
const __dirname = path.dirname(fileURLToPath(import.meta.url));

import RULES from './gap-rules.mjs';

const outDir = process.argv[2] || path.resolve(process.cwd(), 'graphify-out');
const graphPath = path.join(outDir, 'graph.json');
if (!fs.existsSync(graphPath)) {
  console.error(`ERROR: ${graphPath} does not exist.`);
  process.exit(1);
}

console.log(`Loading ${graphPath} ...`);
const g = JSON.parse(fs.readFileSync(graphPath, 'utf8'));
console.log(`  ${g.nodes.length} nodes, ${g.links.length} edges`);

const results = [];
for (const r of RULES) {
  let out;
  try { out = r.detect(g); }
  catch (err) { out = { hit:true, error: String(err) }; }
  results.push({ rule: r, result: out });
}

// Generate GAP_REPORT.md
const mdLines = [];
const now = new Date().toISOString();
const sevEmoji = { P0:'', P1:'', P2:'', P3:'' };
mdLines.push(`# Graphify Gap Report`);
mdLines.push(``);
mdLines.push(`Generated: ${now}  `);
mdLines.push(`Source: ${graphPath}  `);
mdLines.push(`Graph: ${g.nodes.length} nodes · ${g.links.length} edges · ${new Set(g.nodes.map(n=>n.community)).size} communities`);
mdLines.push(``);
mdLines.push(`## Summary`);
mdLines.push(``);
const hits = results.filter(r => r.result && r.result.hit);
const bySev = {};
for (const r of hits) bySev[r.rule.severity] = (bySev[r.rule.severity]||0)+1;
mdLines.push(`| Severity | Count |`);
mdLines.push(`|---|---|`);
for (const s of ['P0','P1','P2','P3']) mdLines.push(`| ${s} | ${bySev[s]||0} |`);
mdLines.push(``);
const byFix = {};
for (const r of hits) byFix[r.rule.fixability] = (byFix[r.rule.fixability]||0)+1;
mdLines.push(`| Fixability | Count |`);
mdLines.push(`|---|---|`);
for (const f of ['auto','warn','manual']) mdLines.push(`| ${f} | ${byFix[f]||0} |`);
mdLines.push(``);

// Per-rule detail
mdLines.push(`## Findings`);
mdLines.push(``);
for (const s of ['P0','P1','P2','P3']) {
  const inSev = results.filter(r => r.rule.severity === s);
  if (inSev.length === 0) continue;
  mdLines.push(`### ${s}`);
  mdLines.push(``);
  for (const {rule, result} of inSev) {
    const status = result && result.hit ? 'HIT' : 'PASS';
    mdLines.push(`#### ${rule.id} · ${status} · ${rule.fixability}`);
    mdLines.push(``);
    mdLines.push(`**${rule.title}**`);
    mdLines.push(``);
    if (result && result.hit) {
      mdLines.push(`<details><summary>Evidence</summary>`);
      mdLines.push('');
      mdLines.push('```json');
      mdLines.push(JSON.stringify(result, null, 2).slice(0, 4000));
      mdLines.push('```');
      mdLines.push('');
      mdLines.push(`</details>`);
    } else {
      mdLines.push(`No findings.`);
    }
    mdLines.push(``);
  }
}

// Append subsystem scorecard (best-effort — subsystems.mjs may not be importable if outDir is external)
try {
  const { SUBSYSTEMS, enumerateFilesystem, classifyNode } = await import('./subsystems.mjs');
  mdLines.push(`## Subsystem Coverage`);
  mdLines.push(``);
  mdLines.push(`| Subsystem | FS files | Indexed | Coverage | Communities | Max density | Edges (int/bnd) |`);
  mdLines.push(`|---|---|---|---|---|---|---|`);
  for (const [id, sub] of Object.entries(SUBSYSTEMS)) {
    const fsFiles = await enumerateFilesystem(sub);
    const subsystemPatterns = [...(sub.frontend_paths || []), ...(sub.backend_paths || [])];
    const relevantFs = fsFiles.filter(f => /\.(ts|tsx|rs|py|js|mjs)$/i.test(f) && subsystemPatterns.some(p => p.test(f.replace(/\\/g, '/'))));
    const nodes = [];
    for (const n of g.nodes) {
      const tags = classifyNode(n, { [id]: sub });
      if (tags.some(t => t.startsWith(id + ':'))) nodes.push(n);
    }
    const indexedFiles = new Set(nodes.map(n => (n.source_file||'').replace(/\\/g,'/'))).size;
    const coverage = relevantFs.length ? (indexedFiles / relevantFs.length * 100).toFixed(0) : 'n/a';
    const byC = new Set(nodes.map(n => n.community));
    const ids = new Set(nodes.map(n => n.id));
    let internal = 0, boundary = 0;
    for (const e of g.links) {
      const s = ids.has(e.source), t = ids.has(e.target);
      if (s && t) internal++;
      else if (s || t) boundary++;
    }
    // densest community
    const commSizes = new Map();
    for (const n of g.nodes) commSizes.set(n.community, (commSizes.get(n.community)||0)+1);
    const byCommCount = new Map();
    for (const n of nodes) byCommCount.set(n.community, (byCommCount.get(n.community)||0)+1);
    let maxDensity = 0;
    for (const [c, n] of byCommCount) {
      if ((commSizes.get(c)||0) < 20) continue;
      const d = n / commSizes.get(c);
      if (d > maxDensity) maxDensity = d;
    }
    mdLines.push(`| ${sub.label} | ${relevantFs.length} | ${indexedFiles} | ${coverage}% | ${byC.size} | ${(maxDensity*100).toFixed(1)}% | ${internal}/${boundary} |`);
  }
  mdLines.push(``);
  mdLines.push(`Full detail: [SUBSYSTEM_MAP.md](SUBSYSTEM_MAP.md) · run \`node scripts/graphify-post/subsystem-audit.mjs\` to regenerate.`);
  mdLines.push(``);
} catch (err) {
  mdLines.push(`> Subsystem coverage unavailable: ${err.message}`);
  mdLines.push(``);
}

const reportPath = path.join(outDir, 'GAP_REPORT.md');
fs.writeFileSync(reportPath, mdLines.join('\n'));
console.log(`\n${hits.length}/${results.length} rules HIT`);
console.log(`by severity: P0=${bySev.P0||0} P1=${bySev.P1||0} P2=${bySev.P2||0} P3=${bySev.P3||0}`);
console.log(`by fixability: auto=${byFix.auto||0} warn=${byFix.warn||0} manual=${byFix.manual||0}`);
console.log(`\nReport written: ${reportPath}`);

// Also emit a machine-readable summary for CI
const jsonSummary = {
  generated_at: now,
  graph_nodes: g.nodes.length,
  graph_edges: g.links.length,
  total_rules: results.length,
  hits: hits.length,
  by_severity: bySev,
  by_fixability: byFix,
  findings: results.map(r => ({
    id: r.rule.id,
    title: r.rule.title,
    severity: r.rule.severity,
    fixability: r.rule.fixability,
    hit: !!(r.result && r.result.hit),
    detail: r.result,
  })),
};
fs.writeFileSync(path.join(outDir, 'GAP_REPORT.json'), JSON.stringify(jsonSummary, null, 2));
console.log(`Machine-readable: ${path.join(outDir, 'GAP_REPORT.json')}`);
