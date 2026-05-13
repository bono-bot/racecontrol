---
artifact: §S-146 V1↔V2 RCA · CLUSTER scope
boundary: wallet-ledger / discount-class economic-outcome paths
status: AUTHORED-AWAITING-CAPTAIN-AMPLIFIER-AND-MMA-STEP-1
authored: 2026-05-13 IST
author: bono
boundary-class: foundational-CLUSTER (wallet + billing + DB-schema simultaneous)
mma-step-1: PENDING-Captain-budget-auth (~$0.10-1 OpenRouter; cluster amortizes 5 atoms across 1 MMA)
parent-cascade: §S-252 Captain Q-2-1 ratify (`max_discount_pct = 0.50`) + §S-253/254 row 7.3 substrate (atom 1 of 5; already on feature branch) + §S-256 PRE-SPAWN OPEN-CLAIM
parent-finding: MMA Step 1 DIAGNOSE on row-7.3-alone RCA (2026-05-13 ~17:48 IST, results at `/tmp/mma-row-7.3-results/`) — 4/4 UNANIMOUS BLOCKING gap (DeepSeek R1 + Qwen3 Coder + Gemini 2.5 Pro + Kimi K2.5)
v2-progress-map-rows: 7.3 discount-ceiling · 1.20 top-up bonus ladder · F-05 sibling refund rows · Phase 2-A rate-table-service · Phase 2-E combo-offer · Phase 2-F campaign-object (6-7 rows total)
captain-decisions-needed: MMA-budget Phase β + per-PR merge auth + V1-vs-V2 schema decision (wallet_transactions ALTER OR migrate to v2-db schema)
companion-rca: `RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md` (james-authored §S-248 follow-up; this cluster RCA SUPERSEDES + EXPANDS its scope)
---

# §S-146 V1↔V2 RCA — Wallet-Ledger / Discount-Class CLUSTER

## 1. Boundary map

V2 customer-trust invariant: customer pays no less than (1 - 0.50) × original_price for any session, across ALL economic-equivalent paths. Cluster scope = every V1 path that adds positive wallet balance OR reduces session price, irrespective of whether it's labeled "discount" or "credit" or "refund" or "bonus".

**Path inventory** (per `/root/racecontrol/crates/racecontrol/src/` boundary scan 2026-05-13 ~18:00 IST):

| Path | File:line | Function | Auth | Mechanism | Severity |
|---|---|---|---|---|---|
| **A** topup_wallet | `api/wallet_staff.rs:152-312` | `topup_wallet` | staff JWT | calls `wallet::credit()` for base + 2nd call `txn_type="bonus"` (ladder pct) | LOW for ceiling · HIGH for taxonomy |
| **B** gateway webhook | `api/wallet_gateway.rs:41-212` | `payment_gateway_webhook` | `payment_webhook_secret` header (TODO: full HMAC-SHA256 verification incomplete; only "secret is set" structural guard) | calls `wallet::credit_in_tx()` with `txn_type="gateway_topup"`; cap `10_000_00` (₹10,000) per call | **HIGH — pre-merge blocker** |
| **C** refund_wallet (unreferenced) | `api/wallet_ops.rs:337-359` | `refund_wallet` non-`reference_id` path | staff JWT (no `require_role` at handler) | `wallet::credit()` with `txn_type="refund_manual"`; hardcoded ₹500 cap per call; no daily accumulator | MEDIUM |
| **D** approve_incentive_bonus | `api/billing_views.rs:96-152` | `approve_incentive_bonus` | staff JWT | `wallet::credit_wallet()` with `txn_type ∈ {"review_bonus","follow_bonus"}`; hardcoded ₹50/₹25; per-driver flag (one-time) | LOW |
| **E** refund_wallet (referenced) | `api/wallet_ops.rs:247-334` | `refund_wallet` `reference_id` path | staff JWT | **raw `UPDATE wallets SET balance_paise = balance_paise + ?` bypassing `wallet::credit_in_tx` abstraction**; cap `custom_price.max(cost).max(MAX_MANUAL_REFUND_PAISE)` = always ≥ ₹5,000 regardless of actual session cost | **HIGH — pre-merge blocker** |

**Existing discount-policy primitives** (composition surfaces for cluster cap):

