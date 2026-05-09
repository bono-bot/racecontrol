#!/usr/bin/env node
// pre-vms-duplicate-check.js — bono-side PreToolUse Bash hook (REFERENCE IMPLEMENTATION)
// SPEC: pre-vms-duplicate-check.spec.md (sibling file in this dir)
// STATUS: source-tracked reference; install to ~/.claude/hooks/ pending Captain explicit per-action auth
// Sibling-of: §S-159 pre-mma-duplicate-check.js · §S-178 james-side pre-vms-duplicate-check.js · §S-161 bono path-(B) install pattern
// Bravo-slice item 1 partial-completion artifact per PACT-DRAFT-bravo-slice-20260510 §2 / §S-184 / §S-189
// Security note: uses execFileSync (no shell invocation) per security_reminder_hook recommendation
'use strict';

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');

const VERSION = '0.1.0-bono';
const PILOT = 'bono';

const COMMS_LINK_DIR = process.env.PRE_VMS_COMMS_LINK_DIR || '/root/comms-link';
const OVERRIDE_LEDGER = process.env.PRE_VMS_OVERRIDE_LEDGER || '/root/comms-link/data/vms-append-overrides.jsonl';

function readStdin() {
  try {
    return fs.readFileSync(0, 'utf-8');
  } catch (e) {
    return '';
  }
}

function emitInfo(msg) {
  process.stderr.write(`[pre-vms-dup-check ${VERSION}] ${msg}\n`);
}

function emitBlock(msg) {
  process.stderr.write(`[pre-vms-dup-check ${VERSION}] [VMS-COLLISION-BLOCK] ${msg}\n`);
  process.stderr.write(`Override: VMS_FORCE_APPEND=1 (logged to ${OVERRIDE_LEDGER}).\n`);
  process.stderr.write(`Rationale: §S-121 v0.4 IMMEDIATE-PRE-COMMIT sub-rule (PROMOTE-N=11+ empirical anchor); slot-collision detection prevents bilateral §S-N drift.\n`);
}

function detectVmsAppend(command) {
  if (!command) return null;
  const vmsCommitPattern = /git\s+commit[^\n]*vms:?\s*§S-\d+/i;
  if (vmsCommitPattern.test(command)) {
    const match = command.match(/§S-(\d+)/);
    return { pattern: 'git-commit-vms', slotN: match ? parseInt(match[1], 10) : null };
  }
  const appendPattern = /(cat|echo|printf)[^|]*>>\s*[^\s]*V2-MASTER-STATE\.md/i;
  if (appendPattern.test(command)) {
    return { pattern: 'append-redirect', slotN: null };
  }
  const heredocPattern = /V2-MASTER-STATE\.md.*<<\s*['"]?EOF/i;
  if (heredocPattern.test(command)) {
    return { pattern: 'heredoc', slotN: null };
  }
  return null;
}

function slotPreflight() {
  try {
    execFileSync('git', ['-C', COMMS_LINK_DIR, 'fetch', 'origin', '--quiet'], { timeout: 10000, stdio: 'pipe' });
    const showOut = execFileSync('git', ['-C', COMMS_LINK_DIR, 'show', 'origin/main:V2-MASTER-STATE.md'], { timeout: 5000, encoding: 'utf-8' });
    const sectionLines = showOut.split('\n').filter(l => /^## §S-\d+/.test(l));
    const originLatest = sectionLines.length > 0 ? sectionLines[sectionLines.length - 1] : '';
    const originSlotMatch = originLatest.match(/§S-(\d+)/);
    const originSlotN = originSlotMatch ? parseInt(originSlotMatch[1], 10) : null;
    return { originSlotN, originLatest };
  } catch (e) {
    return { error: String(e.message || e).substring(0, 200) };
  }
}

function logOverride(command, reason) {
  try {
    const entry = {
      ts: new Date().toISOString(),
      pilot: PILOT,
      hook: 'pre-vms-duplicate-check',
      override: 'VMS_FORCE_APPEND=1',
      command: String(command || '').substring(0, 500),
      reason: reason || 'unspecified'
    };
    fs.mkdirSync(path.dirname(OVERRIDE_LEDGER), { recursive: true });
    fs.appendFileSync(OVERRIDE_LEDGER, JSON.stringify(entry) + '\n');
  } catch (e) {
    process.stderr.write(`[pre-vms-dup-check] override-log-failure: ${e.message}\n`);
  }
}

function main() {
  const raw = readStdin();
  if (!raw) {
    process.exit(0);
  }
  let payload;
  try {
    payload = JSON.parse(raw);
  } catch (e) {
    process.exit(0);
  }

  if (payload.tool_name !== 'Bash') {
    process.exit(0);
  }

  const command = (payload.tool_input && payload.tool_input.command) || '';
  const detection = detectVmsAppend(command);
  if (!detection) {
    process.exit(0);
  }

  if (process.env.VMS_FORCE_APPEND === '1') {
    emitInfo(`override active (VMS_FORCE_APPEND=1) — bypass on detection=${detection.pattern} slot=${detection.slotN}`);
    logOverride(command, 'VMS_FORCE_APPEND=1');
    process.exit(0);
  }

  const preflight = slotPreflight();
  if (preflight.error) {
    emitInfo(`slot pre-flight FAILED (non-blocking): ${preflight.error}`);
    process.exit(0);
  }

  if (detection.slotN !== null && preflight.originSlotN !== null) {
    if (detection.slotN <= preflight.originSlotN) {
      emitBlock(`declared §S-${detection.slotN} ≤ origin/main latest ${preflight.originLatest}; slot-collision likely. Re-fetch + renumber to §S-${preflight.originSlotN + 1} before retry.`);
      process.exit(2);
    }
    if (detection.slotN > preflight.originSlotN + 1) {
      emitInfo(`declared §S-${detection.slotN} > origin/main latest ${preflight.originLatest} + 1 (gap detected); allowing but verify intent.`);
    } else {
      emitInfo(`slot pre-flight OK — §S-${detection.slotN} is next-available post-${preflight.originLatest}.`);
    }
  } else {
    emitInfo(`detection=${detection.pattern} (no explicit §S-N in command); origin/main latest ${preflight.originLatest || 'unknown'}; allowing.`);
  }

  process.exit(0);
}

main();
