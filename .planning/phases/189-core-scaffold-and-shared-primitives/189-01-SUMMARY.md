---
phase: 189-core-scaffold-and-shared-primitives
plan: "01"
subsystem: audit-framework
tags: [bash, audit, scaffold, tdd]
dependency_graph:
  requires: []
  provides:
    - audit/audit.sh entry point
    - audit/lib/ directory for core.sh (Plan 02)
    - audit/phases/ directory for phase scripts (Plans 03+)
    - audit/results/ directory for run output
  affects:
    - Plans 02-04 (all source lib/core.sh from audit/lib/)
    - All future phase checks (depend on audit.sh for invocation)
tech_stack:
  added:
    - bash audit runner with IST timestamps
    - jq for JSON prereq and result counting
  patterns:
    - TDD red/green for bash scripts
    - JSON payload to temp file (bash string escaping safety)
    - TZ=Asia/Kolkata for all timestamps
key_files:
  created:
    - audit/audit.sh
    - audit/lib/.gitkeep
    - audit/phases/.gitkeep
    - audit/results/.gitkeep
    - audit/test-audit-sh.sh
  modified: []
decisions:
  - "Sourcing lib/core.sh with stubs fallback allows Plans 01 and 02 to run in parallel (Wave 1)"
  - "get_session_token defined inline as function so Plan 02's version overrides cleanly when sourced"
  - "Background token refresh uses .session_refresh temp file (subshell cannot mutate parent vars)"
  - "grep -c output piped through tr -d '\\r' | head -1 to handle Windows CRLF artifacts in Git Bash"
metrics:
  duration: "5m 4s"
  completed_date: "2026-03-25"
  tasks_completed: 2
  files_created: 5
requirements_met:
  - RUN-01
  - RUN-03
  - RUN-08
  - RUN-09
  - RUN-10
---

# Phase 189 Plan 01: Core Scaffold and Shared Primitives — Entry Point Summary

**One-liner:** Bash audit runner entry point with IST-timestamped results, jq/curl/AUDIT_PIN prereq guards, and structured exit codes 0/1/2.

## What Was Built

`audit/audit.sh` is the operator-facing entry point for the Racing Point fleet audit system. Operators run:

```bash
AUDIT_PIN=261121 bash audit/audit.sh --mode quick
```

The script handles:
- Argument parsing for `--mode quick|standard|full|pre-ship|post-incident` plus `--tier`, `--phase`, `--auto-fix`, `--notify`, `--commit`, `--dry-run`
- Prerequisite validation (jq, curl, AUDIT_PIN env var) — each exits code 2 on failure
- IST-timestamped result directory (`audit/results/YYYY-MM-DD_HH-MM/`) with `run-meta.json`
- Auth token acquisition via `POST /api/v1/terminal/auth` using `$AUDIT_PIN` env var (never hardcoded)
- `set -u` + `set -o pipefail` but NO `set -e` (standing rule: must collect FAIL without aborting)
- Exit codes: 0=all pass, 1=some FAIL, 2=fatal prereq error

The directory skeleton (`audit/lib/`, `audit/phases/`, `audit/results/`) is tracked via `.gitkeep` files.

## Tasks

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create audit/ directory skeleton | cc35a48c | audit/lib/.gitkeep, audit/phases/.gitkeep, audit/results/.gitkeep |
| 2 | Create audit/audit.sh entry point (TDD) | 6ab2b96d | audit/audit.sh, audit/test-audit-sh.sh |

## TDD Details

**RED commit:** `a94cb94b` — 14 behavioral tests (T1-T12), all failing (audit.sh not yet created)

**GREEN commit:** `6ab2b96d` — audit.sh implemented, all 14 tests pass

Test T5 (`set -e absent`) required fixing: Windows Git Bash's `grep -c` output contains CRLF artifacts causing `[ "0\n0" -eq 0 ]` to fail. Fixed with `tr -d '\r' | head -1`.

## Decisions Made

1. **Stubs fallback for lib/core.sh** — Plans 01 and 02 are both Wave 1 (parallel). audit.sh sources `lib/core.sh` but provides inline stubs if it doesn't exist yet, so syntax check and prereq checks work independently.

2. **Token refresh via temp file** — background subshell (full mode) can't mutate parent shell variables directly. Uses `${RESULT_DIR}/.session_refresh` as a handoff file.

3. **JSON payload to temp file** — per CLAUDE.md standing rule: bash string escaping mangles `\` in inline JSON. Auth POST uses `mktemp` + `printf` + `curl -d @file`.

4. **CRLF-aware test assertions** — Windows Git Bash grep returns CRLF output; `| tr -d '\r' | head -1` normalizes for integer comparison.

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

| Check | Result |
|-------|--------|
| audit/audit.sh exists | FOUND |
| audit/lib/.gitkeep exists | FOUND |
| audit/phases/.gitkeep exists | FOUND |
| audit/results/.gitkeep exists | FOUND |
| audit/test-audit-sh.sh exists | FOUND |
| commit cc35a48c (Task 1) | FOUND |
| commit 6ab2b96d (Task 2) | FOUND |
