---
phase: 04-deployment-pipeline
plan: "03"
subsystem: deploy
tags: [rolling-deploy, canary, billing-safety, kiosk-ui, websocket]
dependency_graph:
  requires: [04-01, 04-02]
  provides: [deploy_rolling, deploy_status, DeployPanel]
  affects: [billing.rs, state.rs, ws/mod.rs, kiosk/settings]
tech_stack:
  added: []
  patterns:
    - Canary-first rolling deploy with session-aware scheduling
    - Session-end hook pattern (billing timer removal triggers pending deploy)
    - WaitingSession queuing: binary URL stored in pending_deploys map
    - TDD red-green-verify for WaitingSession serde
key_files:
  created:
    - kiosk/src/components/DeployPanel.tsx
  modified:
    - crates/rc-common/src/types.rs
    - crates/racecontrol/src/deploy.rs
    - crates/racecontrol/src/state.rs
    - crates/racecontrol/src/billing.rs
    - crates/racecontrol/src/api/routes.rs
    - crates/racecontrol/src/ws/mod.rs
    - kiosk/src/lib/types.ts
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/app/settings/page.tsx
decisions:
  - WaitingSession is_active() returns false — it is a queued state, not actively deploying; watchdog must not block on it
  - deploy_rolling() resolves pod IPs at call time from AppState.pods (only deploys to known/connected pods)
  - Session-end hook is wired in both tick_all_timers (natural expiry) and end_billing_session (manual/early end) for full coverage
  - DeployRolling WS handler replaced inline implementation with deploy_rolling() call for DRY behavior
  - DeployPanel uses pod_N format (underscores) matching AppState map keys, not pod-N (dashes) from the plan context
  - POST /api/deploy/rolling route placed before /deploy/:pod_id to avoid Axum path conflict
  - deploying flag in DeployPanel resets when anyActive returns false (watches for all in-progress states clearing)
metrics:
  duration: "~20 min"
  completed_at: "2026-03-13T02:19:44Z"
  tasks_completed: 4
  files_modified: 9
---

# Phase 4 Plan 03: Rolling Deploy Orchestration + Kiosk Deploy UI Summary

Rolling deploy with canary-first ordering and billing-session protection. Uday and James can now deploy rc-agent to all 8 pods from the kiosk settings page without touching a terminal — and paying customers are never disrupted.

## What Was Built

### Task 1: WaitingSession variant + deploy_status helper (TDD)
- Added `WaitingSession` to `DeployState` enum in rc-common/types.rs with `waiting_session` serde
- `is_active()` returns false for WaitingSession (queued, not deploying — watchdog safe)
- Added `deploy_step_label()` arm: "Waiting for active billing session to end"
- TDD: RED tests written first (compile failures confirmed), GREEN by adding variant
- Added `deploy_status()` public function to deploy.rs returning HashMap of all 8 pod states
- All 55 rc-common + 100 racecontrol tests pass

### Task 2: deploy_rolling() with canary-first + session-aware scheduling
- Added `pending_deploys: RwLock<HashMap<String, String>>` to AppState
- `deploy_rolling(state, binary_url)`: deploys pod_8 as canary first (synchronous), verifies Complete/Idle before proceeding, halts with error if canary fails
- Remaining pods (1-7): checks active_timers, sets WaitingSession + stores URL in pending_deploys if session active, deploys immediately if idle, 5s inter-pod delay
- `check_and_trigger_pending_deploy(state, pod_id)`: called when billing session ends, removes URL from pending_deploys and spawns deploy_pod
- Session-end hook wired in billing.rs at both timer removal points: `tick_all_timers` (natural expiry + pause-timeout) and `end_billing_session` (manual/early end)

### Task 3: POST /api/deploy/rolling endpoint + WS handler update
- Added `deploy_rolling_handler`: POST /api/deploy/rolling returns 202 and spawns deploy_rolling(); returns 409 if any deploy is active
- Wired before `/deploy/:pod_id` to avoid Axum path collision
- Updated `DashboardCommand::DeployRolling` in ws/mod.rs to call `deploy_rolling()` instead of the old inline pod-by-pod spawner (which had no session-awareness or canary verification)

### Task 4: DeployPanel kiosk component
- Added `waiting_session` to `DeployState` TS union type in kiosk/src/lib/types.ts
- `useKioskSocket`: handles `deploy_progress` WS event, maintains `deployStates: Map<string, DeployState>`, exposes `sendDeployRolling(binaryUrl)` helper
- `DeployPanel` component: binary URL input (pre-filled), "Deploy All" button, 8 pod cards in 4-column grid
  - Pod 8 shows "CANARY" badge (Racing Red #E10600)
  - Color coding: yellow (#eab308) for in-progress, green (#22c55e) for complete, red (#E10600) for failed, grey (#5A5A5A) for idle/queued
  - WaitingSession shows "Queued / Waiting for session" label
  - Failed state shows truncated failure reason with full tooltip
- Wired into settings page under "Agent Deploy" section with descriptive copy

## Verification Results

- `cargo test -p rc-common`: 55 passed
- `cargo test -p racecontrol-crate`: 100 passed (13 integration)
- `cargo build -p racecontrol-crate`: clean (5 pre-existing warnings only)
- `npx next build`: compiled successfully, 0 TypeScript errors

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Adaptation] Pod ID format mismatch — plan used pod-N (dashes), codebase uses pod_N (underscores)**
- **Found during:** Task 2 — when implementing deploy_rolling() sort logic
- **Issue:** Plan's interface showed `format!("pod-{}", n)` but AppState maps (backoffs, watchdog, deploy states) all use `pod_N` (underscore) format as established in Phase 1
- **Fix:** Used `pod_8`/`pod_N` format in deploy_rolling() to match existing AppState keys
- **Files modified:** crates/racecontrol/src/deploy.rs
- **Commit:** 29611fd

**2. [Rule 1 - Adaptation] deploy_pod() signature includes pod_ip — plan's interface showed binary_url only**
- **Found during:** Task 2 — deploy_rolling() needed to call deploy_pod()
- **Issue:** Plan context showed simplified signature. Actual deploy_pod() in Plan 02 takes (state, pod_id, pod_ip, binary_url)
- **Fix:** deploy_rolling() resolves pod_ip from AppState.pods for each pod before calling deploy_pod()
- **Files modified:** crates/racecontrol/src/deploy.rs

**3. [Rule 2 - Enhancement] WaitingSession is_active() explicit — not added to existing is_active() match**
- **Found during:** Task 1 — implementing WaitingSession
- **Issue:** The existing `is_active()` match needed WaitingSession excluded explicitly since it's a queued state
- **Fix:** Added WaitingSession to the not-active match arm
- **Files modified:** crates/rc-common/src/types.rs

## Self-Check: PASSED

All files verified present:
- FOUND: crates/racecontrol/src/deploy.rs
- FOUND: crates/rc-common/src/types.rs
- FOUND: crates/racecontrol/src/state.rs
- FOUND: crates/racecontrol/src/billing.rs
- FOUND: kiosk/src/components/DeployPanel.tsx
- FOUND: kiosk/src/app/settings/page.tsx
- FOUND: .planning/phases/04-deployment-pipeline/04-03-SUMMARY.md

All commits verified:
- 269ce9a: test(04-03): add failing WaitingSession tests (RED), then implement (GREEN)
- 29611fd: feat(04-03): add deploy_rolling() with canary-first + session-aware scheduling
- 3dfe648: feat(04-03): add POST /api/deploy/rolling endpoint + update WS command handler
- bb88ddb: feat(04-03): add DeployPanel component to kiosk settings page
