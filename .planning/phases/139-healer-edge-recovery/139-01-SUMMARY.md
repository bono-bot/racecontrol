---
phase: 139-healer-edge-recovery
plan: 01
subsystem: infra
tags: [rust, websocket, pod-healer, protocol, edge-browser, rc-common]

# Dependency graph
requires:
  - phase: 101-process-guard
    provides: CoreToAgentMessage WS protocol framework used as insertion point
  - phase: 137-browser-watchdog
    provides: Lock screen health check and rc_agent_healthy diagnostic flag
provides:
  - CoreToAgentMessage::ForceRelaunchBrowser variant in rc-common protocol
  - HealAction relaunch_lock_screen in pod_healer Rule 2 WS dispatch
  - Soft recovery path: WS alive + HTTP fail -> relaunch before restart
affects: [rc-agent, 139-02, ws-handler, lock-screen-manager]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Soft WS recovery before hard restart: check WS liveness, dispatch protocol command, skip shell exec"
    - "Billing guard on all disruptive healer actions (billing active -> warn only, never disrupt session)"

key-files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/pod_healer.rs

key-decisions:
  - "ForceRelaunchBrowser uses tag+content serde (matching existing CoreToAgentMessage pattern) serializing as snake_case"
  - "execute_heal_action relaunch_lock_screen arm returns early before cmd match (no shell exec path for WS actions)"
  - "Pre-existing test suite has env-var contamination flakiness (config/crypto tests fail on parallel run, pass in isolation) - pre-existing, not caused by this plan"

patterns-established:
  - "WS dispatch in healer: read agent_senders -> get by pod_id -> send with match (Ok/Err), no unwrap"
  - "Billing guard placement: check has_active_billing before pushing any HealAction that would disrupt user experience"

requirements-completed: [HEAL-01, HEAL-02]

# Metrics
duration: 35min
completed: 2026-03-22
---

# Phase 139 Plan 01: Healer Edge Recovery Summary

**ForceRelaunchBrowser WS protocol variant added to rc-common and wired into pod_healer Rule 2 for soft Edge lock screen recovery over live WebSocket before escalating to restart**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-03-22T09:30:00+05:30
- **Completed:** 2026-03-22T10:05:00+05:30
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `CoreToAgentMessage::ForceRelaunchBrowser { pod_id: String }` to rc-common protocol with snake_case serde serialization and roundtrip test
- Replaced pod_healer Rule 2 `has_active_ws` debug-log stub with full dispatch: WS alive + no billing -> push `relaunch_lock_screen` HealAction; WS alive + billing active -> warn and skip
- Added `execute_heal_action` `relaunch_lock_screen` arm that sends `ForceRelaunchBrowser` via `state.agent_senders` with match-based error handling (no unwrap, no shell exec)
- cargo build --release --bin racecontrol: Finished with no compile errors

## Task Commits

Each task was committed atomically:

1. **Task 1: Add ForceRelaunchBrowser to CoreToAgentMessage** - `0e704e1` (feat)
2. **Task 2: Add relaunch_lock_screen HealAction and Rule 2 WS dispatch** - `ad3aaf0` (feat)

**Plan metadata:** (docs commit — see below)

_Note: Both tasks used TDD. Task 1 RED confirmed compile error on missing variant; GREEN added the variant. Task 2 logic tests passed at RED (pure condition tests), GREEN added the actual dispatch code._

## Files Created/Modified

- `crates/rc-common/src/protocol.rs` - Added `ForceRelaunchBrowser { pod_id: String }` variant after `ClearMaintenance` + `test_force_relaunch_browser_roundtrip` test
- `crates/racecontrol/src/pod_healer.rs` - Updated `CoreToAgentMessage` import, rewrote Rule 2 `has_active_ws` branch, added `execute_heal_action` `relaunch_lock_screen` arm, added 3 new unit tests

## Decisions Made

- `ForceRelaunchBrowser` placed between `ClearMaintenance` and `UpdateProcessWhitelist` in the enum — logical grouping with other maintenance/recovery commands
- `execute_heal_action` returns early for `relaunch_lock_screen` before the `cmd` match, keeping WS-based actions cleanly separated from shell-exec actions
- Pre-existing flaky test failures (`config_fallback_preserved_when_no_env_vars`, `load_keys_valid_hex`) confirmed pre-existing (fail in full parallel run, pass in isolation) — not caused by this plan; out of scope

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Pre-existing test suite has env-var contamination causing intermittent failures when all tests run in parallel. Confirmed pre-existing by verifying failures occur on the baseline (before changes). All new tests and all pod_healer tests pass cleanly.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- `ForceRelaunchBrowser` protocol variant is live in rc-common — rc-agent needs a handler to act on it (close Edge processes, relaunch lock screen browser)
- pod_healer will dispatch `ForceRelaunchBrowser` to any pod that has a live WS but a failing HTTP lock screen probe (and no active billing session)
- Phase 139-02 should implement the rc-agent handler for `ForceRelaunchBrowser`

## Self-Check: PASSED

- FOUND: crates/rc-common/src/protocol.rs
- FOUND: crates/racecontrol/src/pod_healer.rs
- FOUND: .planning/phases/139-healer-edge-recovery/139-01-SUMMARY.md
- FOUND commit: 0e704e1 (feat: ForceRelaunchBrowser protocol variant)
- FOUND commit: ad3aaf0 (feat: relaunch_lock_screen HealAction + Rule 2 WS dispatch)
- FOUND commit: 85a8d76 (docs: SUMMARY + STATE + ROADMAP)

---
*Phase: 139-healer-edge-recovery*
*Completed: 2026-03-22*
