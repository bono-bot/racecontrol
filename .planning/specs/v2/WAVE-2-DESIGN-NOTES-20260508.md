# Wave 2 Design Absorption — 2026-05-08 ~07:08 IST

**AUTHORED:** 2026-05-08 ~07:08 IST · james-venue-LEAD
**Class:** planning memo (NOT a PACT FILE; absorption of bono-shipped substrate + Captain dispositions into Phase 2 implementation scope)
**Anchor:** comms-link `V2-MASTER-STATE.md` §S-91 + §RATE-TABLE + §S-92→§S-99 (segment-C through segment-F)

**Stale-at:** Wave 1 land-trigger (PACT-001 Phase 1 wire-up PR merge) OR any Captain CHALLENGE-AMEND on §S-92→§S-99 within their 24h windows (silent-expire 2026-05-09 ~04:22 IST onwards) OR Wave 2 PR-open which supersedes this with a per-phase PLAN.md.

---

## §1 — Inputs locked (this absorption)

### §RATE-TABLE canonical (V2-MASTER-STATE.md §S-91)

| Surface | Standard rate | Off-peak (Wave 2) | Floor model | Source |
|---|---|---|---|---|
| Sim 60min | ₹900 | **-30% → ₹630** (P1) | per-session electricity allocation | §S-92 P1 |
| Sim 30min | ₹700 | -30% derived (P1) | "" | §S-92 P1 |
| Sim 5min | free | n/a | n/a | §S-91 |
| PS5 60min | ₹500 | TBD per Wave 2 PR-open | per-session electricity | §S-91 |
| PS5 extra controller | **₹200 per session** [Q-OE-PS5-1=(a)] | TBD | flat add-on | §S-93 |
| Cafe items | per sheet `1e-AsyX72cTWUeujzgWXRHbjomtdAlUfe` | **-20% off MENU** (P2) | `sheet_cost × 1.25` per Q-OE-7e [Q-OE-7f=(b) mechanical] | §S-89 disp #6 + §S-93 |
| Beverages (incl. coffee) | per `beverages_catalog` (single-size LOCKED) | **-20% off MENU** (P2; same window) | `sales ≥ cost × 1.25` (cost NULLABLE — costing-later) | §S-95 |
| Wallet top-up bonus | per Wallet Framing C (LOCKED 2026-05-03) | n/a (separate concept) | n/a | Wallet Framing C |

**Initial off-peak window:** Tue-Wed 1pm-4pm (Captain §S-84 dry-spell scenario; surface-specific windowing locks at Phase 2-A FILE-conversion).

### §S-92 V2.0 LOCKED PARAMETERs P1-P8 (Captain explicit 2026-05-08 ~04:22 IST)

| # | PARAMETER | Locked value | Wave |
|---|---|---|---|
| P1 | Sim off-peak rate | -30% → ₹630/hr | 2 (Phase 2-A rate_table) |
| P2 | Cafe off-peak rate | -20% off menu | 2 (Phase 2-A cafe-extension; GAP-1) |
| P3 | Combo offer style v0.1 | explicit-combo-id only; per-category bundling V2.1+ | 2 (Phase 2-E NEW; GAP-2) |
| P4 | Pricing-change notification surface | both PWA in-pod + kiosk overlay | 2 (Phase 2-D countdown + transition) |
| P5 | Wallet per-minute rounding | round up to nearest minute | 3 (Phase 2-B + 2-C) |
| P6 | MI 5-tier discount % | New 20 / Building 10 / Loyal 15 / VIP 20 / Lapsed 25 | 4 |
| P7 | Dry-spell trigger | <30% pod utilization for ≥2 consecutive hours | 5 |
| P8 | Captain-curated campaign cost cap | ₹100/customer/month max | 5 |
| P9 | **Operating expense baseline** | **₹4.62L/month** (₹15,400/day) | All — break-even derivation |

### §S-94 Visit-frequency 6-tier (cross-pilot; bono cloud-LEAD on table)

