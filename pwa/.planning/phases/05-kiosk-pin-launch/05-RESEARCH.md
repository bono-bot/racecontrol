# Phase 5: Kiosk PIN Launch - Research

**Researched:** 2026-03-21
**Domain:** Kiosk UI (Next.js/React) + Rust/Axum backend endpoint for remote booking PIN redemption
**Confidence:** HIGH

## Summary

Phase 5 connects the remote booking flow (Phase 4) to the venue kiosk. A customer who booked remotely arrives at the kiosk, enters their 6-character alphanumeric PIN, and the system validates it against the local `reservations` table (synced from cloud), assigns a pod, launches the game, and shows the customer their pod number.

The existing codebase already has most building blocks: `validate_pin_kiosk()` handles 4-digit walk-in PINs via `auth_tokens`, `pod_reservation::find_idle_pod()` assigns pods, `auth::launch_or_assist()` triggers game launch, and `StaffLoginScreen.tsx` provides a reusable numpad pattern. The new work is a **separate PIN redemption endpoint** that validates against the `reservations` table (not `auth_tokens`), bridges to the existing pod assignment + game launch flow, and a new kiosk UI page/component for 6-char alphanumeric PIN entry with rate limiting and lockout.

**Primary recommendation:** Build a new `POST /api/v1/kiosk/redeem-pin` endpoint that looks up the PIN in `reservations`, calls `find_idle_pod()` + `create_reservation()` + `launch_or_assist()`, and marks the reservation as `redeemed`. On the kiosk, add a new "Have a PIN?" entry point on the landing page that opens a 6-char alphanumeric numpad screen.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- New PIN entry screen on the kiosk -- accessible from the kiosk home/booking page
- Large numpad-style input for 6-character alphanumeric PIN
- Clear visual feedback: each character fills a box as entered
- Submit button validates against local server
- Success: shows assigned pod number + "Head to Pod X" with game loading indicator
- Failure: "Invalid PIN" with attempt counter, lockout message after 10 failures
- New endpoint: POST /kiosk/redeem-pin (or similar) on local server
- Validates PIN exists in local reservations table (synced from cloud via Phase 3)
- Checks reservation status is "confirmed" (not expired/cancelled/already redeemed)
- Assigns first available pod (using existing pod assignment logic)
- Marks reservation as "redeemed" immediately (one-time use)
- Triggers game launch on assigned pod (using existing game launch flow)
- Returns pod number and game loading status to kiosk
- Track PIN attempts per kiosk (by kiosk IP or session)
- Max 5 attempts per minute
- Lockout after 10 consecutive failures -- 5 minute cooldown
- Show remaining attempts and lockout timer on UI
- Use existing pod assignment logic from pod_reservation.rs
- First available pod (no specific pod promised in remote booking)
- If no pods available: "All pods busy -- please wait" with retry option
- Once assigned, game launches automatically via rc-agent

### Claude's Discretion
- Exact kiosk PIN entry component styling (should match existing kiosk design language)
- Rate limiting storage mechanism (in-memory vs SQLite)
- Whether to show a QR code scanner as alternative to PIN entry
- Game loading animation/progress indicator design
- How to handle the gap between "redeemed" and "game actually launched"

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| KIOSK-01 | Kiosk displays PIN entry screen for walk-in customers with remote bookings | New kiosk page/component with 6-char alphanumeric numpad, accessible from landing page. StaffLoginScreen.tsx provides reusable pattern. |
| KIOSK-02 | PIN validated against local server's synced reservations | New `POST /api/v1/kiosk/redeem-pin` endpoint queries `reservations` table WHERE `pin = ? AND status = 'confirmed'` |
| KIOSK-03 | Valid PIN triggers pod assignment (first available) and game launch | Chains `find_idle_pod()` -> `create_reservation()` -> `defer_billing_start()` -> `launch_or_assist()` |
| KIOSK-04 | Rate limiting: max 5 attempts per minute, lockout after 10 failures | Existing `auth_rate_limit_layer()` handles 5/min server-side via tower-governor. Kiosk UI tracks consecutive failures client-side for lockout display + 5 min cooldown. Server-side lockout via in-memory HashMap keyed by IP. |
| KIOSK-05 | PIN is one-time use -- marked as redeemed immediately | `UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now'), pod_number = ? WHERE id = ?` inside the redeem handler |
| KIOSK-06 | Customer sees assigned pod number and game loading status | Success screen shows pod number prominently + spinner/loading state. WebSocket `game_state_changed` event can update loading -> active status. |
</phase_requirements>

