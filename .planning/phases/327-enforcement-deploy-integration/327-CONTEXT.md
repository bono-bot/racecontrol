# Phase 327: Enforcement & Deploy Integration - Context

**Gathered:** 2026-04-06
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

Frontend completion claims require screenshot evidence, and deploys automatically detect visual regressions. Two components: (1) Claude Code hook blocking "fixed/done/resolved" claims for frontend changes without screenshot evidence, (2) deploy script integration auto-running page crawler after deploy with build hash verification.

Requirements: HOOK-01, HOOK-02, HOOK-03, DEPLOY-01, DEPLOY-02, DEPLOY-03

Success criteria:
1. Claude Code hook blocks "fixed/done/resolved" claims for frontend changes unless screenshot file newer than last code edit exists
2. Hook only fires for frontend file changes (Next.js, CSS, React) — not Rust/scripts
3. After deploy-nextjs.sh completes, page crawler runs automatically, deploy exits with failure if visual regressions detected
4. Deploy output includes build hash verification table (expected vs running on all targets)

</domain>

<decisions>
## Implementation Decisions

### Claude Code Hook
- Hook goes in ~/.claude/hooks/ as a Node.js file (like existing cgp-enforce.js, backlog-enforce.js)
- Hook type: PreToolUse — triggers before Write/Edit/Bash tools when output contains "fixed/done/resolved" keywords
- Actually, better as a **PostToolUse or custom check** — needs to detect completion CLAIMS in assistant output, not tool calls
- Most practical: add to existing cgp-session-inject.js as an additional check, or create screenshot-enforce.js
- Frontend file detection: check if recent tool calls modified files in web/, kiosk/, pwa/, or files ending in .tsx/.css/.scss
- Screenshot evidence: check if tests/screenshots/ has files newer than the session start or last code edit

### Deploy Integration
- Modify existing deploy-nextjs.sh to add post-deploy crawler step
- Build hash verification: curl /api/v1/health on all targets, compare build_id against expected (git rev-parse --short HEAD)
- Targets from MEMORY.md: Server .23, Pods 1-8, POS .20, Bono VPS
- Use scripts/check-alive.sh patterns for multi-target verification

### Claude's Discretion
All other implementation details at Claude's discretion.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `~/.claude/hooks/cgp-enforce.js` — existing PreToolUse hook pattern (blocks tool calls, returns denial message)
- `~/.claude/hooks/cgp-session-inject.js` — UserPromptSubmit hook (injects reminders into context)
- `~/.claude/hooks/backlog-enforce.js` — UserPromptSubmit hook scanning memory for incomplete work
- `scripts/deploy-nextjs.sh` — existing deploy script for Next.js apps
- `scripts/deploy-server.sh` — server deploy with build hash verification pattern
- `scripts/check-alive.sh` — multi-probe connectivity checker
- `tests/page-crawler/crawl.spec.ts` — Phase 325 page crawler
- `tests/visual-regression/visual.spec.ts` — Phase 326 visual regression tests

### Established Patterns
- Hooks are Node.js files reading from stdin (JSON with tool name, input, etc.)
- Hooks output JSON to stdout: { "decision": "block"|"allow", "reason": "..." }
- Deploy scripts use bash with curl for health checks
- Build hash: `curl -s http://target:8080/api/v1/health | jq -r .build_id`

### Integration Points
- Hook registered in ~/.claude/settings.json under hooks array
- deploy-nextjs.sh called manually or via deploy chain
- Page crawler invoked via: `npx playwright test --config tests/page-crawler/playwright.config.ts`
- Visual regression via: `npm run vr:compare`

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
