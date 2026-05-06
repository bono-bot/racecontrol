# Cognitive Gate Protocol v4.3 — "Backlog Gate"

**Purpose:** Prevent false completion claims. Everything else is secondary.

**Root cause (researched):** RLHF trains AI agents to produce confident, completion-signaling language. 45.4% of AI-generated PRs contain "descriptions claiming unimplemented changes" (23,247 PR study, Jan 2026). The agent generates "done" because that token follows naturally from having performed actions — not because it verified outcomes. Rules don't fix this because the agent can recite rules fluently while violating them. Only structural blocks work.

**v4.0 change (2026-04-04):** 147 rules consolidated to 40. Research found rule volume itself causes false confidence — formatted compliance output substitutes for actual thinking. Fewer rules with harder enforcement beats more rules with no enforcement.

**Scope:** All Racing Point systems — venue, cloud, PWA, WhatsApp, comms-link. E2E = customer journey (phone → venue → pod → cloud sync), not infrastructure round-trips.

---

## The 5 Hard Gates (cannot be skipped — hook-enforced or structurally blocked)

These gates have CODE enforcement. They block action, not just advise.

### H1: Problem Before Action
**Hook:** `cgp-enforce.js` (PreToolUse) — DENIES first action tool call until G0 block produced.
**What:** Write PROBLEM + PLAN before any Bash/Edit/Write/Agent call.
**Trivial bypass:** `G0: trivial — <reason>` for single-file reads or simple questions.
**Why it works:** Structural block. Cannot produce action output without this gate passing.

### H2: Two-Phase Completion
**Enforcement:** Structural — fix and verify MUST be in separate messages.
**What:** NEVER claim "done/fixed/deployed/PASS" in the same message as the last action.
**Why it works:** Message boundary is unambiguous. No self-judgment required.

### H3: Evidence Before Claims
**Hook:** `cgp-session-inject.js` (UserPromptSubmit) — injects reminder every prompt.
**What:** Before any completion word, show:
1. **What behavior** was tested (specific — not "health OK")
2. **Raw output** proving it (paste the actual command + result)
3. **Where** the test ran FROM — state the machine explicitly. If the user specified target machines (e.g. "test on POS," "use server .23"), evidence MUST come FROM those machines. James-local Playwright ≠ POS browser. SSH curl from James ≠ browser on the server. If you can't test from the specified target, say so — don't silently substitute.
4. **What was NOT tested** (there is ALWAYS something — empty list = lie)

**Anti-theater rule:** If the "evidence" is a health endpoint, build_id, or ws=True, it is NOT evidence of the fix working. It's evidence the binary is running. Name the ACTUAL behavior.

**Observations, not verdicts (v4.4, 2026-04-11):** Report what you observed, not what you concluded. Write "API returned `{ok: true}`. Screenshot shows blanking screen, no game visible." Do NOT write "PASS" or "FAIL." Contradictions between observations are obvious without labels; labels hide them. The completion bias lives in the verdict — the moment you write "PASS," you've committed to a position and will defend it against your own evidence.
_Why: 2026-04-11 — game launch API returned `{ok: true, verified: true}`. Screenshot showed blanking screen (game not running). James wrote "PASS" for game launch and downgraded the contradictory screenshot to "INFO." The verdict created the bias; without it, the contradiction would have been obvious to anyone reading the report._

**Anti-substitution rule (v4.2, 2026-04-04):** Testing from Machine A when the user said Machine B is not "close enough" — it's a different network path, different browser context, different DNS/proxy resolution. The bug that prompted this rule: kiosk at `:3300` works from James (Playwright) but fails from server browser because `:3300` has no API proxy — only `:8080` (racecontrol reverse proxy) routes API calls correctly. This class of bug is INVISIBLE to any test that doesn't run from the specified machine.

### H4: Target Enumeration Before "Everywhere"
**Enforcement:** Any claim containing "all," "everywhere," "fleet-wide," "every," or "complete" MUST be preceded by an explicit target list with per-target evidence.
**What:** Before saying "deployed everywhere":
1. **Grep** for all locations the change touches
2. **List** each target with evidence (command output, not assertion)
3. **Missing targets** = the claim is false. Period.

**The full target list (from MEMORY.md):**
Server .23 | Pods 1-8 | POS .130 | James .27 | Bono VPS | Cloud apps | Comms-link (James) | Comms-link (Bono)

**Why it works:** Forces enumeration before assertion. The grep IS the verification — not a formatted table you fill in after the fact.