## Standard Stack

### Core (Already in Project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Next.js | 16.1.6 | Kiosk app framework | Already in use, App Router |
| React | 19.2.3 | UI components | Already in kiosk |
| TypeScript | 5.9.3 | Type safety | Already configured strict |
| Tailwind CSS | 4 | Styling | Already in kiosk |
| Axum | (project version) | HTTP server | Already the backend framework |
| SQLx | (project version) | Database queries | Already used for reservations table |
| tower-governor | (project version) | Rate limiting | Already used in `auth/rate_limit.rs` |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde_json | (project version) | JSON request/response | Already used in routes.rs |
| uuid | (project version) | Reservation IDs | Already used in pod_reservation.rs |

No new dependencies required. Everything needed is already in the project.

## Architecture Patterns

### Backend: Redeem PIN Endpoint Flow

```
POST /api/v1/kiosk/redeem-pin { pin: "ABC123" }
    |
    v
1. Rate limit check (tower-governor: 5/min per IP — already on auth routes)
2. Lockout check (in-memory HashMap<IP, (fail_count, locked_until)>)
3. Query: SELECT * FROM reservations WHERE pin = ? AND status = 'confirmed' AND expires_at > datetime('now')
4. If not found: increment fail counter, return error
5. If found:
   a. UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now')
   b. find_idle_pod() -> pod_id
   c. pod_reservation::create_reservation(driver_id, pod_id)
   d. billing::defer_billing_start(pod_id, driver_id, pricing_tier_id, ...)
   e. auth::launch_or_assist(pod_id, billing_session_id, experience_id, ...)
   f. Clear lock screen on pod agent
   g. Reset fail counter for this IP
6. Return: { pod_number, pod_id, driver_name, experience_name, allocated_seconds }
```

### Key Insight: Two Reservation Systems

The codebase has TWO different reservation concepts:
1. **`reservations` table** (Phase 4): Remote bookings with 6-char PINs, synced from cloud. Status: pending_debit -> confirmed -> redeemed/expired/cancelled. Pod-agnostic (no pod assigned at booking time).
2. **`pod_reservations` table**: Local pod assignments. Created when a pod is assigned to a driver. Has pod_id, driver_id, status (active/completed/expired).

The redeem-pin flow BRIDGES these: it reads from `reservations` (to validate the remote PIN) and writes to `pod_reservations` (to assign a pod locally).

### Kiosk UI: New Route or Component?

**Recommendation:** New component `PinRedeemScreen.tsx` (NOT a new route). Add a "Have a PIN?" button on the landing page (`/`) that opens a full-screen overlay or navigates to the PIN entry. This keeps it discoverable without creating a separate page that could be hard to find.

**Alternative considered:** New `/redeem` route. Rejected because the landing page is the primary entry point and adding a button there is simpler and more discoverable.

### Recommended Component Structure
```
kiosk/src/
├── components/
│   └── PinRedeemScreen.tsx    # NEW: 6-char alphanumeric PIN entry + success/error states
├── app/
│   └── page.tsx               # MODIFIED: Add "Have a PIN?" button that toggles PinRedeemScreen
└── lib/
    └── api.ts                 # MODIFIED: Add redeemPin() method
```

