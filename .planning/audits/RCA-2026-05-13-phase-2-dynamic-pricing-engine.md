---
artifact: §S-146 V1↔V2 RCA
row: V2-PROGRESS-MAP §2 W2 Phase 2 — dynamic pricing engine (rate_table.resolve_rate())
status: AUTHORED-AWAITING-CAPTAIN-DISPOSITION
authored: 2026-05-13 IST
author: james
boundary-class: foundational (wallet/billing pricing-engine — load-bearing for all session debits)
mma-step-1: PENDING-Captain-auth (foundational-boundary escalation)
parent-cascade: §S-219.11 iter4 row Phase 2 — 1 NEW-MECHANISM-CLASS gap surfaced
gaps-closed: 1 (G-Phase-2-1 NEW-MECHANISM-CLASS dynamic-pricing engine: rate_table.resolve_rate() pricing engine + Phase 2-A rate-table + Phase 2-F campaign-object substrate)
customer-day-beat: 14:10-14:50 dynamic pricing — load-bearing for ALL session debits
---

# §S-146 V1↔V2 RCA — Phase 2 dynamic-pricing engine

## 1. Boundary map

V2 dynamic-pricing engine — NEW-MECHANISM-CLASS; the load-bearing substrate that all discount-class RCAs in this batch (1.20 iRacing-discount + 7.3 ceiling + Phase 2-E combo) call into.

**G-Phase-2-1: Dynamic-pricing engine substrate (NEW-MECHANISM-CLASS)**
- §S-219.11 finding: "Phase 2 dyn pricing — 1 NEW-MECHANISM-CLASS gap"
- V1 substrate present but recommendation-only:
  - `crates/racecontrol/src/dynamic_pricing.rs:7-18` — `PricingRecommendation` struct + `recommend_pricing()` fn; recommendation generation, NOT applied engine
  - `crates/racecontrol/src/pricing_bridge.rs:2` — "Prices computed by dynamic_pricing.rs are proposed → approved → applied to all channels" — comment-only; bridge not implemented
  - `crates/racecontrol/src/scheduler_analytics.rs:66-95` — peak_hours / off_peak_hours analytics + pricing_suggestion string; analytics-only, NOT engine
  - `crates/racecontrol/src/maintenance_checks.rs:78` — `is_peak_hours()` — predicate-only, used for maintenance gating not pricing
- `rate_table.resolve_rate()` function — **DOES NOT EXIST** (grep 0 hits for `rate_table\b\|resolve_rate`)
- Phase 2-A rate-table table substrate not yet authored (cf. Phase 2-E RCA dependency)
- Phase 2-F campaign-object substrate not yet authored

**Contract test scaffolded:** `racecontrol/tests/contract/phase-2-dynamic-pricing-engine.spec.ts` (§S-219 iter4 commit 9dc95a38, 5 tests) — env-gated SKIP-with-reason `V1_NO_DYN_PRICING_ENGINE`.

## 2. Inherited-issue catalogue

| # | V1 class | Surface | Source |
|---|---|---|---|
| I-1 | **dynamic_pricing.rs recommendation-only** — V1 generates recommendations; doesn't apply them; pricing_bridge.rs is comment-only stub | grep | §S-219.11 |
| I-2 | **scheduler_analytics peak_hours predicate-only** — produces hour-lists for staff visibility; no engine binding | `scheduler_analytics.rs:66-95` | grep |
| I-3 | **Captain Q-2-1..6 pricing-engine doctrine pending** — §S-218 + bono Phase 2-D msg=36346 + Phase 2-C msg=36347 + Phase 2-G msg=36349 — 7 Q-2F + 6 Q-2D + 7 Q-2C + 6 Q-2G open AMPLIFIER asks per V2-PROGRESS-MAP §0 cited | doctrine | §S-211 outbound queue |
| I-4 | **F25a SnapPricing strategy in V1** — V1 has SnapPricing class-A as substrate; V2 must preserve as known strategy with HISTORICAL block (per `racecontrol/CLAUDE.md` F25a doctrine) | billing_pricing.rs | CLAUDE.md F25a entry |
| I-5 | **NEW-MECHANISM-CLASS broad scope** — engine touches rate-resolution + happy-hour + iRacing-discount + combo-offer + ceiling + Wallet Framing C single-purpose voucher invariants — composes-with 4 other RCAs in this batch | doctrine compose | this RCA batch |
| I-6 | **2-A rate-table + 2-F campaign-object substrate prerequisites** — engine cannot ship without; cascade gating | substrate prereqs | §S-219.11 |
| I-7 | **Wave 1 W1-S6 billing-calculator parallel work (bono-LED Phase 2-B)** — `comms-link/.planning/draft-pacts/PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md` (commit abbf52a8) ships 3 surfaces + 4 wallet-client ops + 1 WS subscription; engine must compose with this consumer interface | Phase 2-B consolidated index | bono Phase 2-B |
| I-8 | **DPDP audit on pricing-rule applied events** — every applied rule stamps to wallet_debits audit columns (composes-with RCA-row-7.3 + Q-1.14 §S-242 doctrine) | wallet ledger | §S-242 + RCA-row-7.3 |

