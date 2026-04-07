---
phase: 333-mp-local-server
plan: 01
subsystem: game-launch
tags: [assetto-corsa, multiplayer, lobby-sync, direct-launch, acserver]

# Dependency graph
requires:
  - phase: ac-launcher-v1.0
    provides: SP direct acs.exe launch, race.ini [REMOTE] section, DETACHED_PROCESS pattern
  - phase: ac-server
    provides: AcServerManager, generate_server_cfg_ini, port_allocator, start_ac_server
provides:
  - MP launch via direct acs.exe (no Content Manager dependency)
  - Lobby sync monitor polling acServer HTTP API
  - DashboardEvent::LobbyUpdate with Forming/AllReady/Active phases
  - connected_pods tracking on AcServerInstance
affects: [admin-dashboard, kiosk-mp-booking, deploy]

# Tech tracking
tech-stack:
  added: []
  patterns: [lobby-sync-polling, proceed-anyway-timeout]

key-files:
  created: []
  modified:
    - crates/rc-agent/src/ac_launcher.rs
    - crates/racecontrol/src/ac_server.rs

key-decisions:
  - "Direct acs.exe launch for MP eliminates Content Manager dependency entirely"
  - "Lobby sync uses 3s polling of acServer HTTP /INFO endpoint (not tasklist)"
  - "120s timeout with proceed-anyway pattern for lobby sync"
  - "Content Manager functions kept as dead_code for reference, not deleted"

patterns-established:
  - "Lobby sync: poll acServer /INFO, track clients count, broadcast LobbyUpdate phases"
  - "Proceed-anyway timeout: 120s lobby wait, then race starts with available pods"

requirements-completed: []

# Metrics
duration: 41min
completed: 2026-04-07
---

# Phase 333: MP Local Server + Sync Lobby Summary

**Direct acs.exe MP launch eliminating Content Manager dependency, with lobby sync monitor polling acServer HTTP API for pod connection tracking**

## Performance

- **Duration:** 41 min
- **Started:** 2026-04-07T20:18:45Z
- **Completed:** 2026-04-07T20:59:24Z
- **Tasks:** 3
- **Files modified:** 2

## Accomplishments

### Task 1: Eliminate Content Manager for MP launches
- `direct_launch_fallback()` rewritten to use direct acs.exe with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` for both SP and MP
- Content Manager URI path (`acmanager://`) marked deprecated with `#[allow(dead_code)]`
- 5 CM-related functions (`find_cm_exe`, `launch_via_cm`, `diagnose_cm_failure`, `get_cm_exit_code`, `read_cm_log_errors`) all marked deprecated
- race.ini `[REMOTE]` section already writes `SERVER_PORT` (UDP port) correctly
- Commit: `e08f60c9`

### Task 2: Add lobby sync monitor
- `monitor_lobby_sync()` async function added to `ac_server.rs`
- Polls `GET http://127.0.0.1:{http_port}/INFO` every 3 seconds
- Tracks `clients` field from acServer JSON response
- Updates `connected_pods` on `AcServerInstance` for dashboard visibility
- Broadcasts `DashboardEvent::LobbyUpdate` with phase transitions: Forming -> AllReady -> Active
- 120s timeout: proceeds with available pods (proceed-anyway pattern)
- Gracefully exits if session is stopped/removed externally
- Spawned as background tokio task from `start_ac_server()` after pod launch commands sent
- Commit: `5567a4eb`

### Task 3: Add tests
- `test_launch_json_includes_server_port`: verifies UDP port (not HTTP port) in launch JSON
- `test_server_cfg_register_to_lobby_disabled`: LAN server must not register to Kunos lobby
- `test_server_cfg_has_all_port_fields`: UDP/TCP/HTTP ports all present
- `test_entry_list_slot_count_matches_max_clients`: N pods = N entry slots
- `test_lobby_status_serde_roundtrip`: LobbyStatus JSON roundtrip
- `test_lobby_phase_transitions`: all 5 phases serialize correctly
- `test_admin_password_deterministic`: same inputs = same password
- Commit: `eb9822fb`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] Fallback path still used Content Manager**
- **Found during:** Task 1
- **Issue:** `direct_launch_fallback()` still tried CM for MP even though primary path used direct launch
- **Fix:** Rewrote fallback to use same DETACHED_PROCESS pattern as primary path
- **Files modified:** `crates/rc-agent/src/ac_launcher.rs`
- **Commit:** `e08f60c9`

## What Already Existed (Not New)

These were already implemented before this phase:
- `use_direct_launch = true` in primary launch path (already set)
- `server_port: config.udp_port` in launch JSON (already present)
- `write_remote_section()` correctly writes `[REMOTE]` with `SERVER_PORT`
- `LobbyStatus`, `LobbyPhase` types in `rc-common/types.rs`
- `DashboardEvent::LobbyUpdate` variant in protocol
- `connected_pods` field on `AcServerInstance`

## What This Phase Added

1. **Fallback path alignment** - `direct_launch_fallback()` now matches primary path
2. **Lobby sync monitor** - `monitor_lobby_sync()` with HTTP polling, phase broadcasts, timeout
3. **Integration** - Lobby monitor spawned from `start_ac_server()` automatically
4. **Tests** - 7 new tests covering launch JSON, server config, entry list, lobby phases

## Known Stubs

None - all code paths are fully wired.

## Verification

- `cargo build --release --bin racecontrol` - passes
- `cargo build --release --bin rc-agent` - passes
- `cargo test -p racecontrol-crate -- test_lobby` - 2 tests pass
- Pre-existing test compile error (`as_deref` on `SimType` in unrelated file) blocks full test suite but is not caused by this phase

## Self-Check: PASSED

- FOUND: crates/rc-agent/src/ac_launcher.rs
- FOUND: crates/racecontrol/src/ac_server.rs
- FOUND: .planning/phases/333-mp-local-server/333-01-SUMMARY.md
- FOUND: commit e08f60c9 (ac_launcher CM elimination)
- FOUND: commit 5567a4eb (lobby sync monitor)
- FOUND: commit eb9822fb (tests)
- VERIFIED: cargo build --release --bin racecontrol (success)
- VERIFIED: cargo build --release --bin rc-agent (success)
