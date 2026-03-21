# Codebase Concerns

**Analysis Date:** 2026-03-21

## Tech Debt

**Hardcoded API Secrets:**
- Issue: Terminal endpoint uses hardcoded secret `"rp-terminal-2026"` in API header
- Files: `src/lib/api.ts` lines 795, 805
- Impact: Terminal access not properly gated by authentication; secret embedded in client code is exposed to all users
- Fix approach: Move terminal auth to JWT-based session verification. Pass session token in header only, never embed secrets in client code

**Empty Error Handling Returns:**
- Issue: `fetchApi` returns `{} as T` on JWT auth errors, masking actual errors
- Files: `src/lib/api.ts` line 85
- Impact: Callers cannot distinguish auth failure from network error; silently fails and redirects without logging
- Fix approach: Return explicit error type (e.g., `{ error: "session_expired" }`) instead of empty object; callers should handle auth errors explicitly

**Silent Network Failures:**
- Issue: Multiple `catch` blocks silently swallow errors without logging or user feedback
- Files: `src/app/wallet/topup/page.tsx` lines 56, 115; `src/app/wallet/history/page.tsx` line 110; `src/app/dashboard/page.tsx` line 43
- Impact: Data load failures go unnoticed; users see incomplete or stale data without knowing requests failed
- Fix approach: Implement consistent error logging strategy. Show toast notifications for all network failures, not just selected flows

**Type Coercion with `any`:**
- Issue: Multiple `as any` casts in chart/telemetry code bypass type safety
- Files: `src/components/TelemetryChart.tsx` line 33; `src/app/book/page.tsx` line 902; `src/app/leaderboard/page.tsx` line 72; `src/app/scan/page.tsx` lines 48, 112, 186
- Impact: Cannot catch data shape mismatches at compile time; runtime crashes if API response schema changes
- Fix approach: Define strict TypeScript interfaces for all external data. Replace `any` with proper union types

---

## Known Bugs

**QR Code Cleanup Not Guaranteed:**
- Issue: QR scanner cleanup in `handleScan` uses try-catch but doesn't verify stop completed
- Files: `src/app/scan/page.tsx` lines 45-52
- Impact: Scanner may leak references or continue running if stop() fails; memory leak on repeated scans
- Workaround: Page unmount cleanup (lines 107-118) attempts secondary cleanup, but race condition possible
- Fix approach: Add explicit state flag to track scanner lifecycle; verify stop() in both error and success paths

**Null Dereference in Group Session Logic:**
- Issue: Code assumes `gRes.group_session.members` is always defined before accessing
- Files: `src/app/dashboard/page.tsx` lines 36-38
- Impact: If API returns `group_session` without `members` array, `.find()` throws runtime error
- Trigger: API contract mismatch or partial response
- Fix approach: Add explicit `members?.length > 0` check before calling `.find()`

**Razorpay Checkout Script Loading Race:**
- Issue: `handleTopUp` checks `window.Razorpay` immediately, but Script tag uses `strategy="afterInteractive"`
- Files: `src/app/wallet/topup/page.tsx` lines 68, 128
- Impact: If user clicks "Top Up" before checkout.js loads, shows error but doesn't retry automatically
- Trigger: Fast click on "Top Up" button on slow connection
- Fix approach: Add retry logic with exponential backoff, or use Script `onLoad` callback to gate button

**Phone Number Formatting Inconsistency:**
- Issue: Login adds `+91` prefix, but API expects consistent format
- Files: `src/app/login/page.tsx` lines 42, 54
- Impact: If user enters `+919876543210`, it gets double-prefixed to `+91+919876543210`
- Trigger: User manually enters country code
- Fix approach: Normalize input: strip existing `+91`, accept `10` or `+91<10>` formats

**Confetti Gate Check Vulnerable to Timing:**
- Issue: Confetti uses `sessionStorage` to prevent re-trigger, but check happens after component mounts
- Files: `src/components/Confetti.tsx` lines 51-55
- Impact: Very fast page navigation may trigger multiple confetti animations simultaneously
- Fix approach: Move gate check to useEffect dependency, or use ref to track state

---

## Security Considerations

**Client-Side Auth Token Storage:**
- Risk: `localStorage.getItem("rp_token")` stores JWT in browser storage, vulnerable to XSS
- Files: `src/lib/api.ts` lines 22, 26, 30; `src/app/layout.tsx` lines 41-42; multiple pages
- Current mitigation: Token stored in localStorage; no Content-Security-Policy or HttpOnly cookie fallback
- Recommendations:
  1. Migrate to HttpOnly cookies (set by server on `/login`)
  2. Implement CSP header: `default-src 'self'; script-src 'self' https://checkout.razorpay.com`
  3. Add SameSite=Strict to all cookies
  4. Store temp session ID in sessionStorage only, never JWT

