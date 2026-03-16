---
phase: 05-synchronized-group-play
verified: 2026-03-16T06:00:00Z
status: passed
score: 15/15 must-haves verified
gaps: []
human_verification:
  - test: "Trigger a 2-pod multiplayer session — both pods validate PINs, confirm AC launches on both simultaneously"
    expected: "Both pods receive LaunchGame within <5s of the last PIN validation; neither launches early"
    why_human: "Coordinated timing requires live pods and a running AC server to confirm simultaneity"
  - test: "Enable continuous mode via POST, let a race finish, observe auto-restart"
    expected: "New race starts within 15s; staff sees updated session on kiosk without manual intervention"
    why_human: "Requires a running acServer.exe process exit event; cannot simulate with grep/file checks"
  - test: "Force a pod into game_state=error during a multiplayer session, observe kiosk UI"
    expected: "Pod card shows orange 'Join Failed' banner and 'Retry Join' button; other pods unaffected"
    why_human: "Requires live WebSocket event from rc-core to kiosk; UI rendering cannot be verified statically"
  - test: "Click 'Retry Join' on a failed pod"
    expected: "Pod receives StopGame then LaunchGame; game_state transitions back to 'launching'"
    why_human: "Requires live pod connection and observable state transition on kiosk dashboard"
---

# Phase 5: Synchronized Group Play — Verification Report

**Phase Goal:** Group events run smoothly — all pods launch and join the server at the same time, staff can run continuous races that auto-restart, and if a pod fails to join the server, staff can see and fix it without restarting everything.
**Verified:** 2026-03-16T06:00:00Z
**Status:** PASSED (automated checks) + Human verification items noted
**Re-verification:** No — initial verification
**Plans verified:** 05-01 (GROUP-01, GROUP-02) and 05-02 (GROUP-03, GROUP-04)

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `validate_pin()` calls `find_group_session_for_token()` and `on_member_validated()` for group members | VERIFIED | `auth/mod.rs:423-443` — calls both functions, result determines `is_group_member` |
| 2 | `validate_pin()` skips `launch_or_assist()` for group members (`is_group_member` flag) | VERIFIED | `auth/mod.rs:506-508` — `if !is_group_member { launch_or_assist(...) }` |
| 3 | AC server start removed from `book_multiplayer()` and `book_multiplayer_kiosk()` — deferred to `on_member_validated` | VERIFIED | `multiplayer.rs:307-309` and `multiplayer.rs:1652-1654` — deferral comment only, no `start_ac_server` call |
| 4 | `start_ac_lan_for_group()` called from `on_member_validated()` when all members validated | VERIFIED | `multiplayer.rs:627` — `let _ = start_ac_lan_for_group(state, group_session_id).await` inside all-validated branch |
| 5 | `AcServerInstance` has `continuous_mode` field | VERIFIED | `ac_server.rs:25` — `pub continuous_mode: bool` |
| 6 | `AcServerInstance` has `group_session_id` field | VERIFIED | `ac_server.rs:27` — `pub group_session_id: Option<String>` |
| 7 | `monitor_continuous_session()` exists and checks billing before restart | VERIFIED | `ac_server.rs:860+` — polls every 5s, checks `billing.active_timers`, restarts or stops |
| 8 | `check_and_stop_multiplayer_server()` in `billing.rs` respects `continuous_mode` | VERIFIED | `billing.rs:2514-2523` — early return when `inst.continuous_mode` is true |
| 9 | `POST /ac/session/{session_id}/continuous` endpoint exists | VERIFIED | `routes.rs:106` — `.route("/ac/session/{session_id}/continuous", post(ac_server_set_continuous))` |
| 10 | `retry_pod_join()` exists in `ac_server.rs` | VERIFIED | `ac_server.rs:781+` — sends StopGame then LaunchGame, resets game tracker |
| 11 | `update_session_config()` exists in `ac_server.rs` | VERIFIED | `ac_server.rs:745+` — updates track/car on continuous-mode session |
| 12 | `POST /ac/session/retry-pod` and `POST /ac/session/update-config` routes exist | VERIFIED | `routes.rs:107-108` — both routes registered |
| 13 | `KioskPodState` includes `"join_failed"` | VERIFIED | `types.ts:348-350` — `"join_failed"` variant in the union type |
| 14 | `useKioskSocket.ts` handles `"ac_server_update"` and `"group_session_all_validated"` events | VERIFIED | `useKioskSocket.ts:288-296` — both cases in WebSocket message switch; `acServerInfo` and `multiplayerGroup` returned |
| 15 | `KioskPodCard.tsx` shows "Join Failed" banner with "Retry Join" button; `staff/page.tsx` passes `acSessionId` and `onRetryJoin` | VERIFIED | `KioskPodCard.tsx:507-528` — top-level `join_failed` block with orange banner + button; `staff/page.tsx:434-447` — `acSessionId` from `multiplayerGroup.pod_ids`, `onRetryJoin` calls `api.retryPodJoin()` |

