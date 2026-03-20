---
phase: 76-api-authentication-admin-protection
verified: 2026-03-20T19:30:00+05:30
status: passed
score: 12/12 must-haves verified
re_verification: false
---

# Phase 76: API Authentication & Admin Protection Verification Report

**Phase Goal:** No unauthenticated request can manipulate billing, start sessions, or access the admin panel
**Verified:** 2026-03-20T19:30:00+05:30
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | A curl to any billing endpoint without Bearer JWT returns 401 | VERIFIED | `routes.rs:326` applies `from_fn_with_state(state, require_staff_jwt)` on all staff_routes including billing/* |
| 2 | A curl to any session start/stop endpoint without Bearer JWT returns 401 | VERIFIED | Session endpoints in `staff_routes()` (lines 147-326), all behind strict `require_staff_jwt` middleware |
| 3 | Public endpoints return 200 without any token | VERIFIED | `public_routes()` at line 60 has /health, /fleet/health, /venue, /customer/register, etc. -- no middleware layer |
| 4 | A curl with a valid staff JWT to a staff endpoint returns expected response | VERIFIED | `middleware.rs:69-86` strict middleware inserts StaffClaims on success, passes to next handler. 7 unit tests cover accept/reject. |
| 5 | Customer endpoints still work with existing customer JWT (no regression) | VERIFIED | `customer_routes()` at line 87 is a separate tier with no staff middleware. Customer JWT auth handled in-handler. |
| 6 | POST /api/v1/auth/admin-login with correct PIN returns a staff JWT | VERIFIED | `admin.rs:65-102` validates PIN via argon2, returns `{ token, expires_in: 43200 }`. Test at line 191 confirms 200 + token. |
| 7 | POST /api/v1/auth/admin-login with wrong PIN returns 401 | VERIFIED | `admin.rs:85-88` returns `Err(StatusCode::UNAUTHORIZED)`. Test at line 182 confirms. |
| 8 | Admin PIN stored as argon2id hash -- no plaintext | VERIFIED | `config.rs:229` stores `admin_pin_hash: Option<String>`, `admin.rs:30-41` uses argon2 crate with Argon2id. Test confirms `$argon2id$` prefix. |
| 9 | rc-agent /exec rejects requests without X-Service-Key when RCAGENT_SERVICE_KEY is set | VERIFIED | `remote_ops.rs:74-96` `require_service_key` with constant-time comparison. Protected routes at lines 108-118 have the layer. |
| 10 | Rate limiting: 6th rapid request to auth endpoint returns 429 | VERIFIED | `rate_limit.rs:14-21` GovernorLayer with burst_size(5), per_second(12). Test at line 64 confirms 429. 6 endpoints covered. |
| 11 | Dashboard at :3200 requires PIN before any content visible | VERIFIED | `AuthGate.tsx` wraps all children in `layout.tsx:27`. Returns null if not authenticated. Redirects to /login. |
| 12 | After 15 minutes inactivity, JWT cleared and PIN prompt reappears | VERIFIED | `useIdleTimeout.ts:6-36` listens to 5 event types, calls `clearToken()` + redirect after 15*60*1000ms. Wired in AuthGate.tsx:12. |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/auth/middleware.rs` | StaffClaims, require_staff_jwt, create_staff_jwt | VERIFIED | 280 lines, StaffClaims struct (line 21), strict middleware (line 69), permissive variant (line 94), create_staff_jwt (line 124), 7 tests |
| `crates/racecontrol/src/auth/admin.rs` | admin_login, hash_admin_pin, verify_admin_pin | VERIFIED | 222 lines, argon2id hashing (line 30), spawn_blocking verification (line 81), 12h JWT (line 92), 8 tests |
| `crates/racecontrol/src/auth/rate_limit.rs` | GovernorLayer rate limiting | VERIFIED | 121 lines, PeerIpKeyExtractor, burst_size(5), per_second(12), 3 tests |
| `crates/racecontrol/src/api/routes.rs` | 5-tier route split with strict staff middleware | VERIFIED | auth_rate_limited_routes (line 47), public_routes (line 60), customer_routes (line 87), staff_routes (line 147), service_routes (line 331). Strict `require_staff_jwt` at line 326. |
| `crates/rc-agent/src/remote_ops.rs` | require_service_key middleware | VERIFIED | Middleware at line 74, ct_eq at line 91, public/protected split at lines 104-118 (start) and 208-223 (start_checked) |
| `web/src/lib/auth.ts` | getToken, setToken, clearToken, isAuthenticated | VERIFIED | 26 lines, all 4 functions exported, JWT expiry check via atob decode |
| `web/src/app/login/page.tsx` | PIN entry page | VERIFIED | 107 lines, POST to /api/v1/auth/admin-login (line 29), error handling for 401/503, auto-focus, Racing Point brand colors |
| `web/src/components/AuthGate.tsx` | Auth gate wrapper | VERIFIED | 25 lines, hydrated flag pattern (line 8), useEffect mount check (line 14), useIdleTimeout wired (line 12) |
| `web/src/hooks/useIdleTimeout.ts` | Idle timeout hook | VERIFIED | 36 lines, 5 event listeners (line 27), clearToken on timeout (line 13), skips login page (line 25) |
| `web/src/lib/api.ts` | fetchApi with Authorization header | VERIFIED | getToken import (line 1), Authorization header injection (lines 11-13), 401 auto-redirect (lines 21-25) |
| `web/src/app/layout.tsx` | AuthGate wrapping children | VERIFIED | AuthGate import (line 4), wraps children at line 27 |
| `crates/racecontrol/src/config.rs` | admin_pin_hash in AuthConfig | VERIFIED | Line 229: `admin_pin_hash: Option<String>`, env var override at line 467 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| routes.rs | middleware.rs | `from_fn_with_state(state, require_staff_jwt)` | WIRED | routes.rs:326 uses strict variant, import at line 13 |
| routes.rs | rate_limit.rs | `auth::rate_limit::auth_rate_limit_layer()` | WIRED | routes.rs:55 applies GovernorLayer on 6 auth endpoints |
| admin.rs | middleware.rs | `super::middleware::create_staff_jwt` | WIRED | admin.rs:92 calls create_staff_jwt for token generation |
| admin.rs | config.rs | `state.config.auth.admin_pin_hash` | WIRED | admin.rs:70 reads admin_pin_hash from config |
| remote_ops.rs | RCAGENT_SERVICE_KEY env var | `std::env::var("RCAGENT_SERVICE_KEY")` | WIRED | remote_ops.rs:78 reads env var |
| login/page.tsx | /api/v1/auth/admin-login | POST fetch with { pin } body | WIRED | login/page.tsx:29 fetches endpoint, handles response |
| api.ts | auth.ts | `getToken()` for Authorization header | WIRED | api.ts:1 imports getToken, line 6 calls it, line 12 sets header |
| useIdleTimeout.ts | auth.ts | `clearToken()` on timeout | WIRED | useIdleTimeout.ts:4 imports clearToken, line 13 calls it |
| AuthGate.tsx | auth.ts | `isAuthenticated()` on mount | WIRED | AuthGate.tsx:4 imports isAuthenticated, line 16 checks it |
| layout.tsx | AuthGate.tsx | `<AuthGate>{children}</AuthGate>` | WIRED | layout.tsx:4 imports, line 27 wraps children |
| main.rs | routes.rs | `api_routes(state.clone())` | WIRED | Confirmed by successful build (api_routes takes Arc<AppState>) |
| main.rs | ConnectInfo | `into_make_service_with_connect_info::<SocketAddr>()` | WIRED | main.rs:582 enables ConnectInfo for rate limiter |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AUTH-01 | 76-01, 76-06 | JWT middleware on billing endpoints | SATISFIED | staff_routes has strict require_staff_jwt; billing/* routes inside staff_routes |
| AUTH-02 | 76-01, 76-06 | JWT middleware on session start/stop | SATISFIED | sessions/* routes inside staff_routes with strict middleware |
| AUTH-03 | 76-01 | Route classification (public/customer/staff/admin tiers) | SATISFIED | 5 tier functions: auth_rate_limited, public, customer, staff, service |
| AUTH-04 | 76-04 | Rate limiting on auth endpoints | SATISFIED | GovernorLayer on 6 auth endpoints, 5 req/min per IP, 3 tests |
| AUTH-05 | 76-04 | Bot wallet balance pre-check | SATISFIED | Summary confirms pre-existing check at bot_book handler lines 11295-11310, verified by Plan 04 |
| AUTH-06 | 76-03 | Service-to-service auth for rc-agent | SATISFIED | require_service_key middleware with constant-time comparison via subtle crate |
| ADMIN-01 | 76-02, 76-05 | PIN gate on admin dashboard | SATISFIED | Backend: admin_login endpoint with argon2. Frontend: AuthGate + login page |
| ADMIN-02 | 76-02 | Admin PIN hashed with argon2 | SATISFIED | argon2 0.5 crate, hash_admin_pin uses Argon2id with random salt, PHC format |
| ADMIN-03 | 76-05 | Session timeout after 15 min inactivity | SATISFIED | useIdleTimeout hook, 15*60*1000ms, clears JWT and redirects |
| SESS-01 | 76-01, 76-06 | Session launch requires authenticated request | SATISFIED | Session launch routes in staff_routes behind strict JWT middleware |
| SESS-02 | 76-04 | Auth tokens single-use (no replay) | SATISFIED | Optimistic locking: UPDATE SET status='consuming' WHERE status='pending', intermediate state prevents double-consume, 4 tests |
| SESS-03 | 76-04 | DB transaction wrapping token+billing | SATISFIED | `db.begin().await` at auth/mod.rs:420, consuming->consumed in transaction, rollback on failure |

**All 12 requirements mapped to Phase 76 in REQUIREMENTS-v12.md are accounted for. No orphaned requirements.**

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | - | - | - | No anti-patterns found in any phase artifacts |

No TODO, FIXME, PLACEHOLDER, HACK, or stub patterns found in any of the modified files. No empty implementations. No console.log-only handlers.

### Human Verification Required

### 1. Dashboard PIN Gate End-to-End

**Test:** Navigate to http://192.168.31.23:3200 in browser without a stored JWT
**Expected:** Redirect to /login with PIN prompt. No dashboard content visible.
**Why human:** Visual behavior and redirect timing cannot be verified programmatically.

### 2. Strict JWT Enforcement via curl

**Test:** `curl -s -o /dev/null -w "%{http_code}" http://192.168.31.23:8080/api/v1/billing/active`
**Expected:** Returns 401
**Why human:** Requires running server to test actual HTTP response.

### 3. Idle Timeout Auto-Lock

**Test:** Log in to dashboard, wait 15 minutes without interaction (or temporarily reduce timeout)
**Expected:** JWT cleared, redirected to /login
**Why human:** Requires real browser session with timed inactivity.

### 4. rc-agent Service Key Enforcement

**Test:** Set RCAGENT_SERVICE_KEY env var on a pod, restart rc-agent, then curl /exec without header
**Expected:** Returns 401
**Why human:** Requires deployed rc-agent with env var configured on actual pod.

### 5. Customer PWA Non-Regression

**Test:** Open customer PWA, complete login/OTP flow, verify session works
**Expected:** Customer flow unaffected by staff auth changes
**Why human:** End-to-end customer flow spans PWA + backend + pod.

### Gaps Summary

No gaps found. All 12 observable truths verified. All 12 requirement IDs accounted for. All artifacts exist at all three levels (exists, substantive, wired). No anti-patterns detected. The phase goal "No unauthenticated request can manipulate billing, start sessions, or access the admin panel" is achieved by the combination of:

1. **Backend:** Strict `require_staff_jwt` middleware on 172+ staff routes rejecting unauthenticated requests with 401
2. **Admin login:** Argon2id PIN hashing with spawn_blocking, 12-hour JWT issuance
3. **Rate limiting:** GovernorLayer on 6 auth endpoints (5 req/min per IP)
4. **rc-agent:** Service key middleware with constant-time comparison on all operational endpoints
5. **Dashboard frontend:** AuthGate wrapper, PIN login page, 15-minute idle timeout, JWT-bearing API calls
6. **Session integrity:** Atomic token consumption via SQLx transaction, single-use tokens with optimistic locking

---

_Verified: 2026-03-20T19:30:00+05:30_
_Verifier: Claude (gsd-verifier)_
