# F25 — Billing Additive Tier Ladder — Implementation Plan

**Captain doctrine:** §AMEND-3 Way A locked + §AMEND-3.II Foundation/Strategy/Config separation + §AMEND-3.III pre-V2 snap is V1-era + §AMEND-4 kaizen.
**MMA Step 1 consensus:** `MMA-CONSENSUS-STEP1.md` (4/5 models, vendor-diverse).
**Branch:** `feat/f25-billing-additive-tier-ladder` from `origin/main` HEAD `989883c2`.

---

## Scope split — UNANIMOUS MMA recommendation

### F25a (this PR — this session)

**Behavior:** ZERO change for customer. Pure structural refactor that introduces the Foundation/Strategy/Config separation and shipsboth Strategy implementations (Snap + WayA) in code. Default Strategy stays Snap → customer pricing unchanged. Vivek regression NOT exercised yet.

**Why split:** §AMEND-4 kaizen + 4/4 MMA consensus on PR scoping. Smallest reversible change first; behavior flip second.

**Diff target:** ~400-600 lines mostly in `crates/racecontrol/src/billing_pricing.rs`, `billing.rs`, `billing_session_end.rs`, `billing_timer_expiry_timeout.rs`, `billing_tests.rs`. Possibly minor touches to `state.rs` for Strategy storage (or thread the strategy ref through call-sites without state-storage — kaizen pick).

**Test discipline:** Characterization first. Existing snap-pricing tests must STILL pass byte-identical (parity proves no behavior change). New tests cover Way A math + tier validation + degenerate Configs.

### F25b (next PR — next session)

**Behavior:** Customer pricing FLIPS to Way A. ₹2,700/150min. Vivek regression gate fires.

**Diff target:** ~50-100 lines (one-line default-Strategy swap × call-sites, plus snap-test deletion + Way A test rewrite).

**Why this is "small":** F25a does all the structural heavy-lifting. F25b is just `impl Default for SessionPricingContext` flipping from `SnapPricingStrategy` to `WayAAdditiveLadder` + test corrections.

---

## F25a action list — ranked by risk

### A1. Add `PricingStrategy` trait + 2 impls (LOW risk)
- File: `crates/racecontrol/src/billing_pricing.rs`
- Add trait + WayAAdditiveLadder + SnapPricingStrategy
- WayAAdditiveLadder.cumulative_cost_paise: empty-tiers fallback (rate=2500, log warn)
- WayAAdditiveLadder.rate_at_minute_paise: documented `<` semantics ("rate FOR upcoming N+1th partial-minute")
- SnapPricingStrategy delegates to existing free fn (no logic change, just re-encapsulates)
- **Risk:** trait-object dispatch cost in hot path (per-second tick). **Mitigation:** inline calls; benchmark if doubt; the trait method is ~10 instructions max. Cost is acceptable.
- **Rollback:** `git revert <commit>` — Strategy trait is purely additive, removing it doesn't affect compilation of unchanged callers.

### A2. Add tier-validation in `refresh_rate_tiers` (LOW risk, defensive)
- File: same
- Validate: tiers ASC by tier_order; threshold_minutes=0 only on highest tier_order; rate_per_min_paise > 0
- On invalid: log error, retain previous valid in-memory state
- DB CHECK constraint: deferred to migration sub-PACT (per consensus)
- **Risk:** misordered prod data could log spam. **Mitigation:** rate-limit warning to 1/hour per validation failure.
- **Rollback:** validation is a wrapper around existing fetch; revert removes validator only.

### A3. Refactor `compute_session_cost` to take `&dyn PricingStrategy` (MEDIUM risk)
- File: same
- Add new arg: `strategy: &dyn PricingStrategy` (or `impl PricingStrategy` for monomorphization in non-hot-path).
- Wire `_tiers` (closes P0-2 placeholder gap from line 178-195 doctrine).
- Update all call-sites at billing.rs / billing_session_end.rs / billing_timer_expiry_timeout.rs.
- **Risk:** signature change ripples. Mitigation: Rust compiler catches all callers; characterization tests prove behavior preservation.
- **Rollback:** revert + manual re-instantiation of removed args.

### A4. Refactor `snap_debit_amount` to take Strategy (MEDIUM risk)
- File: `crates/racecontrol/src/billing.rs:236-241`
- Same pattern as A3.
- **Risk:** per-tick caller; performance-sensitive. **Mitigation:** single virtual call per tick (60s cadence per session). Negligible.

### A5. Refactor 3 refund functions to take Strategy (MEDIUM risk)
- File: `billing_pricing.rs:269-304`
- `compute_refund` / `compute_refund_with_rates` / `compute_per_minute_refund` all take strategy
- Delete `compute_refund` thin wrapper if call-sites can pass strategy directly (kaizen).
- **P2-2 floor-vs-ceiling: NO CHANGE in F25a.** Captain disposition Q-PRICE-3.
- **Rollback:** revert.