**Score:** 15/15 truths verified

---

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `crates/rc-core/src/auth/mod.rs` | VERIFIED | `find_group_session_for_token()` called at line 423; `on_member_validated()` at 429; `is_group_member` guard at 506 |
| `crates/rc-core/src/multiplayer.rs` | VERIFIED | `on_member_validated()` at line 574; calls `start_ac_lan_for_group()` at 627; AC server start removed from both booking functions |
| `crates/rc-core/src/ac_server.rs` | VERIFIED | `continuous_mode` + `group_session_id` fields on `AcServerInstance`; `set_continuous_mode()`, `monitor_continuous_session()`, `retry_pod_join()`, `update_session_config()` all present |
| `crates/rc-core/src/billing.rs` | VERIFIED | `continuous_mode` guard at line 2518 — defers stop to monitor loop |
| `crates/rc-core/src/api/routes.rs` | VERIFIED | `/ac/session/{session_id}/continuous`, `/ac/session/retry-pod`, `/ac/session/update-config` all registered |
| `crates/rc-common/src/types.rs` | VERIFIED | `AcServerInfo.continuous_mode: bool` with `#[serde(default)]` at line 570 |
| `kiosk/src/lib/types.ts` | VERIFIED | `AcServerInfo`, `MultiplayerGroupStatus` interfaces defined; `"join_failed"` in `KioskPodState` |
| `kiosk/src/hooks/useKioskSocket.ts` | VERIFIED | `acServerInfo` and `multiplayerGroup` state; both WebSocket event handlers; both returned |
| `kiosk/src/components/KioskPodCard.tsx` | VERIFIED | `onRetryJoin`/`acSessionId` props; `derivePodState()` with `isMultiplayerPod`; `join_failed` styling in compact + full modes; top-level join_failed block with "Retry Join" button |
| `kiosk/src/lib/api.ts` | VERIFIED | `retryPodJoin()`, `updateAcSessionConfig()`, `setAcContinuousMode()` all present |
| `kiosk/src/app/staff/page.tsx` | VERIFIED | Destructures `acServerInfo` and `multiplayerGroup` from `useKioskSocket`; passes `acSessionId` (from `multiplayerGroup.pod_ids`) and `onRetryJoin` to each `KioskPodCard` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `auth::validate_pin()` | `multiplayer::find_group_session_for_token()` | Called with `token_id` to detect group membership | WIRED | `auth/mod.rs:423` |
| `auth::validate_pin()` | `multiplayer::on_member_validated()` | Called when group member detected | WIRED | `auth/mod.rs:429` |
| `multiplayer::on_member_validated()` | `start_ac_lan_for_group()` | When `all_validated=true` | WIRED | `multiplayer.rs:627` |
| `start_ac_lan_for_group()` | `crate::ac_server::start_ac_server()` | Builds config, calls with pod_ids | WIRED | `multiplayer.rs:1112` |
| `ac_server_set_continuous` handler | `monitor_continuous_session()` | `tokio::spawn` when enabled | WIRED | Confirmed by routes.rs handler pattern (spawns monitor on enable) |
| `billing::check_and_stop_multiplayer_server()` | continuous_mode guard | Reads `inst.continuous_mode` before stopping | WIRED | `billing.rs:2518` |
| `KioskPodCard` "Retry Join" button | `api.retryPodJoin()` | `onRetryJoin(pod.id)` callback | WIRED | `KioskPodCard.tsx:523` |
| `staff/page.tsx` | `KioskPodCard acSessionId + onRetryJoin` | `multiplayerGroup.pod_ids.includes(pod.id)` | WIRED | `staff/page.tsx:434-447` |
| `useKioskSocket` `ac_server_update` case | `setAcServerInfo` | Sets state on WebSocket event | WIRED | `useKioskSocket.ts:288-290` |
| `useKioskSocket` `group_session_all_validated` case | `setMultiplayerGroup` | Sets state on WebSocket event | WIRED | `useKioskSocket.ts:293-295` |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| GROUP-01 | 05-01 | All pods in a multiplayer group launch AC and join the server simultaneously | SATISFIED | `on_member_validated()` → `start_ac_lan_for_group()` → `start_ac_server()` sends `LaunchGame` to all pods at once; individual launch blocked via `is_group_member` guard |
| GROUP-02 | 05-01 | Staff can enable "continuous" mode — auto-starts new session when race ends while billing active | SATISFIED | `AcServerInstance.continuous_mode`, `set_continuous_mode()`, `monitor_continuous_session()`, `POST /ac/session/{id}/continuous` |
| GROUP-03 | 05-02 | If any pod fails to join the AC server, staff sees which pod failed and can retry from kiosk | SATISFIED | `join_failed` KioskPodState, orange banner + "Retry Join" button in `KioskPodCard.tsx`, `retry_pod_join()` backend function, `POST /ac/session/retry-pod` |
| GROUP-04 | 05-02 | Staff can change track/car between races in continuous mode without stopping the full AC server | SATISFIED | `update_session_config()` in `ac_server.rs`, `POST /ac/session/update-config`, `api.updateAcSessionConfig()` in kiosk |

