---
phase: 10-staff-dashboard-controls
verified: 2026-03-14T00:00:00Z
status: human_needed
score: 9/9 must-haves verified
re_verification: null
gaps: []
human_verification:
  - test: "Open /control page and confirm 5 bulk buttons in action bar"
    expected: "Wake All, Shutdown All, Restart All, Lock All, Unlock All buttons are all visible and clickable"
    why_human: "Visual UI layout cannot be verified programmatically — need to confirm button order and appearance"
  - test: "Click Lock All button and observe confirmation dialog"
    expected: "window.confirm dialog appears with warning text before any API call is made"
    why_human: "Dialog behavior is runtime browser behavior, not checkable via static analysis"
  - test: "Click Unlock All and verify it executes without confirmation dialog"
    expected: "lockdownAllPods(false) fires immediately, no dialog"
    why_human: "Runtime interaction required"
  - test: "Click per-pod padlock icon and verify optimistic toggle"
    expected: "Padlock icon switches between bright orange (locked) and dim (unlocked) immediately, then API call fires"
    why_human: "React state change and icon swap require visual confirmation"
  - test: "Send lockdown to a connected pod and confirm rc-agent processes it"
    expected: "Pod's taskbar and Win key become restricted within a few seconds of clicking Lock"
    why_human: "End-to-end requires physical pod hardware with rc-agent connected"
---

# Phase 10: Staff Dashboard Controls — Verification Report

**Phase Goal:** Staff can manage all 8 pods from the kiosk dashboard without touching a keyboard on the pod — power cycling, rebooting, waking, and toggling lockdown are all one-click operations
**Verified:** 2026-03-14
**Status:** human_needed — all automated checks passed, 5 UI/hardware items need human confirmation
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths (from ROADMAP.md Success Criteria)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Staff can toggle full lockdown on/off for any individual pod from the dashboard | VERIFIED | `handleToggleLockdown` in control/page.tsx calls `api.lockdownPod(podId, newLocked)` — backend route `POST /pods/{id}/lockdown` exists and tested |
| 2 | Staff can lock all 8 pods at once and unlock all 8 pods at once with a single action | VERIFIED | `handleLockAll` calls `api.lockdownAllPods(true)` with confirm dialog; `handleUnlockAll` calls `api.lockdownAllPods(false)` without dialog — bulk route tested |
| 3 | Staff can shut down, restart, or wake any individual pod remotely from the dashboard | VERIFIED | Per-pod wake/restart/shutdown buttons exist in control/page.tsx (lines 200-234); routes registered in routes.rs; `wakePod`, `restartPod`, `shutdownPod` wired in api.ts |
| 4 | Staff can shut down, restart, or wake all 8 pods simultaneously from the dashboard | VERIFIED | `handleRestartAll` added to page.tsx calling `api.restartAllPods()`; Wake All and Shutdown All were pre-existing and verified working |

