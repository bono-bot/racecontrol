---
phase: 172-standing-rules-sync
plan: "01"
subsystem: documentation
tags: [standing-rules, claude-md, multi-repo, sync]
dependency_graph:
  requires: [racecontrol/CLAUDE.md]
  provides: [standing rules in 14 repos]
  affects: [all James-side active repos]
tech_stack:
  added: []
  patterns: [canonical source reference, repo-specific rule subset]
key_files:
  created:
    - C:/Users/bono/racingpoint/deploy-staging/CLAUDE.md
    - C:/Users/bono/racingpoint/pod-agent/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-admin/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-api-gateway/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-discord-bot/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-google/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-mcp-calendar/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-mcp-drive/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-mcp-gmail/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-mcp-sheets/CLAUDE.md
    - C:/Users/bono/racingpoint/racingpoint-whatsapp-bot/CLAUDE.md
    - C:/Users/bono/racingpoint/rc-ops-mcp/CLAUDE.md
    - C:/Users/bono/racingpoint/whatsapp-bot/CLAUDE.md
    - C:/Users/bono/racingpoint/people-tracker/CLAUDE.md
  modified:
    - C:/Users/bono/racingpoint/racecontrol/LOGBOOK.md
decisions:
  - "people-tracker has no git remote configured — CLAUDE.md committed locally only, push skipped"
  - "people-tracker uses Python rules subset (no TypeScript rules) since it is FastAPI/YOLOv8"
  - "racingpoint-admin gets extra rules: Next.js hydration + UI must reflect config truth"
metrics:
  duration_seconds: 334
  completed_date: "2026-03-23T20:49:00+05:30"
  tasks_completed: 2
  files_created: 14
  files_modified: 1
---

# Phase 172 Plan 01: Standing Rules Sync — SUMMARY

**One-liner:** Propagated all six Standing Rule sections from racecontrol CLAUDE.md to 14 active repos as verbatim canonical subsets.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Write CLAUDE.md for deploy-staging and pod-agent | cfbf530 (deploy-staging), 622a223 (pod-agent) | 2 CLAUDE.md files |
| 2 | Write CLAUDE.md for 12 Node.js/Python repos | batch (see below) | 12 CLAUDE.md files |

## Repos Updated

| Repo | Commit | Rules Sections | Pushed |
|------|--------|----------------|--------|
| deploy-staging | cfbf530 | Deploy + Process + Code Quality (.bat) | Yes |
| pod-agent | 622a223 | Code Quality (Rust) + Deploy + Debugging | Yes |
| racingpoint-admin | 36dab4f | Code Quality (TS+Next.js) + Process + Comms | Yes |
| racingpoint-api-gateway | 61af38f | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-discord-bot | 61d4364 | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-google | 75bb703 | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-mcp-calendar | 1ee0479 | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-mcp-drive | f289035 | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-mcp-gmail | 485262f | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-mcp-sheets | 58b9b01 | Code Quality (TS) + Process + Comms | Yes |
| racingpoint-whatsapp-bot | b29f595 | Code Quality (TS) + Process + Comms | Yes |
| rc-ops-mcp | fed6a7d | Code Quality (TS) + Process + Comms | Yes |
| whatsapp-bot | ea18b20 | Code Quality (TS) + Process + Comms | Yes |
| people-tracker | 10fd1c9 | No Fake Data + Process + Comms (Python) | No — no remote |

## Repos Skipped / Notes

- **people-tracker:** No git remote configured. CLAUDE.md written and committed locally (10fd1c9). Push skipped — not a failure, just a local-only repo.
- **racecontrol:** Already has the canonical CLAUDE.md (source of truth). Not modified.
- **comms-link:** Already had CLAUDE.md updated in a prior plan. Not in scope for this plan.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written.

### Minor Notes

- people-tracker has no configured push remote — local commit only. Flagged in decisions.
- The LF→CRLF warning on all repos is expected git behavior on Windows, not an error.

## Self-Check: PASSED

All 14 CLAUDE.md files confirmed present on disk. All 13 git pushes succeeded (people-tracker has no remote — local commit only).
