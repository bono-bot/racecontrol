---
phase: 05-synchronized-group-play
plan: 01
subsystem: multiplayer
tags: [assetto-corsa, multiplayer, billing, ac-server, coordinated-launch, continuous-mode]

# Dependency graph
requires:
  - phase: 04-multiplayer-server-lifecycle
    provides: "AcServerManager, check_and_stop_multiplayer_server, group_sessions.ac_session_id, book_multiplayer, book_multiplayer_kiosk"
provides:
  - "validate_pin() correctly identifies group members via find_group_session_for_token() and calls on_member_validated()"
  - "Coordinated AC launch: server starts only when ALL group members validate PINs (not at booking time)"
  - "Continuous mode: staff can enable auto-restart races via POST /ac/session/{id}/continuous"
  - "monitor_continuous_session() detects acServer process exit, checks billing, restarts or stops"
affects: [05-02, group-play, billing-lifecycle]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Deferred AC server start: booking creates tokens, server starts on PIN validation completion"
    - "Mutable session_id loop in monitor (no recursive tokio::spawn) to handle session_id changes across restarts"
    - "serde(default) on new fields for rolling deploy backward compatibility"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/auth/mod.rs
    - crates/racecontrol/src/multiplayer.rs
    - crates/racecontrol/src/ac_server.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/rc-common/src/types.rs

key-decisions:
  - "Use find_group_session_for_token(token_id) instead of broken pod+status query — token_id unambiguously identifies group membership at status='accepted'"
  - "Remove AC server start from book_multiplayer and book_multiplayer_kiosk — defer to on_member_validated()->start_ac_lan_for_group()"
  - "Mutable current_session_id loop in monitor avoids recursive tokio::spawn (which failed Send check due to !Send Child in AcServerInstance)"
  - "Continuous mode guard in check_and_stop_multiplayer_server: defer stop to monitor loop when flag active"

patterns-established:
  - "GROUP-01: validate_pin group detection via token_id lookup, not pod+status lookup"
  - "GROUP-02: Continuous mode opt-in via POST /ac/session/{id}/continuous, monitor loop owns lifecycle"

requirements-completed: [GROUP-01, GROUP-02]

# Metrics
duration: 18min
completed: 2026-03-16
---

# Phase 5 Plan 01: Synchronized Group Play — Coordinated Launch + Continuous Mode Summary

**PIN-gated coordinated AC launch (all pods start simultaneously when all members validate) and staff-toggleable continuous mode that auto-restarts races within 15s as long as any billing is active**

## Performance

- **Duration:** 18 min
- **Started:** 2026-03-16T00:00:00Z
- **Completed:** 2026-03-16T00:18:00Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments

- Wired validate_pin() to call on_member_validated() for group members — the coordinated launch now actually triggers when the last member validates
- Fixed the broken group_session_id lookup (was querying status='validated' at PIN time when members are status='accepted' — always returned None)
- Removed premature AC server start from book_multiplayer() and book_multiplayer_kiosk() — server now starts only after all PINs validated
- Added continuous_mode field to AcServerInstance + AcServerInfo; staff enable via POST /ac/session/{id}/continuous
- monitor_continuous_session() polls every 5s, detects acServer process exit, checks billing timers, auto-restarts with 10s delay or stops cleanly

## Task Commits

1. **Task 1: Wire coordinated launch** - `74b4c8a` (feat)
2. **Task 2: Add continuous mode** - `2a27a96` (feat)

## Files Created/Modified

- `crates/racecontrol/src/auth/mod.rs` — Replaced broken group_session_id query with find_group_session_for_token(); added on_member_validated() call; skips launch_or_assist() for group members
- `crates/racecontrol/src/multiplayer.rs` — Removed AC server start from book_multiplayer() and book_multiplayer_kiosk() (both MULTI-01 blocks); removed now-unused `use crate::ac_server` import
- `crates/racecontrol/src/ac_server.rs` — Added continuous_mode + group_session_id fields to AcServerInstance; set_continuous_mode(); monitor_continuous_session() with mutable session loop
- `crates/racecontrol/src/billing.rs` — Added continuous_mode guard in check_and_stop_multiplayer_server() to defer stop to monitor loop
- `crates/racecontrol/src/api/routes.rs` — Added POST /ac/session/{session_id}/continuous route and ac_server_set_continuous handler
- `crates/rc-common/src/types.rs` — Added continuous_mode: bool with serde(default) to AcServerInfo

## Decisions Made

- Used find_group_session_for_token(token_id) not pod+status query — token_id is the correct identifier since the auth_token_id is stored in group_session_members at booking time
- Removed AC server start from booking entirely — on_member_validated() already called start_ac_lan_for_group() when all_validated=true, this was the correct completion point
- Monitor loop uses mutable current_session_id variable instead of recursive tokio::spawn — recursive spawn failed because AcServerInstance contains std::process::Child which is !Send, making the future !Send; the loop approach avoids the problem entirely
- staff_book_multiplayer() left unchanged — it bypasses PIN validation (all pre-validated), correct to still start immediately

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Removed unused `use crate::ac_server` import from multiplayer.rs**
- **Found during:** Task 1 (removing book_multiplayer AC server start block)
- **Issue:** After removing both AC server start calls from booking functions, the direct `use crate::ac_server` import became unused (start_ac_lan_for_group still uses crate::ac_server:: fully qualified path)
- **Fix:** Removed the unused `use crate::ac_server;` line to eliminate warning
- **Files modified:** crates/racecontrol/src/multiplayer.rs
- **Verification:** cargo build passes cleanly, no unused import warning
- **Committed in:** 74b4c8a (Task 1 commit)

**2. [Rule 1 - Bug] Rewrote monitor_continuous_session to avoid recursive tokio::spawn**
- **Found during:** Task 2 (implementing monitor loop)
- **Issue:** Original design used `tokio::spawn(monitor_continuous_session(...))` inside the monitor for restart. Compiler rejected this: AcServerInstance contains std::process::Child which is !Send on Windows, making the future !Send, failing the tokio::spawn Send bound
- **Fix:** Replaced recursive spawn with mutable `current_session_id` loop — same semantics, no nested spawn needed. The outer spawn in routes.rs (the initial spawn) still works because it only moves an Arc<AppState> + String into the closure
- **Files modified:** crates/racecontrol/src/ac_server.rs
- **Verification:** cargo build passes, all 344 tests pass
- **Committed in:** 2a27a96 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 - Bug)
**Impact on plan:** Both fixes necessary for correctness. The import cleanup is cosmetic. The monitor refactor is required for compilation. No scope creep.

## Issues Encountered

- `std::process::Child` is `!Send` on Windows — blocked `tokio::spawn` of recursive monitor call. Resolved by converting to iterative loop with mutable session_id tracking.

## Next Phase Readiness

- Coordinated launch path fully wired: booking → PIN validation → on_member_validated → start_ac_lan_for_group → LaunchGame all pods
- Continuous mode operational: staff POST endpoint + monitor loop handles restart/stop lifecycle
- Phase 5 Plan 02 (join failure recovery) can proceed — the coordinated launch path is now tested and working
- 344 tests passing (238 racecontrol + 106 rc-common)

---
*Phase: 05-synchronized-group-play*
*Completed: 2026-03-16*
