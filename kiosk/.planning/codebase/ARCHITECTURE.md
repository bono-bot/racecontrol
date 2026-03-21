# Architecture

**Analysis Date:** 2026-03-21

## Pattern Overview

**Overall:** Next.js 16 App Router with real-time WebSocket-driven state synchronization and multi-screen kiosk experience

**Key Characteristics:**
- Client-side (CSR) with React 19 for interactive UI components
- Real-time bidirectional communication via WebSocket (`/ws/dashboard`)
- Type-safe data flow using TypeScript interfaces from `@/lib/types`
- RESTful API integration with RaceControl server (port 8080)
- Multi-mode operation: customer landing, staff terminal, spectator display, debugging
- Session-based authentication with PIN validation and staff login
- Responsive 4x2 pod grid layout with live telemetry streaming

## Layers

**Presentation (UI Components):**
- Purpose: Render interactive kiosk screens and staff dashboards
- Location: `src/components/`
- Contains: React components (TSX) for pod cards, modals, panels, displays
- Depends on: `useKioskSocket` hook, `api` client, `@/lib/types`
- Used by: App route pages (`src/app/*/page.tsx`)
- Key files:
  - `KioskPodCard.tsx` — individual pod status display with telemetry
  - `StaffLoginScreen.tsx` — PIN authentication for staff
  - `SetupWizard.tsx` — multi-step booking/experience selection
  - `SidePanel.tsx` — driver management, game picker, wallet topup

**State Management (Hooks):**
- Purpose: Manage real-time data synchronization and application state
- Location: `src/hooks/`
- Contains: Custom React hooks for WebSocket, setup wizard state
- Depends on: WebSocket API, `@/lib/types`
- Used by: All page components
- Key files:
  - `useKioskSocket.ts` — central WebSocket connection, event handlers, state management (pods, billing, telemetry, game states, auth tokens)
  - `useSetupWizard.ts` — booking flow state (driver registration, pricing selection, game config)

**API Client Layer:**
- Purpose: Encapsulate HTTP API calls to RaceControl server
- Location: `src/lib/api.ts`
- Contains: Typed API wrappers for all backend endpoints
- Depends on: `@/lib/types`, Fetch API
- Used by: Components via `api.*` imported object
- Key patterns:
  - `fetchApi<T>()` — generic typed fetch wrapper
  - Health & fleet endpoints: `/api/v1/health`, `/api/v1/fleet/health`
  - Booking: `/api/v1/customer/book`, `/api/v1/kiosk/book-multiplayer`
  - Billing: `/api/v1/billing/*` (start, split options, continue-split)
  - Pod control: `/api/v1/pods/{id}/*` (power, lockdown, freedom mode, unrestrict)
  - Games: `/api/v1/games/*` (launch, relaunch, stop)
  - Staff: `/api/v1/staff/validate-pin`
  - Kiosk experiences: `/api/v1/kiosk/experiences`, `/api/v1/kiosk/pod-launch-experience`

**Type System:**
- Purpose: Central type definitions for data contracts
- Location: `src/lib/types.ts`
- Contains: TypeScript interfaces for all domain objects
- Key types:
  - `Pod` — pod status, driver, game state, session info
  - `BillingSession` — driver billing, allocated time, remaining seconds, status
  - `TelemetryFrame` — real-time vehicle data (speed, RPM, brake, throttle, lap number)
  - `GameLaunchInfo` — game state, process info, diagnostics
  - `AuthTokenInfo` — PIN/QR auth tokens, status, allocation
  - `KioskExperience` — preset experiences (game, track, car, duration)
  - `GameLaunchRequest` — game launch requests from staff
  - Deploy states: `DeployState` (idle, downloading, starting, complete, failed)

**Utilities & Constants:**
- Purpose: Shared helpers and configuration
- Location: `src/lib/` and root config
- Contains: Game display mapping, constants, formatting helpers
- Key files:
  - `gameDisplayInfo.ts` — game abbr and logo paths for UI rendering
  - `constants.ts` — game list, difficulty presets, class colors

## Data Flow

**Customer PIN Entry & Session Start:**

