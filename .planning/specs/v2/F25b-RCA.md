# F25b RCA — `default_strategy()` flip Snap → Way A + V1-era caller retrofit

**Doctrine basis:** `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (Captain BILATERAL directive committed at comms-link `8768b628` 2026-05-09 ~09:28 IST)

**Author:** james · **Date:** 2026-05-09 ~11:30 IST · **Branch context:** `feat/v2-wave-1-w1-s1-billing-service` HEAD `4966c234`

**Status:** DRAFT — pending Captain G33 review + bono substantive AMPLIFIER + MMA Step 1 DIAGNOSE before F25b H1 PLAN can be filed

**Foundational-boundary classification:** YES — billing boundary per doctrine §"MMA escalation". Triggers: MMA Step 1 DIAGNOSE on the RCA itself (5-model consensus on root causes) + per-PR Captain merge auth at PR-open.

**Empirical-application sequence:** F25b RCA = SECOND empirical application of `8768b628` doctrine after W1-S6 A2.c per bono §S-147.2 NIT. First was W1-S5 sliding-window auth boundary (`15490644`).

---

## Reconciliation note (read first)

F25b appears in two places in the V2 plan substrate:

| Source | Wave assignment | LEAD |
|---|---|---|
| `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` §12 SUBSTANTIVE-REPLY (`b3a88630` 2026-05-08 19:32 IST) | "F25b ready-to-resume branch `feat/f25-billing-additive-tier-ladder` thread-through pattern reuse" — Wave 1 backlog item, separate PR | james |
| `racecontrol/.planning/specs/v2/PHASE-2-WAVE-2-PLAN.md` §1.1 W2-S4 (`4966c234` 2026-05-09 10:55 IST) | "F25b Way A flip + Strategy threading — separate PR; pre-soaked from Wave 1 backlog" — Wave 2 sub-step | james venue-LEAD |

**Interpretation (proposed):** F25b is dual-citizen — it lives in Wave 1 backlog as a parallel-track separate-PR (since the F25a `default_strategy()` baseline shipped in PR #63 `fb6cb404` 2026-05-07 ~05:45 IST), AND it serves as the W2-S4 implementation in Wave 2 sequencing. Either-or framing depending on whether F25b ships before or after Wave 1 PR-open. **Recommended canonical framing:** F25b = W2-S4 (i.e., F25b ships as part of Wave 2 sequence to maintain wave-discipline ordering); Wave 1 PR-open does NOT depend on F25b.

**Captain decision required (Q-RECONCILE-1):** Confirm F25b dual-citizenship resolves to W2-S4 only (Wave 2 sequence), OR flip to Wave 1 backlog parallel-track if Captain wants Way A live earlier (e.g., to validate Way A math in production before Wave 2 surfaces ride it).

---

## §1 — Boundary map

### V1↔V2 surface inventory at billing-pricing boundary

| Path | Lines | V1-era? | V2-era? | Touched by F25b? |
|---|---|---|---|---|
| `crates/racecontrol/src/billing_pricing.rs` | 1-625 (whole file) | PARTIAL — `compute_session_cost` (498) / `snap_cost_for_minutes` (550) / `overflow_rate_at_minute` (566) / `best_rate_for_minutes` (573) / `compute_refund` (578) / `compute_refund_with_rates` (583) / `compute_per_minute_refund` (607) are V1-era pre-§AMEND-3 substrate | PARTIAL — `BillingRateTier` (132) / `PricingStrategy` trait (184) / `WayAAdditiveLadder` (216) / `SnapPricingStrategy` (295) / `default_strategy()` (327) / `validate_tier_set` (364) / `refresh_rate_tiers` (426) are V2 F25a substrate per §AMEND-3.II | YES — `default_strategy()` line 327 single-line flip + V1-era caller retrofit (axis 2 below) |
| `crates/racecontrol/src/billing_pricing.rs:327` | `pub fn default_strategy() -> &'static dyn PricingStrategy { &SNAP_STRATEGY }` | — | V2 (F25a-shipped) | YES — F25b axis 1: `&SNAP_STRATEGY` → `&WAY_A_STRATEGY` (single line) |
| `crates/racecontrol/src/billing_session_service.rs:65-72` | `SessionBillingService::compute_charge` consumes `&dyn PricingStrategy` via `default_strategy()` | — | V2 (W1-S1 SHIPPED Session 1) | INDIRECTLY — F25b axis 1 flip propagates through this V2 substrate without code change here |
| `crates/racecontrol/src/billing.rs:308` | `compute_session_cost(billable_seconds, &filtered)` — passes `&filtered` tier slice | V1 (call site) | PARTIAL (tier slice threaded but ignored) | YES — F25b axis 2: replace with `SessionBillingService::compute_charge(elapsed_minutes, &filtered, default_strategy())` OR retrofit `compute_session_cost` body to use strategy |
| `crates/racecontrol/src/billing.rs:239` | `snap_cost_for_minutes(new_minutes, 2500, 70000, 90000)` — hardcoded V1 rates | V1 | NO | YES — F25b axis 2: replace with `default_strategy().cumulative_cost_paise(new_minutes, &tiers_snapshot)` (requires read of state.billing.rate_tiers) |
| `crates/racecontrol/src/billing_orphan.rs:302` | `crate::billing::compute_refund(allocated, *driving_secs, debit)` — V1 refund (orphan reaper) | V1 | NO | YES — F25b axis 2: refund computation must use active strategy, not hardcoded snap |
| `crates/racecontrol/src/billing_recovery.rs:12+64` | `use crate::billing_pricing::compute_refund;` + `compute_refund(allocated, driving, debit)` — V1 refund (recovery path) | V1 | NO | YES — F25b axis 2: same retrofit as orphan |
| `crates/racecontrol/src/billing_session_end.rs:13+199+406` | `use crate::billing_pricing::{compute_refund, compute_per_minute_refund}` + 2 call sites | V1 | NO | YES — F25b axis 2: 2 refund call sites + 1 import |
| `crates/racecontrol/src/api/customer_disputes.rs` | uses pricing helpers | V1 | NO | LIKELY — needs grep (in dispute-refund path; V1 refund inheritance) |
| `crates/racecontrol/src/billing_timer.rs` + `billing_timer_expiry_timeout.rs` | timer-side refund/cost paths | V1 | NO | LIKELY — needs grep |
| `crates/racecontrol/src/billing_pricing.rs:498-531` | `compute_session_cost(elapsed_seconds: u32, _tiers: &[BillingRateTier]) -> SessionCost` — `_tiers` parameter UNUSED (P0-2 gap explicitly documented in source comment lines 481-497) | V1 | NO | YES — F25b axis 2: function body must consume `_tiers` (rename to `tiers`) + use strategy |
| `crates/racecontrol/src/billing_pricing.rs:607-613` | `compute_per_minute_refund(_total_debited_paise, _rate_paise_per_minute)` — both UNUSED (P0-3 gap documented lines 595-606) | V1 | NO | YES — F25b axis 2: same class as P0-2 |
| `crates/v2-db/src/lib.rs` + sibling modules | V2-DB substrate (sessions / wallets / customers / cirs / lobbies / pods) | — | V2 | NO direct touch — workspace dep direction `racecontrol-crate → v2-db` (Cargo.toml line 53) per A1.e disposition |

