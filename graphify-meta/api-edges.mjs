#!/usr/bin/env node
// graphify-meta/api-edges.mjs
// Real inter-module edge discovery: grep each module's source for HTTP URL patterns
// like `/api/v1/...`, `fetch("/...")`, `axios.get(...)`, then match the URL path
// against racecontrol's route handler declarations to find genuine cross-module calls.
//
// Fixes session-3:
//   - Dedupe matched_routes.backend_files by path (was 3x copies per entry)
//   - Normalize absolute Windows paths out of backend_files
//   - Score tiebreaker: first-segment-prefix match wins score=1 ties
//   - Capture HTTP verb (GET/POST/DELETE/PUT/PATCH) when present before URL literal
//   - Back-edges: racecontrol → consumer modules (outgoing webhooks/relay calls)
//   - Axios baseURL resolution: detect axios.create({baseURL}) + relative paths
//
// Produces api-edges.json consumed by build-meta.mjs as a second edge channel.

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

const CONSUMER_MODULES = [
  'racingpoint-admin',
  'comms-link',
  'whatsapp-bot',
  'people-tracker',
  'marketing',
  'rc-ops-mcp',
  // racecontrol self-references are excluded — it's the backend
];
// Slice-level consumers: racecontrol's own frontends (kiosk/web/pwa) live INSIDE the
// racecontrol repo but are logically separate consumers of the backend API. Include
// them so meta-map shows frontend → backend coupling.
const SLICE_CONSUMERS = [
  { id: 'rc.kiosk', subdir: 'racecontrol/kiosk' },
  { id: 'rc.web',   subdir: 'racecontrol/web' },
  { id: 'rc.pwa',   subdir: 'racecontrol/pwa' },
];

// ---------------------------------------------------------------------------
// 1. Collect racecontrol route handlers
// ---------------------------------------------------------------------------

console.log('[1/5] Collecting racecontrol route handlers ...');
const rcGraph = JSON.parse(fs.readFileSync(path.join(ROOT, 'racecontrol/graphify-out/graph.json'), 'utf8'));

// Normalize a source_file: strip ROOT prefix + collapse to forward slashes + strip
// anything before 'crates/racecontrol/' or 'src/api/' so comparisons are consistent.
function normalizePath(raw) {
  if (!raw) return '';
  let s = raw.replace(/\\/g, '/');
  // Strip ROOT prefix if present (case-insensitive for drive letters on Windows)
  const rootLower = ROOT.toLowerCase();
  if (s.toLowerCase().startsWith(rootLower + '/')) {
    s = s.slice(ROOT.length + 1);
  } else if (s.toLowerCase().startsWith(rootLower)) {
    s = s.slice(ROOT.length);
  }
  // Strip leading 'racecontrol/' since all route files live under the server repo
  if (s.startsWith('racecontrol/')) s = s.slice('racecontrol/'.length);
  return s;
}

const routeNodes = rcGraph.nodes.filter(n => {
  const sf = normalizePath(n.source_file || '');
  return /^(crates\/racecontrol\/)?src\/api\//i.test(sf);
});

const routeHints = new Map(); // url_token → array of {sf, label, id}
for (const n of routeNodes) {
  const sf = normalizePath(n.source_file || '');
  const m = sf.match(/api\/([a-z_0-9]+)\.rs/i);
  if (!m) continue;
  const stem = m[1].replace(/^(api_)?/, '');
  const segments = stem.split('_').filter(Boolean);
  const urlTokens = new Set([
    stem,
    stem.replace(/_/g, '/'),
    stem.replace(/_/g, '-'),
    ...segments,
  ]);
  for (let i = 0; i < segments.length - 1; i++) {
    urlTokens.add(segments[i] + '/' + segments[i + 1]);
  }
  for (const tok of urlTokens) {
    if (!tok || tok.length < 3) continue;
    if (!routeHints.has(tok)) routeHints.set(tok, new Map());
    // Dedupe per token by sf to avoid the 3x-copies bug
    const bySf = routeHints.get(tok);
    if (!bySf.has(sf)) bySf.set(sf, { sf, label: n.label, id: n.id });
  }
}
console.log(`   ${routeNodes.length} backend route-handler nodes found`);
console.log(`   ${routeHints.size} distinct URL-tokens extracted from route paths`);

// ---------------------------------------------------------------------------
// 2. Walk consumer module source trees
// ---------------------------------------------------------------------------

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

