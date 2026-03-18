---
gsd_state_version: 1.0
milestone: v7.0
milestone_name: E2E Test Suite
status: in_progress
stopped_at: "Phase 41 Plan 01 complete — shell test library created and refactored"
last_updated: "2026-03-19T21:35:20.000Z"
last_activity: 2026-03-19 — Phase 41-01 complete, lib/common.sh and lib/pod-map.sh created
progress:
  total_phases: 4
  completed_phases: 0
  total_plans: 2
  completed_plans: 1
  percent: 12
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-19)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v7.0 E2E Test Suite — Phase 41: Test Foundation (ready to plan)

## Current Position

Phase: 41 of 44 (Test Foundation)
Plan: 1 of 2 (complete)
Status: In progress — Plan 41-01 complete, Plan 41-02 next
Last activity: 2026-03-19 — Plan 41-01 complete: shell test library + pod map

Progress: [█░░░░░░░░░] 12%

## Performance Metrics

**Velocity:**
- Total plans completed: 1 (v7.0 milestone)
- Average duration: 3 min
- Total execution time: 0.05 hours

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| 41-01 | 3 min | 2 | 5 |

**Recent Trend:** On track

*Updated after each plan completion*

## Accumulated Context

### Decisions

(v7.0 E2E Test Suite — key constraints from research)
- Playwright version locked to 1.58.2 with bundled Chromium — msedge channel has documented 30s hang after headed tests
- `workers: 1` and `fullyParallel: false` are mandatory — game launch tests mutate live pod state and collide if parallelized
- `reuseExistingServer: true` is mandatory — venue kiosk is already running on :3300, Playwright must attach not restart
- data-testid attributes must be added to kiosk source (Phase 42) BEFORE any wizard spec is written (Phase 43)
- Pre-test cleanup fixture must exist BEFORE any stateful test — stale games/billing poison subsequent test runs
- Shell scripts own HTTP API verification; Playwright owns browser layer — never blur this boundary
- `/api/v1/fleet/health` is the correct endpoint for ws_connected checks (NOT `/api/v1/pods`) — confirmed pitfall
- F1 25 Steam launch ID is 3059520 (EA Anti-Cheat bootstrapper), NOT store ID 2805550 — must verify on Pod 8
- Pod 8 is the sole test target — never run launch tests on pods 1–7 (may have live customer sessions)
- run-all.sh is the final integration point — only writable once all phase scripts exist (Phase 44)

(Phase 41-01 — shell test library decisions)
- summary_exit exits with FAIL count only — skips are informational, do not cause failure
- lib/common.sh has NO set options — callers manage their own error handling (smoke.sh needs -e, game-launch.sh does not)
- pod_ip() uses hyphens (pod-1 through pod-8) matching POD_ID variable format — Python dict used underscores and silently failed
- TTY check gates ANSI colors — CI gets clean text, terminals get colors

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- BIOS AMD-V disabled on Ryzen 7 5800X — v6.0 blocked; must enable SVM Mode before Phase 36

### Blockers/Concerns

- v6.0 (Phases 36–40) is blocked on BIOS AMD-V — Phase 41 may start independently as pure test infrastructure
- Phase 42 gate: data-testid audit of kiosk/src/app/book/ must confirm attributes exist or be added before wizard specs are scoped
- Phase 43 gate: Steam app IDs for EA Anti-Cheat wrapped games require manual verification on Pod 8 before launch specs are written

## Session Continuity

Last session: 2026-03-19
Stopped at: Completed .planning/phases/41-test-foundation/41-01-PLAN.md
Resume file: None
Next action: `/gsd:execute-phase 41` (plan 41-02)