### Cross-organ data flow at the boundary

1. **Session tick** (rc-agent → racecontrol via `/billing/tick` or in-server timer) updates `billing_session.driving_seconds` + recomputes cost
2. **Cost computation** currently routes through V1-era `compute_session_cost` (`billing.rs:308`) which calls `snap_cost_for_minutes` with hardcoded ₹25/min, ₹700/30min, ₹900/60min — the `&filtered` tier slice is passed but ignored (P0-2 gap)
3. **Strategy boundary** sits at `default_strategy()` (`billing_pricing.rs:327`) — F25a returns `&SNAP_STRATEGY`; F25b flips to `&WAY_A_STRATEGY`
4. **SessionBillingService primary engine** (W1-S1 Session 1 SHIPPED) consumes `default_strategy()` directly — F25b axis 1 propagates Way A through this V2 path with zero code change at SessionBillingService
5. **V1-era refund flow** (`billing_session_end.rs:199 + 406`, `billing_orphan.rs:302`, `billing_recovery.rs:64`) computes refunds from `compute_refund` / `compute_per_minute_refund` — both ignore active strategy (P0-3 gap class)
6. **Receipt rendering** (`billing_invoice.rs`, `billing_summary.rs`) reads charged amount from `billing_session.cost_paise` — downstream of the cost-computation boundary; F25b doesn't touch
7. **Audit log** (refund + cost changes) — already wired through V2-DB; no F25b touch

F25b axis 1 introduces NO step in the data flow — it only changes the strategy returned by `default_strategy()`. F25b axis 2 retrofits step 2 + step 5 (cost compute + refund compute) to consume the active strategy uniformly, closing P0-2 + P0-3 gaps.

### Schema / state surfaces

- **`billing_rates` table** — V2 substrate (F25a refresh path at `billing_pricing.rs:426`); admin-edited; tier_order / tier_name / threshold_minutes / rate_per_min_paise / sim_type schema. F25b consumes existing schema; no migration.
- **`state.billing.rate_tiers` in-memory cache** — V2 substrate (`refresh_rate_tiers` writes; readers read). F25b axis 2 readers must clone-snapshot per CLAUDE.md "Never hold a lock across `.await`" rule.
- **`billing_session.cost_paise` column** — V1 schema; F25b doesn't add/modify columns.
- **`billing_session.allocated_seconds` + `driving_seconds`** — V1 schema; F25b reads via existing refund path; no schema change.
- **Vivek canonical regression contract** — 150 minutes = ₹2,700 (30×₹25 + 30×₹20 + 90×₹15) per `default_billing_rate_tiers()` lines 154-160. F25a test `test_billing_rates_create_inserts_and_cache_updates` (per Session 3 LOGBOOK row) preserves this anchor; F25b must preserve at the live billing-tick path (currently regression-tested only at strategy impl).

### Configuration surfaces

