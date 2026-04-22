#!/usr/bin/env node
// scripts/graphify-find-orphan-calls.mjs — one-off bug hunt.
//
// Find frontend HTTP calls (fetch, axios) that don't resolve to any Rust
// axum route declaration. Uses the same regex approach as the cross-process
// extractor but emits RAW unmatched calls for human review, not just edges.

import fs from 'node:fs';
import path from 'node:path';

const SKIP = new Set([
  'node_modules', '.git', 'target', '.next', 'dist', 'build',
  'graphify-out', 'graphify-out-unified', 'graphify-out-cross-process',
  'graphify-out-static-extras', '.planning', 'deploy-staging', 'worktrees',
  'bono-mirror', 'backups', 'audit', 'docs', 'racecontrol-f2',
  // Graphify tooling — contains regex patterns that mimic HTTP calls.
  'graphify-meta', 'graphify-benchmark', 'graphify-helpers', 'graphify-post',
  // Agent-spawned worktree scratch space — shadow copies of the repo.
  '.claude',
]);

function walk(d, exts, out = []) {
  let es;
  try { es = fs.readdirSync(d, { withFileTypes: true }); } catch { return out; }
  for (const e of es) {
    if (SKIP.has(e.name)) continue;
    const f = path.join(d, e.name);
    if (e.isDirectory()) walk(f, exts, out);
    else if (e.isFile() && exts.includes(path.extname(e.name))) out.push(f);
  }
  return out;
}

// Pass 1: gather route paths.
// 1a) Rust axum `.route("/path", ...)` declarations.
const routeRe = /\.route\s*\(\s*["']([^"']+)["']/g;
const routes = new Set();
for (const f of walk('.', ['.rs'])) {
  const c = fs.readFileSync(f, 'utf-8');
  let m;
  while ((m = routeRe.exec(c)) !== null) routes.add(m[1]);
}

// 1b) Next.js app-router handlers: src/app/api/**/route.{ts,tsx,js,jsx}
//     = HTTP route at /api/**. Derive URL from file path; support [param]->:param
//     and [...slug] catch-all wildcards.
for (const f of walk('.', ['.ts', '.tsx', '.js', '.jsx'])) {
  const fn = f.replace(/\\/g, '/');
  if (!/\/app\/api\/.*\/route\.(ts|tsx|js|jsx)$/.test(fn)) continue;
  const url = fn
    .replace(/^.*?\/app(\/api\/.*)\/route\.(ts|tsx|js|jsx)$/, '$1')
    .replace(/\/\[\.\.\..*?\]/g, '/*')
    .replace(/\/\[(\w+)\]/g, '/:$1');
  routes.add(url);
}

// 1c) Next.js catch-all proxy: /api/rc/[...path]/route.ts rewrites to racecontrol
// at PROXY_PREFIX + first-segment. If racecontrol has /api/v1/fleet/health and the
// proxy forwards /api/rc/fleet/health → /api/v1/fleet/health, then /api/rc/fleet/health
// is NOT an orphan. We mark /api/rc/** as resolved if the rewritten target exists.
console.log(`routes declared: ${routes.size}`);

// Pass 2: gather frontend calls.
const calls = [];
const patterns = [
  /\bfetch\s*\(\s*[`"']([^`"']+)[`"']/g,
  /\baxios\s*\.\s*(?:get|post|put|delete|patch)\s*\(\s*[`"']([^`"']+)[`"']/g,
  /\baxios\s*\(\s*\{\s*url\s*:\s*[`"']([^`"']+)[`"']/g,
];
for (const f of walk('.', ['.ts', '.tsx', '.js', '.jsx', '.mjs'])) {
  const c = fs.readFileSync(f, 'utf-8');
  const lines = c.split('\n');
  for (let i = 0; i < lines.length; i++) {
    for (const re of patterns) {
      let m;
      while ((m = re.exec(lines[i])) !== null) {
        const url = m[1];
        if (url.startsWith('/') && !url.startsWith('/_next') && !url.startsWith('//')) {
          calls.push({ file: f.replace(/\\/g, '/'), line: i + 1, url: url.split('?')[0] });
        }
      }
    }
  }
}
console.log(`frontend calls: ${calls.length}`);

// Pass 3: classify.
//
// Next.js rewrite conventions: some apps front axum routes via same-origin
// rewrites in next.config.ts. We encode those conventions so frontend calls
// through a documented proxy don't false-flag as orphans.
//   - /api/rc/*  → /api/v1/*   (admin-app convention, Phase 367 scaffolding)
//   - /api/*     → /api/*      (kiosk/web passthrough — already direct match)
const REWRITES = [
  { from: /^\/api\/rc\//, to: '/api/v1/' },
];

// Normalize both sides to a wildcard. Frontend URLs contain `${var}` template
// literals from `fetch(\`/api/x/${id}/y\`)`. Axum routes contain `{param}` (axum
// 0.7+) or `:param` placeholders. Reduce all three to a single wildcard token.
function normalize(url) {
  return url
    .replace(/\$\{[^}]+\}/g, '__WILD__')  // JS template vars
    .replace(/\{[^}]+\}/g, '__WILD__')    // axum 0.7+ params
    .replace(/:\w+/g, '__WILD__');        // axum/express :param
}

// Axum nest("/api/v1", ...) prefixes — scanner reads inner `.route()` strings
// so the routes set contains "/admin/x" rather than "/api/v1/admin/x". Strip
// known mount prefixes from candidates before comparing.
const NEST_PREFIXES = ['/api/v1', '/api'];

function routeMatches(url) {
  const candidates = [url];
  for (const rw of REWRITES) {
    if (rw.from.test(url)) candidates.push(url.replace(rw.from, rw.to));
  }
  // For each candidate, also try the path with a known mount prefix stripped.
  const expanded = candidates.slice();
  for (const c of candidates) {
    for (const p of NEST_PREFIXES) {
      if (c.startsWith(p + '/')) expanded.push(c.slice(p.length));
    }
  }
  return matchAny(expanded);
}

function matchAny(candidates) {
  for (const cand of candidates) {
    const normCand = normalize(cand);
    for (const r of routes) {
      if (normalize(r) === normCand) return true;
      // Wildcard catch-all handlers (Next.js [...slug] becomes /*).
      if (r.endsWith('/*')) {
        const prefix = normalize(r.slice(0, -2));
        if (normCand.startsWith(prefix + '/')) return true;
      }
    }
  }
  return false;
}
const orphans = calls.filter(c => !routeMatches(c.url));
console.log(`orphan calls (no matching route): ${orphans.length}\n`);

const byUrl = {};
for (const o of orphans) (byUrl[o.url] ||= []).push(o);
const sorted = Object.entries(byUrl).sort((a, b) => b[1].length - a[1].length);
console.log('top 20 orphan URLs:');
for (const [url, occ] of sorted.slice(0, 20)) {
  console.log(`  ${String(occ.length).padStart(3)} ${url}`);
  console.log(`      e.g. ${occ[0].file}:${occ[0].line}`);
}
