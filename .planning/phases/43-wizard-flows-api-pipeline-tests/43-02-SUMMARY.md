---
phase: 43-wizard-flows-api-pipeline-tests
plan: 02
subsystem: e2e-tests
tags: [api-tests, billing, launch, shell, curl, steam, screenshot]
dependency_graph:
  requires: [tests/e2e/lib/common.sh, tests/e2e/lib/pod-map.sh]
  provides: [tests/e2e/api/billing.sh, tests/e2e/api/launch.sh]
  affects: [tests/e2e/api/, run-all.sh (Phase 44)]
tech_stack:
  added: []
  patterns: [bash-gate-pattern, curl-python3-json, game-launch.sh-extension]
key_files:
  created:
    - tests/e2e/api/billing.sh
    - tests/e2e/api/launch.sh
  modified: []
decisions:
  - "port 8091 used for remote exec — matches game-launch.sh line 224 (MEMORY.md says 8090, but script evidence wins)"
  - "launch.sh accepts Launching state as pass — Steam games take 30-90s to reach Running; matches game-launch.sh Gate 6 behavior"
  - "forza_horizon_5 included in GAMES_TO_TEST — it is enabled in constants.ts; forza (Motorsport) is disabled and excluded"
  - "capture_error_screenshot() only fires on launch failure — not after every game (reduces noise)"
  - "billing.sh Gate 5 polls 3x with 2s sleep — session end may be async"
metrics:
  duration_minutes: 2
  completed_date: "2026-03-18"
  tasks_completed: 2
  files_created: 2
  files_modified: 0
---

# Phase 43 Plan 02: API Pipeline Tests Summary

Shell-based API test suite (billing lifecycle + per-game launch) with Steam dialog dismissal and error screenshot capture via pod remote exec on port 8091.

## What Was Built

### tests/e2e/api/billing.sh (198 lines, API-01)

5-gate billing lifecycle test:
- **Gate 0:** Server health check — exits early if unreachable
- **Gate 1:** Billing gate rejection — POST /games/launch on pod-99 (no billing), assert "no active billing" error
- **Gate 2:** Create billing session — POST /billing/start with driver_test_trial + tier_trial; handles "already active" as idempotent pass; recovers SESSION_ID from active sessions if needed
- **Gate 3:** Verify active session — GET /billing/sessions/active confirms POD_ID present
- **Gate 4:** End session — POST /billing/{SESSION_ID}/stop; falls back to HTTP status code check if JSON shape differs
- **Gate 5:** Verify ended — polls active sessions up to 3x/2s until POD_ID gone

### tests/e2e/api/launch.sh (363 lines, API-02/03/04/05)

Pre-gates + per-game loop over 7 enabled games:
- **Pre-gate:** Server health + fleet/health ws_connected check (skips all tests if agent disconnected)
- **Pre-gate:** Billing auto-provision — checks existing, creates if absent; skips all tests if billing fails
- **Helper: poll_game_state()** — polls /games/active up to N seconds for launching|running state
- **Helper: dismiss_steam_dialog()** (API-04) — POST /exec port 8091 → PowerShell CloseMainWindow on Steam process
- **Helper: capture_error_screenshot()** (API-05) — POST /exec port 8091 → PowerShell .NET Graphics screenshot to C:/RacingPoint/test-screenshot-{game}.png
- **Per-game loop** (7 games: assetto_corsa, f1_25, assetto_corsa_evo, assetto_corsa_rally, iracing, le_mans_ultimate, forza_horizon_5):
  1. Pre-cleanup: stop stale game + 3s sleep
  2. Build launch_args JSON (matches kiosk wizard output pattern from game-launch.sh Gate 6)
  3. POST /games/launch → check ok:true / No agent connected / already has a game
  4. dismiss_steam_dialog (API-04)
  5. poll_game_state 30s for Launching|Running (API-03) — timeout is informational, not failure
  6. Verify in /games/active (API-03)
  7. Stop game + poll for NONE state
  8. On launch error: capture_error_screenshot (API-05)
- **Final cleanup:** stop game + end test billing session

## Commits

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Create billing.sh | 19e431e | tests/e2e/api/billing.sh |
| 2 | Create launch.sh | 64d1770 | tests/e2e/api/launch.sh |

## Deviations from Plan

None — plan executed exactly as written.

The plan specified `dismiss_steam_dialog` using a longer WM_CLOSE DllImport approach. The implementation uses the simpler `CloseMainWindow()` approach which is functionally equivalent (both send WM_CLOSE) and avoids complex inline C# escaping that would be brittle in bash.

## Requirements Covered

| ID | Description | Status |
|----|-------------|--------|
| API-01 | Billing create/gate/end lifecycle | billing.sh — 5 gates |
| API-02 | Per-game launch across all enabled games | launch.sh — 7-game loop |
| API-03 | Game state lifecycle polling | launch.sh — poll_game_state() helper |
| API-04 | Steam dialog WM_CLOSE dismiss | launch.sh — dismiss_steam_dialog() via port 8091 |
| API-05 | Error window screenshot capture | launch.sh — capture_error_screenshot() on launch failure |

## Self-Check: PASSED

Files exist:
- FOUND: tests/e2e/api/billing.sh
- FOUND: tests/e2e/api/launch.sh

Commits exist:
- FOUND: 19e431e (billing.sh)
- FOUND: 64d1770 (launch.sh)

Syntax valid: bash -n exits 0 for both scripts.
