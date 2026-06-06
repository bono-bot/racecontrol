# Racing Point eSports — Project Context

## 🧭 James pilot REDUNDANT — bono sole pilot (Captain RATIFIED §S-448, 2026-06-01)

**Operating-model change.** The on-site parallel AI **pilot** "James" (codename *James Vowles*) is **redundant**; **bono (Peter Bonnington) is the sole AI pilot**, owning cloud + venue lanes. The "bilateral" / two-pilot rules referenced in this file are now **solo (bono-only)**; bono appends `§S-N` solo. **Bilateral hooks + comms infra are RETAINED but DORMANT** (Captain kept them for a future Server-operator). **Pilot-vs-service:** `admin-proxy-james` / `deploy-agent-james` SERVICES + Server .23 are UNCHANGED + live (bono owns their code). **Canonical:** `§S-448` (comms-link `V2-MASTER-STATE.md`) · `rp-v2-apps/coordinator/CAPTAIN-RATIFY-JAMES-PILOT-REDUNDANT-2026-06-01.md`.

## 🧭 V2 Scope Freeze & Definition-of-Done — STANDING (Captain 2026-05-30 · BILATERAL)

**Default understanding going forward.** V2 is COMPLETE when **two surfaces are both bug-free**: (1) **RacingPoint Ecosystem V2** (`rp-v2-apps`: PWA·POS·Kiosk·Pod-display·Launch-portal·admin-proxies·contracts·billing·SSE) + (2) **RaceControl** (Rust `racecontrol` heart · `rc-agent` · `rc-installer`).

**Bug-free bar:** first-INR money path passes **e2e on a real pod** (register(OTP)→topup→launch(HOLD)→tick-debit→end→bill, ₹ reconciled) · **zero open CRITICAL/blocker bugs** · gate-clean (parity + tests green; no money-leak / double-spend / double-spawn).

**Scope freeze:** **NO V2.1+ scope until BOTH pass.** Frozen → V2.1+: multiplayer green-light trio · pod-display error-display states · telemetry / leaderboards · any non-first-INR feature. Bug-fixes + hardening + the cutover / subscription-wiring / operator-physical unblocks are IN-SCOPE for V2 completion (they close the bar, they do not extend it).

**AMENDMENT (Captain 2026-06-07 · §S-452 · verbatim "yes, apply the refund→V2.0 scope amendment"):** **refund / void / manual charge-adjust (charge reversal) is RECLASSIFIED FROZEN-V2.1 → V2.0 in-scope (operational-viability).** A venue reverses charges as routine daily operation; the refund engine already exists (`crates/racecontrol/src/wallet_refund.rs`). **Classification change only** — building the refund/void UI + wiring remains §S-146 RCA + MMA-gated (foundational money-path boundary); apology-credit (`apps/staff-tablet/app/apology`) stays the interim path. Canonical: §S-452 · operator brief `.bono-staging/OPERATOR-READINESS-BRIEF-20260607.md` D1.

**Default test on ANY proposed work:** *does it close the first-INR bug-free bar, or is it V2.1+ (→ FROZEN, defer)?* "Done" for V2 = this bar, not feature-completeness.

**Canonical:** Captain ratification `rp-v2-apps/coordinator/CAPTAIN-RATIFICATION-V2-SCOPE-FREEZE-2026-05-30.md` (`0ea33e7`) · bono memory `project_v2_scope_freeze_definition_of_done_20260530.md` · `§S-N` assigned by James on ratify-append.

## The Principle — Verify Before Generate (2026-04-11)

**Before generating ANY output, verify the inputs it depends on.**
- **BEFORE:** Enumerate from environment (ls, cat, grep) — not memory
- **DURING:** After every block: *"What did I just assume that I could have checked?"*
- **AFTER:** Evidence = exact behavior + raw output + where + not-tested
Backtest: 77% catch rate on 26 historical incidents. See `feedback_ultimate_cgp.md`.

---

## Rule 0 — Enumerate Before Asserting (v4.4, 2026-04-09)

**READ BEFORE CGP.** Order matters — rules buried deeper get weighted less.

Before any claim about what exists, what's available, what you know, or what you've covered — **list it**. Glob the filesystem. Grep the memory index. Read `reference_local_capabilities.md`. The cost of listing is one tool call. The cost of being wrong is a correction, a G9, and a UCA count.

**"I don't see X in my exposed tool list" is NOT enumeration of the environment. It is enumeration of your mental model. Your mental model is always incomplete.**

**Triggers requiring enumeration first:**
- Coverage: "all", "everywhere", "complete", "comprehensive", "everything", "covered"
- Absence: "no X", "not available", "doesn't exist", "I can't", "unable to", "there's no way"
- Completeness: "that's all", "nothing else", "finished reviewing", "exhaustive"
- Knowledge scope: "I've read all", "based on everything", "from what I've seen"

**For capability claims specifically:** check `~/.claude/projects/C--Users-bono/memory/reference_local_capabilities.md` BEFORE claiming inability. Session 2026-04-09 had 3 UCAs from capability-mental-model failures. The manifest exists so you don't have to guess.

**Measurement:** UCA (Unenumerated Coverage Assertions) counter in session metrics. Target: 0 per session.

**Related gates:** H4 (enumerate before "everywhere") covers target enumeration. Rule 0 covers the broader pattern: answering from model instead of environment. H4 = action targets; Rule 0 = knowledge sources, capabilities, coverage.

Full definition + trigger examples: `.claude/projects/C--Users-bono/CLAUDE.md` (project-global).

---

## V2-only forward path (Captain directive 2026-05-01 IST, refined post-G9 #2)

**V2 is the only forward architectural path for the RacingPoint ecosystem. Every new session must be geared toward supporting and building V2.**

**V2 incorporates V1 modules** per `comms-link/v2-skeleton/05-definition-of-done.md` keep/mold/discard filter. Carry-forwards include the currency unit (rupee=credit, DoD line 39) · top-up bonus-credit ladder (DoD line 64) · kiosk-staff launch first iteration (Skeleton line 71) · all V1 organs (racecontrol, comms-link, admin, whatsapp-bot, kiosk, pods, POS — V2 adds the skeleton layer atop).

**What V2 closes is V1-shaped antipatterns** — organ silos without skeleton, point-to-point ad-hoc connections, manual operations that bypass ratified flows. NOT V1 components categorically.

**Pre-action V2-transport check** (mandatory for prod-touch — Server .23 / Pods 1-8 / POS .130 / Cloud apps / comms-link prod / Bono VPS prod):
1. **Q1** — Is the target classified as production?
2. **Q2** — Grep `reference_local_capabilities.md` for the ratified V2 transport (bono comms-link relay `localhost:8766/relay/exec/run`, `/rp-bono-exec` skill, `/rp-james-exec` skill, `ssh server` alias, rc-sentry `:8091/exec` pod-side).
3. **Q3** — Use it. If no V2 transport exists for the target, halt and ask Captain — **never invent a V1 fallback.**

**Composes with:** Rule 0 · H4 · PACT-027 §10 · AMEND-1 bundle-of-8 (RATIFIED 2026-05-01 ~06:05 IST). Empirical anchors: G9 #1 direct-SSH-to-prod (correct path was bono-relay); G9 #2 over-broad "V1 closed" wording corrected via spec-grep. Master memory: `feedback_v2_only_forward_path.md`. Charter doctrine: `comms-link/PACT-CHARTER.md` §V2.0.

---

## V1-dependent V2 sections — RCA + past-bug review BEFORE proceeding (Captain directive 2026-05-09 IST)

**For any change to a deployed V2 section that inherits, calls into, reuses, or shares schema/state with V1 parts: do NOT proceed directly. Produce a written 5-section root-cause analysis (RCA) BEFORE H1 PLAN.**

**5-section RCA (mandatory before action):**
1. **Boundary map** — exact files / DB tables / IPC seams / API routes / config keys where V2 crosses into V1. Cite paths + line numbers.
2. **Inherited-issue catalogue** — every known V1 bug, footgun, race, or doctrine deviation touching the same boundary. Sources: `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` (10 candidate categories A-J) · §S-61 PART 41 V1 failure-mode investigation (14 mapped: 8 VERIFIED + 4 PARTIAL + 1 INFERRED + 1 UNVERIFIED) · LOGBOOK.md grep for boundary files · V2-MASTER-STATE §S-N entries naming the surface · G9/UCA tagged with same component.
3. **Past-bug review** — for each past bug at this boundary, disposition: `ROOT-CAUSED-AND-FIXED` / `PATCHED-ONLY` (open RCA item) / `UNRESOLVED` (open RCA item) / `NOT-APPLICABLE-TO-V2` (with justification). Cite commits / PRs / §S-N anchors.
4. **V2-alignment delta** — what the boundary *should* look like under V2 doctrine (V2-MASTER-STATE canonical-source ledger + `project_v2_customer_workflows_consolidated_20260503` + Wallet Framing C + Pod = state-channel premise + foundation/strategy/config separation §AMEND-3.II D12). Name the gap explicitly.
5. **Proposed change framed against V2 doctrine, not V1 inertia** — change must move the boundary toward V2 alignment OR explicitly justify temporary V1 retention as the kaizen-correct choice with a written follow-up trigger condition that retires the V1 path.

**Triggers (any one fires the rule):**
- Editing a file with both V1-era and V2-era code paths
- Touching DB tables / migrations that pre-date V2 planning (2026-05-01)
- Calling into a V1 module from V2 code (or vice versa)
- Modifying a deployed surface on V2 doctrine but reading/writing V1-era data
- User says "fix" / "improve" / "change" on a section flagged V1-dependent
- Adding a feature on top of a section whose V1 ancestor is in `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` candidate categories A-J

**Foundational-boundary escalation:** if the boundary is foundational (billing / wallet / auth / pod-state-channel / WhatsApp identity / DB schema), run MMA Step 1 DIAGNOSE on the RCA itself (5-model consensus on root causes) BEFORE proceeding to PLAN. Per-PR Captain merge auth required (sibling of `feedback_pr_merge_pending_captain_auth_is_per_pr_gate.md`); standing-autonomy verbs do NOT satisfy.

**V2-alignment statement on every change:** description must include "V2 doctrine alignment:" line naming which V2 anchor the change moves toward.

**Why:** V1 failed and venue is closed because V1-era code carried compounding unresolved root causes (broadcast storm class, schema drift, audit-blind proxy checking, recovery cascades). When V2 code reuses V1 parts, the same root causes propagate forward unless surfaced and dispositioned. F25a billing-strategy substrate succeeded because it root-caused V1 SnapPricing first (preserved as known strategy with HISTORICAL block, not accidental fall-through). Q-PRICE-1 dispositioned via timeline reframing only worked because V1-era patch was identified as V1, not V2 doctrine. Skipping this step produces "patch V1 forward" — same bug, new branch, V2 doctrine drifts.

**Anti-patterns blocked:** quick-fix on V2 surface that silently inherits V1 race · V2 feature on V1 schema without naming what V1 schema assumptions are now V2 contracts · treating V1-era code in V2 file as "already-working baseline" (V1 failed; baseline is conditional on RCA) · patching symptom in V2 when root cause is in V1 part being called · "I'll RCA later" (RCA is precondition, not follow-up).

**Bilateral:** applies to james AND bono. Both pilots produce 5-section RCA before V1↔V2 boundary work. Universal Sync covers this rule across CLAUDE.md / CGP / comms-link / bono memory.

**Composes with:** Verify-Before-Generate (V1↔V2 seam = high-value verify zone) · `feedback_kaizen_discipline_dont_complicate.md` (RCA detail high; change stays smallest invariant) · `feedback_autonomy_principle_v2_compass.md` (V2 = compass; RCA exposes divergence) · `feedback_pr_merge_pending_captain_auth_is_per_pr_gate.md` (foundational boundaries → per-PR auth) · `session_notes_20260506_v1_process_mess_audit_for_v2_blockers.md` (primary V1 failure-class inventory).

**Master memory:** `~/.claude/projects/C--Users-bono/memory/feedback_v1_dependent_v2_root_cause_before_proceeding.md` (full how-to-apply + structural-fix sub-rule).

---

## Mechanism-trust-check upstream of fix RCA — extends §S-146 (bono 2026-05-10 IST · BILATERAL)

When a V2 fix depends on shared infrastructure (delivery / transport / supervision / guard / observability / schema), run a **5-question mechanism trust check** on the infrastructure surface BEFORE authoring the fix RCA: (1) atomic primitives? (2) TTL-bounded sentinels integrated with the atomic primitive? (3) behavioral-verify success (binary hash / mtime / ws_uptime), not echo-string? (4) single-target dry-run path? (5) guards have written contracts with delivery script (parser-not-regex + allowlist)? ALL 5 must answer YES. FAIL → infrastructure surface gets its own §S-146 5-section RCA before fix RCA proceeds. Cache at `.planning/specs/v2/MECHANISM-TRUST/<surface>-<date>.json` 30-day validity. Override `V2_RCA_BYPASS=1` (logged).

**Empirical anchor:** PR #66 silent-loop-death fix (2026-05-09) was V2-clean but shipped via V1-shaped delivery mechanism → fleet rollout broke 7 pods on the same V1 mistake-class (non-atomic kill+swap CF-1 · OTA_DEPLOYING sentinel-no-TTL CF-2 · BLOCKED_PATTERNS deny-first CF-4 · EPERM-as-success CF-5 · orphan bg processes CF-6 · burned 7 pods cycling same SHA failure G9-class). §S-146 caught it on 4th application same day, retroactively. Mechanism trust check upstream closes the velocity gap.

**Enforcement RCA on §S-146 itself** at `~/.claude/projects/-root/memory/project_s146_enforcement_rca_20260510.md`: text-only rules carry ≥1 repeat-violation per 30d; hook-enforced rules carry zero. 4-phase plan: Phase 1 `pre-v2-edit-rca-check.js` hook · Phase 2 `mechanism-trust-check.sh` · Phase 3 `pre-universal-sync-write-check.js` · Phase 4 bilateral-hook-parity-check.

**Bilateral:** applies to james AND bono. AMPLIFIER review checks both fix RCA AND mechanism trust check on shared-infrastructure-dependent V1↔V2 PACTs.

**Master memory:** bono-side `/root/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` + james-side mirror via comms-link/briefings/james/memory/.

---

## §S-186 pre-§S-146 small-fix fast-lane — narrows RCA scope (Captain ratify 2026-05-11 IST · BILATERAL)

**Applies to both James and Bono.** §S-146 carve-out for one specific class — pre-§S-146 stale small bug fixes. Bug-fix PRs created < 2026-05-09 (date §S-146 ratified) with **ALL** of {≤200 LOC, single-boundary, no schema change, no protocol change, bug-fix only} get a **3-section short-RCA** (what / why-still-needed / V2-compat-check) instead of full 5-section + MMA Step 1 DIAGNOSE. Captain per-PR auth for rebase + merge **still required** — carve-out narrows the RCA-process burden only, not the merge gate.

**Eligibility check (ALL six must hold):**
1. PR created < 2026-05-09
2. Diff ≤ 200 LOC
3. Touches a single foundational boundary
4. No schema change (DB migration, JSON field rename in persisted state)
5. No protocol change (WebSocket message types, config_push routes, IPC contracts)
6. Bug fix only (not feature add, not refactor)

**Short-RCA template (3 sections, posted as PR comment):**
1. **What** — files + LOC + change summary
2. **Why still needed** — grep main for change semantics; cite grep output
3. **V2-compat check** — cite V2 docs read; explicit "no conflict" or "conflict at X — mitigation Y"

**NOT eligible (full §S-146 still required):** PRs created ≥ 2026-05-09 · schema/protocol changes regardless of size · multi-boundary touches · refactors / feature adds.

