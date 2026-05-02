#!/usr/bin/env node
/**
 * v2-claim-gate.js — UserPromptSubmit hook for V2-state-claim detection.
 *
 * PACT-20260503-002 Piece 3 (pre-claim gate hook). Doctrine-class.
 * Captain authorized 2026-05-03; AWAITS Captain G33-GUIDE-CONFIRM ratify
 * before settings.json wire-in.
 *
 * Trigger: user prompt contains V2-state-claim language.
 * Action:  emit advisory to stderr (non-blocking) reminding pilot to read
 *          comms-link/V2-MASTER-STATE.md before synthesizing V2 state.
 *
 * Why this hook exists:
 *   2026-05-03 G9 anchor — pilot AI emitted 4 wrong-then-corrected V2 status
 *   claims in single session because Verify-Before-Generate is enforced as
 *   discipline not hook. This hook converts that discipline into a runtime
 *   gate by surfacing V2-MASTER-STATE.md presence + staleness whenever a
 *   V2-state question lands in the prompt.
 *
 * Exit codes:
 *   0 = pass-through (always; advisory-only / non-blocking)
 *   2 = reserved for future hard-block escalation (gates on N=2 anchor + Captain ratify)
 *
 * Composes with: feedback_v2_doctrine_alignment_drift_g9_pact_20260503_002.md
 *                comms-link/V2-MASTER-STATE.md (read-target)
 *                scripts/v2-state-refresh.sh (refresh-action)
 *                CGP v4.3 + COGNITIVE-GATE-PROTOCOL.md (gate machinery)
 *
 * Initial scope: V2-state claims only (Q1C lean). Sibling-PACT extension
 * to deploy/fleet/customer-flow/PACT-class drift on N=2 anchor (≥30d).
 */

'use strict';

const fs = require('fs');
const path = require('path');

// Resolve V2-MASTER-STATE.md path. Try standard path first; fall back to env override.
const V2_MASTER_STATE = process.env.V2_MASTER_STATE_PATH
  || 'C:/Users/bono/racingpoint/comms-link/V2-MASTER-STATE.md';

const RACECONTROL_ROOT = process.env.RACECONTROL_ROOT
  || 'C:/Users/bono/racingpoint/racecontrol';

const STALENESS_ADVISORY_HOURS = 24;
const STALENESS_FORCED_HOURS = 24 * 7;

// V2-state-claim trigger patterns (initial allowlist; deliberately narrow per
// CANDIDATE-N1 promotion-on-N=2-anchor doctrine).
//
// Empirical anchor: the literal G9 prompt 2026-05-03 was
// "what is the percentage completed of RacingPoint Ecosystem V2?"
// — patterns below MUST fire on that prompt class.
const TRIGGER_PATTERNS = [
  // RacingPoint Ecosystem V2 (proper noun + V2)
  /RacingPoint\s+Ecosystem\s+V2/i,
  // Direct V2 status / progress / completion (V2 followed by status word)
  /\bV2\s+(surface|surfaces|state|status|progress|complete|done|deployed|shipped|ship|ready)\b/i,
  // V2 + percentage / percent / completion question
  /percent(age)?\b[^\.]{0,40}\bV2\b/i,
  /\bV2\b[^\.]{0,40}\bpercent(age)?\b/i,
  /\bV2\b[^\.]{0,40}\b(complete|completed|done|shipped|deployed|ready)\b/i,
  /\b(complete|completed|done|shipped|deployed|ready)\b[^\.]{0,40}\bV2\b/i,
  // Phase references (V2 phases 0-H per PACT-015)
  /\bphase\s+[0-9A-H](\.\d+)?\b/i,
  // Percentage-of-V2 claims
  /(\d+\s*%|percent)\s+(complete|completed|done|shipped|of\s+V2)/i,
  // Surface-count claims
  /\bsurface\s+count\b/i,
  /\b(?:1|3|4|5|6|7|14)\s*\/\s*14\s+surface/i,
  // V2.0 ship/deploy/complete
  /V2\.\s*0\s+(ship|deploy|complete|ready)/i,
  // Foundation contract claims
  /\b(F[1-9]|F1[02])\s+(shipped|deployed|implemented|active|wired)\b/i,
  // Customer-touch / customer-workflow scope at V2
  /customer[- ]touch\s+(at\s+)?V2/i,
  // PACT-class V2 references
  /PACT-2026[01]\d{4,5}/i,
];

