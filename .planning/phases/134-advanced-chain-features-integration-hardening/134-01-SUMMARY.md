---
phase: 134-advanced-chain-features-integration-hardening
plan: "01"
subsystem: comms-link
tags: [chain-orchestrator, template-resolution, output-templating, retry, backoff, tdd]

# Dependency graph
requires:
  - phase: 133-task-delegation-audit-trail
    provides: AuditLogger and delegation protocol wiring used by ChainOrchestrator callers
  - phase: 132-chain-orchestrator-foundation
    provides: ChainOrchestrator base class with stdout piping, continue_on_error, chain timeout

provides:
  - ChainOrchestrator with named template resolution via templatesFn injection
  - chains.json config with deploy-bono and health-check-bono templates
  - "{{prev_stdout}} substitution in step args with metacharacter sanitization"
  - Per-step retry with linear backoff (retryBackoffMs * attempt) and new execId per attempt

affects:
  - 134-02 (daemon wiring - james/index.js and bono/index.js will wire chains.json templatesFn)
  - Any plan that calls ChainOrchestrator.execute()

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "templatesFn injection: testable config loading (inject mock in tests, readFileSync in prod)"
    - "Sanitize-before-substitute: prevStdout stripped of shell metacharacters before arg replacement"
    - "Retry with fresh execId: broker dedup avoids blocking re-use of same ID across attempts"
    - "Linear backoff: retryBackoffMs * attemptNumber (attempt 1 = 1x, attempt 2 = 2x)"

key-files:
  created:
    - C:/Users/bono/racingpoint/comms-link/chains.json
  modified:
    - C:/Users/bono/racingpoint/comms-link/shared/chain-orchestrator.js
    - C:/Users/bono/racingpoint/comms-link/test/chain-orchestrator.test.js

key-decisions:
  - "templatesFn defaults to () => ({}) for backward compatibility - no constructor signature breakage"
  - "Inline steps take precedence over template when both provided (steps wins)"
  - "Sanitization strips ; | & $ ` \\ ' \" ( ) { } < > newlines to prevent shell injection via substituted values"
  - "Retry does not re-attempt on broker timeout - only on non-zero exitCode from exec_result"
  - "prevStdout passed to next step comes from the successful retry attempt stdout (not empty)"

patterns-established:
  - "Injectable fn pattern: templatesFn for config loading keeps orchestrator testable without FS coupling"
  - "TDD RED-GREEN: 9 existing tests unchanged, 11 new tests added before implementation"

requirements-completed: [CHAIN-06, CHAIN-07, CHAIN-08]

# Metrics
duration: 31min
completed: 2026-03-22
---

# Phase 134 Plan 01: Advanced Chain Features Summary

**ChainOrchestrator extended with named template resolution (chains.json + templatesFn injection), {{prev_stdout}} arg substitution with shell metacharacter sanitization, and per-step retry with linear backoff and new execId per attempt**

## Performance

- **Duration:** 31 min
- **Started:** 2026-03-22T23:00:22Z (IST 04:30)
- **Completed:** 2026-03-22T23:31:00Z (IST 05:01)
- **Tasks:** 1 (TDD: RED commit + GREEN commit)
- **Files modified:** 3

## Accomplishments

- chains.json created at comms-link root with deploy-bono and health-check-bono named templates
- ChainOrchestrator.execute() resolves steps from templatesFn when template name is provided; inline steps always take precedence
- {{prev_stdout}} in step.args replaced with sanitized prev step stdout (strips ; | & $ ` ' " () {} <> \n)
- Per-step retry loop: up to step.retries extra attempts, each with a fresh execId, linear backoff (retryBackoffMs * attempt)
- All 20 tests pass: 9 existing (no regressions) + 11 new covering templates, templating, retry

## Task Commits

Each task was committed atomically (TDD pattern):

1. **Task 1 RED: Failing tests** - `2ce60c8` (test)
2. **Task 1 GREEN: Implementation + chains.json** - `2f017eb` (feat)

**Logbook/chore:** `27f4c30` (chore: logbook entries for 134-01)

## Files Created/Modified

- `C:/Users/bono/racingpoint/comms-link/chains.json` - Named chain templates config (deploy-bono, health-check-bono)
- `C:/Users/bono/racingpoint/comms-link/shared/chain-orchestrator.js` - Extended with templatesFn, output templating, retry loop
- `C:/Users/bono/racingpoint/comms-link/test/chain-orchestrator.test.js` - 11 new TDD tests added (tests 10-20)

## Decisions Made

- **templatesFn defaults to `() => ({})`** — backward compatible, no constructor breakage for existing callers
- **Inline steps take precedence over template** — unambiguous: if steps array is non-empty, template is ignored
- **Sanitization approach** — strip dangerous chars from prevStdout before substitution (not after), protecting all downstream args
- **Retry does not re-attempt on broker timeout** — timeouts indicate infra issues, not transient step failures; retrying would just waste the timeout window again
- **Fresh execId per retry** — ExecResultBroker dedup is keyed by execId; reuse would silently resolve to the original (stale) result

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- ChainOrchestrator is complete with all three internal enhancements
- Phase 134-02 will wire chains.json templatesFn into james/index.js and bono/index.js daemons
- chains.json is already at comms-link root, ready for production readFileSync wiring

---
*Phase: 134-advanced-chain-features-integration-hardening*
*Completed: 2026-03-22*