All 4 GROUP requirements: SATISFIED. No orphaned requirements.

---

### Anti-Patterns Found

No blockers or warnings found. Spot checks on modified files:

- `auth/mod.rs` — no TODO/FIXME/placeholder near the group detection logic
- `ac_server.rs` — monitor loop uses iterative `current_session_id` approach (not recursive spawn) — correct for `!Send` constraint on Windows
- `KioskPodCard.tsx` — `join_failed` block is a top-level sibling (not nested inside `on_track` block) — TypeScript narrowing issue was caught and fixed during execution
- `billing.rs` — continuous_mode guard inserted before the AC server stop call, correct placement

---

### Human Verification Required

#### 1. Coordinated Launch Timing

**Test:** Create a 2-pod multiplayer group session via staff booking. Have both pods enter their PINs (second pod validates last). Observe AC launch behavior on both pods.
**Expected:** Both pods receive `LaunchGame` command within 5s of each other; neither pod launches before the second PIN is entered; AC server starts only at that moment.
**Why human:** Timing window and live pod state transitions cannot be verified by grep. Requires actual pods running rc-agent.

#### 2. Continuous Mode Auto-Restart

**Test:** Staff sends `POST /ac/session/{id}/continuous` with `{"enabled": true}` on a running group session. Let a race complete (acServer.exe exits naturally or is killed). Observe within 15 seconds.
**Expected:** New AC session starts automatically; pods receive a fresh `LaunchGame`; kiosk shows updated session; billing remains active throughout.
**Why human:** Requires a live acServer.exe process lifecycle. Process exit detection in `monitor_continuous_session` cannot be triggered without running the binary.

#### 3. Join Failure Display

**Test:** Start a multiplayer group session. Force one pod's AC to fail (kill game process immediately on that pod). Wait for `game_state=error` from rc-agent.
**Expected:** Kiosk dashboard pod card for the failed pod turns orange, shows "Join Failed" label and "Retry Join" button. Other pods remain unaffected and show normal state.
**Why human:** Requires live WebSocket event delivery from rc-core to kiosk browser. Static analysis confirms the handler exists but cannot confirm the event fires correctly from billing/game state integration.

#### 4. Retry Join Execution

**Test:** With a pod showing "Join Failed", click the "Retry Join" button on the kiosk dashboard.
**Expected:** Pod receives `StopGame` then `LaunchGame` within 1s; pod card transitions from `join_failed` to `selecting` (launching); game attempts to reconnect to the running AC server.
**Why human:** Requires live pod-agent connection and observable state machine transition.

---

### Summary

Phase 5 goal is fully achieved at the code level. All 15 must-have items are present and wired:

- **GROUP-01 (coordinated start):** The path from PIN entry → `find_group_session_for_token` → `on_member_validated` → `start_ac_lan_for_group` → `start_ac_server` (sends LaunchGame to all pods simultaneously) is fully wired. Individual `launch_or_assist` is skipped for group members. The premature AC server start was removed from both booking functions.

- **GROUP-02 (continuous mode):** `AcServerInstance` carries the `continuous_mode` flag; `monitor_continuous_session` polls for process exit and restarts within 15s if billing is still active; `billing.rs` defers stop to the monitor loop when the flag is set; the `POST /ac/session/{id}/continuous` API is registered and spawns the monitor on enable.

- **GROUP-03 (join failure recovery):** Backend `retry_pod_join()` sends StopGame+LaunchGame to one pod for an active session; `POST /ac/session/retry-pod` is registered; kiosk `KioskPodState` has `join_failed`; `KioskPodCard` shows an orange "Join Failed" banner with "Retry Join" button as a top-level JSX block; `staff/page.tsx` wires the callback via `multiplayerGroup.pod_ids` (authoritative source for pod membership).

- **GROUP-04 (mid-session config change):** `update_session_config()` mutates track/car on the live `AcServerInstance.config`; the monitor loop re-reads `inst.config` on next restart; `POST /ac/session/update-config` is registered; `api.updateAcSessionConfig()` exists in kiosk.

Four human verification items are noted for live system testing. These are behavioral timing tests that require running pods and cannot be confirmed via static analysis.

---

_Verified: 2026-03-16T06:00:00Z_
_Verifier: Claude (gsd-verifier)_
