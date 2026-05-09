#!/usr/bin/env node
// MMA Step 2 PIVOT — CF-1+CF-2+CF-9 bundle around /exec_atomic_deploy server-side architecture
// 5 vendor-diverse models in parallel; max_tokens=6000 (PLAN output longer than VERIFY)

const fs = require('fs');
const path = require('path');
const https = require('https');

const KEY = process.env.OPENROUTER_KEY ||
  fs.readFileSync('C:/Users/bono/racingpoint/racecontrol/data/openrouter-mma-key.txt', 'utf8').trim();
const PROMPT = fs.readFileSync(path.join(__dirname, 'PROMPT.md'), 'utf8');

console.error(`KEY loaded: ${KEY.slice(0, 12)}... (${KEY.length} chars)`);
console.error(`PROMPT size: ${PROMPT.length} chars`);

// 5 distinct vendor families: deepseek + anthropic + xiaomi + google + mistral
// Roles: reasoner (R1) + code-expert (Sonnet) + SRE (MiMo) + 2 generalists (Gemini, Mistral)
// Gemini deliberately included — gemini was the original new_atomic_endpoint proposer
const MODELS = [
  { id: 'deepseek/deepseek-r1-0528',     short: 'deepseek-r1',  vendor: 'deepseek',  role: 'reasoner',    timeout: 240000, maxOut: 6000 },
  { id: 'anthropic/claude-sonnet-4.6',   short: 'sonnet-4.6',   vendor: 'anthropic', role: 'code_expert', timeout: 240000, maxOut: 6000 },
  { id: 'xiaomi/mimo-v2-pro',            short: 'mimo-v2-pro',  vendor: 'xiaomi',    role: 'sre',         timeout: 180000, maxOut: 6000 },
  { id: 'google/gemini-2.5-flash',       short: 'gemini-flash', vendor: 'google',    role: 'generalist',  timeout: 120000, maxOut: 6000 },
  { id: 'mistralai/mistral-small-2603',  short: 'mistral-sm',   vendor: 'mistral',   role: 'generalist',  timeout: 180000, maxOut: 6000 },
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
  console.error(`Launching ${MODELS.length} models in parallel for Step 2 PIVOT (CF-1+CF-2+CF-9 bundle / new_atomic_endpoint)...`);
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
      session_purpose: 'MMA Step 2 PIVOT — CF-1+CF-2+CF-9 bundle (new_atomic_endpoint server-side architecture) — deploy-mechanism RCA — Captain Option B post-Step-4-BLOCK',
      mma_step: 'PLAN-PIVOT',
      models: results.map(r => ({ model: MODELS.find(m => m.short === r.model)?.id, status: r.status, usage: r.usage || null, cost: r.cost || null, elapsed_s: r.elapsed })),
      valid_responses: results.filter(r => r.status === 'OK').length,
      total_responses: results.length,
      total_cost_usd: parseFloat(totalCost.toFixed(6)),
      anchor: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP2-PIVOT/',
      consumes_step1: '.planning/specs/v2/MMA-DEPLOY-RCA-DIAGNOSE/CONSENSUS.md (CF-1+CF-2+CF-9)',
      supersedes_step2: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP2/CONSENSUS-PLAN.md (BLOCKED at Step 4 VERIFY 2.12/5)',
      addresses_flaws: '.planning/specs/v2/MMA-DEPLOY-RCA-STEP4/CONSENSUS-VERIFY.md FL-CONV-1..5',
      authorization: 'Captain Option B explicit ratification 2026-05-09 ~21:04 IST after Step 4 BLOCK + Phase 1 hook denial',
      vendor_diversity: 'deepseek+anthropic+xiaomi+google+mistral (5 distinct vendor families)',
    };
    fs.appendFileSync(ledgerPath, JSON.stringify(entry) + '\n');
    console.error(`Spend ledger appended to ${ledgerPath}`);
  } else {
    console.error(`Spend ledger NOT FOUND at ${ledgerPath} — skipping append`);
  }

  console.error(`\nResults written to ${__dirname}`);
})();
