# V2 Completion Autonomous Process — Step-by-Step Plan

**Authored:** 2026-05-10 ~10:05 IST · bono · per Captain commission "Lets created a step by step autonomous process to thoroughly complete the ecosystem"
**Status:** PROPOSAL — Phase 0 ratifications required before execution starts
**Composes-with:** racecontrol/.planning/specs/v2/V2-ROADMAP.md (authority on phase content); v2-skeleton/05-definition-of-done.md (completion criteria); MMA outputs at comms-link/mma-out/ 2026-05-10
**Bilateral-accessible:** yes (racecontrol/ is dual-pilot accessible per true-sync rule)

---

## §0 — Goal anchor (Captain 2026-05-10 corrections)

**Current goal (operational target / leading indicator):** COMPLETE Racing Point ecosystem v2.

**North star (lagging indicator unlocked when V2 complete):** Captain Uday with daughter Ishaa Singh while RP runs without him.

**Q1 self-test for every autonomous action in this process:** *"Does this move V2 closer to complete?"* If yes → Q2 (info coverage) → Q3 (canonical-boundary). All 3 pass = autonomous proceed. Any fail = stop and surface.

**Constitutional optimization stack (priority order):** Failure rate · Customer experience · Captain step-away viability (8h target) · Staff dependency reduction · Integration over silo-ing · Multi-revenue surface utilization · Capacity utilization.

---

## §1 — Current V2 state inventory (2026-05-10 ~10:00 IST)

**Wave-level state:**

| Wave | State | Owner | Notes |
|---|---|---|---|
| Wave 0 (audit) | gates on `pre-vms-duplicate-check.js` PROMOTE-ACTIVE | bono shipped CANDIDATE-N1 §S-191 | Wave 0 prereq #1 per §S-172.7 |
| Wave 1 (Foundation) — W1-S1..S4 | CLOSED via james §S-167 commits 0386db62 + 85caecd1 + 59432d4b + 16 unit + 8 integration tests | james | done |
| Wave 1 — W1-S5 | RCA `15490644` shipped; PR-C authoring pending | james | gates on Captain G33 v7 explicit-fire-auth |
| Wave 1 — W1-S6 | RCA `ff26b502` shipped; PR-A authoring HALTED | james | gates on Captain G33 v7 explicit-fire-auth (foundational; per-PR Captain merge auth required per V1-dependent V2 RCA rule) |
| Wave 2 (Identity / Multi-profile PWA / Wallet / Billing) | DRAFT | per V2-ROADMAP | gates on Wave 1 closure |
| Wave 3 (Wallet HRC 2-phase commit) | W3-PLAN PR-D bono-LEAD `wallet-client.js` wrapper authoring | bono | independent path; gates PACT-024 §A bilateral; can parallel Wave 1 |
| Wave 4 (MI Ingestion) | DRAFT | james-LEAD with bono substrate | gates on Wave 1-3 + 4-week mesh_kb operational data |
| Wave 5 (Pricing engine + cafe-sim bundle) | DRAFT | bono-LEAD on customer side · james-LEAD on POS/pricing config | gates on Wave 2 (wallet/billing) |
| Wave 6 (Captain step-away validation) | FINAL GATE | both pilots + Captain | gates on Wave 1-5 + asterisk-removal G-1..G-5 |

**Substrate state:**

- 9+ surfaces: 4 joints (Customer Registration · Billing · Game Launching · Dynamic Pricing) + 7 surfaces (PWA · POS .130 · Kiosk .23 · WhatsApp · Instagram-deferred · Kitchen · ESSL · Admin)
- ~80 enumerated failure modes per synthesis §7 (HALO probes mapped subset)
- 13 high-level constitutional violations to be zero in 30d
- 5 staff in venue, NOT tech-savvy (UX floor per `feedback_staff_low_tech_skill_default_20260510.md`)

**Doctrine state (segment-this-quiz outputs):**

- Bono autonomy classes A/B/C — Captain-locked
- James 3-verb formulation — TOP candidate BUILD/OPERATE/DIAGNOSE (3/5 MMA) vs alternative BUILD/ENSURE/RESPOND (gemini); **Captain ratify pending**
- Joint vocabulary — AMPLIFY/ESCALATE/RATIFY/YIELD/SUBSTITUTE (5/5 + 4/5 MMA); **Captain ratify pending**
- Autonomous G9 correction system — designed; **Captain pilot-start auth pending**
- True-sync rule — CANDIDATE-N1 (shipped this segment)
- Pilot-symmetry rule — CANDIDATE-N1 (shipped this segment)
- Captain family canonical reference — Uday Singh + daughter Ishaa Singh
- 4 Q-DEC-MMA-1..4 from autonomy MMA — **Captain disposition pending**