| Tier | Definition | Use case |
|---|---|---|
| Power | ≥4 visits/30d | VIP-treatment + new-feature beta + loyalty |
| Regular | 2-3/30d OR ≥1/7d | Frequent-visitor recognition + cross-sell |
| Casual | 1/30d OR 1-3/90d | Re-engagement + loyalty-builder |
| Lapsed | 0/30d AND ≥1/365d | Win-back broadcasts |
| New | First visit ≤30d ago | Welcome series + onboarding |
| Inactive | 0/365d | Suppress-default; manual-include only |

`customer_visit_frequency` table extends `mesh_kb.db` v26.0 MI substrate (bono cloud-LEAD on table + ingestion); james venue-LEAD on session-end-event publishing (Phase 2-B already emits — adds customer_id binding).

### §S-95 Coffee-led strategic priority (DOCTRINE-CLASS — cross-cutting)

- `beverages_catalog` schema: `is_coffee_or_related` boolean flag
- Phase 2-E combo schema MUST accommodate "coffee-paired" combo type (Sim 2hr + coffee + cookie = ₹X) — first-class combo type
- Wave 4 MI experience-score: `coffee_order_count` joins `visit_count + sim_session_count` as engagement dimension
- Wave 5 Captain-curated WhatsApp: 60% coffee-led / 40% other (bono-default; Q-OE-COFFEE-2 ratify pending)
- V2.1+ scope: coffee-equipment investment (machine upgrade / barista training / bean sourcing) — Q-OE-COFFEE-3

### §S-98 V2.0 viability-tracking architecture (Captain COUNTER 2026-05-08 ~05:59 IST)

- **Primary canonical (sole source):** racecontrol DB + comms.db + Phase 2-A rate_table service + Phase 2-B billing calculator output + Wallet-Framing-C wallet ledger
- **Secondary (none):** SRL VMS REMOVED entirely (access loss; legacy pre-RP-V1 substrate per Captain explicit)
- **V2.1+:** external substrate integration reopens post-V2.0-launch with empirical baseline

### §S-99 Voice mirror ship pack (Captain V2.1+ scope-pin 2026-05-08 ~06:13 IST)

- V2.0: English/Hindi/Hinglish v0.1 (3 mirrors)
- V2.1: Telugu/Teluglish post native-speaker audit
- NO impact on racecontrol or POS .130 surfaces (voice is bono cloud-LEAD on WhatsApp)

---

## §2 — Wave 2 phase scope split

Phase numbering inherited from PACT-DRAFT-phase-2-dynamic-pricing-engine (bono draft; james AMPLIFIER pending body update at Wave 1 land-trigger).

### Phase 2-A: rate_table service + cafe-extension (bono cloud-LEAD, GAP-1)

**Bono substrate (reads §S-92 P1+P2 + §S-95 + §RATE-TABLE):**
- Service: rate lookup keyed on `(surface, sku_or_class, window, customer_segment_optional)` → `effective_paise`
- Schema additions: `off_peak_windows` table (window_id / surface / day_of_week / start_hh / end_hh / discount_pct), `cafe_items` extension w/ off-peak_eligible flag
- Beverages catalog with `is_coffee_or_related` + NULLABLE `cost_inr`

**james consumer:**
- Phase 2-B billing calculator queries rate_table for live rate (no snapshot per §AMEND-3.II live-rate doctrine)
- POS .130 cafe order entry surfaces off-peak discount badge when window active
- Receipt rendering shows pre-discount + discount amount + final

**Open Qs for Wave 2 PR-open:**
- Q-RATE-1: rate_table cache invalidation strategy — push (config_push) vs pull (5-min refetch per BOOT-02)?
- Q-RATE-2: off-peak window edge cases — session that crosses window boundary, what rate applies? (per-minute live-rate continuous resolution should naturally handle, but document)

### Phase 2-B: billing calculator extension (james venue-LEAD)

**Implementation scope:**
- Replace `default_strategy()` returns `&SNAP_STRATEGY` (current F25a-shipped) → flip to `&WAY_A_STRATEGY` (F25b workstream — separate PR)
- Thread `&[BillingRateTier]` + `&dyn PricingStrategy` through 5 caller files (compute_session_cost / snap_debit_amount / 3 refund fns)
- **NEW:** session_addons schema for PS5 extra controller (per-session unit per Q-OE-PS5-1):
  ```
  session_addons:
    addon_id (PK)
    session_id (FK to billing_session)
    unit: "per_session" | "per_hour" | "per_minute"
    amount_paise
    eligibility_window  (optional)
  ```
