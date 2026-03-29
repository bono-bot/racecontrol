---
phase: 260-notifications-resilience-ux
plan: 03
subsystem: rc-agent + racecontrol
tags: [resilience, hardware, usb, billing, clock-drift, crash-rate, controls-ini]
dependency_graph:
  requires: []
  provides: [hardware-disconnect-detection, crash-rate-maintenance-flag, clock-drift-visibility, ffb-leakage-prevention]
  affects: [fleet-health-api, ws-handler, ac-launcher, event-loop]
tech_stack:
  added: [pod_crash_events table]
  patterns: [edge-triggered disconnect detection, clock drift comparison, crash rate sliding window]
key_files:
  created: []
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-common/src/types.rs
    - crates/rc-agent/src/event_loop.rs
    - crates/rc-agent/src/ac_launcher.rs
    - crates/rc-agent/src/main.rs
    - crates/racecontrol/src/ws/mod.rs
    - crates/racecontrol/src/fleet_health.rs
    - crates/racecontrol/src/db/mod.rs
decisions:
  - "agent_timestamp added to PodInfo (types.rs) not inline Heartbeat variant — PodInfo is the heartbeat payload, fields belong there"
  - "prev_wheelbase_connected defaults to true to suppress false disconnect on first heartbeat tick"
  - "Clock drift stored as signed i64 (server - agent): positive = agent behind, negative = agent ahead"
  - "DashboardEvent::PodAlert not added — no matching variant exists; WhatsApp + billing pause satisfy RESIL-04"
  - "RESIL-07 uses docs/cfg path identical to bootstrap_ac_config to ensure consistent path resolution"
  - "Crash rate maintenance_flag cleared only manually (staff action) — auto-clear would mask recurring hardware issues"
metrics:
  duration_minutes: 45
  completed_date: "2026-03-29"
  tasks_completed: 2
  tasks_total: 2
  files_modified: 8
---

# Phase 260 Plan 03: Agent Resilience (RESIL-04/06/07/08) Summary

Agent-side and server-side resilience: USB wheelbase disconnect triggers billing pause + WhatsApp alert within one 5s heartbeat cycle; fresh controls.ini on every AC launch prevents FFB leakage; clock drift visible in fleet health; >3 crashes/hr auto-flags pod for maintenance.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Hardware disconnect detection + controls.ini reset + clock sync (agent-side) | 66f29d87 | protocol.rs, types.rs, event_loop.rs, ac_launcher.rs, main.rs |
| 2 | Server-side hardware disconnect handling + crash rate anomaly + clock drift check | 5304f087 | ws/mod.rs, fleet_health.rs, db/mod.rs |

## What Was Built

### RESIL-04: Hardware USB Disconnect Detection
- **Agent side:** `prev_wheelbase_connected` field in `ConnectionState` tracks previous HID state. On each heartbeat tick, if state transitions `true → false`, agent sends `AgentMessage::HardwareDisconnect { pod_id, device: "wheelbase", timestamp }` and logs at ERROR level.
- **Protocol:** `HardwareDisconnect` variant added to `AgentMessage` enum in `rc-common/src/protocol.rs`.
- **Server side:** Handler in `ws/mod.rs` pauses active billing session (`PausedGamePause`), logs ERROR with device name, sends WhatsApp alert to staff via `send_whatsapp`.

### RESIL-06: Crash Rate Anomaly Detection
- **DB:** `pod_crash_events` table with `id, pod_id, crash_type, created_at` + index on `(pod_id, created_at)` — added to `db/mod.rs` migration.
- **Server:** On every `GameCrashed` event, INSERT into `pod_crash_events`. Query count for this pod in last 1 hour. If `count > 3`: set `maintenance_flag = true` in `FleetHealthStore`, log ERROR, send WhatsApp alert.
- **Fleet health:** `maintenance_flag`, `crashes_last_hour`, exposed in both `FleetHealthStore` and `PodFleetStatus` API response.

### RESIL-07: Controls.ini FFB Leakage Prevention
- **ac_launcher.rs:** Before `set_ffb()` call, unconditionally write `[FF]\nGAIN=70\nMIN_FORCE=0.05\nFILTER=0.00\n` to controls.ini. `set_ffb()` then overwrites GAIN with the requested preset. Baseline is always clean — no session-to-session leakage.

### RESIL-08: Clock Drift Detection
- **Protocol:** `agent_timestamp: Option<String>` added to `PodInfo` struct (populated per-heartbeat via `Utc::now().to_rfc3339()`).
- **Server:** In heartbeat handler, parse `agent_timestamp`, compare with `Utc::now()`, compute `drift_secs = server - agent`. If `abs(drift) > 5s`: log WARN. Store `clock_drift_secs: Option<i64>` in `FleetHealthStore`, exposed in fleet health API.

## Deviations from Plan

### Auto-fixed Issues

None — plan executed exactly as written, with one minor structural deviation:

**[Structural] `agent_timestamp` added to PodInfo in types.rs, not inline in Heartbeat variant**
- **Found during:** Task 1
- **Reason:** `Heartbeat(PodInfo)` is a newtype variant — inline struct fields aren't supported in this pattern. The field belongs on `PodInfo` which IS the heartbeat payload.
- **Impact:** Plan acceptance check `grep -q "agent_timestamp" crates/rc-common/src/protocol.rs` does not pass (field is in types.rs). All functional requirements satisfied.

**[Missing variant] DashboardEvent::PodAlert not added**
- **Found during:** Task 2
- **Reason:** No `PodAlert` variant exists in `DashboardEvent` enum. The plan mentioned a dashboard broadcast but the acceptance criteria did not require it.
- **Fix:** Removed dashboard broadcast; kept WhatsApp alert + billing pause (both in acceptance criteria).

## Known Stubs

None — all features are fully wired. The `maintenance_flag` must be manually cleared by staff (no auto-clear timer). This is intentional: persistent flags force investigation.

## Verification

- `cargo check --bin racecontrol`: PASS
- `cargo check --bin rc-agent`: PASS
- `cargo check -p rc-common`: PASS
- `cargo test --bin racecontrol`: 4/4 PASS
- `cargo test -p rc-common`: 190/190 PASS
- Full pre-push gate: 664 tests PASS

## Self-Check: PASSED

Files exist:
- crates/rc-common/src/protocol.rs: HardwareDisconnect variant present
- crates/rc-common/src/types.rs: agent_timestamp field present
- crates/rc-agent/src/event_loop.rs: prev_wheelbase_connected + RESIL-04 logic present
- crates/rc-agent/src/ac_launcher.rs: RESIL-07 controls.ini reset present
- crates/racecontrol/src/ws/mod.rs: HardwareDisconnect handler + clock_drift + RESIL-06 present
- crates/racecontrol/src/fleet_health.rs: maintenance_flag + clock_drift_secs + crashes_last_hour present
- crates/racecontrol/src/db/mod.rs: pod_crash_events migration present

Commits verified:
- 66f29d87: Task 1 (agent-side)
- 5304f087: Task 2 (server-side)
