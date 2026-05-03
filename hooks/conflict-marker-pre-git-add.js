#!/usr/bin/env node
/**
 * conflict-marker-pre-git-add.js — PreToolUse Bash hook detecting
 * git-merge conflict markers in files about to be staged or committed.
 *
 * PACT-20260503-005 substrate-fix for G9 #2 ACTIVE doctrine
 * (`feedback_g9_bare_git_add_accepts_conflict_markers.md`).
 *
 * Trigger: PreToolUse on Bash for `git add ...` / `git commit ...` /
 *          `git rebase --continue`.
 * Action:  scan target files for line-anchored conflict markers; if any,
 *          BLOCK (exit 2) with advisory naming the offending file:line.
 *
 * Why this hook exists:
 *   2026-05-03 same-session N=2 anchor — bare `git add` of a previously-
 *   conflicted file accepted the file with `<<<<<<<` / `=======` /
 *   `>>>>>>>` markers intact, then `git commit` + `git push` landed the
 *   broken file in origin/main. Twice (commits c2cf832 + cdab38c).
 *   Both fix-forwarded after-the-fact (commits dcc6130 + 0724c47), but
 *   the manual Read→grep gate is empirically skipped under cascade
 *   pressure. This hook converts the discipline into a runtime gate.
 *
 * Exit codes:
 *   0 = pass-through (no markers found, or non-git command, or exempt path)
 *   2 = BLOCK (markers found in staged file; commit/add aborted)
 *
 * Bypass (rare; use only when committing actual marker examples in docs):
 *   BYPASS_CONFLICT_MARKER_CHECK=1 — explicit env-var override
 *   File-path allowlist: any path matching CONFLICT_MARKER_ALLOWLIST_REGEX
 *
 * Composes-with:
 *   - PACT-002 v2-claim-gate.js (structural model: PreToolUse hook + advisory)
 *   - PACT-026 atomic-publish-guard (INBOX-class atomic-publish primitive)
 *   - inbox-append.js conflict-marker carve-out (same family of guards)
 *   - feedback_g9_bare_git_add_accepts_conflict_markers.md (G9 #2 ACTIVE doctrine)
 *   - CGP H3 evidence-before-claims (verify file content before staging)
 *
 * Captain authorization:
 *   PACT-20260503-005 substrate-class child-PACT under composite-ratify-event
 *   #4 OPTION-A §2.1 cascade pre-ratify Level B; AWAITS bono AMPLIFIER for
 *   §2.1 auto-ratify gate close (MMA ≥4.0 + CGP H3 + V2-MASTER-STATE.md
 *   §S-N append). Settings.json wire-in DEFERRED until §2.1 gate closes.
 */

'use strict';

const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

// Line-anchored conflict-marker patterns. The whitespace-anchored variants
// match real conflict markers but NOT documentation snippets that quote
// markers inline (e.g., a memo discussing `<<<<<<<` in a code-fenced block).
const MARKER_REGEXES = [
  /^<<<<<<<\s/m,    // <<<<<<< followed by whitespace (branch name)
  /^=======$/m,    // ======= alone on line
  /^>>>>>>>\s/m,    // >>>>>>> followed by whitespace (branch name)
];

// Path allowlist: paths matching this regex are exempt from the check.
// Initial seed: documentation discussing conflict markers + the hook itself.
// Extend conservatively — false-positive cost is low, false-negative cost
// is broken commits in origin/main audit-trail files.
const CONFLICT_MARKER_ALLOWLIST_REGEX = new RegExp([
  // The hook itself (which contains marker patterns in regex form)
  'hooks/conflict-marker-pre-git-add\\.js',
  // The G9 #2 doctrine memo (discusses markers in body)
  'feedback_g9_bare_git_add_accepts_conflict_markers\\.md',
  // PACT proposals discussing conflict-marker class
  'proposals/PACT-2026\\d{4}-\\d{3}-conflict-marker',
  // Literal documentation files about conflict resolution
  'docs/.*CONFLICT.*\\.md',
].join('|'));

const BYPASS = process.env.BYPASS_CONFLICT_MARKER_CHECK === '1';

