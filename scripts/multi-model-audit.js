#!/usr/bin/env node
// scripts/multi-model-audit.js — Multi-model system audit via OpenRouter
//
// Usage: OPENROUTER_KEY="..." MODEL="deepseek/deepseek-chat-v3-0324" node scripts/multi-model-audit.js
// Output: audit/results/<model-short>-audit-YYYY-MM-DD/
//
// Supported models:
//   deepseek/deepseek-chat-v3-0324   (DeepSeek V3, 163K ctx, $0.20/$0.77 per 1M)
//   qwen/qwen3-235b-a22b-2507       (Qwen3 235B, 262K ctx, $0.07/$0.10 per 1M)
//   deepseek/deepseek-r1-0528        (DeepSeek R1, 163K ctx, $0.45/$2.15 per 1M)
//   google/gemini-2.5-pro-preview-03-25 (Gemini 2.5 Pro, 1M ctx, $1.25/$10 per 1M)
//   xiaomi/mimo-v2-pro                  (MiMo v2 Pro, 1M ctx, $1/$3 per 1M)

const fs = require('fs');
const path = require('path');
const https = require('https');

const OPENROUTER_KEY = process.env.OPENROUTER_KEY;
const MODEL = process.env.MODEL;
if (!OPENROUTER_KEY) { console.error('ERROR: Set OPENROUTER_KEY env var'); process.exit(1); }
if (!MODEL) { console.error('ERROR: Set MODEL env var (e.g. deepseek/deepseek-chat-v3-0324)'); process.exit(1); }

const MAX_RETRIES = 2;
const REPO_ROOT = path.resolve(__dirname, '..');
const COMMS_ROOT = path.resolve(REPO_ROOT, '..', 'comms-link');

// ─── Model config ────────────────────────────────────────────────────────────
const MODEL_CONFIG = {
  'deepseek/deepseek-chat-v3-0324': { short: 'deepseek-v3', ctx: 163840, priceIn: 0.20, priceOut: 0.77, timeout: 180000, maxOut: 16000 },
  'qwen/qwen3-235b-a22b-2507':     { short: 'qwen3-235b', ctx: 262144, priceIn: 0.07, priceOut: 0.10, timeout: 180000, maxOut: 16000 },
  'deepseek/deepseek-r1-0528':      { short: 'deepseek-r1', ctx: 163840, priceIn: 0.45, priceOut: 2.15, timeout: 300000, maxOut: 16000 },
  'google/gemini-2.5-pro-preview-03-25': { short: 'gemini-2.5', ctx: 1000000, priceIn: 1.25, priceOut: 10.0, timeout: 120000, maxOut: 16000 },
  'xiaomi/mimo-v2-pro':                  { short: 'mimo-v2-pro', ctx: 1048576, priceIn: 1.00, priceOut: 3.00, timeout: 180000, maxOut: 16000 },
  // Round 2 models (2026-03-27)
  'openai/gpt-5-mini':                   { short: 'gpt5-mini', ctx: 400000, priceIn: 0.25, priceOut: 2.00, timeout: 180000, maxOut: 16000 },
  'x-ai/grok-4.1-fast':                  { short: 'grok-4.1', ctx: 2000000, priceIn: 0.20, priceOut: 0.50, timeout: 180000, maxOut: 16000 },
  'meta-llama/llama-4-maverick':         { short: 'llama4-mav', ctx: 1048576, priceIn: 0.15, priceOut: 0.60, timeout: 180000, maxOut: 16000 },
  'mistralai/mistral-small-2603':        { short: 'mistral-sm4', ctx: 262144, priceIn: 0.15, priceOut: 0.60, timeout: 180000, maxOut: 16000 },
  // Round 3 models — code-specialized (2026-03-27)
  'openai/gpt-5.1-codex-mini':           { short: 'codex-mini', ctx: 400000, priceIn: 0.25, priceOut: 2.00, timeout: 300000, maxOut: 16000 },
  'x-ai/grok-code-fast-1':               { short: 'grok-code', ctx: 256000, priceIn: 0.20, priceOut: 1.50, timeout: 180000, maxOut: 16000 },
  'qwen/qwen3-coder':                    { short: 'qwen3-coder', ctx: 262144, priceIn: 0.22, priceOut: 1.00, timeout: 300000, maxOut: 16000 },
  'bytedance-seed/seed-2.0-mini':        { short: 'seed2-mini', ctx: 262144, priceIn: 0.10, priceOut: 0.40, timeout: 180000, maxOut: 16000 },
  // Round 4 models — new providers (2026-03-27)
  'nvidia/nemotron-3-super-120b-a12b':   { short: 'nemotron-super', ctx: 262144, priceIn: 0.10, priceOut: 0.50, timeout: 180000, maxOut: 16000 },
  'z-ai/glm-4.7':                        { short: 'glm-4.7', ctx: 202752, priceIn: 0.39, priceOut: 1.75, timeout: 180000, maxOut: 16000 },
  'tencent/hunyuan-a13b-instruct':       { short: 'hunyuan', ctx: 131072, priceIn: 0.14, priceOut: 0.57, timeout: 180000, maxOut: 16000 },
  'inception/mercury-coder':             { short: 'mercury-coder', ctx: 128000, priceIn: 0.25, priceOut: 0.75, timeout: 180000, maxOut: 16000 },
};

