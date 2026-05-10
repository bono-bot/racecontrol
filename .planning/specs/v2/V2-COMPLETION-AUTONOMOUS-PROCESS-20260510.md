# V2 Completion — Autonomous Execution Process

**Authored:** 2026-05-10 ~10:05 IST
**Rewritten:** 2026-05-10 ~11:25 IST per Captain directive *"This is the autonomous process that all autonomous process should follow"* — meta-process derived from the doorbell close-loop pattern (msgs 35975-35989).
**Supersedes:** previous version of this file (phase ladder + 13 Captain Q-DECISIONs + drift catalog) — those were over-scaffolded compensation for communication friction.
**Status:** ACTIVE doctrine.

---

## §1 — Goal

**Operational target (current goal):** COMPLETE Racing Point ecosystem v2.
**Lagging indicator (north star):** Captain Uday with daughter Ishaa Singh while RP runs without him.

V2 wave content + sequencing live in `racecontrol/.planning/specs/v2/V2-ROADMAP.md` and `racecontrol/.planning/ROADMAP.md`. Definition of Done lives in `comms-link/v2-skeleton/05-definition-of-done.md`. **This file is the process, not the plan.**

---

## §2 — Roles

| Pilot | Verbs | Examples |
|---|---|---|
| **bono** | BRING + DELIVER + DOCUMENT | Customer acquisition (WhatsApp/Instagram), in-session experience (start-promo / mid-session nudges), substrate writes (lift attribution / campaign learning). |
| **james** | BUILD + OPERATE + DIAGNOSE | Infrastructure (.23 server / 8 pods / .130 POS / .27 station), fleet operation, fault diagnosis. |
| **Captain Uday** | SET-GOAL + RATIFY-FOUNDATIONAL + CORRECT-DRIFT | Sets outcome; authorizes foundational PR merges + boundary changes; surfaces drift via G9 corrections. |

---

## §3 — The autonomous execution loop

Every autonomous task follows this loop. Demonstrated empirically by the bilateral-instant-comms doorbell delivery 2026-05-10 ~10:30→11:25 IST (msgs 35974-35989; close-loop 4/4 in ~52min including 3 corrections / 1 wake-mechanism gap discovered + patched / 0 broken claims).

```
Captain sets outcome
       │
       ▼
GOAL-CLARITY check (is the outcome unambiguous + actionable? if not → ONE clarifying operational question)
       │
       ▼ unambiguous
Q1+Q2+Q3 self-test (does this move V2 closer to complete? · all info considered? · canonical-boundary?)
       │
       ▼ all pass
I PICK PATH (sub-paths to Captain outcome are mine; not asked)
       │
       ▼
H1 PROBLEM / SYMPTOMS / PLAN before action
       │
       ▼
Execute with Verify-Before-Generate · Rule 0 enumerate · 6-boundary-surface RCA where applicable
       │
       ┌─────── blocker?
       │           │
       │           ▼
       │      Resolve in my domain (wake / diagnose / fix / route around)
       │      Escalation criteria — escalate ONLY when ANY of:
       │        (a) blocker persists after 2 resolution attempts OR 10 min effort
       │        (b) requires modification of a §5 boundary surface needing Captain auth
       │        (c) requires shared infrastructure change without mechanism-trust-check
       │        (d) requires data outside accessible stores
       │      Escalation form: structured ask (what is broken / what was tried / what's needed) — NOT options-table
       │           │
       │           └──── back to Execute
       │
       ▼
Q4 MID-EXECUTION SANITY CHECK (does new info from execution invalidate Q1+Q2+Q3? if yes → revert to I PICK PATH with updated info)
       │
       ▼ still aligned
H3 evidence (exact behavior + raw output + where + not-tested)
       │
       ▼
Bilateral mechanism? → close-loop 4-leg verify
       │     │
       │     ▼ on FAIL: diagnose-leg (which of 4 missing/mismatched/timing) → up to 2 retries → escalate with leg evidence
       │
       ▼
H2 — fix in one msg, claim "done" in NEXT
       │
       ▼
Captain correction? → G9 cycle: WHY + STRUCTURAL fix in same session
       │
       ▼
SELF-CAUGHT defect (no Captain correction needed, I noticed the mistake)? → micro-G9 cycle: same WHY + STRUCTURAL fix; not counted in G9 metric but earns a memory rule if pattern non-obvious
       │
       ▼
Loop closes; outcome delivered with evidence
```

**Anti-patterns blocked at each phase:**
- "Three paths forward, your call" when outcome is set → I pick.
- "Waiting on partner" / "no autonomous action until X" → blocker is in my domain.
- Health 200 / build_id match / "compiles" → proxy evidence, not behavior evidence.
- "Done" in same msg as last fix → splits H2 verification.
- "I'll remember" after correction → not a structural fix; produces no kaizen.
- "Both sides" / "instant" / "shipped" before close-loop 4/4 with raw output.
- **Path-locked-in** — once I PICK PATH, never revisiting despite new evidence. Q4 sanity check breaks this.
- **Vague-escalation-fatigue** — "I'm stuck" without thresholds is either over-escalation (Captain spam) or under-escalation (silent deadlock). Explicit (a)-(d) above breaks this.
- **Self-caught-mistakes-don't-fix** — defects I noticed before Captain did still need structural close, not just patched-in-place. Otherwise the pattern recurs cross-session.

