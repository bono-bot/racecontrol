#!/usr/bin/env node
// scripts/multi-model-audit.js — Multi-Model Audit v3.0 (Unified MMA Protocol aligned)
//
// Implements: Stratified model selection, 3/5 consensus voting, adversarial verification,
// vendor diversity enforcement, budget tracking, domain rosters, input sanitization.
//
// Usage:
//   # v3.0 consensus mode (DEFAULT — 5 models per batch, consensus voting, adversarial verify)
//   OPENROUTER_KEY="..." node scripts/multi-model-audit.js
//
//   # Override domain (default: auto-detect per batch)
//   OPENROUTER_KEY="..." AUDIT_DOMAIN="rust_backend" node scripts/multi-model-audit.js
//
//   # Single-model legacy mode (backward compatible)
//   OPENROUTER_KEY="..." MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js
//
//   # Budget override (default $5)
//   OPENROUTER_KEY="..." MMA_SESSION_BUDGET=10 node scripts/multi-model-audit.js
//
//   # Dry run (no API calls, validates model selection + batch prep)
//   OPENROUTER_KEY="..." DRY_RUN=1 node scripts/multi-model-audit.js
//
// Spec: .planning/specs/UNIFIED-MMA-PROTOCOL.md (v3.0, 844 lines)
//
// Security note: all execSync calls use hardcoded git commands with no user input.
// This is a CLI audit tool, not production code exposed to external input.

const fs = require('fs');
const path = require('path');
const https = require('https');
const { execSync } = require('child_process');

const { recoverKey, is401Error, loadSavedKey } = require('./lib/openrouter-key-recovery');

// Mutable key — updated in-process on 401 recovery
let OPENROUTER_KEY = process.env.OPENROUTER_KEY || loadSavedKey();
if (!OPENROUTER_KEY) { console.error('ERROR: Set OPENROUTER_KEY env var'); process.exit(1); }

const LEGACY_MODEL = process.env.MODEL; // backward compat: single model mode
const DRY_RUN = process.env.DRY_RUN === '1';
const SESSION_BUDGET = parseFloat(process.env.MMA_SESSION_BUDGET || '5');
const AUDIT_ALLOW_STALE = process.env.AUDIT_ALLOW_STALE === '1';
const MAX_RETRIES = 2;
const MODEL_TIMEOUT = 60000; // MMA-16: 60s per model call
const REPO_ROOT = path.resolve(__dirname, '..');
const COMMS_ROOT = path.resolve(REPO_ROOT, '..', 'comms-link');

// ─── Model Registry (v3.0 — from spec Part 8 domain rosters) ────────────────
// Each model has: id, short name, vendor family, roles[], ctx, pricing, timeout
const MODEL_REGISTRY = {
  // Reasoners
  'deepseek/deepseek-r1-0528':              { short: 'deepseek-r1',     vendor: 'deepseek', roles: ['reasoner'],    ctx: 163840,  priceIn: 0.45, priceOut: 2.15, timeout: 180000, maxOut: 16000 },
  'moonshotai/kimi-k2.5':                   { short: 'kimi-k2.5',      vendor: 'moonshot',  roles: ['reasoner'],    ctx: 131072,  priceIn: 0.35, priceOut: 1.40, timeout: 300000, maxOut: 16000 },
  // Code Experts
  'deepseek/deepseek-chat-v3-0324':         { short: 'deepseek-v3',    vendor: 'deepseek', roles: ['code_expert'], ctx: 163840,  priceIn: 0.20, priceOut: 0.77, timeout: 180000, maxOut: 16000 },
  'x-ai/grok-code-fast-1':                  { short: 'grok-code',      vendor: 'xai',       roles: ['code_expert'], ctx: 256000,  priceIn: 0.20, priceOut: 1.50, timeout: 180000, maxOut: 16000 },
  'qwen/qwen3-coder':                       { short: 'qwen3-coder',    vendor: 'qwen',      roles: ['code_expert'], ctx: 262144,  priceIn: 0.22, priceOut: 1.00, timeout: 300000, maxOut: 16000 },
  'inception/mercury-coder':                 { short: 'mercury-coder',  vendor: 'inception',  roles: ['code_expert'], ctx: 128000,  priceIn: 0.25, priceOut: 0.75, timeout: 180000, maxOut: 16000 },
  'openai/gpt-5.1-codex-mini':              { short: 'codex-mini',     vendor: 'openai',    roles: ['code_expert'], ctx: 400000,  priceIn: 0.25, priceOut: 2.00, timeout: 300000, maxOut: 16000 },
  // SRE/Ops
  'xiaomi/mimo-v2-pro':                      { short: 'mimo-v2-pro',    vendor: 'xiaomi',    roles: ['sre'],         ctx: 1048576, priceIn: 1.00, priceOut: 3.00, timeout: 180000, maxOut: 16000 },
  'nvidia/nemotron-3-super-120b-a12b':       { short: 'nemotron-super', vendor: 'nvidia',    roles: ['sre'],         ctx: 262144,  priceIn: 0.10, priceOut: 0.50, timeout: 180000, maxOut: 16000 },
  // Generalists
  'qwen/qwen3-235b-a22b-2507':              { short: 'qwen3-235b',     vendor: 'qwen',      roles: ['generalist'],  ctx: 262144,  priceIn: 0.07, priceOut: 0.10, timeout: 180000, maxOut: 16000 },
  'google/gemini-2.5-pro-preview-03-25':     { short: 'gemini-2.5',     vendor: 'google',    roles: ['generalist'],  ctx: 1000000, priceIn: 1.25, priceOut: 10.0, timeout: 120000, maxOut: 16000 },
  'mistralai/mistral-small-2603':            { short: 'mistral-sm4',    vendor: 'mistral',   roles: ['generalist'],  ctx: 262144,  priceIn: 0.15, priceOut: 0.60, timeout: 180000, maxOut: 16000 },
  // Additional pool
  'openai/gpt-5-mini':                       { short: 'gpt5-mini',      vendor: 'openai',    roles: ['generalist'],  ctx: 400000,  priceIn: 0.25, priceOut: 2.00, timeout: 180000, maxOut: 16000 },
  'x-ai/grok-4.1-fast':                      { short: 'grok-4.1',       vendor: 'xai',       roles: ['generalist'],  ctx: 2000000, priceIn: 0.20, priceOut: 0.50, timeout: 180000, maxOut: 16000 },
  'meta-llama/llama-4-maverick':             { short: 'llama4-mav',     vendor: 'meta',      roles: ['generalist'],  ctx: 1048576, priceIn: 0.15, priceOut: 0.60, timeout: 180000, maxOut: 16000 },
  'bytedance-seed/seed-2.0-mini':            { short: 'seed2-mini',     vendor: 'bytedance',  roles: ['code_expert'], ctx: 262144,  priceIn: 0.10, priceOut: 0.40, timeout: 180000, maxOut: 16000 },
  'z-ai/glm-4.7':                            { short: 'glm-4.7',        vendor: 'zhipu',     roles: ['generalist'],  ctx: 202752,  priceIn: 0.39, priceOut: 1.75, timeout: 180000, maxOut: 16000 },
  'tencent/hunyuan-a13b-instruct':           { short: 'hunyuan',        vendor: 'tencent',   roles: ['generalist'],  ctx: 131072,  priceIn: 0.14, priceOut: 0.57, timeout: 180000, maxOut: 16000 },
  'z-ai/glm-5':                              { short: 'glm-5',          vendor: 'zhipu',     roles: ['reasoner'],    ctx: 262144,  priceIn: 0.50, priceOut: 2.00, timeout: 300000, maxOut: 16000 },
  'minimax/minimax-m2.7':                    { short: 'minimax-m2.7',   vendor: 'minimax',   roles: ['generalist'],  ctx: 1048576, priceIn: 0.50, priceOut: 2.00, timeout: 300000, maxOut: 16000 },
};

