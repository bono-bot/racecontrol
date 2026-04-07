# Phase 339: API Endpoints - Context

**Gathered:** 2026-04-07
**Status:** Ready for planning

<domain>
## Phase Boundary

Update wallet API response schemas to expose rupee/bonus tracking fields from Phase 338. Add cash refund endpoint. Rename JSON fields to match unified "credits" language. All consumers (admin portal, POS, kiosk, customer PWA) get the same response schema from the same port (8080).

**Depends on:** Phase 338 (wallet core logic must exist)

</domain>

<decisions>
## Implementation Decisions

### Response Field Naming (WalletInfo serde renames)
- **D-01:** Use `#[serde(rename = "...")]` on WalletInfo struct fields to produce roadmap-matching JSON names:
  - `balance_paise` → `"balance_credits"` in JSON
  - `total_credited_paise` → `"total_credited"`
  - `total_debited_paise` → `"total_spent"`
  - `rupee_deposited_paise` → `"rupee_deposited"`
  - `rupee_refunded_paise` → `"rupee_refunded"`
  - `bonus_credited_paise` → `"bonus_credited"`
  - `max_cash_refund` stays as-is (no `_paise` suffix already)
- **D-02:** Rust field names stay unchanged (they're correct — values ARE in paise). Only JSON serialization changes.
- **D-03:** Cloud sync is NOT affected — it builds its own JSON from SQL `json_object(...)`, not from WalletInfo serde.
- **D-04:** Add `transactions_count: i64` field to WalletInfo, serde renamed to `"transactions_count"`. Computed via COUNT query in get_wallet_info().

### WalletTransaction serde rename
- **D-05:** WalletTransaction fields keep current names. `currency_type` already has the right name. No renames needed here.

### GET /wallet/{driver_id} handler
- **D-06:** Handler already returns `Json(json!({"wallet": wallet_info}))`. After D-01 serde renames, the response automatically matches SC-1: `{ balance_credits, rupee_deposited, rupee_refunded, bonus_credited, max_cash_refund, total_spent, transactions_count }`.
- **D-07:** Customer endpoint `/customer/wallet` gets the same WalletInfo struct — unified contract.

### POST /wallet/{driver_id}/topup response
- **D-08:** Change topup_wallet handler response fields:
  - `new_balance_paise` → `new_balance_credits`
  - `bonus_paise` → `bonus_credits_granted`
  - Add `rupee_amount` = the original deposit amount_paise (before bonus)
  - Add `max_cash_refund` = new max after this topup
- **D-09:** The handler manually constructs `Json(json!({...}))` — field renames are in the handler code, not serde.
- **D-10:** Idempotent replay response also gets the new field names.

### Cash Refund Endpoint (NEW)
- **D-11:** New route: `POST /wallet/{driver_id}/cash-refund` → `cash_refund_wallet` handler
- **D-12:** Registered in staff routes (same auth tier as existing refund). Admin/manager only — NOT cashier.
- **D-13:** Request body: `{ amount_paise: i64, notes: Option<String> }`
- **D-14:** Response: `{ status: "ok", type: "cash_refund", amount: i64, new_balance_credits: i64, max_cash_refund_remaining: i64 }` or `{ error: "Exceeds max cash refund of X" }`
- **D-15:** Calls `wallet::cash_refund(state, driver_id, amount_paise, staff_id, notes)` — staff_id extracted from StaffClaims
- **D-16:** Pre-check: call `wallet::get_max_cash_refund()` to show cap in error message (actual enforcement is TOCTOU-safe inside cash_refund())

### Existing Refund Endpoint Update
- **D-17:** Existing `POST /wallet/{driver_id}/refund` stays as credit refund only (refund_session / refund_manual)
- **D-18:** Update response to include `type: "credit_refund"` field for SC-3 differentiation
- **D-19:** Add `max_cash_refund` field to refund response so admin UI can show "or refund X as cash"

### GET /wallet/transactions update
- **D-20:** `currency_type` field already in WalletTransaction struct from Phase 338. SC-4 is satisfied by the existing serialization.
- **D-21:** Verify the `all_wallet_transactions` handler (staff date-based view) also includes currency_type — it uses the same WalletTransaction struct, so it should.

### Summary endpoint enhancement
- **D-22:** The `all_wallet_transactions` summary object should add: `total_rupee_deposits`, `total_bonus_credits`, `total_cash_refunds` alongside existing `total_credits_paise` / `total_debits_paise`.

### Webhook response update
- **D-23:** `payment_gateway_webhook` response: add `new_balance_credits` (renamed from `balance_after_paise`) for consistency. Keep `ok` field.

### Claude's Discretion
- Whether to add deprecation warnings for old field names or just change them (breaking change is acceptable — no external consumers)
- Error message formatting for cash refund cap exceeded
- Whether to add a `GET /wallet/{driver_id}/max-cash-refund` convenience endpoint (not in SC, but useful for admin UI polling)

</decisions>

<specifics>
## Specific Ideas

- All value fields remain in paise internally — the "credits" naming is display-only (1 credit = 1 paise)
- Cash refund is the ONLY new endpoint. All other changes are response field renames/additions on existing endpoints.
- The refund_wallet handler has complex MMA-203 logic (₹5000 max, ₹500 without reference, TOCTOU prevention) — cash_refund is deliberately separate to avoid entangling these security caps
- No per-frontend variants — admin, POS, kiosk, customer PWA all hit port 8080 and get the same JSON schema
- Admin dashboard (Phase 340) will consume these field names directly — naming must match before Phase 340 starts

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Wallet API handlers (PRIMARY — being modified)
- `crates/racecontrol/src/api/routes.rs` lines 449-455 — Staff wallet route registrations
- `crates/racecontrol/src/api/routes.rs` lines 222-223 — Customer wallet route registrations
- `crates/racecontrol/src/api/routes.rs` lines 8703-8712 — get_wallet handler
- `crates/racecontrol/src/api/routes.rs` lines 8724-8877 — topup_wallet handler
- `crates/racecontrol/src/api/routes.rs` lines 9215-9375 — refund_wallet handler
- `crates/racecontrol/src/api/routes.rs` lines 9378-9392 — customer_wallet handler
- `crates/racecontrol/src/api/routes.rs` lines 9148-9205 — all_wallet_transactions handler
- `crates/racecontrol/src/api/routes.rs` lines 8912-9070 — payment_gateway_webhook handler

### Wallet core (called by handlers — Phase 338 output)
- `crates/racecontrol/src/wallet.rs` — credit, debit, cash_refund, get_wallet_info, get_max_cash_refund, get_transactions
- `crates/rc-common/src/types.rs` lines 779-803 — WalletInfo and WalletTransaction structs (serde renames go here)

### Cloud sync (verify not broken by serde renames)
- `crates/racecontrol/src/cloud_sync.rs` lines 698-721 — wallet push (builds own JSON from SQL, NOT from WalletInfo serde)

### Phase 338 context (decisions to preserve)
- `.planning/phases/338-wallet-core-logic/338-CONTEXT.md` — D-07 through D-14 (cash_refund design, max_cash_refund formula)

### Business rules
- Memory: `~/.claude/projects/C--Users-bono/memory/project_credits_rupees_separation.md` — Full business model

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `wallet::cash_refund()` — Already implemented in Phase 338, ready to call from new endpoint
- `wallet::get_max_cash_refund()` — Returns cap for error messages
- `wallet::get_wallet_info()` — Already fetches all new columns, computes max_cash_refund
- `StaffClaims` extractor — Used by existing wallet handlers for auth

### Established Patterns
- All wallet handlers use `State(state): State<Arc<AppState>>` + `Path(driver_id): Path<String>`
- Staff handlers extract `claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>`
- Responses are `Json<Value>` with manual `json!({...})` construction
- Error responses: `Json(json!({"error": "message"}))`
- Idempotency pattern in topup_wallet: check idempotency_key before processing

### Integration Points
- Staff routes registered at lines 449-455 in routes.rs — new cash-refund route goes here
- `wallet::cash_refund()` signature: `(state, driver_id, amount_paise, staff_id, notes)` — maps directly to handler params
- WalletInfo in rc-common/types.rs is shared across crates — serde renames affect all consumers

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope. Admin dashboard UI (Phase 340), POS/kiosk display changes (Phase 341), and cloud sync updates (Phase 342) are explicitly later phases.

</deferred>

---

*Phase: 339-api-endpoints*
*Context gathered: 2026-04-07*
*[auto] All decisions derived from ROADMAP.md success criteria + codebase analysis. Business rules locked by Uday 2026-04-07.*
