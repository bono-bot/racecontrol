#!/usr/bin/env node
// graphify-meta/api-edges.mjs
// Real inter-module edge discovery: grep each module's source for HTTP URL patterns
// like `/api/v1/...`, `fetch("/...")`, `axios.get(...)`, then match the URL path
// against racecontrol's route handler declarations to find genuine cross-module calls.
//
// Produces api-edges.json consumed by build-meta.mjs as a second edge channel.

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

// This script can live either at `<repo-root>/graphify-meta/` (legacy) or at
// `<repo-root>/racecontrol/graphify-meta/` (current). Resolve ROOT by walking
// up from the script until we find a directory that contains both 'racecontrol'
// and 'racingpoint-admin' subdirectories. Fall back to env var GRAPHIFY_META_ROOT.
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

const CONSUMER_MODULES = [
  'racingpoint-admin',
  'comms-link',
  'whatsapp-bot',
  'people-tracker',
  'marketing',
  'rc-ops-mcp',
  // racecontrol self-references are excluded — it's the backend
];

// Collect racecontrol route declarations from its graph.json (backend routes
// live in /api/* paths per convention).
console.log('[1/3] Collecting racecontrol route handlers ...');
const rcGraph = JSON.parse(fs.readFileSync(path.join(ROOT, 'racecontrol/graphify-out/graph.json'), 'utf8'));
const routeNodes = rcGraph.nodes.filter(n => {
  const sf = (n.source_file || '').replace(/\\/g, '/');
  return /crates\/racecontrol\/src\/api\//i.test(sf);
});
// Extract probable route-URL tokens from source_file path
// e.g. api/admin_gamification.rs → routes under /api/admin/gamification, /api/gamification, /api/admin
const routeHints = new Map(); // url_fragment → array of {file, node}
for (const n of routeNodes) {
  const sf = (n.source_file || '').replace(/\\/g, '/');
  const m = sf.match(/api\/([a-z_0-9]+)\.rs/i);
  if (!m) continue;
  const stem = m[1].replace(/^(api_)?/, '');
  const segments = stem.split('_').filter(Boolean);
  // Emit every segment as a candidate token PLUS common combinations so both
  // /api/staff (→ auth_staff.rs) and /api/auth (→ auth_staff.rs) match.
  const urlTokens = new Set([
    stem,
    stem.replace(/_/g, '/'),
    stem.replace(/_/g, '-'),
    ...segments,                          // every segment: [auth, staff]
  ]);
  // For multi-segment files also emit adjacent pair joins (e.g. billing_session → billing/session)
  for (let i = 0; i < segments.length - 1; i++) {
    urlTokens.add(segments[i] + '/' + segments[i + 1]);
  }
  for (const tok of urlTokens) {
    if (!tok || tok.length < 3) continue;  // skip noise like "v1"
    if (!routeHints.has(tok)) routeHints.set(tok, []);
    routeHints.get(tok).push({ sf, label: n.label, id: n.id });
  }
}
console.log(`   ${routeNodes.length} backend route-handler nodes found`);
console.log(`   ${routeHints.size} distinct URL-tokens extracted from route paths`);

// For each consumer module, grep for URL-like patterns and match against route hints
console.log('[2/3] Grepping consumer modules for URL patterns ...');
const urlEdges = [];

// Walk a dir for relevant source files, returning paths
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
    } else if (/\.(ts|tsx|js|mjs|cjs|py|rs)$/i.test(ent.name)) {
      acc.push(p);
    }
  }
  return acc;
}

// Routing-group prefixes that appear between /api/ and the real resource segment.
// e.g. /api/rc/staff — "rc" is a namespace group, the real handler is for "staff".
const ROUTING_PREFIXES = new Set(['rc', 'v1', 'v2', 'v3']);

// Normalize a URL path: strip template params, query strings, trailing slash.
// /api/drivers/${id}?foo=1  → /api/drivers
// /api/rc/staff/:id/validate-pin → /api/rc/staff/validate-pin
function normalizeUrl(raw) {
  let u = raw
    .replace(/\$\{[^}]*\}/g, '')       // ${id}
    .replace(/\/:[a-zA-Z0-9_]+/g, '')  // /:id
    .replace(/\{[a-zA-Z0-9_]+\}/g, '') // {id}
    .replace(/\?.*$/, '')              // query string
    .replace(/\/\/+/g, '/')            // collapse slashes produced by stripping
    .replace(/\/$/, '');               // trailing slash
  return u;
}

