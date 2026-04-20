#!/usr/bin/env node
// racecontrol/scripts/graphify-post/repair.mjs
// Applies auto-fix rules to a COPY of graph.json → writes graph.repaired.json.
// Never overwrites graph.json. Separately patches each community_*.html to enable
// the legend (G11) by mapping node.group = node.community.
//
// Usage:
//   node scripts/graphify-post/repair.mjs [graphify-out-dir]
//
// To apply for real, operator renames graph.repaired.json → graph.json after review.

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
const origSize = fs.statSync(graphPath).size;
console.log(`  ${g.nodes.length} nodes, ${g.links.length} edges, ${(origSize/1024/1024).toFixed(1)}MB`);

// G11 RETRACTED — per-community legend works as designed, was a misdiagnosis.
const changes = [];
for (const r of RULES) {
  if (r.fixability !== 'auto' || !r.apply) continue;
  try {
    const result = r.apply(g);
    changes.push({ rule: r.id, title: r.title, result });
    console.log(`  [${r.id}] ${r.title} → ${JSON.stringify(result)}`);
  } catch (err) {
    console.error(`  [${r.id}] FAILED: ${err}`);
    changes.push({ rule: r.id, title: r.title, error: String(err) });
  }
}

const repairedPath = path.join(outDir, 'graph.repaired.json');
fs.writeFileSync(repairedPath, JSON.stringify(g, null, 0));
const newSize = fs.statSync(repairedPath).size;
console.log(`\nWrote ${repairedPath}`);
console.log(`  size: ${(origSize/1024/1024).toFixed(1)}MB → ${(newSize/1024/1024).toFixed(1)}MB  (Δ ${((newSize-origSize)/1024).toFixed(0)}KB)`);

// ============================================================================
// HTML community file patch — G11: enable legend by setting group = community
// ============================================================================
const commDir = path.join(outDir, 'communities');
if (fs.existsSync(commDir)) {
  const htmls = fs.readdirSync(commDir).filter(f => /^community_\d+\.html$/.test(f));
  console.log(`\nPatching ${htmls.length} community HTML files (G11 legend fix) ...`);
  let patched = 0;
  for (const fname of htmls) {
    const p = path.join(commDir, fname);
    const src = fs.readFileSync(p, 'utf8');
    const m = src.match(/const\s+RAW_NODES\s*=\s*(\[[\s\S]*?\])\s*;/m);
    if (!m) continue;
    let arr;
    try { arr = JSON.parse(m[1]); } catch { continue; }
    // Inject group = file_type so the per-community legend is meaningful
    // (group=community would give 1 entry since this file IS one community).
    // Also fall back to first source-dir segment if file_type is missing.
    let changed = 0;
    for (const n of arr) {
      if (n.group === undefined) {
        const sf = (n.source_file || '').replace(/\\/g,'/');
        const topDir = sf.split('/').filter(Boolean)[0] || 'unknown';
        n.group = n.file_type || topDir;
        changed++;
      }
    }
    if (changed > 0) {
      const outHtml = src.replace(m[0], `const RAW_NODES = ${JSON.stringify(arr)};`);
      const repairedHtmlPath = p.replace(/\.html$/, '.repaired.html');
      fs.writeFileSync(repairedHtmlPath, outHtml);
      patched++;
    }
  }
  console.log(`  wrote ${patched} *.repaired.html files (side-by-side, originals preserved)`);
}

// Write a CHANGES.md next to the repaired graph
const changesPath = path.join(outDir, 'GAP_REPAIR_CHANGES.md');
const lines = ['# Gap Repair Changes', '', `Generated: ${new Date().toISOString()}`, '',
  '| Rule | Title | Effect |', '|---|---|---|'];
for (const c of changes) {
  lines.push(`| ${c.rule} | ${c.title} | ${JSON.stringify(c.result || c.error)} |`);
}
lines.push('');
lines.push('**How to apply:** review `graph.repaired.json` + `communities/*.repaired.html`.');
lines.push('When satisfied, copy over the originals:');
lines.push('```');
lines.push(`  mv graph.json graph.json.bak-$(date -u +%Y%m%d-%H%M%S)`);
lines.push(`  mv graph.repaired.json graph.json`);
lines.push(`  for f in communities/*.repaired.html; do mv "$f" "${'\${f%.repaired.html}.html'}"; done`);
lines.push('```');
fs.writeFileSync(changesPath, lines.join('\n'));
console.log(`\nChange log: ${changesPath}`);
console.log(`\nDONE. Review ${repairedPath} before replacing live graph.json.`);