| Primitive | File:line | Value | Call-sites |
|---|---|---|---|
| STAFF-01 `DISCOUNT_APPROVAL_THRESHOLD_PAISE` | `billing_pricing.rs:144` | `5000` (₹50) | `billing_discount.rs:79` (only) |
| FATM-10 `DISCOUNT_FLOOR_PAISE` | `billing_pricing.rs:149` | `0` (disabled) | `billing_discount.rs:173`, `billing_start.rs:227` + response fields |
| MAX_DISCOUNT_PCT_DEFAULT (§S-253/254) | `pricing/discount_ceiling.rs:25` | `0.50` (Captain-ratified §S-252) | `billing_start.rs:200-220`, `billing_discount.rs:148-170` |

**Composition ordering** (this cluster commits to convention): **ceiling BEFORE floor** at billing call-sites. Existing substrate (§S-253/254) follows this convention. MAOR Tier-1 verified the empirical equivalence with negative-guard in place. Documented in `billing_start.rs:201` comment.

**Critical schema observation:** No `wallet_debits` table exists. V1 uses `wallet_transactions` with signed `amount_paise` (negative = debit) + `txn_type` string discriminator. The row-7.3-alone RCA §4.C proposed `ALTER wallet_debits ADD ...` — that table doesn't exist. The cluster RCA must decide: (a) ALTER V1 `wallet_transactions`, OR (b) write audit columns to a NEW table (e.g., `discount_clamp_events`), OR (c) migrate to V2 `v2-db/migrations/20260503000001_initial_schema.sql` wallet schema. **Captain decision queue D-CLUSTER-1.**

**Audit-trail surface:** `audit_log` (`migrate_policy.rs:21-34`) with `action_type='discount_applied'` is the canonical discount-event ledger; `billing_audit_log` records FSM transitions only. The cluster substrate must stamp clamp-events here OR add a sibling `discount_clamp_events` table.

## 2. Inherited-issue catalogue

V1 wallet-ledger / discount-class failure-class touching this cluster boundary:

