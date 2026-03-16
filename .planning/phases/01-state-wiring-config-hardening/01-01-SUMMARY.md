---
phase: 01-state-wiring-config-hardening
plan: "01"
subsystem: infra
tags: [rust, axum, tokio, racecontrol, rc-agent, watchdog, config-validation]

requires: []
provides:
  - "AppState::new() pre-populates pod_backoffs with EscalatingBackoff entries for pod_1 through pod_8"
  - "create_initial_backoffs() helper function testable independently of full AppState"
  - "validate_config() in rc-agent enforces pod.number 1-8, non-empty pod.name, ws:// URL prefix"
  - "load_config() returns Err on missing config file (no default fallback)"
  - "LockScreenState::ConfigError variant with branded error page render"
  - "maybe_send_first_boot_email() in racecontrol with flag file pattern"
affects:
  - "02-watchdog-hardening"
  - "pod_monitor"
  - "pod_healer"

tech-stack:
  added: []
  patterns:
    - "create_initial_backoffs() helper extracted for unit testability without constructing full AppState"
    - "Config errors shown via lock screen before process exits — customer never sees blank screen"
    - "Flag file pattern for one-time startup operations (email_verified.flag)"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/main.rs
    - crates/rc-agent/src/main.rs
    - crates/rc-agent/src/lock_screen.rs
    - crates/rc-agent/src/debug_server.rs

key-decisions:
  - "Used hardcoded 1..=8 range for pod_backoffs pre-population (not config.pods.count) per plan spec"
  - "key format is pod_{N} with underscore to match pod_monitor.rs entry() pattern"
  - "ConfigError lock screen shows generic message only — technical details logged to tracing::error! only"
  - "All validate_config errors collected before returning (not fail-fast) so admin sees all issues at once"
  - "wss:// URLs accepted in addition to ws:// to support cloud connections"

patterns-established:
  - "extract testable pure functions from struct methods to enable unit testing without heavy dependencies"
  - "start early lock screen server before config load so startup errors are always visible to staff"

requirements-completed: [WD-02, DEPLOY-01]

duration: 45min
completed: 2026-03-13
---

# Phase 1 Plan 01: State Wiring & Config Hardening Summary

**EscalatingBackoff pre-populated for all 8 pods in AppState, rc-agent fails fast with branded lock screen on missing or invalid config (no silent default fallback)**

## Performance

- **Duration:** 45 min
- **Started:** 2026-03-13T00:00:00Z
- **Completed:** 2026-03-13T00:45:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- AppState::new() now pre-populates pod_backoffs for pod_1 through pod_8 — pod_monitor never hits a missing key for known pods
- rc-agent startup validates config before running; missing file returns Err (not silent default "Pod 01")
- LockScreenState::ConfigError variant renders a branded Racing Point error page (red #E10600 header, generic message) before exit(1)
- First-boot email test added to racecontrol startup with flag file pattern to prevent repeat sends
- Fixed two pre-existing compile errors blocking rc-agent from building at all

## Task Commits

Each task was committed atomically:

1. **Task 1: Pre-populate pod_backoffs in AppState + first-boot email test** - `a7a36f1` (feat)
2. **Task 2: Harden rc-agent config validation with branded lock screen error** - `124b6f4` (feat)

## Files Created/Modified
- `crates/racecontrol/src/state.rs` - Added create_initial_backoffs() helper, wired into AppState::new(), 5 unit tests
- `crates/racecontrol/src/main.rs` - Added maybe_send_first_boot_email() with flag file pattern, called after AppState construction
- `crates/rc-agent/src/main.rs` - Added validate_config(), removed default fallback from load_config(), early lock screen server before config load, 8 unit tests
- `crates/rc-agent/src/lock_screen.rs` - Added ConfigError variant, render_config_error_page(), show_config_error() method
- `crates/rc-agent/src/debug_server.rs` - Added ConfigError arm to non-exhaustive match

## Decisions Made
- Hardcoded 1..=8 range for pod backoff initialization (not config-driven) per plan specification
- key format "pod_{N}" with underscore matches existing pod_monitor.rs entry() pattern at lines 148/315
- ConfigError shows only "Configuration Error - contact staff" — technical details go to tracing::error! only
- validate_config collects ALL errors before returning (not fail-fast) so staff sees all issues at once
- wss:// accepted alongside ws:// to support future cloud/TLS connections

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed missing installed_games field in PodInfo struct literal**
- **Found during:** Task 2 (running tests to establish baseline)
- **Issue:** rc-agent/src/main.rs used `installed_games` as a field in PodInfo struct literal, but PodInfo in rc-common/src/types.rs does not have that field — compile error blocked all testing
- **Fix:** Removed `installed_games` from PodInfo struct literal; the data is still logged via the preceding tracing::info! call
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo build -p rc-agent-crate succeeded
- **Committed in:** 124b6f4 (Task 2 commit)

**2. [Rule 1 - Bug] Fixed LockScreenManager::new called with extra argument**
- **Found during:** Task 2 (same baseline compile run)
- **Issue:** main.rs called LockScreenManager::new(lock_event_tx, config.pod.number) with 2 args, but the constructor only takes 1 (event_tx)
- **Fix:** Removed the extra config.pod.number argument
- **Files modified:** crates/rc-agent/src/main.rs
- **Verification:** cargo build -p rc-agent-crate succeeded
- **Committed in:** 124b6f4 (Task 2 commit)

**3. [Rule 1 - Bug] Fixed non-exhaustive match in debug_server.rs**
- **Found during:** Task 2 (adding ConfigError variant to LockScreenState enum)
- **Issue:** debug_server.rs had a match on LockScreenState that would fail to compile once ConfigError was added
- **Fix:** Added ConfigError arm returning "config_error" string
- **Files modified:** crates/rc-agent/src/debug_server.rs
- **Verification:** cargo test -p rc-agent-crate --no-run succeeded
- **Committed in:** 124b6f4 (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (3 Rule 1 bugs, pre-existing compile errors)
**Impact on plan:** All fixes necessary to compile and test rc-agent. No scope creep. The bugs were pre-existing from a previous commit that referenced features not yet added to rc-common.

## Issues Encountered
- EscalatingBackoff fields (attempt, last_attempt_at) are private in rc-common — tests had to use public API (attempt() method, ready() method) instead of direct field access. Tests adjusted accordingly.
- rc-agent test binary hangs when run without a filter (tokio HID/USB device access in some tests blocks indefinitely). Tests that can run without hardware (validate_config) pass cleanly with filter.

## User Setup Required
None - no external service configuration required. First-boot email is optional (only runs if email is configured in racecontrol.toml).

## Next Phase Readiness
- pod_backoffs in AppState pre-populated — Phase 2 watchdog hardening can rely on no missing-key panics
- validate_config() in place — no pod will silently start as "Pod 01" with wrong config
- Phase 2 (02-watchdog-hardening) can now safely read pod_backoffs without race on first access

## Self-Check: PASSED
- a7a36f1: FOUND in git log
- 124b6f4: FOUND in git log
- crates/racecontrol/src/state.rs: FOUND
- crates/rc-agent/src/lock_screen.rs: FOUND
- .planning/phases/01-state-wiring-config-hardening/01-01-SUMMARY.md: FOUND
- All rc-common tests (30): PASSED
- All racecontrol tests (36 including 5 new state tests): PASSED
- validate_config tests (8): PASSED

---
*Phase: 01-state-wiring-config-hardening*
*Completed: 2026-03-13*