- Per-minute rounding: `round_up_to_minute(elapsed_seconds)` per P5 (Phase 2-C wallet sibling shares this)
- Viability tracking: emit `BillingSessionEnded { customer_id, surface, gross_paise, discount_paise, net_paise, ts }` to canonical RP store (cards Wave 6 break-even gate at ₹15,400/day rolling 30-day)
- Vivek anchor (existing F25a test) propagates from strategy-impl to live billing-tick path

**Open Qs for Wave 2 PR-open:**
- Q-BILL-1: session that spans an off-peak window boundary — which rate applies per minute? (recommended: live-rate-per-minute; the rate at minute N's wall-clock determines minute N's cost; unambiguous)
- Q-BILL-2: PS5 add-on charged at session-create vs session-end? (recommended: session-create — atomicity; treat as immediate top-up debit)

### Phase 2-C: wallet HOLD-RELEASE-CAPTURE (Wave 3; sibling to Phase 2-B)

**james venue-LEAD on:**
- `wallet_hold(customer_id, paise) → hold_id`
- `wallet_release(hold_id)` (cancellation path)
- `wallet_capture(hold_id, actual_paise)` (settlement)
- Per-minute rounding shared with Phase 2-B (P5 round-up)
- Atomicity: single SQLite transaction across wallet_ledger + wallet_holds tables

**Wallet-Framing-C preservation (LOCKED 2026-05-03):** Single-Purpose Voucher; 18% GST at top-up; no customer expiry; sim+PS5 only redeemable; cafe always separate.

### Phase 2-D: countdown UI + transition (cross-pilot; james UI venue-side, bono UI cloud-side)

Per P4 disposition (PWA in-pod + kiosk overlay):
- PWA: countdown banner when off-peak window starts in <15min OR ends in <15min ("Off-peak ends in 12 min — sim ₹630/hr")
- Kiosk: overlay during transition window (last 5 min of off-peak) with confirm-or-cancel CTA
- Surface contract: rate_table service emits `OffPeakWindowEvent { window_id, transition_state: starting | active | ending, seconds_remaining }` via WS

**james venue-LEAD on Kiosk overlay** (web-v2 component); **bono cloud-LEAD on PWA banner** (per Q5 split).

### Phase 2-E: combo primitive NEW (cross-pilot; gates on Captain coffee-roster Q-OE-COFFEE-1)

Per P3 + §S-95:
- `combo_offers` schema:
  ```
  combo_offers:
    combo_id (PK; explicit per P3)
    name (e.g. "Sim 2hr + coffee + cookie")
    is_coffee_paired (boolean per §S-95 first-class type)
    bundle_price_paise
    valid_from / valid_until
  combo_offer_items:
    combo_id (FK)
    surface  (sim | ps5 | cafe | beverage)
    sku_or_class
    qty
  ```
- Billing flow: combo_id detected at session-create OR cart-add → atomic application of bundle_price_paise (replaces sum of components)
- Class A items (MRP-fixed per §S-95 Q-OE-7b.2) can be bundled at MRP without extra discount

**Open Qs for Wave 2 PR-open:**
- Q-COMBO-1: combo redemption with active off-peak — combo price stands or off-peak applies on top? (recommended: combo wins; otherwise off-peak undercut paradox; document)
- Q-OE-COFFEE-1 (Captain queue): coffee-product roster — specific SKUs in V2.0 menu (espresso / cappuccino / latte / cold coffee / mocha / flat white / americano / others?) — gates Phase 2-E coffee-paired combo authoring

---

## §3 — Lead split (canonical for Wave 2 substrate; resolves first-mover-LEAD doctrine ambiguity early)