| # | V1 failure class | Surface | Severity |
|---|---|---|---|
| I-1 | **STAFF-01 manager-approval gate is 2-state, not continuous cap** — above threshold = manager-code-required; below = free hand; cap = unbounded (manager can approve any amount); collusion vector unbounded | `billing_discount.rs:82-122` | MEDIUM (pre-existing; ceiling+floor compose to bound) |
| I-2 | **FATM-10 floor is 0 in production** — no minimum-payable enforcement today; floor (FATM-10) ≠ ceiling (MAX_DISCOUNT_PCT) primitive distinction by name | `billing_pricing.rs:149` | LOW (Captain can flip non-zero post-Phase-2-Pricing-Calibration) |
| **I-3** | **Path B (`payment_gateway_webhook`) HMAC verification incomplete** — TODO on full HMAC-SHA256; only "secret-is-set" structural guard; attacker who knows secret + endpoint can POST arbitrary `amount_paise` ≤ ₹10K per call; ceiling primitive does NOT clamp this path | `api/wallet_gateway.rs:41-212` | **HIGH — BLOCKING** (MMA Step 1 4/4 unanimous flagged this class) |
| **I-4** | **Path E (`refund_wallet` referenced) raw UPDATE bypasses `wallet::credit_in_tx`** — directly writes `wallets.balance_paise`; cap formula `custom_price.max(cost).max(MAX_MANUAL_REFUND_PAISE)` = always ≥ ₹5,000 regardless of session cost (50× actual ₹100 session) | `api/wallet_ops.rs:247-334` | **HIGH — BLOCKING** (MMA Step 1 Qwen I-9 + Kimi Q1.b composes here) |
| I-5 | **Path B `gateway_topup` taxonomy gap** — `is_topup = txn_type.starts_with("topup_")` (line 147 `credit_in_tx`); `"gateway_topup"` starts with `"gateway_"` NOT `"topup_"` → `rupee_deposited_paise` is NOT incremented for gateway credits → cash-refund cap (`max_cash_refund`) does NOT account for gateway-deposited funds | `wallet.rs:132-148` (`credit_in_tx`) | MEDIUM (rounds-trip-loss class; auditable but quietly off-books) |
| I-6 | **Bonus-vs-discount taxonomy convention-enforced, not type-enforced** — `wallet::credit()` uses `txn_type` STRING to route bonus vs topup; caller passing `txn_type="adjustment"` is classified as bonus indistinguishably | `wallet.rs:192, 232-248` | MEDIUM (MMA Qwen I-10 + Kimi Q1.b finding) |
| I-7 | **`wallet_transactions.txn_type` CHECK constraint narrower than actual usage** — original CREATE TABLE CHECK lists 12 types; usage includes `gateway_topup`, `refund_cash`, `bonus_registration/review/follow` (NOT in CHECK); SQLite CHECK added via ALTER NOT retroactively enforced; CHECK enforces on NEW DBs but not existing | `migrate_billing.rs:316` | MEDIUM (V1↔V2 schema-migration class) |
| I-8 | **Staff-discount re-application after refund (Qwen I-9 vector — staff-discount sub-path)** — coupon path CLOSED via `coupon_redemptions` persistence; **staff-discount path OPEN** — no per-driver daily accumulator on `staff_discount_paise` field at session start | `api/billing_start.rs:189-197` + `api/billing_discount.rs` | MEDIUM (Qwen I-9 finding sub-vector; staff approval already required above ₹50 but cumulative-per-day unbounded) |
| I-9 | **Kimi audit-integrity 2PC gap** — no distributed-transaction boundary ensuring `clamp()` + `wallet_transactions` row write are atomic under network partition; current substrate at `pricing/discount_ceiling.rs` is pure function but DB INSERT can succeed-while-clamp-result-not-logged | `billing_discount.rs:181-220` UPDATE + `wallet.rs::credit_in_tx` | LOW-MEDIUM (1/4 MMA flagged; existing UPDATE+`accounting::log_admin_action` is best-effort; full 2PC out of scope for Phase 1) |
| I-10 | **MMA-substitute attribution drift (RCA-row-7.3 I-6 carried forward)** — ceiling primitive is doctrine-encoded constant in code (not Captain-decision-per-campaign) so this class is structurally PATCHED-BY-DESIGN | `pricing/discount_ceiling.rs:25` | NOT-APPLICABLE-TO-V2 (already mitigated by const-in-code design) |
| I-11 | **Cloud-venue config drift (Kimi Q2.d)** — `state.config.discount_ceiling_pct` future-override-hook loaded from local node config without consensus sync; if both sides have rows with different values → split-brain clamp on cross-node-resumed sessions | future override seam | LOW-MEDIUM (POST-MERGE-PATCH per Kimi; today `max_discount_pct(&state)` returns const so this is forward-only concern) |
| I-12 | **Customer-touching priority class** (RCA-row-7.3 I-5 retained) — DoD §3.5 customer perceives final price; bypass via Path B/E = customer pays Rs.7 on what should be Rs.500 session = clear policy violation | wallet → leaderboard → customer | **PRIORITIZES CLUSTER** (drives BLOCKING severity classification) |

## 3. Past-bug review

