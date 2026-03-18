---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: verifying
stopped_at: Completed .planning/phases/44-deploy-verification-master-script/44-02-PLAN.md
last_updated: "2026-03-18T23:58:49.298Z"
last_activity: "2026-03-19 — Plan 44-01 complete: deploy verification master script (DEPL-01/02/04)"
progress:
  total_phases: 14
  completed_phases: 4
  total_plans: 10
  completed_plans: 9
  percent: 89
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-19)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v7.0 E2E Test Suite — Phase 41: Test Foundation (ready to plan)

## Current Position

Phase: 44 of 44 (Deploy Verification Master Script)
Plan: 1 of 1 (complete)
Status: Phase 44 Plan 01 complete — deploy verification master script created
Last activity: 2026-03-19 — Plan 44-01 complete: deploy verification master script (DEPL-01/02/04)

Progress: [█████████░] 89%

## Performance Metrics

**Velocity:**
- Total plans completed: 3 (v7.0 milestone)
- Average duration: 7 min
- Total execution time: 0.33 hours

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| 41-01 | 3 min | 2 | 5 |
| 41-02 | 7 min | 2 | 6 |
| 42-01 | 10 min | 2 | 3 |
| 42-02 | 2 min | 1 | 4 |
| 43-01 | 1 min | 1 | 1 |
| 43-02 | 2 min | 2 | 2 |
| 44-01 | 2 min | 1 | 1 |

**Recent Trend:** On track

*Updated after each plan completion*
| Phase 44-deploy-verification-master-script P02 | 5 | 1 tasks | 1 files |

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

(Phase 41-02 — Playwright and nextest decisions)
- Playwright 1.58.2 with bundled Chromium — msedge channel has documented 30s hang (GitHub #22776)
- fullyParallel:false and workers:1 are non-negotiable — game launch tests mutate live pod state
- reuseExistingServer:true — venue kiosk already running on :3300, must attach not restart
- baseURL defaults to http://192.168.31.23:3300 — KIOSK_BASE_URL env var overrides for dev/CI
- playwright.config.ts at repo root — auto-discovered by npx playwright test without --config flag
- cargo-nextest per-process isolation is the default — not explicitly configured in nextest.toml
- node_modules/ was missing from root .gitignore — fixed; root node_modules was being tracked in git

(Phase 42-01 — data-testid attribute decisions)
- data-testid naming is consistent between book/page.tsx and SetupWizard.tsx for shared step names (step-select-game, step-select-plan, etc.) — Phase 43 specs work against both paths
- wizard-back-btn applies to both back button instances in SetupWizard.tsx footer (non-review and review conditional blocks both use same testid)
- pin-modal placed on PinModal component's outer fixed div inside the component function body (not on render call site in CustomerLanding)
- [Phase 42]: jsErrors array scoped to module level (not testInfo metadata) — simpler than testInfo cast approach; workers:1 means no concurrency issue
- [Phase 42]: outputDir set to ./tests/e2e/results/ — collocates screenshots with test source, not at repo root test-results/
- [Phase 42]: Keyboard nav test accepts either advanced or stayed on plan step — only asserts Tab/Enter cause no JS errors as regression guard
- [Phase 43]: AC wizard BROW-02 tests preset path only (default experienceMode=preset) — custom track/car path separate concern
- [Phase 43]: session_splits and driving_settings steps use 3s isVisible guards — test survives different tier configs on live server
- [Phase 43]: Experience DB guards: empty experience list is non-fatal — step rendered without experiences is acceptable behavior
- [Phase 43-02]: Remote exec port is 8091 — MEMORY.md says 8090 but game-launch.sh line 224 uses 8091; script evidence wins
- [Phase 43-02]: Launching state accepted as API-03 pass — Steam games take 30-90s to reach Running, matches game-launch.sh Gate 6
- [Phase 43-02]: forza_horizon_5 IS in GAMES_TO_TEST; forza (Motorsport) excluded (enabled:false in constants.ts)
- [Phase 43-02]: capture_error_screenshot fires only on launch failure to reduce noise; Steam dialog dismiss fires after every accepted launch
- [Phase 44-01]: fleet/health fetched once and reused for Gates 5/6/7 — avoids 3 redundant 10s HTTP calls per verify run
- [Phase 44-01]: build_id absent from fleet/health is skip() not fail() — older rc-agent versions predate the field; absence alone does not indicate deploy failure
- [Phase 44-01]: Gate 3 EADDRINUSE detection polls :3300 up to 30s with 5s intervals — Node.js startup can take 10-20s after deploy restart
- [Phase 44-01]: log_to_ai_debugger() format is [YYYY-MM-DD HH:MM:SS] GATE: name | FAILURE: msg — one line per failure, plain text, suitable for AI debugger parsing
- [Phase 44-deploy-verification-master-script]: run-all.sh uses PIPESTATUS[0] after tee pipe to capture phase exit codes; does not source lib/common.sh (orchestrator not test script); RESULTS_DIR exported for deploy/verify.sh AI log co-location

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- BIOS AMD-V disabled on Ryzen 7 5800X — v6.0 blocked; must enable SVM Mode before Phase 36

### Blockers/Concerns

- v6.0 (Phases 36–40) is blocked on BIOS AMD-V — Phase 41 may start independently as pure test infrastructure
- Phase 42 gate: RESOLVED — 97 data-testid attributes added to book/page.tsx, SetupWizard.tsx, page.tsx in Plan 42-01
- Phase 43 gate: Steam app IDs for EA Anti-Cheat wrapped games require manual verification on Pod 8 before launch specs are written

## Session Continuity

Last session: 2026-03-18T23:58:49.295Z
Stopped at: Completed .planning/phases/44-deploy-verification-master-script/44-02-PLAN.md
Resume file: None
Next action: Phase 44 complete — all phases complete for v7.0 E2E Test Suite milestone