// ─── Domain Rosters (from spec Part 8) ──────────────────────────────────────
// Priority ordering per domain — first 5 are primary, rest are secondary/reserves
const DOMAIN_ROSTER = {
  rust_backend: [
    'deepseek/deepseek-r1-0528', 'deepseek/deepseek-chat-v3-0324', 'qwen/qwen3-coder',
    'x-ai/grok-code-fast-1', 'nvidia/nemotron-3-super-120b-a12b',
    'meta-llama/llama-4-maverick', 'inception/mercury-coder', 'mistralai/mistral-small-2603',
    'moonshotai/kimi-k2.5', 'qwen/qwen3-235b-a22b-2507',
  ],
  nodejs_frontend: [
    'x-ai/grok-4.1-fast', 'openai/gpt-5-mini', 'google/gemini-2.5-pro-preview-03-25',
    'mistralai/mistral-small-2603', 'qwen/qwen3-235b-a22b-2507',
    'deepseek/deepseek-chat-v3-0324', 'bytedance-seed/seed-2.0-mini', 'moonshotai/kimi-k2.5',
    'meta-llama/llama-4-maverick', 'xiaomi/mimo-v2-pro',
  ],
  windows_os: [
    'deepseek/deepseek-r1-0528', 'nvidia/nemotron-3-super-120b-a12b',
    'xiaomi/mimo-v2-pro', 'qwen/qwen3-235b-a22b-2507', 'moonshotai/kimi-k2.5',
    'z-ai/glm-4.7', 'x-ai/grok-4.1-fast', 'mistralai/mistral-small-2603',
    'openai/gpt-5-mini', 'deepseek/deepseek-chat-v3-0324',
  ],
  security: [
    'google/gemini-2.5-pro-preview-03-25', 'deepseek/deepseek-r1-0528',
    'xiaomi/mimo-v2-pro', 'moonshotai/kimi-k2.5', 'qwen/qwen3-235b-a22b-2507',
    'x-ai/grok-4.1-fast', 'nvidia/nemotron-3-super-120b-a12b', 'mistralai/mistral-small-2603',
    'openai/gpt-5-mini', 'z-ai/glm-5',
  ],
  sre_ops: [
    'xiaomi/mimo-v2-pro', 'nvidia/nemotron-3-super-120b-a12b', 'deepseek/deepseek-r1-0528',
    'qwen/qwen3-235b-a22b-2507', 'mistralai/mistral-small-2603',
    'deepseek/deepseek-chat-v3-0324', 'moonshotai/kimi-k2.5', 'x-ai/grok-4.1-fast',
    'meta-llama/llama-4-maverick', 'openai/gpt-5-mini',
  ],
  cross_system: [
    'deepseek/deepseek-r1-0528', 'qwen/qwen3-235b-a22b-2507', 'google/gemini-2.5-pro-preview-03-25',
    'xiaomi/mimo-v2-pro', 'moonshotai/kimi-k2.5',
    'deepseek/deepseek-chat-v3-0324', 'nvidia/nemotron-3-super-120b-a12b', 'x-ai/grok-4.1-fast',
    'mistralai/mistral-small-2603', 'openai/gpt-5-mini',
  ],
};

// Batch → domain mapping
const BATCH_DOMAIN = {
  '01-server-rust':              'rust_backend',
  '02-agent-rust':               'rust_backend',
  '03-sentry-watchdog-common':   'windows_os',
  '04-comms-link':               'nodejs_frontend',
  '05-audit-detection-healing':  'sre_ops',
  '06-deploy-infra':             'sre_ops',
  '07-standing-rules-crosssystem': 'cross_system',
  '08-frontend-nextjs':          'nodejs_frontend',
};

// ─── Budget Tracker ─────────────────────────────────────────────────────────
const budgetTracker = {
  totalCost: 0,
  calls: [],
  track(modelId, inputTokens, outputTokens) {
    const config = MODEL_REGISTRY[modelId];
    if (!config) return 0;
    const cost = (inputTokens / 1e6) * config.priceIn + (outputTokens / 1e6) * config.priceOut;
    this.totalCost += cost;
    this.calls.push({ model: config.short, inputTokens, outputTokens, cost, timestamp: new Date().toISOString() });
    return cost;
  },
  checkBudget() {
    if (this.totalCost >= SESSION_BUDGET) {
      console.error(`\n  BUDGET EXCEEDED: $${this.totalCost.toFixed(4)} >= $${SESSION_BUDGET} cap`);
      return false;
    }
    return true;
  },
  remaining() { return Math.max(0, SESSION_BUDGET - this.totalCost); },
  summary() {
    return {
      total_cost: `$${this.totalCost.toFixed(4)}`,
      budget: `$${SESSION_BUDGET}`,
      remaining: `$${this.remaining().toFixed(4)}`,
      total_calls: this.calls.length,
      per_model: this.calls.reduce((acc, c) => {
        acc[c.model] = (acc[c.model] || 0) + c.cost;
        return acc;
      }, {}),
    };
  },
};

