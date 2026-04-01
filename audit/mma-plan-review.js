#!/usr/bin/env node
// MMA Plan Review — 4 models via OpenRouter
// Usage: OPENROUTER_KEY="..." node audit/mma-plan-review.js

const fs = require('fs');
const path = require('path');
const https = require('https');

const { recoverKey, is401Error, loadSavedKey } = require('../scripts/lib/openrouter-key-recovery');

let OPENROUTER_KEY = process.env.OPENROUTER_KEY || loadSavedKey();
if (!OPENROUTER_KEY) { console.error('ERROR: Set OPENROUTER_KEY'); process.exit(1); }

const PLAN_FILE = path.join(__dirname, '..', '.planning', 'phases', 'LEADERBOARD-TELEMETRY-PLAN.md');
const PLAN_CONTENT = fs.readFileSync(PLAN_FILE, 'utf-8');

const dateStr = new Date().toISOString().split('T')[0];
const OUTPUT_DIR = path.join(__dirname, 'results', `mma-plan-review-${dateStr}`);
fs.mkdirSync(OUTPUT_DIR, { recursive: true });

const MODELS = [
  { id: 'qwen/qwen3-235b-a22b-2507', short: 'qwen3', timeout: 180000 },
  { id: 'deepseek/deepseek-chat-v3-0324', short: 'deepseek-v3', timeout: 180000 },
  { id: 'deepseek/deepseek-r1-0528', short: 'deepseek-r1', timeout: 300000 },
  { id: 'google/gemini-2.5-pro-preview-03-25', short: 'gemini-2.5', timeout: 180000 },
];

const SYSTEM_PROMPT = `You are a senior software architect reviewing a milestone plan for a racing esports venue management system.

SYSTEM CONTEXT:
- Rust/Axum server + SQLite + Next.js frontend monorepo
- 8 gaming pods (each with rc-agent sending UDP game telemetry via WebSocket to server)
- 1 central server (racecontrol) on .23, manages billing, sessions, leaderboards
- 3 leaderboard display machines on Tailscale (intermittently online)
- Games: Assetto Corsa, F1 25, iRacing, LMU, AC Evo, Forza
- telemetry_samples table EXISTS but NO production INSERT code — data never flows in
- driver_ratings table EXISTS but NO computation logic
- Cloud sync via Bono VPS (30s interval), venue-authoritative for laps/billing
- Standing rules: no .unwrap() in Rust, no any in TS, Pod 8 canary first, Session 1 only for rc-agent
- SQLite WAL mode already enabled on production DB
- Existing leaderboard endpoints serve multi-game data with ?sim_type= filtering

REVIEW THE PLAN FOR:
1. Architecture bugs — missing data flows, dead ends, incorrect assumptions about existing code
2. Performance traps — SQLite write bottlenecks (8 pods × 10Hz = 80 writes/sec), memory, disk I/O
3. Security gaps — auth on new endpoints, PII in telemetry, injection vectors
4. Correctness bugs — edge cases in rating formula, race conditions in concurrent writes, WS broadcast timing
5. Missing requirements — what would customers/operators expect that's not listed?
6. Deployment risks — binary size, migration on live DB, rollback strategy
7. Integration gaps — how phases interact with existing billing, cloud sync, kiosk, admin dashboard

Return ONLY a JSON array of findings. Each finding must have:
{"id": "F-XX", "severity": "P1|P2|P3", "category": "architecture|performance|security|correctness|missing|deployment|integration", "phase": "251|252|253|254|255|general", "description": "detailed description", "recommendation": "specific fix"}

P1 = will cause data loss, security breach, or system failure in production
P2 = will cause degraded experience, operational burden, or maintenance debt
P3 = improvement opportunity, nice-to-have

Be SPECIFIC. Reference technical details. Don't be vague.`;

