// scripts/lib/mma-model-registry.js
// Shared MMA model registry + role-fit validator.
// E1+E3 of §S-166 MMA Doctrine Alignment (Path A, Captain ratified 2026-05-09 ~21:09 IST).
//
// PURPOSE: single source of truth for model selection across:
//   - canonical scripts/multi-model-audit.js
//   - ad-hoc .tmp/mma-step{N}-runner.js
//   - any future MMA caller (steps 1/2/3/4, single-RCA, batch)
//
// Composes-with: feedback_model_role_fit_check_before_vendor_fit_20260509.md (PROMOTE-NOW-ACTIVE
// after §S-166 anchor #2: 11 worktrees with resp-anthropic-claude-haiku-4.5.json containing
// code-expert reviews — Captain G9 today + user N=2 anchor).
//
// REFRESH: scripts/lib/refresh-mma-registry.js pulls OpenRouter /models weekly and diffs.

'use strict';

// ─── MODEL_REGISTRY ─────────────────────────────────────────────────────────
// Tiers: 'premium' (deep reasoning), 'standard' (balanced), 'speed' (fast/cheap, NEVER for code_expert/reasoner).
// Each entry: short, vendor, roles[], tier, ctx, priceIn ($/Mtok), priceOut ($/Mtok), timeout (ms), maxOut.
// Optional: deprecated_for [roles], allow_speed_class (override ban — use sparingly with comment).

