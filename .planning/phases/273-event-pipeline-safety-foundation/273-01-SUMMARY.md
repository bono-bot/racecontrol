---
phase: 273-event-pipeline-safety-foundation
plan: 01
subsystem: meshed-intelligence
tags: [fleet-event, broadcast, event-bus, diagnostic-engine, predictive-maintenance]
dependency_graph:
  requires: []
  provides: [FleetEvent, Incident, FleetEventBus, fleet-event-broadcast]
  affects: [diagnostic_engine, predictive_maintenance, tier_engine]
tech_stack:
  added: [tokio::sync::broadcast]
  patterns: [broadcast-fan-out, string-serialized-cross-crate-types]
key_files:
  created:
    - crates/rc-common/src/fleet_event.rs
  modified:
    - crates/rc-common/src/lib.rs
    - crates/rc-agent/src/diagnostic_engine.rs
    - crates/rc-agent/src/predictive_maintenance.rs
    - crates/rc-agent/src/main.rs
decisions:
  - "Used String fields in FleetEvent (not rc-agent enums) to avoid cross-crate dependency"
  - "Used format!(\"{:?}\") for pod_state_snapshot since FailureMonitorState lacks Serialize derive"
  - "Kept existing mpsc DiagnosticEvent channel for backward compatibility with tier_engine"
  - "Added predictive scan bridge inline in diagnostic_engine (also kept standalone task for logging)"
metrics:
  duration: ~15min
  completed: 2026-04-01
  tasks: 2/2
  files_created: 1
  files_modified: 4
requirements: [PRO-01, PRO-06]
---

# Phase 273 Plan 01: FleetEvent Types and Broadcast Event Bus Summary

FleetEvent broadcast bus with 5 event variants, wired into diagnostic_engine and predictive_maintenance for immediate fan-out to multiple subscribers.

## Tasks Completed

### Task 1: Define FleetEvent types in rc-common
- Created `crates/rc-common/src/fleet_event.rs` with:
  - `FleetEvent` enum (5 variants): AnomalyDetected, PredictiveAlert, FixApplied, FixFailed, Escalated
  - `Incident` struct with UUID id, source_event, created_at, idempotency_key (Option)
  - `FleetEventBus` struct wrapping `tokio::sync::broadcast::Sender<FleetEvent>` (capacity 256)
- Added `pub mod fleet_event;` to `crates/rc-common/src/lib.rs`
- All types derive Clone, Debug, Serialize, Deserialize
- FleetEventBus gated behind `#[cfg(feature = "tokio")]` matching existing pattern

### Task 2: Wire broadcast channel and emit FleetEvents
- **main.rs**: Created `FleetEventBus::new(256)` wrapped in Arc, passed sender to diagnostic_engine::spawn and predictive_maintenance task
- **diagnostic_engine.rs**: Added `broadcast::Sender<FleetEvent>` and `node_id: String` params to spawn(). After each anomaly detection, emits `FleetEvent::AnomalyDetected` via broadcast. Runs inline predictive scan and bridges alerts as `FleetEvent::PredictiveAlert`.
- **predictive_maintenance.rs**: Added `alert_to_fleet_event()` helper that converts `PredictiveAlert` to `FleetEvent::PredictiveAlert` with string-serialized fields.
- Severity mapping: ProcessCrash/GameLaunchFail/DisplayMismatch = "high", ErrorSpike/ViolationSpike/WsDisconnect = "medium", Periodic/others = "low"

## Verification Results

1. `cargo check -p rc-common` -- PASS (0 errors)
2. `cargo check -p rc-agent-crate` -- PASS (0 errors, 23 pre-existing warnings)
3. `cargo test -p rc-common` -- PASS (208 tests passed)
4. `cargo test -p rc-agent-crate --no-run` -- PASS (test compilation clean)
5. No `.unwrap()` in new code -- confirmed via grep

## Commits

| Hash | Message |
|------|---------|
| c262ddb3 | feat(273): FleetEvent broadcast event bus (PRO-01, PRO-06) |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] FailureMonitorState lacks Serialize derive**
- **Found during:** Task 2
- **Issue:** Plan called for `serde_json::to_string(&event.pod_state)` but FailureMonitorState only derives Debug, Clone
- **Fix:** Used `format!("{:?}", event.pod_state)` for debug-string serialization
- **Files modified:** crates/rc-agent/src/diagnostic_engine.rs

**2. [Rule 2 - Missing functionality] Predictive maintenance standalone task lacked broadcast**
- **Found during:** Task 2
- **Issue:** Plan only wired broadcast into diagnostic_engine's inline predictive scan, but the standalone predictive_maintenance task in main.rs also produces alerts that should be broadcast
- **Fix:** Added fleet_bus.sender() to the standalone predictive_maintenance task as well, with lifecycle logging
- **Files modified:** crates/rc-agent/src/main.rs

## Known Stubs

None -- all data paths are wired and active.

## Self-Check: PASSED
