#!/usr/bin/env node
// MMA Triage Router CLI — wraps POST /api/v1/diagnose/triage.
//
// Usage: node scripts/diagnose/triage.js "pod 3 iracing steam dialog"
//        node scripts/diagnose/triage.js --caller=gsd-debug --target=pod_4 "query"
//        node scripts/diagnose/triage.js --help
//
// Env:   RC_SERVER_URL       (default http://192.168.31.23:8080)
//        STAFF_TOKEN         (required — /diagnose/triage is staff-JWT gated)
//        TRIAGE_TIMEOUT_MS   (default 5000)
//        GSD_PHASE           (if set, caller_context defaults to gsd-debug)
//
// Exit codes:
//   0 — TRIAGE_FAST, should_run_mma=false (oracle handled it)
//   1 — AUDIT_DEEP / RESEARCH_NEW / TRIAGE_FAST+gsd-debug (MMA recommended)
//   2 — REJECT_AMBIGUOUS (query needs clarification)
//   3 — error (server down, auth, parse)
//
// Spec: .planning/specs/MMA-TRIAGE-ROUTER.md (Commit 2 of 3).
//
// ARCH FINDING 2026-04-18 (still open): rc-agent's Tier-0 check in
// crates/rc-agent/src/ai_debugger.rs calls /mesh/audit-check (the old endpoint)
// with NO auth headers. The route sits in the staff-JWT block, so every
// rc-agent Tier-0 call has been silently 401'ing since MMA-v29. Separate bug;
// this CLI targets the richer /diagnose/triage endpoint and authenticates
// with STAFF_TOKEN, so the issue above doesn't apply here — but that
// autonomous path is still broken. Fix options: (A) move /mesh/audit-check
// back to public routes, (B) add X-Service-Key variant and teach rc-agent
// to send it. See Gap 4 handoff in MEMORY.md.

const http = require('node:http');
const https = require('node:https');
const { URL } = require('node:url');

const SERVER = process.env.RC_SERVER_URL || 'http://192.168.31.23:8080';
const TIMEOUT_MS = parseInt(process.env.TRIAGE_TIMEOUT_MS || '5000', 10);
const STAFF_TOKEN = process.env.STAFF_TOKEN || '';
const DEFAULT_CALLER = process.env.GSD_PHASE ? 'gsd-debug' : 'ops-triage';

function usage() {
  const lines = [
    'MMA Triage Router CLI',
    '',
    'usage: node scripts/diagnose/triage.js [options] "<query>"',
    '',
    'options:',
    '  --caller=<ctx>   caller_context: ops-triage | gsd-debug | mma-audit | unknown',
    '                   default: gsd-debug if $GSD_PHASE set, else ops-triage',
    '  --target=pod_N   add to targets_hint (repeatable)',
    '  --json           output raw JSON verdict only (for scripting)',
    '  --help           show this',
    '',
    'env:',
    '  RC_SERVER_URL       default http://192.168.31.23:8080',
    '  STAFF_TOKEN         required (staff-JWT gated endpoint)',
    '  TRIAGE_TIMEOUT_MS   default 5000',
    '  GSD_PHASE           if set, caller defaults to gsd-debug',
    '',
    'exit codes: 0=TRIAGE_FAST (no MMA), 1=should_run_mma, 2=ambiguous, 3=error',
  ];
  console.error(lines.join('\n'));
  process.exit(64);
}

function parseArgs(argv) {
  const opts = { caller: DEFAULT_CALLER, targets: [], json: false, queryParts: [] };
  for (const a of argv) {
    if (a === '--help' || a === '-h') {
      usage();
    } else if (a.startsWith('--caller=')) {
      opts.caller = a.slice('--caller='.length);
    } else if (a.startsWith('--target=')) {
      opts.targets.push(a.slice('--target='.length));
    } else if (a === '--json') {
      opts.json = true;
    } else if (a.startsWith('--')) {
      console.error(`unknown option: ${a}`);
      usage();
    } else {
      opts.queryParts.push(a);
    }
  }
  opts.query = opts.queryParts.join(' ').trim();
  return opts;
}

