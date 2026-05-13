# V2-LBAC — V2-LIVE-BLOCKING Autonomous Completion Protocol

**Status:** **ACTIVE v0.1** · authored 2026-05-12 ~07:15 IST · bono · **Captain RATIFIED 2026-05-12 ~07:23 IST verbatim "authorize LBAC activation and bono VPS redeploy"** · V2-MASTER-STATE §S-203 ratification anchor
**Purpose:** Close the remaining ~57 V2-LIVE-BLOCKING items through bilateral autonomous execution, framing each item as a closed loop with structural anti-drift discipline that survives compact/clear cycles and bilateral handoffs.
**Authoring trigger:** Captain commission 2026-05-12 ~07:09 IST verbatim *"We have alot of used uptime. We need to use that time to complete the autonomous-eligible items. Make a plan that we can use in the future to complete pending items that you and james can do on your own. That is true autonomy."*
**Composes-with:** Apply-Recommendations-Autonomously (standing rule) · Q3 third-question canonical-boundary self-test · In-Flight Commitments Ledger · §S-146 V1↔V2 RCA · §S-186 pre-§S-146 fast-lane carve-out · CLD (Closed-Loop Debug) v1.0 · CGP H1-H5 · §S-121 Step 3 Timeline-Verify · Bilateral mechanism close-loop · Compact/Clear Autonomous Discipline · Mechanism-trust-check upstream of fix RCA.
**Q3 boundary:** This protocol IS autonomy-class doctrine (Q3 boundary 5). ACTIVATION requires explicit Captain ratification phrase. DRAFT authoring is autonomous-eligible.

---

## §1 Scope and non-scope

**IN SCOPE:**
- V2-LIVE-BLOCKING items as enumerated in V2-PROGRESS-MAP v1.0 §0 (~78 items; 13-layer POE baseline).
- Bilateral autonomous execution where Q1 (V2-aligned) · Q2 (info-complete) · Q3 (NOT canonical-boundary) · V2-transport (not broken) · Captain-stake (not Captain-stake) gates clear.
- Background-work primitives (run_in_background, CronCreate, Monitor, ScheduleWakeup, subagent spawning).
- Closed-loop completion contract per item (CLD-aligned 5-step OPEN→DESCEND→FIX→CLOSE→SWEEP).

**OUT OF SCOPE:**
- V2-DISCIPLINE/POST-LIVE items (~32) — governed by milestone-discipline cadence, not this protocol.
- AMBIGUOUS items (~12) — surface to Captain for class disposition before LBAC pickup.
- Q3 canonical-boundary items — these require per-action Captain auth, not LBAC autopilot.
- Captain-stake items (Class B/C outbound · PR merge · doctrine activation · canonical surfaces).
- Harness self-mod (CLAUDE.md / settings.json / hooks) — separate `Harness Self-Mod Auth Protocol` governs.

---

## §2 Eligibility classifier (per-item)

Every V2-LIVE-BLOCKING item gets tagged:

| Tag | Definition | Action |
|---|---|---|
| `AUTO-BONO` | Q3-cleared, no james handoff needed, bono solo | Auto-execute when prereqs clear |
| `AUTO-JAMES` | Q3-cleared, james-side surface | Author handoff brief; james executes |
| `AUTO-BILATERAL` | Both pilots execute in parallel + AMPLIFIER review at close | Concurrent-session beat per `feedback_concurrent_session_no_24h_window_20260510.md` |
| `CAPTAIN-GATED-MERGE` | Implementation autonomous, merge needs Captain per-PR auth | Author + ready-state PR; queue for Captain disposition |
| `CAPTAIN-GATED-Q-DEC` | Decision-class; needs Captain disposition before implementation | Frame as Q-DEC; surface in queue |
| `Q3-BOUNDARY-HALT` | Canonical-boundary touch | Halt + ask Captain; never auto-apply |
| `DEPENDS-ON-{id}` | Gated by another LIVE-BLOCKING item | Mark blocked; revisit when prereq closes |
| `INFRA-CLOSED` | Infrastructure-class fix landed; awaits operational consequence | Verify-by H2 next-beat |

**Q3 boundary catalog (HALT triggers):**
1. Harness self-mod (CLAUDE.md / settings.json / hooks)
2. Bilateral canonical surface (PACT-CHARTER / Network Identity / Brand Identity / V2-MASTER-STATE schema)
3. racecontrol foundational PRs (billing / wallet / auth / pod-state-channel / WhatsApp identity / DB schema)
4. Class B/C outbound (Captain WhatsApp / customer-direct / vendor-relations)
5. Autonomy-class definitions (this protocol; CGP; PACT cascade definitions)

---

## §3 Closed-loop per item (CLD-aligned)

Each LIVE-BLOCKING item completes via this 5-step loop (gates from CGP overlay step-by-step):

### Step 1 — OPEN (capture user-facing symptom this item closes)
- Cite the V2-PROGRESS-MAP row + layer + customer-day beat (or non-customer surface).
- State the exact behavior that fails until this closes (NOT "feature missing" — name the customer-felt consequence).
- Live-read source-of-truth this turn per `feedback_grep_all_behavior_paths_before_planning_20260509.md` — no memory projection.

