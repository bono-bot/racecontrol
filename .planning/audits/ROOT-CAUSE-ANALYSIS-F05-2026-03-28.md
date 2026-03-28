# Root Cause Analysis: Why F-05 Was Missed
## Unified Protocol v3.1 | 2026-03-28

## The Bug

**F-05 (P1):** `end_billing_session()` at `billing.rs:2213` overwrites `wallet_debit_paise`
with `final_cost_paise` (per-minute rate calc) before reading it at line 2255 for proportional
refund calculation. Customer loses money on every early session end.

**Impact:** For a 30-min session (Rs.700) ended at 15 min:
- Expected refund: Rs.350 (50% of Rs.700 original debit)
- Actual refund: Rs.187.50 (50% of Rs.375 per-minute cost)
- Customer loss: Rs.162.50 per incident

---

## Timeline: Every Prior Audit That Should Have Caught This

### 1. v3.0 Billing & POS (Shipped 2026-03-24)
- Billing was built here (6 phases, 10 plans, 26 requirements)
- `end_billing_session()` was written during this milestone
- **No E2E flow test was created** for the early-end refund path
- Unit test `partial_refund_calculation` (billing.rs:3220) tests the **formula in isolation**
  but never exercises the actual `end_billing_session()` function

### 2. MMA Audit (2026-03-25, 2026-03-27)
- **32+ models** across 6 rounds, 334 raw findings, 119 bugs fixed
- **Batch 01** covered `crates/racecontrol/src/*.rs` including billing.rs (4,702 lines)
- billing.rs was in scope, but **no model flagged F-05**

### 3. Overnight Full Audit (2026-03-28)
- 5 models (GPT-5.4, Sonnet, Gemini, Nemotron, Opus synthesis)
- 3 rounds, P1s fixed
- **Billing was not in the audit scope** (focused on v26.1 in-progress work: 687 uncommitted lines)

### 4. MMA 2026-03-28 (or-*.md results, 9 models)
- DeepSeek V3, Grok 4.1, Kimi K2.5 all found billing-ADJACENT bugs:
  - Kimi: P1-003 refund race condition, P2-001 undefined variable in refund
  - Grok: BUG-001 SUM NULL, BUG-002 missing rows_affected
  - DeepSeek V3: TOCTOU in start_billing_session
- **None found F-05** — they found bugs in `refund_wallet()` (manual staff refund), not in
  `end_billing_session()` (automatic proportional refund on game exit)

---

## Root Causes (5 systemic failures)

### RC-1: Unit Test Isolation Fallacy

**The test exists but tests the wrong thing.**

```rust
// billing.rs:3220 — test_partial_refund_calculation()
let allocated: i64 = 1800;
let driving_seconds: i64 = 900;
let wallet_debit_paise: i64 = 70000;

let remaining = allocated - driving_seconds;
let refund = (remaining as f64 / allocated as f64 * wallet_debit_paise as f64) as i64;
assert_eq!(refund, 35000); // PASS — formula is correct!
```

This test passes because it uses **hardcoded `wallet_debit_paise = 70000`** — the value
that would exist if the DB column wasn't overwritten. The test never exercises the actual
`end_billing_session()` function where the column gets overwritten at line 2213 before
being read at line 2255.

**The formula is correct. The data feeding the formula is wrong.**

This is a classic unit test trap: testing the algorithm in isolation proves the math works,
but misses the integration bug where the input data is corrupted by a prior step in the
same function.

### RC-2: `end_billing_session()` Has ZERO Test Coverage

| Function | Unit Tests | Integration Tests |
|----------|-----------|------------------|
| `compute_session_cost()` | 10 tests | 0 |
| `BillingTimer.tick()` | 6 tests | 4 tests |
| `BillingTimer.remaining_seconds()` | 1 test | 1 test |
| `partial_refund_calculation` | 1 test | 1 test (indirect) |
| **`end_billing_session()`** | **0 tests** | **0 tests** |
| `wallet::credit()` | 0 | 1 test |
| `wallet::debit()` | 0 | 1 test |
| `wallet::refund()` | 0 | 1 test (indirect) |

