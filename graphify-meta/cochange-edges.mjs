#!/usr/bin/env node
// graphify-meta/cochange-edges.mjs
// Mine git history to find file-pairs that ship together — a recall oracle for the
// static scanners (api/db/ws/label). Files that co-change frequently across N commits
// share implicit coupling (shared data contract, protocol, sentinel, config, etc.)
// even when no static link exists.
//
// Scope: intra-racecontrol (kiosk + web + pwa + crates/*). Cross-repo co-change would
// require timestamp-window correlation across 3 independent repos — stretch goal.
//
// Output:
//   cochange-edges.json — per-module-pair co-change counts + top file pairs + diff vs
//                         meta-graph.json (which pairs have NO detected edge).
//   cochange-gaps.md   — human-readable list of suspected hidden coupling.
//
// Threshold: pairs that co-changed in >= MIN_COMMITS distinct commits are kept.
// Commits touching > MAX_FILES are skipped (typically format/refactor/rename sweeps
// that create spurious co-change signal).

import fs from 'fs';
import path from 'path';
import { spawnSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');  // …/racecontrol

const SINCE    = process.env.COCHANGE_SINCE || '2 years ago';
const MAX_FILES = +(process.env.COCHANGE_MAX_FILES || 40);   // skip mega-commits
const MIN_COMMITS = +(process.env.COCHANGE_MIN_COMMITS || 3); // keep pairs changed together >= N

// ---------------------------------------------------------------------------
// 1. Slurp git log — one record per commit with file list
// ---------------------------------------------------------------------------

console.log(`[1/4] git log --since="${SINCE}" ...`);
const r = spawnSync('git', [
  '-C', REPO_ROOT,
  'log',
  `--since=${SINCE}`,
  '--no-merges',          // skip merge commits (inflate co-change signal)
  '--name-only',
  '--pretty=format:|||%H',
], { encoding: 'utf8', maxBuffer: 200 * 1024 * 1024 });

if (r.status !== 0) {
  console.error('git log failed:', r.stderr);
  process.exit(1);
}

const commits = [];
let cur = null;
for (const line of r.stdout.split('\n')) {
  if (line.startsWith('|||')) {
    if (cur) commits.push(cur);
    cur = { hash: line.slice(3), files: [] };
  } else if (line.trim() && cur) {
    cur.files.push(line.trim());
  }
}
if (cur) commits.push(cur);

const rawCommits = commits.length;
const kept = commits.filter(c => c.files.length > 0 && c.files.length <= MAX_FILES);
console.log(`   ${rawCommits} commits found, ${kept.length} kept (skipped ${rawCommits - kept.length} mega/empty)`);

// ---------------------------------------------------------------------------
// 2. Map file → module id (same scheme as meta-graph nodes)
// ---------------------------------------------------------------------------

function fileToModule(f) {
  if (f.startsWith('kiosk/')) return 'rc.kiosk';
  if (f.startsWith('web/')) return 'rc.web';
  if (f.startsWith('pwa/')) return 'rc.pwa';
  if (/^crates\/rc-agent\//.test(f)) return 'rc.rc-agent';
  if (/^crates\/rc-sentry\//.test(f)) return 'rc.rc-sentry';
  // racecontrol's own API files route to the appropriate sliced API sub-module
  if (/^crates\/racecontrol\/src\/api\/admin_/.test(f)) return 'rc.admin-api';
  if (/^crates\/racecontrol\/src\/api\/billing_/.test(f)) return 'rc.billing-api';
  if (/^crates\/racecontrol\/src\/api\/customer_/.test(f)) return 'rc.customer-api';
  // Everything else under racecontrol = umbrella
  return 'racecontrol';
}

function ignoreFile(f) {
  // Skip noise: lockfiles, generated, docs, CI, tests, config
  if (!/\.(ts|tsx|js|mjs|cjs|py|rs|rsx|toml|yaml|yml|json|sql)$/i.test(f)) return true;
  if (/^\.planning\//.test(f)) return true;
  if (/^\.github\//.test(f)) return true;
  if (/package-lock\.json$|Cargo\.lock$/.test(f)) return true;
  if (/\.min\.(js|css)$/.test(f)) return true;
  if (/^(LOGBOOK|SWAPLOG|HANDOFF|README)/.test(f)) return true;
  if (/\.(md|txt|bak)$/i.test(f)) return true;
  return false;
}

// ---------------------------------------------------------------------------
// 3. Count (moduleA, moduleB) co-changes + retain top file-pair examples
// ---------------------------------------------------------------------------

console.log('[2/4] Counting module-pair co-changes ...');
const pairCount = new Map();       // "modA|modB" -> commit count
const pairFileExamples = new Map(); // "modA|modB" -> Map(file-pair-key -> count)

for (const c of kept) {
  const files = c.files.filter(f => !ignoreFile(f));
  if (files.length < 2) continue;

  // Group files by module within this commit
  const modFiles = new Map();
  for (const f of files) {
    const m = fileToModule(f);
    if (!modFiles.has(m)) modFiles.set(m, []);
    modFiles.get(m).push(f);
  }
  const mods = [...modFiles.keys()].sort();
  if (mods.length < 2) continue;

  // Emit one event per distinct module-pair touched in this commit
  for (let i = 0; i < mods.length; i++) {
    for (let j = i + 1; j < mods.length; j++) {
      const key = `${mods[i]}|${mods[j]}`;
      pairCount.set(key, (pairCount.get(key) || 0) + 1);
      // Remember a representative file pair from this commit
      if (!pairFileExamples.has(key)) pairFileExamples.set(key, new Map());
      const examples = pairFileExamples.get(key);
      const fa = modFiles.get(mods[i])[0];
      const fb = modFiles.get(mods[j])[0];
      const fKey = `${fa} :: ${fb}`;
      examples.set(fKey, (examples.get(fKey) || 0) + 1);
    }
  }
}

// Filter by MIN_COMMITS threshold
const kept2 = [...pairCount.entries()]
  .filter(([, n]) => n >= MIN_COMMITS)
  .sort((a, b) => b[1] - a[1]);

console.log(`   ${pairCount.size} total module-pair co-changes, ${kept2.length} above threshold (>=${MIN_COMMITS} commits)`);

// ---------------------------------------------------------------------------
// 4. Diff against meta-graph edges
// ---------------------------------------------------------------------------

console.log('[3/4] Diffing against meta-graph.json ...');
const metaPath = path.join(__dirname, 'meta-graph.json');
const meta = fs.existsSync(metaPath) ? JSON.parse(fs.readFileSync(metaPath, 'utf8')) : { edges: [] };
const metaPairs = new Set();
for (const e of meta.edges) {
  // Exclude ONLY `co-change` from the self-diff — otherwise hidden pairs would move
  // to covered after first rebuild (co-change explaining itself).
  // `contains` IS a valid explanation (parent-child coupling is expected co-change).
  if (e.type === 'co-change') continue;
  // Symmetric — co-change coupling is undirected
  metaPairs.add(`${e.source}|${e.target}`);
  metaPairs.add(`${e.target}|${e.source}`);
}

const covered = [];
const hidden = [];
for (const [pair, count] of kept2) {
  const [a, b] = pair.split('|');
  const hasEdge = metaPairs.has(`${a}|${b}`) || metaPairs.has(`${b}|${a}`);
  const entry = {
    module_a: a, module_b: b,
    cochange_commits: count,
    has_meta_edge: hasEdge,
    top_file_pairs: [...(pairFileExamples.get(pair) || new Map()).entries()]
      .sort((x, y) => y[1] - x[1]).slice(0, 3)
      .map(([pair, n]) => ({ pair, n })),
  };
  if (hasEdge) covered.push(entry); else hidden.push(entry);
}

// ---------------------------------------------------------------------------
// 5. Emit artifacts
// ---------------------------------------------------------------------------

console.log('[4/4] Writing artifacts ...');
fs.writeFileSync(path.join(__dirname, 'cochange-edges.json'), JSON.stringify({
  generated_at: new Date().toISOString(),
  since: SINCE,
  commits_scanned: rawCommits,
  commits_kept: kept.length,
  min_commits: MIN_COMMITS,
  max_files_per_commit: MAX_FILES,
  covered_by_meta: covered,
  hidden_coupling: hidden,
}, null, 2));

// Markdown report — the high-value artifact for a human reviewer
const md = [];
md.push(`# Co-change vs meta-graph gap report`);
md.push('');
md.push(`Generated: ${new Date().toISOString()}  `);
md.push(`Source: \`git log --since="${SINCE}"\` in racecontrol repo`);
md.push(`Scanned: ${rawCommits} commits, kept ${kept.length} (dropped ${rawCommits - kept.length} mega/empty/merge)`);
md.push(`Threshold: >= ${MIN_COMMITS} co-changes per pair`);
md.push('');
md.push(`## Hidden coupling (co-change detected, NO meta-graph edge)`);
md.push('');
md.push(`These module pairs changed together often in git history but the static scanners`);
md.push(`(api-call / shared-db-table / shared-ws-variant / label-overlap / contains) found`);
md.push(`no edge. Each is either: (a) a genuine coupling channel my scanners can't see`);
md.push(`(file-system sentinel, shared config, process-spawn, dynamic protocol), or`);
md.push(`(b) coincidental (both edited in the same cleanup/refactor without real dependency).`);
md.push(``);
md.push(`Investigate each row to distinguish real gap vs noise.`);
md.push('');
if (hidden.length === 0) {
  md.push('*None — every co-changing module pair has at least one scanner-detected edge.*');
} else {
  md.push('| Module A | Module B | Co-change commits | Top file pair |');
  md.push('|---|---|---|---|');
  for (const h of hidden) {
    const fp = h.top_file_pairs[0] ? h.top_file_pairs[0].pair : '—';
    md.push(`| \`${h.module_a}\` | \`${h.module_b}\` | ${h.cochange_commits} | \`${fp}\` |`);
  }
}
md.push('');
md.push(`## Covered pairs (co-change + existing edge)`);
md.push('');
md.push(`Sanity check: these pairs both co-change AND have a scanner-detected edge.`);
md.push(`High counts here mean the scanners caught real coupling — confirming their value.`);
md.push('');
if (covered.length === 0) {
  md.push('*None.*');
} else {
  md.push('| Module A | Module B | Co-change commits |');
  md.push('|---|---|---|');
  for (const c of covered.slice(0, 30)) {
    md.push(`| \`${c.module_a}\` | \`${c.module_b}\` | ${c.cochange_commits} |`);
  }
}
md.push('');
md.push(`## How to act on this report`);
md.push('');
md.push(`1. Read the "Hidden coupling" table row by row.`);
md.push(`2. For each row, open the top file pair and trace: *does a data contract flow`);
md.push(`   between these two files?* If yes = real gap, file a new scanner rule.`);
md.push(`   If no = coincidence, note it in HANDOFF-GAPS.md and move on.`);
md.push(`3. Real gaps typically come in these classes:`);
md.push(`   - **File-system sentinel** (one writes \`MAINTENANCE_MODE\`, other reads it)`);
md.push(`   - **Shared config** (both read same TOML key)`);
md.push(`   - **Process spawn** (A starts B.exe)`);
md.push(`   - **Dynamic protocol** (HashMap-keyed event type, generated message ID)`);
md.push(`   - **Documentation cross-reference** (stale — update linked docs)`);

fs.writeFileSync(path.join(__dirname, 'cochange-gaps.md'), md.join('\n'));
console.log(`   wrote cochange-edges.json + cochange-gaps.md`);

// Console summary
console.log('');
console.log('Hidden coupling (co-change but no scanner edge):');
if (hidden.length === 0) {
  console.log('  (none)');
} else {
  for (const h of hidden.slice(0, 15)) {
    console.log(`  ${h.module_a.padEnd(20)} ↔ ${h.module_b.padEnd(20)}  ${String(h.cochange_commits).padStart(4)} commits`);
  }
  if (hidden.length > 15) console.log(`  ... and ${hidden.length - 15} more`);
}
console.log('');
console.log(`Total: ${hidden.length} hidden, ${covered.length} covered. See cochange-gaps.md for details.`);