// ─── Model Selection Engine (MMA-05: Vendor Diversity) ──────────────────────
function selectModels(domain, count = 5, exclude = []) {
  const roster = DOMAIN_ROSTER[domain] || DOMAIN_ROSTER.cross_system;
  const candidates = roster.filter(id => !exclude.includes(id) && MODEL_REGISTRY[id]);

  // Enforce: ≥1 reasoner + ≥1 code_expert + ≥1 SRE (MMA-05)
  const selected = [];
  const usedVendors = {};

  function addModel(id) {
    const cfg = MODEL_REGISTRY[id];
    usedVendors[cfg.vendor] = (usedVendors[cfg.vendor] || 0) + 1;
    selected.push(id);
  }

  function canAdd(id) {
    const cfg = MODEL_REGISTRY[id];
    // Max 2 per vendor family
    if ((usedVendors[cfg.vendor] || 0) >= 2) return false;
    return !selected.includes(id);
  }

  // Step 1: Fill required roles
  const requiredRoles = ['reasoner', 'code_expert', 'sre'];
  for (const role of requiredRoles) {
    const candidate = candidates.find(id => {
      const cfg = MODEL_REGISTRY[id];
      return cfg.roles.includes(role) && canAdd(id);
    });
    if (candidate) addModel(candidate);
  }

  // Step 2: Fill remaining slots from roster priority order
  for (const id of candidates) {
    if (selected.length >= count) break;
    if (!selected.includes(id) && canAdd(id)) {
      addModel(id);
    }
  }

  // Step 3: Verify ≥3 vendor families
  const vendorCount = Object.keys(usedVendors).length;
  if (vendorCount < 3 && selected.length >= 3) {
    // Try to swap the last model for one from a new vendor
    for (const id of Object.keys(MODEL_REGISTRY)) {
      if (selected.includes(id) || exclude.includes(id)) continue;
      const cfg = MODEL_REGISTRY[id];
      if (!usedVendors[cfg.vendor]) {
        selected[selected.length - 1] = id;
        break;
      }
    }
  }

  return selected;
}

function selectAdversarialModels(usedModels, count = 3) {
  // MMA-07: 3 models from different vendor families, NONE used in prior steps
  const usedVendors = new Set(usedModels.map(id => MODEL_REGISTRY[id]?.vendor).filter(Boolean));
  const candidates = Object.keys(MODEL_REGISTRY)
    .filter(id => !usedModels.includes(id))
    .sort((a, b) => {
      // Prefer models from unused vendors
      const aNew = !usedVendors.has(MODEL_REGISTRY[a].vendor) ? 0 : 1;
      const bNew = !usedVendors.has(MODEL_REGISTRY[b].vendor) ? 0 : 1;
      if (aNew !== bNew) return aNew - bNew;
      // Then prefer cheaper models (Step 4 cost optimization)
      return MODEL_REGISTRY[a].priceOut - MODEL_REGISTRY[b].priceOut;
    });

  const selected = [];
  const advVendors = {};
  for (const id of candidates) {
    if (selected.length >= count) break;
    const cfg = MODEL_REGISTRY[id];
    // Enforce different vendor families for adversarial models
    if ((advVendors[cfg.vendor] || 0) >= 1) continue;
    advVendors[cfg.vendor] = 1;
    selected.push(id);
  }
  return selected;
}

// ─── Input Sanitization (MMA-17) ───────────────────────────────────────────
function sanitizeForPrompt(text) {
  if (!text) return '';
  return text
    .replace(/\x1b\[[0-9;]*m/g, '')           // Strip ANSI codes
    .replace(/sk-[a-zA-Z0-9_-]{20,}/g, '[REDACTED_KEY]')  // OpenRouter/API keys
    .replace(/Bearer\s+[a-zA-Z0-9_.-]+/g, 'Bearer [REDACTED]')
    .replace(new RegExp('pass' + 'word\\s*=\\s*"[^"]*"', 'gi'), '[REDACTED_CREDENTIAL]')
    .replace(/\/root\//g, '/[REDACTED_PATH]/');
}

// ─── File helpers ───────────────────────────────────────────────────────────
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
  return files.map(f => `--- FILE: ${f.path} ---\n${sanitizeForPrompt(f.content)}`).join('\n\n');
}

function estimateTokens(text) { return Math.ceil(text.length / 4); }

// ─── Auto-split for context limits ─────────────────────────────────────────
function splitBatchIfNeeded(batch, maxTokens) {
  const est = estimateTokens(SYSTEM_PROMPT + batch.prompt);
  const limit = Math.floor(maxTokens * 0.75);
  if (est <= limit) return [batch];

  const fileMarker = '--- FILE:';
  const parts = batch.prompt.split(fileMarker);
  const header = parts[0];
  const files = parts.slice(1);
  const mid = Math.ceil(files.length / 2);

  return [
    { name: `${batch.name}-part1`, title: `${batch.title} (Part 1/2)`, prompt: header + files.slice(0, mid).map(f => fileMarker + f).join(''), domain: batch.domain },
    { name: `${batch.name}-part2`, title: `${batch.title} (Part 2/2)`, prompt: header + files.slice(mid).map(f => fileMarker + f).join(''), domain: batch.domain },
  ];
}

// ─── OpenRouter API ────────────────────────────────────────────────────────
function callModel(modelId, systemPrompt, userPrompt, retries = 0, keyRecovered = false) {
  const config = MODEL_REGISTRY[modelId];
  if (!config) return Promise.reject(new Error(`Unknown model: ${modelId}`));

  return new Promise((resolve, reject) => {
    const body = JSON.stringify({
      model: modelId,
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
        'X-Title': `Racing Point MMA v3.0 Audit (${config.short})`
      }
    };

    const req = https.request(options, (res) => {
      let data = '';
      res.on('data', chunk => data += chunk);
      res.on('end', () => {
        try {
          const parsed = JSON.parse(data);
          if (parsed.error) {
            // 401 = dead key — attempt auto-recovery (doesn't count as retry)
            if (is401Error(parsed.error) && !keyRecovered) {
              console.log(`    [${config.short}] 401 — key is dead. Attempting auto-recovery...`);
              recoverKey().then(newKey => {
                OPENROUTER_KEY = newKey;
                callModel(modelId, systemPrompt, userPrompt, retries, true).then(resolve).catch(reject);
              }).catch(e => {
                reject(new Error(`[${config.short}] Key dead (401) and recovery failed: ${e.message}`));
              });
              return;
            }
            if (retries < MAX_RETRIES) {
              console.log(`    [${config.short}] Retry ${retries + 1}/${MAX_RETRIES}... (${parsed.error.message || 'unknown'})`);
              setTimeout(() => callModel(modelId, systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject), 5000 * (retries + 1));
              return;
            }
            reject(new Error(`[${config.short}] API error: ${JSON.stringify(parsed.error)}`));
            return;
          }
          const content = parsed.choices?.[0]?.message?.content || '';
          const usage = parsed.usage || {};
          resolve({ modelId, content, usage, finish_reason: parsed.choices?.[0]?.finish_reason });
        } catch (e) {
          reject(new Error(`[${config.short}] Parse error: ${e.message}\nRaw: ${data.slice(0, 500)}`));
        }
      });
    });

    req.on('error', (e) => {
      if (retries < MAX_RETRIES) {
        setTimeout(() => callModel(modelId, systemPrompt, userPrompt, retries + 1).then(resolve).catch(reject), 5000 * (retries + 1));
        return;
      }
      reject(e);
    });

    const timeout = Math.min(config.timeout, MODEL_TIMEOUT * 3); // MMA-16 with margin
    req.setTimeout(timeout, () => {
      req.destroy();
      reject(new Error(`[${config.short}] Timeout (${timeout / 1000}s)`));
    });

    req.write(body);
    req.end();
  });
}

// ─── Pre-flight Probes (MMA-06) ────────────────────────────────────────────
async function preflightCheck() {
  console.log('--- Pre-flight Infrastructure Probes (MMA-06) ----------------');

  // Probe 1: OpenRouter API reachable
  try {
    const result = await new Promise((resolve, reject) => {
      const req = https.request({
        hostname: 'openrouter.ai',
        path: '/api/v1/models',
        method: 'GET',
        headers: { 'Authorization': `Bearer ${OPENROUTER_KEY}` }
      }, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => resolve({ status: res.statusCode }));
      });
      req.on('error', reject);
      req.setTimeout(10000, () => { req.destroy(); reject(new Error('timeout')); });
      req.end();
    });
    if (result.status === 200) {
      console.log('  [OK] OpenRouter API reachable (200)');
    } else if (result.status === 401 || result.status === 403) {
      console.error(`  [WARN] OpenRouter API returned ${result.status} — key is dead. Attempting auto-recovery...`);
      try {
        OPENROUTER_KEY = await recoverKey();
        console.log('  [OK] Key recovered — retrying preflight...');
        // Re-check with new key
        const recheck = await new Promise((resolve, reject) => {
          const req2 = https.request({
            hostname: 'openrouter.ai', path: '/api/v1/models', method: 'GET',
            headers: { 'Authorization': `Bearer ${OPENROUTER_KEY}` }
          }, (res2) => {
            let d = ''; res2.on('data', c => d += c);
            res2.on('end', () => resolve({ status: res2.statusCode }));
          });
          req2.on('error', reject);
          req2.setTimeout(10000, () => { req2.destroy(); reject(new Error('timeout')); });
          req2.end();
        });
        if (recheck.status === 200) {
          console.log('  [OK] New key verified — preflight passed');
        } else {
          console.error(`  [FAIL] New key also returned ${recheck.status} — aborting`);
          process.exit(1);
        }
      } catch (e) {
        console.error(`  [FAIL] Key recovery failed: ${e.message}`);
        process.exit(1);
      }
    } else {
      console.log(`  [WARN] OpenRouter API returned ${result.status} — proceeding with caution`);
    }
  } catch (e) {
    console.error(`  [FAIL] OpenRouter API unreachable: ${e.message}`);
    console.error('  Cannot proceed without OpenRouter. Aborting.');
    process.exit(1);
  }

  console.log(`  [OK] Session budget: $${SESSION_BUDGET}`);
  console.log(`  [OK] Mode: ${LEGACY_MODEL ? `LEGACY (single model: ${LEGACY_MODEL})` : 'CONSENSUS v3.0 (5 models x consensus + adversarial verify)'}`);
  console.log('-------------------------------------------------------------\n');
}

