#!/usr/bin/env node
/**
 * MMA Step 4 VERIFY adversarial gate on PV2-OPT-E bridge PLAN.
 *
 * 3 vendor-disjoint Tier-1 models per §S-166. Avoids vendors used in
 * prior Step 4 rounds (anthropic, openai, nvidia, moonshot, xai, xiaomi).
 *
 * Models:
 *   - deepseek/deepseek-r1-0528 (reasoner)
 *   - qwen/qwen3-coder (code_expert)
 *   - mistralai/mistral-small-2603 (generalist as SRE-substitute)
 *
 * Output: resp-<short>.json per model + RESULTS.md summary.
 */

'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');

const REGISTRY = require('../../../../scripts/lib/mma-model-registry.js');
const { MODEL_REGISTRY, validateRoleAssignment } = REGISTRY;

const SPEC_DIR = __dirname;
const PLAN_PATH = path.join(SPEC_DIR, 'PLAN.md');
const PROMPT_PATH = path.join(SPEC_DIR, 'PROMPT.md');

const MODELS = [
  { id: 'deepseek/deepseek-r1-0528',          role: 'reasoner' },
  { id: 'qwen/qwen3-coder',                   role: 'code_expert' },
  { id: 'mistralai/mistral-small-2603',       role: 'generalist' },
];

// E3 enforcement: validate every pick BEFORE any API call
for (const m of MODELS) {
  const v = validateRoleAssignment(m.id, m.role);
  if (!v.valid) {
    console.error(`E3 ROLE-FIT REJECT: ${m.id} as ${m.role} — ${v.reason}`);
    process.exit(2);
  }
  console.error(`E3 OK: ${m.id} as ${m.role}`);
}

// Vendor disjoint check
const vendors = new Set(MODELS.map(m => MODEL_REGISTRY[m.id].vendor));
if (vendors.size < 3) {
  console.error(`VENDOR-DIVERSITY FAIL: only ${vendors.size} families across ${MODELS.length} models`);
  process.exit(3);
}
console.error(`VENDOR-DIVERSITY OK: ${vendors.size} families (${[...vendors].join(', ')})`);

// Read PLAN + PROMPT, inline PLAN into PROMPT
const planContent = fs.readFileSync(PLAN_PATH, 'utf8');
const promptTemplate = fs.readFileSync(PROMPT_PATH, 'utf8');
const finalPrompt = promptTemplate.replace('{{PLAN_CONTENT}}', planContent);

console.error(`Prompt size: ${finalPrompt.length} chars`);

// API key resolution (env first, saved file fallback)
let API_KEY = process.env.OPENROUTER_KEY;
if (!API_KEY) {
  const savedPath = path.resolve(__dirname, '../../../../data/openrouter-mma-key.txt');
  try {
    API_KEY = fs.readFileSync(savedPath, 'utf8').trim();
    console.error(`Loaded saved key from ${savedPath}`);
  } catch (e) {
    console.error(`No OPENROUTER_KEY env and saved key unreadable: ${e.message}`);
    process.exit(4);
  }
}

function callModel(modelId, prompt) {
  return new Promise((resolve) => {
    const cfg = MODEL_REGISTRY[modelId];
    const body = JSON.stringify({
      model: modelId,
      messages: [{ role: 'user', content: prompt }],
      max_tokens: 6000,
      temperature: 0.2,
    });

    const opts = {
      hostname: 'openrouter.ai',
      port: 443,
      path: '/api/v1/chat/completions',
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${API_KEY}`,
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body),
        'HTTP-Referer': 'https://racingpoint.cloud',
        'X-Title': 'MMA-BRIDGE-VERIFY-PV2-OPT-E',
      },
      timeout: cfg.timeout || 180000,
    };

    const t0 = Date.now();
    const req = https.request(opts, (res) => {
      let chunks = '';
      res.on('data', (d) => { chunks += d; });
      res.on('end', () => {
        const wall = Date.now() - t0;
        try {
          const j = JSON.parse(chunks);
          resolve({ modelId, status: res.statusCode, wall, body: j });
        } catch (e) {
          resolve({ modelId, status: res.statusCode, wall, error: 'JSON parse', raw: chunks.slice(0, 500) });
        }
      });
    });
    req.on('error', (e) => resolve({ modelId, error: e.message, wall: Date.now() - t0 }));
    req.on('timeout', () => { req.destroy(); resolve({ modelId, error: 'timeout', wall: Date.now() - t0 }); });
    req.write(body);
    req.end();
  });
}

(async () => {
  console.error(`\nFiring ${MODELS.length} models in parallel...`);
  const t0 = Date.now();
  const results = await Promise.all(MODELS.map(m => callModel(m.id, finalPrompt)));
  const wall = Date.now() - t0;
  console.error(`\nAll done in ${wall}ms\n`);

  const summary = [];
  let totalCost = 0;
  for (const r of results) {
    const cfg = MODEL_REGISTRY[r.modelId];
    const short = cfg.short;
    const outFile = path.join(SPEC_DIR, `resp-${short.replace(/[\/]/g, '-')}.json`);
    fs.writeFileSync(outFile, JSON.stringify(r, null, 2));

    if (r.body && r.body.usage) {
      const inTok = r.body.usage.prompt_tokens || 0;
      const outTok = r.body.usage.completion_tokens || 0;
      const cost = (inTok * cfg.priceIn + outTok * cfg.priceOut) / 1e6;
      totalCost += cost;
      const content = r.body.choices?.[0]?.message?.content || '<NO CONTENT>';
      const finishReason = r.body.choices?.[0]?.finish_reason || 'unknown';
      summary.push({
        short,
        status: r.status,
        wall_ms: r.wall,
        in_tok: inTok,
        out_tok: outTok,
        cost_usd: cost.toFixed(4),
        finish: finishReason,
        content_preview: content.slice(0, 300),
        content_size: content.length,
      });
      console.error(`${short.padEnd(20)} status=${r.status} wall=${r.wall}ms tok=${inTok}/${outTok} cost=$${cost.toFixed(4)} finish=${finishReason} content=${content.length}c`);
    } else {
      summary.push({ short, status: r.status || 'ERR', wall_ms: r.wall, error: r.error || JSON.stringify(r.body).slice(0, 200) });
      console.error(`${short.padEnd(20)} ERROR status=${r.status} ${r.error || JSON.stringify(r.body).slice(0, 100)}`);
    }
  }

  const resultsMd = [
    '# MMA Step 4 VERIFY — PV2-OPT-E Bridge — Per-Model Results',
    '',
    `**Run:** ${new Date().toISOString()}`,
    `**Wall:** ${wall}ms`,
    `**Total cost:** $${totalCost.toFixed(4)}`,
    `**Models:** ${MODELS.map(m => m.id).join(' / ')}`,
    '',
    '| Model | Status | Wall (ms) | In tok | Out tok | Cost USD | Finish | Content size |',
    '|-------|--------|-----------|--------|---------|----------|--------|--------------|',
    ...summary.map(s => `| ${s.short} | ${s.status} | ${s.wall_ms} | ${s.in_tok || '-'} | ${s.out_tok || '-'} | ${s.cost_usd || '-'} | ${s.finish || '-'} | ${s.content_size || s.error || '-'} |`),
    '',
    'See resp-<short>.json for full responses. Synthesis at CONSENSUS-VERIFY.md.',
  ].join('\n');
  fs.writeFileSync(path.join(SPEC_DIR, 'RESULTS.md'), resultsMd);
  console.error(`\nTotal cost: $${totalCost.toFixed(4)}`);
  console.error(`RESULTS.md written.`);
})();
