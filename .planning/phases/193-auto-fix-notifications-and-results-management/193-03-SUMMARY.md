---
phase: 193-auto-fix-notifications-and-results-management
plan: 03
subsystem: audit-pipeline
tags: [audit, pipeline, wiring, integration-test, git-commit]
dependency_graph:
  requires: [193-01, 193-02]
  provides: [RSLT-03, full-pipeline-wiring]
  affects: [audit/audit.sh, audit/test/test-pipeline.sh]
tech_stack:
  added: [audit/test/test-pipeline.sh]
  patterns: [pipeline-wiring, flag-gating, integration-testing]
key_files:
  modified: [audit/audit.sh]
  created: [audit/test/test-pipeline.sh]
decisions:
  - "fixes.sh sourced after notify.sh (both after existing source block), guards with declare -f at call sites"
  - "run_auto_fixes inserted as Step 1.5 between apply_suppressions and finalize_results so fix results are captured in finalize counts"
  - "git commit block uses subshell so cd/git failures don't affect main shell; no git push (operator choice)"
  - "test-pipeline.sh uses structural grep tests not live execution for pipeline order and flag gating"
metrics:
  duration_minutes: 10
  completed_date: "2026-03-25"
  tasks_completed: 2
  files_changed: 2
---

# Phase 193 Plan 03: Pipeline Wiring + Integration Test Summary

Full audit pipeline wired: fixes.sh sourced + run_auto_fixes (Step 1.5) + git commit block (--commit flag) added to audit.sh; 7-test integration suite validates all flag gates and pipeline order.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Wire fixes.sh and notify.sh into audit.sh, add --commit | 9699ecad | audit/audit.sh |
| 2 | Create integration test for full pipeline | 61235e7e | audit/test/test-pipeline.sh |

## What Was Built

### Task 1: Pipeline Wiring in audit.sh

**Changes to audit/audit.sh:**

1. **fixes.sh sourcing** added after the existing notify.sh source block (lines 52-54): guarded with `[ -f ... ]` pattern matching all other source blocks.

2. **run_auto_fixes call** inserted as Step 1.5 after `apply_suppressions` and before `finalize_results` (line 475): guarded with `declare -f` pattern. This placement ensures fix results are captured when `finalize_results` counts statuses.

3. **git commit block** added after `send_notifications` and before exit code counting (lines 507-518): gated on `COMMIT:-false = true` + RESULT_DIR existence. Uses subshell for isolation. Commit message includes mode + IST timestamp. No git push (operator choice).

**Final pipeline order (line numbers):**
- 470: apply_suppressions
- 475: run_auto_fixes (NEW)
- 480: finalize_results
- 485: compute_delta
- 490: generate_report
- 500: send_notifications (was already present)
- 507: git commit block (NEW)
- exit code counting

### Task 2: Integration Test Suite

**audit/test/test-pipeline.sh** — 7 structural tests, all passing:

| # | Test | Method |
|---|------|--------|
| 1 | --auto-fix off by default | Source fixes.sh with AUTO_FIX=false, run_auto_fixes, check no fixes.jsonl |
| 2 | --notify off by default | Source notify.sh with NOTIFY=false, send_notifications, check no output files |
| 3 | --commit flag absent means no git activity | grep COMMIT:-false + git add in audit.sh |
| 4 | Pipeline order verification | grep line numbers for all 7 pipeline steps, assert order |
| 5 | All libs syntax-check | bash -n on fixes.sh, notify.sh, audit.sh |
| 6 | APPROVED_FIXES whitelist has exactly 3 entries | Source fixes.sh, check array length |
| 7 | Dry-run with all flags exits 0 | AUDIT_PIN=000000 bash audit.sh --mode quick --auto-fix --notify --commit --dry-run |

## Verification Results

```
bash -n audit/audit.sh: PASS
bash audit/test/test-pipeline.sh: 7/7 tests passed (EXIT_CODE=0)
Pipeline order confirmed at lines: suppress=470 fix=475 finalize=480 delta=485 report=490 notify=500 commit=507
```

## Deviations from Plan

### Pre-existing wiring (notify.sh)

**Found during:** Task 1 read_first

**Issue:** audit.sh already had notify.sh sourced (lines 48-50) and send_notifications called (lines 491-493) from a previous session. The plan described adding both, but only half was missing.

**Fix:** Added only the missing pieces: fixes.sh sourcing + run_auto_fixes call + git commit block. Did not duplicate the notify.sh wiring.

**Files modified:** audit/audit.sh (no additional change needed)

### Test grep pattern escaping (Python string issue)

**Found during:** Task 2 verification

**Issue:** Python string escaping for `"${COMMIT:-false}" = "true"` produced a malformed grep pattern with a form-feed character (`\x0c`). Tests 3 and 4 initially failed.

**Fix:** Simplified grep patterns to `COMMIT:-false` (partial match without surrounding quotes) + separate `git add.*RESULT_DIR` check for Test 3. Used Python `open().readlines()` patch to fix the broken lines.

**Files modified:** audit/test/test-pipeline.sh

## Self-Check

## Self-Check: PASSED

- FOUND: .planning/phases/193-auto-fix-notifications-and-results-management/193-03-SUMMARY.md
- FOUND: commit 9699ecad (feat(193-03): wire fixes.sh and notify.sh into audit.sh, add --commit flag)
- FOUND: commit 61235e7e (test(193-03): add integration test for full audit pipeline)
- FOUND: audit/audit.sh (modified, 27 insertions)
- FOUND: audit/test/test-pipeline.sh (created, 156 lines)
- VERIFIED: 7/7 integration tests pass
- VERIFIED: bash -n audit/audit.sh syntax OK
