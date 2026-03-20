---
phase: 57-session-end-safety
plan: 03
subsystem: ffb
tags: [openffboard, hid, ffb, safety, power-cap, hardware-validation]

# Dependency graph
requires:
  - phase: 57-02-session-end-orchestrator
    provides: safe_session_end(), close_conspit_link(), restart_conspit_link(), all 10 call sites wired
provides:
  - Startup power cap (80%) via set_gain(80) at rc-agent boot
  - Hardware-validated session-end safety sequence across all 4 games
affects: [58-conspit-link-process-hardening, 61-ffb-preset-tuning]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Startup power cap via set_gain(80) immediately after HID detection probe"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Power cap placed after zero_force_with_retry() probe — ensures wheelbase is detected before attempting set_gain"
  - "set_gain(80) failure is warn-level (not error) — missing wheelbase at boot is expected on headless pods"
  - "Idlespring target=2000 confirmed acceptable on hardware — no tuning adjustment needed"

patterns-established:
  - "Startup safety: zero_force_with_retry() probe followed by set_gain(80) power cap"

requirements-completed: [SAFE-01, SAFE-04]

# Metrics
duration: 2min
completed: 2026-03-20
---

# Phase 57 Plan 03: Startup Power Cap + Hardware Validation Summary

**80% startup power cap wired via set_gain(80) at boot, hardware-validated on canary pod across all 4 games with correct wheel centering**

## Performance

- **Duration:** 2 min (code task) + hardware verification checkpoint
- **Started:** 2026-03-20T09:00:00Z
- **Completed:** 2026-03-20T09:15:00Z
- **Tasks:** 2 (1 auto + 1 checkpoint:human-verify)
- **Files modified:** 1

## Accomplishments
- Added set_gain(80) startup power cap in main.rs immediately after HID wheelbase detection
- Caps maximum force to 80% (9.6Nm on 12Nm bases, 6.4Nm on 8Nm bases) for venue safety
- Hardware verification on canary pod confirmed full session-end safety sequence works on all 4 games
- Phase 57 complete: all SAFE-01 through SAFE-07 requirements satisfied

## Task Commits

Each task was committed atomically:

1. **Task 1: Add startup power cap set_gain(80) in main.rs** - `17637aa` (feat)
2. **Task 2: Hardware verification on canary pod** - checkpoint:human-verify (approved, no code commit)

## Files Created/Modified
- `crates/rc-agent/src/main.rs` - Added set_gain(80) call after zero_force_with_retry() probe at startup

## Decisions Made
- Power cap placed after zero_force_with_retry() — wheelbase must be detected before set_gain can succeed
- Ok(false) from set_gain logged at debug level (no wheelbase found is normal during headless boot)
- Err from set_gain logged at warn level (HID write failure worth investigating but not fatal)
- Idlespring target=2000 works well on hardware — no adjustment needed from Plan 02 default

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 57 (Session-End Safety) is fully complete — all 7 SAFE requirements verified on hardware
- Phase 58 (ConspitLink Process Hardening) can proceed
- The safe_session_end() orchestrator, HID building blocks, and startup power cap are all production-ready
- Fleet deployment: build rc-agent --release and deploy to all pods via staging HTTP server

## Self-Check: PASSED
- 57-03-SUMMARY.md: FOUND
- Commit 17637aa: FOUND

---
*Phase: 57-session-end-safety*
*Completed: 2026-03-20*