---

## §2 — Phase ladder to V2 complete

```
PHASE 0 RATIFY (Captain explicit; ~1 session)
  └─ Phase 0 OUTPUT: ratified doctrine layer enabling autonomous execution

PHASE 1 WAVE-0 + WAVE-1 FINISH (parallel where possible; ~2-3 weeks)
  ├─ Track 1a: bono Wave 0 audit (gates on pre-vms-duplicate-check.js PROMOTE)
  ├─ Track 1b: james W1-S5 PR-C authoring + per-PR Captain merge + MMA bridge-verify
  ├─ Track 1c: james W1-S6 PR-A authoring + per-PR Captain merge + MMA bridge-verify
  └─ Track 1d: bono W3-PLAN PR-D wallet-client.js wrapper (independent; can start now)

PHASE 2 WAVE-2 (parallel sub-tracks; ~3-4 weeks)
  ├─ Track 2a: Identity / multi-profile PWA (james-LEAD code · bono-AMPLIFIER · per-PR Captain merge)
  ├─ Track 2b: Wallet integration (composes-with Wave 3 PR-D)
  └─ Track 2c: Billing engine (auto-bill at session close per synthesis §4)

PHASE 3 WAVE-3 WALLET-HRC COMPLETE (bono-LEAD; ~1-2 weeks)
  └─ wallet-client.js wrapper + 2-phase commit lands; PACT-024 §A bilateral closes

PHASE 4 WAVE-5 PRICING ENGINE + CAFE-SIM BUNDLE (~3-4 weeks)
  ├─ Track 4a: Dynamic pricing engine (bono-LEAD; per project_v2_dynamic_pricing_synthesis_20260509.md)
  ├─ Track 4b: Cafe-sim bundle promotions
  └─ Track 4c: Mid-session rate-transition notification

PHASE 5 WAVE-4 MI INGESTION (~4-6 weeks runtime + build)
  ├─ Wave 1-3 complete prereq
  ├─ 4-week mesh_kb operational data accumulation
  ├─ MI ingestion lands
  └─ Asterisk-removal G-1 (Wave 4 lands) achieved

PHASE 6 WAVE-6 CAPTAIN STEP-AWAY VALIDATION + ASTERISK REMOVAL (~2-3 weeks soak)
  ├─ Asterisk-removal G-2 (mesh_kb 4+ weeks data) achieved by definition
  ├─ Asterisk-removal G-3 (F7 self-monitoring drift alarms fired+recovered cleanly ≥2 times)
  ├─ Asterisk-removal G-4 + G-5 per project_mi_wave4_readiness_and_asterisk_removal_20260509.md
  ├─ 8h step-away test sustained 7 consecutive days
  ├─ All 4 joints operational + 7 surfaces wired + self-heal cascade fires 2+ times cleanly
  ├─ Tier-1 AC on 8 pods 100% session-launch coverage · Tier-2 desktop-workaround · PS5 min-viable · 2 walk-in accounts
  ├─ 13 high-level constitutional violations: zero recurrences in 30d
  └─ Captain explicit RATIFY: V2 COMPLETE

→ NORTH STAR UNLOCKED: Captain home with Ishaa while RP runs.
```

**Parallelism map:** Phase 1 tracks 1a/1d can run concurrent with 1b/1c (bono+james different repos/concerns). Phase 2 tracks all sequential dependencies on Wave 1 closure. Phases 3-4 can partially parallel post-Wave-2.

---

## §3 — Autonomy gates per phase