The most critical billing function — the one that handles session end, DB updates, pod
state clearing, and refund calculation — has **zero test coverage**. The integration test
at line 1072 (`test_billing_pause_timeout_refund`) manually calculates the refund and
directly calls `wallet::refund()`, **completely bypassing `end_billing_session()`**.

### RC-3: MMA Prompt Structure Blind Spot

The MMA protocol feeds models **whole files** with a generic prompt:

> "6 standard audit categories: security, code quality, reliability, integration, process,
> infrastructure. Enhanced: 3 absence-based categories (what's missing, stuck states,
> cross-system assumptions)."

**What this catches:** Security holes, unwrap(), missing error handling, race conditions,
stuck states, SQL injection, auth bypass.

**What this misses:** Cross-line temporal data flow within a single function where:
1. A DB UPDATE changes a column value (line 2213)
2. A subsequent SELECT reads that same column (line 2255)
3. The read returns the NEW value, not the ORIGINAL value
4. The bug only manifests when `final_cost_paise != original wallet_debit_paise`

This is a **semantic data flow bug** — it requires tracing the actual value of
`wallet_debit_paise` through the UPDATE→SELECT sequence and understanding that the
pricing tier price (Rs.700) differs from the per-minute rate calculation (Rs.375 for 15min).

No model in 32+ audits was prompted to ask: "After this UPDATE, what value will the
subsequent SELECT return?"

### RC-4: billing.rs Size vs Context Window

`billing.rs` is **4,702 lines**. The bug spans lines 2213 and 2255 — only 42 lines apart.
But the surrounding function `end_billing_session()` starts at line 2158 and runs to 2335
(177 lines). The context needed to understand the bug includes:

1. Line 2178: `final_cost_paise` = per-minute rate calc (not tier price)
2. Line 2213: UPDATE overwrites `wallet_debit_paise` with `final_cost_paise`
3. Line 2255: SELECT reads `wallet_debit_paise` (now overwritten)
4. Line 2267: Refund uses the overwritten value

This is within a single screenful of code. But models processing 4,702 lines of billing
logic have attention distributed across hundreds of functions. The F-05 pattern (write then
read same column in same function) is unusual — most UPDATE→SELECT patterns involve
different tables or different columns.

### RC-5: No E2E Financial Flow Test

The Unified Protocol has 4 layers:
1. **Quality Gate** — cargo test, contract tests, syntax checks
2. **E2E Round-Trip** — exec/chain/health verification
3. **Standing Rules** — operational compliance
4. **MMA** — AI model consensus

