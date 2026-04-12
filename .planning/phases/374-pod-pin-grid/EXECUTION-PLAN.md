# Phase 374: Pod PIN Grid — Execution Plan

**Scope:** Agent-side / kiosk-side portion of Phase 374 — the 4-digit PIN Grid that appears on pods for PWA self-service launch.

**Requirements:** PWAL-02 (Customer enters PIN on pod's 4-digit PIN Grid — game launches without staff)

**Depends on:** Server-side PIN generation (pin_launch.rs, built separately as PWAL-01)

---

## 1. Current State Analysis

### What already exists

**Two independent PIN entry systems:**

1. **rc-agent Native Lock Screen (Win32)** — `crates/rc-agent/src/lock_screen.rs`
   - A Win32-painted native overlay window that rc-agent renders directly on the pod display
   - Has a `LockScreenState::PinEntry` state with token_id, driver_name, pricing_tier_name, allocated_seconds
   - Emits `LockScreenEvent::PinEntered { pin }` which the event_loop forwards as `AgentMessage::PinEntered { pod_id, pin }` to the server via WebSocket
   - The server's `auth::handle_pin_entered()` calls `validate_pin()` which checks against `auth_tokens` table
   - **This is the EXISTING pod-side PIN flow** — it is triggered when the server sends a `ShowPinEntry` equivalent (via auth token assignment)
   - **Known bug (staff-launch-pin-grid-block.md):** For non-AC games, the lock screen stays visible after game launch because `close_browser()` is never called in the non-AC branch. This is being fixed separately.

2. **Kiosk Web App StaffLoginScreen** — `kiosk/src/components/StaffLoginScreen.tsx`
   - A Next.js web component at `/staff` that renders a 4-digit PIN numpad for **staff** authentication
   - Calls `api.validateStaffPin(pin)` which hits `POST /api/v1/staff/validate-pin`
   - Returns JWT token stored in sessionStorage for staff session
   - **This is staff-only and MUST NOT be confused with customer PIN entry**

3. **Server-side kiosk PIN validation** — `crates/racecontrol/src/auth/mod.rs`
   - `validate_pin_kiosk()` — HTTP endpoint `POST /api/v1/auth/kiosk/validate-pin` that validates a PIN against `auth_tokens` table, atomically consuming it and starting billing
   - Returns `KioskPinResult { billing_session_id, pod_id, pod_number, driver_name, pricing_tier_name, allocated_seconds }`

4. **Reservation PIN redemption** — `crates/racecontrol/src/reservation.rs`
   - `redeem_pin()` — HTTP endpoint `POST /api/v1/kiosk/redeem-pin` for remote booking flow
   - Validates 4-digit numeric PIN against `reservations` table, assigns idle pod, starts billing, launches game
   - Returns pod assignment, session info

5. **API client already wired** — `kiosk/src/lib/api.ts`
   - `api.validateKioskPin(pin, pod_id)` — already exists, calls `POST /auth/kiosk/validate-pin`
   - `api.redeemPin(pin)` — already exists, calls `POST /kiosk/redeem-pin`

### What does NOT exist

- **No customer-facing PIN Grid page in the kiosk web app.** The kiosk customer landing page (`page.tsx`) shows a 4x2 pod grid with live telemetry — it has NO PIN entry UI
- **No kiosk web route for self-service PIN entry.** The `/pod/[number]/page.tsx` page shows the `PodKioskView` component which has idle/launching/in_session states but no customer PIN input
- The architecture doc mentions a "PinModal" component on the landing page, but reading `page.tsx` confirms this does NOT exist in the actual code

### The "6 digit PIN grid blocking staff launch" bug

Per `.planning/debug/staff-launch-pin-grid-block.md`:
- The "PIN grid" that was blocking staff launch was the **rc-agent native Win32 lock screen** (not a web page)
- Root cause: `show_launch_splash()` shows the native window, but `close_browser()` was never called after non-AC game launch
- This is in the native lock screen layer, NOT the kiosk web app
- **This bug is separate from Phase 374 and should be fixed before Phase 374 begins**

---

## 2. Architecture Decision: Where the PIN Grid Lives

### Decision: Kiosk web page at `/pod/[number]`, NOT native lock screen

**Rationale:**
- The native lock screen (`lock_screen.rs`) already has `PinEntry` state, but it is painted by rc-agent's Win32 code — it cannot be styled, has no web APIs, and is coupled to the agent
- The kiosk web app already runs on port 3300 on each pod (or on the server at `.23:3300` and pods display it via Edge)
- The `/pod/[number]` route already exists and has the right architecture: it connects via WebSocket, knows which pod it is, and has access to all kiosk APIs
- Phase 374 requirement PWAL-03 demands the self-service path be "completely independent from staff launch path in code" — a new component in the kiosk web app achieves this cleanly

**The PIN Grid will be a new React component rendered within the existing `/pod/[number]` page when the pod is idle and no staff session is active.**

### Flow Architecture

```
Customer books on PWA (phone)
  --> Server creates reservation with 4-digit PIN (PWAL-01, separate)
  --> Customer walks to pod
  --> Pod screen shows kiosk page at /pod/N
  --> When pod is idle: PIN Grid component is displayed
  --> Customer enters 4-digit numeric PIN
  --> Kiosk calls api.redeemPin(pin) [POST /kiosk/redeem-pin]
  --> Server validates PIN, assigns pod, starts billing, launches game
  --> WebSocket pushes billing_session_list update
  --> Pod kiosk transitions to "launching" / "in_session" view
  --> Game launches on pod (server sends LaunchGame to rc-agent)
```

This flow uses the EXISTING `redeem_pin()` server endpoint. No new server endpoints are needed for the kiosk side.

---

## 3. UI Design: Customer PIN Grid

### Layout (Fullscreen, touch-optimized)

```
+--------------------------------------------------+
|                                                  |
|          RACING POINT                            |
|          eSports & Cafe                          |
|                                                  |
|          Enter Your PIN                          |
|          [ _ ] [ _ ] [ _ ] [ _ ]                 |
|                                                  |
|          +-----+  +-----+  +-----+              |
|          |  1  |  |  2  |  |  3  |              |
|          +-----+  +-----+  +-----+              |
|          +-----+  +-----+  +-----+              |
|          |  4  |  |  5  |  |  6  |              |
|          +-----+  +-----+  +-----+              |
|          +-----+  +-----+  +-----+              |
|          |  7  |  |  8  |  |  9  |              |
|          +-----+  +-----+  +-----+              |
|          +-------+ +-----+ +-------+            |
|          | CLEAR | |  0  | |  <--  |            |
|          +-------+ +-----+ +-------+            |
|                                                  |
|   "Book at racingpoint.in or ask at reception"   |
|                                                  |
+--------------------------------------------------+
```

### Design Specifications

- **Background:** `#0A0A0A` (rp-black), full-screen, no scroll
- **Brand:** "RACING POINT" in Orbitron/display font, "POINT" in `#E10600` (rp-red)
- **Subtitle:** "Enter Your PIN" in white, large (text-2xl or text-3xl)
- **PIN display:** 4 large boxes (w-20 h-24), filled dots when digit entered, current box highlighted with rp-red border
- **Numpad:** 3x4 grid, large touch targets (h-20 minimum), same styling as `StaffLoginScreen` numpad
- **Buttons:** 0-9 digits + "Clear" (resets all) + backspace icon (removes last digit)
- **Auto-submit:** When 4th digit is entered, automatically call the API (same pattern as `StaffLoginScreen`)
- **Footer hint:** Small text "Book at racingpoint.in or ask at reception" in zinc-500
- **No "Staff Login" link** on this view — staff access is at `/staff`, not on the pod kiosk page

### States

| State | Visual | Duration |
|-------|--------|----------|
| `idle` | PIN Grid shown | Until PIN entered or session starts |
| `validating` | Spinner overlay "Validating..." | 1-3 seconds |
| `success` | Green checkmark + "Welcome, {name}!" + allocated time | 3 seconds, then auto-transitions |
| `error` | Red X + error message (from server) | 5 seconds, then returns to idle PIN Grid |
| `error:insufficient_funds` | Red X + "Insufficient credits. Please top up at reception." | 10 seconds, then returns to idle |
| `error:expired` | Red X + "PIN expired. Please book again." | 10 seconds, then returns to idle |
| `error:invalid` | Red X + "Invalid PIN. Please check and try again." | 5 seconds, then returns to idle |

### Error Mapping

The `redeem_pin()` endpoint returns structured errors via `RedeemPinError`:
- `is_pin_error: true` → "Invalid PIN" or "PIN expired" → show `error:invalid` or `error:expired`
- `is_pending_debit: true` → "Being processed" → show brief "Processing, please wait" then retry
- Infrastructure errors → generic "Something went wrong, please ask at reception"

---

## 4. File-by-File Plan: Kiosk Changes

### 4.1 New Component: `kiosk/src/components/CustomerPinGrid.tsx`

**Purpose:** Self-contained PIN Grid component for customer self-service PIN entry.

**Props:**
```typescript
interface CustomerPinGridProps {
  podId: string;       // The pod this kiosk page represents
  podNumber: number;   // For display ("Pod 04")
  onSuccess: (result: RedeemPinResponse) => void;  // Called after successful PIN redemption
}
```

**Internal State:**
```typescript
type PinGridStep = "idle" | "validating" | "success" | "error";

const [step, setStep] = useState<PinGridStep>("idle");
const [pin, setPin] = useState("");
const [errorMsg, setErrorMsg] = useState("");
const [successData, setSuccessData] = useState<RedeemPinResponse | null>(null);
```

**Behavior:**
1. Renders a full-screen PIN numpad (0-9, Clear, Backspace)
2. On 4th digit entered → auto-submit:
   - Set step to "validating"
   - Call `api.redeemPin(pin)`
   - On success: set step to "success", show welcome screen for 3s, then call `onSuccess()`
   - On error: set step to "error", show error message, auto-reset to "idle" after 5-10s
3. No staff JWT used — `redeemPin()` calls a public endpoint (no auth header)
4. Component is purely presentational — does not manage WebSocket state

**Key Design Decisions:**
- Reuse the same numpad styling as `StaffLoginScreen` (grid-cols-3, h-20 buttons, rp-surface bg)
- Do NOT reuse `StaffLoginScreen` itself — that component is tightly coupled to staff auth flow
- Use `api.redeemPin(pin)` (NOT `api.validateKioskPin(pin, pod_id)`) because the redemption flow handles pod assignment, billing start, and game launch atomically on the server side

### 4.2 Modify: `kiosk/src/app/pod/[number]/page.tsx`

**Current behavior:** Shows `PodKioskView` in standalone mode for all states (idle, in_session, etc.)

**New behavior:** When the pod is idle AND no auth token is pending AND no billing is active:
- Show `CustomerPinGrid` instead of the experience selector idle view
- When `CustomerPinGrid` fires `onSuccess`, the WebSocket will push billing/game state updates that transition the page to launching/in_session views automatically

**Changes:**
```typescript
// Before PodKioskView render, check if we should show PIN Grid
const isIdle = pod.status === "idle" || pod.status === "available";
const hasNoBilling = !billing;
const hasNoAuthToken = !authToken;
const showPinGrid = isIdle && hasNoBilling && hasNoAuthToken;

if (showPinGrid) {
  return (
    <CustomerPinGrid
      podId={pod.id}
      podNumber={pod.number}
      onSuccess={() => {
        // No action needed — WebSocket will push state updates
        // that cause this component to re-render with billing data
      }}
    />
  );
}

// Otherwise render PodKioskView as before
return <PodKioskView ... />;
```

**Critical: PIN Grid MUST NOT appear when:**
- Pod has an active billing session (status "in_session")
- Pod has a pending auth token (staff already assigned a customer)
- Pod is disabled/offline
- Pod is in session completion (60s summary screen)

### 4.3 Modify: `kiosk/src/lib/api.ts` — No changes needed

`api.redeemPin(pin)` already exists and is correctly wired to `POST /kiosk/redeem-pin`. The `RedeemPinResponse` type is already imported from `@racingpoint/types`.

### 4.4 Modify: `kiosk/src/lib/types.ts` — Verify RedeemPinResponse type

Check that `RedeemPinResponse` from `@racingpoint/types` has the fields the PIN Grid needs:
- `pod_number` — to show "You're on Pod 4"
- `driver_name` — to show "Welcome, {name}!"
- `allocated_seconds` — to show "30 minutes"
- `pricing_tier_name` — to show plan name
- `error` — for error cases

If any fields are missing, add them to `@racingpoint/types` in `crates/rc-common/src/`.

### 4.5 No changes to: `kiosk/src/components/PodKioskView.tsx`

The `PodKioskView` component already handles all post-PIN states correctly:
- `idle` state shows experience selector (but we intercept before this in `/pod/[number]`)
- `waiting` state shows "Awaiting Customer" with PIN display
- `launching` state shows game loading spinner
- `in_session` state shows Racing HUD / telemetry

The PIN Grid replaces the `idle` view at the page level, not inside `PodKioskView`.

### 4.6 No changes to: `kiosk/src/components/StaffLoginScreen.tsx`

Staff login is completely separate. Staff PIN goes to `POST /staff/validate-pin` and returns a JWT. Customer PIN goes to `POST /kiosk/redeem-pin` and returns a session assignment. Different endpoints, different auth flows, different components.

### 4.7 No changes to: `kiosk/src/hooks/useKioskSocket.ts`

The WebSocket hook already handles all the events needed:
- `billing_session_list` / `billing_session_changed` — triggers billing state update
- `game_state_changed` — triggers game state update
- `pod_update` — triggers pod status change

When the server processes `redeem_pin()`, it starts billing and launches the game. The WebSocket events flow automatically and the page re-renders out of the PIN Grid state.

---

## 5. rc-agent Changes Needed

### 5.1 No rc-agent code changes required for the kiosk PIN Grid

The kiosk PIN Grid is a web page served by the kiosk Next.js app. The customer interaction is:
1. Customer sees web page on pod monitor (served by kiosk at :3300)
2. Customer enters PIN on web page
3. Web page calls server HTTP API
4. Server processes reservation, starts billing, sends `LaunchGame` to rc-agent via WS
5. rc-agent receives `LaunchGame` and launches the game (existing flow)

The rc-agent does NOT need to know about the kiosk PIN Grid. It receives the same `LaunchGame` message regardless of whether the launch was initiated by staff, PWA self-service, or any other path.

### 5.2 Prerequisite: Fix the lock screen overlay bug

The bug documented in `.planning/debug/staff-launch-pin-grid-block.md` MUST be fixed before Phase 374 can work:
- When `LaunchGame` arrives for a non-AC game, `show_launch_splash()` shows the native Win32 lock screen overlay
- This overlay will cover the kiosk web page (which is running in a browser window underneath)
- If `close_browser()` is never called (the current bug), the lock screen stays on top permanently

**Fix required (in ws_handler.rs):** Add `state.lock_screen.close_browser()` after non-AC game process is confirmed started, analogous to the AC branch at line 767. This fix is already documented in the debug file and should be implemented as a prerequisite.

### 5.3 Edge case: Native lock screen vs kiosk web page layer conflict

When both the native Win32 lock screen AND the kiosk web page are running:
- The native lock screen is a TOPMOST Win32 window — it renders ABOVE the browser
- If the lock screen is in `ScreenBlanked` state, it shows a black screen over everything
- If the lock screen is in `PinEntry` state (from the OLD flow), it shows the native PIN entry over the kiosk web page

**For Phase 374:** When the pod is idle and the kiosk web page shows the PIN Grid:
- The native lock screen should be in `ScreenBlanked` state (pure black — it IS showing between sessions)
- **Problem:** `ScreenBlanked` covers the kiosk web page
- **Solution options:**
  a. The native lock screen enters `Hidden` state when kiosk web page is the active UI (preferred)
  b. The kiosk PIN Grid runs ON the native lock screen (requires significant rc-agent changes)
  c. The native lock screen is disabled entirely when kiosk web mode is active

**Recommended approach (a):** When the server detects that a pod should show the kiosk web PIN Grid (pod idle, no session), it sends a `ClearLockScreen` message to rc-agent, which sets the native lock screen to `Hidden`. The kiosk web page then handles all idle-state UI including the PIN Grid. When a session starts, the native lock screen takes over for session management (active session HUD, countdown warnings).

This requires a server-side change: after a billing session ends and the pod becomes idle, send `ClearLockScreen` to the agent so the web kiosk can be seen. This may already happen — needs verification during implementation.

---

## 6. Configuration & Display Routing

### How does the pod display the kiosk web page?

Pods run Edge in kiosk mode pointing to the kiosk URL. The current URL is likely `http://192.168.31.23:3300/kiosk/pod/{number}` (the kiosk runs on the server, pods display it via browser).

**Verification needed during implementation:**
1. What URL does Edge on each pod load? Check rc-agent's browser launch config
2. Does the pod browser point to `/pod/{number}` or to `/` (the customer landing page)?
3. If it points to `/`, the PIN Grid would need to be on the landing page instead

### If pods currently show `/` (landing page):

The landing page shows an 8-pod grid — not appropriate for a single-pod PIN entry. Two options:
- **Option A:** Change pod browsers to point to `/pod/{number}` where `{number}` is the pod's own number
- **Option B:** Add a URL parameter to the landing page that activates single-pod PIN mode: `/?pod=4`

**Recommended: Option A** — each pod's browser should load `/kiosk/pod/{number}`. This gives each pod its own dedicated view. This requires updating the Edge kiosk URL in rc-agent's config for each pod.

---

## 7. Test Plan

### 7.1 Unit Tests

| Test | File | Description |
|------|------|-------------|
| PIN digit entry | `CustomerPinGrid.test.tsx` | Pressing digits 1-9-0 updates display, 4th digit triggers submit |
| Clear button | `CustomerPinGrid.test.tsx` | Clear resets all digits |
| Backspace button | `CustomerPinGrid.test.tsx` | Backspace removes last digit |
| Auto-submit on 4 digits | `CustomerPinGrid.test.tsx` | Verify `api.redeemPin` called when 4th digit entered |
| Error display | `CustomerPinGrid.test.tsx` | Error message shown on API failure, auto-reset after timeout |
| Success display | `CustomerPinGrid.test.tsx` | Welcome screen shown on success, auto-hides after 3s |
| No extra digits | `CustomerPinGrid.test.tsx` | Cannot enter more than 4 digits |

### 7.2 Integration Tests

| Test | Description |
|------|-------------|
| PIN Grid shown when idle | Navigate to `/pod/1`, mock pod as idle — verify PIN Grid renders |
| PIN Grid hidden when in_session | Mock pod with active billing — verify PodKioskView renders, not PIN Grid |
| PIN Grid hidden when auth pending | Mock pod with pending auth token — verify PodKioskView renders |
| PIN Grid hidden when disabled | Mock pod as disabled — verify disabled view renders |
| Transition on success | Enter valid PIN → mock successful redeem → verify WebSocket state change transitions to launching view |
| Error recovery | Enter invalid PIN → verify error shown → verify auto-return to PIN Grid |

### 7.3 E2E Test (Manual, on venue hardware)

**Prerequisites:**
- Server-side PIN generation is working (PWAL-01 complete)
- Lock screen overlay bug is fixed (staff-launch-pin-grid-block fix deployed)
- Pod browser points to `/kiosk/pod/{number}`

**Test steps:**
1. Customer books on PWA → receives 4-digit PIN
2. Walk to pod (e.g., Pod 4)
3. Pod screen shows kiosk web page with PIN Grid
4. Enter the 4-digit PIN
5. Verify: "Validating..." spinner appears
6. Verify: "Welcome, {name}!" success screen appears for 3s
7. Verify: Screen transitions to game launching view
8. Verify: Game actually launches on the pod
9. Verify: Staff kiosk at `/staff` shows Pod 4 as "in_session"

**Error path tests:**
10. Enter wrong PIN → verify error message → verify returns to PIN Grid
11. Enter expired PIN → verify "PIN expired" message
12. Enter PIN for customer with zero wallet balance → verify "insufficient credits" message
13. Enter PIN while pod is in use (race condition) → verify graceful handling

### 7.4 Non-interference Tests

| Test | Description |
|------|-------------|
| Staff flow unaffected | Staff logs in at `/staff`, selects pod, launches game → entire flow works as before |
| Native lock screen flow unaffected | If server assigns auth token to pod (old flow), native lock screen PIN entry still works |
| Spectator page unaffected | `/spectator` page shows pod grid correctly |
| Customer landing unaffected | `/` (root) page shows 8-pod overview correctly |

---

## 8. Deployment Checklist

1. [ ] Fix lock screen overlay bug (staff-launch-pin-grid-block) — prerequisite
2. [ ] Build `CustomerPinGrid.tsx` component
3. [ ] Modify `/pod/[number]/page.tsx` to show PIN Grid when idle
4. [ ] Verify `RedeemPinResponse` type has all needed fields
5. [ ] Verify the `redeemPin` API endpoint works for the self-service flow
6. [ ] Run kiosk unit tests (`npm test`)
7. [ ] Rebuild kiosk (`npm run build`)
8. [ ] Deploy kiosk to server `.23` (port 3300)
9. [ ] Deploy kiosk to cloud (Bono VPS) — deploy parity
10. [ ] Verify pod browsers point to `/kiosk/pod/{number}` (may need rc-agent config change)
11. [ ] Verify native lock screen is hidden when pod is idle with kiosk web active
12. [ ] E2E test on one pod (Pod 8 canary first)
13. [ ] Roll out to all pods
14. [ ] Verify staff flow still works (non-interference)

---

## 9. Open Questions (Resolve Before Implementation)

1. **Pod browser URL:** What URL do pod Edge browsers currently load? If `/kiosk` (root), need to change to `/kiosk/pod/{N}`. Check rc-agent config or lock_screen.rs browser launch code.

2. **Native lock screen interaction:** When pod is idle, is the native lock screen in `ScreenBlanked` (covering the browser) or `Hidden` (browser visible)? If blanked, we need the server to send `ClearLockScreen` when transitioning to idle-with-kiosk mode.

3. **Server-side PIN generation status:** Is PWAL-01 (PIN generation in pin_launch.rs) complete? The kiosk PIN Grid depends on the server creating PINs that `redeem_pin()` can validate.

4. **Public endpoint auth:** `POST /kiosk/redeem-pin` — does it require staff JWT? If yes, it needs to be moved to public routes (customer has no JWT). Check `routes.rs` for auth middleware on this endpoint.

5. **Error response shape:** What exact JSON does `redeem_pin()` return on error? The `RedeemPinError` struct fields need to match what `CustomerPinGrid` parses. Verify the serde serialization format.

---

## 10. Summary

| Item | Status |
|------|--------|
| New files | 1: `kiosk/src/components/CustomerPinGrid.tsx` |
| Modified files | 1: `kiosk/src/app/pod/[number]/page.tsx` |
| Potentially modified | 1: `kiosk/src/lib/types.ts` (if RedeemPinResponse needs fields) |
| rc-agent changes | 0 (prerequisite bug fix is separate) |
| Server changes | 0 (uses existing `redeem_pin` endpoint) |
| New routes | 0 (uses existing `/pod/[number]` route) |
| New endpoints | 0 (uses existing `POST /kiosk/redeem-pin`) |
| Test files | 1: `kiosk/src/components/__tests__/CustomerPinGrid.test.tsx` |
