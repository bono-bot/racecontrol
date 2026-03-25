---
phase: 192
plan: 03
subsystem: audit-intelligence
tags: [suppression, severity-scoring, audit, shell]
dependency_graph:
  requires:
    - audit/lib/core.sh (emit_result schema — phase/host/message fields)
    - audit/results/*/phase-*.json (phase result files to batch-process)
  provides:
    - audit/lib/suppress.sh (check_suppression, apply_suppressions, get_severity_score)
    - audit/suppress.json (operator-managed suppression config with mandatory expiry)
  affects:
    - audit/lib/report.sh (future — uses get_severity_score for sorting)
    - audit/audit.sh (future — calls apply_suppressions after all phases complete)
tech_stack:
  added: []
  patterns:
    - jq test() for regex pattern matching in shell
    - TZ=Asia/Kolkata for IST date comparison
    - mktemp+mv for atomic JSON rewrite (suppress apply)
key_files:
  created:
    - audit/lib/suppress.sh
    - audit/suppress.json
  modified: []
decisions:
  - "192-03: host_pattern in suppress.json is regex (pod-91-.*), not glob — matches jq test() directly"
  - "192-03: check_suppression returns reason via stdout, not via global variable — composable for apply_suppressions"
  - "192-03: apply_suppressions uses mktemp+mv for atomic JSON rewrite — prevents partial writes on failure"
  - "192-03: Expired entries silently ignored (not an error) — expired suppress config is self-healing"
metrics:
  duration: "8 minutes"
  completed_date: "2026-03-25"
  tasks_completed: 2
  tasks_total: 2
  files_created: 2
  files_modified: 0
---

# Phase 192 Plan 03: Suppression Engine and Severity Scoring Summary

Suppression engine (check/apply against suppress.json with IST expiry enforcement) plus numeric severity scoring — known issues acknowledged with mandatory expiry dates, SUPPRESSED in reports with visible reason, never silently hidden.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Create audit/suppress.json seed file | d216e550 | audit/suppress.json |
| 2 | Create audit/lib/suppress.sh with 3 functions | 6b09a14f | audit/lib/suppress.sh |

## What Was Built

### audit/suppress.json
Operator-managed suppression config. Seed entry: Pod 8 NVIDIA Surround known issue (phase 17, host pattern `pod-91-.*`, message pattern `1024x768`, expires 2026-04-25). Schema enforces 7 mandatory fields including `expires_date` — no indefinite suppressions allowed.

### audit/lib/suppress.sh — 3 exported functions

**`check_suppression(phase host message)`**
Reads suppress.json, iterates entries via jq. Matches phase exactly, applies regex to host (jq `test()`), applies regex to message (or empty = match any), and enforces `expires_date >= today` (IST). Returns reason via stdout + exit 0 if suppressed; exit 1 if not. Expired entries are silently ignored — phase reverts to actual status.

**`apply_suppressions()`**
Batch processor called after all phases complete. Iterates `$RESULT_DIR/phase-*.json`, calls `check_suppression` for each FAIL/WARN result, rewrites matching files with `status: "SUPPRESSED"` and `suppression_reason` added. Uses mktemp+mv for atomic rewrite. Prints `Suppressed: N phase(s)` summary.

**`get_severity_score(status severity)`**
Returns numeric score: FAIL/P1=100, FAIL/P2=80, FAIL/P3=60, WARN/P1=50, WARN/P2=40, WARN/P3=30, SUPPRESSED=10, QUIET=5, PASS=0. Enables priority sorting in report generation.

## Verification

```
bash -n audit/lib/suppress.sh        # PASS — no syntax errors
jq . audit/suppress.json             # PASS — valid JSON
grep -c 'export -f' audit/lib/suppress.sh  # 3
IST date comparison present          # PASS (TZ=Asia/Kolkata)
SUPPRESSED status written to file    # PASS
```

Functional test: apply_suppressions correctly rewrites phase-17/pod-91 FAIL to SUPPRESSED with reason, leaves phase-01/server FAIL unchanged.

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- audit/suppress.json: FOUND (d216e550)
- audit/lib/suppress.sh: FOUND (6b09a14f)
- Commits d216e550 and 6b09a14f: verified in git log