### Backend Structure
```
crates/racecontrol/src/
├── api/
│   └── routes.rs              # MODIFIED: Add POST /kiosk/redeem-pin route + handler
└── reservation.rs             # MODIFIED: Add redeem_pin() function
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Server-side rate limiting | Custom middleware | `auth_rate_limit_layer()` (tower-governor) | Already exists, handles 5/min per IP, returns 429 |
| Pod assignment | Custom pod finder | `pod_reservation::find_idle_pod()` + `create_reservation()` | Already handles idle check + active reservation filtering |
| Game launch | Custom launch logic | `auth::launch_or_assist()` | Handles experience lookup, game config, agent messaging |
| Billing start | Manual billing | `billing::defer_billing_start()` | Defers billing until game reaches LIVE status |
| PIN numpad UI | New numpad from scratch | Adapt `StaffLoginScreen.tsx` pattern | Identical interaction: boxes + numpad + auto-submit, just 6 chars + alphanumeric |
| WebSocket state | Custom polling | `useKioskSocket` hook | Already provides game_state_changed events for loading status |

**Key insight:** The entire "assign pod + start billing + launch game" pipeline already exists in `validate_pin_kiosk()`. The redeem-pin endpoint just needs to bridge from `reservations` table lookup to the same pipeline.

## Common Pitfalls

### Pitfall 1: Race Condition on PIN Redemption
**What goes wrong:** Two kiosks (or rapid double-tap) redeem the same PIN simultaneously.
**Why it happens:** SELECT then UPDATE is not atomic without transaction isolation.
**How to avoid:** Use atomic UPDATE...RETURNING pattern (like existing `validate_pin_kiosk` uses for auth_tokens):
```sql
UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now'), pod_number = ?
WHERE id = (
    SELECT id FROM reservations
    WHERE pin = ? AND status = 'confirmed' AND expires_at > datetime('now')
    LIMIT 1
) AND status = 'confirmed'
RETURNING id, driver_id, experience_id
```
**Warning signs:** Two pods assigned to the same reservation.

### Pitfall 2: Reservation Status Mismatch
**What goes wrong:** PIN exists but status is `pending_debit` (debit intent not yet processed by local server).
**Why it happens:** Cloud creates reservation as `pending_debit`, local debit_intent processing updates to `confirmed`. Sync lag means the reservation might arrive before the debit is processed.
**How to avoid:** Only accept `confirmed` status. If `pending_debit`, return a friendly "Your booking is being processed, please wait a moment" message.
**Warning signs:** Customer has valid PIN but gets "Invalid PIN" error.

### Pitfall 3: Alphanumeric PIN vs Numeric-Only Existing PINs
**What goes wrong:** Existing PIN inputs are 4-digit numeric only. Remote booking PINs are 6-char alphanumeric (A-Z, 2-9).
**Why it happens:** `StaffLoginScreen` and landing page `PinModal` only have digits 0-9.
**How to avoid:** The new PIN entry component MUST include letter buttons (A-Z minus ambiguous chars) or a full QWERTY layout. Given the PIN charset is `ABCDEFGHJKMNPQRSTUVWXYZ23456789` (31 chars), a grid layout with letters + numbers works well.
**Warning signs:** Customer cannot type their PIN because it contains letters.

### Pitfall 4: No Pods Available After PIN Validated
**What goes wrong:** PIN is valid and marked redeemed, but `find_idle_pod()` returns None.
**Why it happens:** All 8 pods are in use.
**How to avoid:** Check pod availability BEFORE marking the reservation as redeemed. If no pods available, return error without consuming the PIN.
**Warning signs:** Reservation marked redeemed but customer gets "no pods available" — PIN is now unusable.

### Pitfall 5: Game Launch Gap
**What goes wrong:** Customer sees "Head to Pod X" but the game hasn't actually started yet.
**Why it happens:** Game launch is async — `launch_or_assist()` sends command to agent, game process takes 10-30 seconds to start.
**How to avoid:** Show the pod number immediately, then show a loading indicator. The WebSocket `game_state_changed` event will update when the game is actually launching/running. The success screen should say "Head to Pod X — game is loading" and optionally auto-close after 15 seconds.
**Warning signs:** Customer goes to pod and sees a blank screen.

### Pitfall 6: Lockout State Persistence
**What goes wrong:** Server restart clears in-memory lockout state, allowing brute force after restart.
**Why it happens:** Using HashMap for lockout tracking.
**How to avoid:** Acceptable risk for this use case — the 5/min tower-governor rate limit persists structurally (token bucket refill). The lockout is defense-in-depth. An attacker would need to restart the server to bypass the 10-failure lockout, which requires physical access.

## Code Examples

### Backend: Redeem PIN Handler (Rust)
```rust
// Source: Based on existing validate_pin_kiosk() pattern in auth/mod.rs
// and pod_reservation::find_idle_pod() + create_reservation()