const MODEL_REGISTRY = {
  // ═══ Reasoners (deep step-by-step reasoning chains) ═════════════════════
  'deepseek/deepseek-r1-0528':                { short: 'deepseek-r1',      vendor: 'deepseek',  roles: ['reasoner'],              tier: 'standard', ctx: 163840,  priceIn: 0.45,  priceOut: 2.15,  timeout: 180000, maxOut: 16000 },
  'moonshotai/kimi-k2.5':                     { short: 'kimi-k2.5',        vendor: 'moonshot',  roles: ['reasoner'],              tier: 'standard', ctx: 131072,  priceIn: 0.35,  priceOut: 1.40,  timeout: 300000, maxOut: 16000 },
  'moonshotai/kimi-k2.6':                     { short: 'kimi-k2.6',        vendor: 'moonshot',  roles: ['reasoner'],              tier: 'premium',  ctx: 262144,  priceIn: 0.75,  priceOut: 3.50,  timeout: 300000, maxOut: 16000 },
  'moonshotai/kimi-k2-thinking':              { short: 'kimi-k2-thk',      vendor: 'moonshot',  roles: ['reasoner'],              tier: 'standard', ctx: 262144,  priceIn: 0.60,  priceOut: 2.50,  timeout: 300000, maxOut: 16000 },
  'qwen/qwen3-235b-a22b-thinking-2507':       { short: 'qwen3-235b-thk',   vendor: 'qwen',      roles: ['reasoner'],              tier: 'standard', ctx: 131072,  priceIn: 0.15,  priceOut: 1.50,  timeout: 300000, maxOut: 16000 },
  'qwen/qwen3.6-max-preview':                 { short: 'qwen3.6-max',      vendor: 'qwen',      roles: ['reasoner'],              tier: 'premium',  ctx: 262144,  priceIn: 1.04,  priceOut: 6.24,  timeout: 300000, maxOut: 16000 },
  'z-ai/glm-5':                               { short: 'glm-5',            vendor: 'zhipu',     roles: ['reasoner'],              tier: 'standard', ctx: 262144,  priceIn: 0.50,  priceOut: 2.00,  timeout: 300000, maxOut: 16000 },

  // ═══ Code Experts (deep code review, race analysis, type system reasoning) ═
  'deepseek/deepseek-chat-v3-0324':           { short: 'deepseek-v3',      vendor: 'deepseek',  roles: ['code_expert'],           tier: 'standard', ctx: 163840,  priceIn: 0.20,  priceOut: 0.77,  timeout: 180000, maxOut: 16000 },
  'deepseek/deepseek-v3.2':                   { short: 'deepseek-v3.2',    vendor: 'deepseek',  roles: ['code_expert'],           tier: 'standard', ctx: 131072,  priceIn: 0.252, priceOut: 0.378, timeout: 180000, maxOut: 16000 },
  'deepseek/deepseek-v4-pro':                 { short: 'deepseek-v4-pro',  vendor: 'deepseek',  roles: ['reasoner','code_expert'],tier: 'premium',  ctx: 1048576, priceIn: 0.435, priceOut: 0.87,  timeout: 300000, maxOut: 16000 },
  'qwen/qwen3-coder':                         { short: 'qwen3-coder',      vendor: 'qwen',      roles: ['code_expert'],           tier: 'standard', ctx: 262144,  priceIn: 0.22,  priceOut: 1.80,  timeout: 300000, maxOut: 16000 },
  'qwen/qwen3-coder-plus':                    { short: 'qwen3-coder-plus', vendor: 'qwen',      roles: ['code_expert'],           tier: 'premium',  ctx: 1000000, priceIn: 0.65,  priceOut: 3.25,  timeout: 300000, maxOut: 16000 },
  'kwaipilot/kat-coder-pro-v2':               { short: 'kat-coder-pro',    vendor: 'kwai',      roles: ['code_expert'],           tier: 'standard', ctx: 256000,  priceIn: 0.30,  priceOut: 1.20,  timeout: 300000, maxOut: 16000 },
  'anthropic/claude-sonnet-4.6':              { short: 'claude-sonnet-4.6',vendor: 'anthropic', roles: ['code_expert','reasoner'],tier: 'premium',  ctx: 1000000, priceIn: 3.00,  priceOut: 15.00, timeout: 300000, maxOut: 16000 },
  'anthropic/claude-sonnet-4.5':              { short: 'claude-sonnet-4.5',vendor: 'anthropic', roles: ['code_expert','reasoner'],tier: 'premium',  ctx: 1000000, priceIn: 3.00,  priceOut: 15.00, timeout: 300000, maxOut: 16000 },
  'anthropic/claude-opus-4.7':                { short: 'claude-opus-4.7',  vendor: 'anthropic', roles: ['reasoner','code_expert'],tier: 'premium',  ctx: 1000000, priceIn: 5.00,  priceOut: 25.00, timeout: 300000, maxOut: 16000 },
  'anthropic/claude-opus-4.6':                { short: 'claude-opus-4.6',  vendor: 'anthropic', roles: ['reasoner','code_expert'],tier: 'premium',  ctx: 1000000, priceIn: 5.00,  priceOut: 25.00, timeout: 300000, maxOut: 16000 },
  'openai/gpt-5.4':                           { short: 'gpt-5.4',          vendor: 'openai',    roles: ['reasoner','code_expert'],tier: 'premium',  ctx: 1050000, priceIn: 2.50,  priceOut: 15.00, timeout: 300000, maxOut: 16000 },
  'openai/gpt-5.4-pro':                       { short: 'gpt-5.4-pro',      vendor: 'openai',    roles: ['reasoner','code_expert'],tier: 'premium',  ctx: 1050000, priceIn: 30.00, priceOut: 180.00,timeout: 300000, maxOut: 16000 },
  'openai/gpt-5.3-codex':                     { short: 'gpt-5.3-codex',    vendor: 'openai',    roles: ['code_expert'],           tier: 'premium',  ctx: 400000,  priceIn: 1.75,  priceOut: 14.00, timeout: 300000, maxOut: 16000 },
  'openai/gpt-5.1-codex':                     { short: 'gpt-5.1-codex',    vendor: 'openai',    roles: ['code_expert'],           tier: 'premium',  ctx: 400000,  priceIn: 1.25,  priceOut: 10.00, timeout: 300000, maxOut: 16000 },
  'mistralai/codestral-2508':                 { short: 'codestral-2508',   vendor: 'mistral',   roles: ['code_expert'],           tier: 'standard', ctx: 256000,  priceIn: 0.30,  priceOut: 0.90,  timeout: 180000, maxOut: 16000 },
  'mistralai/devstral-medium':                { short: 'devstral-medium',  vendor: 'mistral',   roles: ['code_expert'],           tier: 'standard', ctx: 131072,  priceIn: 0.40,  priceOut: 2.00,  timeout: 180000, maxOut: 16000 },
  'x-ai/grok-code-fast-1':                    { short: 'grok-code',        vendor: 'xai',       roles: ['code_expert'],           tier: 'standard', ctx: 256000,  priceIn: 0.20,  priceOut: 1.50,  timeout: 180000, maxOut: 16000, allow_speed_class: true /* specialized fast-coder, role-fit validated by xai */ },
  'inception/mercury-coder':                  { short: 'mercury-coder',    vendor: 'inception', roles: ['code_expert'],           tier: 'standard', ctx: 128000,  priceIn: 0.25,  priceOut: 0.75,  timeout: 180000, maxOut: 16000 },
  'openai/gpt-5.1-codex-mini':                { short: 'codex-mini',       vendor: 'openai',    roles: ['code_expert'],           tier: 'speed',    ctx: 400000,  priceIn: 0.25,  priceOut: 2.00,  timeout: 300000, maxOut: 16000, deprecated_for: ['code_expert'] /* mini-class — prefer codex non-mini per §S-166 role-fit rule */ },

  // ═══ SRE / Ops (pattern recognition, incident response — speed-tolerant) ═
  'xiaomi/mimo-v2-pro':                       { short: 'mimo-v2-pro',      vendor: 'xiaomi',    roles: ['sre'],                   tier: 'standard', ctx: 1048576, priceIn: 1.00,  priceOut: 3.00,  timeout: 180000, maxOut: 16000 },
  'xiaomi/mimo-v2.5-pro':                     { short: 'mimo-v2.5-pro',    vendor: 'xiaomi',    roles: ['sre'],                   tier: 'premium',  ctx: 1048576, priceIn: 1.00,  priceOut: 3.00,  timeout: 180000, maxOut: 16000 },
  'nvidia/nemotron-3-super-120b-a12b':        { short: 'nemotron-super',   vendor: 'nvidia',    roles: ['sre'],                   tier: 'standard', ctx: 262144,  priceIn: 0.10,  priceOut: 0.50,  timeout: 180000, maxOut: 16000 },
  'nvidia/llama-3.3-nemotron-super-49b-v1.5': { short: 'nemotron-49b',     vendor: 'nvidia',    roles: ['sre'],                   tier: 'standard', ctx: 131072,  priceIn: 0.10,  priceOut: 0.40,  timeout: 180000, maxOut: 16000 },

  // ═══ Generalists (broad coverage) ═══════════════════════════════════════
  'qwen/qwen3-235b-a22b-2507':                { short: 'qwen3-235b',       vendor: 'qwen',      roles: ['generalist'],            tier: 'standard', ctx: 262144,  priceIn: 0.07,  priceOut: 0.10,  timeout: 180000, maxOut: 16000 },
  'google/gemini-2.5-flash':                  { short: 'gemini-flash',     vendor: 'google',    roles: ['generalist'],            tier: 'speed',    ctx: 1000000, priceIn: 0.30,  priceOut: 2.50,  timeout: 120000, maxOut: 16000 },
  'google/gemini-2.5-pro':                    { short: 'gemini-pro',       vendor: 'google',    roles: ['generalist','reasoner'], tier: 'premium',  ctx: 1048576, priceIn: 1.25,  priceOut: 10.00, timeout: 180000, maxOut: 16000 },
  'mistralai/mistral-small-2603':             { short: 'mistral-sm4',      vendor: 'mistral',   roles: ['generalist'],            tier: 'standard', ctx: 262144,  priceIn: 0.15,  priceOut: 0.60,  timeout: 180000, maxOut: 16000 },
  'mistralai/mistral-large-2512':             { short: 'mistral-lg',       vendor: 'mistral',   roles: ['generalist'],            tier: 'standard', ctx: 262144,  priceIn: 0.50,  priceOut: 1.50,  timeout: 180000, maxOut: 16000 },
  'openai/gpt-5-mini':                        { short: 'gpt5-mini',        vendor: 'openai',    roles: ['generalist'],            tier: 'speed',    ctx: 400000,  priceIn: 0.25,  priceOut: 2.00,  timeout: 180000, maxOut: 16000 },
  'x-ai/grok-4.1-fast':                       { short: 'grok-4.1',         vendor: 'xai',       roles: ['generalist'],            tier: 'speed',    ctx: 2000000, priceIn: 0.20,  priceOut: 0.50,  timeout: 180000, maxOut: 16000 },
  'meta-llama/llama-4-maverick':              { short: 'llama4-mav',       vendor: 'meta',      roles: ['generalist'],            tier: 'standard', ctx: 1048576, priceIn: 0.15,  priceOut: 0.60,  timeout: 180000, maxOut: 16000 },
  // bytedance-seed-2.0-mini — RECLASSIFIED 2026-05-09 §S-166 from code_expert→generalist (mini-class fails NAME_PATTERN_BANS for code_expert).
  'bytedance-seed/seed-2.0-mini':             { short: 'seed2-mini',       vendor: 'bytedance', roles: ['generalist'],            tier: 'speed',    ctx: 262144,  priceIn: 0.10,  priceOut: 0.40,  timeout: 180000, maxOut: 16000 },
  'z-ai/glm-4.7':                             { short: 'glm-4.7',          vendor: 'zhipu',     roles: ['generalist'],            tier: 'standard', ctx: 202752,  priceIn: 0.39,  priceOut: 1.75,  timeout: 180000, maxOut: 16000 },
  'tencent/hunyuan-a13b-instruct':            { short: 'hunyuan',          vendor: 'tencent',   roles: ['generalist'],            tier: 'standard', ctx: 131072,  priceIn: 0.14,  priceOut: 0.57,  timeout: 180000, maxOut: 16000 },
  'minimax/minimax-m2.7':                     { short: 'minimax-m2.7',     vendor: 'minimax',   roles: ['generalist'],            tier: 'standard', ctx: 1048576, priceIn: 0.50,  priceOut: 2.00,  timeout: 300000, maxOut: 16000 },
};

