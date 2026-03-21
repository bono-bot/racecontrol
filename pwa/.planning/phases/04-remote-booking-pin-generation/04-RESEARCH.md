# Phase 4: Remote Booking + PIN Generation - Research

**Researched:** 2026-03-21
**Domain:** Cloud booking API (Rust/Axum) + PWA booking UI (Next.js) + WhatsApp integration
**Confidence:** HIGH

## Summary

Phase 4 builds the remote booking flow: a customer on the cloud PWA selects an experience, pays via wallet (debit_intent pattern from Phase 3), receives a 6-character PIN displayed on screen and via WhatsApp, and can manage (view/cancel/modify) the reservation before arriving at the venue. The backend creates cloud-authoritative reservations in the `reservations` table (already created in Phase 3) and a background scheduler task handles expiry cleanup with automatic wallet refunds.

The existing codebase provides strong foundations: the `reservations` and `debit_intents` tables are already migrated (Phase 3), the `cloud_sync.rs` already syncs both tables bidirectionally, the WhatsApp OTP delivery via Evolution API is a proven pattern in `auth/mod.rs`, and the PWA booking wizard at `/book` already handles experience selection, pricing tier selection, and wallet debit. The key new work is: (1) new cloud-side reservation API endpoints, (2) PIN generation, (3) WhatsApp PIN delivery, (4) scheduler-based expiry cleanup, (5) new PWA pages for booking confirmation and reservation management.

**Primary recommendation:** Split into 3 plans: (1) Backend API endpoints (create/cancel/modify reservation + PIN generation + WhatsApp delivery), (2) Scheduler expiry cleanup + refund logic, (3) PWA booking flow + reservation management pages.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Customer selects experience + car/track + duration tier on PWA booking page
- Cloud API creates reservation in `reservations` table (cloud-authoritative, synced in Phase 3)
- Pod is NOT assigned at booking time -- reservation is pod-agnostic
- PIN: 6-character alphanumeric (A-Z0-9, excluding ambiguous chars O/0/I/1/L), generated server-side
- Reservation status state machine: pending_debit -> confirmed | expired | cancelled (redeemed added at Phase 5)
- One active reservation per customer enforced at API level
- Booking debits wallet via debit_intent pattern (Phase 3 infrastructure)
- Cloud creates debit_intent, local processes it, balance syncs back
- Cancellation creates a credit/refund debit_intent (negative amount or separate refund mechanism)
- Expired reservation cleanup refunds wallet automatically
- Use existing WhatsApp integration (OTP sending pattern) to deliver PIN after booking
- Message template: "Your Racing Point PIN: {PIN}. Valid for 24 hours. Show this at the kiosk when you arrive."
- If WhatsApp delivery fails, PIN is still displayed in PWA (WhatsApp is convenience, not critical path)
- Default TTL: 24 hours from booking time
- Background scheduler task runs every 5 minutes to mark expired reservations
- Expired reservations with wallet debit get automatic refund via debit_intent
- New `/book` flow for remote booking (extending existing booking page)
- Booking confirmation page shows PIN prominently
- `/reservations` page to view/cancel/modify active reservation
- Modification: can change experience/duration but not extend beyond original TTL

