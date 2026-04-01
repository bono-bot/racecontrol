#!/usr/bin/env node
// Quick OpenRouter MMA runner — sends audit prompt to a single model
// Usage: OPENROUTER_KEY="..." MODEL="qwen/qwen3-235b-a22b-2507" node audit/or-audit-runner.js
const fs = require('fs');
const https = require('https');
const { recoverKey, is401Error, loadSavedKey } = require('../scripts/lib/openrouter-key-recovery');

let KEY = process.env.OPENROUTER_KEY || loadSavedKey();
const MODEL = process.env.MODEL;
if (!KEY || !MODEL) { console.error('Set OPENROUTER_KEY and MODEL'); process.exit(1); }

const prompt = fs.readFileSync('audit/mma-workflow-prompt.txt', 'utf8');
const shortName = MODEL.split('/').pop();

function sendRequest(apiKey, keyRecovered = false) {
  const body = JSON.stringify({
    model: MODEL,
    messages: [{ role: 'user', content: prompt }],
    max_tokens: 8000,
    temperature: 0.3,
  });

  const options = {
    hostname: 'openrouter.ai',
    path: '/api/v1/chat/completions',
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${apiKey}`,
      'HTTP-Referer': 'https://racingpoint.in',
      'X-Title': 'MMA Cross-App Audit',
    },
    timeout: 300000,
  };

  console.error(`[${shortName}] Starting...`);
  const req = https.request(options, (res) => {
    let data = '';
    res.on('data', chunk => data += chunk);
    res.on('end', () => {
      try {
        const json = JSON.parse(data);
        if (json.error) {
          if (is401Error(json.error) && !keyRecovered) {
            console.error(`[${shortName}] 401 — key is dead. Attempting auto-recovery...`);
            recoverKey().then(newKey => {
              KEY = newKey;
              sendRequest(newKey, true);
            }).catch(e => {
              console.error(`[${shortName}] Key recovery failed: ${e.message}`);
              process.exit(1);
            });
            return;
          }
          console.error(`[${shortName}] ERROR: ${JSON.stringify(json.error)}`);
          process.exit(1);
        }
        const content = json.choices?.[0]?.message?.content || 'No response';
        const usage = json.usage || {};
        const outFile = `audit/results/or-${shortName}-${new Date().toISOString().split('T')[0]}.md`;
        fs.mkdirSync('audit/results', { recursive: true });
        fs.writeFileSync(outFile, `# OpenRouter MMA: ${MODEL}\n\n${content}\n\n---\nTokens: in=${usage.prompt_tokens||'?'} out=${usage.completion_tokens||'?'}\n`);
        console.error(`[${shortName}] Done → ${outFile} (${usage.prompt_tokens||'?'}/${usage.completion_tokens||'?'} tokens)`);
        // Also output to stdout for capture
        console.log(content);
      } catch (e) {
        console.error(`[${shortName}] Parse error: ${e.message}`);
        console.error(data.slice(0, 500));
        process.exit(1);
      }
    });
  });
  req.on('error', e => { console.error(`[${shortName}] Network error: ${e.message}`); process.exit(1); });
  req.on('timeout', () => { req.destroy(); console.error(`[${shortName}] Timeout`); process.exit(1); });
  req.write(body);
  req.end();
}

sendRequest(KEY);