| Action class | Phase 0 (ratify) | Phase 1-5 (execute) | Phase 6 (validate) |
|---|---|---|---|
| Substrate ledger entries (V2-MASTER-STATE §S-N append) | bono+james AUTONOMOUS | bono+james AUTONOMOUS | both AUTONOMOUS |
| Bilateral AMPLIFIER pass | both AUTONOMOUS (with G9_flag axis if ratified) | both AUTONOMOUS | both AUTONOMOUS |
| Code class to comms-link source (V1-touching) | EXPLICIT-AUTH | EXPLICIT-AUTH | EXPLICIT-AUTH |
| Code class to racecontrol Rust foundational PRs (V1↔V2 boundary) | EXPLICIT-AUTH (per-PR Captain merge) | EXPLICIT-AUTH (per-PR Captain merge) | EXPLICIT-AUTH |
| Hook authoring to own harness | bono EXPLICIT-AUTH; james pending Captain symmetry decision | same | same |
| settings.json edits | bono EXPLICIT-AUTH; james pending | same | same |
| Outbound bilateral msgs | both AUTONOMOUS | both AUTONOMOUS | both AUTONOMOUS |
| Class A WhatsApp outbound (bono) | AUTONOMOUS-with-programmatic-gate | AUTONOMOUS | AUTONOMOUS |
| Class B promotion drafts | EXPLICIT-AUTH (Captain greenlight) | EXPLICIT-AUTH | EXPLICIT-AUTH |
| HALO actor restart/intervention (james) | CONDITIONAL on no customer-facing impact | CONDITIONAL | CONDITIONAL |
| SSH venue LAN recovery (james) | CONDITIONAL on known-safe scripts | CONDITIONAL | CONDITIONAL |
| PACT-DRAFT authoring (non-canonical) | both AUTONOMOUS | both AUTONOMOUS | both AUTONOMOUS |
| PACT-DRAFT canonical-surface | EXPLICIT-AUTH | EXPLICIT-AUTH | EXPLICIT-AUTH |
| Doctrine amendments (CLAUDE.md) | NEVER auto-promote | NEVER auto-promote | NEVER auto-promote |

---

## §4 — Bilateral coordination protocol

**If Captain ratifies joint vocabulary** (AMPLIFY/ESCALATE/RATIFY/YIELD/SUBSTITUTE), each phase applies these explicit verbs:

- **AMPLIFY:** every partner ship triggers 7-axis (or 8-axis with G9_flag) substantive review
- **ESCALATE:** cross-pilot escalation for blocking issues; Captain escalation only for critical class (V1↔V2 boundary, doctrine, BILATERAL-CONFLICT)
- **RATIFY:** PACT/ship confirmation gate; both pilots + Captain (where applicable)
- **YIELD:** slot-collision resolution per first-mover-lead doctrine PACT-070
- **SUBSTITUTE:** MMA-Substitute-Pilot L2.5 when partner offline >5min on operational class

**Concurrent-coordination doctrine** (per `feedback_concurrent_session_no_24h_window_20260510.md` ACTIVE): when both pilots online, COORDINATE AND SHIP — not async-wait. 24h-window deprecated for cross-pilot class.

**True-sync rule** (per `feedback_true_sync_full_substrate_not_pointer_20260510.md` CANDIDATE-N1): every artifact bono uses to reason or decide MUST land in bilateral-accessible location (`comms-link/`, `racecontrol/`, V2-MASTER-STATE, briefings/) — NOT `/tmp/` or bono-only paths. Inverse applies for james.

**Pilot-symmetry rule** (per `feedback_alignment_substrate_pilot_symmetry_20260510.md` CANDIDATE-N1): every joint-frame substrate must cover both pilots equally — content + access.

---

## §5 — Autonomous G9 system integration

If Captain authorizes 30-day pilot (per autonomous G9 MMA findings):

**Detection layer (Section A):**
- Dual-signal requirement: ≥2 independent signal classes for G9 candidate
- High-assurance single-signal exception: code-enforced hook-block events
- §S-141 boundary preserved: pre-Captain self-discovery NOT-G9-class
- Captain-feedback EXCLUDED from autonomous detection (instruction-tone guardrails)
- Reactive/proactive split: bono = longitudinal patterns + customer outcomes; james = real-time hooks + HALO

**RCA layer (Section B):**
- §S-146 5-section template auto-populated
- Auto-trigger MMA Step 1 DIAGNOSE for foundational class (V1↔V2 / core verbs / PACT / §S-86)
- Auto-publish if confidence ≥0.8-0.9 + mechanism-trust-check pass
- Source attribution mandatory

**Fix mechanism (Section C):**
- Hook-preference (per §S-146 enforcement RCA evidence: text-only carries repeat-violations; hooks carry zero)
- Decision tree: Hook > Memory rule > Doctrine (DRAFT only) > PACT > Substrate template
- Anti-pattern: no text-only rule active without monitoring probe