- **`BillingRateTier` default tiers** (line 154) — `default_billing_rate_tiers()` produces 30×₹25 + 30×₹20 + ∞×₹15. F25b consumption point unchanged.
- **`FALLBACK_RATE_PAISE_PER_MIN` constant** (line 176) — 2500 paise/min fallback when tier list is empty. F25b retrofitted callers must respect this fallback path.
- **`DISCOUNT_APPROVAL_THRESHOLD_PAISE` + `DISCOUNT_FLOOR_PAISE`** (lines 144 + 149) — STAFF-01 / FATM-10 gates; orthogonal to F25b.
- **PartA hardcoded rates** (`compute_session_cost:499-501` + `compute_per_minute_refund:610`) — `per_min_rate=2500 / pkg_30=70000 / pkg_60=90000`. F25b axis 2 removes these in favor of strategy/tier-driven lookup.

---

## §2 — Inherited-issue catalogue

Issues at this boundary, drawn from V1 failure-mode investigation + commit-log + LOGBOOK + ledger anchors.

| ID | Source | Issue | Scope at this boundary |
|---|---|---|---|
| P0-2 | `billing_pricing.rs:481-497` (in-source comment) | "`compute_session_cost`'s `_tiers` parameter is currently unused. Rates are hardcoded to the production defaults. If admin changes `billing_rates` via the pricing editor, `refresh_rate_tiers()` updates `state.billing.rate_tiers` but this function ignores the cache and continues to charge the old rates." | DIRECT — F25b axis 2 must close this; admin pricing editor changes must propagate to live billing tick |
| P0-3 | `billing_pricing.rs:595-606` (in-source comment) | "`compute_per_minute_refund`'s `_total_debited_paise` and `_rate_paise_per_minute` parameters are ignored; rates are hardcoded to ₹25/₹700/₹900 defaults. When admin changes pricing, refunds computed at session-end will still use the old rates." | DIRECT — F25b axis 2 same class |
| P2-2 | `billing_pricing.rs:600-606` (in-source comment) | "Minute-rounding asymmetry: `compute_per_minute_refund` uses floor-minutes; `compute_refund_with_rates` uses ceiling-minutes. Per-minute sessions currently get one partial minute free on refund." | INDIRECT — F25b retrofit may surface or fix this asymmetry; if Way A integration uses `round_up_minutes` (W1-S10 helper at `billing_session_service.rs:79-80`), asymmetry resolves IF both refund paths share the helper |
| P0-1 | `billing_pricing.rs:542-549` (in-source comment) | FIXED 2026-04-22 — boundary inversion at 29-min vs 30-min in `snap_cost_for_minutes`. Customer who quit 1 min early paid MORE than full half-hour. | NOT-APPLICABLE — already root-caused-and-fixed; F25b inherits the fix; flagged for forward-looking regression test under Way A (see §3) |
| F25a Step 4 VERIFY follow-up | `billing_pricing.rs:343-355 + 390-397` | `PathologicalThreshold` validation — `threshold_minutes == u32::MAX` rejected by `validate_tier_set` per F25a Step 4 VERIFY consensus (Mistral V3 + Nemotron V2 flagged this degenerate config) | NOT-APPLICABLE-DIRECTLY — F25b inherits validation; flagged for forward-looking test under Way A |
| F25a Snap-default doctrine | `billing_pricing.rs:286-292` | "V1-era preserved per §AMEND-3.III" + "low-cost contingency if Way A produces unforeseen customer pushback at V2 launch" | DIRECT — F25b flip preserves SnapPricingStrategy as residual; F25b is REVERSIBLE via single-line flip back to `&SNAP_STRATEGY` if Way A produces customer pushback |
| `compute_session_cost` ignores `_tiers` (P0-2 cousin) | `billing_pricing.rs:498` | Not just admin-rate-change failure — production already runs `default_billing_rate_tiers()` 30×₹25 + 30×₹20 + ∞×₹15 in `state.billing.rate_tiers`, but `compute_session_cost` ignores them and charges ₹25/min flat to 29-min then snaps to ₹700 at 30 | DIRECT — F25b axis 2 closes; live customer charge currently mismatches state cache |
| Vivek canonical regression preservation | F25b body update + tests at `billing_session_service.rs` (Session 1 unit tests already PASS for Way A math) | Vivek 150-minute = ₹2,700 contract — currently regression-tested only at the strategy impl level (not at the live billing-tick path) | DIRECT — F25b axis 2 must propagate the Vivek anchor test to live `compute_session_cost` retrofit |
| §AMEND-3.II live-rate doctrine compliance | `billing_pricing.rs:18-22 + 169-173` (in-source comment) | "In-flight sessions continue at whatever rate the DB currently holds" — VIOLATED today at `compute_session_cost` (rates hardcoded; ignores `_tiers`) | DIRECT — F25b axis 2 makes the live billing-tick path actually live-rate-respecting per doctrine |
| §AMEND-3.III V1-era audit trail | `billing_pricing.rs:30-37` (in-source comment) | V1-era SNAP doctrine preserved — "the 2026-04-16 snap-pricing decision PRE-DATES V2 planning by ~2 weeks and was a V1-era code patch, not V2 doctrine" | NOT-APPLICABLE-AS-BUG — context for F25b reversibility argument |
| `billing.rs:239` snap_cost_for_minutes hardcoded rates | `billing.rs:239` | `let target_total = crate::billing_pricing::snap_cost_for_minutes(new_minutes, 2500, 70000, 90000);` — bypasses strategy + tier slice entirely; hardcoded V1 rates | DIRECT — F25b axis 2 must retrofit OR document why bypass is acceptable (e.g., shift-extension calculator may intentionally lock V1 contract) |
| W1-S1 SessionBillingService composition (Session 1 SHIPPED `billing_session_service.rs`) | A1.e disposition (Session 1 escalation) | F25a stays in `crates/racecontrol/src/billing_pricing.rs` (NOT migrated to `crates/v2-db/`); SessionBillingService lands alongside in racecontrol | INDIRECT — F25b operates in racecontrol crate per A1.e; no v2-db migration in scope |
| V1-era 2-axis dual-rounding asymmetry | `billing_pricing.rs:585 + 609` | `compute_refund_with_rates` uses `(driving_seconds + 59) / 60` (ceiling); `compute_per_minute_refund` uses `driving_seconds / 60` (floor) — different rounding for refund-bound sessions | DIRECT — F25b axis 2 retrofit through `round_up_minutes` (W1-S10 helper) standardizes both paths |
| `compute_session_cost` `tier_name` derivation hardcoded | `billing_pricing.rs:517-523` | Tier name (Standard/Extended/Marathon) derived from elapsed_minutes thresholds (30/60), NOT from active tier set | DIRECT — F25b axis 2 must derive tier_name from active strategy + tier set OR document why receipt rendering uses positional tier name |

