#!/usr/bin/env node
// scripts/deploy/bug-tracker-mi-seed.js
//
// Seeds BUG-TRACKER.md entries into audit_known_issues so MI Tier 0
// can short-circuit ops triage on documented bugs. Complements
// deploy-mi-seed.sh (which only covers deploy-gap entries).
//
// Background: SR #15 says "audit findings feed Meshed Intelligence."
// In practice only deploy gaps were seeded; INV-*, V-*, ZL-*, GLC-*, S-*
// from BUG-TRACKER.md had no path into the DB. Result: when ops queries
// arrive ("why does pod_3 iRacing fail?"), Tier 0 finds nothing and
// MMA gets invoked on symptoms-only — exactly the FM-4 failure mode.
//
// Usage:
//   STAFF_TOKEN="..." node scripts/deploy/bug-tracker-mi-seed.js
//   RCAGENT_SERVICE_KEY="..." node scripts/deploy/bug-tracker-mi-seed.js
//   SERVER_URL=http://192.168.31.23:8080 STAFF_TOKEN="..." node scripts/deploy/bug-tracker-mi-seed.js
//   DRY_RUN=1 node scripts/deploy/bug-tracker-mi-seed.js   # print payload, don't POST
//
// Endpoint: POST /api/v1/mesh/audit-seed{-service}
// Idempotency: backed by problem_key UNIQUE constraint + INSERT-or-update
//              behavior in mesh_audit_seed_handler. Safe to re-run.

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');

// ─── Config ──────────────────────────────────────────────────────────────
const SERVER_URL = process.env.SERVER_URL || 'http://192.168.31.23:8080';
const REPO_ROOT = path.resolve(__dirname, '..', '..');
const BUG_TRACKER = path.join(REPO_ROOT, 'BUG-TRACKER.md');
const DRY_RUN = process.env.DRY_RUN === '1';
const SERVICE_KEY = process.env.RCAGENT_SERVICE_KEY;
const STAFF_TOKEN = process.env.STAFF_TOKEN;

// IST date — Git Bash TZ=Asia/Kolkata silently fails on Windows
function istDate() {
  const now = new Date();
  const istMs = now.getTime() + (5.5 * 60 * 60 * 1000);
  return new Date(istMs).toISOString().slice(0, 10);
}

// ─── Status normalization (BUG-TRACKER → audit_known_issues.fix_status) ──
const STATUS_MAP = {
  'OPEN': 'pending',
  'NEEDS_INVESTIGATION': 'pending',
  'NEEDS_VENUE_VERIFY': 'code_fixed',
  'EXPECTED_SELF_RESOLVE': 'code_fixed',
  'CODE_FIXED': 'code_fixed',
  'DEPLOYED': 'deployed',
  'VERIFIED': 'verified',
  'CLOSED': 'verified',
  'BY_DESIGN': 'by_design',
  'BLOCKED': 'blocked',
};

function normalizeStatus(raw) {
  if (!raw) return 'pending';
  const cleaned = raw.replace(/\*\*/g, '').trim().toUpperCase();
  return STATUS_MAP[cleaned] || 'pending';
}

// ─── Severity normalization ──────────────────────────────────────────────
function normalizeSeverity(raw) {
  if (!raw) return 'P3';
  const m = raw.replace(/\*\*/g, '').trim().toUpperCase().match(/P[0-3]/);
  return m ? m[0] : 'P3';
}

// ─── Target extraction from free text ────────────────────────────────────
const ALL_PODS = ['pod_1', 'pod_2', 'pod_3', 'pod_4', 'pod_5', 'pod_6', 'pod_7', 'pod_8'];