**Multi-fix sessions (v4.5, 2026-04-16):** When multiple fixes are applied in a single session, EACH fix requires its own G1 proof block with per-fix evidence BEFORE any completion summary. Do NOT batch fixes into a summary table without individual verification. A summary that says "Fix A: RESOLVED, Fix B: VERIFIED" is a claim — not evidence. Each fix is independently verifiable and must be independently verified.
_Why: 2026-04-16 — 5 fixes applied in overnight autonomous session. Completion report presented as a table with "RESOLVED/VERIFIED" labels. Only 1 of 5 had actual G1 evidence (terminal_secret had before/after logs). The other 4 had no raw output proving the fix worked. The summary format encouraged labeling over verification._

### H5: User Corrections Are Mandatory Retrospectives
**What:** Every user correction ("good catch," "you missed," "that's wrong") triggers:
1. **Why** the error happened (root cause, not excuse)
2. **What structural change** prevents recurrence (not "I'll remember next time" — that's the bias talking) — see CANDIDATE-N1 disposition below
3. **Session G9 counter** — target: 0. Report in every gate summary.

**Why it works:** Turns errors into protocol improvements. The counter creates accountability.

**CANDIDATE-N1 doctrine (v4.6, 2026-05-01 IST — Captain G33-CONFIRM ratify):**
Every G9 produces (a) WHY root cause + (b) **candidate** structural fix tagged `CANDIDATE-N1` + (c) defer-active condition (default: second independent firing of same root-class within 30 days). Promote to active rule only after N=2 confirmation. **Code-enforced hooks are exempt** — they ship at N=1 because their false-positive cost is bounded by an explicit BYPASS escape verb.
_Why: 2026-05-01 — same session produced G9 #1 + G9 #2 within 30 minutes; G9 #1 over-broad rule (`recipient-already-monitoring`) was applied immediately and triggered the harness denial that produced G9 #2. Pattern: H5's "structural change" requirement, applied at N=1 evidence, manufactures over-broad rules from one ambiguous signal. Memory-resident rules from N=1 evidence add noise faster than they remove errors. Code-enforced hooks remain exempt because they have an explicit BYPASS escape verb (e.g. `BYPASS_AXIS_CLASSIFICATION=1`) that bounds their false-positive cost. Anchor: Captain Uday verbatim-quote of proposed amendment text mapped to G33-CONFIRM verb per autonomous-PACT-operation directive. Master memory: `feedback_g9_candidate_n1_doctrine_h5_amendment_20260501.md`._

---

## The 5 Soft Gates (advisory — require discipline, no code enforcement)

These gates help when followed but have no structural block. They are explicitly marked as SOFT because pretending they're hard creates false confidence.

### S1: Competing Hypotheses
**When:** Unexpected data or before concluding anything is "offline/down/dead."
**What:** 2+ hypotheses with specific tests. Single hypothesis = insufficient.
**Multi-probe:** Before "offline" — run `bash scripts/check-alive.sh <target>`. Script checks ping (LAN + Tailscale) + HTTP health. Verdict: UP/DEGRADED/DOWN. If script says DEGRADED (some probes pass), system is ON — investigate the failing probes, don't conclude offline. NEVER conclude offline from a manual single ping.

### S2: Context Parking
**When:** Topic changes while work is open.
**What:** PAUSED + STATUS + NEXT + RESUME BY.

### S3: Dependency Cascade
**When:** Changing shared interfaces (APIs, configs, DB schemas).
**What:** Grep all consumers. Update each. Repeat recursively.

### S4: Apply, Don't Summarize
**When:** User shares a link, methodology, or reference during active problem.
**What:** Apply it to the current problem FIRST. Document SECOND.

### S5: Canary Before Fleet
**When:** Deploying to multiple targets.
**What:** Pod 8 first. Verify. Then fleet. Test display changes on ONE pod before all.

---

## 15 Standing Rules (consolidated from 147)

Each rule is here because it prevented a documented incident AND cannot be automated away.

### Deploy
1. **Use the deploy scripts.** `deploy-server.sh` (server), hash-based deploy (pods). Don't hand-chain cmd.exe commands.
2. **Delete before SCP on Windows.** SCP silently fails to overwrite. `del` → SCP → verify content.
3. **Rebuild ALL frontends after server deploy.** Stale frontend JS + new server WS = silent connect/disconnect loop invisible to health checks.
4. **Session 1 for rc-agent.** Session 0 breaks ALL GUI operations. Verify with `tasklist /V` → Session column = Console.
5. **Touch build.rs before release builds.** Cargo caches binaries; new commits don't trigger rebuild. `touch crates/*/build.rs` before `cargo build --release`.

### Verify
6. **Verify the EXACT behavior, not proxies.** Health 200, build_id match, ws=True prove the binary runs — NOT that the bug is fixed. Test the specific data flow that was broken.
7. **MMA before cross-system bridges.** Any feature spanning 2+ system boundaries needs multi-model audit. Single-system changes don't.
8. **Cause Elimination before fix.** Document symptom → List ALL hypotheses (min 3) → Test & eliminate one by one → Fix confirmed cause → Log.

### Operate  
9. **Fix one system, fix ALL.** After fixing anything on one machine: does this apply to all pods/POS/server/cloud? If yes, roll out fleet-wide in the same step.
10. **Auto-push + notify.** Every commit → `git push` → comms-link WS message → INBOX.md. No ranking "important" vs "minor."

### Permanence & Sync
11. **Permanence Gate: every fix must survive redeploy.** Before claiming "fixed": (a) Is the fix in committed source code? → Permanent. (b) Is it a manual edit on a deployed artifact (server.js, .env, schtask, registry)? → TEMPORARY. Temporary fixes MUST have either a deploy script that re-applies them OR the root cause fixed in source. _Why: Admin RC_URL manually injected into server.js — overwritten on next deploy. Two admin schtasks created manually — will drift._
12. **Universal rule sync: every rule/protocol update must sync to ALL locations. No exceptions.** After ANY change to CGP, CLAUDE.md, standing rules, hooks, or operational protocols: sync to ALL of these locations in the SAME session: (a) `racecontrol/COGNITIVE-GATE-PROTOCOL.md` (source of truth), (b) `~/.claude/hooks/cgp-session-inject.js` (hook injection), (c) `~/.claude/projects/*/CLAUDE.md` (all project contexts), (d) `comms-link/CLAUDE.md` (Bono's context), (e) Bono VPS via comms-link relay (`git_pull`), (f) memory files that reference the rule. Missing ANY location = rule drift = silent protocol divergence between James and Bono. _Why: Permanence Gate existed only in memory for weeks. H3 v4.2 was added to hook+protocol but not synced to Bono. Rules that exist in one place but not another cause contradictory behavior._

### Verify (domain-specific)
11. **Use verification scripts when they exist.** `verify-action.sh` (game-launch, deploy, session-end, blanking), `pod-verify.sh` (Session context, edge count). If the script exists and returns PASS/FAIL, use it. If it doesn't exist, say so — don't pretend you verified. Script FAIL = do NOT claim done.
12. **Financial flow E2E for billing changes.** Trace actual currency values through: create customer → topup → book → launch → end (early/normal/cancel) → verify refund/balance. Any function that UPDATEs then SELECTs same DB column = audit for overwrite. _Why: F-05 lost Rs.162.50 per customer per early-end session._
13. **Session start: check fleet health + MAINTENANCE_MODE.** At session start, run fleet health snapshot. Any pod with `ws_connected: false` → check for MAINTENANCE_MODE via SSH before investigating. Stale MAINTENANCE_MODE blocked 3 pods for 1.5+ hours with no alert.

### Audit
14. **Ecosystem manifest before audit claims.** Any audit (ecosystem, security, fleet) MUST load `ECOSYSTEM-MANIFEST.json` and check every `critical: true` system. Audit is incomplete if any critical system has no coverage. List skipped systems explicitly — "I audited everything" without manifest verification is an H4 violation.
15. **Audit findings feed Meshed Intelligence.** After any code audit, seed findings into `audit_known_issues` via `POST /api/v1/mesh/audit-seed`. This lets MI Tier 0 short-circuit diagnosis for known code bugs instead of wasting AI credits. _Why: 2026-04-04 ecosystem audit found 92 bugs. MI had no way to know about them — would have wasted Ollama/OpenRouter trying to diagnose runtime symptoms of code bugs._

---

## Emergency Protocol (Phase E)

**Trigger:** Customer unable to race, 3+ pods offline, server down.

**7-Minute Recovery:**
- Minute 0-2: TRIAGE — how many pods? Customers waiting?
- Minute 2-5: STABILIZE (reboot, clear MAINTENANCE_MODE, restart via schtasks, paper billing)
- Minute 5-7: COMMUNICATE — WhatsApp Uday if >2 pods or >15 min

**During emergency:** H1 (problem definition) deferred. H2 (two-phase), H3 (evidence), H4 (targets) still apply. You can act fast but cannot claim "fixed" without evidence.

---

## Metrics: How We Know This Works

### Primary Metric: False Claim Rate (FCR)
**Definition:** Number of times user corrects a completion claim ÷ total completion claims per session.
**Target:** FCR < 10% (currently estimated ~30-40% based on documented corrections).
**Measurement:** G9 counter (user corrections) ÷ Gate Summary count.

### Secondary Metrics
- **Gate overhead:** Tokens spent on gate compliance vs. problem-solving. Target: < 15% (estimated 30-40% under v3.6).
- **Time to first evidence:** How many messages between "I'll fix it" and actual evidence paste. Target: ≤ 2 messages.
- **Enumeration before assertion:** Did grep/list precede "everywhere" claims? Binary yes/no per instance.
- **UCA — Unenumerated Coverage Assertions (v4.4, 2026-04-09):** Count of claims made about the state of a set (coverage, capability existence, completeness) without a preceding enumeration of the set. Examples: "I've read all the feedback files" without `ls feedback_*.md` first; "I can't send email" without `ls ~/.claude/*email*`; "that's all of them" without grep. Each UCA = one G9 unless self-caught and corrected in the same message. **Target: 0 per session.** Origin: session 2026-04-09 had 3 UCAs on the same root cause (missed 40 feedback files, missed email capability, missed 5 MCP servers) — all were instances of answering from mental model instead of enumerating environment. Structural fix: this metric + `reference_local_capabilities.md` manifest + CLAUDE.md Rule 0 (enumerate before assert).

### How to Track
At session end, report:
```
SESSION METRICS: Claims: N | Corrections: N | FCR: N% | G9s: N | UCAs: N | Overhead: ~N%
```

---

## What Was Removed (and why)

| Removed | Why |
|---------|-----|
| 87 incident-specific micro-rules | Consolidated into 10 principles. "Never restart explorer on NVIDIA Surround" → "Canary before fleet" |
| Verification script requirements (pod-verify.sh, verify-fix.sh, verify-action.sh) | Scripts may not exist; requiring non-existent tools creates false confidence. H3 (evidence) is the actual gate. |
| Gate Summary formatted block | Compliance theater — producing `GATES: [G0,G1] \| PROOFS: [Y,Y]` felt like verification but wasn't. Replaced with metrics. |
| 6 lifecycle phases with 8-item checklists each | Checklists became rote. Replaced with 5 hard gates that structurally block. |
| G1 Memory Update (4th proof item) | 20% token overhead; incentivized easy fixes over hard investigation. Memory updates happen naturally or via hooks. |
| G2 fleet scope formatted table | Moved into H4 (target enumeration). The grep IS the verification, not a table filled after the fact. |
| G3 (Apply Now), G6 (Context Parking), G7 (Tool Verification), G8 (Dependency Cascade) | Moved to soft gates S2-S4. These help when followed but pretending they're hard created false confidence. |
| 169 gate items classification | Complexity without proportional benefit. 5 hard + 5 soft + 10 rules = 20 things to remember, not 169. |

---

## Standing Rule #16 — Definition of Shipped (v4.3, 2026-04-06)

A milestone/phase/fix is NOT "SHIPPED", "DONE", "RESOLVED", or "FIXED" until ALL of:
1. Code committed AND pushed to remote
2. Binary built from that commit
3. Deployed to ALL applicable targets (Server, Pods, POS, Cloud)
4. Behavior verified on at least ONE target (not just health/build_id)
5. Memory file updated with deploy evidence (commit hash + target + date)

Memory files MUST use these statuses:
- **COMMITTED** — code in git, not deployed
- **DEPLOYED-PARTIAL** — deployed to some targets, not all
- **DEPLOYED** — on all targets, not behavior-verified
- **VERIFIED** — deployed + behavior evidence recorded
- **DEFERRED** — intentionally postponed (with reason + date)

"SHIPPED", "DONE", "RESOLVED", "FIXED" are ALIASES for VERIFIED only.
Any memory file using these words without deploy evidence violates this rule.

_Why: 20 items across 6 sessions were marked SHIPPED/DONE/RESOLVED when they were only COMMITTED. The memory file felt like completion but was documentation of incompletion. Root cause: treating "committed to git" as the finish line instead of "verified working in production on all targets."_

## Standing Rule #17 — Session-Start Backlog Review (v4.3, 2026-04-06)

**Hook:** `backlog-enforce.js` (UserPromptSubmit) — scans memory for incomplete work every prompt.

At the start of every session (enforced by hook):
1. Hook scans all `project_*.md` memory files for incomplete work patterns (NOT DEPLOYED, PENDING, INCOMPLETE, etc.)
2. Items matching are surfaced as context on every prompt
3. If count >= 3 (WIP limit) AND user is requesting new feature work, Claude must clear backlog first
4. User can explicitly override ("ignore backlog", "skip backlog") — Claude notes the override

Clearing backlog means one of:
- Deploy + verify → update memory to VERIFIED
- Explicitly close → update memory to DEFERRED with reason
- User says "won't fix" → update memory to CLOSED

**"Next session" is NOT a valid disposition.** If you can't finish it now, mark it DEFERRED with a concrete reason, not "pending."

_Why: "Pick up in next session" appeared in 5+ memory files. None were ever picked up. Each new session started fresh with new problems. The backlog hook ensures incomplete work is visible every turn, not buried in memory files nobody re-reads._

## Standing Rule #18 — Debugging Reference Docs (v4.3, 2026-04-06)

Before investigating any production issue, consult the debugging reference docs in `docs/`:

| Doc | Use When |
|-----|----------|
| `SERVICE-REFERENCE.md` | Need per-binary details: modules, config, ports, common failures, debug checklists |
| `ERROR-CATALOG.md` | Encountered a known error — check root cause + fix before investigating from scratch |
| `LOG-LOCATIONS.md` | Need to find logs — lists every log file on every machine + quick debug commands |
| `DATA-FLOW-DIAGRAMS.md` | Tracing a data flow bug — 9 flow diagrams showing where data can break |
| `ARCHITECTURE.md` | Need system overview — crates, topology, WebSocket protocol, recovery tiers |
| `API.md` | Need endpoint reference — all ~403 routes across 7 auth tiers |

**Order:** ERROR-CATALOG (known issue?) → LOG-LOCATIONS (find evidence) → SERVICE-REFERENCE (understand the binary) → DATA-FLOW-DIAGRAMS (trace the flow). Architecture/API are reference, not debugging entry points.

_Why: Same class as "check LOGBOOK/git/memory BEFORE investigating" (feedback_diagnostic_order.md). These docs capture the accumulated knowledge from 34 shipped milestones, 60-phase audits, and 14-model MMA runs. Investigating from scratch when the answer is already documented wastes session time and rediscovers known issues._

---

## Standing Rule #19 — V2-only forward path (v4.3, 2026-05-01, refined post-G9 #2)

**Captain directive 2026-05-01 IST: V2 is the only forward architectural path for the RacingPoint ecosystem. Every new session must be geared toward supporting and building V2.**

**V2 incorporates V1 modules** per `comms-link/v2-skeleton/05-definition-of-done.md` keep/mold/discard filter. Explicit carry-forwards: currency unit (rupee=credit) · top-up bonus-credit ladder · kiosk-staff launch first iteration · V1 organs (racecontrol, comms-link, admin, whatsapp-bot, kiosk, pods, POS — V2 adds the skeleton layer atop). What V2 closes is **V1-shaped antipatterns** (organ silos without skeleton, point-to-point ad-hoc connections, manual operations bypassing ratified flows), NOT V1 components categorically.

**Pre-action V2-transport check** (mandatory for prod-touch — Server .23 / Pods 1-8 / POS .130 / Cloud apps / comms-link prod / Bono VPS prod):
1. **Q1** — Is the target classified as production?
2. **Q2** — Grep `reference_local_capabilities.md` for ratified V2 transport: bono comms-link relay `localhost:8766/relay/exec/run`, `/rp-bono-exec` skill, `/rp-james-exec` skill, `ssh server` alias, rc-sentry `:8091/exec` pod-side.
3. **Q3** — Use it. If no V2 transport exists, halt and ask Captain — **never invent a V1 fallback.**

**Why:** Direct host-shell from a pilot to prod bypasses ratified relay, skips substrate-immutability, ignores transport-channel doctrine. V1-shaped operations are not authorized even when faster.

**Composes with:** Rule 0 (Enumerate Before Asserting) · H4 (target enumeration) · PACT-027 §10 (PACT-discipline hooks) · AMEND-1 bundle-of-8 RATIFIED 2026-05-01 ~06:05 IST (substrate-immutability active). Charter doctrine: `comms-link/PACT-CHARTER.md` §V2.0. Master memory: `feedback_v2_only_forward_path.md`.

**Empirical anchors:**
- james G9 #1 2026-05-01 IST — direct `ssh racing-point-server-1` for PACT-086 §5.2 evidence-pull denied by harness production-classifier; correct V2 path was bono-relay
- james G9 #2 2026-05-01 IST — initial doctrine wording "V1 is closed" over-broad, contradicting V2 spec; refined via spec-grep on `v2-skeleton/05-definition-of-done.md`

**Sibling-PACT candidate:** `pre-prod-touch-transport-check.js` + `pre-doctrine-codification-check.js` (PreToolUse Bash + Edit/Write hooks composing with PACT-027 §10 hook bundle).

---

## Standing Rule #20 — Wall-clock environment-fetched (v4.3, 2026-05-06, james PART 46 self-G9 + Captain H5 trigger)

**Captain H5-trigger 2026-05-06 ~12:25 IST: structural fix mandatory after destructive temporal-mental-arithmetic action.**

Before any temporal-elapsed claim ("X hours stuck", "Y minutes ago", "way past expected", "session has been going for"), the wall-clock value MUST come from environment — either the `[wall-clock]` line injected by the `wall-clock-inject.js` hook on every UserPromptSubmit, OR a fresh `date -u` invocation in the same turn. NEVER project mentally over session-start markers, earlier hook timestamps, or memory of session-elapsed.

**Hook:** `wall-clock-inject.js` (UserPromptSubmit) — emits `[wall-clock] <UTC> (<DOW> <IST>) — fetched from environment, not memory-projected` line in every prompt's additionalContext block. Always-on availability of current time.

**Why:** james 2026-05-06 PART 46 self-G9 #1 — saw `IN_PROGRESS` status on bono PR #62 CI checks at session-start, mentally projected "5 hours stuck" without running `date -u`, canceled 4 healthy CI runs as collateral (run IDs 25420136647, 25420136645, 25420315150, 25420315146). Actual elapsed at cancel-time: ~16 minutes — well within PR #61 baseline 33min build. Real cost: ~10min wall-clock + runner-minutes + 06:31:30Z PR-event run set permanently lost (cancellation irreversible). Self-G9 #2 same session: `kaizen-N=1-as-shield-from-structural-fix` — cited kaizen-correction-triage to defer the structural fix; Captain H5-triggered demand corrected the deferral. Class: foundational Verify-Before-Generate violation on temporal claims; time is environment, fetch don't project.

**Composes with:** Verify-Before-Generate principle (CLAUDE.md foundation) · `cgp-session-inject.js` (paired UserPromptSubmit hook) · H3 evidence-before-claims (temporal anchor is part of WHERE+WHEN claim discipline) · Rule 0 enumerate-before-asserting (time-claim is knowledge-coverage assertion requiring environment enumeration) · `kaizen-correction-triage` (this rule's escalation is the empirical demonstration that kaizen-N=1 deferral is not a license to skip H5 when destructive action already shipped).

**Universal Sync targets:** james `~/.claude/hooks/wall-clock-inject.js` SHIPPED + `settings.json` registered + verified harness pickup ✓ · bono `/root/.claude/hooks/wall-clock-inject.js` PENDING via comms-link git mirror · bono `~/.claude/settings.json` registration PENDING bono-session-side · CGP doc this entry · MEMORY.md index `feedback_wall_clock_mental_arithmetic_as_truth_g9_n1.md` SHIPPED · V2-MASTER-STATE §S-70 PENDING post-PR-#62-CI-settle.

---

## Predecessor

Replaces CGP v3.6 (756 lines, 147 rules, 169 gate items, 10 gates). Preserves: gates that had hard enforcement (G0→H1, Two-Phase→H2, G1→H3, G2→H4, G9→H5). Removes: gates that were declarative-only (G3, G6, G7 moved to soft; G8 consolidated into S3). Archives: all standing rules from CLAUDE.md into `docs/STANDING-RULES-ARCHIVE-v3.md` for reference.

Date: 2026-04-04. Research basis: Perplexity detailed query on RLHF sycophancy, completion bias, 23K PR study, MIT overconfidence study. Internal audit: 147 rules classified (10 essential, 30 redundant, 15 harmful, 30 obsolete, 62 consolidatable).
