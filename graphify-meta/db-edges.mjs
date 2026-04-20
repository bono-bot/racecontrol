#!/usr/bin/env node
// graphify-meta/db-edges.mjs
// Discover SHARED-DB-TABLE coupling between modules.
// Two modules that both read/write the same SQL table have implicit coupling that the
// HTTP api-edges channel cannot see (they may talk via DB, not HTTP).
//
// Heuristic: grep each module's source for raw-SQL patterns:
//   FROM <table>
//   INSERT INTO <table>
//   UPDATE <table> SET
//   DELETE FROM <table>
//   Prisma: prisma.<model>.findMany()/create()/update()/delete()
//   SQLAlchemy: session.query(<Model>)
//
// Emit: db-edges.json — list of shared-table edges between modules.

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
  'racecontrol',
  'racingpoint-admin',
  'comms-link',
  'whatsapp-bot',
  'people-tracker',
  'pod-agent',
  'rc-ops-mcp',
];

// SQL keyword patterns that strongly indicate a specific table name follows.
// `[a-zA-Z_][a-zA-Z0-9_\.]*` captures until whitespace/paren — covers `staff`, `billing_sessions`, `public.customers`.
// The FROM and JOIN patterns are VERY noisy (Python `from X import Y` + English prose)
// so we only apply them when accompanied by SELECT within the preceding 200 chars,
// which means we need contextual matching — handled inline below.
const SQL_PATTERNS = [
  // bare read patterns — only applied with SELECT context (see scanForSql)
  { verb: 'read',   re: /\bFROM\s+([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: true },
  { verb: 'read',   re: /\bJOIN\s+([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: false },  // JOIN alone is SQL-specific
  // write patterns — strong SQL signatures, no context requirement
  { verb: 'write',  re: /\bINSERT\s+INTO\s+([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: false },
  { verb: 'write',  re: /\bUPDATE\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+SET\b/gi, requires_select: false },
  { verb: 'write',  re: /\bDELETE\s+FROM\s+([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: false },
  { verb: 'schema', re: /\bCREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: false },
  { verb: 'schema', re: /\bALTER\s+TABLE\s+([a-zA-Z_][a-zA-Z0-9_]*)\b/gi, requires_select: false },
];
// Prisma model access: prisma.staff.findMany(), prisma.customer.create()
const PRISMA_RE = /\bprisma\.([a-zA-Z_][a-zA-Z0-9_]*)\.(findMany|findFirst|findUnique|create|createMany|update|updateMany|upsert|delete|deleteMany|count)\b/g;

// SQL reserved words + keywords we DON'T want captured as table names
const SQL_STOPWORDS = new Set([
  'select', 'where', 'group', 'order', 'limit', 'offset', 'having', 'as', 'on', 'and', 'or', 'not',
  'in', 'like', 'is', 'null', 'between', 'exists', 'with', 'by', 'asc', 'desc',
  'values', 'set', 'into', 'table', 'view', 'index', 'database', 'schema', 'column',
  'left', 'right', 'inner', 'outer', 'cross', 'full', 'natural', 'using',
  'case', 'when', 'then', 'else', 'end', 'if', 'union', 'all', 'distinct',
  // Common SQLite pseudo-tables we don't care about as coupling
  'sqlite_master', 'sqlite_sequence', 'pragma',
  // Python stdlib / common import targets — caught by `from X import` false positives
  'datetime', 'pathlib', 'collections', 'typing', 'functools', 'itertools', 'dataclasses',
  'os', 'sys', 'json', 'time', 're', 'math', 'random', 'abc', 'enum', 'copy', 'struct',
  'subprocess', 'threading', 'asyncio', 'logging', 'argparse', 'hashlib', 'base64',
  'urllib', 'http', 'socket', 'ssl', 'pickle', 'csv', 'io', 'shutil', 'glob',
  // JS/TS common `from 'x' import` targets
  'react', 'next', 'express', 'axios', 'fs', 'path', 'url', 'crypto', 'util',
  'events', 'stream', 'buffer', 'child_process', 'zlib',
  // English prose words that get caught by FROM in markdown docs inside source files
  'the', 'a', 'an', 'this', 'that', 'these', 'those', 'each', 'every', 'any', 'some',
  'here', 'there', 'now', 'then', 'above', 'below',
  // Git/doc boilerplate
  'git', 'commit', 'branch', 'origin', 'main', 'master', 'head', 'tag', 'ref',
  // Common variable/word matches that are never real tables
  'cwd', 'env', 'args', 'kwargs', 'self', 'cls', 'this',
  // Abstract words that appear in every codebase
  'active', 'inactive', 'pending', 'last', 'first', 'previous', 'current', 'new', 'old',
  'user', 'admin', 'server', 'client', 'session', 'jwt', 'auth', 'key', 'token',
  'environment', 'config', 'settings', 'options', 'params', 'data', 'result', 'response',
  'peer', 'non', 'e', 'non_api', 'non_ws',
]);

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
    } else if (/\.(ts|tsx|js|mjs|cjs|py|rs|sql|prisma)$/i.test(ent.name)) {
      acc.push(p);
    }
  }
  return acc;
}

function normalizeTable(raw) {
  if (!raw) return null;
  // Strip schema prefix: `public.staff` → `staff`
  const parts = raw.split('.');
  const t = parts[parts.length - 1].toLowerCase();
  if (SQL_STOPWORDS.has(t)) return null;
  if (t.length < 3) return null;  // skip 1-2 char false positives
  return t;
}

console.log('[1/3] Scanning modules for SQL/Prisma references ...');
const modTables = new Map();  // module → Map(table → {verbs:Set, files:Set})

for (const mod of MODULES) {
  const modDir = path.join(ROOT, mod);
  if (!fs.existsSync(modDir)) continue;
  const tables = new Map();
  const files = walkSources(modDir);
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    const isPython = /\.py$/i.test(f);
    for (const pat of SQL_PATTERNS) {
      pat.re.lastIndex = 0;
      let m;
      while ((m = pat.re.exec(src)) !== null) {
        // Python .py files: FROM catches `from X import Y`. Skip unless we see
        // SELECT within 200 chars before this position.
        if (pat.requires_select) {
          const preceding = src.slice(Math.max(0, m.index - 200), m.index);
          if (!/\bSELECT\b/i.test(preceding)) continue;
        }
        // Language-specific skip: Python `from module import`
        if (isPython && /^\s*from\s+/i.test(src.slice(Math.max(0, m.index - 10), m.index + 10))) continue;
        const tbl = normalizeTable(m[1]);
        if (!tbl) continue;
        if (!tables.has(tbl)) tables.set(tbl, { verbs: new Set(), files: new Set() });
        tables.get(tbl).verbs.add(pat.verb);
        tables.get(tbl).files.add(f.replace(ROOT, '').replace(/\\/g, '/'));
      }
    }
    // Prisma (admin panel uses this)
    PRISMA_RE.lastIndex = 0;
    let m;
    while ((m = PRISMA_RE.exec(src)) !== null) {
      const model = m[1].toLowerCase();
      const op = m[2].toLowerCase();
      const verb = /^(find|count)/.test(op) ? 'read' : 'write';
      // Prisma models are camelCase singular; tables are often snake_case plural.
      // We keep the model name as-is here (dedup across modules works either way).
      if (!tables.has(model)) tables.set(model, { verbs: new Set(), files: new Set() });
      tables.get(model).verbs.add(verb);
      tables.get(model).files.add(f.replace(ROOT, '').replace(/\\/g, '/'));
    }
  }
  modTables.set(mod, tables);
  console.log(`   ${mod.padEnd(24)} ${tables.size} distinct tables referenced`);
}

console.log('[2/3] Computing shared-table edges ...');
const edges = [];
const modules = [...modTables.keys()];
for (let i = 0; i < modules.length; i++) {
  for (let j = i + 1; j < modules.length; j++) {
    const A = modules[i], B = modules[j];
    const tA = modTables.get(A), tB = modTables.get(B);
    const shared = [];
    for (const [tbl, infoA] of tA) {
      const infoB = tB.get(tbl);
      if (!infoB) continue;
      shared.push({
        table: tbl,
        [`${A}_verbs`]: [...infoA.verbs].sort(),
        [`${B}_verbs`]: [...infoB.verbs].sort(),
      });
    }
    if (shared.length === 0) continue;
    edges.push({
      from: A, to: B,
      type: 'shared-db-table',
      weight: shared.length,
      shared_tables: shared.slice(0, 20), // cap detail
    });
  }
}

console.log(`   ${edges.length} shared-table edges across ${modules.length} modules`);

console.log('[3/3] Writing db-edges.json ...');
const out = path.join(__dirname, 'db-edges.json');
fs.writeFileSync(out, JSON.stringify({
  generated_at: new Date().toISOString(),
  note: 'Shared SQL-table references discovered via grep. Regenerable via db-edges.mjs.',
  edges: edges.sort((a, b) => b.weight - a.weight),
  per_module_table_counts: Object.fromEntries(
    [...modTables.entries()].map(([m, t]) => [m, t.size])
  ),
}, null, 2));
console.log(`Wrote ${out}`);
console.log('');
console.log('Top 10 shared-table edges:');
for (const e of edges.slice(0, 10)) {
  const sample = e.shared_tables.slice(0, 5).map(s => s.table).join(', ');
  console.log(`  ${e.from.padEnd(20)} ↔ ${e.to.padEnd(20)}  w=${e.weight}  [${sample}]`);
}