function extractTargets(text) {
  if (!text) return [];
  const lower = text.toLowerCase();
  const targets = new Set();
  const excluded = new Set();

  // "all except Pod N" / "except Pod N" — defines an EXCLUSION, not a target.
  // Without this, "7 pods (all except Pod 8)" extracts pod_8 (wrong direction).
  const exceptMatches = lower.match(/except\s+pod[\s_-]?(\d)/g) || [];
  for (const m of exceptMatches) {
    const n = m.match(/\d/)[0];
    excluded.add(`pod_${n}`);
  }
  // If exclusions found, start with all pods minus the excluded.
  if (exceptMatches.length > 0) {
    for (const p of ALL_PODS) {
      if (!excluded.has(p)) targets.add(p);
    }
  }

  // Explicit pod refs (skip if excluded).
  const podMatches = lower.match(/pod[\s_-]?(\d)/g) || [];
  for (const m of podMatches) {
    const n = m.match(/\d/)[0];
    const pid = `pod_${n}`;
    if (n >= '1' && n <= '8' && !excluded.has(pid)) targets.add(pid);
  }

  // Aggregate phrasings (still respect exclusions).
  if (/all\s+(8\s+)?pods|pods?\s*1\s*-\s*8|fleet/.test(lower)) {
    for (const p of ALL_PODS) {
      if (!excluded.has(p)) targets.add(p);
    }
  }
  if (/server|\.23\b|racecontrol/.test(lower)) targets.add('server_23');
  if (/vps|cloud|bono/.test(lower)) targets.add('cloud_bono_vps');
  if (/pos\b|\.130\b|\.20\b/.test(lower)) targets.add('pos_130');
  if (/comms.?link/.test(lower)) targets.add('comms_link');

  return [...targets];
}

// ─── Commit hash extraction ──────────────────────────────────────────────
function extractCommitHash(text) {
  if (!text) return null;
  const m = text.match(/`?\b([0-9a-f]{7,12})\b`?/);
  return m ? m[1] : null;
}

// ─── Symptom-pattern extraction ──────────────────────────────────────────
// Emits short tokens (1-3 words, <=25 chars) so fleet_kb.check_audit_known_issues
// substring match (query.contains(pattern)) hits realistic James queries.
// Match direction: the stored pattern must be a substring of the query — so
// patterns MUST be shorter than typical queries (~20-40 chars).
const DOMAIN_KEYWORDS = [
  // subsystems / files
  'steam dialog', 'python.ini', 'zero laps', 'maintenance_mode',
  'apps preset', 'racecontrol plugin', 'lock screen', 'blanking',
  'staff pin', 'kiosk allowlist', 'process guard', 'pre-flight', 'preflight',
  // processes / components
  'orphan process', 'orphan cleanup', 'orphan session', 'orphan refund',
  'werfault', 'werreport', 'conspitlink', 'ffb driver', 'wheelbase',
  'content manager', 'rc-agent', 'rc-sentry', 'racecontrol.exe',
  // games
  'ac rally', 'ac evo', 'le mans', 'f1 25', 'forza', 'iracing',
  'automobilista', 'assetto corsa',
  // symptoms
  'never launches', 'never loads', 'never clears', 'never fires',
  'hang at launch', 'crash on launch', 'launch timeout', 'launch failure',
  'stopping state', 'launching state', 'trial auto-end', 'billing loop',
  'session stuck', 'refund', 'ws disconnect', 'ws instability',
  'path not found', 'silent fail', 'exits unexpectedly',
  'game process exited', 'exited unexpectedly', 'no exit code',
  'no logs', 'dial tone', 'green dot', 'red dot',
];

