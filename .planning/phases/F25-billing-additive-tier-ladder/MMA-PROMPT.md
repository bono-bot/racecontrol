# F25 Billing — Additive Tier Ladder MMA Prompt

**Repo HEAD:** `989883c2` (origin/main)
**Branch:** `feat/f25-billing-additive-tier-ladder`
**Wall-clock:** 2026-05-06 IST (T+0)

---

## Step 1 DIAGNOSE — Question for the panel

You are reviewing a Rust billing-pricing change for a real production system that bills customers per minute on simulator pods. The architectural decision is **already locked** by the Captain (the human owner). Your job is **NOT** to redesign the architecture. Your job is to find bugs, edge cases, doctrinal inconsistencies, and migration risks in the proposed change shape, **before any code is written**.

Be adversarial. Trace specific values through specific lines. Cite file paths and line numbers. Reject the framing if you find a problem with the locked doctrine itself — but only if you can name the specific scenario that breaks.

---

## Locked doctrine (do not redesign)

### §AMEND-3 — Way A Additive Tier Ladder (V2.0 default)

Per-minute billing uses an **additive tier ladder** (NOT snap-to-package).

| Tier | Threshold (cumulative minutes) | Rate (paise/min) |
|------|--------------------------------|------------------|
| 1 (Standard) | 30 | 2500 (₹25) |
| 2 (Extended) | 60 | 2000 (₹20) |
| 3 (Marathon) | unlimited | 1500 (₹15) |

**Vivek canonical regression gate:**
- 150 min → 30 × ₹25 + 30 × ₹20 + 90 × ₹15 = ₹750 + ₹600 + ₹1,350 = **₹2,700 (270000 paise)**
- 30 min → 30 × ₹25 = **₹750 (75000 paise)**
- 60 min → ₹750 + 30 × ₹20 = **₹1,350 (135000 paise)**

This is a **price increase** vs prior snap pricing (snap: 30=₹700, 60=₹900, 150=₹2,250). §AMEND-3.III timeline-reframes this — V2 launch IS the customer-comms vehicle (venue closed since V1 failure → V2 reopening = pricing-reset event from customer perspective; no "promotion ended" message needed).

### §AMEND-3.II — Foundation / Strategy / Config separation

Three layers, separated by interface:

1. **Foundation** (immutable, ships V2.0): a billing accumulator that takes whatever Strategy returns and applies it. No knowledge of pricing math.
2. **Strategy** (pluggable, Way A is the v2.0 default): a trait/function shape that takes elapsed minutes + Config and returns cumulative cost in paise. Other strategies (e.g. SnapToPackage) MAY be implemented later as low-cost contingency without re-touching Foundation.
3. **Config** (admin-editable, live): tier definitions read from the DB `billing_rates` table at session-tick time. Mid-session rate changes propagate live (D11 reversed in §AMEND-3.II; no snapshot infra). In-flight sessions continue at whatever rate the DB currently holds.

### §AMEND-3.III — pre-V2 snap is V1-era code patch

The Uday 2026-04-16 snap-pricing decision (file header at `crates/racecontrol/src/billing_pricing.rs:6-9`) PRE-DATES V2 planning by ~2 weeks. It was a V1-era code patch, not V2 doctrine. F25 supersedes it.

### §AMEND-4 — kaizen discipline

Smallest invariant for observed requirement. Defer speculative scaffolding. Do not engineer pre-emptive safety nets without a concrete surface.

---

## Current code at HEAD `989883c2`

### File 1: `crates/racecontrol/src/billing_pricing.rs:241-254`

```rust
/// Snap-to-package cost for N whole minutes. Customer always gets best deal.
pub fn snap_cost_for_minutes(minutes: u32, per_min_rate: i64, pkg_30_price: i64, pkg_60_price: i64) -> i64 {
    if minutes == 0 { return 0; }
    if minutes >= 60 {
        let extra = (minutes - 60) as i64;
        return pkg_60_price + extra * (pkg_60_price / 60);
    }
    if minutes >= 30 {
        let extra = (minutes - 30) as i64;
        let cost = pkg_30_price + extra * (pkg_30_price / 30);
        return cost.min(pkg_60_price);
    }
    let linear = (minutes as i64) * per_min_rate;
    linear.min(pkg_30_price)
}
```