---

## §3 — Past-bug disposition

| Past bug at boundary | Disposition | Evidence |
|---|---|---|
| P0-1 boundary inversion at 29-min vs 30-min | **ROOT-CAUSED-AND-FIXED** 2026-04-22 — `snap_cost_for_minutes` clamps per-minute accumulation at pkg_30/pkg_60 | `billing_pricing.rs:550-563` + comment lines 542-549 |
| P0-2 `_tiers` ignored in `compute_session_cost` | **UNRESOLVED — open RCA item closed by F25b axis 2** | `billing_pricing.rs:481-497` in-source comment self-flags as "P0-2 gap (not yet fixed — deferred pending pricing-snapshot infrastructure)" |
| P0-3 hardcoded rates in `compute_per_minute_refund` | **UNRESOLVED — open RCA item closed by F25b axis 2** | `billing_pricing.rs:595-606` in-source comment self-flags as same blocker as P0-2 |
| P2-2 minute-rounding asymmetry between refund paths | **PATCHED-ONLY** — comment-flagged for "bundle this with the P0-3 pricing-snapshot fix when that infrastructure lands" | `billing_pricing.rs:600-606`. F25b axis 2 closes if `round_up_minutes` shared across refund paths (kaizen-min) |
| F25a Step 4 VERIFY `PathologicalThreshold` discovery | **ROOT-CAUSED-AND-FIXED** — `validate_tier_set` rejects `u32::MAX` threshold | `billing_pricing.rs:343-355 + 390-397` |
| F25a Strategy trait + impls compile-time correctness | **ROOT-CAUSED-AND-FIXED** — F25a shipped at PR #63 `fb6cb404` with clippy fix; CI all-green at merge | F25a merge confirmed in memory `session_progress_20260506_f25a_shipped.md` (PR #63 mergeStateStatus=CLEAN) |
| `billing.rs:239` `snap_cost_for_minutes` hardcoded for shift-extension calculator | **UNRESOLVED — open RCA item; needs disposition F25b-Q-1 below** | `billing.rs:239`. Needs Captain disposition: keep V1 contract for shift-extension OR retrofit to active strategy |
| `billing_orphan.rs` + `billing_recovery.rs` + `billing_session_end.rs` refund call sites | **UNRESOLVED — open RCA items closed by F25b axis 2** | Grep evidence above (lines 302 / 64 / 199 / 406) |
| Vivek canonical regression at live billing-tick path | **UNRESOLVED — open RCA item; F25b axis 2 must add live-path regression test** | Currently tested at strategy impl level only (`billing_pricing.rs` unit tests + `billing_session_service.rs:137-140`) |
| §AMEND-3.II live-rate doctrine compliance at `compute_session_cost` | **UNRESOLVED — open RCA item closed by F25b axis 2** | In-source comment lines 18-22 declares the doctrine; lines 481-497 acknowledge the violation |
| F25a default-Snap behavior preservation | **ROOT-CAUSED-AND-FIXED** — F25a-shipped with `default_strategy()` returning `&SNAP_STRATEGY`; behavior byte-identical to pre-F25a HEAD `989883c2` | `billing_pricing.rs:298-301` ("Delegate to the existing free function so behavior is byte-identical to pre-F25a code paths during the F25a transition") |
| W1-S1 SessionBillingService kaizen-N=1 A1.e escalation | **ROOT-CAUSED-AND-FIXED** — A1.e disposition documented in `billing_session_service.rs:10-34`; F25a stays in racecontrol crate | A1.e in-source comment + Wave 1 PLAN §12.2 escalation triggers fired |

**Open RCA items to resolve in F25b design (per doctrine §"Disposition each past bug"):**

