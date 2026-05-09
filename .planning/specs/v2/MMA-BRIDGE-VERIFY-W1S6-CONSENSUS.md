# MMA Step 4 VERIFY (mma-bridge-verify) — W1-S6-PLAN.md (PR-A detail plan)

**Anchor:** racecontrol HEAD `638ef2da` on `feat/v2-wave-1-w1-s1-billing-service` · target `W1-S6-PLAN.md` (cascade #7 detail PLAN, shipped at `7a575b9a` → re-anchored at `ae946125`) · Phase 1 PR-A code substrate shipped at `638ef2da`.

**Run timestamp:** 2026-05-09 ~22:53 IST (UTC `2026-05-09T17:23:14Z` per ledger entry).

**Authority:** user verb shortcut `mma-bridge-verify` 2026-05-09 ~22:51 IST + Captain "Proceed autonomously" 2026-05-09 ~21:08 IST + AskUserQuestion target = "W1-S6-PLAN.md (PR-A) detail plan" 22:51 IST.

**Verdict:** **FAIL** (mean weighted 3.50/5 < 4.0 gate; ≥1 BLOCKING divergence convergent across 2/3 valid responses) — HALT cascade per Captain G33 v5 #11 contingency. PR-A Phase 2 (dispatch.rs + audit.rs) authoring HALTED pending Captain disposition.

---

## §1 — Panel composition

| Slot | Model | Role | Vendor | Status | Output | Cost | Elapsed |
|------|-------|------|--------|--------|--------|------|---------|
| 1 | `anthropic/claude-sonnet-4.6` | code_expert | anthropic | OK | 16344 chars (verdict + 5 dims + 5 divergences + 4 novel) | $0.1917 | 86.8s |
| 2 | `moonshotai/kimi-k2.5` | reasoner | moonshot | OK | 5217 chars (verdict + 5 dims + 3 divergences + 1 novel) | $0.0293 | 153.9s |
| 3 | `nvidia/nemotron-3-super-120b-a12b` | sre | nvidia | INVALID-EARLY-TERMINATION | 34 chars / 14 output tokens (refused/truncated; no `<OUTPUT>` block) | $0.0037 | 2.9s |

**Total spend:** $0.2247 · **Wall time:** 153.9s parallel · **Valid responses:** 2/3.

**Vendor diversity:** anthropic + moonshot + nvidia (3 distinct vendor families; ≥3 ✓; vendor-disjoint from Steps 1+2 panels deepseek/qwen/mimo/gemini/mistral; vendor-disjoint from Wave A.2 prior Step 4 panel deepseek/grok/nemotron[same family]/kimi within ≥60min window — Wave A.2 ran 162min ago = outside §S-159 hook window; deploy-RCA PIVOT Step 4 ran 75min ago = outside §S-159 hook window).

**Role-fit per `feedback_model_role_fit_check_before_vendor_fit_20260509.md` PROMOTE-NOW-ACTIVE doctrine:** all 3 picks Tier-1 / NO speed-class names in reasoner+code_expert slots ✓.

**§S-159 pre-MMA-duplicate-check hook:** PASSED (different RCA-key from prior MMA runs in 60min window; different target = W1-S6-PLAN.md vs prior Wave A.2 meta-plan + deploy-RCA cascade).

**Sample-size note:** With 1 invalid response (nemotron API early-termination at 14 output tokens — likely model-specific JSON schema rejection or content-policy refusal), the panel reduces to 2 valid responses for adversarial scoring. Per MMA Protocol v4.0 VERIFY step minimum is 3 models; this run is technically below adversarial minimum. However, the 2 valid responses converge on multiple BLOCKING/MAJOR findings, so substantive verdict is robust despite reduced N. Future Step 4 adversarial runs should swap nemotron for an alternative SRE model (candidates: `xiaomi/mimo-v2.5-pro`, `nvidia/nemotron-49b`).

---

## §2 — Per-model raw scores

| Model | Verdict | Weighted | Coverage | Correctness | Risk | Completeness | Divergences |
|-------|---------|----------|----------|-------------|------|--------------|-------------|
| sonnet-4.6 | PASS-WITH-AMENDMENTS* | **4.15** | 4.2 | 3.8 | 4.1 | 3.9 | 3.5 |
| kimi-k2.5 | FAIL | **2.85** | 4.5 | 2.0 | 3.0 | 2.5 | 0.0 |
| nemotron | INVALID | n/a | n/a | n/a | n/a | n/a | n/a |

*Sonnet self-labeled `PASS-WITH-AMENDMENTS` but listed 1 BLOCKING divergence — internally inconsistent per its own verdict mapping rules ("≥4.0 AND no BLOCKING → PASS-WITH-AMENDMENTS, otherwise FAIL"). Treated as substantive `FAIL` for consensus purposes.

**Mean weighted (valid responses):** (4.15 + 2.85) / 2 = **3.50/5** · **PASS gate:** ≥4.0 · **Verdict:** **FAIL** (under threshold + BLOCKING divergence flagged by both).

**Per-dimension means (valid responses):**
- coverage: (4.2 + 4.5) / 2 = 4.35 ← strong coverage
- correctness: (3.8 + 2.0) / 2 = 2.9 ← weak (BLOCKING-class flaws drag this down)
- risk: (4.1 + 3.0) / 2 = 3.55
- completeness: (3.9 + 2.5) / 2 = 3.2
- divergences: (3.5 + 0.0) / 2 = 1.75 ← worst dimension (kimi awarded 0 due to BLOCKING)

---

## §3 — Convergent findings (≥2/2 valid models)

### FL-CONV-1 — PLAN §4.8 audit-schema contradiction with Phase 1 shipped substrate

**Severity:** BLOCKING (sonnet) / MAJOR (kimi) → **consensus BLOCKING-class.**

**Issue:** PLAN §4.8 specifies "DB migration `20260510000001_pin_lockout_state_v2.sql` extends `audit_log.action_type` CHECK constraint to include these 5 new values." But the migration shipped in Phase 1 at `638ef2da` does NOT extend `audit_log.action_type` — it introduces a SIBLING V2-bounded table `pin_lockout_events` with its own bounded CHECK constraint (per §S-158 V2 Audit-Log Doctrine, avoiding the PACT-091 V1 antipattern of action-vs-action_type drift).

The two designs are mutually exclusive:
- **Design A (PLAN §4.8):** extend V1 `audit_log.action_type` CHECK constraint — adds V2 action types to the V1 table — V1 antipattern prone to schema drift.
- **Design B (Phase 1 shipped):** introduce V2-bounded sibling table `pin_lockout_events` with bounded CHECK from start — V2-doctrine-compliant per §S-158.

**Impact:** Phase 2 audit.rs authoring is BLOCKED on resolving which design the PLAN endorses. The shipped Phase 1 substrate is correct per V2 Audit-Log Doctrine; the PLAN documentation is wrong.

**Disposition path:** PLAN amendment to §4.8 wording — change "extends `audit_log.action_type` CHECK constraint to include these 5 new values" → "introduces V2-bounded sibling table `pin_lockout_events` with its own bounded `action_type` CHECK from start (per §S-158 V2 Audit-Log Doctrine; avoids PACT-091 V1 antipattern of action-vs-action_type drift)." This is a DOCUMENTATION FIX — code is correct, doc must catch up.

### FL-CONV-2 — §4.3 retry-queue backoff re-interpretation contradicts Captain G33 v5 #6 ratified spec

**Severity:** MAJOR (sonnet) / BLOCKING (kimi) → **consensus BLOCKING-class.**

**Issue:** Captain G33 v5 #6 ratified Q-W1-S6-NEW-2 verbatim: "10s · 60s · 300s · 3 attempts." PLAN §4.3 re-interprets this as "**3 total attempts** with backoffs `[0s, 10s, 60s]` applied between attempts; the `300s` slot in Captain spec is **RESERVED** for V2.1+ extension to 4 attempts (out of scope V2.0)."

This is a unilateral interpretation of a ratified Captain spec. Three readings of the original Captain spec are possible:
- **(A) Most natural:** 3 retries-after-failure with backoffs 10s/60s/300s between them = 4 total dispatch attempts (initial + 3 retries with growing backoff).
- **(B) PLAN's:** 3 total attempts with backoffs [0s, 10s, 60s] (300s reserved for V2.1+).
- **(C) Alternative:** 3 attempts with backoffs [10s, 60s, 300s] where the 300s applies to the final attempt's wait window.

The PLAN's interpretation (B) discards the 300s slot, reducing extended-SMTP-outage resilience. Without explicit Captain ratification of (B) over (A) or (C), this is a unilateral modification of a ratified spec.

**Disposition path:** Surface to Captain as Q-DECISION-G33v7 — clarify retry-queue backoff interpretation. Cannot be resolved autonomously per per-PR Captain auth doctrine + ratified-spec immutability.

### FL-CONV-3 — §6.2 V1-untouched claim vs §3.1 middleware.rs primary touch listing

**Severity:** MAJOR (kimi) / not-flagged (sonnet) → **single-model MAJOR.**

**Issue:** §6.2 rollback path step 4 states "V1 code path is left UNTOUCHED in PR-A — `auth/middleware.rs` V1 lockout retained until PR-C cutover." But §3.1 PR-A primary touch list includes `crates/racecontrol-crate/src/auth/middleware.rs` as a target.

These are not strictly contradictory (the file is touched but the V1 lockout-state semantic path inside is preserved), but the §6.2 wording is imprecise.

**Disposition path:** PLAN amendment to §6.2 step 4 wording — change "V1 code path is left UNTOUCHED in PR-A" → "V1 lockout-state semantic code path within `auth/middleware.rs` is left UNTOUCHED in PR-A; new V2 publisher hooks added alongside per §3.1, but V1 path remains the active hot path until PR-C cutover." This is a DOCUMENTATION FIX.

---

## §4 — Sonnet-only substantive findings (single-model, high signal)

These were flagged by sonnet (most substantive response, 16344 chars). Single-model = CANDIDATE-N1 status; promote on N=2 if independently confirmed in subsequent run.

### FL-SING-1 — §4.1 LockoutCheckGuard RAII publisher-side TOCTOU not closed

**Severity:** MAJOR (sonnet only).

**Issue:** Wave A.2.1 §3 surgical closure for F-CONS-15 atomic TOCTOU lockout-check+revoke specifies LockoutCheckGuard RAII type ensuring atomic predicate-check + state-write on the publisher side. PLAN §4.1 specifies bare `is_locked_out(staff_id) -> Result<LockoutPredicate, LockoutError>` read with no RAII guard. The W1-S5 consumer (PR-C) cannot close the publisher-side TOCTOU window unilaterally; it must be closed at the publisher boundary.

**Disposition path:** PLAN amendment to §4.1 — add `LockoutCheckGuard` RAII type spec per Wave A.2.1 §3 closure. Could be authored autonomously since Wave A.2.1 already specifies the design.

### FL-SING-2 — §5.1 `lockout_state_persisted_to_db` test contradicts in-memory ratification

**Severity:** MINOR (sonnet only).

**Issue:** Test name `lockout_state_persisted_to_db` implies DB persistence; but Q-S6-6 ratified in-memory HashMap with restart-loses-state acceptable. Test cannot pass against in-memory design.

**Disposition path:** Rename test to `lockout_state_persists_in_memory_across_request_boundary` or remove; clarify in-memory semantics.

### FL-SING-3 — §2 DKIM/SPF "document absence" softens Q-S6-4 Captain-gate

**Severity:** MINOR (sonnet only).

**Issue:** §2 item 2 says "document absence" for missing DKIM/SPF. Q-S6-4 ratification requires Captain Q-DECISION gate before proceeding ("ship-with-risk NOT pre-authorized"). PLAN softens a ratified gate.

**Disposition path:** PLAN amendment to §2 item 2 — change "OR document absence" → "OR ESCALATE to Captain Q-DECISION (ship-with-risk NOT pre-authorized per Q-S6-4)."

### FL-SING-4 — Novel: cashier_role_lockout test has no RCA anchor

**Severity:** P2 novel (sonnet only).

**Issue:** §5.1 specifies `cashier_role_lockout_separate_from_manager_lockout` test asserting per-role lockout boundary isolation. Lockout in entire RCA + Q-DECISION corpus is keyed per-staff-id, not per-role. Test either tests a non-existent invariant or introduces undocumented design requirement.

**Disposition path:** Remove test OR document per-role boundary as a NEW design requirement requiring its own RCA anchor.

### FL-SING-5 — Novel: WhatsApp fire-and-forget audit-log fidelity loss

**Severity:** P1 novel (sonnet only).

**Issue:** §4.5 prevents WhatsApp from entering retry queue (correct), but neither §4.5 nor §4.8 specifies an audit-log entry for WhatsApp dispatch failure. RCA §5 item 5 specifies `whatsapp_captain_dispatched: ok | timeout | error | not_applicable` in audit JSON payload, but PLAN §4.8 action types list only `lockout_alert_dispatched_whatsapp` (success) and `lockout_alert_dispatch_failed` (generic) without distinguishing WhatsApp timeout vs error.

**Disposition path:** PLAN amendment to §4.8 — add `lockout_alert_dispatched_whatsapp_timeout` and `lockout_alert_dispatched_whatsapp_error` to action types list, or use payload sub-field discrimination.

### FL-SING-6 — Novel: subscribe() forward-compat hook no-op default unspecified

**Severity:** P2 novel (sonnet only).

**Issue:** §4.2 specifies `LockoutManager::subscribe(consumer_fn)` with "first consumer is no-op until PR-C lands." If Vec<callbacks> empty Vec is fine; if Option<fn> the None case must be explicitly handled in `is_locked_out` to avoid panic path.

**Disposition path:** PLAN amendment to §4.2 — specify subscriber storage mechanism (Vec recommended) + no-op default behavior contract.

### FL-SING-7 — Novel: dispatch_timeout_secs default 10 vs F-CONS-18 RCA default 5

**Severity:** P2 novel (sonnet only).

**Issue:** §3.1 config spec sets `auth.dispatch_timeout_secs` default = 10. F-CONS-18 mitigation in RCA + Step 1 amended CONSENSUS specifies default = 5s (mimo flagged 10s as "more forgiving baseline pending Session 5 probe"). PLAN adopts 10s without documenting deviation or citing mimo recommendation as justification.

**Disposition path:** PLAN amendment to §3.1 — either revert to 5s default per RCA, or add inline note citing mimo "more forgiving baseline" recommendation.

### Kimi-only novel finding

Kimi flagged the same Phase 1 substrate inconsistency as a novel finding (already covered as FL-CONV-1 above; not double-counting).

---

## §5 — Captain Q-DECISION compliance (per-model)

| Q-DECISION | Sonnet | Kimi | Consensus |
|-----------|--------|------|-----------|
| Q-W1-CROSS-1 | PASS | PASS | PASS |
| Q-W1-CROSS-2 | PASS | PASS | PASS |
| Q-W1-S6-NEW-2 | FAIL | FAIL | **FAIL** (FL-CONV-2 backoff re-interpretation) |
| NEW-Q-1 | PASS | PASS | PASS |
| NEW-Q-2 | FAIL | FAIL | **FAIL** (FL-CONV-1 audit-schema doctrine mismatch) |

**Wave A.2.1 closures compliance:**

| Closure | Sonnet | Consensus |
|---------|--------|-----------|
| F-CONS-15 TOCTOU publisher | PARTIAL (FL-SING-1) | PARTIAL |
| F-CONS-2 F-05 W1-S6 extension | PASS | PASS |

**Phase 1 substrate consistency:** INCONSISTENT (both valid models — driven by FL-CONV-1).

---

## §6 — Verdict and disposition

**Verdict:** FAIL (3.50/5 < 4.0 gate; ≥1 BLOCKING-class divergence convergent across 2/2 valid models; sample-size note: 2/3 valid responses, below MMA Protocol VERIFY-step minimum-3 but substantive verdict robust due to convergence).

**Cascade impact:** PR-A Phase 2 (dispatch.rs + audit.rs) authoring HALTED per Captain G33 v5 #11 score-block contingency. Phase 1 (DB migration + lockout.rs scaffold + auth/mod.rs registration) at `638ef2da` remains SHIPPED and consistent with Phase 1 portion of the PLAN (specifically §3, §4.1, §4.2 — code is correct; doc inconsistencies are at §4.8 and §6.2).

**Sibling pipeline reference:** Wave A.2 PLAN Step 4 VERIFY 2026-05-09 14:39Z scored 3.988/5 = FAIL (margin 0.012); cascade halted; resolved via Captain Option C hybrid disposition (Wave A.2.1 surgical amendment + Q-DECISION-G33v6 split). This W1-S6-PLAN VERIFY follows the same gate-discipline pattern.

---

## §7 — Disposition options for Captain G33 v7

**Option A — Surgical amendment (recommended for autonomy alignment):** Author PLAN amendment closing FL-CONV-1 + FL-CONV-3 + FL-SING-1 + FL-SING-3 + FL-SING-7 (documentation fixes + Wave A.2.1 closure consumption). Surface FL-CONV-2 to Captain as Q-DECISION-G33v7 retry-queue backoff clarification. Re-VERIFY only after Captain ratifies retry-queue interpretation. Estimated cost: $0 amendment + ~$0.05-0.10 re-VERIFY.

**Option B — HALT + Captain-only Q-DECISION batch:** Surface ALL findings (FL-CONV-1+2+3 + 4 sonnet-singletons) as Captain Q-DECISION-G33v7 batch; let Captain ratify each disposition individually before any amendment authoring. Estimated cost: $0 (no spend until Captain disposes).

**Option C — Phase 2 author with awareness gates:** Proceed with Phase 2 (dispatch.rs + audit.rs) authoring with explicit code-comments acknowledging each finding + TODO markers for Captain disposition. Re-VERIFY post-Phase-2 with amendments folded in. RISKY — compounds the gap; not recommended.

**Option D — Re-VERIFY with substituted SRE model:** Re-run VERIFY with `xiaomi/mimo-v2.5-pro` substituted for `nvidia/nemotron-3-super-120b-a12b` to recover the 3rd valid response and confirm/refute the 2/2 consensus. Estimated cost: $0.05-0.08 (re-run only the 1 missing slot).

**Option E — Defer to next session:** Park this VERIFY as in-progress; pick up next session with full Captain disposition batch + re-VERIFY. Lowest immediate spend; defers progress.

**Default-recommendation:** **Option A** (surgical amendment for autonomous-fix-eligible items + Q-DECISION surface for ratified-spec interpretation gap; aligns with autonomous V2-progression auth + Captain Option C hybrid precedent set 20:28 IST on Wave A.2 PLAN failure).

---

## §8 — NOT TESTED

- nemotron-3-super-120b-a12b refusal/early-termination root cause (could be content-policy, JSON schema, or prompt-class refusal — not investigated this turn)
- Re-VERIFY post-amendment (gates Captain disposition)
- Dimension recomputation if 3rd valid response recovered via Option D substitution
- Bono AMPLIFIER absorption (deferred to next bilateral cycle per Wave A.2 RCA precedent)
- Spend-ledger accumulator vs $5 daily cap end-of-day reconciliation (cumulative MMA-day spend now ~$1.005)

---

— james / 2026-05-09 ~22:53 IST · MMA Step 4 VERIFY (mma-bridge-verify) on W1-S6-PLAN.md FAILED 3.50/5 · 2/3 valid models / $0.2247 · 2 BLOCKING/MAJOR-class convergent + 7 sonnet-singletons + 1 kimi-singleton (covered) · Captain G33 v5 #11 contingency triggered HALT cascade · 5 disposition options A-E surfaced · default-recommendation Option A surgical amendment + Q-DECISION-G33v7 retry-queue clarification · 0 G9 self-caught · cumulative MMA-day spend ~$1.005/$5 cap.
