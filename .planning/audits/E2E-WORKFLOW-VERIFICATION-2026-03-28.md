# E2E Workflow Verification Report — Customer Journey Simulation
## Unified Protocol v3.1 | 2026-03-28 Evening

## Scenario

> Walk-in customer → Staff creates profile on POS kiosk → Tops up Rs.1000 →
> Launches Assetto Corsa Single Player for 30 min → Game crashes at ~15 min →
> Verify refund, session recording, settings, audit trail.

---

## LAYER 1: Customer Creation via POS Kiosk — PASS

### Flow (Staff Kiosk Path)

| Step | Action | Code Location |
|------|--------|---------------|
| 1 | Staff opens kiosk → taps pod → DriverRegistration opens | `kiosk/src/app/staff/page.tsx` |
| 2 | Staff enters name + optional phone → "Create" | `kiosk/src/components/DriverRegistration.tsx:41-50` |
| 3 | `POST /api/v1/drivers` creates record | `routes.rs:1619-1663` |
| 4 | DB: INSERT into `drivers` (id, name, name_enc, phone_hash, phone_enc) | `routes.rs:1646-1657` |
| 5 | Returns `{id, name}` → UI advances to tier selection | `routes.rs:1660` |

### Two Creation Paths

| Feature | `/api/v1/drivers` (Staff Kiosk) | `/customer/register` (Self-Service) |
|---------|--------------------------------|--------------------------------------|
| Auth | Staff JWT | OTP + customer JWT |
| Required fields | `name` only | `name`, `dob`, `waiver_consent` |
| Wallet auto-created | **NO** (lazy on first credit) | **YES** (`routes.rs:5709`) |
| Waiver captured | No | Yes |
| Age validation (min 12) | No | Yes |
| Guardian for minors | No | Yes |
| Duplicate check | No | Yes (name+DOB) |
| `registration_completed` | Not set (stays 0) | Set to 1 |

**Note:** Wallet is lazy-created via `ensure_wallet()` in `wallet::credit()` at `wallet.rs:76` on first topup. Not a blocker.

---

## LAYER 2: Wallet Topup Rs.1000 — PASS

### Flow

| Step | Action | Code Location |
|------|--------|---------------|
| 1 | Staff taps "1000" quick preset in WalletTopup panel | `WalletTopup.tsx:19-27` |
| 2 | Selects "Cash" payment method | UI |
| 3 | `POST /api/v1/wallet/{driver_id}/topup` `{amount_paise: 100000, method: "cash"}` | `routes.rs:6421-6497` |
| 4 | `wallet::credit(100000, "topup_cash")` — atomic DB tx | `wallet.rs:62-129` |
| 5 | Bonus tier check: `100,000 < 200,000` (min for 10%) → **No bonus** | `routes.rs:6454-6461` |
| 6 | Journal: `DEBIT acc_cash 100000 / CREDIT acc_wallet 100000` (fire-and-forget) | `wallet.rs:140-141` |
| 7 | Admin audit log + WhatsApp alert | `routes.rs:6482-6490` |

### Conversion Result

```
Input:    Rs.1000 cash
Paise:    100,000
Bonus:    0 (below Rs.2000 threshold — tiers: Rs.2000=10%, Rs.4000=20%)
Credits:  100,000 paise = Rs.1,000 displayed
```

### DB Records

| Table | Record |
|-------|--------|
| `wallets` | `balance_paise=100000, total_credited_paise=100000` |
| `wallet_transactions` | `amount_paise=100000, txn_type='topup_cash', balance_after=100000` |
| `journal_entries` | Topup entry (fire-and-forget) |
| `journal_entry_lines` | 2 lines: acc_cash DEBIT, acc_wallet CREDIT |
| `audit_log` | wallet_topup action |

---

## LAYER 3: Game Launch — AC Single Player, 30 min — PASS

### Billing Is Deferred (Critical Design)

