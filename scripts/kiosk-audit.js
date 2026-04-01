#!/usr/bin/env node
// scripts/kiosk-audit.js — Kiosk-focused multi-model audit via OpenRouter
//
// Usage: OPENROUTER_KEY="sk-or-v1-..." MODEL="deepseek/deepseek-chat-v3-0324" node scripts/kiosk-audit.js
// Output: audit/results/kiosk-<model-short>-audit-YYYY-MM-DD/
//
// Follows the 3-round methodology: General → Code → Reasoning
// Each round should fix+commit before the next.
//
// Security note: all execSync calls use hardcoded git commands with no user input.
// This is a CLI audit tool, not production code exposed to external input.

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

const { recoverKey, is401Error, loadSavedKey, bootstrapKey } = require('./lib/openrouter-key-recovery');

let OPENROUTER_KEY = process.env.OPENROUTER_KEY || loadSavedKey();
const MODEL = process.env.MODEL;
// Deferred bootstrap — resolved before first API call via ensureKey()
if (!MODEL) { console.error('ERROR: Set MODEL env var'); process.exit(1); }

const MAX_RETRIES = 2;
const REPO_ROOT = path.resolve(__dirname, '..');
const KIOSK_ROOT = path.join(REPO_ROOT, 'kiosk');

// ─── Model config ────────────────────────────────────────────────────────────
const MODEL_CONFIG = {
  // Round 1 — General
  'deepseek/deepseek-chat-v3-0324': { short: 'deepseek-v3', ctx: 163840, priceIn: 0.20, priceOut: 0.77, timeout: 180000, maxOut: 16000 },
  'google/gemini-2.5-pro-preview-03-25': { short: 'gemini-2.5', ctx: 1000000, priceIn: 1.25, priceOut: 10.0, timeout: 180000, maxOut: 16000 },
  // Round 2 — Code-specialized
  'x-ai/grok-code-fast-1':              { short: 'grok-code', ctx: 256000, priceIn: 0.20, priceOut: 1.50, timeout: 180000, maxOut: 16000 },
  'openai/gpt-5.1-codex-mini':          { short: 'codex-mini', ctx: 400000, priceIn: 0.25, priceOut: 2.00, timeout: 300000, maxOut: 16000 },
  // Round 3 — Reasoning
  'deepseek/deepseek-r1-0528':          { short: 'deepseek-r1', ctx: 163840, priceIn: 0.45, priceOut: 2.15, timeout: 300000, maxOut: 16000 },
  'moonshotai/kimi-k2.5':               { short: 'kimi-k2.5', ctx: 131072, priceIn: 0.22, priceOut: 1.00, timeout: 300000, maxOut: 16000 },
  // Round 4 — Vision + multimodal (image/UI understanding)
  'google/gemini-2.5-flash':             { short: 'gemini-flash', ctx: 1000000, priceIn: 0.15, priceOut: 0.60, timeout: 180000, maxOut: 16000 },
  'openai/gpt-4.1':                     { short: 'gpt-4.1', ctx: 1048576, priceIn: 2.00, priceOut: 8.00, timeout: 300000, maxOut: 16000 },
  // Round 5 — Additional code/programming models
  'qwen/qwen3-coder':                   { short: 'qwen3-coder', ctx: 262144, priceIn: 0.22, priceOut: 1.00, timeout: 300000, maxOut: 16000 },
  'anthropic/claude-sonnet-4':          { short: 'claude-sonnet-4', ctx: 200000, priceIn: 3.00, priceOut: 15.00, timeout: 300000, maxOut: 16000 },
};

const config = MODEL_CONFIG[MODEL];
if (!config) {
  console.error(`Unknown model: ${MODEL}\nSupported: ${Object.keys(MODEL_CONFIG).join(', ')}`);
  process.exit(1);
}

const dateStr = new Date().toISOString().split('T')[0];
const OUTPUT_DIR = path.join(REPO_ROOT, 'audit', 'results', `kiosk-${config.short}-audit-${dateStr}`);
fs.mkdirSync(OUTPUT_DIR, { recursive: true });

// ─── File helpers ────────────────────────────────────────────────────────────
function readFile(filePath) {
  try { return fs.readFileSync(filePath, 'utf-8'); } catch { return null; }
}

