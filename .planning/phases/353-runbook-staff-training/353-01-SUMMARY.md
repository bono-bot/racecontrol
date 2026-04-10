---
phase: 353-runbook-staff-training
plan: "01"
subsystem: ops-documentation
tags: [runbooks, staff-training, incident-log, morning-review, comms-link, schtask]
dependency_graph:
  requires: [347-admin-staff-management, 346-cafe-menu-proxy]
  provides: [OPS-15, OPS-16, OPS-17, OPS-18, OPS-19]
  affects: [staff-training, morning-review-ritual]
tech_stack:
  added: []
  patterns: [markdown-runbooks, windows-scheduled-task-xml, bat-env-sourcing]
key_files:
  created:
    - docs/runbooks/runbook-admin-broken.md
    - docs/runbooks/runbook-staff-pin.md
    - docs/runbooks/runbook-cafe-menu.md
    - docs/runbooks/morning-review-task.xml
    - docs/runbooks/morning-review.bat
    - comms-link/data/static-commands.json (comms-link repo)
  modified:
    - comms-link/.gitignore (comms-link repo, added !data/static-commands.json exception)
decisions:
  - "Placeholder URL used for incident log Google Sheet (PLACEHOLDER-pending-creation) — must be replaced when James/Uday creates the real sheet"
  - "static-commands.json created in comms-link repo (separate from racecontrol), .gitignore exception added"
  - "morning-review.bat uses for/f to source comms-link.env — works in schtask context without interactive shell"
  - "XML calls bat via cmd.exe /C — avoids inline env var complexity in Arguments field"
  - "em-dash replaced with double-dash (--) in all bat/message strings per CLAUDE.md ASCII-only constraint"
metrics:
  duration_minutes: 15
  completed_date: "2026-04-11"
  tasks_completed: 3
  files_created: 6
  files_modified: 1
---

# Phase 353 Plan 01: Runbook One-Pagers + Morning Review Summary

**One-liner:** Three A4-printable staff runbooks (OPS-15/16/17) plus incident log placeholder and morning review schtask wired via comms-link send-message.js.

## What Was Built

### Task 0: Auto-approved (checkpoint:human-action)
Google Sheet creation step was auto-approved with placeholder URL per execution directive. All files embed `https://docs.google.com/spreadsheets/d/PLACEHOLDER-pending-creation/edit?usp=sharing` — **must be updated when James/Uday creates the real sheet.**

### Task 1: Three Printable Runbook One-Pagers
Commit: `df8155bc`

Three markdown files written to `docs/runbooks/`, each fitting on one A4 page (<=19 lines):

| File | Requirement | Content |
|------|-------------|---------|
| `runbook-admin-broken.md` | OPS-15 | Ctrl+Shift+R → incognito → WhatsApp Bono → incident log URL |
| `runbook-staff-pin.md` | OPS-16 | /staff/manage URL [requires Phase 347 deploy] → Change PIN → Verified on venue |
| `runbook-cafe-menu.md` | OPS-17 | Admin → Cafe → edit → save → verify on POS within 10s |

All three follow D-04 template: When-to-use + numbered steps + If-stuck + DO NOT section.

### Task 2: Morning Review Infrastructure
Commits: `e2ffc1db` (racecontrol), `48b8753` (comms-link)

| File | Purpose |
|------|---------|
| `docs/runbooks/morning-review.bat` | Sources comms-link.env, runs send-message.js with incident log URL |
| `docs/runbooks/morning-review-task.xml` | Windows Scheduled Task XML — MorningReview-Daily at 02:30 UTC (08:00 IST) |
| `comms-link/data/static-commands.json` | morning_review command registry entry (new file) |

**Install on James .27:**
```
schtasks /Create /XML C:\Users\bono\racingpoint\racecontrol\docs\runbooks\morning-review-task.xml /TN MorningReview-Daily
```

## Deviations from Plan

### Auto-approved checkpoint
Task 0 (Google Sheet creation) was auto-approved with placeholder URL per execution directive. The placeholder `PLACEHOLDER-pending-creation` is embedded in all 4 files that reference the incident log. When the real sheet is created, update these files:
1. `docs/runbooks/runbook-admin-broken.md` (2 occurrences)
2. `docs/runbooks/morning-review.bat` (1 occurrence)
3. `docs/runbooks/morning-review-task.xml` (1 occurrence in Description)
4. `comms-link/data/static-commands.json` (1 occurrence in command string)

### comms-link gitignore
`comms-link/data/*.json` was covered by `*.json` in .gitignore. Added `!data/static-commands.json` exception to allow tracking this committed config file (same pattern as `data/memory-sync.json` which was force-added previously).

### DO NOT count discrepancy
Plan verification comments expected `grep -c "DO NOT"` to return 3/3/2 for the three runbooks. Actual counts are 4/4/3 because the `## DO NOT:` header line itself contains "DO NOT" and counts as an additional match. Content is correct — three DO NOT bullets plus one header per runbook.

## Known Stubs

- **Incident log URL:** All files use `https://docs.google.com/spreadsheets/d/PLACEHOLDER-pending-creation/edit?usp=sharing` — this is an intentional placeholder. Plan 353-02 (training session checkpoint) depends on the real URL being set. Resolves when James/Uday creates the Google Sheet and runs a sed replacement on all 4 affected files.
- **Bono phone number in runbook-admin-broken.md:** `+91-XXXXX-XXXXX` placeholder — must be filled in with Bono's actual WhatsApp number before printing.

## NOT TESTED

- schtask registration on James .27 — deferred to Plan 353-02 checkpoint (requires manual import of XML)
- Physical printing — deferred until Phase 347 deploys (runbook-staff-pin.md references Phase 347 feature)
- comms-link message delivery via morning-review.bat — requires live COMMS_PSK in environment
- Uday sign-off — deferred to Plan 353-02 training session

## Self-Check: PASSED

| Item | Status |
|------|--------|
| docs/runbooks/runbook-admin-broken.md | FOUND |
| docs/runbooks/runbook-staff-pin.md | FOUND |
| docs/runbooks/runbook-cafe-menu.md | FOUND |
| docs/runbooks/morning-review-task.xml | FOUND |
| docs/runbooks/morning-review.bat | FOUND |
| comms-link/data/static-commands.json | FOUND |
| Task 1 commit df8155bc | FOUND |
| Task 2 commit e2ffc1db | FOUND |
| comms-link commit 48b8753 | FOUND (separate repo) |