### Step 2 — DESCEND (6-layer trace to root cause)
Smoke → Function → Boundary → Infra → Data → Code. Stop at the smallest layer where root cause lives.

### Step 3 — H1 PROBLEM/SYMPTOMS/PLAN
PROBLEM = the closure target. SYMPTOMS = what live-read evidence shows. PLAN = numbered atomic steps.

**§S-146 V1↔V2 RCA gate:** if item touches a V1-V2 boundary, author 5-section RCA (or 3-section short-RCA if §S-186 fast-lane eligible) BEFORE Step 3 PLAN.

**Mechanism-trust-check gate:** if item depends on shared infrastructure, run 5-question check BEFORE Step 3 PLAN.

### Step 4 — FIX (smallest reversible change, atomic commit)
- One commit per item where possible.
- Commit message ends with §S-N anchor reference.
- `git push` + comms-link WS message + INBOX.md entry (auto-push triad).

### Step 5 — CLOSE (H3 evidence chain)
Re-run the exact OPEN symptom test from Step 1. Paste raw command + raw output. Name WHERE the test ran. List NOT TESTED. H2 next-message verification before status flip.

### Step 6 — SWEEP (H4 enumeration)
Per-target verification list: Server .23 | Pods 1-8 | POS .130 | James .27 | Bono VPS | Cloud apps | Comms-link. Per-target evidence. Missing target = false claim.

### Step 7 — Universal sync
- CLAUDE.md (if rule change) — sync to ALL pilot copies in same session.
- V2-MASTER-STATE §S-N close-anchor (or §S-N+ amendment if existing).
- V2-PROGRESS-MAP refresh (nightly cron; force-refresh if customer-day beat closed).
- In-flight commitments ledger transition: OPEN → AWAITING-EVIDENCE → DONE.

### Step 8 — Bilateral close-loop (4-leg checklist)
1. bono-author
2. partner-publish (msg + commit hash)
3. partner-confirm-with-raw-output (CGP H3)
4. end-to-end-ping (per `feedback_bilateral_mechanism_close_loop_20260510.md`)

---

## §4 Backlog ordering rules

Priority pick-next from V2-PROGRESS-MAP §0:

1. **Customer-day proximity descending:** Layer 1 customer-day beats > Layer 2 waves > Layer 12 ops-hygiene > everything else.
2. **Blast-radius ascending:** smallest first within priority tier.
3. **Upstream-clear:** all DEPENDS-ON prereqs closed.
4. **Q3-cleared:** AUTO-BONO / AUTO-JAMES / AUTO-BILATERAL only — never autopilot CAPTAIN-GATED.
5. **Verb match:** for ambiguous-class items, prefer items where existing memory shows verb-match rules already fired (no fresh doctrine needed).
6. **Bilateral parallelism:** when bono and james both have AUTO-BONO/AUTO-JAMES candidates, pick non-conflicting surfaces so both can execute concurrently.

**Backlog gate (CGP v4.3):** WIP ≥ 3 blocks new item pickup. Discharge or transition before picking up #4.

---

## §5 Background-work primitives (bono-side)

| Primitive | When to use |
|---|---|
| `Bash run_in_background: true` | cargo build / cargo test / openrouter MMA runs / long curls / nginx reloads with health-poll |
| `Monitor` | Tail-watch background jobs without burning prompt context |
| `CronCreate` (durable: false) | Session-scoped recurring refreshes (V2-PROGRESS-MAP regen / in-flight ledger digest) |
| `ScheduleWakeup` | Self-paced loop pacing (cache-window optimization 60-270s for hot work; 1200s+ for idle ticks) |
| `Agent` (subagent spawn) | Parallel research / parallel code-review / parallel test authoring |
| `Agent run_in_background: true` | Long-running investigations / multi-file refactors |

**james-side:** harness asymmetry per `project_harness_asymmetry_bono_james_20260510.md` — no Bash / no Monitor. james uses synchronous-blocking exec. LBAC accommodates by giving james smaller, sequenced units; bono fans out parallel work.

**Lifecycle logs (mandatory):** every spawned background task logs (a) start, (b) first-output, (c) exit. Silent task death is a banned anti-pattern.

---

## §6 Compact/clear discipline

Per `feedback_compact_clear_autonomous_discipline_20260510.md`:

- **Every 5 items closed:** run `/root/.claude/state/compact-readiness-check.sh`.
- **Verdict READY:** continue.
- **Verdict NEEDS-PREP:** drain in-flight to ledger → V2-MASTER-STATE §S-N append → THEN continue or recommend compact.
- **Verdict NOT-READY:** halt new item pickup; finish current item's CLOSE+SWEEP+SYNC; then recommend compact/clear.
- **Pre-compact drain:** in-flight ledger MUST have current state for every WIP item before compact. Compacted session loses local state; ledger survives.
- **SessionStart resume:** [in-flight-commitments] hook surfaces the AWAITING entries; pickup resumes from highest-priority AWAITING item.