#[derive(serde::Deserialize)]
struct RedeemPinRequest {
    pin: String,
}

async fn kiosk_redeem_pin(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(req): Json<RedeemPinRequest>,
) -> Json<Value> {
    let pin = req.pin.trim().to_uppercase();
    if pin.len() != 6 {
        return Json(json!({ "error": "PIN must be 6 characters" }));
    }

    // Check lockout (in-memory, per-IP)
    // ... lockout check logic ...

    // Atomic find + redeem: check availability first
    let pod_id = match pod_reservation::find_idle_pod(&state).await {
        Some(id) => id,
        None => return Json(json!({ "error": "All pods are currently in use. Please wait a moment and try again." })),
    };

    let pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id).map(|p| p.number).unwrap_or(0)
    };

    // Atomic UPDATE reservations — prevents double-redeem
    let row = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now'),
         pod_number = ?, updated_at = datetime('now')
         WHERE id = (
             SELECT id FROM reservations
             WHERE pin = ? AND status = 'confirmed' AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'confirmed'
         RETURNING id, driver_id, experience_id",
    )
    .bind(pod_number)
    .bind(&pin)
    .fetch_optional(&state.db)
    .await;

    // ... continue with pod_reservation::create_reservation, defer_billing_start, launch_or_assist ...
}
```

### Kiosk: PIN Entry Component (TypeScript/React)
```tsx
// Source: Adapted from StaffLoginScreen.tsx pattern
// Key difference: 6 chars, alphanumeric, includes letter grid

const PIN_CHARS = "ABCDEFGHJKMNPQRSTUVWXYZ23456789".split("");

function PinRedeemScreen({ onBack }: { onBack: () => void }) {
  const [pin, setPin] = useState("");
  const [step, setStep] = useState<"entry" | "validating" | "success" | "error">("entry");
  const [failCount, setFailCount] = useState(0);
  const [lockedUntil, setLockedUntil] = useState<number | null>(null);

  function handleChar(char: string) {
    if (pin.length < 6) setPin(prev => prev + char);
  }

  async function handleSubmit() {
    if (pin.length !== 6) return;
    setStep("validating");
    try {
      const res = await api.redeemPin(pin);
      if (res.error) {
        setFailCount(prev => prev + 1);
        // ... handle lockout at 10 failures ...
        setStep("error");
      } else {
        setStep("success");
        // Show pod number from res.pod_number
      }
    } catch {
      setStep("error");
    }
  }

  // 6 PIN display boxes
  // Character grid (letters + numbers)
  // Submit button
}
```

### API Client Addition
```typescript
// Source: Add to kiosk/src/lib/api.ts following existing pattern
redeemPin: (pin: string) =>
  fetchApi<{
    error?: string;
    pod_number?: number;
    pod_id?: string;
    driver_name?: string;
    experience_name?: string;
    allocated_seconds?: number;
    billing_session_id?: string;
  }>("/kiosk/redeem-pin", {
    method: "POST",
    body: JSON.stringify({ pin }),
  }),
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 4-digit numeric PIN on pod screen | 6-char alphanumeric PIN at kiosk | Phase 4/5 (now) | Kiosk PIN entry needs alphanumeric input, not just digits |
| Walk-in only (auth_tokens) | Remote booking + walk-in (reservations + auth_tokens) | Phase 4 | Two parallel PIN systems: auth_tokens for walk-in, reservations for remote |
| Pod pre-assigned at booking | Pod assigned at redemption | Phase 4 design | find_idle_pod() called at redemption time, not booking time |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust: `cargo test` (built-in), Kiosk: none detected |
| Config file | Cargo.toml (Rust), no test config for kiosk |
| Quick run command | `cargo test -p racecontrol -- reservation` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| KIOSK-01 | PIN entry screen renders with 6-char input | manual-only | Visual verification on kiosk | N/A |
| KIOSK-02 | PIN validated against reservations table | unit | `cargo test -p racecontrol -- redeem_pin` | Wave 0 |
| KIOSK-03 | Valid PIN triggers pod assignment + game launch | integration | `cargo test -p racecontrol -- redeem_pin_assigns_pod` | Wave 0 |
| KIOSK-04 | Rate limiting: 5/min, lockout after 10 | unit | `cargo test -p racecontrol -- redeem_pin_lockout` | Wave 0 |
| KIOSK-05 | PIN marked redeemed (one-time use) | unit | `cargo test -p racecontrol -- redeem_pin_one_time` | Wave 0 |
| KIOSK-06 | Customer sees pod number + loading | manual-only | Visual verification on kiosk | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p racecontrol -- reservation`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests for redeem_pin handler` -- covers KIOSK-02, KIOSK-03, KIOSK-05
- [ ] `tests for lockout logic` -- covers KIOSK-04
- [ ] No kiosk test infrastructure exists (no jest/vitest configured) -- UI testing is manual-only

