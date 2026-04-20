#!/usr/bin/env node
// racecontrol/scripts/graphify-post/subsystem-audit.mjs
// Per-subsystem coverage audit: compares filesystem truth against graph.json indexing.
// Produces SUBSYSTEM_MAP.md with per-subsystem scorecard.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';
const __dirname = path.dirname(fileURLToPath(import.meta.url));

import { SUBSYSTEMS, enumerateFilesystem, classifyNode } from './subsystems.mjs';

const outDir = process.argv[2] || 'graphify-out';
const graphPath = path.join(outDir, 'graph.json');
if (!fs.existsSync(graphPath)) { console.error(`ERROR: ${graphPath} missing`); process.exit(1); }

const g = JSON.parse(fs.readFileSync(graphPath, 'utf8'));
console.log(`Loaded ${g.nodes.length} nodes, ${g.links.length} edges`);

// Build adjacency for community-lookup
const nodeComm = new Map();
for (const n of g.nodes) nodeComm.set(n.id, n.community);

const report = [];
report.push(`# Subsystem Map Audit`);
report.push(``);
report.push(`Generated: ${new Date().toISOString()}`);
report.push(`Graph: ${g.nodes.length} nodes · ${g.links.length} edges`);
report.push(``);

// Summary table
report.push(`## Scorecard`);
report.push(``);
report.push(`| Subsystem | FS files | Indexed | Coverage | Communities | Max density | Boundary/Internal edges | Status |`);
report.push(`|---|---|---|---|---|---|---|---|`);

const summaries = [];
for (const [id, sub] of Object.entries(SUBSYSTEMS)) {
  // Filesystem truth — walk repo_root, then filter to files that match subsystem patterns
  const fsFiles = await enumerateFilesystem(sub);
  const subsystemPatterns = [...(sub.frontend_paths || []), ...(sub.backend_paths || [])];
  const relevantFs = fsFiles.filter(f => {
    if (!/\.(ts|tsx|rs|py)$/i.test(f)) return false;
    const fNorm = f.replace(/\\/g,'/');
    return subsystemPatterns.some(p => p.test(fNorm));
  });

  // Indexed nodes
  const nodes = [];
  for (const n of g.nodes) {
    const tags = classifyNode(n, { [id]: sub });
    if (tags.some(t => t.startsWith(id + ':'))) nodes.push(n);
  }

  // Distinct source_files indexed
  const indexedFiles = new Set(nodes.map(n => (n.source_file||'').replace(/\\/g,'/')));
  const coverage = relevantFs.length ? (indexedFiles.size / relevantFs.length * 100) : 0;

  // Communities
  const byC = new Map();
  for (const n of nodes) byC.set(n.community, (byC.get(n.community)||0)+1);
  const commSizes = new Map();
  for (const n of g.nodes) commSizes.set(n.community, (commSizes.get(n.community)||0)+1);
  let maxDensity = 0, maxDensityComm = null;
  for (const [c, n] of byC) {
    if ((commSizes.get(c)||0) < 20) continue;
    const d = n / commSizes.get(c);
    if (d > maxDensity) { maxDensity = d; maxDensityComm = c; }
  }

  // Edges
  const ids = new Set(nodes.map(n => n.id));
  let internal = 0, boundary = 0;
  for (const e of g.links) {
    const s = ids.has(e.source), t = ids.has(e.target);
    if (s && t) internal++;
    else if (s || t) boundary++;
  }

  // Status
  let status = '';
  if (coverage < 30) status = `MISSING — scan scope gap`;
  else if (maxDensity < 0.25) status = `SCATTERED — no coherent cluster`;
  else if (boundary / Math.max(internal, 1) > 2.5) status = `BRIDGE-HEAVY — consumers not indexed`;
  else status = `OK`;

  summaries.push({ id, sub, fsCount: relevantFs.length, indexed: indexedFiles.size, nodes, coverage, byC, maxDensity, maxDensityComm, internal, boundary, status, relevantFs });

  report.push(`| ${sub.label} | ${relevantFs.length} | ${indexedFiles.size} | ${coverage.toFixed(0)}% | ${byC.size} | ${maxDensity ? `${(maxDensity*100).toFixed(1)}% (c${maxDensityComm})` : 'n/a'} | ${boundary}/${internal} | ${status} |`);
}
report.push(``);