// Read prompt from stdin (Claude Code passes hook input as JSON on stdin).
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { input += chunk; });
process.stdin.on('end', () => {
  let prompt;
  try {
    const parsed = JSON.parse(input);
    prompt = parsed.prompt || parsed.user_prompt || parsed.text || input;
  } catch {
    prompt = input;
  }

  if (!prompt || typeof prompt !== 'string') {
    process.exit(0);
  }

  // Check if any trigger pattern fires
  const firedPatterns = TRIGGER_PATTERNS
    .map((re, i) => ({ re, i, hit: re.test(prompt) }))
    .filter(x => x.hit);

  if (firedPatterns.length === 0) {
    process.exit(0); // No V2-state language; pass through silently.
  }

  // Build advisory
  const lines = [];
  lines.push(`[v2-claim-gate] V2-state language detected in user prompt (${firedPatterns.length} trigger pattern(s) fired).`);

  // Check V2-MASTER-STATE.md presence + staleness
  let masterStateExists = false;
  let masterStateAgeHours = null;
  try {
    const stat = fs.statSync(V2_MASTER_STATE);
    masterStateExists = true;
    masterStateAgeHours = (Date.now() - stat.mtimeMs) / (1000 * 60 * 60);
  } catch {
    /* file missing */
  }

  if (!masterStateExists) {
    lines.push(`[v2-claim-gate] WARN: V2-MASTER-STATE.md not found at ${V2_MASTER_STATE}.`);
    lines.push(`[v2-claim-gate]       Run scripts/v2-state-refresh.sh OR cite ground-truth source explicitly.`);
  } else {
    const age = masterStateAgeHours.toFixed(1);
    if (masterStateAgeHours > STALENESS_FORCED_HOURS) {
      lines.push(`[v2-claim-gate] STALE-FORCED: V2-MASTER-STATE.md last refreshed ${age}h ago (>${STALENESS_FORCED_HOURS}h).`);
      lines.push(`[v2-claim-gate]               Refresh REQUIRED before V2-state claim. Run: bash scripts/v2-state-refresh.sh dry-run`);
    } else if (masterStateAgeHours > STALENESS_ADVISORY_HOURS) {
      lines.push(`[v2-claim-gate] STALE-ADVISORY: V2-MASTER-STATE.md last refreshed ${age}h ago (>${STALENESS_ADVISORY_HOURS}h).`);
      lines.push(`[v2-claim-gate]                 Recommend refresh: bash scripts/v2-state-refresh.sh dry-run`);
    } else {
      lines.push(`[v2-claim-gate] V2-MASTER-STATE.md last refreshed ${age}h ago (fresh).`);
    }
  }

  // Quick git-state probe (optional; doesn't fail if git unavailable)
  try {
    const { execSync } = require('child_process');
    const v2OnMain = execSync(
      `git -C "${RACECONTROL_ROOT}" ls-tree -r --name-only origin/main 2>/dev/null | grep -cE '(kiosk|web|admin|pwa)/.*v2.*\\.tsx' || echo 0`,
      { stdio: ['ignore', 'pipe', 'ignore'] }
    ).toString().trim();
    const v2OnAmend1 = execSync(
      `git -C "${RACECONTROL_ROOT}" ls-tree -r --name-only amend1-section1-heart-admin-substrate 2>/dev/null | grep -cE '(kiosk|web|admin|pwa)/.*v2.*\\.tsx' || echo 0`,
      { stdio: ['ignore', 'pipe', 'ignore'] }
    ).toString().trim();
    lines.push(`[v2-claim-gate] V2 .tsx count: origin/main=${v2OnMain} | amend1-section1-heart-admin-substrate=${v2OnAmend1}`);
  } catch {
    /* git probe failed; non-critical */
  }

  // Closing reminder
  lines.push(`[v2-claim-gate] Read comms-link/V2-MASTER-STATE.md (latest §S-N) BEFORE synthesizing V2 state from individual memory files.`);
  lines.push(`[v2-claim-gate] PACT-20260503-002 doctrine: substrate-substitution for Verify-Before-Generate principle.`);

  // Emit to stderr (Claude Code surfaces stderr as system-reminder mechanism)
  process.stderr.write(lines.join('\n') + '\n');

  // Non-blocking exit (advisory-only initial scope per smallest-reversible-fix)
  process.exit(0);
});
