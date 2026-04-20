#!/usr/bin/env node
/**
 * workflow-monitor.js — Bono's Day-2 pm2 daemon.
 *
 * Every tick:
 *   1. ff-only pull /root/racecontrol + /root/comms-link (skip tick on conflict,
 *      not reset --hard — preserves local in-flight Bono session state).
 *   2. Run scripts/audit/run-all-checkers.sh. Emits .planning/board-state/status.json.
 *   3. Diff vs .planning/board-state/status-prev.json; detect red↔green transitions
 *      AND new red checks.
 *   4. On transition: append INBOX entry (from bono) + commit status files authored
 *      as "bono" so git co-change history carries the transitions as first-class.
 *   5. Overwrite status-prev.json with current status.
 *
 * Registered via: pm2 start scripts/monitor/workflow-monitor.js \
 *   --name workflow-monitor --cron '*\/5 * * * *' --no-autorestart
 *
 * DUTY 2 (Perplexity MCP second-opinion on new reds) and DUTY 3 (MMA VERIFY on
 * James's cross-system commits) are stubbed at their extension points below.
 * See docs/BONO-WORKFLOW-MONITOR.md for the full spec.
 *
 * All shell-outs use execFileSync (no shell) to prevent command injection from
 * generator output embedded in commit messages.
 */

'use strict';
const { execFileSync } = require('node:child_process');
const { readFileSync, writeFileSync, existsSync, copyFileSync, mkdirSync } = require('node:fs');
const { dirname, join } = require('node:path');

const RACECONTROL  = '/root/racecontrol';
const COMMSLINK    = '/root/comms-link';
const STATUS_PATH  = join(RACECONTROL, '.planning/board-state/status.json');
const PREV_PATH    = join(RACECONTROL, '.planning/board-state/status-prev.json');
const INBOX_APPEND = join(COMMSLINK, 'scripts/inbox-append.js');
const LOG_PATH     = '/root/.claude/projects/-root/workflow-monitor.log';

function logLine(msg) {
  const ts = new Date().toISOString();
  const line = `[${ts}] ${msg}\n`;
  process.stderr.write(line);
  try {
    mkdirSync(dirname(LOG_PATH), { recursive: true });
    writeFileSync(LOG_PATH, line, { flag: 'a' });
  } catch { /* log-write best-effort */ }
}

function runFile(file, args, opts = {}) {
  return execFileSync(file, args, { stdio: ['ignore', 'pipe', 'pipe'], ...opts })
    .toString().trim();
}

function tryPullFfOnly(repo) {
  try {
    runFile('git', ['fetch', '--quiet'], { cwd: repo });
    runFile('git', ['pull', '--ff-only', '--quiet'], { cwd: repo });
    return { ok: true };
  } catch (e) {
    return { ok: false, err: String(e.message).split('\n')[0] };
  }
}

function runOrchestrator() {
  try {
    const out = execFileSync('bash', ['scripts/audit/run-all-checkers.sh'],
      { cwd: RACECONTROL, stdio: ['ignore', 'pipe', 'pipe'] }).toString();
    return { exit: 0, out };
  } catch (e) {
    // Exit 1 (red) and exit 2 (generator error) are expected outcomes, not crashes.
    return { exit: e.status ?? -1, out: (e.stdout ?? '').toString() };
  }
}

function loadJson(path) {
  if (!existsSync(path)) return null;
  try { return JSON.parse(readFileSync(path, 'utf8')); }
  catch (e) { logLine(`json parse failed for ${path}: ${e.message}`); return null; }
}

function diffChecks(prev, curr) {
  const transitions = [];
  const prevMap = new Map((prev?.checks ?? []).map(c => [c.name, c]));
  const currMap = new Map((curr?.checks ?? []).map(c => [c.name, c]));

  for (const [name, c] of currMap) {
    const p = prevMap.get(name);
    if (!p) {
      if (c.status === 'red') transitions.push({ name, from: 'new', to: 'red', check: c });
      continue;
    }
    if (p.status !== c.status) transitions.push({ name, from: p.status, to: c.status, check: c });
  }

  const prevExit = prev?.overall_exit;
  const currExit = curr?.overall_exit;
  const overallFlip = (prev !== null && prevExit !== currExit)
    ? { from: prevExit, to: currExit } : null;

  return { transitions, overallFlip };
}

function gapCount(check) {
  const m = (check?.output ?? '').match(/GAPS\s*\((\d+)\)/);
  return m ? parseInt(m[1], 10) : 0;
}

function formatInboxBody(overallFlip, transitions, commit) {
  const lines = [];
  if (overallFlip) {
    const arrow = overallFlip.from === 0 && overallFlip.to !== 0 ? 'GREEN → RED'
                : overallFlip.from !== 0 && overallFlip.to === 0 ? 'RED → GREEN'
                : `exit ${overallFlip.from} → ${overallFlip.to}`;
    lines.push(`**Overall: ${arrow}** at racecontrol \`${commit}\`.`);
    lines.push('');
  }
  if (transitions.length) {
    lines.push('**Check transitions:**');
    for (const t of transitions) {
      const g = gapCount(t.check);
      lines.push(`- \`${t.name}\`: ${t.from} → ${t.to}` + (g ? ` (${g} gaps)` : ''));
    }
    lines.push('');
  }
  lines.push('_Posted by pm2 workflow-monitor (Duty 1). Full status in `.planning/board-state/status.json`._');
  return lines.join('\n');
}

