#!/usr/bin/env node
// Unit tests for scripts/diagnose/triage.js — closes NOT TESTED item (5)
// from Commit 2: --target repeatable, --caller, --json parsing. Pure
// function tests, no server required.
//
// Run: node scripts/diagnose/triage.test.js
// Exits 0 on all-pass, 1 on any failure.

const assert = require('node:assert/strict');
const { parseArgs, formatVerdict, exitCodeFor } = require('./triage.js');

let pass = 0;
let fail = 0;
function run(name, fn) {
  try {
    fn();
    console.log(`ok  ${name}`);
    pass += 1;
  } catch (e) {
    console.error(`FAIL ${name}`);
    console.error(`     ${e.message}`);
    fail += 1;
  }
}

run('parseArgs: single target', () => {
  const opts = parseArgs(['--target=pod_3', 'pod_3 iracing']);
  assert.deepEqual(opts.targets, ['pod_3']);
  assert.equal(opts.query, 'pod_3 iracing');
});

run('parseArgs: multiple --target flags accumulate', () => {
  const opts = parseArgs(['--target=pod_3', '--target=pod_4', 'query here']);
  assert.deepEqual(opts.targets, ['pod_3', 'pod_4']);
});

run('parseArgs: --caller overrides default', () => {
  const opts = parseArgs(['--caller=gsd-debug', 'query']);
  assert.equal(opts.caller, 'gsd-debug');
});

run('parseArgs: --json flag set', () => {
  const opts = parseArgs(['--json', 'query']);
  assert.equal(opts.json, true);
});

run('parseArgs: query assembled from remaining args', () => {
  const opts = parseArgs(['why', 'does', 'pod_4', 'fail']);
  assert.equal(opts.query, 'why does pod_4 fail');
});

run('parseArgs: no targets → empty array', () => {
  const opts = parseArgs(['some query']);
  assert.deepEqual(opts.targets, []);
});

run('exitCodeFor: TRIAGE_FAST + no MMA → 0', () => {
  assert.equal(
    exitCodeFor({ verdict: 'TRIAGE_FAST', should_run_mma: false }),
    0
  );
});

run('exitCodeFor: TRIAGE_FAST + gsd-debug forcing MMA → 1', () => {
  assert.equal(
    exitCodeFor({ verdict: 'TRIAGE_FAST', should_run_mma: true }),
    1
  );
});

run('exitCodeFor: AUDIT_DEEP → 1', () => {
  assert.equal(exitCodeFor({ verdict: 'AUDIT_DEEP', should_run_mma: true }), 1);
});

run('exitCodeFor: RESEARCH_NEW → 1', () => {
  assert.equal(
    exitCodeFor({ verdict: 'RESEARCH_NEW', should_run_mma: true }),
    1
  );
});

run('exitCodeFor: REJECT_AMBIGUOUS → 2', () => {
  assert.equal(
    exitCodeFor({ verdict: 'REJECT_AMBIGUOUS', should_run_mma: false }),
    2
  );
});

run('formatVerdict: TRIAGE_FAST includes bug_id and affects_targets', () => {
  const out = formatVerdict('test', {
    verdict: 'TRIAGE_FAST',
    bug_id: 'INV-9',
    hit_type: 'EXACT',
    confidence: 0.95,
    fix_status: 'code_fixed',
    fixed_in_build: '68f4d61e',
    affects_targets: ['pod_1', 'pod_3'],
    matched_pattern: 'steam dialog',
    escalation_message: 'test esc',
    should_run_mma: false,
  });
  assert.match(out, /BUG_ID: INV-9/);
  assert.match(out, /HIT_TYPE: EXACT/);
  assert.match(out, /AFFECTS: pod_1, pod_3/);
  assert.match(out, /test esc/);
});

run('formatVerdict: AUDIT_DEEP includes symptom classes', () => {
  const out = formatVerdict('test', {
    verdict: 'AUDIT_DEEP',
    relevant_symptom_classes: ['zero-laps', 'python-ini'],
    rationale: 'no hit',
    should_run_mma: true,
  });
  assert.match(out, /CLASSES: zero-laps, python-ini/);
  assert.match(out, /SHOULD_RUN_MMA: true/);
});

run('formatVerdict: REJECT_AMBIGUOUS lists clarifying questions', () => {
  const out = formatVerdict('broken', {
    verdict: 'REJECT_AMBIGUOUS',
    rationale: 'too vague',
    clarifying_questions: ['Which pod?', 'What symptom?'],
    should_run_mma: false,
  });
  assert.match(out, /Which pod\?/);
  assert.match(out, /What symptom\?/);
});

console.log(`\n${pass} passed, ${fail} failed`);
process.exit(fail > 0 ? 1 : 0);
