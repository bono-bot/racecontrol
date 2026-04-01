#!/usr/bin/env node
// scripts/gemini-audit.js — Full system audit via Gemini 2.5 Pro (OpenRouter)
//
// Usage: OPENROUTER_KEY="sk-or-v1-..." node scripts/gemini-audit.js
// Output: audit/results/gemini-audit-YYYY-MM-DD/

const fs = require('fs');
const path = require('path');
const https = require('https');

const { recoverKey, is401Error, loadSavedKey, bootstrapKey } = require('./lib/openrouter-key-recovery');

let OPENROUTER_KEY = process.env.OPENROUTER_KEY || loadSavedKey();
// Deferred bootstrap — resolved before first API call

const REPO_ROOT = path.resolve(__dirname, '..');
const COMMS_ROOT = path.resolve(REPO_ROOT, '..', 'comms-link');
const MODEL = 'google/gemini-2.5-pro-preview-03-25';
const MAX_RETRIES = 2;

// Output directory
const dateStr = new Date().toISOString().split('T')[0];
const OUTPUT_DIR = path.join(REPO_ROOT, 'audit', 'results', `gemini-audit-${dateStr}`);
fs.mkdirSync(OUTPUT_DIR, { recursive: true });

// ─── File reader helpers ─────────────────────────────────────────────────────

function readFile(filePath) {
  try {
    const content = fs.readFileSync(filePath, 'utf-8');
    return content;
  } catch {
    return null;
  }
}

function readFilesFromDir(dir, extensions, maxDepth = 3, currentDepth = 0) {
  const results = [];
  if (currentDepth > maxDepth) return results;
  if (!fs.existsSync(dir)) return results;

  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.name === 'node_modules' || entry.name === '.git' ||
        entry.name === 'target' || entry.name === '.next' ||
        entry.name === 'dist' || entry.name === '.planning') continue;

    if (entry.isDirectory()) {
      results.push(...readFilesFromDir(fullPath, extensions, maxDepth, currentDepth + 1));
    } else if (extensions.some(ext => entry.name.endsWith(ext))) {
      const content = readFile(fullPath);
      if (content && content.length < 50000) { // skip huge files
        const relPath = path.relative(REPO_ROOT, fullPath).replace(/\\/g, '/');
        results.push({ path: relPath, content });
      }
    }
  }
  return results;
}

function bundleFiles(files) {
  return files.map(f => `--- FILE: ${f.path} ---\n${f.content}`).join('\n\n');
}

function estimateTokens(text) {
  return Math.ceil(text.length / 4);
}

// ─── OpenRouter API caller ───────────────────────────────────────────────────

function callGemini(systemPrompt, userPrompt, retries = 0, keyRecovered = false) {
  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: MODEL,
      messages: [
        { role: 'system', content: systemPrompt },
        { role: 'user', content: userPrompt }
      ],
      max_tokens: 16000,
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
        'X-Title': 'Racing Point Audit'
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
                callGemini(systemPrompt, userPrompt, retries, true).then(resolve).catch(reject);
              }).catch(e => {
                reject(new Error(`Key dead (401) and recovery failed: ${e.message}`));
              });
              return;
            }
            if (retries < MAX_RETRIES) {
              console.log(`  Retry ${retries + 1}/${MAX_RETRIES}...`);
              setTimeout(() => {
                callGemini(systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject);
              }, 5000);
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
        setTimeout(() => {
          callGemini(systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject);
        }, 5000);
        return;
      }
      reject(e);
    });

    req.setTimeout(120000, () => {
      req.destroy();
      reject(new Error('Request timeout (120s)'));
    });

    req.write(body);
    req.end();
  });
}

// ─── Audit batches ───────────────────────────────────────────────────────────

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

For each finding, report:
- SEVERITY: P1 (critical/security), P2 (reliability/data), P3 (quality/process)
- CATEGORY: security|reliability|integration|code-quality|process|infrastructure
- FILE: exact file path
- LINE: approximate line number if possible
- FINDING: what's wrong
- IMPACT: what could happen
- FIX: recommended action

Be thorough. Flag EVERYTHING suspicious. Better to over-report than miss a real issue.`;

async function runAudit() {
  if (!OPENROUTER_KEY) {
    console.log('[bootstrap] No API key — auto-provisioning...');
    try { OPENROUTER_KEY = await bootstrapKey(); } catch (e) {
      console.error(`[bootstrap] FATAL: ${e.message}`); process.exit(1);
    }
  }
  console.log('=== Racing Point Full Audit via Gemini 2.5 Pro ===');
  console.log(`Output: ${OUTPUT_DIR}`);
  console.log('');

  const batches = [];
  let totalInputTokens = 0;
  let totalOutputTokens = 0;

  // ── Batch 1: Core Rust — racecontrol server ──
  const serverRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'racecontrol', 'src'), ['.rs']);
  batches.push({
    name: '01-server-rust',
    title: 'Racecontrol Server (Rust/Axum)',
    prompt: `Audit the racecontrol server — the central Rust/Axum service running on :8080.
