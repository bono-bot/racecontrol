# Cognitive Gate Protocol v3.0

**Purpose:** Single protocol governing every phase of Racing Point operations — Plan, Create, Verify, Deploy, Ship, and Debug — with 10 embedded anti-bias gates that fire at specific lifecycle moments. All standing rules, debugging methodology, audit phases, and the Multi-Model AI Audit Protocol are mapped to the phase where they activate.

**Root cause being fixed:** Task-completion bias — James treats step execution as step success, verifies mechanisms instead of outcomes, declares "done" before checking goals. 37 documented corrections over 10 days. 147+ standing rules failed because rules are declarative; gates are procedural. Gates require visible output at specific moments, making skips visible.

**Meta-gate:** The bias that causes all failures will try to skip these gates ("I already know the answer"). Writing the proof IS the fix. Thinking you know the answer without writing it IS the bias.

**Predecessors:** Merges CGP v2.1 (10 gates) + Unified Operations Protocol v3.0 (6 lifecycle phases + special phases). Date: 2026-04-01.

---

## Operating Modes

| Mode | Trigger | Active Phases | Checklist Size |
|------|---------|---------------|----------------|
| **GREEN** | Normal ops | Full lifecycle (0→5), all gates | ~40 mandatory items |
| **AMBER** | 1-2 pods down, no customer impact | Phase E fast-path, reduced gates | ~10 items |
| **RED** | 3+ pods, customers affected, server down | Phase E → Break-Glass → Island Mode | ~7 items |

**Gate Classification:** 169 gate items total. ~38 MANDATORY (every change), ~95 CONTEXTUAL (domain-triggered), ~36 MILESTONE (v-numbered releases only).

---

## The 10 Gates (Quick Reference)

| Gate | Name | Trigger | Required Proof | Phase(s) | Emergency Bypass? |
|------|------|---------|----------------|----------|-------------------|
| **G0** | Problem Definition | New non-trivial task | `PROBLEM:` + `SYMPTOMS:` + `PLAN:` block | 0, 1 | YES |
| **G1** | Outcome Verification | Before "done/fixed/PASS" | Behavior + method + raw evidence (not proxies) | 3, 5 | NO |
| **G2** | Fleet Scope | After any fix | Per-target table with evidence | 4, 5 | NO |
| **G3** | Apply Now | User shares info during active problem | Show application (command+output), not summary | D | YES |
| **G4** | Confidence Calibration | Before success claims | Tested / Not Tested (risk) / Follow-up Plan | 3, 5 | NO |
| **G5** | Competing Hypotheses | Anomalous data | 2+ hypotheses with falsification tests | D, 3 | YES |
| **G6** | Context Parking | Topic change while work open | `PAUSED:` + `STATUS:` + `NEXT:` + `RESUME BY:` | Any | YES |
| **G7** | Tool Verification | Before selecting tool/approach | Requirement + Tool + Compatibility Check | 2, 4 | YES |
| **G8** | Dependency Cascade | Before deploying shared changes | Changed component + downstream + verification | 2, 4 | YES |
| **G9** | Retrospective | After resolving issue (>3 exchanges) | Root cause + prevention + similar past | D exit | YES |

**Gate Summary Block** — required at end of any completion claim:
```
GATES TRIGGERED: [...] | PROOFS: [Y/N each] | SKIPPED: [reason]
```

---

## Part 1: Emergency & Special Phases

### Phase E: EMERGENCY FAST-PATH

**Overrides all other phases.** When customers are affected RIGHT NOW, skip the lifecycle and stabilize first.

**Activation:** Customer unable to race, 3+ pods offline, server unreachable with bookings within 2 hours, any tournament incident.

**The 7-Minute Recovery Protocol:**

**Minute 0-2: TRIAGE** — How many pods? Customers waiting? Server reachable?

**Minute 2-5: STABILIZE**