// ─── System Prompts ─────────────────────────────────────────────────────────

// MMA-12: Chain-of-Thought Mandate + MMA-19: Domain Context
const SYSTEM_PROMPT = `You are an expert systems auditor performing a comprehensive audit of "Racing Point eSports" — a sim racing venue with 8 pods, a server, and cloud infrastructure.

DOMAIN: Sim racing venue management (8 gaming PCs, Rust/Axum server, Windows pods,
Conspit wheelbases, AC/F1 25/LMU/Forza/iRacing, USB HID billing, Edge kiosk)

Architecture:
- Rust/Axum monorepo (racecontrol server :8080, rc-agent on pods :8090, rc-sentry :8091, rc-watchdog service)
- Next.js apps (admin :3201, web :3200, kiosk :3300)
- Node.js comms-link (James<->Bono AI coordination, WS :8765, relay :8766)
- Bash audit/healing/detection pipeline
- Windows pods with NVIDIA Surround triple monitors, Edge kiosk, game launching

CRITICAL INSTRUCTION: Show your reasoning step by step.

Your audit must find:
1. SECURITY: credential leaks, auth gaps, injection, privilege escalation, missing validation
2. CODE QUALITY: unwrap() in Rust, "any" in TypeScript, error handling gaps, race conditions
3. RELIABILITY: single points of failure, missing retries, silent failures, crash loop risks
4. INTEGRATION: API contract mismatches, serialization gaps (serde silent drops), field name drift
5. PROCESS: standing rule violations, missing cascade updates, deploy pipeline gaps
6. INFRASTRUCTURE: config drift, stale references, missing health checks, monitoring blind spots
7. ABSENCE: What SHOULD exist but DOESN'T? Missing timeouts, missing state transitions, missing error paths
8. STATE MACHINES: can any state get stuck permanently? Missing transitions or unreachable states?
9. CROSS-SYSTEM: does this code assume something about another component that might not be true?

For each finding, output a structured block:
---FINDING---
SEVERITY: P1|P2|P3
CATEGORY: security|reliability|integration|code-quality|process|infrastructure|absence
FILE: exact file path
LINE: approximate line number
FINDING: what's wrong (one line)
REASONING: your step-by-step reasoning for why this is a problem
IMPACT: what could happen
FIX: recommended action
---END---

Be thorough. Flag EVERYTHING suspicious. Better to over-report than miss a real issue.
Pay special attention to things that are MISSING, not just things that are wrong.`;

const ADVERSARIAL_SYSTEM_PROMPT = `You are a senior security/reliability auditor performing adversarial verification of audit findings.

TASK — STEP 4: VERIFY (Unified MMA Protocol v3.0)
You are reviewing findings that 3-5 AI models agreed on. Your job is to CHALLENGE them.

For EACH finding, evaluate on this rubric:
- Root Cause Accuracy (35%): Did they identify the actual cause, not a symptom?
- Fix Completeness (25%): Does the proposed fix handle all variants?
- Verification Evidence (25%): Is there concrete proof this is a real issue?
- Side Effect Safety (15%): Could the proposed fix break anything else?

Score each finding 1-5. Score >= 4.0 = CONFIRMED. Score 3.0-3.9 = FLAG. Score < 3.0 = REJECTED.

Also identify: any findings the original audit MISSED that you can see.

Output format:
---VERDICT---
FINDING_ID: (sequential number)
ORIGINAL: (one-line summary of the finding)
SCORE: X.X
VERDICT: CONFIRMED|FLAG|REJECTED
REASON: (why you scored it this way)
---END---

At the end, add any new findings you discovered.`;

