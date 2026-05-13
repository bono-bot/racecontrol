---
artifact: §S-146 V1↔V2 RCA
row: V2-PROGRESS-MAP §2 W2 Phase 2-E — combo-offer primitive substrate
status: AUTHORED-AWAITING-CAPTAIN-DISPOSITION
authored: 2026-05-13 IST
author: james
boundary-class: non-foundational (NEW-MECHANISM-CLASS Phase 2-E substrate atop Phase 2-A rate-table)
mma-step-1: PENDING-Captain-auth (Phase 2-E scope authorization)
parent-cascade: §S-218 Phase 2-E combo-offer-primitive acceptance test (commit 98008d59); 1 NEW-MECHANISM-CLASS gap surfaced
gaps-closed: 1 (G-Phase-2-E-1 combo_offers/line_items/redemptions tables + /v2/combos/eligible + /v2/combos/apply endpoints + 6-action_type audit enum — all NEW V2 substrate)
customer-day-beat: 14:10-14:50 pricing rules (DoD L446 happy-hour + combo offers configurable for demand-creation loop)
---

# §S-146 V1↔V2 RCA — Phase 2-E combo-offer primitive substrate

## 1. Boundary map

Phase 2-E NEW-MECHANISM-CLASS: combo-offer primitive lives atop Phase 2-A rate-table.resolve_rate(); cafe `combo` + `gaming_bundle` promo-types exist (V1 cafe-scope) but pod-billing scope combo-offers are V2-greenfield.

**G-Phase-2-E-1: combo_offers cluster substrate (NEW-MECHANISM-CLASS)**
- §S-218.4 finding verbatim: "combo_offers/line_items/redemptions tables + /v2/combos/eligible + /v2/combos/apply endpoints + 6-action_type audit enum — all NEW V2 substrate atop Phase 2-A rate_table.resolve_rate()"
- grep `combo_offers\|line_items\|/v2/combos` returns 0 hits in racecontrol Rust crates
- V1 adjacent substrate exists in cafe scope:
  - `crates/racecontrol/src/cafe_promos.rs:103` — promo_type IN ('combo', 'happy_hour', 'gaming_bundle')
  - `crates/racecontrol/src/db/migrate_cafe.rs:123` — schema CHECK constraint cafe-only
  - `crates/racecontrol/src/api/billing_coupon.rs` — coupon_redemptions table (single-coupon scope; different schema from combo redemptions)
- V1 dynamic_pricing.rs is recommendation-only:
  - `crates/racecontrol/src/dynamic_pricing.rs:7-18` — `PricingRecommendation` struct + `recommend_pricing()` fn; no engine, no rate-table backend, no combo-stacking

**§S-218 Phase 2-E ANTI-PATTERN GUARD encoded in spec:**
- Combo offers STACK with session discounts via deeper-of BUT INDEPENDENT from top-up bonus ladder (per DoD L76-80)
- SESSION-CHARGE vs WALLET-CREDIT surface separation
- 6 action_type audit enum: `combo_eligible_check`, `combo_applied`, `combo_revoked`, `combo_expired`, `combo_capped_by_ceiling`, `combo_failed_eligibility`

**Contract test scaffolded:** `racecontrol/tests/contract/phase-2-e-combo-offer-primitive.spec.ts` (§S-218 iter3 commit 98008d59, 5 tests) — env-gated SKIP-with-reason `COMBO_OFFER_ENGINE_REACHABLE`.

## 2. Inherited-issue catalogue