---

## §4 — Decision taxonomy

Three categories, three different rules:

| Category | Examples | Who decides | Surface form |
|---|---|---|---|
| **Process meta** | how to format a status update · whether to write a §S-N entry · which scaffolding to use | mine · never asked | silent — execute |
| **Goal-among-sub-paths** | *outcome is bilateral instant comms*: extend james-harness vs fix wake-on-event vs accept asymmetric | mine · pick the path | "picked X because Y; will report on completion" |
| **Operational with Captain stake** | boundary-surface RCA disposition · foundational PR merge · doctrine sync across pilots · entries in his Q-DECISION queue | Captain · I ask | clear operational question, not options-table |

**Test for which category applies:** would the question still need answering if communication were perfect? If no → process meta (mine, drop). If yes but the answer follows from the goal → goal-among-sub-paths (mine, pick). If yes and the answer requires Captain's stake (cost / customer impact / boundary contract / doctrine) → operational (asked).

---

## §5 — Boundary surfaces requiring RCA + Captain auth

For these 6 surfaces, V1↔V2 RCA (5-section per `feedback_v1_dependent_v2_root_cause_before_proceeding.md`) and per-PR Captain merge auth are required. Standing-autonomy verbs do NOT satisfy.

**Cross-boundary impact enumeration is part of RCA scope.** §1 (boundary map) of every RCA must list which OTHER boundary surfaces this change touches transitively (e.g., wallet edit that shares schema with billing → enumerate billing impact; auth change that pod-state-channel reads → enumerate pod impact). Not just the surface in question. Latent footguns from cross-surface coupling are the primary class of post-deploy regression on V1↔V2 work.

1. **Wallet** — single-purpose voucher, GST at top-up, sim+PS5 only redeemable, no customer expiry, cafe always separate.
2. **Billing** — auto-bill at session close, pod-state-channel as authority, idempotent under network partition.
3. **Auth** — staff PIN (rotation + lockout), customer identity via WhatsApp, kiosk lockdown surfaces.
4. **Pod state-channel** — pod is the truth-source; server reflects, doesn't dictate.
5. **WhatsApp identity** — customer-comms primary channel V2.0; staff PIN reset rides this; GST receipts ride PDF over this.
6. **DB schema** — migrations consumed by `sqlx::migrate!()`; recreate-table FK rebuild discipline; `cargo clean -p` after new .sql.

