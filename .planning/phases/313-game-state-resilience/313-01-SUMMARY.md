---
phase: 313-game-state-resilience
plan: 01
subsystem: game-launcher
tags: [resilience, game-state, ws-reconnect, timeout]
dependency_graph:
  requires: [312-01]
  provides: [GSTATE-01, GSTATE-02, GSTATE-03]
  affects: [game_launcher, ws-reconnect]
tech_stack:
  added: []
  patterns: [7-case-reconciliation, hard-cap-timeout, backfill-on-health-tick]
key_files:
  created: []
  modified:
    - crates/racecontrol/src/game_launcher.rs
    - crates/racecontrol/src/ws/mod.rs
decisions:
  - "Pod is always source of truth for game state, except for Launching <30s (in-flight protection)"
  - "Backfill launched_at=None on first health tick rather than inline during reconciliation"
  - "180s hard cap chosen to exceed any reasonable dynamic timeout (AC max ~120s)"
metrics:
  duration: 15m
  completed: "2026-04-03"
  tasks: 2
  files: 2
---

# Phase 313 Plan 01: Game State Resilience Summary

**One-liner:** 180s hard-cap timeout for stuck Launching trackers, immediate cleanup on stop ACK, and 7-case smart WS reconnect reconciliation with pod as source of truth.

## Tasks Completed

| Task | Name | Commit | Key Changes |
|------|------|--------|-------------|
| 1 | GSTATE-01 + GSTATE-03: Launching hard-cap timeout and stop ACK cleanup | c0219f30 | 180s hard cap, launched_at backfill, stop ACK removes tracker |
| 2 | GSTATE-02: Smart WS reconnect reconciliation | eb0db70b | 7-case merge logic, in-flight launch protection, GSTATE-02 logging |

## Implementation Details

### GSTATE-01: 180-Second Hard-Cap Timeout

In `check_game_health()`, after the existing dynamic timeout check:
- Added a second check: if elapsed > 180s AND not already timed out, force timeout regardless of `dynamic_timeout_secs` value.
- For trackers with `launched_at == None` (e.g., created from reconnect reconciliation): collected into `needs_launched_at` vec, backfilled with current time after read lock is dropped. Next 5s health tick starts the real countdown.

### GSTATE-03: Stop ACK Cleanup

In `stop_game()`, when ACK is received with `result.success == true`:
- Immediately remove the GameTracker entry from `active_games` (if still in Stopping state).
- Update pod info to `game_state = Idle`, `current_game = None`.
- The existing 30s Stopping timeout spawn is kept as a safety net for cases where ACK fails or is not received.

### GSTATE-02: Smart Reconnect Reconciliation

Replaced the blind overwrite logic in `ws/mod.rs` with a 7-case match on `(server_state, pod_game_state)`:

| Case | Server | Pod | Action |
|------|--------|-----|--------|
| 1 | None | Idle | No action |
| 2 | None | Running/Launching/Loading | Create tracker (externally_tracked=true) |
| 3 | Some(_) | Running/Launching/Loading | Update tracker state from pod |
| 4 | Launching | Idle | Keep if launched <30s ago; remove if >30s |
| 5 | Running/Loading/Stopping/Error | Idle | Remove tracker (game ended while disconnected) |
| 6 | Some(_) | Stopping/Error | Update tracker state |
| 7 | None | Stopping/Error | Create transient tracker for dashboard visibility |

Key design decision: pod is ALWAYS source of truth, except Case 4 (recent Launching <30s) protected by GSTATE-01 180s hard-cap.

## Deviations from Plan

None -- plan executed exactly as written.

## Verification

- 807 lib tests pass (0 failures)
- 39 game_launcher-specific tests pass
- Release build succeeds
- No `.unwrap()` in new production code
- GSTATE markers present: 8 in game_launcher.rs, 9 in ws/mod.rs
- 8 pre-existing integration test failures in lap/notification tests (unrelated to this plan)

## Known Stubs

None -- all code is fully wired with no placeholders.

## Self-Check: PASSED

- All modified files exist on disk
- Both task commits verified in git history (c0219f30, eb0db70b)
- SUMMARY.md created at expected path