// Read JSON tool-call payload from stdin (Claude Code PreToolUse contract).
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { input += chunk; });
process.stdin.on('end', () => {
  let toolName, toolInput;
  try {
    const parsed = JSON.parse(input);
    toolName = parsed.tool_name || parsed.tool || '';
    toolInput = parsed.tool_input || parsed.input || {};
  } catch {
    process.exit(0); // Malformed payload; pass-through (don't block on parse failure)
  }

  if (toolName !== 'Bash' || !toolInput || typeof toolInput.command !== 'string') {
    process.exit(0);
  }

  if (BYPASS) {
    process.stderr.write('[conflict-marker-pre-git-add] BYPASS_CONFLICT_MARKER_CHECK=1 — skipping check (allowed for marker-example commits)\n');
    process.exit(0);
  }

  const command = toolInput.command;

  // Identify git operations that stage/commit content. We deliberately keep
  // the trigger surface narrow — only commands that move staged content into
  // a commit. `git diff` / `git log` etc. are silent passes.
  //
  // Triggers:
  //   - git add ...
  //   - git commit ... (without `-m '...'` only triggers; with -m we still scan staged)
  //   - git rebase --continue
  //   - git merge --continue
  //   - git cherry-pick --continue
  const ADD_TRIGGER = /\bgit\s+add\b/;
  const COMMIT_TRIGGER = /\bgit\s+commit\b/;
  const RESUME_TRIGGER = /\bgit\s+(rebase|merge|cherry-pick|am)\s+--continue\b/;

  const isAdd = ADD_TRIGGER.test(command);
  const isCommit = COMMIT_TRIGGER.test(command);
  const isResume = RESUME_TRIGGER.test(command);

  if (!isAdd && !isCommit && !isResume) {
    process.exit(0);
  }

  // Identify candidate files to check.
  // Strategy:
  //   - For `git add <paths>`: extract the paths from the command line
  //     directly. (Globs like `git add .` or `git add -A` we resolve via
  //     `git diff --name-only` of the working-tree-vs-HEAD; for `git add -p`
  //     we fall back to staged content.)
  //   - For `git commit ...` and resume verbs: scan `git diff --cached`
  //     content (already-staged content about to be committed).
  //
  // We do NOT shell out to `git ls-files` etc. for performance and to keep
  // the hook side-effect-free. Worst-case false-negative: a path expansion
  // we can't resolve gets passed through. Net-net the hook covers the two
  // empirical N=1+N=2 anchor cases (bare `git add <file>` + post-rebase
  // `git rebase --continue`), which is the structural goal.

  const filesToCheck = [];
  const cwd = toolInput.cwd || process.cwd();

  if (isAdd) {
    // Parse `git add <args>`. Strip flags, keep paths.
    // Handle `git add .` and `git add -A` / `--all` by scanning all changed files.
    const afterAdd = command.split(/\bgit\s+add\b/)[1] || '';
    const tokens = afterAdd.trim().split(/\s+/).filter(Boolean);

    const wantsEverything = tokens.some(t => t === '.' || t === '-A' || t === '--all' || t === '-u' || t === '--update');
    const explicitPaths = tokens.filter(t => !t.startsWith('-'));

    if (wantsEverything) {
      // Scan all working-tree changes vs HEAD that are about to be staged.
      try {
        const out = execSync('git diff --name-only HEAD', { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
        out.split('\n').filter(Boolean).forEach(p => filesToCheck.push(p));
      } catch {
        /* not a git repo or no HEAD; skip silently */
      }
    } else {
      explicitPaths.forEach(p => filesToCheck.push(p));
    }
  } else {
    // commit / resume — scan currently-staged content via `git diff --cached --name-only`
    try {
      const out = execSync('git diff --cached --name-only', { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] });
      out.split('\n').filter(Boolean).forEach(p => filesToCheck.push(p));
    } catch {
      /* not a git repo or no staged content; pass-through */
      process.exit(0);
    }
  }

  if (filesToCheck.length === 0) {
    process.exit(0);
  }

  // Scan each candidate file for line-anchored markers.
  const offenders = [];
  for (const relPath of filesToCheck) {
    if (CONFLICT_MARKER_ALLOWLIST_REGEX.test(relPath)) {
      continue; // exempt
    }
    const absPath = path.isAbsolute(relPath) ? relPath : path.join(cwd, relPath);
    let content;
    try {
      content = fs.readFileSync(absPath, 'utf8');
    } catch {
      continue; // file missing / not readable; skip silently
    }
    for (const re of MARKER_REGEXES) {
      const match = content.match(re);
      if (match) {
        // Find the line number of the FIRST marker (cheap; not all matches)
        let lineNum = 1;
        const idx = content.indexOf(match[0]);
        if (idx >= 0) {
          lineNum = content.slice(0, idx).split('\n').length;
        }
        offenders.push({ file: relPath, line: lineNum, marker: match[0].trim() });
        break;
      }
    }
  }

  if (offenders.length === 0) {
    process.exit(0); // all clean; pass through
  }

  // BLOCK with advisory.
  const lines = [];
  lines.push('');
  lines.push('┌─────────────────────────────────────────────────────────────────┐');
  lines.push('│ CONFLICT MARKERS DETECTED — git stage/commit BLOCKED            │');
  lines.push('└─────────────────────────────────────────────────────────────────┘');
  lines.push(`Hook: conflict-marker-pre-git-add.js v0.1 (PACT-20260503-005)`);
  lines.push(`Why:  G9 #2 ACTIVE doctrine — bare \`git add\` accepts conflict`);
  lines.push(`      markers as resolution. Anchor commits c2cf832 (pact-slots.jsonl)`);
  lines.push(`      + cdab38c (INBOX.md), both fix-forwarded after origin/main pollution.`);
  lines.push('');
  lines.push('Offending file(s):');
  for (const o of offenders) {
    lines.push(`  - ${o.file}:${o.line} contains "${o.marker}..."`);
  }
  lines.push('');
  lines.push('Resolution:');
  lines.push('  1. Read each conflicted file; find conflict regions');
  lines.push('  2. Edit to remove the <<<<<<< / ======= / >>>>>>> markers + decide which side(s) to keep');
  lines.push('  3. Verify: grep -c \'^<<<<<<<\\|^=======$\\|^>>>>>>>\' <file>  →  must return 0');
  lines.push('  4. Re-run the original git command');
  lines.push('');
  lines.push('Bypass (rare — only for legitimate marker-example commits in docs):');
  lines.push('  BYPASS_CONFLICT_MARKER_CHECK=1 git ...');
  lines.push('');

  process.stderr.write(lines.join('\n') + '\n');
  process.exit(2);
});
