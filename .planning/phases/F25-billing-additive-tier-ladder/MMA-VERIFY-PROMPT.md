# F25a — MMA Step 4 VERIFY adversarial audit

You are an **adversarial code reviewer** examining a Rust diff for a real production billing system. Your job is to find bugs, design flaws, edge cases, security issues, or test-suite gaps — NOT to praise the code. Be specific. Cite line numbers. Trace specific value paths.

## Context (read carefully)

This is **F25a**, a behavior-preserving refactor. The change introduces a `PricingStrategy` trait + 2 implementations (`WayAAdditiveLadder`, `SnapPricingStrategy`) but does NOT yet route them into live customer-billing paths. The `default_strategy()` returns `&SNAP_STRATEGY` so all existing callers (which still call free functions like `snap_cost_for_minutes`) produce byte-identical output to the pre-diff code.

F25b (next PR, separate Captain auth) will:
- Flip `default_strategy()` to return `&WAY_A_STRATEGY`
- Wire callers to fetch tiers from DB and route through Strategy
- Customer pricing flips to Way A (₹2,700 for 150min vs prior snap ₹2,250)

**Vivek canonical regression** (must hold in F25b):
- 150min under Way A = 30×₹25 + 30×₹20 + 90×₹15 = ₹2,700 (270000 paise)
- F25a `f25a_waya_vivek_150min_is_2700_rupees` test asserts this directly against `WAY_A_STRATEGY.cumulative_cost_paise(150, &default_billing_rate_tiers())`.

## What was already verified (deterministic checks)

- `cargo build --release -p racecontrol-crate` exit 0 (only pre-existing warnings)
- `cargo test -p racecontrol-crate --release --lib` 1039 passed / 0 failed / 2 ignored
- 18 new F25a tests all pass:
  - Vivek anchor + 12 boundary minutes + monotonicity 0..200
  - SnapPricingStrategy byte-parity with `snap_cost_for_minutes` for 17 sample minutes
  - Empty/single-tier/sim-specific degenerate Configs
  - Tier validator: 7 cases (valid + 5 rejection types + valid-without-unlimited)

## Your job

Below is the **full diff** (`git diff origin/main` for the 2 source files; planning docs excluded). Find:

1. **Correctness bugs** in `WayAAdditiveLadder::cumulative_cost_paise` or `rate_for_next_minute_paise`. Trace specific minute counts through the implementation. Look especially for off-by-one, overflow on `i64.saturating_mul`, integer vs u32 mixing.
2. **Tier-validation gaps**: scenarios `validate_tier_set` accepts that would produce wrong Way A math. (E.g. a tier with `threshold_minutes = u32::MAX`? Two universal tiers tied at thresholds with different rates?)
3. **Test-suite gaps**: behavior the tests do NOT cover that could break F25b. Specifically: anything Way A would do differently from snap once it's the live default.
4. **Doctrinal/consistency issues**: places where the doctrine claim ("F25a is no-behavior-change") is technically violated by this diff. Even non-customer-facing changes (logs, error paths) count if they could affect production behavior.
5. **Threading/concurrency**: the strategies are static singletons (`pub static SNAP_STRATEGY: SnapPricingStrategy`). Any soundness concerns? Any reason this is wrong vs `&'static dyn` from a function?
6. **Security**: any input that flows from untrusted source (DB, admin UI) into the math without validation? `validate_tier_set` runs only inside `refresh_rate_tiers` — is there any other path?

## Output format

```json
{
  "model_name": "<your model id>",
  "step": "VERIFY",
  "score": <integer 1..5; 5 = ship as-is, 4 = ship with minor concerns, 3 = revise before ship, 2 = significant rework needed, 1 = blocker>,
  "findings": [
    { "id": "V1", "category": "correctness|validation|tests|doctrine|threading|security",
      "severity": "P0|P1|P2",
      "file_line": "path:line",
      "scenario": "specific input or condition",
      "value_trace": "if applicable, walk variables through code",
      "fix": "concrete change recommendation" }
  ],
  "summary": "<1-3 sentences: ship verdict + biggest concern if any>"
}
```

Score ≥4.0 average across panel = PASS per MMA Protocol v3.0. Be honest — if the diff is genuinely clean, score 5 with empty findings. If you find a real bug, drop the score and name it.

## DIFF (full text follows)

