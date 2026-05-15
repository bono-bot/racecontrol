#!/usr/bin/env node
/**
 * CGP v4.3 Stranded-Branch Detector — UserPromptSubmit (v0.1.0 DRAFT)
 *
 * Sibling to backlog-enforce.js. Backlog gate watches MEMORY for undeployed
 * claims; this hook watches GIT REFS for the same class one step earlier:
 * a feature branch that is ahead of origin/main and has no open PR.
 *
 * Discovered class 2026-05-15 IST: Captain G9 "live kiosk missing V2 design
 * theme" traced to amend1-section1-heart-admin-substrate (3 V2 kiosk commits,
 * never merged, no open PR). Class enumeration found 30 such branches across
 * racecontrol. Standing Rule #16 + Backlog Gate + B8 commit-discipline all
 * existed; none watched git refs. This hook closes that gap.
 *
 * Authored bilateral pending bono AMPLIFIER review.
 * Reference: comms-link briefings inbox-msg-james-2026-05-15-{1114,1137}-IST-*.md
 *
 * Behavior:
 *   - Scans racecontrol + comms-link for unmerged-to-origin/main branches
 *   - Excludes branches with an open PR (when gh CLI + cached PR list available)
 *   - Excludes branches idle <STALE_DAYS (default 3) — fresh work isn't stranded
 *   - Excludes "rescue/", "swaplog/", "phase-373-canary*" transient classes
 *   - Emits non-blocking UserPromptSubmit additionalContext
 *
 * NEVER blocks — this is awareness, not enforcement. Wire mode in settings.json
 * is "UserPromptSubmit" only; do NOT add to PreToolUse.
 */

const fs = require('fs');
const path = require('path');
const os = require('os');
const { execFileSync } = require('child_process');

const STALE_DAYS = 3;
const MAX_BRANCHES_REPORTED = 12;

// Repos to scan. Add new repos here; multi-repo per `feedback_git_C_abspath_multirepo_20260512`.
const REPOS = [
  { name: 'racecontrol', path: path.join(os.homedir(), 'racingpoint', 'racecontrol') },
  { name: 'comms-link', path: path.join(os.homedir(), 'racingpoint', 'comms-link') },
];

// Transient branch classes — never flag these as stranded.
const TRANSIENT_PATTERNS = [
  /^rescue\//,
  /^swaplog\//,
  /^phase-\d+-canary/,
  /^audit\//,
  /^security\/redact-/,  // one-shot redaction branches
];

function git(repoPath, args) {
  try {
    return execFileSync('git', ['-C', repoPath, ...args], {
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
      timeout: 5000,
      windowsHide: true,
    }).trim();
  } catch (_) { return ''; }
}

