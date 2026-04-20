#!/usr/bin/env node
// graphify-meta/struct-edges.mjs
// Discover SHARED-TYPE-NAME coupling across Rust/TS boundaries.
//
// Motivation: kiosk ↔ rc-agent co-changed 30x because kiosk's setup wizard sends
// `AcLaunchParams` payload that rc-agent's `ac_launcher.rs` parses. ws-edges only
// matches enum variants (`AgentMessage::ConfigPush`) — it misses payload structs.
// This scanner finds Rust `pub struct X` / `pub enum X` whose NAME also appears as
// a TypeScript `interface X` / `type X = ` / `class X` / type annotation.
//
// False positives: generic names (Config, Request, Response) will match. Controlled
// by (a) PascalCase requirement + min-length, (b) stopword list, (c) only count
// when module A defines + module B references (not both define independently).
//
// Emits struct-edges.json consumed by build-meta.mjs.

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

const MODULES = [
  { id: 'rc.rc-agent',        subdir: 'racecontrol/crates/rc-agent', lang: 'rust' },
  { id: 'rc.rc-sentry',       subdir: 'racecontrol/crates/rc-sentry', lang: 'rust' },
  { id: 'racecontrol',        subdir: 'racecontrol/crates/racecontrol', lang: 'rust' },
  { id: 'rc.kiosk',           subdir: 'racecontrol/kiosk',   lang: 'ts' },
  { id: 'rc.web',             subdir: 'racecontrol/web',     lang: 'ts' },
  { id: 'rc.pwa',             subdir: 'racecontrol/pwa',     lang: 'ts' },
  { id: 'racingpoint-admin',  subdir: 'racingpoint-admin',   lang: 'ts' },
  { id: 'comms-link',         subdir: 'comms-link',          lang: 'ts' },
  { id: 'whatsapp-bot',       subdir: 'whatsapp-bot',        lang: 'ts' },
];

// Rust-side type DEFINITIONS (struct / enum with Serialize derive means cross-process
// contract). Capture: the preceding #[derive(...)] attribute + the `pub (struct|enum) NAME`.
const RUST_DEF_RE = /#\[derive\([^)]*(?:Serialize|Deserialize)[^)]*\)\][^\n]*\n\s*(?:pub\s+)?(struct|enum)\s+([A-Z][A-Za-z0-9_]+)/g;
// TS-side type DEFINITIONS
const TS_DEF_RE = /\b(?:export\s+)?(?:type|interface|class)\s+([A-Z][A-Za-z0-9_]+)/g;
// Cross-language REFERENCES: Rust `Type::` / `: Type` / `<Type>` ; TS `: Type` / `<Type>`
const TYPE_REF_RE = /\b([A-Z][A-Za-z0-9_]{3,})\b/g;

// Generic type names that appear in every codebase and aren't meaningful coupling.
const TYPE_STOPLIST = new Set([
  'Config', 'Request', 'Response', 'Error', 'Result', 'Ok', 'Err', 'None', 'Some',
  'String', 'Vec', 'HashMap', 'HashSet', 'BTreeMap', 'Option', 'Arc', 'Rc', 'Mutex',
  'RwLock', 'Cell', 'RefCell', 'Box', 'Pin', 'Future', 'Stream', 'Sender', 'Receiver',
  'Object', 'Array', 'Map', 'Set', 'Date', 'Number', 'Boolean', 'Function',
  'React', 'Component', 'Props', 'State', 'Ref', 'Element', 'FC', 'JSX',
  'Promise', 'Buffer', 'URL', 'URLSearchParams', 'FormData', 'FileList',
  'WebSocket', 'RequestInit', 'AbortController', 'AbortSignal',
  'Next', 'Page', 'Layout', 'Router', 'Route', 'Link', 'Head', 'Image',
  'App', 'Main', 'Module', 'Export', 'Default', 'Type',
  // SerdeJSON / common
  'Value', 'Null', 'Bool', 'Number', 'Object',
  // Common single-purpose name collisions
  'User', 'Session', 'Token', 'Auth', 'Login', 'Logout',
]);