### Claude's Discretion
- Exact PIN generation algorithm (cryptographically secure random)
- Background scheduler implementation (tokio::spawn interval vs cron-like)
- PWA booking UI component layout and styling
- Error handling UX for failed bookings
- WhatsApp message template exact wording

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BOOK-01 | Customer can book an experience from PWA at home (select game, car/track, duration tier) | Existing `/book` page wizard handles selection; extend with remote booking path that skips pod assignment |
| BOOK-02 | Booking creates a pod-agnostic reservation (no specific pod assigned at booking time) | `reservations` table already has schema with nullable `pod_number`; new API creates entry without pod |
| BOOK-03 | 6-character alphanumeric PIN generated on booking, displayed to customer | Use `rand::Rng` with charset `A-Z2-9` (excluding O,0,I,1,L); `reservations.pin` column exists |
| BOOK-04 | PIN delivered to customer via WhatsApp message | Evolution API pattern from `auth/mod.rs` lines 1060-1093; reuse same HTTP client pattern |
| BOOK-05 | Customer can view, cancel, or modify their reservation from PWA | New `/reservations` page + API endpoints: GET/PUT/DELETE on reservations |
| BOOK-06 | Reservations expire after configurable TTL (default: 24 hours) | `reservations.expires_at` column exists; scheduler tick checks `expires_at < datetime('now')` |
| BOOK-07 | Expired reservations auto-cleaned up with wallet refund if debited | Scheduler marks expired, creates refund debit_intent; `wallet::refund()` exists in wallet.rs |
| API-04 | New reservation endpoints: create, cancel, modify, redeem (PIN validation) | New routes in `customer_routes()` and potentially `kiosk_routes()` for PIN redemption (Phase 5) |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum | (existing) | HTTP API framework | Already in use, all routes in routes.rs |
| sqlx | (existing) | SQLite async queries | Already in use for all DB operations |
| rand | (existing) | Secure random PIN generation | Already a dependency; `rand::thread_rng()` is CSPRNG |
| uuid | (existing) | Reservation ID generation | Already used for all entity IDs |
| chrono | (existing) | Timestamp handling, TTL calculation | Already used in scheduler.rs and cloud_sync.rs |
| reqwest | (existing) | WhatsApp Evolution API HTTP calls | Already used for OTP delivery |
| Next.js | (existing) | PWA frontend framework | Already in use for all pages |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | (existing) | Background scheduler spawn | For expiry cleanup interval task |
| serde/serde_json | (existing) | Request/response serialization | All API payloads |

No new dependencies required. Everything needed is already in Cargo.toml and package.json.

## Architecture Patterns

### Backend: New API Endpoints

The cloud racecontrol instance needs new endpoints. These run on the CLOUD server (not local), so they create reservations and debit_intents directly in the cloud DB. The sync layer (Phase 3) handles propagation to local.

```
New routes (added to customer_routes()):
  POST   /customer/reservation          -- create booking + PIN
  GET    /customer/reservation          -- get active reservation
  PUT    /customer/reservation          -- modify reservation
  DELETE /customer/reservation          -- cancel reservation

Note: PIN redemption (POST /reservation/redeem) is Phase 5 (Kiosk).
```

### Pattern 1: Cloud Reservation Creation Flow
**What:** Customer books -> cloud creates reservation + debit_intent -> local processes debit -> sync updates status
**When to use:** Every remote booking

The flow:
1. Customer selects experience + pricing tier on PWA
2. Cloud API validates: one active reservation per customer, sufficient wallet balance (check synced wallet)
3. Cloud generates 6-char PIN, creates `reservations` row with status `pending_debit`
4. Cloud creates `debit_intents` row with amount = pricing tier price
5. Cloud returns PIN to PWA immediately (optimistic -- debit pending)
6. Local server picks up debit_intent in next sync cycle, processes wallet debit
7. debit_intent status updates to `completed` or `failed`
8. If debit fails, reservation status -> `failed` (handled by sync cycle)
9. WhatsApp PIN delivery fires async (non-blocking)

### Pattern 2: PIN Generation
**What:** Cryptographically random 6-character alphanumeric
**Charset:** `A B C D E F G H J K M N P Q R S T U V W X Y Z 2 3 4 5 6 7 8 9` (31 chars, excluding O/0/I/1/L)
**Collision handling:** Check uniqueness against active PINs before insert

```rust
// Source: established pattern, rand crate
use rand::Rng;

const PIN_CHARSET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

fn generate_pin() -> String {
    let mut rng = rand::thread_rng();
    (0..6).map(|_| {
        let idx = rng.gen_range(0..PIN_CHARSET.len());
        PIN_CHARSET[idx] as char
    }).collect()
}
```

