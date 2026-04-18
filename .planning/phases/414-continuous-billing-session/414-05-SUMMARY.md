---
phase: 414-continuous-billing-session
plan: 05
subsystem: kiosk-frontend
tags: [kiosk, frontend, billing, idle-warning, paused-meter, continuous-session]
dependency_graph:
  requires:
    - 414-03 (DashboardEvent::IdleWarning protocol + BillingTick in WaitingForGame)
    - 414-04 (handle_game_off rewrite, auto-end loop, stop_billing branch)
  provides:
    - IdleWarningDialog component (Branch A + Branch B)
    - Paused-meter UI branch in LiveSessionPanel
    - IdleWarning WS event handler in useKioskSocket
    - IdleWarningDialog mounted in staff/page.tsx (top-level overlay)
    - 3 kiosk label fixes (Between Games vs Game Loading/Waiting for Game)
    - web/StatusBadge Phase 414 coarse-label decision documented
  affects:
    - kiosk/src/components/IdleWarningDialog.tsx (NEW)
    - kiosk/src/components/LiveSessionPanel.tsx
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/app/staff/page.tsx
    - web/src/components/StatusBadge.tsx
tech_stack:
  added: []
  patterns:
    - "shadcn Dialog + Button primitives over Radix (already vendored)"
    - "Stable ref pattern for WS callback (onIdleWarningRef) — avoids stale closure in useCallback([], [])"
    - "isMidStreamWaiting flag to branch paused-meter vs first-wait in LiveSessionPanel"
    - "fsmLabel(status, elapsedSeconds?) — optional param for label differentiation"
key_files:
  created:
    - kiosk/src/components/IdleWarningDialog.tsx
  modified:
    - kiosk/src/components/LiveSessionPanel.tsx
    - kiosk/src/components/KioskPodCard.tsx
    - kiosk/src/hooks/useKioskSocket.ts
    - kiosk/src/app/staff/page.tsx
    - web/src/components/StatusBadge.tsx
decisions:
  - "Stable ref for onIdleWarning callback: useKioskSocket uses useCallback([], []) for the WS connect function; passing onIdleWarning directly would capture a stale closure. Used onIdleWarningRef pattern (same as React docs recommend for event callbacks in memoized functions)."
  - "isMidStreamWaiting derived flag: cleaner than repeating the billing.status + elapsed_seconds condition in multiple JSX locations."
  - "Bottom End Session button hidden (not removed) when isMidStreamWaiting — UI-SPEC discretionary call 1. The paused-meter End session button is the correct CTA; showing two End Session buttons at once is confusing."
  - "fsmLabel() extended with optional elapsedSeconds param (not a breaking change — default undefined preserves old behavior for all non-waiting_for_game statuses)."
  - "Web StatusBadge: no functional change — comment only. Admin context does not always have elapsed_seconds so coarse 'Loading...' is accepted per UI-SPEC deferred decision."
  - "Task 3 (venue checkpoint): AWAITING human verification — autonomous: false per plan. Plan 04 backend must be deployed to test server before verification is possible."
metrics:
  duration_minutes: 22
  completed: "2026-04-18T04:49:00Z"
  tasks_total: 3
  tasks_complete: 2
  task3_status: "awaiting_checkpoint"
  loc_added: 290
  loc_removed: 16
---

# Phase 414 Plan 05: Wave 5 Kiosk Frontend Summary

Wave 5 kiosk frontend: new `IdleWarningDialog` modal with Branch A (can continue) / Branch B (out of credits), paused-meter UI in `LiveSessionPanel` for mid-stream `WaitingForGame` sessions, `idle_warning` WS event handler wired to page-level state, and three label fixes differentiating "Between Games" from "Game Loading" / "Waiting for Game" across the kiosk.

## Tasks Completed

### Task 1 — IdleWarningDialog component (commit `a4654235`)

New file `kiosk/src/components/IdleWarningDialog.tsx` (~140 LOC):

- **Branch A** (`can_continue=true`): title "Still here?", local 1s/sec countdown, balance display, primary "Tap to continue" (Racing Red solid), secondary "End session now" (outline grey)
- **Branch B** (`can_continue=false`): title "Out of credits", countdown, wallet balance message, sole CTA "End session" (outlined red border-2)
- Local countdown via `setInterval` + reset on `session_id` change (server re-broadcast support)
- `autoFocus` on primary CTA each branch (AC-16)
- `aria-live="polite"` on countdown spans (AC-9)
- `font-mono tabular-nums` on all numeric readouts (AC-15)
- No `: any` types, no `#FF4400` deprecated orange
- Exports: `IdleWarningDialog` + `IdleWarningPayload`
- Stable ref pattern to avoid stale closure inside `useCallback([], [])` WS handler