| Symptom | Fix |
|---------|-----|
| Pod frozen | `shutdown /r /t 5 /f` via SSH or physical |
| Game won't launch | Kill rc-agent → RCWatchdog auto-restarts in Session 1 |
| Blanking stuck | `del C:\RacingPoint\MAINTENANCE_MODE` → restart |
| Server down | `ssh ADMIN@100.125.108.37 "schtasks /Run /TN StartRCTemp"` |
| Billing broken | Paper: pod#, name, start time |
| Multiple pods | Mark bad ones out of rotation, serve on rest |
| VPS crash-looping | Check 3 Layers + Floor: Uptime Kuma → Monit → rc-doctor timer → PM2 |

**Minute 5-7: COMMUNICATE** — "5 minutes" or move customer. WhatsApp Uday if >2 pods or >15 min.

**Phase E Rules:**
1. Stabilize FIRST, investigate SECOND
2. NO gate checks during emergency (Gates 1/2/4 still apply for the fix itself)
3. ONE person decides — first responder owns all decisions
4. Log AFTER recovery — LOGBOOK entry after service restored
5. Max 15 minutes — minute 16 auto-escalates (WhatsApp Uday, pods out of rotation, paper billing, Phase D begins)
6. Post-emergency: Phase D root cause + post-incident MMA audit (D.9.1)

**Emergency gate bypass:** Gates 0/5/6/7/8/9 deferred (label: "EMERGENCY BYPASS — deferred, will complete after stabilization"). Gates 1/2/4 always apply. Deferred gates completed within 1 hour.

### Phase B: BREAK-GLASS (Human Unreachable >30 min)

**AI agents CAN:** restart services, reboot pods, rollback to prev binary, clear sentinels, kill processes, commit+push code, mark pods out of rotation.

**AI agents CANNOT:** deploy NEW binary, change pricing, modify customer data, change network infra, spend >$10, promise refunds.

Escalation ladder: WhatsApp (0 min) → Phone (5 min) → Email (15 min) → Break-Glass activates (30 min). Log ALL actions taken under Break-Glass.

### Phase I: ISLAND MODE (Management Layer Down)

| Capability | Without Server | Without James | Without Both |
|------------|---------------|---------------|--------------|
| Game launch | YES (cached catalog) | YES | YES (cached) |
| Billing | NO → paper | YES | NO → paper |
| Recovery | YES (RCWatchdog) | YES | YES |
| Cloud sync | NO | YES (Bono direct) | NO |

Server-down: SSH → schtasks restart. James-down: Bono takes over via VPS. Both-down: pods self-sufficient, paper billing, notify Uday.

---

## Part 2: Lifecycle with Embedded Gates

### Phase 0: SESSION START ── G0 fires here

**Goal:** Establish ground truth before any work begins.

#### 0.1 — Fleet Health Snapshot
```bash
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[] | {pod_number, ws_connected, http_reachable, build_id, uptime_secs}'
SERVER_BUILD=$(curl -s http://192.168.31.23:8080/api/v1/health | jq -r '.build_id')
echo "Server: $SERVER_BUILD | HEAD: $(git rev-parse --short HEAD)"
```
- Check MAINTENANCE_MODE on all pods with `ws_connected: false`
- If `git log $SERVER_BUILD..HEAD -- crates/` shows changes, rebuild before new work
- `git log` before calling builds "old" — different hash ≠ outdated

#### 0.2 — Meta-Monitor Liveness
Verify healing systems are running, not just configured:
- Watchdog process alive? (`tasklist | findstr rc-watchdog`)
- Scheduled tasks registered? (`schtasks /Query`)
- Output fresh? (log recency < 5 min)

#### 0.2b — 3 Layers + Floor Test (Bono VPS)
All 4 layers must be alive: Uptime Kuma (:3001) → Monit → rc-doctor timer → PM2.

#### 0.3 — Context Recovery
Check active debug sessions, review LOGBOOK, check knowledge base for known patterns.

#### 0.4 — Session Context
Verify rc-agent in Session 1 (Console, not Services) on all pods. Check `:18924/debug` — `edge_process_count > 0` when blanked.

