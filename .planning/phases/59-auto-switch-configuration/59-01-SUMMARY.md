---
phase: 59-auto-switch-configuration
plan: 01
subsystem: infra
tags: [rust, ffb, conspit-link, rc-agent, json, startup-self-heal]

# Dependency graph
requires:
  - phase: 58-conspitlink-process-hardening
    provides: restart_conspit_link_hardened(), backup_conspit_configs(), _impl(Option<&Path>) testable pattern
provides:
  - ensure_auto_switch_config() in ffb_controller.rs — places Global.json at C:\RacingPoint\ with AresAutoChangeConfig=open
  - place_global_json() — atomic write, serde_json parse+modify, compare-before-write
  - verify_game_to_base_config() — detects/adds missing VENUE_GAME_KEYS entries
  - Startup wiring in main.rs — runs before enforce_safe_state
  - 9 unit tests covering all behaviors (place, noop, atomic, game keys, full, no-restart)
affects:
  - 59-02 (if exists): builds on same ensure_auto_switch_config function
  - 60-pre-launch-profile-loading: depends on correct GameToBaseConfig.json mappings
  - 61-ffb-preset-tuning: verify_game_to_base_config logs missing .Base files for Phase 61 to fix

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "ensure_auto_switch_config_impl(install_dir: Option<&Path>, runtime_dir: Option<&Path>) — dual-override _impl() for both install and runtime dirs"
    - "Conditional CL restart: gated on install_dir.is_none() so tests never trigger real restart_conspit_link_hardened()"
    - "Compare-before-write: serde_json parse + force field + serialize, compare string equality to avoid unnecessary restart"
    - "Atomic write: write to .json.tmp then std::fs::rename to final target"

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ffb_controller.rs
    - crates/rc-agent/src/main.rs

key-decisions:
  - "Added dual-override _impl() pattern (separate install_dir and runtime_dir) since the function needs to test both source and destination independently — unlike backup which uses a single base_dir"
  - "Conditional restart gating: install_dir.is_none() means production path, so real restart_conspit_link_hardened only fires in production — never in tests"
  - "verify_game_to_base_config logs warning but does NOT fix missing .Base file paths in Phase 59 — that requires pod inspection in Phase 61"
  - "VENUE_GAME_KEYS constant: 3 confirmed keys (AC, F1 25, ACC); AC EVO/AC Rally keys deferred to Phase 61 pod inspection"

patterns-established:
  - "Dual-dir _impl() pattern: when a function needs independent install_base and runtime_base overrides, use two Option<&Path> params rather than a single base_dir"
  - "Test-gated restart: production-only side effects gated on install_dir.is_none() so unit tests never trigger hardware/process operations"

requirements-completed: [PROF-01, PROF-02, PROF-04]

# Metrics
duration: 25min
completed: 2026-03-24
---

# Phase 59 Plan 01: Auto-Switch Configuration Summary

**rc-agent startup self-heal: places Global.json at C:\RacingPoint\ forcing AresAutoChangeConfig=open, verifies GameToBaseConfig.json game mappings, and restarts ConspitLink only when config changed**

## Performance

- **Duration:** 25 min
- **Started:** 2026-03-24T12:43:00Z
- **Completed:** 2026-03-24T13:04:26Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- `ensure_auto_switch_config()` + `_impl()` added to `ffb_controller.rs` with dual-dir testable pattern
- `place_global_json()` uses serde_json parse+force+serialize with compare-before-write and atomic rename
- `verify_game_to_base_config()` detects/adds missing VENUE_GAME_KEYS, warns on missing .Base files (fix deferred to Phase 61)
- Wired into `main.rs` as `spawn_blocking` BEFORE the 8s-delayed `enforce_safe_state` block
- 9 unit tests cover all required behaviors; `cargo test auto_switch` and `cargo check` both pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement ensure_auto_switch_config with unit tests** - `fde3cd87` (feat)
2. **Task 2: Wire ensure_auto_switch_config into main.rs startup sequence** - `9540c91b` (feat)

## Files Created/Modified
- `crates/rc-agent/src/ffb_controller.rs` - Added ensure_auto_switch_config(), place_global_json(), verify_game_to_base_config(), AutoSwitchConfigResult struct, ensure_auto_switch_config_in_dir() test helper, 9 unit tests (+519 lines)
- `crates/rc-agent/src/main.rs` - Inserted spawn_blocking call for ensure_auto_switch_config() before delayed startup cleanup block (+23 lines)

## Decisions Made
- Dual-override `_impl()` pattern (two `Option<&Path>` params) rather than a single `base_dir` — the function needs to test both source (install dir) and destination (runtime dir) independently
- Conditional CL restart: `if install_dir.is_none()` gates the real `restart_conspit_link_hardened()` call — tests set the flag without triggering real restart
- `verify_game_to_base_config` logs warnings for missing `.Base` file paths but does not fix them — Phase 61 pod inspection is needed to confirm exact paths on hardware
- VENUE_GAME_KEYS has 3 confirmed entries; AC EVO / AC Rally keys are deferred to Phase 61 (Pitfall 6 in RESEARCH.md)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None. All test frameworks and dependencies (serde_json) were already in the workspace.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `ensure_auto_switch_config()` is fully implemented and wired — ready for Pod 8 canary deploy + manual verification (human tests ConspitLink auto-switch on hardware)
- Phase 61 (FFB Preset Tuning) can build directly on `verify_game_to_base_config()` to fix missing `.Base` file paths once actual pod game keys are confirmed
- Before deploying: `touch crates/rc-agent/build.rs && cargo build --release --bin rc-agent` to embed fresh GIT_HASH

---
*Phase: 59-auto-switch-configuration*
*Completed: 2026-03-24*
