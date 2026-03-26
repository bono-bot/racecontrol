# Phase 209: Pre-Ship Gate and Process Tooling - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Create domain-matched verification gate (gate-check.sh) that blocks deploys based on change type (visual/network/parse/billing/config), and a Cause Elimination Process helper (fix_log.sh) that enforces 5-step structured debugging. Pure bash tooling — zero Rust compile dependency.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion — pure bash tooling phase. Key constraints from requirements:
- gate-check.sh enhances existing `test/gate-check.sh` with domain-matched verification (GATE-01 through GATE-04)
- Visual changes (lock_screen, blanking, overlay, kiosk, Edge, browser, display, screen, CSS/HTML) require VISUAL_VERIFIED=true (GATE-02)
- Network changes (ws_handler, fleet_exec, cloud_sync, http, api/v1, WebSocket, port) require live curl test (GATE-03)
- Parse changes (parse, from_str, serde, toml::from_str, u32::parse, trim, config loading) require test input + expected output (GATE-04)
- fix_log.sh prompts for 5 fields: symptom, hypotheses, elimination, confirmed cause, verification (GATE-05)
- LOGBOOK.md already exists at repo root — fix_log.sh appends structured entries

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `test/gate-check.sh` — existing gate check script (Suite 0 security checks)
- `LOGBOOK.md` — existing logbook at repo root
- `comms-link/test/security-check.js` — security gate (SEC-GATE-01) integrated into gate-check.sh Suite 0

### Established Patterns
- bash + jq for tooling scripts
- `audit/` directory for fleet audit scripts
- `scripts/` directory for operational scripts
- Standing rule: "any bug taking >30 min to isolate MUST use this process before declaring fixed"

### Integration Points
- gate-check.sh Suite 0 — domain verification added alongside existing security checks
- fix_log.sh — new script in scripts/ directory
- LOGBOOK.md — append target for fix_log.sh entries

</code_context>

<specifics>
## Specific Ideas

- gate-check.sh domain detection uses `git diff --name-only` against HEAD~1 or staged files to classify change type
- fix_log.sh should be interactive (read from stdin) with clear prompts for each field
- LOGBOOK.md sample entry should follow the 5-step Cause Elimination template from CLAUDE.md Debugging Methodology section

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>