function walkSources(dir, acc = [], lang, depth = 0) {
  if (depth > 10) return acc;
  let entries;
  try { entries = fs.readdirSync(dir, { withFileTypes: true }); }
  catch { return acc; }
  for (const ent of entries) {
    const p = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (/^(node_modules|\.next|\.git|__pycache__|venv|\.venv|target|dist|build|graphify-out|\.claude)$/.test(ent.name)) continue;
      walkSources(p, acc, lang, depth + 1);
    } else {
      const ext = lang === 'rust' ? /\.rs$/i : /\.(ts|tsx|js|mjs|cjs|jsx)$/i;
      if (ext.test(ent.name)) acc.push(p);
    }
  }
  return acc;
}

function isValidTypeName(name) {
  if (!name || name.length < 5) return false;  // skip short generics
  if (TYPE_STOPLIST.has(name)) return false;
  if (!/^[A-Z]/.test(name)) return false;
  return true;
}

console.log('[1/3] Scanning modules for type definitions + references ...');
const modDefs = new Map();  // mod -> Set<typeName>
const modRefs = new Map();  // mod -> Map(typeName -> Set<file>)

for (const { id: mod, subdir, lang } of MODULES) {
  const modDir = path.join(ROOT, subdir);
  if (!fs.existsSync(modDir)) continue;
  const defs = new Set();
  const refs = new Map();
  const files = walkSources(modDir, [], lang);

  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    // Definitions
    const defRe = lang === 'rust' ? RUST_DEF_RE : TS_DEF_RE;
    defRe.lastIndex = 0;
    let m;
    while ((m = defRe.exec(src)) !== null) {
      const name = lang === 'rust' ? m[2] : m[1];
      if (!isValidTypeName(name)) continue;
      defs.add(name);
    }
    // References (every PascalCase token — noisy but filtered downstream)
    TYPE_REF_RE.lastIndex = 0;
    while ((m = TYPE_REF_RE.exec(src)) !== null) {
      const name = m[1];
      if (!isValidTypeName(name)) continue;
      if (!refs.has(name)) refs.set(name, new Set());
      refs.get(name).add(f.replace(ROOT, '').replace(/\\/g, '/'));
    }
  }
  modDefs.set(mod, defs);
  modRefs.set(mod, refs);
  console.log(`   ${mod.padEnd(24)} ${defs.size} definitions, ${refs.size} distinct references`);
}

console.log('[2/3] Matching definitions in module A against references in module B ...');
const edges = [];
const mods = [...modDefs.keys()];
for (const A of mods) {
  for (const B of mods) {
    if (A === B) continue;
    const defs = modDefs.get(A);
    const refs = modRefs.get(B);
    if (!defs || !refs) continue;
    const shared = [];
    for (const name of defs) {
      if (!refs.has(name)) continue;
      // If B also DEFINES this name locally, it's not really a cross-module contract
      // (both sides define independently — generic name collision or actual copy).
      // Keep the match but tag it `both_define: true` so downstream can filter.
      const bothDefine = modDefs.get(B).has(name);
      shared.push({
        name,
        both_define: bothDefine,
        sample_B_refs: [...refs.get(name)].slice(0, 2),
      });
    }
    if (shared.length === 0) continue;
    edges.push({
      from: A, to: B,
      type: 'shared-struct-name',
      weight: shared.length,
      // Prefer single-definition matches (A defines, B only references = strongest)
      single_def_count: shared.filter(s => !s.both_define).length,
      shared_types: shared.slice(0, 15),
    });
  }
}
// Sort by single_def count (strong signal) desc
edges.sort((a, b) => b.single_def_count - a.single_def_count || b.weight - a.weight);
console.log(`   ${edges.length} shared-type edges`);

console.log('[3/3] Writing struct-edges.json ...');
const out = path.join(__dirname, 'struct-edges.json');
fs.writeFileSync(out, JSON.stringify({
  generated_at: new Date().toISOString(),
  note: 'Cross-language shared type names. A -> B means A defines `pub struct X` (or `interface X`) and B references the name X. High single_def_count = strong payload-schema coupling.',
  edges,
}, null, 2));
console.log(`Wrote ${out}`);
console.log('');
console.log('Top 10 shared-type edges (by single-definition matches):');
for (const e of edges.slice(0, 10)) {
  const sample = e.shared_types.filter(t => !t.both_define).slice(0, 3).map(t => t.name).join(', ');
  console.log(`  ${e.from.padEnd(20)} -> ${e.to.padEnd(20)}  single=${e.single_def_count}/total=${e.weight}  [${sample}]`);
}