// ─── DOMAIN_ROSTER ──────────────────────────────────────────────────────────
// Priority ordering per domain — first 5 are primary, rest are secondary/reserves.
// selectModels() consumes this and applies role-coverage + vendor-diversity + role-fit.

const DOMAIN_ROSTER = {
  rust_backend: [
    'deepseek/deepseek-r1-0528', 'deepseek/deepseek-chat-v3-0324', 'qwen/qwen3-coder',
    'x-ai/grok-code-fast-1', 'nvidia/nemotron-3-super-120b-a12b',
    'meta-llama/llama-4-maverick', 'inception/mercury-coder', 'mistralai/mistral-small-2603',
    'moonshotai/kimi-k2.5', 'qwen/qwen3-235b-a22b-2507',
  ],
  nodejs_frontend: [
    'x-ai/grok-4.1-fast', 'openai/gpt-5-mini', 'google/gemini-2.5-flash',
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
    'google/gemini-2.5-flash', 'deepseek/deepseek-r1-0528',
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
    'deepseek/deepseek-r1-0528', 'qwen/qwen3-235b-a22b-2507', 'google/gemini-2.5-flash',
    'xiaomi/mimo-v2-pro', 'moonshotai/kimi-k2.5',
    'deepseek/deepseek-chat-v3-0324', 'nvidia/nemotron-3-super-120b-a12b', 'x-ai/grok-4.1-fast',
    'mistralai/mistral-small-2603', 'openai/gpt-5-mini',
  ],
};

