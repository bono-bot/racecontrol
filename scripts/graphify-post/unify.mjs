#!/usr/bin/env node
// racecontrol/scripts/graphify-post/unify.mjs
// Tier 2 cross-repo unification: emits `cross_repo` edges between matching
// non-hub labels across sibling graphify graphs. Output written to a merged
// graph.json that downstream tooling (query, path, explain) can traverse.
//
// Hub-noise filter: a label that appears in ≥2 repos is only linked if its
// degree is BELOW the 90th percentile in each source graph. This excludes
// `main()`, `GET()`, `ok()`, `spawn()`, etc — which share a name in every
// Rust repo but mean different things in each context.
//
// Usage:
//   node scripts/graphify-post/unify.mjs           # default GRAPHS list
//   node scripts/graphify-post/unify.mjs --out <dir>
//
// Default output: graphify-out-unified/ in repo root.

import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '../../');

const GRAPHS = [
  { repo: 'racecontrol',        path: 'graphify-out/graph.json' },
  { repo: 'comms-link',         path: '../comms-link/graphify-out/graph.json' },
  { repo: 'admin',              path: '../racingpoint-admin/graphify-out/graph.json' },
  { repo: 'whatsapp-bot',       path: '../whatsapp-bot/graphify-out/graph.json' },
  { repo: 'pod-agent',          path: '../pod-agent/graphify-out/graph.json' },
  { repo: 'rc-ops-mcp',         path: '../rc-ops-mcp/graphify-out/graph.json' },
  { repo: 'people-tracker',     path: '../people-tracker/graphify-out/graph.json' },
  { repo: 'marketing',          path: '../marketing/graphify-out/graph.json' },
  { repo: 'mcp-gmail',          path: '../racingpoint-mcp-gmail/graphify-out/graph.json' },
  { repo: 'mcp-drive',          path: '../racingpoint-mcp-drive/graphify-out/graph.json' },
  { repo: 'mcp-calendar',       path: '../racingpoint-mcp-calendar/graphify-out/graph.json' },
  { repo: 'mcp-sheets',         path: '../racingpoint-mcp-sheets/graphify-out/graph.json' },
  { repo: 'claude-skills',      path: '../../.claude/skills/graphify-out/graph.json' },
  { repo: 'memory',             path: '../../.claude/projects/C--Users-bono/memory/graphify-out/graph.json' },
  // P1.5 (2026-04-22): Bono-side graphs mirrored via scp into bono-mirror/.
  // Refresh cadence: whenever graphify_bono_all is rerun on Bono (~daily via cron
  // once P6.2 lands). Repos are SEPARATE from James-side siblings (whatsapp-bot vs
  // racingpoint-whatsapp-bot, etc) — same project family, different codebases.
  { repo: 'bono-whatsapp-bot',    path: '../bono-mirror/racingpoint-whatsapp-bot-bono/graphify-out/graph.json' },
  { repo: 'bono-discord-bot',     path: '../bono-mirror/racingpoint-discord-bot-bono/graphify-out/graph.json' },
  { repo: 'bono-hiring-bot',      path: '../bono-mirror/racingpoint-hiring-bot/graphify-out/graph.json' },
  { repo: 'bono-cloud-dashboard', path: '../bono-mirror/racingpoint-cloud-dashboard/graphify-out/graph.json' },
  { repo: 'bono-google',          path: '../bono-mirror/racingpoint-google/graphify-out/graph.json' },
  { repo: 'bono-dashboard',       path: '../bono-mirror/racingpoint-dashboard/graphify-out/graph.json' },
  { repo: 'bono-api-gateway',     path: '../bono-mirror/racingpoint-api-gateway/graphify-out/graph.json' },
  { repo: 'bono-meta-corpus',     path: '../bono-mirror/meta-corpus/graphify-out/graph.json' },
];

const OUT_DIR = process.argv.includes('--out')
  ? process.argv[process.argv.indexOf('--out') + 1]
  : path.join(REPO_ROOT, 'graphify-out-unified');

// -----------------------------------------------------------------------------
// 1) Load every graph that exists. Missing ones are logged and skipped.
// -----------------------------------------------------------------------------

const loaded = [];
for (const g of GRAPHS) {
  const abs = path.resolve(REPO_ROOT, g.path);
  if (!fs.existsSync(abs)) {
    console.warn(`skip: ${g.repo} (${abs} missing)`);
    continue;
  }
  const graph = JSON.parse(fs.readFileSync(abs, 'utf8'));
  loaded.push({ ...g, abs, graph });
  console.log(`loaded: ${g.repo} — ${graph.nodes.length} nodes, ${graph.links.length} edges`);
}

if (loaded.length < 2) {
  console.error('unify needs at least 2 graphs; aborting');
  process.exit(1);
}

// -----------------------------------------------------------------------------
// 2) Per-graph, compute 90th-percentile degree as hub-noise filter threshold.
// -----------------------------------------------------------------------------

for (const L of loaded) {
  const deg = new Map();
  for (const e of L.graph.links) {
    deg.set(e.source, (deg.get(e.source) || 0) + 1);
    deg.set(e.target, (deg.get(e.target) || 0) + 1);
  }
  L.degreeOf = deg;
  const sorted = [...deg.values()].sort((a, b) => a - b);
  L.p90 = sorted.length ? sorted[Math.floor(sorted.length * 0.9)] : 0;
}

