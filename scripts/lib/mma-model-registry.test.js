// scripts/lib/mma-model-registry.test.js — §S-166 smoke test (v0.1, 2026-05-18)
//
// Run: node scripts/lib/mma-model-registry.test.js
// Expected: stdout "SMOKE TEST PASS …" and process exit 0; any AssertionError exits non-zero.

'use strict';

const assert = require('assert');
const { validateRoleAssignment } = require('./mma-model-registry');

// Positive — Captain's authorized 5-model pool for D-04 §S-397 spawn (2026-05-18 16:42 IST auth;
// composite anchor renumbered from §S-396 to §S-397 because §S-396 was consumed by D-COMMS-PEER
// ratify at racecontrol 45fe3733 — Captain correction 2026-05-18 ~16:54 IST)
const AUTHORIZED_POOL = [
  ['anthropic/claude-opus-4.7',    'reasoner'],
  ['anthropic/claude-sonnet-4.6',  'code_expert'],
  ['xiaomi/mimo-v2.5-pro',         'sre'],
  ['qwen/qwen3-coder-plus',        'code_expert'],
  ['mistralai/mistral-large-2512', 'generalist'],
];
for (const [modelId, role] of AUTHORIZED_POOL) {
  const r = validateRoleAssignment(modelId, role);
  assert.strictEqual(r.valid, true, `AUTH-POOL: ${modelId} for ${role} must pass (got: ${r.reason})`);
}

// Negative — speed-class names must reject for reasoner / code_expert
const SPEED_CLASS_REJECTIONS = [
  ['anthropic/claude-haiku-4.5',   'reasoner'],
  ['google/gemini-2.5-flash',      'code_expert'],
  ['openai/gpt-5-mini',            'reasoner'],
  ['bytedance-seed/seed-2.0-mini', 'code_expert'],
];
for (const [modelId, role] of SPEED_CLASS_REJECTIONS) {
  const r = validateRoleAssignment(modelId, role);
  assert.strictEqual(r.valid, false, `SPEED-CLASS: ${modelId} for ${role} must reject`);
  assert.ok(r.reason.includes('speed-class'), `SPEED-CLASS: reason must mention 'speed-class' (got: ${r.reason})`);
}

// Positive — Grok-Code-Fast-1 allow-list exception for code_expert
{
  const r = validateRoleAssignment('x-ai/grok-code-fast-1', 'code_expert');
  assert.strictEqual(r.valid, true, `ALLOW-LIST: grok-code-fast-1 must pass for code_expert (got: ${r.reason})`);
}

// Negative — Grok-Code-Fast-1 NOT allow-listed for reasoner
{
  const r = validateRoleAssignment('x-ai/grok-code-fast-1', 'reasoner');
  assert.strictEqual(r.valid, false, `ALLOW-LIST: grok-code-fast-1 must reject for reasoner`);
}

// Positive — speed-class tokens permitted in tolerant roles (sre / domain / generalist)
const TOLERANT_ROLE_PASSES = [
  ['google/gemini-2.5-flash',      'generalist'],
  ['openai/gpt-5-mini',            'generalist'],
  ['mistralai/mistral-small-2603', 'generalist'],
];
for (const [modelId, role] of TOLERANT_ROLE_PASSES) {
  const r = validateRoleAssignment(modelId, role);
  assert.strictEqual(r.valid, true, `TOLERANT: ${modelId} for ${role} must pass (got: ${r.reason})`);
}

// False-positive guard — "minimax" contains "mini" but is not a speed-class name
{
  const r = validateRoleAssignment('minimax/minimax-m2.7', 'reasoner');
  assert.strictEqual(r.valid, true, `FP-GUARD: minimax must NOT trigger 'mini' speed-class (got: ${r.reason})`);
}

// Defensive — invalid inputs
{
  const r = validateRoleAssignment('', 'reasoner');
  assert.strictEqual(r.valid, false, 'EMPTY-INPUT: empty modelId must reject');
}
{
  const r = validateRoleAssignment('anthropic/claude-opus-4.7', 'bogus-role');
  assert.strictEqual(r.valid, false, 'INVALID-ROLE: unknown role must reject');
}

// Registry-wide — every MODEL_REGISTRY entry's declared roles must pass §S-166.
// Reads multi-model-audit.js as source (avoids require() side-effects: the script
// initializes OPENROUTER_KEY at module load). Regex matches the canonical
// "'vendor/id': { ... roles: [...] }" entry pattern; DOMAIN_ROSTER string-array
// entries lack the `roles:` token and are skipped.
{
  const fs = require('fs');
  const path = require('path');
  const src = fs.readFileSync(path.join(__dirname, '..', 'multi-model-audit.js'), 'utf8');
  const entryRe = /^\s*'([^']+\/[^']+)':\s*\{[^}]*roles:\s*\[([^\]]+)\]/gm;
  let scanned = 0;
  let m;
  while ((m = entryRe.exec(src)) !== null) {
    const modelId = m[1];
    const roles = m[2].match(/'([a-z_]+)'/g).map((s) => s.replace(/'/g, ''));
    scanned += 1;
    for (const role of roles) {
      const r = validateRoleAssignment(modelId, role);
      assert.strictEqual(
        r.valid,
        true,
        `REGISTRY-WIDE: ${modelId} declared role "${role}" must pass §S-166 (got: ${r.reason})`
      );
    }
  }
  assert.ok(scanned >= 10, `REGISTRY-WIDE: expected ≥10 entries scanned (got ${scanned})`);
}

console.log('SMOKE TEST PASS — §S-166 validator behaves per CLAUDE.md May-2026 catalog');
process.exit(0);
