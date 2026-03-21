---
phase: 05-kiosk-pin-launch
verified: 2026-03-21T09:00:00+05:30
status: human_needed
score: 7/7 must-haves verified
re_verification: false
human_verification:
  - test: "Open kiosk at http://192.168.31.23:3300, tap 'Have a PIN?' in footer"
    expected: "Full-screen PIN entry overlay appears with 6 empty boxes and 31-character alphanumeric grid"
    why_human: "Visual layout and touch responsiveness cannot be verified programmatically"
  - test: "Enter 6 characters using the grid buttons, tap Submit with an invalid PIN"
    expected: "Error state shows 'Invalid PIN' and remaining attempts count (e.g., '9 attempts remaining')"
    why_human: "Requires live API call and UI state transition verification on device"
  - test: "Submit an invalid PIN 10 times consecutively"
    expected: "Lockout state appears with MM:SS countdown timer ticking down live"
    why_human: "Live countdown timer behavior requires human observation"
  - test: "Submit a valid confirmed reservation PIN"
    expected: "'Head to Pod X' with large pod number in Racing Red, experience name, tier, and 'Your game is loading...' animation; game launches on the assigned pod"
    why_human: "End-to-end flow requires a live confirmed reservation and pod agent connectivity"
---

# Phase 5: Kiosk PIN Launch Verification Report

**Phase Goal:** Customer enters PIN at venue kiosk and the game auto-launches on an assigned pod with zero staff interaction
**Verified:** 2026-03-21T09:00:00+05:30
**Status:** human_needed
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | POST /api/v1/kiosk/redeem-pin with a valid confirmed PIN returns pod_number, experience_name, allocated_seconds | VERIFIED | `redeem_pin()` in reservation.rs lines 482-666 returns all fields; route wired at routes.rs:59 |
| 2 | PIN is atomically marked redeemed — second request with same PIN gets error | VERIFIED | Atomic `UPDATE ... WHERE id = (SELECT ... AND status = 'confirmed') AND status = 'confirmed' RETURNING` at reservation.rs:514-527 |
| 3 | If no pods are idle, PIN is NOT consumed and error returned | VERIFIED | `pod_reservation::find_idle_pod()` called at line 503 BEFORE the atomic UPDATE; returns early on None |
| 4 | After 10 consecutive failures from same IP, endpoint returns lockout error with cooldown timer | VERIFIED | `PinLockoutState` + `PIN_LOCKOUT` LazyLock at routes.rs:4297-4303; lockout triggered at fail_count >= 10 (line 4380) |
| 5 | pending_debit PINs return a distinct 'booking being processed' message without consuming | VERIFIED | Separate query for `status = 'pending_debit'` at reservation.rs:490-499 returns early with distinct message |
| 6 | Customer sees 'Have a PIN?' button on kiosk landing page and can open full-screen PIN entry | VERIFIED | Button at page.tsx:326-331, PinRedeemScreen imported at line 7, overlay at line 358-360 |
| 7 | Success screen shows 'Head to Pod X' with pod number prominently and game loading status | VERIFIED | PinRedeemScreen.tsx:233-246 renders `text-8xl` pod number + "Your game is loading..." animate-pulse |

