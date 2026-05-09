# W3 RCA — Wallet HOLD-RELEASE-CAPTURE state machine (PACT-024 sibling)

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (Captain BILATERAL directive committed at comms-link `8768b628` 2026-05-09 ~09:28 IST)

**Author:** james · **Date:** 2026-05-09 ~10:35 IST · **Branch context:** `feat/v2-wave-1-w1-s1-billing-service` HEAD `15490644` (W1-S5 RCA DRAFT directly upstream)

**Status:** DRAFT — pending Captain G33 review + bono AMPLIFIER + MMA Step 1 DIAGNOSE before W3 H1 PLAN can be filed; ALSO gated on Wave 1 closure (W1-S6/S7/S8 not yet shipped) per V2.0 6-wave ordering

**Foundational-boundary classification:** YES — wallet boundary per doctrine §"MMA escalation" (named explicitly alongside billing/auth/pod-state-channel/WhatsApp identity/DB schema). Triggers: MMA Step 1 DIAGNOSE on the RCA itself + per-PR Captain merge auth at every W3 PR-open.

**Sibling-PACT linkage:** PACT-20260504-024 wallet concurrency / idempotency extension (FILED-AWAITS-AMPLIFIER 2026-05-04 ~03:42 IST) + PACT-024-024a-wallet-concurrency-substrate-ship-plan.md (pre-authored 2026-05-04 ~17:15 IST) + PACT-024b "Kidneys" reconciliation worker (future sibling) + future T-F1 bonus-arbitrage exploit fix PACT (gates on this landing first)

---

## Pre-flight reconciliation notes (read first)

Three doctrine reconciliation points must be Captain-dispositioned before this RCA can be closed:

| # | Source | Says |
|---|---|---|
| 1 | `crates/v2-db/src/wallets.rs:155-159` (W1-S2 SHIPPED 2026-05-08 ~21:30 IST) | "HOLD-RELEASE-CAPTURE state machine + idempotency cascade DEFERRED to Wave 3 (per PHASE-1-WAVE-1-PLAN.md §1.2 OOS / PACT-024 wallet concurrency parent). Wave 1 surface is naive optimistic UPDATE; race-safety relies on SQLite single-writer + WHERE-guarded predicate + CHECK(balance_credits >= 0)." |
| 2 | `comms-link/proposals/PACT-20260504-024-wallet-concurrency-idempotency-extension.md` status field | `FILED-AWAITS-AMPLIFIER` — james AMPLIFIER vote on Q1-Q5 still outstanding 5 days after FILE; bono is LEAD; substrate-class auto-ratify path requires AMPLIFIER + MMA + CGP H3 + §S-N append per §2.1 OPTION-A |
| 3 | `comms-link/.planning/ship-plans/PACT-024-024a-wallet-concurrency-substrate-ship-plan.md` §A targets | references `racecontrol/crates/racecontrol/src/wallet.rs` + `racecontrol/crates/racecontrol/src/billing.rs` + `racecontrol/src/api/routes.rs`. **STALE.** Wave 1 W1-S2 (`crates/v2-db/src/wallets.rs::WalletService`) is now the canonical V2 wallet surface. Re-targeting required. |

**Q-W3-RECONCILE-1 (Captain decision required):** Confirm Wave 3 IS PACT-024 + 024a executed under V2.0 6-wave plan, AND authorize re-targeting from `crates/racecontrol/src/wallet.rs` (V1) → `crates/v2-db/src/wallets.rs` (V2 W1-S2 canonical). PACT-024a §A is otherwise valid; only the file-path targets need bumping.

**Q-W3-RECONCILE-2 (Captain decision required):** Authorize Wave 3 implementation entry AFTER Wave 1 closure (W1-S6 PIN-LOCKOUT + W1-S7 WhatsApp PIN + W1-S8 fallback + Quality Gate + E2E + visual + venue Server .23 rebuild) OR pull-forward into Wave 1 tail. Wave-2 dynamic pricing PACT-DRAFT triggers on Wave 1 land, not Wave 3 — so wave order can hold.

**Q-W3-RECONCILE-3 (Captain decision required):** PACT-024 AMPLIFIER vote — james must disposition Q1-Q5 (or Captain rules them via §S-N) before this RCA's §5 proposal becomes executable. The bono recommendations (Q1-a / Q2-c / Q3-c / Q4-c / Q5-c) remain my proposed defaults but require explicit ratification.

---

## §1 — Boundary map

### V1 ↔ V2 wallet surface inventory