1. Customer taps available pod on landing page (`src/app/page.tsx`)
2. PIN modal (`PinModal` component) captures 4-digit code
3. `api.validateKioskPin(pin, pod_id)` calls backend `/auth/kiosk/validate-pin`
4. On success: displays welcome modal with allocated time, auto-closes
5. WebSocket receives `billing_session_list` update with new active session
6. Pod card transitions from "idle" → "in_session" with live telemetry

**Booking Flow (Multi-step Wizard):**

1. Customer taps "Book a Session" button (landing page footer)
2. Routes to `/book` page (`src/app/book/page.tsx`) — 1308 lines, handles:
   - Phone OTP authentication via `api.customerLogin()` + `api.customerVerifyOtp()`
   - Driver registration (name, email, phone)
   - Pricing tier selection (displayed from `/pricing` endpoint)
   - Kiosk experience picker (preset vs custom game config)
   - Session splits (e.g., 3x15-minute splits within 45 minutes)
   - Multiplayer booking (multi-pod sessions)
3. Final booking call: `api.customerBook(token, {pricing_tier_id, experience_id})`
4. Backend returns PIN, pod number, allocated seconds
5. Display success screen with PIN for customer

**Staff Terminal Flow:**

1. Staff navigates to `/staff` → `StaffLoginScreen` component
2. PIN validation via `api.validateStaffPin(pin)`
3. Sets `sessionStorage.kiosk_staff_name` + `kiosk_staff_id` for 30-min session
4. Routes to `/control` (staff control panel) after hydration check
5. Renders full pod grid with staff-only controls:
   - Wake/shutdown/restart individual pods
   - Pod enable/disable (maintenance)
   - Experience launch (quick-start preset games)
   - Lockdown toggles (taskbar/keyboard restrictions)
   - Bulk power actions (wake all, shutdown all, restart all)

**Real-time State Synchronization via WebSocket:**

1. `useKioskSocket` hook establishes connection to `ws://{host}:8080/ws/dashboard`
2. On connect, listens for events:
   - `pod_list` — full pod inventory on startup
   - `pod_update` — individual pod state change
   - `billing_session_list` — active sessions on startup
   - `billing_tick` — per-second billing countdown updates
   - `billing_session_changed` — session completion, split continuation trigger
   - `telemetry` — live vehicle data (speed, RPM, brake, lap times)
   - `lap_completed` — valid lap recorded
   - `game_state_changed` — game launch/stop/error states
   - `assistance_needed` — driver help request popup
   - `game_launch_requested` — banner notification (Phase 80+)
   - `GameLaunchRequested` — external game launch request
3. Hook updates local Maps (pods, billing, telemetry) → React state
4. Components derive UI from state (pod cards, active session panels)
5. 15-second debounce on disconnect to prevent false "Connecting..." flashes

**Pod Control Flow:**

1. Staff clicks "Wake Pod" / "Shutdown" / "Restart" / "Toggle Lock" on pod card
2. Components call `api.wakePod()`, `api.shutdownPod()`, `api.lockdownPod()`, etc.
3. Backend executes pod command via `rc-agent` on that pod
4. WebSocket broadcasts `pod_update` with new status
5. Local hook state updates → component re-renders pod card

**Game Launch & Session Flow:**

1. Customer (at pod) or staff (staff terminal) initiates game launch
2. `api.launchGame(pod_id, sim_type, launch_args)` → backend queues game start
3. Backend detects game process on pod agent, broadcasts `game_state_changed` → "launching"
4. Billing session starts via `billing_session_list` event
5. Kiosk pod card shows active session with live telemetry
6. When game stops: `game_state_changed` → "idle" + `billing_session_changed` → "completed"
7. If session is split: `pendingSplitContinuation` state triggers between-sessions UI

**Multiplayer Group Booking & Reservation:**

1. Booking flow with `pod_count > 1` calls `api.kioskBookMultiplayer(token, {pod_count, experience_id})`
2. Backend allocates multiple pods, returns `KioskMultiplayerResult` with assignments (pins, pod numbers, roles)
3. Customer receives group session ID and individual PINs for each pod
4. WebSocket broadcasts `group_session_all_validated` with `MultiplayerGroupStatus`
5. On `/control`, `acServerInfo` state shows AC server status + connected pods
6. Staff can configure AC session via `api.updateAcSessionConfig()` before pods join

**State Management:**