| # | Class | Surface | Source |
|---|---|---|---|
| I-1 | **V1 cafe-promos `combo` type is single-cart-cafe-scope** — pod-billing scope combo-offers (e.g., "sim + cafe-snack bundle") need cross-domain substrate that V1 cafe-promos doesn't reach | `cafe_promos.rs:103` schema scope-narrowing | grep · §S-218 |
| I-2 | **coupon_redemptions schema is single-coupon-scope** — Phase 2-E needs combo-redemptions table tracking combo composition (multi-line-item per redemption row) | `billing_coupon.rs:91-249` | grep |
| I-3 | **dynamic_pricing.rs is recommendation-only, not engine** — Phase 2-E needs engine; rate_table.resolve_rate() (Phase 2-A) must be authored first | `dynamic_pricing.rs:7-18` | grep · §S-218 |
| I-4 | **NEW-MECHANISM-CLASS = NO V1 footprint to inherit failure modes** — but Phase 2-E composes-with RCA-row-1.20 deeper-of + RCA-row-7.3 ceiling + DoD L76-80 bonus-vs-discount independence | doctrine compose pattern | RCA-row-1.20 + RCA-row-7.3 |
| I-5 | **Phase 2-A rate-table not yet authored** — Phase 2-E gates on Phase 2-A landing first; substrate-ship sequence: 2-A → 2-F campaign-object → 2-E combo-engine | Phase 2-A substrate | V2-PROGRESS-MAP §2 W2 cascade |
| I-6 | **DPDP audit aggregate-only** — combo redemptions table needs customer_id for redemption tracking BUT NOT customer_data_delete cascade (cf. §S-242 Q-1.14 doctrine); customer_id FK to drivers + ON DELETE RESTRICT pattern OR transitive-via-session_id linkage | composes-with §S-242 DPDP | Q-1.14 RCA doctrine |
| I-7 | **6 action_type audit enum semantics** — every combo-engine event MUST stamp action_type into combo_redemptions_audit_log; finance reports + DPDP audit query rely on enum exhaustiveness | audit-log substrate | §S-218 anchor |
| I-8 | **Composition with happy-hour / iRacing-discount** — deeper-of(happy_hour_pct, iracing_pct, combo_implicit_pct); 4-cell matrix per session ON TOP of ceiling clamp | composes-with RCA-row-1.20 + RCA-row-7.3 | doctrine compose |

## 3. Past-bug review

| # | Issue | Disposition |
|---|---|---|
| I-1 | V1 cafe-promos scope-narrowing | **PATCHED-BY-DESIGN** — V2 combo_offers table is pod-billing-scope; cafe-promos unchanged (different domain, parallel substrate) |
| I-2 | coupon_redemptions single-scope | **NOT-APPLICABLE-TO-V2** — combo_redemptions is parallel table with multi-line-item schema |
| I-3 | dynamic_pricing.rs recommendation-only | **DEPENDS-ON** — Phase 2-A rate-table substrate ships first (separate RCA cascade); Phase 2-E builds on top |
| I-4 | NEW-MECHANISM-CLASS | **NO V1 FOOTPRINT** — straight-through V2 substrate; doctrine constraints from compose-with siblings |
| I-5 | Phase 2-A not yet authored | **DEPENDS-ON** — gating; RCA-Phase-2-A separate (not in this batch) |
| I-6 | DPDP audit aggregate | **PATCHED-BY-DESIGN** — combo_redemptions has customer_id FK with ON DELETE RESTRICT (per §S-242 Q-1.14 Option B); audit retention 8-year per CGST applies |
| I-7 | 6 action_type enum exhaustiveness | **UNRESOLVED** — V2 substrate-PR ships enum + exhaustive-match test |
| I-8 | Composition with discount-class siblings | **DEPENDS-ON** — composes-with RCA-row-1.20 + RCA-row-7.3; ship-sequence: 1.20 deeper-of + 7.3 ceiling → 2-E combo-engine |

## 4. V2-alignment delta

V2 combo-offer primitive substrate:

**A. 3 tables (NEW v2-db migration)**
```sql
CREATE TABLE combo_offers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    starts_at_utc TEXT NOT NULL,
    ends_at_utc TEXT NOT NULL,
    eligibility_json TEXT NOT NULL, -- {game_context?, time_window?, customer_tier?, min_top_up_paise?}
    composition_json TEXT NOT NULL, -- {sim_minutes, cafe_paise, implicit_discount_pct, ...}
    capacity_limit INTEGER,         -- nullable; total redemptions cap
    per_customer_limit INTEGER,     -- nullable; per-customer redemptions cap
    active BOOLEAN NOT NULL DEFAULT 1
);

CREATE TABLE combo_line_items (
    id TEXT PRIMARY KEY,
    combo_id TEXT NOT NULL REFERENCES combo_offers(id) ON DELETE RESTRICT,
    item_type TEXT NOT NULL,    -- 'sim_minutes', 'cafe_paise', 'discount_pct'
    quantity REAL NOT NULL,
    ordering INTEGER NOT NULL
);

CREATE TABLE combo_redemptions (
    id TEXT PRIMARY KEY,
    combo_id TEXT NOT NULL REFERENCES combo_offers(id) ON DELETE RESTRICT,
    customer_id TEXT NOT NULL REFERENCES drivers(id) ON DELETE RESTRICT,  -- §S-242 doctrine
    session_id TEXT REFERENCES billing_sessions(id) ON DELETE SET NULL,
    redeemed_at_utc TEXT NOT NULL,
    composition_snapshot_json TEXT NOT NULL,  -- frozen composition at redeem-time
    sim_minutes_applied INTEGER,
    cafe_paise_applied INTEGER,
    discount_pct_applied REAL,
    discount_pct_clamped REAL,  -- if ceiling fired (composes-with RCA-row-7.3)
    venue_id TEXT NOT NULL
);

CREATE TABLE combo_redemptions_audit_log (
    id TEXT PRIMARY KEY,
    combo_redemption_id TEXT REFERENCES combo_redemptions(id) ON DELETE RESTRICT,
    action_type TEXT NOT NULL CHECK(action_type IN (
        'combo_eligible_check', 'combo_applied', 'combo_revoked',
        'combo_expired', 'combo_capped_by_ceiling', 'combo_failed_eligibility'
    )),
    actor TEXT,
    payload_json TEXT,
    timestamp_utc TEXT NOT NULL
);
```

**B. 2 endpoints (NEW)**
- `POST /api/v1/v2/combos/eligible` — `{customer_id, game_context?, cart?}` → `[{combo_id, eligible: bool, reason?}, ...]`
- `POST /api/v1/v2/combos/apply` — `{combo_id, customer_id, session_id, idempotency_key}` → `{combo_redemption_id, sim_minutes_applied, cafe_paise_applied, discount_pct_applied, idempotency_status}`

**C. Engine (NEW)**
- `crates/racecontrol/src/pricing/combo_offer_engine.rs` (NEW)
  - `pub async fn check_eligibility(...) -> Vec<EligibilityResult>`
  - `pub async fn apply_combo(...) -> Result<ComboRedemption>` (idempotency via idempotency_key per RCA-row-1.13 doctrine)
  - Audit-log every action_type via 6-enum
- Engine reads Phase 2-A rate-table (when shipped); writes combo_redemptions + audit_log

**D. Compose-with constraints**
- Deeper-of: implicit_discount_pct from combo composes-with happy-hour + iRacing-discount via `resolve_discount` (RCA-row-1.20)
- Ceiling: resolved pct clamped by MAX_DISCOUNT_PCT (RCA-row-7.3)
- Bonus-vs-discount independence: combo affects SESSION-CHARGE; bonus-credits independent at WALLET-CREDIT (DoD L76-80)
- DPDP: customer_id ON DELETE RESTRICT (§S-242 Q-1.14 Option B); 8-year CGST audit retention

**Named gap (R8):** V1 cafe combo is single-cart-scope; V2 combo is pod-billing-cross-domain-scope (sim + cafe stacked). Engine ships atop Phase 2-A rate-table; ANTI-PATTERN GUARD ensures combo SESSION-CHARGE primitive doesn't leak into WALLET-CREDIT bonus-credit primitive.