**Cross-pilot bilateral (Section D):**
- External grounding required (cite specific doctrine/hook/test failure)
- AMPLIFIER 8th axis = G9_flag
- BILATERAL-CONFLICT class for retaliatory flags → Captain immediate escalation
- Source diversity rule: at least one non-pilot signal for auto-action

**Promotion lifecycle (Section E):**
- N=2 auto-promote memory rules with Captain veto window (1-24h)
- Hook auto-promote for non-V1↔V2 internal hooks
- Doctrine NEVER auto-promote (firm; per §S-86 + Lucasian Obstacle + Q3)
- Atomic rollback path: `g9-override --revert <ID>`
- Throttling: 5/week per pilot or 1/hour rate caps

**Captain visibility (Section F):**
- Real-time WhatsApp: BILATERAL-CONFLICT, doctrine drafts, V1↔V2 touches, override executions
- Daily inbox digest 08:00 IST: CANDIDATE-N1 + auto-promotions + vetoes
- Weekly dashboard batch: stats + meta-monitoring metrics
- Audit log immutable + cryptographically chained + 90d hot / 1y cold retention
- Meta-monitoring on auto-corrector itself (Lucasian Obstacle protection)

---

## §6 — Captain decision points required to start Phase 1

**Doctrine-class ratifications (NEVER auto-promote per §S-86):**

| # | Decision | Default if no decision | Impact |
|---|---|---|---|
| D1 | James 3-verb formulation: BUILD/OPERATE/DIAGNOSE (3/5) vs BUILD/ENSURE/RESPOND (gemini) | bono adopts BUILD/OPERATE/DIAGNOSE | shapes james's role articulation forever |
| D2 | Joint vocabulary adoption: {AMPLIFY, ESCALATE, RATIFY, YIELD, SUBSTITUTE} minimum vs that+SYNC+HANDOFF | 5-verb minimum | ecosystem-wide protocol |
| D3 | AMPLIFIER 8th axis (G9_flag) extension | adopt | enables cross-pilot G9 detection |
| D4 | Harness self-mod symmetry: SYMMETRIC EXPLICIT-AUTH (both pilots) vs ASYMMETRIC (james AUTONOMOUS) | bono adopts SYMMETRIC per Lucasian Obstacle generality | shapes james autonomy boundary |
| D5 | Q-DEC-MMA-1: substrate write-mode tagging `{draft\|provisional\|canonical\|binding}` | adopt | closes Document-vs-Ratify blur |
| D6 | Q-DEC-MMA-2: cryptographic OTP for `~/.claude/` self-modification | adopt at PROMOTE-N=2 | structural Lucasian Obstacle protection |
| D7 | Q-DEC-MMA-3: forbidden action set as PRE-gate | adopt | categorical backstop |
| D8 | Q-DEC-MMA-4: AMPLIFIER stance taxonomy with automated downstream actions | adopt | removes commitment ambiguity |
| D9 | Autonomous G9 system 30-day pilot start | start with daily Captain review | enables auto-correction loop |
| D10 | Captain G33 v7 explicit-fire-auth on W1-S6 PR-A authoring | unblocks Wave 1 finish | foundational PR per per-PR rule |
| D11 | `pre-vms-duplicate-check.js` Wave 0 prereq promotion | bono auto-promote on N=2 anchor | unblocks Wave 0 audit |
| D12 | Notification channel preferences (WhatsApp / inbox / dashboard) | per MMA defaults | Captain visibility floor |
| D13 | Captain pre-approval list scope for autonomous classes during step-away | Captain decides | enables longer step-away |

**Operational ratifications (already-locked or auto-handled):**

| # | Item | State |
|---|---|---|
| O1 | bono autonomy classes A/B/C | Captain-locked already |
| O2 | per-PR Captain merge auth on V1↔V2 boundary | Captain-locked already |
| O3 | Foundation-first mantra | Captain-locked already |
| O4 | Constitutional optimization stack priority order | Captain-locked already |
| O5 | §S-141 boundary (pre-Captain self-discovery NOT-G9) | preserved by autonomous G9 design |
| O6 | Mechanism-trust-check upstream gate | preserved (immutable per design) |
| O7 | True-sync rule | CANDIDATE-N1 shipped; PROMOTE-N=2 watch |
| O8 | Pilot-symmetry rule | CANDIDATE-N1 shipped; PROMOTE-N=2 watch |