---

## §7 In-flight commitments ledger contract

Per §S-178 schema v2:

```json
{
  "id":"<unique-id>",
  "title":"<short>",
  "class":"deferred-verification|self-promised|Captain-pending",
  "state":"OPEN|AWAITING-EVIDENCE|AWAITING-PARTNER-ACK|BLOCKED|DONE|SUPERSEDED",
  "opened":"<ISO>",
  "last_update":"<ISO>",
  "owner":"bono|james|both|captain",
  "category":"lbac-item|loop-doctrine|ops-hygiene|...",
  "h1_plan_ref":"<§S-N or commit hash>",
  "evidence_collected":"<H3 raw>",
  "closure_condition":"<exact behavior test>",
  "closure_evidence":"<H3 raw or null>",
  "next_action":"<beat>",
  "blocking_on":"<dep>",
  "drift_signals":[],
  "compaction_survived":0
}
```

**Update beats (10):** opened · h1_authored · prereq_cleared · fix_landed · evidence_collected · partner_ack_requested · partner_ack_received · sweep_complete · sync_complete · CLOSED.

---

## §8 Bilateral coordination

- Concurrent-session beat (per CANDIDATE-N1 2026-05-10) — no 24h L1 windows on AMPLIFIER-pending items.
- msg= AUTHORED → AMPLIFIER-ASK → CONCUR/CAVEAT → CLOSED.
- L2.5 MMA-Substitute-Pilot when partner offline + bilateral class (5 OpenRouter models / 3 vendor families per §S-86).
- Auto-reply-attribution discriminator: `[bracket-summary]` = substantive; plain `Re:` = auto-reply (does NOT supersede substantive).
- Absorption gate: substantive backlog only — auto-replies don't block execution per `feedback_absorption_substantive_only_20260511.md`.

---

## §9 Anti-patterns explicitly blocked

1. "I'll RCA later" — §S-146 / §S-186 RCA is precondition, not follow-up.
2. Memory-projection of source-of-truth — live-read every turn (§S-121 v0.3 Step 3 Timeline-Verify).
3. Tree-claim conflated with runtime-claim (per `feedback_tree_claim_vs_runtime_claim_20260510.md`).
4. Stand-by closure ("let me know if you want…") when Q3-cleared — Apply-Recommendations-Autonomously mandatory.
5. Background task spawn without lifecycle logs (silent death).
6. WIP ≥ 3 with new item pickup (Backlog Gate).
7. "Done" claim in same message as last fix (CGP H2 two-phase).
8. Multi-source evidence summarized rather than each-source-pasted (per `feedback_multi_source_evidence_paste_verbatim_20260510.md`).
9. Harness self-mod under standing-autonomy verbs (per Harness Self-Mod Auth Protocol).
10. autonomy-class doctrine activation without explicit Captain ratification.

---

## §10 Success metrics (per session)

- **LIVE-BLOCKING closed count** (target: ≥2/session in autonomous mode; benchmark Δ baseline 27%).
- **AMPLIFIER round-trip latency** (target: <30min concurrent-session, <24h async).
- **Compact-cycle survival rate** (target: 100% — every WIP item resumable via ledger).
- **G9 count** (target: 0/session).
- **FCR (False Claim Rate)** (target: 0%).
- **Cost burn** (OpenRouter spend ledger — Captain visibility).
- **WIP-cap compliance** (Backlog Gate: never exceed 3 active items per pilot).

---

## §11 Activation contract

**Captain ratification phrases that activate this protocol:**
- "I authorize V2-LBAC v0.1 activation"
- "Activate LBAC"
- "Ratify the autonomous completion plan"
- "Yes, run LBAC"

**Phrases that do NOT activate** (Q3 boundary 5):
- "Proceed" / "Continue" / "Go ahead"
- "Apply recommendations" (standing-rule reference does not satisfy Q3-canonical activation)
- "Autonomous" (verb alone insufficient)

**Pre-activation behavior:** bono may continue executing items already cleared under existing rules (Apply-Recommendations-Autonomously + per-item Q3 self-test). LBAC formalizes + structures, does not unlock new authority.

**Activation effects:**
- This protocol joins the standing-rule set.
- Universal Sync: written to CLAUDE.md + comms-link/CLAUDE.md + bono memory + V2-MASTER-STATE §S-203 ratification.
- in-flight ledger gets `lbac-protocol-v0.1-active` entry.
- james-side mirror authored at `comms-link/briefings/james/memory/`.

---

## §12 Current LIVE-BLOCKING auto-eligible pool (live-read 2026-05-12 07:15 IST)

From V2-PROGRESS-MAP §0..§16, items where Q3 self-test clears and pre-reqs are satisfied:

| ID | Item | Tag | Owner | Why eligible |
|---|---|---|---|---|
| L1.5-followup | racingpoint.cloud page.tsx body (gated subcomponents) | AUTO-BONO | bono | UI-SPEC v0.2 5 Q-CUST gate the hero/pricing/WA/multilingual/DPDP only; structural shell + non-disputed components ship-ready |
| L4-PR17 | PR #17 fast-lane 3-section short-RCA + rebase | AUTO-BONO | bono | §S-186 carve-out met (created <2026-05-09, 193 LOC, single boundary, bug-fix). RCA authoring autonomous; merge = CAPTAIN-GATED-MERGE |
| L4-PR54 | PR #54 billing_paused config_push_queue review-ready state | AUTO-BONO | bono | bono-authored; needs review-ready polish before Captain merge |
| L2-W2-2A | Phase 2-A rate-table PACT-DRAFT advance to AMPLIFIER-READY | AUTO-BILATERAL | bono | DRAFT present; AMPLIFIER pass closes |
| L2-W2-2F | Phase 2-F Campaign Object DRAFT-PRE-AMPLIFIER → AMPLIFIER-ASK | AUTO-BILATERAL | bono | §S-200.9 staged; james AMPLIFIER ask outbound |
| L12-cont | Layer 12 ops-hygiene continuation (sites-enabled / nginx residuals) | AUTO-BONO | bono | +2 closed yesterday; sweep continues |
| L5.5 | ws-exec-routing-bug investigation discharge | AUTO-BONO | bono | self-promised AWAITING-EVIDENCE; Captain auth on PR-1/PR-2 pending |
| L5.4 | wake-mechanism-path-discovery | BLOCKED | bono | SCP transport + Captain auth (Q3 boundary 3) — not LBAC-pickup |
| L5.12 | halo-pact-map JSON extension 16→36 | AUTO-BILATERAL | bono | Captain Q-DEC delegation pending — DRAFT extension authoring autonomous |
| L7.8 | cirs_lookup_handler phone-leak fix | CAPTAIN-GATED-MERGE | james | Wave 1 billing-engine; james-side; LBAC stages handoff |

WIP-cap budget: 3 concurrent for bono; james parallel pool per his harness asymmetry.