**Score:** 7/7 truths verified (automated); human confirmation needed for 4 visual/runtime truths

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/reservation.rs` | `redeem_pin()` function | VERIFIED | `pub async fn redeem_pin` at line 482, ~185 lines, full 16-step flow |
| `crates/racecontrol/src/api/routes.rs` | `kiosk_redeem_pin` handler + route registration | VERIFIED | Handler at line 4317, route registered in `auth_rate_limited_routes()` at line 59 |
| `kiosk/src/components/PinRedeemScreen.tsx` | Full-screen PIN entry component, 5 states, min 100 lines | VERIFIED | 332 lines, all 5 states: entry/validating/success/error/lockout |
| `kiosk/src/lib/api.ts` | `redeemPin()` API method | VERIFIED | `redeemPin` at line 396-411, calls POST `/kiosk/redeem-pin` |
| `kiosk/src/app/page.tsx` | 'Have a PIN?' button + PinRedeemScreen toggle | VERIFIED | Button at line 326-331, import at line 7, conditional render at lines 358-360 |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `routes.rs` | `reservation.rs` | `reservation::redeem_pin()` | WIRED | routes.rs:4359 calls `reservation::redeem_pin(&state, &req.pin)` |
| `reservation.rs` | `pod_reservation.rs` | `pod_reservation::find_idle_pod()` | WIRED | reservation.rs:503 calls `pod_reservation::find_idle_pod(state)` |
| `reservation.rs` | `billing.rs` | `billing::defer_billing_start()` | WIRED | reservation.rs:560 calls `billing::defer_billing_start(...)` |
| `reservation.rs` | `auth/mod.rs` | `auth::launch_or_assist()` | WIRED | reservation.rs:621 calls `auth::launch_or_assist(state, &pod_id, ...)` |
| `PinRedeemScreen.tsx` | `api.ts` | `api.redeemPin(pin)` | WIRED | PinRedeemScreen.tsx:53 calls `api.redeemPin(pin)` in handleSubmit |
| `page.tsx` | `PinRedeemScreen.tsx` | import + state toggle | WIRED | page.tsx:7 imports PinRedeemScreen, state at line 63, render at line 358-360 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| KIOSK-01 | 05-02 | Kiosk displays PIN entry screen for walk-in customers with remote bookings | SATISFIED | "Have a PIN?" button on landing page, PinRedeemScreen full-screen overlay |
| KIOSK-02 | 05-01 | PIN validated against local server's synced reservations | SATISFIED | SQL query against `reservations` table checking `status = 'confirmed'` and `expires_at > datetime('now')` |
| KIOSK-03 | 05-01 | Valid PIN triggers pod assignment (first available) and game launch | SATISFIED | `find_idle_pod()` assigns first idle pod, `launch_or_assist()` starts game |
| KIOSK-04 | 05-01 | Rate limiting: max 5 attempts per minute, lockout after 10 failures | SATISFIED | Tower-governor layer (5/min) in `auth_rate_limited_routes()`; in-handler `PinLockoutState` lockout after 10 failures |
| KIOSK-05 | 05-01 | PIN is one-time use — marked as redeemed immediately on successful validation | SATISFIED | Atomic UPDATE with RETURNING at reservation.rs:514-527; double-redeem impossible at DB level |
| KIOSK-06 | 05-02 | Customer sees assigned pod number and game loading status after PIN entry | SATISFIED | Success state renders pod number in `text-8xl text-[#E10600]` + "Your game is loading..." |

All 6 phase requirements (KIOSK-01 through KIOSK-06) are SATISFIED. No orphaned requirements.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| None found | — | — | — | — |

Scanned: `reservation.rs`, `routes.rs` (handler section), `PinRedeemScreen.tsx`, `api.ts`, `page.tsx`. No TODO/FIXME, no placeholder returns, no stub-only handlers, no empty implementations found.

---

### Human Verification Required

#### 1. PIN Entry Grid Visual and Touch

**Test:** Open kiosk at http://192.168.31.23:3300, tap "Have a PIN?" in the footer
**Expected:** Full-screen overlay (#1A1A1A background) with "Enter Your Booking PIN" title, 6 empty boxes at top, and 31-character alphanumeric grid (A-Z minus I/L/O, digits 2-9) in 7-column layout
**Why human:** Grid layout, button sizing (56px), and touch target responsiveness require physical kiosk screen verification

#### 2. Error State With Remaining Attempts

**Test:** Enter 6 characters and submit an invalid PIN (e.g. "AAAAAA")
**Expected:** Error state shows "Invalid PIN" with amber text "9 attempts remaining"; "Try Again" button resets to entry
**Why human:** Requires live API call against local racecontrol server

#### 3. Lockout Countdown Timer

**Test:** Submit an invalid PIN 10 times consecutively
**Expected:** After the 10th failure, lockout state appears with a live MM:SS countdown (5:00 counting down); timer auto-transitions back to entry when it reaches 0:00
**Why human:** Live countdown behavior requires observation across time

#### 4. End-to-End PIN Redemption With Real Booking

**Test:** Create a confirmed reservation via Phase 4 flow, note the PIN, enter it at the kiosk
**Expected:** "Head to Pod X" success screen with pod number, experience name, tier, duration; assigned pod's lock screen clears and game launches automatically; success screen auto-closes after 15 seconds
**Why human:** Requires live confirmed reservation, pod agent connectivity, and observing the pod screen — not automatable

---

### Overall Assessment

All 7 observable truths are verified at code level. The backend implementation is complete and correct:
- Atomic double-redeem prevention via SQL `UPDATE ... RETURNING` with `AND status = 'confirmed'`
- Pod availability checked before PIN consumption (no lost reservations on full venue)
- pending_debit guard returns distinct message without consuming reservation
- Two-layer rate limiting: tower-governor (5/min burst) + per-IP lockout after 10 failures

The frontend implementation is complete and substantive:
- PinRedeemScreen.tsx at 332 lines covers all 5 states with correct logic
- Character set exactly matches backend: `ABCDEFGHJKMNPQRSTUVWXYZ23456789` (31 chars)
- api.redeemPin() correctly targets `/kiosk/redeem-pin` (no auth header — public endpoint)
- "Have a PIN?" button is visible and wired

The 4 human verification items are runtime/visual confirmations, not gaps. The phase goal is code-complete.

---

_Verified: 2026-03-21T09:00:00+05:30_
_Verifier: Claude (gsd-verifier)_
