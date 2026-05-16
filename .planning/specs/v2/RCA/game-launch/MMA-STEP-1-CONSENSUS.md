# MMA Step 1 DIAGNOSE Consensus — RCA #1 Lock-across-await pattern class

**Authored:** bono · 2026-05-16 ~13:30 IST · 5-model OpenRouter consensus per Captain composite auth verb 13:15 IST
**Captain auth:** `"Authorize D-RCA1-1 + D-RCA2-1 (MMA Step 1 DIAGNOSE) in single composite verb (~$10 budget · ~30min wall-clock · 5-model OpenRouter per RCA · ≥3 vendor families)"` (2026-05-16 ~13:15 IST)
**Doctrine:** UNIFIED-MMA-PROTOCOL.md v4.0 · §S-220 MAOR datapoint #2 follow-up · §S-388 close-anchor
**Panel:** 5 models · 5 vendor families · ≥3-family doctrine satisfied
**Cost:** $0.074 (RCA #1 primary $0.057 + nvidia/moonshot retry $0.017) of ~$5 budget = 1.5% utilized

---

## §1 — Panel composition

| Model | Vendor | Role | Status | Tokens | Cost |
|---|---|---|---|---|---|
| deepseek/deepseek-r1-0528 | deepseek | reasoner | OK | 3150 | $0.00773 |
| qwen/qwen3-coder | qwen | code_expert | OK | 1135 | $0.00162 |
| nvidia/nemotron-3-super-120b-a12b | nvidia | sre | OK (retry) | 4500 | $0.00243 |
| google/gemini-2.5-pro | google | generalist | OK | 4496 | $0.04725 |
| moonshotai/kimi-k2.5 | moonshot | reasoner_alt | OK (retry) | 4500 | $0.01449 |

**Vendor families:** 5 distinct (deepseek + qwen + nvidia + google + moonshot) — exceeds ≥3 doctrine floor.

---

## §2 — Q1 Root-cause validation · **5/5 CONSENSUS**

**PRIMARY-ROOT-CAUSE (verbatim convergence across all 5 models):**
> "Lock-across-await with `tokio::sync::RwLock` under 8-pod concurrent launch storm" IS the correct proximate root-cause framing for Sites A/B/C/D.

**Secondary factors (union across panel · ≥3 models agreed on each):**
- Shared-state coupling: centralized `RwLock<HashMap<pod_id, _>>` design forces every pod interaction to a single contention point (deepseek + qwen + google + moonshot + nvidia · 5/5)
- Missing static enforcement: v27.0 MMA P1 ratified doctrine textually, never enabled `clippy::await_holding_lock` (5/5)
- I/O latency unbounded across await points (sqlx UPDATE, mpsc send) coupled with held guards (5/5)
- Site B post-hoc `drop(senders)` at L434 = developer-aware-but-mechanism-wrong (moonshot · individually flagged)

**Deeper underlying issue (alternative framing · 5/5 acknowledge):**
> Architectural debt: `agent_senders` should be migrated to actor-per-pod model or DashMap to eliminate centralized locking. The lock-across-await pattern is the IMMEDIATE bug; the central-lock architecture is what MAKES it catastrophic under load.

**Consensus verdict:** Root-cause framing in §S-146 RCA is ACCURATE. Architectural-debt is contributing-but-not-replacement.

---

## §3 — Q2 Severity rank per site · **PARTIAL CONSENSUS**

**Per-site verdicts:**

| Site | deepseek | qwen | nvidia | google | moonshot | Consensus | Confidence |
|---|---|---|---|---|---|---|---|
| **Site C** (compound nested) | **P0** | **P0** | **P0** | **P1** | **P0** | **P0 (4/5)** | 80% |
| **Site A** (billing-adjacent + sqlx) | P1 | **P0** | P0 | **P0** | **P0** | **P0 (4/5)** | 80% |
| **Site B** (RE-relaunch spawn) | P2 | P1 | P1 | P1 | P1 | **P1 (4/5)** | 80% |
| **Site D** (manual relaunch) | P1 | P1 | P1 | P1 | P1 | **P1 (5/5)** | 100% |

**Convergence:**
- **Site C is P0 (compound nesting / deadlock-class)** — strictly worse than simple lock-across-await due to nested outer-read + inner-write across await. 4/5 P0 verdict.
- **Site A is P0 (billing-adjacent + sqlx UPDATE across write-lock)** — high-frequency, large-blast-radius, write-prefer-starvation risk. 4/5 P0 verdict.
- **Site B/D are P1** (lower frequency · pod-local impact · starvation class not deadlock).

**Notable model disagreements (sharpening signal):**
- deepseek rates Site A as P1 (not P0) emphasizing it's "write-lock-starvation not deadlock"
- google rates Site C as P1 (not P0) emphasizing it's "starvation not active deadlock without external contention"

Both edge cases are defensible. Recommendation: treat A + C as PEER-P0 with C having slightly higher absolute-deadlock-risk + A having higher frequency-blast-radius.

---

## §4 — Q3 Missed root causes · **UNION of panel hypotheses** (for MMA Step 2 PLAN exploration)

Cross-panel ranking by mention frequency:

1. **Write-prefer starvation cascade** (4/5 mention) — tokio RwLock favors writers; Sites B/C/D readers can starve Site A's write
2. **HashMap reallocation / collision latency** (4/5 mention) — large maps under concurrent write extend lock duration unpredictably
3. **Cooperative-yield gaps** (3/5 mention) — async tasks may not voluntarily yield under tight polling, extending guard hold
4. **SQLite WAL-mode blocking** (2/5 mention) — synchronous=NORMAL or WAL checkpoints can extend Site A's `.execute().await`
5. **Lock-contention storm pattern** (3/5 mention) — all 4 sites firing simultaneously during fleet-wide launch · cascading delays

**Recommendation:** Step 2 PLAN should explicitly verify hypotheses 1 + 2 via synthetic load harness before committing to fix-shape.

---

## §5 — Q4 Fix-approach · **5/5 CONVERGE on HYBRID**

**PREFERRED-APPROACH (5/5 panel):** **hybrid** — `AgentSenderRegistry::send_to()` class-level helper for Sites B/C/D + per-site snapshot-then-await for Site A (because A's lock is `active_timers` not `agent_senders`).

**Convergent helper signature (model recommendations align):**

```rust
// In crates/racecontrol/src/agent_registry.rs (or similar)
pub struct AgentSenderRegistry {
    senders: tokio::sync::RwLock<HashMap<PodId, mpsc::Sender<CoreMessage>>>,
}

impl AgentSenderRegistry {
    /// Snapshot-then-await: never holds read guard across send.await
    pub async fn send_to(
        &self,
        pod_id: &PodId,
        msg: CoreMessage,
    ) -> Result<(), SendError> {
        let tx_opt = {
            let senders = self.senders.read().await;
            senders.get(pod_id).cloned()  // mpsc::Sender is Clone
        }; // guard dropped here
        match tx_opt {
            Some(tx) => tx.send(msg).await.map_err(|_| SendError::ChannelClosed),
            None => Err(SendError::PodNotConnected),
        }
    }
}
```

**Per-site dispositions:**
- **Site B** (game_launcher_state.rs:422-433): migrate to `registry.send_to()` · ~6 LOC delta · removes class entirely
- **Site C** (game_launcher_ops_stop.rs:73-82): migrate to `registry.send_to()` for outer · handle inner `pending_command_acks.write()` cleanup in separate scope after send completes · removes compound nesting risk
- **Site D** (game_launcher_ops_relaunch.rs:69-79): migrate to `registry.send_to()` · ~8 LOC delta · removes class entirely
- **Site A** (game_launcher_state.rs:253-270): NOT covered by registry (different lock family); per-site fix = scope billing field snapshot in inner block → drop guard → execute SQLx query · OR migrate to dedicated billing actor task (longer-horizon · Stage 3 alternative)

**Trade-off summary (panel consensus):**
- Class-level helper for B/C/D: lower maintenance burden · enforces discipline · single chokepoint for clippy + tests
- Per-site for A: billing semantics require different mutation pattern (cannot trivially be helper'd)
- Foundation helper performance overhead is negligible vs deadlock-risk eliminated

**Stale-at:** Stage 2 PLAN should converge on this hybrid OR explicitly justify alternative (actor-per-pod migration is the only credible alternative · panel views it as longer-horizon).

---

## §6 — Q5 Recurrence-blocker · **5/5 AGREE-WITH-CAVEATS on clippy lint**

**LINT-ENABLE-VERDICT (5/5):** **yes-with-caveats** — enable `clippy::await_holding_lock = "deny"` in workspace `Cargo.toml` `[lints]` table

**False-positive risks (panel-aggregated):**
- Wrappers with non-trivial `Drop` impls may trip lint when guard scope is actually correctly bounded
- Macro-generated code may produce false positives in cargo-expand contexts
- Intentional cases where lock-across-await is architecturally correct (e.g., atomic distributed operations) — rare in this codebase

**Companion measures (panel union):**
1. **CI gate**: add `cargo clippy --workspace --all-targets -- -D clippy::await_holding_lock` to CI workflow as mandatory check
2. **Custom lint or macro** (optional): enforce "clone sender pattern" — any `mpsc::Sender` retrieved from `RwLock<HashMap>` must be `.cloned()` before guard release
3. **Targeted training / runbook**: "Rust Async Pitfalls" doc covers post-hoc drop misunderstanding (Site B anti-pattern)
4. **CLAUDE.md amendment**: codify the counter-anchor pattern (game_launcher_support.rs:227-230) as the canonical example in the Code Quality section

**Consensus position:** Lint enable is NECESSARY (closes recurrence per §S-146 enforcement RCA — text-only rules ≥1 recurrence/30d) but SUFFICIENT only if paired with CI gate. The lint without CI gate = same enforcement gap that allowed the current recurrence.

---

## §7 — Q6 Secondary concerns (out-of-scope but flagged)

Convergent secondary observations across panel:

1. **Site A SQL injection / type-safety:** raw string `UPDATE` query with manual binds invites SQLi-class risk on type drift. Recommend `sqlx::query!` macro migration. (deepseek + moonshot)
2. **Site C error swallowing:** `let _ = tx.send(...)` and `if let Err(e)` patterns ignore send failures. Should propagate via `GameStateUpdate::Error` or structured log + metric. (qwen + nvidia + moonshot)
3. **HashMap inefficiency:** all 4 sites use full-map locking; under 8-pod contention, consider `dashmap` or sharded-lock alternative as longer-horizon optimization. (deepseek + qwen + moonshot)
4. **Missing timeouts:** no timeout on SQLx execute or mpsc send — risk of indefinite hang on network/DB failure. (qwen + moonshot)

---

## §8 — Recommended path to Stage 2 PLAN

Per MMA Protocol v4.0 Step 2:

1. **Captain auth for Stage 2 PLAN** (~$5 budget · 5-model panel converges on fix-shape):
   - Class-level helper API surface refinement (signature · error type · backpressure handling)
   - Site A specific fix-pattern (snapshot-mutate vs actor-task)
   - Site C compound-nesting unwinding sequence
   - Synthetic load harness design for V-RCA1-3 verification

2. **D-RCA1-1 status:** **COMPLETE** — Step 1 DIAGNOSE consensus achieved. Captain may proceed to authorize:
   - D-RCA1-2 per-PR auth Site A fix (after Stage 2 PLAN converges)
   - D-RCA1-3 per-PR auth Sites B+C+D fix (or class-level helper PR)
   - D-RCA1-4 workspace `clippy::await_holding_lock = "deny"` + CI gate (foundation-class · zero-blast · highest-leverage)

3. **MAOR v0.1→v0.2 promotion criteria:** this consensus + the §S-388 datapoint count toward N=2 of N≥5. Need 3 more cascades with ≥1 defect each + zero rubber-stamp inversions to qualify v0.2 promotion. Forward window: 2026-08-13.

---

## §9 — Composes-with

- §S-388 close-anchor (MAOR Tier-2 datapoint #2 source)
- §S-146 V1↔V2 RCA gate (parent · this consensus is Step 1 of §S-146 process)
- §S-220 MAOR v0.1 ratify (datapoint #2 toward N=5 promotion)
- §S-345 soak-in-parallel-with-live (Server .23 deploy continues during fix-PR)
- §14.6.2 cascade-class-stratified RESET (Site A fix is A-foundational-schema-billing-adjacent → YES RESET on deploy)
- `feedback_capability_claim_without_probe_20260514.md` (independent source verification axis-3 satisfied via bono AMPLIFIER read)
- §S-146 enforcement RCA · `project_s146_enforcement_rca_20260510.md` (text-only-rule recurrence rate evidence: this RCA is THE 30d recurrence anchor for lock-across-await class)
- `~/.claude/projects/-root/memory/feedback_apply_recommendations_autonomously_20260510.md` Pre-Commit Exception (Captain composite verb covers Stage 1 execution + Stage 2 PLAN auth queue)

---

## §10 — Universal Sync targets

- `racecontrol/.planning/specs/v2/RCA/game-launch/MMA-STEP-1-CONSENSUS.md` ✓ (this file)
- `comms-link/data/openrouter-spend-bono.jsonl` ✓ (12 entries appended for this MMA · surface `MMA-STEP-1-RCA1-*`)
- `comms-link/V2-MASTER-STATE.md` — close-anchor §S-N+ DEFERRED to next bono session (carry-forward note this consensus + RCA #2 sibling)
- bono `MEMORY.md` SUPPLEMENT-16 — pickup-class note for next bono session
- `~/.claude/CLAUDE.md` harness — NOT-APPLICABLE
- james-side bilateral pickup — partner-memory-read hook on next james engagement

---

**Independence axes verification (MAOR §14.1):**
- Axis 1 (subagent type ≠ author): consensus models are external OpenRouter models · bono session is the synthesizer · independent from james-authoring · ✓
- Axis 2 (no shared context): each model received only the RCA citations + structured questions · no prior context · ✓
- Axis 3 (independent source-of-truth reads): bono read source files (game_launcher_state.rs · game_launcher_support.rs · ws_handler.rs) independently before authoring this consensus · ✓

**Anti-rubber-stamp briefing applied** per §S-220.5: confidence ≥75% (all model verdicts above floor) · DO-NOT-report items suppressed · intentional patterns noted (counter-anchor in support.rs) · explicit per-question verdicts · source-of-truth pointers cited.

---

End of MMA-STEP-1-CONSENSUS for RCA #1 lock-across-await pattern class.
