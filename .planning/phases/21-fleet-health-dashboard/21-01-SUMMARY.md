---
phase: 21-fleet-health-dashboard
plan: "01"
subsystem: rc-core
tags: [fleet-health, api, background-task, tdd]
dependency_graph:
  requires: []
  provides: [GET /api/v1/fleet/health, FleetHealthStore, pod_fleet_health AppState field]
  affects: [ws/mod.rs StartupReport handler, ws/mod.rs Disconnect handler, api/routes.rs]
tech_stack:
  added: []
  patterns: [futures_util::future::join_all for parallel probes, RwLock<HashMap> for fleet state, dedicated reqwest::Client with 3s timeout]
key_files:
  created:
    - crates/rc-core/src/fleet_health.rs
  modified:
    - crates/rc-core/src/lib.rs
    - crates/rc-core/src/state.rs
    - crates/rc-core/src/main.rs
    - crates/rc-core/src/ws/mod.rs
    - crates/rc-core/src/api/routes.rs
key_decisions:
  - Used futures_util::future::join_all (already a dependency) instead of adding new futures crate
  - Dedicated probe client with 3s connect+read timeout; never reuse state.http_client (30s)
  - uptime_secs computed live from stored agent_started_at, not snapshotted at StartupReport time
  - clear_on_disconnect preserves http_reachable (probe-driven) but clears version/agent_started_at
  - /fleet/health is public (no auth) for Uday's phone LAN access
metrics:
  duration: "~6 min"
  completed: "2026-03-15"
  tasks_completed: 2
  files_changed: 6
  tests_added: 13
  tests_total: 279
---

# Phase 21 Plan 01: Fleet Health Backend Summary

**One-liner:** Real-time fleet health API backed by per-pod FleetHealthStore with 15s HTTP probes, StartupReport data storage, and live uptime computation.

## What Was Built

A complete backend fleet health system for 8 pods, exposed as `GET /api/v1/fleet/health`.

**New module: `crates/rc-core/src/fleet_health.rs`**

- `FleetHealthStore`: per-pod in-memory state — `version`, `agent_started_at`, `http_reachable`, `last_http_check`, `crash_recovery`. Stored in `AppState::pod_fleet_health` (RwLock<HashMap<String, FleetHealthStore>>).
- `store_startup_report()`: called from WS StartupReport handler. Stores version, computes `agent_started_at = Utc::now() - uptime_secs`, stores `crash_recovery`.
- `clear_on_disconnect()`: called from both graceful Disconnect and ungraceful socket-drop. Clears version/agent_started_at/crash_recovery but preserves `http_reachable` (probe state survives disconnect).
- `start_probe_loop()`: tokio task, 15s interval, probes all registered pod IPs at `:8090/health` in parallel using `futures_util::future::join_all`. Dedicated `reqwest::Client` with 3s timeout.
- `fleet_health_handler()`: iterates pod_numbers 1..=8, assembles response with live `uptime_secs = (now - agent_started_at).num_seconds()`, both `ws_connected` and `http_reachable` as independent booleans, ISO-8601 timestamps.

**Wire-up in existing files:**

- `state.rs`: added `pod_fleet_health: RwLock<HashMap<String, FleetHealthStore>>` field + initializer
- `lib.rs`: registered `pub mod fleet_health`
- `main.rs`: spawns `fleet_health::start_probe_loop(state.clone())` after udp_heartbeat
- `ws/mod.rs`: StartupReport handler calls `store_startup_report`; Disconnect handler calls `clear_on_disconnect`; ungraceful cleanup also calls `clear_on_disconnect` (only for non-stale disconnects)
- `api/routes.rs`: added `.route("/fleet/health", get(fleet_health::fleet_health_handler))`

## API Response Shape

```json
{
  "pods": [
    {
      "pod_number": 1,
      "pod_id": "pod_1",
      "ws_connected": true,
      "http_reachable": true,
      "version": "0.5.2",
      "uptime_secs": 3847,
      "crash_recovery": false,
      "ip_address": "192.168.31.89",
      "last_seen": "2026-03-15T13:00:00Z",
      "last_http_check": "2026-03-15T13:07:00Z"
    }
    // ... 7 more pods
  ],
  "timestamp": "2026-03-15T13:07:52Z"
}
```

Pods with no PodInfo registered: pod_id=null, ws_connected=false, http_reachable=false, all optional fields null.

## Tests

13 new unit tests in `fleet_health::tests`:
- `fleet_health_store_default_is_all_false_and_none`
- `fleet_health_store_startup_report_sets_version`
- `fleet_health_store_startup_report_computes_agent_started_at`
- `fleet_health_store_startup_report_sets_crash_recovery`
- `fleet_health_store_startup_report_does_not_clear_http_reachable`
- `fleet_health_clear_on_disconnect_clears_version_and_started_at`
- `fleet_health_clear_on_disconnect_preserves_http_reachable`
- `fleet_health_uptime_computed_live_increases_over_time`
- `fleet_health_version_from_store_is_propagated`
- `fleet_health_http_reachable_from_store_is_propagated`
- `fleet_health_ws_connected_false_when_no_sender`
- `fleet_health_ws_connected_true_when_sender_exists_and_open`
- `fleet_health_ws_connected_false_when_receiver_dropped`

Total test suite: 279 tests (238 unit + 41 integration) — all passing.

## Commits

| Hash | Message |
|------|---------|
| 9eec64d | feat(21-01): add fleet_health module with FleetHealthStore, probe loop, and GET handler |
| 9d2f986 | feat(21-01): wire StartupReport/Disconnect into fleet health + register /fleet/health route |

## Deviations from Plan

None — plan executed exactly as written.

The only minor adaptation: the plan mentioned `futures::future::join_all` but `futures` is not a dependency. Used `futures_util::future::join_all` instead, which is already in `Cargo.toml`. Same function, different crate path.

## Self-Check: PASSED

- `crates/rc-core/src/fleet_health.rs`: EXISTS (440 lines)
- `crates/rc-core/src/lib.rs`: contains `pub mod fleet_health`
- `crates/rc-core/src/state.rs`: contains `pod_fleet_health`
- `crates/rc-core/src/ws/mod.rs`: contains `pod_fleet_health`
- `crates/rc-core/src/api/routes.rs`: contains `fleet/health`
- `crates/rc-core/src/main.rs`: contains `fleet_health::start_probe_loop`
- Commit 9eec64d: EXISTS
- Commit 9d2f986: EXISTS
- `cargo test -p rc-core fleet_health`: 13/13 PASSED
- `cargo test -p rc-core`: 279/279 PASSED