function appendInbox(body) {
  try {
    execFileSync('node', [INBOX_APPEND, '--from', 'bono', '--stdin'],
      { cwd: COMMSLINK, input: body, stdio: ['pipe', 'pipe', 'pipe'] });
    return { ok: true };
  } catch (e) {
    return { ok: false, err: String(e.message).split('\n')[0] };
  }
}

function commitAsBonoWithStdinMsg(repo, paths, msg) {
  try {
    runFile('git', ['add', ...paths], { cwd: repo });
    const staged = runFile('git', ['diff', '--cached', '--stat'], { cwd: repo });
    if (!staged) return { ok: true, skipped: true };
    // -F - reads message from stdin: no shell, no length limits, no escaping needed.
    execFileSync('git',
      ['-c', 'user.name=bono', '-c', 'user.email=bono@racingpoint.in',
       'commit', '-F', '-'],
      { cwd: repo, input: msg, stdio: ['pipe', 'pipe', 'pipe'] });
    runFile('git', ['push'], { cwd: repo });
    return { ok: true, skipped: false };
  } catch (e) {
    return { ok: false, err: String(e.message).split('\n')[0] };
  }
}

function tick() {
  logLine('tick start');

  const rcPull = tryPullFfOnly(RACECONTROL);
  const clPull = tryPullFfOnly(COMMSLINK);
  if (!rcPull.ok) { logLine(`rc pull skip: ${rcPull.err}`); return; }
  if (!clPull.ok) { logLine(`cl pull skip: ${clPull.err}`); return; }

  const orch = runOrchestrator();
  logLine(`orchestrator exit=${orch.exit}`);
  if (orch.exit === 2) {
    const body = `**Generator error** — orchestrator exit 2. Source tree unparseable.\n\n\`\`\`\n${orch.out.slice(0, 2000)}\n\`\`\``;
    appendInbox(body);
    commitAsBonoWithStdinMsg(COMMSLINK, ['INBOX.md'], 'inbox: workflow-monitor generator-error alert');
    return;
  }

  const current = loadJson(STATUS_PATH);
  if (!current) { logLine('no current status.json — aborting'); return; }
  const previous = loadJson(PREV_PATH);

  if (previous === null) {
    // First run: establish baseline silently. Any existing red is pre-existing,
    // not a transition. Next tick will see actual changes.
    copyFileSync(STATUS_PATH, PREV_PATH);
    logLine('first-run baseline written; no alert');
    return;
  }

  const { transitions, overallFlip } = diffChecks(previous, current);

  if (!overallFlip && !transitions.length) {
    copyFileSync(STATUS_PATH, PREV_PATH);
    logLine('no transitions');
    return;
  }

  const body = formatInboxBody(overallFlip, transitions, current.commit ?? 'unknown');
  const inbox = appendInbox(body);
  logLine(`inbox append: ${inbox.ok ? 'ok' : 'FAIL: ' + inbox.err}`);

  const statusCommit = commitAsBonoWithStdinMsg(
    RACECONTROL,
    ['.planning/board-state/status.json', '.planning/board-state/status-prev.json'],
    `board-state: transition @ ${current.commit ?? '?'}\n\n${body}`);
  logLine(`status commit: ${statusCommit.ok ? (statusCommit.skipped ? 'skipped' : 'ok') : 'FAIL: ' + statusCommit.err}`);

  const inboxCommit = commitAsBonoWithStdinMsg(
    COMMSLINK, ['INBOX.md'],
    'inbox: workflow-monitor transition alert');
  logLine(`inbox commit: ${inboxCommit.ok ? (inboxCommit.skipped ? 'skipped' : 'ok') : 'FAIL: ' + inboxCommit.err}`);

  copyFileSync(STATUS_PATH, PREV_PATH);

  // --- Duty 2 extension point: Perplexity MCP second-opinion on new red checks.
  //     for (const t of transitions.filter(t => t.to === 'red')) { queuePerplexityReview(t); }
  //     One specialist per class (auth|cascade|drift|hw), $1/hr budget cap, log
  //     to memory/mma-log.jsonl. Appends response to the same INBOX entry on return.
  //     Will ship in a follow-up commit.

  // --- Duty 3 extension point: MMA Step 4 VERIFY on cross-system commits.
  //     Triggered when pulled commits touch customer_legal.rs, rc-agent ws_handler,
  //     server ws/*.rs, or kiosk↔rc-agent↔server bridges. 3 adversarial models,
  //     score <4.0 → RED alert. Will ship in a follow-up commit.

  logLine('tick end');
}

tick();
