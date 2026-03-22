# Phase 144: GSD Quality Gate - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Wire the integration test suite (Phase 143) into GSD execution as an automatic quality gate. When a comms-link phase completes, the verifier should run integration tests. Failures block phase completion.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion.

Key guidance:
- GATE-01: Create a single entry point script (test/run-all.sh or similar) that runs: contract tests + integration tests + syntax check. Exit 0 = pass, non-zero = fail.
- GATE-02: Add a check to the GSD verifier workflow or the comms-link CLAUDE.md that instructs the verifier to run the integration test after comms-link phases. This could be a CLAUDE.md instruction or a .claude/hooks/ PostToolUse hook.
- GATE-03: The entry point script should output clear pass/fail with failing test names.
- Practical approach: add a "## Pre-Ship Gate" section to comms-link/CLAUDE.md that instructs Claude to run `node --test test/contract.test.js && node --test test/integration.test.js && node scripts/syntax-check.js` before marking any comms-link phase as shipped.
- Also update the rp-bono-exec skill to mention the quality gate.

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- test/integration.test.js — Phase 143 (exec + chain + message relay tests)
- test/contract.test.js — Phase 143 (15 pure contract assertions)
- scripts/syntax-check.js — Phase 143 (node --check all source files)

### Integration Points
- comms-link/CLAUDE.md — add pre-ship gate instructions
- GSD verifier reads CLAUDE.md project instructions and follows them
- .claude/hooks/ could add a PostToolUse hook for automatic gate

</code_context>

<specifics>
## Specific Ideas

Keep it simple — a CLAUDE.md instruction + a single bash script. Don't over-engineer with hooks or workflow modifications.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
