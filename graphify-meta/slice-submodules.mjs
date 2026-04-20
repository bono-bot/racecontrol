#!/usr/bin/env node
// graphify-meta/slice-submodules.mjs
// Priority G from HANDOFF: extract kiosk/web/admin-api/billing/rc-agent as separate
// sub-module graphs by filtering racecontrol/graphify-out/graph.json by source_file prefix.
//
// Output:  racecontrol/graphify-out-<slice>/graph.json  (one per slice)
//          consumed by build-meta.mjs as child nodes under racecontrol.
//
// Why: racecontrol is a monorepo containing multiple coherent sub-systems (Rust backend,
// two Next.js frontends, a pod-agent Rust crate). Treating them as one opaque super-node
// in the meta-map loses structure. Slicing lets the meta-map show how kiosk talks to
// admin-api talks to billing etc.

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
const RC_OUT = path.join(ROOT, 'racecontrol', 'graphify-out');

// Normalize a node's source_file to forward-slashes and strip anything before
// the first occurrence of 'racecontrol/' for consistent prefix matching.
function normSf(n) {
  const sf = (n.source_file || '').replace(/\\/g, '/');
  const idx = sf.indexOf('racecontrol/');
  return idx >= 0 ? sf.slice(idx + 'racecontrol/'.length) : sf;
}

// Each slice has a predicate on the normalized source_file string.
// Intentionally favoured declarative prefixes over a single big regex for clarity.
const SLICES = [
  {
    id: 'kiosk',
    label: 'Kiosk PWA',
    purpose: 'Next.js PWA at :3300 — customer-facing pod UI, driver login, experience picker',
    predicate: sf => sf.startsWith('kiosk/'),
  },
  {
    id: 'web',
    label: 'Web Dashboard',
    purpose: 'Next.js staff/POS dashboard at :3200 — billing, fleet health, customer wallet',
    predicate: sf => sf.startsWith('web/'),
  },
  {
    id: 'pwa',
    label: 'PWA (legacy)',
    purpose: 'Earlier PWA codebase (predecessor to kiosk/). May be stale.',
    predicate: sf => sf.startsWith('pwa/'),
  },
  {
    id: 'admin-api',
    label: 'Admin API (backend)',
    purpose: 'Rust handlers at crates/racecontrol/src/api/admin_*.rs — staff endpoints',
    predicate: sf => /^(crates\/racecontrol\/)?src\/api\/admin_/.test(sf),
  },
  {
    id: 'billing-api',
    label: 'Billing API (backend)',
    purpose: 'Rust handlers at crates/racecontrol/src/api/billing_*.rs — drive-time billing',
    predicate: sf => /^(crates\/racecontrol\/)?src\/api\/billing_/.test(sf),
  },
  {
    id: 'customer-api',
    label: 'Customer API (backend)',
    purpose: 'Rust handlers at crates/racecontrol/src/api/customer_*.rs — public/auth endpoints',
    predicate: sf => /^(crates\/racecontrol\/)?src\/api\/customer_/.test(sf),
  },
  {
    id: 'rc-agent',
    label: 'rc-agent (pod binary)',
    purpose: 'Pod-side Rust binary (crates/rc-agent) — HID/UDP polling, game launch, WS client',
    predicate: sf => sf.startsWith('crates/rc-agent/'),
  },
  {
    id: 'rc-sentry',
    label: 'rc-sentry (pod watchdog)',
    purpose: 'Pod-side auxiliary (crates/rc-sentry) — exec endpoint, rc-agent restart',
    predicate: sf => sf.startsWith('crates/rc-sentry/'),
  },
];

console.log(`[slice] Reading ${RC_OUT}/graph.json ...`);
const src = JSON.parse(fs.readFileSync(path.join(RC_OUT, 'graph.json'), 'utf8'));
console.log(`   source: ${src.nodes.length} nodes, ${src.links.length} edges`);

const results = [];

