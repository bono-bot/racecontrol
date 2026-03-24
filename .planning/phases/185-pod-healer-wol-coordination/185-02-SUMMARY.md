---
phase: 185-pod-healer-wol-coordination
plan: "02"
subsystem: racecontrol/pod_healer
tags: [wol, recovery, maintenance-mode, graduated-recovery, coordination]
dependency_graph:
  requires: [185-01]
  provides: [context-aware WoL with 3 pre-checks, WOL_SENT sentinel, MAINT-04 compliance]
  affects: [crates/racecontrol/src/pod_healer.rs]
tech_stack:
  added: []
  patterns:
    - Recovery event ring buffer query for cross-machine coordination
    - rc-sentry /files endpoint for direct MAINTENANCE_MODE file check
    - rc-sentry /exec endpoint for WOL_SENT sentinel before magic packet
key_files:
  modified:
    - crates/racecontrol/src/pod_healer.rs
decisions:
  - "WakeOnLan inserted between TierOneRestart and AiEscalation — WoL is an escalation step, not a recovery step, so AI escalation and staff alert still follow it"
  - "MAINTENANCE_MODE check uses direct rc-sentry /files endpoint (not fleet_health cache) — fleet_health can be stale if pod just entered maintenance; direct check is authoritative"
  - "WOL_SENT sentinel write failure is non-blocking — rc-sentry may be down when pod is offline; proceeding with WoL anyway is safer than blocking the escalation"
  - "Recovery event query window is 60s — matches the grace window documented in plan; long enough to catch race conditions between rc-sentry restart and pod_healer cycle"
metrics:
  duration_minutes: 12
  completed: "2026-03-25T03:13:00+05:30"
  tasks_completed: 1
  files_changed: 1
---

# Phase 185 Plan 02: Context-Aware WoL Coordination Summary

**One-liner:** Context-aware WoL with recovery event query (60s grace window), MAINTENANCE_MODE file check via rc-sentry /files (MAINT-04), and WOL_SENT sentinel write before magic packet — integrated as WakeOnLan step between TierOneRestart and AiEscalation in graduated recovery.

## What Was Built

A new `PodRecoveryStep::WakeOnLan` variant inserted into the graduated recovery ladder between `TierOneRestart` and `AiEscalation`. Before sending a WoL magic packet, pod_healer now performs three pre-checks:

1. **CHECK 1 — Recovery event query:** Queries `state.recovery_events` ring buffer for the target pod. If rc-sentry performed a `Restart` action with `spawn_verified=Some(true)` within the last 60 seconds, WoL is skipped (sentry already handled recovery). Advances to `AiEscalation`.

2. **CHECK 2 — MAINTENANCE_MODE file (MAINT-04):** HTTP GET to `http://<pod_ip>:8091/files?path=C%3A%5CRacingPoint%5CMAINTENANCE_MODE`. If the file exists (HTTP 200), WoL is skipped with `SkipMaintenanceMode` logged. Advances to `AlertStaff` (staff must clear MAINTENANCE_MODE before pod recovers). This prevents the `WoL -> restart -> MAINTENANCE_MODE -> WoL` infinite loop documented in standing rules.

3. **CHECK 3 — WOL_SENT sentinel:** HTTP POST to `http://<pod_ip>:8091/exec` with `echo WOL_SENT > C:\RacingPoint\WOL_SENT`. Non-blocking — if rc-sentry is unreachable, WoL proceeds anyway with a warning.

After all checks pass, WoL is sent via `wol::send_wol(mac_addr)` with cascade guard enforcement.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Context-aware WoL with recovery event query, MAINTENANCE_MODE check, WOL_SENT sentinel | `9abadb82` | `crates/racecontrol/src/pod_healer.rs` |

## Tests

30 pod_healer tests pass (3 new):
- `test_wol_step_exists_in_enum` — verifies WakeOnLan variant compiles
- `test_graduated_recovery_step_order` — verifies Waiting -> TierOneRestart -> WakeOnLan -> AiEscalation -> AlertStaff
- `test_skip_wol_when_sentry_restart_recent` — creates a RecoveryEventStore with spawn_verified=true event, verifies query finds it and skip logic triggers

Release build: clean (`cargo build --release --bin racecontrol`).

Pre-existing test failures (unrelated to this plan):
- `config::tests::config_fallback_preserved_when_no_env_vars` — existed before 185-02
- `crypto::encryption::tests::load_keys_valid_hex` — existed before 185-02

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check

- [x] `crates/racecontrol/src/pod_healer.rs` — exists, modified
- [x] Commit `9abadb82` — exists in git log
- [x] `grep -n "WakeOnLan"` — shows enum variant (line 91), match arm (line 771), test (line 1842)
- [x] `grep -n "MAINTENANCE_MODE"` — shows file check at line 811
- [x] `grep -n "WOL_SENT"` — shows sentinel write at line 844
- [x] `grep -n "send_wol"` — shows call at line 898
- [x] 30 pod_healer tests pass, 0 failures in new code