// -----------------------------------------------------------------------------
// 3) Build label→[{repo, node, degree}] map. Labels from `label` or `id`.
// -----------------------------------------------------------------------------

const labelMap = new Map();
for (const L of loaded) {
  for (const n of L.graph.nodes) {
    const label = (n.label || n.id || '').toString();
    if (!label) continue;
    if (!labelMap.has(label)) labelMap.set(label, []);
    labelMap.get(label).push({
      repo: L.repo,
      node: n,
      degree: L.degreeOf.get(n.id) || 0,
      p90: L.p90,
    });
  }
}

// -----------------------------------------------------------------------------
// 4) Emit cross_repo edges. Rule: label appears in ≥2 repos AND each occurrence
//    has degree < that repo's p90 (filters hub-noise like `main()`, `GET()`).
// -----------------------------------------------------------------------------

const crossRepoEdges = [];
let hubSuppressed = 0;
let tooRareSuppressed = 0;

for (const [label, occurrences] of labelMap.entries()) {
  if (occurrences.length < 2) continue;

  const belowP90 = occurrences.filter(o => o.degree < o.p90);
  if (belowP90.length < 2) {
    if (occurrences.some(o => o.degree >= o.p90)) hubSuppressed++;
    else tooRareSuppressed++;
    continue;
  }

  // Star pattern: link the highest-degree non-hub occurrence to every other
  // below-p90 occurrence. Keeps edge count O(N) per label rather than O(N^2).
  belowP90.sort((a, b) => b.degree - a.degree);
  const hub = belowP90[0];
  for (let i = 1; i < belowP90.length; i++) {
    const other = belowP90[i];
    crossRepoEdges.push({
      source: `${hub.repo}::${hub.node.id}`,
      target: `${other.repo}::${other.node.id}`,
      relation: 'cross_repo_label_match',
      confidence: 0.5,
      confidence_tier: 'AMBIGUOUS',
      via_label: label,
    });
  }
}

// -----------------------------------------------------------------------------
// 5) Assemble unified graph. Node IDs are namespaced `<repo>::<origId>`.
// -----------------------------------------------------------------------------

const unifiedNodes = [];
const unifiedEdges = [];

for (const L of loaded) {
  for (const n of L.graph.nodes) {
    unifiedNodes.push({
      ...n,
      id: `${L.repo}::${n.id}`,
      source_repo: L.repo,
    });
  }
  for (const e of L.graph.links) {
    unifiedEdges.push({
      ...e,
      source: `${L.repo}::${e.source}`,
      target: `${L.repo}::${e.target}`,
    });
  }
}

for (const e of crossRepoEdges) {
  unifiedEdges.push(e);
}

const unified = {
  nodes: unifiedNodes,
  links: unifiedEdges,
  schema_version: 1,
  unified_from: loaded.map(l => ({ repo: l.repo, nodes: l.graph.nodes.length, edges: l.graph.links.length })),
  cross_repo_edges_added: crossRepoEdges.length,
};

// -----------------------------------------------------------------------------
// 6) Write output + summary report.
// -----------------------------------------------------------------------------

if (!fs.existsSync(OUT_DIR)) fs.mkdirSync(OUT_DIR, { recursive: true });

const outPath = path.join(OUT_DIR, 'graph.json');
fs.writeFileSync(outPath, JSON.stringify(unified, null, 2));
console.log(`\nwrote: ${outPath}`);
console.log(`  unified: ${unifiedNodes.length} nodes, ${unifiedEdges.length} edges`);
console.log(`  cross_repo_edges: ${crossRepoEdges.length} added`);
console.log(`  hub-noise suppressed: ${hubSuppressed} labels (≥p90 in some repo)`);
console.log(`  single-repo only: ${tooRareSuppressed} labels`);

const report = `# Unified Graph Report

**Generated:** ${new Date().toISOString()}
**Source graphs:** ${loaded.length}
**Unified nodes:** ${unifiedNodes.length}
**Unified edges:** ${unifiedEdges.length}
**Cross-repo edges added:** ${crossRepoEdges.length}
**Hub-noise labels suppressed:** ${hubSuppressed} (appeared in some repo at ≥ p90 degree)
**Single-repo labels:** ${tooRareSuppressed}

## Per-repo inventory

| Repo | Nodes | Edges | p90 degree |
|------|-------|-------|------------|
${loaded.map(l => `| ${l.repo} | ${l.graph.nodes.length} | ${l.graph.links.length} | ${l.p90} |`).join('\n')}

## Sample cross-repo edges

${crossRepoEdges.slice(0, 20).map(e => `- \`${e.via_label}\`: ${e.source} ↔ ${e.target}`).join('\n')}

## Caveats

- Label-match is a weak signal (\`confidence_tier=AMBIGUOUS\`, \`confidence=0.5\`).
  Same name across repos may mean different things (e.g. \`load()\` in Rust vs TS).
- Hub-noise filter is per-repo p90 of node degree — \`main()\`, \`GET()\`, \`ok()\`
  etc. are excluded automatically when they're hubs in their source graph.
- Does NOT do AST-level symbol resolution (G3/G5). A proper fix needs upstream
  graphify changes — see feedback_graphify_utilization_roadmap.md.
`;

fs.writeFileSync(path.join(OUT_DIR, 'UNIFY_REPORT.md'), report);
console.log(`  report: ${path.join(OUT_DIR, 'UNIFY_REPORT.md')}`);
