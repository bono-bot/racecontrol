#!/usr/bin/env node
// scripts/graphify-incremental.mjs — P5.5 MVP.
//
// Wraps `graphify update <dir>` with a file-sha short-circuit. The full rebuild
// is ~40s on racecontrol (2230 files). If zero source files have changed since
// the last run, the rebuild is a no-op — skip it entirely and keep the existing
// graph.json. On the first run, or when any file changes, fall through to the
// real `graphify update`.
//
// Usage:
//   node scripts/graphify-incremental.mjs              # racecontrol repo root
//   node scripts/graphify-incremental.mjs ../comms-link
//   node scripts/graphify-incremental.mjs --force      # skip short-circuit
//
// State: graphify-out/.graphify/file-hashes.json (per target dir).
//
// This is a wrapper, not a graphify replacement. It does NOT attempt partial
// rebuilds (merging deltas into an existing graph.json) — that requires
// graphify's internal extractor API. The short-circuit handles the common case
// (running graphify on a clean working tree) which is 80%+ of calls from the
// post-commit hook.

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execSync } from 'node:child_process';

const FORCE = process.argv.includes('--force');
const TARGET = path.resolve(process.argv.find(a => !a.startsWith('-') && a !== process.argv[0] && a !== process.argv[1]) || '.');

// Source file extensions to hash — same list graphify itself processes.
const EXTS = new Set(['.rs', '.ts', '.tsx', '.js', '.jsx', '.py', '.sh', '.mjs', '.toml', '.json', '.md', '.yaml', '.yml']);
const SKIP_DIRS = new Set(['node_modules', '.git', 'target', '.next', 'dist', 'build', 'graphify-out', '.planning', 'deploy-staging', 'graphify-out-unified', 'bono-mirror']);

function walkSources(root) {
  const results = [];
  const walk = (dir) => {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const entry of entries) {
      if (SKIP_DIRS.has(entry.name)) continue;
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && EXTS.has(path.extname(entry.name))) results.push(full);
    }
  };
  walk(root);
  return results;
}

function hashFile(p) {
  try {
    const buf = fs.readFileSync(p);
    return crypto.createHash('sha256').update(buf).digest('hex');
  } catch { return ''; }
}

function loadPrevHashes(stateFile) {
  try { return JSON.parse(fs.readFileSync(stateFile, 'utf-8')); } catch { return null; }
}

function saveHashes(stateFile, obj) {
  fs.mkdirSync(path.dirname(stateFile), { recursive: true });
  fs.writeFileSync(stateFile, JSON.stringify(obj, null, 2));
}

// ─── main ─────────────────────────────────────────────────────────────────
const stateFile = path.join(TARGET, 'graphify-out', '.graphify', 'file-hashes.json');
const graphFile = path.join(TARGET, 'graphify-out', 'graph.json');

console.log(`[incremental] target: ${TARGET}`);

const prev = loadPrevHashes(stateFile);
const files = walkSources(TARGET);
console.log(`[incremental] walking ${files.length} candidate source files`);

const current = {};
for (const f of files) {
  current[path.relative(TARGET, f).replace(/\\/g, '/')] = hashFile(f);
}

let shouldSkip = false;
let changedCount = 0;
let addedCount = 0;
let removedCount = 0;

if (prev && prev.files && fs.existsSync(graphFile) && !FORCE) {
  for (const [p, h] of Object.entries(current)) {
    if (!prev.files[p]) addedCount++;
    else if (prev.files[p] !== h) changedCount++;
  }
  for (const p of Object.keys(prev.files)) {
    if (!current[p]) removedCount++;
  }
  shouldSkip = (changedCount === 0 && addedCount === 0 && removedCount === 0);
}

if (shouldSkip) {
  const prevTs = prev.timestamp ? `(last built ${prev.timestamp})` : '';
  console.log(`[incremental] no source changes detected ${prevTs} — skipping rebuild (saved ~40s)`);
  console.log(`[incremental] existing graph: ${graphFile}`);
  process.exit(0);
}

if (prev && prev.files) {
  console.log(`[incremental] changes: +${addedCount} new, ~${changedCount} modified, -${removedCount} removed → running full rebuild`);
} else {
  console.log(`[incremental] no prior hash state — running full rebuild${FORCE ? ' (--force)' : ''}`);
}

// Fall through to real graphify update.
try {
  execSync(`graphify update "${TARGET}"`, { stdio: 'inherit', cwd: TARGET });
} catch (err) {
  // graphify can exit non-zero on viz errors (G12) while still writing graph.json.
  console.error(`[incremental] graphify exited with code ${err.status} — checking graph.json`);
  if (!fs.existsSync(graphFile)) {
    console.error(`[incremental] FATAL: graph.json not produced, propagating error`);
    process.exit(err.status || 1);
  }
}

// Persist new hashes for next run.
saveHashes(stateFile, {
  timestamp: new Date().toISOString(),
  target: TARGET,
  files: current,
  file_count: files.length,
});
console.log(`[incremental] hashes saved: ${stateFile}`);