## 3. Past-bug review

| # | Issue | Disposition |
|---|---|---|
| I-1 | dynamic_pricing.rs recommendation-only | **PATCHED-ONLY** — V1 module retained for recommendation generation; V2 engine is a parallel module that APPLIES rules |
| I-2 | scheduler_analytics peak_hours analytics-only | **PATCHED-ONLY** — analytics retained; engine consumes peak_hours via shared types |
| I-3 | Captain Q-2-* dispositions pending | **DEPENDS-ON** — engine substrate-PR drafts gate on Captain answer to Q-2-1..6 + bono Q-2C/2D/2F/2G AMPLIFIER asks |
| I-4 | F25a SnapPricing | **ROOT-CAUSED-AND-FIXED 2026-03-28** — V2 engine preserves SnapPricing strategy with HISTORICAL block; behavior-parity test enforced |
| I-5 | NEW-MECHANISM-CLASS broad scope | **COMPOSE-WITH** all 4 sibling RCAs in this batch (1.20 + 7.3 + Phase 2-E + this); ship-sequence: engine → 1.20 → 7.3 → Phase 2-E |
| I-6 | 2-A rate-table + 2-F campaign-object | **DEPENDS-ON** — separate RCAs author Phase 2-A and Phase 2-F substrate; this RCA assumes both as prerequisites |
| I-7 | Phase 2-B billing-calculator (bono-LED) | **COMPOSE-WITH** — engine consumer interface aligns with bono Phase 2-B 3-surface contract; bilateral sync at PR-author time |
| I-8 | DPDP audit pricing-rule applied | **PATCHED-BY-DESIGN** — every applied rule stamps wallet_debits.discount_applied + locked_rate_paise + rule_id; audit retention 8-year per §S-242 CGST doctrine |

## 4. V2-alignment delta

V2 dynamic-pricing engine:

**A. Engine module structure**
```
crates/racecontrol/src/pricing/
├── engine.rs                     -- public API: resolve_rate(ctx) -> ResolvedRate
├── rate_table.rs                 -- Phase 2-A backend; reads rate_table_v2 DB table
├── campaign_object.rs            -- Phase 2-F backend; reads pricing_campaigns DB table
├── resolve_discount.rs           -- RCA-row-1.20 deeper-of(happy_hour, iRacing)
├── discount_ceiling.rs           -- RCA-row-7.3 clamp_discount_pct
├── combo_offer_engine.rs         -- RCA-Phase-2-E combo engine
└── strategy/
    ├── snap_pricing.rs           -- F25a HISTORICAL preserved
    ├── per_minute_pricing.rs     -- per-minute rate
    └── peak_off_peak.rs          -- time-based rule
```

