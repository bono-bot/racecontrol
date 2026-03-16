---
phase: 27-tailscale-mesh-internet-fallback
plan: 03
subsystem: infra
tags: [tailscale, axum, broadcast-channel, event-push, bono-relay, pod-monitor]

# Dependency graph
requires:
  - phase: 27-01
    provides: BonoConfig in config.rs with tailscale_bind_ip + relay_port fields
  - phase: 27-02
    provides: bono_relay::spawn(), bono_relay::build_relay_router(), BonoEvent enum
provides:
  - bono_relay::spawn() wired into main.rs — event push loop starts at server boot
  - Second Axum listener on Tailscale IP:8099 when tailscale_bind_ip configured
  - BonoEvent::PodOnline/PodOffline emitted from pod_monitor.rs at state-transition boundaries
affects:
  - 27-04 (deploy script: server binary now contains Tailscale relay binding)
  - 27-05 (config + verification: tailscale_bind_ip field must be set for listener to activate)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Second Axum listener via tokio::spawn(axum::serve(ts_listener, relay_router)): bind before state consumed"
    - "state.clone() for second listener placed BEFORE .with_state(state) in router build"
    - "let _ = broadcast::Sender::send(): discard RecvError when no receivers — non-fatal pattern"
    - "BonoEvent emissions at state-transition boundaries only — not in polling loops"

key-files:
  created: []
  modified:
    - crates/racecontrol/src/main.rs
    - crates/racecontrol/src/pod_monitor.rs

key-decisions:
  - "Second listener placed BEFORE .with_state(state) — Rust move semantics require state.clone() before main router consumes state"
  - "Non-fatal relay bind: Tailscale may not be connected at server startup, error logged but main server continues"
  - "PodOnline emitted at natural recovery site (backoff.attempt() > 0 + fresh heartbeat) — this is the offline->online transition"
  - "PodOffline emitted inside pod.status != PodStatus::Offline guard — fires exactly once per offline transition"
  - "last_seen_secs_ago set to 0 in PodOffline: exact duration not available at emit site without additional calculation"

patterns-established:
  - "Bono relay event channel: use let _ = send() everywhere — RecvError::NoReceivers is expected before spawn() subscribes"
  - "Multi-listener Axum: tokio::spawn the secondary before building main router to avoid state move issues"

requirements-completed: [TS-02, TS-03, TS-06]

# Metrics
duration: 5min
completed: 2026-03-16
---

# Phase 27 Plan 03: Wire bono_relay into main.rs + Pod State Emissions Summary

**bono_relay::spawn() wired into server startup with optional Tailscale second listener on :8099, and PodOnline/PodOffline events emitted from pod_monitor.rs at state-transition boundaries**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-16T11:48:49Z
- **Completed:** 2026-03-16T11:53:54Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- bono_relay::spawn(state.clone()) now called in main.rs after cloud_sync::spawn() — relay event-push loop starts at server boot
- Second Axum listener conditionally binds on Tailscale IP:8099 when tailscale_bind_ip is configured and bono.enabled is true; non-fatal if Tailscale not yet connected
- pod_monitor.rs emits BonoEvent::PodOnline at natural recovery (offline->online) and BonoEvent::PodOffline at heartbeat-stale transition (online->offline)
- Full test suite green: 112 rc-common + 248 racecontrol-crate tests passing, 0 regressions

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire bono_relay into main.rs — spawn + second listener** - `04fa1a1` (feat)
2. **Task 2: Wire BonoEvent::PodOnline/PodOffline emissions into pod_monitor.rs** - `cede51c` (feat)

**Plan metadata:** [docs commit — see final_commit step]

## Files Created/Modified

- `crates/racecontrol/src/main.rs` - Added bono_relay to imports; added bono_relay::spawn() after cloud_sync::spawn(); added second Axum listener block before .with_state(state)
- `crates/racecontrol/src/pod_monitor.rs` - Added BonoEvent import; PodOnline emission at recovery; PodOffline emission at heartbeat-stale transition

## Decisions Made

- **Second listener ordering:** state.clone() must be called before `.with_state(state)` consumes the Arc. The second listener block is inserted BEFORE `let app = Router::new()` to enforce this at compile time.
- **Non-fatal relay bind:** Tailscale may not be connected at server startup. Bind failure logs a tracing::error but main server continues. The relay becomes available whenever Tailscale connects and the server restarts.
- **PodOnline at backoff reset site:** The correct offline->online transition is where `backoff.attempt() > 0` and heartbeat is fresh — this is the only place a pod transitioning from a failure state to healthy is detected.
- **last_seen_secs_ago = 0:** The exact seconds since last seen is not trivially available at the PodOffline emit site without additional calculation. Zero is acceptable per plan spec.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required for this plan. Tailscale configuration is handled in Plan 05.

## Next Phase Readiness

- Rust server code for Phase 27 is complete. main.rs + bono_relay.rs + pod_monitor.rs all wired.
- Plan 04: PowerShell deploy script for Tailscale on all 8 pods + server
- Plan 05: racecontrol.toml config fields + human verification of full flow

---
*Phase: 27-tailscale-mesh-internet-fallback*
*Completed: 2026-03-16*