function postTriage(body) {
  return new Promise((resolve, reject) => {
    const u = new URL('/api/v1/diagnose/triage', SERVER);
    const client = u.protocol === 'https:' ? https : http;
    const payload = JSON.stringify(body);
    const headers = {
      'Content-Type': 'application/json',
      'Content-Length': Buffer.byteLength(payload),
    };
    if (STAFF_TOKEN) {
      headers['Authorization'] = `Bearer ${STAFF_TOKEN}`;
    }
    const req = client.request(
      u.toString(),
      { method: 'POST', timeout: TIMEOUT_MS, headers },
      (res) => {
        let chunks = '';
        res.on('data', (d) => {
          chunks += d;
        });
        res.on('end', () => {
          if (res.statusCode === 401 || res.statusCode === 403) {
            return reject(new Error(`HTTP ${res.statusCode} — set STAFF_TOKEN env var`));
          }
          if (res.statusCode !== 200) {
            return reject(new Error(`HTTP ${res.statusCode}: ${chunks.slice(0, 300)}`));
          }
          try {
            resolve(JSON.parse(chunks));
          } catch (e) {
            reject(new Error(`bad JSON: ${e.message} :: ${chunks.slice(0, 200)}`));
          }
        });
      }
    );
    req.on('timeout', () => {
      req.destroy(new Error(`timeout after ${TIMEOUT_MS}ms`));
    });
    req.on('error', reject);
    req.write(payload);
    req.end();
  });
}

function formatVerdict(query, verdict) {
  const ts = new Date().toISOString();
  const header = `=== TRIAGE VERDICT (${ts}) ===\nQUERY: ${query}\nSERVER: ${SERVER}\nVERDICT: ${verdict.verdict}`;

  switch (verdict.verdict) {
    case 'TRIAGE_FAST':
      return [
        header,
        `BUG_ID: ${verdict.bug_id}`,
        `HIT_TYPE: ${verdict.hit_type}  (confidence ${verdict.confidence})`,
        `FIX_STATUS: ${verdict.fix_status}${verdict.fixed_in_build ? ` (in build ${verdict.fixed_in_build})` : ''}`,
        `AFFECTS: ${(verdict.affects_targets || []).join(', ') || '(unspecified)'}`,
        `MATCHED_PATTERN: "${verdict.matched_pattern}"`,
        `SHOULD_RUN_MMA: ${verdict.should_run_mma}`,
        '---',
        verdict.escalation_message || '(no escalation message)',
        '---',
      ].join('\n');

    case 'AUDIT_DEEP':
      return [
        header,
        `CLASSES: ${(verdict.relevant_symptom_classes || []).join(', ')}`,
        `SHOULD_RUN_MMA: ${verdict.should_run_mma}`,
        `RATIONALE: ${verdict.rationale}`,
        'NOTE: feed relevant_symptom_classes into multi-model-audit.js query-relevant bundle (Commit 3 of spec).',
      ].join('\n');

    case 'RESEARCH_NEW':
      return [
        header,
        `SHOULD_RUN_MMA: ${verdict.should_run_mma}`,
        `RATIONALE: ${verdict.rationale}`,
        'NOTE: novel symptom — consider human review before burning MMA budget.',
      ].join('\n');

    case 'REJECT_AMBIGUOUS':
      return [
        header,
        `SHOULD_RUN_MMA: ${verdict.should_run_mma}`,
        `RATIONALE: ${verdict.rationale}`,
        'CLARIFYING QUESTIONS:',
        ...(verdict.clarifying_questions || []).map((q) => `  - ${q}`),
      ].join('\n');

    default:
      return `${header}\n(unknown verdict shape)\n${JSON.stringify(verdict, null, 2)}`;
  }
}

function exitCodeFor(verdict) {
  if (verdict.verdict === 'REJECT_AMBIGUOUS') return 2;
  if (verdict.verdict === 'TRIAGE_FAST' && !verdict.should_run_mma) return 0;
  return 1; // AUDIT_DEEP, RESEARCH_NEW, or TRIAGE_FAST+should_run_mma
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!opts.query) usage();

  const body = {
    query: opts.query,
    caller_context: opts.caller,
  };
  if (opts.targets.length > 0) body.targets_hint = opts.targets;

  try {
    const verdict = await postTriage(body);
    if (opts.json) {
      console.log(JSON.stringify(verdict, null, 2));
    } else {
      console.log(formatVerdict(opts.query, verdict));
    }
    process.exit(exitCodeFor(verdict));
  } catch (err) {
    const ts = new Date().toISOString();
    console.error(
      `=== TRIAGE ERR (${ts}) ===\nQUERY: ${opts.query}\nSERVER: ${SERVER}\nERROR: ${err.message}`
    );
    process.exit(3);
  }
}

main();
