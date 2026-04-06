# Phase 328: AI Self-Audit - Context

**Gathered:** 2026-04-06
**Status:** Ready for planning
**Mode:** Auto-generated (infrastructure phase — discuss skipped)

<domain>
## Phase Boundary

James autonomously identifies pages that look wrong by comparing live screenshots against documented expected behavior. This is the capstone phase — it makes James self-auditing without user intervention.

Requirements: AUDIT-01, AUDIT-02, AUDIT-03, AUDIT-04

Success criteria:
1. Every critical page has a description file documenting expected layout, data sources, and key interactions
2. James can read a fresh screenshot via the Read tool and compare it against the page description to spot anomalies
3. Running the self-audit produces an anomaly report listing pages that don't match expected behavior with specific discrepancies
4. When James starts a session involving frontend work, the self-audit runs automatically to establish baseline awareness

</domain>

<decisions>
## Implementation Decisions

### Page Descriptions
- Create a JSON or markdown file per app with expected behavior descriptions for each critical page
- Location: tests/page-audit/descriptions/ (or similar)
- Each page description includes: URL, expected layout elements, expected data sources, key interactions, what "wrong" looks like
- Focus on the same 10 critical pages from Phase 326 visual regression: web /, /fleet, /billing, /billing/history, /sessions, /drivers, /games; kiosk /, /staff, /control
- Descriptions should be concise — what a human would check when looking at the page

### Self-Audit Workflow
- A script that: (1) runs page crawler on critical pages, (2) reads each screenshot, (3) compares against description, (4) generates anomaly report
- The "comparison" step is done by the AI reading the screenshot image file via the Read tool + reading the description file
- Anomaly report: markdown file listing pages with discrepancies
- Can be run standalone or integrated into session start

### Session Start Integration
- A Claude Code hook or CLAUDE.md instruction that triggers self-audit at session start for frontend work
- Lightweight check: only runs if the session involves frontend files
- Could be a UserPromptSubmit hook that detects frontend context and injects "run self-audit" reminder

### Claude's Discretion
All implementation details at Claude's discretion.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets (from Phases 325-327)
- `tests/page-crawler/crawl.spec.ts` — page crawler (captures screenshots)
- `tests/page-crawler/routes.ts` — 84 routes across 3 apps
- `tests/page-crawler/auth-setup.ts` — staff PIN auth
- `tests/visual-regression/mask-config.ts` — knows which elements are dynamic per page
- `tests/visual-regression/visual.spec.ts` — 10 critical pages already selected
- `scripts/deploy-verify.sh` — post-deploy verification with crawler integration

### Established Patterns
- Claude Code can read image files via the Read tool (multimodal)
- Screenshots saved to tests/screenshots/{app}/{route}/{timestamp}.png
- Existing CLAUDE.md has session-start instructions and standing rules

### Integration Points
- Self-audit script invokes page crawler for screenshot capture
- Anomaly report written to a known location for session review
- CLAUDE.md or hook triggers self-audit at session start

</code_context>

<specifics>
## Specific Ideas

No specific requirements — infrastructure phase.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