### File 1: `crates/racecontrol/src/billing_pricing.rs:196-229` (per-minute live caller)

```rust
pub fn compute_session_cost(elapsed_seconds: u32, _tiers: &[BillingRateTier]) -> SessionCost {
    let per_min_rate: i64 = 2500;
    let pkg_30: i64 = 70000;
    let pkg_60: i64 = 90000;

    let elapsed_secs = elapsed_seconds as i64;
    let whole_minutes = elapsed_secs / 60;
    let partial_seconds = elapsed_secs % 60;

    let base_cost = snap_cost_for_minutes(whole_minutes as u32, per_min_rate, pkg_30, pkg_60);
    let partial_cost = if partial_seconds > 0 {
        let current_rate = overflow_rate_at_minute(whole_minutes as u32, per_min_rate, pkg_30, pkg_60);
        (partial_seconds * current_rate) / 60
    } else {
        0
    };
    // ... assemble SessionCost ...
}
```

Note: `_tiers` parameter is intentionally unused (P0-2 gap, preserved as forward placeholder). Rates hardcoded at lines 197-199. This is the per-minute live caller invoked once per second by the timer tick.

### File 2: `crates/racecontrol/src/billing.rs:235-241` (per-minute debit)

```rust
/// Compute debit (or credit-back) for the next per-minute tick using snap pricing.
pub fn snap_debit_amount(&self) -> i32 {
    let billable_seconds = self.elapsed_seconds.saturating_sub(self.recovery_pause_seconds);
    let new_minutes = billable_seconds / 60;
    let target_total = crate::billing_pricing::snap_cost_for_minutes(new_minutes, 2500, 70000, 90000);
    (target_total - self.total_debited_paise as i64) as i32
}
```

Called once per per-minute-mode session every 60s. Returns delta. `record_snap_debit(amount)` (line 250-256) handles negative amounts as credit-back at snap boundaries (e.g. 30→snap-down ₹50 if linear-was-₹750). Under Way A there are no boundaries → credit-back path becomes dead under Way A but must remain accessible to other Strategy implementations (e.g. SnapToPackage contingency strategy).

### File 1: `crates/racecontrol/src/billing_pricing.rs:269-304` (3 refund paths)

```rust
pub fn compute_refund(allocated_seconds: i64, driving_seconds: i64, wallet_debit_paise: i64) -> i64 {
    compute_refund_with_rates(allocated_seconds, driving_seconds, wallet_debit_paise, 2500, 70000, 90000)
}

pub fn compute_refund_with_rates(allocated_seconds: i64, driving_seconds: i64, wallet_debit_paise: i64,
    per_min_rate_paise: i64, pkg_30_price_paise: i64, pkg_60_price_paise: i64) -> i64 {
    if allocated_seconds <= 0 || wallet_debit_paise <= 0 || driving_seconds >= allocated_seconds { return 0; }
    let minutes_used = ((driving_seconds + 59) / 60) as u32;  // CEILING minutes
    let actual_cost = snap_cost_for_minutes(minutes_used, per_min_rate_paise, pkg_30_price_paise, pkg_60_price_paise);
    let refund = wallet_debit_paise - actual_cost;
    if refund > 0 { refund } else { 0 }
}

pub fn compute_per_minute_refund(wallet_debit_paise: i64, _total_debited_paise: i64,
    _rate_paise_per_minute: i64, driving_seconds: i64) -> i64 {
    if wallet_debit_paise <= 0 { return 0; }
    let minutes_used = (driving_seconds / 60) as u32;  // FLOOR minutes (P2-2 asymmetry)
    let actual_charge = snap_cost_for_minutes(minutes_used, 2500, 70000, 90000);
    let refund = wallet_debit_paise - actual_charge;
    if refund > 0 { refund } else { 0 }
}
```

