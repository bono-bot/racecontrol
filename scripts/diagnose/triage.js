#!/usr/bin/env node
// James-facing triage wrapper over /api/v1/mesh/audit-check.
// Usage: node scripts/diagnose/triage.js "pod 3 iracing steam dialog"
// Env:   RC_SERVER_URL (default http://192.168.31.23:8080)
//        STAFF_TOKEN        (required — route sits in staff-JWT block)
//        TRIAGE_TIMEOUT_MS  (default 5000)
//
// Behaviour:
//   HIT  -> prints VENUE_REQUIRED block, exits 0
//   MISS -> prints NO_MATCH (falls through to MMA), exits 1
//   ERR  -> prints ERR line, exits 2
//
// ARCH FINDING 2026-04-18: rc-agent's check_audit_known_issues in
// crates/rc-agent/src/ai_debugger.rs sends this same GET with NO auth headers.
// The route was moved to staff-JWT block in MMA-v29 ("Metrics, mesh, admin,
// cameras moved from public_routes"). Every rc-agent Tier 0 call has been
// silently 401'ing since that move. The intended autonomous short-circuit is
// dead in production. Fix options: (A) move /mesh/audit-check back to public
// routes, (B) add X-Service-Key auth and teach rc-agent to send it. Decision
// pending with user. See MEMORY.md handoff.
//
// Known limitation: /audit-check returns first match only, not a ranked list.
// If a query matches multiple audit entries, the caller sees one escalation
// message with no indication others matched. Fix path if this hurts UX: add
// /audit-check-all on server returning Vec<Match>, client-side rank by
// pattern specificity.

const http = require('node:http');
const { URL } = require('node:url');

const SERVER = process.env.RC_SERVER_URL || 'http://192.168.31.23:8080';
const TIMEOUT_MS = parseInt(process.env.TRIAGE_TIMEOUT_MS || '5000', 10);
const STAFF_TOKEN = process.env.STAFF_TOKEN || '';

function usage() {
  console.error('usage: node scripts/diagnose/triage.js "<symptom query>"');
  console.error('env:   RC_SERVER_URL (default http://192.168.31.23:8080)');
  process.exit(64);
}

function fetchAuditCheck(symptom) {
  return new Promise((resolve, reject) => {
    const u = new URL('/api/v1/mesh/audit-check', SERVER);
    u.searchParams.set('symptom', symptom);
    const headers = STAFF_TOKEN ? { Authorization: `Bearer ${STAFF_TOKEN}` } : {};
    const req = http.get(u.toString(), { timeout: TIMEOUT_MS, headers }, (res) => {
      let chunks = '';
      res.on('data', (d) => { chunks += d; });
      res.on('end', () => {
        if (res.statusCode !== 200) {
          return reject(new Error(`HTTP ${res.statusCode}: ${chunks.slice(0, 200)}`));
        }
        try { resolve(JSON.parse(chunks)); }
        catch (e) { reject(new Error(`bad JSON: ${e.message} :: ${chunks.slice(0, 200)}`)); }
      });
    });
    req.on('timeout', () => { req.destroy(new Error(`timeout after ${TIMEOUT_MS}ms`)); });
    req.on('error', reject);
  });
}

function formatVerdict(query, body) {
  const ts = new Date().toISOString();
  const header = `=== TRIAGE VERDICT (${ts}) ===\nQUERY: ${query}\nSERVER: ${SERVER}`;
  if (body && body.matched === true) {
    return [
      header,
      'VERDICT: VENUE_REQUIRED',
      `ACTION: ${body.action || 'skip_diagnosis'}`,
      '---',
      body.escalation || '(no escalation message)',
      '---',
      'NOTE: /audit-check returns first match only; additional entries may have matched.',
    ].join('\n');
  }
  return [
    header,
    'VERDICT: NO_MATCH',
    'ACTION: fall_through_to_mma',
    'NOTE: if you expected a hit, check (a) seeder ran against this server, (b) symptom_patterns short enough to substring-match query.',
  ].join('\n');
}

async function main() {
  const query = process.argv.slice(2).join(' ').trim();
  if (!query) usage();
  try {
    const body = await fetchAuditCheck(query);
    console.log(formatVerdict(query, body));
    process.exit(body && body.matched === true ? 0 : 1);
  } catch (err) {
    console.error(`=== TRIAGE ERR ===\nQUERY: ${query}\nSERVER: ${SERVER}\nERROR: ${err.message}`);
    process.exit(2);
  }
}

main();