| Path | Lines | V1-era? | V2-era? | Touched by W3? |
|---|---|---|---|---|
| `crates/racecontrol/src/wallet.rs` | 1-end (whole file) | YES — `ensure_wallet`, `get_balance`, `get_wallet_info`, `credit_in_tx`, `debit_in_tx`, `currency_type_for`, `resolve_wallet_owner`. driver_id-scoped, rupee/credit columns (Phase 337/338, 2026-04-07) | NO | INDIRECT — V1 surface stays for V1 driver_id flows; W3 does NOT modify it; cloud_sync_debit.rs bridge unchanged |
| `crates/racecontrol/src/wallet_refund.rs` | 1-end | YES — `cash_refund`, `get_max_cash_refund`, `refund`, `get_transactions`. TOCTOU-safe inside-tx cap-check (D-14, Phase 338-02) | NO | INDIRECT — W1-S3 refund 3-band routing already shipped; W3 may need a "refund-during-HOLD" branch |
| `crates/racecontrol/src/api/wallet_ops.rs` | 1-end | YES (PACT-018-era) | NO | NO |
| `crates/racecontrol/src/api/wallet_gateway.rs` | 1-end | YES | NO | INDIRECT — if W3 adds HTTP routes for hold/release/capture, gateway pattern is the V1 sibling reference |
| `crates/racecontrol/src/api/wallet_staff.rs` | 1-end | YES | NO | NO |
| `crates/racecontrol/src/billing_session_end.rs` | 1-end (key: `end_billing_session` + `end_billing_session_public`) | YES — calls into V1 wallet_refund + cloud_sync_debit; F-05 historical site (UPDATE-then-SELECT same column) | NO | INDIRECT — V1 billing path stays; V2 Wave 1 service skeleton (W1-S1) is the new path; W3 hooks into V2 `WalletService::capture` not V1 `end_billing_session` |
| `crates/racecontrol/src/billing_session_lifecycle.rs:26-80+` | dashboard command handler | YES (Phase 385 v49.0) | NO | INDIRECT — DashboardCommand::EndBilling currently calls V1 end_billing_session; eventual V2 cutover ties to W3 capture |
| `crates/racecontrol/src/cloud_sync_debit.rs` | 1-end | YES — cloud→venue debit-intent sync (D-20) | NO | INDIRECT — cross-environment HOLD coordination question (Q-W3-9) gates whether this needs extension |
| `crates/racecontrol/src/billing.rs` + 30+ billing_*.rs files | many | YES (Phase 385 split) | NO | INDIRECT — V2 `crates/v2-db/src/billing/` (W1-S1) is the V2-side canonical; V1 stays for V1 driver_id sessions |
| `crates/v2-db/src/wallets.rs:1-71` | Wallet/WalletTopup/WalletRedemption struct + PaymentMethod/RedemptionKind enums | — | YES (PACT-013 substrate; W1-S2 ship 2026-05-08) | YES — adds `WalletHold` struct + `HoldStatus` enum |
| `crates/v2-db/src/wallets.rs:72-122` | GST helpers (`operator_net_paise`, `gst_amount_paise`, `RP_GST_RATE_BPS_18`) | — | YES (W1-S9 ship) | NO — pure helpers; HOLD math reuses unchanged |
| `crates/v2-db/src/wallets.rs:124-338` | `WalletService::reserve_credits` + `reconcile_redemption` (W1-S2 SHIPPED) | — | YES | YES — `reserve_credits` either DEPRECATED + replaced by `WalletService::hold` OR refactored into thin wrapper around `hold` for Wave-1 callers |
| `crates/v2-db/migrations/20260503000001_initial_schema.sql:34-44` | `wallets` table (customer_id, balance_credits with CHECK >= 0, last_activity_at, breakage_recognized_at) | — | YES (PACT-013 schema) | YES — column ADD `held_credits INTEGER NOT NULL DEFAULT 0 CHECK (held_credits >= 0)` OR keep wallets unchanged + add separate `wallet_holds` table (Q-W3-2) |
| `crates/v2-db/migrations/20260503000001_initial_schema.sql:106-end` | `wallet_topups` audit table (8-year retention; CHECK gst_rate_bps = 1800; NEVER-DELETE trigger via migration 002) | — | YES | NO directly; W3 does NOT touch top-up path |
| `crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql` | `wallet_redemptions` FK repair (NF-james-4) | — | YES | INDIRECT — Wave 3 captures call `INSERT INTO wallet_redemptions`; FK to sessions(id) must be intact at W3 boundary; W1-S2 verified |
| `crates/v2-db/src/sessions.rs` | session struct + insert/select | — | YES | INDIRECT — W3 capture composes session_id into wallet_redemptions row |
| `crates/v2-db/src/lib.rs` (Error enum) | `WalletNotFound`, `InsufficientFunds {customer_id, balance_credits, required_credits}` | — | YES | YES — adds `HoldNotFound`, `HoldAlreadyCaptured`, `HoldAlreadyReleased`, `IdempotencyKeyExpired`, `VersionConflict`, `MaxRetriesExceeded` variants |
| `crates/racecontrol/src/billing_game_status.rs` | game_running heartbeat handler (V1) | YES | NO | INDIRECT — V1↔V2 bridge: heartbeat triggers W3 capture transition; bridge module is new |
| `crates/racecontrol/src/game_launcher_state.rs` + `game_launcher_ops.rs` | game launch state machine (V1) | YES | NO | INDIRECT — launch-success signals W3 capture; launch-fail signals W3 release |

### Cross-organ data flow at the boundary

1. **Top-up** (POS .130 cash / PWA UPI / Kiosk .23 UPI) → V2 `wallet_topups` row + `wallets.balance_credits += credits` + tax invoice. Idempotency-key propagation per PACT-024a §A.4 — out of W3 production scope, in W3 design scope (cross-pilot).
2. **Session start** → kiosk/POS calls a V2 billing-service entrypoint (W1-S1) → `WalletService::hold(customer_id, session_id, credits, idempotency_key)` (NEW W3) → `INSERT wallet_holds` + atomic `UPDATE wallets SET balance_credits = balance_credits - ?, held_credits = held_credits + ? WHERE balance_credits >= ?` (or columnless variant per Q-W3-2).
3. **Game launch attempt** → V1 game_launcher_ops issues the launch; outcome:
   - launch-success + first `game_running` heartbeat → V1↔V2 bridge → `WalletService::capture(hold_id, actual_credits=initial_minute_charge, redeemed_for)` (NEW W3) → `INSERT wallet_redemptions` (initial min) + state=ACTIVE_CAPTURED.
   - launch-fail (telemetry timeout / acs.exe crash / etc.) → bridge → `WalletService::release_hold(hold_id, reason)` (NEW W3) → `UPDATE wallets SET balance_credits = balance_credits + held_credits, held_credits = 0` + state=RELEASED. Bonus credits stay distinguishable via wallet_topups source-tag (Q5-c hybrid).
4. **Mid-session** → per-minute charge accrues via `WalletService::extend_capture(session_id, additional_credits)` (NEW W3 sibling, optional — could be deferred to W3-S5 sub-step) OR remains as a single end-of-session capture amount.
5. **Session end** (normal / early / staff cancel / hardware fail / game crash / low-balance) → `WalletService::finalize_capture(hold_id, actual_credits, redeemed_for)` (NEW W3) → state=FINALIZED + audit + idempotency-key recorded.
6. **Refund-during-HOLD** (W1-S3 3-band) → `release_hold` if HOLD state, then refund flows via existing W1-S3 routing — NO direct interaction with capture state. (Q-W3-12)
7. **Idle-timeout** (K5 fixed-window today / W1-S5 sliding-window if pulled forward) → staff session expires; if HOLD is OPEN, must NOT auto-release (customer is still in pod) — release happens via session terminal-state, not auth boundary. (Q-W3-13)

### Schema / state surfaces

