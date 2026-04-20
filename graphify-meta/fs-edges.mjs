#!/usr/bin/env node
// graphify-meta/fs-edges.mjs
// Discover FILESYSTEM-SENTINEL coupling — modules A and B couple implicitly when
// A writes a file and B reads it (or vice versa). This class was invisible to
// api/db/ws/label scanners but revealed by git co-change mining (rc-agent ↔ rc-sentry
// shipping together 15 times was driven by MAINTENANCE_MODE + GRACEFUL_RELAUNCH
// file sentinels, not HTTP).
//
// Heuristic: grep each module for literal Windows/Unix paths containing RacingPoint
// sentinel names. When two modules reference the same path → coupling edge.
//
// Emits fs-edges.json consumed by build-meta.mjs as channel 4.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
function findEcosystemRoot(start) {
  let d = start;
  for (let i = 0; i < 6; i++) {
    if (fs.existsSync(path.join(d, 'racecontrol')) && fs.existsSync(path.join(d, 'racingpoint-admin'))) return d;
    const parent = path.dirname(d);
    if (parent === d) break;
    d = parent;
  }
  return null;
}
const ROOT = (process.env.GRAPHIFY_META_ROOT || findEcosystemRoot(__dirname) || 'C:/Users/bono/racingpoint').replace(/\\/g, '/');

// Split racecontrol/crates further: rc-agent + rc-sentry are separate pod binaries
// with their own process lifecycle. Grouping them loses the MAINTENANCE_MODE /
// GRACEFUL_RELAUNCH sentinel coupling that motivated this scanner in the first place.
const MODULES = [
  { id: 'rc.rc-agent',        subdir: 'racecontrol/crates/rc-agent' },
  { id: 'rc.rc-sentry',       subdir: 'racecontrol/crates/rc-sentry' },
  { id: 'racecontrol.server', subdir: 'racecontrol/crates/racecontrol' },
  { id: 'rc.kiosk',           subdir: 'racecontrol/kiosk' },
  { id: 'rc.web',             subdir: 'racecontrol/web' },
  { id: 'rc.pwa',             subdir: 'racecontrol/pwa' },
  { id: 'racingpoint-admin',  subdir: 'racingpoint-admin' },
  { id: 'comms-link',         subdir: 'comms-link' },
  { id: 'whatsapp-bot',       subdir: 'whatsapp-bot' },
  { id: 'people-tracker',     subdir: 'people-tracker' },
  { id: 'pod-agent',          subdir: 'pod-agent' },
  { id: 'rc-ops-mcp',         subdir: 'rc-ops-mcp' },
];

// Paths that look like RacingPoint filesystem contracts. Match absolute and relative.
//   C:\RacingPoint\MAINTENANCE_MODE        — pod sentinel
//   C:/RacingPoint/GRACEFUL_RELAUNCH
//   /root/racingpoint/.../events.jsonl     — Linux VPS paths
//   C:\Users\bono\racingpoint\...         — James local dev
// Filename-only match (e.g. "MAINTENANCE_MODE") is too noisy; require 1+ path separator
// in the literal so we only capture real path references.
const PATH_LITERAL_RE = /["'`]((?:[A-Za-z]:[\/\\]|\/[a-zA-Z_.\-0-9]+[\/\\]|\.[\/\\])[A-Za-z0-9_.\-\/\\ ]*[A-Za-z0-9_.\-])["'`]/g;

// Filter out noise that matches the regex but isn't semantic coupling:
//   .js/.ts/.py/.rs source files, node_modules paths, CRLF-endings, URL schemes
function isNoisyPath(p) {
  if (/\.(js|ts|tsx|mjs|cjs|py|rs|rsx|json|md|toml|yaml|yml|css|scss|html|svg|png|jpg|lock|sh|bat|ps1)$/i.test(p)) return true;
  if (/node_modules|\.git[\/\\]|\.next|target[\/\\]|dist[\/\\]|build[\/\\]/.test(p)) return true;
  if (p.length < 6) return true;
  // Skip raw prose URL-ish matches that happened to have slashes
  if (/^\.{1,2}[\/\\]/.test(p) && p.length < 12) return true;
  return false;
}

// Canonicalize: lowercase, collapse path separators, drop trailing slash, normalize
// C:\RacingPoint and c:/racingpoint into the same token.
function canon(p) {
  return p.toLowerCase().replace(/\\/g, '/').replace(/\/+/g, '/').replace(/\/$/, '');
}

function walkSources(dir, acc = [], depth = 0) {
  if (depth > 10) return acc;
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); }
  catch { return acc; }
  for (const ent of entries) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (/^(node_modules|\.next|\.git|__pycache__|venv|\.venv|target|dist|build|graphify-out|\.claude)$/.test(ent.name)) continue;
      walkSources(p, acc, depth + 1);
    } else if (/\.(ts|tsx|js|mjs|cjs|py|rs|rsx|toml|yaml|yml|sh|bat|ps1)$/i.test(ent.name)) {
      acc.push(p);
    }
  }
  return acc;
}