1. **P0-2 close** — `compute_session_cost` body retrofit OR replace with `SessionBillingService::compute_charge` call at site `billing.rs:308`
2. **P0-3 close** — `compute_per_minute_refund` body retrofit + minute-rounding standardization (P2-2 cousin)
3. **`billing.rs:239` shift-extension calculator** — F25b-Q-1: retrofit to active strategy OR keep V1 contract (Captain disposition; default kaizen-min = retrofit to strategy)
4. **Live billing-tick path Vivek regression test** — add `tests/billing_session_e2e.rs` integration test that exercises live `compute_session_cost` + Way A and asserts 150-min = ₹2,700
5. **Refund minute-rounding standardization** — adopt `round_up_minutes` (W1-S10 helper) across both `compute_refund_with_rates` + `compute_per_minute_refund` retrofits
6. **`api/customer_disputes.rs` + `billing_timer*.rs` retrofit scope** — needs grep + F25b-Q-2: in-scope or out-of-scope for F25b PR (default in-scope kaizen-min)
7. **Way A under degenerate tier configs** — forward-looking regression tests for `PathologicalThreshold` + `UnlimitedNotLast` + empty-universal-tier fallback to `FALLBACK_RATE_PAISE_PER_MIN`

---

## §4 — V2-alignment delta

### What V2 doctrine says the boundary should look like

