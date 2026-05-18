// scripts/lib/mma-model-registry.js — §S-166 enforcement validator v0.1 (2026-05-18)
//
// validateRoleAssignment(modelId, role) — rejects speed-class names from reasoner/code_expert
// roles per CLAUDE.md May-2026 MMA catalog. Grok-Code-Fast-1 is the only allow-listed exception
// (code_expert role only). SRE / domain / generalist roles are speed-class-tolerant.
//
// Doctrine source: COGNITIVE-GATE-PROTOCOL.md §MMA Step 1 model pool table (§S-166 enforcement).
// Anchor: feedback_model_role_fit_check_before_vendor_fit_20260509.md (PROMOTE-NOW-ACTIVE).

'use strict';

const SPEED_CLASS_TOKENS = ['haiku', 'nano', 'flash', 'mini', 'lite', 'small', 'edge', 'tiny', 'fast'];
const SPEED_CLASS_ROLES = new Set(['reasoner', 'code_expert']);
const CODE_EXPERT_ALLOW_LIST = ['grok-code-fast-1'];
const VALID_ROLES = ['reasoner', 'code_expert', 'sre', 'domain', 'generalist'];

function containsSpeedClassToken(modelId) {
  const lower = modelId.toLowerCase();
  for (const token of SPEED_CLASS_TOKENS) {
    // Token must be delimited by /, -, ., digit, or start/end. Prevents false-match on
    // substrings like "minimax" (contains "mini" but is not a speed-class name).
    const re = new RegExp(`(^|[/\\-.])${token}([/\\-.0-9]|$)`, 'i');
    if (re.test(lower)) return token;
  }
  return null;
}

function isAllowListedException(modelId, role) {
  if (role !== 'code_expert') return false;
  const lower = modelId.toLowerCase();
  return CODE_EXPERT_ALLOW_LIST.some((allow) => lower.includes(allow));
}

function validateRoleAssignment(modelId, role) {
  if (typeof modelId !== 'string' || modelId.length === 0) {
    return { valid: false, reason: 'modelId must be non-empty string' };
  }
  if (!VALID_ROLES.includes(role)) {
    return { valid: false, reason: `role must be one of ${VALID_ROLES.join(', ')} (got "${role}")` };
  }
  if (!SPEED_CLASS_ROLES.has(role)) {
    return { valid: true, reason: `role "${role}" is speed-class-tolerant per CLAUDE.md May-2026 catalog` };
  }
  const speedToken = containsSpeedClassToken(modelId);
  if (!speedToken) {
    return { valid: true, reason: `${modelId} has no speed-class token (role "${role}")` };
  }
  if (isAllowListedException(modelId, role)) {
    return { valid: true, reason: `allow-listed exception: ${modelId} explicitly permitted for code_expert` };
  }
  return {
    valid: false,
    reason: `§S-166: speed-class token "${speedToken}" in ${modelId} forbidden for role "${role}" per CLAUDE.md May-2026 catalog`,
  };
}

module.exports = {
  validateRoleAssignment,
  SPEED_CLASS_TOKENS,
  CODE_EXPERT_ALLOW_LIST,
  VALID_ROLES,
};