| # | Issue | Disposition | Cite |
|---|---|---|---|
| I-1 STAFF-01 2-state gate | **PATCHED-ONLY** — V2 cluster adds ceiling primitive ON TOP of STAFF-01 (composable; STAFF-01 gates above-threshold; ceiling clamps absolute upper). Substrate at §S-253/254 already lands this. | §S-253/254 commit `8d0ea7cc` + `4c778828` |
| I-2 FATM-10 floor=0 | **NOT-A-BUG** — Captain Q2 2026-05-05 Open-by-Default-Flagged-to-Close doctrine; Captain decides post-launch from empirical. | racecontrol/CLAUDE.md §"Security Debt" |
| **I-3 Path B HMAC TODO** | **UNRESOLVED** — pre-merge BLOCKER per MMA 4/4 finding. **V2-CLUSTER-SUBSTRATE ATOM 2 of 5**: complete HMAC-SHA256 verification at `api/wallet_gateway.rs` before merge. ~30-50 LOC + idempotency-key timing-safe-eq check. | MMA-Step1-DIAGNOSE-row-7.3-bono-2026-05-13 results |
| **I-4 Path E raw UPDATE** | **UNRESOLVED** — pre-merge BLOCKER. **V2-CLUSTER-SUBSTRATE ATOM 3 of 5**: route Path E refund through `wallet::credit_in_tx` abstraction OR add equivalent guard (ceiling check + audit-log + rupee/bonus column tracking); fix cap formula to be session-cost-bounded not 50×-cost. ~40-80 LOC. | explorer.rs:294-324 |
| I-5 Gateway taxonomy mismatch | **PARTIAL-FIX-CANDIDATE-FOLLOW-UP** — fix `is_topup` prefix-match to include `"gateway_topup"` OR rename to `"topup_gateway"`; small (~3 LOC). Acceptable to defer to post-merge atom IF I-3 closes the bigger HMAC bypass. | `wallet.rs:147` |
| I-6 Taxonomy convention-only | **DEFERRED** — full enum-based type-system rework is V2.1+ scope; documented as known-acceptable for V2.0 with `currency_type_for()` helper centralization. | `wallet.rs:14-19` |
| I-7 CHECK constraint narrow | **CAPTAIN-DECISION D-CLUSTER-1** — three options: (a) V1 schema fix ALTER + drop CHECK constraint to match actual usage; (b) V2-db migration NEW; (c) leave-as-known-V1-debt-track. Cluster substrate punts to Captain. | `migrate_billing.rs:316` |
| I-8 Staff-discount per-day open | **FOLLOW-UP** — daily accumulator: `SELECT SUM(staff_discount_paise) FROM billing_sessions WHERE driver_id = ? AND created_at > now() - 24h` at session start; reject if cumulative > ₹X (Captain-decides X). NOT pre-merge BLOCKER (existing manager-approval gate above ₹50 is in path). | `api/billing_start.rs:189-197` |
| I-9 Kimi 2PC | **DEFERRED** — full 2PC out of Phase 1; `accounting::log_admin_action` + `wallet_transactions` INSERT in same tokio task is best-effort durability; documented as known-Phase-2 atom. | `billing_discount.rs:181-220` |
| I-10 MMA-attribution | **PATCHED-BY-DESIGN** — already not-applicable. | `pricing/discount_ceiling.rs:25` |
| I-11 Cloud-venue drift | **POST-MERGE-PATCH** — `max_discount_pct(&state)` const-return today eliminates this for Phase 1; revisit when override-hook lands. | `pricing/discount_ceiling.rs:35` |
| I-12 Customer-touching priority | **PRIORITIZES CLUSTER** — drives I-3 + I-4 to BLOCKING. | §S-256.2 |

## 4. V2-alignment delta

V2 doctrine for the cluster boundary:
> All positive-balance-mutations on `wallets.balance_paise` AND all session-price-reductions must pass through the cluster cap. Direct-UPDATE bypasses are anti-pattern. Audit-trail rows for clamp events must be ledger-stamped (not just `tracing::warn!`).

**Cluster substrate atoms (5 total; Phase β scope):**

### Atom 1 of 5 — Discount ceiling primitive (✓ ALREADY AUTHORED §S-253/254)
- `pricing/discount_ceiling.rs` const + clamp_discount_pct + clamp_discount_paise + ClampResult + 7 unit tests
- Wired at billing_start.rs + billing_discount.rs upstream of FATM-10 floor
- **State:** ON-FEATURE-BRANCH (8d0ea7cc + 4c778828)

### Atom 2 of 5 — Path B HMAC verification (MANDATORY PRE-MERGE)
- Complete HMAC-SHA256 verification at `api/wallet_gateway.rs:41-212`
- Use `ring` or `hmac` crate; timing-safe `subtle::ConstantTimeEq` for comparison
- Verify signature over canonical request body + timestamp; reject if drift > 5min (replay-prevention beyond idempotency_key)
- ~30-50 LOC; unit tests for: valid sig + invalid sig + replay (same idempotency_key+ts) + drift > 5min

### Atom 3 of 5 — Path E refund via abstraction (MANDATORY PRE-MERGE)
- Route `api/wallet_ops.rs:294-324` referenced-refund through `wallet::credit_in_tx` (call the abstraction; remove raw UPDATE)
- Fix `MAX_MANUAL_REFUND_PAISE` cap formula: replace `custom_price.max(cost).max(MAX_MANUAL_REFUND_PAISE)` with `cost.min(MAX_MANUAL_REFUND_PAISE)` (cap by SMALLER of session-cost or absolute-limit, not LARGER)
- Add `accounting::post_refund` call (currently missing on referenced path)
- ~40-80 LOC; unit + integration tests

