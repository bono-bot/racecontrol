# Codebase Concerns

**Analysis Date:** 2026-03-21

## Security Concerns

### PIN Validation Missing Request Validation

**Issue:** Staff and kiosk PIN endpoints receive plain text PINs via POST without rate limiting or brute-force protection visible in client code.

- **Files:** `src/lib/api.ts` (lines 150-175), `src/components/StaffLoginScreen.tsx`
- **Risk:** 4-digit PIN has only 10,000 possible combinations. Client submits immediately after 4 digits without artificial delay.
- **Current mitigation:** Server-side rate limiting (assumed but not visible in kiosk code)
- **Recommendations:**
  - Add client-side 2-second delay after failed attempts
  - Implement exponential backoff for consecutive failures
  - Add rate limiting headers validation in `fetchApi` catch blocks

### Authentication Token Handling in Browser State

**Issue:** Auth tokens and staff credentials stored in `sessionStorage` with automatic UI access.

- **Files:** `src/app/staff/page.tsx` (lines 26-28), `src/app/book/page.tsx` (line 58)
- **Risk:** Tokens available to any compromised script via XSS. No HTTPOnly flag equivalent in browser context.
- **Current mitigation:** 30-minute inactivity timeout in staff terminal
- **Recommendations:**
  - Never log or console-output tokens/sensitive auth data
  - Consider server-side session management instead of client-side token storage
  - Add CSP headers at server level to prevent inline script injection

### Walk-In Booking Logging

**Issue:** Anonymous staff walk-in bookings logged via unauthenticated debug endpoint with no access control check.

- **Files:** `src/app/book/page.tsx` (lines 397-405)
- **Pattern:** Hardcoded hostname lookup + inline fetch without error handling integration
- **Risk:** Bypasses audit trail if debug endpoint doesn't validate source
- **Recommendations:**
  - Use `api.createDebugIncident()` instead of direct fetch
  - Ensure debug incident creation requires staff authentication

### Bearer Token in API Responses

**Issue:** Bearer tokens returned in API responses logged via `console.error` in error states.

- **Files:** `src/app/staff/page.tsx` (line 265), `src/app/book/page.tsx` (line 303)
- **Risk:** Tokens visible in browser console and dev tools (accessible to any user with developer access)
- **Recommendations:**
  - Never log error responses containing tokens
  - Sanitize error messages before display: `"Login failed. Try again later."`

---

## Tech Debt & Fragile Areas

### Large Monolithic Pages

**Issue:** Booking flow condensed into single 1,300-line component.

- **Files:** `src/app/book/page.tsx` (1308 lines)
- **Why fragile:** Phase state machine, auth flows, wizard integration, multiplayer logic all in one component
- **Safe modification:** Extract phases into separate sub-components; move auth logic to custom hook
- **Test coverage gaps:** No unit tests visible for:
  - Phone → OTP → Wizard phase transitions
  - Multiplayer pod count validation
  - Error recovery paths

### Setup Wizard Type Safety

**Issue:** Unsafe type casts when accessing session types and config.

- **Files:** `src/components/SetupWizard.tsx` (line 553)
- **Pattern:**
  ```typescript
  const available = (t as unknown as Record<string, unknown>).available_session_types as string[] | undefined;
  ```
- **Risk:** Silently fails if server schema changes
- **Fix approach:** Add runtime validation with zod or similar; add integration tests

### JSON.parse Without Error Handling

**Issue:** Wizard build output parsed without try-catch in booking handlers.

- **Files:** `src/app/book/page.tsx` (lines 288, 329)
- **Pattern:**
  ```typescript
  bookingData.custom = JSON.parse(wizard.buildLaunchArgs());
  ```
- **Risk:** Malformed buildLaunchArgs() crashes booking flow silently
- **Recommendation:** Wrap in try-catch; validate schema before parsing

### API Error Response Assumption

**Issue:** All API endpoints assume `res.error` field without validating response structure.

- **Files:** `src/app/staff/page.tsx` (lines 194, 221, 264), `src/app/book/page.tsx` (lines 293, 334)
- **Pattern:** No schema validation for non-200 responses
- **Risk:** Silent failures if API changes response format
- **Fix approach:** Add response validator in `fetchApi()` or per endpoint

### Unsafe Type Assertions in Debug Flow

**Issue:** Debug endpoint responses cast to unknown then typed without validation.