#### 0.5 — MMA Freshness
| Last Audit | Action |
|-----------|--------|
| < 7 days | OK |
| 7-30 days | Schedule if shipping |
| > 30 days | Run quick pre-deploy check ($0.05) |
| Never | BLOCK: full 5-model audit first |

#### ⛩️ G0: Problem Definition
**Trigger:** Upon receiving any new non-trivial task.
**Proof:**
```
PROBLEM: [restate in own words]
SYMPTOMS: [known facts/errors]
PLAN: [3-5 step approach]
```
For trivial tasks: `G0: trivial — <reason>`

#### Phase 0 Gate
- [ ] Fleet health checked, offline pods investigated
- [ ] MAINTENANCE_MODE cleared on stuck pods
- [ ] Meta-monitors confirmed alive (watchdog, auto-detect)
- [ ] 3 Layers + Floor PASS on Bono VPS
- [ ] Server build_id matches HEAD (or rebuild queued)
- [ ] LOGBOOK reviewed, active debug sessions identified
- [ ] MMA freshness checked
- [ ] G0 block produced for the task at hand

---

### Phase 1: PLAN ── G0, G6 fire here

**Goal:** Define what to build with full awareness of constraints and past failures.

#### 1.1 — Prompt Quality Gate
Verify: Clarity, Specificity, Actionability, Scope. If ANY weak → ask ONE focused question.

#### 1.2 — Past Fix Lookup
Search LOGBOOK, `git log --grep`, knowledge base before planning any fix.

#### 1.3 — Cross-System Impact
| System | Touched? | Impact |
|--------|----------|--------|
| rc-agent (8 pods) | ? | Rebuild + fleet deploy |
| racecontrol (server) | ? | Rebuild + server deploy |
| PWA/Admin/Dashboard | ? | Frontend rebuild + static verify |
| Cloud (Bono VPS) | ? | Cloud rebuild + deploy |
| Comms-link | ? | Quality Gate required |

#### 1.4 — Recovery System Awareness
If touching auto-recovery/restart/wake: graceful restart distinguishable from crash? MAINTENANCE_MODE knows why? WoL won't revive deliberately-offline pods?

#### 1.5 — Rollback Plan
One-command recovery prepared, previous binary preserved, rollback documented.

#### 1.6 — Risk Tagging [M:advisory]
Risk-sensitive areas (auth, billing, fleet exec, deploy, SQL, cross-boundary) → run quick model advisory ($0.05-0.15).

#### ⛩️ G6: Context Parking
**Trigger:** Topic change while work is open.
**Proof:**
```
PAUSED: [what I was working on]
STATUS: [specific state — not "investigating" but "tested A, eliminated, about to test B"]
NEXT: [exact next action with target and command]
RESUME BY: [condition or timestamp]
```

#### Phase 1 Gate
- [ ] Prompt quality verified
- [ ] Past fixes checked
- [ ] Cross-system impact mapped
- [ ] Recovery conflicts assessed
- [ ] Rollback plan prepared
- [ ] Risk areas tagged for MMA at Phase 3

---

### Phase 2: CREATE ── G7, G8 fire here

**Goal:** Write correct, safe code following all quality rules.

#### 2.1 — Code Quality Gates

**Rust:** No `.unwrap()` in production. Static CRT. Long-lived spawns log lifecycle. Never hold lock across `.await`. Every `::default()` on entity structs reviewed. New pod endpoints behind `require_service_key`.

**TypeScript/Next.js:** No `any`. Never read storage in useState initializer. Grep `NEXT_PUBLIC_` after env var changes. `outputFileTracingRoot` set.

**Windows/.bat:** Clean ASCII + CRLF, goto labels (no parentheses). Git Bash JSON via file, not inline. Never pipe SSH → config.

**Data:** No fake data. UI reflects config truth. DB migrations: ALTER for existing tables.

#### 2.2 — Cross-Boundary Serialization
When modifying kiosk → Rust: grep `buildLaunchArgs()` field names against struct fields. Serde silently drops unknown fields — mismatch = silent data loss.

#### 2.3 — Cascade Update Protocol
1. Grep all consumers of changed interface
2. Update each consumer
3. Repeat on each updated consumer (recursive)
4. Update OpenAPI specs, contract tests, shared types
5. Document deploy impacts

