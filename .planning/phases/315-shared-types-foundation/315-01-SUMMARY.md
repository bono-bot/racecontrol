---
phase: 315
plan: 01
subsystem: rc-common
tags: [shared-types, game-intelligence, v41, foundation]
dependency_graph:
  requires: []
  provides: [InstalledGame, GameInventory, ComboAvailabilityStatus, ComboValidationResult, ComboAvailabilityMatrix, LaunchTimeline, LaunchTimeoutConfig, ComboHealthSummary, CrashLoopReport]
  affects: [rc-agent, racecontrol, protocol]
tech_stack:
  added: []
  patterns: [shared-types-in-rc-common, additive-protocol-variants]
key_files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/protocol.rs
decisions:
  - "All new types added to rc-common/types.rs to avoid cross-crate import cycles between rc-agent and racecontrol"
  - "Used serde(other) Unknown catch-all for forward compatibility — new variants won't crash old agents"
  - "LaunchTimeoutConfig has Default impl with 90s per v41.0 spec so rc-agent can use it before config push arrives"
metrics:
  duration_secs: 391
  completed_date: "2026-04-03"
  tasks_completed: 5
  files_modified: 2
---

# Phase 315 Plan 01: Game Intelligence Shared Types Summary

**One-liner:** Five type groups + four AgentMessage variants in rc-common forming the data contract for v41.0 Game Intelligence System — game inventory, combo validation, launch timelines, combo health, and crash loop detection.

## What Was Built

Added 254 lines of new Rust types and protocol variants to `rc-common`, establishing the wire format and data model for all v41.0 Game Intelligence features:

### Type Groups Added (types.rs)

1. **Game Inventory** (`InstalledGame`, `GameInventory`) — Per-pod game scan results with exe path, scan method, steam app ID, and launchability flag. Replaces the simple `Vec<SimType>` for v41.0.

2. **Combo Availability** (`ComboAvailabilityStatus`, `ComboValidationResult`, `PodComboStatus`, `ComboAvailabilityEntry`, `ComboAvailabilityMatrix`) — Full result type for boot-time combo validation, including which paths were checked and why a combo is invalid.

3. **Launch Timeline** (`LaunchTimelineEvent`, `LaunchTimeline`, `SimLaunchTimeout`, `LaunchTimeoutConfig`) — Structured capture of every event during a launch attempt (command → process detected → playable signal → billing started). Default 90s timeout per v41.0 spec.

4. **Combo Health** (`ComboHealthStatus`, `ComboHealthSummary`) — Server-computed health classification (Healthy/Degraded/Flagged/Disabled/InsufficientData) with success rate, avg launch time, and flag reasons.

5. **Crash Loop** (`CrashLoopReport`) — Crash loop detection report with crash count, time window, exit codes, and timestamps for WhatsApp alerting.

### Protocol Variants Added (protocol.rs)

Four new `AgentMessage` variants:
- `GameInventoryUpdate(GameInventory)` — pod → server on boot/reconnect
- `ComboValidationReport { pod_id, results }` — pod → server after boot validation
- `LaunchTimelineReport(LaunchTimeline)` — pod → server after each launch
- `CrashLoopDetected(CrashLoopReport)` — pod → server on crash loop trigger

## Verification

- `cargo check -p rc-common`: PASS (1 pre-existing warning, not introduced here)
- `cargo check -p racecontrol-crate`: PASS (31 pre-existing warnings)
- `cargo check -p rc-agent-crate`: PASS (67 pre-existing warnings)
- `cargo test -p rc-common`: PASS — 235 tests, 0 failures

## Deviations from Plan

None — plan executed exactly as written. The plan was created in the same session since phase 315 directory did not exist yet. The five tasks (GameInventory, ComboAvailability, LaunchTimeline, ComboHealth, CrashLoop) were combined into a single commit since they're all in the same two files with no inter-dependencies between tasks.

## Known Stubs

None. These are pure type definitions with no logic. The downstream phases (316-319) will implement the actual scanning, validation, and launch tracing that populates these types.

## Self-Check: PASSED

- `crates/rc-common/src/types.rs` — FOUND (modified)
- `crates/rc-common/src/protocol.rs` — FOUND (modified)
- Commit `4e6a2717` — FOUND in git log
- 235 tests pass — VERIFIED
