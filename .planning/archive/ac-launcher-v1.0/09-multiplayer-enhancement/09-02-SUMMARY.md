---
phase: 09-multiplayer-enhancement
plan: 02
subsystem: billing
tags: [multiplayer, billing-sync, group-session, timeout-eviction, tokio]

# Dependency graph
requires:
  - phase: 03-billing-synchronization
    provides: "WaitingForGameEntry, defer_billing_start, handle_game_status_update"
provides:
  - "MultiplayerBillingWait coordinator for group billing sync"
  - "group_session_id on WaitingForGameEntry for multiplayer detection"
  - "multiplayer_waiting map on BillingManager"
  - "60-second timeout eviction for non-connecting pods"
  - "Group-aware auth callers that query group_session_members"
affects: [09-multiplayer-enhancement]

# Tech tracking
tech-stack:
  added: []
  patterns: ["Group billing coordination via MultiplayerBillingWait state machine", "Timeout eviction with tokio::spawn + 60s sleep"]

key-files:
  created: []
  modified:
    - "crates/rc-core/src/billing.rs"
    - "crates/rc-core/src/auth/mod.rs"

key-decisions:
  - "MultiplayerBillingWait stores expected_pods and live_pods HashSets for O(1) membership checks"
  - "Timeout spawned once per group (timeout_spawned flag) to prevent duplicate 60s timers"
  - "Timeout consumes and removes MultiplayerBillingWait entry; late LIVE signals are no-ops"
  - "Auth callers query group_session_members table to detect multiplayer context at defer_billing_start time"
  - "Single-player backward compat: group_session_id=None triggers immediate billing start on LIVE"

patterns-established:
  - "Group billing wait: first pod creates MultiplayerBillingWait with DB query for expected pods"
  - "All-live check: live_pods.len() >= expected_pods.len() triggers billing for all waiting_entries"
  - "Timeout eviction: non-live pods removed, billing starts for live pods only"

requirements-completed: [MULT-03]

# Metrics
duration: 9min
completed: 2026-03-14
---

# Phase 9 Plan 02: Synchronized Billing Summary

**Group-aware billing coordinator that holds billing until all multiplayer participants reach STATUS=LIVE, with 60s timeout eviction for non-connecting pods**

## Performance

- **Duration:** 9 min
- **Started:** 2026-03-14T04:40:44Z
- **Completed:** 2026-03-14T04:49:22Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Synchronized billing for multiplayer sessions: billing starts for ALL group members simultaneously when every player is on-track
- 60-second connection timeout evicts non-connecting pods so remaining players can proceed
- All 4 auth call sites (PIN, QR, PWA, kiosk) now query group_session_members and pass group_session_id to defer_billing_start
- Single-player billing behavior completely unchanged (backward compatible)
- 11 new tests covering multiplayer billing coordination and timeout eviction (43 billing tests, 189 rc-core tests total, all pass)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add group_session_id, MultiplayerBillingWait, auth callers** - `d281b1d` (feat)
2. **Task 2: 60-second connection timeout tests** - `fb06b50` (test)

_TDD approach: tests and implementation in same commits since structural tests validate data flow_

## Files Created/Modified
- `crates/rc-core/src/billing.rs` - Added MultiplayerBillingWait struct, group_session_id to WaitingForGameEntry, multiplayer_waiting to BillingManager, multiplayer-aware handle_game_status_update, multiplayer_billing_timeout function, 11 new tests
- `crates/rc-core/src/auth/mod.rs` - All 4 defer_billing_start call sites now query group_session_members for multiplayer detection and pass group_session_id parameter

## Decisions Made
- MultiplayerBillingWait uses HashSet for expected_pods and live_pods for O(1) membership checks
- timeout_spawned boolean flag prevents duplicate 60s timeout spawns per group
- When timeout fires and MultiplayerBillingWait entry is already consumed (all pods connected), it's a no-op
- Auth callers use graceful fallback: .ok().flatten() on group_session_members query so DB errors don't block single-player flow
- AcStatus::Off handler cleans up from multiplayer_waiting if pod was still waiting (game crash during loading)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Multiplayer billing sync complete (MULT-03)
- Ready for remaining Phase 9 plans (lobby UI, server launch coordination)
- All 189 rc-core tests pass with zero failures

## Self-Check: PASSED

- [x] billing.rs exists
- [x] auth/mod.rs exists
- [x] Commit d281b1d exists
- [x] Commit fb06b50 exists
- [x] SUMMARY.md exists

---
*Phase: 09-multiplayer-enhancement*
*Completed: 2026-03-14*