- **`wallets.held_credits`** (NEW, if Q-W3-2 = inline column path) — INTEGER NOT NULL DEFAULT 0 CHECK (held_credits >= 0). Atomic guard on hold preserves "balance + held = total acquired credits" invariant.
- **`wallet_holds`** (NEW table, if Q-W3-2 = separate-table path) — schema:
  ```sql
  CREATE TABLE wallet_holds (
      id              TEXT PRIMARY KEY NOT NULL,
      wallet_id       TEXT NOT NULL REFERENCES wallets(id) ON DELETE RESTRICT,
      session_id      TEXT NOT NULL REFERENCES sessions(id) ON DELETE RESTRICT,
      customer_id     TEXT NOT NULL REFERENCES customers(id) ON DELETE RESTRICT,
      credits_held    INTEGER NOT NULL CHECK (credits_held > 0),
      status          TEXT NOT NULL DEFAULT 'pending'
                      CHECK (status IN (
                          'pending','active_captured','released','finalized'
                      )),
      release_reason  TEXT,
      idempotency_key TEXT,           -- composes-with `idempotency_keys` table; nullable until W3 ships idempotency wave
      created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
      captured_at     TEXT,
      released_at     TEXT,
      finalized_at    TEXT,
      version         INTEGER NOT NULL DEFAULT 0  -- optimistic locking per PACT-024 Q1-a
  );
  CREATE INDEX idx_wallet_holds_wallet  ON wallet_holds(wallet_id);
  CREATE INDEX idx_wallet_holds_session ON wallet_holds(session_id);
  CREATE INDEX idx_wallet_holds_open    ON wallet_holds(status) WHERE status IN ('pending','active_captured');
  -- NEVER-DELETE trigger pattern from migration 002 sibling extension
  ```
- **`idempotency_keys`** (NEW table per PACT-024a §A.1.2) — per-event-type unique + forensic-indexed; partial index on reconciliation_required.
- **`wallets.version`** (NEW) — optimistic-lock predicate column per PACT-024a §A.1.1 (or absorbed into wallet_holds.version if separate-table path).

### Configuration surfaces

- `idempotency_cache_ttl_secs` (NEW) — Q3-c hybrid window; default 86400 (24h). Captain Q-W3-5.
- `optimistic_lock_max_retries` (NEW) — bounded backoff retry count; default 3. PACT-024a §A.2.1.
- `optimistic_lock_backoff_ms_base` (NEW) — backoff base; default 10ms with `* 2^attempt + jitter`.
- `hold_timeout_secs` (NEW, optional W3-S sub-step) — orphan-hold sweep threshold for PACT-024b reconciliation worker; default deferred to PACT-024b spec.

---

## §2 — Inherited-issue catalogue

Issues at this boundary, drawn from V1 failure-mode investigation + commit-log + LOGBOOK + ledger anchors + this branch's just-shipped W1 substrate.