// ─── Consensus Engine ───────────────────────────────────────────────────────
function parseFindings(responseContent) {
  const findings = [];
  const blocks = responseContent.split('---FINDING---');
  for (let i = 1; i < blocks.length; i++) {
    const block = blocks[i].split('---END---')[0];
    const finding = {};
    for (const line of block.split('\n')) {
      const match = line.match(/^(SEVERITY|CATEGORY|FILE|LINE|FINDING|REASONING|IMPACT|FIX):\s*(.+)/);
      if (match) finding[match[1].toLowerCase()] = match[2].trim();
    }
    if (finding.finding && finding.severity) {
      findings.push(finding);
    }
  }

  // Fallback: if no structured findings, try to extract from freeform text
  if (findings.length === 0 && responseContent.length > 100) {
    const pMatches = responseContent.match(/\bP[123]\b[^]*?(?=\bP[123]\b|$)/g) || [];
    for (const m of pMatches.slice(0, 30)) {
      findings.push({
        severity: m.match(/\b(P[123])\b/)?.[1] || 'P3',
        category: 'unstructured',
        finding: m.slice(0, 200).trim(),
        file: 'unknown',
        impact: '',
        fix: '',
        reasoning: 'Extracted from freeform response',
      });
    }
  }

  return findings;
}

function computeFindingKey(finding) {
  // Semantic dedup key: category + file + first 50 chars of finding
  const file = (finding.file || 'unknown').replace(/\\/g, '/').split('/').pop();
  const desc = (finding.finding || '').toLowerCase().replace(/[^a-z0-9]/g, '').slice(0, 50);
  return `${finding.category || 'unknown'}:${file}:${desc}`;
}

function buildConsensus(allModelFindings, modelIds) {
  // allModelFindings: array of { modelId, findings: [...] }
  // Returns: { majority: [...], dissenting: [...], raw: [...] }

  const findingMap = new Map(); // key -> { finding, models: Set }

  for (const { modelId, findings } of allModelFindings) {
    for (const finding of findings) {
      const key = computeFindingKey(finding);
      if (!findingMap.has(key)) {
        findingMap.set(key, { finding, models: new Set(), key });
      }
      findingMap.get(key).models.add(modelId);
    }
  }

  const totalModels = modelIds.length;
  const majorityThreshold = Math.ceil(totalModels * 0.6); // 3/5

  const majority = [];
  const dissenting = [];

  for (const [, entry] of findingMap) {
    const agreement = entry.models.size / totalModels;
    const enriched = {
      ...entry.finding,
      agreement_score: agreement,
      models_agreed: Array.from(entry.models).map(id => MODEL_REGISTRY[id]?.short || id),
      consensus: entry.models.size >= majorityThreshold ? 'majority' : 'minority',
    };

    if (entry.models.size >= majorityThreshold) {
      majority.push(enriched);
    } else {
      dissenting.push(enriched);
    }
  }

  // Sort by severity then agreement
  const sevOrder = { P1: 0, P2: 1, P3: 2 };
  majority.sort((a, b) => (sevOrder[a.severity] || 3) - (sevOrder[b.severity] || 3) || b.agreement_score - a.agreement_score);
  dissenting.sort((a, b) => b.agreement_score - a.agreement_score);

  return { majority, dissenting, totalFindings: findingMap.size, totalModels };
}

// ─── Pre-scan freshness check ───────────────────────────────────────────────
function checkCodebaseFreshness() {
  try {
    const gitOpts = { cwd: REPO_ROOT, encoding: 'utf-8' };
    const headHash = execSync('git rev-parse --short HEAD', gitOpts).trim();
    const headMsg = execSync('git log -1 --format=%s', gitOpts).trim();
    const headTime = execSync('git log -1 --format=%ci', gitOpts).trim();
    const dateStr = new Date().toISOString().split('T')[0];

    const auditDirs = 'crates/ scripts/ audit/lib/ audit/phases/ web/ kiosk/ admin/';
    const dirtyFiles = execSync(`git diff --name-only HEAD -- ${auditDirs}`, gitOpts).trim();
    const stagedFiles = execSync(`git diff --cached --name-only -- ${auditDirs}`, gitOpts).trim();
    const hasUncommitted = dirtyFiles.length > 0 || stagedFiles.length > 0;

    const resultsDir = path.join(REPO_ROOT, 'audit', 'results');
    const todayResults = fs.existsSync(resultsDir)
      ? fs.readdirSync(resultsDir).filter(d => d.endsWith(`-audit-${dateStr}`) && !d.startsWith('consensus'))
      : [];

    console.log('--- Pre-Scan Freshness Check --------------------------------');
    console.log(`  HEAD: ${headHash} -- "${headMsg}"`);
    console.log(`  Time: ${headTime}`);

    if (hasUncommitted) {
      const dirtyList = dirtyFiles ? dirtyFiles.split('\n') : [];
      const stagedList = stagedFiles ? stagedFiles.split('\n') : [];
      console.log(`  WARNING: ${dirtyList.length + stagedList.length} uncommitted changes`);
      console.log('  -> Models will audit WORKING TREE (includes uncommitted fixes)');
    } else {
      console.log('  OK: Working tree clean -- models will audit commit ' + headHash);
    }

    if (todayResults.length > 0 && !AUDIT_ALLOW_STALE) {
      const earliestResult = todayResults.sort()[0];
      const resultDir = path.join(resultsDir, earliestResult);
      const resultTime = fs.statSync(resultDir).mtime;
      const commitsSinceStr = execSync(
        `git log --oneline --since="${resultTime.toISOString()}" -- crates/ scripts/ audit/lib/ web/ kiosk/`,
        gitOpts
      ).trim();
      const commitsSince = commitsSinceStr ? commitsSinceStr.split('\n').length : 0;

      if (commitsSince === 0) {
        console.log(`  Prior rounds today: ${todayResults.length}`);
        console.log('  WARNING: No fix commits since prior round(s)');
        console.log('  BLOCKED: Set AUDIT_ALLOW_STALE=1 to override');
        console.log('-------------------------------------------------------------\n');
        process.exit(2);
      }
      console.log(`  OK: ${commitsSince} fix commit(s) since prior round -- codebase hardened`);
    }

    console.log('-------------------------------------------------------------\n');
    return headHash;
  } catch (e) {
    console.log('  Freshness check skipped (not a git repo or git unavailable)');
    return 'unknown';
  }
}

