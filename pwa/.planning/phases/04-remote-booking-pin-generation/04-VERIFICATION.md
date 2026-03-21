---
phase: 04-remote-booking-pin-generation
verified: 2026-03-21T14:00:00+05:30
status: gaps_found
score: 9/11 must-haves verified
re_verification: false
gaps:
  - truth: "Customer can select experience + tier and complete a remote booking from PWA"
    status: failed
    reason: "book/page.tsx checks res.status === 'reserved' but backend returns status = 'pending_debit'. The PIN confirmation screen (CloudPinScreen) never renders; execution always falls into the error branch showing 'Reservation failed'."
    artifacts:
      - path: "pwa/src/app/book/page.tsx"
        issue: "Line 268: `if (res.status === 'reserved' && res.pin)` — backend returns 'pending_debit', not 'reserved'"
    missing:
      - "Change condition to `res.pin` (status-independent) OR to `res.status === 'pending_debit'` to match backend response"

  - truth: "Customer can modify their reservation (change experience/duration) from /reservations"
    status: failed
    reason: "reservations/page.tsx checks res.status === 'modified' but backend modify_reservation() returns status='pending_debit' and a separate 'modified: true' boolean field. The modify success branch is unreachable; modify always shows 'Modify failed'."
    artifacts:
      - path: "pwa/src/app/reservations/page.tsx"
        issue: "Line 102: `if (res.status === 'modified' && res.pin)` — backend returns status='pending_debit' with modified=true, not status='modified'"
    missing:
      - "Change condition to check `res.pin` (or `res.modified === true`) rather than `res.status === 'modified'`"
---

# Phase 4: Remote Booking + PIN Generation Verification Report

**Phase Goal:** Customer can book an experience from their phone at home and receive a PIN for venue redemption
**Verified:** 2026-03-21T14:00:00+05:30 (IST)
**Status:** gaps_found
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|---------|
| 1 | POST /customer/reservation/create creates a pod-agnostic reservation with 6-char PIN and debit_intent | VERIFIED | reservation.rs: create_reservation() inserts with no pod_number, generates pin from 31-char charset, INSERT INTO debit_intents origin='cloud' |
| 2 | GET /customer/reservation returns the active reservation for authenticated customer | VERIFIED | routes.rs line 157: GET /customer/reservation -> customer_get_reservation -> reservation::get_active_reservation |
| 3 | PUT /customer/reservation/modify modifies experience/duration (cancel+rebook) | VERIFIED | reservation.rs: modify_reservation() calls cancel_reservation() then creates new with original expires_at |
| 4 | DELETE /customer/reservation cancels reservation and creates refund debit_intent | VERIFIED | reservation.rs: cancel_reservation() checks debit status, creates negative amount_paise intent on 'completed' |
| 5 | PIN is delivered via WhatsApp after booking (fire-and-forget) | VERIFIED | reservation.rs line 170: tokio::spawn fires send_pin_whatsapp without blocking |
| 6 | Only one active reservation per customer enforced | VERIFIED | reservation.rs line 65: SELECT WHERE driver_id AND status IN ('pending_debit','confirmed') — returns error if found |
| 7 | Reservations with status 'confirmed'/'pending_debit' past expires_at are auto-expired | VERIFIED | scheduler.rs line 186: expire_reservations() UPDATE status='expired' WHERE expires_at < datetime('now') |
| 8 | Expired reservations with completed debit get wallet refund via negative debit_intent | VERIFIED | scheduler.rs line 217-230: origin='local', amount_paise=-amount on 'completed' intent |
| 9 | Customer can view their active reservation at /reservations | VERIFIED | reservations/page.tsx: useEffect calls api.getReservation(), renders PIN card, status badge, experience/price/expiry |
| 10 | Customer can select experience + tier and complete a remote booking from PWA (PIN shown) | FAILED | book/page.tsx line 268 checks `res.status === "reserved"` but backend returns `"pending_debit"` — CloudPinScreen never renders |
| 11 | Customer can modify their reservation (change experience/duration) from /reservations | FAILED | reservations/page.tsx line 102 checks `res.status === "modified"` but backend returns `status="pending_debit"` with `modified=true` — modify success branch unreachable |