function readFilesFromDir(dir, extensions, maxDepth = 4, currentDepth = 0) {
  const results = [];
  if (currentDepth > maxDepth || !fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (['node_modules', '.git', 'target', '.next', 'dist', '__tests__'].includes(entry.name)) continue;
    if (entry.isDirectory()) {
      results.push(...readFilesFromDir(fullPath, extensions, maxDepth, currentDepth + 1));
    } else if (extensions.some(ext => entry.name.endsWith(ext))) {
      const content = readFile(fullPath);
      if (content && content.length < 50000) {
        results.push({ path: path.relative(REPO_ROOT, fullPath).replace(/\\/g, '/'), content });
      }
    }
  }
  return results;
}

function bundleFiles(files) {
  return files.map(f => `--- FILE: ${f.path} ---\n${f.content}`).join('\n\n');
}

function estimateTokens(text) { return Math.ceil(text.length / 4); }

// ─── OpenRouter API ──────────────────────────────────────────────────────────
function callModel(systemPrompt, userPrompt, retries = 0, keyRecovered = false) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: MODEL,
      messages: [
        { role: 'system', content: systemPrompt },
        { role: 'user', content: userPrompt }
      ],
      max_tokens: config.maxOut,
      temperature: 0.2
    });

    const options = {
      hostname: 'openrouter.ai',
      path: '/api/v1/chat/completions',
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${OPENROUTER_KEY}`,
        'HTTP-Referer': 'https://racingpoint.in',
        'X-Title': `Racing Point Kiosk Audit (${config.short})`
      }
    };

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            if (is401Error(parsed.error) && !keyRecovered) {
              console.log('  401 — key is dead. Attempting auto-recovery...');
              recoverKey().then(newKey => {
                OPENROUTER_KEY = newKey;
                callModel(systemPrompt, userPrompt, retries, true).then(resolve).catch(reject);
              }).catch(e => {
                reject(new Error(`Key dead (401) and recovery failed: ${e.message}`));
              });
              return;
            }
            if (retries < MAX_RETRIES) {
              console.log(`  Retry ${retries + 1}/${MAX_RETRIES}... (${parsed.error.message || 'unknown'})`);
              setTimeout(() => callModel(systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject), 5000 * (retries + 1));
              return;
            }
            reject(new Error(`API error: ${JSON.stringify(parsed.error)}`));
            return;
          }
          const content = parsed.choices?.[0]?.message?.content || '';
          const usage = parsed.usage || {};
          resolve({ content, usage });
        } catch (e) {
          reject(new Error(`Parse error: ${e.message}\nRaw: ${data.slice(0, 500)}`));
        }
      });
    });

    req.on('error', (e) => {
      if (retries < MAX_RETRIES) {
        setTimeout(() => callModel(systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject), 5000 * (retries + 1));
        return;
      }
      reject(e);
    });

    req.setTimeout(config.timeout, () => {
      req.destroy();
      reject(new Error(`Request timeout (${config.timeout / 1000}s)`));
    });

    req.write(body);
    req.end();
  });
}

// ─── Kiosk-specific system prompt ────────────────────────────────────────────
const SYSTEM_PROMPT = `You are an expert frontend security and code quality auditor. You are auditing the KIOSK application of "Racing Point eSports" — a sim racing venue with 8 pods.

The kiosk is a Next.js (TypeScript) app running on :3300 with basePath "/kiosk". It is the CUSTOMER-FACING interface displayed on each pod's screen. Customers use it to:
- Select and launch racing games (Assetto Corsa, F1, Forza, iRacing, LMU)
- View their active billing session timer and costs
- See live telemetry during racing
- Register as drivers, top up wallet
- View leaderboards and PBs

Architecture:
- Next.js 15 with App Router, standalone output mode
- Talks to Rust/Axum server at :8080 via REST API and WebSocket
- WebSocket for real-time updates (pod state, billing, game state, telemetry)
- Runs in Edge kiosk mode on Windows pods with NVIDIA Surround triple monitors
- Staff login (PIN) for privileged operations (refunds, manual billing, deploy)

CRITICAL STANDING RULES from this project (violations are bugs):
1. No "any" in TypeScript — type everything explicitly
2. No sessionStorage/localStorage in useState initializer — must use useEffect + hydrated flag (SSR hydration mismatch)
3. No dangerouslySetInnerHTML with unsanitized input (XSS)
4. NEXT_PUBLIC_ vars must never contain secrets and must not default to localhost
5. Every field sent to the Rust API must match the exact Rust struct field name — serde silently drops unknown fields
6. WS tokens must not be in URLs (use sub-protocol or headers)
7. Error boundaries required — component errors must not crash the entire app
8. Fetch calls need AbortController timeouts (30s)
9. API URL construction must not use string concatenation with user input

Your audit must find:
1. SECURITY: XSS (innerHTML, unescaped input), auth token leaks, CORS issues, exposed secrets in NEXT_PUBLIC_ vars, JWT handling gaps
2. CODE QUALITY: "any" types, missing error handling, TypeScript strict violations, dead code, unused imports
3. RELIABILITY: missing error boundaries, unhandled promise rejections, missing loading states, race conditions in async ops, WS reconnection gaps
4. INTEGRATION: field name mismatches between kiosk TypeScript and Rust API structs (CRITICAL — serde silently drops), missing API response validation, stale API URLs
5. UX: missing loading indicators, no timeout feedback, stuck states, accessibility issues
6. ABSENCE: what SHOULD exist but DOESN'T? Missing input validation, missing timeouts, missing state cleanup on unmount, missing null checks

I am also providing the Rust API types (from rc-common and racecontrol) so you can CROSS-CHECK field names and types between the TypeScript kiosk code and the Rust backend. Pay special attention to:
- TypeScript field names vs Rust struct field names (must match exactly for serde)
- TypeScript types vs Rust types (e.g., number vs u32/i64, string vs SimType enum)
- Missing fields that the Rust API expects but the kiosk doesn't send

For each finding, report:
- SEVERITY: P1 (security/data-loss), P2 (reliability/integration), P3 (quality/UX)
- CATEGORY: security|reliability|integration|code-quality|ux|absence
- FILE: exact file path
- LINE: approximate line number
- FINDING: what's wrong
- IMPACT: what could happen
- FIX: recommended action

Be thorough. Flag EVERYTHING suspicious. Better to over-report than miss a real issue.`;

// ─── Pre-scan freshness check ────────────────────────────────────────────────
function checkCodebaseFreshness() {
  try {
    const gitOpts = { cwd: REPO_ROOT, encoding: 'utf-8' };
    const headHash = execSync('git rev-parse --short HEAD', gitOpts).trim();
    const headMsg = execSync('git log -1 --format=%s', gitOpts).trim();
    const headTime = execSync('git log -1 --format=%ci', gitOpts).trim();

    const dirtyFiles = execSync('git diff --name-only HEAD -- kiosk/', gitOpts).trim();
    const stagedFiles = execSync('git diff --cached --name-only -- kiosk/', gitOpts).trim();
    const hasUncommitted = dirtyFiles.length > 0 || stagedFiles.length > 0;

    // Check for prior kiosk audit rounds today
    const resultsDir = path.join(REPO_ROOT, 'audit', 'results');
    const todayKioskResults = fs.existsSync(resultsDir)
      ? fs.readdirSync(resultsDir).filter(d => d.startsWith('kiosk-') && d.endsWith(`-audit-${dateStr}`) && d !== `kiosk-${config.short}-audit-${dateStr}`)
      : [];

    console.log('--- Pre-Scan Freshness Check (Kiosk) -------------------------');
    console.log(`  HEAD: ${headHash} -- "${headMsg}"`);
    console.log(`  Time: ${headTime}`);

    if (hasUncommitted) {
      console.log('  WARNING: Uncommitted kiosk changes detected');
      console.log('  -> Models will audit WORKING TREE (includes uncommitted fixes)');
    } else {
      console.log('  OK: Working tree clean for kiosk/ -- auditing commit ' + headHash);
    }

    if (todayKioskResults.length > 0) {
      const earliestResult = todayKioskResults.sort()[0];
      const resultDir = path.join(resultsDir, earliestResult);
      const resultTime = fs.statSync(resultDir).mtime;
      const commitsSinceStr = execSync(
        `git log --oneline --since="${resultTime.toISOString()}" -- kiosk/`,
        gitOpts
      ).trim();
      const commitsSince = commitsSinceStr ? commitsSinceStr.split('\n').length : 0;

      console.log(`  Prior kiosk rounds today: ${todayKioskResults.length} (${todayKioskResults.join(', ')})`);
      if (commitsSince > 0) {
        console.log(`  OK: ${commitsSince} kiosk fix commit(s) since first round`);
      } else {
        console.log('  WARNING: No kiosk fix commits since prior round(s)');
        if (!process.env.AUDIT_ALLOW_STALE) {
          console.log('\n  BLOCKED: Set AUDIT_ALLOW_STALE=1 to override');
          console.log('-------------------------------------------------------------\n');
          process.exit(2);
        }
        console.log('  -> AUDIT_ALLOW_STALE=1 set -- proceeding anyway');
      }
    }

    console.log('-------------------------------------------------------------\n');

    fs.writeFileSync(path.join(OUTPUT_DIR, '_freshness.json'), JSON.stringify({
      head_hash: headHash, head_message: headMsg, head_time: headTime,
      has_uncommitted_changes: hasUncommitted, scan_time: new Date().toISOString(),
      prior_kiosk_rounds: todayKioskResults
    }, null, 2));

    return headHash;
  } catch (e) {
    console.log('  Freshness check skipped');
    return 'unknown';
  }
}

// ─── Audit execution ─────────────────────────────────────────────────────────
async function runAudit() {
  if (!OPENROUTER_KEY) {
    console.log('[bootstrap] No API key — auto-provisioning...');
    try { OPENROUTER_KEY = await bootstrapKey(); } catch (e) {
      console.error(`[bootstrap] FATAL: ${e.message}`); process.exit(1);
    }
  }
  if (!MODEL) { console.error('ERROR: Set MODEL env var'); process.exit(1); }
  console.log(`=== Racing Point KIOSK Audit via ${config.short} (${MODEL}) ===`);
  console.log(`Context: ${(config.ctx / 1024).toFixed(0)}K | Pricing: $${config.priceIn}/$${config.priceOut} per 1M`);
  console.log(`Output: ${OUTPUT_DIR}\n`);

  const auditedCommit = checkCodebaseFreshness();

  let totalInputTokens = 0;
  let totalOutputTokens = 0;
  const allFindings = [];

  // ─── Bundle kiosk source files ───────────────────────────────────────────
  const kioskSrc = readFilesFromDir(path.join(KIOSK_ROOT, 'src'), ['.ts', '.tsx']);
  const kioskConfigs = [
    { path: 'kiosk/next.config.ts', content: readFile(path.join(KIOSK_ROOT, 'next.config.ts')) },
    { path: 'kiosk/package.json', content: readFile(path.join(KIOSK_ROOT, 'package.json')) },
    { path: 'kiosk/.env.production.local', content: readFile(path.join(KIOSK_ROOT, '.env.production.local')) },
  ].filter(f => f.content);

  // ─── Bundle Rust API types for cross-boundary checking ───────────────────
  const rustTypes = [
    { path: 'crates/rc-common/src/types.rs', content: readFile(path.join(REPO_ROOT, 'crates', 'rc-common', 'src', 'types.rs')) },
    { path: 'crates/rc-common/src/protocol.rs', content: readFile(path.join(REPO_ROOT, 'crates', 'rc-common', 'src', 'protocol.rs')) },
  ].filter(f => f.content);

  const kioskBundle = bundleFiles([...kioskSrc, ...kioskConfigs]);
  const rustBundle = bundleFiles(rustTypes);

  // ─── Build batches ───────────────────────────────────────────────────────
  const fullPrompt = `Audit ALL kiosk source code. This is the complete kiosk application:

${kioskBundle}

--- RUST API TYPES (for cross-boundary checking) ---
The following Rust types define the server-side API contract. Check that every field the kiosk sends matches the Rust struct field name and type EXACTLY. Serde will silently drop mismatched fields.

${rustBundle}`;

  const totalTokens = estimateTokens(SYSTEM_PROMPT + fullPrompt);
  console.log(`Total estimated tokens: ~${(totalTokens / 1000).toFixed(0)}K`);

  let batches;
  if (totalTokens <= config.ctx * 0.75) {
    // Fits in one batch
    batches = [{
      name: '01-kiosk-full',
      title: 'Complete Kiosk Audit (Source + Integration)',
      prompt: fullPrompt
    }];
  } else {
    // Split: kiosk source in batch 1, integration focus in batch 2
    console.log(`Splitting into 2 batches (${(totalTokens / 1000).toFixed(0)}K > ${(config.ctx * 0.75 / 1000).toFixed(0)}K limit)`);
    batches = [
      {
        name: '01-kiosk-source',
        title: 'Kiosk Source — Security, Quality, UX',
        prompt: `Audit the kiosk source code for security, code quality, reliability, and UX issues:\n\n${kioskBundle}`
      },
      {
        name: '02-kiosk-integration',
        title: 'Kiosk↔Rust Integration — Cross-Boundary Contract',
        prompt: `Audit the kiosk↔server integration. Cross-check every field name and type between TypeScript and Rust.

KIOSK API + HOOKS + TYPES:
${bundleFiles(kioskSrc.filter(f => f.path.includes('lib/') || f.path.includes('hooks/')))}

RUST API CONTRACT:
${rustBundle}`
      }
    ];
  }

  console.log(`Prepared ${batches.length} batch(es)\n`);

  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    const inputTokens = estimateTokens(SYSTEM_PROMPT + batch.prompt);
    console.log(`[${i + 1}/${batches.length}] ${batch.title}`);
    console.log(`  Est. input: ~${(inputTokens / 1000).toFixed(0)}K tokens`);

    try {
      const result = await callModel(SYSTEM_PROMPT, batch.prompt);
      const actualIn = result.usage.prompt_tokens || inputTokens;
      const actualOut = result.usage.completion_tokens || 0;
      totalInputTokens += actualIn;
      totalOutputTokens += actualOut;

      const cost = (actualIn / 1e6) * config.priceIn + (actualOut / 1e6) * config.priceOut;
      console.log(`  Actual: ${actualIn} in / ${actualOut} out — $${cost.toFixed(4)}`);

      const outputPath = path.join(OUTPUT_DIR, `${batch.name}.md`);
      fs.writeFileSync(outputPath, `# Kiosk Audit: ${batch.title}\n\n` +
        `**Model:** ${MODEL}\n` +
        `**Tokens:** ${actualIn} input / ${actualOut} output\n` +
        `**Cost:** $${cost.toFixed(4)}\n\n---\n\n${result.content}\n`);

      allFindings.push({ batch: batch.title, content: result.content });
      console.log(`  Saved: ${outputPath}\n`);

      if (i < batches.length - 1) await new Promise(r => setTimeout(r, 2000));
    } catch (err) {
      console.error(`  ERROR: ${err.message}\n`);
      allFindings.push({ batch: batch.title, content: `ERROR: ${err.message}` });
    }
  }

  // ─── Combined report ─────────────────────────────────────────────────────
  const totalCost = (totalInputTokens / 1e6) * config.priceIn + (totalOutputTokens / 1e6) * config.priceOut;

  let report = `# Racing Point KIOSK Audit — ${config.short}\n\n`;
  report += `**Date:** ${new Date().toISOString()}\n`;
  report += `**Model:** ${MODEL} (via OpenRouter)\n`;
  report += `**Audited Commit:** ${auditedCommit}\n`;
  report += `**Total Tokens:** ${totalInputTokens.toLocaleString()} input / ${totalOutputTokens.toLocaleString()} output\n`;
  report += `**Total Cost:** $${totalCost.toFixed(4)}\n`;
  report += `**Scope:** Kiosk only (${kioskSrc.length} source files)\n\n---\n\n`;

  for (const f of allFindings) {
    report += `## ${f.batch}\n\n${f.content}\n\n---\n\n`;
  }

  const reportPath = path.join(OUTPUT_DIR, 'KIOSK-AUDIT-REPORT.md');
  fs.writeFileSync(reportPath, report);

  console.log('=== KIOSK AUDIT COMPLETE ===');
  console.log(`Model: ${MODEL} (${config.short})`);
  console.log(`Total: ${totalInputTokens.toLocaleString()} in / ${totalOutputTokens.toLocaleString()} out`);
  console.log(`Cost: $${totalCost.toFixed(4)}`);
  console.log(`Report: ${reportPath}`);
}

runAudit().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