**Next-3 pickup (pre-activation, under existing standing rules):**
1. L4-PR17 (§S-186 fast-lane RCA authoring)
2. L12-cont (Layer 12 ops-hygiene sweep continuation)
3. L4-PR54 (review-ready polish on bono's own PR)

These three are autonomous-eligible under existing Apply-Recommendations-Autonomously rule (Q1+Q2+Q3 cleared, no canonical-boundary touch, bono surfaces only). LBAC activation does not unlock new authority for them — only structures the closed-loop discipline.

---

## §13 Versioning + change log

- **v0.1 (2026-05-12 07:15 IST)** — initial draft authored by bono. Captain ratification pending. § S-203 ratification anchor staged.
- **v0.1 ACTIVATION (2026-05-12 ~07:23 IST)** — Captain RATIFIED via verbatim "authorize LBAC activation and bono VPS redeploy"; §S-203 ratification anchor landed.
- **v0.1 AMENDMENT §S-220 (2026-05-13 ~10:25 IST)** — MAOR v0.1 REVIEW step inserted between Step 4 FIX and Step 5 CLOSE per Captain auth "Authorize §S-220 publish"; AMPLIFIER A1+A2+A3 encoded as §14.1 below. Composes-with `MAOR-PROTOCOL.md` (racecontrol commit `0360fde9`).
- **v0.1 AMENDMENT §S-221 (2026-05-13 ~10:40 IST)** — F1 SCOPE GATE + F3 ACCOUNTING REFORM ratified as language per Captain auth "Proceed" + named-surface §S-221 in immediate-prior context. Encoded as §14.2 + §14.3 below. Composes-with `MMA-orchestration-fix-bono-2026-05-13` findings (comms-link commit `d3480014`) + `F1-GATE-RETROSPECTIVE-20260513.md` (racecontrol commit `1aec0e23`).
- **v0.1 INLINE ENCODING (2026-05-13 ~10:55 IST)** — F1+F3+MAOR amendments encoded inline at this canonical doc per Captain verbatim "V-LBAC-PROTOCOL.md F1+F3 inline encoding". §14 added below. Cascade flow updated in §14.4.

Updates land via §S-N amendment + bono memory feedback file rotation.

---

## §14 — Amendments (post-§S-203 ratify)

This section encodes the doctrine amendments ratified at §S-220 (2026-05-13 ~10:25 IST) and §S-221 (2026-05-13 ~10:40 IST). Original §1-§13 doctrine remains active; §14 amendments compose with and supersede where explicitly noted.

### §14.1 — §S-220 MAOR v0.1 REVIEW step insertion (Captain auth verbatim "Authorize §S-220 publish")

**Closed-loop cascade flow update:** Step 4.5 REVIEW inserted between Step 4 FIX and Step 5 CLOSE.

Original §3 cascade (8 steps):
```
Step 1 OPEN → Step 2 DESCEND → Step 3 H1 → Step 4 FIX → Step 5 CLOSE → Step 6 SWEEP → Step 7 SYNC → Step 8 BILATERAL
```

Post-§S-220 cascade (9 steps):
```
Step 1 OPEN → Step 2 DESCEND → Step 3 H1 → Step 4 FIX → Step 4.5 REVIEW (NEW) → Step 5 CLOSE → Step 6 SWEEP → Step 7 SYNC → Step 8 BILATERAL
```

**Step 4.5 — REVIEW (MAOR Tier-1 batch, mandatory every iter):**

- 1 `feature-dev:code-reviewer` agent reviews the full N-file batch in one pass (different subagent type from authors)
- Confidence-filtered to ≥75
- Catches cross-file consistency issues that per-file review misses
- Output: per-file verdict (clean | N findings) — silent pass NOT allowed
- Block/pass rules: 0 CRITICAL = push immediately; 0 CRITICAL ≥1 IMPORTANT = push with review-noted disposition logged in §S-N close-anchor; ≥1 CRITICAL = block push until fixed OR Captain explicitly accepts with rationale
- Tier-2 per-file deep dive triggered by ≥1 CRITICAL OR N>7 files

**3 independence axes (mandatory; all three apply):**
1. Different subagent type (authors=`general-purpose`; reviewer=`feature-dev:code-reviewer`)
2. No shared context (spawn reviewer via Agent tool default — fresh context; reviewer cannot see author's reasoning, only the artifact)
3. Reviewer reads authoritative sources independently (never trust author's DoD/spec/RCA citations; reviewer must grep cited line itself)

**Anti-rubber-stamp briefing requirements (mandatory):**
1. Confidence threshold ≥75
2. DO NOT report list (generic suggestions / refactor proposals / "could be more tests" / style padding banned)
3. What's intentional (env-gated SKIP / gap-discovery / doctrinal header patterns)
4. Verdict requirement (explicit per-file "clean" or N findings)
5. Source-of-truth pointers (DoD path / RCA file path; reviewer must read independently)

**Re-review after fixes (close-loop):** Same protocol applies to FIX commits. Tier-1 MAOR re-review on fixes BEFORE pushing fix commit; reviewer validates VERIFIED-FIXED / PARTIAL / REGRESSED / NEW-ISSUE per finding + scans for new issues.

**Bilateral default-off opt-in:** Self-review default (each pilot reviews own iter, cheapest, adequate for most cascades). Opt-in cross-pilot review reserved for foundational-boundary iters per V1↔V2 RCA escalation pattern (billing / wallet / auth / pod-state-channel / WhatsApp identity / DB schema).

**Cost calibration (empirical 2026-05-13):** Tier-1 batch ~$0.20-0.30 / ~110s wall-time; Tier-1 re-review ~$0.10-0.20 / ~50s. Total MAOR overhead: ~$0.30-0.50 / ~3min per iter. <50% overhead added to cascade.

**v0.1→v0.2 promotion criteria (AMPLIFIER A2 tightened §S-220.5):**
- N≥5 iter cascades have completed REVIEW step
- Reviewer caught ≥1 real defect AT EACH iter (not just cumulative)
- 0 false-positive findings of "rubber-stamp inverted" class
- 0 false-negatives detected by retrospective Captain disposition across N iters

**A3 hook enforcement (§S-220.6):** `pre-push-maor-check.js` v0.1 priority (PreToolUse blocker on Bash patterns matching `git push.*racecontrol` when uncommitted contract-test files exist without MAOR receipt at `.planning/specs/v2/MAOR-RECEIPTS/<surface>-<date>.json`). **Install gated on Captain explicit harness-mechanism-auth per-session naming the path** `~/.claude/hooks/pre-push-maor-check.js`.

**Empirical anchor:** First application 2026-05-13 ~09:55 IST surfaced 4 real defects (1 CRITICAL paise/credits cross-file unit confusion + 3 IMPORTANT — PII fixture violation + broken Axum skip-gate + dead-end composition test) on §S-217+§S-219 6-file batch (~3700 LOC). Re-review verdict clean post-fix.

**Canonical:** `.planning/specs/v2/MAOR-PROTOCOL.md` (racecontrol commit `0360fde9`); §S-220 ratify anchor at comms-link V2-MASTER-STATE.md commit `c09e2723`.

### §14.2 — §S-221 F1 SCOPE GATE: substrate-existence pre-spawn verification (Captain auth verbatim "Proceed" + named-surface)

**Closed-loop cascade flow update:** Step 3.5 F1 GATES inserted between Step 3 H1 PLAN and Step 4 FIX, fires when the row is acceptance-test-scaffolding class (composes-with §1.5 of original doctrine).

**Step 3.5 — F1 SCOPE GATE (mandatory for acceptance-test scaffolding work):**

| Gate | Check | If absent → |
|---|---|---|
| **G-F1-1** | Endpoint exists in `racecontrol/src/api/routes.rs` (or sub-router) | file row as `ENGINEERING-IN-FLIGHT (substrate-missing)` |
| **G-F1-2** | Configurable threshold/constant exists in `racecontrol/src/` (e.g., `MAX_DISCOUNT_PCT`, `BUSINESS_HOURS`) | file as `ENGINEERING-IN-FLIGHT (configurable-missing)` |
| **G-F1-3** | Field shape exists in `racecontrol/src/{state,api}/` (e.g., `last_telemetry_at`, `current_lap`) | file as `ENGINEERING-IN-FLIGHT (shape-missing)` |
| **G-F1-4** | Behavioral mechanism exists in `racecontrol/src/billing/` or relevant module | file as `ENGINEERING-IN-FLIGHT (mechanism-missing)` |
| **G-F1-5** | Composes-with §S-146 V1↔V2 RCA gate for foundational-boundary rows | RCA-first |

**Verdict logic:** if ALL 4 gates PASS → row qualifies as `TEST-SCAFFOLDED` (substrate exists; test is the missing piece). If ANY gate FAILS → row is `ENGINEERING-IN-FLIGHT` with sub-state per failed gate; test is premature; substrate work is the gating item.

**Exception — SCAFFOLD-AHEAD:** Captain may explicit-auth a `SCAFFOLD-AHEAD` iter for FORWARD-INTENT classes — kaizen-correct V1-retention exemptions where substrate is intentionally absent for V2-doctrine reasons (e.g., `/operating-window` endpoint absent because V2 routes through `/scheduler/status`; the test encodes the V2 contract anyway). Must be logged as `SCAFFOLD-AHEAD` with explicit Captain quote in the row's §S-N close-anchor.

**Anti-pattern BLOCKED:** authoring an env-gated SKIP test that asserts a behavioral expectation against a V1 substrate that doesn't exist, then flipping V2-PROGRESS-MAP row to IN-FLIGHT, and reporting "acceptance test authored — substrate landing happens at V1↔V2 wire-up time". This is the racing-pattern MMA root-cause; F1 closes it.

**Empirical anchor:** F1-gate retrospective audit 2026-05-13 ~10:55 IST (`F1-GATE-RETROSPECTIVE-20260513.md` commit `1aec0e23`) applied F1 to §S-213→§S-219 cascade rows: 28% PASS / 11% PASS-CONDITIONAL / **61% FAIL**. Direct empirical confirmation of MMA scope-quality root cause hypothesis (3-of-3 model consensus 2026-05-13).

### §14.3 — §S-221 F3 ACCOUNTING REFORM: TEST-SCAFFOLDED ≠ ENGINEERING-IN-FLIGHT (Captain auth verbatim "Proceed" + named-surface)

**V2-PROGRESS-MAP row status definitions amend (composes-with V2-PROGRESS-MAP §0 commit `8b1a7850`):**

| State | Definition | Counts toward V2.0 % closed? |
|---|---|---|
| `ENGINEERING-IN-FLIGHT` | code work in progress toward V2.0 unblock; V1 substrate exists; will close on behavior observable at V2 entry point (PWA/kiosk/staff app) | **YES** |
| `TEST-SCAFFOLDED` | acceptance test authored env-gated SKIP-with-reason; V1 substrate may or may not exist; tracked separately at §0.X rollup card | **NO** |
| `TEST-SCAFFOLDED → ENGINEERING-IN-FLIGHT` | promoted when F1 G-F1-1..4 gates pass for that row | **YES** post-promote |
| `ENGINEERING-IN-FLIGHT (substrate-missing)` | F1 G-F1-1 surfaced engineering item; row is real V2.0 blocker | YES (when reframed) |
| `ENGINEERING-IN-FLIGHT (configurable-missing)` | F1 G-F1-2 surfaced engineering item | YES |
| `ENGINEERING-IN-FLIGHT (shape-missing)` | F1 G-F1-3 surfaced engineering item | YES |
| `ENGINEERING-IN-FLIGHT (mechanism-missing)` | F1 G-F1-4 surfaced engineering item | YES |
| `DONE` | behavior observable at V2 entry point + acceptance test passes (no SKIP) | **YES** |

**Forward-only disposition decision:** §S-204+ rows that flipped to IN-FLIGHT under prior accounting are NOT retroactively reclassified. They remain IN-FLIGHT until their next disposition cycle, at which point F3 framing applies. Forward-only avoids ledger churn at the cost of carrying ~18 rows under mixed accounting for the §S-220+ window. Rationale: retroactive reclassification would require ~3-5h bono effort against §S-204 cascade with no closure-rate benefit.

**Closure rate restatement under F3 (Layer 1 example):**

| Reading | Layer 1 figure |
|---|---|
| Pre-§S-221 reported | "Layer 1 acceptance-test cascade phase essentially complete" — 19/20 rows DONE/IN-FLIGHT/PARTIAL/BLOCKED |
| Post-§S-221 F3 framing | 2 DONE (1.5 + 1.18) + 5 TEST-SCAFFOLDED + 2 PASS-CONDITIONAL + 11 ENGINEERING-IN-FLIGHT — **true ENGINEERING completion ~10%** |

V2.0 % closed restatement at next §0 rollup refresh per §16 stale-at 2026-05-18 (estimate under F3: ~20-25% from currently-reported 32%).

### §14.4 — Updated closed-loop cascade flow (post-§S-220+§S-221)

Original §3 cascade (8 steps) → Post-amendment cascade (10 steps):

```
Step 1 OPEN — capture user-facing symptom
Step 2 DESCEND — 6-layer trace to root cause (Smoke → Function → Boundary → Infra → Data → Code)
Step 3 H1 PROBLEM/SYMPTOMS/PLAN — CGP gate
Step 3.5 F1 SCOPE GATE (NEW §14.2) — pre-spawn substrate verification G-F1-1..5
            └── if FAIL: reframe row as ENGINEERING-IN-FLIGHT (sub-state); FIX is substrate engineering
            └── if PASS: row is TEST-SCAFFOLDED-eligible; proceed
Step 4 FIX — smallest reversible change, atomic commit
Step 4.5 REVIEW (NEW §14.1) — MAOR Tier-1 batch (mandatory)
            └── 0 CRITICAL: proceed to push
            └── ≥1 CRITICAL: block until fixed; re-review per §14.1
            └── 0 CRITICAL ≥1 IMPORTANT: push with disposition logged
Step 5 CLOSE — H3 evidence chain (raw command + raw output + WHERE + NOT TESTED)
Step 6 SWEEP — H4 per-target enumeration (Server .23 / Pods 1-8 / POS .130 / James .27 / Bono VPS / Cloud apps / Comms-link)
Step 7 SYNC — Universal Sync rule (CLAUDE.md + comms-link + memory + V2-MASTER-STATE §S-N + V2-PROGRESS-MAP)
Step 8 BILATERAL — 4-leg close-loop (author-commit / partner-publish / partner-confirm / end-to-end-ping)
```

### §14.5 — Pre-spawn OPEN-CLAIM mechanism (§S-219 iter4 mitigation; pending v0.2 promotion)

For multi-agent cascade work (parallel general-purpose subagent orchestration):
1. Publish §S-N OPEN-CLAIM entry on V2-MASTER-STATE.md naming claimed rows + push BEFORE agent spawn
2. Partner pilot sees CLAIM via session-start `partner-memory-read.js` + send-message.js inbox
3. Partner picks NON-OVERLAPPING rows if cascading concurrently
4. Close-anchor §S-N appended within ~10min target

**Status:** CANDIDATE pending MAOR v0.2 absorption (§S-220.12 carry-forward). Not promoted to standalone V-LBAC v0.2 amendment per AMPLIFIER review.

### §14.6 — DEPRECATE-trigger active watch (2026-05-13 → 2026-05-20)

Per MMA Q4 stop-condition: if post-fix gap rate ≥20% per cascade OR new sub-class emerges within 7d of §S-220+§S-221 ratify (2026-05-13), bono surfaces DEPRECATE recommendation for multi-agent orchestration methodology in favor of simpler serialized workflow (single-agent + F1+F3 + manual review).

**Baseline measurement:** pre-fix gap rate ~26% (per F1-gate retrospective §5.3; 5 gap-class instances across 19 row-cascades). Already exceeds 20% threshold pre-fix. Forward 7d watch measures POST-fix gap rate under F1+F3+MAOR active.

**Triggers:**
- ≥20% gap rate (any iter5+ cascade)
- NEW sub-class emergence not in iter1-4 catalogued list (Write-BLOCKED / concurrent-cascade-collision / probe-as-body / FALSE-SUCCESS-REPORT / UNTRACKED-FILE-LIVE-SYNC-CLOBBER)

**Window:** 2026-05-13 → 2026-05-20 (stale-at). If triggered, bono surfaces DEPRECATE recommendation; Captain decides whether to proceed with deprecation or accept extension.

### §14.6.1 — Candidate C ratify (cascade-class-stratified DEPRECATE thresholds)

**Status:** RATIFIED 2026-05-13 ~20:18 IST per V-LBAC-DEPRECATE-TRIGGER-AUDIT-20260513.md interpretation analysis. Captain disposition under §S-265 pre-commitment exception (item 5 of 8 in NEXT-SESSION DIRECTIVE 19:10 IST queue) + continuation directive 19:26 IST ("Proceed to complete all LIVE blocking with MOAR. If you still ask my permission on any task, consider it a gap and fix it."). Audit-trail anchor: comms-link §S-268.

The single ≥20% threshold in §14.6 is REFINED to cascade-class-stratified thresholds (each class measured independently) per Candidate C:

| Class | Definition | Threshold | Measurement source |
|---|---|---|---|
| **Class M (multi-agent test)** | Cascade where ≥2 parallel general-purpose subagents author contract tests in single window | post-fix STRUCTURAL-GAP rate ≥20% | iter5+ multi-agent test cascade output (currently stand-down) |
| **Class A (atom-substrate)** | Cascade where atomic substrate atoms (≤10 LOC each) authored + MAOR Tier-1 reviewed | post-fix real-defect MAOR rate ≥20% per atom | `~/.claude/state/maor-audit.jsonl` per-atom CRITICAL count |
| **Class U (audit-only)** | Audit deliverable cascade (RCA / forensic / verification audits) — no code substrate | post-fix correctness-finding rate ≥20% per audit | post-Captain-ratify finding count |

**DEPRECATE recommendation scope (Candidate C policy):** class-scoped, NOT blanket. A Class A DEPRECATE retires atom-substrate-cascade methodology in favor of single-agent atom authoring (which is effectively what Phase β PR #72 already was). A Class U DEPRECATE retires audit-only cascade in favor of single-pilot audits. Class M DEPRECATE requires iter5+ measurement first (cannot trigger on stand-down state).

**Current data anchors (forward 7d window 2026-05-13 → 2026-05-20):**
- **Class A:** Phase β (PR #72 / commit 1a2991b4) — 5 atoms · 0 real-defect MAOR Tier-1 findings (CONDITIONAL-DISPOSITIONED-PASS via §S-263 Finding-1 PRE-EXISTING + Finding-2 FALSE-POSITIVE) · **0% gap rate** (well under threshold)
- **Class U:** §S-262 iter11 deploy-surface verify — 0 audits with post-Captain-ratify findings · **N/A** (insufficient datapoints)
- **Class M:** stand-down (unmeasured · cannot DEPRECATE-trigger without iter5; preserves §S-219 iter4 last-known state)
- **D-CLUSTER-8 rename (PR #73 / commit 670b5531):** Class A · 0 real-defect MAOR findings · MAOR Tier-1 self-review PASS · contributes 0% to Class A gap rate

**Class M stand-down maintenance protocol:** V-LBAC-5 forward window 2026-05-20 extension OR explicit Captain authorization to run iter5 multi-agent test cascade required for forward measurement before window closes. If neither occurs by 2026-05-20: bono surfaces Class M decision-point (extend stand-down further · run forward measurement cascade · DEPRECATE Class M without measurement as predetermined).

**Anchor:** V-LBAC-DEPRECATE-TRIGGER-AUDIT-20260513.md (racecontrol commit `6edbb51c`) · §S-268 close-anchor (comms-link, this session) · §S-265 pre-commitment exception (covers item 5 of 8).

### §14.7 — Composes-with extended

- **§S-220 MAOR v0.1 RATIFY** — `.planning/specs/v2/MAOR-PROTOCOL.md` (commit `0360fde9`) + V2-MASTER-STATE comms-link `c09e2723`
- **§S-221 F1+F3 RATIFY** — V2-MASTER-STATE comms-link `048081f1`
- **MMA findings doc** — `comms-link/.planning/research/mma-multi-agent-orchestration-fix-20260513.md` (commit `d3480014`)
- **F1-gate retrospective audit** — `.planning/specs/v2/F1-GATE-RETROSPECTIVE-20260513.md` (commit `1aec0e23`)
- **V2-PROGRESS-MAP §0 F3 framing note** — `racecontrol` commit `8b1a7850`
- **MMA Protocol v4.0 Phase 3 OpenRouter** — canonical MMA transport (`feedback_mma_channel_openrouter_not_perplexity_20260511.md`)
- **§S-N close-anchor push standing rule** — `feedback_sn_close_anchor_push_standing_rule_20260512.md` (atomic edit→add→commit→push)
- **Harness-mechanism-auth sub-clause** — `feedback_apply_recommendations_autonomously_20260510.md` (A3 hook install gating)

### §14.8 — V-LBAC-5 forward gap-rate watch (new self-test)

Added to §10 success metrics:

- **V-LBAC-5:** Forward 7d gap rate <20% post-§S-220+§S-221 ratify — ACTIVE WATCH (DEPRECATE-trigger at 2026-05-20)

Conditional on V-LBAC-5 PASS for v0.1 → v0.2 promotion. v0.1 → v0.2 promotion: all of V-LBAC-1 + V-LBAC-2 + V-LBAC-3 + V-LBAC-4 + V-LBAC-5 PASS → promote with refinements; any FAIL → root-cause + structural amendment OR DEPRECATE recommendation.

---

— bono · 2026-05-13 ~10:55 IST · V2-LBAC-PROTOCOL.md v0.1 amendments §14 encoded inline per Captain commission "V-LBAC-PROTOCOL.md F1+F3 inline encoding" · §14.1 §S-220 MAOR REVIEW step insertion + AMPLIFIER A1+A2+A3 + §14.2 §S-221 F1 SCOPE GATE (G-F1-1..5 + SCAFFOLD-AHEAD exception + anti-pattern BLOCKED) + §14.3 §S-221 F3 ACCOUNTING REFORM (TEST-SCAFFOLDED ≠ ENGINEERING-IN-FLIGHT + forward-only + closure rate restatement) + §14.4 updated 10-step cascade flow + §14.5 pre-spawn OPEN-CLAIM mechanism + §14.6 DEPRECATE-trigger active watch + §14.7 composes-with extended + §14.8 V-LBAC-5 forward gap-rate watch · canonical path matches `racecontrol/CLAUDE.md` doctrine pointer · §13 change log updated with §S-220 + §S-221 + inline-encoding entries