**Terminal Secret Exposed in Client:**
- Risk: Terminal auth secret `"rp-terminal-2026"` hardcoded in client JS, visible in network tab
- Files: `src/lib/api.ts` lines 795, 805
- Current mitigation: None; assumes secret stays confidential
- Recommendations:
  1. Remove hardcoded secret immediately
  2. Use JWT-based terminal session tokens (short TTL, signed by server)
  3. Only allow terminal access from James's machine IP (192.168.31.27)
  4. Log all terminal commands to audit trail with user/timestamp

**Razorpay Key Exposed in Client:**
- Risk: Razorpay `key_id` from `createTopupOrder` response is shown in browser memory and network logs
- Files: `src/app/wallet/topup/page.tsx` lines 91, 128
- Current mitigation: None; key_id is meant to be client-accessible per Razorpay docs
- Recommendations:
  1. Verify key_id is "publishable" key (safe for client), not secret
  2. Implement Server-Side Encryption (SSE) for payment responses
  3. Validate order_id on server after payment success before crediting wallet

**Leaderboard Data Unvalidated:**
- Risk: Public leaderboard API response (`publicApi.leaderboard()`) has no schema validation
- Files: `src/app/leaderboard/page.tsx` lines 57, 70-77
- Impact: If server sends malformed data, `entries.map()` may crash or display garbage
- Fix approach: Add Zod/io-ts schema validation for all public API responses

**Clear.html Logout Page in Public Dir:**
- Risk: `/clear.html` endpoint is unauthenticated and clears all localStorage
- Files: `public/clear.html` lines 9-10
- Impact: Any user with app URL can trigger logout of another user via redirect or iframe
- Fix approach:
  1. Move logout to authenticated POST endpoint `/api/logout`
  2. Require CSRF token
  3. Verify user ID in session

---

## Performance Bottlenecks

**Leaderboard Full Reload on Track Change:**
- Problem: Switching tracks fetches entire leaderboard dataset without pagination
- Files: `src/app/leaderboard/page.tsx` lines 66-80
- Cause: `api.leaderboard(track)` has no limit parameter; returns all records
- Improvement path:
  1. Add `limit` and `offset` query params to API call
  2. Implement infinite scroll or pagination UI
  3. Cache track data in state to avoid refetch

**TelemetryChart Large Sample Set Rendering:**
- Problem: All telemetry samples rendered to recharts simultaneously; no decimation
- Files: `src/components/TelemetryChart.tsx` lines 69-77
- Cause: Samples uploaded per driver at 50Hz+ = 3000+ points per 1-minute lap
- Improvement path:
  1. Implement sample decimation (every Nth point or max-points-per-chart)
  2. Use responsive container to adjust quality based on viewport
  3. Add debounce to chart re-renders on window resize

**Book Wizard Catalog Loaded Eagerly:**
- Problem: AC Catalog (all tracks, cars, presets) fetched on page load, not just when needed
- Files: `src/app/book/page.tsx` lines 138-145
- Cause: Catalog needed for preset display, but downloaded before user selects flow
- Improvement path:
  1. Lazy-load catalog only when user reaches "Track" step
  2. Load presets separately from full catalog
  3. Implement catalog pagination if 500+ items

**Dashboard Parallel Requests Without Cache:**
- Problem: Every dashboard page load fires 4 API calls in parallel with no caching
- Files: `src/app/dashboard/page.tsx` lines 24-29
- Cause: No stale-while-revalidate or SWR strategy; no localStorage cache
- Improvement path:
  1. Implement React Query or SWR for automatic caching
  2. Cache profile for 5min, stats for 1min, sessions for 30s
  3. Reduce parallelization: `profile` must load first; load `stats` after

---

## Fragile Areas

**Book Wizard Step State Machine:**
- Files: `src/app/book/page.tsx` (entire file, 900+ lines)
- Why fragile:
  1. 8-9 step wizard with complex branching (`STEP_LABELS_SINGLE` vs `STEP_LABELS_MULTI`)
  2. State scattered across multiple useState calls (step, tier, game, mode, track, car, difficulty, transmission, selectedFriends, couponCode, etc.)
  3. No unified state machine; step logic depends on previous selections being non-null
  4. Radio button selections not validated before proceeding to next step
- Safe modification:
  1. Refactor to single reducer + context for state
  2. Add validation schema (Zod) to enforce required fields per step
  3. Write characterization tests for all step transitions
- Test coverage: Minimal; only user-facing tests, no unit tests for step logic

**Fetch Error Boundary Missing:**
- Files: `src/lib/api.ts` (entire fetchApi function)
- Why fragile:
  1. No distinction between 4xx (client error), 5xx (server error), timeout
  2. JWT auto-logout on any "error" field in response, even if error is unrelated to auth
  3. No retry logic; transient failures cause immediate logout
  4. No logging; impossible to diagnose why requests failed
- Safe modification:
  1. Add explicit error type classification
  2. Implement exponential backoff retry for 5xx
  3. Add request/response logging (redacted tokens)
  4. Only logout on `401 Unauthorized` or `"JWT decode error"` specifically
- Test coverage: None; all error paths untested