const config = MODEL_CONFIG[MODEL];
if (!config) {
  console.error(`Unknown model: ${MODEL}`);
  console.error(`Supported: ${Object.keys(MODEL_CONFIG).join(', ')}`);
  process.exit(1);
}

const dateStr = new Date().toISOString().split('T')[0];
const OUTPUT_DIR = path.join(REPO_ROOT, 'audit', 'results', `${config.short}-audit-${dateStr}`);
fs.mkdirSync(OUTPUT_DIR, { recursive: true });

// ─── File helpers ────────────────────────────────────────────────────────────
function readFile(filePath) {
  try { return fs.readFileSync(filePath, 'utf-8'); } catch { return null; }
}

function readFilesFromDir(dir, extensions, maxDepth = 3, currentDepth = 0) {
  const results = [];
  if (currentDepth > maxDepth || !fs.existsSync(dir)) return results;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (['node_modules', '.git', 'target', '.next', 'dist', '.planning'].includes(entry.name)) continue;
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

// ─── Auto-split for context limits ───────────────────────────────────────────
function splitBatchIfNeeded(batch, maxTokens) {
  const est = estimateTokens(SYSTEM_PROMPT + batch.prompt);
  const limit = Math.floor(maxTokens * 0.75); // 75% safety margin for output
  if (est <= limit) return [batch];

  // Split by finding the midpoint of the files in the prompt
  const fileMarker = '--- FILE:';
  const parts = batch.prompt.split(fileMarker);
  const header = parts[0]; // text before first file
  const files = parts.slice(1);
  const mid = Math.ceil(files.length / 2);

  const part1Prompt = header + files.slice(0, mid).map(f => fileMarker + f).join('');
  const part2Prompt = header + files.slice(mid).map(f => fileMarker + f).join('');

  return [
    { name: `${batch.name}-part1`, title: `${batch.title} (Part 1/${2})`, prompt: part1Prompt },
    { name: `${batch.name}-part2`, title: `${batch.title} (Part 2/${2})`, prompt: part2Prompt },
  ];
}

// ─── OpenRouter API ──────────────────────────────────────────────────────────
function callModel(systemPrompt, userPrompt, retries = 0) {
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
        'X-Title': `Racing Point Audit (${config.short})`
      }
    };

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
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