- **Central Hub:** `useKioskSocket` hook manages all real-time state
- **Local Component State:** Pin entry, modal steps, UI toggles
- **SessionStorage:** Staff authentication (name, ID) — persists across page reloads, cleared on logout
- **Maps (not arrays):** Pods, billing timers, telemetry, game states keyed by `pod_id` for efficient lookups
- **Debouncing:** 15s disconnect debounce prevents UI flicker during CPU spikes (e.g., game launch)
- **Auto-cleanup:** Billing sessions removed from state when completed; assistance requests auto-expire after 10s

## Key Abstractions

**Pod Card State Machine:**

File: `src/components/KioskPodCard.tsx`

Function `derivePodState()` maps pod + billing + game + auth state → `KioskPodState`:
- `idle` — pod available, no session
- `registering` — pending assignment (PIN not yet consumed)
- `waiting` — assignment consumed, waiting for game launch
- `selecting` — customer in game selection UI
- `loading` — game process detected, billing not yet started
- `on_track` — game running, billing active
- `crashed` — game crashed, session ended
- `join_failed` — multiplayer join failed
- `ending` — session completed, billing stopped

Used to render appropriate UI: available button → active session → error state

**Billing Session Lifecycle:**

File: `src/lib/types.ts` → `BillingStatus` enum

States:
- `pending` — created, not yet started
- `active` — billing running (second ticker)
- `paused_manual` — staff paused (not fully implemented in kiosk)
- `completed` — normal end (all time used)
- `ended_early` — manual stop by staff
- `cancelled` — cancelled before start

Components watch for `billing_session_changed` event to detect transitions.

**Auth Token Workflow:**

File: `src/lib/types.ts` → `AuthTokenInfo`, `AuthTokenStatus`

States:
- `pending` — created, waiting for consumption
- `consumed` — PIN entered on pod, session starting
- `expired` — timeout without consumption
- `cancelled` — staff cancelled

Flow: booking generates token → customer enters PIN on pod (consume) → billing starts

**Panel Mode Dispatcher:**

File: `src/app/staff/page.tsx` → `PanelMode` type

Drives side panel behavior:
- `null` — hidden (default)
- `setup` — new driver registration + booking
- `live_session` — active billing session controls
- `waiting` — pending split continuation
- `wallet_topup` — credit addition UI
- `game_picker` — game selection + launch

Staff clicks pod → triggers mode change → component unmounts old panel, mounts new one

## Entry Points

**Customer Landing Page:**
- Location: `src/app/page.tsx`
- Triggers: User navigates to `/` (root kiosk)
- Responsibilities:
  - Display 4x2 pod grid (8 total slots)
  - Show pod availability (idle/active count badges)
  - Handle PIN modal for session start
  - WebSocket pod list subscription
  - Link to booking (`/book`) and staff login (`/staff`)

**Booking Page:**
- Location: `src/app/book/page.tsx` (1308 lines)
- Triggers: Customer taps "Book a Session" button
- Responsibilities:
  - Phone OTP authentication flow
  - Driver registration
  - Pricing tier selection
  - Experience/game picking
  - Session splits configuration
  - Multiplayer pod reservation
  - Generate and display PIN for customer

**Staff Control Panel:**
- Location: `src/app/control/page.tsx`
- Triggers: Successful staff PIN auth at `/staff`
- Responsibilities:
  - Full pod grid with real-time status
  - Bulk power actions (wake/shutdown/restart all)
  - Pod enable/disable for maintenance
  - Lockdown toggles
  - Experience launching
  - Monitored pod state via WebSocket

**Staff Login:**
- Location: `/staff` (handled by `StaffLoginScreen` component)
- Entry: Unauthenticated staff member
- Responsibilities:
  - 4-digit PIN entry
  - Validate via `api.validateStaffPin()`
  - Set sessionStorage auth tokens
  - 30-minute inactivity auto-logout

**Spectator / Debug Pages:**
- Location: `src/app/spectator/page.tsx`, `src/app/debug/page.tsx`
- Usage: External displays, operations monitoring
- Responsibilities:
  - Live spectator feed (pod carousel, leaderboard)
  - Debug activity monitor (pod health, billing events, game errors)
  - Incident creation + diagnosis system
  - Activity log viewer

