#!/usr/bin/env node
// graphify-meta/import-edges.mjs
// Discover SHARED-CLIENT-LIBRARY coupling between JS/TS modules via import
// statement analysis. Reveals the channel behind kiosk ↔ web co-change (36 commits)
// that api/db/ws/fs scanners couldn't see — both frontends relative-import the same
// utility modules (WS hooks, API clients, shared types).
//
// Heuristic: parse `import ... from '<path>'` + `require('<path>')` + dynamic
// `import('<path>')`. Ignore npm packages (no `./` or `../` prefix). For relative
// paths that escape the current module's subdir (e.g. kiosk importing from
// `../web/shared/foo`), emit a cross-module import edge.
//
// Emits import-edges.json consumed by build-meta.mjs.

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

// Modules with JS/TS code. Rust crates skipped (they have their own module system).
const MODULES = [
  { id: 'rc.kiosk',           subdir: 'racecontrol/kiosk' },
  { id: 'rc.web',             subdir: 'racecontrol/web' },
  { id: 'rc.pwa',             subdir: 'racecontrol/pwa' },
  { id: 'racingpoint-admin',  subdir: 'racingpoint-admin' },
  { id: 'comms-link',         subdir: 'comms-link' },
  { id: 'whatsapp-bot',       subdir: 'whatsapp-bot' },
  { id: 'rc-ops-mcp',         subdir: 'rc-ops-mcp' },
];

// import X from 'path' | import { Y } from 'path' | require('path') | import('path')
const IMPORT_RE = /(?:import\s+(?:[^'"]*\s+from\s+)?|require\s*\(\s*|import\s*\(\s*)["']([^"']+)["']/g;

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
    } else if (/\.(ts|tsx|js|mjs|cjs|jsx)$/i.test(ent.name)) {
      acc.push(p);
    }
  }
  return acc;
}

// Resolve a relative import to an absolute repo-root-relative canonical path.
// Mirrors TS/webpack resolution minimally: if `./foo`, try foo.ts / foo.tsx / foo/index.ts.
function resolveImport(importer, spec) {
  if (!/^\.{1,2}\//.test(spec)) return null;  // skip npm packages
  const base = path.dirname(importer);
  const candidates = [
    path.join(base, spec),
    path.join(base, spec + '.ts'),
    path.join(base, spec + '.tsx'),
    path.join(base, spec + '.js'),
    path.join(base, spec + '.mjs'),
    path.join(base, spec, 'index.ts'),
    path.join(base, spec, 'index.tsx'),
    path.join(base, spec, 'index.js'),
  ];
  for (const c of candidates) {
    if (fs.existsSync(c)) return c.replace(/\\/g, '/');
  }
  // Import points somewhere real-sounding but file missing — still return
  // normalized path so cross-module detection works.
  return candidates[0].replace(/\\/g, '/');
}

// Map a file path to its owning module id (same scheme as build-meta MODULES).
function fileToModule(absPath) {
  const relToRoot = absPath.replace(ROOT + '/', '').replace(/\\/g, '/');
  for (const m of MODULES) {
    if (relToRoot.startsWith(m.subdir + '/') || relToRoot === m.subdir) return m.id;
  }
  return null;
}

console.log('[1/3] Scanning JS/TS imports in', MODULES.length, 'modules ...');
const edgesByPair = new Map();  // "A|B" -> { count, top_files: Map(key -> count) }

for (const { id: mod, subdir } of MODULES) {
  const modDir = path.join(ROOT, subdir);
  if (!fs.existsSync(modDir)) continue;
  const files = walkSources(modDir);
  let crossModuleImports = 0;
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    IMPORT_RE.lastIndex = 0;
    let m;
    while ((m = IMPORT_RE.exec(src)) !== null) {
      const spec = m[1];
      const resolved = resolveImport(f, spec);
      if (!resolved) continue;
      // Normalize: path.resolve to strip `..`, then re-relative to ROOT
      const normAbs = path.resolve(resolved).replace(/\\/g, '/');
      const targetMod = fileToModule(normAbs);
      if (!targetMod || targetMod === mod) continue;
      crossModuleImports++;
      const key = [mod, targetMod].sort().join('|');
      if (!edgesByPair.has(key)) edgesByPair.set(key, { count: 0, files: new Map() });
      const ent = edgesByPair.get(key);
      ent.count++;
      const importerRel = f.replace(ROOT, '').replace(/\\/g, '/');
      const targetRel = normAbs.replace(ROOT, '');
      const fp = `${importerRel} -> ${targetRel}`;
      ent.files.set(fp, (ent.files.get(fp) || 0) + 1);
    }
  }
  console.log(`   ${mod.padEnd(24)} ${files.length} files, ${crossModuleImports} cross-module imports`);
}

console.log('[2/3] Emitting edges ...');
const edges = [];
for (const [key, ent] of edgesByPair) {
  const [a, b] = key.split('|');
  edges.push({
    from: a, to: b,
    type: 'shared-import',
    weight: ent.count,
    top_imports: [...ent.files.entries()].sort((x, y) => y[1] - x[1]).slice(0, 3)
      .map(([pair, n]) => ({ pair, n })),
  });
}
edges.sort((a, b) => b.weight - a.weight);
console.log(`   ${edges.length} shared-import edges`);

console.log('[3/3] Writing import-edges.json ...');
const out = path.join(__dirname, 'import-edges.json');
fs.writeFileSync(out, JSON.stringify({
  generated_at: new Date().toISOString(),
  note: 'JS/TS import-statement coupling. Two modules sharing a relative import reveals client-library coupling invisible to HTTP/SQL/WS scanners.',
  edges,
}, null, 2));
console.log(`Wrote ${out}`);
console.log('');
console.log('Top 10 shared-import edges:');
for (const e of edges.slice(0, 10)) {
  const sample = e.top_imports[0]?.pair.split(' -> ')[1]?.split('/').slice(-2).join('/') || '';
  console.log(`  ${e.from.padEnd(20)} ↔ ${e.to.padEnd(20)}  w=${e.weight}  [${sample}]`);
}