for (const slice of SLICES) {
  // Filter nodes
  const keptNodes = src.nodes.filter(n => slice.predicate(normSf(n)));
  if (keptNodes.length === 0) {
    console.log(`  [${slice.id}] 0 nodes — skipping`);
    results.push({ ...slice, status: 'EMPTY', nodes: 0, edges: 0 });
    continue;
  }
  const keptIds = new Set(keptNodes.map(n => n.id));
  // Filter edges where BOTH endpoints survive
  const keptLinks = (src.links || []).filter(e => keptIds.has(e.source) && keptIds.has(e.target));
  const communitiesPresent = new Set(keptNodes.map(n => n.community).filter(c => c !== undefined));

  const out = {
    directed: src.directed || false,
    multigraph: src.multigraph || false,
    graph: {},  // no partition field consumed by meta-map currently
    nodes: keptNodes,
    links: keptLinks,
    hyperedges: [],  // drop hyperedges — they link across slices
    _slice_meta: {
      parent: 'racecontrol',
      slice_id: slice.id,
      slice_label: slice.label,
      slice_purpose: slice.purpose,
      generated_at: new Date().toISOString(),
      source_graph: `${RC_OUT}/graph.json`,
      source_graph_mtime: fs.statSync(path.join(RC_OUT, 'graph.json')).mtime.toISOString(),
      node_count: keptNodes.length,
      edge_count: keptLinks.length,
      communities_present: [...communitiesPresent].sort((a, b) => a - b),
    },
  };

  const outDir = path.join(ROOT, 'racecontrol', `graphify-out-${slice.id}`);
  fs.mkdirSync(outDir, { recursive: true });
  fs.writeFileSync(path.join(outDir, 'graph.json'), JSON.stringify(out));
  // Minimal report so consumers can see slice stats without parsing JSON
  const report = [
    `# ${slice.label} — sliced from racecontrol`,
    '',
    `- **Purpose:** ${slice.purpose}`,
    `- **Nodes:** ${keptNodes.length}`,
    `- **Edges (internal):** ${keptLinks.length}`,
    `- **Communities present:** ${communitiesPresent.size}`,
    `- **Source:** ${path.relative(ROOT, path.join(RC_OUT, 'graph.json'))}`,
    `- **Generated:** ${new Date().toISOString()}`,
    '',
    '## Top files by node count',
    '',
  ];
  const byFile = new Map();
  for (const n of keptNodes) {
    const sf = normSf(n);
    byFile.set(sf, (byFile.get(sf) || 0) + 1);
  }
  for (const [sf, cnt] of [...byFile.entries()].sort((a, b) => b[1] - a[1]).slice(0, 10)) {
    report.push(`- \`${sf}\` — ${cnt}`);
  }
  fs.writeFileSync(path.join(outDir, 'GRAPH_REPORT.md'), report.join('\n'));
  console.log(`  [${slice.id}] ${keptNodes.length} nodes, ${keptLinks.length} edges → ${path.relative(ROOT, outDir)}`);
  results.push({ ...slice, status: 'OK', nodes: keptNodes.length, edges: keptLinks.length, communities: communitiesPresent.size });
}

console.log();
console.log('Summary:');
console.log('  Slice              Status  Nodes  Edges  Communities');
for (const r of results) {
  console.log(
    `  ${r.id.padEnd(18)} ${r.status.padEnd(6)}  ${String(r.nodes || 0).padStart(5)}  ${String(r.edges || 0).padStart(5)}  ${String(r.communities || 0).padStart(5)}`
  );
}

// Persist a manifest so build-meta.mjs can list children without re-reading each slice
const manifestPath = path.join(__dirname, 'slices.json');
fs.writeFileSync(manifestPath, JSON.stringify({
  generated_at: new Date().toISOString(),
  source: `${RC_OUT}/graph.json`,
  slices: results.filter(r => r.status === 'OK').map(r => ({
    id: r.id,
    label: r.label,
    purpose: r.purpose,
    parent: 'racecontrol',
    graph_path: path.join(ROOT, 'racecontrol', `graphify-out-${r.id}`, 'graph.json').replace(/\\/g, '/'),
    nodes: r.nodes,
    edges: r.edges,
    communities: r.communities,
  })),
}, null, 2));
console.log(`\nManifest: ${path.relative(ROOT, manifestPath)}`);
