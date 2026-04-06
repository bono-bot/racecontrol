---
phase: 321-external-monitoring-alert-chain
plan: 03
subsystem: rc-sentry
tags: [monitoring, blanking, gdi, screen-verification]
dependency_graph:
  requires: [321-02]
  provides: [screen_verify_module, blanking_verification]
  affects: [rc-sentry, crash-handler]
tech_stack:
  added: [winapi/wingdi, winapi/winuser]
  patterns: [gdi-pixel-sampling, raii-dc-guard, thread-local-mock-testing]
key_files:
  created:
    - crates/rc-sentry/src/screen_verify.rs
  modified:
    - crates/rc-sentry/Cargo.toml
    - crates/rc-sentry/src/main.rs
decisions:
  - "Used evaluate_results() helper to separate pixel evaluation logic from GDI calls for testability"
  - "Test mock uses thread-local RefCell for pixel data -- avoids any global state conflicts"
  - "Blanking check gated on restarted && spawn_verified -- only fires when rc-agent is confirmed running"
metrics:
  duration_seconds: 328
  completed: 2026-04-06T22:22:00+05:30
  tasks_completed: 1
  tasks_total: 1
  files_changed: 3
  tests_added: 9
requirements: [MON-05]
---

# Phase 321 Plan 03: Screen Blanking Verification Summary

GDI pixel sampling to verify blanking screen (#1A1A1A) is actually displayed after rc-agent restart, with WhatsApp alert on failure.

## What Was Built

### screen_verify.rs Module
- `BlankingStatus` enum with `Blanked`, `NotBlanked`, `Unknown` variants (each carries counts)
- `verify_blanking()` function samples 9 points in a 3x3 grid on the virtual screen
- `GetPixel()` via winapi for synchronous pixel color reading (~1us per call)
- RGB tolerance of +/-10 per channel on expected color #1A1A1A (Asphalt Black)
- 80% threshold: 8+ of 9 valid points must match to declare blanked
- Screen bounds checking via `GetSystemMetrics(SM_CXVIRTUALSCREEN/SM_CYVIRTUALSCREEN)`
- RAII `DcGuard` struct ensures `ReleaseDC()` on all exit paths (no DC leak)
- Non-Windows stub returns `Unknown` for cross-compilation

### Crash Handler Integration
- After spawn-verified restart, 15-second delay for Edge to launch
- `verify_blanking()` called on crash-handler thread (does not block watchdog polling)
- `NotBlanked` triggers WhatsApp alert via `send_whatsapp_alert()` (from Plan 02)
- `Unknown` logged as warning (inconclusive)
- `Blanked` logged as info (success)
- Gated on `result.restarted && result.spawn_verified` -- only fires when rc-agent confirmed alive

### Cargo.toml
- Added `wingdi` and `winuser` features to winapi dependency

## Tests

9 unit tests using thread-local mock pixel data:

| Test | Scenario | Expected |
|------|----------|----------|
| test_blanking_all_match | All 9 points RGB(26,26,26) | Blanked{9,9} |
| test_blanking_none_match | All 9 points RGB(255,255,255) | NotBlanked{0,9} |
| test_blanking_below_threshold | 7/9 match (77.7%) | NotBlanked{7,9} |
| test_blanking_at_threshold | 8/9 match (88.8%) | Blanked{8,9} |
| test_blanking_tolerance_boundary | RGB(36,36,36) matches, RGB(37,37,37) does not | Blanked then NotBlanked |
| test_blanking_invalid_pixel_skipped | 1 CLR_INVALID excluded from total | Blanked{8,8} |
| test_color_matches_exact | RGB(26,26,26) | true |
| test_color_matches_within_tolerance | RGB(16,26,36) etc. | true |
| test_color_no_match_outside_tolerance | RGB(37,26,26) etc. | false |

## Commits

| Hash | Message |
|------|---------|
| 351eb69b | feat(321-03): add screen blanking verification via GDI pixel sampling |

## Deviations from Plan

None -- plan executed exactly as written.

## Known Stubs

None -- all functionality is fully wired.

## Decisions Made

1. **evaluate_results() helper function** -- Separated pixel evaluation logic from GDI/mock code for cleaner testability
2. **Thread-local RefCell for mocks** -- Each test sets its own mock data without global state conflicts
3. **Gated on spawn_verified** -- Blanking check only fires when rc-agent is confirmed running, avoiding false negatives when restart itself failed

## Self-Check: PASSED

- FOUND: crates/rc-sentry/src/screen_verify.rs
- FOUND: .planning/phases/321-external-monitoring-alert-chain/321-03-SUMMARY.md
- FOUND: commit 351eb69b
- All 9 tests pass
- cargo check -p rc-sentry compiles successfully