Note P2-2 asymmetry: package refund uses ceiling minutes; per-minute refund uses floor minutes. Customer gets one partial minute free on per-minute refund. F25 should bundle this fix.

### Strategy interface candidate shape (proposed)

```rust
pub trait PricingStrategy: Send + Sync {
    /// Cumulative cost in paise for N whole elapsed minutes given tier config.
    fn cumulative_cost_paise(&self, minutes: u32, tiers: &[BillingRateTier]) -> i64;
    /// Per-minute overflow rate at a given elapsed minute (for partial-second proration).
    fn rate_at_minute_paise(&self, elapsed_minutes: u32, tiers: &[BillingRateTier]) -> i64;
}

pub struct WayAAdditiveLadder;

impl PricingStrategy for WayAAdditiveLadder {
    fn cumulative_cost_paise(&self, minutes: u32, tiers: &[BillingRateTier]) -> i64 {
        // Tiers ordered by tier_order ASC; threshold_minutes is cumulative ceiling
        // (0 = unlimited; only allowed on the last tier).
        let mut total: i64 = 0;
        let mut consumed: u32 = 0;
        for tier in tiers.iter().filter(|t| t.sim_type.is_none()) {  // universal tiers only for now
            let tier_capacity = if tier.threshold_minutes == 0 {
                u32::MAX
            } else {
                tier.threshold_minutes.saturating_sub(consumed)
            };
            let billable_in_tier = (minutes - consumed).min(tier_capacity);
            total += (billable_in_tier as i64) * tier.rate_per_min_paise;
            consumed += billable_in_tier;
            if consumed >= minutes { break; }
        }
        total
    }
    fn rate_at_minute_paise(&self, elapsed_minutes: u32, tiers: &[BillingRateTier]) -> i64 {
        for tier in tiers.iter().filter(|t| t.sim_type.is_none()) {
            if tier.threshold_minutes == 0 || elapsed_minutes < tier.threshold_minutes {
                return tier.rate_per_min_paise;
            }
        }
        tiers.last().map(|t| t.rate_per_min_paise).unwrap_or(2500)
    }
}
```

The `BillingRateTier` struct already exists at `billing_pricing.rs:104-112` and is loaded from DB at `refresh_rate_tiers()` (line 135-161); the `default_billing_rate_tiers()` fixture at line 126-132 already returns the correct three-tier shape. The DB column is `threshold_minutes` (cumulative ceiling, 0 = unlimited) — Way A semantics map directly.

---

## Specific questions for the panel

**Each model: produce a numbered list of findings. For each finding, state: (severity P0/P1/P2), (file:line if applicable), (specific scenario that triggers), (proposed fix or change).**

### Q1 — Strategy interface shape
Is the proposed `PricingStrategy` trait (above) sufficient for both per-minute live billing AND refund-path math? Specifically:
- Does `cumulative_cost_paise` give enough info for the per-minute tick caller (`snap_debit_amount`) to compute a per-minute delta correctly?
- Does it cover refund computation (cumulative cost at minutes_used → wallet_debit - cumulative_cost)?
- Are there edge cases (0 min, exactly-on-threshold, > 24h sessions, empty tier list, single-tier list, sim-specific tiers) the proposed impl mishandles?

### Q2 — Live-rate (no snapshot) propagation
Per §AMEND-3.II D11-reversed, mid-session rate changes propagate live (no snapshot at session start). If admin lowers tier-2 rate from 2000 to 1500 paise/min when a customer has already accumulated 45 min:
- What does the customer see for their next tick?
- What does the receipt show at session end?
- Is there a fairness scenario this breaks? (Captain has explicitly accepted that "favourable changes propagate, unfavourable changes also propagate" — kaizen discipline closed Q-PRICE-2.)
- Trace the value of `total_debited_paise` and the next `snap_debit_amount` return value through this scenario.

