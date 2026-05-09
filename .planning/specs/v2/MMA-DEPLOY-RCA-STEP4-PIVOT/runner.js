#!/usr/bin/env node
// MMA Step 4 VERIFY (PIVOT round) — adversarial gate on /exec_atomic_deploy PIVOT PLAN
// 3 Tier-1 compliant models per §S-166 (May 2026 model pool catalog)
// Vendor-fresh from PIVOT (PIVOT used: deepseek+anthropic+xiaomi+google+mistral)

const fs = require('fs');
const path = require('path');
const https = require('https');

const KEY = process.env.OPENROUTER_KEY ||
  fs.readFileSync('C:/Users/bono/racingpoint/racecontrol/data/openrouter-mma-key.txt', 'utf8').trim();
const PROMPT = fs.readFileSync(path.join(__dirname, 'PROMPT.md'), 'utf8');

console.error(`KEY loaded: ${KEY.slice(0, 12)}... (${KEY.length} chars)`);
console.error(`PROMPT size: ${PROMPT.length} chars`);

// Step 4 VERIFY PIVOT — 3 adversarial models, Tier-1 compliant per §S-166
// Vendor diversity within step: moonshot + xai + xiaomi (3 distinct families)
// Roles: reasoner (kimi-k2.5) + code-expert (grok-code-fast-1 per allow-list exception) + SRE (mimo-v2-pro)
// Note: gpt-5.4-nano explicitly NOT used (banned for reasoner role per §S-166); kimi-k2.5 is Tier-1 reasoner
const MODELS = [
  { id: 'moonshotai/kimi-k2.5',          short: 'kimi-k2.5',     vendor: 'moonshot', role: 'reasoner',    timeout: 240000, maxOut: 5000 },
  { id: 'x-ai/grok-code-fast-1',         short: 'grok-code',     vendor: 'xai',      role: 'code_expert', timeout: 180000, maxOut: 5000 },
  { id: 'xiaomi/mimo-v2-pro',            short: 'mimo-v2-pro',   vendor: 'xiaomi',   role: 'sre',         timeout: 180000, maxOut: 5000 },
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
  console.error(`Launching ${MODELS.length} adversarial models in parallel for Step 4 VERIFY PIVOT...`);
  const results = await Promise.all(MODELS.map(callModel));
  const wallElapsed = ((Date.now() - startWall) / 1000).toFixed(1);
  console.error(`\nAll models completed in ${wallElapsed}s wall-clock`);

  fs.writeFileSync(path.join(__dirname, 'results.json'), JSON.stringify(results, null, 2));

  let totalCost = 0;
  console.error('\n--- COST ---');
  for (const r of results) {
    if (r.cost != null) { totalCost += r.cost; console.error(`  ${r.model}: $${r.cost.toFixed(4)}`); }
    else if (r.status !== 'OK') { console.error(`  ${r.model}: ${r.status} ${r.error?.message || r.error || ''}`); }
  }
  console.error(`  TOTAL: $${totalCost.toFixed(4)}`);

  const ledgerPath = 'C:/Users/bono/racingpoint/comms-link/data/openrouter-spend-james.jsonl';
  if (fs.existsSync(ledgerPath)) {
    const entry = {
      timestamp: new Date().toISOString(),
      pilot: 'james',
      session_purpose: 'MMA Step 4 VERIFY PIVOT — adversarial gate on /exec_atomic_deploy PIVOT PLAN — deploy-mechanism RCA',
      mma_step: 'VERIFY-PIVOT',
      models: results.map(r => ({ model: MODELS.find(m => m.short === r.model)?.id, status: r.status, usage: r.usage || null, cost: r.cost || null, elapsed_s: r.elapsed })),
      valid_responses: results.filter(r => r.status === 'OK').length,
      total_responses: results.length,
      total_cost_usd: parseFloat(totalCost.toFixed(6)),
      anchor: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP4-PIVOT/',
      consumes_pivot: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP2-PIVOT/CONSENSUS-PLAN.md',
      authorization: 'Captain PV-OPT-1 explicit ratification 2026-05-09 ~21:30 IST',
      vendor_diversity: 'moonshot+xai+xiaomi (3 distinct vendor families)',
      tier1_compliance: 'all 3 models Tier-1 per §S-166: kimi-k2.5 (reasoner) + grok-code-fast-1 (code-expert allow-list exception) + mimo-v2-pro (SRE)',
    };
    fs.appendFileSync(ledgerPath, JSON.stringify(entry) + '\n');
    console.error(`Spend ledger appended to ${ledgerPath}`);
  } else {
    console.error(`Spend ledger NOT FOUND at ${ledgerPath} — skipping append`);
  }

  console.error(`\nResults written to ${__dirname}`);
})();
