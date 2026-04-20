#!/usr/bin/env node
// graphify-meta/ws-edges.mjs
// Discover WebSocket MESSAGE-TYPE coupling between modules.
//
// The real data flow racecontrol↔rc-agent↔kiosk↔admin is WebSocket-based. Both sides
// reference enum variants like WsServerMsg::ConfigPush, WsClientMsg::AgentHello,
// DashboardPush::FleetHealth. Grep each module for `::VariantName` and cross-reference
// against the canonical MessageType enum defined in rc-common.
//
// Emit: ws-edges.json — list of (sender → receiver) edges labelled by message variant.

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

// Canonical WS-message enum names to search for. Discovered via grep of rc-common/src/.
// These are the primary protocol enums used across racecontrol / rc-agent / kiosk / web.
const WS_ENUMS = [
  // rc-common protocol (primary WS contract)
  'AgentMessage', 'CoreToAgentMessage', 'DashboardEvent', 'DashboardCommand',
  'AiChannelMessage', 'MeshMessage',
  // rc-common events (cross-module semantics)
  'FleetEvent', 'GameEvent',
  // racecontrol-side events (fired into broadcast channels)
  'BillingEvent', 'BonoEvent',
  // rc-agent-side events
  'LockScreenEvent', 'HeartbeatEvent',
  // crate-specific
  'GuardianEvent', 'AlertEvent',
];

const MODULES = [
  'racecontrol',
  'racingpoint-admin',
  'comms-link',
  'whatsapp-bot',
  'people-tracker',
  'pod-agent',
  'rc-ops-mcp',
];

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

// Build a combined regex: (WsServerMsg|WsClientMsg|...)::([A-Z][A-Za-z0-9_]+)
const VARIANT_RE = new RegExp(`\\b(${WS_ENUMS.join('|')})(?:::|\\.)(${'[A-Z][A-Za-z0-9_]+'})`, 'g');
// TS/JS side uses string literals: `{ type: 'ConfigPush' }` or `case 'FleetHealth':`
const TYPE_STR_RE = /(?:type|kind|msg_type|message_type|event)\s*[:=]\s*["']([A-Z][A-Za-z0-9_]+)["']/g;
const CASE_STR_RE = /\bcase\s+["']([A-Z][A-Za-z0-9_]+)["']/g;

console.log('[1/3] Scanning modules for WS message variants ...');
const modVariants = new Map();  // mod → Map(variant → Set<files>)

for (const mod of MODULES) {
  const modDir = path.join(ROOT, mod);
  if (!fs.existsSync(modDir)) continue;
  const variants = new Map();
  const files = walkSources(modDir);
  for (const f of files) {
    let src;
    try { src = fs.readFileSync(f, 'utf8'); }
    catch { continue; }
    // Rust-style enum variants
    VARIANT_RE.lastIndex = 0;
    let m;
    while ((m = VARIANT_RE.exec(src)) !== null) {
      const variant = m[2];
      if (!variants.has(variant)) variants.set(variant, new Set());
      variants.get(variant).add(f.replace(ROOT, '').replace(/\\/g, '/'));
    }
    // TS/JS string-type discriminators (requires file to have WS context)
    if (/websocket|ws|socket/i.test(src.slice(0, 500))) {
      TYPE_STR_RE.lastIndex = 0;
      while ((m = TYPE_STR_RE.exec(src)) !== null) {
        const variant = m[1];
        if (!variants.has(variant)) variants.set(variant, new Set());
        variants.get(variant).add(f.replace(ROOT, '').replace(/\\/g, '/'));
      }
      CASE_STR_RE.lastIndex = 0;
      while ((m = CASE_STR_RE.exec(src)) !== null) {
        const variant = m[1];
        if (!variants.has(variant)) variants.set(variant, new Set());
        variants.get(variant).add(f.replace(ROOT, '').replace(/\\/g, '/'));
      }
    }
  }
  modVariants.set(mod, variants);
  console.log(`   ${mod.padEnd(24)} ${variants.size} distinct variants`);
}

console.log('[2/3] Computing shared-variant edges ...');
const edges = [];
const modules = [...modVariants.keys()];
for (let i = 0; i < modules.length; i++) {
  for (let j = i + 1; j < modules.length; j++) {
    const A = modules[i], B = modules[j];
    const vA = modVariants.get(A), vB = modVariants.get(B);
    const shared = [];
    for (const [variant] of vA) {
      if (vB.has(variant)) shared.push(variant);
    }
    if (shared.length === 0) continue;
    edges.push({
      from: A, to: B,
      type: 'shared-ws-variant',
      weight: shared.length,
      shared_variants: shared.slice(0, 30),
    });
  }
}

console.log(`   ${edges.length} shared-WS-variant edges`);

console.log('[3/3] Writing ws-edges.json ...');
const out = path.join(__dirname, 'ws-edges.json');
fs.writeFileSync(out, JSON.stringify({
  generated_at: new Date().toISOString(),
  note: 'WebSocket message-variant coupling. Shared enum variants across modules indicate WS protocol participation.',
  enums_scanned: WS_ENUMS,
  edges: edges.sort((a, b) => b.weight - a.weight),
  per_module_variant_counts: Object.fromEntries(
    [...modVariants.entries()].map(([m, v]) => [m, v.size])
  ),
}, null, 2));
console.log(`Wrote ${out}`);
console.log('');
console.log('Top 10 WS variant edges:');
for (const e of edges.slice(0, 10)) {
  const sample = e.shared_variants.slice(0, 4).join(', ');
  console.log(`  ${e.from.padEnd(20)} ↔ ${e.to.padEnd(20)}  w=${e.weight}  [${sample}]`);
}