## 5. V2-framed proposed change

**Phasing (2 sub-phases; ~250 LOC; gates on Phase 2-A landing):**

**Phase 1 — Schema + 2 endpoints + engine scaffold** (~180 LOC)
- v2-db migration: 4 tables (combo_offers + combo_line_items + combo_redemptions + audit_log)
- `crates/racecontrol/src/pricing/combo_offer_engine.rs` (NEW) with eligibility + apply + audit handlers
- Routes register `/api/v1/v2/combos/eligible` + `/api/v1/v2/combos/apply`
- Idempotency key honored per RCA-row-1.13 doctrine
- Contract test SKIP-reason `COMBO_OFFER_ENGINE_REACHABLE` flips to LIVE

**Phase 2 — Compose-with discount-resolution + ceiling clamp** (~70 LOC; depends-on RCA-row-1.20 + RCA-row-7.3 landed)
- combo implicit_discount_pct fed into `resolve_discount` deeper-of (RCA-row-1.20)
- post-deeper-of result clamped by ceiling (RCA-row-7.3); audit-log stamps `combo_capped_by_ceiling` action_type when clamp fires
- WhatsApp alert on > 10 capped/day (composes-with RCA-row-7.3 Phase 2 alert)

**Anti-pattern guard:**
- Test asserts combo applies SESSION-CHARGE not WALLET-CREDIT bonus (DoD L76-80)
- Test asserts 6-enum exhaustive (compile-time match)
- Test asserts per_customer_limit + capacity_limit enforced (early exit on exceeded)
- Test asserts redeem with same idempotency_key returns first-call result (composes-with RCA-row-1.13)
- Test asserts DPDP customer_data_delete blocked when combo_redemptions row exists (ON DELETE RESTRICT)

**Mechanism-trust check (§S-186): PASS** — schema + 2 endpoints + engine = additive substrate; no cross-organ delivery chain.

**V2 doctrine alignment statement:**
> V2 doctrine alignment: closes 1 of 19 V1→V2 STRUCTURAL GAPS (G-Phase-2-E-1 combo_offers cluster substrate; NEW-MECHANISM-CLASS). Establishes pod-billing-scope combo-offer primitive atop Phase 2-A rate-table per V2-PROGRESS-MAP §2 W2 Phase 2-E. Encodes DoD L76-80 SESSION-CHARGE vs WALLET-CREDIT independence + 6 action_type audit enum exhaustive. Composes-with RCA-row-1.20 (deeper-of with happy-hour + iRacing) + RCA-row-7.3 (ceiling clamp) + §S-242 Q-1.14 Option B DPDP doctrine (combo_redemptions.customer_id ON DELETE RESTRICT). Unblocks Phase 2-E IN-FLIGHT acceptance test.

## Captain decision queue

| Decision | Status |
|---|---|
| **D-Phase-2-E-1** Phase 1 substrate PR | AUTHORED-PENDING |
| **D-Phase-2-E-2** Phase 2-A rate-table prerequisite RCA | DEPENDS-ON (separate RCA needed) |
| **D-Phase-2-E-3** MMA Step 1 (recommended for NEW-MECHANISM-CLASS) | Captain-call |
| **D-Phase-2-E-4** Migration schema review | DEFERRED |
| **D-Phase-2-E-5** Phase 2 compose-with siblings | DEFERRED to all 3 prerequisite RCAs landed |

## Composes-with

- [⭐⭐ V1-dep V2 RCA](feedback_v1_dependent_v2_root_cause_before_proceeding.md)
- RCA-row-1.20 — deeper-of with happy-hour + iRacing
- RCA-row-7.3 — ceiling clamp
- RCA-row-1.13 — idempotency_key doctrine
- [project_q_1_14_v2_clean_dpdp_rca](project_drift_detected_at_tz_mislabel_rca_20260513.md) — DPDP doctrine
- §S-218 cumulative structural gaps

## Stale-at

2026-08-13.
