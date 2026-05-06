```json
{
  "model_name": "adversarial-auditor-v4.2",
  "step": "VERIFY",
  "score": 3,
  "findings": [
    {
      "id": "V1",
      "category": "correctness",
      "severity": "P1",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:220-240",
      "scenario": "Integer overflow in `WayAAdditiveLadder::cumulative_cost_paise` when `minutes` is large and `rate_per_min_paise` is high",
      "value_trace": [
        "1. `billable_in_tier` (u32) = min(minutes - consumed, tier_capacity) can be up to u32::MAX",
        "2. Cast to i64: `(billable_in_tier as i64)` preserves value",
        "3. `saturating_mul(tier.rate_per_min_paise)` where `tier.rate_per_min_paise` could be i64::MAX",
        "4. `saturating_add` to `total` may still overflow if intermediate product exceeds i64::MAX",
        "Example: 1,000,000 minutes at 10,000,000 paise/min → 1e13 paise total (exceeds i64::MAX)"
      ],
      "fix": "Add explicit overflow checks or switch to `u128` for intermediate calculations. Alternatively, cap maximum allowed `rate_per_min_paise` in `validate_tier_set` to prevent pathological values."
    },
    {
      "id": "V2",
      "category": "correctness",
      "severity": "P1",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:200-205",
      "scenario": "Incorrect tier filtering in `universal_tiers_ordered` when `sim_type` is `Some` but empty string",
      "value_trace": [
        "1. `sim_type.is_none()` filter excludes tiers with `Some(\"\")` (empty string variant)",
        "2. Empty string is a valid `Option<String>` but semantically equivalent to `None` for sim-agnostic tiers",
        "3. Could lead to missing tiers in the universal ladder if DB contains `Some(\"\")` entries"
      ],
      "fix": "Update filter to `t.sim_type.as_deref().map(|s| s.is_empty()).unwrap_or(true)` to treat empty strings as universal."
    },
    {
      "id": "V3",
      "category": "validation",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:380-420",
      "scenario": "`validate_tier_set` accepts `threshold_minutes = u32::MAX` which breaks `WayAAdditiveLadder`",
      "value_trace": [
        "1. Tier with `threshold_minutes = u32::MAX` and `tier_order = 1` would make all subsequent tiers unreachable",
        "2. `cumulative_cost_paise` would only ever use the first tier's rate (since `minutes - consumed` can never exceed `u32::MAX - consumed`)",
        "3. Validation only checks `threshold_minutes != 0` for last tier, not for reasonable bounds"
      ],
      "fix": "Add validation rule: `threshold_minutes` must be <= some reasonable maximum (e.g., 1000 hours = 60,000 minutes) or enforce `threshold_minutes == 0` for tiers after the first unlimited tier."
    },
    {
      "id": "V4",
      "category": "validation",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:380-420",
      "scenario": "`validate_tier_set` allows duplicate `threshold_minutes` values for different `tier_order`s",
      "value_trace": [
        "1. Two tiers with same `threshold_minutes` but different `tier_order` and `rate_per_min_paise`",
        "2. `WayAAdditiveLadder` will use the first one encountered (due to `tier_order` sorting)",
        "3. Could lead to silent mispricing if admin expects the other rate to apply at that threshold",
        "Example: tier_order=1 (threshold=30, rate=2500) and tier_order=2 (threshold=30, rate=3000) → only 2500 applies"
      ],
      "fix": "Add validation rule: `threshold_minutes` must be unique across all tiers (excluding unlimited tiers with threshold=0)."
    },
    {
      "id": "V5",
      "category": "tests",
      "severity": "P1",
      "file_line": "crates/racecontrol/src/billing_tests.rs:3320-3647",
      "scenario": "Missing test for `rate_per_min_paise` overflow in `WayAAdditiveLadder`",
      "value_trace": [
        "1. No test verifies behavior when `rate_per_min_paise` is i64::MAX or near it",
        "2. No test checks `saturating_mul` behavior with large inputs",
        "3. Edge case: `rate_per_min_paise = i64::MAX` and `minutes = 2` → should saturate at i64::MAX, not wrap"
      ],
      "fix": "Add test case with `rate_per_min_paise = i64::MAX` and verify saturation behavior for both `cumulative_cost_paise` and `rate_for_next_minute_paise`."
    },
    {
      "id": "V6",
      "category": "tests",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_tests.rs:3320-3647",
      "scenario": "No test for `threshold_minutes` overflow in tier validation",
      "value_trace": [
        "1. `validate_tier_set` doesn't check if `threshold_minutes` sum exceeds u32::MAX",
        "2. Could lead to silent integer overflow when calculating `tier_capacity` in `cumulative_cost_paise`",
        "Example: tier1.threshold=u32::MAX-1, tier2.threshold=2 → sum overflows"
      ],
      "fix": "Add test with tiers whose cumulative thresholds exceed u32::MAX and verify validation rejects it."
    },
    {
      "id": "V7",
      "category": "doctrine",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:160-180",
      "scenario": "Doctrinal violation: `default_strategy()` return type is not future-proof",
      "value_trace": [
        "1. Function returns `&'static dyn PricingStrategy` but hardcodes `&SNAP_STRATEGY`",
        "2. F25b will change this to `&WAY_A_STRATEGY`, requiring a code change and recompilation",
        "3. Violates the 'pluggable' doctrine — should fetch from config/DB at runtime",
        "4. Static linkage makes it impossible to change strategy without redeploying the binary"
      ],
      "fix": "Make `default_strategy()` read from a runtime config (e.g., feature flag or DB) to avoid recompilation for strategy changes. Return `Arc<dyn PricingStrategy>` to allow dynamic selection."
    },
    {
      "id": "V8",
      "category": "threading",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:340-350",
      "scenario": "Static singletons (`SNAP_STRATEGY`, `WAY_A_STRATEGY`) are unnecessary and limit flexibility",
      "value_trace": [
        "1. Strategies are zero-sized types, so static instances don't save memory",
        "2. Prevents dynamic strategy selection at runtime (e.g., per-customer or per-session strategies)",
        "3. `default_strategy()` could instead return a boxed trait object constructed from config"
      ],
      "fix": "Replace static singletons with a factory function (e.g., `create_strategy(name: &str) -> Box<dyn PricingStrategy>`) and store the active strategy in `AppState`."
    },
    {
      "id": "V9",
      "category": "security",
      "severity": "P1",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:150-170",
      "scenario": "Unvalidated `rate_per_min_paise` from DB could cause DoS or incorrect billing",
      "value_trace": [
        "1. `refresh_rate_tiers` validates tiers but doesn't cap `rate_per_min_paise`",
        "2. Extremely high rates (e.g., i64::MAX) could cause integer overflow in `cumulative_cost_paise`",
        "3. Negative rates (though rejected by validation) could have been exploited before validation was added",
        "4. No rate limits enforced (e.g., max 100,000 paise/min to prevent accidental misconfiguration)"
      ],
      "fix": "Add explicit bounds checking in `validate_tier_set` for `rate_per_min_paise` (e.g., 1 <= rate <= 100,000). Log warnings for rates outside expected ranges (e.g., > ₹100/min)."
    },
    {
      "id": "V10",
      "category": "correctness",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_pricing.rs:245-250",
      "scenario": "Edge case in `rate_for_next_minute_paise` when `elapsed_minutes` equals a tier's threshold",
      "value_trace": [
        "1. For a tier with `threshold_minutes = 30`, `elapsed_minutes = 30` should return the *next* tier's rate (minute 31)",
        "2. Current logic uses `elapsed_minutes < threshold_minutes`, which is correct",
        "3. But if a tier has `threshold_minutes = 0` (unlimited) and is not last, it will incorrectly match first",
        "Example: tiers = [(order=1, threshold=0, rate=1000), (order=2, threshold=30, rate=2000)]. For elapsed=0, returns 1000 (correct). For elapsed=30, should return 2000 but might return 1000 if unlimited tier is checked first."
      ],
      "fix": "Reorder the condition to check `threshold_minutes == 0` *after* bounded tiers: `if elapsed_minutes < threshold_minutes { ... } else if threshold_minutes == 0 { ... }`."
    },
    {
      "id": "V11",
      "category": "tests",
      "severity": "P2",
      "file_line": "crates/racecontrol/src/billing_tests.rs:3320-3647",
      "scenario": "Missing test for concurrent calls to `refresh_rate_tiers`",
      "value_trace": [
        "1. `refresh_rate_tiers` updates `state.billing.rate_tiers` under a write lock",
        "2. No test verifies behavior when two concurrent refreshes occur",
        "3. Potential for race conditions if validation fails in one thread but another thread's refresh succeeds",
        "4. No test for lock contention or deadlocks"
      ],
      "fix": "Add a test with two concurrent calls to `refresh_rate_tiers` (one with valid tiers, one with invalid) and verify the final state is consistent (either all valid or all previous)."
    }
  ],
  "summary": "Blocker-level integer overflow risks in `WayAAdditiveLadder` (V1) and doctrinal violations in strategy selection (V7) require fixes before F25b. Validation gaps (V3, V4) and missing edge-case tests (V5, V6) could lead to silent mispricing. Threading model (V8, V11) is technically sound but limits future flexibility. Recommend targeted fixes for P1/P2 findings and a follow-up audit after revisions."
}
```
