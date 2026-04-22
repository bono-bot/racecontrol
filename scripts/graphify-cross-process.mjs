#!/usr/bin/env node
// scripts/graphify-cross-process.mjs — P5.2 MVP.
//
// Regex-based extractor for cross-process / cross-language edges that pure-AST
// graphify can't see:
//   - HTTP: JS/TS fetch("/path") / axios.{get,post,…}("/path") → matches Rust
//     axum .route("/path", method) handlers.
//   - Subprocess: JS/TS spawn("bin") / execFile("bin") / Rust Command::new("bin")
//     → matches known Rust [[bin]] names from Cargo.toml.
//   - UDP: Rust UdpSocket::bind("0.0.0.0:PORT") / .send_to(_, ("ip", PORT)) →
//     keyed by port number.
//
// Output: graphify-out-cross-process/edges.json — a JSON list of edge objects
// with {source_file, source_line, target, relation, confidence_tier}.
// Downstream: unify.mjs can optionally load this and emit edges into the
// unified graph.
//
// Scope: MVP. Accepts false positives (tuning: confidence_tier AMBIGUOUS).
// Directed: source = caller / sender; target = route / binary / port.
//
// Usage:
//   node scripts/graphify-cross-process.mjs
//   node scripts/graphify-cross-process.mjs --repos racecontrol,../comms-link

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');

// ─── Config ──────────────────────────────────────────────────────────────
const TARGETS = [
  REPO_ROOT,
  path.resolve(REPO_ROOT, '..', 'comms-link'),
];

const SOURCE_EXTS = { js: ['.js', '.mjs'], ts: ['.ts', '.tsx'], rs: ['.rs'] };
const SKIP_DIRS = new Set(['node_modules', '.git', 'target', '.next', 'dist', 'build', 'graphify-out', 'graphify-out-unified', 'graphify-out-cross-process', '.planning', 'deploy-staging', 'worktrees', 'bono-mirror', 'backups', 'audit', 'docs']);

// ─── File walk ────────────────────────────────────────────────────────────
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

// ─── Pattern extractors ───────────────────────────────────────────────────

