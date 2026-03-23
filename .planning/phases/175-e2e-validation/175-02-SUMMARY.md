---
phase: 175-e2e-validation
plan: "02"
subsystem: test-infrastructure
tags: [e2e, testing, bash, triage, cross-sync]
dependency_graph:
  requires: [175-01]
  provides: [test/e2e/run-cross-sync.sh, test/e2e/TRIAGE.md]
  affects: []
tech_stack:
  added: []
  patterns: [bash-interactive-guide, markdown-triage-log]
key_files:
  created:
    - test/e2e/run-cross-sync.sh
    - test/e2e/TRIAGE.md
  modified: []
decisions:
  - "run-cross-sync.sh uses read -rp (not read -p) for POSIX compatibility and safer input handling"
  - "Inline fleet/health fallback added to pre-flight when check-health.sh is not found at expected path"
  - "3.4.5 timezone check partially automated via Python3 JSON parse of /api/v1/sessions timestamps"
  - "TRIAGE.md sign-off checklist references all four E2E requirements explicitly as checkboxes"
metrics:
  duration_minutes: 3
  tasks_completed: 2
  tasks_total: 3
  files_created: 2
  files_modified: 0
  completed_date: "2026-03-23"
requirements_marked_complete: []
---

# Phase 175 Plan 02: Cross-Sync Test Guide and Triage Log Summary

**One-liner:** Interactive bash guide for 21 cross-cutting tests (sections 3.1–3.4) with curl state verification + TRIAGE.md failure classification log with phase sign-off checklist.

## What Was Built

### test/e2e/run-cross-sync.sh

Interactive test guide for all 21 tests in sections 3.1–3.4 of E2E-TEST-SCRIPT.md:

- **Pre-flight:** Calls `check-health.sh` if found; falls back to inline curl probe of `:8080`, `:3200`, `:3300`
- **Setup instructions:** Prints two-browser setup steps (POS at `:3200`, Kiosk at `:3300`) and waits for Enter
- **Section 3.1 (5 tests):** Responsiveness and display — manual instructions with `ask_result` prompts
- **Section 3.2 (5 tests):** Real-time sync — step-by-step guide + `curl fleet/health` and `curl sessions` between browser actions to verify server-side state
- **Section 3.3 (5 tests):** Error handling — instructions for DevTools offline simulation and rapid-click debounce testing
- **Section 3.4 (6 tests):** Edge cases — instructions for special characters, empty states; test 3.4.5 (timezone) uses Python3 JSON parse of `/api/v1/sessions` to check for `+05:30` in timestamps
- **Results:** Each test recorded via `record_result` helper; appended to `E2E-TEST-RESULTS-{date}.md` (creating it if absent)
- **Summary:** PASS/FAIL/SKIP totals printed at end + appended as table to report file

### test/e2e/TRIAGE.md

Authoritative failure classification log for Phase 175:

- **Triage Status table:** Checkboxes for E2E-01 through E2E-04
- **Fixed Failures table:** Test ID / Description / Root Cause / Fix / Commit — for resolved failures
- **Known Issues table:** Test ID / Description / Root Cause / Severity / Decision — for deferred failures
- **5-step triage process:** Reproduce → Root Cause → Classify → Record → Sign Off
- **Severity guide:** Critical (must fix before ship) / High / Low with definitions
- **Phase 175 Sign-off checklist:** 12 items covering execution, triage, REQUIREMENTS.md, ROADMAP.md, LOGBOOK.md, git push, Bono notification

## Execution Status

**Framework built. Test execution PENDING — server and pods are currently offline.**

The E2E framework is now complete:

| File | Status |
|------|--------|
| `test/e2e/run-e2e.sh` | Built (plan 01, commit 3afbe827) |
| `test/e2e/E2E-REPORT-TEMPLATE.md` | Built (plan 01, commit b8908888) |
| `test/e2e/run-cross-sync.sh` | Built (this plan, commit 2033ff1c) |
| `test/e2e/TRIAGE.md` | Built (this plan, commit 2606b98c) |

Requirements E2E-01 through E2E-04 will be marked complete when the human checkpoint (Task 3) is approved after live test execution.

## Deviations from Plan

None — plan executed exactly as written.

Implementation notes (not deviations):
- Used `read -rp` instead of `read -p` for safer input handling in bash
- Pre-flight uses a path-based lookup for `check-health.sh` with inline fallback (plan said "call check-health.sh" but didn't specify the exact path — chose `deploy-staging/check-health.sh` based on CLAUDE.md references)
- 3.4.5 timezone automation uses Python3 `json.load` for reliable parsing rather than grep (avoids false positives on timestamp strings)

## Self-Check

```bash
[ -f "test/e2e/run-cross-sync.sh" ] && echo "FOUND: run-cross-sync.sh" || echo "MISSING: run-cross-sync.sh"
[ -f "test/e2e/TRIAGE.md" ] && echo "FOUND: TRIAGE.md" || echo "MISSING: TRIAGE.md"
```

**FOUND:** test/e2e/run-cross-sync.sh (commit 2033ff1c)
**FOUND:** test/e2e/TRIAGE.md (commit 2606b98c)
**bash -n test/e2e/run-cross-sync.sh:** PASSED
**TRIAGE.md E2E-01..E2E-04 references:** 16 occurrences — PASSED
**Fixed Failures table (required columns):** PASSED
**Known Issues table (required columns):** PASSED
**Triage process steps 1-5:** PASSED
**Sign-off checklist with E2E-01..E2E-04:** PASSED

## Self-Check: PASSED