- **Files:** `src/app/debug/page.tsx` (lines 628-629)
- **Pattern:**
  ```typescript
  if ((res as unknown as { error?: string }).error) { ... }
  ```
- **Risk:** No guarantee structure matches
- **Recommendation:** Use runtime type guard or add schema validation

---

## Performance Bottlenecks

### Socket Message Accumulation Without Limits

**Issue:** Recent laps and activity logs append to state arrays without culling during long sessions.

- **Files:** `src/hooks/useKioskSocket.ts` (lines 124, 279)
- **Patterns:**
  ```typescript
  setRecentLaps((prev) => [lap, ...prev].slice(0, 50));
  setActivityLog((prev) => [entry, ...prev].slice(0, 500));
  ```
- **Current capacity:** Capped at 50 laps, 500 activity entries — safe but wasteful
- **Scaling path:** Monitor memory usage under 8-pod concurrent load; consider IndexedDB for persistent log

### Wallet Balance Fetching in Loop

**Issue:** Staff page fetches wallet balances sequentially for all active billing sessions on every render.

- **Files:** `src/app/staff/page.tsx` (lines 96-110)
- **Pattern:** No debouncing or batch fetching
- **Improvement path:**
  - Add 5-second debounce on wallet fetch
  - Batch requests: `GET /wallet/batch?driver_ids=...`

### WebSocket Reconnection Debounce

**Issue:** Connection debounced 15 seconds from close, but immediate reconnect attempt every 3 seconds meanwhile.

- **Files:** `src/hooks/useKioskSocket.ts` (lines 323-335)
- **Risk:** UI shows "Disconnected" after 15s even though code retrying every 3s
- **Improvement:** Show "Connecting..." state between close and 15s timeout

---

## Known Bugs & Fragile Behaviors

### Multiplayer Pod Count Out of Range

**Issue:** `podCount` state accepts 2-8 but no validation against fleet size.

- **Files:** `src/app/book/page.tsx` (line 71)
- **Risk:** User selects 8 pods, only 3 available; booking fails after showing success
- **Trigger:** Booking page loaded when fleet has fewer pods than user selects
- **Workaround:** API returns error; user sees error phase
- **Fix approach:** Validate `podCount` against available pods on each change

### Staff Inactivity Timeout vs Socket Activity

**Issue:** Inactivity timer resets on keyboard/pointer events, but staff can remain "active" sending commands while socket disconnected.

- **Files:** `src/app/staff/page.tsx` (lines 45-50), `src/hooks/useKioskSocket.ts` (line 338)
- **Risk:** Staff assumes logged in, socket reconnects later with stale state
- **Recommendation:** Also listen for connection loss; show timeout warning before logout

### Session Storage Cleared Without Component Unmount

**Issue:** Staff logout clears `sessionStorage` but component state not reset to initial "idle" page.

- **Files:** `src/app/staff/page.tsx` (lines 38-41)
- **Risk:** Rapid re-login shows cached pod/session state from previous session
- **Workaround:** User clicks "Back" or page refresh
- **Fix approach:** Reset state on logout: `setSelectedPodId(null); setPanelMode(null);`

### Phone Input Type Mismatch

**Issue:** Phone number collected as string but no validation for international format or length.

- **Files:** `src/app/book/page.tsx` (line 56)
- **Risk:** Indian numbers (10 digits) vs other formats; API may reject silently
- **Recommendation:** Add phone validation: require 10+ digits for Indian numbers

### Multiplayer Result Pin Display After Success

**Issue:** In multiplayer mode, success phase shows individual PINs but no pod assignment order.

- **Files:** `src/app/book/page.tsx` (line 72)
- **Risk:** User doesn't know which PIN goes to which pod (Pod 1, 2, 3, etc.)
- **Trigger:** Multiplayer booking completes
- **Recommendation:** Display assignments table: Pod # | PIN | Duration

---

## Missing Critical Error Handling

### No Offline Fallback

**Issue:** All pages assume API connectivity; no offline mode or cached data fallback.

- **Files:** `src/lib/api.ts`, all pages
- **Risk:** Network glitch = complete feature unavailability
- **Improvement path:**
  - Cache pricing tiers, experiences, pod list in localStorage
  - Show stale data with warning badge during offline

### Missing Error Boundary for Async Operations

**Issue:** Catch blocks use `alert()` or `setErrorMsg()` but no global error logging.

