// scripts/lib/refresh-mma-registry.js — STUB (E2 of §S-166 MMA Doctrine Alignment)
//
// PURPOSE: pull OpenRouter /api/v1/models, diff against MODEL_REGISTRY, write drift to
// data/mma-registry-drift.jsonl. Run weekly via cron-skill (E6 cadence — DEFERRED to next session).
//
// USAGE:
//   node scripts/lib/refresh-mma-registry.js          # diff to JSONL, exit
//   node scripts/lib/refresh-mma-registry.js --print  # also print human summary to stdout
//
// OUTPUT: each line is one drift event:
//   {"ts":"...","kind":"ADDED|REMOVED|REPRICED|ALIASED","modelId":"...","details":{...}}
//
// Composes-with: scripts/lib/mma-model-registry.js (registry source-of-truth this script diffs).
// NOT YET WIRED: cron-skill registration via /schedule. Safe to run manually any time (read-only,
// writes to data/mma-registry-drift.jsonl only).

'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');

const { MODEL_REGISTRY } = require('./mma-model-registry');

const DRIFT_LOG = path.resolve(__dirname, '..', '..', 'data', 'mma-registry-drift.jsonl');
const OPENROUTER_API = 'https://openrouter.ai/api/v1/models';
const PRINT = process.argv.includes('--print');

function fetchJson(url) {
  return new Promise((resolve, reject) => {
    https.get(url, { timeout: 30000 }, (res) => {
      const chunks = [];
      res.on('data', c => chunks.push(c));
      res.on('end', () => {
        try { resolve(JSON.parse(Buffer.concat(chunks).toString('utf8'))); }
        catch (e) { reject(e); }
      });
    }).on('error', reject).on('timeout', () => reject(new Error('timeout')));
  });
}

function appendDrift(events) {
  const ts = new Date().toISOString();
  const lines = events.map(e => JSON.stringify({ ts, ...e })).join('\n') + '\n';
  fs.mkdirSync(path.dirname(DRIFT_LOG), { recursive: true });
  fs.appendFileSync(DRIFT_LOG, lines, 'utf8');
}

async function main() {
  const catalog = await fetchJson(OPENROUTER_API);
  const catalogById = new Map();
  for (const m of catalog.data || []) {
    catalogById.set(m.id, m);
  }

  const events = [];
  const registryIds = new Set(Object.keys(MODEL_REGISTRY));

  // Detect REMOVED (in registry, not in catalog) — likely deprecated upstream
  for (const id of registryIds) {
    if (!catalogById.has(id)) {
      events.push({ kind: 'REMOVED', modelId: id, details: { note: 'absent from OpenRouter catalog — verify deprecation' } });
    }
  }

  // Detect ADDED candidates: catalog flagships matching tier patterns we lack
  // (Conservative — don't flood. Only flag entries clearly in scope: 70B+ params, ctx >= 128k.)
  // For now we only emit ADDED when catalog has a model whose ID matches our naming-template-of-interest.
  const FLAGSHIP_PATTERNS = [
    /^anthropic\/claude-(opus|sonnet)-/i,
    /^openai\/gpt-(5\.|6\.)/i,
    /^deepseek\/deepseek-(v[3-9]|r[1-9])/i,
    /^qwen\/qwen3.*-(coder|max|thinking)/i,
    /^moonshotai\/kimi-k[2-9]/i,
    /^google\/gemini-[2-9]\.[0-9]-(pro|flash)$/i,
    /^x-ai\/grok-[4-9]/i,
    /^xiaomi\/mimo-v[2-9]/i,
    /^nvidia\/nemotron-/i,
    /^mistralai\/(codestral|devstral|mistral-large)/i,
  ];
  for (const [id, m] of catalogById.entries()) {
    if (registryIds.has(id)) continue;
    if (FLAGSHIP_PATTERNS.some(re => re.test(id))) {
      events.push({
        kind: 'ADDED_CANDIDATE',
        modelId: id,
        details: {
          ctx: m.context_length || null,
          priceIn: m.pricing?.prompt || null,
          priceOut: m.pricing?.completion || null,
          note: 'flagship pattern — manual classification needed (role tags + tier)'
        }
      });
    }
  }

  // Detect REPRICED — pricing diff in catalog vs registry
  for (const [id, cfg] of Object.entries(MODEL_REGISTRY)) {
    const m = catalogById.get(id);
    if (!m) continue;
    const catIn = parseFloat(m.pricing?.prompt || '0') * 1e6;   // catalog is per-token; registry is per-Mtok
    const catOut = parseFloat(m.pricing?.completion || '0') * 1e6;
    const driftIn  = Math.abs(catIn  - cfg.priceIn)  / Math.max(cfg.priceIn,  0.001);
    const driftOut = Math.abs(catOut - cfg.priceOut) / Math.max(cfg.priceOut, 0.001);
    if (driftIn > 0.10 || driftOut > 0.10) {  // >10% drift
      events.push({
        kind: 'REPRICED',
        modelId: id,
        details: {
          registry: { priceIn: cfg.priceIn, priceOut: cfg.priceOut },
          catalog:  { priceIn: catIn.toFixed(4), priceOut: catOut.toFixed(4) },
          driftIn: driftIn.toFixed(3),
          driftOut: driftOut.toFixed(3),
        }
      });
    }
  }

  if (events.length === 0) {
    if (PRINT) console.log('No drift detected.');
    return;
  }

  appendDrift(events);
  if (PRINT) {
    console.log(`Drift events: ${events.length}`);
    for (const e of events) {
      console.log(`  ${e.kind.padEnd(18)} ${e.modelId}`);
    }
    console.log(`Appended to ${DRIFT_LOG}`);
  }
}

main().catch(e => { console.error('refresh-mma-registry failed:', e.message); process.exit(1); });