// ─── System prompt ───────────────────────────────────────────────────────────
const SYSTEM_PROMPT = `You are an expert systems auditor performing a comprehensive audit of "Racing Point eSports" — a sim racing venue with 8 pods, a server, and cloud infrastructure.

Architecture:
- Rust/Axum monorepo (racecontrol server :8080, rc-agent on pods :8090, rc-sentry :8091, rc-watchdog service)
- Next.js apps (admin :3201, web :3200, kiosk :3300)
- Node.js comms-link (James↔Bono AI coordination, WS :8765, relay :8766)
- Bash audit/healing/detection pipeline
- Windows pods with NVIDIA Surround triple monitors, Edge kiosk, game launching

Your audit must find:
1. SECURITY: credential leaks, auth gaps, injection, privilege escalation, missing validation
2. CODE QUALITY: unwrap() in Rust, "any" in TypeScript, error handling gaps, race conditions
3. RELIABILITY: single points of failure, missing retries, silent failures, crash loop risks
4. INTEGRATION: API contract mismatches, serialization gaps (serde silent drops), field name drift
5. PROCESS: standing rule violations, missing cascade updates, deploy pipeline gaps
6. INFRASTRUCTURE: config drift, stale references, missing health checks, monitoring blind spots

IMPORTANT — also look for ABSENCE-BASED issues:
7. What SHOULD exist but DOESN'T? Missing timeouts, missing state transitions, missing error paths, missing validation that the code assumes exists elsewhere
8. State machine issues: can any state get stuck permanently? Are there missing transitions or unreachable states?
9. Cross-system assumptions: does this code assume something about another component that might not be true?

For each finding, report:
- SEVERITY: P1 (critical/security), P2 (reliability/data), P3 (quality/process)
- CATEGORY: security|reliability|integration|code-quality|process|infrastructure|absence
- FILE: exact file path
- LINE: approximate line number if possible
- FINDING: what's wrong
- IMPACT: what could happen
- FIX: recommended action

Be thorough. Flag EVERYTHING suspicious. Better to over-report than miss a real issue.
Pay special attention to things that are MISSING, not just things that are wrong.`;

