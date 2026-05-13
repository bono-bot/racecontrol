#!/usr/bin/env node
/**
 * MMA Step 4 VERIFY Round 4 — adversarial gate on v0.4 deploy-mechanism PLAN.
 *
 * 3 vendor-disjoint Tier-1 models per §S-166, NEW vendors not used in
 * Round 2 (moonshot+xai+xiaomi) or Round 3 (deepseek+qwen+mistral):
 *   - z-ai/glm-5                          (zhipu, reasoner, premium)
 *   - kwaipilot/kat-coder-pro-v2          (kwai, code_expert, standard)
 *   - nvidia/nemotron-3-super-120b-a12b   (nvidia, sre, standard)
 *
 * Vendor families: zhipu + kwai + nvidia — 3 distinct, all fresh-to-this-RCA.
 *
 * Captain auth: 2026-05-13 ~14:48 IST "Authorize ~$0.05-0.10 MMA spend" for v0.4 VERIFY.
 *
 * Output: resp-<short>.md per model + meta-<short>.json + results.json
 * Consensus synthesis: CONSENSUS-VERIFY.md (written by post-runner step)
 */

'use strict';

const fs = require('fs');
const path = require('path');
const https = require('https');

const REGISTRY = require('../../../../scripts/lib/mma-model-registry.js');
const { MODEL_REGISTRY, validateRoleAssignment } = REGISTRY;

const SPEC_DIR = __dirname;
const PLAN_PATH = path.join(SPEC_DIR, 'PLAN-v0.4.md');
const PROMPT_PATH = path.join(SPEC_DIR, 'PROMPT.md');
const RESULTS_PATH = path.join(SPEC_DIR, 'results.json');

const MODELS = [
  { id: 'z-ai/glm-5',                          role: 'reasoner' },
  { id: 'kwaipilot/kat-coder-pro-v2',          role: 'code_expert' },
  { id: 'nvidia/nemotron-3-super-120b-a12b',   role: 'sre' },
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
        'X-Title': 'MMA-DEPLOY-RCA-VERIFY-V04',
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
  const totalWall = Date.now() - t0;

  let totalCost = 0;
  let valid = 0;
  const summary = [];

  for (const r of results) {
    const cfg = MODEL_REGISTRY[r.modelId];
    const short = cfg.short;

    if (r.error) {
      console.error(`[${short}] ERROR: ${r.error}`);
      summary.push({ model: r.modelId, short, role: MODELS.find(m=>m.id===r.modelId).role, vendor: cfg.vendor, status: 'ERROR', error: r.error, wall: r.wall });
      continue;
    }

    if (r.body.error) {
      console.error(`[${short}] API ERROR: ${JSON.stringify(r.body.error)}`);
      summary.push({ model: r.modelId, short, role: MODELS.find(m=>m.id===r.modelId).role, vendor: cfg.vendor, status: 'API_ERROR', error: r.body.error, wall: r.wall });
      continue;
    }

    const choice = r.body.choices?.[0];
    if (!choice) {
      console.error(`[${short}] NO CHOICES in response: ${JSON.stringify(r.body).slice(0, 300)}`);
      summary.push({ model: r.modelId, short, role: MODELS.find(m=>m.id===r.modelId).role, vendor: cfg.vendor, status: 'NO_CHOICES', wall: r.wall });
      continue;
    }

    const text = choice.message?.content || '';
    const usage = r.body.usage || {};
    const cost = usage.cost || 0;
    totalCost += cost;
    valid += 1;

    // Write response
    const respPath = path.join(SPEC_DIR, `resp-${short}.md`);
    fs.writeFileSync(respPath, text);

    // Write meta
    const metaPath = path.join(SPEC_DIR, `meta-${short}.json`);
    fs.writeFileSync(metaPath, JSON.stringify({
      model: r.modelId,
      short,
      role: MODELS.find(m=>m.id===r.modelId).role,
      vendor: cfg.vendor,
      status: r.status,
      finish_reason: choice.finish_reason,
      usage,
      wall_ms: r.wall,
      cost,
    }, null, 2));

    console.error(`[${short}] OK ${r.wall}ms — ${usage.completion_tokens||0} tokens, $${cost.toFixed(4)}, finish=${choice.finish_reason}`);
    summary.push({
      model: r.modelId, short,
      role: MODELS.find(m=>m.id===r.modelId).role,
      vendor: cfg.vendor,
      status: 'OK',
      finish_reason: choice.finish_reason,
      tokens_in: usage.prompt_tokens,
      tokens_out: usage.completion_tokens,
      cost,
      wall_ms: r.wall,
    });
  }

  const results_summary = {
    timestamp: new Date().toISOString(),
    pilot: 'james',
    session_purpose: 'MMA Step 4 VERIFY Round 4 — adversarial gate on v0.4 hardened-script deploy-mechanism PLAN (substrate-evolved Option Y)',
    mma_step: 'VERIFY-V04',
    models: summary,
    valid_responses: valid,
    total_responses: results.length,
    total_cost_usd: totalCost,
    wall_ms: totalWall,
    anchor: '.planning/specs/v2/MMA-DEPLOY-RCA-VERIFY-V04/',
    consumes_step1: '.planning/specs/v2/MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md (CF-1..CF-9)',
    consumes_round3_findings: '.planning/specs/v2/MMA-BRIDGE-VERIFY-PV2-OPT-E/CONSENSUS-VERIFY.md (CR-FL-X+A+B+C+D+E+F+G+H)',
    supersedes_pivot_path: 'PV2-OPT-B Win32 LockFileEx (deferred as overengineering given substrate evolution)',
    authorization: 'Captain disposition (A) 2026-05-13 ~14:48 IST: "drive Step 4 VERIFY to PASS on a v0.4 plan ... or commit to LockFileEx pivot. Authorize ~$0.05-0.10 MMA spend"',
    vendor_diversity: `${vendors.size} distinct vendor families (${[...vendors].join(' + ')}); disjoint from Round 2 + Round 3 vendor sets`,
    tier1_compliance: 'all 3 models Tier-1 per §S-166: glm-5 (reasoner premium) + kat-coder-pro-v2 (code_expert standard) + nemotron-super (sre standard)',
    pass_threshold: 4.0,
    block_threshold: '<4.0',
  };

  fs.writeFileSync(RESULTS_PATH, JSON.stringify(results_summary, null, 2));
  console.error(`\nResults: valid=${valid}/${results.length} · cost=$${totalCost.toFixed(4)} · wall=${(totalWall/1000).toFixed(1)}s`);
  console.error(`Wrote: ${RESULTS_PATH}`);
})();
