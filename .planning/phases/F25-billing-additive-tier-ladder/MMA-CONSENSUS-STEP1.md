# F25 MMA Step 1 DIAGNOSE — Consensus Synthesis

**Date:** 2026-05-06 IST
**HEAD:** `989883c2`
**Panel:** 5 models targeted, 4 with structured JSON findings (DeepSeek R1 0528, DeepSeek V3.2, Gemini 2.5 Flash, Qwen3 235B). MiMo v2 Pro returned reasoning but exhausted token budget before JSON wrapper — reasoning preserved at `diagnose/reasoning-mimo.md` (24KB) for ambiguity resolution; not counted toward consensus tally.

**Vendor families with valid output:** DeepSeek (R1 + V3.2), Google (Gemini), Qwen = 3 families ≥ 3 ✓.
**Roles:** 1 reasoner (R1) + 1 code expert (V3.2) + 1 generalist (Qwen) + 1 domain (Gemini) ✓.

---

## CONSENSUS — STRONG (4/4 agree)

| Item | Severity | Decision |
|------|----------|----------|
| **Empty tier list edge case** (Q1) | P0/P1 | Strategy must handle `tiers.is_empty()` — fallback to DEFAULT_RATE=2500 paise/min, OR panic with clear message. Pick: **fallback** (production safety). |
| **Mid-session live rate change traced correctly** (Q2) | confirmed | Doctrine accepted as-is. `total_debited_paise` accumulates via per-tick deltas; new rate applied to next tick. No code change needed for behavior, but **explicit test required**. |
| **Per-minute refund must route through Strategy** (Q3) | P0 | `compute_per_minute_refund` currently calls `snap_cost_for_minutes` directly. Refactor to call `Strategy::cumulative_cost_paise(minutes_used, &current_tiers)`. Same for `compute_refund_with_rates`. |
| **Keep credit-back machinery in `record_snap_debit`** (Q4) | P1 | Negative-amount path is NOT dead under live-rate doctrine — admin rate decrease can produce negative tick deltas under Way A. R1 F4 names this directly: "non-monotonic cost after rate decrease". KEEP. (Plus future SnapToPackage contingency strategy reuses it.) |
| **DB tier validation: threshold_minutes=0 only on last tier** (Q6) | P0/P1 | Production DB allows misordered tiers. `refresh_rate_tiers()` must validate ordering + threshold_minutes=0 only on highest tier_order. R1 also suggests DB CHECK constraint as defense-in-depth. **Pick: app-side validation in v2.0 PR; DB CHECK deferred to migration sub-PACT.** |
| **PR scoping — SPLIT into F25a + F25b** (Q8) | P0 | UNANIMOUS. F25a = Strategy trait + WayAAdditiveLadder + Foundation refactor + snap-default-preserved (no behavior change, characterization tests). F25b = switch default to Way A + test rewrite + customer-pricing flip. |
| **File header — move snap to HISTORICAL block** (Q7) | P2 | UNANIMOUS pick of option (b). Preserves audit trail of doctrine evolution per kaizen. |

## CONSENSUS — MAJORITY (3/4 agree)