## Open Questions

1. **Alphanumeric Input Layout**
   - What we know: PIN charset is 31 chars (23 letters + 8 digits). StaffLoginScreen uses a 3x4 numpad.
   - What's unclear: Best layout for 31 characters on a touch kiosk. Full QWERTY? 6x6 grid? Letter row + number row?
   - Recommendation: Use a compact grid layout (e.g., 7 columns x 5 rows = 35 slots, 31 filled + Clear/Backspace/Submit/empty). Letters in alphabetical order for discoverability. This is Claude's discretion per CONTEXT.md.

2. **pending_debit Status Handling**
   - What we know: Reservations start as `pending_debit`, transition to `confirmed` when debit_intent is processed locally.
   - What's unclear: How long does debit processing typically take? Could a customer arrive before their reservation reaches `confirmed`?
   - Recommendation: Return a distinct message for `pending_debit` PINs: "Your booking is being processed. Please try again in a minute." Do NOT consume the PIN.

3. **QR Code Scanner Alternative**
   - What we know: CONTEXT.md lists "Whether to show a QR code scanner" as Claude's discretion.
   - Recommendation: Skip QR for Phase 5. The kiosk hardware likely does not have a camera. PIN entry is sufficient. Can be added later if needed.

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/reservation.rs` — PIN generation, create/cancel/modify reservation
- `crates/racecontrol/src/pod_reservation.rs` — find_idle_pod(), create_reservation(), end_reservation()
- `crates/racecontrol/src/auth/mod.rs` — validate_pin_kiosk() (lines 1244-1447), launch_or_assist(), create_auth_token()
- `crates/racecontrol/src/auth/rate_limit.rs` — auth_rate_limit_layer() using tower-governor
- `crates/racecontrol/src/api/routes.rs` — kiosk_routes(), customer_book_session() flow (lines 6183-6390)
- `crates/racecontrol/src/db/mod.rs` — reservations table schema (lines 2315-2338)
- `kiosk/src/components/StaffLoginScreen.tsx` — PIN numpad UI pattern
- `kiosk/src/app/page.tsx` — Current landing page with 4-digit PIN modal
- `kiosk/src/lib/api.ts` — validateKioskPin() pattern
- `kiosk/.planning/codebase/ARCHITECTURE.md` — Data flow, WebSocket events, auth flow

### Secondary (MEDIUM confidence)
- `kiosk/src/components/PodKioskView.tsx` — Pod state derivation, can inform success screen design

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in project, no new deps needed
- Architecture: HIGH - follows existing patterns (validate_pin_kiosk, find_idle_pod, launch_or_assist)
- Pitfalls: HIGH - derived from reading actual codebase race conditions and status flows

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable codebase, patterns well-established)
