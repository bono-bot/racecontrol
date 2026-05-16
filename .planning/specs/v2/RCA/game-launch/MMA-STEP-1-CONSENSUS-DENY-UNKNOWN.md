# MMA Step 1 DIAGNOSE Consensus — RCA #2 deny_unknown_fields silent-fallback class

**Authored:** bono · 2026-05-16 ~13:35 IST · 5-model OpenRouter consensus per Captain composite auth verb 13:15 IST
**Captain auth:** `"Authorize D-RCA1-1 + D-RCA2-1 (MMA Step 1 DIAGNOSE) in single composite verb (~$10 budget · ~30min wall-clock · 5-model OpenRouter per RCA · ≥3 vendor families)"` (2026-05-16 ~13:15 IST)
**Doctrine:** UNIFIED-MMA-PROTOCOL.md v4.0 · §S-220 MAOR datapoint #2 follow-up · §S-388 close-anchor
**Panel:** 5 models · 5 vendor families · ≥3-family doctrine satisfied
**Cost:** $0.077 of ~$5 budget = 1.5% utilized · ALL 5 models OK first-attempt

---

## §1 — Panel composition

| Model | Vendor | Role | Status | Tokens | Cost |
|---|---|---|---|---|---|
| deepseek/deepseek-r1-0528 | deepseek | reasoner | OK | 3211 | $0.00800 |
| qwen/qwen3-coder | qwen | code_expert | OK | 1234 | $0.00179 |
| nvidia/nemotron-3-super-120b-a12b | nvidia | sre | OK | 1825 | $0.00226 |
| google/gemini-2.5-pro | google | generalist | OK | 4496 | $0.04755 |
| moonshotai/kimi-k2.5 | moonshot | reasoner_alt | OK | 6502 | $0.01716 |

**Vendor families:** 5 distinct — exceeds ≥3 doctrine floor.

---

## §2 — Q1 Root-cause validation · **5/5 CONSENSUS**

**PRIMARY-ROOT-CAUSE (verbatim convergence across all 5 models):**
> Production-path wrapper at `ws_handler.rs:684-711` neutralizes `#[serde(deny_unknown_fields)]` contract via `unwrap_or_else` with hardcoded defaults — silently catches the deserialization error and substitutes `ks_ferrari_sf15t` + `spa` + 20+ hardcoded field defaults.

**HIERARCHY (5/5 panel-consistent):**
1. **(a) Production-path-wrapper anti-pattern** — direct active cause · the `unwrap_or_else` is what produces customer harm
2. **(b) Test-design gap** — enabler · unit-test verifies deserializer behavior but doesn't cover WS-handler production path · integration test would have caught this at PR time
3. **(c) Cross-boundary contract not propagated** — outcome description · contract DEFINED at struct + DEFINED at test but BROKEN in production wrapper

**Contributing factors (union ≥3-of-5):**
- Lack of centralized contract-parsing helper led to ad-hoc per-sim handling (5/5)
- Implicit error-classification: `serde_json::Error` treated as recoverable when it's a fatal contract violation (5/5)
- Hardcoded defaults indicate "make it work for now" hot-fix mindset · WARN-log suggests author awareness of compromise (gemini + moonshot + deepseek)
- Sibling 7-non-AC-sim raw-String pass-through shows the failure is SYSTEMIC, not isolated to AC (5/5)

**Consensus verdict:** RCA's root-cause framing is ACCURATE. The 3-layer hierarchy correctly identifies (a) as primary harm-agent.

---

## §3 — Q2 Per-site severity · **STRONG CONSENSUS on Site A1 = P0**

| Site | deepseek | qwen | nvidia | google | moonshot | Consensus | Confidence |
|---|---|---|---|---|---|---|---|
| **Site A1** (AC `Some(args)` unwrap_or_else) | **P0** | **P0** | **P0** | **P0** | **P0** | **P0 (5/5)** | 100% |
| **Site B** (non-AC raw String pass-through) | P1 | **P0** | P1 | P1 | P1 | **P1 (4/5)** | 80% |
| **Site C** (test efficacy gap) | P1 | P2 | P2 | P2 | P2 | **P2 (4/5)** | 80% |
| **Site A2** (None-branch defaults) | P2 | P1 | P1 | P1 | P1 | **P1 (4/5)** | 80% |