function loadOpenPRs(repoPath) {
  // Try cached PR list first (refreshed by an external cron, not this hook)
  const cachePath = path.join(repoPath, '.git', 'pr-list-cache.json');
  if (fs.existsSync(cachePath)) {
    try {
      const cache = JSON.parse(fs.readFileSync(cachePath, 'utf8'));
      const ageMs = Date.now() - new Date(cache.fetched_at).getTime();
      if (ageMs < 30 * 60 * 1000) {  // 30 min TTL
        return new Set(cache.head_refs || []);
      }
    } catch (_) {}
  }
  // Fallback: best-effort gh call with 2s timeout — silently empty on miss
  try {
    const out = execFileSync(
      'gh', ['pr', 'list', '--state', 'open', '--limit', '100', '--json', 'headRefName', '-q', '.[].headRefName'],
      { cwd: repoPath, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'], timeout: 2000, windowsHide: true }
    );
    return new Set(out.split('\n').filter(Boolean));
  } catch (_) {
    return null;  // signal: PR check unavailable
  }
}

function scanRepo(repo) {
  if (!fs.existsSync(repo.path)) return { items: [], prCheckSkipped: false };
  const openPRs = loadOpenPRs(repo.path);
  const prCheckSkipped = openPRs === null;

  // for-each-ref + filter ahead-of-origin/main
  const refs = git(repo.path,
    ['for-each-ref', '--format=%(refname:short)|%(committerdate:short)', 'refs/heads/']
  );
  if (!refs) return { items: [], prCheckSkipped };

  const items = [];
  const cutoffMs = Date.now() - STALE_DAYS * 24 * 60 * 60 * 1000;

  for (const line of refs.split('\n')) {
    const m = line.match(/^(.+?)\|(\d{4}-\d{2}-\d{2})$/);
    if (!m) continue;
    const [, branch, dateStr] = m;
    if (branch === 'main') continue;
    if (TRANSIENT_PATTERNS.some(p => p.test(branch))) continue;
    if (openPRs && openPRs.has(branch)) continue;  // has PR → not stranded

    // ahead-of-main check
    const isAncestor = (function() {
      try {
        execFileSync('git', ['-C', repo.path, 'merge-base', '--is-ancestor', branch, 'origin/main'],
          { stdio: 'ignore', timeout: 3000, windowsHide: true });
        return true;
      } catch (_) { return false; }
    })();
    if (isAncestor) continue;

    const ahead = git(repo.path, ['rev-list', '--count', `origin/main..${branch}`]);
    if (!ahead || ahead === '0') continue;

    // staleness — only flag if older than STALE_DAYS
    const dateMs = new Date(dateStr + 'T00:00:00Z').getTime();
    if (dateMs > cutoffMs) continue;

    const lastSubject = git(repo.path,
      ['log', '-1', '--pretty=format:%s', branch]
    ).substring(0, 80);

    items.push({
      repo: repo.name,
      branch,
      date: dateStr,
      ahead: Number(ahead),
      subject: lastSubject,
    });
  }
  return { items, prCheckSkipped };
}

// --- Main ---
let stdinData = '';
try { stdinData = fs.readFileSync(0, 'utf8'); } catch (_) {}

let userPrompt = '';
try {
  const hookInput = JSON.parse(stdinData);
  userPrompt = hookInput?.hookSpecificInput?.userMessage || '';
} catch (_) {}

// Skip background-agent prompts (same convention as backlog-enforce)
try {
  const { isAgentPrompt } = require('./lib/agent-prompt-detect.js');
  if (isAgentPrompt(userPrompt)) process.exit(0);
} catch (_) {}

const allItems = [];
let anyPrCheckSkipped = false;
for (const repo of REPOS) {
  const { items, prCheckSkipped } = scanRepo(repo);
  allItems.push(...items);
  if (prCheckSkipped) anyPrCheckSkipped = true;
}

if (allItems.length === 0) process.exit(0);

// Sort by ahead-count desc (bigger stranded work first), then by date asc (older = staler)
allItems.sort((a, b) => b.ahead - a.ahead || a.date.localeCompare(b.date));

const lines = [];
lines.push(`STRANDED-BRANCH DETECTOR (v0.1.0 DRAFT) — ${allItems.length} branch(es) ahead of origin/main, idle >${STALE_DAYS}d, no open PR:`);
lines.push('');

for (const item of allItems.slice(0, MAX_BRANCHES_REPORTED)) {
  lines.push(`  [${item.repo}] ${item.branch} | ${item.date} | ahead=${item.ahead} | ${item.subject}`);
}
if (allItems.length > MAX_BRANCHES_REPORTED) {
  lines.push(`  ... and ${allItems.length - MAX_BRANCHES_REPORTED} more`);
}

lines.push('');
if (anyPrCheckSkipped) {
  lines.push(`NOTE: gh CLI unreachable for at least one repo — some PR-having branches may appear here. Refresh: \`gh pr list --state open\`.`);
}
lines.push(`Class: COMMITTED-not-SHIPPED. Standing Rule #16 + B8 commit-discipline apply. To inspect a branch: \`git log origin/main..<branch> --name-only\`.`);

const output = {
  hookSpecificOutput: {
    hookEventName: 'UserPromptSubmit',
    additionalContext: lines.join('\n'),
  },
};
process.stdout.write(JSON.stringify(output));
process.exit(0);