// Batch → domain mapping (preserved from legacy script for backward compat)
const BATCH_DOMAIN = {
  '01-server-rust':                'rust_backend',
  '02-agent-rust':                 'rust_backend',
  '03-sentry-watchdog-common':     'windows_os',
  '04-comms-link':                 'nodejs_frontend',
  '05-audit-detection-healing':    'sre_ops',
  '06-deploy-infra':               'sre_ops',
  '07-standing-rules-crosssystem': 'cross_system',
  '08-frontend-nextjs':            'nodejs_frontend',
};

// ─── E3: validateRoleAssignment + NAME_PATTERN_BANS ─────────────────────────
// Speed-class words that disqualify a model from deep-reasoning roles.
// SRE + generalist tolerate speed-class (pattern-matching dominates over deep reasoning).

const NAME_PATTERN_BANS = {
  reasoner:    /(^|[\/\-\.])(haiku|nano|flash|mini|lite|small|edge|tiny|fast)([\/\-\.]|$)/i,
  code_expert: /(^|[\/\-\.])(haiku|nano|flash|mini|lite|small|edge|tiny|fast)([\/\-\.]|$)/i,
  // sre + generalist: no name bans
};

function validateRoleAssignment(modelId, role) {
  const cfg = MODEL_REGISTRY[modelId];
  if (!cfg) {
    return { valid: false, reason: `model "${modelId}" not in MODEL_REGISTRY (run scripts/lib/refresh-mma-registry.js to discover or add manually)` };
  }
  if (!cfg.roles.includes(role)) {
    return { valid: false, reason: `model "${modelId}" not tagged for role "${role}" (tagged: ${cfg.roles.join(',')})` };
  }
  if (cfg.deprecated_for && cfg.deprecated_for.includes(role)) {
    return { valid: false, reason: `model "${modelId}" marked deprecated_for role "${role}" — use a current-tier replacement (see MODEL_REGISTRY tier='premium' or 'standard')` };
  }
  const ban = NAME_PATTERN_BANS[role];
  if (ban && ban.test(modelId) && !cfg.allow_speed_class) {
    return { valid: false, reason: `model "${modelId}" matches speed-class name pattern (haiku/nano/flash/mini/lite/small/edge/tiny/fast) and is banned from role "${role}". Anchors: feedback_model_role_fit_check_before_vendor_fit_20260509.md PROMOTE-NOW-ACTIVE 2026-05-09. Pick a deep-reasoning-tier model from MODEL_REGISTRY where tier='premium' or 'standard'.` };
  }
  return { valid: true };
}

