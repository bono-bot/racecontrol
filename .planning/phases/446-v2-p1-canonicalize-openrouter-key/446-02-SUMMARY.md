---
phase: 446
plan: "446-02"
subsystem: whatsapp-bot
tags: [canonicalize, env-var, dual-read, deprecation-warn, security, js]
dependency_graph:
  requires: []
  provides: [CANON-446-02, CANON-446-04]
  affects: [whatsapp-bot/src/services/claudeService.js, Bono-VPS-pm2-whatsapp-bot]
tech_stack:
  added: []
  patterns: [canonical-first-IIFE, one-shot-deprecation-warn]
key_files:
  created: []
  modified:
    - whatsapp-bot/src/services/claudeService.js
decisions:
  - "Keep local const name OPENROUTER_API_KEY (not renamed to OPENROUTER_KEY) — line 69 Authorization header stays byte-identical; Phase 448 owns central typed loader"
  - "Extend redaction regex to /OPENROUTER(_API)?_KEY/g — covers both spellings, explicitly excludes OPENROUTER_MGMT_KEY (out of scope)"
  - "console.warn (not logger.warn) in IIFE — logger import available but using console.warn avoids any future logger-init race at module top-level"
metrics:
  duration: "~8 minutes"
  completed: "2026-04-21T11:25Z"
  tasks_completed: 4
  files_modified: 1
---

# Phase 446 Plan 02: Canonicalize OPENROUTER_KEY in whatsapp-bot claudeService.js — Summary

One-liner: IIFE dual-read at `claudeService.js:3` now tries `OPENROUTER_KEY` first, falls back to `OPENROUTER_API_KEY` with one-shot `console.warn`; redaction regex extended to cover both spellings.

## What Was Built

Applied the Phase 446 canonical-first dual-read pattern to the sole JS OpenRouter read site:
- `whatsapp-bot/src/services/claudeService.js` line 3: single env read replaced with IIFE
- `claudeService.js` line 35 (formerly): redaction regex extended from `/OPENROUTER_API_KEY/g` to `/OPENROUTER(_API)?_KEY/g`
- Line 69 Authorization header: byte-identical, unchanged

## Commit

**Repository:** `C:/Users/bono/racingpoint/whatsapp-bot` (sibling repo, branch `main`)

**Hash:** `0981afb`

**Subject:** `refactor(446): canonicalize OPENROUTER_KEY in claudeService.js (dual-read + one-shot deprecation warn)`

**Diff stat:**
```
 src/services/claudeService.js | 18 ++++++++++++++++--
 1 file changed, 16 insertions(+), 2 deletions(-)
```

## Evidence — CGP H3

### BEHAVIOR: 3-state inline smoke test

All three states run at James-local `C:\Users\bono\racingpoint\whatsapp-bot`.

**State A — Neither env var set (no warn expected)**
```
Command:
  unset OPENROUTER_KEY; unset OPENROUTER_API_KEY
  node -e "let warned=false; const orig=console.warn; console.warn=(...a)=>{warned=true;orig(...a);}; require('./src/services/claudeService'); if(warned){process.exit(1);} console.log('NO_ENV_OK');"

stdout: NO_ENV_OK
stderr: (empty)
exit:   0
```

**State B — Canonical OPENROUTER_KEY set, deprecated unset (no warn expected)**
```
Command:
  unset OPENROUTER_API_KEY
  OPENROUTER_KEY=dummy-test-only-not-a-real-key node -e "let warned=false; const orig=console.warn; console.warn=(...a)=>{warned=true;orig(...a);}; require('./src/services/claudeService'); if(warned){process.exit(1);} console.log('CANONICAL_PATH_OK_no_warn');"

stdout: CANONICAL_PATH_OK_no_warn
stderr: (empty)
exit:   0
```

**State C — Deprecated OPENROUTER_API_KEY set, canonical unset (one warn expected)**
```
Command:
  unset OPENROUTER_KEY
  OPENROUTER_API_KEY=dummy-test-only-not-a-real-key node -e "let warnText=''; const orig=console.warn; console.warn=(...a)=>{warnText+=a.join(' ');orig(...a);}; require('./src/services/claudeService'); if(!warnText.includes('OPENROUTER_API_KEY is deprecated')){process.exit(1);} console.log('DEPRECATION_PATH_OK_warn_fired');"

stdout: DEPRECATION_PATH_OK_warn_fired
stderr: [whatsapp-bot] OPENROUTER_API_KEY is deprecated — rename to OPENROUTER_KEY in pm2 env (read once, will not repeat)
exit:   0
```

Observation: deprecation warn fires in State C only — canonical-first semantics confirmed.

### RAW OUTPUT: Lint check

`scripts.lint` is MISSING from `whatsapp-bot/package.json`. The fallback `npx eslint` (v9.2.1) could not run because there is no `eslint.config.(js|mjs|cjs)` flat config file in the repo (ESLint v9 dropped `.eslintrc.*` support). Neither lint path succeeded.