### Task 2 — Wire IdleWarning + paused-meter + label fixes (commit `29508f64`)

**useKioskSocket.ts:**
- New `UseKioskSocketOptions` interface with `onIdleWarning?: (payload: IdleWarningPayload) => void`
- `onIdleWarningRef` stable ref — updated every render, read inside the `connect` callback
- `case "idle_warning"` handler calls `onIdleWarningRef.current` with typed payload

**staff/page.tsx:**
- `idleWarning` state declared before `useKioskSocket` call (so `setIdleWarning` is in scope)
- `onIdleWarning` callback passed to `useKioskSocket` — sets `idleWarning` state
- `IdleWarningDialog` mounted at page root (below `ConfirmDialog`):
  - `onContinue`: `setIdleWarning(null)` + `setSelectedPodId(podId)` + `setPanelMode("game_picker")`
  - `onEndSession`: `setIdleWarning(null)` + `handleEndSession(sessionId)` (routes through ConfirmDialog)
  - `onDismiss`: `setIdleWarning(null)`

**LiveSessionPanel.tsx:**
- `isMidStreamWaiting` flag: `billing.status === "waiting_for_game" && elapsed_seconds > 0`
- New paused-meter branch renders when flag true: "PAUSED — BETWEEN GAMES" pill, frozen cost in credits (font-mono tabular-nums), driving time, "Continue with another game" (bg-rp-red) + "End session" (border-2 border-rp-red)
- First-wait `LaunchTimerBanner` preserved for `elapsed_seconds === 0` (AC-1 backwards compat)
- Bottom End Session button wrapped in `{!isMidStreamWaiting && ...}` — avoids two End Session buttons (UI-SPEC discretionary call 1)
- Session Timer label: "Between Games" when `elapsed_seconds > 0`, "Game Loading" when `elapsed_seconds === 0` (AC-1)

**KioskPodCard.tsx:**
- `fsmLabel(status, elapsedSeconds?)` — `waiting_for_game` now returns "Between Games" when `elapsedSeconds > 0`, "Waiting for Game" otherwise
- Both call sites updated to pass `billing.elapsed_seconds`

**web/StatusBadge.tsx:**
- Comment-only: Phase 414 decision documented (coarse "Loading..." label accepted for admin context)

## AC Coverage

| AC | Description | Status |
|----|-------------|--------|
| AC-1 | Paused-meter triggers on waiting_for_game + elapsed>0; first-wait unchanged | Code-verified |
| AC-2 | Cumulative cost frozen (no local increment in paused branch) | Code-verified |
| AC-3 | "PAUSED — BETWEEN GAMES" string present with em-dash | Code-verified |
| AC-4 | Continue button bg-rp-red, font-semibold, py-4 (~56px) | Code-verified |
| AC-5 | End session outlined border-2 border-rp-red pattern | Code-verified |
| AC-6 | Continue → panelMode="game_picker" | Code-verified |
| AC-7 | End → handleEndSession → ConfirmDialog | Code-verified |
| AC-8 | IdleWarningDialog mounts on idle_warning WS event | Code-verified |
| AC-9 | Local 1s countdown with aria-live="polite" | Code-verified |
| AC-10 | Branch A: "Still here?", "Tap to continue", "End session now" | Code-verified |
| AC-11 | Branch B: "Out of credits", sole "End session" CTA | Code-verified |
| AC-12 | No #FF4400 / orange-4/5/6 in new files | Code-verified |
| AC-13 | No new CSS variables | Code-verified |
| AC-14 | Touch targets ≥44px (py-4=56px continue, size="lg"=44px modal buttons) | Code-verified |
| AC-15 | font-mono tabular-nums on cost, countdown, driving time | Code-verified |
| AC-16 | autoFocus on primary CTA, ESC dismiss via shadcn Dialog | Code-verified |
| AC-17 | Dialog overlays paused-meter; both coexist | Code-verified (shadcn Dialog overlay) |
| AC-18 | No layout shift — both states render in same SidePanel shell | Code-verified |

**AC-1 through AC-18: code-verifiable items all pass. Venue verification (physical behavior) pending Task 3 checkpoint.**

## Build Verification

```
cd kiosk && npm run build
→ 27 JS chunks compiled successfully
→ 0 TypeScript errors
→ /staff page included in build output
```