function extractSymptomPatterns(id, bugDesc, analysis) {
  const patterns = new Set();
  const combined = `${bugDesc || ''} ${analysis || ''}`;
  const lower = combined.toLowerCase();

  // 1. Bug ID tag (lowercase, dashed). James may type "inv-9" or "zl-1".
  if (id) patterns.add(id.toLowerCase());

  // 2. Error codes and structured tokens. Do NOT regex-extract bug-IDs from the
  //    combined text — that pulls cross-referenced IDs (INV-3's analysis mentions
  //    V-2 and INV-1) into this entry's patterns, causing false-positive matches
  //    when James queries a different bug. Step 1 already adds this entry's own ID.
  const regexes = [
    /\bexit\s*code\s*\d+\b/g,
    /\bos\s*error\s*\d+\b/g,
    /\b0x[0-9a-f]{4,10}\b/g,
    /\bport\s*\d+\b/g,
  ];
  for (const re of regexes) {
    for (const m of lower.match(re) || []) {
      patterns.add(m.replace(/\s+/g, ' ').trim());
    }
  }

  // 3. Domain keyword whitelist (high-precision semantic terms).
  for (const kw of DOMAIN_KEYWORDS) {
    if (lower.includes(kw)) patterns.add(kw);
  }

  // 4. Short 2-3 word chunks from bug title as fallback coverage.
  //    Split only on em-dash/en-dash/colon (NOT hyphen — preserves compounds
  //    like "pre-flight", "trial auto-end").
  if (bugDesc) {
    const cleaned = bugDesc
      .replace(/\*\*/g, '')
      .replace(/`/g, '')
      .replace(/\([^)]*\)/g, '')
      .replace(/\[[^\]]*\]/g, '')
      .toLowerCase();
    for (const chunk of cleaned.split(/—|–|:|\u2014/)) {
      const t = chunk.trim().replace(/[,\.]/g, '').replace(/\s+/g, ' ');
      const words = t.split(' ').filter(Boolean);
      // Keep only 2-4 word phrases under 25 chars. Longer = won't substring-hit.
      if (t.length >= 6 && t.length <= 25 && words.length >= 2 && words.length <= 4) {
        patterns.add(t);
      }
    }
  }

  // 5. Fallback: if nothing extracted, take first 2-3 content words of bug.
  if (patterns.size === 0 && bugDesc) {
    const words = bugDesc.toLowerCase().replace(/[^a-z0-9\s\-\.]/g, '').split(/\s+/).filter(w => w.length >= 3);
    if (words.length >= 2) patterns.add(words.slice(0, 3).join(' ').slice(0, 25));
  }

  return [...patterns].slice(0, 15);
}

// ─── Markdown table parser ───────────────────────────────────────────────
function parseTables(md) {
  const lines = md.split('\n');
  const tables = [];
  let current = null;

  for (const line of lines) {
    const isTableRow = /^\s*\|.*\|\s*$/.test(line);
    if (isTableRow) {
      if (!current) current = [];
      current.push(line);
    } else if (current) {
      if (current.length >= 3) tables.push(current);
      current = null;
    }
  }
  if (current && current.length >= 3) tables.push(current);

  return tables.map(tableLines => {
    const cells = tableLines.map(line =>
      line.trim().replace(/^\||\|$/g, '').split('|').map(c => c.trim())
    );
    const headers = cells[0].map(h => h.toLowerCase().replace(/[^a-z0-9 ]/g, '').trim());
    const rows = cells.slice(2);
    return { headers, rows };
  });
}

// ─── Per-row extractor (handles all BUG-TRACKER table variants) ──────────
function extractEntry(headers, row) {
  const get = (...names) => {
    for (const name of names) {
      const idx = headers.findIndex(h => h.includes(name));
      if (idx >= 0 && row[idx]) return row[idx];
    }
    return '';
  };

  const id = get('id').replace(/\*\*/g, '').replace(/`/g, '').trim();
  if (!/^[A-Z]+-\d+$/.test(id)) return null;

  const bug = get('bug');
  if (!bug) return null;

  const severity = normalizeSeverity(get('severity'));
  const status = normalizeStatus(get('status'));
  const fixCommit = get('fix commit', 'fixed by') || extractCommitHash(get('notes', 'fix') || bug);
  const fixedInBuild = extractCommitHash(fixCommit) || '';

  // Different table layouts use different column names for analysis vs operational notes.
  // 'analysis' is the dedicated analysis column (only INV tables have it).
  // 'notes' often contains a mix of analysis + deploy status; treat as supplemental.
  const analysisText = get('analysis') || '';
  const notesText = get('notes') || '';
  const fixDescription = get('next step', 'verify how', 'fix') || notesText ||
                          'See BUG-TRACKER.md for details.';

  // Pod refs frequently live in the Bug column ("1 crash on Pod 8", "Pods 3+4").
  // Include Bug + Notes in the target extraction sources to avoid the all-pods fallback.
  const targetSources = [
    get('pods', 'pods top 3'),
    get('deployed to'),
    get('affects'),
    bug,
    analysisText,
    notesText,
  ].join(' ');
  const affects = extractTargets(targetSources);
  if (affects.length === 0) affects.push('server_23', ...ALL_PODS);

  // Symptom patterns: prefer Bug column + analysis; fall back to notes if no analysis.
  const symptomSourceText = analysisText || notesText;
  const symptomPatterns = extractSymptomPatterns(id, bug, symptomSourceText);
  if (symptomPatterns.length === 0) symptomPatterns.push(bug.toLowerCase().slice(0, 25));

  // Root cause: Bug column IS the canonical symptom; prepend it so the field always
  // names the problem before adding any analysis/notes detail.
  const bugClean = bug.replace(/\*\*/g, '');
  const rootCauseDetail = analysisText.replace(/\*\*/g, '') || notesText.replace(/\*\*/g, '');
  const rootCause = (rootCauseDetail
    ? `${bugClean}. ${rootCauseDetail}`
    : bugClean).slice(0, 800);

  const escalation = `[${id}] ${bugClean.slice(0, 120)} — ${fixDescription.replace(/\*\*/g, '').slice(0, 200)}`;

  return {
    problem_key: `bug_tracker:${id}`,
    severity,
    symptom_patterns: symptomPatterns,
    root_cause: rootCause,
    fix_description: fixDescription.replace(/\*\*/g, '').slice(0, 800),
    fix_status: status,
    fixed_in_build: fixedInBuild,
    affects: affects,
    audit_date: istDate(),
    escalation_message: escalation,
  };
}

// ─── Main ────────────────────────────────────────────────────────────────
function main() {
  if (!fs.existsSync(BUG_TRACKER)) {
    console.error(`ERROR: BUG-TRACKER.md not found at ${BUG_TRACKER}`);
    process.exit(1);
  }

  const md = fs.readFileSync(BUG_TRACKER, 'utf-8');
  const tables = parseTables(md);
  console.error(`Parsed ${tables.length} markdown tables from BUG-TRACKER.md`);

  const findings = [];
  const seen = new Set();

  for (const table of tables) {
    for (const row of table.rows) {
      const entry = extractEntry(table.headers, row);
      if (!entry) continue;
      if (seen.has(entry.problem_key)) continue;
      seen.add(entry.problem_key);
      findings.push(entry);
    }
  }

  console.error(`Extracted ${findings.length} unique BUG-TRACKER entries`);
  if (findings.length === 0) {
    console.error('No entries extracted — check BUG-TRACKER.md structure or parser logic.');
    process.exit(1);
  }

  const payload = { findings };

  if (DRY_RUN) {
    console.log(JSON.stringify(payload, null, 2));
    console.error('\n[DRY RUN] No POST sent. Set DRY_RUN=0 (or unset) to send.');
    return;
  }

  let endpoint, headers;
  if (SERVICE_KEY) {
    endpoint = `${SERVER_URL}/api/v1/mesh/audit-seed-service`;
    headers = { 'Content-Type': 'application/json', 'X-Service-Key': SERVICE_KEY };
  } else if (STAFF_TOKEN) {
    endpoint = `${SERVER_URL}/api/v1/mesh/audit-seed`;
    headers = { 'Content-Type': 'application/json', 'Authorization': `Bearer ${STAFF_TOKEN}` };
  } else {
    console.error('ERROR: Set RCAGENT_SERVICE_KEY or STAFF_TOKEN to authenticate against MI seed endpoint.');
    process.exit(1);
  }

  console.error(`POSTing ${findings.length} findings to ${endpoint} ...`);

  const url = new URL(endpoint);
  const lib = url.protocol === 'https:' ? https : http;
  const body = JSON.stringify(payload);

  const req = lib.request({
    hostname: url.hostname,
    port: url.port,
    path: url.pathname,
    method: 'POST',
    headers: { ...headers, 'Content-Length': Buffer.byteLength(body) },
  }, (res) => {
    let data = '';
    res.on('data', chunk => data += chunk);
    res.on('end', () => {
      console.error(`HTTP ${res.statusCode}`);
      console.log(data);
      if (res.statusCode !== 200) process.exit(1);
    });
  });

  req.on('error', (err) => {
    console.error(`Request failed: ${err.message}`);
    process.exit(1);
  });

  req.setTimeout(30000, () => {
    req.destroy();
    console.error('Request timed out after 30s');
    process.exit(1);
  });

  req.write(body);
  req.end();
}

main();