Wallet is debited at BOOKING time, but billing TIMER only starts when agent reports
`AcStatus::Live` from shared memory. Prevents charging for failed launches.

### Customer Self-Book Flow (Path A)

| Step | Action | Code Location |
|------|--------|---------------|
| 1 | `POST /customer/book` `{pricing_tier_id: "30min"}` | `routes.rs:6917-7131` |
| 2 | Validate tier: 30min = 70,000 paise (Rs.700) | `routes.rs:6928-6938` |
| 3 | Check balance: `100,000 >= 70,000` → OK | `routes.rs:6980-6991` |
| 4 | `wallet::debit(70000, "debit_session")` — atomic | `routes.rs:7022-7034` |
| 5 | Find idle pod, create reservation + PIN | `routes.rs:7004-7110` |
| 6 | Customer enters PIN → `POST /auth/validate-pin` | `auth/mod.rs:379-560` |
| 7 | Atomic token consumption + `billing::defer_billing_start()` | `billing.rs:422-456` |
| 8 | Auto-launch AC on pod via WS | `auth/mod.rs:535` |

### AC Launch Sequence on Pod

| Step | What | Code Location |
|------|------|---------------|
| 1 | Kill existing AC: `taskkill /IM acs.exe /F` | `ac_launcher.rs:340-356` |
| 2 | Write `race.ini` (car, track, AI, session type) | `ac_launcher.rs:1122-1142` |
| 3 | Write `assists.ini` (ABS, TC, DAMAGE=0 hardcoded) | `ac_launcher.rs:1146-1178` |
| 4 | Set FFB in `controls.ini` [FF] GAIN=70 (medium) | `ac_launcher.rs:497-526` |
| 5 | Safety verify: DAMAGE=0, SESSION_START=100 | `ac_launcher.rs:1183-1211` |
| 6 | Spawn `acs.exe` directly (single player) | `ac_launcher.rs:378-424` |
| 7 | Wait for PID stable 3s | `ac_launcher.rs:1314-1348` |
| 8 | Minimize background, bring game to foreground | `ac_launcher.rs:434-438` |

### Billing Transition: WaitingForGame → Active

| Trigger | Code Location |
|---------|---------------|
| Agent reads AC shared memory `acpmf_graphics` offset 4 = 2 (LIVE) | `assetto_corsa.rs:84-90` |
| Agent sends `GameStatusUpdate { status: Live }` via WS | Agent WS handler |
| Server `handle_game_status_update(AcStatus::Live)` | `billing.rs:463-662` |
| Remove from `waiting_for_game`, call `start_billing_session()` | `billing.rs:612-622` |
| Timer starts: `elapsed_seconds` ticks up per second | `billing.rs:271-291` |

### Wallet After Booking

```
Before: 100,000 paise (Rs.1,000)
Debit:  -70,000 paise (Rs.700) for 30min tier
After:   30,000 paise (Rs.300) remaining
```

---

## LAYER 4: Credit Deduction — 30 min Session — PASS

### Key Insight: Pay Upfront, Refund Unused

Credits are NOT deducted per-minute. Full session cost is debited at BOOKING. Billing
timer tracks elapsed time. At session end, proportional refund issued if ended early.

| Parameter | Value |
|-----------|-------|
| Allocated | 1800 seconds (30 min) |
| Wallet debit | 70,000 paise (Rs.700) — from pricing tier |
| Rate tier 1 (Standard) | 2,500 paise/min for first 30 min |
| Timer tick | Every 1s (`billing.rs:271-291`) |
| Timer increments | `elapsed_seconds += 1`, `driving_seconds += 1` |

### Tier Price vs Rate Calculation

Pricing tier = **fixed price** (Rs.700 for 30 min). Per-minute rates (Rs.25/min) used for
display and proportional refund calc. Wallet debit = tier price, not rate x minutes.

---