| ID | Source | Issue | Scope at this boundary |
|---|---|---|---|
| F-05 (P1, 2026-03-28) | `ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` + LOGBOOK 2026-04-07 | V1 `end_billing_session` overwrites `wallet_debit_paise` at line 2213 BEFORE reading at line 2255 for refund calc. Customer loses ₹162.50 per early-end. Survived 32+ model audits. | DIRECT-CRITICAL — W3 HOLD-RELEASE-CAPTURE state machine MUST NOT introduce a similar UPDATE-then-SELECT-same-column pattern. Specifically, capture's UPDATE wallets + INSERT wallet_redemptions must read `credits_held` BEFORE updating it; or reconcile delta math must occur on snapshot values not post-UPDATE values. |
| W1-S2 wallet_redemptions FK dangling (NF-james-4, 2026-05-08) | `crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql` | `ALTER TABLE RENAME` rewrote FK targets in sibling tables (PACT-018 migration 003); wallet_redemptions FK pointed at dropped `sessions_old_pact018`. Class: SQLite recreate-table pattern requires sibling-table rebuild in same migration. | DIRECT — W3 migrations adding `wallet_holds` table or `wallet_topups`-touch-from-hold paths must apply same recreate-table discipline; if any sibling table later renames, FK from wallet_holds must be rebuilt. |
| W1-S2 no idempotency key | `crates/v2-db/src/wallets.rs:155-159` docstring | "Wave 1 kaizen-min: NO idempotency key (deferred to Wave 3 PACT-024). Concurrent reserve calls for the same customer rely on SQLite single-writer + WHERE-guarded predicate" | DIRECT — W3 closes this gap; idempotency_keys table + per-event-type unique key construction + 24h cached-replay. |
| PACT-024 status FILED-AWAITS-AMPLIFIER | `comms-link/proposals/PACT-20260504-024-wallet-concurrency-idempotency-extension.md:9` | bono LEAD; james AMPLIFIER vote on Q1-Q5 outstanding 5 days. Substrate-class auto-ratify gate per §2.1 OPTION-A composite-#4 path-c gates on AMPLIFIER landing. | DIRECT-CRITICAL — W3 implementation BLOCKED on Q1-Q5 disposition. This RCA proposes bono defaults Q1-a / Q2-c / Q3-c / Q4-c / Q5-c (deferred for Q4) — Captain G33 or james AMPLIFIER must ratify. |
| PACT-024a targets STALE | `comms-link/.planning/ship-plans/PACT-024-024a-wallet-concurrency-substrate-ship-plan.md` §A.2-§A.5 | references `racecontrol/crates/racecontrol/src/wallet.rs` + `billing.rs` + `routes.rs`. W1-S2 (2026-05-08) moved canonical V2 wallet to `crates/v2-db/src/wallets.rs::WalletService`. | DIRECT — W3 adopts PACT-024a §A SQL/Rust verbatim BUT re-targets file paths to v2-db crate. No semantic change — only path bumps. |
| Bonus credit source-tag preservation | PACT-024 §3 Q5 + Wallet Framing C source-tagging | Hybrid (Q5-c) atomic-with-top-up + separate idempotency-key for forensic. Wave 3 must preserve source-tag through hold→capture so refund-fairness retains "what kind of credits did customer redeem" signal. | DIRECT — wallet_redemptions row carries credits_redeemed (homogeneous bucket); audit query for source-traceback flows from redemption row → hold row → top-up row. wallet_holds.id linkage may need persisting in wallet_redemptions (NEW column). |
| T-F1 bonus arbitrage exploit | PACT-024 §1 trigger top-tier (gemini-3.1) | Customer farms credits via forced launch failures with discount applied. Closure depends on this PACT (HRC) landing first. | DIRECT-CRITICAL FOR FUTURE — W3 HRC IS the substrate for T-F1 closure: capture only fires on `game_running` heartbeat, release on launch-fail returns FULL credits including bonus, no discount-on-rebooked-credit. Future T-F1 PACT extends but doesn't replace W3. |
| §AMEND-3.II D12 Foundation/Strategy/Config separation (F25a precedent) | F25a substrate `9f1c0a37` | Strategy trait for substitutable behavior. F25a `BillingStrategy` precedent. | COULD-APPLY — `WalletStateStrategy` trait abstracting hold-vs-immediate-debit. KAIZEN says only-2-strategies-no-third = direct logic; W3 has only 1 strategy (HRC) — overscope per kaizen. Captain decision (Q-W3-trait). |
| K1 §AMEND-1.E refund reason-code enum | Captain §S-79 (7f193030 + NF-bono-1 26677e42) | 12×4 reason-code enum for refunds (W1-S3+S4 shipped). | COMPOSES-WITH — `wallet_holds.release_reason` may want to share a controlled vocabulary. Sub-question: does release_reason need to be a tight enum or open-string? Recommendation: tight enum aligned with K1 reason-codes + new HRC-specific reasons (e.g. `launch_failed_pre_run`, `staff_cancel_pre_run`, `customer_cancel_pre_run`). |
| Cloud-venue debit sync (D-20) | `crates/racecontrol/src/cloud_sync_debit.rs` | Cloud→venue process_debit_intents sync (currency_type-aware post Phase 338-01). PACT-024 §1 names PWA UPI + POS .130 + Kiosk .23 as multi-source top-up surfaces. | INDIRECT-OPEN — Q-W3-9 captures whether HOLD state is venue-only OR cross-environment (cloud PWA initiates HOLD via comms-link relay). Default: venue-only (PWA top-up flows separately into V2 wallet_topups but HOLD is venue-bound). |
| K5 fixed-window idle-timeout (PR #64 ship) + W1-S5 sliding-window (DRAFT just upstream) | `crates/racecontrol/src/auth/middleware.rs:103-114` + `W1-S5-RCA.md` §5 | staff JWT idle-timeout interaction with active customer HOLDs. If staff session expires mid-customer-pod-session, customer's hold must NOT auto-release (customer is still racing). | INDIRECT — W3 release path is gated on session terminal-state (game-running stop / launch-fail / staff cancel), NOT staff auth state. Document this invariant in W3 spec. Captain decision (Q-W3-13). |
| Refund-during-HOLD scenario | W1-S3 refund 3-band | Customer requests refund while HOLD is active (game launching, mid-launch, etc.). | INDIRECT — W3 spec must define the order: release_hold first, then refund flows via W1-S3 (which composes with reason-codes). Refund cannot fire AFTER capture without going through W3 capture-reversal path (NEW W3-S sub-step OR deferred to W3.1). |
| Wallet Framing C: cafe always separate | `project_v2_wallet_framing_c_locked_20260503.md` | Cafe orders never touch credit balance. POS hard-blocks credit-for-cafe. | NOT-APPLICABLE-DIRECTLY — W3 redeemed_for CHECK ('sim','ps5') in v2-db migration 001 already enforces. W3 does not extend to cafe. |
| Wallet Framing C: credits never expire customer-facing | doctrine | Internal breakage at 24-36 months Ind AS 115; never customer-facing forfeiture. | NOT-APPLICABLE-DIRECTLY — W3 HRC operates on active credits; expiry is dormancy-class, separate concern. Confirm wallet_holds does NOT introduce hold-expiry that returns credits to dormant pool (must always return to customer balance_credits). |
| §S-61 V1 process mess audit categories | `briefings/<pilot>/memory/session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` | 14 mapped failure modes A-J. None directly map to wallet/billing concurrency (V1 had single-source POS billing; cloud sync was bolt-on); adjacent class: V1 broadcast-storm touches WS + cloud sync timing. | NOT-APPLICABLE-DIRECTLY — W3 wallet HRC sits in DB-transaction layer not WS layer; broadcast-storm class doesn't propagate. |
| Phase 338-02 D-14 max_cash_refund TOCTOU | `crates/racecontrol/src/wallet.rs:92-94` (V1) | TOCTOU-safe cap check inside-tx; clamp `[0, balance_paise]` on raw cash-refund-cap. | NOT-APPLICABLE-DIRECTLY-TO-V2 — V2 uses single balance_credits and no rupee/credit split; cash refund concept doesn't apply (V2 redeemable for sim/PS5 only, not cash refundable). PRESERVES the lesson: cap-check inside-tx is the right pattern, ported to W3 release_hold (release amount must equal credits_held inside-tx). |

---

## §3 — Past-bug disposition

Per doctrine §"Disposition each past bug":

| Past bug at boundary | Disposition | Evidence |
|---|---|---|
| F-05 (UPDATE-then-SELECT same column) | **PATCHED-ONLY** (V1 cash_refund/get_max_cash_refund inside-tx pattern, Phase 338-02 commit `3e3779eb`+`307ee0d8`+`1ec42704` 2026-04-07). Structural anti-pattern (read-after-write same column) is NOT enforced anywhere in the codebase. **W3 must NOT reintroduce.** | `ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md`; LOGBOOK 2026-04-07 entries |
| W1-S2 wallet_redemptions FK dangling | **ROOT-CAUSED-AND-FIXED** (`20260508000001_wallet_redemptions_fk_repair.sql`); class lesson encoded in racecontrol CLAUDE.md "SQLite ALTER TABLE RENAME rewrites foreign-key references in OTHER tables" rule | Migration commit; CLAUDE.md update same session |
| W1-S2 no idempotency key | **OPEN-RCA-ITEM — closed by W3** | `crates/v2-db/src/wallets.rs:155-159` docstring explicitly defers |
| PACT-024 awaits james AMPLIFIER | **OPEN-RCA-ITEM — gates W3 implementation** | PACT-20260504-024 status `FILED-AWAITS-AMPLIFIER` |
| PACT-024a targets stale | **OPEN-RCA-ITEM — closed by Q-W3-RECONCILE-1** | Path mismatch enumerated in §1 above |
| Bonus credit source-tag through HRC | **OPEN-RCA-ITEM — closed by W3 design (NEW)** — wallet_redemptions row needs hold_id linkage OR forensic query path through wallet_holds; previously unspecified | PACT-024a §A only addresses idempotency-keys table; doesn't address bonus-tag through hold |
| T-F1 bonus arbitrage exploit | **NOT-YET-CLOSED — gates on W3 landing** | PACT-024 §1 top-tier; future PACT post-W3 |
| §AMEND-3.II D12 (Foundation/Strategy/Config) | **NOT-APPLICABLE-TO-W3 (kaizen overscope)** — only 1 strategy (HRC); F25a precedent has 2 (snap/proportional refund) which justified the trait. Document the choice in W3 spec. | F25a `9f1c0a37` ship; kaizen-discipline rule |
| K1 reason-codes (W1-S3+S4) | **ROOT-CAUSED-AND-FIXED** — composes with `wallet_holds.release_reason` enum; NEW HRC-specific reason-codes added | W1-S3 ship 7f193030 + NF-bono-1 26677e42 |
| Cloud-venue debit sync (D-20) | **NOT-APPLICABLE-TO-W3-DEFAULT** — W3 default scope is venue-only HOLD (Q-W3-9). Cross-environment HOLD coordination is PACT-024c future sibling. Document the boundary; do not propagate V1 cloud-sync complexity into W3. | `crates/racecontrol/src/cloud_sync_debit.rs` |
| K5 fixed-window / W1-S5 sliding-window idle-timeout | **OPEN-RCA-ITEM — design decision in W3 spec** — Q-W3-13: HOLDs are NOT auth-bound; release fires on session terminal-state. Cross-link to W1-S5 RCA `15490644`. | `auth/middleware.rs:103-114`; `W1-S5-RCA.md` §1 |
| Refund-during-HOLD scenario | **OPEN-RCA-ITEM — design decision in W3 spec** — Q-W3-12: refund must release_hold first, then route via W1-S3 3-band; capture-reversal is W3.1+ scope | W1-S3 substrate `0386db62` |
| Wallet Framing C cafe-credit block | **ROOT-CAUSED-AND-FIXED** — schema CHECK in v2-db migration 001 `redeemed_for IN ('sim','ps5')` | `20260503000001_initial_schema.sql:46` |
| Wallet Framing C credits-never-expire-customer-facing | **ROOT-CAUSED-AND-FIXED in doctrine** — W3 release path always returns credits to balance_credits, NOT to dormant pool; W3 spec confirms invariant | doctrine + W3 spec §5 below |
| Phase 338-02 D-14 TOCTOU pattern | **PATTERN-PRESERVED** — port the inside-tx cap-check lesson to W3 release_hold (released_credits must equal hold.credits_held read inside-tx, not external) | wallet.rs:92-94 V1 reference |

**Open RCA items to resolve in W3 design (per doctrine):**

1. **PACT-024 Q1-Q5 dispositions** (Q-W3-RECONCILE-3) — CRITICAL; gates W3 implementation entry
2. **PACT-024a target re-aim** to v2-db crate (Q-W3-RECONCILE-1) — file-path bump only
3. **wallet_holds inline-column vs separate-table** (Q-W3-2)
4. **Bonus credit source-tag through HRC** — wallet_redemptions row schema decision
5. **Refund-during-HOLD ordering** (Q-W3-12)
6. **Idle-timeout-during-HOLD ordering** (Q-W3-13)
7. **Cross-environment HOLD scope** (Q-W3-9)

---

## §4 — V2-alignment delta

### What V2 doctrine says the boundary should look like

| V2 anchor | Statement | Current alignment |
|---|---|---|
| `project_v2_wallet_framing_c_locked_20260503.md` | "Atomic on canonical wallet" + "Credits never expire customer-facing" + "Cafe always separate" + "18% GST at top-up" | ALIGNED — W1-S2 substrate carries forward; W3 preserves all four invariants (HOLDs return-to-balance not return-to-dormant; no cafe extension; no top-up-time GST change) |
| `PACT-20260504-024-wallet-concurrency-idempotency-extension.md` (FILED) | Optimistic lock + idempotency keys + atomic Launch-Commit-Rollback escrow | NOT ALIGNED — W1-S2 is single-step optimistic UPDATE without explicit hold or escrow; W3 closes this |
| `PACT-024-024a-wallet-concurrency-substrate-ship-plan.md` §A | SQL migration + Rust helpers + cross-surface client wrappers | ALIGNED-FUTURE (after Q-W3-RECONCILE-1 path-bump); the §A SQL/Rust patterns are the implementation plan W3 adopts |
| `PHASE-1-WAVE-1-PLAN.md` row 30 + line 43 | "HOLD-RELEASE-CAPTURE state machine + idempotency cascade — Wave 3 (PACT-024 sibling)" | ALIGNED-FUTURE — this RCA is the gate-precursor |
| `feedback_v2_doctrine_alignment_drift_g9_pact_20260503_002.md` (V2-MASTER-STATE canonical-source ledger) | All V2 state changes go through ledger | NEEDS-LEDGER-ROW — W3 disposition lands in V2-MASTER-STATE §S-N at PR-open time |
| `project_v2_customer_workflows_consolidated_20260503.md` | 5 base + 6 missed customer scenarios | ALIGNED — Scenario 1 (kiosk billing close) + Scenario 2 (PWA top-up) + Scenario 4 (multi-source POS top-up) all benefit from W3; HRC is invisible to customer (state machine internal) |
| `feedback_kaizen_discipline_dont_complicate.md` | Smallest invariant for observed requirement | RISK — W3 introduces a state machine + new table + idempotency table + cross-pilot wrappers. Justification: PACT-024 §1 trigger evidence (5/5 mid-tier MMA + top-tier T-F1 corroboration) + concurrent-close empirical risk = NOT overscope |
| `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (THIS doctrine) | RCA before action | THIS DOCUMENT is the RCA; satisfies the gate once Captain G33 + bono AMPLIFIER + MMA Step 1 land |
| `feedback_emergent_directed_spend_protocol.md` Rule 4 (specify-codebase-identity) | Don't substitute mental model for environment | OK — every claim in this RCA cites a path/line/commit; W1-S2 v2-db move was verified by Read |
| §AMEND-3.II D12 Foundation/Strategy/Config | Strategy classes for substitutable behavior | NOT-APPLICABLE-TO-W3 — only 1 strategy (HRC); kaizen says no trait until ≥2; document choice |
| W1-S5 sliding-window idle-timeout RCA `15490644` (just upstream) | Auth boundary RCA pattern | COMPOSES-WITH — W3 RCA mirrors structural template; cross-link Q-W3-13 |
| F25a BillingStrategy substrate `9f1c0a37` (MERGED to main) | V2 doctrine alignment via Strategy trait + HISTORICAL block + behavior parity tests | COMPOSES-WITH — pattern-precedent for V1↔V2 boundary handling: V1 SnapPricing preserved as known strategy; V2 ProportionalRefund added; tests prove parity. W3 analog: W1-S2 reserve_credits PRESERVED-AS-V1-WAVE-1-COMPATIBILITY-LAYER while W3 hold/release/capture is the V2 canonical |

### Named gaps

- **Gap-1:** W1-S2 ships single-step optimistic UPDATE without hold state. W3 introduces explicit `wallet_holds` schema + state machine.
- **Gap-2:** No `game_running` heartbeat → V2 capture trigger today. V1 `billing_game_status.rs` has the heartbeat; W3 introduces V1↔V2 bridge module.
- **Gap-3:** PACT-024a's targets stale (racecontrol/wallet.rs vs v2-db/wallets.rs). Re-target.
- **Gap-4:** `idempotency_keys` table per PACT-024a §A.1 not yet shipped; W3 ships bundled.
- **Gap-5:** Cross-environment (cloud PWA + venue) HOLD-RELEASE coordination not specified. Captain decision Q-W3-9; default venue-only.
- **Gap-6:** Refund 3-band (W1-S3) interaction with HOLD is not specified; W3 spec defines order release_hold→W1-S3 routing.
- **Gap-7:** K5 / W1-S5 sliding-window idle-timeout interaction with active HOLDs not specified; W3 spec confirms HOLDs are session-bound NOT auth-bound.
- **Gap-8:** Bonus credit source-tag through HRC requires wallet_redemptions row schema decision (NEW hold_id column or wallet_topups-walk for source).
- **Gap-9:** F-05 anti-pattern (UPDATE-then-SELECT same column) not codified as a lint or test pattern; W3 capture path is at-risk if author uses naïve copy-from-W1-S2.
- **Gap-10:** Wave 3 implementation must NOT introduce auto-cleanup of orphan holds during W3 ship — orphan-cleanup is PACT-024b "Kidneys" reconciliation worker scope (sibling).

---

## §5 — V2-framed proposal

**V2 doctrine alignment:** This change moves the wallet boundary from V1-era rupee/credit-mixed driver_id-scoped flows + W1-S2 single-step optimistic UPDATE → V2 customer_id-scoped Single-Purpose-Voucher Wallet-Framing-C HOLD-RELEASE-CAPTURE state machine with PACT-024 idempotency contract. It closes Wave 3 of the V2.0 6-wave plan and unblocks future T-F1 bonus-arbitrage exploit fix. Pattern-precedent: F25a BillingStrategy substrate (MERGED `9f1c0a37`) — V1 known-strategy preserved + V2 canonical added + behavior parity tests.

### Implementation sketch (kaizen-min, minus §AMEND-3.II D12 trait per Gap; minus W3.1 capture-reversal per kaizen)

W3 ships in 13 sub-steps grouped into 4 commits/PRs (per Q-W3-10 split per-PR Captain auth):

**PR-A: Schema + idempotency substrate**

1. **Migration `2026XXX_wallet_holds_and_idempotency_keys.sql`:**
   - CREATE TABLE wallet_holds (per §1 schema sketch, per Q-W3-2 default = separate-table)
   - CREATE TABLE idempotency_keys (PACT-024a §A.1.2 verbatim, ported to V2 SQLite)
   - 3 indexes per §1
   - NEVER-DELETE trigger pair via migration 002 sibling extension (audit retention)
   - ALTER TABLE wallets ADD COLUMN version INTEGER NOT NULL DEFAULT 0 (optimistic lock per Q1-a)
   - LOC: ~80-120 SQL

2. **`crates/v2-db/src/wallet_holds.rs` NEW** (or extend `wallets.rs` per architectural call):
   - `WalletHold` struct + `HoldStatus` enum + `WalletHoldsService` (mirrors `WalletService`)
   - LOC: ~80-120 Rust

3. **`crates/v2-db/src/idempotency.rs` NEW:**
   - `IdempotencyOutcome` enum (FreshKey / CachedReplay / Expired409)
   - `check_idempotency_key`, `record_idempotency_key` (PACT-024a §A.2.2/A.2.3 verbatim, ported to v2-db Error type)
   - `with_optimistic_retry` wrapper (PACT-024a §A.2.1)
   - LOC: ~120-180 Rust

4. **`crates/v2-db/src/lib.rs::Error` extensions:**
   - `HoldNotFound { hold_id }`, `HoldAlreadyCaptured { hold_id }`, `HoldAlreadyReleased { hold_id }`, `IdempotencyKeyExpired`, `VersionConflict`, `MaxRetriesExceeded`
   - LOC: ~20

5. **Tests for PR-A:**
   - Unit: optimistic-lock retry success/failure, idempotency cached replay, idempotency 409 expiry, hold state transition rules
   - Integration: concurrent hold+release on same wallet, FK enforcement, NEVER-DELETE trigger
   - LOC: ~200-300

**PR-B: Hold/release/capture state machine on `WalletService`**

6. **`WalletService::hold(customer_id, session_id, credits, idempotency_key) -> Result<HoldId, Error>`:**
   - check_idempotency_key (FreshKey / CachedReplay branch)
   - with_optimistic_retry on wallet UPDATE (`balance_credits >= ?` guard)
   - INSERT wallet_holds (status=pending, version=0)
   - record_idempotency_key (success outcome)
   - Returns hold_id
   - **F-05 anti-pattern guard:** read snapshot, compute new values from snapshot, UPDATE, then INSERT — NEVER read-after-write the same column
   - LOC: ~70-100 Rust

7. **`WalletService::release_hold(hold_id, reason) -> Result<i64 (new_balance), Error>`:**
   - Read hold with version
   - Validate status ∈ {pending, active_captured} → ERR HoldAlreadyReleased / HoldAlreadyCaptured
   - with_optimistic_retry: UPDATE wallets balance_credits += hold.credits_held WHERE version = hold.version (version bump on hold + wallet)
   - UPDATE wallet_holds status='released', released_at=now, release_reason=?
   - LOC: ~60-90 Rust

8. **`WalletService::capture(hold_id, actual_credits, redeemed_for) -> Result<i64 (new_balance), Error>`:**
   - Read hold + wallet snapshot (TOCTOU-safe inside-tx; F-05 lesson)
   - Validate status=pending → ERR HoldAlreadyCaptured if active_captured/finalized; ERR HoldAlreadyReleased if released
   - delta = hold.credits_held - actual_credits (overage refund or underage debit; reuse W1-S2 reconcile_redemption logic but on hold-bound math, NOT wallet snapshot post-update)
   - INSERT wallet_redemptions (hold_id NEW column linkage; preserves bonus source-tag walk)
   - UPDATE wallet_holds status='active_captured' (or 'finalized' if final-capture variant)
   - UPDATE wallets balance_credits per delta
   - LOC: ~80-120 Rust

9. **`WalletService::reserve_credits` REFACTOR** — thin wrapper around `hold` that immediately captures with actual=reserved (preserves Wave 1 caller compatibility); add deprecation TODO comment with migration target Wave 4 caller transitions to explicit hold/release/capture
   - LOC: ~10 (just delegation)

10. **Tests for PR-B (per PACT-024a §B.1+B.2):**
    - `hold_optimistic_atomic_under_concurrency`
    - `release_hold_returns_credits_to_balance`
    - `capture_overage_refunds_via_reconcile`
    - `capture_underage_debits_via_reconcile`
    - `idempotency_cached_replay_within_24h`
    - `idempotency_expired_returns_409`
    - `version_conflict_triggers_retry`
    - `external_resource_consumed_flags_reconciliation` (PACT-024b feed)
    - `bonus_source_tag_preserved_through_hrc` (NEW W3 invariant)
    - `f05_anti_pattern_regression_check` (read-after-write-same-column does NOT exist)
    - LOC: ~400-500

**PR-C: V1↔V2 bridge — game_running heartbeat + W1-S3 refund integration**

11. **`crates/racecontrol/src/v2_bridge/wallet_hrc_bridge.rs` NEW:**
    - Subscribe to V1 game_launcher state events
    - On launch-success + first game_running heartbeat → call `WalletService::capture` against V2 hold
    - On launch-fail (acs.exe crash, telemetry timeout, hardware fail) → call `WalletService::release_hold` with reason-code mapped from V1 fail-reason enum
    - On staff cancel pre-launch → release with `staff_cancel_pre_run`
    - LOC: ~150-200 Rust

12. **W1-S3 refund-during-HOLD extension** — `refund_routing.rs` adds branch: if session has open hold, release first then route via 3-band; if hold is captured, refund flows existing path (Wave 3.1 capture-reversal is OOS for W3)
    - LOC: ~30-50

**PR-D: Cross-pilot client wrappers + observability + DEPLOY MANIFEST refresh**

13. **bono-side: PWA + POS + Kiosk client wrappers** for Idempotency-Key header propagation per PACT-024a §A.4 (`shared/wallet-client.js` NEW)
    - `generateIdempotencyKey(eventType, scope)` factory
    - `walletTouch()` wrapper that adds header + handles 409
    - bono-LEAD per PACT-024a; cross-pilot scope; W3 PR-D DEPENDS on bono shipping this in parallel
    - LOC: ~80-120 JavaScript

14. **Observability:**
    - New tracing spans on hold/release/capture per request
    - Per-customer hold-active count exposed via /api/v1/admin/wallets/holds/open (NEW staff-protected endpoint)
    - `wallet.holds.active` and `wallet.holds.captured` and `wallet.holds.released` Prometheus counters
    - LOC: ~50-80

15. **Memory + DEPLOY MANIFEST + LOGBOOK + V2-MASTER-STATE row** per Wave 0 PR #64 model + W1-S5 ship pattern

### Estimated size

- Migrations: ~80-120 LOC SQL
- Production code: ~250-380 LOC Rust + ~80-120 LOC JS (bono-side)
- Tests: ~400-500 LOC
- Documentation: 6 memory files + LOGBOOK + V2-MASTER-STATE row + DEPLOY MANIFEST + per-PR PLAN.md docs
- Risk surface: foundational wallet boundary; **MMA Step 1 DIAGNOSE required** (per doctrine; ~$3-5 OpenRouter)
- Estimated session length: **~5-8 hours total** across 4 PRs (PACT-024a §G "2-2.5h" estimate predates W1-S2 v2-db move + cross-environment scope expansion + V1↔V2 bridge module + observability surface)
- Wave 3 PR-cadence: split per Q-W3-10 — each PR gets per-PR Captain merge auth (PROMOTED-N=1 per Wave 0 K1)
- Wave 3 DEPLOY PARITY: Server .23 + Bono VPS racecontrol both rebuild; PWA + POS + Kiosk frontends rebuild for PR-D

### Open Captain Q-DECISIONs surfaced by this RCA

| ID | Question | Default if Captain doesn't disposition |
|---|---|---|
| Q-W3-RECONCILE-1 | Confirm Wave 3 IS PACT-024 + 024a executed under V2.0 6-wave; authorize re-targeting from `racecontrol/wallet.rs` → `v2-db/wallets.rs`. PACT-024a §A SQL/Rust patterns adopted verbatim with file-path bump only. | DEFAULT: Yes per PHASE-1-WAVE-1-PLAN.md row 30 + line 43; W1-S2 placement supersedes PACT-024a §A original target paths |
| Q-W3-RECONCILE-2 | Wave 3 entry timing — AFTER Wave 1 closure (W1-S6/S7/S8 + Q-Gate + E2E) OR pull-forward into W1 tail | DEFAULT: AFTER per V2.0 6-wave order; this RCA is governance pre-author so Wave 3 ships fast when entered |
| Q-W3-RECONCILE-3 | PACT-024 Q1-Q5 dispositions (currently FILED-AWAITS-AMPLIFIER) | DEFAULT: bono recommendations Q1-a optimistic / Q2-c per-event-type+forensic / Q3-c hybrid 24h / Q4-c hybrid (DEFER to PACT-024b) / Q5-c hybrid atomic+separate-key |
| Q-W3-2 | wallet_holds schema: separate `wallet_holds` table OR inline `wallets.held_credits` column? | DEFAULT: separate table per audit-trail clarity + concurrent-holds support (lobby host pays for N pods → N hold rows on one wallet) + matches PACT-024a §A.1.2 idempotency_keys table sibling pattern |
| Q-W3-trait | Adopt §AMEND-3.II D12 `WalletStateStrategy` trait OR keep direct logic per kaizen? | DEFAULT: keep direct (only 1 strategy = HRC; F25a precedent had 2 strategies + future Wave-2 dynamic pricing) |
| Q-W3-9 | Cross-environment HOLD: cloud PWA initiates HOLD via comms-link relay, venue captures? OR venue-only? | DEFAULT: venue-only HOLD initial; PWA top-up still flows via separate path; cross-environment HOLD coordination is PACT-024c future sibling |
| Q-W3-10 | Wave 3 PR-cadence: 4 PRs per §5 split (Schema → Service → Bridge → Cross-pilot) OR fewer/more granular? | DEFAULT: 4 PRs per §5 sketch; per-PR Captain auth at each open |
| Q-W3-12 | Refund-during-HOLD ordering: release_hold-then-W1-S3 vs HOLD-aware refund branch | DEFAULT: release_hold-then-W1-S3 (cleanest; capture-reversal is W3.1+ scope) |
| Q-W3-13 | Idle-timeout-during-HOLD: HOLDs are session-bound NOT auth-bound (idle-timeout does NOT auto-release HOLD); release fires only on session terminal-state | DEFAULT: yes, session-bound; document invariant in W3 spec |
| Q-W3-bonus-tag | wallet_redemptions schema: ADD COLUMN hold_id (NEW FK) for source-tag walk OR derive via session_id→wallet_holds JOIN? | DEFAULT: ADD COLUMN hold_id (single-hop forensic query; FK preserves audit; no JOIN cost) |
| Q-W3-cap-rev | Capture-reversal (refund-after-capture) — W3 scope or defer to W3.1? | DEFAULT: defer to W3.1 (kaizen; existing W1-S3 refund path handles capture-reversal symptomatically via wallet_topups + wallet_redemptions math; full state-machine reversal can wait) |
| Q-W3-orphan-sweep | Orphan-hold cleanup (HOLDs that lost their session via crash) — W3 scope or defer to PACT-024b "Kidneys"? | DEFAULT: defer to PACT-024b reconciliation worker per ship-plan §F |

---

## NOT TESTED (RCA AUTHORING phase — pre-implementation, pre-MMA)

This is an authoring artifact, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is W3 PR-A through PR-D scope; this RCA is the gate-precursor only
- **MMA Step 1 DIAGNOSE on this RCA** — gated on Captain budget approval (~$3-5 OpenRouter); 5-model consensus on root causes per doctrine §"MMA escalation"
- **bono substantive AMPLIFIER on this RCA** — bilateral doctrine; bono review pending. Bono is also LEAD on PACT-024 itself; AMPLIFIER on this RCA is a meta-layer (RCA-of-PACT)
- **james AMPLIFIER on PACT-024 Q1-Q5** — separate doctrine action; until landed, Wave 3 implementation BLOCKED
- **Captain G33 ratification of Q-W3-RECONCILE-1+2+3 and Q-W3-{2,9,10,12,13,trait,bonus-tag,cap-rev,orphan-sweep}** — disposition needed before W3 PR-A can be filed
- **Per-PR Captain merge auth at every W3 PR-open** — gate STANDS for all 4 PRs (foundational wallet boundary)
- **Wave 1 closure** — W1-S5 (this branch's prior commit) + W1-S6 PIN-LOCKOUT + W1-S7 WhatsApp PIN + W1-S8 fallback + Quality Gate + E2E + visual + venue Server .23 rebuild all gate Wave 3 entry per Q-W3-RECONCILE-2 default
- **PACT-024a §A.4 cross-surface client wrappers** — bono-side PWA + POS + Kiosk; W3 PR-D depends on bono shipping in parallel; not yet authored bono-side
- **PACT-024b "Kidneys" reconciliation worker FILE** — sibling-PACT not yet authored; orphan-hold cleanup deferred there per Q-W3-orphan-sweep default
- **Future T-F1 bonus-arbitrage exploit fix PACT** — gates on W3 landing; out of W3 scope
- **Real Razorpay/UPI provider integration** for cross-environment HOLD (Q-W3-9) — venue-only default avoids this; if Captain flips to cross-environment, separate PACT
- **DPDP retention class for `wallet_holds` and `idempotency_keys` tables** — composes-with PACT-013 §S-27 CAVEAT-1 carry-forward; legal/compliance review needed
- **Production-shape concurrent staff request load** (8-pod close + 4-customer top-up + 1-refund storm under MMA-derived stress) — separate workstream gates on PR-B integration test pass
- **Memory-file Universal Sync** for the bono mirror of this RCA — TBD whether RCA artifacts trigger Universal Sync (probably NO; project-planning-doc class, not project-scope feedback rule); flag for Captain confirmation per W1-S5 RCA precedent
- **F-05 anti-pattern lint or test pattern codification** — proposed in §3 disposition; NOT shipped as part of W3 (separate Standing Rule sub-PACT candidate); W3 manually guards via §5.6 design comment

---

## Read trail

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628` 2026-05-09 ~09:28 IST; bilateral both pilots)
- `comms-link/proposals/PACT-20260504-024-wallet-concurrency-idempotency-extension.md` (PACT-024 sibling; bono LEAD; status FILED-AWAITS-AMPLIFIER)
- `comms-link/.planning/ship-plans/PACT-024-024a-wallet-concurrency-substrate-ship-plan.md` (PACT-024a substrate ship plan; pre-authored 2026-05-04 ~17:15 IST; targets stale per Q-W3-RECONCILE-1)
- `crates/v2-db/src/wallets.rs:1-71` (V2 wallet structs + GST helpers; W1-S9 + PACT-013)
- `crates/v2-db/src/wallets.rs:124-338` (W1-S2 WalletService::reserve_credits + reconcile_redemption; SHIPPED 2026-05-08 ~21:30 IST)
- `crates/v2-db/migrations/20260503000001_initial_schema.sql:34-44` (V2 wallets table)
- `crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql` (W1-S2 NF-james-4 FK repair)
- `crates/racecontrol/src/wallet.rs` (V1 wallet; F-05 historical site)
- `crates/racecontrol/src/wallet_refund.rs` (V1 cash_refund + D-14 inside-tx pattern)
- `crates/racecontrol/src/billing_session_end.rs` (V1 end_billing_session; F-05 site)
- `crates/racecontrol/src/billing_session_lifecycle.rs` (V1 dashboard command handler)
- `crates/racecontrol/src/cloud_sync_debit.rs` (V1 cloud-venue sync; D-20)
- `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` row 30 + §1.2 line 43 (W1-S2 placement + Wave 3 deferral)
- `racecontrol/.planning/specs/v2/W1-S5-RCA.md` (W1-S5 sliding-window idle-timeout RCA; structural mirror; just upstream commit `15490644`)
- `project_v2_wallet_framing_c_locked_20260503.md` (Captain-locked Single-Purpose Voucher doctrine)
- F-05 ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md (UPDATE-then-SELECT anti-pattern reference)
- F25a `9f1c0a37` MERGED on main (BillingStrategy V1↔V2 boundary precedent)
- LOGBOOK 2026-04-07 entries (Phase 337/338 wallet rupee/credit separation; Phase 338-02 cash_refund D-14)
- Captain §S-82 (Q1-Q4 PACT-001 dispositions 2026-05-07 ~05:22 IST) + §S-83 (V2.0 6-wave plan) + §S-92 (PARAMETERs P1-P9 STATIC for Wave 1)

---

— james / 2026-05-09 ~10:35 IST · W3 RCA DRAFT authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" 2026-05-09 ~10:07 IST · gates on Captain G33 review of Q-W3-RECONCILE-{1,2,3} + Q-W3-{2,9,10,12,13,trait,bonus-tag,cap-rev,orphan-sweep} + bono AMPLIFIER on this RCA + PACT-024 Q1-Q5 disposition (5 days outstanding) + MMA Step 1 DIAGNOSE before W3 H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at every W3 PR-open (foundational wallet boundary; 4 PRs per §5 default split) · Wave 3 implementation entry gated on Wave 1 closure per Q-W3-RECONCILE-2 default