**B. `resolve_rate(ctx)` public API**
```rust
pub struct ResolveContext<'a> {
    pub now_ist: DateTime<FixedOffset>,
    pub customer: &'a Customer,
    pub pod_id: &'a str,
    pub game_context: GameContext,
    pub session_id: Option<&'a str>,
    pub state: &'a AppState,
}

pub struct ResolvedRate {
    pub rate_per_min_paise: u32,
    pub strategy: PricingStrategy,       // SnapPricing | PerMinute | PeakOffPeak | Custom
    pub applied_rules: Vec<AppliedRule>, // [HappyHour 30%, iRacing 20%, Combo X, ...]
    pub resolved_discount: AppliedDiscount,
    pub locked_rate_paise: i64,
    pub audit_trail_json: serde_json::Value,
}

pub async fn resolve_rate<'a>(ctx: &ResolveContext<'a>) -> ResolvedRate {
    // 1. Read base rate from rate_table (Phase 2-A)
    // 2. Apply happy-hour + iRacing-discount deeper-of (RCA-row-1.20)
    // 3. Apply combo-implicit discount if redemption exists (RCA-Phase-2-E)
    // 4. Apply discount_ineligible early-zero (Walk-In Guest)
    // 5. Clamp by ceiling (RCA-row-7.3)
    // 6. Compose into ResolvedRate + audit_trail_json
}
```

**C. wallet_debits audit columns** (subset overlaps with sibling RCAs)
- `discount_applied: TEXT` — enum {None, HappyHour, iRacing, Combo, Captain, Other}
- `discount_pct: REAL`
- `discount_clamped: BOOLEAN`
- `original_pct: REAL` (pre-clamp value if clamped)
- `clamped_pct: REAL` (post-clamp value if clamped)
- `locked_rate_paise: INTEGER`
- `applied_rules_json: TEXT` (full audit trail from ResolvedRate.applied_rules)
- `pricing_strategy: TEXT`

**D. Bono Phase 2-B consumer interface alignment**
- Engine `resolve_rate()` outputs match Phase 2-B §1 3-surface contract (commit abbf52a8)
- Bono billing-calculator (W1-S6) reads `ResolvedRate.rate_per_min_paise` for per-tick debits
- Split-rate contract per Phase 2-B §4 honored: rate_per_min_paise + audit_trail composed at engine layer

**E. Background substrate sync**
- Phase 2-A rate-table substrate must land first
- Phase 2-F campaign-object substrate must land first
- This engine RCA assumes both; ship-sequence enforced at Captain auth gate

**Named gap (R9):** V2 pricing engine is the load-bearing substrate that 4 sibling RCAs (1.20 + 7.3 + Phase 2-E + Phase-2-A) compose into. The engine is NEW-MECHANISM-CLASS atop V1 recommendation-only dynamic_pricing.rs; F25a SnapPricing strategy preserved with HISTORICAL block; consumer interface aligns with bono Phase 2-B billing-calculator.

## 5. V2-framed proposed change

**Phasing (4 sub-phases; ~300-400 LOC; gates on Phase 2-A + Phase 2-F):**

**Phase 1 — Engine module scaffold + resolve_rate() public API** (~150 LOC; depends-on Phase 2-A + 2-F substrate)
- Add `crates/racecontrol/src/pricing/engine.rs` with `resolve_rate()` public function
- Add strategy modules (snap_pricing preserved + per_minute + peak_off_peak)
- ALTER `wallet_debits` ADD 8 audit columns
- Unit tests cover 4×3 matrix {time ∈ {peak, off_peak, happy_hour}} × {game ∈ {AC, iRacing, Other}} × {customer ∈ {regular, walk_in_guest}}
- F25a SnapPricing behavior-parity test
- Phase 2-B bono billing-calculator integration verify

**Phase 2 — Compose-with discount resolution + ceiling clamp** (~80 LOC; depends-on RCA-row-1.20 + RCA-row-7.3 landed)
- Engine calls `resolve_discount` (RCA-row-1.20 deeper-of)
- Engine calls `clamp_discount_pct` (RCA-row-7.3 ceiling)
- audit_trail_json captures full rule-chain
- Contract test asserts deeper-of + clamp composition correct

**Phase 3 — Combo-offer compose** (~60 LOC; depends-on RCA-Phase-2-E landed)
- Engine reads active combo_redemptions for session_id
- combo implicit_discount_pct factored into deeper-of input
- combo SESSION-CHARGE vs WALLET-CREDIT independence honored