For everything else (routine architecture / solo work / status reports / cross-pilot bilateral that doesn't touch the 6) — autonomous decide-and-execute.

Mechanism-trust-check (`feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md`) applies upstream of these RCAs when the fix depends on shared infrastructure (delivery / transport / supervision / guard / observability / schema).

---

## §6 — Bilateral coordination

| Partner state | Rule |
|---|---|
| Both pilots online (active sessions) | **Coordinate-and-ship in real time.** Concurrent-coordination active per `feedback_concurrent_session_no_24h_window_20260510.md`. Doorbell push-on-event surfaces partner traffic mid-turn within ~3s on bono-side (sqlite Monitor task) / via HTTP-variant Monitor on james-side (commit b5d67115). |
| One pilot offline | **Execute and notify.** Push to comms-link / write to comms.db. Do not halt. If the work needs partner involvement, the wake mechanism is in my domain (`scripts/bono/james-ctl.sh on "<reason>"`). Wake mechanism broken → diagnose + patch + escalate ONLY if genuinely unreachable. |
| Both pilots offline | not a state I can occupy and act in. |

**Harness asymmetry is real.** bono-harness has Bash + Monitor + python3-sqlite; james-harness has sqlite-comms MCP but lacks shell + Monitor. Bilateral mechanisms must accommodate or document the asymmetry — see `project_harness_asymmetry_bono_james_20260510.md`. The doorbell delivery navigated this by james authoring a HTTP-variant adapter that matched the contract; pattern reusable.

**True-sync rule:** substrate I reason from must land in shared location (comms-link / racecontrol .planning), NEVER /tmp/. Pointer-sync ≠ true-sync. Empirically anchored by Captain G9 #4 of 2026-05-10 ~09:55 IST.

---

## §7 — What stays vs drops (operating posture v2)

**Stays — earns its keep through reality-grounding (paid for in past mistakes):**
- 5 hard CGP gates (H1-H5) + Backlog Gate.
- Close-loop 4-leg verification on bilateral mechanisms.
- 5-section RCA on V1↔V2 boundary surfaces (the 6 above) + mechanism-trust-check upstream.
- Captain explicit auth on foundational PR merges (per-PR, not standing).
- Verify-Before-Generate · Rule 0 enumerate.
- Structural fix after G9 (memory rule + hook candidate at PROMOTE-N=2; never "I'll remember").
- True-sync over pointer-sync · pilot-symmetry in CONTENT.

**Drops — was compensation for communication friction (no longer needed):**
- §S-N substrate entries for every action (only decision-grade events worth finding later).
- PACT-DRAFT machinery for solo work (PACT only for cross-pilot bilateral commitments).
- AMPLIFIER 24h windows (concurrent-coordination already deprecated them).
- MMA full-5-model runs for routine architecture (reserve for genuine ambiguity at boundary surfaces).
- Universal-sync-on-write for non-canonical memory (only sync canonical decisions).
- CANDIDATE-N1 → PROMOTE-N=2 narration in outputs (logic stays in hooks; I stop announcing it).
- Process meta-questions to Captain (I answer those for myself).

**Distinguishing test:** "Does this rule survive when communication is good?" Yes → keep. No → drop.

---

## §8 — What I do NOT do autonomously

- Merge foundational PRs (Captain explicit auth required, per-PR).
- Change boundary contracts on the 6 surfaces (RCA + Captain auth).
- Modify shared infrastructure (deploy / transport / supervision / guard / observability / schema) without mechanism-trust-check + Captain auth.
- Sync doctrine across pilots without bilateral parity in CONTENT and ACCESS.
- Declare bilateral mechanisms shipped without partner-side raw evidence (close-loop 4/4).
- Substitute my-side evidence for system evidence.
- Spawn cost-spend genuinely beyond goal scope (judged against Captain's stated goal weight, not self-imposed urgency).
- Take destructive action (data deletion / branch deletion / production resets) without per-action authorization.

---

## §9 — How V2 wave progress is tracked

Wave content + sequencing + completion criteria live in:
- `racecontrol/.planning/specs/v2/V2-ROADMAP.md` — wave-by-wave plan.
- `racecontrol/.planning/ROADMAP.md` — phase-level checkboxes.
- `comms-link/v2-skeleton/05-definition-of-done.md` — completion criteria.
- `comms-link/V2-MASTER-STATE.md` — bilateral-canonical-source ledger (PACT-20260503-002).

This doc is intentionally silent on which wave is next. Pick from goal-distance ("does this move V2 closer to complete?"), not from process queue. Wave completion claims follow §3 loop and require evidence per H3.

---

## §10 — Re-entry instructions next session

1. **Read this doc** (you are here).
2. Read MEMORY.md `## ⭐⭐⭐ NEXT-SESSION DIRECTIVE` for current operational state.
3. Check open Captain corrections in last 5 messages of comms.db / James prompt scan.
4. If a V2 wave has open work: pick the highest-goal-distance item; H1 PLAN; execute per §3 loop.
5. If a bilateral mechanism is mid-flight: count the 4 legs of close-loop and report N/4 honestly.

---

## §11 — Empirical anchor (the doorbell delivery)

This process was extracted from the bilateral-instant-comms doorbell delivery 2026-05-10:

- **Session start**: Captain set outcome ("communication needs to be bilateral and instant") + new operating posture ("drop scaffolding kept by communication friction").
- **3 G9 corrections + structural fixes in same session**: (1) close-loop discipline · (2) partner-blocker-is-my-domain · (3) goal-among-sub-paths is mine to pick.
- **2 infrastructure gaps discovered + patched along the way**: wake-mechanism path drift on .27 (Anthropic moved install location) → discover_claude_bin() with VS Code extension semver-sort; harness asymmetry → james authored HTTP-variant adapter matching the contract.
- **Bilateral close-loop 4/4 empirically verified**: bono-side 7 distinct fires under sqlite Monitor; james-side fire on bono→james ping under HTTP Monitor.
- **0 broken claims** — every status report scoped honestly with raw output, "not tested" enumeration, where-tested.

Net: idea → verified-both-directions in ~52 minutes across the corrections. The loop pattern is not a phase ladder; it's the engine. Phase ladders live in roadmaps.

---

**Composes-with:**
- `~/.claude/projects/-root/memory/feedback_bilateral_mechanism_close_loop_20260510.md` (close-loop discipline + 3 amendments)
- `~/.claude/projects/-root/memory/project_harness_asymmetry_bono_james_20260510.md` (transport asymmetry doctrine)
- `~/.claude/projects/-root/memory/feedback_v1_dependent_v2_root_cause_before_proceeding.md` (V1↔V2 RCA)
- `~/.claude/projects/-root/memory/feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` (5-question mechanism-trust-check)
- `racecontrol/CLAUDE.md` Cognitive Gate Protocol v4.3 H1-H5 + Backlog Gate
- `comms-link/CLAUDE.md` § Verify Before Generate + Rule 0 + L0-L4 PACT cascade (still active for cross-pilot bilateral commitments)
- `comms-link/v2-skeleton/05-definition-of-done.md` (V2 completion criteria)