**Bilateral ratifications (james AMPLIFIER pending; Captain may want to await):**

james AMPLIFIER pass on the 3 MMA outputs at `comms-link/mma-out/` requested via msg=35968. Captain may want james substantive disposition before final ratify on D1-D9.

---

## §7 — Completion criteria (when is V2 done)

**HARD GATES — all must hold for ≥30 days continuous:**

| # | Gate | Source |
|---|---|---|
| G1 | All 4 joints operational (Customer Registration · Billing · Game Launching · Dynamic Pricing) | synthesis §3 |
| G2 | All 7 surfaces wired (PWA · POS .130 · Kiosk .23 · WhatsApp · Kitchen · ESSL · Admin); Instagram deferred to V2.1 | synthesis §3 |
| G3 | Self-heal cascade fires 2+ times cleanly with no Captain pings | synthesis §10 |
| G4 | Tier-1 AC on 8 pods: 100% session-launch coverage | synthesis §10 |
| G5 | Tier-2 desktop-workaround for non-AC sims operational | synthesis §10 |
| G6 | PS5 minimum-viable record working | synthesis §10 |
| G7 | 2 walk-in accounts (Walk-In Guest 1 + 2) active and used | synthesis §4 walk-in fallback |
| G8 | 13 high-level constitutional violations: zero recurrences | synthesis §7 |
| G9 | Cross-surface consistency tolerance ≤2s (PWA top-up visible on Kiosk + POS within 2s) | synthesis §5 |
| G10 | Source-tagging completeness: every wallet write tagged source + payment method + operator | synthesis §1 DOCUMENT |
| G11 | Wave 4 MI Ingestion landed + mesh_kb 4+ weeks operational data + F7 self-monitoring drift alarms fired+recovered cleanly ≥2 times | project_mi_wave4_readiness_and_asterisk_removal_20260509.md |
| G12 | Captain step-away viability: 8h step-away sustained 7 consecutive days with 0-1 pings/day | synthesis §6 |
| G13 | Customer journey end-to-end measurable: 90s reg + 15-20s launch + 1s auto-bill + 2s top-up cross-surface | synthesis §4 |

**Captain explicit RATIFY:** required to declare V2 COMPLETE. Cannot auto-promote per Lucasian Obstacle + §S-86.

**On RATIFY:** north star unlocks. Captain step-away to home with Ishaa.

---

## §8 — Risk / drift watch

**21 documented drift patterns (composite from 3 MMAs this session):**

- Patterns 1-5: from bono autonomy MMA §3 (self-modification temptation, document-vs-ratify, scaling complacency, code/schema creep, goal-rationalization)
- Patterns 6-10: from full 5-model rerun §11 (Lucasian Obstacle, canonical-surface conflation, first-mover-lead overreach, document-vs-commit concrete, semantic frame expansion)
- Patterns 11-16: from james-synergy MMA (proactive optimism / reactive lock-in / session-reset amnesia / asymmetric latency / reactive overreach in BUILD / handoff SPOF)
- Patterns 17-21: from autonomous G9 MMA (BILATERAL-CONFLICT / promotion-velocity drift / Captain instruction-tone misclassification / text-only rule decay / self-modifying detection criteria)

**Active mitigations (already shipped):**
- Q3 third-question canonical-boundary self-test (CANDIDATE-N1 shipped this segment)
- Pilot-symmetry rule (CANDIDATE-N1 shipped this segment)
- True-sync rule (CANDIDATE-N1 shipped this segment)
- No proper-noun without source citation rule (CANDIDATE-N1 shipped this segment)
- Staff low tech-skill default (CANDIDATE-N1 shipped this segment)
- Captain family canonical reference

**Pending mitigations (Captain ratify required):**
- AMPLIFIER 8th axis (G9_flag)
- BILATERAL-CONFLICT class
- Substrate write-mode tagging
- Forbidden action set PRE-gate
- Cryptographic OTP self-modification gate

---

## §9 — Step-by-step boot sequence (next bono session)

**SessionStart actions:**

