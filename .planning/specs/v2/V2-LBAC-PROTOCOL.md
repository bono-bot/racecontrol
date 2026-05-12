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

Updates land via §S-N amendment + bono memory feedback file rotation.