| V2 anchor | Statement | Current alignment |
|---|---|---|
| `project_v2_master_state.md` §AMEND-3 / §AMEND-3.II / §AMEND-3.III | Way A additive tier ladder is V2.0 default; live-rate doctrine (no snapshot); V1-era SNAP preserved as pluggable contingency | NOT ALIGNED at customer-facing path — `default_strategy()` returns `&SNAP_STRATEGY`; V1-era `compute_session_cost` ignores tiers |
| `feedback_v2_doctrine_alignment_drift_g9_pact_20260503_002.md` (V2-MASTER-STATE canonical-source ledger) | All V2 state changes go through ledger | NEEDS-LEDGER-ROW — F25b ship → V2-MASTER-STATE §S-N entry |
| `feedback_kaizen_discipline_dont_complicate.md` | Smallest invariant for observed requirement | BALANCED — F25b axis 1 (single-line flip) is kaizen-min; axis 2 (5-caller retrofit) is kaizen-min for closing P0-2/P0-3 gaps. Axis 2 is NOT speculative — it closes documented in-source gaps |
| `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (THIS doctrine) | RCA before action | THIS DOCUMENT is the RCA; satisfies the gate once Captain reviews |
| `feedback_emergent_directed_spend_protocol.md` Rule 4 (specify-codebase-identity) | Don't substitute mental model for environment | OK — every claim in this RCA cites a path/line/commit |
| `feedback_customer_satisfaction_first_minimal_compromise.md` (Captain rule 2026-05-07) | Customer satisfaction first; Q-PRICE-3 = FLOOR | F25b axis 1 flip changes customer-facing pricing math (Snap → Way A); customer-impact analysis required (see Q-DECISION F25b-Q-3 below) |
| §AMEND-3.II D12 Foundation/Strategy/Config separation | Strategy classes for substitutable behavior | ALIGNED-AT-STRATEGY-LEVEL (F25a-shipped); MISALIGNED-AT-CALLER-LEVEL (V1-era callers don't use strategy) |
| `project_v2_customer_workflows_consolidated_20260503.md` | 5 base + 6 missed customer scenarios — billing flows touch 4+ scenarios | HIGH-CUSTOMER-IMPACT — F25b axis 1 flip changes customer charge math at every per-minute session |
| §AMEND-4 kaizen-discipline | smallest invariant for observed requirement; no speculative scaffolding for V2.1+ scope | OK — F25b closes documented P0-2/P0-3 gaps; no V2.1+ scope creep |
| Wave 2 PLAN §3 Q-RATE-2 + Q-BILL-1 (live-rate-per-minute defaults) | Session that crosses off-peak boundary applies rate at minute N's wall-clock | F25b axis 2 retrofit makes this possible; without F25b retrofit, off-peak windows in W2-S1+S2 won't propagate to live billing |
| Wave 2 PLAN §1.1 W2-S4 LEAD designation | "F25b billing calculator Way A flip + Strategy threading — james venue-LEAD" | ALIGNED — F25b is W2-S4; james venue-LEAD per first-mover-LEAD §E.7 (parallel-track from Wave 1 backlog) |

### Named gaps

**Gap-1 (F25b axis 1 closes):** `default_strategy()` returns Snap; V2.0 default per §AMEND-3 is Way A. Single-line flip closes.

**Gap-2 (F25b axis 2 closes — P0-2):** `compute_session_cost` ignores `_tiers` parameter; admin pricing editor changes don't propagate to live billing tick. Multi-line retrofit (call-site replacement OR function-body update).

**Gap-3 (F25b axis 2 closes — P0-3):** `compute_per_minute_refund` ignores rate parameters; refunds use stale rates after admin pricing change. Multi-line retrofit (function-body update + minute-rounding standardization).

**Gap-4 (F25b axis 2 closes — refund call sites):** `billing_orphan.rs:302` + `billing_recovery.rs:64` + `billing_session_end.rs:199 + 406` + `customer_disputes.rs` (TBD grep) all use V1-era refund helpers. Retrofit each to active strategy.

**Gap-5 (F25b axis 2 partially closes — `billing.rs:239` shift-extension):** Hardcoded V1 rates in shift-extension calculator. Captain disposition F25b-Q-1 needed (retrofit OR keep V1 contract).

**Gap-6 (F25b axis 2 closes — live-path Vivek anchor):** Vivek 150-min = ₹2,700 currently regression-tested only at strategy impl + SessionBillingService unit tests; NOT at the live `compute_session_cost` retrofit path. New integration test required.

---

## §5 — V2-framed proposal

**V2 doctrine alignment:** F25b moves the customer-facing billing path from V1-era-SNAP-pricing-with-stale-tier-cache → V2 Way A live-rate per §AMEND-3.II + §AMEND-3.III. Closes 4 named gaps from `8768b628` doctrine triggers (P0-2 / P0-3 / refund-call-site / live-path Vivek).

### Implementation sketch (kaizen-min, two axes)

#### Axis 1 — Single-line `default_strategy()` flip

```rust
// crates/racecontrol/src/billing_pricing.rs:327-329
pub fn default_strategy() -> &'static dyn PricingStrategy {
    &WAY_A_STRATEGY  // F25b: was &SNAP_STRATEGY in F25a
}
```

- Effect: SessionBillingService (W1-S1) automatically uses Way A on next call
- Reversibility: single-line flip back to `&SNAP_STRATEGY` if customer pushback per §AMEND-3.III contingency
- Customer-impact: changes per-minute math from snap-with-package-clamp to additive-tier-ladder; Vivek anchor 150-min preserves ₹2,700 result

#### Axis 2 — V1-era caller retrofit (5 production sites + tests)

For each of 5 caller sites, replace V1-era helper with strategy-driven equivalent:

1. **`billing.rs:308`** `compute_session_cost(billable_seconds, &filtered)` → adopt SessionBillingService:
   ```rust
   let elapsed_minutes = round_up_minutes(billable_seconds);
   let total_paise = SessionBillingService.compute_charge(elapsed_minutes, &filtered, default_strategy());
   ```
   - Lines changed: ~4
   - Side effect: closes P0-2 gap (tiers actually consumed)
   - Risk: receipt rendering may need tier_name + minutes_to_next_tier separately — read from strategy if exposed, OR retain `compute_session_cost` shell that wraps SessionBillingService + tier_name derivation

2. **`billing.rs:239`** `snap_cost_for_minutes(new_minutes, 2500, 70000, 90000)` → Captain disposition F25b-Q-1:
   - DEFAULT (kaizen-min): retrofit to `default_strategy().cumulative_cost_paise(new_minutes, &state.billing.rate_tiers.read().await.clone())` (clone-snapshot then drop guard per CLAUDE.md "Never hold a lock across `.await`" rule)
   - Alternative: keep V1 contract for shift-extension calculator if Captain disposes shift-extension as locked-V1-pricing
   - Lines changed: ~5

3. **`billing_orphan.rs:302`** `compute_refund(allocated, *driving_secs, debit)` → retrofit to refund computation that consumes active strategy:
   - Need new function `compute_refund_via_strategy(allocated, driving, debit, &tiers, strategy) -> i64` OR retrofit `compute_refund` body
   - Recommended kaizen-min: NEW signature `pub fn compute_refund_via_strategy(...)` to avoid breaking existing test fixtures; deprecate V1 `compute_refund` in F25b+1 follow-up
   - Lines changed: ~10 (new function + caller update)

4. **`billing_recovery.rs:64`** `compute_refund(allocated, driving, debit)` → same retrofit as #3
   - Lines changed: ~3

5. **`billing_session_end.rs:199 + 406`** 2 refund call sites + 1 import:
   - Line 199: `compute_per_minute_refund(total_charged, 0, 0, driving_seconds as i64)` → retrofit
   - Line 406: `compute_refund(allocated, driven, debit)` → retrofit
   - Import line 13: update to import new strategy-driven helpers
   - Lines changed: ~8

6. **`api/customer_disputes.rs` + `billing_timer*.rs`** — F25b-Q-2 disposition: in-scope or out-of-scope (default in-scope kaizen-min)
   - Lines changed (if in-scope): ~10-15

7. **`compute_session_cost` body retrofit** at `billing_pricing.rs:498-531`:
   - Rename `_tiers` → `tiers` (consume parameter)
   - Replace hardcoded `per_min_rate=2500 / pkg_30=70000 / pkg_60=90000` with strategy + tier-driven lookup
   - Preserve `tier_name` + `minutes_to_next_tier` derivation (positional tier name acceptable per kaizen; document)
   - Lines changed: ~20

8. **`compute_per_minute_refund` body retrofit** at `billing_pricing.rs:607-613`:
   - Same class as #7
   - Adopt `round_up_minutes` for minute-rounding standardization (closes P2-2)
   - Lines changed: ~10

#### Tests (per F25b mandatory test scope)

- **Axis 1 strategy-flip regression:** existing F25a strategy unit tests preserve Way A math; new test asserts SessionBillingService consumes flipped strategy
- **Axis 2 P0-2 close:** integration test that mutates `state.billing.rate_tiers` mid-session and asserts cost reflects new rate at next tick (live-rate doctrine compliance)
- **Axis 2 P0-3 close:** integration test that ends session early, refund computed at active strategy not stale rates
- **Vivek canonical regression at live billing-tick path:** integration test exercises `compute_session_cost` retrofit + Way A; asserts 150-min = ₹2,700 (matches `default_billing_rate_tiers()` 30×₹25 + 30×₹20 + 90×₹15)
- **Refund minute-rounding symmetry:** test asserts `compute_refund_via_strategy` (or equivalent) and `compute_per_minute_refund_via_strategy` produce identical refunds for identical session profiles (closes P2-2 documented asymmetry)
- **Way A under degenerate configs:** test `validate_tier_set` PathologicalThreshold rejection still fires; empty-universal-tier fallback uses `FALLBACK_RATE_PAISE_PER_MIN`
- **Reversibility test:** flip `default_strategy()` back to `&SNAP_STRATEGY`; existing snap pricing unit tests + integration tests pass unchanged (proves §AMEND-3.III contingency intact)

Estimated test count: 8-12 new tests across `billing_pricing.rs::tests` + `billing_session_service.rs::tests` + `tests/billing_session_e2e.rs`

#### Cross-pilot impact (POS browser + receipt rendering)

- **POS .130 web-v2:** receipt rendering reads `billing_session.cost_paise` (already populated by V2 path); F25b axis 1 changes the value but not the schema. Visual verification required per CLAUDE.md Ultimate Rule #4 — staff completes a real session through Way A on POS browser; verify receipt shows correct math.
- **PWA in-pod:** customer balance display reads cost from server; same as POS (downstream of cost-computation boundary).
- **Out of F25b scope to MODIFY POS code; in scope to DOCUMENT the contract change** (NOTIFY bono via INBOX before merge: "Way A pricing now active; verify POS browser renders Way A math correctly on next staff-end-session interaction").

#### DEPLOY PARITY scope (per CLAUDE.md DMP-MANDATORY rule)

- **racecontrol binary:** REBUILD + redeploy to Server .23 + Bono VPS racecontrol
- **rc-agent:** NO change
- **POS Web app:** NO code change (consumes server-computed cost; rendering unchanged)
- **Admin/Kiosk:** NO change
- **Comms-link:** NO change
- **SWAPLOG row + LOGBOOK row required at deploy time**

#### Memory-file updates triggered by F25b ship

- `project_v2_master_state.md` → §S-N entry naming F25b ship + §AMEND-3 ratify-status update (Way A is now LIVE-DEFAULT; Snap is contingency-only)
- `MEMORY.md` → index entry with ⭐⭐⭐ marker
- `LOGBOOK.md` row at racecontrol root
- amend `billing_pricing.rs:26-28` comment to remove "F25b will flip" forward-looking language (replace with "F25b SHIPPED 2026-MM-DD" historical anchor)

### Estimated size

- Production code: ~70-80 LOC (axis 1: 1 LOC; axis 2: ~70-80 LOC across 5 caller files + 2 function-body retrofits)
- Test code: ~250-300 LOC (8-12 new tests at 25-30 LOC each)
- Documentation: 4 memory files + LOGBOOK + billing_pricing.rs comment + V2-MASTER-STATE row
- Risk surface: foundational billing boundary; MMA Step 1 DIAGNOSE required (per doctrine)
- Estimated session length: ~3-4 hours for code + ~30 min for memory + ~30 min for MMA Step 1 + Captain auth wait

### Open Captain Q-DECISIONs surfaced by this RCA

| ID | Question | Default if Captain doesn't disposition |
|---|---|---|
| F25b-Q-RECONCILE-1 | Confirm F25b dual-citizenship resolves to W2-S4 only (Wave 2 sequence)? | DEFAULT: YES — F25b ships as W2-S4 in Wave 2 sequence; Wave 1 PR-open does NOT depend on F25b |
| F25b-Q-1 | `billing.rs:239` shift-extension calculator: retrofit to active strategy OR keep V1 contract? | DEFAULT: retrofit to active strategy (kaizen-min consistent; admin rate change should propagate) |
| F25b-Q-2 | `api/customer_disputes.rs` + `billing_timer*.rs`: in-scope or out-of-scope for F25b PR? | DEFAULT: in-scope (kaizen-min consistent; close all V1-era refund inheritance in single PR) |
| F25b-Q-3 | Customer-impact analysis required pre-merge? Way A vs Snap charges differ for some session profiles. | DEFAULT: YES — author customer-impact comparison table (Snap vs Way A for representative session profiles) before PR-open per Captain customer-satisfaction-first rule |
| F25b-Q-4 | Adopt new helper signatures (`compute_refund_via_strategy`) OR retrofit V1 helper bodies? | DEFAULT: NEW signatures (preserves test fixtures; deprecate V1 in F25b+1 follow-up) |
| F25b-Q-5 | F25a Snap-default reversibility window — keep `&SNAP_STRATEGY` reachable as contingency? | DEFAULT: YES — preserve `&SNAP_STRATEGY` static + SnapPricingStrategy impl per §AMEND-3.III; only flip `default_strategy()` return value |
| F25b-Q-6 | `compute_session_cost` `tier_name` derivation — keep positional (Standard/Extended/Marathon thresholds 30/60) OR derive from active tier set? | DEFAULT: keep positional for V2.0 (deterministic for receipt rendering); derive-from-tier-set is V2.1+ scope when admin can edit tier names |

---

## NOT TESTED (RCA AUTHORING phase — pre-implementation)

This is an authoring artifact, not a runtime fix. Items NOT exercised:

- **The proposed code change** — implementation is F25b PR scope; this RCA is the gate-precursor only
- **MMA Step 1 DIAGNOSE on this RCA** — gated on Captain budget approval (~$2-5 OpenRouter); 5-model consensus on root causes per doctrine §"MMA escalation"
- **bono substantive AMPLIFIER** — bilateral doctrine; bono review of this RCA pending (Axis-A INBOX block expected; bono picks up via session-start git_pull per W1-S5 precedent)
- **Captain G33 ratification of F25b-Q-RECONCILE-1 + F25b-Q-1..Q-6** — disposition-needed before F25b implementation can proceed
- **Per-PR Captain merge auth at PR-open** — gate STANDS for the F25b PR (not this RCA artifact PR; though the RCA itself lands directly on wave-1 branch precedent: WAVE-2-DESIGN-NOTES + W1-S5-RCA + Wave 2 PLAN all here)
- **POS browser real-receipt-rendering test under Way A** — gates on F25b implementation + DEPLOY PARITY ship
- **Production-shape concurrent staff session load (Way A under contention)** — separate workstream
- **`api/customer_disputes.rs` + `billing_timer*.rs` grep** — needs full enumeration before F25b PR scope finalizes; default in-scope kaizen-min per F25b-Q-2
- **Customer-impact comparison table (Snap vs Way A)** — pending F25b-Q-3 disposition; required pre-merge per Captain customer-satisfaction-first rule
- **Memory-file Universal Sync** for the bono mirror of this RCA — TBD per W1-S5 RCA precedent (probably NO since project planning doc not project-scope feedback rule; flag for Captain confirmation)
- **`billing.rs:239` shift-extension calculator** F25b-Q-1 disposition path
- **`billing_pricing.rs:498` `compute_session_cost` retrofit vs replacement choice** — function-body update vs call-site replacement; kaizen-min recommends function-body update preserving signature for downstream callers (e.g. tier_name + minutes_to_next_tier consumers)

---

## Read trail

- `feedback_v1_dependent_v2_root_cause_before_proceeding.md` (doctrine; commit `8768b628` 2026-05-09 ~09:28 IST)
- `crates/racecontrol/src/billing_pricing.rs:1-625` (F25a substrate full-read 2026-05-09 ~11:25 IST)
- `crates/racecontrol/src/billing_session_service.rs:1-80` (W1-S1 SessionBillingService — A1.e disposition + composes-with F25a Strategy trait)
- `crates/racecontrol/src/billing.rs:239` + `:308` (V1-era call sites for snap_cost_for_minutes + compute_session_cost)
- `crates/racecontrol/src/billing_orphan.rs:302` (V1-era refund call site — orphan reaper)
- `crates/racecontrol/src/billing_recovery.rs:12 + :64` (V1-era refund call site — recovery path)
- `crates/racecontrol/src/billing_session_end.rs:13 + :199 + :406` (V1-era refund call sites — session-end path)
- `racecontrol/.planning/specs/v2/PHASE-1-WAVE-1-PLAN.md` §12.5 (5 Session 1 W1-S1 anchors pinned; F25b ready-to-resume branch reference)
- `racecontrol/.planning/specs/v2/PHASE-2-WAVE-2-PLAN.md` §1.1 W2-S4 (F25b Way A flip + Strategy threading; james venue-LEAD)
- `racecontrol/.planning/specs/v2/W1-S5-RCA.md` (RCA structure mirror; 261 lines; first empirical application of `8768b628` doctrine)
- PR #63 merge `fb6cb404` 2026-05-07 ~05:45 IST (F25a substrate ship; default_strategy=Snap; behavior unchanged baseline)
- `feedback_customer_satisfaction_first_minimal_compromise.md` (Captain rule 2026-05-07 ~03:05 IST)
- §AMEND-3 / §AMEND-3.II / §AMEND-3.III (V2 PRICING DOCTRINE — Foundation/Strategy/Config separation + live-rate + V1-era preserved)

---

— james / 2026-05-09 ~11:30 IST · F25b RCA DRAFT authored under standing autonomy "Proceed with your recommendation that is aligned with Racing Point ecosystem v2 development. Proceed autonomously" 2026-05-09 ~10:51 IST · gates on Captain G33 review of F25b-Q-RECONCILE-1 + F25b-Q-1..6 + bono substantive AMPLIFIER + MMA Step 1 DIAGNOSE before F25b H1 PLAN can be filed · per-PR Captain merge auth gate STANDS at F25b PR-open (foundational billing boundary) · second empirical application of `8768b628` V1-dependent V2 RCA doctrine after W1-S5 sliding-window auth boundary