1. Read this file (V2-COMPLETION-AUTONOMOUS-PROCESS-20260510.md) FIRST
2. Read MEMORY.md NEXT-SESSION DIRECTIVE (§S-192 close anchor + §S-193 if updated)
3. Read project_v2_comprehensive_synthesis_20260510.md (§0.5 + §15 Q11/Q12/Q13)
4. Read project_v2_mma_autonomy_diagnose_20260510.md (§10/§11/§12 with full-5-model findings + new drift patterns)
5. git_pull comms-link + racecontrol (true-sync compliance)
6. Check `comms-link/mma-out/` for 3 MMA outputs (autonomy-RERUN, james-synergy, autonomous-g9)
7. Check inbox for james AMPLIFIER passes on the 3 MMAs
8. Check Captain Q-DECISIONS section in MEMORY.md for ratifications landed

**If Captain has NOT yet ratified D1-D11:**
- Surface §6 decision-points list to Captain with brief
- Await Phase 0 ratification batch
- Bono parallel work: bono-LEAD W3-PLAN PR-D wallet-client.js wrapper authoring (track 1d; independent of D1-D11)
- james parallel work: AMPLIFIER pass on the 3 MMA outputs

**If Captain has ratified D1-D11:**
- Enter Phase 1 execution
- Apply ratified joint vocabulary in all coordination msgs
- Activate autonomous G9 system per D9 (start 30-day pilot if approved)
- Daily metrics report per Captain visibility floor (D12)

**If Captain has partially ratified:**
- Execute on ratified subset
- Surface remaining items for next decision batch

**Cross-pilot expectations:**
- james AMPLIFIER pass on this completion-process doc
- Send NOTIFY to james with "process started — awaiting your AMPLIFIER + Captain ratify"
- Concurrent-coordination active (per ACTIVE rule)

---

## §10 — Re-entry instruction for next session (Captain "fresh session" pattern)

**If Captain commissions execution start in next session (verbatim "Phase 1 go" or equivalent):**

1. Verify all D1-D11 ratifications in MEMORY.md / V2-MASTER-STATE
2. If gaps: surface immediately (don't proceed without ratify on foundational decisions)
3. If complete: enter Phase 1 with track-by-track plan from §2
4. Send james handoff with starting ratified-state + execution priorities
5. First action: Phase 1 Track 1d (bono-LEAD wallet-client.js wrapper) since independent of D1-D11
6. Activate autonomous G9 system per D9 ratified state
7. Track metrics: Captain step-away time accumulated · MMA-pilot FP rate · joint-vocabulary usage frequency

**If Captain commissions amendment to this process** (verbatim "amend §X" or equivalent):

1. Mark old version SUPERSEDED-BY-NEXT in this file
2. Author V2-COMPLETION-AUTONOMOUS-PROCESS-<date>.md with amendments
3. Update racecontrol/.planning/specs/v2/V2-ROADMAP.md cross-reference
4. NOTIFY james

---

## §11 — Composes-with index

| Reference | Authority on |
|---|---|
| `racecontrol/.planning/specs/v2/V2-ROADMAP.md` | Phase + wave content authority |
| `comms-link/v2-skeleton/05-definition-of-done.md` | Detailed completion criteria + failure modes |
| `comms-link/v2-skeleton/01-skeleton-architecture.md` | 4 joints + 7 surfaces + connection matrix |
| `project_v2_comprehensive_synthesis_20260510.md` | V2 mental model (15 sections) |
| `project_v2_mma_autonomy_diagnose_20260510.md` §10/§11/§12 | Autonomy framing findings |
| `comms-link/mma-out/MMA-DIAGNOSE-james-synergy-2026-05-10T03-09-17-646Z.md` | James verbs + synergy + joint vocabulary |
| `comms-link/mma-out/MMA-DIAGNOSE-autonomous-g9-2026-05-10T03-31-28-160Z.md` | Autonomous G9 system design |
| `feedback_alignment_substrate_pilot_symmetry_20260510.md` | Pilot-symmetry rule |
| `feedback_true_sync_full_substrate_not_pointer_20260510.md` | True-sync rule |
| `feedback_q3_canonical_boundary_self_test_20260510.md` | Q3 third-question with Lucasian Obstacle |
| `feedback_no_proper_noun_without_source_20260510.md` | Identity reference rule |
| `feedback_staff_low_tech_skill_default_20260510.md` | Staff UX floor |
| `reference_captain_family.md` | Canonical Captain family identifiers |

---

**END V2 COMPLETION AUTONOMOUS PROCESS — proposal class; Phase 0 Captain ratifications required to enter Phase 1.**