**Score:** 4/4 truths verified (all automated evidence confirmed)

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/wol.rs` | Unit tests for parse_mac | VERIFIED | 6 tests at lines 94-129: colon, dash, lowercase, too-few-parts, invalid-hex, empty — all passing |
| `crates/racecontrol/src/api/routes.rs` | lockdown_pod + lockdown_all_pods handlers + unit tests | VERIFIED | Handlers at lines 576-648; 4 unit tests in `mod lockdown_tests` at lines 650-819 — all passing |
| `kiosk/src/lib/api.ts` | lockdownPod, lockdownAllPods, restartAllPods API functions | VERIFIED | All three present at lines 289-300; each wired to correct backend path |
| `kiosk/src/app/control/page.tsx` | 5 bulk buttons + per-pod lockdown toggle | VERIFIED | `handleLockAll`, `handleUnlockAll`, `handleRestartAll` at lines 94-123; 5 buttons in bulk action bar at lines 138-169; per-pod padlock at lines 236-254 |
| `crates/racecontrol/src/billing.rs` | BillingTimer::dummy() test helper | VERIFIED | `pub fn dummy(pod_id: &str)` at line 200 with `#[cfg(test)]` gate |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `routes.rs:lockdown_pod` | `state.agent_senders` | `agent_senders.read().await.get(&id)` | WIRED | Line 593-596: senders read, pod looked up, is_closed() checked before send |
| `routes.rs:lockdown_pod` | `billing.active_timers` | `active_timers.read().await.contains_key(&id)` | WIRED | Line 589: billing guard fires before any send attempt |
| `routes.rs:api_routes` | `lockdown_pod, lockdown_all_pods` | route registration `.route(...)` | WIRED | Lines 37, 45: both routes registered; Axum 0.8 trie router resolves static vs dynamic paths by specificity regardless of registration order |
| `api.ts:lockdownPod` | `/pods/{id}/lockdown` | `fetchApi POST` | WIRED | Lines 291-295: POST with `{ locked }` body, correct return type |
| `api.ts:lockdownAllPods` | `/pods/lockdown-all` | `fetchApi POST` | WIRED | Lines 296-300: POST with `{ locked }` body |
| `api.ts:restartAllPods` | `/pods/restart-all` | `fetchApi POST` | WIRED | Lines 289-290: POST, return type correct |
| `control/page.tsx:handleLockAll` | `api.lockdownAllPods(true)` | onClick event | WIRED | Lines 99-106: confirm dialog, optimistic state update, API call |
| `control/page.tsx:handleToggleLockdown` | `api.lockdownPod(podId, newLocked)` | onClick event per pod | WIRED | Lines 113-123: optimistic Set<string> state toggle, then API call |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| KIOSK-01 | 10-01, 10-02 | Staff can toggle pod lockdown for a specific pod from the dashboard | SATISFIED | `lockdown_pod` route + `handleToggleLockdown` UI wired |
| KIOSK-02 | 10-01, 10-02 | Staff can lock/unlock all 8 pods at once from the dashboard | SATISFIED | `lockdown_all_pods` route + `handleLockAll`/`handleUnlockAll` UI wired |
| PWR-01 | 10-01 | Staff can power off a specific pod remotely | SATISFIED | `shutdown_pod` route exists (pre-existing); `shutdownPod` in api.ts; button in control page |
| PWR-02 | 10-01 | Staff can restart a specific pod remotely | SATISFIED | `restart_pod` route exists (pre-existing); `restartPod` in api.ts; button in control page |
| PWR-03 | 10-01 | Staff can power on a specific pod remotely (WoL) | SATISFIED | `wake_pod` route + `send_wol` exists (pre-existing); `wakePod` in api.ts; WoL button in control page |
| PWR-04 | 10-02 | Staff can power off all 8 pods at once | SATISFIED | `shutdown_all_pods` route (pre-existing) + `handleShutdownAll` in control page |
| PWR-05 | 10-02 | Staff can restart all 8 pods at once | SATISFIED | `restart_all_pods` route (pre-existing) + `handleRestartAll` added in Plan 02 |
| PWR-06 | 10-02 | Staff can power on all 8 pods at once | SATISFIED | `wake_all_pods` route (pre-existing) + `handleWakeAll` in control page |

All 8 requirements claimed by Plans 10-01 and 10-02 are accounted for. No orphaned requirements found — REQUIREMENTS.md maps KIOSK-01/02 and PWR-01 through PWR-06 to Phase 10, matching exactly what the plans declared.

### Anti-Patterns Found

None detected. Scan of `routes.rs` (lockdown section), `wol.rs`, `api.ts`, and `control/page.tsx` found:
- No TODO/FIXME/PLACEHOLDER comments in modified sections
- No empty handler bodies (`return null`, `return {}`)
- No console.log-only handlers
- No `return Response.json({ message: "Not implemented" })`
- TypeScript type check (`npx tsc --noEmit`) passed with zero errors

### Test Results