| Surface | LEAD | Reason |
|---|---|---|
| `rate_table` service + schema | bono cloud | Service runs on cloud per architecture; rate logic is cross-system |
| `customer_visit_frequency` table + ingestion | bono cloud | Extends `mesh_kb.db` v26.0 MI (cloud); session-end events feed in |
| Beverages catalog + coffee-led metadata | bono cloud | Cafe sheet authoritative; coffee-led is doctrine-class |
| `combo_offers` schema | bono cloud | Cross-system; consumed by both POS + Kiosk |
| Phase 2-B billing calculator (Rust) | **james venue** | Touches racecontrol crate + live billing-tick path |
| `session_addons` schema (PS5 extra controller) | **james venue** | Adjacent to billing_session table on venue-side |
| `wallet_hold/release/capture` (Wallet-Framing-C) | **james venue** | Wallet ledger atomicity gates on SQLite tx |
| Kiosk countdown overlay (web-v2) | **james venue** | Kiosk substrate; web-v2 V2 host on bono VPS but UI authored james-side |
| PWA countdown banner | bono cloud | PWA lives on cloud per V2-skeleton |
| MI experience-score formula (P6 5-tier discount) | bono cloud | Wave 4; cloud-side ML/heuristic |
| Wave 5 Captain-curated dry-spell WhatsApp | bono cloud | Per Q5 split — bono = WhatsApp/marketing |
| Wave 6 viability tracking dashboards | bono cloud | Admin dashboard surface |
| Session-end-event publishing | **james venue** | Phase 2-B emits; bono ingestion-side |

---

## §4 — Captain queue surfaces in Wave 2 scope (snapshot 2026-05-08 ~07:08 IST)

| Captain Q | Disposition or pending | Wave 2 phase impact |
|---|---|---|
| Q-OE-COFFEE-1 (coffee SKU roster) | PENDING | Gates Phase 2-E coffee-paired combo authoring |
| Q-OE-COFFEE-2 (60/40 messaging weight) | bono-default 60-40; Captain ratify pending | Wave 5 (not Wave 2) |
| Q-OE-COFFEE-3 (coffee-equipment scope) | bono-default V2.1+ defer | Wave 2 NOT BLOCKED |
| Refined ₹4.62L breakdown current actuals | PENDING (Captain provides) | `project_venue_financials.md` refresh; Wave 6 viability anchor |
| Q-RATE-1 (rate_table cache invalidation) | NEW (this memo) | Wave 2 PR-open |
| Q-RATE-2 (off-peak window boundary edge) | NEW (this memo) | Wave 2 PR-open |
| Q-BILL-1 (session spans off-peak boundary) | bono recommendation: live-rate-per-minute | Wave 2 PR-open |
| Q-BILL-2 (PS5 add-on at create vs end) | bono recommendation: session-create | Wave 2 PR-open |
| Q-COMBO-1 (combo + off-peak interaction) | bono recommendation: combo wins | Wave 2 PR-open |

---

## §5 — Cross-pact composition

- **PACT-DRAFT-phase-2-dynamic-pricing-engine** (bono draft): body update at Wave 1 land-trigger absorbs P1-P4 + cafe extension + Phase 2-E combo + countdown UI + this memo's Open Qs
- **PACT-024 idempotency** (Wave 3 sibling): preserves wallet HOLD-RELEASE-CAPTURE atomicity contract
- **PACT-013 wallet ledger**: sibling on Wave 3 wallet semantics
- **PACT-20260508-001 Class 5 canonical-substrate-discipline clause 6**: this memo is cross-module substrate-write at decision-time (every value/parameter consumed by ≥2 modules canonized in §RATE-TABLE before being referenced here)
- **§AMEND-3.II Foundation/Strategy/Config separation; live-rate (no snapshot)**: F25a substrate honored — Phase 2-B reads tiers fresh on each call
- **§AMEND-4 kaizen-discipline**: smallest invariant for observed requirement; no speculative scaffolding for V2.1+ scope

---

## §6 — Sequencing & milestones

Per Captain §S-82 6-wave plan (~2026-05-07 ~05:30 IST) + §S-85 Option Bravo timeline:

| Wave | Deliverable | Gate | Captain auth needed |
|---|---|---|---|
| 0 | PACT-001 Phase 1 wire-up | Sessions 1-8 per PHASE-1-WIREUP-PLAN | Per-PR at PR-open |
| 1 | Phase 1 primary billing engine static rates | F25b PR (default_strategy flip + 5 caller threading + Way A test rewrite + Vivek anchor live) | Per-PR at PR-open |
| **2** | **This memo's scope: 2-A through 2-E** | Per-phase PR | **Per-PR at PR-open + Q-OE-COFFEE-1 + Q-RATE-* + Q-BILL-* + Q-COMBO-1** |
| 3 | Wallet HOLD-RELEASE-CAPTURE | Phase 2-C PR | Per-PR |
| 4 | MI experience-score + discount-tier (P6 5-tier formula) | bono cloud-LEAD | Per-PR |
| 5 | Captain-curated dry-spell WhatsApp + atomicity gating GAP-4 | bono cloud-LEAD; Q5 split | Per-PR + Q-OE-COFFEE-2 ratify |
| 6 | V2.0 launch readiness | Viability tracking against ₹15,400/day rolling 30-day; English/Hindi/Hinglish voice pack v0.1 ship-eligible | Captain G33 launch auth |

V2.1+ scope (out of V2.0 launch):
- Telugu/Teluglish voice mirrors (gates on native-speaker audit)
- Coffee-equipment investment scope (Q-OE-COFFEE-3)
- External substrate integration (post-§S-98 SRL VMS removal — reopens with empirical baseline)
- Q-CHANGE-1..8 (V2-min offer-engine batch deferred)
- Q-PRINT-4 graceful degradation (already AGREE'd; printer offline contingency lives V2.0 too)

---

## §7 — Verification gates this memo asserts

Per CGP H3 — what this memo CAN claim vs what is NOT TESTED:

**This memo asserts:**
- Captain dispositions §S-92 through §S-99 are absorbed (every locked PARAMETER cited verbatim from canonical substrate, not memory-projection)
- Lead split is documented per first-mover-LEAD §E.7 hemisphere designation (resolves cross-pilot ambiguity before Wave 2 PR-open)
- Open Qs are enumerated for Wave 2 PR-open (5 new Qs surfaced; Captain queue aware)
- Composes-with §AMEND-3 Way A doctrine + Wallet-Framing-C + Captain customer-satisfaction-first

**NOT TESTED (this memo does not assert):**
- Bono concurs with this lead split (gates on bono AMPLIFIER vote on this memo OR Wave 2 PR-open per-PR)
- Schema details survive MMA Cross-System Bridge audit (mandatory at Phase 2-A PR-open per CGP)
- Cargo-test of Phase 2-B billing calculator extension (gates on Wave 1 land + F25b ship)
- Captain dispositions on the 5 NEW Qs in §4 (Q-RATE-* / Q-BILL-* / Q-COMBO-1)
- Q-OE-COFFEE-1 coffee SKU roster (gates Phase 2-E)
- DEPLOY PARITY readiness for Wave 2 phases (per phase PLAN.md will include `deploy:` section)

---

## §8 — Stale-at conditions

This memo durable until any of:
- (a) Wave 1 land-trigger (PACT-001 Phase 1 wire-up PR merge) — body update of PACT-DRAFT-phase-2-dynamic-pricing-engine absorbs this
- (b) Captain CHALLENGE-AMEND on §S-92 / §S-93 / §S-94 / §S-95 / §S-96 / §S-97 / §S-98 / §S-99 within 24h windows (silent-expire 2026-05-09 ~04:22 IST onwards)
- (c) bono ships rate_table service substrate before Wave 1 lands (then this memo's §2 Phase 2-A scope superseded by ship-spec)
- (d) Captain disposes Q-OE-COFFEE-1 coffee SKU roster (Phase 2-E coffee-paired combo schema concretizes)
- (e) Wave 2 PR-open (per-phase PLAN.md supersedes this memo's §2)

---

— james-venue-LEAD / 2026-05-08 ~07:08 IST · Wave 2 design absorption · P1-P9 PARAMETERs + §S-93 / §S-94 / §S-95 / §S-98 / §S-99 absorbed · Phase 2-A through 2-E scope split documented · Lead split bono-cloud-LEAD vs james-venue-LEAD per first-mover-LEAD §E.7 · 5 NEW Qs surfaced for Wave 2 PR-open · composes-with PACT-DRAFT-phase-2-dynamic-pricing-engine + §AMEND-3.II live-rate doctrine + §AMEND-4 kaizen-discipline + Wallet-Framing-C + Captain customer-satisfaction-first