### Q3 — Refund-path consistency
The 3 refund functions (`compute_refund`, `compute_refund_with_rates`, `compute_per_minute_refund`) all currently call `snap_cost_for_minutes`. Under Way A:
- Should all 3 route through the same Strategy, or do package-mode and per-minute-mode need different paths?
- The P2-2 minute-rounding asymmetry (ceiling vs floor minutes) — bundle the fix or defer? Captain's kaizen discipline says smallest invariant — argue both sides.
- For a per-minute customer who quits at exactly 30:00 driving_seconds with wallet_debit=75000 paise: what should the refund be?
- For a per-minute customer who quits at 29:59 driving_seconds with wallet_debit=75000 paise: what should the refund be?

### Q4 — Credit-back dead path
Under monotonic Way A there are no snap boundaries → `record_snap_debit(negative)` should never fire. Should we:
(a) Keep the negative-amount machinery in `record_snap_debit` (dead but harmless, preserves Strategy-interface capability for SnapToPackage contingency)?
(b) Add a debug-assert that Way A never produces negative deltas?
(c) Remove negative path entirely (simplifies but couples Foundation to Strategy)?

§AMEND-4 kaizen discipline argues for smallest change. Argue strongest interpretation.

### Q5 — Test surface
The current test suite (`crates/racecontrol/src/billing_tests.rs` ~31 hits, `crates/racecontrol/tests/integration.rs` lines 3666-3732 incl. self-flagged "structurally obsolete since 29dd79a8") is anchored on snap pricing. Way A regression suite must cover:
- Vivek 150min = ₹2,700 (270000 paise)
- 30min boundary = ₹750
- 60min boundary = ₹1,350
- 1min, 29min, 31min, 59min, 61min monotonic-additive verify
- Live-rate change mid-session (admin updates tier mid-tick → next tick uses new rate)
- Refund at 0min driving (full refund), at exact-threshold, at marathon-tier
- 0-tier and 1-tier degenerate Config

What's MISSING from this list? What tests exist today that should be DELETED outright (snap-specific math that can't be Way A-adapted)?

### Q6 — Migration safety
Database side:
- `billing_rates` table already exists per `billing_pricing.rs:135-161` (`refresh_rate_tiers`). Schema has `tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type`. Way A maps directly.
- Are there any production data states (e.g. tier with threshold_minutes=0 in middle of ladder, overlapping thresholds, missing universal tier when sim-specific exist) that break the proposed Strategy?
- Does the `default_billing_rate_tiers()` fixture (line 126-132) need to change?

### Q7 — Doctrinal mismatches
The file header at `billing_pricing.rs:6-9` describes Uday 2026-04-16 SNAP PRICING. Per §AMEND-3.III this is V1-era. Should F25 PR:
(a) Replace the header with Way A description?
(b) Move the snap header into a `// HISTORICAL:` block?
(c) Delete it?

The "Customer always gets best deal" contract at line 232 — does Way A respect this? Argue both sides. (Hint: ₹2,700 > ₹2,250 at 150min — by what definition is Way A "better deal"?)

### Q8 — PR scoping
Should F25 be a single PR or split? Candidates:
- F25a: Strategy trait + WayAAdditiveLadder + Foundation refactor (no behavior change yet — old snap fn delegates through Strategy)
- F25b: Switch default Strategy from snap to Way A (behavior change, where customer pricing flips)
- F25c: P2-2 floor-vs-ceiling refund fix
- F25d: Test rewrite

Recommend a scoping. Smallest reversible PR principle vs review-cost principle. The Captain's per-PR auth gate (each PR needs explicit authorization before merge) makes split-PR cost higher than single-PR cost — but blast radius of single-PR is bigger. Argue.

---

## Output format

Return a **JSON object** with this shape:

```json
{
  "model_name": "<your model id>",
  "step": "DIAGNOSE",
  "findings": [
    { "id": "F1", "question": "Q1", "severity": "P0|P1|P2", "file_line": "path:line", "scenario": "...", "fix": "..." },
    ...
  ],
  "consensus_recommendations": [
    "<short recommendation 1>",
    "<short recommendation 2>"
  ],
  "concerns_for_pr_author": [
    "<concern 1>",
    "<concern 2>"
  ]
}
```

Aim for 8-15 findings. Don't pad — prefer fewer high-quality findings over many shallow ones.
