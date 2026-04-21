#!/usr/bin/env node
// racecontrol/scripts/graphify-post/stable-community-ids.mjs
//
// Tier 5 / G12 fix — graphify assigns sequential integer community IDs
// (0, 1, 2, ...) that reshuffle on every rebuild (77.8% drift between
// hour-apart snapshots per the gap catalogue). This post-processor
// rewrites each community ID to a deterministic content-hash prefix
// derived from the community's sorted member-ID list.
//
// After this runs, "community a7f3" will refer to the same cluster across
// rebuilds as long as the members don't meaningfully change. Partial
// membership changes cascade (a cluster that loses one node gets a new
// ID) — this is intentional: stability should reflect semantic stability,
// not mere ordering stability.
//
// Usage:
//   node scripts/graphify-post/stable-community-ids.mjs [graph-path]
//
// Default target: graphify-out/graph.json. Writes in place after backup
// to graphify-out/graph.pre-stable-ids.json.

import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const defaultPath = path.resolve(__dirname, '../../graphify-out/graph.json');
const target = process.argv[2] || defaultPath;

if (!fs.existsSync(target)) {
  console.error(`ERROR: graph not found at ${target}`);
  process.exit(1);
}

const g = JSON.parse(fs.readFileSync(target, 'utf8'));
console.log(`loaded: ${g.nodes.length} nodes, ${g.links.length} edges`);

// Group nodes by original community id
const byCommunity = new Map();
for (const n of g.nodes) {
  const c = n.community;
  if (c === undefined || c === null) continue;
  if (!byCommunity.has(c)) byCommunity.set(c, []);
  byCommunity.get(c).push(n);
}

console.log(`communities: ${byCommunity.size}`);

// For each community, sort member IDs lexicographically, hash with sha256,
// take first 8 hex chars. Collisions across 551 communities on 64 bits of
// entropy are negligible (birthday bound ~1 in 10^9).
const remap = new Map();
const collisions = new Map();

for (const [origId, nodes] of byCommunity.entries()) {
  const sortedIds = nodes.map(n => n.id).sort();
  const digest = crypto.createHash('sha256')
    .update(sortedIds.join('\n'))
    .digest('hex')
    .slice(0, 8);

  if (collisions.has(digest)) {
    console.warn(`WARN: hash collision ${digest} between communities ${collisions.get(digest)} and ${origId} — extending prefix`);
    // Extend prefix to avoid collision
    const extended = crypto.createHash('sha256')
      .update(sortedIds.join('\n'))
      .digest('hex')
      .slice(0, 12);
    remap.set(origId, `c_${extended}`);
  } else {
    remap.set(origId, `c_${digest}`);
    collisions.set(digest, origId);
  }
}

// Back up the original before rewriting
const backupPath = target.replace(/\.json$/, '.pre-stable-ids.json');
fs.copyFileSync(target, backupPath);
console.log(`backup: ${backupPath}`);

// Rewrite node.community to stable IDs
for (const n of g.nodes) {
  if (n.community !== undefined && n.community !== null) {
    const stable = remap.get(n.community);
    if (stable) {
      n.community_int = n.community; // preserve original for debugging
      n.community = stable;
    }
  }
}

// Annotate graph with the remap so downstream tooling can reverse if needed
g.community_id_scheme = {
  version: 1,
  algorithm: 'sha256(sorted(member_ids))[:8]',
  generated_at: new Date().toISOString(),
  mappings: Object.fromEntries(remap.entries()),
};

fs.writeFileSync(target, JSON.stringify(g, null, 2));
console.log(`wrote: ${target}`);
console.log(`  communities remapped: ${remap.size}`);
console.log(`  schema_version: 1`);

// Quick summary: top-5 communities by size, with stable IDs
const sorted = [...byCommunity.entries()]
  .map(([orig, members]) => ({ orig, stable: remap.get(orig), size: members.length }))
  .sort((a, b) => b.size - a.size)
  .slice(0, 5);

console.log('\nTop 5 by size (for reference):');
for (const s of sorted) {
  console.log(`  ${s.stable} <- was ${s.orig} (${s.size} members)`);
}
