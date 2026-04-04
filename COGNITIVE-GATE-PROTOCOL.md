# Cognitive Gate Protocol v4.0

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

**Anti-substitution rule (v4.2, 2026-04-04):** Testing from Machine A when the user said Machine B is not "close enough" — it's a different network path, different browser context, different DNS/proxy resolution. The bug that prompted this rule: kiosk at `:3300` works from James (Playwright) but fails from server browser because `:3300` has no API proxy — only `:8080` (racecontrol reverse proxy) routes API calls correctly. This class of bug is INVISIBLE to any test that doesn't run from the specified machine.

### H4: Target Enumeration Before "Everywhere"
**Enforcement:** Any claim containing "all," "everywhere," "fleet-wide," "every," or "complete" MUST be preceded by an explicit target list with per-target evidence.
**What:** Before saying "deployed everywhere":
1. **Grep** for all locations the change touches
2. **List** each target with evidence (command output, not assertion)
3. **Missing targets** = the claim is false. Period.

**The full target list (from MEMORY.md):**
Server .23 | Pods 1-8 | POS .20 | James .27 | Bono VPS | Cloud apps | Comms-link (James) | Comms-link (Bono)

**Why it works:** Forces enumeration before assertion. The grep IS the verification — not a formatted table you fill in after the fact.

### H5: User Corrections Are Mandatory Retrospectives
**What:** Every user correction ("good catch," "you missed," "that's wrong") triggers:
1. **Why** the error happened (root cause, not excuse)
2. **What structural change** prevents recurrence (not "I'll remember next time" — that's the bias talking)
3. **Session G9 counter** — target: 0. Report in every gate summary.

**Why it works:** Turns errors into protocol improvements. The counter creates accountability.

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

## 13 Standing Rules (consolidated from 147)

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

### How to Track
At session end, report:
```
SESSION METRICS: Claims: N | Corrections: N | FCR: N% | G9s: N | Overhead: ~N%
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

## Predecessor

Replaces CGP v3.6 (756 lines, 147 rules, 169 gate items, 10 gates). Preserves: gates that had hard enforcement (G0→H1, Two-Phase→H2, G1→H3, G2→H4, G9→H5). Removes: gates that were declarative-only (G3, G6, G7 moved to soft; G8 consolidated into S3). Archives: all standing rules from CLAUDE.md into `docs/STANDING-RULES-ARCHIVE-v3.md` for reference.

Date: 2026-04-04. Research basis: Perplexity detailed query on RLHF sycophancy, completion bias, 23K PR study, MIT overconfidence study. Internal audit: 147 rules classified (10 essential, 30 redundant, 15 harmful, 30 obsolete, 62 consolidatable).
