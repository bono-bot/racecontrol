#!/usr/bin/env node
// scripts/graphify-static-extras.mjs — closes Phase-5 benchmark gaps B/C/D/E
// with regex-based post-processors. Complements pure-AST graphify without
// requiring upstream changes.
//
// Categories covered:
//   B — reads_field / writes_field (Rust struct-field access)
//   C — reads_file / writes_file / reads_env (external-boundary syscalls)
//   D — dispatched_to (static impl enumeration for dyn Trait)
//   E — calls_macro (Rust macro! invocations)
//
// Output: graphify-out-static-extras/edges.json
//
// Usage:
//   node scripts/graphify-static-extras.mjs                    # default targets
//   node scripts/graphify-static-extras.mjs --target fixture   # single dir

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');

const argvTargets = process.argv.slice(2).filter((_, i, a) => a[i - 1] === '--target');
const TARGETS = argvTargets.length
  ? argvTargets.map(t => path.resolve(REPO_ROOT, t))
  : [REPO_ROOT, path.resolve(REPO_ROOT, '..', 'comms-link')];

const SKIP_DIRS = new Set(['node_modules', '.git', 'target', '.next', 'dist', 'build', 'graphify-out', 'graphify-out-unified', 'graphify-out-cross-process', 'graphify-out-static-extras', '.planning', 'deploy-staging', 'worktrees', 'bono-mirror', 'backups', 'audit', 'docs']);

function walk(root, exts) {
  const out = [];
  (function rec(dir) {
    let entries;
    try { entries = fs.readdirSync(dir, { withFileTypes: true }); } catch { return; }
    for (const e of entries) {
      if (SKIP_DIRS.has(e.name)) continue;
      const full = path.join(dir, e.name);
      if (e.isDirectory()) rec(full);
      else if (e.isFile() && exts.includes(path.extname(e.name))) out.push(full);
    }
  })(root);
  return out;
}

function readFile(p) { try { return fs.readFileSync(p, 'utf-8'); } catch { return ''; } }

