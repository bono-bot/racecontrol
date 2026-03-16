---
phase: 27-tailscale-mesh-internet-fallback
plan: 02
subsystem: infra
tags: [tailscale, mesh, relay, tokio, broadcast, axum, bono, rust]

# Dependency graph
requires:
  - phase: 27-01
    provides: BonoConfig struct, bono_relay.rs skeleton with BonoEvent/RelayCommand enums, spawn() stub
provides:
  - Full bono_relay::spawn() with tokio broadcast receiver loop that POSTs events to Bono's webhook
  - push_event() using state.http_client with 5s timeout
  - handle_command() Axum handler with X-Relay-Secret auth validation
  - build_relay_router() returning Router with /relay/command POST + /relay/health GET
  - AppState.bono_event_tx broadcast::Sender<BonoEvent> field (capacity 256)
  - 4 bono_relay unit tests (3 existing + relay_command_serialization)
affects:
  - 27-03 (Plan 03 wires build_relay_router() into main.rs second TcpListener on Tailscale IP)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "tokio broadcast receiver loop: subscribe() then match rx.recv().await on Ok/Lagged/Closed — mirrors dashboard_tx subscriber pattern"
    - "push_event() 5s timeout (not 30s like cloud_sync) — relay events are ephemeral, fail-fast is correct"
    - "X-Relay-Secret auth: simple shared-secret header validation, no JWT — machine-to-machine on Tailscale IP only"
    - "build_relay_router() returns Router with with_state(state) — called by main.rs to get second Axum app for Tailscale listener"
    - "RecvError::Lagged is warn not error — dropped events acceptable, relay is best-effort for ops visibility"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/bono_relay.rs
    - crates/racecontrol/src/state.rs

key-decisions:
  - "push_event timeout 5s not 30s: relay events are ephemeral (session_start, lap_recorded); missing one is acceptable but blocking the loop for 30s is not"
  - "bono_event_tx capacity 256 (vs dashboard_tx 1024): events are low-frequency (billing/lap events only), 256 is generous headroom"
  - "handle_command() rejects when expected_secret is empty: enforces relay_secret is configured before accepting any commands"
  - "RelayCommand::LaunchGame defers to Phase 28: relay channel is established and authenticated, game_launcher wiring is Phase 28 scope"
  - "GetStatus uses p.number not p.pod_number: PodInfo field is number (auto-fixed during Task 2)"

patterns-established:
  - "bono relay event push: send via bono_event_tx.send(event) from anywhere in racecontrol — spawn() subscriber loop handles the HTTP POST"

requirements-completed:
  - TS-02
  - TS-03
  - TS-04

# Metrics
duration: 15min
completed: 2026-03-16
---

# Phase 27 Plan 02: Bono Relay Full Implementation Summary

**tokio broadcast event push loop + X-Relay-Secret Axum handler wired to AppState.bono_event_tx, 248 tests green**

## Performance

- **Duration:** ~15 min
- **Started:** 2026-03-16T11:38:11Z
- **Completed:** 2026-03-16T11:53:00Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- AppState gains `bono_event_tx: broadcast::Sender<BonoEvent>` with capacity 256, initialized in `AppState::new()`
- Full `spawn()` replaces skeleton: subscribes to broadcast channel, POSTs each event to webhook URL with 5s timeout, handles Lagged/Closed error cases
- `handle_command()` Axum handler validates X-Relay-Secret header, routes LaunchGame/StopGame/GetStatus commands (LaunchGame deferred to Phase 28)
- `build_relay_router()` returns Router ready for Plan 03 to bind to Tailscale IP
- 4 bono_relay tests pass (3 preserved + new relay_command_serialization), full suite at 248 tests with 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Add bono_event_tx broadcast channel to AppState** - `edd3c83` (feat)
2. **Task 2: Implement full bono_relay.rs — spawn loop, push_event, handle_command** - `ac886a7` (feat)

## Files Created/Modified

- `crates/racecontrol/src/state.rs` - Added bono_event_tx field and initialization in AppState::new()
- `crates/racecontrol/src/bono_relay.rs` - Full implementation: spawn() loop, push_event(), handle_command(), build_relay_router(), relay_health(), relay_command_serialization test

## Decisions Made

- **5s timeout for push_event:** Webhook events are ephemeral — if Bono's VPS is down, missing one lap_recorded event is fine. Blocking the receiver loop for 30s (like cloud_sync) would queue up events. Fail fast and log.
- **bono_event_tx capacity 256:** Dashboard events can burst (all pods simultaneously), hence 1024. Bono relay events are billing/session lifecycle — 8 pods * most active scenario is under 50 events before drain.
- **Reject when relay_secret empty:** `if expected_secret.is_empty() || provided_secret != expected_secret` — ensures the endpoint is never accidentally open when relay_secret isn't configured.
- **LaunchGame/StopGame return "queued":** Phase 27 establishes the relay channel; actual pod command dispatch via game_launcher is Phase 28 scope. The endpoint is functional and authenticated.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] PodInfo.number not PodInfo.pod_number**
- **Found during:** Task 2 (implement full bono_relay.rs)
- **Issue:** Plan's GetStatus code used `p.pod_number` but PodInfo struct field is `p.number` (confirmed in rc-common/src/types.rs)
- **Fix:** Changed `p.pod_number == pod_number` to `p.number == pod_number` in handle_command() GetStatus arm
- **Files modified:** crates/racecontrol/src/bono_relay.rs
- **Verification:** cargo build -p racecontrol-crate compiles cleanly; 4 tests pass
- **Committed in:** ac886a7 (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (Rule 1 - Bug)
**Impact on plan:** Necessary correctness fix — wrong field name caused compile error E0609. No scope creep.

## Issues Encountered

None beyond the auto-fixed PodInfo field name above. rc-agent-crate test output files experienced a system-level ENOENT issue (another Claude Code process cleanup) but the crate builds with exit 0 and my changes only modify racecontrol-crate which has no dependency on rc-agent-crate.

## User Setup Required

None - no external service configuration required. Tailscale installation and racecontrol.toml `[bono]` configuration will be documented in a later plan when the Tailscale listener is wired up (Plan 03).

## Next Phase Readiness

- Plan 27-03 can now call `bono_relay::spawn(state.clone())` from main.rs and `bono_relay::build_relay_router(state)` to bind to Tailscale IP
- Any module can emit events via `state.bono_event_tx.send(BonoEvent::SessionStart { ... })`
- 248 tests green, no regressions
- relay_secret validation enforced — Plan 03 config documentation should note relay_secret is required

## Self-Check: PASSED

- FOUND: crates/racecontrol/src/bono_relay.rs (full implementation)
- FOUND: crates/racecontrol/src/state.rs (bono_event_tx field)
- FOUND: commit edd3c83 (Task 1 — AppState bono_event_tx)
- FOUND: commit ac886a7 (Task 2 — full bono_relay.rs)
- 248 racecontrol-crate tests green confirmed
- rc-agent-crate builds with exit 0 (no new compilation errors)

---
*Phase: 27-tailscale-mesh-internet-fallback*
*Completed: 2026-03-16*