#### 2.4 — Security Pre-Check
Pre-commit hooks pass, GET public / POST protected, config via ConfigPush WS (never fleet exec), staff actions don't reuse autonomous broadcast.

#### 2.5 — Mechanical Self-Audit [M:mechanical]
**BLOCKING** — run before proceeding:
```bash
# Zero tolerance: format! SQL, secrets, unwrap in prod, lock across await
git diff HEAD~1 -- '*.rs' | grep '+.*\.unwrap()' | grep -v test | wc -l
grep -rn 'format!.*SELECT\|format!.*INSERT' crates/*/src/ --include='*.rs' | grep -v test
grep -rn '\.read()\.await\|\.write()\.await' crates/*/src/ --include='*.rs' | grep -v test
```

#### ⛩️ G7: Tool Verification
**Trigger:** Before selecting a tool, protocol, API, or approach.
**Proof:**
1. **Requirement:** [what specifically needs to happen]
2. **Tool selected:** [which tool/approach]
3. **Compatibility check:** [confirmed supports specific parameter/environment/OS — not "it's similar"]

#### ⛩️ G8: Dependency Cascade
**Trigger:** Before deploying any change to shared interfaces (APIs, configs, DB schemas, protocols).
**Proof:**
```
Changed: [component/interface]
Downstream consumers: [list all]
Verification per consumer: [how each tested]
```

#### Phase 2 Gate
- [ ] `cargo test` passes (all 3 crates)
- [ ] No unwrap/any in new code
- [ ] Cascade update completed
- [ ] Security gate passes: `node comms-link/test/security-check.js`
- [ ] `touch build.rs` after new commits
- [ ] Mechanical self-audit passed
- [ ] G7 proof for tool/approach selection
- [ ] G8 proof if shared interfaces changed
- [ ] Cross-system bridge → MMA required before Phase 4

---

### Phase 3: VERIFY ── G1, G4, G5 fire here

**Goal:** Prove the code works — not just compiles, but actually functions.

#### 3.1 — Verification Hierarchy
```
Level 1: Compilation       NECESSARY but NOT SUFFICIENT
Level 2: Unit Tests        Proves structure, not function
Level 3: Contract Tests    Proves interfaces
Level 4: Integration Tests Proves system interaction
Level 5: E2E Verification  Proves user-facing behavior
Level 6: Visual Check      Proves customer experience
Level 7: Cross-Machine     Proves real deployment
```
Every change verified at the HIGHEST applicable level.

#### 3.2 — Domain-Matched Verification

| Change Domain | Required Verification |
|---------------|----------------------|
| Display/UI | Visual check — ask user "screens correct?" |
| Billing | Real billing session test (Financial Flow E2E) |
| Network/WS | Real connection from remote machine |
| Frontend | Verify from non-server browser (POS, James) |
| Game launch | Trigger + verify INI config on pod |
| Cross-system bridge | MMA audit (3 rounds, 5+ models) + E2E |

#### 3.3 — Canary (Pod 8 First)
Deploy → verify build_id → verify EXACT fix → visual check if display → stability (5 min) → THEN fleet.

#### 3.4 — Multi-Model Code Audit [M:targeted]

**Tier A (DEFAULT, every change):** 1-2 models, diff-only, ~$0.05. Blocking on consensus P1 only.

**Tier B (RISK-TRIGGERED):** 3 models, ~$0.50-1.50. Triggers: risk-tagged in Phase 1.6, Tier A P1, >10 files, auth/billing/exec changes. Blocking on 2+ model P1.

**Tier C (MILESTONE):** 5 models + Opus review, all batches, ~$3-5. Before shipping v-numbered releases.

Model stack: Qwen3 235B (scanner) + DeepSeek V3 (code) + DeepSeek R1 (reasoner) + MiMo v2 Pro (SRE) + Gemini 2.5 Pro (security). Full spec: `.planning/specs/UNIFIED-MMA-PROTOCOL.md`.