31^6 = ~887 million possible PINs. Collision probability negligible with max ~100 active reservations.

### Pattern 3: WhatsApp PIN Delivery
**What:** Send PIN via Evolution API (same as OTP delivery)
**Source:** `auth/mod.rs` lines 1060-1093

```rust
// Reuse exact pattern from auth::send_otp
if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
    &state.config.auth.evolution_url,
    &state.config.auth.evolution_api_key,
    &state.config.auth.evolution_instance,
) {
    let wa_phone = format_wa_phone(&customer_phone);
    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = json!({
        "number": wa_phone,
        "text": format!(
            "Your Racing Point PIN: *{}*\n\nValid for 24 hours.\nShow this at the kiosk when you arrive.\n\nRacing Point eSports & Cafe",
            pin
        )
    });
    // Fire and forget -- PIN is still shown in PWA
    let client = reqwest::Client::new();
    let _ = client.post(&url).header("apikey", evo_key).json(&body).send().await;
}
```

### Pattern 4: Scheduler Expiry Cleanup
**What:** Background task running every 5 minutes to expire + refund reservations
**Source:** Existing `scheduler.rs` pattern (60s tick, task-based)

Two options:
1. **Add to existing scheduler.rs tick** -- simplest, runs every 60s (already exists), add expiry check
2. **Separate tokio::spawn interval** -- independent 5-min interval

**Recommendation:** Add to existing `scheduler.rs::tick()` function. It already runs every 60 seconds. Add a reservation expiry check that:
- Finds reservations where `status = 'confirmed' AND expires_at < datetime('now')`
- Updates status to `expired`
- Creates refund debit_intent for each expired reservation that had a completed debit

### Pattern 5: PWA Booking Flow Modification
**What:** Existing `/book` page assigns a pod immediately. Remote booking skips pod assignment.
**Approach:** Detect if running on cloud (check API URL or add feature flag). On cloud:
- After experience/tier selection, call new `POST /customer/reservation` instead of existing `/customer/book`
- Show confirmation page with PIN prominently displayed
- Add new `/reservations` page for managing active reservation

### Recommended Project Structure (new files)
```
crates/racecontrol/src/
    reservation.rs              # New module: PIN generation, reservation CRUD, expiry logic
    api/routes.rs               # Add new customer_reservation_* handlers
    scheduler.rs                # Add expire_reservations() call to tick()

pwa/src/
    app/reservations/page.tsx   # New: view/cancel/modify active reservation
    app/book/confirmation.tsx   # New: PIN display + WhatsApp delivery status (or inline in book/page.tsx)
    lib/api.ts                  # Add reservation API methods
```

### Anti-Patterns to Avoid
- **Debiting wallet directly on cloud:** Cloud NEVER modifies wallet directly. Always use debit_intent pattern. Local is the single writer for wallets.
- **Assigning pod at booking time:** Remote bookings are pod-agnostic. Pod assignment happens at kiosk (Phase 5).
- **Blocking on WhatsApp delivery:** WhatsApp is fire-and-forget. PIN must be shown in PWA regardless of WhatsApp success.
- **Using `pod_reservations` table for remote bookings:** The `pod_reservations` table is for in-venue pod assignments. Use the `reservations` table (created in Phase 3) for remote bookings.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Random PIN generation | Custom random algorithm | `rand::thread_rng()` + charset filter | thread_rng() is CSPRNG on all platforms |
| WhatsApp messaging | Custom HTTP client setup | Copy Evolution API pattern from auth/mod.rs | Proven pattern, handles phone format, error logging |
| Wallet operations | Direct SQL balance updates | `wallet::debit()` / `wallet::refund()` via debit_intents | Atomic transactions, double-entry accounting, sync-safe |
| Background scheduling | Custom timer implementation | Add to existing scheduler.rs tick() | Already running, battle-tested, has settings/logging |
| UUID generation | Custom ID schemes | `uuid::Uuid::new_v4()` | Already used everywhere in codebase |