function callOpenRouter(model, keyRecovered = false) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: model.id,
      max_tokens: 16000,
      temperature: 0.3,
      messages: [
        { role: 'system', content: SYSTEM_PROMPT },
        { role: 'user', content: `Review this milestone plan:\n\n${PLAN_CONTENT}` }
      ]
    });

    const options = {
      hostname: 'openrouter.ai',
      path: '/api/v1/chat/completions',
      method: 'POST',
      headers: {
        'Authorization': `Bearer ${OPENROUTER_KEY}`,
        'Content-Type': 'application/json',
        'HTTP-Referer': 'https://racingpoint.cloud',
        'Content-Length': Buffer.byteLength(body),
      },
    };

    console.log(`>>> [${model.short}] Sending request to ${model.id}...`);
    const startTime = Date.now();

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
        console.log(`>>> [${model.short}] Response received in ${elapsed}s (HTTP ${res.statusCode})`);

        // Save raw response
        fs.writeFileSync(path.join(OUTPUT_DIR, `${model.short}-raw.json`), data);

        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            if (is401Error(parsed.error) && !keyRecovered) {
              console.error(`>>> [${model.short}] 401 — key is dead. Attempting auto-recovery...`);
              recoverKey().then(newKey => {
                OPENROUTER_KEY = newKey;
                callOpenRouter(model, true).then(resolve).catch(reject);
              }).catch(e => {
                resolve({ model: model.short, error: `Key dead and recovery failed: ${e.message}`, findings: [] });
              });
              return;
            }
            console.error(`>>> [${model.short}] API Error:`, parsed.error.message || parsed.error);
            resolve({ model: model.short, error: parsed.error.message || JSON.stringify(parsed.error), findings: [] });
            return;
          }
          const content = parsed.choices?.[0]?.message?.content || '';
          const usage = parsed.usage || {};

          // Save extracted content
          fs.writeFileSync(path.join(OUTPUT_DIR, `${model.short}-findings.md`), content);

          // Try to parse JSON findings from content
          let findings = [];
          try {
            // Find JSON array in content (might be wrapped in ```json blocks)
            const jsonMatch = content.match(/\[[\s\S]*\]/);
            if (jsonMatch) {
              findings = JSON.parse(jsonMatch[0]);
            }
          } catch (e) {
            console.log(`>>> [${model.short}] Could not parse JSON from response, saved as markdown`);
          }

          const cost = ((usage.prompt_tokens || 0) / 1e6 * (model.id.includes('qwen') ? 0.07 : model.id.includes('v3') ? 0.20 : model.id.includes('r1') ? 0.45 : 1.25)) +
                       ((usage.completion_tokens || 0) / 1e6 * (model.id.includes('qwen') ? 0.10 : model.id.includes('v3') ? 0.77 : model.id.includes('r1') ? 2.15 : 10.0));

          console.log(`>>> [${model.short}] ${findings.length} findings, ~$${cost.toFixed(3)} (${usage.prompt_tokens || '?'}in/${usage.completion_tokens || '?'}out)`);
          resolve({ model: model.short, findings, cost, usage });
        } catch (e) {
          console.error(`>>> [${model.short}] Parse error:`, e.message);
          resolve({ model: model.short, error: e.message, findings: [] });
        }
      });
    });

    req.on('error', (e) => {
      console.error(`>>> [${model.short}] Request error:`, e.message);
      resolve({ model: model.short, error: e.message, findings: [] });
    });

    req.setTimeout(model.timeout, () => {
      console.error(`>>> [${model.short}] Timeout after ${model.timeout/1000}s`);
      req.destroy();
      resolve({ model: model.short, error: 'timeout', findings: [] });
    });

    req.write(body);
    req.end();
  });
}