console.log('[1/3] Scanning modules for filesystem path literals ...');
const modPaths = new Map();  // moduleId -> Map(canon_path -> Set<files>)

for (const { id: mod, subdir } of MODULES) {
  const modDir = path.join(ROOT, subdir);
  if (!fs.existsSync(modDir)) continue;
  const paths = new Map();
  const files = walkSources(modDir);
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    PATH_LITERAL_RE.lastIndex = 0;
    let m;
    while ((m = PATH_LITERAL_RE.exec(src)) !== null) {
      const raw = m[1];
      if (isNoisyPath(raw)) continue;
      // Only care about paths that look like cross-process contracts
      // Must mention RacingPoint, /root/racingpoint, or /c/Users/bono/racingpoint
      if (!/racingpoint|racecontrol/i.test(raw)) continue;
      const c = canon(raw);
      if (!paths.has(c)) paths.set(c, new Set());
      paths.get(c).add(f.replace(ROOT, '').replace(/\\/g, '/'));
    }
  }
  modPaths.set(mod, paths);
  console.log(`   ${mod.padEnd(24)} ${paths.size} distinct RacingPoint-family path literals`);
}

console.log('[2/3] Computing shared-path edges ...');
const edges = [];
const mods = [...modPaths.keys()];
for (let i = 0; i < mods.length; i++) {
  for (let j = i + 1; j < mods.length; j++) {
    const A = mods[i], B = mods[j];
    const pA = modPaths.get(A), pB = modPaths.get(B);
    const shared = [];
    for (const [p, filesA] of pA) {
      if (pB.has(p)) {
        shared.push({
          path: p,
          [`${A}_refs`]: [...filesA].slice(0, 2),
          [`${B}_refs`]: [...pB.get(p)].slice(0, 2),
        });
      }
    }
    if (shared.length === 0) continue;
    edges.push({
      from: A, to: B,
      type: 'shared-fs-path',
      weight: shared.length,
      shared_paths: shared.slice(0, 20),
    });
  }
}

console.log(`   ${edges.length} shared-fs-path edges`);

console.log('[3/3] Writing fs-edges.json ...');
const out = path.join(__dirname, 'fs-edges.json');
fs.writeFileSync(out, JSON.stringify({
  generated_at: new Date().toISOString(),
  note: 'Shared filesystem path literals — captures sentinel files, config, binaries that couple two modules.',
  edges: edges.sort((a, b) => b.weight - a.weight),
  per_module_path_counts: Object.fromEntries(
    [...modPaths.entries()].map(([m, ps]) => [m, ps.size])
  ),
}, null, 2));
console.log(`Wrote ${out}`);
console.log('');
console.log('Top 10 shared-fs-path edges:');
for (const e of edges.slice(0, 10)) {
  const sample = e.shared_paths.slice(0, 2).map(s => s.path.split('/').slice(-2).join('/')).join(', ');
  console.log(`  ${e.from.padEnd(20)} ↔ ${e.to.padEnd(20)}  w=${e.weight}  [${sample}]`);
}