**None of these layers test a financial flow end-to-end:**
- Quality Gate: runs `cargo test` (unit tests that don't exercise `end_billing_session()`)
- E2E: tests exec/chain endpoints, not billing
- Standing Rules: checks operational compliance, not business logic
- MMA: broad code review, not scenario simulation

The E2E workflow verification I performed today was the **first time anyone traced
actual Rupee values through the complete customer journey**. This revealed F-05 because
I calculated: "Rs.700 debited at booking → UPDATE writes Rs.375 → SELECT reads Rs.375
→ refund = 50% of Rs.375 = Rs.187.50, not Rs.350."

---

## Why Specifically No Model Caught F-05

### Models That Got Close

| Model | What They Found | Why They Missed F-05 |
|-------|----------------|---------------------|
| **Kimi K2.5** | P1-003: Refund race condition in `refund_wallet()` | Analyzed the **manual staff refund** path, not the **automatic early-end refund** in `end_billing_session()` |
| **Grok 4.1** | BUG-001: SUM NULL in refund query | Found a different bug in the same refund area — focused on NULL handling, not data flow |
| **DeepSeek V3** | TOCTOU in `start_billing_session()` | Found race at session START, didn't trace to session END |
| **DeepSeek R1** | (reasoning model) | Likely processed billing.rs but focused on state machine transitions, not column value tracing |

### The Attention Gap

All 9 models that audited billing.rs on 2026-03-28 found bugs in:
- `start_billing_session()` — TOCTOU, duplicate sessions
- `refund_wallet()` — auth bypass, SUM NULL, race condition
- Kiosk retry — duplicate debit on timeout

None found bugs in `end_billing_session()` because:
1. The function looks structurally correct — UPDATE then SELECT, standard pattern
2. The column name `wallet_debit_paise` appears to store what it describes
3. The semantic mismatch (tier price vs rate calc) requires domain knowledge of the pricing model
4. The bug only activates when `final_cost_paise != wallet_debit_paise`, which requires understanding
   that the pricing tier (Rs.700/30min) differs from the per-minute rate (Rs.25/min x 15min = Rs.375)

---

## Protocol Improvements Required

### P-1: Add Financial Flow E2E Layer (Layer 2.5)

**Current Layer 2 (E2E):** Tests exec/chain/health endpoints only.

**New Layer 2.5 (Financial E2E):** Trace actual currency values through complete flows:
1. Create customer → Topup Rs.X → Book session → Launch → End early → Verify refund
2. Create customer → Topup Rs.X → Full session → Verify no refund
3. Create customer → Topup Rs.X → Book → Cancel before start → Verify full refund
4. Topup with bonus tier → Verify bonus + journal entries

**Implementation:** Add `test_financial_e2e_*` integration tests that exercise
`end_billing_session()` directly, not just the refund formula.

### P-2: MMA Prompt Enhancement — Data Flow Tracing

Add to the MMA system prompt:

> **Category 10: Financial Data Flow**
> For any function that both WRITES to and READS from the same database table:
> 1. Trace the exact value written by UPDATE/INSERT
> 2. Trace what value a subsequent SELECT returns
> 3. Verify the SELECT returns the INTENDED value, not a value corrupted by the preceding write
> 4. Pay special attention to financial fields (paise, credits, debit, refund, cost, price)

### P-3: Critical Function Test Coverage Requirement

**Standing Rule (new):** Any function that handles money (debit, credit, refund) must have:
1. At least one integration test exercising the FULL function (not just the formula)
2. The test must use realistic values (tier price != rate x minutes)
3. The test must verify wallet balance after the complete flow

**Enforcement:** Add to quality gate: `grep -c 'end_billing_session\|start_billing_session\|wallet::debit\|wallet::credit\|wallet::refund' tests/` must be >= 1 per function.

### P-4: Scenario-Based Audit Layer

**Current MMA:** "Find bugs in this code" (code-out, bug-list)

**New layer:** "Simulate this scenario and trace values" (scenario-in, values-out)

Provide models with a concrete scenario:
> "Customer books 30-min session at Rs.700 tier. Per-minute rate is Rs.25/min.
> Game crashes at 15 min. Trace wallet_debit_paise through end_billing_session().
> What refund does the customer receive?"

This forces models to trace actual values instead of pattern-matching for generic bug classes.

### P-5: Column Mutation Detector

**Automated check:** For any function that both UPDATEs and SELECTs the same table, flag if:
- The UPDATE modifies a column that the SELECT later reads
- The two operations are in the same function scope
- The column is financial (contains "paise", "credit", "debit", "refund", "cost", "price")

This can be a simple grep-based rule in the quality gate.

---

## Summary

| Root Cause | Fix | Priority |
|-----------|-----|----------|
| RC-1: Unit test tests formula, not integration | Add integration test for `end_billing_session()` | P1 |
| RC-2: Zero test coverage on critical function | Write tests for EndedEarly + Cancelled paths | P1 |
| RC-3: MMA prompt doesn't ask for data flow tracing | Add Category 10 to audit prompt | P2 |
| RC-4: Large file dilutes model attention | Consider scenario-based prompts for billing | P2 |
| RC-5: No financial flow in E2E layer | Add Layer 2.5 financial E2E | P1 |

**Bottom line:** The audit system catches security bugs, pattern bugs, and absence bugs
effectively (119 found, 48 that Opus missed alone). But it has a structural blind spot
for **temporal data flow bugs within a single function** where a DB write corrupts a
subsequent DB read. This class of bug requires either:
1. An integration test that exercises the full flow with realistic values, OR
2. A scenario-based prompt that forces value tracing through specific code paths

Neither existed. Both should be added.