**Pod Detail Page:**
- Location: `src/app/pod/[number]/page.tsx`
- Triggers: Navigate to `/pod/1` (pod number 1-8)
- Responsibilities:
  - Single pod deep-dive view
  - Full telemetry + lap history for driver
  - Staff session controls
  - Game configuration panel

## Error Handling

**Strategy:** Try-catch wrapping API calls + user-facing error messages in modals/alerts

**Patterns:**

1. **PIN Validation Errors:**
   - File: `src/app/page.tsx` → `handleSubmit()` function
   - On error: show error modal for 10 seconds, auto-return to numpad
   - User can retry
   - Display message from backend (e.g., "PIN expired", "Pod offline")

2. **API Fetch Errors:**
   - File: `src/lib/api.ts` → `fetchApi<T>()` wrapper
   - Throws on non-200 response with error text (first 200 chars)
   - Components wrap calls in try-catch, display toast/modal with message
   - No retry logic built-in; user initiates manual retry

3. **WebSocket Disconnections:**
   - File: `src/hooks/useKioskSocket.ts`
   - `socket.onclose` → attempts reconnect every 3 seconds
   - 15-second debounce before showing "Disconnected" UI (prevent flickers)
   - Components check `connected` boolean to conditionally disable buttons

4. **Game Launch Failures:**
   - File: `GameLaunchInfo` type includes `diagnostics` with error codes
   - `error_message` field populated by backend on launch failure
   - UI shows error and "Retry" or "Relaunch" buttons
   - Components call `api.relaunchGame()` or `api.retryPodJoin()`

5. **Billing Edge Cases:**
   - Insufficient time: `api.startBilling()` returns error
   - Session auto-end at 0 seconds: `billing_session_changed` event triggers cleanup
   - Split continuation timeout: `pendingSplitContinuation` state expires UI after 2 minutes

## Cross-Cutting Concerns

**Logging:**
- Console logging: `console.log()` with `[Kiosk]` prefix for WebSocket events
- No persistent log storage in kiosk; backend logs stored in RaceControl server
- Example: `console.log("[Kiosk] Connected to RaceControl")`

**Validation:**

1. **PIN Entry:** Must be exactly 4 digits (0-9), auto-submit on 4th digit
2. **Phone Number:** Validated by backend during OTP flow
3. **Pricing Tier:** Must be active (`is_active: true`) to display
4. **Experience:** Must exist in catalog + be active
5. **Pod Number:** Must be 1-8, mapped to grid slots

**Authentication:**

1. **Customer PIN:** 4-digit code generated by backend, consumed once per session start
2. **Staff PIN:** 4-digit code, global (not per-pod)
3. **Session Storage:** Staff name/ID stored in `sessionStorage` (client-side, cleared on logout)
4. **No Token Expiry in Kiosk:** Rely on backend for token validation; kiosk just forwards PIN
5. **OTP Flow:** Phone-based auth for booking, SMS delivery handled by backend

**Hydration:**
- Staff auth state: read from `sessionStorage` in `useEffect` after mount (not in SSR)
- Example: `setStaffName(sessionStorage.getItem("kiosk_staff_name"))`
- Prevents mismatch between server and client renders

**Real-time Consistency:**

1. **Optimistic Updates:** None; all state changes driven by WebSocket events
2. **Eventual Consistency:** Components show state from last WebSocket message
3. **Stale State:** If WebSocket disconnects, UI shows last known state + "Connecting..." badge
4. **Deduplication:** Maps (keyed by pod_id, pod_number, driver_id) prevent duplicate entries

**Performance:**

1. **Large Lists:** Recent laps stored as array, capped at 50 entries via `slice(0, 50)`
2. **Pod Grid:** 8 slots (fixed), rendered even if pod offline (placeholder card)
3. **Telemetry:** Latest frame per pod_id (Map keeps only current, not history)
4. **Billing Timers:** Per-pod session (one active session per pod max)
5. **Activity Log:** Capped at 500 entries via `slice(0, 500)`

**Accessibility:**
- ARIA labels on pod cards (testid attributes for testing)
- Keyboard navigation: tab through buttons, enter to activate
- Color contrast: RP colors (`#E10600` red, `#1A1A1A` black) meet WCAG AA
- Font sizes: minimum 12px for body, larger for headers

---

*Architecture analysis: 2026-03-21*