### A6. Remove `_tiers` placeholder param from compute_session_cost (LOW)
- Replace with real `tiers` param threaded from caller through Strategy.
- Cleans P0-2 gap doctrine in `billing_pricing.rs:178-195`.

### A7. Update file header (`billing_pricing.rs:1-9`) (TRIVIAL)
- Move Uday 2026-04-16 SNAP block to `// HISTORICAL (V1-era, superseded by §AMEND-3 Way A 2026-05-06):`
- Add new header for Foundation/Strategy/Config separation
- **Rollback:** revert.

### A8. Update "best deal" comment (`billing_pricing.rs:231-232`) (TRIVIAL)
- Update wording to: "Customer always gets best deal under current Strategy (never penalized for early quit)."
- **Rollback:** revert.

### A9. Tests — characterization (`billing_tests.rs`) (LOW)
- ALL existing snap-pricing tests must pass byte-identical via SnapPricingStrategy.
- Add tests for WayAAdditiveLadder math (Vivek 150min anchor + boundaries, NOT exercised against the live billing code path — only against the strategy impl directly. F25b moves these to live-path tests).
- Add tests for tier-validation rejection (threshold=0-mid; unordered).
- Add tests for empty-tier fallback (cumulative_cost_paise(N) == N × 2500).
- **Rollback:** test additions only — never breaks behavior.

### A10. PR body (TRIVIAL)
- Reference §AMEND-3 / §AMEND-3.II / §AMEND-3.III / §AMEND-4
- Link MMA-PROMPT.md + MMA-CONSENSUS-STEP1.md
- Q-PRICE-3 + Q-PRICE-4 surfaced for Captain disposition
- F25b ready-to-resume handoff section
- Explicit "Pending: Captain per-PR merge auth"

---

## Risk matrix

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Behavior change leaks into F25a | Low | High (customer pricing) | Characterization tests + SnapPricingStrategy parity test suite required-passing |
| Trait dispatch perf in tick hot path | Very low | Low | Tick is 1/sec; one virtual call ≈ 10ns. Negligible. |
| Tier validation rejects valid prod state | Low | Medium | Retain previous in-memory state on invalid; rate-limited warning |
| Q-PRICE-3 / Q-PRICE-4 unresolved blocks F25b | Medium | Medium | F25a doesn't depend on these answers; F25b waits for Captain |
| `racecontrol-f2/` worktree contains parallel work that conflicts | Low | Low | F25a touches billing files only; f2 worktree is on `feat/f2-temporal-invariant-check` (orthogonal scope). Verify no conflict at PR time. |

---

## MMA Step 4 VERIFY plan (post-implementation)

Per MMA Protocol v3.0:
1. **Deterministic checks first:**
   - `cargo build --release -p racecontrol-crate` exit 0
   - `cargo test -p racecontrol-crate` all green (existing snap tests + new Way A unit tests)
   - `WayAAdditiveLadder.cumulative_cost_paise(150, &default_billing_rate_tiers()) == 270000` (Vivek anchor, asserted at impl-direct call, not through the live billing flow)
   - `SnapPricingStrategy.cumulative_cost_paise(150, ...) == snap_cost_for_minutes(150, 2500, 70000, 90000)` (parity)
2. **3-model adversarial audit on the diff** — different models from Step 1. Pick: Mistral Medium + Grok Code Fast 1 + Nemotron 70B (3 vendor families NOT in Step 1 panel). Ask: "Does the diff preserve customer-billing behavior under SnapPricingStrategy default? Does WayAAdditiveLadder math match the Vivek 150min=₹2,700 anchor? Are there overflow / underflow / off-by-one issues in the trait implementations?"
3. **Score ≥4.0 (out of 5) = PASS.** Below 4.0 → backtrack to A1-A9 fix.

---

## Definition of "F25a SHIPPED"

(Per CLAUDE.md "Shipped Means Works For The User" — but F25a is structural-only, no user-facing behavior:)

- [ ] Code committed on `feat/f25-billing-additive-tier-ladder`
- [ ] `cargo build --release` green at HEAD of branch
- [ ] `cargo test -p racecontrol-crate` green; SnapPricingStrategy parity tests pass
- [ ] PR opened on GitHub
- [ ] PR body includes: §AMEND links, MMA artifacts links, Q-PRICE-3 + Q-PRICE-4, F25b ready-to-resume section, explicit "Pending: Captain per-PR merge auth"
- [ ] MMA Step 4 VERIFY adversarial pass
- [ ] LOGBOOK row appended for the F25a commit
- [ ] V2-MASTER-STATE §S-N appended noting F25 split decision + F25a PR opened
- [ ] NO MERGE this session — Captain per-PR auth gate held
- [ ] NO DEPLOY this session — F25a is no-behavior-change but per-PR gate stands

(Definition of "F25 fully SHIPPED" is F25b's Captain auth + merge + Vivek regression assertion runs against live billing path.)