// Rust axum routes: .route("/path", get(handler))
function extractRustRoutes(filePath, content) {
  const routes = [];
  const lines = content.split('\n');
  const routeRe = /\.route\s*\(\s*["']([^"']+)["']\s*,\s*(get|post|put|delete|patch|options)\s*\(/g;
  for (let i = 0; i < lines.length; i++) {
    let m;
    while ((m = routeRe.exec(lines[i])) !== null) {
      routes.push({ file: filePath, line: i + 1, path: m[1], method: m[2].toUpperCase() });
    }
  }
  return routes;
}

// JS/TS fetch/axios/http client calls
function extractHttpClientCalls(filePath, content) {
  const calls = [];
  const lines = content.split('\n');
  const patterns = [
    /\bfetch\s*\(\s*[`"']([^`"']+)[`"']/g,
    /\baxios\s*\.\s*(get|post|put|delete|patch)\s*\(\s*[`"']([^`"']+)[`"']/g,
    /\baxios\s*\(\s*\{\s*url\s*:\s*[`"']([^`"']+)[`"']/g,
  ];
  for (let i = 0; i < lines.length; i++) {
    for (const re of patterns) {
      let m;
      while ((m = re.exec(lines[i])) !== null) {
        const url = m[m.length - 1];
        // Only track relative/local URLs (starting with /) — external URLs are out of scope.
        if (url.startsWith('/')) {
          calls.push({ file: filePath, line: i + 1, url, method: m[1] ? m[1].toUpperCase() : 'GET' });
        }
      }
    }
  }
  return calls;
}

// Subprocess spawns — JS/TS and Rust
function extractSubprocessCalls(filePath, content) {
  const calls = [];
  const lines = content.split('\n');
  const patterns = [
    // JS: spawn('bin', …), execFile('bin', …), exec('bin …')
    /\b(?:spawn|execFile|exec|spawnSync|execSync)\s*\(\s*[`"']([^\s`"']+)/g,
    // Rust: Command::new("bin"), tokio::process::Command::new("bin")
    /Command::new\s*\(\s*["']([^"']+)["']/g,
  ];
  for (let i = 0; i < lines.length; i++) {
    for (const re of patterns) {
      let m;
      while ((m = re.exec(lines[i])) !== null) {
        const bin = m[1];
        if (bin && !bin.startsWith('http')) {
          calls.push({ file: filePath, line: i + 1, bin });
        }
      }
    }
  }
  return calls;
}

// UDP port binds — Rust only (telemetry consumers)
function extractUdpBinds(filePath, content) {
  const binds = [];
  const lines = content.split('\n');
  const re = /UdpSocket::bind\s*\(\s*["'][\d.:]*?:(\d{2,5})["']/g;
  for (let i = 0; i < lines.length; i++) {
    let m;
    while ((m = re.exec(lines[i])) !== null) {
      binds.push({ file: filePath, line: i + 1, port: parseInt(m[1], 10) });
    }
  }
  return binds;
}

// ─── Scan all targets ─────────────────────────────────────────────────────
console.log('[cross-process] scanning targets...');

const rustRoutes = [];
const httpCalls = [];
const subprocessCalls = [];
const udpBinds = [];

for (const target of TARGETS) {
  if (!fs.existsSync(target)) continue;
  const repoName = path.basename(target);
  console.log(`  ${repoName}:`);

  const rustFiles = walk(target, SOURCE_EXTS.rs);
  const jsFiles = walk(target, [...SOURCE_EXTS.js, ...SOURCE_EXTS.ts]);

  for (const f of rustFiles) {
    const c = readFile(f);
    for (const r of extractRustRoutes(f, c)) rustRoutes.push({ ...r, repo: repoName });
    for (const s of extractSubprocessCalls(f, c)) subprocessCalls.push({ ...s, repo: repoName });
    for (const u of extractUdpBinds(f, c)) udpBinds.push({ ...u, repo: repoName });
  }
  for (const f of jsFiles) {
    const c = readFile(f);
    for (const h of extractHttpClientCalls(f, c)) httpCalls.push({ ...h, repo: repoName });
    for (const s of extractSubprocessCalls(f, c)) subprocessCalls.push({ ...s, repo: repoName });
  }

  console.log(`    rust: ${rustFiles.length}, js/ts: ${jsFiles.length}`);
}

console.log(`\n[cross-process] extracted:`);
console.log(`  ${rustRoutes.length} Rust HTTP routes`);
console.log(`  ${httpCalls.length} JS/TS HTTP client calls`);
console.log(`  ${subprocessCalls.length} subprocess spawns`);
console.log(`  ${udpBinds.length} UDP binds`);

// ─── Build edges ──────────────────────────────────────────────────────────
const edges = [];

// HTTP edges: client path ↔ matching route path. Match rules:
//   HIGH confidence: exact path match
//   AMBIGUOUS:       client path starts with route path AND route path is
//                    non-trivial (>1 segment). Trivial routes like "/" or
//                    "/api" would spuriously match every call — skip them.
//   Methods must also match (or route method is catch-all).
function routeIsNonTrivial(routePath) {
  // Treat as trivial if it's just "/" or has <2 path segments.
  if (routePath === '/' || routePath.length <= 1) return false;
  const segs = routePath.split('/').filter(Boolean);
  return segs.length >= 2;
}

for (const call of httpCalls) {
  for (const route of rustRoutes) {
    const routePathNoParams = route.path.replace(/:\w+/g, '');
    const callPathTrimmed = call.url.split('?')[0];
    if (call.method !== route.method && call.method !== 'GET') continue; // rough method filter
    const exact = callPathTrimmed === route.path;
    const prefix = !exact
      && routeIsNonTrivial(routePathNoParams)
      && callPathTrimmed.startsWith(routePathNoParams);
    if (exact || prefix) {
      edges.push({
        source: `${call.repo}:${path.relative(REPO_ROOT, call.file).replace(/\\/g, '/')}:${call.line}`,
        target: `${route.repo}:${path.relative(REPO_ROOT, route.file).replace(/\\/g, '/')}:${route.line}`,
        source_label: `${call.method} ${call.url}`,
        target_label: `${route.method} ${route.path}`,
        relation: 'http_call',
        confidence_tier: exact ? 'HIGH' : 'AMBIGUOUS',
      });
    }
  }
}

// Subprocess edges: spawner → bin. Link to a Rust [[bin]] target if the bin name matches.
// For MVP we just emit edges regardless of matching target.
for (const s of subprocessCalls) {
  edges.push({
    source: `${s.repo}:${path.relative(REPO_ROOT, s.file).replace(/\\/g, '/')}:${s.line}`,
    target: `subprocess:${s.bin}`,
    source_label: `spawn(${s.bin})`,
    target_label: s.bin,
    relation: 'spawns_process',
    confidence_tier: 'AMBIGUOUS',
  });
}

// UDP edges: sender ↔ binder (for MVP we only emit binds; senders need additional extraction)
for (const u of udpBinds) {
  edges.push({
    source: `${u.repo}:${path.relative(REPO_ROOT, u.file).replace(/\\/g, '/')}:${u.line}`,
    target: `udp_port:${u.port}`,
    source_label: `UdpSocket::bind(:${u.port})`,
    target_label: `:${u.port}`,
    relation: 'binds_port',
    confidence_tier: 'HIGH',
  });
}

console.log(`  → ${edges.length} cross-process edges`);

// ─── Write output ─────────────────────────────────────────────────────────
const OUT_DIR = path.join(REPO_ROOT, 'graphify-out-cross-process');
fs.mkdirSync(OUT_DIR, { recursive: true });
const edgesPath = path.join(OUT_DIR, 'edges.json');
fs.writeFileSync(edgesPath, JSON.stringify({
  generated: new Date().toISOString(),
  rust_routes: rustRoutes.length,
  http_calls: httpCalls.length,
  subprocess_calls: subprocessCalls.length,
  udp_binds: udpBinds.length,
  edges,
}, null, 2));
console.log(`\nwrote: ${edgesPath}`);