### Atom 4 of 5 — wallet_transactions schema decision + audit columns (CAPTAIN-DECISION D-CLUSTER-1)
- Captain decides: (a) V1 ALTER + drop narrow CHECK, (b) V2-db NEW table, (c) hybrid (V1 audit-only ALTER + V2-db long-term)
- IF (a): `ALTER wallet_transactions ADD discount_clamped BOOLEAN DEFAULT 0 + original_pct REAL + clamped_pct REAL`; OR new `discount_clamp_events(id, txn_id, session_id, original_pct, clamped_pct, cap_source, ts)` table
- IF (b): map to v2-db schema; longer-term migration
- Schema atom blocks substrate atoms 2/3 from observability-tracking but does NOT block functional ceiling enforcement

### Atom 5 of 5 — Audit-log stamp on clamp events
- Wire `accounting::log_admin_action(action_type="discount_clamped", ...)` at every clamp-event call-site
- Composes-with audit_log existing pattern (already used for `discount_applied`)
- ~10-15 LOC

**Follow-ups (NOT pre-merge):**
- Path A bonus-vs-discount taxonomy type-enforcement (V2.1+ scope)
- Path C unreferenced-refund daily-per-driver accumulator
- I-5 gateway taxonomy prefix-match fix (3 LOC)
- I-8 staff-discount daily-per-driver accumulator
- I-9 full 2PC audit-trail durability
- I-11 cloud-venue config consensus sync (when override-hook lands)
- WhatsApp alert > 10 clamps/day (Phase 2 observability)

## 5. V2-framed proposed change

**Phasing (1 cluster PR):**

### Phase α — RCA + AMPLIFIER (this turn + 24h window)
- This RCA filed at racecontrol/.planning/audits/RCA-2026-05-13-wallet-ledger-discount-cluster.md
- §S-256 CLOSE-ANCHOR at comms-link
- james-AMPLIFIER 24h vote window per §S-146 foundational-boundary doctrine

### Phase β — MMA Step 1 + substrate cascade + PR (next session, ~5-8h)
1. MMA Step 1 DIAGNOSE on cluster RCA (~$0.10-1)
2. Spawn code-cascade agent for substrate atoms 2-5
3. MAOR Tier-1 review (v0.2 per §S-255)
4. Push feature branch to origin
5. Open PR with cluster context + atom checklist
6. Captain per-PR merge auth

### Phase γ — post-merge (deferred)
- V2-PROGRESS-MAP row flips: 7.3 + 1.20 + sibling
- Phase 2 observability (WhatsApp alert + admin dashboard metric)
- Follow-up atoms above

**Anti-pattern guards (encoded in test cascade):**
- Path B: HMAC verification rejected on invalid sig + replay + drift
- Path E: refund through abstraction; cap formula uses MIN not MAX
- Cluster: ceiling clamps Path B + E AT THEIR EQUIVALENT-ECONOMIC-OUTCOME LAYER (not just billing_discount) — invariant test: total positive credit per session ≤ original_price × (1 - 0.50) cap with all 5 paths exercised

**§S-186 Mechanism-trust check (5Q on the wallet-ledger infrastructure):**
1. **Atomic primitives?** YES — `credit_in_tx` and `debit_in_tx` use `pool.begin()` transactions; raw UPDATE path E is the gap (Atom 3 closes it)
2. **TTL-bounded sentinels?** N/A (no sentinels in clamp logic)
3. **Behavioral-verify success?** YES — unit tests + integration tests via contract spec `pricing-discount-ceiling.spec.ts` (5 tests env-gated; Phase γ flip when V2 endpoint lands)
4. **Single-target dry-run?** YES — `cargo test` exercises clamp + path-E + path-B in isolation
5. **Guard contracts?** YES — `MAX_DISCOUNT_PCT_DEFAULT` const-in-code (not config); HMAC secret env-var with timing-safe-eq; CHECK constraint decision per D-CLUSTER-1

**Verdict: PASS** post-Atoms-2-and-3-landing (Atom 1 already PASS).