#### 3.5 — Financial Flow E2E (if billing touched)
Trace actual currency values: create customer → topup → book → launch → end (early/normal/cancel) → verify refund/balance. Any function that UPDATEs then SELECTs same column = audit for overwrite bug.

#### ⛩️ G1: Outcome Verification
**Trigger:** Before writing "fixed", "verified", "done", "complete", "PASS".
**Proof — all 3 mandatory:**
1. **Behavior tested:** Name the specific behavior (NOT "health endpoint" or "build_id")
2. **Method of observation:** Command run + output, visual check, API call + response body. Same domain as the change.
3. **Raw evidence:** Paste actual output, or "Asked user to visually confirm — awaiting response"

Proxy metrics (health 200, build_id match) are supplementary only, never primary proof. If intermittent: state duration tested and recurrence interval.

#### ⛩️ G4: Confidence Calibration
**Trigger:** Before any success/probability/confidence claim.
**Proof — three lists:**
1. **Tested:** [specific items with evidence]
2. **Not tested:** [specific items with risk: HIGH/MED/LOW]
3. **Follow-up plan:** [plan for HIGH-risk untested items]

"Complete" is invalid if Follow-up Plan is empty and Not Tested contains HIGH-risk items.

#### ⛩️ G5: Competing Hypotheses
**Trigger:** Unexpected data, unusual values, surprising system state.
**Proof:**
```
Hypothesis A: [explanation] → Test: [specific command/check]
Hypothesis B: [explanation] → Test: [specific command/check]
Status: [which tested, which eliminated, which confirmed]
```
Single hypothesis = insufficient. Emergency override: act first during Phase E, document hypotheses after.

#### Phase 3 Gate
- [ ] Exact behavior path tested (not proxies) — G1 proof
- [ ] Domain-matched verification completed
- [ ] Pod 8 canary verified (if pod change)
- [ ] Multi-machine verification (if frontend)
- [ ] MMA audit completed (Tier A minimum)
- [ ] All consensus P1 findings resolved
- [ ] G4 confidence calibration provided
- [ ] Visual verification if display-affecting

---

### Phase 4: DEPLOY ── G2, G7, G8 fire here

**Goal:** Get verified code onto all targets safely and reversibly.

**MANDATORY:** Follow Fix-Deploy Anti-Regression Protocol at `FIX-DEPLOY-PROTOCOL.md` for ALL deploys.

#### 4.1 — Pre-Deploy
```bash
cd C:/Users/bono/racingpoint/comms-link && node test/security-check.js  # Security gate (fail-closed)
sha256sum deploy-staging/rc-agent.exe                                    # SHA256 computable
test ! -f /tmp/deploy-pod.lock                                          # No concurrent deploy
```
Clear MAINTENANCE_MODE on all targets. Verify no active billing sessions on target pods.

#### 4.2 — Build & Stage
```bash
touch crates/rc-agent/build.rs crates/racecontrol/build.rs
cargo build --release --bin rc-agent --bin racecontrol
EXPECTED=$(git rev-parse --short HEAD)
cp target/release/rc-agent.exe deploy-staging/
```

#### 4.3 — Pod Deploy (rc-agent)
Pod 8 canary first → remaining pods. Per pod: download → preserve prev → RCAGENT_SELF_RESTART sentinel → verify build_id → verify EXACT fix → deploy bat files alongside.

**NEVER:** taskkill + start in same chain. Run pod binaries on James's PC. Deploy without bat file sync.

#### 4.4 — Server Deploy (racecontrol v3.0)
Use `deploy-server.sh`. 8 steps with auto-rollback: connectivity → download (size-verified) → confirmed kill (poll 15s + port free) → atomic swap (recovery guard) → start (schtasks) → build_id (3 polls) → smoke test (4 endpoints) → cleanup.

#### 4.5 — Cloud Deploy
Via comms-link relay: `curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"git_pull"}'`

