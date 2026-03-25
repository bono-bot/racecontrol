---
phase: 192-intelligence-layer
plan: "04"
subsystem: audit
tags: [audit, report, intelligence-layer, markdown, json]
dependency_graph:
  requires: ["192-01", "192-02", "192-03"]
  provides: ["generate_report", "audit-report.md", "audit-summary.json", "full-intelligence-orchestration"]
  affects: ["audit/audit.sh", "audit/lib/report.sh"]
tech_stack:
  added: []
  patterns: ["temp-file-atomic-write", "declare-f-guard", "jq-report-generation"]
key_files:
  created:
    - audit/lib/report.sh
  modified:
    - audit/audit.sh
decisions:
  - "report.sh: dual-format output — audit-report.md for humans, audit-summary.json for machines (Phase 193 automation)"
  - "Verdict logic: FAIL if any FAIL count > 0, WARN if any WARN count > 0, else PASS — SUPPRESSED excluded from FAIL"
  - "Atomic writes via temp file + mv — prevents partial files on errors"
  - "All 4 orchestration calls guarded with declare -f — backward compatible if a lib file is missing"
  - "Intelligence layer order: suppress -> finalize -> delta -> report — suppressions happen before counting"
metrics:
  duration_secs: 177
  completed_date: "2026-03-25T15:52:26+05:30"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 1
requirements_satisfied: [INTL-06, INTL-07]
---

# Phase 192 Plan 04: Report Generation & Intelligence Layer Wiring Summary

Dual-format audit report generation (Markdown + JSON) wired as final step of full intelligence layer: suppress -> finalize -> delta -> report.

## Objective

Create `audit/lib/report.sh` with `generate_report` function producing both `audit-report.md` and `audit-summary.json` in `RESULT_DIR`. Wire all 4 intelligence libraries into `audit.sh` in correct order.

## Tasks Completed

| # | Name | Commit | Key Files |
|---|------|--------|-----------|
| 1 | Create audit/lib/report.sh with generate_report function | `e0109f78` | audit/lib/report.sh (created, 372 lines) |
| 2 | Wire all intelligence libraries into audit.sh — source + orchestrate | `399ef8b0` | audit/audit.sh (35 lines added) |

## What Was Built

### audit/lib/report.sh

Single exported function `generate_report` producing two output files:

**audit-report.md** (Markdown, human-readable):
- Header: date, mode, venue state, verdict
- Summary table: PASS/WARN/FAIL/QUIET/SUPPRESSED/Total counts
- Results by Tier: per-tier tables with phase/host/status/severity/message, tiers 1-18 named
- Delta section: Regressions / Improvements / Persistent Issues / New Issues (with prev->curr status transitions)
- Suppressed Issues table: phase/host/reason/message for SUPPRESSED results
- Fix Actions table: phase/host/action/before/after/timestamp from fixes.jsonl
- Footer: generator stamp with IST timestamp

**audit-summary.json** (JSON, machine-readable):
- generated_at, mode, venue_state, result_dir
- counts: pass/warn/fail/quiet/suppressed/total
- delta: has_previous, regression, improvement, persistent, new_issue
- verdict: PASS | WARN | FAIL

### audit.sh Intelligence Layer

Sources delta.sh, suppress.sh, report.sh after results.sh (all with file-exists guards).

Orchestration block after "Phase runner complete":
1. `apply_suppressions` — rewrites FAIL/WARN to SUPPRESSED where suppress.json matches
2. `finalize_results` — counts final statuses (SUPPRESSED not counted as FAIL), updates index.json
3. `compute_delta` — compares against previous run, writes delta.json
4. `generate_report` — reads all data, produces audit-report.md + audit-summary.json

All 4 calls guarded with `declare -f` for backward compatibility.

Exit code counting: `status == "FAIL"` only — SUPPRESSED phases do not trigger exit 1.

## Verification

All plan verification steps passed:
- `bash -n audit/audit.sh` — no syntax errors
- `bash -n audit/lib/report.sh` — no syntax errors
- `grep 'generate_report' audit/audit.sh` — call present
- `AUDIT_PIN=261121 bash audit/audit.sh --help` — exits 0
- All 4 intelligence library source lines in audit.sh (core, parallel, results, delta, suppress, report — 6 total)
- Orchestration order confirmed: apply_suppressions (462) -> finalize_results (467) -> compute_delta (472) -> generate_report (477)

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- `audit/lib/report.sh` exists — FOUND
- `e0109f78` commit — FOUND (`git log --oneline | grep e0109f78`)
- `399ef8b0` commit — FOUND (`git log --oneline | grep 399ef8b0`)
- All acceptance criteria verified: 10/10 for Task 1, 9/9 for Task 2