- **Files:** `src/app/staff/page.tsx` (43 error handlers), `src/app/book/page.tsx` (5+ error handlers)
- **Risk:** User errors disappear; no server-side audit trail
- **Recommendation:** Add error logger service; send to `/api/v1/logs/client-error`

### Incomplete Async Error Propagation

**Issue:** Some async operations swallow errors entirely via `.catch(() => {})`.

- **Files:** `src/app/book/page.tsx` (lines 92-93, 405), `src/components/DriverRegistration.tsx` (lines 50)
- **Pattern:**
  ```typescript
  api.getAcCatalog().then((data) => setCatalog(data)).catch(() => {});
  ```
- **Risk:** Silent failures without user feedback
- **Recommendation:** Log and show fallback state: `"Could not load catalog"`

---

## Data Validation Gaps

### No Request Body Validation

**Issue:** API request bodies constructed from user input without schema validation.

- **Files:** `src/app/book/page.tsx` (lines 288-292)
- **Pattern:** Direct `JSON.stringify(data)` without runtime checks
- **Risk:** Backend receives malformed data; silent rejection
- **Fix approach:** Add zod schema validation before API calls

### Missing Pod ID Validation

**Issue:** `pod_id` used in URLs without checking format or existence.

- **Files:** `src/app/book/page.tsx` (line 421), `src/components/DriverRegistration.tsx` (line 89)
- **Pattern:** `.replace(/\D/g, "")` assumes format without validation
- **Risk:** XSS if pod_id contains user data
- **Recommendation:** Validate against pod list before display

### Unvalidated Cast Types in Socket Messages

**Issue:** WebSocket messages cast to specific types without runtime validation.

- **Files:** `src/hooks/useKioskSocket.ts` (lines 98-307)
- **Pattern:** `const pod = msg.data as Pod;` without checking required fields
- **Risk:** Missing fields cause silent state corruption
- **Fix approach:** Add runtime schema validator in `socket.onmessage`

---

## Testing & Coverage Gaps

### No Unit Tests Visible

**Files:** No `.test.ts`, `.spec.ts` files found in `src/`

- **Untested areas:**
  - Phone number validation logic
  - OTP flow state machine
  - Wizard step transitions
  - Multiplayer pod allocation
  - Billing session creation
  - Staff PIN entry with backspace handling

### No Integration Tests for Booking Flow

**Risk:**
- Phone → OTP → Wizard → Success path never verified end-to-end
- API response schema changes break silently
- Multiplayer race conditions not caught

### No E2E Tests

**Risk:** UI sequences (inactivity timeout, split session continuation) untested against actual browser/server interaction

---

## Missing Features & Gaps

### No Booking Cancellation Flow

**Issue:** Once PIN issued, no way for customer to cancel/refund via kiosk.

- **Files:** `src/app/book/page.tsx` (success phase)
- **Impact:** Customer stuck if wrong pod number shown; must call staff
- **Fix approach:** Add "Wrong Pod?" button in success phase → calls `/auth/cancel/{id}`

### No Real-Time Availability Check

**Issue:** Pod list loaded once on page load; no live update of availability.

- **Files:** `src/app/book/page.tsx` (line 91)
- **Risk:** User books pod that just became unavailable
- **Improvement:** Listen to pod status updates via socket; disable unavailable tiers

### No Wallet Balance Check Before Booking

**Issue:** Customer can book without verifying sufficient wallet balance.

- **Files:** `src/app/book/page.tsx` (booking handler)
- **Risk:** Booking succeeds, session starts, then fails at usage when wallet insufficient
- **Fix approach:**
  1. Fetch customer wallet balance after OTP verification
  2. Show balance and warn if < tier price
  3. Block booking if insufficient

### No Multiplayer Session Synchronization

**Issue:** Multiplayer success phase shows individual PINs but no mechanism to verify all pods start together.

- **Files:** `src/app/book/page.tsx` (multiplayer success)
- **Risk:** One pod launches game while others still waiting; desync
- **Recommendation:** Implement ready/confirm flow or server-side session join gate

---

## Dependencies at Risk

### Outdated Type Definitions

- `@types/react` at ^19 but react ^19.2.3 — minor version lag
- `@types/node` at ^22 — general

**Risk:** Type mismatches on major updates

**Recommendation:** Run `npm audit` regularly; pin versions after audit

### No Input Validation Library