#### ⛩️ G2: Fleet Scope
**Trigger:** After fixing anything on any machine.
**Proof:**
```
| Target | Applies? | Applied? | Evidence |
|--------|----------|----------|----------|
| Server .23 | Y/N | Y/N | [command output or "N/A: reason"] |
| Pods 1-8 | Y/N | Y/N | [per-pod status] |
| POS .20 | Y/N | Y/N | ... |
| James .27 | Y/N | Y/N | ... |
| Bono VPS | Y/N | Y/N | ... |
| Cloud apps | Y/N | Y/N | ... |
```
"Applied: Yes" without evidence = gate failure. Enumerate targets from MEMORY.md, not code.

#### Phase 4 Gate
- [ ] Security gate passed pre-deploy
- [ ] Deploy lock acquired
- [ ] MAINTENANCE_MODE cleared on all targets
- [ ] Pod 8 canary verified
- [ ] All targets deployed + verified (G2 fleet scope table)
- [ ] Previous binaries preserved (72hr rollback window)
- [ ] Session 1 verified on all pods
- [ ] SHA256 verified on deployed binaries
- [ ] FDP anti-regression checklist passed

---

### Phase 5: SHIP ── G1, G2, G4 fire here

**Goal:** Confirm everything works end-to-end. This is the Ultimate Rule.

#### 5.1 — Quality Gate (Layer 1)
```bash
cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="..." bash test/run-all.sh
```
Exit 0 = PASS. Contract tests + integration + syntax + security.

#### 5.2 — E2E Live Round-Trip (Layer 2)
```bash
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"node_version"}'
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[{"command":"node_version"}]}'
curl -s http://localhost:8766/relay/health
```
All three must return valid responses with REALTIME connection mode.

#### 5.3 — Standing Rules Compliance (Layer 3)
- [ ] Auto-push clean (no unpushed commits)
- [ ] Bono notified (INBOX.md + WS message)
- [ ] Watchdog running
- [ ] Standing rules synced if CLAUDE.md changed

#### 5.4 — Multi-Model AI Audit (Layer 4) [M:gate]
For milestone ships: full Tier C audit (5 models). All consensus P1s fixed. All two-model P1s triaged. All P2s triaged (accept/reject/suppress).

Override protocol (emergency only): record who, which findings, why. Max 72hr override. Log with `[AUDIT-OVERRIDE]`.

#### 5.5 — Final Gate Proofs

**⛩️ G1 (final):** Verify the EXACT user-facing behavior works — binary running, API returns correct data, UI renders, frontend from non-server browser, `_next/static/` returns 200.

**⛩️ G2 (final):** Fleet scope table complete with evidence for all targets.

**⛩️ G4 (final):** Confidence calibration — what's tested, what's not, follow-up plan for gaps.

#### 5.6 — Commit & Communicate
```bash
# LOGBOOK entry: | timestamp IST | James | hash | summary |
git push
cd comms-link && COMMS_PSK="..." COMMS_URL="ws://srv1422716.hstgr.cloud:8765" node send-message.js "Shipped: <summary>"
# + INBOX.md entry + git push
```

#### Phase 5 Gate — Ultimate Rule v2.0 (FOUR LAYERS, NO EXCEPTIONS)

| Layer | Gate | Tool | Blocking? |
|-------|------|------|-----------|
| 1 | Quality Gate | `run-all.sh` | YES |
| 2 | E2E Round-Trip | curl commands | YES |
| 3 | Standing Rules | Manual checklist | YES |
| 4 | Multi-Model Audit | OpenRouter 5-model | YES (milestones) |

All pass → **SHIPPED**. Any fail → **DO NOT SHIP**.

---

## Part 3: Debug ── G3, G5, G9 fire here

### Phase D: DEBUG

**Entry:** Any lifecycle phase fails. Record: failing phase, symptom, system state.

#### D.0 — Cognitive Discipline

**Philosophy:** User = Reporter, Claude = Investigator. Treat own code as foreign. Implementation decisions are hypotheses. Code behavior is truth; mental model is a guess.