**Token Migration Logic in Layout:**
- Files: `src/app/layout.tsx` lines 39-46
- Why fragile:
  1. Script runs on every page load; clears token if version != "2"
  2. If `rp_auth_v` get corrupted (e.g., set to null), causes infinite logout loop
  3. No way to opt-out or debug the migration
- Safe modification:
  1. Run token migration only once on first mount (use sessionStorage flag)
  2. Log migration to console in dev mode
  3. Add versioning scheme (1→2→3) with explicit changelog
- Test coverage: Zero

---

## Scaling Limits

**Leaderboard Memory Usage:**
- Current capacity: Tested with 500 entries; 1000+ entries start causing jank
- Limit: ~2000 entries before browser tab becomes unresponsive
- Scaling path:
  1. Implement windowing (render only visible rows)
  2. Add server-side pagination (100 per page)
  3. Use IndexedDB to cache leaderboard locally

**Simultaneous API Requests:**
- Current capacity: Dashboard loads 4 requests in parallel; book wizard lazy-loads 1-3 more
- Limit: Browser tab slowdown with >10 concurrent requests; possible connection pool exhaustion
- Scaling path:
  1. Implement request queue (max 4 concurrent, queue rest)
  2. Add abort controller for cancelled requests
  3. Implement priority queue (user-initiated > background)

**TelemetryChart Large Lap Data:**
- Current capacity: 5000 samples = ~100sec lap at 50Hz; renders in ~1-2 seconds
- Limit: 20000+ samples cause 10+ second render, browser freeze
- Scaling path: Decimation as noted above; consider worker threads for data processing

---

## Dependencies at Risk

**Html5-Qrcode (QR Scanner):**
- Risk: Unmaintained library; last update May 2023. Camera API compatibility issues on newer Android
- Impact: Scan page fails silently on some phones; users cannot start sessions via QR
- Migration plan: Switch to `jsQR` (smaller, more recent) or native `BarcodeDetector` API (if available)

**Razorpay Checkout (Payment Integration):**
- Risk: External CDN dependency; script loading can be slow or fail in low-bandwidth areas
- Impact: Payment page unusable if https://checkout.razorpay.com is down
- Mitigation: Implement fallback to server-side payment gateway, or cache script locally
- Current mitigation: None; relies on external CDN

**Recharts (Telemetry Visualization):**
- Risk: Large bundle size (100KB); used only on telemetry page, not critical path
- Impact: First meaningful paint delayed on slow connection
- Migration plan: Lazy-load recharts with dynamic import (already done, line 9); consider smaller alternative like `lightweight-charts` if performance critical

---

## Missing Critical Features

**Offline Support:**
- Problem: All pages require network; no offline-first PWA caching
- Blocks: Users cannot view recent sessions, leaderboard, or stats on 2G network
- Solution: Implement Service Worker with Cache API. Cache GET responses for 1 hour. Queue offline mutations (bookings) and retry when online.

**Error Logging & Observability:**
- Problem: Silent failures; no visibility into what requests are failing or why
- Blocks: Cannot diagnose production issues without user reports
- Solution: Implement Sentry integration or custom error tracking. Log all failed requests with redacted payloads.

**Request Timeout Handling:**
- Problem: No timeout on fetch calls; requests can hang indefinitely
- Blocks: Users see frozen UI if network is slow; no way to retry
- Solution: Add `AbortController` with 30s timeout to all fetch calls. Show user "Network slow" message.

**Input Validation & XSS Prevention:**
- Problem: User inputs (phone, name, otp) not validated against server schema
- Blocks: Cannot catch client-side validation mismatches early
- Solution: Add Zod schema for all form inputs. Validate before sending to server.

---

## Test Coverage Gaps

**Book Wizard State Logic:**
- What's not tested: Step transitions, validation, error states, coupon application, multiplayer flow
- Files: `src/app/book/page.tsx` (900 lines, 0 tests)
- Risk: Refactoring breaks wizard without detection; regressions in production
- Priority: High

**API Error Handling:**
- What's not tested: JWT expiry, 500 errors, network timeouts, malformed responses
- Files: `src/lib/api.ts` (entire file)
- Risk: Silent failures; auth edge cases cause infinite redirects
- Priority: High

**Payment Flow:**
- What's not tested: Razorpay success/failure handlers, bonus calculation, wallet update
- Files: `src/app/wallet/topup/page.tsx` (425 lines, 0 tests)
- Risk: Users charged but credits not added; bonus incorrectly calculated
- Priority: Critical

**Form Validation:**
- What's not tested: Login phone formatting, registration fields, minor guardian logic
- Files: `src/app/login/page.tsx`, `src/app/register/page.tsx`
- Risk: Phone number double-prefix; minor without guardian info gets past validation
- Priority: High

**QR Scanner Cleanup:**
- What's not tested: Camera permission denial, scanner cleanup on unmount, double-scan handling
- Files: `src/app/scan/page.tsx` (lines 20-118)
- Risk: Leaked camera resource; scanner stuck in scanning state
- Priority: Medium

---

*Concerns audit: 2026-03-21*