## LAYER 5: Premature End (Game Crash at 15 min) — CONDITIONAL FAIL (F-05)

### Crash Detection Flow

| Step | What | Code Location |
|------|------|---------------|
| 1 | AC crashes → acs.exe exits | OS level |
| 2 | Shared memory STATUS = 0 (OFF) | `assetto_corsa.rs` |
| 3 | Agent sends `GameStatusUpdate { status: Off }` | WS message |
| 4 | Server: `handle_game_status_update(AcStatus::Off)` | `billing.rs:683` |
| 5 | `end_billing_session(state, &session_id, EndedEarly)` | `billing.rs:2158-2305` |

### Expected Refund (15 min driven of 30 min)

```
allocated_seconds  = 1800 (30 min)
driving_seconds    = 900  (15 min)
wallet_debit_paise = 70,000 (Rs.700) — ORIGINAL debit at booking

remaining = 1800 - 900 = 900
refund    = (900 * 70000) / 1800 = 35,000 paise (Rs.350)
```

### BUG F-05: wallet_debit_paise Overwritten Before Refund Calc

**Location:** `billing.rs:2213` (UPDATE) vs `billing.rs:2255` (SELECT)

**Sequence:**
1. Line 2178: `final_cost_paise` calculated from per-minute rate tiers (NOT tier price)
   - At 15 min: `900 * 2500 / 60 = 37,500 paise`
2. Line 2213: `UPDATE billing_sessions SET wallet_debit_paise = 37500` — **OVERWRITES original 70,000**
3. Line 2255: `SELECT wallet_debit_paise FROM billing_sessions` — **reads 37,500 (not 70,000)**
4. Line 2267: `refund = (900 * 37500) / 1800 = 18,750 paise`

**Result:**
```
EXPECTED refund: Rs.350 (based on original Rs.700 debit)
ACTUAL refund:   Rs.187.50 (based on overwritten cost)
Customer LOSES:  Rs.162.50
```

**Fix:** Save original `wallet_debit_paise` before the UPDATE at line 2213, use it for refund calc.

---

## LAYER 6: Session Recording — PASS

### DB Records Across Full Journey

| Table | Records | Key Fields |
|-------|---------|------------|
| `billing_sessions` | 1 | `status='ended_early', driving_seconds=900, allocated_seconds=1800` |
| `billing_events` | 3 | `created` → `started` → `ended_early` |
| `wallet_transactions` | 3 | `topup_cash(+100000)` → `debit_session(-70000)` → `refund_session(+refund)` |
| `journal_entries` | 3 | topup → debit → refund (fire-and-forget) |
| `journal_entry_lines` | 6 | 2 per journal entry |
| `audit_log` | 1+ | wallet_topup action |

### Session Status Transitions

```
billing_sessions.status: 'active' → 'ended_early'

billing_events:
  1. 'created'     (driving_seconds = 0) — at start_billing_session
  2. 'started'     (driving_seconds = 0) — at start_billing_session
  3. 'ended_early' (driving_seconds = 900) — at end_billing_session
```

---

## LAYER 7: Game Settings Verification — PASS

### INI Files Written

| File | Path on Pod | Key Settings |
|------|------------|-------------|
| `race.ini` | `C:\Users\User\Documents\Assetto Corsa\cfg\race.ini` | Car, track, AI_LEVEL, SESSION_TYPE, DAMAGE=0 |
| `assists.ini` | same `cfg\assists.ini` | ABS, TC, AUTO_SHIFTER, DAMAGE=0 (hardcoded) |
| `controls.ini` | same `cfg\controls.ini` | [FF] GAIN=70 (medium preset) |

### Safety Enforcement

| Check | Enforced | Code Location |
|-------|----------|---------------|
| DAMAGE=0 | Hardcoded in assists.ini writer | `ac_launcher.rs:1146-1178` |
| DAMAGE=0 in race.ini | Post-write verify | `ac_launcher.rs:1183-1196` |
| SESSION_START=100 | Post-write verify | `ac_launcher.rs:1190` |
| Launch rejected on violation | `bail!("SAFETY VIOLATION")` | `ac_launcher.rs:1186,1193` |