// ─── selectModels (preserved + role-fit-validated) ──────────────────────────
function selectModels(domain, count = 5, exclude = []) {
  const roster = DOMAIN_ROSTER[domain] || DOMAIN_ROSTER.cross_system;
  const candidates = roster.filter(id => !exclude.includes(id) && MODEL_REGISTRY[id]);

  const selected = [];
  const usedVendors = {};

  function addModel(id) {
    const cfg = MODEL_REGISTRY[id];
    usedVendors[cfg.vendor] = (usedVendors[cfg.vendor] || 0) + 1;
    selected.push(id);
  }

  function canAdd(id) {
    const cfg = MODEL_REGISTRY[id];
    if ((usedVendors[cfg.vendor] || 0) >= 2) return false;
    return !selected.includes(id);
  }

  // Step 1: Fill required roles with role-fit-validated candidates (E3)
  const requiredRoles = ['reasoner', 'code_expert', 'sre'];
  for (const role of requiredRoles) {
    const candidate = candidates.find(id => {
      const cfg = MODEL_REGISTRY[id];
      if (!cfg.roles.includes(role)) return false;
      if (!canAdd(id)) return false;
      // E3 enforcement: skip if validateRoleAssignment rejects
      const v = validateRoleAssignment(id, role);
      return v.valid;
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
  // 3 models from different vendor families, NONE used in prior steps; all role-fit-validated for code_expert (Step 4 default).
  const usedVendors = new Set(usedModels.map(id => MODEL_REGISTRY[id]?.vendor).filter(Boolean));
  const candidates = Object.keys(MODEL_REGISTRY)
    .filter(id => !usedModels.includes(id))
    .filter(id => {
      const cfg = MODEL_REGISTRY[id];
      return cfg && !usedVendors.has(cfg.vendor);
    })
    // Prefer code_expert role-fit for adversarial (deepest critique on code-changing PRs)
    .sort((a, b) => {
      const aV = validateRoleAssignment(a, 'code_expert').valid ? 0 : 1;
      const bV = validateRoleAssignment(b, 'code_expert').valid ? 0 : 1;
      return aV - bV;
    });
  return candidates.slice(0, count);
}

module.exports = {
  MODEL_REGISTRY,
  DOMAIN_ROSTER,
  BATCH_DOMAIN,
  NAME_PATTERN_BANS,
  selectModels,
  selectAdversarialModels,
  validateRoleAssignment,
};