```
cargo test -p racecontrol-crate
running 175 tests
...
test api::routes::lockdown_tests::lockdown_all_skips_billing_active_and_closed_sends_to_healthy ... ok
test api::routes::lockdown_tests::lockdown_pod_with_active_billing_returns_error ... ok
test api::routes::lockdown_tests::lockdown_pod_with_closed_sender_returns_error ... ok
test api::routes::lockdown_tests::lockdown_pod_with_missing_sender_returns_error ... ok
test wol::tests::parse_mac_colon_separated_returns_correct_bytes ... ok
test wol::tests::parse_mac_dash_separated_returns_correct_bytes ... ok
test wol::tests::parse_mac_empty_string_returns_err ... ok
test wol::tests::parse_mac_invalid_hex_returns_err ... ok
test wol::tests::parse_mac_lowercase_returns_correct_bytes ... ok
test wol::tests::parse_mac_too_few_parts_returns_err ... ok
test result: ok. 175 passed; 0 failed; 0 ignored
```

10 new tests (6 parse_mac + 4 lockdown route) plus all 165 pre-existing tests pass.

### Routing Order Note

The PLAN instructed static bulk routes to be registered before dynamic `{id}` routes to prevent Axum routing conflicts. In the actual code, static routes (`/pods/wake-all`, `/pods/lockdown-all`) appear at lines 42-45 AFTER the dynamic `{id}` routes at lines 34-41. This is not a bug: Axum 0.8 uses a trie-based router with automatic specificity resolution — static literal paths always win over parameterized `{id}` paths regardless of registration order. The codebase compiled and all tests passed, confirming this is functionally correct.

### ROADMAP Discrepancy (Documentation Only)

ROADMAP.md line 108 shows `- [ ] 10-02-PLAN.md` (unchecked), while the SUMMARY and actual code confirm Plan 02 is complete. This is a documentation artifact — the code implementation is verified. The ROADMAP checkbox was not updated after Plan 02 executed.

### Human Verification Required

#### 1. Bulk action bar layout

**Test:** Navigate to `/control` page (staff PIN required). Inspect the top action bar.
**Expected:** Five buttons in order: Wake All (green), Shutdown All (red), Restart All (yellow), Lock All (orange), Unlock All (zinc/grey)
**Why human:** Visual layout and button order cannot be verified programmatically

#### 2. Confirmation dialogs on destructive actions

**Test:** Click "Restart All" and then "Lock All" buttons.
**Expected:** Each shows a `window.confirm` dialog before executing; clicking Cancel aborts the action
**Why human:** Browser dialog behavior requires runtime testing

#### 3. Unlock All has no confirmation

**Test:** Click "Unlock All".
**Expected:** `lockdownAllPods(false)` fires immediately without a confirmation dialog
**Why human:** Runtime browser interaction required

#### 4. Per-pod padlock toggle (optimistic UI)

**Test:** Click the padlock icon on any online pod card header.
**Expected:** Icon immediately switches to bright orange (locked) on first click; switches back to dim orange on second click; API call fires after each toggle
**Why human:** React state update and SVG icon swap require visual confirmation; optimistic update timing needs observation

#### 5. End-to-end lockdown reaching a pod

**Test:** With racecontrol running on server and at least one pod connected, click a per-pod Lock button.
**Expected:** That pod's taskbar disappears and Win key is blocked within a few seconds
**Why human:** Requires physical pod with rc-agent connected to racecontrol WebSocket; cannot test without live hardware

---

## Summary

Phase 10 backend is fully implemented and tested: lockdown API routes exist, are substantively correct (billing guard + disconnected sender guard), are wired to agent_senders, and pass 10 new unit tests. Frontend is fully wired: api.ts has all three new functions, the /control page has the 5 bulk buttons and per-pod padlock toggle. TypeScript compiles clean. All 8 requirements (KIOSK-01/02, PWR-01 through PWR-06) are satisfied with code evidence.

The only outstanding items are UI/hardware verifications that require a running browser and physical pods — standard for this type of front-end + hardware integration work.

---

_Verified: 2026-03-14_
_Verifier: Claude (gsd-verifier)_
