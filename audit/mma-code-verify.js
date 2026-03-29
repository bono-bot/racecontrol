#!/usr/bin/env node
// MMA Code Verification — Review actual code changes after execution
// Usage: OPENROUTER_KEY="..." node audit/mma-code-verify.js

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execFileSync } = require('child_process');

const OPENROUTER_KEY = process.env.OPENROUTER_KEY;
if (!OPENROUTER_KEY) { console.error('ERROR: Set OPENROUTER_KEY'); process.exit(1); }

// Get the actual diff using execFileSync (safe, no shell injection)
const diff = execFileSync('git', ['diff', 'HEAD'], { maxBuffer: 1024 * 1024 * 10, cwd: path.resolve(__dirname, '..') }).toString();

// Get new files content
const newFiles = ['crates/racecontrol/src/telemetry_store.rs', 'crates/racecontrol/src/driver_rating.rs', 'web/src/app/leaderboard-display/page.tsx'];
let newFileContents = '';
for (const f of newFiles) {
  const fullPath = path.join(__dirname, '..', f);
  if (fs.existsSync(fullPath)) {
    const content = fs.readFileSync(fullPath, 'utf-8');
    newFileContents += `\n=== ${f} ===\n${content.slice(0, 15000)}\n`;
  }
}

const dateStr = new Date().toISOString().split('T')[0];
const OUTPUT_DIR = path.join(__dirname, 'results', `mma-code-verify-${dateStr}`);
fs.mkdirSync(OUTPUT_DIR, { recursive: true });

const MODELS = [
  { id: 'qwen/qwen3-235b-a22b-2507', short: 'qwen3', timeout: 180000 },
  { id: 'deepseek/deepseek-chat-v3-0324', short: 'deepseek-v3', timeout: 180000 },
  { id: 'deepseek/deepseek-r1-0528', short: 'deepseek-r1', timeout: 300000 },
  { id: 'google/gemini-2.5-pro-preview-03-25', short: 'gemini-2.5', timeout: 180000 },
];

const SYSTEM_PROMPT = `You are a senior Rust/TypeScript code reviewer auditing actual code changes for a racing esports venue management system.

SYSTEM: Rust/Axum + SQLite + Next.js. 8 gaming pods, 1 server, 3 display machines. Production system handling real billing and customer data.

THE CHANGES IMPLEMENT:
1. Telemetry sample persistence — separate telemetry.db, batched writer, 10Hz cap
2. Driver skill rating system — pace/consistency/experience algorithm, async worker
3. Telemetry visualization — recharts-based charts on leaderboard pages
4. Real-time leaderboard updates — WS broadcast on record breaks, debounced
5. Leaderboard display kiosk — full-screen auto-rotating display for wall-mounted screens

REVIEW THE CODE FOR:
1. **Bugs** — logic errors, off-by-one, race conditions, null/None mishandling
2. **Security** — injection, auth bypass, PII leaks, unsafe unwrap
3. **Performance** — unnecessary allocations, missing indexes, blocking async, lock contention
4. **Correctness** — edge cases (empty DB, no laps, concurrent writes, WS disconnects)
5. **Standing rules violations** — .unwrap() in production Rust, 'any' in TypeScript, locks across .await

Return ONLY a JSON array. Each finding: {"id": "V-XX", "severity": "P1|P2|P3", "file": "path", "line_hint": "approx line or description", "description": "...", "fix": "specific code fix"}

P1 = will cause crash, data loss, or security breach in production
P2 = degraded experience or maintenance burden
P3 = improvement opportunity`;

function callOpenRouter(model, content) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: model.id,
      max_tokens: 16000,
      temperature: 0.3,
      messages: [
        { role: 'system', content: SYSTEM_PROMPT },
        { role: 'user', content }
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

    console.log(`>>> [${model.short}] Sending...`);
    const startTime = Date.now();

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        const elapsed = ((Date.now() - startTime) / 1000).toFixed(1);
        console.log(`>>> [${model.short}] Done in ${elapsed}s (HTTP ${res.statusCode})`);
        fs.writeFileSync(path.join(OUTPUT_DIR, `${model.short}-raw.json`), data);
        try {
          const parsed = JSON.parse(data);
          const content = parsed.choices?.[0]?.message?.content || '';
          fs.writeFileSync(path.join(OUTPUT_DIR, `${model.short}-findings.md`), content);
          let findings = [];
          try {
            const jsonMatch = content.match(/\[[\s\S]*\]/);
            if (jsonMatch) findings = JSON.parse(jsonMatch[0]);
          } catch (e) {}
          console.log(`>>> [${model.short}] ${findings.length} findings`);
          resolve({ model: model.short, findings });
        } catch (e) {
          resolve({ model: model.short, findings: [], error: e.message });
        }
      });
    });
    req.on('error', (e) => resolve({ model: model.short, findings: [], error: e.message }));
    req.setTimeout(model.timeout, () => { req.destroy(); resolve({ model: model.short, findings: [], error: 'timeout' }); });
    req.write(body);
    req.end();
  });
}

async function main() {
  console.log(`\n=== MMA Code Verification — ${MODELS.length} Models ===`);
  console.log(`Diff size: ${(diff.length / 1024).toFixed(1)}KB`);
  console.log(`New files: ${newFiles.join(', ')}\n`);

  // Truncate diff if too large (keep first 60K chars)
  const truncatedDiff = diff.length > 60000 ? diff.slice(0, 60000) + '\n\n[... truncated ...]' : diff;

  const reviewContent = `Review these code changes:\n\n## NEW FILES\n${newFileContents}\n\n## DIFF (modified files)\n\`\`\`diff\n${truncatedDiff}\n\`\`\``;

  const results = await Promise.all(MODELS.map(m => callOpenRouter(m, reviewContent)));

  const allFindings = [];
  for (const r of results) {
    for (const f of r.findings) allFindings.push({ ...f, source: r.model });
  }

  const p1 = allFindings.filter(f => f.severity === 'P1');
  const p2 = allFindings.filter(f => f.severity === 'P2');
  const p3 = allFindings.filter(f => f.severity === 'P3');

  console.log(`\nP1: ${p1.length}, P2: ${p2.length}, P3: ${p3.length}, Total: ${allFindings.length}`);

  let md = `# MMA Code Verification Report\n\n`;
  md += `**Date:** ${new Date().toISOString()}\n`;
  md += `**Total:** ${allFindings.length} (P1: ${p1.length}, P2: ${p2.length}, P3: ${p3.length})\n\n`;

  for (const sev of ['P1', 'P2', 'P3']) {
    const items = allFindings.filter(f => f.severity === sev);
    if (items.length === 0) continue;
    md += `## ${sev}\n\n`;
    for (const f of items) {
      md += `### ${f.id} [${f.source}] ${f.file || 'general'}\n`;
      md += `${f.description}\n**Fix:** ${f.fix}\n\n`;
    }
  }

  fs.writeFileSync(path.join(OUTPUT_DIR, 'VERIFICATION-REPORT.md'), md);
  fs.writeFileSync(path.join(OUTPUT_DIR, 'findings.json'), JSON.stringify(allFindings, null, 2));
  console.log(`\nReport: ${OUTPUT_DIR}/VERIFICATION-REPORT.md`);
}

main().catch(e => { console.error('Fatal:', e); process.exit(1); });
