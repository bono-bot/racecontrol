# Phase 216: Pipeline Self-Test Suite - Context

**Gathered:** 2026-03-26
**Status:** Ready for planning

<domain>
## Phase Boundary

Offline test suite for the auto-detect pipeline. Tests every detector and escalation tier against known-good and known-bad inputs without touching live infrastructure. Run before each production run to confirm detection correctness.

</domain>

<decisions>
## Implementation Decisions

### Claude's Discretion
All implementation choices are at Claude's discretion -- pure infrastructure phase.

Key constraints from success criteria:
- TEST-01: test-auto-detect.sh tests all 6 pipeline steps, PASS/FAIL per step
- TEST-02: Detector tests with fixture files (fake JSONL, fake TOML) -- no live pods
- TEST-03: Escalation ladder test with mocked pod that never recovers -- verifies tier ordering
- TEST-04: Bono coordination race condition test -- simultaneous lock acquisition
- All tests use mocked inputs (fixture files, mock functions) -- zero network calls
- Test framework: bash with assertion helpers (no external test framework)

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- scripts/auto-detect.sh -- pipeline to test (6 steps)
- scripts/detectors/*.sh -- 6 detectors to test individually
- scripts/healing/escalation-engine.sh -- escalation ladder to test
- scripts/coordination/coord-state.sh -- lock mechanism to test
- audit/lib/core.sh -- safe_remote_exec (must be mocked in tests)

### Established Patterns
- bash -n syntax checking (already used in Phase 212)
- Fixture files for offline testing
- Function mocking via function override after source

### Integration Points
- Test suite at audit/test/test-auto-detect.sh (main entry)
- Individual test files per concern (detectors, escalation, coordination)
- Can be wired into pre-run validation step of auto-detect.sh

</code_context>

<specifics>
## Specific Ideas

- Mock safe_remote_exec to return fixture data instead of hitting real pods
- Fixture directory: audit/test/fixtures/ with known-good and known-bad inputs
- Each test function returns 0 (pass) or 1 (fail), aggregated at end
- Color output: green PASS, red FAIL

</specifics>

<deferred>
## Deferred Ideas

None -- test suite is scope-complete in this phase.

</deferred>