async function main() {
  console.log(`\n=== MMA Plan Review — ${MODELS.length} Models ===`);
  console.log(`Plan: ${PLAN_FILE}`);
  console.log(`Output: ${OUTPUT_DIR}/\n`);

  // Run all 4 models in parallel
  const results = await Promise.all(MODELS.map(m => callOpenRouter(m)));

  // Cross-model analysis
  console.log('\n=== Cross-Model Consensus ===\n');

  const allFindings = [];
  for (const r of results) {
    for (const f of r.findings) {
      allFindings.push({ ...f, source: r.model });
    }
  }

  // Group by severity
  const p1 = allFindings.filter(f => f.severity === 'P1');
  const p2 = allFindings.filter(f => f.severity === 'P2');
  const p3 = allFindings.filter(f => f.severity === 'P3');

  console.log(`P1 (Critical):  ${p1.length} findings`);
  console.log(`P2 (Important): ${p2.length} findings`);
  console.log(`P3 (Nice-to-have): ${p3.length} findings`);
  console.log(`Total: ${allFindings.length} findings from ${results.length} models\n`);

  // Find consensus P1s (mentioned by 2+ models)
  const p1Descriptions = p1.map(f => f.description.toLowerCase().slice(0, 80));
  const consensusP1s = [];
  for (let i = 0; i < p1.length; i++) {
    const similar = p1.filter((f, j) => j !== i && (
      f.description.toLowerCase().includes(p1[i].category) ||
      f.phase === p1[i].phase && f.category === p1[i].category
    ));
    if (similar.length > 0 && !consensusP1s.find(c => c.phase === p1[i].phase && c.category === p1[i].category)) {
      consensusP1s.push(p1[i]);
    }
  }

  if (consensusP1s.length > 0) {
    console.log('=== Consensus P1s (2+ models agree) ===');
    for (const f of consensusP1s) {
      console.log(`  [${f.id}] Phase ${f.phase} (${f.category}): ${f.description.slice(0, 120)}`);
    }
    console.log('');
  }

  // Save cross-model report
  const report = {
    date: new Date().toISOString(),
    models: results.map(r => ({ model: r.model, findingCount: r.findings.length, cost: r.cost, error: r.error })),
    totalFindings: allFindings.length,
    p1Count: p1.length,
    p2Count: p2.length,
    p3Count: p3.length,
    consensusP1s,
    allFindings,
  };

  fs.writeFileSync(path.join(OUTPUT_DIR, 'CROSS-MODEL-REPORT.json'), JSON.stringify(report, null, 2));

  // Human-readable report
  let md = `# MMA Plan Review — Cross-Model Report\n\n`;
  md += `**Date:** ${new Date().toISOString()}\n`;
  md += `**Models:** ${results.map(r => r.model).join(', ')}\n`;
  md += `**Total findings:** ${allFindings.length} (P1: ${p1.length}, P2: ${p2.length}, P3: ${p3.length})\n\n`;

  if (p1.length > 0) {
    md += `## P1 — Critical (Must Fix Before Execution)\n\n`;
    for (const f of p1) {
      md += `### ${f.id} [${f.source}] Phase ${f.phase} — ${f.category}\n`;
      md += `**Description:** ${f.description}\n`;
      md += `**Recommendation:** ${f.recommendation}\n\n`;
    }
  }

  if (p2.length > 0) {
    md += `## P2 — Important (Should Fix)\n\n`;
    for (const f of p2) {
      md += `### ${f.id} [${f.source}] Phase ${f.phase} — ${f.category}\n`;
      md += `**Description:** ${f.description}\n`;
      md += `**Recommendation:** ${f.recommendation}\n\n`;
    }
  }

  if (p3.length > 0) {
    md += `## P3 — Nice-to-Have\n\n`;
    for (const f of p3) {
      md += `- **${f.id}** [${f.source}] Phase ${f.phase}: ${f.description.slice(0, 200)}\n`;
    }
  }

  fs.writeFileSync(path.join(OUTPUT_DIR, 'CROSS-MODEL-REPORT.md'), md);
  console.log(`\nReport saved: ${OUTPUT_DIR}/CROSS-MODEL-REPORT.md`);
  console.log(`JSON data: ${OUTPUT_DIR}/CROSS-MODEL-REPORT.json`);

  const totalCost = results.reduce((s, r) => s + (r.cost || 0), 0);
  console.log(`\nEstimated cost: ~$${totalCost.toFixed(2)}`);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