// ─── B: struct fields (reads_field / writes_field) ────────────────────────
// MVP approach: find struct definitions + their field names, then scan for
// `<var>.<field_name>` access patterns in each file where <field_name>
// matches a known struct field. Accepts false positives (method names that
// happen to coincide with field names) → confidence AMBIGUOUS.
function extractStructFieldEdges(rustFiles, currentEnclosingFn) {
  // Pass 1: collect all struct fields as (struct_name, field_name).
  const fields = []; // { struct, field, file, line }
  const structRe = /^\s*(?:pub(?:\(.+?\))?\s+)?struct\s+(\w+)\s*\{/m;
  for (const f of rustFiles) {
    const content = readFile(f);
    const lines = content.split('\n');
    for (let i = 0; i < lines.length; i++) {
      const m = lines[i].match(structRe);
      if (!m) continue;
      const structName = m[1];
      // Read field lines until we see closing brace.
      for (let j = i + 1; j < Math.min(i + 60, lines.length); j++) {
        if (/^\s*\}/.test(lines[j])) break;
        const fm = lines[j].match(/^\s*(?:pub(?:\(.+?\))?\s+)?(\w+)\s*:/);
        if (fm && fm[1] !== 'fn' && fm[1] !== 'let') {
          fields.push({ struct: structName, field: fm[1], file: f, line: j + 1 });
        }
      }
    }
  }

  // Pass 2: for each .rs file, find reads and writes of any known field.
  // Writes: `<var>.<field> =`, struct-literal `Struct { field: ... }`.
  // Reads: `<var>.<field>` NOT followed by `(` (method call) NOT followed by `=` (write).
  const edges = [];
  const fieldNames = new Set(fields.map(f => f.field));

  for (const f of rustFiles) {
    const content = readFile(f);
    const lines = content.split('\n');
    // Track enclosing fn by simple bracket-depth walk (MVP — accepts outer-scope as "file").
    let enclosingFn = path.basename(f, '.rs');
    const fnRe = /\bfn\s+(\w+)\s*[<(]/;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i];
      const fnMatch = line.match(fnRe);
      if (fnMatch) enclosingFn = fnMatch[1];

      // Field read: `.fieldname` NOT followed by `(` or `=`.
      const readRe = /\.(\w+)(?!\s*[(\w=])/g;
      let m;
      while ((m = readRe.exec(line)) !== null) {
        if (fieldNames.has(m[1])) {
          edges.push({
            source: enclosingFn,
            target: m[1],
            relation: 'reads_field',
            confidence_tier: 'AMBIGUOUS',
            file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
            line: i + 1,
          });
        }
      }

      // Field write: `<var>.<field> =`
      const writeRe = /\b\w+\.(\w+)\s*=(?!=)/g;
      while ((m = writeRe.exec(line)) !== null) {
        if (fieldNames.has(m[1])) {
          edges.push({
            source: enclosingFn,
            target: m[1],
            relation: 'writes_field',
            confidence_tier: 'AMBIGUOUS',
            file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
            line: i + 1,
          });
        }
      }

      // Struct-literal initializer: `Struct { field: ... }`
      const initRe = /\b(\w+)\s*\{\s*(\w+)\s*:/g;
      while ((m = initRe.exec(line)) !== null) {
        if (fieldNames.has(m[2])) {
          edges.push({
            source: enclosingFn,
            target: m[2],
            relation: 'writes_field',
            confidence_tier: 'AMBIGUOUS',
            file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
            line: i + 1,
          });
        }
      }
    }
  }
  return { fields, edges };
}

// ─── C: external boundary (reads_file / writes_file / reads_env) ──────────
function extractExternalBoundary(rustFiles) {
  const edges = [];
  const patterns = [
    { re: /fs::read_to_string\s*\(\s*["']([^"']+)["']/g, relation: 'reads_file' },
    { re: /fs::read\s*\(\s*["']([^"']+)["']/g, relation: 'reads_file' },
    { re: /fs::write\s*\(\s*["']([^"']+)["']/g, relation: 'writes_file' },
    { re: /fs::OpenOptions::new\s*\(\s*\)[\s\S]{0,80}?\.open\s*\(\s*["']([^"']+)["']/g, relation: 'opens_file' },
    { re: /env::var\s*\(\s*["']([^"']+)["']/g, relation: 'reads_env' },
    { re: /std::env::var\s*\(\s*["']([^"']+)["']/g, relation: 'reads_env' },
  ];
  for (const f of rustFiles) {
    const content = readFile(f);
    const lines = content.split('\n');
    let enclosingFn = path.basename(f, '.rs');
    const fnRe = /\bfn\s+(\w+)\s*[<(]/;
    for (let i = 0; i < lines.length; i++) {
      const fm = lines[i].match(fnRe);
      if (fm) enclosingFn = fm[1];
      for (const { re, relation } of patterns) {
        let m;
        while ((m = re.exec(lines[i])) !== null) {
          edges.push({
            source: enclosingFn,
            target: m[1],
            relation,
            confidence_tier: 'HIGH',
            file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
            line: i + 1,
          });
        }
      }
    }
  }
  return edges;
}

// ─── D: dyn-Trait dispatch (static impl enumeration) ──────────────────────
// For each `dyn TraitName` call site `<var>.<method>()`, enumerate every
// `impl TraitName for <Type>` declaration and emit
// `dispatched_to <Type>::<method>` edges. Over-approximates
// (includes impls the call site can't actually reach) but captures structure
// pure-AST loses.
function extractDispatchEdges(rustFiles) {
  // Pass 1: find all impls: (trait, type, file, line).
  const impls = [];
  const implRe = /\bimpl(?:<[^>]*>)?\s+(\w+)(?:<[^>]*>)?\s+for\s+(\w+)/g;
  for (const f of rustFiles) {
    const content = readFile(f);
    let m;
    while ((m = implRe.exec(content)) !== null) {
      const before = content.slice(0, m.index);
      const line = before.split('\n').length;
      impls.push({ trait: m[1], type: m[2], file: f, line });
    }
  }
  const implsByTrait = {};
  for (const i of impls) (implsByTrait[i.trait] ||= []).push(i);

  // Pass 2: find `dyn TraitName` parameter types, track enclosing fn, then
  // find method calls within that fn and emit dispatch edges per impl.
  const edges = [];
  // `dyn` must appear in the param list `(…)` so we don't match across two
  // adjacent fn defs (fn handle → fn dispatch(h: &dyn Handler) — the `dyn`
  // belongs to dispatch, not handle).
  const dynFnRe = /\bfn\s+(\w+)\s*\([^)]{0,200}\bdyn\s+(\w+)/g;
  for (const f of rustFiles) {
    const content = readFile(f);
    let m;
    while ((m = dynFnRe.exec(content)) !== null) {
      const fnName = m[1];
      const traitName = m[2];
      const targets = implsByTrait[traitName] || [];
      if (!targets.length) continue;
      // Inspect the next ~30 lines for method calls (MVP — accepts noise).
      const startIdx = m.index;
      const slice = content.slice(startIdx, startIdx + 800);
      const methodRe = /\.(\w+)\s*\(/g;
      const methodsCalled = new Set();
      let mm;
      while ((mm = methodRe.exec(slice)) !== null) methodsCalled.add(mm[1]);
      const line = content.slice(0, startIdx).split('\n').length;
      for (const method of methodsCalled) {
        for (const impl of targets) {
          edges.push({
            source: fnName,
            target: `${impl.type}::${method}`,
            relation: 'dispatched_to',
            confidence_tier: 'AMBIGUOUS',
            file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
            line,
          });
        }
      }
    }
  }
  return { impls, edges };
}

// ─── E: calls_macro ───────────────────────────────────────────────────────
function extractMacroCalls(rustFiles) {
  const edges = [];
  const re = /\b(\w+)!\s*\(/g;
  // Skip macro names that are noise or not interesting at the graph level.
  const SKIP = new Set(['vec', 'format', 'include_str', 'include_bytes', 'concat', 'stringify']);
  for (const f of rustFiles) {
    const content = readFile(f);
    const lines = content.split('\n');
    let enclosingFn = path.basename(f, '.rs');
    const fnRe = /\bfn\s+(\w+)\s*[<(]/;
    for (let i = 0; i < lines.length; i++) {
      const fm = lines[i].match(fnRe);
      if (fm) enclosingFn = fm[1];
      let m;
      while ((m = re.exec(lines[i])) !== null) {
        if (SKIP.has(m[1])) continue;
        edges.push({
          source: enclosingFn,
          target: `${m[1]}!`,
          relation: 'calls_macro',
          confidence_tier: 'HIGH',
          file: path.relative(REPO_ROOT, f).replace(/\\/g, '/'),
          line: i + 1,
        });
      }
    }
  }
  return edges;
}

// ─── Run ──────────────────────────────────────────────────────────────────
console.log('[static-extras] scanning:');
const allRustFiles = [];
for (const t of TARGETS) {
  if (!fs.existsSync(t)) continue;
  const files = walk(t, ['.rs']);
  console.log(`  ${path.basename(t)}: ${files.length} rust files`);
  allRustFiles.push(...files);
}

const { fields, edges: fieldEdges } = extractStructFieldEdges(allRustFiles);
const boundaryEdges = extractExternalBoundary(allRustFiles);
const { impls, edges: dispatchEdges } = extractDispatchEdges(allRustFiles);
const macroEdges = extractMacroCalls(allRustFiles);

const allEdges = [...fieldEdges, ...boundaryEdges, ...dispatchEdges, ...macroEdges];

console.log(`\n[static-extras] summary:`);
console.log(`  B field edges:      ${fieldEdges.length}  (from ${fields.length} struct fields)`);
console.log(`  C boundary edges:   ${boundaryEdges.length}`);
console.log(`  D dispatch edges:   ${dispatchEdges.length}  (from ${impls.length} impls)`);
console.log(`  E macro edges:      ${macroEdges.length}`);
console.log(`  TOTAL:              ${allEdges.length}`);

const OUT_DIR = path.join(REPO_ROOT, 'graphify-out-static-extras');
fs.mkdirSync(OUT_DIR, { recursive: true });
const out = {
  generated: new Date().toISOString(),
  targets: TARGETS.map(t => path.basename(t)),
  struct_fields_found: fields.length,
  impls_found: impls.length,
  edge_count_by_category: {
    B_struct_fields: fieldEdges.length,
    C_external_boundary: boundaryEdges.length,
    D_dynamic_dispatch: dispatchEdges.length,
    E_macro_expansion: macroEdges.length,
  },
  edges: allEdges,
};
fs.writeFileSync(path.join(OUT_DIR, 'edges.json'), JSON.stringify(out, null, 2));
console.log(`\nwrote: ${path.join(OUT_DIR, 'edges.json')}`);
