---
phase: 172-standing-rules-sync
plan: "02"
subsystem: documentation
tags: [standing-rules, compliance, comms-link]
dependency_graph:
  requires: []
  provides: [check-rules-compliance.sh, comms-link-categorized-rules]
  affects: [all-repos]
tech_stack:
  added: []
  patterns: [categorized-rule-headers, grep-compliance-check]
key_files:
  created:
    - C:/Users/bono/racingpoint/deploy-staging/check-rules-compliance.sh
  modified:
    - C:/Users/bono/racingpoint/comms-link/CLAUDE.md
decisions:
  - "Used Write tool (not heredoc) for .sh file to ensure LF line endings on Windows Git Bash"
  - "Added Bono VPS exec and Standing Rules Sync rules to comms-link Comms section (were in racecontrol but missing from comms-link)"
  - "Added Prompt Quality Check to comms-link Process section (was missing)"
  - "Added No `any` TypeScript and Git Bash JSON rules to comms-link Code Quality section (were missing)"
metrics:
  duration_seconds: 133
  completed_date: "2026-03-23T20:45:00+05:30"
  tasks_completed: 2
  files_changed: 3
---

# Phase 172 Plan 02: comms-link Rules + Compliance Script Summary

Updated comms-link CLAUDE.md with explicit `### Comms`, `### Code Quality`, `### Process`, `### Debugging` category headers and wrote automated compliance check script at deploy-staging.

## Tasks Completed

### Task 1: Add categorized rule section headers to comms-link CLAUDE.md

**Commit:** `80db379` in comms-link repo

The "## Shared Standing Rules" section was rewritten from a flat numbered list (12 rules) to 4 explicit category sections. All existing rules were preserved and assigned to appropriate categories. Additionally, 4 rules present in racecontrol CLAUDE.md but missing from comms-link were added:

- `### Comms`: Bono VPS exec (v18.0), Standing Rules Sync (2 new rules added)
- `### Code Quality`: No `any` in TypeScript, Git Bash JSON (2 new rules added)
- `### Process`: Prompt Quality Check (1 new rule added)
- `### Debugging`: All 5-step Cause Elimination content preserved

**Verification:**
```
grep "^### " comms-link/CLAUDE.md | grep -E "Comms|Code Quality|Process|Debugging"
### Comms
### Code Quality
### Process
### Debugging
```

### Task 2: Write the standing-rules compliance check script

**Commit:** `db1e89f` in deploy-staging repo

Created `C:/Users/bono/racingpoint/deploy-staging/check-rules-compliance.sh`.

Script checks:
- 13 Node.js repos: requires `### Code Quality`, `### Process`, `### Comms`
- 1 Rust repo (pod-agent): requires `### Code Quality`, `### Deploy`, `### Debugging`
- 1 Ops repo (deploy-staging): requires `### Deploy`, `### Process`

Exits 0 with "All repos compliant" when all sections present. Exits 1 listing each missing section.

**Syntax check:** `bash -n check-rules-compliance.sh` → Syntax OK

**Smoke test output (pre plan 01):**
```
COMPLIANCE FAILURES:
  - racingpoint-admin: CLAUDE.md missing entirely (x3)
  - racingpoint-api-gateway: CLAUDE.md missing entirely (x3)
  - racingpoint-discord-bot: CLAUDE.md missing entirely (x3)
  - ... (13 repos with missing CLAUDE.md — expected, plan 01 handles these)
  - pod-agent: CLAUDE.md missing entirely (x3)
Expected failures before plan 01 runs
```
comms-link passed (sections now present). Script correctly identifies what remains for plan 01.

## Deviations from Plan

None - plan executed exactly as written.

## Self-Check: PASSED

| Item | Status |
|------|--------|
| C:/Users/bono/racingpoint/deploy-staging/check-rules-compliance.sh | FOUND |
| C:/Users/bono/racingpoint/comms-link/CLAUDE.md | FOUND |
| 172-02-SUMMARY.md | FOUND |
| Commit 80db379 (comms-link CLAUDE.md) | FOUND |
| Commit db1e89f (compliance script) | FOUND |