**Score:** 9/11 truths verified

---

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/racecontrol/src/reservation.rs` | PIN generation, reservation CRUD, WhatsApp delivery | VERIFIED | 544 lines; generate_unique_pin, create_reservation, get_active_reservation, cancel_reservation, modify_reservation, send_pin_whatsapp all present and substantive |
| `crates/racecontrol/src/api/routes.rs` | 4 customer reservation routes | VERIFIED | Lines 157-159: GET+DELETE at /customer/reservation, POST at /customer/reservation/create, PUT at /customer/reservation/modify |
| `crates/racecontrol/src/lib.rs` | `pub mod reservation;` | VERIFIED | Line 35: `pub mod reservation;` confirmed |
| `crates/racecontrol/src/scheduler.rs` | expire_reservations() function called from tick() | VERIFIED | Lines 173-176: `expire_reservations(state).await` called from tick(); function defined at line 186 |
| `pwa/src/lib/api.ts` | createReservation, getReservation, cancelReservation, modifyReservation API methods | VERIFIED | Lines 998-1035: all 4 methods present, hitting correct /customer/reservation paths |
| `pwa/src/app/book/page.tsx` | Remote booking flow calling api.createReservation() | PARTIAL | IS_CLOUD detection and api.createReservation() call exist (line 264) but success condition `res.status === "reserved"` never matches backend response |
| `pwa/src/app/reservations/page.tsx` | Reservation management page with view/cancel/modify | PARTIAL | Page exists with full UI; cancel flow works correctly; modify success condition `res.status === "modified"` never matches backend response |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `routes.rs` | `reservation.rs` | `reservation::` function calls | VERIFIED | Lines 4851-4900: all 4 handlers call reservation:: module functions with driver_id extracted |
| `reservation.rs` | debit_intents table | INSERT INTO debit_intents | VERIFIED | Lines 134-144, 307-316, 422-433: debit_intents created with origin='cloud' for creates/modifies |
| `reservation.rs` | WhatsApp Evolution API | reqwest POST /message/sendText | VERIFIED | Line 512: `format!("{}/message/sendText/{}", evo_url, evo_instance)` — follows exact auth/mod.rs pattern |
| `scheduler.rs` | reservations table | UPDATE status='expired' WHERE expires_at | VERIFIED | Line 198-202: UPDATE reservations SET status='expired' |
| `scheduler.rs` | debit_intents table | INSERT refund intent (negative amount) | VERIFIED | Lines 220-229: INSERT origin='local', amount_paise=-amount_paise |
| `book/page.tsx` | `api.ts` | api.createReservation() | PARTIAL | Call exists at line 264 but response status check is wrong — PIN confirmation unreachable |
| `reservations/page.tsx` | `api.ts` | api.getReservation(), api.cancelReservation(), api.modifyReservation() | PARTIAL | All 3 calls present; getReservation + cancelReservation work; modifyReservation success check is wrong |
| `api.ts` | /customer/reservation | fetchApi HTTP calls | VERIFIED | Lines 1008, 1017, 1021, 1032: all 4 methods hit /customer/reservation with correct HTTP verbs |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|---------|
| BOOK-01 | 04-01, 04-03 | Customer can book from PWA (select game, car/track, duration tier) | PARTIAL | Booking wizard and API call exist; PIN never shown due to status mismatch bug |
| BOOK-02 | 04-01 | Booking creates pod-agnostic reservation (no pod assigned) | VERIFIED | reservation.rs INSERT has no pod_number; status='pending_debit' |
| BOOK-03 | 04-01 | 6-char alphanumeric PIN generated on booking, displayed to customer | PARTIAL | PIN generated correctly (PIN_CHARSET line 16), displayed in CloudPinScreen — but CloudPinScreen unreachable due to status bug |
| BOOK-04 | 04-01 | PIN delivered via WhatsApp | VERIFIED | tokio::spawn + send_pin_whatsapp fires after create_reservation() regardless of UI bug |
| BOOK-05 | 04-03 | Customer can view, cancel, or modify reservation from PWA | PARTIAL | View: works. Cancel: works. Modify: success branch unreachable (status check bug) |
| BOOK-06 | 04-02 | Reservations expire after 24h TTL | VERIFIED | INSERT expires_at = datetime('now', '+24 hours'); scheduler checks expires_at < datetime('now') |
| BOOK-07 | 04-02 | Expired reservations auto-cleaned up with wallet refund if debited | VERIFIED | expire_reservations() handles both pending (cancel) and completed (negative refund intent) |
| API-04 | 04-01 | New reservation endpoints: create, cancel, modify, redeem (PIN validation) | PARTIAL | Create, cancel, modify endpoints verified; redeem/PIN validation is Phase 5 scope (kiosk) |

**Note on API-04:** The plan text includes "redeem (PIN validation)" but this was acknowledged as Phase 5 scope. The 3 implemented endpoints satisfy the Phase 4 intent.

---

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|---------|--------|
| `pwa/src/app/book/page.tsx` | 268 | `res.status === "reserved"` — status value not matching backend | Blocker | CloudPinScreen never renders; customer sees "Reservation failed" after successful booking |
| `pwa/src/app/reservations/page.tsx` | 102 | `res.status === "modified"` — status value not matching backend | Blocker | Modify success branch unreachable; customer sees "Modify failed" after successful modification |
| `pwa/src/app/book/page.tsx` | 264-265 | `api.createReservation(custom.track, tier.id)` — passes track ID as experience_id | Warning | Track ID is unlikely to match a kiosk_experiences.id — booking will likely fail with "Invalid experience" on the backend |

---

### Human Verification Required

#### 1. CloudPinScreen Visual Appearance (once status bug is fixed)

**Test:** Set NEXT_PUBLIC_IS_CLOUD=true, complete booking wizard, submit booking
**Expected:** PIN displayed in large Racing Red letters with copy button, experience name, price, and expiry time
**Why human:** Visual layout and styling cannot be verified programmatically

#### 2. Cancel flow end-to-end

**Test:** Log in with a customer that has an active reservation, visit /reservations, click Cancel, confirm
**Expected:** Confirmation dialog shows, cancellation completes, success message shows with refund credits
**Why human:** Requires real backend + auth session; dialog interaction is visual

#### 3. WhatsApp PIN delivery

**Test:** Create a reservation for a driver with a valid phone number
**Expected:** WhatsApp message received with format "Your Racing Point PIN: *XXXXXX*..."
**Why human:** Requires Evolution API credentials and a real WhatsApp number

---

### Gaps Summary

Two status-check mismatches break the end-to-end cloud booking flow:

**Gap 1 (Blocker) — book/page.tsx status check:** `create_reservation()` on the backend returns `{ "status": "pending_debit", "pin": "ABC123", ... }`. The PWA at line 268 checks `res.status === "reserved"`, which never matches. The entire cloud booking success path — the CloudPinScreen component with PIN display — is unreachable. Every cloud booking attempt falls into the error handler showing "Reservation failed" even when the backend successfully created the reservation and sent the WhatsApp message.

**Gap 2 (Blocker) — reservations/page.tsx modify check:** `modify_reservation()` returns `{ "status": "pending_debit", "modified": true, "pin": "XYZ789", ... }`. The page at line 102 checks `res.status === "modified"`, which never matches. Modification attempts always show "Modify failed" even when the operation succeeded.

**Additional concern:** In book/page.tsx line 264, `api.createReservation(custom.track, tier.id)` passes the track ID as the `experience_id`. The backend queries `kiosk_experiences WHERE id = ?` with this value. Track IDs (e.g., "ks_nurburgring") are very unlikely to match experience IDs (which are UUIDs), so cloud bookings from the custom booking wizard will also fail with "Invalid experience" at the backend level. The simple/preset booking path may behave differently; this warrants testing.

Both status bugs are trivially fixed (one-line changes). The experience_id mapping issue in cloud mode may require a design decision (map track to experience server-side, or require experience selection in cloud mode).

---

_Verified: 2026-03-21T14:00:00+05:30 (IST)_
_Verifier: Claude (gsd-verifier)_