```
grep -n ": any" kiosk/src/components/IdleWarningDialog.tsx → CLEAN
grep -n "#FF4400" kiosk/src/components/IdleWarningDialog.tsx → CLEAN
```

## Task 3 Status: AWAITING VENUE CHECKPOINT

Task 3 is `type="checkpoint:human-verify"` with `autonomous: false`. The plan requires physical venue verification of all 18 AC items with Plan 04 backend deployed.

**Pre-condition (per plan-checker W2):** Plan 04 backend must be deployed to test server before Task 3 can run. Plan 04 backend changes (`handle_game_off` rewrite, 15-min auto-end loop, `stop_billing` branch, `BillingTick` in `WaitingForGame`) are required for the UI to exhibit the expected behavior.

**What Uday/James/Bono need to do:**
1. Deploy Plan 04 backend (commits `976cdd93`, `f1600e09`, `f0597923`) to server .23
2. Build and deploy kiosk frontend (this plan's commits `a4654235`, `29508f64`)
3. Follow verification steps in 414-05-PLAN.md Task 3 `<how_to_verify_human>` section
4. Reply "approved" if all 18 AC items pass, or describe which AC failed

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 — Bug] `JSX.Element` namespace not available**
- **Found during:** Task 1 TypeScript check
- **Issue:** `): JSX.Element | null` return type caused `error TS2503: Cannot find namespace 'JSX'` — this tsconfig uses React 18 jsx transform without global JSX namespace
- **Fix:** Changed return type annotation to `React.ReactElement | null`, imported `React` directly
- **Files modified:** `kiosk/src/components/IdleWarningDialog.tsx`
- **Commit:** `a4654235`

**2. [Rule 1 — Bug] Stale closure risk in useKioskSocket**
- **Found during:** Task 2, Subtask 2a
- **Issue:** `useKioskSocket` uses `useCallback([], [])` for the `connect` function (no deps — intentional to avoid reconnecting on every render). Passing `onIdleWarning` directly from options would capture the initial (possibly undefined) value and never update.
- **Fix:** Added `onIdleWarningRef` that is updated every render via `useEffect` (no deps). The `connect` callback reads `onIdleWarningRef.current` at call time, always getting the latest version.
- **Files modified:** `kiosk/src/hooks/useKioskSocket.ts`
- **Commit:** `29508f64`

**3. [Rule 1 — Bug] Duplicate `idleWarning` state declaration**
- **Found during:** Task 2, Subtask 2b
- **Issue:** State was initially added after `useKioskSocket` call, but the `onIdleWarning` callback passed to `useKioskSocket` referenced `setIdleWarning` which wasn't yet in scope (React rules of hooks require stable ordering but more importantly JS hoisting meant the reference would be to an undefined variable at the time the hook options object is created).
- **Fix:** Moved `idleWarning` state declaration before the `useKioskSocket` call. Removed the duplicate declaration that had been added later.
- **Files modified:** `kiosk/src/app/staff/page.tsx`
- **Commit:** `29508f64`

## Known Stubs

None. All UI branches are wired to live data from the WS event stream. The paused-meter UI reads from `billing.cost_paise` and `billing.elapsed_seconds` which are populated by Plan 03's BillingTick emission in WaitingForGame mid-stream branch. The IdleWarningDialog is driven by live `idle_warning` WS events from Plan 03's broadcast.

## Self-Check: PASSED

Files exist:
- FOUND: `kiosk/src/components/IdleWarningDialog.tsx`
- FOUND: `kiosk/src/components/LiveSessionPanel.tsx`
- FOUND: `kiosk/src/components/KioskPodCard.tsx`
- FOUND: `kiosk/src/hooks/useKioskSocket.ts`
- FOUND: `kiosk/src/app/staff/page.tsx`
- FOUND: `web/src/components/StatusBadge.tsx`

Commits exist:
- FOUND: `a4654235` (Task 1 — IdleWarningDialog)
- FOUND: `29508f64` (Task 2 — wire + paused-meter + labels)

Build: kiosk `npm run build` → 27 chunks, 0 errors.

## Pointer to Next Plan

→ `.planning/phases/414-continuous-billing-session/414-06-PLAN.md` — Wave 6: venue financial E2E test + deploy parity (CLAUDE.md mandate before ship). Includes: Plan 04 backend deploy to server .23 + cloud, kiosk frontend deploy, E2E financial flow verification tracing actual currency values through complete flows.
