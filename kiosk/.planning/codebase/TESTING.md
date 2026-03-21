# Testing Patterns

**Analysis Date:** 2026-03-21

## Test Status

**No automated test framework configured.** The kiosk is an interactive UI without unit or integration test setup. No test files exist in the repository.

- Jest: Not installed
- Vitest: Not installed
- Testing Library: Not installed
- Test files: None (no `.test.ts`, `.spec.ts`, `.test.tsx`, or `.spec.tsx` files in `src/`)

## Why No Tests

The kiosk is a **real-time React + Next.js dashboard** with:
1. **Direct WebSocket dependency** — connects to server at runtime, no mock infrastructure
2. **Full-screen interactive UI** — PIN entry, modal overlays, touch events
3. **Local browser state** — modals, timers, inactivity tracking
4. **Zero business logic** — all logic in backend (rc-agent, racecontrol server)

Testing would require:
- Mock WebSocket server (not in scope)
- Browser/DOM simulation (Would need Testing Library + jsdom)
- Component snapshot testing (Fragile for animated UIs)

## Test Strategy (If Needed)

If manual or automated testing becomes necessary:

**Manual Testing Approach (Current):**
- Deploy to server at `localhost:3300` or `192.168.31.23:3300`
- Test via actual kiosk touchscreen or browser
- Verify WebSocket connection via DevTools Network tab
- Check console logs in browser DevTools (all logs prefixed `[Kiosk]`)

**Potential Automated Approach (Future):**
- **Unit tests:** Logic functions only (formatters, state derivation)
  - `formatLapTime(ms)` → `"0:01.234"`
  - `derivePodState(pod, billing, ...)` → state enum
- **Integration tests:** API wrapper tests with mocked fetch
  - Mock `fetch()` responses
  - Test retry/error handling in `lib/api.ts`
- **E2E tests:** Playwright/Cypress against live server
  - Real WebSocket connection
  - Pin validation flow
  - Pod card rendering

## Development Testing

**Local Development Server:**
```bash
npm run dev           # Start dev server on localhost:3300
```

**Verify Connection:**
1. Open browser DevTools (F12)
2. Go to Network tab → WS (WebSocket) filter
3. Look for connection to `ws://localhost:8080/ws/dashboard` (or `192.168.31.23:8080` on server)
4. Check Console tab for `[Kiosk] Connected to RaceControl` message

**Test Interactive Features:**
- PIN Entry: Click pod card → enter 4 digits
- Modals: Click buttons to open/close overlays
- State: Use DevTools React tab to inspect `useKioskSocket()` hook state
- WebSocket Events: Check Network tab WS tab for message frames

## Logging for Debugging

**All debug output uses prefixed console logs:**

```typescript
// Connection lifecycle
console.log("[Kiosk] Connected to RaceControl");
console.log("[Kiosk] Disconnected, retrying in 3s...");
console.log("[Kiosk] 15s debounce expired -- marking disconnected");

// Data updates
console.log("[Kiosk] Pod reservation changed:", data);

// Errors
console.warn("[Kiosk] Parse error:", error);
console.error("[ErrorBoundary] Caught:", error);
console.error("Start Now failed:", error);
```

**To debug:**
1. Open browser DevTools (F12)
2. Console tab filters: search `[Kiosk]`, `[ErrorBoundary]`, etc.
3. Check WebSocket frames (Network → WS tab) to see raw messages

## Build Verification

**Build process:**
```bash
npm run build         # Verify TypeScript compilation, no type errors
```

**Next.js lint:**
```bash
npm run lint          # Run Next.js built-in linter
```

No automated tests run. Compilation and TypeScript strict mode act as the primary safety net.

## Error Boundary Coverage

**Component:** `src/components/ErrorBoundary.tsx`

- Catches React render errors
- Displays fallback UI with "Tap to Reload" button
- Logs error to console via `componentDidCatch(error)`
- Only errors in `<ErrorBoundary>` children are caught (wrapped in `src/app/layout.tsx`)

**What It Catches:**
- Component render exceptions
- Lifecycle method errors

**What It Doesn't Catch:**
- Event handler errors (no try-catch in click handlers)
- Async errors (promises, fetch, WebSocket)
- Server-side rendering errors

## Manual Test Scenarios

**Scenario 1: WebSocket Connection Loss**
1. Start kiosk (should show "Live")
2. Stop racecontrol server (kill port 8080)
3. Kiosk should show "Connecting..." after 15s
4. Restart server
5. Kiosk should auto-reconnect and show "Live"

**Scenario 2: PIN Entry Flow**
1. Click idle pod card
2. Enter 4-digit PIN
3. Auto-submits on 4th digit
4. Shows "Validating PIN..."
5. Success: shows welcome screen with pod number
6. Error: shows error message, auto-returns to numpad after 10s

**Scenario 3: Active Session Display**
1. Start a billing session via API or staff panel
2. Kiosk pod card should show telemetry (speed, RPM, brake, laps)
3. Timer should count down from allocated seconds
4. Remove billing session (end/pause/resume)
5. UI should update immediately via WebSocket event

**Scenario 4: Billing Warning**
1. Start billing session with <60 seconds remaining
2. Should display warning overlay
3. Auto-dismiss after 10 seconds

## TypeScript Strict Checking

All files compiled with `"strict": true`:
- `noImplicitAny`: All types explicit
- `strictNullChecks`: Null/undefined handled explicitly
- `strictFunctionTypes`: Function parameter types exact
- `noImplicitThis`: `this` binding always explicit

**Verify:**
```bash
npx tsc --noEmit     # Type check without emitting
```

Should show zero errors.

## Performance Notes

**No performance tests configured.**

**Known Performance Characteristics:**
- WebSocket reconnection: 3s retry interval
- Connection debounce: 15s before showing "Disconnected" in UI
- Billing countdown: 1s local tick between WebSocket updates
- Recent laps: Keep last 50 laps in state (slice on update)
- Activity log: Keep last 500 entries in state

**Monitor in DevTools Performance tab:**
- React render time (should be <100ms for state updates)
- WebSocket message processing (should be <50ms per message)

## Future Test Infrastructure

If automation needed, recommended stack:
- **Unit tests:** Jest + React Testing Library
- **E2E tests:** Playwright (can test real WebSocket)
- **Coverage:** Start with critical paths (PIN validation, state derivation)
- **CI/CD:** Run in GitHub Actions on PR

---

*Testing analysis: 2026-03-21*