**Key insight:** The existing codebase has all the building blocks. This phase is primarily wiring them together with new API endpoints and a new PWA page, not building new infrastructure.

## Common Pitfalls

### Pitfall 1: Double-Debit on Retry
**What goes wrong:** Customer retries booking, two debit_intents created, wallet debited twice
**Why it happens:** Network timeout on first request, customer clicks again
**How to avoid:** Enforce one active reservation per customer at API level (check `reservations WHERE driver_id = ? AND status IN ('pending_debit', 'confirmed')`). Return existing reservation if already exists.
**Warning signs:** Multiple `pending_debit` rows for same driver

### Pitfall 2: Stale Wallet Balance on Cloud
**What goes wrong:** Cloud shows sufficient balance but local wallet has been debited by another transaction
**Why it happens:** Sync lag between cloud and local (up to 30s)
**How to avoid:** Cloud does optimistic balance check, but debit_intent on local is authoritative. If local debit fails (insufficient), debit_intent status -> `failed`, reservation -> `failed`. PWA should poll reservation status.
**Warning signs:** Reservation stuck in `pending_debit` for > 60s

### Pitfall 3: Orphaned Debit Intents
**What goes wrong:** Reservation cancelled but debit_intent still pending or already completed
**Why it happens:** Race condition between cancellation and debit processing
**How to avoid:** Cancellation must: (1) check debit_intent status, (2) if pending -> cancel intent, (3) if completed -> create refund intent. Use a transaction or careful status checks.
**Warning signs:** Cancelled reservation with completed debit_intent and no refund

### Pitfall 4: PIN Collision
**What goes wrong:** Two active reservations get same PIN
**Why it happens:** Unlikely (31^6 space) but possible under high load
**How to avoid:** After generating PIN, check `SELECT COUNT(*) FROM reservations WHERE pin = ? AND status IN ('pending_debit', 'confirmed')`. Retry up to 3 times.
**Warning signs:** PIN lookup returns multiple rows

### Pitfall 5: Two Different Reservation Systems
**What goes wrong:** Confusion between `pod_reservations` (in-venue, pod-assigned) and `reservations` (remote, pod-agnostic)
**Why it happens:** Both tables exist, different purposes
**How to avoid:** Clear naming in code. `pod_reservations` is for the existing in-venue flow (customer_book_session). `reservations` is for remote booking (this phase). They serve different purposes and should not be mixed.
**Warning signs:** Querying wrong table for wrong operation

### Pitfall 6: Expiry Refund Creates Sync Loop
**What goes wrong:** Expiry on local creates refund debit_intent, syncs to cloud, cloud re-processes
**Why it happens:** Origin tag not set correctly on refund intent
**How to avoid:** Set `origin = 'local'` on locally-created refund intents. Cloud sync filter skips rows originating from cloud.
**Warning signs:** Duplicate refund transactions

## Code Examples

### Reservation Creation (Backend)
```rust
// New handler: POST /customer/reservation
async fn customer_create_reservation(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<CreateReservationRequest>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Check one-active-reservation constraint
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pin FROM reservations
         WHERE driver_id = ? AND status IN ('pending_debit', 'confirmed')",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    if let Ok(Some((id, pin))) = existing {
        return Json(json!({
            "error": "You already have an active reservation",
            "reservation_id": id,
            "pin": pin,
        }));
    }

    // Validate experience exists
    // Validate pricing tier and get price
    // Generate unique PIN
    // Create reservation with status = 'pending_debit', expires_at = now + 24h
    // Create debit_intent
    // Fire WhatsApp PIN delivery (async, non-blocking)
    // Return PIN + reservation_id
}
```