### FFB Mapping

| Preset | GAIN | Code |
|--------|------|------|
| light | 40 | `ac_launcher.rs:499` |
| medium | 70 | `ac_launcher.rs:500` |
| strong | 100 | `ac_launcher.rs:501` |

---

## LAYER 8: Edge Cases

### Handled

| Scenario | Handling | Code |
|----------|----------|------|
| Insufficient balance | `balance < final_price` → error with balances | `routes.rs:6985-6991` |
| Double booking | Checks active reservation → rejects | `routes.rs:6994-7001` |
| Pod already has session | `active_timers.contains_key` → rejects | `billing.rs:1724-1728` |
| Disconnect pause | Auto-pause, 10min timeout → auto-end + refund | `billing.rs:862-1345` |
| Game pause (in-game) | `AcStatus::Pause` → pause timer | `billing.rs:275-288` |
| Launch timeout (180s) | Remove from waiting | `billing.rs:401-416` |
| Cancelled (never drove) | Full refund | `billing.rs:2282-2305` |
| Atomic debit race | `WHERE balance_paise >= ?` | `wallet.rs:188-196` |
| PIN brute force | 5 failures/pod → lockout | `auth/mod.rs:391-400` |
| TOCTOU on session | DB UNIQUE partial index | `billing.rs:1729-1730` |

---

## FINDINGS

| ID | Sev | Finding | Location | Status |
|----|-----|---------|----------|--------|
| **F-05** | **P1** | `end_billing_session` overwrites `wallet_debit_paise` with `final_cost_paise` (per-minute calc) before reading it for refund — customer loses money on early end | `billing.rs:2213 vs 2255` | **OPEN — needs fix** |
| F-01 | P2 | Staff-created customers bypass waiver/age/dedup, `registration_completed=0` | `routes.rs:1619-1663` | By design (physical) |
| F-02 | P3 | Journal entries fire-and-forget after wallet commit | `wallet.rs:131-138` | Known, documented |
| F-03 | P3 | No rate limiting on wallet topup endpoint | `routes.rs:6421` | Low risk (staff-only) |
| F-04 | Info | Integer division rounding: max 1 paisa loss | `billing.rs:2267` | Negligible |

---

## UNIFIED PROTOCOL GATE

| Layer | Result |
|-------|--------|
| Quality Gate | N/A (workflow analysis, not code change) |
| E2E | **FAIL** — F-05 incorrect refund on early end |
| Standing Rules | PASS |
| MMA | N/A (analysis phase) |

### **OVERALL: CONDITIONAL FAIL — F-05 (P1) must be fixed before next deploy**

### F-05 Fix Plan

**Root cause:** Line 2213 writes `final_cost_paise` (per-minute rate calc) into
`wallet_debit_paise` column before line 2255 reads it for refund proportional calc.

**Fix:** Read original `wallet_debit_paise` from the in-memory timer or DB BEFORE the
UPDATE overwrites it. Use the original value for refund calculation.

```rust
// Before the UPDATE at line 2212:
let original_wallet_debit = sqlx::query_as::<_, (Option<i64>,)>(
    "SELECT wallet_debit_paise FROM billing_sessions WHERE id = ?"
)
.bind(session_id)
.fetch_optional(&state.db).await.ok().flatten()
.and_then(|r| r.0);

// ... existing UPDATE at line 2212-2223 ...

// At line 2264, use original_wallet_debit instead of DB read:
if let Some(debit) = original_wallet_debit {
    if debit > 0 && (driving_seconds as i64) < allocated {
        let remaining = allocated - driving_seconds as i64;
        let refund_amount = (remaining * debit) / allocated;
        // ...
    }
}
```