Focus on: route auth coverage, SQL injection, error handling (.unwrap()), WebSocket security, fleet exec safety, billing logic, game state management, API endpoint validation.

${bundleFiles(serverRs)}`
  });

  // ── Batch 2: rc-agent (pod agent) ──
  const agentRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-agent', 'src'), ['.rs']);
  batches.push({
    name: '02-agent-rust',
    title: 'RC-Agent Pod Agent (Rust)',
    prompt: `Audit the rc-agent — runs on each of 8 Windows pods (:8090). Handles game launching, lock screen, process guard, health reporting, remote exec.
Focus on: command injection via exec endpoint, process guard bypass, game launch security, self-restart safety, Session 0 vs Session 1 issues, Windows-specific bugs.

${bundleFiles(agentRs)}`
  });

  // ── Batch 3: rc-sentry + rc-watchdog + rc-common ──
  const sentryRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-sentry', 'src'), ['.rs']);
  const watchdogRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-watchdog', 'src'), ['.rs']);
  const commonRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-common', 'src'), ['.rs']);
  const guardRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-process-guard', 'src'), ['.rs']);
  batches.push({
    name: '03-sentry-watchdog-common',
    title: 'RC-Sentry, RC-Watchdog, RC-Common, Process Guard (Rust)',
    prompt: `Audit the supporting Rust crates:
- rc-sentry (:8091): pod watchdog, restart logic, schtasks integration
- rc-watchdog: Windows service for Session 1 process recovery (WTSQueryUserToken)
- rc-common: shared types, boot resilience, config
- rc-process-guard: allowlist enforcement, violation tracking

Focus on: restart loop safety, MAINTENANCE_MODE handling, Session 0/1 correctness, allowlist bypass, type mismatches between crates, silent failures.

${bundleFiles([...sentryRs, ...watchdogRs, ...commonRs, ...guardRs])}`
  });

  // ── Batch 4: Comms-link (Node.js) ──
  const commsShared = readFilesFromDir(path.join(COMMS_ROOT, 'shared'), ['.js']);
  const commsJames = readFilesFromDir(path.join(COMMS_ROOT, 'james'), ['.js']);
  const commsBono = readFilesFromDir(path.join(COMMS_ROOT, 'bono'), ['.js']);
  const commsRoot = [
    { path: 'comms-link/send-message.js', content: readFile(path.join(COMMS_ROOT, 'send-message.js')) },
    { path: 'comms-link/send-exec.js', content: readFile(path.join(COMMS_ROOT, 'send-exec.js')) },
    { path: 'comms-link/chains.json', content: readFile(path.join(COMMS_ROOT, 'chains.json')) },
  ].filter(f => f.content);
  batches.push({
    name: '04-comms-link',
    title: 'Comms-Link (Node.js — James↔Bono coordination)',
    prompt: `Audit the comms-link system — WebSocket-based coordination between James (on-site AI) and Bono (VPS AI).
Focus on: PSK authentication strength, exec command injection, shell relay safety, dynamic registry abuse, message tampering, audit log integrity, chain orchestration race conditions.

${bundleFiles([...commsShared, ...commsJames, ...commsBono, ...commsRoot])}`
  });

  // ── Batch 5: Audit/Detection/Healing pipeline (Bash) ──
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
  batches.push({
    name: '05-audit-detection-healing',
    title: 'Audit Pipeline, Detectors, Healing Engine (Bash)',
    prompt: `Audit the autonomous detection and healing pipeline:
- audit/audit.sh + lib/*.sh: 60-phase audit runner with parallel execution
- scripts/detectors/*.sh: crash loop, config drift, log anomaly, schema gap, bat drift, flag desync detection
- scripts/healing/escalation-engine.sh: 5-tier graduated escalation (retry→restart→WoL→cloud failover→human)
- scripts/auto-detect.sh: orchestrator that runs detectors and feeds findings to healing

Focus on: race conditions in parallel execution, sentinel file handling, billing gate bypass, escalation loop risks, suppress.json expiry bugs, notification flooding, command injection in bash, error handling.

${bundleFiles([...auditLib, ...auditPhases, ...auditRoot, ...detectors, ...healing, ...autoDetect])}`
  });

  // ── Batch 6: Deploy pipeline + configs + bat files ──
  const deployScripts = readFilesFromDir(path.join(REPO_ROOT, 'scripts', 'deploy'), ['.sh', '.bat', '.ps1']);
  const rootConfigs = [
    { path: 'scripts/stage-release.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'stage-release.sh')) },
    { path: 'scripts/deploy-pod.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'deploy-pod.sh')) },
    { path: 'scripts/deploy-server.sh', content: readFile(path.join(REPO_ROOT, 'scripts', 'deploy-server.sh')) },
    { path: 'Cargo.toml', content: readFile(path.join(REPO_ROOT, 'Cargo.toml')) },
    { path: '.cargo/config.toml', content: readFile(path.join(REPO_ROOT, '.cargo', 'config.toml')) },
  ].filter(f => f.content);
  batches.push({
    name: '06-deploy-infra',
    title: 'Deploy Pipeline, Configs, Infrastructure',
    prompt: `Audit the deploy pipeline and infrastructure configs:
- stage-release.sh: security pre-flight → cargo build → SHA256 → manifest
- deploy-pod.sh / deploy-server.sh: binary deployment with security gates
- Cargo.toml: workspace config, dependencies, features
- .cargo/config.toml: static CRT, build flags

Focus on: deploy pipeline integrity, binary verification gaps, rollback safety, dependency vulnerabilities, build reproducibility, manifest tampering, missing security gates.

${bundleFiles([...deployScripts, ...rootConfigs])}`
  });

  // ── Batch 7: Standing rules + CLAUDE.md + cross-system review ──
  const claudeMd = readFile(path.join(REPO_ROOT, 'CLAUDE.md')) || '';
  const commsClaude = readFile(path.join(COMMS_ROOT, 'CLAUDE.md')) || '';
  batches.push({
    name: '07-standing-rules-crosssystem',
    title: 'Standing Rules, Cross-System Integration, Process Compliance',
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

  // ── Run all batches ──
  console.log(`Prepared ${batches.length} audit batches\n`);

  const allFindings = [];

  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    const inputTokens = estimateTokens(SYSTEM_PROMPT + batch.prompt);
    console.log(`[${i + 1}/${batches.length}] ${batch.title}`);
    console.log(`  Est. input: ~${(inputTokens / 1000).toFixed(0)}K tokens`);

    try {
      const result = await callGemini(SYSTEM_PROMPT, batch.prompt);

      const actualIn = result.usage.prompt_tokens || inputTokens;
      const actualOut = result.usage.completion_tokens || 0;
      totalInputTokens += actualIn;
      totalOutputTokens += actualOut;

      console.log(`  Actual: ${actualIn} in / ${actualOut} out`);
      console.log(`  Cost: $${((actualIn / 1e6) * 1.25 + (actualOut / 1e6) * 10).toFixed(4)}`);

      // Save individual batch result
      const outputPath = path.join(OUTPUT_DIR, `${batch.name}.md`);
      fs.writeFileSync(outputPath, `# Audit Batch: ${batch.title}\n\n` +
        `**Model:** ${MODEL}\n` +
        `**Tokens:** ${actualIn} input / ${actualOut} output\n` +
        `**Cost:** $${((actualIn / 1e6) * 1.25 + (actualOut / 1e6) * 10).toFixed(4)}\n\n` +
        `---\n\n${result.content}\n`);

      allFindings.push({ batch: batch.title, content: result.content });
      console.log(`  Saved: ${outputPath}`);
      console.log('');

      // Rate limit courtesy
      if (i < batches.length - 1) {
        await new Promise(r => setTimeout(r, 2000));
      }
    } catch (err) {
      console.error(`  ERROR: ${err.message}`);
      allFindings.push({ batch: batch.title, content: `ERROR: ${err.message}` });

      const outputPath = path.join(OUTPUT_DIR, `${batch.name}.md`);
      fs.writeFileSync(outputPath, `# Audit Batch: ${batch.title}\n\n**ERROR:** ${err.message}\n`);
    }
  }

  // ── Write combined report ──
  const totalCost = (totalInputTokens / 1e6) * 1.25 + (totalOutputTokens / 1e6) * 10;

  let combined = `# Racing Point Full System Audit — Gemini 2.5 Pro\n\n`;
  combined += `**Date:** ${new Date().toISOString()}\n`;
  combined += `**Model:** ${MODEL} (via OpenRouter)\n`;
  combined += `**Total Tokens:** ${totalInputTokens.toLocaleString()} input / ${totalOutputTokens.toLocaleString()} output\n`;
  combined += `**Total Cost:** $${totalCost.toFixed(4)}\n`;
  combined += `**Batches:** ${batches.length}\n\n`;
  combined += `---\n\n`;

  for (const finding of allFindings) {
    combined += `## ${finding.batch}\n\n${finding.content}\n\n---\n\n`;
  }

  const combinedPath = path.join(OUTPUT_DIR, 'FULL-AUDIT-REPORT.md');
  fs.writeFileSync(combinedPath, combined);
  console.log('=== AUDIT COMPLETE ===');
  console.log(`Total: ${totalInputTokens.toLocaleString()} in / ${totalOutputTokens.toLocaleString()} out`);
  console.log(`Cost: $${totalCost.toFixed(4)}`);
  console.log(`Report: ${combinedPath}`);
}

runAudit().catch(err => {
  console.error('Fatal error:', err);
  process.exit(1);
});
