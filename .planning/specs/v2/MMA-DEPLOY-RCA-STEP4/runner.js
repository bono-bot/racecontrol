#!/usr/bin/env node
// MMA Step 4 VERIFY — adversarial review of CF-1+CF-2 bundle PLAN
// 3 vendor-disjoint models from Steps 1+2 (used: deepseek/qwen/xiaomi/google/mistral/xai/moonshot)
// Role coverage: 1 reasoner + 1 code expert + 1 SRE per Protocol v3.0

const fs = require('fs');
const path = require('path');
const https = require('https');

const REPO_ROOT = path.resolve(__dirname, '..', '..', '..', '..');
const KEY = process.env.OPENROUTER_KEY ||
  fs.readFileSync(path.join(REPO_ROOT, 'data', 'openrouter-mma-key.txt'), 'utf8').trim();
const PROMPT = fs.readFileSync(path.join(__dirname, 'PROMPT.md'), 'utf8');

console.error(`KEY loaded: ${KEY.slice(0, 12)}... (${KEY.length} chars)`);
console.error(`PROMPT size: ${PROMPT.length} chars`);

// Step 4 VERIFY — 3 adversarial models, vendor-disjoint from Steps 1+2.
// Vendor-fresh slots: anthropic, openai, nvidia
const MODELS = [
  { id: 'anthropic/claude-sonnet-4.6',                short: 'sonnet-4.6',     vendor: 'anthropic', role: 'code_expert', timeout: 240000, maxOut: 4000 },
  { id: 'openai/gpt-5.4-nano',                        short: 'gpt-5.4-nano',   vendor: 'openai',    role: 'reasoner',    timeout: 180000, maxOut: 4000 },
  { id: 'nvidia/llama-3.1-nemotron-70b-instruct',     short: 'nemotron-70b',   vendor: 'nvidia',    role: 'sre',         timeout: 180000, maxOut: 4000 },
];

function callModel(model) {
  return new Promise((resolve) => {
    const start = Date.now();
    const body = JSON.stringify({
      model: model.id,
      messages: [{ role: 'user', content: PROMPT }],
      max_tokens: model.maxOut,
      temperature: 0.2,
    });
    const req = https.request({
      hostname: 'openrouter.ai',
      path: '/api/v1/chat/completions',
      method: 'POST',
      headers: {
        'Authorization': 'Bearer ' + KEY,
        'Content-Type': 'application/json',
        'Content-Length': Buffer.byteLength(body),
      },
      timeout: model.timeout,
    }, (res) => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        const elapsed = ((Date.now() - start) / 1000).toFixed(1);
        try {
          const obj = JSON.parse(data);
          if (obj.error) {
            console.error(`  [${model.short}] ${elapsed}s | API-ERROR ${JSON.stringify(obj.error).slice(0, 200)}`);
            resolve({ model: model.short, status: 'API_ERROR', error: obj.error, elapsed: parseFloat(elapsed) });
            return;
          }
          const content = obj.choices?.[0]?.message?.content || '';
          const usage = obj.usage || {};
          const cost = obj.usage?.cost ?? null;
          fs.writeFileSync(path.join(__dirname, `resp-${model.short}.md`), content);
          fs.writeFileSync(path.join(__dirname, `meta-${model.short}.json`), JSON.stringify({
            model: model.id,
            usage,
            cost,
            finish_reason: obj.choices?.[0]?.finish_reason,
            elapsed_s: parseFloat(elapsed),
          }, null, 2));
          console.error(`  [${model.short}] ${elapsed}s | in=${usage.prompt_tokens} out=${usage.completion_tokens} | $${cost} | content=${content.length} chars | finish=${obj.choices?.[0]?.finish_reason}`);
          resolve({ model: model.short, status: 'OK', usage, cost, elapsed: parseFloat(elapsed), finish: obj.choices?.[0]?.finish_reason });
        } catch (e) {
          fs.writeFileSync(path.join(__dirname, `raw-${model.short}.txt`), data);
          console.error(`  [${model.short}] ${elapsed}s | PARSE_FAIL ${e.message}`);
          resolve({ model: model.short, status: 'PARSE_FAIL', error: e.message, elapsed: parseFloat(elapsed) });
        }
      });
    });
    req.on('error', e => { console.error(`  [${model.short}] NET_ERROR ${e.message}`); resolve({ model: model.short, status: 'NET_ERROR', error: e.message, elapsed: (Date.now()-start)/1000 }); });
    req.on('timeout', () => { console.error(`  [${model.short}] TIMEOUT @${model.timeout}ms`); req.destroy(); resolve({ model: model.short, status: 'TIMEOUT', elapsed: (Date.now()-start)/1000 }); });
    req.write(body);
    req.end();
  });
}

(async () => {
  const startWall = Date.now();
  console.error(`Launching ${MODELS.length} adversarial models in parallel for Step 4 VERIFY...`);
  const results = await Promise.all(MODELS.map(callModel));
  const wallElapsed = ((Date.now() - startWall) / 1000).toFixed(1);
  console.error(`\nAll models completed in ${wallElapsed}s wall-clock`);

  // Save results.json summary
  fs.writeFileSync(path.join(__dirname, 'results.json'), JSON.stringify(results, null, 2));

  // Cost summary
  let totalCost = 0;
  console.error('\n--- COST ---');
  for (const r of results) {
    if (r.cost != null) { totalCost += r.cost; console.error(`  ${r.model}: $${r.cost.toFixed(4)}`); }
    else if (r.status !== 'OK') { console.error(`  ${r.model}: ${r.status} ${r.error || ''}`); }
  }
  console.error(`  TOTAL: $${totalCost.toFixed(4)}`);

  // Append spend ledger
  const ledgerPath = path.join(REPO_ROOT, '..', 'comms-link', 'data', 'openrouter-spend-james.jsonl');
  if (fs.existsSync(ledgerPath)) {
    const entry = {
      timestamp: new Date().toISOString(),
      pilot: 'james',
      session_purpose: 'MMA Step 4 VERIFY — adversarial gate on CF-1+CF-2 bundle PLAN — deploy-mechanism RCA',
      mma_step: 'VERIFY',
      models: results.map(r => ({ model: MODELS.find(m => m.short === r.model)?.id, status: r.status, usage: r.usage || null, cost: r.cost || null, elapsed_s: r.elapsed })),
      valid_responses: results.filter(r => r.status === 'OK').length,
      total_responses: results.length,
      total_cost_usd: parseFloat(totalCost.toFixed(6)),
      anchor: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP4/',
      consumes_step2: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md',
      authorization: 'recommended sequencing per session_handoff_20260509_deploy_mechanism_rca_step2_plan.md §11 option 2 + Captain "go" 2026-05-09 ~20:18 IST',
      vendor_diversity: 'anthropic+openai+nvidia (3 fresh vendors disjoint from Steps 1+2: deepseek/qwen/xiaomi/google/mistral/xai/moonshot)',
    };
    fs.appendFileSync(ledgerPath, JSON.stringify(entry) + '\n');
    console.error(`Spend ledger appended to ${ledgerPath}`);
  } else {
    console.error(`Spend ledger NOT FOUND at ${ledgerPath} — skipping append`);
  }

  console.error(`\nResults written to ${__dirname}`);
})();