### PIN Generation with Uniqueness Check
```rust
const PIN_CHARSET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";
const PIN_LENGTH: usize = 6;

async fn generate_unique_pin(db: &SqlitePool) -> Result<String, String> {
    let mut rng = rand::thread_rng();
    for _ in 0..5 {
        let pin: String = (0..PIN_LENGTH)
            .map(|_| PIN_CHARSET[rng.gen_range(0..PIN_CHARSET.len())] as char)
            .collect();

        let exists = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM reservations WHERE pin = ? AND status IN ('pending_debit', 'confirmed')",
        )
        .bind(&pin)
        .fetch_one(db)
        .await
        .map(|r| r.0 > 0)
        .unwrap_or(true);

        if !exists {
            return Ok(pin);
        }
    }
    Err("Failed to generate unique PIN after 5 attempts".into())
}
```

### Expiry Cleanup in Scheduler
```rust
// Added to scheduler::tick()
async fn expire_reservations(state: &Arc<AppState>) -> anyhow::Result<()> {
    // Find expired confirmed reservations
    let expired = sqlx::query_as::<_, (String, String, Option<String>)>(
        "SELECT id, driver_id, debit_intent_id FROM reservations
         WHERE status = 'confirmed' AND expires_at < datetime('now')",
    )
    .fetch_all(&state.db)
    .await?;

    for (res_id, driver_id, debit_intent_id) in &expired {
        // Mark reservation as expired
        sqlx::query(
            "UPDATE reservations SET status = 'expired', updated_at = datetime('now') WHERE id = ?",
        )
        .bind(res_id)
        .execute(&state.db)
        .await?;

        // Create refund debit_intent if original debit was completed
        if let Some(intent_id) = debit_intent_id {
            let completed = sqlx::query_as::<_, (i64,)>(
                "SELECT amount_paise FROM debit_intents WHERE id = ? AND status = 'completed'",
            )
            .bind(intent_id)
            .fetch_optional(&state.db)
            .await?;

            if let Some((amount,)) = completed {
                // Create refund intent (negative amount or separate refund)
                let refund_id = uuid::Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id, status, origin)
                     VALUES (?, ?, ?, ?, 'pending', 'local')",
                )
                .bind(&refund_id)
                .bind(driver_id)
                .bind(-amount)  // negative = refund
                .bind(res_id)
                .execute(&state.db)
                .await?;
            }
        }
    }

    if !expired.is_empty() {
        tracing::info!("[scheduler] Expired {} reservations", expired.len());
    }
    Ok(())
}
```