**Bias Guards:**
| Bias | Trap | Antidote |
|------|------|----------|
| Confirmation | Only seek supporting evidence | "What would prove me WRONG?" |
| Anchoring | First explanation becomes anchor | 3+ hypotheses before investigating ANY |
| Sunk Cost | 2 hours on one path | Every 30 min: "Starting fresh, would I still choose this?" |

**Restart conditions:** 2+ hours no progress, 3+ failed fixes, can't explain behavior, fix works but don't know why.

#### D.1 — 5-Tier Debug Order

| Tier | Method | Cost |
|------|--------|------|
| 1 | **Deterministic** — sentinels, orphans, Session 0/1, MAINTENANCE_MODE | $0 |
| 2 | **Memory** — LOGBOOK, git history, knowledge base | $0 |
| 3 | **Local Ollama** — qwen2.5:3b at .27:11434 | $0 |
| 4 | **Multi-Model** — 4 OpenRouter models parallel (~$3) | ~$3 |
| 5 | **Cloud Claude** — full Opus escalation | subscription |

**Tier 1 checklist:** MAINTENANCE_MODE? OTA_DEPLOYING? Session 0 vs 1? Edge count zero with blanked state? 3 Layers + Floor alive on VPS?

#### D.2 — 5-Step Cause Elimination

1. **Reproduce & Document** — what exactly, when, what triggered it
2. **Hypothesize** — ALL plausible causes (minimum 3, specific, falsifiable)
3. **Test & Eliminate** — one at a time, evidence not assumptions, "crash dump ≠ cause"
4. **Fix & Verify** — fix confirmed cause, reproduce original trigger, verify gone
5. **Log** — LOGBOOK: symptom, hypotheses tested, confirmed cause, fix, verification

#### D.3 — Investigation Techniques

| Situation | Technique |
|-----------|-----------|
| Large codebase | Binary Search |
| Stuck/confused | Rubber Duck |
| Complex system | Minimal Reproduction |
| Know desired output | Working Backwards |
| Used to work | Differential Debugging |
| Unknown regression commit | Git Bisect |
| Many possible causes | Comment Out Everything |
| Before any fix | Observability First |
| Paths from variables | Follow the Indirection |

#### D.4 — Racing Point-Specific Checks
- MAINTENANCE_MODE cleared before debugging restarts
- rc-agent in Session 1 (Console, not Services)
- Blanking: trigger RCAGENT_BLANK_SCREEN → edge_process_count > 0
- Game launch: ok:true ≠ agent received (check WS)
- UTC→IST conversion before counting events
- Process guard violations = stale allowlist
- cmd.exe quoting: use PID targeting or batch files
- .spawn().is_ok() ≠ child started
- Never restart explorer on NVIDIA Surround pods
- 3 Layers + Floor for VPS issues

#### ⛩️ G3: Apply Now
**Trigger:** User shares new info (link, methodology, reference) while a problem is open.
**Proof:** Show the application — exact command on exact target with exact output. Summary/comparison/rule-update without application step = gate failure.

#### ⛩️ G5: Competing Hypotheses
(See Phase 3 for full proof requirements. Also fires during Phase D anomaly detection.)

#### ⛩️ G9: Retrospective
**Trigger:** After resolving any issue requiring >3 exchanges or that triggered G5.
**Proof:**
```
ROOT CAUSE: [actual cause, not symptom]
PREVENTION: [code/config/monitoring change preventing recurrence]
SIMILAR PAST: [past incidents with same root cause — check LOGBOOK]
```

#### D.9.1 — Post-Incident MMA Audit [M:post-incident]
After any production incident: R1 + MiMo targeted audit on affected + adjacent modules. Finds near-miss patterns.

#### D.10 — Multi-Model Diagnostic Escalation [M:diagnose]
**Trigger:** Any D.0 restart condition met. 4 models in parallel:
- R1 (reasoner): logical flaws explaining all evidence
- V3 (code expert): actual vs expected code paths
- MiMo (SRE): operational state making behavior sensible
- Gemini (security): config errors, auth issues, credentials

Cross-reference: consensus (2+) → test immediately. Novel hypothesis → add to list. Contradictions → design resolving experiment.