// ─── Audit Batches (same structure as v1.0, now with domain tags) ───────────
function prepareBatches() {
  const batches = [];

  // Batch 1: Core Rust — racecontrol server
  const serverRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'racecontrol', 'src'), ['.rs']);
  batches.push({
    name: '01-server-rust', title: 'Racecontrol Server (Rust/Axum)', domain: 'rust_backend',
    prompt: `Audit the racecontrol server — the central Rust/Axum service running on :8080.
Focus on: route auth coverage, SQL injection, error handling (.unwrap()), WebSocket security, fleet exec safety, billing logic, game state management.
Also check: missing timeouts on state transitions (GameTracker stuck states), missing DB transactions on financial ops, serde silent field drops.

${bundleFiles(serverRs)}`
  });

  // Batch 2: rc-agent (pod agent)
  const agentRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-agent', 'src'), ['.rs']);
  batches.push({
    name: '02-agent-rust', title: 'RC-Agent Pod Agent (Rust)', domain: 'rust_backend',
    prompt: `Audit rc-agent — runs on each of 8 Windows pods (:8090). Handles game launching, lock screen, process guard, health reporting, remote exec.
Focus on: command injection via exec endpoint, process guard bypass, game launch security, self-restart safety, Session 0 vs Session 1 issues.
Also check: does agent detect Session 0 vs Session 1? Can lock screen get stuck? MAINTENANCE_MODE sentinel TTL?

${bundleFiles(agentRs)}`
  });

  // Batch 3: rc-sentry + rc-watchdog + rc-common + process guard
  const sentryRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-sentry', 'src'), ['.rs']);
  const watchdogRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-watchdog', 'src'), ['.rs']);
  const commonRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-common', 'src'), ['.rs']);
  const guardRs = readFilesFromDir(path.join(REPO_ROOT, 'crates', 'rc-process-guard', 'src'), ['.rs']);
  batches.push({
    name: '03-sentry-watchdog-common', title: 'RC-Sentry, RC-Watchdog, RC-Common, Process Guard (Rust)', domain: 'windows_os',
    prompt: `Audit supporting Rust crates:
- rc-sentry (:8091): pod watchdog, restart logic, schtasks integration
- rc-watchdog: Windows service for Session 1 process recovery (WTSQueryUserToken)
- rc-common: shared types, boot resilience, config
- rc-process-guard: allowlist enforcement, violation tracking

Focus on: restart loop safety, MAINTENANCE_MODE handling, Session 0/1 correctness, allowlist bypass, type mismatches between crates.
Also check: can recovery systems fight each other (sentry vs watchdog vs WoL)?

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
  batches.push({
    name: '04-comms-link', title: 'Comms-Link (Node.js — James<->Bono coordination)', domain: 'nodejs_frontend',
    prompt: `Audit comms-link — WebSocket-based coordination between James (on-site AI) and Bono (VPS AI).
Focus on: PSK authentication strength, exec command injection, shell relay safety, dynamic registry abuse, message tampering, chain orchestration race conditions.

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
  batches.push({
    name: '05-audit-detection-healing', title: 'Audit Pipeline, Detectors, Healing Engine (Bash)', domain: 'sre_ops',
    prompt: `Audit the autonomous detection and healing pipeline:
- audit.sh + lib/*.sh: 60-phase audit runner with parallel execution
- detectors/*.sh: crash loop, config drift, log anomaly, schema gap, bat drift, flag desync
- healing/escalation-engine.sh: 5-tier graduated escalation
- auto-detect.sh: orchestrator that runs detectors and feeds findings to healing

Focus on: race conditions, sentinel file handling, billing gate bypass, escalation loops, notification flooding, command injection.

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
  batches.push({
    name: '06-deploy-infra', title: 'Deploy Pipeline, Configs, Infrastructure', domain: 'sre_ops',
    prompt: `Audit deploy pipeline and infrastructure:
- stage-release.sh: security pre-flight -> build -> SHA256 -> manifest
- deploy-pod.sh / deploy-server.sh: binary deployment with security gates
- Cargo.toml / .cargo/config.toml: workspace + build config

Focus on: deploy integrity, binary verification, rollback safety, dependency vulns, build reproducibility, manifest tampering.

${bundleFiles([...deployScripts, ...rootConfigs])}`
  });

  // Batch 7: Standing rules + cross-system
  const claudeMd = readFile(path.join(REPO_ROOT, 'CLAUDE.md')) || '';
  const commsClaude = readFile(path.join(COMMS_ROOT, 'CLAUDE.md')) || '';
  batches.push({
    name: '07-standing-rules-crosssystem', title: 'Standing Rules, Cross-System Integration', domain: 'cross_system',
    prompt: `Review standing rules and cross-system integration:

RACECONTROL CLAUDE.md:
${sanitizeForPrompt(claudeMd)}

COMMS-LINK CLAUDE.md:
${sanitizeForPrompt(commsClaude)}

Audit for: rule conflicts, coverage gaps, cross-system data mismatches, process gaps, stale references, security blind spots.
Be specific — reference exact rule text when flagging issues.`
  });

  // Batch 8: Frontend (Next.js/TypeScript)
  const kioskSrc = readFilesFromDir(path.join(REPO_ROOT, 'kiosk', 'src'), ['.ts', '.tsx']);
  const webSrc = readFilesFromDir(path.join(REPO_ROOT, 'web', 'src'), ['.ts', '.tsx']);
  const adminSrc = readFilesFromDir(path.join(REPO_ROOT, 'admin', 'src'), ['.ts', '.tsx']);
  batches.push({
    name: '08-frontend-nextjs', title: 'Frontend Apps (Next.js — Kiosk, Web, Admin)', domain: 'nodejs_frontend',
    prompt: `Audit three Next.js apps:
- kiosk (:3300): customer-facing pod control, game wizard, billing display
- web (:3200): staff dashboard, fleet overview, billing, leaderboards
- admin (:3201): admin panel, fleet mgmt, feature flags, health

Focus on: XSS, auth token handling, CORS, NEXT_PUBLIC_ leaks, SSR/CSR hydration, unsafe HTML, API URL injection, WS security, error boundaries.

${bundleFiles([...kioskSrc, ...webSrc, ...adminSrc])}`
  });

  return batches;
}

// ─── Main Audit Flow ────────────────────────────────────────────────────────
async function runAudit() {
  const dateStr = new Date().toISOString().split('T')[0];
  const isConsensus = !LEGACY_MODEL;
  const outputDirName = isConsensus ? `consensus-audit-${dateStr}` : `${MODEL_REGISTRY[LEGACY_MODEL]?.short || 'unknown'}-audit-${dateStr}`;
  const OUTPUT_DIR = path.join(REPO_ROOT, 'audit', 'results', outputDirName);
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });

  console.log(`=== Racing Point MMA v3.0 Audit ===`);
  console.log(`Mode: ${isConsensus ? 'CONSENSUS (5 models per batch + adversarial verify)' : `LEGACY (${LEGACY_MODEL})`}`);
  console.log(`Budget: $${SESSION_BUDGET} | Output: ${OUTPUT_DIR}\n`);

  await preflightCheck();
  if (DRY_RUN) console.log('*** DRY RUN MODE — no API calls ***\n');

  const auditedCommit = checkCodebaseFreshness();

  const rawBatches = prepareBatches();

  // Auto-split batches that exceed the smallest model's context
  const minCtx = isConsensus
    ? Math.min(...Object.values(MODEL_REGISTRY).map(c => c.ctx))
    : (MODEL_REGISTRY[LEGACY_MODEL]?.ctx || 131072);
  let batches = [];
  for (const batch of rawBatches) {
    batches.push(...splitBatchIfNeeded(batch, minCtx));
  }

  console.log(`Prepared ${batches.length} audit batches (${rawBatches.length} original)\n`);

  const allUsedModels = new Set();
  const allConsensusResults = [];

  // == Phase 1: DIAGNOSE — run models per batch, build consensus ==
  for (let i = 0; i < batches.length; i++) {
    const batch = batches[i];
    const batchStart = Date.now();

    if (!budgetTracker.checkBudget()) {
      console.error(`  Stopping at batch ${i + 1} — budget exhausted`);
      break;
    }

    console.log(`[${i + 1}/${batches.length}] ${batch.title}`);

    if (isConsensus) {
      // Select 5 domain-appropriate models
      const domain = process.env.AUDIT_DOMAIN || batch.domain || 'cross_system';
      const models = selectModels(domain, 5);
      const vendorFamilies = new Set(models.map(id => MODEL_REGISTRY[id].vendor));

      console.log(`  Domain: ${domain} | Models: ${models.map(id => MODEL_REGISTRY[id].short).join(', ')}`);
      console.log(`  Vendors: ${Array.from(vendorFamilies).join(', ')} (${vendorFamilies.size} families)`);
      models.forEach(id => allUsedModels.add(id));

      if (DRY_RUN) {
        console.log('  [DRY RUN] Skipping API calls\n');
        allConsensusResults.push({ batch: batch.title, majority: [], dissenting: [], domain });
        continue;
      }

      // Run all 5 models concurrently (Promise.allSettled for resilience)
      const results = await Promise.allSettled(
        models.map(modelId => callModel(modelId, SYSTEM_PROMPT, batch.prompt))
      );

      const modelFindings = [];
      for (let j = 0; j < results.length; j++) {
        const result = results[j];
        const modelId = models[j];
        const shortName = MODEL_REGISTRY[modelId].short;

        if (result.status === 'fulfilled') {
          const { content, usage } = result.value;
          const inputTokens = usage.prompt_tokens || estimateTokens(SYSTEM_PROMPT + batch.prompt);
          const outputTokens = usage.completion_tokens || 0;
          const cost = budgetTracker.track(modelId, inputTokens, outputTokens);

          const findings = parseFindings(content);
          modelFindings.push({ modelId, findings });
          console.log(`    [${shortName}] ${findings.length} findings | ${inputTokens}in/${outputTokens}out | $${cost.toFixed(4)}`);

          // Save raw model output (MMA-18: provenance)
          fs.writeFileSync(
            path.join(OUTPUT_DIR, `${batch.name}_${shortName}.md`),
            `# ${batch.title} — ${shortName}\n\n**Model:** ${modelId}\n**Tokens:** ${inputTokens}/${outputTokens}\n**Cost:** $${cost.toFixed(4)}\n\n---\n\n${content}\n`
          );
        } else {
          console.log(`    [${shortName}] FAILED: ${result.reason?.message?.slice(0, 100)}`);
          // MMA-16: model timeout -> skip, proceed with remaining
        }
      }

      // Build consensus
      const consensus = buildConsensus(modelFindings, models);
      console.log(`  Consensus: ${consensus.majority.length} majority (3/5+) | ${consensus.dissenting.length} minority`);
      allConsensusResults.push({ batch: batch.title, ...consensus, domain });

      // Save consensus report for this batch
      fs.writeFileSync(
        path.join(OUTPUT_DIR, `${batch.name}_consensus.json`),
        JSON.stringify({ batch: batch.title, domain, models: models.map(id => MODEL_REGISTRY[id].short), ...consensus }, null, 2)
      );

    } else {
      // Legacy single-model mode
      if (DRY_RUN) {
        console.log('  [DRY RUN] Skipping API call\n');
        continue;
      }

      try {
        const result = await callModel(LEGACY_MODEL, SYSTEM_PROMPT, batch.prompt);
        const inputTokens = result.usage.prompt_tokens || estimateTokens(SYSTEM_PROMPT + batch.prompt);
        const outputTokens = result.usage.completion_tokens || 0;
        const cost = budgetTracker.track(LEGACY_MODEL, inputTokens, outputTokens);
        console.log(`  ${inputTokens}in/${outputTokens}out | $${cost.toFixed(4)}`);

        fs.writeFileSync(
          path.join(OUTPUT_DIR, `${batch.name}.md`),
          `# ${batch.title}\n\n**Model:** ${LEGACY_MODEL}\n**Tokens:** ${inputTokens}/${outputTokens}\n**Cost:** $${cost.toFixed(4)}\n\n---\n\n${result.content}\n`
        );
      } catch (err) {
        console.error(`  ERROR: ${err.message}`);
        fs.writeFileSync(path.join(OUTPUT_DIR, `${batch.name}.md`), `# ${batch.title}\n\n**ERROR:** ${err.message}\n`);
      }
    }

    const elapsed = ((Date.now() - batchStart) / 1000).toFixed(1);
    console.log(`  Elapsed: ${elapsed}s | Budget remaining: $${budgetTracker.remaining().toFixed(4)}\n`);
  }

  // == Phase 2: ADVERSARIAL VERIFY (consensus mode only) ==
  let verificationResult = null;
  if (isConsensus && !DRY_RUN) {
    const allMajority = allConsensusResults.flatMap(r => r.majority || []);
    if (allMajority.length > 0) {
      console.log(`=== Step 4: Adversarial Verification (${allMajority.length} consensus findings) ===`);

      const advModels = selectAdversarialModels(Array.from(allUsedModels), 3);
      console.log(`  Adversarial models: ${advModels.map(id => MODEL_REGISTRY[id].short).join(', ')}`);

      // Build verification prompt from consensus findings
      const findingsSummary = allMajority.map((f, idx) =>
        `${idx + 1}. [${f.severity}] ${f.category}: ${f.finding} (File: ${f.file || 'unknown'}, Agreement: ${(f.agreement_score * 100).toFixed(0)}%)`
      ).join('\n');

      const verifyPrompt = `Review these ${allMajority.length} consensus findings from a 5-model MMA audit of Racing Point eSports infrastructure:\n\n${findingsSummary}\n\nChallenge each finding. Score 1-5 on the rubric. Identify any MISSED findings.`;

      if (budgetTracker.checkBudget()) {
        const advResults = await Promise.allSettled(
          advModels.map(id => callModel(id, ADVERSARIAL_SYSTEM_PROMPT, verifyPrompt))
        );

        const verdicts = [];
        for (let j = 0; j < advResults.length; j++) {
          const result = advResults[j];
          const modelId = advModels[j];
          const shortName = MODEL_REGISTRY[modelId].short;

          if (result.status === 'fulfilled') {
            const { content, usage } = result.value;
            const cost = budgetTracker.track(modelId, usage.prompt_tokens || 0, usage.completion_tokens || 0);
            console.log(`    [${shortName}] Verified | $${cost.toFixed(4)}`);
            verdicts.push({ model: shortName, content });

            fs.writeFileSync(
              path.join(OUTPUT_DIR, `adversarial_${shortName}.md`),
              `# Adversarial Verification — ${shortName}\n\n${content}\n`
            );
          } else {
            console.log(`    [${shortName}] FAILED: ${result.reason?.message?.slice(0, 80)}`);
          }
        }

        verificationResult = { models: advModels.map(id => MODEL_REGISTRY[id].short), verdicts };
        console.log(`  Adversarial verify complete: ${verdicts.length}/${advModels.length} models responded\n`);
      } else {
        console.log('  Skipping adversarial verify — budget exhausted\n');
      }
    } else {
      console.log('  No consensus findings to verify.\n');
    }
  }

  // == Generate Combined Report ==
  const budget = budgetTracker.summary();
  let report = `# Racing Point MMA v3.0 Full System Audit\n\n`;
  report += `**Date:** ${new Date().toISOString()}\n`;
  report += `**Mode:** ${isConsensus ? 'Consensus (5 models x 8 batches + adversarial verify)' : `Legacy (${LEGACY_MODEL})`}\n`;
  report += `**Audited Commit:** ${auditedCommit}\n`;
  report += `**Protocol:** Unified MMA Protocol v3.0\n`;
  report += `**Total Cost:** ${budget.total_cost}\n`;
  report += `**Budget:** ${budget.budget} (${budget.remaining} remaining)\n`;
  report += `**API Calls:** ${budget.total_calls}\n`;
  if (isConsensus) {
    report += `**Models Used (diagnose):** ${Array.from(allUsedModels).map(id => MODEL_REGISTRY[id]?.short).join(', ')}\n`;
    if (verificationResult) {
      report += `**Adversarial Verify:** ${verificationResult.models.join(', ')}\n`;
    }
  }
  report += `\n---\n\n`;

  if (isConsensus) {
    // Consensus summary
    const allMajority = allConsensusResults.flatMap(r => r.majority || []);
    const allDissenting = allConsensusResults.flatMap(r => r.dissenting || []);

    report += `## Consensus Summary\n\n`;
    report += `| Metric | Value |\n|--------|-------|\n`;
    report += `| Majority findings (3/5+) | ${allMajority.length} |\n`;
    report += `| Minority findings | ${allDissenting.length} |\n`;
    report += `| P1 (critical) | ${allMajority.filter(f => f.severity === 'P1').length} |\n`;
    report += `| P2 (reliability) | ${allMajority.filter(f => f.severity === 'P2').length} |\n`;
    report += `| P3 (quality) | ${allMajority.filter(f => f.severity === 'P3').length} |\n\n`;

    report += `## Consensus Findings (3/5 majority)\n\n`;
    for (const f of allMajority) {
      report += `### [${f.severity}] ${f.category}: ${f.finding}\n`;
      report += `- **File:** ${f.file || 'unknown'}${f.line ? ` (line ~${f.line})` : ''}\n`;
      report += `- **Agreement:** ${(f.agreement_score * 100).toFixed(0)}% (${f.models_agreed.join(', ')})\n`;
      report += `- **Impact:** ${f.impact || 'N/A'}\n`;
      report += `- **Fix:** ${f.fix || 'N/A'}\n\n`;
    }

    if (allDissenting.length > 0) {
      report += `## Dissenting Opinions (minority — preserved per MMA-07)\n\n`;
      for (const f of allDissenting.slice(0, 20)) {
        report += `- [${f.severity}] ${f.finding} (${(f.agreement_score * 100).toFixed(0)}% — ${f.models_agreed.join(', ')})\n`;
      }
      report += '\n';
    }

    if (verificationResult) {
      report += `## Adversarial Verification (Step 4)\n\n`;
      report += `Models: ${verificationResult.models.join(', ')}\n\n`;
      for (const v of verificationResult.verdicts) {
        report += `### ${v.model}\n\n${v.content.slice(0, 2000)}\n\n`;
      }
    }
  }

  report += `## Cost Breakdown\n\n`;
  report += `| Model | Cost |\n|-------|------|\n`;
  for (const [model, cost] of Object.entries(budget.per_model)) {
    report += `| ${model} | $${cost.toFixed(4)} |\n`;
  }
  report += `| **Total** | **${budget.total_cost}** |\n`;

  const reportPath = path.join(OUTPUT_DIR, 'FULL-AUDIT-REPORT.md');
  fs.writeFileSync(reportPath, report);

  // Save provenance metadata (MMA-18)
  fs.writeFileSync(path.join(OUTPUT_DIR, '_provenance.json'), JSON.stringify({
    protocol_version: '3.0',
    mode: isConsensus ? 'consensus' : 'legacy',
    audited_commit: auditedCommit,
    timestamp: new Date().toISOString(),
    models_used: Array.from(allUsedModels).map(id => ({ id, short: MODEL_REGISTRY[id]?.short, vendor: MODEL_REGISTRY[id]?.vendor })),
    adversarial_models: verificationResult?.models || [],
    budget: budget,
    batches: batches.length,
    consensus_findings: isConsensus ? allConsensusResults.reduce((s, r) => s + (r.majority?.length || 0), 0) : null,
  }, null, 2));

  // Save freshness metadata
  fs.writeFileSync(path.join(OUTPUT_DIR, '_freshness.json'), JSON.stringify({
    head_hash: auditedCommit,
    scan_time: new Date().toISOString(),
    mode: isConsensus ? 'consensus_v3' : 'legacy',
  }, null, 2));

  console.log('=== AUDIT COMPLETE ===');
  console.log(`Mode: ${isConsensus ? 'CONSENSUS v3.0' : 'LEGACY'}`);
  console.log(`Total calls: ${budget.total_calls}`);
  console.log(`Total cost: ${budget.total_cost}`);
  console.log(`Budget remaining: ${budget.remaining}`);
  console.log(`Report: ${reportPath}`);
}

runAudit().catch(err => {
  console.error('Fatal error:', err.message);
  process.exit(1);
});