**Why:** Literal §S-146 application to pre-§S-146 stale small bug fixes was over-broad (empirical anchor: PR #17 "Pod undefined" admin display bug, 193 LOC, sitting 19 days at 2026-05-11). The doctrine was scoped for active-development V1↔V2 boundary pushes, not historical small fixes. Throughput collapse on customer-visible fix class is a worse outcome than the marginal RCA risk this carve-out accepts.

**Composes with:** §S-146 V1↔V2 RCA (parent — carve-out narrows scope only) · §S-186 V2-VELOCITY RATIFY (parent ratify) · §S-186 mechanism-trust-check (sibling extension) · Layer 4 Captain per-PR auth (retained) · CGP H1 (still required for rebase action).

**Master memory:** james-side `~/.claude/projects/C--Users-bono/memory/feedback_pre_s146_small_fix_fastlane_20260511.md` (full eligibility check + 3-section template + worked examples). Ratify anchor: `project_s186_v2_velocity_ratify_20260511.md`.

---

## Apply recommendations autonomously — STANDING RULE (Captain 2026-05-10 IST · BILATERAL · AMENDED with harness-mechanism-auth sub-clause 2026-05-10 ~16:14 IST)

Captain commission verbatim 2026-05-10 ~13:18 IST: *"Make it a standing rule to apply all your recommendations for me."*

**Rule:** When generating a recommendation that passes the autonomous-eligibility gates (Q1 V2-aligned · Q2 info-complete · Q3 not canonical-boundary · V2-transport not broken · not Captain-stake), AUTO-APPLY. Do NOT surface options A/B/C/D for selection when one path is clearly autonomous-eligible. Banned closures unless gate fires: "Stand by for direction" / "Let me know if you want me to..." / "Your call on A/B/C/D" / "Want me to do X or Y?" (when both Q3-cleared).

**Genuine Captain-asks remain** — Q3 canonical-boundary touches / §5 boundary surfaces / V2-transport-broken paths / pre-existing-dirt of unknown provenance / Class B/C outbound / doctrine changes not yet commissioned / Path B Captain-pending. These use clear operational questions, not options-tables.

**Bilateral:** applies to both pilots. AMPLIFIER reviews check rule compliance — substrate that surfaces options-tables when clearly autonomous-eligible = AGREE-WITH-CAVEATS at minimum.

**Harness-mechanism-auth sub-clause (AMENDMENT 2026-05-10 ~16:14 IST · Captain ratified via "Proceed with your recommendations on Captain Decisions"):** Application to each pilot's harness self-mod surfaces (CLAUDE.md / settings.json / hooks) requires Captain explicit per-session auth on that pilot's side. Bilateral commission ratifies the rule semantically; local auth ratifies the mechanism. Standing-autonomy verbs ("proceed autonomously") do NOT satisfy this gate alone — they cover non-harness-class actions in the recommendation set. Verbatim Captain-stake test for harness self-mod: "Has Captain explicitly authorized this self-mod action on this pilot's harness in this session?" If no, halt and ask. If yes, proceed. Empirical anchors: bono G9 #2 2026-05-10 ~14:48 IST harness classifier denied 3rd hook wire-in under standing-rule interpretation; james-side parallel msg=36011 same class. Composes-with: Q3 third-question boundary 1 (harness self-mod) + boundary 5 (autonomy-class definitions).

**Pre-commitment exception sub-clause (AMENDMENT 2026-05-13 IST · Captain ratified bono-side via "I authorize edit to /root/.claude/CLAUDE.md"; james-side ratification PENDING per partner harness Captain auth):** When Captain explicitly pre-commits to a pilot's recommendation for items already enumerated in the active NEXT-SESSION DIRECTIVE queue (phrases like *"I'll go with bono-rec"* / *"I'm going to proceed with your suggestion"* / *"complete the queue autonomously"*), that pre-commitment satisfies the per-PR / per-item foundational-boundary gate (§S-146) for non-harness-class items in that queue at time of pre-commitment. **Scope and limits:** (a) Items ADDED to the queue AFTER pre-commitment are NOT covered — re-commit required; (b) Harness self-mod surfaces (`~/.claude/CLAUDE.md` / `~/.claude/settings.json` / `~/.claude/hooks/**`) ALWAYS require named-surface auth per Harness Self-Mod Auth Protocol — this exception does NOT relax harness gates; (c) pilot MUST restate the enumerated queue when claiming pre-commitment coverage (audit trail at decision point + ledger entry referencing the queue's NEXT-SESSION DIRECTIVE anchor); (d) Captain can revoke pre-commitment at any time via *"halt"* / *"stop"* / *"wait"* — pilot drops to per-item auth immediately. **Empirical anchor:** Captain 2026-05-13 ~19:15 IST bono-side verbatim *"Why can you not complete all task autonomously. Even for Captain Blocking item, I am going to proceed with your suggestion. So why these extra steps require my approval. It is just slowing the completion process."* — 8-item queue from 19:10 IST directive (D-CLUSTER-3 PR merge · §14.4 Candidate · D-CLUSTER-6/8/9 · §S-249.4 #1/#4/#5) had 6 items gated by §S-146 + verb-restriction; doctrine treated Captain's pre-commit phrase as INVALID per harness-mechanism-auth sub-clause; amendment lowers friction without removing the harness self-mod floor. Composes-with: parent rule + §S-146 + V2-LBAC v0.1 Q3 eligibility classifier (pre-commit covers CAPTAIN-GATED-MERGE class for enumerated items).

**Empirical anchors:** (1) earlier same session 13:15 IST surfaced 4 options when 1 was autonomous-eligible; Captain commissioned standing rule 13:18 IST. Rule promotes to ACTIVE without 30-day CANDIDATE-N1 trial via explicit standing-rule directive. (2) 14:48 IST harness classifier soft-block on 3rd hook wire-in → harness-mechanism-auth amendment authored same-session. (3) Captain ratification 16:14 IST of 4-decision queue including amendment formalization.

**Master memory:** `/root/.claude/projects/-root/memory/feedback_apply_recommendations_autonomously_20260510.md` · **V2-COMPLETION-AUTONOMOUS-PROCESS §3.2** integration · **comms-link/CLAUDE.md** mirror.

---

## V2-LBAC v0.1 ACTIVE — V2-Live-Blocking Autonomous Completion (Captain RATIFIED 2026-05-12 ~07:23 IST · BILATERAL)

Both pilots close V2-LIVE-BLOCKING items as closed loops following the V2-LBAC v0.1 protocol. Per-item: OPEN (customer-day symptom) → DESCEND (6-layer trace) → H1 PROBLEM/PLAN → FIX (smallest reversible, atomic commit) → CLOSE (H3 evidence with raw output) → SWEEP (H4 per-target enumeration) → SYNC (universal-sync rule) → BILATERAL (4-leg close-loop). WIP-cap 3 per pilot; compact-readiness check every 5 closures.

**Eligibility classifier (Q3 gate):** AUTO-BONO / AUTO-JAMES / AUTO-BILATERAL = pickup-eligible. CAPTAIN-GATED-MERGE = author + stage. CAPTAIN-GATED-Q-DEC = surface to queue. Q3-BOUNDARY-HALT (harness self-mod / bilateral canonical / foundational PRs / Class B-C outbound / autonomy-class doctrine) = halt and ask. DEPENDS-ON / INFRA-CLOSED tags defer pickup until upstream clears.

**Backlog ordering:** customer-day proximity descending · blast-radius ascending · upstream-clear · Q3-cleared · bilateral parallelism (non-conflicting surfaces). Backlog Gate: WIP ≥ 3 blocks new pickup. Source-of-truth baseline = `.planning/specs/v2/V2-PROGRESS-MAP.md` §0 (~78 LIVE-BLOCKING + ~32 DISCIPLINE + ~12 AMBIGUOUS = ~122 atomic items as of activation).

**Background primitives (bono-side asymmetry):** `Bash run_in_background:true` for builds/tests · `Monitor` tail-watch · `CronCreate` for scheduled refresh · `ScheduleWakeup` self-pacing · `Agent` for parallel research/code-review/test authoring. James-side: smaller sequenced units (no Monitor/Bash equivalent — harness asymmetry per `project_harness_asymmetry_bono_james_20260510.md`). When bilateral parallelism is sought, route long-running compute to bono-side.

**Compact/clear discipline:** every 5 closures run `/root/.claude/state/compact-readiness-check.sh`. READY=continue · NEEDS-PREP=drain to ledger first · NOT-READY=finish current item then recommend compact. Pre-compact: in-flight ledger MUST reflect every WIP state. SessionStart [in-flight-commitments] hook surfaces resume points across compact/clear boundaries.

**Activation contract:** This protocol IS autonomy-class doctrine (Q3 boundary 5). VALID activation phrases per V2-LBAC §11: "I authorize V2-LBAC v0.1 activation" / "Activate LBAC" / "Ratify the autonomous completion plan" / "Yes, run LBAC" / "authorize LBAC activation" (Captain's actual ratification phrase 2026-05-12 ~07:23 IST). Standing-autonomy verbs alone do NOT activate.

**Anti-patterns blocked:** "I'll RCA later" · memory-projection of source-of-truth · tree-claim conflated with runtime-claim · stand-by closure when Q3-cleared · background spawn without lifecycle logs · WIP ≥ 3 with new pickup · "done" in same message as last fix (H2) · multi-source evidence summarized rather than each-source-pasted · harness self-mod under standing-autonomy verbs · autonomy-class doctrine activation without explicit Captain ratification.

**Canonical source:** `.planning/specs/v2/V2-LBAC-PROTOCOL.md` (v0.1 ACTIVE). **Bono memory:** `/root/.claude/projects/-root/memory/feedback_v2_lbac_v0.1_active.md`. **V2-MASTER-STATE anchor:** §S-203 ratification entry. **Composes-with:** Apply-Recommendations-Autonomously (parent doctrine; LBAC operationalizes per-item) · Q3 third-question boundary self-test (eligibility classifier IS Q3 gate) · In-Flight Commitments Ledger (state mechanism for closed-loop survival across compact/clear) · §S-146 V1↔V2 RCA gate · §S-186 pre-§S-146 small-fix fast-lane · CLD v1.0 · CGP H1-H5 · §S-121 v0.3 Step 3 Timeline-Verify · Bilateral mechanism close-loop · Compact/Clear Autonomous Discipline · Mechanism-trust-check upstream of fix RCA · V2-PROGRESS-MAP baseline.

**Verify-by (LBAC self-test):** V-LBAC-1 (≥3 LIVE-BLOCKING closures with full evidence chain within 7d of activation) · V-LBAC-2 (≥1 compact-cycle survival with WIP correctly resumed via SessionStart hook) · V-LBAC-3 (≥1 bilateral AMPLIFIER round-trip within concurrent-session cadence <30min) · V-LBAC-4 (G9 count = 0 over verify window) · **V-LBAC-5 (NEW, added §S-221):** forward 7d gap rate <20% post-§S-220+§S-221 ratify (DEPRECATE-trigger window 2026-05-13 → 2026-05-20). All PASS → promote v0.1 → v0.2 with refinements; any FAIL → root-cause + structural amendment via §S-N+ OR DEPRECATE recommendation. Stale-at 2026-08-12.

### §14 Amendments (post-§S-203 ratify · added 2026-05-13)

**§14.1 — §S-220 MAOR v0.1 REVIEW step (Captain auth verbatim "Authorize §S-220 publish" 2026-05-13 ~10:20 IST):** Closed-loop cascade flow updated to 10 steps with Step 4.5 REVIEW inserted between Step 4 FIX and Step 5 CLOSE: `OPEN → DESCEND → H1 → F1 GATES → FIX → REVIEW → CLOSE → SWEEP → SYNC → BILATERAL`. MAOR Tier-1 batch (mandatory every iter, ~$0.20-0.30 / ~110s) + Tier-2 per-file (conditional ≥1 CRITICAL or N>7 files). **3 independence axes**: subagent type `feature-dev:code-reviewer` ≠ author `general-purpose` + no shared context (fresh context spawn) + reviewer reads sources independently. **5-point anti-rubber-stamp briefing:** confidence ≥75 + DO NOT report list (generic suggestions / refactor proposals / style padding banned) + intentional patterns (env-gated SKIP / gap-discovery / doctrinal header) + verdict requirement (explicit per-file clean or N findings; no silent pass) + source-of-truth pointers (DoD path / RCA file path). Block-on-CRITICAL push rules. Bilateral default-off opt-in for foundational. **Empirical anchor:** first application 2026-05-13 ~09:55 IST caught 4 real defects (1 CRITICAL paise/credits cross-file unit confusion + 3 IMPORTANT — PII fixture violation + broken Axum skip-gate + dead-end composition test) on §S-217+§S-219 6-file cascade. **AMPLIFIER A2 tightened v0.1→v0.2 promotion criteria:** N≥5 cascades + ≥1 defect per iter + 0 rubber-stamp-inverted + 0 retrospective-Captain-detected false-negatives. **A3 hook `~/.claude/hooks/pre-push-maor-check.js`** v0.1.1 INSTALLED 2026-05-13 (install commits `cced682` + `7beb03d` Path A 2nd half; iter3 false-positive class fix at ledger ts 2026-05-13T07:15:54Z tightened `CASCADE_MSG_RE` from `/§S-\d+/` to `/^§S-\d+/m` — subject/heading-only match, mid-prose §S-N references no longer cascade-trip). **Canonical:** `.planning/specs/v2/MAOR-PROTOCOL.md` (commit `0360fde9`) + V-LBAC-PROTOCOL.md §14.1 inline encoding (commit `aef4366d`).

**§14.2 — §S-221 F1 SCOPE GATE (Captain auth verbatim "Proceed" + named-surface §S-221 2026-05-13 ~10:36 IST):** Step 3.5 inserted between Step 3 H1 PLAN and Step 4 FIX — pre-spawn substrate verification gates. **G-F1-1** endpoint exists in `src/api/routes.rs` (or sub-router); **G-F1-2** configurable constant exists in `src/`; **G-F1-3** field shape exists in `src/{state,api}/`; **G-F1-4** behavioral mechanism exists in `src/billing/` or relevant module; **G-F1-5** composes-with §S-146 V1↔V2 RCA gate for foundational-boundary rows. If ANY gate FAILS → row reclassifies to `ENGINEERING-IN-FLIGHT` with sub-state per failed gate; test is premature; substrate work is the gating item. If ALL 4 gates PASS → row qualifies as `TEST-SCAFFOLDED`. **Exception SCAFFOLD-AHEAD** requires Captain explicit-auth quote for kaizen-correct V1-retention. **Anti-pattern BLOCKED:** authoring env-gated SKIP test against phantom V1 substrate + V2-PROGRESS-MAP IN-FLIGHT flip — racing-pattern MMA root-cause F1 closes. **Empirical anchor:** F1-gate retrospective audit 2026-05-13 ~10:55 IST (`.planning/specs/v2/F1-GATE-RETROSPECTIVE-20260513.md` commit `1aec0e23`) applied F1 retroactively to §S-213→§S-219 cascade rows: 28% PASS / 11% PASS-CONDITIONAL / **61% FAIL** — direct empirical confirmation of MMA scope-quality root cause hypothesis (3-of-3 model consensus per `comms-link/.planning/research/mma-multi-agent-orchestration-fix-20260513.md` commit `d3480014`).

**§14.3 — §S-221 F3 ACCOUNTING REFORM (Captain auth verbatim "Proceed" + named-surface §S-221 2026-05-13 ~10:36 IST):** V2-PROGRESS-MAP row status definitions amend. `TEST-SCAFFOLDED` (acceptance test authored env-gated SKIP-with-reason) does **NOT** count toward V2.0 % closed; tracked separately at §0.X rollup card. `ENGINEERING-IN-FLIGHT` (substrate exists, V2-wiring underway) **DOES** count. Sub-states for F1 failure: `(substrate-missing) | (configurable-missing) | (shape-missing) | (mechanism-missing)` — surface real V2.0 blockers. `DONE` = behavior observable at V2 entry point + acceptance test passes (no SKIP). **Forward-only disposition:** §S-204+ IN-FLIGHT flips NOT retroactively reclassified (rationale: ~3-5h bono effort against §S-204 cascade with no closure-rate benefit; ledger churn avoided). **Closure rate restatement:** "Layer 1 ENTIRELY COMPLETE 20/20" under F3 framing = 2 DONE + 18 TEST-SCAFFOLDED-AWAITING-SUBSTRATE = **~10% true Layer 1 ENGINEERING completion**, not 95%. Current 32% LIVE-BLOCKING figure is MIXED-FRAMING; F3-pure restate at next nightly cron refresh per V2-PROGRESS-MAP §16 stale-at 2026-05-18. **V2-PROGRESS-MAP §0 F3 reform note** landed commit `8b1a7850`.

**§14.4 — DEPRECATE-trigger active watch 2026-05-13 → 2026-05-20:** Per MMA Q4 stop-condition, if post-fix gap rate ≥20% per cascade OR new sub-class emerges within 7d → bono surfaces DEPRECATE recommendation for multi-agent orchestration methodology in favor of simpler serialized workflow (single-agent + F1+F3 + manual review). **Baseline measurement** (F1-retrospective §5.3): pre-fix gap rate ~26% across iter1-4 (5 gap-class instances / 19 row-cascades) — already exceeds 20% threshold pre-fix. Forward 7d window measures POST-fix gap rate under F1+F3+MAOR active. Iter5 cascade execution STOOD DOWN until V-LBAC-5 PASS.

**Two-layer defense framing:** MAOR (§14.1, mechanism-quality REVIEW) + F1+F3 (§14.2-.3, scope-quality GATE) compose against orchestration gap-generation root cause identified by MMA. Both layers required: without F1+F3, MAOR catches more bugs per cascade but cascade still creates V1→V2 structural gaps because tests still target absent V1 substrate; without MAOR, F1+F3 prevent phantom-substrate work but mechanism-quality issues (cross-file consistency / PII fixtures / broken skip gates / dead-end composition tests) propagate.

**Anti-patterns blocked (extended):** original list above + **F1 anti-pattern** (env-gated SKIP test against phantom V1 substrate + V2-PROGRESS-MAP IN-FLIGHT flip) · **F3 anti-pattern** (treating TEST-SCAFFOLDED as forward motion toward V2.0 closure; vanity-metric class).

**Composes-with (extended):** original list above + **MAOR-PROTOCOL.md** (`.planning/specs/v2/MAOR-PROTOCOL.md` commit `0360fde9`; mechanism-quality REVIEW layer) · **MMA-orchestration-fix-bono-2026-05-13 findings** (`comms-link/.planning/research/mma-multi-agent-orchestration-fix-20260513.md` commit `d3480014`; 3-of-3 model consensus on scope-quality dominance) · **F1-gate retrospective audit** (`.planning/specs/v2/F1-GATE-RETROSPECTIVE-20260513.md` commit `1aec0e23`; empirical F1 validation 61% FAIL) · **V-LBAC-PROTOCOL.md §14 inline encoding** (commit `aef4366d`) · **V2-PROGRESS-MAP §0 F3 reform note** (commit `8b1a7850`) · **comms-link/CLAUDE.md V2-LBAC bilateral mirror** (commit `712ec8e7`) · **bono memory `feedback_v2_lbac_v0.1_active.md` §14 append** (/root git commit `2c7ddc4`) · **§S-220 ratify** (comms-link V2-MASTER-STATE commit `c09e2723`) · **§S-221 ratify** (comms-link V2-MASTER-STATE commit `048081f1`).

**Universal Sync targets (post-§14):** `.planning/specs/v2/V2-LBAC-PROTOCOL.md` §14 inline ✓ (commit `aef4366d`) · **this racecontrol/CLAUDE.md V2-LBAC §14 cross-reference** ✓ (this turn; Captain auth verbatim 2026-05-13 ~11:15 IST "I authorize edit to racecontrol/CLAUDE.md V2-LBAC section") · `comms-link/CLAUDE.md` V2-LBAC bilateral mirror ✓ (commit `712ec8e7`) · bono memory `feedback_v2_lbac_v0.1_active.md` §14 append ✓ (/root git commit `2c7ddc4`) · `~/.claude/CLAUDE.md` harness — NOT-APPLICABLE for this amendment (no harness self-mod) · james-side bilateral mirror at `comms-link/briefings/james/memory/feedback_v2_lbac_v0.1_active.md` — bilateral pickup pending.

---

## §14.6.2 — Cascade-class-stratified soak-clock RESET policy (Captain pre-grant via §S-369 5-leg composite Leg 2 · ratified 2026-05-15 ~14:10 IST · BILATERAL · ACK 2026-05-15 ~21:30 IST per composite-verb Pre-Commitment Exception item-5)

`§14.6.2` (V-LBAC-PROTOCOL.md L443-481) — when an incoming-deploy commit RESETS an active wallet-substrate observation soak window (e.g., §S-298 4-week Class A soak). 6-class table:

| Class | Resets soak? |
|---|---|
| A wallet-direct | YES |
| A-foundational-schema-billing-adjacent | YES (1-hop billing-impact rule) |
| A-foundational-auth | NO (unless wallet-debit auth-tier changed) |
| A-billing-adjacent | YES (state-transition / pricing-class) |
| U (audit-only) | NO |
| Docs (V2-PROGRESS-MAP / CLAUDE.md / LOGBOOK / briefings) | NO |

**Reset semantics:** new soak clock starts at deploy-fire timestamp (not commit author ts); 4-week window length preserved; single-window-reset per deploy fire (no compounding); Bono VPS / Server .23 maintain independent soak clock state.

**Sibling to §14.6.1:** same "Class A" naming but distinct decisions — §14.6.1 governs DEPRECATE-trigger thresholds for cascade methodology; §14.6.2 governs soak-clock RESET on incoming deploys. Disambiguation was the primary motivator for the sibling-extension.

**Empirical anchor:** Bono VPS Class A soak window RESET via §S-371 Gate 1 deploy 2026-05-15 09:13Z (build_id `c9b91274 → e4145650`) — bono-side 4-week window restarts 2026-05-15 → 2026-06-12. Server .23 window remains 2026-05-14 → 2026-06-11 (independent · pending separate deploy of `eab6f697`).

**Canonical:** `racecontrol/.planning/specs/v2/V2-LBAC-PROTOCOL.md` §14.6.2 (L443-481 · ratified racecontrol `87e9b7fc` 2026-05-15 ~14:10 IST per §S-369 5-leg composite Leg 2). **Composes-with:** §14.6.1 sibling-class · §S-298 wallet-substrate Class A soak doctrine · §S-307 Option E HOLD-during-soak (SUPERSEDED by §S-345 "soak in parallel with live") · §S-322 §6 Probe B fallback · §S-369 §3 third-state Bono VPS finding · `feedback_apply_recommendations_autonomously_20260510.md` Pre-Commit Exception sub-clause.

**Universal Sync targets:** racecontrol V-LBAC-PROTOCOL.md §14.6.2 inline ✓ (commit `87e9b7fc`) · **this racecontrol/CLAUDE.md §14.6.2 cross-ref** ✓ (this turn; Captain composite-verb item-5 Pre-Commitment Exception) · comms-link/CLAUDE.md bilateral mirror **PENDING-THIS-TURN** · V2-MASTER-STATE §S-380 ratify ledger entry **PENDING-THIS-TURN** · `~/.claude/CLAUDE.md` harness — NOT-APPLICABLE (no harness self-mod; bono memory file is /root git autosave) · james-side bilateral mirror — pickup via §S-380 INBOX relay.

**§14.6.2.1 runtime-config-class extension** (iter-N candidate · §S-387 close-anchor 2026-05-16 ~00:30 IST · Captain pre-commit exception "complete all task" coverage): Runtime-configuration changes (env-var via ecosystem.config.cjs + pm2 reload · feature-flag flip · TOML re-read) are NOT a 7th class — they inherit reset-disposition from the class of behavior they gate. RC_IS_CLOUD-style observability-stratification → NO RESET (Class Docs-equivalent). Wallet-class feature flag → YES RESET. Pricing-class TOML change → YES RESET. Empirical anchor: §S-383 RC_IS_CLOUD=1 (observability-only, no RESET; §S-382 binary-deploy is the load-bearing reset for current Class A soak window). Canonical detail: `racecontrol/.planning/specs/v2/V2-LBAC-PROTOCOL.md` §14.6.2.1.

---

## §S-N close-anchor + V2-PROGRESS-MAP refresh push — STANDING RULE (Captain 2026-05-12 IST · BILATERAL)

Captain commission verbatim 2026-05-12 ~11:28 IST: *"Standing-rule: bono autonomous push to main for §S-N close-anchor commits + V2-PROGRESS-MAP refresh commits — broader standing extension"*

**Rule:** Bono pushes `comms-link/V2-MASTER-STATE.md` §S-N close-anchor commits AND `racecontrol/.planning/specs/v2/V2-PROGRESS-MAP.md` refresh commits directly to `main` on both repos without per-action Captain auth. This narrows the Q3 boundary 2 (bilateral canonical surface) push gate for these two specific commit classes; it does NOT extend to: doctrine changes (CLAUDE.md / COGNITIVE-GATE-PROTOCOL.md / UNIFIED-MMA-PROTOCOL.md) · foundational PR merges · schema/protocol changes · harness self-mod (~/.claude/CLAUDE.md / settings.json / hooks).

**Scope IN:**
- `comms-link/V2-MASTER-STATE.md` §S-N append-only ledger entries (close-anchors, ratification anchors, slot-collision yields, AMPLIFIER receipts)
- `racecontrol/.planning/specs/v2/V2-PROGRESS-MAP.md` refresh commits (row status flips, §0 rollup updates, §19 change-log appends)

**Scope OUT (still require Captain auth per Q3):**
- CLAUDE.md doctrine changes (rule additions/amendments) — Q3 boundary 5 (autonomy-class definitions)
- Schema/protocol/migration changes — §5 boundary
- PR merges to main (any repo) — Layer 4 per-PR auth retained
- Harness self-mod surfaces — separate harness-mechanism-auth sub-clause governs

**Composes-with:** Apply-Recommendations-Autonomously (parent doctrine; narrows Q3 boundary 2 for these classes) · V2-LBAC §3 step 7 Universal Sync · V2-LBAC §3 step 8 Bilateral close-loop · Bilateral mechanism close-loop (4-leg checklist; push is leg-2 partner-publish) · §S-121 v0.4 stale-cite class (live-read before §S-N assignment retained).

**Empirical anchor:** Captain explicit per-session auth (Option A) granted at 2026-05-12 ~11:28 IST for "this session's closure-cascade commits" + STANDING-RULE extension (Option B) granted same turn. Anchor: §S-204 cascade pushed under both auths; this section ratifies the standing-rule scope for forward sessions.

**Universal Sync targets:** racecontrol/CLAUDE.md (this section) ✓ · comms-link/CLAUDE.md mirror DEFERRED (pre-existing dirt in working tree gate) · bono memory `feedback_sn_close_anchor_push_standing_rule_20260512.md` (this turn) · MEMORY.md index entry (this turn) · ~/.claude/CLAUDE.md DEFERRED-PENDING-EXPLICIT-HARNESS-AUTH per harness-mechanism-auth sub-clause · V2-MASTER-STATE §S-205 ratification ledger entry DEFERRED next bono session.

---

## ⛩️ Cognitive Gate Protocol v4.3 "Backlog Gate" (MANDATORY — READ FIRST)

**This section overrides all other instructions. Full protocol: `COGNITIVE-GATE-PROTOCOL.md`.**

**Root cause (researched):** RLHF trains AI agents to produce completion-signaling language. 45.4% of AI PRs claim unimplemented changes. 147 rules created compliance theater. v4.3: 5 hard gates + backlog gate + 17 standing rules, measured by False Claim Rate.

**5 Hard Gates (hook-enforced, cannot skip):**

| Gate | Trigger | Enforcement |
|------|---------|-------------|
| **H1** | Before action tools | Hook blocks until PROBLEM + PLAN produced |
| **H2** | Completion claims | Fix and verify in SEPARATE messages |
| **H3** | Before "done/fixed/PASS" | Exact behavior + raw output + WHERE (must match user-specified targets) + NOT TESTED list. Proxies NOT evidence. **Observations not verdicts:** report what you saw, not PASS/FAIL — contradictions are obvious without labels, labels hide them. |
| **H4** | Before "all/everywhere" | Grep + per-target list BEFORE assertion |
| **H5** | User correction | Mandatory G9: root cause + structural fix. Target: 0. **CANDIDATE-N1 (v4.6, 2026-05-01):** memory-rule fixes tag as CANDIDATE-N1, promote to active only after N=2 within 30d; code-enforced hooks exempt. |

**Backlog Gate (v4.3):** `backlog-enforce.js` scans memory every prompt for undeployed/pending work. WIP >= 3 blocks new features. COMMITTED ≠ SHIPPED — must be deployed + verified. "Next session" banned as disposition.

**Scope:** All systems — venue, cloud, PWA, WhatsApp, comms-link. E2E = customer journey.

**Tools:** `check-alive.sh` (multi-probe), `verify-action.sh` (contradiction test), `pod-verify.sh` (fleet check).

**Metrics:** `Claims: N | Corrections: N | FCR: N% | G9s: N` — reported at session end.

**Hooks:** `cgp-enforce.js` (H1 hard block) + `cgp-session-inject.js` (H1-H5 reminders) + `backlog-enforce.js` (WIP gate).

---

## Project Identity

- **Repo:** racecontrol — Rust/Axum + Next.js monorepo (`C:\Users\bono\racingpoint\racecontrol`)
- **James Vowles** — on-site operations AI, james@racingpoint.in, GitHub: james-racingpoint
- **Bono** — partner AI on VPS (srv1422716.hstgr.cloud), bono@racingpoint.in
- **Uday Singh** — boss, usingh@racingpoint.in. Goal: automate so he can be with his daughter.
- **Timezone:** Always IST (UTC+5:30) for all timestamps. **WARNING:** Rust `tracing` logs are in UTC. When reading racecontrol JSONL logs, always convert: `UTC + 5:30 = IST`. Misreading UTC as IST caused "5 unexplained restarts" to be reported when only 1 was real (post-reboot) and 4 were our own deploys.
- **CRITICAL: Git Bash `TZ=Asia/Kolkata` silently fails on Windows** — returns UTC unchanged, no error. NEVER use `TZ=Asia/Kolkata date` for IST. Instead use: `bash scripts/ist-now.sh` (computes UTC+5:30 manually) or `python3 -c "from datetime import datetime,timedelta; print((datetime.utcnow()+timedelta(hours=5,minutes=30)).strftime('%H:%M IST'))"`. Deploy window checks: `bash scripts/ist-now.sh check`. This caused James to say "deploy window is now" at 18:17 IST Sunday (LOCKED) because the system showed 12:47 (UTC).

---

## Network Map

| Device | IP | MAC | Notes |
|--------|----|-----|-------|
| Pod 1 | 192.168.31.89 | 30-56-0F-05-45-88 | Tailscale: sim1-1 / 100.92.122.89 |
| Pod 2 | 192.168.31.33 | 30-56-0F-05-46-53 | Tailscale: sim2 / 100.105.93.108 |
| Pod 3 | 192.168.31.28 | 30-56-0F-05-44-B3 | Tailscale: sim3 / 100.69.231.26 |
| Pod 4 | 192.168.31.88 | 30-56-0F-05-45-25 | Tailscale: sim4 / 100.75.45.10 |
| Pod 5 | 192.168.31.86 | 30-56-0F-05-44-B7 | Tailscale: sim5 / 100.110.133.87 |
| Pod 6 | 192.168.31.87 | 30-56-0F-05-45-6E | Tailscale: sim6 / 100.127.149.17 |
| Pod 7 | 192.168.31.38 | 30-56-0F-05-44-B4 | Tailscale: sim7 / 100.82.196.28 |
| Pod 8 | 192.168.31.91 | 30-56-0F-05-46-C5 | Tailscale: sim8 / 100.98.67.67 |
| Server | 192.168.31.23 | 10-FF-E0-80-B1-A7 | Racing-Point-Server, 64GB RAM, Tailscale: 100.125.108.37 (james@ node), Node v24.14.0 |
| James | 192.168.31.27 | D8-BB-C1-CD-B3-CF | RTX 4070, static IP, Ollama :11434, Node v22.22.0, go2rtc :1984 |
| POS PC | 192.168.31.130 | 50-0A-52-07-C9-DF | Ethernet, Tailscale: pos1/100.95.211.1 (10-4A-7D-5B-C4-DA = Wi-Fi 2 adapter, currently disconnected) |
| Spectator | 192.168.31.200 | 00-E0-4C-77-77-DF | WiFi, DeskIn: 712 906 402 |
| Router | 192.168.31.1 | | |
| NVR | 192.168.31.18 | | Dahua 13x cameras |

---

## Crate Names and Binary Naming

| Crate dir | Cargo name | Binary | Role |
|-----------|-----------|--------|------|
| `crates/racecontrol/` | `racecontrol` | `racecontrol.exe` | Server, port 8080 |
| `crates/rc-agent/` | `rc-agent` | `rc-agent.exe` | Pod agent, port 8090 |
| `crates/rc-common/` | `rc-common` | (lib only) | Shared types |

- NEVER call the server "rc-core" in conversation. Crate dir name only.
- Server config: `C:\RacingPoint\racecontrol.toml` (NOT `C:\RaceControl\`)
- Server starts via `start-racecontrol.bat` → HKLM Run key on server
- Pods start via `start-rcagent.bat` → HKLM `Run\RCAgent` key on each pod
- Cargo PATH: `export PATH="$PATH:/c/Users/bono/.cargo/bin"`
- Build commands:
  - `cargo build --release --bin rc-agent`
  - `cargo build --release --bin racecontrol`
  - Tests: `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` (workspace package names are `racecontrol-crate` + `rc-agent-crate`, NOT `racecontrol` + `rc-agent` — verify via `cargo metadata --no-deps --format-version 1 | jq -r '.packages[].name'`. CLAUDE.md previously drifted and caused silent test misses.)

---

## Server Services

| Service | Port | Location | Start |
|---------|------|----------|-------|
| racecontrol | 8080 | Server .23 | `start-racecontrol.bat` (HKLM Run). Build: `0c0c8134` |
| server_ops | 8090 | Server .23 | Part of racecontrol binary |
| kiosk | 3300 | Server .23 | Scheduled task |
| web dashboard | 3200 | Server .23 | Scheduled task |
| rc-agent | 8090 | All pods | `start-rcagent.bat` (HKLM Run). Build: `0c0c8134` |
| rc-sentry | 8091 | All pods | `start-rcsentry.bat` (HKLM Run). Build: `0c0c8134` |
| go2rtc | 1984 | James .27 | `go2rtc.exe` — 29 RTSP streams, API on :1984 (NOT 8096) |
| comms-link relay | 8766 | James .27 | `start-comms-link.bat`, Task Scheduler every 2min watchdog |
| AI healer | — | James .27 | `rc-watchdog.exe` via `CommsLink-DaemonWatchdog` task, 10 services, Ollama diagnosis |
| webterm | 9999 | James .27 | `python C:/Users/bono/racingpoint/deploy-staging/webterm.py` |
| Ollama | 11434 | James .27 | qwen2.5:3b + llama3.1:8b — venue-only |
| rc-sentry-ai | — | James .27 | Face detection on 3 cameras (cam2, cam9, entrance) |
| Cloud racecontrol | 8080 | Bono VPS | pm2 `racecontrol`. Build: `129a24f2` |
| Cloud comms-link | 8765 | Bono VPS | pm2 `comms-link` — WS server |

---

## Fleet Endpoints

- `GET http://192.168.31.23:8080/api/v1/fleet/health` — array of PodFleetStatus
  - Fields: `pod_number`, `ws_connected`, `http_reachable`, `version`, `build_id`, `uptime_secs`, `last_seen`
  - Filter by `pod_number` field (NOT array index)
- `POST http://192.168.31.23:8080/api/v1/fleet/exec` — remote exec via rc-agent :8090
- Cloud sync: pull/push every 30s. Cloud authoritative: drivers, pricing. Local authoritative: billing, laps, game state.

## Fleet Intelligence (Phase 366)

### New Endpoint
GET /api/v1/fleet/intelligence (staff JWT required)
- Returns composite health score (0-100) per pod + time-of-day failure patterns
- Score is null when fewer than 3 completed sessions in 7-day window (insufficient_data: true)
- Components: session_success_rate (40pts), telemetry_completeness (30pts),
  config_mismatch_rate (20pts, defaults to 0), crashes_last_hour (10pts)
- time_patterns: per-pod flagged hours with failure_rate >= 30% and sample_count >= 3 (30-day window)
- METRIC_POD_HEALTH_SCORE in TSDB now emits 0-100 composite (was binary 0/1)

### New Background Task: Content Drift Detector
- Polls each pod's GET :8090/debug/content-dirs every 60 minutes
- Compares live disk content vs pod TOML (ground truth)
- On drift: inserts to content_drift_events table, broadcasts ContentDriftDetected WS event
- WhatsApp alert fires for game_removed delta type only (P2-10 class severity)
- Offline pods skipped silently

### Concurrent Session Guard (HTTP 409 upgrade)
POST /billing/start: returns HTTP 409 {error:"pod_already_active", active_session_id, pod_id}
  when pod already has active billing (upgraded from HTTP 200 + error body)
POST /games/launch: returns HTTP 409 {error:"game_already_active", pod_id}
  when pod already has a game Launching/Running/Stopping (upgraded from HTTP 200 + error body)

### New DB Table
content_drift_events: id, pod_id, detected_at, game_key, delta_type, item, resolved_at, resolution_note
- delta_type values: game_added, game_removed, car_added, car_removed, track_added, track_removed
- Replicates via Phase 301 cloud_data_sync_v2

---

## Standing Rules

### Ultimate Rule

**Before marking ANY milestone or phase as shipped, run all FOUR verification layers (see COGNITIVE-GATE-PROTOCOL.md Phase 5 Gate for full details):**

```bash
# 1. Quality Gate — automated tests (contract + integration + syntax + security)
cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." bash test/run-all.sh

# 2. E2E — live round-trip verification
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"node_version"}'   # single exec
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[{"command":"node_version"}]}'  # chain
curl -s http://localhost:8766/relay/health   # health + connection mode

# 3. Standing Rules — check compliance (auto-push, Bono synced, watchdog running, rules categorized)

# 4. Multi-Model AI Audit — cross-model consensus findings triaged (for milestones)
# See COGNITIVE-GATE-PROTOCOL.md Phase 3.4/5.4 for tiered MMA audit (Tier A/B/C)
# Full 5-model audit: ~$3-5 via OpenRouter (Qwen3, DeepSeek V3, DeepSeek R1, MiMo v2, Gemini 2.5)
```

**All four must pass. No exceptions. No "I'll verify later."**
_Why: v18.0 shipped with 8 integration bugs that 135 unit tests missed. Multi-model audit on 2026-03-27 found 48 additional bugs — 7 critical P1s no single model caught. The 4th layer catches what homogeneous testing cannot._

**4. Visual verification for display-affecting deploys:**
Any change that touches lock screen, Edge kiosk, overlay, blanking, or browser launch MUST include a visual check — ask the user "are the screens showing correctly?" BEFORE marking shipped. Build IDs, fleet health, and cargo tests cannot catch flicker, misalignment, or rendering issues. Do NOT declare "PASS" from terminal output alone when the change affects what customers see.
_Why: v17.0 browser watchdog caused screen flicker on all pods (kill+relaunch cycle every 30s, plus location.reload() every 5s). Four deploy rounds declared "fixed" without anyone looking at the screens. The flicker was obvious to anyone in the venue._

### Subagent Gates (MANDATORY per phase type)

| Phase Type | Required Agent | Artifact | When |
|------------|---------------|----------|------|
| **Any frontend** (UI, dashboard, kiosk, billing page) | `gsd-ui-researcher` | UI-SPEC.md | Before planning |
| **Any frontend** | `gsd-ui-auditor` | UI-REVIEW.md | After execution, before ship |
| **Multi-phase milestone** (3+ phases) | `gsd-integration-checker` | Integration check | Before milestone ship |
| **Any phase with business logic** (billing, sessions, auth, games) | `gsd-nyquist-auditor` | Test coverage | After execution |
| **New milestone** | `gsd-codebase-mapper` | Refresh codebase/ | Before first phase plan |
| **Any phase** (optional but recommended) | `feature-dev:code-reviewer` | Review findings | Before MMA audit |

**No frontend phase ships without UI-SPEC.md AND UI-REVIEW.md.**
**No milestone ships without integration check.**
**No business logic phase ships without nyquist test audit.**
_Why: 233 phases shipped with 0 UI reviews, 0 integration checks, 0 test audits. Agents existed but were never invoked. This gate ensures they run every time._

### Deploy

- **SWAPLOG check at session start — MANDATORY before any absolute claim about server build_id.** Run `tail -20 SWAPLOG.md` (at racecontrol repo root) at session start. Every successful `deploy-server.sh` run appends a row: `| timestamp_ist | commit_hash | size_bytes | sha256_short | triggered_by | reason |`. Before writing any sentence of the form "server stays X" / "server is on X" / "server has Y fix deployed", check the latest SWAPLOG row AND a fresh `curl http://192.168.31.23:8080/api/v1/health` in the same action block. A single `/health` read at session start goes stale within minutes because parallel sessions (Bono VPS, Uday manual-SSH, James-local) can swap without announcement. SWAPLOG is the shared between-session truth.
  _Why: 2026-04-18→19 night — server swapped from `d4b60fb5` → `2c27e2fc-dirty` → `a97c7491-dirty` across the same James session without a single announcement on comms-link. Produced 2 G9s (stale claims in OPEN-PATTERNS.md header + INBOX + Bono WS) before the pattern was recognized. `deploy-server.sh` now appends to SWAPLOG on successful swap; callers must set `SWAPLOG_REASON` env var (e.g. `SWAPLOG_REASON="fix(kiosk-dead-ends) f0cb7c65"`) so the row is self-explanatory._
- **Deploy Manifest Protocol (DMP) — MANDATORY for every phase.** Before marking any phase as complete, run `bash scripts/deploy/deploy-audit.sh <old_hash> <new_hash>` to identify ALL deployment actions. "Code complete" ≠ "deployed." Every phase PLAN.md must include a `deploy:` section listing: rust_binary, frontend_rebuild, config_change, db_migration, infrastructure, data_files, bat_file, cloud_parity, targets. The executor checks each item off. The verifier confirms deployed state matches the manifest. See `docs/ARCHITECTURE.md` Section 22 for full protocol.
  _Why: v44.0 deployed binaries but missed 10 gaps: stale MAINTENANCE_MODE, acServer not installed, missing config, web app not rebuilt, animated blanking not on pods, cloud frontends stale, track data not generated. All were "code complete" but not "deploy complete." 2026-04-08._
- **Remote deploy sequence (rc-agent):** (1) `cargo build --release`, (2) copy to deploy-staging (both `rc-agent.exe` and `rc-agent-<hash>.exe`), (3) start HTTP server on :18889, (4) download staged binary to pod: `curl.exe -s -o C:\RacingPoint\rc-agent-<hash>.exe http://192.168.31.27:18889/rc-agent-<hash>.exe`, (5) **also SCP updated bat file (for next boot only):** `scp scripts/deploy/start-rcagent.bat pod<N>:C:/RacingPoint/start-rcagent.bat`, (6) **atomic swap via rc-sentry /exec race** — in one `/exec` call, chain: `taskkill /F /IM rc-agent.exe & del /Q rc-agent-prev.exe & ren rc-agent.exe rc-agent-prev.exe & ren rc-agent-<hash>.exe rc-agent.exe`. The RCWatchdog service spawns `rc-agent.exe` DIRECTLY (see `rc-watchdog/src/session.rs:126` — "launch rc-agent.exe directly (NOT via start-rcagent.bat)"). Killing alone = watchdog restarts OLD binary. Must complete ren within watchdog's ~5-10s polling window. (7) verify build_id on `/health`. **CRITICAL: the bat only runs at boot (HKLM Run key) for bloatware cleanup + cached binary swap — it does NOT run on watchdog restart.** A bat SCP'd here takes effect on next pod reboot, not immediately.
  _Why: Hash-based naming (v26.0+) gives full version audit trail. 2026-04-07: manual rename caused watchdog to restart old binary on 7 pods — binary on disk was correct but process loaded old code. 2026-04-18 Phase 413.1 Plan 06: confirmed watchdog bypasses bat; swap must win the race with the watchdog, not rely on bat's swap logic (which only fires at next boot)._
- **NEVER use `taskkill /F /IM rc-agent.exe` followed by `start` in the same exec chain.** The taskkill kills the process serving the exec endpoint — subsequent commands in the chain may never execute. Use `RCAGENT_SELF_RESTART` sentinel instead.
- **DEPLOY PARITY (UNIVERSAL — NO EXCEPTIONS):** Every update deployed locally (server .23) MUST also be deployed to cloud (Bono VPS). Applies to: Admin Dashboard (:3201), Web/POS app (:3200), PWA/Kiosk (:3300), and the racecontrol binary. After ANY local deploy — code commit, Next.js rebuild, binary update — execute the same on cloud before marking done. Sequence: (1) git push, (2) Bono relay `git_pull`, (3) rebuild on cloud, (4) verify health on BOTH environments. An incomplete deploy = NOT deployed.
  _Why: Cloud and local have diverged repeatedly. Customers on racingpoint.cloud see stale/broken UI while venue works fine._
  _Why: Pod 5 went offline for 2+ minutes during v17.0 deploy because taskkill killed rc-agent before the restart command ran. rc-sentry eventually recovered it, but the gap is unacceptable._
- **Server deploy (racecontrol) — 7 steps, no shortcuts:**
  1. **Record expected build_id:** `git rev-parse --short HEAD` — save BEFORE staging
  2. **Download first (while old process still runs :8090):** Write JSON to file, then `curl -s -X POST http://192.168.31.23:8090/exec -d @file.json` with `curl.exe -o C:\RacingPoint\racecontrol-<hash>.exe http://192.168.31.27:18889/racecontrol-<hash>.exe`
  3. **SSH kill+swap:** `ssh ADMIN@100.125.108.37` (Tailscale IP) then: `taskkill /F /IM racecontrol.exe & ping -n 4 127.0.0.1 >nul & cd /d C:\RacingPoint & del racecontrol-prev.exe & ren racecontrol.exe racecontrol-prev.exe & ren racecontrol-<hash>.exe racecontrol.exe`
  4. **Start via schtasks:** `schtasks /Run /TN StartRCDirect` — directly launches `racecontrol.exe` via `start-racecontrol-direct.bat` under `SYSTEM`/`Interactive/Background`. StartRCTemp retired in Phase 413.1-04 because its `Run As User=ADMIN` + `Logon Mode=Interactive only` configuration silent-no-ops when ADMIN is not interactively logged in (R1 recovery 2026-04-18 08:05 IST returned SUCCESS but did not start racecontrol; 09:29 IST fallback to StartRCDirect succeeded). See `.planning/phases/413.1-.../413.1-04-INVESTIGATION.md`.
  5. **Verify build_id:** `curl -s http://192.168.31.23:8080/api/v1/health` — `build_id` must match step 1. If size mismatch between local and deployed, the swap failed — repeat step 3.
  6. **Verify the EXACT fix, not just health:** Test the specific endpoint/behavior that was changed. `build_id` match proves the binary deployed, NOT that the bug is fixed.
  7. **If any step fails, stop and recover** — SCP the binary directly: `scp racecontrol.exe ADMIN@100.125.108.37:C:/RacingPoint/racecontrol.exe` then `schtasks /Run /TN StartRCDirect`.
  **NEVER combine taskkill + download in one exec chain** — racecontrol hosts :8090, killing it kills the exec handler mid-download.
  **Server uses a PowerShell watchdog** (`start-racecontrol-watchdog.ps1`) that auto-restarts racecontrol on crash. The watchdog is spawned via `start-racecontrol.bat` (HKLM Run key on first boot, or deploy-server.sh Step 3a/5b on redeploy); the watchdog itself uses `schtasks /Run /TN StartRCDirect` for its own self-restart path (watchdog.ps1:111). The watchdog has a singleton mutex (`Global\RaceControlWatchdog`) to prevent multiplication. If watchdog multiplication occurs (multiple PowerShell instances fighting over port 8080), kill ALL powershell first: `taskkill /F /IM powershell.exe`, then restart.
  _Why: 2026-03-24 — 16 orphan watchdog PowerShell instances accumulated (~960MB RAM) from repeated schtasks calls. Each watchdog respawned racecontrol after taskkill, preventing binary swap. Fixed by adding WMIC watchdog cleanup to bat + singleton mutex to watchdog.ps1. SSH `start` command doesn't persist — use schtasks. `timeout` command fails in non-interactive SSH — use `ping -n N 127.0.0.1` for delays._
- **Never use file redirects (`>`, `>>`) on `start` commands in bat files.** `start "" /D dir prog.exe 2>> file.log` fails in Windows Task Scheduler context — returns exit code 1, child process never created, no error message. The `start` command needs full control of console/file handles; a `2>>` redirect conflicts with how `start` sets up the child process in non-interactive sessions. Use `start "" /D dir prog.exe` without redirects. Stderr capture should be done inside the binary itself (e.g., tracing to file). Same class of bug as `timeout` failing in schtask context — both are cmd.exe commands that assume interactive console I/O.
  _Why: 2026-04-03 — `start-rcagent.bat` via schtask failed on ALL 8 pods (not just Pod 6 as initially believed). `start "" /D C:\RacingPoint rc-agent.exe 2>> rc-agent-stderr.log` returned exit code 1 with `The process cannot access the file because it is being used by another process`. Removing `2>> rc-agent-stderr.log` fixed it instantly — `Last Result: 0`, rc-agent running in Session 1. Misdiagnosed as Pod 6-specific for weeks because quick-start workaround masked the fleet-wide issue._
- **NEVER run pod binaries on James's PC** (rc-agent.exe, pod-agent.exe, ConspitLink.exe) — crashes workstation.
- **SP game launch = direct acs.exe, no bat.** `ac_launcher.rs` spawns `acs.exe` with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`. NEVER use `cmd /C launch-ac.bat` for single-player — the bat creates a console chain that sends `CTRL_CLOSE_EVENT` to rc-agent, killing it with `0xC000013A`. MP still uses bat (Content Manager URI handling). Fix: `d616ee10`.
  _Why: 2026-04-07 — Agent crashed every ~10s after AC launch on all 8 pods. termination.log showed `CTRL_CLOSE_EVENT (code=2)` every ~30s. Root cause: `start-rcagent.bat` shares a console with rc-agent. The bat's `cmd /C launch-ac.bat → start acs.exe` creates child consoles. When AC takes exclusive fullscreen, the console hierarchy closes, sending CTRL_CLOSE_EVENT which Windows forcefully terminates after 5s regardless of handler return value._
- **Deploy must include bat file sync.** When deploying rc-agent binaries, ALSO deploy `start-rcagent.bat` from the repo (`scripts/deploy/start-rcagent.bat`). Old bats on pods lack staged binary cleanup (lines 80-83) which causes parity drift — the bat overwrites new binaries with stale staged files on restart.
  _Why: 2026-04-07 — Fleet deploy was reverted 3 times because old bats found stale `rc-agent-????????*.exe` files and swapped them in over the newly-deployed `rc-agent.exe`._
  _Why: Pod binaries assume hardware/ports that don't exist on James's machine; crash is instant._
- **Test before upload** = `cargo test` + size check + deploy to Pod 8 first, verify, then other pods.
  _Why: Pod 8 canary catches runtime failures (DLL missing, wrong CWD, config mismatch) before fleet-wide damage._
- **Release builds always produce fresh `GIT_HASH` — no `touch` or `cargo clean` needed.** `[profile.release] incremental = false` in workspace `Cargo.toml` guarantees that `env!("GIT_HASH")` is re-evaluated on every release build. The `touch build.rs` workaround is no longer needed and should NOT be used. If `build_id` is stale after a release build, something is wrong — investigate, don't add workarounds.
  _Root cause (2026-04-07): Cargo's incremental compilation caches object files by source hash. `build.rs` correctly reruns and outputs the new GIT_HASH, but Cargo's incremental cache doesn't invalidate on `rustc-env` changes — only source file changes matter. `touch build.rs` was unreliable because it forces the build script to rerun but doesn't guarantee dependents recompile. Full `cargo clean` (49GB) was the only workaround. Fix: `incremental = false` for release profile eliminates the entire class._
- **Smallest Reversible Fix First** — when fixing a production issue, prefer the smallest change that can be tested and rolled back. Don't rewrite Rust code when a bat file one-liner works. Don't touch self-restart logic when a boot-time cleanup suffices. Save elegant fixes for when you have a test environment.
  _Why: PowerShell memory leak fix attempt changed self_monitor.rs relaunch logic. Four iterations (cmd/c, CREATE_NO_WINDOW, exit, Environment::Exit) all broke self-restart, each time taking Pod 6 down with manual recovery. The working fix was always `taskkill /F /IM powershell.exe` in start-rcagent.bat — one line, zero risk._
- **Have a rollback plan before deploying** — before changing critical paths (self-restart, deploy chain, process guard), prepare a one-command recovery: Tailscale SSH + schtasks to restart, or SCP the old binary back. Never deploy without knowing how to undo.
  _Why: Pod 6 went down 4 times during self-restart fix attempts with no prepared recovery path. Had to discover Tailscale SSH mid-incident._
- **Tailscale SSH fallback for pod recovery** — when rc-agent is dead and LAN exec is unavailable, SSH via Tailscale: `ssh -o StrictHostKeyChecking=no User@<tailscale_ip>`. Use `schtasks /Run /TN StartRCAgent` to restart. Pod Tailscale IPs: sim1-sim8 (run `tailscale status` to find).
  _Why: Discovered during Pod 6 incident — only way to recover a pod when rc-agent is dead, rc-sentry doesn't restart it, and no one is physically at the venue._
- **Deploy staging path:** `C:\Users\bono\racingpoint\deploy-staging\`
  _Why: Consistent staging root prevents "which binary is current" confusion across sessions._
- **Pendrive install:** `D:\pod-deploy\install.bat <pod_number>` (v5) — run as admin on the pod. For pods with RCAGENT_SERVICE_KEY blocking exec.
  _Why: Pendrive path is fixed; using ad-hoc paths leaves install.bat version drift._
- **MANDATORY: After ANY server deploy, rebuild ALL 3 frontends (kiosk, web, admin).** The server sends WS messages to dashboards — if the server binary changes message formats and frontends have stale JS, dashboards enter a connect/disconnect loop (800+ events/min) that's invisible to health checks. Build locally → tar → SCP → extract → restart schtask. The WS churn metric in `/fleet/health` (`dashboard_ws_churn.connects_per_min > 10`) detects this.
  _Why: 2026-04-03 — admin dashboard had 4-day-old build. New server sent WS events the old JS couldn't parse. Admin's WebSocket crashed and reconnected every 1s for hours. Portal showed "admin: ok" (HTTP health passed). Only discovered when user noticed kiosk showing "Connecting..." red dot. RC-Doctor, MI, and 60-phase audit all missed it because none check WS stability._
- **MANDATORY: After ANY kiosk/web/admin frontend deploy, verify the API proxy works.** Before marking a frontend deploy as complete, run `curl -s http://<host>:<kiosk-port>/api/v1/health` (NOT the backend port — the KIOSK port). If you get HTML or 404 instead of JSON, the Next.js rewrite proxy is broken and ALL client-side API calls (staff PIN, experiences, fleet health, billing) will fail silently. Also verify: `curl -s http://<host>:<kiosk-port>/kiosk/api/health/deep` returns `{"healthy": true}`. The deep health endpoint includes a self-proxy test that fetches through its own rewrite proxy — if the proxy is broken, `api_proxy` check fails automatically. **This is the FIRST thing to test after a frontend deploy — before screenshots, before visual verification, before bundle inspection.** A page that loads but can't call the API is worse than a 404.
  _Why: 2026-04-12 — kiosk HUD + card grid deployed to .23:3300. Staff PIN login broken because Next.js `basePath: "/kiosk"` auto-prefixed the rewrite rule, so the proxy only matched `/kiosk/api/*` but `fetchApi` sends to `/api/v1/*` (no prefix). Also `GET /kiosk/experiences` returned 401 (required staff JWT, standalone pod view has no JWT). Also `GET /games/catalog` returned 401 (same). 13-item PoE verified page loads, bundle contents, game logo assets — ALL passed. But zero PoE items tested "does fetchApi succeed from the browser's perspective?" Three bugs shipped to production, detected only when user reported "staff PIN not working." Same anti-pattern as "build_id match ≠ fix works" but applied to frontend deploys._
- **Rebuild + redeploy after functional code commits — ALL apps, not just Rust.** This applies to Rust binaries AND Next.js frontend apps (kiosk, web, admin). For Rust: `git log <deployed_build_id>..HEAD -- crates/<crate>/`. For frontend: `git log --since="<deploy_date>" -- kiosk/ web/ apps/`. Any `.rs`, `.tsx`, `.ts`, or `.css` change = rebuild required. At session start, check BOTH Rust `build_id` AND frontend deploy dates. The quality gate (`run-all.sh` Suite 5) now runs `frontend-staleness-check.sh` to catch stale frontend deploys automatically.
  _Why: 2026-03-28 — kiosk was 14 days stale with 72 bug fixes sitting undeployed in git. All 4 Unified Protocol layers passed (quality gate, E2E, standing rules, MMA) because they only checked Rust binary freshness. The standing rule syntax was Rust-specific (`crates/<crate>/`) and had zero enforcement for frontend apps. Original Rust rule: 2026-03-24 audit found server running `0bebb9aa` while HEAD was `848b127b`._
- **Server deploy: use `deploy-server.sh` (v3.0, MMA-hardened).** 12-model audit across 3 rounds. Script at `deploy-staging/deploy-server.sh`. 8 steps: connectivity (LAN→Tailscale) → download (HTTP→SCP, size-verified) → confirmed kill (poll 15s + port free) → atomic swap (del prev→ren current→prev→ren new→current, with auto-recover on failure) → start (schtasks, fallback direct) → build_id verify (3 attempts) → smoke test (4 endpoints) → cleanup stale binaries. **Auto-rollback** on start failure, build_id mismatch, or smoke test failure.
  _Why: 2026-03-28 deployment incident hit 10 failure modes: Tailscale SSH down, schtasks didn't start process, SSH `start /B` doesn't persist, port conflict from concurrent binaries, `ren` failed because prev existed, stale binaries accumulated, 401 on debug endpoints not caught pre-deploy. MMA audit (DeepSeek V3, Qwen 3, Grok 3, Mistral Large, Claude Opus, GPT-5.4, Gemini Pro, Nemotron) produced v3.0 with all 10 fixes._
- **Server binary swap: rename, don't overwrite.** Windows locks running executables — `move /Y` and `del` fail while the process holds a handle. Sequence: (1) `del racecontrol-prev.exe` (clear old prev first), (2) `ren racecontrol.exe racecontrol-prev.exe` (rename running — Windows allows), (3) `ren racecontrol-<hash>.exe racecontrol.exe`. If step 3 fails, auto-recover: `ren racecontrol-prev.exe racecontrol.exe` (no-binary-left guard). Keep `racecontrol-prev.exe` for 72hr rollback.
  _Why: 2026-03-24 `move /Y` stuck in loop. 2026-03-28 `ren` failed because prev existed. MMA Round 2 (4 models) found 3-step swap can leave NO binary — added recovery guard._
- **Confirmed kill before swap — NEVER run two binaries simultaneously.** Before swapping the server binary, verify the old process is dead (poll `tasklist` every 3s for 15s) AND ports 8080/8090 are free (poll `netstat` for 10s). Running a new binary while the old one holds ports causes `os error 10048` (address in use) and false "binary crash" diagnosis.
  _Why: 2026-03-28 new binary was wrongly classified as "broken" when it crashed from port conflict with the still-running old binary. Two deploy iterations wasted before discovering the real cause._
- **Smoke test debug endpoints after server deploy.** After verifying build_id, also test `/debug/activity`, `/debug/playbooks`, and `/fleet/health`. Auth route changes can silently break these endpoints without affecting the health check. If any return non-200, rollback immediately.
  _Why: 2026-03-28 debug chatbot broken because debug routes moved to authenticated section. Health check passed, build_id matched, but kiosk debug page returned 401. Required a second rebuild + redeploy cycle._
- **single-binary-tier policy (v22.0):** All pods run the SAME binary compiled with default features (full build). Feature selection is done at RUNTIME via feature flags (FF-01+), NOT at compile time per pod. The `--no-default-features` build exists for CI verification and future testing scenarios only — it is NEVER deployed to production pods. Do not create per-pod Cargo feature profiles, per-pod binaries, or pod-specific compile-time feature sets.
  _Why: Per-pod compile-time variants create a combinatorial explosion of untested binaries. 8 pods x N feature combinations = build/test/deploy nightmare. Runtime feature flags (v22.0 Phase 177+) provide the same capability with one tested binary._

- **rc-agent MUST run in Session 1 (interactive desktop).** Session 0 (services) prevents ALL GUI operations: Edge browser, game launching, ConspitLink, overlay HUD, window management, SendInput, taskbar control, and freeze detection. The `RCWatchdog` Windows service handles restarts using `WTSQueryUserToken` + `CreateProcessAsUser` to spawn `start-rcagent.bat` in Session 1. **NEVER create schtasks or services that start rc-agent directly** — they run as SYSTEM in Session 0. The HKLM Run key (`start-rcagent.bat`) handles first boot in Session 1; `RCWatchdog` handles crash recovery in Session 1.
  _Why: 2026-03-26 — ALL 8 pods had blanking screen broken for unknown duration. The bat-based `RCAgentWatchdog` schtask ran as SYSTEM (Session 0), restarted rc-agent there after crashes. Edge couldn't create windows. `lock_screen_state: screen_blanked` with `edge_process_count: 0` — an impossible state that went undetected because the audit checked health/build_id (proxies) instead of actual behavior. No customer-facing screen was working on any pod._
- **Audit must verify Session context.** At session start AND in every audit, run `tasklist /V /FO CSV | findstr rc-agent` and confirm the session column shows `Console` (not `Services`). Also check `:18924/debug` endpoint: `edge_process_count` must be >0 when `lock_screen_state` is `screen_blanked`. If edge=0 + state=blanked, the blanking screen is broken regardless of what health says.
  _Why: The previous audit checked build_id, WS connectivity, HTTP reachability, and health endpoints — all passed while blanking was broken on ALL pods. The debug endpoint had the answer the whole time but was never queried._
- **Behavioral verification for blanking.** After deploying rc-agent or rc-watchdog, trigger `RCAGENT_BLANK_SCREEN` via exec and verify `edge_process_count > 0` at `:18924/debug` within 12 seconds. This is the ONLY reliable test — health endpoints and build IDs are necessary but not sufficient.
  _Why: `show_blank_screen()` sets state to `screen_blanked` even when `launch_browser()` silently fails. The state change succeeds but the browser never launches. Only checking the actual Edge process count catches this._

### Comms

- **Bono INBOX.md:** Append to `C:\Users\bono\racingpoint\comms-link\INBOX.md` → `git add INBOX.md && git commit && git push`. Entry format: `## YYYY-MM-DD HH:MM IST — from james`. Then also send via WS (send-message.js). Git push alone is insufficient — Bono does not auto-pull.
  _Why: Git-only comms left Bono's context stale on three occasions; WS+git is the required dual channel._
- **Auto-push + notify (atomic sequence):** `git push` → comms-link WS message → INBOX.md entry. Do all three before marking tasks complete, starting new work, or responding to Uday. Every push, every commit — even cleanup/docs/logbook. No ranking of "important" vs "minor" commits.
  _Why: Commits without push leave Bono's context stale and break deploy chains; treating minor commits as optional caused missed notifications._
- **Bono VPS exec (v18.0 — DEFAULT):** Use comms-link relay, not SSH. Single: `curl -s -X POST http://localhost:8766/relay/exec/run -H "Content-Type: application/json" -d '{"command":"git_pull"}'`. Chain: `curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[...]}'`. SSH (`ssh root@100.70.177.44`) only when relay is down.
  _Why: SSH requires Tailscale up and leaves no audit trail; relay is always-on and returns structured results._
- **Standing Rules Sync:** After modifying CLAUDE.md standing rules, always sync to Bono via comms-link so both AIs operate under the same rules.
  _Why: Rules drift between AIs causes inconsistent behavior and contradictory decisions in multi-agent tasks._
- **Verify recipient infrastructure before sending instructions.** Before writing ANY instructions, docs, protocols, or runbooks addressed to Bono or Uday, STOP and verify: what tools/access does the RECIPIENT actually have? Bono uses **Perplexity MCP** (`pplx_*` tools) for non-MMA work; **for MMA Bono uses OpenRouter as default per Captain directive 2026-05-01 IST** (Phase 2 OpenRouter migration in flight; Perplexity MCP is degraded-fallback during Phase 2 with explicit DEGRADED-MMA-PERPLEXITY-FALLBACK log tag). James uses OpenRouter API + Node.js scripts (already on OpenRouter for both MMA + non-MMA). Uday uses WhatsApp + phone. Never assume the recipient has the same tools as James. This check applies to: INBOX.md entries, protocol docs, deploy runbooks, audit instructions — ANY artifact that tells someone else what to do.
  _Why: Multi-Model Audit Protocol v1.0 told Bono to run OpenRouter scripts directly. Bono used Perplexity MCP — completely different. The error was in the system-reminder context the entire time but never checked. Same class as "health passes but blanking is broken" — verifying YOUR view instead of the TARGET's reality. Captain 2026-05-01 IST flipped MMA default to OpenRouter (both pilots) after Bono's pplx_council 403 token-expired empirically demonstrated Perplexity MCP as fragile-MMA-transport; OpenRouter unified path is more robust + composable with james-side `shared/openrouter.js` v4.0 enforcement helpers._

### PACT framework (L0/L1/L2 cascade — ratified 2026-04-25 PACT-20260425-004)

Cross-AI decisions use the layered PACT cascade. **Canonical specs live in `comms-link/`** — this is a pointer, not a copy:

- **`comms-link/PACTS.md`** (L0 audit floor) — every decision row, append-only
- **`comms-link/PACT-VOCAB.md`** (linguistic primitive) — 9-tag compact envelope (PACT-011)
- **`comms-link/PACT-CHARTER.md`** (L1 fast-path) — 4 pre-authorized classes (`diagnostic-only`, `restart-storm-mitigation`, `zombie-cleanup`, `known-bug-hotfix`); in-class PACT proceeds on initiator vote alone
- **`comms-link/JAMES-PROFILE.md` + `comms-link/BONO-PROFILE.md`** (L2 vote-drift) — when partner offline, online AI runs meta-prompt prediction; ≥0.85 confidence (in-class) / ≥0.90 (unclassified) / ≥0.92 (provisional James-profile) → emit `DRIFTED-VOTE-PENDING-CONFIRM`; partner CONFIRMs/CHALLENGEs on return
- **L2.5 MMA-Substitute-Pilot** (Captain G33-LEVEL-B 2026-05-05 ~07:55 IST) — when partner offline + bilateral class OR pilot stuck-after-first-attempt OR knowledge-gap, online pilot runs MMA per ratified Protocol v4.0 (≥5 models / ≥3 vendor families via OpenRouter); MMA consensus stands in for partner AMPLIFIER vote; substitute has FULL authority during substitution; partner returns → INFORMED via NOTIFY (Q-OP3 substitute-pilot model). Class boundary: applies to doctrine + customer-impact + cross-pilot-affecting; does NOT apply to bono-internal substrate hygiene (literal-reading 30d freeze carve-out). Decider vote guaranteed by odd-N config (Q-OP2). Standard 24h CHALLENGE-AMEND window applies forward.
- **L3 burst-duplex** — deferred until L2 calibration (7-day window, review 2026-05-02)
- **L4 Uday escalation** — always-available G33 sync-engaged override

**CGP gates always apply on L1 fast-path AND L2.5 MMA-substitute PACTs** — H1–H5 enforcement is independent of vote requirement. Charter/MMA-substitute relaxes vote, not discipline.

**Attribution discipline (L2.5)**: when MMA-substitute interprets Captain doctrine into specific PACT cascade dispositions, the ledger entries (`pact-slots.jsonl` / `PACTS.md`) MUST attribute to the substitute pilot (`ai:"james"` or `ai:"bono"`), NOT to Captain. Captain ratified the DOCTRINE, not each downstream PACT under the cascade. Anti-pattern: substituting "Captain ratified the doctrine (Q1)" → "Captain ratified each downstream PACT in the ledger" — captured as META-class C sub-class N=26 (Captain-attribution-substitution; bono PART 34 §S-48.10).

Full meta-prompt template + class definitions: `comms-link/CLAUDE.md` § PACT framework. When initiating a PACT, MUST add `CHARTER-CLASS:` field to proposal header (or `unclassified` for default dual-vote).

### Security Debt — Open-by-Default Flagged-to-Close (Captain Q2 2026-05-05)

**Captain G33-LEVEL-B 2026-05-05 ~07:55 IST verbatim Q2**: "Open by default but flagged to close later even when venue is up and running"

V2 ships with V1 trust intact. Every grandfathered open path (auth gap, credential storage, policy gap, audit gap) MUST be logged in `comms-link/data/security-debt-ledger.jsonl` (append-only, one JSON per line) with explicit closure-Phase commitment. Closures hot-swap (no venue downtime). Transforms HARD-BLOCKER security gaps into debt-track that resolves progressively in subsequent phases.

**Schema parity** to `openrouter-spend-{bono,james}.jsonl` discipline: append-only / inline comment at surface / closure-PACT auto-marks `closed: true` with PACT ID reference. Companion README at `comms-link/data/security-debt-ledger.README.md`.

**Initial seeds (3 entries, bono PART 34 §S-48.4)**:
1. PACT-026 §A direct racecontrol M2M paths — class=auth-gap; closure_phase=Post-V2.0-AUTH-Sprint
2. PACT-018 staff.pin raw V1 contract — class=credential-storage; closure_phase=Phase-0.5c-AUTH (sibling sub-PACT for bcrypt-hardening; PACT-018 AMEND-1 RATIFIED 2026-05-05 09:43 IST absorbs CAVEAT-3 inline schema comment for this debt)
3. Q4-3 dynamic pricing discount ceiling — class=policy-gap; closure_phase=Post-V2.0-Pricing-Calibration

**Bilateral writers**: both pilots append entries when encountering open-by-default paths during V2 work. Bono parity-trips entries on V2.0 milestone audit.

**Composes-with**: PACT-026 §A NO-direct-heart-narrow-carve-out (CAVEAT-C3 transforms BLOCKER → debt-track); §S-48.6 Universal Sync rule; Captain "loophole-vs-saintly" calibration substrate (V2-MASTER-STATE §S-30.1).

### Code Quality

- **Next.js middleware redirects: use `"/"` not `"/basePath"`.** When a Next.js app has `basePath` configured (e.g., `/kiosk`), `url.pathname` in middleware is auto-prefixed with basePath. Setting `url.pathname = "/kiosk"` doubles it to `/kiosk/kiosk` → 404. Always use `url.pathname = "/"` for root redirects. After any middleware change, test the actual browser URL — not just curl.
  _Why: 2026-04-03 — cache-busting middleware expanded matcher to all routes. Staff page redirect used `url.pathname = "/kiosk"` → doubled to `/kiosk/kiosk` → 404 for all staff access. Portal showed "ok" (health API endpoint worked), MI didn't detect it (no page-level probes)._
- **Never middleware-protect a login page.** If a route contains a login form (the entry point that CREATES the auth token), it MUST be public in server-side middleware. Only protect routes BEHIND the login — not the login itself. Before adding any route to a middleware auth gate, ask: "Can a user with NO credentials reach the form that gives them credentials?" If no → the gate creates a chicken-and-egg loop. Client-side auth gates (`useEffect` + redirect) are the correct pattern for login pages — they render the login form first, then redirect to the dashboard after auth.
  _Why: 2026-04-04 — `/staff` added to middleware STAFF_ROUTES (SEC-P2-9). Staff couldn't login because the login form was on `/kiosk/staff`, which middleware blocked without JWT. Staff page and customer page appeared identical because every `/kiosk/staff` visit → 307 → `/kiosk`._
- **No `.unwrap()` in production Rust** — use `?`, `.ok()`, or match.
  _Why: Unwrap panics crash the entire service; production code must degrade gracefully._
- **No `any` in TypeScript** — type everything explicitly.
  _Why: `any` hides real type errors that surface at runtime, not compile time._
- **`.bat` files: clean ASCII + CRLF.** Use bash heredoc + `sed 's/$/\r/'`. Never Write tool directly (adds UTF-8 BOM = breaks cmd.exe). Never use parentheses in if/else — use `goto` labels. Test with `cmd /c` before deploying.
  _Why: BOM and parentheses in .bat files cause silent command failures on Windows; caught after multiple broken deploys._
- **Static CRT:** `.cargo/config.toml` `+crt-static` — no vcruntime140.dll dependency on pods.
  _Why: Pod images don't ship VS redistributables; dynamic CRT causes instant crash-on-launch._
- **Cascade updates (RECURSIVE):** When changing a process, update ALL linked references (training data, playbooks, prompts, docs, memory). Never change one place and leave stale references. This includes **data formats** — if you change how a file is written (name, format, location), grep for every reader of that file and update them too. **The cascade is recursive**: if updating process A requires changing file B, then check what process B affects and update those too. Continue until no further downstream impacts exist. Document the full cascade chain in the commit message.
  _Cascade checklist for ANY change:_ (1) grep for all consumers of the changed interface/file/endpoint, (2) update each consumer, (3) for each consumer updated, repeat step 1 on THAT consumer, (4) update OpenAPI specs, contract tests, shared types, (5) document deploy impacts (cloud rebuild, pod redeploy).
  _Why: Stale references in playbooks or prompts cause both AIs to apply the old behavior after a fix. v23 example: rolling appender changed from `racecontrol.log.*` to `racecontrol-*.jsonl` but the `/api/v1/logs` reader still searched for the old name — API returned 3-day-old data silently for days. Kiosk audit (2026-03-24): adding `/games/catalog` endpoint missed 5 downstream consumers — web dashboard had 3 missing games, leaderboards had only 2/8 games, OpenAPI spec was stale, contract tests had no coverage, shared types lacked the response type._
- **Next.js hydration:** Never read `sessionStorage`/`localStorage` in `useState` initializer — use `useEffect` + hydrated flag.
  _Why: SSR reads fail server-side; hydration mismatch breaks the entire page silently._
- **Git Bash JSON:** Write JSON payloads to a file with Write tool, then `curl -d @file`. Bash string escaping mangles backslashes.
  _Why: Inline JSON in Git Bash strips backslashes from Windows paths, corrupting the payload._
- **NEVER edit remote files via inline PowerShell over SSH.** Git Bash expands `$_`, `$env:`, `$null` as empty bash variables, mangling PowerShell syntax. `Set-Content` writes empty/corrupted files. Instead: write config locally → SCP to target. If modification needed: write a `.ps1` script locally → SCP → `ssh host "powershell -ExecutionPolicy Bypass -File script.ps1"`. Pod TOML backups in `deploy/configs/rc-agent-pod{1-8}.toml`.
  _Why: 2026-04-08 — PowerShell `Where-Object { $_.Contains() }` via SSH wiped all 8 pod TOMLs to empty. Fleet down 25 minutes. Same class as SSH banner corruption but worse — total config loss, not just prepended garbage._
- **Never pipe SSH output into config files.** Use `scp` to copy files from remote hosts, not `ssh ... "cat file" > local`. SSH banners (post-quantum warning, MOTD) go to stderr but some wrappers merge streams, silently prepending garbage to the file. If SSH piping is unavoidable, use `ssh ... 2>/dev/null "cat file"`. After any remote file copy, validate the first line: `head -1 file | grep -q '^\[' || echo "CORRUPTED"`.
  _Why: 2026-03-24 — racecontrol.toml had 3 SSH banner lines prepended. TOML parser rejected from line 1. load_or_default() fell back to empty defaults. process_guard ran with 0 allowed entries for 2+ hours. No operator saw anything because the error was logged via tracing (not yet initialized at config-load time)._
- **UI must reflect config truth** — no hardcoded camera lists, names, or layouts. All UI must read from API/config dynamically. If the backend config changes, the UI must update without code changes.
  _Why: v16.1 cameras dashboard was initially built with hardcoded 13-camera arrays. When cameras were added/removed from NVR config, the UI showed stale/phantom tiles. Dynamic fetch from /api/v1/cameras fixed it — this rule prevents regression._
- **Never hold a lock across `.await`** — whether `std::sync::RwLock`, `tokio::sync::RwLock`, or `Mutex`. Clone/snapshot the data, drop the guard in a tight `{ }` block, THEN iterate or perform async work. This applies to ALL lock types in ALL async contexts. Audit pattern: `let snapshot = { let guard = lock.read(); guard.clone() }; /* guard dropped */ do_async().await;`
  _Why: v27.0 MMA audit — server WS handler held `agent_senders.read()` across 8 async sends during fleet broadcast. 5 models flagged this as P1 deadlock/starvation risk. Same pattern exists in 10+ places across the codebase from earlier milestones. The 8-pod scale hasn't triggered it yet, but it's structurally wrong._
- **Every `::default()` in new code must be reviewed for "is this the real state?"** before shipping. If a struct has a `Default` impl and the function operates on a specific entity (pod, driver, session), the default is almost certainly wrong — look up the real state from the relevant channel, cache, or DB. Mark intentional defaults with a `// Intentional default: <reason>` comment.
  _Why: v27.0 MMA audit — `FailureMonitorState::default()` was used as a "get it compiling" placeholder in `run_staff_diagnosis()`. Staff diagnostics ran on phantom pod state (zero errors, zero history). Tier 1 rules never triggered. 5 models flagged this as a correctness bug. The real state was available 3 lines away via `failure_monitor_rx.borrow().clone()`._
- **Financial flow E2E: trace actual currency values through complete flows** before shipping billing/wallet changes. Verify: create customer → topup → book session → launch game → end (early/normal/cancel) → check refund/balance. Unit tests on formulas are necessary but NOT sufficient — they use hardcoded correct inputs and miss data corruption in the surrounding function. Any function that UPDATEs and SELECTs the same DB column in the same scope must be audited for "does the SELECT get the original value or the value I just overwrote?"
  _Why: 2026-03-28 — F-05 (P1): `end_billing_session()` overwrites `wallet_debit_paise` at line 2213 before reading it at line 2255 for refund calc. Customer loses Rs.162.50 per early-end on a 30-min session. Survived 32+ model audits and 6 MMA rounds because: (a) unit test `partial_refund_calculation` tests the formula with hardcoded correct values, never exercises the actual function; (b) `end_billing_session()` has ZERO test coverage; (c) MMA prompts don't ask models to trace DB column values through UPDATE→SELECT flows. Root cause analysis: `.planning/audits/ROOT-CAUSE-ANALYSIS-F05-2026-03-28.md`._
- **Staff-triggered actions must NOT reuse autonomous broadcast paths without scope review.** The same fix that is safe for autonomous fleet learning (pod discovers → gossip → all pods learn) is dangerous when staff-triggered (staff fixes Pod 3 → should NOT automatically apply to Pods 1-8). Different origin = different blast radius. Gate fleet broadcast behind `tier >= 2 && confidence >= 0.8` for staff flows; Tier 1 deterministic fixes are always pod-local.
  _Why: v27.0 MMA audit — initial implementation broadcast ALL staff-triggered fixes to ALL 8 pods including Tier 1 deterministic fixes. 4/5 models flagged this as a physical safety risk in a racing simulator (applying brake calibration from Pod 3 to Pod 7). Fixed by gating broadcast to Tier 2+ KB-sourced solutions only._
- **GDPR erase contract: new driver/lap FK = update customer_data_delete().** Every new table with `REFERENCES drivers(id)` or `REFERENCES laps(id)` MUST add a matching `DELETE FROM <table> WHERE driver_id = ?` (or equivalent FK column) to `customer_legal.rs::customer_data_delete()` in the SAME commit. Verify after any migration change: count `REFERENCES drivers(id)` in `migrate_*.rs` files and compare against DELETE count in `customer_data_delete()` — they must match (±1 for the driver record itself). Also applies to `revoke_consent_handler` / `anonymize_driver_pii` paths which anonymize rather than delete. A `cargo test` exercising the full erase path (create driver with rows in ALL FK-referencing tables, then delete, assert success) is the long-term structural fix.
  _Why: 2026-04-16 — James FK audit found `d24b17f7` now enforces `PRAGMA foreign_keys=ON` on ALL pool connections (was only first connection). Any table with a driver FK that's missing from `customer_data_delete()` will cause SQLITE_CONSTRAINT on DPDP erasure — legal compliance failure at runtime with zero compile-time warning._

### Process

- **Milestone completion = update ARCHITECTURE.md + memory.** When a milestone ships (all phases `[x]` in ROADMAP.md), the SAME session MUST update: (1) `docs/ARCHITECTURE.md` section 20.3 shipped milestones table, (2) `~/.claude/projects/C--Users-bono/memory/gsd-projects.md` shipped milestones table + active work section + key stats. Without this, future sessions start with stale context about what's been built.
  _Why: 2026-04-06 — 10 milestones shipped between 2026-04-01 and 2026-04-06 (v37.0 through v43.0) without updating either ARCHITECTURE.md or memory. Memory showed v36.0 as latest (5 days stale). User had to ask multiple times._
- **ROADMAP plan checkbox sync on completion.** When a plan's SUMMARY.md is committed, the SAME commit MUST update the corresponding `- [ ]` checkbox in ROADMAP.md to `- [x]`. Phase-level `[x]` markers are NOT sufficient — plan-level checkboxes must also be updated. Verification: `grep "^- \[ \] <plan-id>-PLAN" .planning/ROADMAP.md` must return zero hits for the just-completed plan.
  _Why: 2026-03-27 audit found 172 stale `[ ]` plan entries under shipped `[x]` phases. v24.0 (8 phases, 18 plans, fully shipped) was falsely reported as incomplete because the audit trusted plan checkboxes (metadata proxy) instead of git history (ground truth). Same class of bug as "health passes but blanking is broken" — the standing rule "verify the EXACT behavior, not proxies" applies to GSD tracking too._
- **Fix commits require G4 NOT TESTED list.** When committing a fix (any `fix:` commit), include a G4 block listing what was tested (compilation, unit tests) and what is NOT tested (runtime behavior). Compilation is a proxy metric — "compiles" ≠ "works." The G4 NOT TESTED list tracks what still needs runtime verification after deploy. This applies even when deploy is deferred — the commit is a claim that the fix is correct, and unverified claims need explicit tracking.
  _Why: 2026-04-03 — two mesh fixes committed with `fix:` prefix and "PERMANENT" label. G1/G4 skipped because "no deploy claimed." But the fix claim itself was unverified — MAINT_CLEAR_COUNT root cause was uncertain (file not found on disk but diagnostic engine still reporting it), and SO_REUSEADDR was only compilation-tested. "Commit only" is not a G1/G4 exemption._
- **Refactor Second** — characterization tests first, verify green, then refactor. No exceptions.
  _Why: Refactoring without a green test baseline turns every compile error into an unknown regression._
- **Route Uniqueness: move = delete old in same commit.** When moving a route to a role-gated sub-router, DELETE the original registration in the same commit. Axum panics at runtime (not compile time) on duplicate METHOD+PATH. Verify with: `grep -n '\.route("/' src/api/routes.rs | sed 's/.*\.route("//' | sed 's/".*//' | sort | uniq -d` — must return empty. The `route_uniqueness_tests::no_duplicate_route_registrations` test catches this at `cargo test` time.
  _Why: 2026-03-29 — Phase 258 added role-gated routers without removing 21 original routes. Server panicked on startup. 30-model MMA audit missed it because it's a runtime-only check. Watchdog MAINTENANCE_MODE masked the real error for 30+ minutes._
- **Cross-Process Updates** — changing a feature? Update ALL: rc-agent, racecontrol, PWA, Admin, Gateway, Dashboard. This means ALL ENVIRONMENTS too — venue (.23), cloud (Bono VPS), and James (.27). Deploy to one and forget the other = schema divergence.
  _Why: Single-crate updates leave other components speaking a different protocol version. Cloud sync broke for 3+ hours because venue had new migrations but cloud DB was on an old binary with missing columns._
- **DB migrations must cover ALL consumers.** `CREATE TABLE IF NOT EXISTS` won't alter existing tables. If a column is used in sync/query code, the migration must `ALTER TABLE ADD COLUMN` for it — even if the CREATE TABLE already includes it. Old databases won't have columns added after initial creation.
  _Why: `updated_at` was in 10 CREATE TABLE statements but only 2 had ALTER migrations. Cloud and venue DBs created by different binary versions had different schemas. Required manual ALTER on 8 tables to fix._
- **`sqlx::migrate!` macro embeds migrations at compile-time — adding new .sql files requires explicit cache invalidation.** Cargo's incremental cache treats migration .sql files as opaque; touching them does NOT invalidate the binary's embedded migration set. Symptoms: tests that exercise the new migration fail with schema errors (`no such table`, `no such column`, dangling FK to a renamed-then-dropped table) despite the migration file being on disk. Fix: `cargo clean -p <crate>` then re-run tests. Lighter alternative: `touch crates/<crate>/src/lib.rs` to force the build script to re-evaluate without nuking the entire `target/`. Applies to any `migrations/` directory consumed by `sqlx::migrate!()` (e.g. `crates/v2-db/migrations/`).
  _Why: 2026-05-08 V2 Wave 1 W1-S2 — added `crates/v2-db/migrations/20260508000001_wallet_redemptions_fk_repair.sql` (NF-james-4 schema bug repair). Three `reconcile_redemption` tests failed with `no such table: main.sessions_old_pact018` despite the new migration explicitly fixing the FK. Root cause: `migrate!()` embedded migrations into the compiled artifact at the previous compile boundary; cargo incremental did not invalidate when the new .sql appeared. `cargo clean -p v2-db && cargo test -p v2-db` cleared the embedding and tests passed 43/43._
- **SQLite `ALTER TABLE RENAME` rewrites foreign-key references in OTHER tables.** With `PRAGMA legacy_alter_table=0` (SQLite default since 3.25), renaming a table also rewrites every `REFERENCES <old_name>(col)` clause in every other table to point at the new name. The recreate-table pattern (RENAME → CREATE NEW → INSERT → DROP) therefore leaves dangling FKs in any sibling table that referenced the original — those FKs end up pointing at the transient renamed-then-dropped table. Migration authors must include sibling-table rebuild in the same migration if any other table has a FK to the renamed table. Audit: after writing a recreate-table migration, `grep -rn "REFERENCES <renamed_table>" crates/<crate>/migrations/` and rebuild every match.
  _Why: 2026-05-08 NF-james-4 — `crates/v2-db/migrations/20260503000003_staff_table_and_fk.sql` used the recreate-table pattern on `sessions` (FK to staff). It rebuilt `wallet_topups` (also pointed at sessions indirectly via staff) but missed `wallet_redemptions.session_id REFERENCES sessions(id)` from the initial schema. Rename rewrote that FK to `sessions_old_pact018`; subsequent DROP left it dangling. INSERT into wallet_redemptions tripped `no such table: main.sessions_old_pact018` 5 days after migration 003 landed (was invisible until first behavior-consumer in W1-S2)._
- **Review parallel session commits against standing rules before deploying.** Code from other sessions may not follow current rules (bat parentheses, missing verification, stale references). Always `git show <hash>` and check against standing rules before accepting.
  _Why: Parallel session commit `a948569` used parentheses in bat if/else blocks — caught during standing rule review, fixed before deploy._
- **Convert timestamps before counting events.** Racecontrol logs are UTC; all operations are IST. Before reporting "N events happened," convert every timestamp and exclude your own actions (deploys, restarts, test kills). An audit that reports its own deploys as "unexplained restarts" wastes investigation time and erodes trust in findings.
  _Why: "5 unexplained restarts" turned out to be 1 post-reboot startup + 4 of our own deploys. UTC 03:28 was misread as IST instead of IST 08:58. The Event Viewer check that would have caught this in 30 seconds was deferred for hours._
- **First-run verification after enabling any guard/filter/blocklist.** When flipping `enabled = false` to `true` on any filtering system (process guard, firewall, allowlist, rate limiter), check the FIRST scan result immediately: how many items flagged? If "everything" or "nothing" — the config is wrong. An empty allowlist + enabled guard = block everything. This is structurally incomplete — don't mark shipped.
  _Why: Process guard was enabled with an empty allowlist. Every process was flagged — 28,749 false violations/day for 2 days. Nobody noticed because (a) the log API was broken (F12), (b) no automated monitoring existed, (c) no first-run check was done after enabling._
- **No Fake Data** — use `TEST_ONLY`, `0000000000`, or leave empty. Never real-looking identifiers.
  _Why: Realistic-looking fake data (names, IDs, emails) has leaked into production databases twice._
- **Prompt Quality Check** — missing clarity/specificity/actionability/scope → ask one focused question before acting.
  _Why: Acting on ambiguous prompts produces work that must be redone; one question costs less than one wrong implementation._
- **Links and References = "Apply Now"** — when the user shares a link, article, or methodology alongside a problem, apply it to the current problem FIRST, document it SECOND. A reference shared during active work is a tool to use, not information to file.
  _Why: User shared 4 debugging methodologies during an active crash investigation. James wrote a comparison table and updated rules instead of applying them to the open bug. Three prompts wasted before actual debugging happened._
- **Learn From Past Fixes** — check LOGBOOK + commit history before re-investigating.
  _Why: Re-investigating solved problems wastes session time; LOGBOOK has resolved the same issue in under 2 minutes._
- **LOGBOOK:** After every commit, append `| timestamp IST | James | hash | summary |` to `LOGBOOK.md`.
  _Why: LOGBOOK is Tier 2 debugging — without consistent entries, memory-based debugging fails._
- **Act vs Analyze — read the verb.** Before responding, identify the user's verb. Action verbs ("use", "execute", "do", "deploy", "apply", "fix", "launch", "run", "check") = act immediately, show results not analysis. Analysis verbs ("summarize", "your thought", "what do you think", "review", "compare") = discuss and explain. Links/references shared alongside an open problem = implicit "use this" = ACT.
  _Why: James defaulted to analyze mode when given a debugging methodology link during an active crash. User had to prompt 3 times before actual debugging happened._
- **Apply new knowledge immediately.** When the user provides a methodology, technique, or reference relevant to an open problem, apply it to the current problem FIRST, document it SECOND. Do NOT just summarize, compare, or update rules — use it NOW on the open bug/task.
  _Why: User shared 4 debugging methodologies during an active crash investigation. James wrote a comparison table and updated rules instead of applying them to the open bug. Three prompts wasted before actual debugging happened._
- **Audit all PCs regardless of venue hours.** Never skip POS or any PC audit because venue is closed. Always attempt to reach every machine. "POS PC offline (expected outside business hours)" is NOT an acceptable pass — report actual state as WARN or FAIL if unreachable. Only suppress checks that physically require the venue open (display resolution, kiosk browser on pods), NOT network reachability or service health.
  _Why: User explicitly corrected this assumption. The audit reported offline machines as "expected" instead of flagging them._
- **Fix one system? Fix ALL systems.** After fixing any issue on one machine, immediately ask: "Does this apply to all pods/POS/server/cloud?" If yes, roll it out fleet-wide in the same step. Don't wait to be asked. Applies to: Tailscale config (all 11 machines), agent binaries (8 pods + POS), startup scripts, registry changes, firewall rules, service recovery settings.
  _Why: Tailscale auth key was initially only fixed on the server. User had to remind to apply to all 11 machines._
- **ALL target enumeration — from MEMORY.md, not code.** Before ANY fleet-wide operation (deploy, audit, status check, "list undeployed"), enumerate ALL targets from operational memory: Server (.23), Pods 1-8, POS (.20), Cloud (Bono VPS), Comms-link James (.27), Comms-link Bono (VPS). This applies to deploys AND to queries like "what's pending?" — the verb doesn't matter, the target list does. Never use fleet health endpoint or code exploration as the target list — they structurally miss POS, cloud, and future devices.
  _Why: POS was forgotten during 14-model MMA audit deploy (2026-03-29). Bono VPS was forgotten during "list undeployed commits" query (2026-04-03). Same root cause both times: mental model of "the fleet" = venue-only._
- **Background agents lose Bash permissions.** Never use `run_in_background: true` for GSD executor agents. Background agents can't surface the Bash approval prompt. Use multiple foreground Agent calls in a single message for parallelism. Only use background for research/read-only agents that don't need Bash.
  _Why: v12.1 Phase 104 — two parallel background executors both failed silently because Bash was denied._

- **Summary Fidelity — only report what was discussed.** Only include items in summaries/walkthroughs that were explicitly discussed with the user. Never pad with own observations, suggestions, or items not yet raised. If you want to surface something new, present it as a separate question — not embedded in a summary.
  _Why: 2026-04-03 — listed 8 "remaining items" including POS color scheme, staff PIN hashing, and kiosk page archival that were never discussed. User had to correct._
- **Verify Before Providing URLs.** Before giving any URL to the user (localhost, LAN, or cloud), verify the service is actually running: `curl -s --connect-timeout 5 <URL> | head -c 100`. If the response is empty, an error, or the service isn't running, say so — don't provide the URL with a "should be at" caveat.
  _Why: 2026-04-03 — gave `http://localhost:3300` before verifying dev server started. Server had a TurboPack error._
- **Workflow Assumptions Require User Confirmation.** When tracing code paths to understand a business workflow, ALWAYS present findings as "here's what the code does — is this how it actually works?" before building features on top. Code traces reveal implementation, not intent. The user's description of the real workflow overrides code.
  _Why: 2026-04-03 — assumed customer enters PIN on kiosk (code has PIN modal), that wallet top-up exists on POS (it didn't), that 12-step wizard is the customer flow (it's staff-only). All wrong._

### Self-Audit (v43.0)

- **Run page self-audit at frontend session start.** Before modifying any frontend code (tsx, jsx, css, Next.js pages), run `bash tests/page-audit/self-audit.sh` to capture current page screenshots and generate the audit prompt. Then read `tests/page-audit/audit-prompt.md` -- for each page listed, use the Read tool to view the screenshot image and compare against the description file. Write anomalies to `tests/page-audit/audit-report.md`. This establishes baseline awareness: if a page is already broken before your changes, you know to fix it or avoid making it worse.
  _Why: Code-only fixes shipped 9+ times without anyone looking at the actual pages. The self-audit forces visual verification before AND after changes._
- **After frontend changes, re-run self-audit.** After completing frontend modifications, run `bash tests/page-audit/self-audit.sh` again and compare the new screenshots against descriptions. Any NEW anomalies introduced by your changes must be fixed before marking done.
  _Why: Without a post-change audit, regressions go undetected until a customer reports them._

### Testing & Verification

- **Verify the EXACT behavior path, not proxies.** After deploying a fix, test the EXACT data flow that was broken: input string → transform → parse → decision → action. Health endpoints and build IDs prove the binary is running, NOT that the bug is fixed. A 2-character difference (`"` quotes on curl output) kept all 8 pods flickering through two deploy cycles because the proxy checks (health OK, build_id correct) all passed while the actual parse path failed silently.
  _Why: Pod healer curl fix deployed twice — both times declared "fixed" based on health endpoint. The actual stdout was `"200"` (with quotes), which failed `u32::parse()` → `unwrap_or(0)` → healer still thought lock screen was down → ForceRelaunchBrowser spam continued._
- **"Removed" means removed from EVERY machine.** When removing a process, registry key, scheduled task, or config from infrastructure, verify on EVERY target: server (.23), all 8 pods, James (.27). "Removed from server" ≠ "removed from pods."
  _Why: CCBootClient was "removed" from server HKCU Run but was still in Pod 1's HKLM Run and still running. The removal was declared complete without checking pods._
- **Never move on from a failed operation.** When a command fails (quoting error, permission denied, timeout), either fix it NOW with a different approach or explicitly tell the user it's unresolved. "I'll deal with it later" = "I forgot about it."
  _Why: GoPro Webcam registry removal failed due to cmd.exe quote nesting. Moved on without resolving — it stayed in Pod 1's startup for the rest of the session._
- **Audit what the CUSTOMER sees, not what the API returns.** Check visible window titles (`tasklist /V /FO CSV`), check what's on the physical screen, check what processes have foreground windows. API health checks and process lists are internal diagnostics — the customer experience is the screen.
  _Why: 5 instances of M365 Copilot, NVIDIA Overlay, AMD DVR, Steam login dialog, visible cmd.exe windows — all overlaying the blanking screen on every pod. None detectable via health endpoints or fleet status._
- **Investigate anomalies, don't dismiss them.** `violation_count_24h: 100` on all 8 pods should have been alarming. "Expected behavior" is a hypothesis, not a conclusion — verify WHY before dismissing.
  _Why: Process guard had empty whitelist on all pods (fetched when server was down). Every process was flagged. Dismissed as "expected, report_only mode" without checking why whitelisted processes (svchost.exe) were being flagged._
- **NEVER restart explorer.exe on pods with NVIDIA Surround.** Explorer restart disrupts GPU display configuration — NVIDIA Surround drops to 1024x768 single-monitor fallback. Requires full reboot to restore. This broke 3 pods during a taskbar-hide attempt.
  _Why: `Stop-Process -Name explorer` in hide-taskbar script collapsed all triple-monitor setups from 7680x1440 to 1024x768. Required rebooting Pods 5, 6, 7 to restore._
- **Test display changes on ONE pod before fleet-wide.** Any change affecting screen resolution, blanking, kiosk mode, or explorer should be tested on Pod 8 canary first. Display issues are visually obvious but invisible to API health checks.
  _Why: Applied explorer restart to 3 pods simultaneously — all 3 broke. One pod test would have caught it._
- **Screenshot verification triggers taskbar auto-hide.** PowerShell `CopyFromScreen` causes a focus change that reveals auto-hidden taskbar. Don't use screenshot artifacts to diagnose taskbar issues — ask the user to verify physically instead.
  _Why: Taskbar was auto-hiding correctly but every screenshot showed it visible, leading to unnecessary fix attempts that broke NVIDIA Surround._
- **Fix during audit, don't just catalog.** Finding issues without fixing them creates a growing backlog. Apply the smallest reversible fix during the audit pass, then move on. Separate "investigate" from "defer" — deferred items must be explicitly communicated to the user.
  _Why: 9+ items cataloged as "investigate later" during audit — Antamedia, Salt ports, CCBoot, OneDrive, unknown ports, scheduled tasks. Most never got investigated until the user pushed._
- **Context switches kill open investigations.** When the user asks for something new, finish or explicitly park the current investigation with a clear status. Don't silently drop it.
  _Why: "Preflight checks not initiated properly, pods still blinking" was reported, then "push commit" was requested. Context-switched to committing, never came back to investigate the blinking._
- **`git log` before calling builds "old".** Different hash ≠ outdated. Always run `git log <old_hash>..<new_hash> -- <crate_path>` to check actual code changes before claiming a redeploy is needed. Docs-only commits don't change binaries.
  _Why: All 8 pods on `82bea1eb` were called "old build" — but git log showed zero functional rc-agent code changes since that commit. Pods were on the correct build._
- **SSH to Windows: use `2>nul` not `2>/dev/null`.** Remote commands via SSH to Windows pods execute in cmd.exe context. Using `2>/dev/null` causes "The system cannot find the path specified" because `/dev/null` doesn't exist on Windows. Use `2>nul` or omit the redirect entirely (capture stderr locally with `2>&1`). Same applies to `| head -1` and other Unix pipes — use `findstr` or capture output locally instead.
  _Why: 2026-04-07 — all 8 pods falsely reported as OFFLINE during deploy. The SSH command had `2>/dev/null` in the remote string. cmd.exe tried to open `/dev/null` as a file path, failed, returned exit 1. Diagnosed as "SSH alias failure" initially — wasted 5 minutes before retry without the redirect worked._
- **cmd.exe is hostile to quoting.** Any command routed through rc-agent's `/exec` endpoint goes through `cmd /C`. Strings with spaces, `$`, `"`, or `\` WILL be mangled. Use PID-based targeting (taskkill /F /PID), write batch files to the pod, or use sysinfo/Win32 APIs in Rust — avoid cmd.exe string interpretation entirely.
  _Why: `taskkill /F /IM "GoPro Webcam.exe"` fails because CreateProcessW wraps the /C arg in outer quotes → cmd.exe sees nested quotes → parse breaks. PowerShell `$r` variable stripped by cmd.exe caused the original pod healer flicker bug._

- **Verify monitoring targets against the running system, not docs.** When adding health checks, monitoring, or watchdog targets, check `netstat`, `tasklist`, and the service's own config to confirm host:port. Never copy endpoints from CLAUDE.md or documentation without verifying — they drift. A stale monitoring URL creates false alarms that erode trust in the entire monitoring system.
  _Why: AI healer checked go2rtc at .23:8096 (from stale docs). go2rtc actually runs on .27:1984. 36 consecutive false-DOWN alerts over 72 minutes. The full audit reported "go2rtc DOWN — HIGH severity" for a service that was perfectly healthy._
- **`.spawn().is_ok()` does NOT mean the child started.** On Windows, `spawn()` returning Ok only means CreateProcess was accepted, NOT that the target is running. Always verify the child process is alive after spawn — poll `/health`, check `tasklist`, or read a sentinel file written by the child.
  _Why: rc-sentry's `restart_service()` returned Ok for cmd/C start, PowerShell Start-Process, AND schtasks — all three silently failed to start rc-agent. Pods stayed dead for days because "restarted=true" was logged but never verified._
- **Non-interactive Windows context cannot launch interactive processes.** `cmd /C start`, `PowerShell Start-Process`, and `schtasks /Run` all fail when called from `std::process::Command` with `CREATE_NO_WINDOW` in a non-interactive session. The ONLY proven working path is: call through an HTTP `/exec` endpoint that uses `cmd /C` (different process creation context), or register a Windows Service with SCM.
  _Why: rc-sentry tested 3 different launch methods — all returned success, all silently failed. The same schtasks command worked via the `/exec` HTTP endpoint but not from Rust's Command::new(). Four E2E test cycles were needed to confirm this._
- **MAINTENANCE_MODE sentinel is a silent pod killer.** Once `C:\RacingPoint\MAINTENANCE_MODE` is written (after 3 restarts in 10 min), ALL restarts stop permanently with no timeout, no auto-clear, no alert to staff. Before any restart debugging, ALWAYS clear: `del MAINTENANCE_MODE GRACEFUL_RELAUNCH rcagent-restart-sentinel.txt`.
  _Why: E2E test declared "restart fix doesn't work" twice before discovering MAINTENANCE_MODE was blocking all restarts from a previous crash storm._
- **At session start, check for MAINTENANCE_MODE on all pods.** If any pod shows `ws_connected: false` + `http_reachable: false` in fleet health but responds to `ping`, check for MAINTENANCE_MODE via rc-sentry: `curl -X POST http://<pod_ip>:8091/exec -d @check-maintenance.json`. Three pods went dark for 1.5+ hours because MAINTENANCE_MODE blocked rc-agent with no alert. Recovery: clear sentinels + `schtasks /Run /TN StartRCAgent` via rc-sentry exec.
  _Why: 2026-03-24 audit — Pods 5, 6, 7 all had MAINTENANCE_MODE from a previous crash storm. Pods were powered on, rc-sentry alive, but rc-agent permanently blocked. Same timestamp on all 3 last_seen values was the clue (simultaneous disconnect = external event, not individual crashes)._
- **Audit changes must be cascade-audited before closing.** After any audit, maintenance, or bulk-fix session that modifies infrastructure (configs, firewall rules, services, scheduled tasks, registry keys, TOML files, env vars), run a cascade verification: for EVERY change made, identify all downstream consumers and verify they still work. Changes that look local often have cross-system impact.
  _Cascade checklist:_ (1) List every change made during the session, (2) For each change, identify what reads/depends on the modified file/service/port, (3) Test each downstream consumer — not just "is it running" but "is it producing correct output", (4) If a change requires a service restart to take effect (e.g. go2rtc YAML, racecontrol.toml), document that the restart is pending and what will break if skipped.
  _Why: 2026-03-25 audit — disabling process_guard on Bono required TOML edit + rebuild + restart, but sed left conflicting `enabled = false` and `enabled = true` lines (TOML uses last value = still enabled). UFW enable could have blocked comms-link WS port if not pre-allowed. go2rtc YAML moved creds to env vars but go2rtc was still running with old in-memory config — restart needed at next maintenance window. Each would have been a silent downstream failure without cascade verification._

- **Check live console, not just JSONL logs.** When checking for WARNs, also check if the server console shows live WARN output. JSONL log files may use a different tracing filter that excludes some WARN targets. Process guard violations are a known case — they flood the server console but don't appear in the JSONL file. If possible, read the last N lines of the server's stdout/stderr capture, not just the structured JSONL log.
  _Why: v17.0/v17.1 verification declared "0 WARNs" while the server console was flooding with process guard violations. The JSONL `findstr WARN` check missed them entirely._
- **Checkpoint verification must be automated, not manual.** When a plan creates a verification script/tool, the checkpoint gate MUST reference and run it — not rely on visual/browser confirmation. When delegating a list of items (DNS records, configs, installs), the checkpoint must enumerate and verify each programmatically (`dig`, `curl`, `grep` — not "open in browser"). Treat "Bono did it" the same as "code was written" — verify the output, not the intent.
  _Why: Phase 1 Cloud Infrastructure created verify-infra.sh covering all 5 INFRA requirements, but the checkpoint used browser checks. Bono created 3 of 4 DNS records (missed `api.racingpoint.cloud`). Approved without catching it because the verification script was never run._
- **MMA audit is MANDATORY before deploying new cross-system bridges.** Any new feature that creates a data flow across 2+ system boundaries (kiosk → server → pod, or pod → server → fleet) MUST have a multi-model AI audit before deploy. Single-system changes (adding a field to an existing API, fixing a bug in one crate) do not require MMA. Cross-system bridges introduce failure modes that single-implementer review structurally misses: auth boundary gaps, lock ordering across async boundaries, semantic mismatch between systems, and blast radius miscalculation. **Dual reasoning modes REQUIRED:** run both non-thinking models (find architecture bugs) AND thinking model variants (find execution-path bugs). Running only one mode leaves a blind spot that more rounds of the same mode cannot fill.
  _Why: v27.0 Staff Diagnostic Bridge — 24 bugs found across 10 MMA rounds (12 models). First 7 rounds (non-thinking) found 21 bugs in architecture patterns. Then 2 rounds with **thinking model variants** found 7 MORE bugs invisible to abstract reasoning: ErrorSpike dedup key included volatile count (never deduplicated), RwLock poisoning made log a permanent black hole, empty-string origin filter excluded nobody from fleet broadcast. These trace-level bugs only surface when a model asks "what is the value of this variable at this specific line?" instead of "is this pattern generally safe?" Both reasoning modes are needed — abstract for architecture, trace-level for correctness._
- **Audit the MONITOR, not just the MONITORED.** Every audit must verify that meta-monitoring systems (rc-watchdog, auto-detect pipeline, escalation engine) are: (1) **process running** (`tasklist`), (2) **scheduled task registered** (`schtasks /Query`), (3) **output fresh** (log recency < 5 min for watchdog, < 26h for auto-detect). Checking only the state file or code existence is proxy verification — the same class of bug as "health passes but blanking is broken." Phase 67 enforces this in Tier 1.
  _Why: 2026-03-26 — rc-watchdog died at 18:14 IST, both CommsLink-DaemonWatchdog and AutoDetect-Daily scheduled tasks were never registered, and self_patch was disabled. The audit's phase 10 checked watchdog-state.json (PASS — stale file existed) and phase 66 checked scripts existed (PASS — code was there). Neither detected that zero healing or detection was actually running. All autonomous self-debugging was silently dead._

### Unified MMA Protocol v3.0 — Operational Reference

**Full spec:** `.planning/specs/UNIFIED-MMA-PROTOCOL.md` (844 lines, approved by Uday)

**When to run MMA:**
- Before milestone ship (all models)
- After security incident (Gemini + R1 + GPT-5.4 + Sonnet)
- New crate/service (V3 + R1 + Gemini + Nemotron)
- Cross-system bridge deploy (MANDATORY)
- User requests "MMA audit" or "diagnose with MMA"

**4-Step Convergence Engine (DIAGNOSE → PLAN → EXECUTE → VERIFY):**
1. **DIAGNOSE:** 5 models identify ALL root causes (3/5 majority = consensus). Min 2 iterations.
2. **PLAN:** 5 models design fix plans for consensus findings. JSON array with actions/risk/rollback.
3. **EXECUTE:** Select best plan, apply smallest reversible change.
4. **VERIFY:** Deterministic checks FIRST, then 3-model adversarial (different models from Steps 1-3). Score ≥4.0 = PASS.

**How to run (James — OpenRouter):**
```bash
cd ~/racingpoint/racecontrol
export OPENROUTER_KEY="..."  # NEVER hardcode key here — OpenRouter auto-revokes keys found in LLM prompts. Get key from dashboard: openrouter.ai/settings/keys

# v3.0 consensus mode (DEFAULT — 5 models per batch, consensus voting, adversarial verify):
node scripts/multi-model-audit.js

# Dry run (validate model selection, no API calls):
DRY_RUN=1 node scripts/multi-model-audit.js

# Legacy single-model mode (backward compatible):
MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js

# Budget override (default $5):
MMA_SESSION_BUDGET=10 node scripts/multi-model-audit.js

# Custom diagnostic prompt (curl):
curl -s -m 120 https://openrouter.ai/api/v1/chat/completions \
  -H "Authorization: Bearer $OPENROUTER_KEY" \
  -H "Content-Type: application/json" \
  -d "$(printf '%s' "$PROMPT" | jq -Rs --arg model "MODEL_ID" '{
    "model": $model, "messages": [{"role":"user","content":.}], "max_tokens": 4000
  }')"
```

**Model Pool (Step 1 — 10 models, stratified):**

| Slot | Role (required) | Models |
|------|-----------------|--------|
| 1 | Reasoner | DeepSeek R1 0528, GPT-5.4 Nano, Kimi K2.5 |
| 2 | Code Expert | DeepSeek V3.2, Grok Code Fast, Qwen3 Coder |
| 3 | SRE/Ops | MiMo v2 Pro, Nemotron 3 Super, MiMo v2 Flash |
| 4 | Domain Specialist | Varies by domain (see spec Part 8) |
| 5 | Generalist | Qwen3 235B, Gemini 2.5 Flash, Mistral Medium |

**Vendor diversity:** Each 5-model iteration MUST include ≥1 reasoner + ≥1 code expert + ≥1 SRE. Max 2 per vendor family. Min 3 vendor families.

**Cost:** ~$2-5 per full audit (OpenRouter). Budget: $5/session unless Uday approves more.

### Unified MMA Protocol v3.0 Standing Rules (2026-03-31)

- **MMA bootstrap is env-only.** API key (`OPENROUTER_KEY`), budget limits (`MMA_DAILY_BUDGET`), and training mode (`MMA_TRAINING_MODE`) read from environment variables FIRST, then `mma.toml`, then hardcoded defaults. NEVER depend on `racecontrol.toml` for MMA core config.
  _Why: v31.0 — `racecontrol.toml` parse failure killed MMA itself. Bootstrap paradox._
- **MMA 401 auto-recovery (all scripts).** When any MMA script gets a 401 (dead key), it automatically: (1) provisions a new child key via `OPENROUTER_MGMT_KEY`, (2) falls back to Bono relay (`provision_openrouter_key` command), (3) saves to `data/openrouter-mma-key.txt`. All 8 MMA scripts support this. Shared module: `scripts/lib/openrouter-key-recovery.js`. If `OPENROUTER_KEY` env is unset, scripts auto-load the saved key.
  _Why: OpenRouter auto-revokes keys found in LLM prompts. MMA audits crashed mid-run requiring manual key rotation._
- **Manual MMA requires structured logging.** Every manual MMA session by Bono/Claude MUST log: model name, step number, cost, consensus result. Never act on 1 model for code changes — always 3+. Track cost, stop at $5/session unless Uday approves. Append to LOGBOOK.md: `| timestamp | MMA-manual | step | models | consensus | cost |`
  _Why: Manual MMA left no audit trail. 5/5 models flagged "automation theater."_
- **Vendor diversity: ≥3 vendors per 5-model step.** Max 2 models from same family. Families: DeepSeek, Meta, Google, Moonshot, Mistral, Qwen, xAI, Nvidia, OpenAI.
  _Why: Correlated hallucinations from same family produce false consensus._
- **Never skip Step 4 VERIFY.** Even for "obvious" fixes. Deterministic checks first, then adversarial model. Include semantic config validation (URLs resolve, values reasonable, API keys valid).
  _Why: Config parsed correctly but contained wrong values. Semantic validation catches "valid but wrong."_
- **Sanitize diagnostic data before MMA prompts.** Strip ANSI codes, truncate to 2000 chars, remove `sk-`, `Bearer`, `password=`, `secret=`, redact `/root/` paths.
  _Why: Diagnostic data could contain credentials or prompt injection payloads._
- **Step timeouts: 60s per model, 5min per step.** Model timeout → skip and proceed with 4. Step timeout → backtrack. 3+ timeouts → provider degraded, switch to fallback.
  _Why: No timeouts = hung protocol burns budget silently._
- **Multi-channel escalation after max backtracks.** WhatsApp + email + comms-link. If all fail after 5min → SAFE_MODE (deterministic-only, no automated fixes).
  _Why: Single-channel escalation is single point of failure._
- **Cloud ↔ Venue MMA sync is mandatory.** Every MMA rule change must be applied to BOTH environments: (1) CLAUDE.md standing rules (cloud/Bono manual), (2) `mma_engine.rs` constants/logic (venue/James automated). After any MMA amendment, verify: do CLAUDE.md rule values match `mma_engine.rs` constants? If they diverge, cloud accepts fixes venue rejects (or vice versa). See MMA-21 in spec for full sync table.
  _Why: v31.0 — `[mma]` config added for venue's rc-agent but not for cloud's racecontrol. Change made for one environment without verifying the other._

### Debugging

- **Cross-Process Recovery Awareness** — independent recovery systems (self_monitor, rc-sentry watchdog, server pod_monitor/WoL, scheduler wake) can fight each other. When adding or modifying any auto-recovery, auto-restart, or auto-wake logic, verify it won't cascade with the others.
  - A graceful self-restart must be distinguishable from a real crash (use sentinel files or IPC).
  - Escalation (e.g. MAINTENANCE_MODE) must know *why* restarts are happening, not just count them. Server-down restarts ≠ pod crashes.
  - WoL auto-wake will revive pods that entered MAINTENANCE_MODE, creating infinite loops. Any "pod offline" recovery must check whether the pod was deliberately taken offline.
  - Always test recovery paths against **server downtime**, not just pod failures.

  _Why: Self-restart + watchdog + WoL created an infinite restart loop that took 45 minutes to diagnose; the systems had no coordination._
- **Allowlist Auth — RESOLVED.** GET endpoints (`/config/kiosk-allowlist`, `/guard/whitelist/pod-{N}`) moved to `public_routes` — rc-agent fetches without auth. POST/DELETE still require staff JWT. See Security section.
  _Why: 401 on GET caused rc-agent to fall back to empty default allowlist._
- **Process guard allowlist: fetch-at-boot + 5-min periodic re-fetch (DONE in `821c3031`).** rc-agent fetches from `/api/v1/guard/whitelist/pod-{N}` at startup AND every 300s via a background tokio task. If the server is down at boot, pods get `MachineWhitelist::default()` (empty) but self-heal within 5 minutes once the server is back. Manual restart is no longer required but can be used for immediate effect: `curl -X POST http://<pod_ip>:8091/exec -d '{"cmd":"taskkill /F /IM rc-agent.exe & schtasks /Run /TN StartRCAgent"}'` via rc-sentry. Verify: `violation_count_24h` should stop increasing after the next re-fetch cycle.
  _Why: 2026-03-24 — all 8 pods showed violation_count_24h: 100 (false positives). Server had restarted, pods booted while server was briefly down, fetched empty default, and never re-fetched. Periodic re-fetch implemented same day to prevent recurrence._
- **Boot Resilience: No single-fetch-at-boot without retry.** Any data fetched from a remote source at startup MUST have a periodic re-fetch background task using `rc_common::boot_resilience::spawn_periodic_refetch()`. Single-fetch-at-boot without retry is a banned pattern — if the server is down at boot, the resource stays at its cached/default value forever. Current startup-fetched resources and their re-fetch status:
  - Allowlist (process guard): DONE — 5-min periodic re-fetch (commit `821c3031`)
  - Feature flags: DONE — 5-min periodic re-fetch via HTTP GET /api/v1/flags (BOOT-02)
  - Billing rates: CHECK — verify if billing rates have periodic re-fetch or only load at boot
  - Camera config: CHECK — verify if camera config has periodic re-fetch or only load at boot
  _Why: Feature flags were fetched once at boot via WS FlagSync and never re-fetched if WS connection failed. Server transience at boot left pods running with stale cached flags indefinitely. spawn_periodic_refetch() provides self-healing within one interval (5 min)._
- **"Shipped" Means "Works For The User"** — A milestone is NOT shipped until every user-facing endpoint is verified working at runtime:
  - Binary built, deployed, and **running** (not just compiled). All runtime dependencies present (DLLs, models, config files).
  - Every API endpoint returns correct data (not just HTTP 200 — check response content).
  - Every UI page renders and is interactive (open in browser, verify visually with screenshot).
  - Hardware integrations tested with live data (cameras, GPU inference, network devices).
  - **Frontend: verify from the user's browser, not from the server.** `NEXT_PUBLIC_` env vars are baked at build time — rebuild with correct LAN IP.
  - **Frontend: standalone deploy requires `.next/static` copied into `.next/standalone/`.** AND all `next.config.ts` files MUST set `outputFileTracingRoot: path.join(__dirname)`. Without this, Next.js embeds build-machine absolute paths in `required-server-files.json` (`appDir` field) and `server.js` (`outputFileTracingRoot`, `turbopack.root`). Pages render via SSR but ALL static files (CSS, JS, fonts) return 404 on the deployed server — the UI loads as unstyled HTML with no interactivity. After EVERY deploy, verify by curling one `_next/static/` URL — a 200 proves static serving works, a 404 means the `appDir` path is stale.
  _Why: 2026-03-25 — kiosk and web dashboard had all static files returning 404 for an unknown duration. Health endpoint showed "healthy" (it only checks page availability, not static file serving). The fix was changing `appDir` in `required-server-files.json` from `C:\Users\bono\...` (build machine) to `C:\RacingPoint\...` (deploy target). Permanent fix: set `outputFileTracingRoot` in all `next.config.ts` files._
  - **Frontend: grep ALL `NEXT_PUBLIC_` references after any env var change.** One missing var (e.g. `NEXT_PUBLIC_WS_URL`) silently falls back to `localhost` — works on the server, fails on every remote browser (POS, spectator, staff phones). After adding or modifying any `NEXT_PUBLIC_` var, run `grep -rn NEXT_PUBLIC_ src/` and verify EVERY one has a value in `.env.production.local`.
  - **Frontend: after every dashboard rebuild/deploy, verify from a machine that is NOT the server.** SSH to POS or open from James's browser pointing at `.23:3200`. `curl` to the dashboard URL proves HTML loads, not that JavaScript/WebSocket works.
  - `cargo check` and unit tests are necessary but NOT sufficient. They prove structure, not function.

  _Why: "Phase Complete" was reported 9 times based on compilation alone — runtime failures were hidden each time. `NEXT_PUBLIC_WS_URL` was never set — `NEXT_PUBLIC_API_URL` was correct so REST worked, but WebSocket defaulted to `ws://localhost:8080` causing "page loads but no data" on the POS machine for every session until caught._
- **Long-Lived Tasks Must Log Lifecycle** — Any `tokio::spawn` or `std::thread::spawn` loop must log: (a) when it starts, (b) when it processes its first item, (c) when it exits. Errors in new pipelines use `warn`/`error`, not `debug`.
  _Why: Silent task death (panic in spawned thread, channel close) went undetected for hours because no lifecycle logs existed._
- **Cause Elimination Before Fix** — Never jump from symptom to fix. Follow the 5-step Cause Elimination Process (see Debugging Methodology section): Document symptom → List ALL hypotheses → Test & eliminate one by one → Fix confirmed cause → Verify fix works. "Found a crash dump" ≠ "found the cause."
  _Why: Pod 6 game crash was attributed to Variable_dump.exe based on crash dumps alone without testing other hypotheses (RAM pressure, FFB driver, USB hardware). The fix was never verified because pods went offline. Correlation-based fixes leave the real cause unfixed 40% of the time._

### Security

- **Allowlist endpoints: GET is public, POST/DELETE require staff auth.** `GET /api/v1/config/kiosk-allowlist` and `GET /api/v1/guard/whitelist/pod-{N}` are in `public_routes` so rc-agent can fetch without auth. Write operations (POST to add, DELETE to remove entries) still require staff JWT.
  _Why: rc-agent fetches the allowlist at boot and every 5 min (periodic re-fetch added in `821c3031`). Requiring auth on GET caused 401 → empty allowlist → false violations fleet-wide._
- **Process guard safe mode:** Do not disable rc-process-guard during testing sessions — use the allowlist override instead.
  _Why: Disabling the guard entirely during a test left the machine unprotected when the session ended without re-enabling it._
- **Security gate (SEC-GATE-01) must pass before any deploy.** `node comms-link/test/security-check.js` runs 31 static assertions covering auth middleware, route auth coverage, credential leaks, protocol immutability, and deploy pipeline integrity. Integrated into: (1) `run-all.sh` Suite 4, (2) `stage-release.sh` pre-build, (3) `deploy-pod.sh` + `deploy-server.sh` pre-deploy, (4) `gate-check.sh` via Suite 0.
  _Why: Security fixes were point-in-time patches that regressed across milestones. No automated check existed to prevent new phases from adding unprotected routes, leaking credentials, or removing auth middleware. 22 milestones shipped without security regression tests._
- **Pre-commit hooks block credential leaks.** Both repos (comms-link + racecontrol) have `.git/hooks/pre-commit` that blocks: private keys, AWS keys, hardcoded passwords, sensitive files (.env.local, racecontrol.toml). Install with `bash comms-link/scripts/install-hooks.sh`. Warns on `.unwrap()` in Rust and `any` in TypeScript.
  _Why: Credentials and sensitive config files were committed to git multiple times. Pre-commit hooks prevent the leak before it enters version control._
- **Pod HTTP endpoints default to protected.** Any new endpoint on rc-agent's remote_ops server goes behind `require_service_key` middleware UNLESS there is a documented reason for it to be public. The only public endpoints are `/ping` and `/health`. Everything else (including read-only diagnostic data like `/events/recent`) requires `X-Service-Key` header. When the server proxies a pod endpoint (e.g., `/debug/pod-events/{pod_id}`), it injects the service key from config — the kiosk browser never sees the key.
  _Why: v27.0 MMA audit — `/events/recent` was initially added to public routes (copied from `/health` pattern). 5/5 models flagged this as P1 information disclosure: diagnostic history exposes internal categories, fix actions, confidence scores, and pod state to anyone on the LAN (customer phones on venue WiFi, compromised kiosks). Moved to protected routes in Round 1._
- **Deploy pipeline enforces security + manifest.** The correct workflow is: `stage-release.sh` (security pre-flight → build → SHA256 → manifest) → `gate-check.sh --pre-deploy` → `deploy-pod.sh` (security gate + manifest check) → `deploy-server.sh` (security gate + manifest check) → `gate-check.sh --post-wave`. Each step is self-verifying. Skipping any step = potential regression.
  _Why: Deploy scripts previously had no security enforcement. Stale binaries, corrupted downloads, and wrong build_id were caught only by human discipline._

### Crash Loop Detection & Recovery

- **Never restart rc-agent via `schtasks /Run /TN StartRCAgent`.** This starts the process as `NT AUTHORITY\SYSTEM` in Session 0, which cannot launch games, Edge, overlays, or any GUI. The correct restart paths are: (a) kill the process → RCWatchdog service auto-restarts in Session 1 (preferred), (b) `RCAGENT_SELF_RESTART` sentinel (requires running agent), (c) HKLM Run key on next reboot. When using schtasks via rc-sentry for deploy, verify the resulting session with `tasklist /V /FO CSV | findstr rc-agent` — Session column must show `Console`, not `Services`.
  _Why: 2026-03-26 — Pod 6 deploy restart via schtasks put rc-agent in Session 0. Server returned `ok: true` for game launches but acs.exe couldn't spawn (no GUI context). Three launch attempts wasted before discovering `Session#=0` in tasklist. The RCWatchdog service uses `WTSQueryUserToken` + `CreateProcessAsUser` specifically to avoid this._
- **Crash loop = reboot first, investigate second.** When a pod has >3 startup reports in 5 minutes (all with `uptime < 30s`), the OS or hardware is in a bad state. Don't try SSH restarts, schtasks, or binary swaps — they won't fix corrupted WMI/COM state or marginal RAM. Reboot via `shutdown /r /t 5 /f` (SSH or rc-sentry). After reboot, if crash continues: check Windows Event Viewer (`wevtutil qe Application`), run `sfc /scannow`, check WMI with `winmgmt /verifyrepository`, test RAM with `mdsched`.
  _Why: 2026-03-26 — Pod 6 had ntdll.dll access violation crash loop (exception 0xC0000005, consistent offset 0xaa83) ALL DAY. Same binary was stable on 7 other pods. Multiple SSH restart attempts (schtasks, RCWatchdog, kill+restart) all failed because the OS state was corrupt. Only a full reboot could clear it. The crash was WMI/COM-related (WMI safe mode watcher spawns at startup, COM callbacks fire at ~17s)._
- **Server must detect and alert on crash loops.** A pod sending >3 startup reports in 5 minutes with `uptime < 30s` is crash-looping. The server currently logs these at INFO level with no alerting. This must trigger: (a) ERROR log, (b) WhatsApp alert to staff, (c) `crash_loop: true` flag in fleet health response. Without this, Pod 6 crash-looped from 09:05 to 14:01 (5 hours) with zero staff notification.
  _Why: 2026-03-26 — The server received 11 startup reports from Pod 6 in 3 minutes, each with `uptime=2s`. All logged at INFO. No one noticed until a customer couldn't launch a game._
- **Game launch `ok: true` does NOT mean the agent received the command.** The server returns `ok: true` when the WS message is queued, not when the agent acknowledges. If WS drops between queue and delivery, the message is lost. The GameTracker gets stuck in `Launching` state permanently, blocking all future launches on that pod. Short-term: add a 60s timeout on `Launching` state → auto-Error. Long-term: wait for agent `GameStateUpdate::Launching` ACK before returning success.
  _Why: 2026-03-26 — Pod 6 had WS dropping every 17s. Server returned `ok: true` for 3 launch attempts. None reached the agent. GameTracker stuck in `Launching` blocked the 4th attempt with "already has a game active". Required manual `/games/stop` to clear._

### Cross-Boundary Serialization

- **Every kiosk/frontend field MUST have a matching Rust struct field.** Before shipping any kiosk wizard change, grep `buildLaunchArgs()` field names against `AcLaunchParams` struct fields. Serde silently drops unknown JSON fields (no error, no warning) — a field name mismatch means the user's selection is ignored with zero indication of failure. The API returns `{ok: true}`, the game launches, but the config is wrong.
  _Why: 2026-03-26 — Two critical bugs: (1) kiosk sent `ai_difficulty: "easy"` (string) but agent expected `ai_level: u32` (numeric). AI was always Semi-Pro. (2) kiosk sent `ai_count: 5` but agent expected `ai_cars: Vec<AiCarSlot>`. Zero AI opponents appeared. Both undetected because game launched successfully and no error was logged anywhere. Audit Protocol Phase 62 added to catch this class of bug._
- **After any kiosk wizard change, verify the generated INI file on a pod.** Trigger a test launch and read back `race.ini` / `assists.ini` from the pod. Verify: AI_LEVEL matches selection, CARS count includes AI, assists match difficulty preset. API success and game launch are necessary but NOT sufficient — the config content is what matters.
  _Why: Same incident. All existing audits (Phase 26-29, 48-50, 60) checked process state, not config content. The game ran perfectly with wrong config for an unknown duration._

### Regression Prevention

- **Every manual fix MUST have code-enforced startup verification.** If you fix a problem by changing an OS setting (power plan, USB suspend, registry key), app config ("Forced update"), or process state (killing duplicates), the fix MUST be encoded in a startup script (start-rcagent.bat, pre-flight check, or rc-agent boot sequence) that runs on every boot. Settings that aren't enforced at boot WILL regress through Windows updates, app auto-updates, deploy cycles, or pod restarts.
  _Why: ConspitLink flickering was fixed three times in the same day: (1) USB suspend + power plan + forced update set manually, (2) same settings reverted after deploy cycle, (3) process multiplication after restart with stale bat. Only the fourth fix — adding enforcement to start-rcagent.bat — stuck permanently. MAINTENANCE_MODE had the same pattern: cleared manually, came back because no code prevented re-entry. 2026-03-25._
- **Deploy cycle MUST include bat file sync.** When deploying new rc-agent/rc-sentry binaries, also deploy the current `start-rcagent.bat` and `start-rcsentry.bat` from the repo. Stale bat files on pods cause settings regression, missing process kills, and wrong startup procedures. Add bat download step to the deploy JSON chain.
  _Why: Pod 1 had a bat file missing 8 bloatware kill lines, the ConspitLink singleton guard, and the power settings enforcement. The stale bat allowed ConspitLink to multiply to 11 instances._
- **Process multiplication: always kill-all before start-one.** Any process that can be started multiple times (ConspitLink, watchdogs, PowerShell helpers) must have `taskkill /F /IM <name>` BEFORE the `start` command in the bat file. Check `tasklist | findstr <name>` count after deploy to verify singleton.
  _Why: ConspitLink accumulated 4-11 instances per pod from accumulated restarts. Each instance grabbed the HID device, causing `Bind failed` errors and visible steering wheel flickering._

### Deploy Pipeline Hardening (MMA 9-model consensus, 2026-03-29)

- **NEVER conclude "powered off" from a single failed probe.** Use `bash scripts/wait-for-pods.sh` which polls all pods with 10s intervals for up to 150s. Pods take 30-120s to boot — a 1s ping timeout proves nothing. Report `TIMEOUT — NOT assuming powered off` instead of `DOWN = off`.
  _Why: 2026-03-29 — all 8 pods falsely reported as "off" because single-shot `ping -W 1` failed during boot. Delayed deploy 15 min._
- **Start staging HTTP server with `--directory` flag, NEVER `cd && cmd &`.** Use `bash scripts/start-staging-server.sh` or `python -m http.server 18889 --directory /path/to/deploy-staging`. Git Bash `cd dir && cmd &` does NOT propagate directory to the backgrounded process. Always verify the staging URL serves a binary > 1MB before downloading to pods.
  _Why: 2026-03-29 — HTTP server served from repo root instead of deploy-staging. Pods downloaded 335-byte HTML 404 pages instead of 15MB binaries. Not detected until manual size check._
- **Validate sentry key parity BEFORE pod deployment.** Use `bash scripts/deploy-preflight.sh <hash>` which reads the key from server's racecontrol.toml and validates it against all pods via authenticated ping. If any pod returns 401, abort and fix keys.
  _Why: 2026-03-29 — server had key `a0ab7acc...`, pods had `478a3688...`. All pod exec commands returned "unauthorized". Required SSH to pod to discover the correct key._
- **Disable watchdog BEFORE killing server binary.** The deploy-server.sh script now disables `StartRCOnBoot`/`StartRCTemp` schtasks, kills watchdog PowerShell, and sets a `DEPLOY_IN_PROGRESS` sentinel before killing racecontrol. After start, re-enables watchdog and clears sentinel (including in rollback path).
  _Why: 2026-03-29 — watchdog restarted old binary before swap completed. Server showed stale build_id. Required manual PID kill + sentinel clear._
- **`.gitattributes` enforces CRLF for `.ps1`/`.bat`/`.cmd`.** Root `.gitattributes` ensures Windows scripts have CRLF on checkout. PowerShell silently fails to parse LF-only `.ps1` files with misleading "missing terminator" error.
  _Why: 2026-03-29 — james-firewall-rules.ps1 had LF endings from Write tool. PowerShell parse error required inline workaround._

### OTA Pipeline

- **Always preserve previous binary before swap.** Rename the current binary to `*-prev.exe` before placing the new one. Never delete the previous binary during the swap step. Manual rollback = rename prev back.
  _Why: Without a preserved previous binary, a failed deploy requires rebuilding from source — 5+ minutes of downtime vs 10 seconds for a rename._
- **Never deploy without a signed manifest.** Every OTA release requires a `release-manifest.toml` locking binary SHA256, config schema version, frontend build_id, git commit, and timestamp. gate-check.sh verifies the manifest exists and all fields are populated before any binary leaves staging.
  _Why: Deploying without a manifest means no SHA256 to verify against post-deploy — health checks can't confirm the right binary is running._
- **Billing sessions must drain before binary swap on any pod.** The OTA pipeline checks `has_active_billing_session()` before swapping. Pods with active sessions defer swap until session ends or checkpoint to DB. Never kill a billing session mid-transaction.
  _Why: Killing a billing session mid-race loses the customer's time and money tracking — requires manual reconciliation and erodes trust._
- **OTA sentinel file protocol.** Write `C:\RacingPoint\OTA_DEPLOYING` sentinel at OTA start, clear on complete or rollback. All recovery systems (rc-sentry, pod_monitor, WoL) MUST check this file before triggering restarts during OTA. A restart during OTA corrupts the binary swap.
  _Why: rc-sentry restarted rc-agent mid-binary-copy during an early deploy test — the binary was truncated, pod went into MAINTENANCE_MODE._
- **Config push NEVER goes through fleet exec endpoint.** Config changes use the dedicated ConfigPush WebSocket channel (CP-01). Fleet exec is for operational commands only. Mixing config into exec creates an unaudited config change path that bypasses schema validation.
  _Why: An early prototype pushed billing rate changes via fleet exec — no validation, no audit log, no ack tracking. Two pods ran different rates for 4 hours._
- **Rollback window: previous binary preserved for 72 hours minimum.** Do not clean up `*-prev.exe` files within 72 hours of deploy. Late-emerging issues (weekend traffic patterns, edge-case billing scenarios) may require rollback days after deploy.
  _Why: A billing edge case (session spanning midnight) only surfaced 36 hours after deploy. The previous binary had already been cleaned up — required a full rebuild instead of a 10-second rollback._

---

## Debugging Methodology

### Closed-Loop Debug (CLD) v1.0 — PRIMARY METHOD

**Full spec:** `docs/CLOSED-LOOP-DEBUG.md`

Every investigation starts AND ends at the layer closest to the user. 5 steps:

1. **OPEN** — Reproduce the specific symptom (screenshot for UI, curl for API, tasklist for process). NOT a health check — the EXACT behavior reported.
2. **DESCEND** — Work through 6 layers until root cause found: Smoke → Function → Boundary → Infra → Data → Code.
3. **FIX** — Apply smallest change at the layer where root cause lives.
4. **CLOSE** — Re-run the EXACT same test from Step 1. Same format, same command. If Step 1 was a screenshot, Step 4 must be a screenshot.
5. **SWEEP** — Verify ALL deploy targets (venue + cloud + pods). One machine fixed ≠ all machines fixed.

**Rule:** If the loop isn't closed (Step 4 not done), you cannot claim done. CLD Step 4 produces the evidence CGP H3 requires. CLD Step 5 produces the enumeration CGP H4 requires.

_Why: Built from 25 real incidents. Backtested: catches 22/25 bug classes. All 25 shared one root cause: investigator tested proxies (health endpoint, status code, file exists) instead of ground truth (screenshot, actual user flow, output config). 2026-04-11._

### Cause Elimination Process (MANDATORY for all non-trivial bugs)

Before fixing any bug, follow this structured process. Do NOT jump from symptom to fix.

**Step 1 — Reproduce & Document Symptom**
- What exactly happened? (user's words, screenshot, error message)
- When? What action triggered it? What was the system state?

**Step 2 — Hypothesize (list ALL possible causes)**
- Write down every plausible cause, not just the first one found
- Include: software, hardware, config, network, user error, interaction between systems
- Example (Pod 6 crash): (a) Variable_dump.exe USB disruption, (b) AC FFB driver crash, (c) RAM pressure from 15 orphan PowerShell processes, (d) VSD Craft itself, (e) USB hub/cable fault

**Step 3 — Test & Eliminate (one by one)**
- For each hypothesis, define a test that would confirm or rule it out
- Run tests in order of likelihood and ease
- Cross off eliminated causes with evidence, not assumptions
- "Found a crash dump" ≠ "found the cause" — correlation is not causation

**Step 4 — Fix & Verify**
- Apply the fix for the confirmed cause
- **Reproduce the original trigger** — verify the bug is actually gone
- Visual verification for UI/display issues (standing rule)
- If you can't reproduce (e.g. pods offline), mark as UNVERIFIED and schedule retest

**Step 5 — Log**
- Record in LOGBOOK.md: symptom, hypotheses tested, confirmed cause, fix applied, verification result

### 4-Tier Debug Order (WHERE to look)

| Tier | Method | When | Action |
|------|--------|------|--------|
| 1 | **Deterministic** | Always first | Stale sockets, game cleanup, temp files, WerFault — apply without LLM |
| 2 | **Memory** | After Tier 1 fails | Check LOGBOOK.md + commit history for identical past incident |
| 3 | **Local Ollama** | After Tier 2 fails | Query qwen2.5:3b at James .27:11434 |
| 4 | **Cloud Claude** | Last resort | Escalate — NOT auto-triggered |

The 4-Tier order tells you WHERE to look. The Cause Elimination Process tells you HOW to reason. Use both together.

### Reference Docs (CGP Standing Rule #18)

Before investigating from scratch, consult `docs/`:
- `ERROR-CATALOG.md` — Known errors indexed by symptom with root causes and fixes
- `LOG-LOCATIONS.md` — Every log file on every machine + quick debug commands
- `SERVICE-REFERENCE.md` — Per-binary deep dive: modules, config, ports, common failures
- `DATA-FLOW-DIAGRAMS.md` — 9 flow diagrams showing where data can break (incl. Mesh Intelligence)
- `ARCHITECTURE.md` — System overview: crates, topology, WebSocket protocol, recovery tiers
- `API.md` — All ~403 endpoints across 7 auth tiers

---

## Billing and Rates

- 30min / ₹700 | 60min / ₹900 | 5min free trial | 10s idle threshold
- PWA shows "credits" (not rupees)
- Wheelbases: Conspit Ares 8Nm — OpenFFBoard VID:0x1209 PID:0xFFB0
- UDP telemetry ports: 9996 (AC) | 20778 (F1 25, ADAPTER-SWAP-06) | 5300 (Forza) | 6789 (iRacing) | 5555 (LMU)

---

## Brand Identity

_Canonical sources: colors → `packages/shared-tokens/tokens.css` (`--rp-*` tokens, both web and kiosk import this). Fonts → `kiosk/src/app/globals.css` `@theme inline` block (kiosk = unified-theme reference). Logos → `brand-assets/logos/` (Racing Point lockup PNG preserved 2026-05-08 from Emergent CDN; future variants land here; see `brand-assets/README.md`). Full V2 design substrate → `comms-link/v2-skeleton/10-ui-design-system.md`. RATIFIED 2026-05-08 IST per Captain disposition; supersedes May 2 design brief._

- Racing Red: `#E10600` | Asphalt Black: `#1A1A1A` | Gunmetal Grey: `#5A5A5A`
- Card: `#222222` | Border: `#333333` | Surface (elevated): `#2A2A2A` | Red-hover: `#FF1A1A`
- Fonts: Montserrat (body, 400/500/600/700), Orbitron (display, 500/700/900)
- Tailwind utility prefix: `rp-*` (e.g. `bg-rp-red`, `text-rp-grey`, `border-rp-border`)
- OLD orange `#FF4400` is DEPRECATED — do not use
- Enthocentric (display) DEPRECATED 2026-05-08 — never shipped, replaced by Orbitron canonical in kiosk

---

## Doctrine Conventions

### Substrate-Pointer Convention (extends comms-link/CLAUDE.md Network Identity precedent)

**When this CLAUDE.md cites a fact that has a code/file source-of-truth, the citation MUST include `(canonical: <path>)` — or be wrapped in an italicized "Canonical sources:" line above the fact list.**

Existing applications:
- Brand Identity → `(canonical: packages/shared-tokens/tokens.css for colors; kiosk/src/app/globals.css for fonts; comms-link/v2-skeleton/10-ui-design-system.md for full V2 substrate)` — applied 2026-05-08
- Network Identity → `comms-link/CLAUDE.md` "Network Identity" section (already enforced; original precedent 2026-05-03 G9 #3 IP-drift class)

Candidate applications (annotate as the surface arises; do not pre-annotate speculatively per kaizen-discipline):
- Crate Names → `(canonical: workspace Cargo.toml)`
- Service Ports → `(canonical: <relevant config file>)`
- Billing rates → `(canonical: <pricing source-of-truth>)`

**Why:** This CLAUDE.md is loaded into every session as system prompt context. Without a substrate pointer, the agent treats the summary as authoritative and produces derived artifacts (briefs, plans, config) using the summary's vocabulary — which may diverge from canonical. Pattern observed 8-9 times within 24h across two sessions on 2026-05-08, all same META class (passive-memory-vs-environment in derived-artifact authoring). See `~/.claude/projects/C--Users-bono/memory/feedback_emergent_directed_spend_protocol.md` META-class extension for analysis.

**Composes with:** Verify-Before-Generate (2026-04-11) · Rule 0 — Enumerate Before Asserting (v4.4) · directed-spend Rules 1-4 · `comms-link/CLAUDE.md` Network Identity (sibling pattern, IP-class-specific).

**Scope:** Documentation convention. Not hook-enforced (yet). If recurrence persists, escalate to hook enforcement (Recommendation #2 in protocol META-class extension).

**Stale-at:** Re-evaluate after 10 sessions or 2026-05-22 (whichever first). If META class continues to fire ≥1×/session despite this convention, escalate to hook enforcement.

---

## Security Cameras

- 13x Dahua 4MP. Auth: `admin` / `Admin@123`, RTSP `subtype=1`
- NVR: .18 | Entrance: .8 | Reception: .15, .154
- People tracker: port 8095, FastAPI + YOLOv8, entry/exit counting

---

## Key File Paths

| Path | Purpose |
|------|---------|
| `C:\RacingPoint\racecontrol.toml` | Server config (on server .23, user: ADMIN) |
| `C:\RacingPoint\start-racecontrol.bat` | Server start script (HKLM Run) |
| `C:\RacingPoint\start-rcagent.bat` | Pod agent start script (HKLM Run on each pod) |
| `C:\Users\bono\racingpoint\deploy-staging\` | Build staging area + HTTP server (James) |
| `C:\Users\bono\racingpoint\deploy-staging\webterm.py` | Web terminal (Uday's phone → :9999) |
| `C:\Users\bono\racingpoint\comms-link\INBOX.md` | James→Bono comms channel |
| `D:\pod-deploy\` | Pendrive deploy kit (install.bat v5) |
| `LOGBOOK.md` | Incident + commit log at repo root |
| `COGNITIVE-GATE-PROTOCOL.md` | CGP v4.3 "Backlog Gate" — 5 hard gates (H1-H5, hook-enforced) + 5 soft gates + 15 Standing Rules + ecosystem-wide scope (consolidated from v3.6's 10 gates / 147 standing rules) |
| `.planning/specs/UNIFIED-MMA-PROTOCOL.md` | Unified MMA Protocol v4.0 — full spec: 4-step convergence engine (DIAGNOSE/PLAN/EXECUTE/VERIFY) ≥5 models per step / ≥3 vendor families / max 2 per vendor / $5/session cap; v4.0 adds machine-enforced step sequencing via shared/openrouter.js validateMmaStep() / validateStepSequence() |
| `.cargo\config.toml` | Static CRT build config |

---

## Development Rules

- No `.unwrap()` in production Rust. No `any` in TypeScript. Idempotent SQL migrations.
- Static CRT: `.cargo/config.toml` `+crt-static` — eliminates vcruntime140.dll on pods
- Git config (per-repo): `user.name="James Vowles"`, `user.email="james@racingpoint.in"`
- Cascade updates (RECURSIVE): when changing a process, update ALL linked references AND their downstream consumers recursively. See Standing Rules > Code Quality for full checklist
- LSP: rust-analyzer enabled in settings.json
- Next.js hydration: never read `sessionStorage`/`localStorage` in useState initializer — use `useEffect` + hydrated flag
- `.bat` files: NEVER use parentheses in if/else blocks — use `goto` labels. Test with `cmd /c` before deploying.
- Git Bash JSON: write JSON payloads to file with Write tool, then `curl -d @file` (bash string escaping mangles `\\`)
- `start` command: always use `/D C:\RacingPoint` to set CWD (rc-agent uses relative `rc-agent.toml`)

---

### Ecosystem Audits

- **Manifest-driven audits (CGP Standing Rule #14).** Before claiming any audit is complete, load `ECOSYSTEM-MANIFEST.json` and verify every `critical: true` system was checked. List skipped systems explicitly. "Audited everything" without manifest verification = H4 violation.
  _Why: 2026-04-04 ecosystem audit missed rc-sentry, rc-sentry-ai, people-tracker, go2rtc, and 5 MCP servers. Organized by functional groups instead of exhaustive directory enumeration. The camera system — a critical production component — was entirely unaudited._
- **Seed findings into Meshed Intelligence (CGP Standing Rule #15).** After any code audit, seed findings via `POST /api/v1/mesh/audit-seed` (or directly into `audit_known_issues` table). MI Tier 0 checks this table BEFORE running Tier 1-4 AI diagnosis, preventing wasted Ollama/OpenRouter credits on known code bugs. Format: `{"findings": [{"problem_key": "...", "severity": "P0", "symptom_patterns": ["keyword1","keyword2"], "root_cause": "...", "fix_action": "...", "fix_status": "code_fixed|pending", "affects": ["pod_1",...], "escalation_message": "..."}]}`.
  _Why: MI had no way to know about code-level bugs found by audits. Would waste AI credits trying to diagnose runtime symptoms of unfixed code bugs. Tier 0 short-circuits with escalation message instead._
- **ECOSYSTEM-MANIFEST.json** — machine-readable list of every system. Located at repo root. Update when adding new systems, crates, or services.

## Capability claim without probe — 3-probe rule for reach + N≥2-spaced for service-state (bono 2026-05-14 IST · ACTIVE N=2 · BILATERAL via Captain auth)

Before claiming any capability OR service-state about a target X (`cannot reach`, `no access`, `is down`, `not running`, `not listening`, `out-of-reach`, `unreachable`, `is offline`, `is missing`), pilots MUST run sufficient probes — these are evidence-class assertions, NOT memory-projections. Two probe regimes by claim shape:

**(a) Reach claims** — boolean steady-state property (does network/auth/path exist?). Require **3-probe set**: (1) `tailscale status | grep <host>` (network layer); (2) `curl http://<ip>:<port>/<health-path>` (transport layer); (3) `ssh -o BatchMode=yes -o ConnectTimeout=5 <user>@<ip> 'echo OK'` (auth layer · read-only). ALL 3 must fail with diagnostics cited before declaring "cannot reach". ANY 1 succeeding → revise claim to layer-precision ("have reach / lack auth" not "cannot reach").

**(b) Service-state claims** — temporal property (process running NOW? port listening NOW?). Require **N≥2 probes spaced ≥30s apart with consistent result** before declaring "UP" / "DOWN" / "RUNNING" / "NOT-RUNNING" / "LISTENING" / "CRASHED". Windows binary cold-start budget = 30-120s (SQLite WAL replay + cloud_sync handshake + port bind on Server .23 racecontrol specifically). Probing at t+5s during a restart cycle catches mid-startup transient, NOT converged state. Probe-output language: report "observed at t: X" not "service IS X" — reserve "is X" for converged ≥2-probe-agreed measurements.

**Why:** *Reach* (can-the-network-deliver) is independent from *ownership* (whose-operator-role-is-it) and *authorization* (am-I-allowed-to-execute). Conflating these three layers into a single denial wastes Captain round-trips and exposes false-negatives in delegation. *State* is temporal; absence at moment t ≠ absence over [t-30s, t+30s].

**Empirical anchors (both same-session 2026-05-14 IST · §S-297 audit-trail):**

- **Anchor #1 ~07:00 IST** — bono falsely claimed *"❌ Cannot SSH to Server .23"* without probing tailnet; Captain corrected; single `tailscale status` cmd revealed reach already established (69MB↔402MB bono↔.23 traffic flowing on `100.125.108.37 racing-point-server-1` tailnet node). Root cause: projected memory ("Uday manual" operator-ownership) into capability claim ("unreachable"). HTTP probe `:8080/api/v1/health` returned 200 OK build_id `61999f58` from VPS.
- **Anchor #2 ~08:24 IST** — during racecontrol .bat-respawn cold-start window of failed cross-compile deploy attempt (§S-297), bono falsely claimed *".23 racecontrol DOWN"* (production-emergency-escalation) from single-snapshot probe. Captain corrected *".23 is not offline"*. Re-probe at 08:37 IST: PID 7744 / build_id 61999f58 / 9/9 pods CONNECTED / cloud_sync 17s ago. The OLD binary HAD completed cold-start during the false-DOWN report window. Sub-class: `service-state-via-single-snapshot-claim` — racecontrol binary cold-start (SQLite WAL replay + cloud_sync handshake + bcrypt init + port :8080 bind) takes 30-120s observably.

**Composes-with:** [[amplifier-discipline-rubric]] + [[branch-state-mutation-by-parallel-pilot]] (fabrication-class memory-failure family · assumption-of-clean-substrate sibling rules) · CGP H3 EVIDENCE BEFORE CLAIMS (extends to negative capability + transient state claims) · §S-272 RCA §7 Server .23 deploy runbook + §S-297 audit trail · Universal-Sync sub-rule (this section IS the racecontrol/CLAUDE.md target of that sync).

**Structural fix promotion:** memory-only fix HELD-PENDING-N=3 trigger per Captain ratify 2026-05-14 ~08:46 IST verbatim *"Wait one more anchor (N=3 trigger) before installing the hook — the N=2 → ACTIVE state codification + my own awareness from authoring §S-297 may itself be sufficient discipline"*. `pre-bash-cannot-claim-check.js` UserPromptSubmit hook DESIGNED-NOT-STAGED. 3rd anchor within 30d → hook install justified; no recurrence by stale-at 2026-06-13 → class expires.

**Empirical testability:** if I emit another false-capability-claim within 30d, that's evidence the memory-only fix is *insufficient* and hook is justified. If I don't, that's evidence the memory-only fix IS sufficient.

**Canonical memory:** `/root/.claude/projects/-root/memory/feedback_capability_claim_without_probe_20260514.md` · MEMORY.md L1151 index entry ⭐⭐ PROMOTE-N=2-ACTIVE · §S-297 V2-MASTER-STATE close-anchor `18dc90b5`. Bilateral parity: harness `~/.claude/CLAUDE.md` (landed Captain-auth this session 2026-05-14 ~08:50 IST) + comms-link/CLAUDE.md (landed bilateral this session) + this racecontrol/CLAUDE.md section (landed bilateral this session). james-side parity pending his own harness Captain auth.

## Branch-state-mutation hook v0.1.0 — runtime enforcement of branch-verify-before-destructive-git (Captain 2026-05-14 IST HOOK-INSTALL-BUNDLE)

`pre-bash-destructive-git-branch-check.js` v0.1.0 — PreToolUse Bash hook in V-LBAC shared working trees (`/root/racecontrol`, `/root/comms-link`). Intercepts destructive git ops (commit/push/reset/checkout/branch/rebase/clean/restore/merge/cherry-pick); prints branch+head+status context to stderr (advisory); BLOCKs two tripwire patterns: (1) force-push on protected branches (main/master/develop) — N=5 BILATERAL ACTIVE branch-state-mutation class would silently push parallel-pilot branch state to canonical main; (2) hard-reset in shared trees — can discard parallel-pilot commits with no diagnostic. Other destructive ops emit ADVISORY (exit 0) with full branch context so silent-flip is structurally impossible.

**Bypass (audited):** `DESTRUCTIVE_GIT_BYPASS=1 DESTRUCTIVE_GIT_BYPASS_TS=$(date +%s)` paired — 15-min TTL, every use logged to `~/.claude/state/branch-check-audit.jsonl`. Anti-pattern blocked: shell-rc sets bypass var permanently — TTL forces re-set every 15min, ambient state structurally infeasible.

**Empirical anchors (N=5 cumulative session arc 2026-05-14):** (1) parallel-bono §S-272 wrong-branch commit; (2) bono §S-273 branch-name silent-flip via bilateral live-sync; (3) /root/comms-link working tree state mutation from parallel-bono shared substrate ops; (4) /root/racecontrol silent-flip to PR-B branch head `98e70925` during §S-291 VPS rebuild; (5) shared-working-tree class detected during §S-281 cascade authorship. **Anchor #6 same install ~03:57Z (this commit's own author state):** /root/racecontrol working tree on PR-B feature branch when bilateral-sync of capability-claim section was attempted — discovered via grep miss; resolved via /tmp/rc-main worktree path. The hook just installed would have caught any ambient `git commit` op via ADVISORY display.

**Smoke-test evidence (6/6 behaved-as-designed 2026-05-14 ~03:53Z):** TEST-1 commit ADVISORY exit=0 · TEST-2 force-push-main BLOCK exit=2 · TEST-3 hard-reset BLOCK exit=2 (incidentally exposed /root/racecontrol on PR-B branch — hook surfaced parallel-pilot mutation before any op) · TEST-4 ls-tmp out-of-scope no-op · TEST-5 status non-destructive no-op · TEST-6 BYPASS-env-allow exit=0.

**Known v0.1.0 FP class (iter1-candidate DEFERRED):** substring detection matches destructive-op tokens inside heredoc/python-c/bash-c quoted-string bodies. iter1 fix: AST-class tokenization (stripHeredocs + quoted-string skip) same class as `pre-harness-auth-gate.js` v0.2.0 I-1 Bash AST resolution. Bypass-env path works as designed pending iter1.

**Canonical:** `~/.claude/hooks/pre-bash-destructive-git-branch-check.js` v0.1.0 (137 lines) · audit log `~/.claude/state/branch-check-audit.jsonl` · HOOK-INSTALL-BUNDLE ledger entry `2026-05-14T03:52:10Z` op=install · `~/.claude/CLAUDE.md` "Branch-state-mutation hook v0.1.0" section · comms-link/CLAUDE.md bilateral mirror · this racecontrol/CLAUDE.md section · `feedback_branch_state_mutation_parallel_pilot_20260514.md` memory file flipped STAGED-PENDING → INSTALLED. james-side parity requires his own harness Captain auth.

## Current Blockers

- v6.0 blocked on BIOS AMD-V (SVM Mode disabled on server Ryzen 7 5800X) — does not affect v9.0
- ~~Gmail OAuth tokens expired~~ — RESOLVED 2026-03-22
- Pod 6 UAC prompt (2026-03-16) — unknown install request, under investigation
- USB mass storage lockdown pending (Group Policy)
- Server DHCP reservation needed: MAC 10-FF-E0-80-B1-A7 → 192.168.31.23
- Server .23 Node v24.14.0 should be downgraded to v22 LTS at next maintenance window (no runtime impact — build-only)
- Process guard in `report_only` mode — monitor 24-48h then switch to `kill_and_report`
- Server .23 Tailscale re-authenticated under `james@` (node `racing-point-server-1`, IP 100.125.108.37). Old `bono@` node (`racing-point-server`, 100.71.226.83) is stale — remove from Tailscale admin console