Fallback verification used: `node --check src/services/claudeService.js`
```
Result: SYNTAX_OK (exit 0)
```

Note for future phases: `whatsapp-bot` has no lint infrastructure. Either add an `eslint.config.mjs` with `@eslint/js` recommended rules, or add a `scripts.lint` entry pointing to `node --check`. Tracked as known gap — not this plan's scope.

### RAW OUTPUT: Static grep audit (CANON-446-02)

```
$ grep -rn 'process.env.OPENROUTER_API_KEY' src/ --include='*.js' --include='*.ts'
src/services/claudeService.js:12:  if (process.env.OPENROUTER_API_KEY && process.env.OPENROUTER_API_KEY.length > 0) {
src/services/claudeService.js:14:    return process.env.OPENROUTER_API_KEY;
```

Both hits are lines 12 and 14 — both inside the IIFE fallback branch (`if` guard and `return`). Zero hits outside the IIFE fallback branch. CANON-446-02 static check satisfied.

### RAW OUTPUT: Redaction regex behavior test

```
node -e "const r=/OPENROUTER(_API)?_KEY/g; const results=['OPENROUTER_KEY','OPENROUTER_API_KEY','OPENROUTER_MGMT_KEY'].map(s=>s.replace(r,'[REDACTED]')); ..."
Output: REGEX_OK
```

- `OPENROUTER_KEY` → `[REDACTED]` (canonical name redacted)
- `OPENROUTER_API_KEY` → `[REDACTED]` (deprecated name redacted)
- `OPENROUTER_MGMT_KEY` → `OPENROUTER_MGMT_KEY` (NOT redacted — intentional carve-out)

### WHERE
James-local `C:\Users\bono\racingpoint\whatsapp-bot` on Node v22.22.0

### NOT TESTED (explicit deferred list)
- Live WhatsApp message → Claude via OpenRouter round-trip — Plan 04 (Bono VPS behavior verify)
- pm2 env update on Bono VPS (deploy step deferred to Plan 04)
- PM2 logs containing zero deprecation warns in production — Plan 04 verification
- Concurrent multi-process warn dedup (IIFE fires once per `require()` call — Node's module cache prevents re-execution, so warn is effectively one-shot per process lifetime)
- Both `OPENROUTER_KEY` and `OPENROUTER_API_KEY` set simultaneously — canonical wins per IIFE logic (tested implicitly by State B but not with both set simultaneously)

## Decisions Made

1. **Local const name preserved as `OPENROUTER_API_KEY`** — Rationale: file-scoped const, not the env-var name. Renaming cascades to redaction regex at line 35 AND Authorization concat at line 69 for zero functional benefit. Phase 448 owns a central typed loader that will resolve this cleanly. Smallest Reversible Fix First per CLAUDE.md.

2. **Redaction regex `/OPENROUTER(_API)?_KEY/g`** — The `(_API)?` optional group is a literal, not a wildcard. Matches exactly `OPENROUTER_KEY` and `OPENROUTER_API_KEY`. Does NOT match `OPENROUTER_MGMT_KEY` — confirmed by inline regex behavior test.

3. **`console.warn` not `logger.warn` in IIFE** — `logger` is available at module top level (line 1 `require('../utils/logger')`), but `console.warn` is simpler and removes any logger-initialization dependency at the IIFE eval point. Also matches the kickoff snippet verbatim.

## Deviations from Plan

### Lint — Fallback Needed

**Found during:** Task 2

**Issue:** `scripts.lint` key is absent from `whatsapp-bot/package.json`. Plan specified fallback to `npx eslint src/services/claudeService.js --max-warnings 0`. ESLint v9.2.1 (installed globally) requires a flat config (`eslint.config.mjs`) — none exists in the repo. `npx eslint` exited 2 with "couldn't find an eslint.config file."

**Action taken:** Used `node --check src/services/claudeService.js` as syntax verification gate (exit 0 = SYNTAX_OK). Noted `scripts.lint` absence in this SUMMARY and in the commit body. CANON-446-04 acceptance ("npm run lint green") could not be fully satisfied with the current repo state.

**Files modified:** none (verification-only)

**Recommendation:** Add to `whatsapp-bot/package.json`: `"scripts": { "lint": "node --check src/" }` or install eslint flat config. A future phase can close this gap.

## Self-Check

Commit `0981afb` exists in whatsapp-bot/main — pushed to origin (git rev-list HEAD..@{upstream} = 0).

File `whatsapp-bot/src/services/claudeService.js` modified — confirmed via diff stat (1 file changed, 16 insertions(+), 2 deletions(-)).

## Known Stubs

None. The IIFE is fully wired — both `process.env.OPENROUTER_KEY` and `process.env.OPENROUTER_API_KEY` read paths are live. No placeholder values. The `Authorization: 'Bearer ' + OPENROUTER_API_KEY` line at ~line 69 resolves correctly from the IIFE's return value.