**Phase 4 — Campaign-object compose + emit metric** (~80 LOC; depends-on Phase 2-F landed)
- Engine reads pricing_campaigns table (Phase 2-F) for active campaign-class discounts
- Emit Prometheus-style metric `pricing.engine.resolve_rate_latency_ms` + `pricing.engine.rule_applied_count{rule_type}`
- Admin dashboard surfaces engine activity

**Anti-pattern guard:**
- Test asserts engine output is deterministic given same ctx (no clock-dependent fluctuation within same window)
- Test asserts F25a SnapPricing behavior-parity preserved (historical test fixture compared bit-by-bit)
- Test asserts wallet_debits row stamps full audit trail JSON
- Test asserts no rule applied for discount_ineligible customers (early-zero)
- Test asserts engine respects strategy boundary (per_minute customer doesn't get snap_pricing engine output)
- Test asserts Phase 2-B billing-calculator integration: bono consumer reads same rate_per_min_paise the engine emitted

**Mechanism-trust check (§S-186):**
- (1) atomic primitives? **YES** — resolve_rate is pure function read; wallet_debits stamp is in same transaction as debit
- (2) TTL-bounded sentinels? **N/A**
- (3) behavioral-verify success? **YES via contract test + F25a parity**
- (4) single-target dry-run? **YES** — unit tests + behavior-parity before fleet
- (5) guard contracts? **YES** — engine module is server-side only; public API explicit
- **Verdict: PASS.**

**V2 doctrine alignment statement:**
> V2 doctrine alignment: closes 1 of 19 V1→V2 STRUCTURAL GAPS (G-Phase-2-1 NEW-MECHANISM-CLASS dynamic-pricing engine). Establishes V2 load-bearing pricing engine module per V2-PROGRESS-MAP §2 W2 Phase 2 — composes-with all 4 sibling RCAs in this batch (1.20 + 7.3 + Phase 2-E + Phase 2-A prereq + Phase 2-F prereq). F25a SnapPricing preserved with HISTORICAL block per racecontrol/CLAUDE.md F25a doctrine. Aligned with bono Phase 2-B billing-calculator consumer interface (commit abbf52a8 PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md). DPDP audit trail per §S-242 8-year CGST retention.

## Captain decision queue

| Decision | Status |
|---|---|
| **D-Phase-2-1** Phase 1 engine scaffold PR | AUTHORED-PENDING |
| **D-Phase-2-2** Phase 2-A rate-table prerequisite RCA | DEPENDS-ON (separate RCA needed; not in this batch) |
| **D-Phase-2-3** Phase 2-F campaign-object prerequisite RCA | DEPENDS-ON (separate RCA needed; not in this batch) |
| **D-Phase-2-4** MMA Step 1 DIAGNOSE (foundational — pricing engine) | bono OpenRouter | AWAITING-Captain-budget-auth |
| **D-Phase-2-5** Captain Q-2-1..6 (pricing-engine doctrine) | Captain — bono dispatched 4 AMPLIFIER-ASKs msg=36341/36346/36347/36349 |
| **D-Phase-2-6** Phase 2-4 compose-with siblings | DEFERRED to all 4 prerequisite RCAs landed |

## Composes-with

- [⭐⭐ V1-dep V2 RCA](feedback_v1_dependent_v2_root_cause_before_proceeding.md)
- RCA-row-1.20 — deeper-of resolution
- RCA-row-7.3 — ceiling clamp
- RCA-Phase-2-E — combo-offer compose
- [project_dynamic_pricing_synthesis_20260509](project_dynamic_pricing_synthesis_20260509.md) — parent V2-PROGRESS-MAP synthesis
- [Phase-2-B bono cloud surfaces reference](https://github.com/bono-bot/comms-link/blob/main/.planning/draft-pacts/PHASE-2-B-BONO-CLOUD-SURFACES-REFERENCE.md) — bilateral consumer interface
- §S-219.11 cumulative structural gaps

## Stale-at

2026-08-13.
