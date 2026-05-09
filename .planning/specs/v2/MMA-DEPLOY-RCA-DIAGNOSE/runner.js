#!/usr/bin/env node
// MMA Step 1 DIAGNOSE — deploy mechanism RCA
// 5 models in parallel, each gets PROMPT.md, response saved per-model, spend logged.

const fs = require('fs');
const path = require('path');
const https = require('https');

const KEY = process.env.OPENROUTER_KEY || fs.readFileSync('C:/Users/bono/racingpoint/racecontrol/data/openrouter-mma-key.txt', 'utf8').trim();
const PROMPT = fs.readFileSync(path.join(__dirname, 'PROMPT.md'), 'utf8');

const MODELS = [
  { id: 'deepseek/deepseek-r1-0528',     short: 'deepseek-r1',  vendor: 'deepseek', role: 'reasoner',    timeout: 180000, maxOut: 8000 },
  { id: 'qwen/qwen3-coder',              short: 'qwen3-coder',  vendor: 'qwen',     role: 'code_expert', timeout: 180000, maxOut: 8000 },
  { id: 'xiaomi/mimo-v2-pro',            short: 'mimo-v2-pro',  vendor: 'xiaomi',   role: 'sre',         timeout: 180000, maxOut: 8000 },
  { id: 'google/gemini-2.5-flash',       short: 'gemini-flash', vendor: 'google',   role: 'generalist',  timeout: 120000, maxOut: 8000 },
  { id: 'mistralai/mistral-small-2603',  short: 'mistral-sm',   vendor: 'mistral',  role: 'generalist',  timeout: 180000, maxOut: 8000 },
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
        try {
          const obj = JSON.parse(data);
          if (obj.error) {
            resolve({ model: model.short, status: 'ERROR', error: obj.error, elapsed: (Date.now()-start)/1000 });
            return;
          }
          const content = obj.choices?.[0]?.message?.content || '';
          const usage = obj.usage || {};
          fs.writeFileSync(path.join(__dirname, `resp-${model.short}.md`), content);
          fs.writeFileSync(path.join(__dirname, `meta-${model.short}.json`), JSON.stringify({
            model: model.id,
            usage,
            finish_reason: obj.choices?.[0]?.finish_reason,
            elapsed_s: (Date.now()-start)/1000,
          }, null, 2));
          resolve({ model: model.short, status: 'OK', usage, elapsed: (Date.now()-start)/1000, finish: obj.choices?.[0]?.finish_reason });
        } catch (e) {
          fs.writeFileSync(path.join(__dirname, `raw-${model.short}.txt`), data);
          resolve({ model: model.short, status: 'PARSE_FAIL', error: e.message, elapsed: (Date.now()-start)/1000 });
        }
      });
    });
    req.on('error', e => resolve({ model: model.short, status: 'NET_ERROR', error: e.message, elapsed: (Date.now()-start)/1000 }));
    req.on('timeout', () => { req.destroy(); resolve({ model: model.short, status: 'TIMEOUT', elapsed: (Date.now()-start)/1000 }); });
    req.write(body);
    req.end();
  });
}

(async () => {
  console.log('Launching 5 models in parallel...');
  const results = await Promise.all(MODELS.map(callModel));
  console.log('\n=== Results ===');
  let totalCost = 0;
  for (const r of results) {
    if (r.status === 'OK') {
      const u = r.usage;
      const m = MODELS.find(x => x.short === r.model);
      // Cost estimate using OpenRouter prices (rough; actual billing comes from OpenRouter)
      console.log(`${r.model}: OK | tokens=${u.prompt_tokens}/${u.completion_tokens} | finish=${r.finish} | ${r.elapsed.toFixed(1)}s`);
    } else {
      console.log(`${r.model}: ${r.status} | ${r.error || ''} | ${r.elapsed.toFixed(1)}s`);
    }
  }
  // Spend log entry
  const spend = {
    timestamp: new Date().toISOString(),
    pilot: 'james',
    session_purpose: 'MMA Step 1 DIAGNOSE — rc-agent fleet deployment mechanism RCA (Captain-requested 2026-05-09 ~19:30 IST)',
    mma_step: 'DIAGNOSE',
    models: results.map(r => ({ model: MODELS.find(m => m.short === r.model)?.id, status: r.status, usage: r.usage || null, elapsed_s: r.elapsed })),
    valid_responses: results.filter(r => r.status === 'OK').length,
    total_responses: results.length,
    notes: 'Deploy-mechanism RCA, NOT codebase audit. Inputs: 11-issue session RCA + architecture + watchdog source.',
  };
  fs.appendFileSync('C:/Users/bono/racingpoint/comms-link/data/openrouter-spend-james.jsonl', JSON.stringify(spend) + '\n');
  console.log('\nSpend logged to openrouter-spend-james.jsonl');
})();