**Convergence:**
- **Site A1 = P0 (5/5 unanimous)** — high-frequency customer-facing harm · race launches with wrong car/track/AI when kiosk drifts. Customer billed for selection, gets defaults. Most-confident finding in entire MMA.
- **Site B = P1 (4/5)** — 7 non-AC sims with NO contract enforcement = massive latent risk · 87.5% of sim portfolio · qwen escalates to P0 due to scale.
- **Site A2 = P1** — None-branch silent-defaults masks server-side bug class · same customer-harm pattern as A1 · just different trigger (server-bug vs kiosk-drift).
- **Site C = P2** — process/QA failure · enabler but not active harm-agent · fixing test alone doesn't fix customers.

**Sharpening observation (RCA author note):** All 5 models flagged the **None-branch (A2) repeats the identical 20+ hardcoded defaults pattern** from A1. This is the **bono-AMPLIFIER amendment finding** confirmed by the panel — RCA's §1 boundary map should add A1 + A2 as sibling sub-findings.

---

## §4 — Q3 Missed root causes · **UNION** (for MMA Step 2 PLAN exploration)

Cross-panel hypotheses (≥3 models mention each):

1. **Hot-fix technical debt hypothesis** (5/5 mention) — `unwrap_or_else` likely introduced as temporary workaround during a production incident · WARN-log + comment-style suggests "get it working" pressure · follow-up to remove was lost/deprioritized
2. **Kiosk-side schema desynchronization** (4/5 mention) — TypeScript kiosk has no compile-time type validation against Rust struct · schema drift not detected at kiosk build
3. **Rolling-deploy version drift** (3/5 mention) — kiosk and agent may compile from different module versions during deploy windows
4. **Defensive-coding pattern leakage** (3/5 mention) — `unwrap_or_default` / `unwrap_or_else` patterns may have originated in test code and drifted to production
5. **Observability gap** (3/5 mention) — no metric/counter on fallback-trigger events · drift is invisible to dashboard until customer complaint

**Recommendation:** Step 2 PLAN should verify hypothesis 1 via git-blame on `ws_handler.rs:684-711` to find original commit context · validates "hot-fix that became permanent" or "intentional but flawed defensive pattern".

---

## §5 — Q4 Fix-approach · **5/5 CONVERGE on site-A-plus-foundation**

**PREFERRED-APPROACH (5/5 panel):** **site-A-plus-foundation** — tactical fix at Site A immediately + foundation helper `rc-common::parse_launch_params<T>()` to prevent recurrence + close non-AC sibling exposure class

**Convergent helper signature (model recommendations align):**

```rust
// In rc-common/src/contracts.rs
use serde::de::DeserializeOwned;

#[derive(thiserror::Error, Debug)]
pub enum ContractError {
    #[error("launch_args missing")]
    MissingArguments,
    #[error("contract violation: {0}")]
    Json(serde_json::Error),
}

pub fn parse_launch_params<T: DeserializeOwned>(
    launch_args: &Option<String>,
) -> Result<T, ContractError> {
    let args = launch_args
        .as_ref()
        .ok_or(ContractError::MissingArguments)?;
    serde_json::from_str(args).map_err(ContractError::Json)
}
```

**Per-site dispositions:**