### PWA Reservation API Methods
```typescript
// Added to api object in src/lib/api.ts
createReservation: (experience_id: string, pricing_tier_id: string) =>
  fetchApi<{
    status?: string;
    reservation_id?: string;
    pin?: string;
    expires_at?: string;
    error?: string;
  }>("/customer/reservation", {
    method: "POST",
    body: JSON.stringify({ experience_id, pricing_tier_id }),
  }),

getReservation: () =>
  fetchApi<{
    reservation?: RemoteReservation | null;
    error?: string;
  }>("/customer/reservation"),

cancelReservation: () =>
  fetchApi<{ status?: string; refund_paise?: number; error?: string }>(
    "/customer/reservation",
    { method: "DELETE" }
  ),

modifyReservation: (experience_id: string, pricing_tier_id: string) =>
  fetchApi<{
    status?: string;
    reservation_id?: string;
    pin?: string;
    error?: string;
  }>("/customer/reservation", {
    method: "PUT",
    body: JSON.stringify({ experience_id, pricing_tier_id }),
  }),
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Pod assigned at booking (customer_book_session) | Pod-agnostic reservation (this phase) | Phase 4 | Remote booking possible without pod availability |
| Direct wallet debit on booking | Debit intent pattern (Phase 3) | Phase 3 | Cloud can create bookings, local processes payments |
| OTP-only WhatsApp | OTP + PIN WhatsApp | Phase 4 | Same Evolution API, new message type |

## Open Questions

1. **Refund mechanism for cancellations**
   - What we know: Debit intents have amount_paise. Refunds need to credit back.
   - What's unclear: Should refund use negative debit_intent amount, or a separate `refund_intents` table, or call `wallet::refund()` directly on the cloud?
   - Recommendation: Use negative amount in debit_intents (simplest, reuses existing sync). When local processes a negative debit_intent, it calls `wallet::credit()`. This keeps the single-writer model (local writes wallets).

2. **Modification pricing delta**
   - What we know: Customer can change experience/duration. Price may change.
   - What's unclear: If new price > old price, create additional debit_intent? If new price < old price, partial refund?
   - Recommendation: Cancel old reservation (refund), create new reservation (debit). Atomic from customer perspective but simpler than delta calculations.

3. **Cloud wallet balance check accuracy**
   - What we know: Cloud has synced wallet balance (up to 30s stale)
   - What's unclear: Should cloud reject bookings that appear to exceed balance, or always create and let local sort it out?
   - Recommendation: Cloud does optimistic check (reject if clearly insufficient). If close to edge, create anyway -- local debit_intent processing handles the authoritative check. Reservation goes to `failed` if debit fails.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) + manual E2E (no Jest config for PWA) |
| Config file | Cargo.toml (Rust tests exist) |
| Quick run command | `cargo test -p racecontrol -- reservation` |
| Full suite command | `cargo test -p rc-common && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BOOK-01 | Customer can book experience from PWA | manual E2E | N/A (UI flow) | N/A |
| BOOK-02 | Pod-agnostic reservation created | unit | `cargo test -p racecontrol -- reservation::test_create` | Wave 0 |
| BOOK-03 | 6-char PIN generated, displayed | unit | `cargo test -p racecontrol -- pin::test_generate` | Wave 0 |
| BOOK-04 | PIN delivered via WhatsApp | manual | N/A (requires Evolution API) | N/A |
| BOOK-05 | View/cancel/modify reservation | unit + manual | `cargo test -p racecontrol -- reservation::test_cancel` | Wave 0 |
| BOOK-06 | Reservations expire after TTL | unit | `cargo test -p racecontrol -- reservation::test_expiry` | Wave 0 |
| BOOK-07 | Expired reservations auto-refund | unit | `cargo test -p racecontrol -- reservation::test_expiry_refund` | Wave 0 |
| API-04 | Reservation CRUD endpoints | integration | `cargo test -p racecontrol -- reservation::test_api` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- reservation`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/reservation.rs` -- new module with unit tests for PIN generation, reservation CRUD, expiry logic
- [ ] No PWA test infrastructure exists (no jest.config, no test files) -- PWA testing is manual E2E only

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/db/mod.rs` lines 2313-2369 -- reservations + debit_intents table schemas (verified existing)
- `crates/racecontrol/src/cloud_sync.rs` lines 23, 288-365 -- sync tables list includes reservations/debit_intents, process_debit_intents function
- `crates/racecontrol/src/auth/mod.rs` lines 998-1093 -- OTP WhatsApp delivery via Evolution API
- `crates/racecontrol/src/scheduler.rs` -- existing 60s tick scheduler pattern
- `crates/racecontrol/src/wallet.rs` -- wallet::debit/credit/refund/get_balance functions
- `crates/racecontrol/src/pod_reservation.rs` -- existing pod reservation module (separate from remote reservations)
- `crates/racecontrol/src/api/routes.rs` lines 6118-6250 -- existing customer_book_session handler pattern
- `pwa/src/lib/api.ts` lines 701-749 -- existing bookSession/bookCustom/activeReservation API methods
- `pwa/src/app/book/page.tsx` -- existing booking wizard with step-by-step flow

### Secondary (MEDIUM confidence)
- `rand` crate `thread_rng()` is CSPRNG -- well-established Rust ecosystem knowledge

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- follows established patterns in codebase (routes, wallet, scheduler, WhatsApp)
- Pitfalls: HIGH -- identified from direct code analysis of existing booking flow and sync layer

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable -- internal codebase, no external dependency changes)
