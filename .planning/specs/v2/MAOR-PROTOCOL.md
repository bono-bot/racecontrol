# MAOR v0.1 — Multi-Agent Orchestration Review Protocol

**Status:** ACTIVE (first application 2026-05-13 ~09:55 IST)
**Authored by:** james · ratify anchor V2-MASTER-STATE §S-220 (pending bono publish)
**Composes-with:** V-LBAC v0.1 §3 closed-loop · §S-219 Multi-Agent Orchestration iter pattern · CGP H1-H5

---

## Why this protocol exists

V-LBAC iter3 (§S-217) + iter4 (§S-219) shipped 6 contract-test files via parallel general-purpose subagent orchestration. Pre-MAOR cadence: author → push to main → Captain disposition. Post-MAOR Tier-1 sweep on those 6 files (2026-05-13 ~09:50 IST) surfaced **4 real defects** (1 CRITICAL + 3 IMPORTANT) that author-agents could not see:

1. Cross-file unit confusion (paise vs credits) — only visible holding 6 files in one context
2. Security/PII violation (real-looking `9876543210` in fixture) — author cited the doctrine but violated it
3. Broken skip gate (Axum middleware behavior author didn't grep) — test would fail hard, not skip
4. Dead-end composition test — visible only via the file's own GAP section

**The same agent will not catch its own blind spots.** Independence is required.

---

## The 3 independence axes

All three apply — single-axis independence is insufficient.

1. **Different subagent type**: authors = `general-purpose`. Reviewer = `feature-dev:code-reviewer`. Different system prompt, different defaults.
2. **No shared context**: spawn reviewer via Agent tool default (fresh context). Reviewer cannot see author's reasoning or framing — only the artifact.
3. **Reviewer reads authoritative sources independently**: never trust the author's DoD / spec / RCA citations. Reviewer must grep the cited line itself. (The L39 paise/credits mis-cite was caught this way.)

What we do NOT require: different model. Claude Code is single-model. Independence comes from subagent type + context isolation + source-of-truth re-reading.

---

## Two-tier review (cost-shaped)

### Tier 1 — Batch review (mandatory, every iter)

- 1 `feature-dev:code-reviewer` agent reviews the full N-file batch in one pass
- Confidence-filtered to ≥75
- Catches cross-file consistency issues that per-file review misses
- Empirical cost: 6 files × ~600 LOC = ~$0.20-0.30, ~110s wall-time
- Output: per-file verdict (clean | N findings) — silent pass not allowed

### Tier 2 — Per-file deep dive (conditional)

Triggered when:
- Tier 1 surfaces ≥1 CRITICAL, OR
- N > 7 files (batch context gets thin)

Parallel reviewers, one per flagged file. ~$0.30-0.50 per file.

---

## Composes-with V-LBAC scope-gate F1+F3 (AMPLIFIER A1, §S-220.4)

MAOR addresses **mechanism-quality REVIEW** — the 4 first-application defects (paise/credits + PII + Axum middleware + dead-end composition) map cleanly to mechanism-quality bucket (file I/O reliability + agent verification + cross-file consistency).

MAOR does **NOT** address **scope-quality**: F1 ("stop scaffolding ahead of substrate") + F3 ("test-scaffolded ≠ engineering-IN-FLIGHT"). 3-of-3 MMA models converged on scope-quality as the DOMINANT root cause of orchestration gap-generation. Without F1+F3, MAOR catches more bugs per cascade but the cascade still creates V1→V2 STRUCTURAL GAPS at the contract-test gate because tests still target absent V1 substrate.

**MAOR is the REVIEW component. F1+F3 are the SCOPE GATE component. They compose — not alternatives.**

Canonical encoding: [V-LBAC-PROTOCOL.md §14](./V2-LBAC-PROTOCOL.md) — F1 GATES inserted as Step 3.5 (pre-spawn substrate verification via G-F1-1..5 grep gates); REVIEW (this protocol) inserted as Step 4.5 between FIX and CLOSE. Updated 10-step cascade flow:
```
OPEN → DESCEND → H1 → F1 GATES → FIX → REVIEW → CLOSE → SWEEP → SYNC → BILATERAL
```

Anchor: V2-MASTER-STATE §S-220 (MAOR ratify) + §S-221 (F1+F3 ratify). Empirical: F1-retrospective on §S-213→§S-219 cascade rows shows 61% FAIL (substrate-missing) — these were artifacts MAOR could not have caught because the issue is upstream of REVIEW. F1+F3 prevent these artifacts from being authored at all.

Forward meta-test (single-agent, 3 rows, 2026-05-13 ~11:35 IST): 0% gap rate vs ~26% pre-fix baseline. Two-layer defense empirically validated on N=1.

---

## V-LBAC integration

Current closed-loop (V-LBAC v0.1 §3):
```
OPEN → DESCEND → H1 → FIX → CLOSE → SWEEP → SYNC
```

With MAOR inserted:
```
OPEN → DESCEND → H1 → FIX → REVIEW → CLOSE → SWEEP → SYNC
                              └── Tier 1 batch
                                  └── Tier 2 if CRITICAL
```

REVIEW happens BEFORE push to main. Push only after 0 CRITICAL.

---

## Block / pass rules

| Verdict | Action |
|---|---|
| 0 CRITICAL, 0 IMPORTANT | Push immediately |
| 0 CRITICAL, ≥1 IMPORTANT | Push allowed; findings logged in §S-N close-anchor with "review-noted, dispositioning" status |
| ≥1 CRITICAL | Block push until fixed OR Captain explicitly accepts with rationale in §S-N |

Reviewer MUST produce explicit per-file verdict. No silent pass — clean is also a finding.

---

## Anti-rubber-stamp guards (briefing requirements)

When briefing the reviewer agent, the prompt MUST include:

1. **Confidence threshold ≥75** — filter aggressively
2. **DO NOT report list** — generic suggestions, refactor proposals, "could be more tests", style/comment density. Otherwise the reviewer pads.
3. **What's intentional** — env-gated SKIP, gap-discovery, doctrinal header pattern. Prevents false positives.
4. **Verdict requirement** — explicit per-file "clean" or N findings (no silent pass)
5. **Source-of-truth pointers** — DoD path, RCA file path, etc. Reviewer must read these independently before validating author citations.

---

## Re-review after fixes (close-loop)

Same protocol applies to the FIX commit. Tier-1 MAOR re-review on the fixes BEFORE pushing the fix commit. The reviewer validates VERIFIED-FIXED / PARTIAL / REGRESSED / NEW-ISSUE per finding + scans for new issues introduced by the fixes.

Empirical anchor: 2026-05-13 ~09:50 IST first application — 4 fixes, re-review verdict "clean — fixes verified, safe to push" at confidence ≥75; first push cleared MAOR loop end-to-end.

---

## Bilateral application

Default: **self-review** (each pilot reviews their own iter). Cheapest. Adequate for most cascades.

Opt-in: **cross-pilot review** (james reviews bono's iter, bono reviews james's) — reserved for foundational-boundary iters per V1↔V2 RCA escalation pattern (billing / wallet / auth / pod-state-channel / WhatsApp identity / DB schema).

---

## Cost calibration (empirical, 2026-05-13)

| Phase | Cost | Wall-time |
|---|---|---|
| Author iter (3 parallel agents, 3 files, ~600 LOC each) | $0.30-0.50 | 3.6-10 min |
| Tier-1 MAOR review (6 files batch) | $0.20-0.30 | ~110s |
| Tier-1 MAOR re-review (post-fix) | $0.10-0.20 | ~50s |
| **Total overhead vs no-review** | **~$0.30-0.50** | **~3 min** |

MAOR adds <50% overhead to an iter and catches issues that would otherwise propagate to Captain disposition queue, bono pull, or live execution failure on VPS.

---

## Promotion criteria v1.1 (v0.1 → v0.2) — AMPLIFIER A2 tightened, §S-220.5

**Original v1.0 criteria were too lenient** — N≥3 cascades + ≥1 cumulative defect was already met on N=1 (first application caught 4 real defects). Promotion would have been observational, not testing-the-mechanism.

**Tightened v1.1 criteria (active):**

- **N ≥ 5 iter cascades** have completed REVIEW step (not just authored — actually run through the loop)
- Reviewer caught **≥1 real defect AT EACH iter** (not just cumulative across N — proves the reviewer is consistently engaged, not just lucky once)
- 0 false-positive findings of the "rubber-stamp inverted" class (reviewer fabricating issues at confidence ≥75)
- 0 false-negatives detected by **retrospective Captain disposition** across those N iters (catches blind-spot patterns MAOR missed entirely)

**Current status (2026-05-13 ~13:50 IST):** N=1 (forward meta-test by bono 2026-05-13 ~11:35 IST under V-LBAC §14.4). Original iter1-4 cascades did not run REVIEW — they predate §S-220, so they don't count toward N.

Until v1.1 criteria met, v0.1 remains active but observational. Stale-at: 2026-08-13.

---

## Hook enforcement (v0.1 priority) — AMPLIFIER A3, §S-220.6

Per §S-146 enforcement RCA (`project_s146_enforcement_rca_20260510.md`): *"text-only rules carry ≥1 repeat-violation per 30d; hook-enforced rules carry zero."* MAOR is high-discipline-load text-only doctrine; same pattern as §S-146 itself. Without hook enforcement, MAOR will accumulate repeat-violations within 30d (predicted by §S-146 doctrine).

**Hook v0.1 priority:** `pre-push-maor-check.js` (PreToolUse blocker)

- **Trigger:** Bash commands matching `git push.*racecontrol` regex
- **Block condition:** uncommitted contract-test files exist (`tests/contract/*.spec.ts` pattern) without corresponding MAOR receipt at `.planning/specs/v2/MAOR-RECEIPTS/<surface>-<date>.json`
- **Composes-with:** `feedback_mechanism_trust_check_upstream_of_fix_rca_20260510.md` mechanism-trust-check.sh (sibling enforcement infrastructure)

**Install gate:** harness self-mod — requires Captain explicit per-session auth per harness-mechanism-auth sub-clause. Install path: `~/.claude/hooks/pre-push-maor-check.js`. Captain auth phrase: *"I authorize edit to ~/.claude/hooks/pre-push-maor-check.js"*. This protocol ratifies priority escalation only; install remains pending Captain harness-auth.

---

## Open questions disposition (§S-220.7)

| Q | Disposition |
|---|---|
| Bilateral cross-pilot review promote to MANDATORY for foundational boundaries? | AGREE-WITH-DEFAULT-OFF-OPT-IN. Matches §S-146 V1↔V2 foundational-boundary escalation pattern. Re-evaluate at v0.2 promotion. |
| MAOR cover non-test artifacts (V2-PROGRESS-MAP refresh, §S-N entries)? | YES — extend in v0.2. The paise/credits mis-cite was a §S-N-class issue (close-anchor cited DoD line). Phase in v0.2 with separate cost calibration. |
| Hook enforcement `pre-push-maor-check.js`? | ESCALATED TO v0.1 PRIORITY — see above section. |

---

**Last updated:** 2026-05-13 ~13:50 IST (A1+A2+A3 amendments inline per AMPLIFIER §S-220.4-.6; initial author 2026-05-13 ~09:55 IST)