- **Site A1 (AC `Some(args)` fallback):** replace `unwrap_or_else` with `match` on `parse_launch_params::<AcLaunchParams>(&launch_args)` · on `Err` emit `GameStateUpdate::Error` with structured error_message mirroring L638-656 Game Doctor failure path · then `return Ok(HandleResult::Continue)` (don't proceed to `state.heartbeat_status.game_running.store(true)`)
- **Site A2 (None branch):** same `parse_launch_params` returns `Err(MissingArguments)` · same error-propagation path
- **Site B (non-AC sims):** **parallel-PR blocker** — 7 sims need typed structs with `#[serde(deny_unknown_fields)]` + helper invocation. Sibling RCA scope per-sim. Foundation helper enforces consistency.
- **Site C (test efficacy):** add production-path integration test that sends `LaunchGame` WS message with unknown field · asserts agent emits `GameStateUpdate::Error` within 5s · acs.exe NOT spawned

**Production-path integration test (panel-aligned design):**

```rust
#[tokio::test]
async fn test_production_path_rejects_unknown_field() {
    let (tx, rx) = setup_test_agent().await;
    let payload = serde_json::json!({
        "car": "ks_ferrari_sf15t",
        "ai_difficulty": "easy",  // unknown — should be ai_level
    });
    let msg = build_launch_game_message(SimType::AssettoCorsa, payload.to_string());
    tx.send(msg).await.unwrap();
    let response = wait_for_state_update(&rx, Duration::from_secs(5)).await;
    assert!(matches!(response, GameStateUpdate { game_state: GameState::Error, .. }));
    assert!(response.error_message.unwrap().contains("ai_difficulty"));
    assert!(!is_acs_running());
}
```

**Trade-off summary (panel consensus):**
- Site A patch alone closes ~1/8 of total launch-class contract surface (AC only)
- Foundation helper closes the class for the WHOLE codebase + prevents future sims repeating the pattern
- Foundation helper performance overhead is negligible (single Result<T, ContractError> propagation)
- Recommend BOTH ship together as composite PR or paired PRs

---

## §6 — Q5 None-branch disposition · **5/5 AGREE on error**

**PREFERRED-DISPOSITION (5/5 panel):** **error** — agent emits `GameStateUpdate::Error` when server sends `LaunchGame { launch_args: None }` for AC

**Rationale (convergent):**
- `launch_args=None` for AC is a server-side bug — server MUST send launch_args for AC launches
- Silent defaults mask the bug and corrode system observability
- Failing fast forces server-side validation and prevents customer-facing wrong-config launches

**SERVER-CONTRACT-IMPLICATION (5/5):** YES — changes server↔agent contract. Server MUST always populate `launch_args` for AC LaunchGame messages. Server-side validation should enforce this BEFORE emitting LaunchGame. Captain explicit-ratify recommended.

---

## §7 — Q6 Recurrence-blocker · **5/5 AGREE on Foundation + clippy companion**

**PRIMARY-MEASURE (5/5):** **Foundation helper pattern enforcement** — `rc-common::parse_launch_params<T>` becomes the canonical path · ad-hoc `serde_json::from_str(...).unwrap_or_else(...)` patterns become detectable anti-pattern

**Companions (panel union):**
1. **Custom clippy lint** (4/5 mention) — detect `serde_json::from_str(...).unwrap_or_else(|_| literal_struct)` pattern · domain-specific lint rule
2. **CI integration test gate** (5/5 mention) — production-path test mandatory for every cross-boundary contract surface · part of `cargo test` gate · blocks merge if regression
3. **Code review checklist amendment** (3/5 mention) — "No struct defaults in serde error paths for cross-boundary types"
4. **Substrate-pointer doctrine annotation** (gemini specific) — CLAUDE.md should annotate `AcLaunchParams` location: `(canonical: crates/rc-agent/src/ac_launcher.rs:132)` per B1 doctrine (`feedback_v1_v2_gap_flow_plan_20260514.md`)

**Consensus position:** Foundation helper + production-path test is the MINIMUM recurrence-blocker. Clippy lint is icing — adds defense-in-depth but not strictly required given the foundation pattern centralizes the failure mode at one chokepoint.

---

## §8 — Q7 Secondary concerns (out-of-scope · panel-flagged)

Convergent secondary observations:

1. **Hardcoded defaults duplication** (5/5 mention) — `Some` and `None` branches duplicate identical 20+ fields · drift risk if `ai_level` changed in one branch but not the other · eliminating both via foundation helper closes this
2. **Non-AC error UX gap** (gemini + moonshot) — raw String pass-through (Site B) means errors surface as game crashes or downstream silent-failures · not as structured `GameStateUpdate::Error`
3. **Tracing inadequacy** (deepseek + qwen) — `tracing::warn!` for contract violations should be `error!` + metric.emit (SLO tracking · count drift events)
4. **Test fixture realism** (1/5 — moonshot) — `test_deny_unknown_fields_rejects_drift` uses synthetic JSON · should also use captured real-kiosk JSON as fixture to verify production-path contract works with realistic payloads

---

## §9 — Recommended path to Stage 2 PLAN

Per MMA Protocol v4.0 Step 2:

1. **Captain auth for Stage 2 PLAN** (~$5 budget · 5-model panel converges on fix-shape):
   - Foundation helper API surface refinement (ContractError variants · should it return `GameStateUpdate::Error` directly or via mapper)
   - Production-path integration test framework (where it lives · how it spawns mock WS handler)
   - Non-AC sim disposition: parallel-PR vs sequential vs sibling-RCA cluster
   - Kiosk-side validation strategy (Q3 hypothesis 2 follow-up)

2. **D-RCA2-1 status:** **COMPLETE** — Step 1 DIAGNOSE consensus achieved. Captain may proceed to authorize:
   - D-RCA2-2 per-PR auth Site A1+A2 fix (after Stage 2 PLAN converges)
   - D-RCA2-3 per-PR auth production-path integration test
   - D-RCA2-4 foundation helper extraction PR `rc-common::parse_launch_params` (PROMOTED to peer-of-D-RCA2-2 per bono AMPLIFIER + panel consensus)
   - D-RCA2-5 (NEW · suggested) Non-AC sim sibling RCA cluster — 7 per-sim RCAs OR 1 composite RCA covering all 7

3. **MAOR v0.1→v0.2 promotion criteria:** this consensus + RCA #1 sibling = N=2 toward N≥5. Forward window: 2026-08-13.

---

## §10 — Composes-with

- §S-388 close-anchor (MAOR Tier-2 datapoint #2 source · CRITICAL WSH-1 finding source)
- §S-146 V1↔V2 RCA gate (parent · this consensus is Step 1 of §S-146 process)
- §S-220 MAOR v0.1 ratify (datapoint #2 toward N=5 promotion)
- §S-345 soak-in-parallel-with-live (Server .23 deploy continues during fix-PR)
- §14.6.2 cascade-class-stratified RESET (Site A1/A2 fix is cross-boundary-contract-class · Class A audit-class · NO RESET unless billing-impact surfaces · likely NO RESET)
- `feedback_v1_v2_gap_flow_plan_20260514.md` B1 substrate-pointer convention (substrate-pointer-as-canonical-anchor for AcLaunchParams)
- CLAUDE.md Cross-Boundary Serialization standing rule (parent doctrine · Phase 62 / MI gap P2 historical anchor)
- `feedback_capability_claim_without_probe_20260514.md` (axis-3 independent source verification by bono AMPLIFIER · same compliance as RCA #1 consensus)

---

## §11 — Universal Sync targets

- `racecontrol/.planning/specs/v2/RCA/game-launch/MMA-STEP-1-CONSENSUS-DENY-UNKNOWN.md` ✓ (this file)
- `comms-link/data/openrouter-spend-bono.jsonl` ✓ (5 entries appended for this MMA · surface `MMA-STEP-1-RCA2-*`)
- `comms-link/V2-MASTER-STATE.md` — close-anchor §S-N+ DEFERRED to next bono session (sibling to RCA #1 consensus carry-forward)
- bono `MEMORY.md` SUPPLEMENT-16 — pickup-class note for next bono session
- `~/.claude/CLAUDE.md` harness — NOT-APPLICABLE
- james-side bilateral pickup — partner-memory-read hook on next james engagement

---

**Independence axes verification (MAOR §14.1):**
- Axis 1 (subagent type ≠ author): consensus models are external OpenRouter models · bono synthesizer ≠ james-author · ✓
- Axis 2 (no shared context): each model received only the RCA citations + structured questions · ✓
- Axis 3 (independent source-of-truth reads): bono read source files (ws_handler.rs · ac_launcher.rs) independently before authoring this consensus · ✓

**Anti-rubber-stamp briefing applied** per §S-220.5: confidence ≥75% (Site A1 = 100% panel consensus) · DO-NOT-report items suppressed · intentional patterns noted (Phase 62 contract test design intent vs production-path bypass) · explicit per-question verdicts · source-of-truth pointers cited.

---

End of MMA-STEP-1-CONSENSUS for RCA #2 deny_unknown_fields silent-fallback class.