// Per-subsystem detail
for (const s of summaries) {
  report.push(`## ${s.sub.label}`);
  report.push(``);
  report.push(`**Purpose:** ${s.sub.purpose}`);
  report.push(``);
  report.push(`**Repo:** \`${s.sub.repo_root}\``);
  report.push(``);
  report.push(`**Coverage:** ${s.indexed} of ${s.fsCount} filesystem source files (${s.coverage.toFixed(1)}%)`);
  report.push(``);
  report.push(`**Community distribution:** ${s.byC.size} communities touched`);
  report.push(``);
  report.push(`| Community | Nodes in subsystem | Community size | Density |`);
  report.push(`|---|---|---|---|`);
  const sortedC = [...s.byC.entries()].sort((a,b)=>b[1]-a[1]).slice(0,8);
  for (const [c, n] of sortedC) {
    const commSize = [...s.nodes].reduce((acc, node) => acc + (nodeComm.get(node.id) === c ? 1 : 0), 0);
    // get full community size
    const fullSize = g.nodes.filter(x => x.community === c).length;
    const density = fullSize ? (n / fullSize * 100).toFixed(1) + '%' : 'n/a';
    report.push(`| c${c} | ${n} | ${fullSize} | ${density} |`);
  }
  report.push(``);
  report.push(`**Edge topology:** ${s.internal} internal · ${s.boundary} boundary · ratio ${(s.boundary/Math.max(s.internal,1)).toFixed(2)}`);
  report.push(``);

  // Missing files (in FS but not in graph)
  const indexed = new Set(s.nodes.map(n => (n.source_file||'').replace(/\\/g,'/').toLowerCase()));
  const missingFiles = s.relevantFs.filter(f => {
    const nf = f.toLowerCase();
    // Check if any indexed file path ends with a matching relative path
    return ![...indexed].some(ix => ix.endsWith(nf.split('racingpoint-admin/').slice(-1)[0]) || ix.endsWith(nf.split('racecontrol/').slice(-1)[0]));
  });
  if (missingFiles.length > 0) {
    report.push(`**Filesystem files NOT in graph (${missingFiles.length}):**`);
    report.push('');
    report.push('<details><summary>Show missing files</summary>');
    report.push('');
    for (const f of missingFiles.slice(0, 50)) {
      report.push(`- ${f.replace(s.sub.repo_root, '')}`);
    }
    if (missingFiles.length > 50) report.push(`- … and ${missingFiles.length - 50} more`);
    report.push('');
    report.push('</details>');
    report.push('');
  }

  // Recommendation
  report.push(`**Recommended action:**`);
  if (s.coverage < 30) {
    report.push(`- **Scope-expand graphify.** Run \`/graphify ${s.sub.repo_root}\` or add the repo to the parent-directory scan.`);
    report.push(`- Code-only files trigger AST-only extraction (no LLM cost) via \`/graphify <path> --update\`.`);
  } else if (s.maxDensity < 0.25) {
    report.push(`- **No community override possible in Louvain** — nodes scatter because the subsystem's internal edge density is low relative to cross-cutting calls.`);
    report.push(`- Consider graphify's \`--cluster-only\` after adding subsystem-scope hints.`);
  } else if (s.boundary / Math.max(s.internal, 1) > 2.5) {
    report.push(`- **Indirect consumers missing** — frontend→backend edges are invisible because fetch() URL strings aren't AST-extractable. Needs \`--mode deep\` semantic extraction.`);
  } else {
    report.push(`- Coverage OK. Monitor for drift on next rebuild.`);
  }
  report.push(``);
}

const reportPath = path.join(outDir, 'SUBSYSTEM_MAP.md');
fs.writeFileSync(reportPath, report.join('\n'));
console.log(`\nWrote ${reportPath}`);

// Also emit console summary
console.log('\n=== SCORECARD ===');
for (const s of summaries) {
  console.log(`${s.sub.label.padEnd(20)} coverage=${s.coverage.toFixed(0).padStart(3)}%  communities=${s.byC.size.toString().padStart(2)}  maxDensity=${(s.maxDensity*100).toFixed(1).padStart(5)}%  edges=${s.internal}/${s.boundary}  ${s.status}`);
}
