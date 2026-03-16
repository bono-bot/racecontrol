---
phase: 04-deployment-pipeline
plan: "01"
subsystem: rc-common, racecontrol, kiosk
tags: [deploy, types, protocol, appstate, typescript]
dependency_graph:
  requires: []
  provides: [DeployState, DeployProgress, DeployPod, DeployRolling, CancelDeploy, pod_deploy_states]
  affects: [racecontrol/pod_monitor, racecontrol/pod_healer, kiosk/types]
tech_stack:
  added: []
  patterns:
    - DeployState uses serde tag=state content=detail (matches WatchdogState tag/content pattern)
    - pod_deploy_states follows pod_watchdog_states initialization pattern
    - TypeScript discriminated union mirrors Rust enum serde output exactly
key_files:
  created: []
  modified:
    - crates/rc-common/src/types.rs
    - crates/rc-common/src/protocol.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/pod_monitor.rs
    - crates/racecontrol/src/pod_healer.rs
    - kiosk/src/lib/types.ts
decisions:
  - "DeployState uses serde(tag=state, content=detail) — consistent with protocol.rs adjacently-tagged enums; TS union uses { state: 'x' } discriminant matching Rust output"
  - "DeployPodStatus placed in protocol.rs (not types.rs) — it is a protocol-level DTO, not a domain type"
  - "is_active() returns false for Idle/Complete/Failed — these three are all terminal/no-op states from the watchdog's perspective"
metrics:
  duration_minutes: 6
  completed_date: "2026-03-13"
  tasks_completed: 4
  files_modified: 6
---

# Phase 4 Plan 1: Deploy Lifecycle Types Summary

One-liner: DeployState enum (9 variants, snake_case serde) + protocol messages (DeployProgress/DeployPod/DeployRolling/CancelDeploy) + AppState.pod_deploy_states + watchdog skip guards + TypeScript discriminated union types.

## What Was Built

### Task 1: DeployState enum (rc-common/types.rs)

Added `DeployState` enum with 9 variants: `Idle`, `Killing`, `WaitingDead`, `Downloading { progress_pct: u8 }`, `SizeCheck`, `Starting`, `VerifyingHealth`, `Complete`, `Failed { reason: String }`.

- Serde: `tag = "state"`, `content = "detail"`, `rename_all = "snake_case"` — matches kiosk wire format
- `Default` impl returns `Idle`
- `is_active()` helper: returns `false` for Idle/Complete/Failed, `true` for all in-progress states
- 12 serde roundtrip tests pass

### Task 2: Protocol messages (rc-common/protocol.rs)

Added:
- `DeployPodStatus` struct (pod_id, state: DeployState, last_updated)
- `DashboardEvent::DeployProgress { pod_id, state, message, timestamp }` — streamed per step
- `DashboardEvent::DeployStatusList(Vec<DeployPodStatus>)` — bulk sent on connect
- `DashboardCommand::DeployPod { pod_id, binary_url }`
- `DashboardCommand::DeployRolling { binary_url }`
- `DashboardCommand::CancelDeploy { pod_id }`

6 new serde roundtrip tests pass (53 total in rc-common).

### Task 3: AppState + watchdog skip logic (racecontrol)

- `AppState.pod_deploy_states: RwLock<HashMap<String, DeployState>>` — new field
- `create_initial_deploy_states()` — pre-populates all 8 pods as `Idle`
- `pod_monitor`: deploy state skip check after WatchdogState check — if `deploy_state.is_active()`, logs and `continue`
- `pod_healer`: deploy state skip check after WatchdogState check — if `deploy_state.is_active()`, logs and `return Ok(())`
- 84 racecontrol tests pass, 13 integration tests pass, `cargo build -p racecontrol-crate` succeeds

### Task 4: TypeScript types (kiosk/src/lib/types.ts)

Added:
- `DeployState` discriminated union matching Rust serde output (`{ state: 'idle' }`, `{ state: 'downloading'; detail: { progress_pct: number } }`, `{ state: 'failed'; detail: { reason: string } }`, etc.)
- `DeployPodStatus` interface
- `DeployProgressEvent` interface

`npx tsc --noEmit` passes (no errors).

## Verification

- `cargo test -p rc-common` — 53 passed (12 DeployState + 6 protocol deploy tests + existing)
- `cargo test -p racecontrol-crate` — 84 passed (1 deploy state init test + existing 83)
- `cargo build -p racecontrol-crate` — succeeds with existing pre-existing warnings (unrelated)
- Manual: DeployState has exactly 9 variants
- Manual: DashboardEvent::DeployProgress has pod_id, state, message, timestamp fields
- Manual: DashboardCommand has DeployPod, DeployRolling, CancelDeploy variants
- Manual: TypeScript types match Rust serde output format (tag=state, content=detail)

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

**Files:**
- FOUND: crates/rc-common/src/types.rs
- FOUND: crates/rc-common/src/protocol.rs
- FOUND: crates/racecontrol/src/state.rs
- FOUND: crates/racecontrol/src/pod_monitor.rs
- FOUND: crates/racecontrol/src/pod_healer.rs
- FOUND: kiosk/src/lib/types.ts

**Commits:**
- 1212e19 — feat(04-01): add DeployState enum (Task 1)
- 37e222e — feat(04-01): add DeployProgress event and Deploy commands (Task 2)
- 1b9f268 — feat(04-01): add pod_deploy_states to AppState + skip logic (Task 3)
- 0b56b5c — feat(04-01): add TypeScript deploy types (Task 4)