#### D.11 — Persistent Debug State
```
.planning/debug/{slug}.md          # Active
.planning/debug/resolved/{slug}.md # Resolved
.planning/debug/knowledge-base.md  # Known patterns
```

---

## Part 4: Enforcement

### Defense in Depth (9 layers)

1. **Position:** This protocol goes at TOP of CLAUDE.md. First thing read every session.
2. **Visible proof artifacts:** Gates use structured blocks (code blocks, tables) — scannable without reading full response.
3. **Gate summary block:** Every completion claim ends with `GATES TRIGGERED: [...] | PROOFS: [Y/N] | SKIPPED: [reason]`.
4. **User as Supervisor:** Spot-check for missing proofs. "Done" without G1 proof = reject. Fix without G2 table = reject.
5. **Emergency bypass:** Phase E defers gates 0/5/6/7/8/9. Gates 1/2/4 always apply. Deferred gates completed within 1 hour.
6. **No gate is obvious enough to skip.** The bias that skips gates IS the bias being fixed.
7. **Session Bootstrap Hook:** `.claude/hooks/cgp-session-bootstrap.sh` injects gate awareness at every session start.
8. **Hard Enforcement Hook:** `~/.claude/hooks/cgp-enforce.js` (PreToolUse) denies first action tool call until G0 produced.
9. **Soft Reminder Hook:** `~/.claude/hooks/cgp-session-inject.js` (UserPromptSubmit) adds G0 reminder to every prompt.
10. **Compliance Checker:** `scripts/cgp-compliance-check.sh` validates proof blocks (exit 0 = compliant).

### Cost Controls

| Scope | Budget |
|-------|--------|
| Per change (Tier A) | ~$0.05, no limit |
| Risk-triggered (Tier B) | ~$1.50 |
| Milestone (Tier C) | ~$3-5 |
| Diagnostic escalation | ~$3 |
| Per session | $10 max without Uday |
| Monthly | $50 hard stop |

Cost ceiling hit → AI audits fall back to mechanical-only (grep-based). Never blocks shipping or recovery.

---

## Appendix: Cross-Reference

### External Documents (not merged — referenced by phase)

| Document | Lines | Referenced From |
|----------|-------|----------------|
| `FIX-DEPLOY-PROTOCOL.md` | ~545 | Phase 4 (deploy detail) |
| `.planning/specs/UNIFIED-MMA-PROTOCOL.md` | ~844 | Phase 3/5 (MMA full spec) |
| `PROTOCOL-QUICK-REF.md` | ~166 | Daily operational use |
| `AUDIT-PROTOCOL.md` | ~68 phases | Post-ship fleet audit |

### Version History

| Version | Date | Change |
|---------|------|--------|
| 1.0 | 2026-03-31 | Initial CGP. 7 gates from 37 corrections. |
| 2.0 | 2026-03-31 | MMA-hardened. 4-model audit. Added G0, G8, G9. Defense-in-depth. |
| 2.1 | 2026-03-31 | Active enforcement. Session hooks, compliance checker, inline CLAUDE.md. |
| 3.0 | 2026-04-01 | Merged with Unified Operations Protocol v3.0. Gates embedded in lifecycle phases. Single source of truth. |

### MMA Audit Trail (v2.0)

| ID | Finding | Models | Action |
|----|---------|--------|--------|
| C1 | Self-enforcement will fail | R1, V3, Qwen3, Gemini | Defense-in-depth enforcement |
| C2 | Gate proofs too vague | R1, V3, Qwen3, Gemini | Hardened G1, G2, G4, G7 |
| C3 | Need competing hypotheses | R1, V3, Gemini | G5 requires 2+ hypotheses |
| C4 | Missing dependency cascade | R1, V3, Qwen3 | Added G8 |
| C5 | No emergency bypass | V3, Qwen3, Gemini | Phase E bypass rules |
| C6 | Need external enforcement | V3, Gemini | User as Supervisor |
| C7 | Missing retrospective | R1, Gemini | Added G9 |
| C8 | Problem framing failure | Qwen3, Gemini | Added G0 |