**V2 doctrine alignment statement:**
> V2 doctrine alignment: closes 4+ of 19 V1→V2 STRUCTURAL GAPS (G-7.3-1 MAX_DISCOUNT_PCT ceiling + G-wallet-credit-bypass + G-refund-coupling + G-gateway-HMAC). Establishes wallet-ledger cluster cap protecting customer-trust invariant across 5 economic-outcome paths. Customer-touching Layer 7 + Layer 1.20 + Phase 2-A/E/F substrate. Captain Q-2-1 ratify §S-252 + Captain "Cluster (B'+)" 17:55 IST drive scope. Composes-with §S-248 + §S-251 james-side §S-146 RCAs (Phase 2-A + Phase 2-F prereqs + 9 V1→V2 RCAs cluster).

## Captain decision queue

| Decision | Surface | Status |
|---|---|---|
| **D-CLUSTER-1** wallet_transactions schema choice (V1 ALTER + drop narrow CHECK · V2-db NEW · hybrid) | Captain | **PENDING-Captain-ratify** |
| **D-CLUSTER-2** MMA Step 1 DIAGNOSE budget for cluster RCA (~$0.10-1) | Captain | **PENDING-Captain-budget-auth** (NOT same as row-7.3-alone D-7.3-3; that one auth'd 17:41 IST was spent at $0.082; this is a new fresh DIAGNOSE on this CLUSTER RCA) |
| **D-CLUSTER-3** Phase β substrate cascade Captain per-PR merge auth | Captain | **PENDING-Phase-β** |
| D-CLUSTER-4 Atom 2 Path B HMAC pre-merge requirement (vs accept-as-known-debt-track) | Captain | **PRE-RECOMMENDATION: pre-merge** (BLOCKING per MMA + foundational-boundary doctrine) |
| D-CLUSTER-5 Atom 3 Path E raw-UPDATE pre-merge requirement (vs accept-as-known-debt-track) | Captain | **PRE-RECOMMENDATION: pre-merge** (BLOCKING per MMA + cap-formula-bug + missing-accounting-call) |
| D-CLUSTER-6 Staff-discount daily accumulator pre-merge vs follow-up | Captain | **PRE-RECOMMENDATION: follow-up** (existing manager-approval gate is mitigating; daily cap is hardening not blocking) |
| D-CLUSTER-7 I-5 gateway taxonomy prefix-match fix (3 LOC) — bundle in cluster PR? | Captain | **PRE-RECOMMENDATION: yes — trivial bundle** |

## Composes-with

- [§S-252](../../../comms-link/V2-MASTER-STATE.md) Captain Q-2-1 ratify `max_discount_pct = 0.50`
- [§S-253 + §S-254](../../../comms-link/V2-MASTER-STATE.md) row 7.3 substrate authored + MAOR Tier-1 PASS (atom 1 of 5)
- [§S-255](../../../comms-link/V2-MASTER-STATE.md) MAOR v0.1→v0.2 promotion (james; lenient reading)
- [§S-256 OPEN-CLAIM](../../../comms-link/V2-MASTER-STATE.md) cluster scope
- [RCA-row-7.3-max-discount-pct-ceiling](RCA-2026-05-13-row-7.3-max-discount-pct-ceiling.md) — companion atom 1 RCA; this cluster RCA SUPERSEDES + EXPANDS its scope to 5 atoms
- [V-LBAC-PROTOCOL §14.1 + §14.2 + §14.3](../specs/v2/V2-LBAC-PROTOCOL.md) — MAOR + F1 + F3 reform
- [racecontrol/CLAUDE.md §S-146 + §S-186](../../CLAUDE.md) — V1↔V2 RCA + mechanism-trust-check upstream
- [security-debt-ledger](../../../comms-link/data/security-debt-ledger.jsonl) — row 3 closure-receipt at line 13 (§S-252); row 12 auth-gap (orthogonal; not closed by this cluster)
- MMA Step 1 DIAGNOSE results — `/tmp/mma-row-7.3-results/` (5 model JSON; 4 substantive)
- `racecontrol/.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md` — F-05 refund-on-early-end RCA (refund-discount-coupling sibling)

## Stale-at

2026-08-13 (90 days).

— bono · 2026-05-13 ~12:33 UTC (Wed 2026-05-13 ~18:03 IST) · Cluster §S-146 V1↔V2 RCA authored · 5 atoms (1 ✓ + 4 pending Phase β) · 12 inherited issues catalogued · 3 mandatory pre-merge + 1 follow-up per MMA-driven scoping · Captain decision queue D-CLUSTER-1 (schema) + D-CLUSTER-2 (MMA budget) + D-CLUSTER-3 (per-PR merge) primary gates