// Extract the canonical resource fragment from an /api/... URL.
// Returns the FIRST non-prefix segment after /api/ (or first segment if no prefix).
// /api/v1/health           → 'health'
// /api/rc/staff            → 'staff'
// /api/rc/pricing/rules    → 'pricing'  (with compound 'pricing/rules' returned as second element)
function extractFragment(normUrl) {
  const m = normUrl.match(/^\/api\/(.+)$/i);
  if (!m) return null;
  const segs = m[1].split('/').filter(Boolean);
  // Skip leading routing-group prefixes
  let i = 0;
  while (i < segs.length && ROUTING_PREFIXES.has(segs[i].toLowerCase())) i++;
  if (i >= segs.length) return null;
  const first = segs[i];
  const compound = segs[i + 1] ? `${first}/${segs[i + 1]}` : null;
  return { first, compound, tail: segs.slice(i).join('/') };
}

// Scan a module for URL patterns via regex over file contents
function grepModule(modDir) {
  const matches = [];
  const files = walkSources(modDir);
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    // Capture /api/.../... URL paths — allow template characters in the string body.
    const urlRe = /["'`](\/api\/[A-Za-z0-9_\-\/\{\}\$\:\.]+)/g;
    let m;
    while ((m = urlRe.exec(src)) !== null) {
      matches.push({ file: f, url: m[1], line_start: src.slice(0, m.index).split('\n').length });
    }
  }
  return matches;
}

for (const mod of CONSUMER_MODULES) {
  const modDir = path.join(ROOT, mod);
  if (!fs.existsSync(modDir)) {
    console.log(`   ${mod}: (missing)`);
    continue;
  }
  const matches = grepModule(modDir);
  console.log(`   ${mod}: ${matches.length} URL literals found`);

  // Deduplicate URL fragments. Key by the normalized canonical fragment so
  // /api/rc/staff and /api/rc/staff/${id} collapse into one edge.
  const urlsFound = new Map();
  for (const m of matches) {
    const norm = normalizeUrl(m.url);
    const frag = extractFragment(norm);
    if (!frag || !frag.first) continue;
    const key = frag.first;
    if (!urlsFound.has(key)) {
      urlsFound.set(key, { url: m.url, norm, first: frag.first, compound: frag.compound, tail: frag.tail, file: m.file, line: m.line_start });
    }
  }
  // Match fragments against routeHints.
  // Try both the first segment ("staff") and the compound ("pricing/rules")
  // so that multi-segment resource routes still match multi-word backend files.
  for (const [, loc] of urlsFound) {
    const candidates = [loc.first, loc.compound, loc.tail].filter(Boolean);
    const matchedRoutes = new Map();  // key → {url_token, backend_files}
    for (const cand of candidates) {
      const normalized = cand.toLowerCase().replace(/-/g, '_');
      for (const [tok, list] of routeHints) {
        if (tok.length < 3) continue;
        if (tok === normalized ||
            tok === normalized.replace(/\//g, '_') ||
            tok.startsWith(normalized + '_') ||
            normalized.startsWith(tok + '_') ||
            tok.includes('_' + normalized) ||
            normalized.includes('_' + tok)) {
          const key = tok;
          if (!matchedRoutes.has(key)) {
            matchedRoutes.set(key, { url_token: tok, backend_files: list.slice(0, 3).map(x => x.sf) });
          }
        }
      }
    }
    if (matchedRoutes.size > 0) {
      urlEdges.push({
        from: mod, to: 'racecontrol',
        type: 'http-api-call',
        url_fragment: loc.first,
        url_path: loc.tail,
        url_normalized: loc.norm,
        matched_routes: [...matchedRoutes.values()].slice(0, 3),
        consumer_file: loc.file.replace(ROOT, '').replace(/\\/g, '/'),
        consumer_line: loc.line,
        url_sample: loc.url,
      });
    }
  }
}

console.log(`[3/3] Discovered ${urlEdges.length} URL-based edges ${[...new Set(urlEdges.map(e => e.from))].length} consumer modules`);

const out = path.join(ROOT, 'graphify-meta', 'api-edges.json');
fs.writeFileSync(out, JSON.stringify({ generated_at: new Date().toISOString(), edges: urlEdges }, null, 2));
console.log(`Wrote ${out}`);

// Summary per consumer
const byFrom = new Map();
for (const e of urlEdges) {
  const key = e.from;
  if (!byFrom.has(key)) byFrom.set(key, new Set());
  byFrom.get(key).add(e.url_fragment);
}
console.log();
console.log('Per-consumer URL fragment count:');
for (const [mod, frags] of byFrom) {
  console.log(`  ${mod.padEnd(24)} ${frags.size} distinct /api/${'{'}fragment${'}'} paths matched`);
}