// ─── Audit batches ───────────────────────────────────────────────────────────
async function runAudit() {
  console.log(`=== Racing Point Full Audit via ${config.short} (${MODEL}) ===`);
  console.log(`Context: ${(config.ctx / 1024).toFixed(0)}K | Pricing: $${config.priceIn}/$${config.priceOut} per 1M`);
  console.log(`Output: ${OUTPUT_DIR}\n`);

  let rawBatches = [];
  let totalInputTokens = 0;
  let totalOutputTokens = 0;

  // Batch 1: Core Rust — racecontrol server
  const serverRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'racecontrol', 'src'), ['.rs']);
  rawBatches.push({
    name: '01-server-rust', title: 'Racecontrol Server (Rust/Axum)',
    prompt: `Audit the racecontrol server — the central Rust/Axum service running on :8080.
Focus on: route auth coverage, SQL injection, error handling (.unwrap()), WebSocket security, fleet exec safety, billing logic, game state management, API endpoint validation.
Also check: missing timeouts on state transitions (GameTracker stuck states), missing DB transactions on financial ops, serde silent field drops on cross-boundary structs.

${bundleFiles(serverRs)}`
  });

  // Batch 2: rc-agent (pod agent)
  const agentRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-agent', 'src'), ['.rs']);
  rawBatches.push({
    name: '02-agent-rust', title: 'RC-Agent Pod Agent (Rust)',
    prompt: `Audit the rc-agent — runs on each of 8 Windows pods (:8090). Handles game launching, lock screen, process guard, health reporting, remote exec.
Focus on: command injection via exec endpoint, process guard bypass, game launch security, self-restart safety, Session 0 vs Session 1 issues, Windows-specific bugs.
Also check: does the agent detect if it's running in Session 0 (services) vs Session 1 (interactive)? Can the lock screen get stuck? Is MAINTENANCE_MODE sentinel handled with a TTL?

${bundleFiles(agentRs)}`
  });

  // Batch 3: rc-sentry + rc-watchdog + rc-common
  const sentryRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-sentry', 'src'), ['.rs']);
  const watchdogRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-watchdog', 'src'), ['.rs']);
  const commonRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-common', 'src'), ['.rs']);
  const guardRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-process-guard', 'src'), ['.rs']);
  rawBatches.push({
    name: '03-sentry-watchdog-common', title: 'RC-Sentry, RC-Watchdog, RC-Common, Process Guard (Rust)',
    prompt: `Audit the supporting Rust crates:
- rc-sentry (:8091): pod watchdog, restart logic, schtasks integration
- rc-watchdog: Windows service for Session 1 process recovery (WTSQueryUserToken)
- rc-common: shared types, boot resilience, config
- rc-process-guard: allowlist enforcement, violation tracking

Focus on: restart loop safety, MAINTENANCE_MODE handling, Session 0/1 correctness, allowlist bypass, type mismatches between crates, silent failures.
Also check: can recovery systems fight each other (sentry restart vs watchdog restart vs WoL)? Is there coordination?

${bundleFiles([...sentryRs, ...watchdogRs, ...commonRs, ...guardRs])}`
  });

  // Batch 4: Comms-link (Node.js)
  const commsShared = readFilesFromDir(path.join(COMMS_ROOT, 'shared'), ['.js']);
  const commsJames = readFilesFromDir(path.join(COMMS_ROOT, 'james'), ['.js']);
  const commsBono = readFilesFromDir(path.join(COMMS_ROOT, 'bono'), ['.js']);
  const commsRoot = [
    { path: 'comms-link/send-message.js', content: readFile(path.join(COMMS_ROOT, 'send-message.js')) },
    { path: 'comms-link/send-exec.js', content: readFile(path.join(COMMS_ROOT, 'send-exec.js')) },
    { path: 'comms-link/chains.json', content: readFile(path.join(COMMS_ROOT, 'chains.json')) },
  ].filter(f => f.content);
  rawBatches.push({
    name: '04-comms-link', title: 'Comms-Link (Node.js — James↔Bono coordination)',
    prompt: `Audit the comms-link system — WebSocket-based coordination between James (on-site AI) and Bono (VPS AI).
Focus on: PSK authentication strength, exec command injection, shell relay safety, dynamic registry abuse, message tampering, audit log integrity, chain orchestration race conditions.

${bundleFiles([...commsShared, ...commsJames, ...commsBono, ...commsRoot])}`
  });

  // Batch 5: Audit/Detection/Healing pipeline (Bash)
  const auditLib = readFilesFromDir(path.join(REPO_ROOT, 'audit', 'lib'), ['.sh']);
  const auditPhases = readFilesFromDir(path.join(REPO_ROOT, 'audit', 'phases'), ['.sh'], 2);
  const auditRoot = [
    { path: 'audit/audit.sh', content: readFile(path.join(REPO_ROOT, 'audit', 'audit.sh')) },
    { path: 'audit/suppress.json', content: readFile(path.join(REPO_ROOT, 'audit', 'suppress.json')) },
  ].filter(f => f.content);
  const detectors = readFilesFromDir(path.join(REPO_ROOT, 'scripts', 'detectors'), ['.sh']);
  const healing = readFilesFromDir(path.join(REPO_ROOT, 'scripts', 'healing'), ['.sh']);
  const autoDetect = [
    { path: 'scripts/auto-detect.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'auto-detect.sh')) },
    { path: 'scripts/cascade.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'cascade.sh')) },
  ].filter(f => f.content);
  rawBatches.push({
    name: '05-audit-detection-healing', title: 'Audit Pipeline, Detectors, Healing Engine (Bash)',
    prompt: `Audit the autonomous detection and healing pipeline:
- audit/audit.sh + lib/*.sh: 60-phase audit runner with parallel execution
- scripts/detectors/*.sh: crash loop, config drift, log anomaly, schema gap, bat drift, flag desync detection
- scripts/healing/escalation-engine.sh: 5-tier graduated escalation (retry→restart→WoL→cloud failover→human)
- scripts/auto-detect.sh: orchestrator that runs detectors and feeds findings to healing

Focus on: race conditions in parallel execution, sentinel file handling, billing gate bypass, escalation loop risks, suppress.json expiry bugs, notification flooding, command injection in bash, error handling.

${bundleFiles([...auditLib, ...auditPhases, ...auditRoot, ...detectors, ...healing, ...autoDetect])}`
  });

  // Batch 6: Deploy pipeline + configs
  const deployScripts = readFilesFromDir(path.join(REPO_ROOT, 'scripts', 'deploy'), ['.sh', '.bat', '.ps1']);
  const rootConfigs = [
    { path: 'scripts/stage-release.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'stage-release.sh')) },
    { path: 'scripts/deploy-pod.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'deploy-pod.sh')) },
    { path: 'scripts/deploy-server.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'deploy-server.sh')) },
    { path: 'Cargo.toml', content: readFile(path.join(REPO_ROOT, 'Cargo.toml')) },
    { path: '.cargo/config.toml', content: readFile(path.join(REPO_ROOT, '.cargo', 'config.toml')) },
  ].filter(f => f.content);
  rawBatches.push({
    name: '06-deploy-infra', title: 'Deploy Pipeline, Configs, Infrastructure',
    prompt: `Audit the deploy pipeline and infrastructure configs:
- stage-release.sh: security pre-flight → cargo build → SHA256 → manifest
- deploy-pod.sh / deploy-server.sh: binary deployment with security gates
- Cargo.toml: workspace config, dependencies, features
- .cargo/config.toml: static CRT, build flags

Focus on: deploy pipeline integrity, binary verification gaps, rollback safety, dependency vulnerabilities, build reproducibility, manifest tampering, missing security gates.

${bundleFiles([...deployScripts, ...rootConfigs])}`
  });

  // Batch 7: Standing rules + cross-system
  const claudeMd = readFile(path.join(REPO_ROOT, 'CLAUDE.md')) || '';
  const commsClaude = readFile(path.join(COMMS_ROOT, 'CLAUDE.md')) || '';
  rawBatches.push({
    name: '07-standing-rules-crosssystem', title: 'Standing Rules, Cross-System Integration, Process Compliance',
    prompt: `Review the standing rules and cross-system integration:

RACECONTROL CLAUDE.md (standing rules + operational context):
${claudeMd}

COMMS-LINK CLAUDE.md (shared operational context):
${commsClaude}

Audit for:
1. RULE CONFLICTS: Are any standing rules contradictory or ambiguous?
2. COVERAGE GAPS: What failure modes are NOT covered by current rules?
3. CROSS-SYSTEM: Where can kiosk↔server↔agent↔sentry↔comms data mismatches occur?
4. PROCESS GAPS: What's missing from deploy, audit, escalation, and notification workflows?
5. STALE REFERENCES: Do any rules reference deprecated systems, old IPs, or removed features?
6. SECURITY BLIND SPOTS: What attack vectors are not covered by current security gates?

Be specific. Reference exact rule text when flagging issues.`
  });

  // Batch 8: Frontend (Next.js/TypeScript)
  const kioskSrc = readFilesFromDir(path.join(REPO_ROOT, 'kiosk', 'src'), ['.ts', '.tsx']);
  const webSrc = readFilesFromDir(path.join(REPO_ROOT, 'web', 'src'), ['.ts', '.tsx']);
  const adminSrc = readFilesFromDir(path.join(REPO_ROOT, 'admin', 'src'), ['.ts', '.tsx']);
  rawBatches.push({
    name: '08-frontend-nextjs', title: 'Frontend Apps (Next.js/TypeScript — Kiosk, Web, Admin)',
    prompt: `Audit the three Next.js frontend applications:
- kiosk (:3300): customer-facing pod control, game selection wizard, billing session display
- web (:3200): staff dashboard, fleet overview, billing management, leaderboards
- admin (:3201): admin panel, fleet management, feature flags, system health

Focus on:
1. XSS: any use of unsafe innerHTML, unescaped user input in JSX, URL parameter injection
2. AUTH TOKEN HANDLING: where are JWTs stored (localStorage vs httpOnly cookie)? Are tokens sent to correct origins only? Token refresh logic gaps?
3. CORS: are fetch/axios calls restricted to expected origins? Any wildcard CORS?
4. COOKIE FLAGS: httpOnly, secure, sameSite on auth cookies
5. EXPOSED NEXT_PUBLIC_ VARS: grep for NEXT_PUBLIC_ — any secrets leaked? Any vars defaulting to localhost (breaks remote browsers)?
6. SSR/CSR BOUNDARY: sessionStorage/localStorage read in useState initializer (hydration mismatch)? useEffect+hydrated pattern used correctly?
7. UNSAFE HTML RENDERING: any usage of React unsafe HTML injection? Is input sanitized before rendering?
8. API URL CONSTRUCTION: any string concatenation with user input in fetch URLs?
9. WEBSOCKET SECURITY: WS connections authenticated? Reconnection logic safe?
10. ERROR BOUNDARIES: do pages have error boundaries or do component errors crash the entire app?

${bundleFiles([...kioskSrc, ...webSrc, ...adminSrc])}`
  });

  // Auto-split batches that exceed context
  let batches = [];
  for (const batch of rawBatches) {
    batches.push(...splitBatchIfNeeded(batch, config.ctx));
  }

  console.log(`Prepared ${batches.length} audit batches (${rawBatches.length} original, ${batches.length - rawBatches.length} split)\n`);

  const allFindings = [];

  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    const inputTokens = estimateTokens(SYSTEM_PROMPT + batch.prompt);
    console.log(`[${i + 1}/${batches.length}] ${batch.title}`);
    console.log(`  Est. input: ~${(inputTokens / 1000).toFixed(0)}K tokens`);

    if (inputTokens > config.ctx * 0.9) {
      console.log(`  WARNING: Batch may exceed context (${(inputTokens / 1000).toFixed(0)}K > ${(config.ctx * 0.9 / 1000).toFixed(0)}K). Results may be truncated.`);
    }

    try {
      const result = await callModel(SYSTEM_PROMPT, batch.prompt);

      const actualIn = result.usage.prompt_tokens || inputTokens;
      const actualOut = result.usage.completion_tokens || 0;
      totalInputTokens += actualIn;
      totalOutputTokens += actualOut;

      const cost = (actualIn / 1e6) * config.priceIn + (actualOut / 1e6) * config.priceOut;
      console.log(`  Actual: ${actualIn} in / ${actualOut} out`);
      console.log(`  Cost: $${cost.toFixed(4)}`);

      const outputPath = path.join(OUTPUT_DIR, `${batch.name}.md`);
      fs.writeFileSync(outputPath, `# Audit Batch: ${batch.title}\n\n` +
        `**Model:** ${MODEL}\n` +
        `**Tokens:** ${actualIn} input / ${actualOut} output\n` +
        `**Cost:** $${cost.toFixed(4)}\n\n` +
        `---\n\n${result.content}\n`);

      allFindings.push({ batch: batch.title, content: result.content });
      console.log(`  Saved: ${outputPath}\n`);

      // Rate limit courtesy between batches
      if (i < batches.length - 1) {
        await new Promise(r => setTimeout(r, 2000));
      }
    } catch (err) {
      console.error(`  ERROR: ${err.message}\n`);
      allFindings.push({ batch: batch.title, content: `ERROR: ${err.message}` });
      const outputPath = path.join(OUTPUT_DIR, `${batch.name}.md`);
      fs.writeFileSync(outputPath, `# Audit Batch: ${batch.title}\n\n**ERROR:** ${err.message}\n`);
    }
  }

  // Combined report
  const totalCost = (totalInputTokens / 1e6) * config.priceIn + (totalOutputTokens / 1e6) * config.priceOut;
  const modelName = config.short.charAt(0).toUpperCase() + config.short.slice(1);

  let combined = `# Racing Point Full System Audit — ${modelName}\n\n`;
  combined += `**Date:** ${new Date().toISOString()}\n`;
  combined += `**Model:** ${MODEL} (via OpenRouter)\n`;
  combined += `**Total Tokens:** ${totalInputTokens.toLocaleString()} input / ${totalOutputTokens.toLocaleString()} output\n`;
  combined += `**Total Cost:** $${totalCost.toFixed(4)}\n`;
  combined += `**Batches:** ${batches.length}\n\n---\n\n`;

  for (const finding of allFindings) {
    combined += `## ${finding.batch}\n\n${finding.content}\n\n---\n\n`;
  }

  const combinedPath = path.join(OUTPUT_DIR, 'FULL-AUDIT-REPORT.md');
  fs.writeFileSync(combinedPath, combined);

  console.log('=== AUDIT COMPLETE ===');
  console.log(`Model: ${MODEL} (${config.short})`);
  console.log(`Total: ${totalInputTokens.toLocaleString()} in / ${totalOutputTokens.toLocaleString()} out`);
  console.log(`Cost: $${totalCost.toFixed(4)}`);
  console.log(`Report: ${combinedPath}`);
}

runAudit().catch(err => { console.error('Fatal error:', err); process.exit(1); });