**Issue:** Zero formal validation (no zod, joi, valibot).

- **Risk:** Schema changes break silently
- **Migration plan:** Add zod + create shared validators in `src/lib/validators.ts`

---

## Scaling Limits

### Session Storage Limit (5-10MB)

**Current usage:** Small (staff session + wizard state)

**Limit:** Browsers typically allow 5-10MB per domain

**Scaling issue:** If caching large data (all pricing tiers, experiences), could hit limit

**Scaling path:**
- Monitor sessionStorage usage
- Move to IndexedDB for >1MB data
- Implement pagination for driver lists

### WebSocket Message Queue

**Current:** No visible message queue in `useKioskSocket`

**Risk:** High-frequency telemetry (100+ msgs/sec) could be buffered excessively

**Scaling path:** Add message deduplication for telemetry; only process latest frame per pod

---

## Hardcoded Values & Configuration

### Hardcoded Timeouts

- `AUTO_RETURN_MS = 30_000` — auto return to home after 30s
- `INACTIVITY_MS = 120_000` — phone screen timeout
- `DEBOUNCE_MS = 15_000` — socket disconnection debounce
- `10_000` — error state auto-reset (lines 65-67)

**Files:** `src/app/book/page.tsx` (lines 16-17), `src/app/staff/page.tsx` (line 42), `src/hooks/useKioskSocket.ts` (lines 331, 14)

**Risk:** These should be configurable per deployment; hard to tune for different venues

**Recommendation:** Move to `.env` or `/kiosk/settings` API

### Hardcoded Pod Count Range

- Multiplayer booking: pod_count 2-8 hardcoded
- **Files:** `src/app/book/page.tsx` (UI hardcodes 2-8 without checking available pods)
- **Improvement:** Read from fleet health; disable out-of-range options

---

## Code Quality Issues

### Excessive Use of `!` Non-Null Assertion

- `wizard.selectedTier!.id` — assumes always present (lines 281, 322)
- **Files:** `src/app/book/page.tsx` (5+ instances)
- **Risk:** Silent null reference if wizard state corrupts
- **Recommendation:** Add null checks instead; show error state if missing

### Unreachable Code Patterns

- `socket.onerror = () => { socket.close(); }` followed by reconnect
- **Files:** `src/hooks/useKioskSocket.ts` (lines 338-340)
- **Pattern:** If socket.onerror already called, `ws.current` may not be updated
- **Recommendation:** Consolidate error/close handlers

### Missing Cleanup in Effects

- Wallet balance fetch effect depends on `billingTimers` but dependencies not exhaustive
- **Files:** `src/app/staff/page.tsx` (line 96)
- **Pattern:** Effect calls `api.getWallet()` but no cleanup for in-flight requests on unmount
- **Risk:** Race condition on rapid staff logout/login

---

## Documentation & Knowledge Transfer

### No API Contract Documentation

**Issue:** API endpoint schemas inferred from TypeScript types, not source of truth.

- **Risk:** When API changes, frontend breaks without clear migration path
- **Recommendation:** Add OpenAPI/Swagger spec or documentation file

### No State Machine Diagram for Booking Flow

**Issue:** 7-phase state machine (phone→otp→wizard→booking→success/error) not documented.

- **Files:** `src/app/book/page.tsx`
- **Risk:** Maintenance burden; new features require understanding complex phase logic
- **Recommendation:** Add ASCII diagram or state machine library (xstate)

### Missing Inline Documentation for Socket Events

**Issue:** WebSocket event handlers not documented with expected payload schemas.

- **Files:** `src/hooks/useKioskSocket.ts` (lines 96-316)
- **Risk:** New events added without clear contract
- **Recommendation:** Add JSDoc for each case statement

---

## Browser Compatibility

### No Graceful Degradation for Older Browsers

**Issue:** Uses modern React 19 APIs without polyfills.

- **Risk:** IE11 or old Safari versions fail silently
- **Recommendation:** Test against supported browser list; document requirements

### WebSocket Not Fallback-Protected

**Issue:** Requires WebSocket support; no fallback to polling.

- **Files:** `src/hooks/useKioskSocket.ts` (line 80)
- **Risk:** Firewalls blocking WebSocket disconnect kiosk from live updates
- **Recommendation:** Add feature detection; fall back to 5-second polling if WS unavailable

---

*Concerns audit: 2026-03-21*