| Item | Severity | Decision | Dissent note |
|------|----------|----------|--------------|
| **"Customer always gets best deal" comment** (Q7) | P0/P1 | Update language. R1: "Best deal under current pricing structure". Gemini: remove. Qwen: remove. V3: move-to-HISTORICAL. **Pick R1's update wording — preserves the contract intent which still holds within Way A's tier ladder (you're charged per-tier additively, never penalized for early-quit).** |
| **Strategy must filter universal tiers** (Q1) | P2 | Document sim-specific tier filtering as future work. Defer per kaizen. |
| **Add live-rate-change test** (Q5) | P1 | Mandatory. Test: mutate `state.billing.rate_tiers` mid-session, verify next-tick debit calculates from new tiers. |
| **Add degenerate Config tests** (Q5) | P1/P2 | 0 tiers / 1 tier / unordered / threshold=0-mid scenarios. |
| **Delete obsolete snap-specific integration tests** (Q5) | P1 | `integration.rs:3666-3732` (line 3666 has self-flagged "structurally obsolete since 29dd79a8" comment) — rewrite around `cumulative_cost_paise`. Per-minute snap tests in `billing_tests.rs` also need rewrite. **Package-mode snap tests retained pending Q-PRICE-3 disposition (see open question below).** |

## SPLIT — NO CONSENSUS (2/4 vs 2/4)

### P2-2 minute-rounding asymmetry (Q3) — DEFER

| Side | Models | Argument |
|------|--------|----------|
| **Ceiling everywhere** | Gemini, V3 | "Per-minute customer pays for any partial minute used." |
| **Floor for per-minute, defer fix** | Qwen, R1 | "Per-minute debit at runtime uses `billable_seconds / 60` (FLOOR — billing.rs:238). Refund must match what was charged. Fixing P2-2 to ceiling would create a NEW asymmetry: customer charged for 29 min via per-minute debit at 29:59 driving_seconds, but refund computed against 30 min cost. Per kaizen, defer P2-2 until in/out semantics are jointly redesigned." |

**Decision: DEFER P2-2 fix from F25. Surface as Q-PRICE-3 in PR body — Captain to decide whether refund minute-rounding should align with debit-floor (qwen+R1) or with package-mode-ceiling (gemini+v3) or be redesigned holistically.**

Rationale: Qwen+R1's argument is internally stronger (refund-debit consistency under additive Way A). But this is a doctrinal call, not a technical one. Captain owns the call.

---

## RAW FINDING DENSITY

| Model | Findings | P0 | P1 | P2 |
|-------|----------|----|----|----|
| Gemini 2.5 Flash | 14 | 4 | 9 | 1 |
| Qwen3 235B | 14 | 3 | 7 | 4 |
| DeepSeek V3.2 | 9 | 3 | 4 | 3 |
| DeepSeek R1 0528 | 14 | 4 | 8 | 2 |
| MiMo v2 Pro (reasoning only) | ~14 | — | — | — |
| **Total unique finding classes** | **~12** | | | |

---

## CONSENSUS-DRIVEN DESIGN DECISIONS FOR F25a (this PR)

1. **Add `PricingStrategy` trait** to `billing_pricing.rs`:
   - `fn cumulative_cost_paise(&self, minutes: u32, tiers: &[BillingRateTier]) -> i64`
   - `fn rate_at_minute_paise(&self, elapsed_minutes: u32, tiers: &[BillingRateTier]) -> i64`
   - **Document semantics**: `cumulative_cost_paise(N)` = cost-after-N-complete-minutes. `rate_at_minute_paise(N)` = rate FOR upcoming `(N+1)`-th partial minute (used for per-second proration during a partial minute). The current `<` strict comparison in `overflow_rate_at_minute` is correct for this semantics — at `elapsed_minutes=30`, the next partial-minute is the 31st minute which is tier 2.

2. **Add `WayAAdditiveLadder` impl** with:
   - `tiers.is_empty()` guard → fallback rate 2500 paise/min, log warning
   - Tier-ordering precondition assumed to be enforced at `refresh_rate_tiers()` (paired commit, see point 6)
   - Sim-specific tier filter: `tiers.iter().filter(|t| t.sim_type.is_none())` for v2.0; sim-specific support deferred (kaizen)

3. **Add `SnapPricingStrategy` impl** that delegates to existing `snap_cost_for_minutes` — preserves current behavior for F25a (this PR), enables F25b switch-flip.

4. **Refactor `compute_session_cost`** to take `&dyn PricingStrategy` parameter (default = `&SnapPricingStrategy` in F25a), wire `_tiers` parameter through (closes P0-2 placeholder).

5. **Refactor `snap_debit_amount`** the same way — caller passes Strategy ref.

6. **Update `refresh_rate_tiers()`** to validate: tiers ordered ASC by `tier_order`; only the highest-`tier_order` tier may have `threshold_minutes=0`; `rate_per_min_paise > 0` for all. On invalid state: log error, retain previous valid in-memory state, alert.

7. **Refactor 3 refund functions** to accept `&dyn PricingStrategy`:
   - `compute_refund` / `compute_refund_with_rates` / `compute_per_minute_refund`
   - Default in F25a: SnapPricingStrategy (no behavior change)
   - **P2-2 floor-vs-ceiling: NO CHANGE in F25a** — deferred to Q-PRICE-3 Captain disposition

8. **`record_snap_debit` negative path** — keep as-is. Add doc comment: "Negative deltas occur when admin lowers rates mid-session under Way A live-rate doctrine, AND under SnapToPackage contingency strategies. Foundation MUST handle both cases."

9. **Update file header** (`billing_pricing.rs:1-9`):
   - Move existing Uday 2026-04-16 SNAP PRICING block to `// HISTORICAL (V1-era, superseded by §AMEND-3 Way A 2026-05-06):`
   - Add new header describing Foundation/Strategy/Config separation per §AMEND-3.II

10. **Update "best deal" comment** (`billing_pricing.rs:231`) to: `"Customer always gets best deal under current pricing structure (Strategy-defined; never penalized for early quit)."` — preserves contract intent, accommodates Way A.

## CONSENSUS-DRIVEN DESIGN DECISIONS FOR F25b (next PR)

1. **Switch default Strategy** from `SnapPricingStrategy` to `WayAAdditiveLadder` in `compute_session_cost` + `snap_debit_amount` + 3 refund functions. **Customer pricing flips.**

2. **Rewrite `billing_tests.rs`** Way A regression suite anchored on Vivek ₹2,700/150min:
   - `cumulative_cost_paise(150) == 270000`
   - `cumulative_cost_paise(30) == 75000`
   - `cumulative_cost_paise(60) == 135000`
   - 1 / 29 / 30 / 31 / 59 / 60 / 61 / 90 / 120 / 150 monotonic-additive verify
   - 0min = 0 cost
   - Empty tiers fallback (2500 paise/min × N)
   - Single-tier fallback
   - Live rate change mid-session test
   - Threshold=0-mid-tier rejection (in `refresh_rate_tiers` validator)
   - SnapPricingStrategy parity-test for backward-compat path

3. **Update `integration.rs:3666-3732`** — delete the snap-pricing-anchored "structurally obsolete since 29dd79a8" tests; rewrite for Way A.

## OPEN QUESTIONS FOR CAPTAIN (PR body)

- **Q-PRICE-3** (split-vote): Should refund minute-rounding align with per-minute debit floor (Qwen+R1) or package-mode ceiling (Gemini+V3)? Defer fix or bundle into F25c?
- **Q-PRICE-4** (implicit, surfaced from F25b scope): Under Way A, does the "30-min package = ₹700" pre-paid product still exist as customer-purchasable (legacy snap pricing carried forward), OR does package mode also flip to Way A math (₹750 for 30-min packaged)? Currently `compute_refund_with_rates` is called from `billing_session_end` for package-mode sessions — this is ambiguous in §AMEND-3 / §AMEND-3.II. Sub-decision before F25b ships.

## CROSS-REFERENCES

- §AMEND-3 (Captain ratify Q1) — Way A locked
- §AMEND-3.II (Captain) — Foundation/Strategy/Config separation; live-rate (no snapshot)
- §AMEND-3.III (Captain) — pre-V2 snap is V1-era; V2 launch IS comms vehicle
- §AMEND-4 (Captain) — kaizen discipline; smallest invariant; defer speculative scaffolding
- F26 (kiosk tier-ladder UI) — separate phase, after F25b ships
- F27 (POS receipt + WhatsApp template alignment) — separate phase
- F28 (admin-panel pricing editor live-rate UX) — separate phase, depends on F25b's Config wiring
- Q-PRICE-1 (§AMEND-3.III dispositioned) — closed
- Q-PRICE-2 (§AMEND-3.II D11-reversal) — closed