const ROUTING_PREFIXES = new Set(['rc', 'v1', 'v2', 'v3']);

function normalizeUrl(raw) {
  return raw
    .replace(/\$\{[^}]*\}/g, '')
    .replace(/\/:[a-zA-Z0-9_]+/g, '')
    .replace(/\{[a-zA-Z0-9_]+\}/g, '')
    .replace(/\?.*$/, '')
    .replace(/\/\/+/g, '/')
    .replace(/\/$/, '');
}

function extractFragment(normUrl) {
  const m = normUrl.match(/^\/api\/(.+)$/i);
  if (!m) return null;
  const segs = m[1].split('/').filter(Boolean);
  let i = 0;
  while (i < segs.length && ROUTING_PREFIXES.has(segs[i].toLowerCase())) i++;
  if (i >= segs.length) return null;
  const first = segs[i];
  const compound = segs[i + 1] ? `${first}/${segs[i + 1]}` : null;
  return { first, compound, tail: segs.slice(i).join('/') };
}

// ---------------------------------------------------------------------------
// 3. Scan a consumer module for /api/ URL literals + HTTP verbs
// ---------------------------------------------------------------------------

// Match: fetch("/api/..."), axios.get("/api/..."), fetchApi("/api/...", {method:'POST'}),
// apiCall("/api/..."), rpc.post("/api/...")
const HTTP_VERB_RE = /\b(fetch|axios|request|fetchApi|apiCall|apiFetch|rpc|http|client|api)\s*(?:\.(get|post|put|patch|delete|head))?\s*\(\s*["'`](\/api\/[A-Za-z0-9_\-\/\{\}\$\:\.]+)/g;
const BARE_URL_RE = /["'`](\/api\/[A-Za-z0-9_\-\/\{\}\$\:\.]+)/g;
const METHOD_IN_INIT_RE = /method\s*:\s*["'](GET|POST|PUT|PATCH|DELETE|HEAD)["']/i;
const AXIOS_CREATE_RE = /axios\.create\s*\(\s*\{[^}]*baseURL\s*:\s*["'`]([^"'`]+)["'`]/g;

function grepModule(modDir) {
  const matches = [];
  const files = walkSources(modDir);
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    // Pass 1: verbed URL literals (fetch/axios/request w/ method)
    let m;
    HTTP_VERB_RE.lastIndex = 0;
    while ((m = HTTP_VERB_RE.exec(src)) !== null) {
      const verb = (m[2] || 'get').toUpperCase();
      const url = m[3];
      matches.push({
        file: f, url, verb,
        line_start: src.slice(0, m.index).split('\n').length,
      });
    }
    // Pass 2: bare /api/ URLs (catches templates like `${API}/api/foo` that verbed pass skipped)
    BARE_URL_RE.lastIndex = 0;
    const verbedUrls = new Set(matches.filter(x => x.file === f).map(x => x.url));
    while ((m = BARE_URL_RE.exec(src)) !== null) {
      const url = m[1];
      if (verbedUrls.has(url)) continue;
      // Try to infer verb from nearby `method:` within 300 chars after the URL,
      // OR from a `.get(...)` / `.post(...)` chain within 80 chars before the URL.
      const after = src.slice(m.index, m.index + 300);
      const before = src.slice(Math.max(0, m.index - 80), m.index);
      const methodInInit = after.match(METHOD_IN_INIT_RE);
      const chainVerb = before.match(/\.(get|post|put|patch|delete|head)\s*\(\s*$/i);
      const verb = methodInInit ? methodInInit[1].toUpperCase()
                 : chainVerb ? chainVerb[1].toUpperCase()
                 : 'UNKNOWN';
      matches.push({
        file: f, url, verb,
        line_start: src.slice(0, m.index).split('\n').length,
      });
    }
    // Pass 3: axios.create baseURL — if baseURL ends in '/api' or '/api/vN',
    // treat relative paths like axios.get('/fleet/health') as /api/fleet/health
    AXIOS_CREATE_RE.lastIndex = 0;
    while ((m = AXIOS_CREATE_RE.exec(src)) !== null) {
      const base = m[1];
      const apiPrefix = base.match(/\/api(\/v\d+)?$/);
      if (!apiPrefix) continue;
      // Now scan the SAME file for axios.get/post/etc. with relative paths not starting with http
      const relRe = /\baxios\s*\.(get|post|put|patch|delete)\s*\(\s*["'`]([^"'`]+)["'`]/g;
      let r;
      while ((r = relRe.exec(src)) !== null) {
        const url = r[2];
        if (/^https?:/.test(url)) continue;
        if (url.startsWith('/api/')) continue; // already caught
        const fullUrl = (base.replace(/\/$/, '') + '/' + url.replace(/^\//, '')).replace(/\/\/+/g, '/');
        matches.push({
          file: f, url: fullUrl, verb: r[1].toUpperCase(),
          line_start: src.slice(0, r.index).split('\n').length,
          inferred_from: `axios.create baseURL=${base}`,
        });
      }
    }
  }
  return matches;
}

// ---------------------------------------------------------------------------
// 4. Match fragments against routeHints with tiebreaker scoring
// ---------------------------------------------------------------------------

function scoreMatch(tok, candidate) {
  // Orderless segment-overlap + prefix bonus
  const tokSegs = tok.split(/[_/]/).filter(Boolean);
  const normSegs = (candidate || '').split(/[_/]/).filter(Boolean);
  const normSet = new Set(normSegs);
  let score = 0;
  for (const seg of tokSegs) if (normSet.has(seg)) score++;
  if (score === 0) return 0;
  // Prefix bonus: if the first token segment matches the first URL segment exactly,
  // add 0.5 — prefers `auth_staff` over `customer_auth` for URL `/api/auth/login`.
  if (tokSegs[0] && normSegs[0] && tokSegs[0] === normSegs[0]) score += 0.5;
  return score;
}

// ---------------------------------------------------------------------------
// 5. Main scan loop — forward edges (consumer → racecontrol)
// ---------------------------------------------------------------------------

console.log('[2/5] Grepping consumer modules for URL patterns ...');
const urlEdges = [];

// Build the consumer list: sibling repos + slice-level frontends under racecontrol.
const ALL_CONSUMERS = [
  ...CONSUMER_MODULES.map(id => ({ id, modDir: path.join(ROOT, id) })),
  ...SLICE_CONSUMERS.map(s => ({ id: s.id, modDir: path.join(ROOT, s.subdir) })),
];

for (const { id: mod, modDir } of ALL_CONSUMERS) {
  if (!fs.existsSync(modDir)) {
    console.log(`   ${mod}: (missing)`);
    continue;
  }
  const matches = grepModule(modDir);
  console.log(`   ${mod}: ${matches.length} URL literals found`);

  // Aggregate per url_fragment + verb — preserves distinct GET vs POST on same path
  const urlsFound = new Map();
  for (const m of matches) {
    const norm = normalizeUrl(m.url);
    const frag = extractFragment(norm);
    if (!frag || !frag.first) continue;
    const key = `${frag.first}:${m.verb}`;
    if (!urlsFound.has(key)) {
      urlsFound.set(key, { ...frag, verb: m.verb, url: m.url, norm, file: m.file, line: m.line_start, inferred_from: m.inferred_from });
    }
  }

  for (const [, loc] of urlsFound) {
    const candidates = [loc.first, loc.compound, loc.tail].filter(Boolean);
    const matchedRoutes = new Map(); // backend_file → best match record
    for (const cand of candidates) {
      const normalized = cand.toLowerCase().replace(/-/g, '_');
      for (const [tok, bySf] of routeHints) {
        if (tok.length < 3) continue;
        const score = scoreMatch(tok, normalized);
        if (score <= 0) continue;
        for (const [sf, hit] of bySf) {
          const existing = matchedRoutes.get(sf);
          if (!existing || score > existing.score) {
            matchedRoutes.set(sf, { url_token: tok, score, backend_file: sf, label: hit.label });
          }
        }
      }
    }
    if (matchedRoutes.size === 0) continue;
    // Rank by score desc, take top 3, output one backend_file each (no duplicates)
    const ranked = [...matchedRoutes.values()].sort((a, b) => b.score - a.score).slice(0, 3);
    const consumerFileRel = loc.file.replace(ROOT, '').replace(/\\/g, '/');
    // Tag edge source: test-file URLs reveal contract coverage but shouldn't inflate
    // production-coupling metrics. /test|spec|__tests__|mock|fixture/ in path = 'test'.
    const edgeSource = /\/(tests?|specs?|__tests__|mocks?|fixtures?)\//i.test(consumerFileRel)
      ? 'test' : 'production';
    urlEdges.push({
      from: mod, to: 'racecontrol',
      type: 'http-api-call',
      verb: loc.verb,
      url_fragment: loc.first,
      url_path: loc.tail,
      url_normalized: loc.norm,
      edge_source: edgeSource,
      matched_routes: ranked.map(r => ({
        url_token: r.url_token,
        score: r.score,
        backend_file: r.backend_file,
        backend_label: r.label,
      })),
      consumer_file: consumerFileRel,
      consumer_line: loc.line,
      url_sample: loc.url,
      ...(loc.inferred_from ? { inferred_from: loc.inferred_from } : {}),
    });
  }
}

console.log(`[3/5] Discovered ${urlEdges.length} forward URL-based edges from ${[...new Set(urlEdges.map(e => e.from))].length} consumer modules`);

// ---------------------------------------------------------------------------
// 6. Back-edges: racecontrol → consumer modules
// ---------------------------------------------------------------------------
// Patterns to discover:
//   - racecontrol calls comms-link relay     → racecontrol → comms-link
//   - racecontrol posts to whatsapp-bot webhook → racecontrol → whatsapp-bot
//   - racecontrol spawns pod-agent subprocess  → racecontrol → pod-agent
//   - racecontrol fetches Bono VPS endpoint   → racecontrol → (bono-vps)

console.log('[4/5] Scanning racecontrol for outgoing cross-module URLs ...');
const backEdges = [];

const BACK_EDGE_TARGETS = [
  { mod: 'comms-link',     pattern: /["'`](http:\/\/(?:localhost|127\.0\.0\.1|100\.82\.33\.94|100\.70\.177\.44):876[56]\/[^"'`]+)/g },
  { mod: 'whatsapp-bot',   pattern: /["'`](https?:\/\/[^"'`]*(?:evolution|whatsapp)[^"'`]*\/[^"'`]+)/gi },
  { mod: 'people-tracker', pattern: /["'`](https?:\/\/[^"'`]*:8095\/[^"'`]+)/g },
  // :8091 is rc-sentry (a slice of racecontrol itself). Represent as rc.rc-sentry so
  // meta-map viz can draw the self-loop through the sliced sub-node correctly.
  { mod: 'rc.rc-sentry',   pattern: /["'`](https?:\/\/[^"'`]*:8091\/[^"'`]+)/g },
  // :8090 is rc-agent on each pod
  { mod: 'rc.rc-agent',    pattern: /["'`](https?:\/\/[^"'`]*:8090\/[^"'`]+)/g },
];

const rcFiles = walkSources(path.join(ROOT, 'racecontrol'));
for (const target of BACK_EDGE_TARGETS) {
  const seen = new Set();
  for (const f of rcFiles) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    target.pattern.lastIndex = 0;
    let m;
    while ((m = target.pattern.exec(src)) !== null) {
      const url = m[1];
      const key = `${target.mod}:${url}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const consumerFileRel = f.replace(ROOT, '').replace(/\\/g, '/');
      const edgeSource = /\/(tests?|specs?|__tests__|mocks?|fixtures?)\//i.test(consumerFileRel)
        ? 'test' : 'production';
      backEdges.push({
        from: 'racecontrol', to: target.mod,
        type: 'http-api-call',
        verb: 'UNKNOWN',
        url_sample: url,
        edge_source: edgeSource,
        consumer_file: consumerFileRel,
        consumer_line: src.slice(0, m.index).split('\n').length,
        direction: 'back-edge',
      });
      if (seen.size >= 20) break; // bound per target
    }
    if (seen.size >= 20) break;
  }
}

console.log(`   ${backEdges.length} back-edges discovered`);

// ---------------------------------------------------------------------------
// 7. Emit
// ---------------------------------------------------------------------------

const allEdges = [...urlEdges, ...backEdges];
console.log(`[5/5] Total edges: ${allEdges.length} (${urlEdges.length} forward, ${backEdges.length} back)`);

const out = path.join(__dirname, 'api-edges.json');
fs.writeFileSync(out, JSON.stringify({ generated_at: new Date().toISOString(), edges: allEdges }, null, 2));
console.log(`Wrote ${out}`);

// Summary
const byFrom = new Map();
for (const e of allEdges) {
  const key = e.from;
  if (!byFrom.has(key)) byFrom.set(key, new Set());
  byFrom.get(key).add(e.url_fragment || e.url_sample);
}
console.log();
console.log('Per-source distinct URL count:');
for (const [mod, frags] of byFrom) {
  console.log(`  ${mod.padEnd(24)} ${frags.size} distinct URLs`);
}
