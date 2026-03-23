# Phase 175: E2E Validation - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Create the E2E test runner framework, test report template, and triage process. Actual test execution deferred until POS and Kiosk are running.

</domain>

<decisions>
## Implementation Decisions

### Test Framework
- Bash script that walks through E2E-TEST-SCRIPT.md sections
- For each test: curl the endpoint or page, check HTTP status, log PASS/FAIL
- Output structured report (markdown table)
- Automated where possible (API endpoints), manual checklist for UI tests

### Test Categories
- Automated: API endpoint tests (HTTP status + JSON shape validation)
- Semi-automated: Page load tests (HTTP 200 + content checks)
- Manual: UI interaction tests (checkbox in report, tester fills in)

### Report Format
- Markdown file: E2E-TEST-RESULTS-{date}.md
- Summary table at top (section / total / pass / fail / skip)
- Detailed results per section
- Known issues section with root cause + follow-up item

### Cross-Cutting Sync Tests
- Start billing on POS, verify reflected on Kiosk (needs WebSocket)
- Book on Kiosk, verify POS shows session
- These require live environment — framework structure only

### Claude's Discretion
- Which of the 231 tests can be automated vs need manual
- Test runner script implementation
- How to handle tests that need specific pod states

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- C:/RacingPoint/E2E-TEST-SCRIPT.md — the 231-test checklist
- check-health.sh — can verify services are up before running tests
- packages/contract-tests/ — Vitest tests for API shape validation

### Integration Points
- POS at :3200 (web dashboard)
- Kiosk at :3300 (kiosk app)
- racecontrol API at :8080

</code_context>

<specifics>
## Specific Ideas

- Test runner should call check-health.sh first — abort if services down
- Reuse contract test patterns for API shape validation
- E2E-TEST-SCRIPT.md is the definitive test list

</specifics>

<deferred>
## Deferred Ideas

- Full Playwright browser automation (future milestone)
- Test execution (needs server online)
- Cross-sync real-time tests (needs WebSocket)

</deferred>
