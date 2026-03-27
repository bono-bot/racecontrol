# Racing Point Unified Operations Protocol v2.0

**Purpose:** Single protocol that governs every phase of work — Plan, Create, Verify, Deploy, Ship, and Debug. All 147+ standing rules, debugging methodologies, audit phases, investigation techniques, and the Multi-Model AI Audit Protocol are mapped to the lifecycle phase where they activate. No rule exists outside this flow.

**Rule:** When starting ANY work (feature, fix, audit, deploy), follow this protocol from Phase 0. Skip nothing. If a phase fails, route to Phase D (Debug) before proceeding.

---

## Protocol Flow

```
Phase 0: SESSION START ──→ Phase 1: PLAN ──→ Phase 2: CREATE ──→ Phase 3: VERIFY ──→ Phase 4: DEPLOY ──→ Phase 5: SHIP
     │                        │  [M:advisory]    │  [M:mechanical]    │  [M:targeted]      │                   │  [M:gate]
     │                        │                  │                    │                    │                   │
     └─── Phase D: DEBUG ◄────┴──────────────────┴────────────────────┴────────────────────┘                   │
              │  [M:post-incident]                                                                             │
              └─── fix ──→ return to failing phase ──→ continue ──→ ─────────────────────────────────────────→ │
                                                                                                               │
                                                                                                          LOGBOOK
                                                                                                               │
                                                                                                     Phase A: AUDIT
                                                                                                     [M:full + fleet]
```

**[M:xxx]** = Multi-Model AI Audit Protocol activation point (see Phase M reference).

**Debug is an escape hatch, not a separate workflow.** When any phase fails, you enter Phase D with the failing phase as context, fix the issue, then return to continue.

**Audit is the final verification.** After shipping, run the full fleet audit (68-phase) + multi-model code audit to catch anything the lifecycle missed.

---

## Phase 0: SESSION START

**Goal:** Establish ground truth before any work begins. Every session starts here.

### 0.1 — Fleet Health Snapshot
```bash
# Check all pods
curl -s http://192.168.31.23:8080/api/v1/fleet/health | jq '.[] | {pod_number, ws_connected, http_reachable, build_id, uptime_secs}'

# Check server build vs HEAD
SERVER_BUILD=$(curl -s http://192.168.31.23:8080/api/v1/health | jq -r '.build_id')
HEAD_BUILD=$(git rev-parse --short HEAD)
echo "Server: $SERVER_BUILD | HEAD: $HEAD_BUILD"
```

**Rules activated:**
- **SR-TESTING-017:** Check MAINTENANCE_MODE on all pods with `ws_connected: false`
- **SR-TESTING-016:** Clear sentinels before debugging: `del MAINTENANCE_MODE GRACEFUL_RELAUNCH rcagent-restart-sentinel.txt`
- **SR-DEPLOY-012:** If `git log $SERVER_BUILD..HEAD -- crates/` shows `.rs` changes, rebuild before new work
- **SR-TESTING-011:** `git log` before calling builds "old" — different hash != outdated

### 0.2 — Meta-Monitor Liveness (Audit Protocol Phase 67)
Verify healing systems are actually running, not just configured:
```bash
# Watchdog process alive?
tasklist | findstr rc-watchdog
# Scheduled tasks registered?
schtasks /Query /TN CommsLink-DaemonWatchdog
schtasks /Query /TN AutoDetect-Daily
# Output fresh? (log recency < 5 min for watchdog)
```

**Rules activated:**
- **SR-TESTING-013:** Verify monitoring targets against running system, not docs
- **Audit the MONITOR, not just the MONITORED** — process running + scheduled task registered + output fresh

### 0.3 — Context Recovery
```bash
# Check for active debug sessions
ls .planning/debug/*.md 2>/dev/null | grep -v resolved

# Check LOGBOOK for recent incidents
tail -20 LOGBOOK.md

# Check knowledge base for known patterns
cat .planning/debug/knowledge-base.md 2>/dev/null | tail -30
```

**Rules activated:**
- **SR-PROCESS-010:** Learn from past fixes — check LOGBOOK + commit history before investigating
- **Knowledge Base Protocol:** Check for known-pattern matches before forming new hypotheses

### 0.4 — Session Context Verification
```bash
# Verify rc-agent Session context on pods (must be Console, not Services)
# Via rc-sentry on each pod:
# tasklist /V /FO CSV | findstr rc-agent → Session column = Console
```

**Rules activated:**
- **SR-DEPLOY (Session 1):** rc-agent MUST run in interactive desktop Session 1, not Session 0
- **Behavioral verification:** Check `:18924/debug` — `edge_process_count` > 0 when `lock_screen_state` = `screen_blanked`

### 0.5 — Multi-Model Audit Freshness Check
```bash
# When was the last multi-model audit?
ls -la audit/results/cross-model-report-*/CROSS-MODEL-REPORT.md 2>/dev/null | tail -1

# Check for untriaged findings from last audit
grep -c "UNTRIAGED\|TODO\|OPEN" audit/results/cross-model-report-*/CROSS-MODEL-REPORT.md 2>/dev/null
```

| Last Audit Age | Action |
|---|---|
| < 7 days | OK — proceed |
| 7-30 days | Note: schedule full audit this session if shipping |
| > 30 days | **WARNING:** Run at least a quick pre-deploy check (Qwen3, $0.05) before any deploy |
| Never | **BLOCK:** Run full 5-model audit before first deploy |

### Phase 0 Gate
- [ ] Fleet health checked, offline pods investigated
- [ ] MAINTENANCE_MODE cleared on any stuck pods
- [ ] Meta-monitors (watchdog, auto-detect) confirmed alive
- [ ] Active debug sessions identified or none
- [ ] Server build_id matches HEAD (or rebuild queued)
- [ ] LOGBOOK reviewed for recent context
- [ ] Multi-model audit freshness checked (< 30 days or quick audit scheduled)

**If any check fails → Phase D (Debug) with Phase 0 context**

---

## Phase 1: PLAN

**Goal:** Define what to build, with full awareness of constraints and past failures.

### 1.1 — Prompt Quality Gate
Before acting on any task:
- **Clarity:** Is the objective unambiguous?
- **Specificity:** Are the exact components/files/endpoints named?
- **Actionability:** Can work begin immediately?
- **Scope:** Are boundaries defined?

If ANY dimension is weak → ask ONE focused question before proceeding.

**Rules activated:**
- **SR-PROCESS-008:** Prompt Quality Check — ask before acting on ambiguous prompts
- **SR-PROCESS-009:** Links and References = Apply Now — apply shared references to current problem FIRST

### 1.2 — Past Fix Lookup
Before planning any fix or feature that touches an area with known issues:
```bash
# Search LOGBOOK for related incidents
grep -i "<keyword>" LOGBOOK.md

# Search commit history
git log --oneline --all --grep="<keyword>" | head -10

# Search knowledge base
grep -i "<keyword>" .planning/debug/knowledge-base.md 2>/dev/null
```

**Rules activated:**
- **SR-PROCESS-010:** Learn from past fixes before re-investigating
- **4-Tier Debug Order, Tier 2:** Memory — check LOGBOOK + commit history

### 1.3 — Cross-System Impact Analysis
For any change, identify ALL affected systems:

| System | Check | Impact |
|--------|-------|--------|
| rc-agent (8 pods) | Does this touch pod behavior? | Rebuild + fleet deploy |
| racecontrol (server) | Does this touch server endpoints/logic? | Rebuild + server deploy |
| PWA/Admin/Dashboard | Does this touch frontend? | Rebuild + static verify |
| Cloud (Bono VPS) | Does this touch sync/cloud features? | Cloud rebuild + deploy |
| Comms-link | Does this touch AI coordination? | Quality Gate required |
| OTA pipeline | Does this touch deploy/update? | OTA sentinel protocol |

**Rules activated:**
- **SR-PROCESS-002:** Cross-Process Updates — ALL apps, ALL environments
- **SR-QUALITY-005:** Cascade updates recursive — update ALL linked references
- **SR-PROCESS-003:** DB migrations must cover ALL consumers

### 1.4 — Recovery System Awareness
If the change touches any auto-recovery, auto-restart, or auto-wake logic:
- [ ] Graceful restart distinguishable from crash? (sentinel files/IPC)
- [ ] MAINTENANCE_MODE escalation knows WHY restarts happen?
- [ ] WoL won't revive deliberately-offline pods?
- [ ] Recovery tested against server downtime, not just pod failures?

**Rules activated:**
- **SR-DEBUGGING-001:** Cross-Process Recovery Awareness — recovery systems must not fight each other

### 1.5 — Rollback Plan
Before any change to critical paths (self-restart, deploy chain, process guard):
- [ ] One-command recovery prepared (Tailscale SSH + schtasks)
- [ ] Previous binary preserved (`*-prev.exe`)
- [ ] Rollback procedure documented

**Rules activated:**
- **SR-DEPLOY-008:** Have a rollback plan before deploying
- **SR-OTA-001:** Always preserve previous binary before swap

### 1.6 — Multi-Model Risk Tagging [M:advisory]
For changes touching **risk-sensitive areas**, run a targeted single-batch advisory audit:

**Risk triggers (any one = run advisory audit):**
- Auth, JWT, session management code
- Billing, wallet, financial transactions
- Fleet exec endpoints, process guard, MAINTENANCE_MODE logic
- Deploy pipeline, OTA, binary swap
- SQL queries, database migrations
- Cross-boundary serialization (kiosk → Rust)

**Advisory audit (non-blocking, ~$0.05-0.15):**
```bash
# Quick scan with cheapest model on the affected crate
export OPENROUTER_KEY="sk-or-v1-..."
MODEL="qwen/qwen3-235b-a22b-2507" BATCH="01" node scripts/multi-model-audit.js
# Review output for risk awareness — does NOT block planning
```

**Output:** Risk hotspots list fed into Phase 3 verification targets.

### Phase 1 Gate
- [ ] Prompt quality verified (clear, specific, actionable, scoped)
- [ ] Past fixes checked (LOGBOOK, git history, knowledge base)
- [ ] Cross-system impact mapped
- [ ] Recovery system conflicts assessed
- [ ] Rollback plan prepared
- [ ] Plan documented (GSD plan or inline)
- [ ] Risk-sensitive areas tagged for multi-model audit at Phase 3

**If planning reveals unknown complexity → Phase D (Debug: investigation_only) to research**

---

## Phase 2: CREATE

**Goal:** Write correct, safe code following all quality rules.

### 2.1 — Code Quality Gates (Automated)

#### Rust
- **SR-QUALITY-001:** No `.unwrap()` in production code — use `?`, `.ok()`, or match
- **SR-QUALITY-004:** Static CRT in `.cargo/config.toml` — `+crt-static`
- **SR-DEBUGGING-005:** Long-lived `tokio::spawn` tasks log lifecycle (start, first item, exit)
- Errors in new pipelines use `warn`/`error`, not `debug`
- `#[cfg(test)]` guards on all destructive functions (taskkill, netsh, sfc)

#### TypeScript / Next.js
- **SR-QUALITY-002:** No `any` type — type everything explicitly
- **SR-QUALITY-006:** Never read `sessionStorage`/`localStorage` in `useState` initializer — use `useEffect` + hydrated flag
- Frontend: grep ALL `NEXT_PUBLIC_` references after any env var change
- `outputFileTracingRoot` set in all `next.config.ts` files

#### Windows / .bat files
- **SR-QUALITY-003:** Clean ASCII + CRLF, no parentheses in if/else — use `goto` labels
- **SR-QUALITY-007:** Git Bash JSON — write payloads to file, then `curl -d @file`
- **SR-QUALITY-008:** Never pipe SSH output into config files — use `scp`
- `start` command: always use `/D C:\RacingPoint` to set CWD

#### Data Integrity
- **SR-PROCESS-007:** No fake data — use `TEST_ONLY` or `0000000000`
- **SR-QUALITY-009:** UI must reflect config truth — no hardcoded lists
- DB migrations: `ALTER TABLE ADD COLUMN` for existing tables, not just `CREATE TABLE IF NOT EXISTS`

### 2.2 — Cross-Boundary Serialization Check
When modifying any kiosk/frontend → Rust interface:
1. Grep `buildLaunchArgs()` field names against `AcLaunchParams` struct fields
2. Verify every frontend field has a matching Rust struct field
3. Serde silently drops unknown JSON fields — name mismatch = silent data loss

**Rules activated:**
- **Cross-Boundary Serialization:** Every kiosk field MUST have a matching Rust struct field

### 2.3 — Cascade Update Protocol
After ANY code change, run the recursive cascade:
1. `grep` all consumers of the changed interface/file/endpoint
2. Update each consumer
3. For each consumer updated, repeat step 1 on THAT consumer
4. Update OpenAPI specs, contract tests, shared types
5. Document deploy impacts (cloud rebuild, pod redeploy)
6. Continue until no downstream impacts remain

**Rules activated:**
- **SR-QUALITY-005:** Cascade updates recursive
- **SR-PROCESS-002:** Cross-Process Updates

### 2.4 — Security Pre-Check
Before committing:
- [ ] No credentials in code (pre-commit hook blocks: private keys, AWS keys, .env.local, racecontrol.toml)
- [ ] GET endpoints public, POST/DELETE require staff JWT
- [ ] Process guard safe mode: allowlist override, never disable entirely
- [ ] Config push via ConfigPush WS channel, NEVER fleet exec

**Rules activated:**
- **SR-SEC-003:** Security gate passes (31 assertions)
- **SR-SEC-004:** Pre-commit hooks installed
- **SR-SECURITY-001/002:** Auth patterns
- **SR-OTA-005:** Config push never through fleet exec

### 2.5 — Test Writing
- **SR-PROCESS-001:** Refactor Second — characterization tests FIRST, verify green, THEN refactor
- Write failing test that reproduces bug (test-first debugging)
- `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`

### 2.6 — Mechanical Self-Audit [M:mechanical]
Run the mechanical checks that ALL AI models (including Opus) structurally miss.
These are **BLOCKING** — fix before proceeding.

```bash
# 1. Count unwrap/expect in changed production code
git diff HEAD~1 -- '*.rs' | grep '+.*\.unwrap()' | grep -v test | wc -l
git diff HEAD~1 -- '*.rs' | grep '+.*\.expect(' | grep -v test | wc -l

# 2. Find format! SQL (injection risk)
grep -rn 'format!.*SELECT\|format!.*INSERT\|format!.*UPDATE\|format!.*DELETE' crates/*/src/ --include='*.rs' | grep -v test

# 3. Find HTTP clients without timeout
grep -rn 'Client::new()' crates/*/src/ --include='*.rs' | grep -v test

# 4. Count untracked tokio::spawn (must have lifecycle logging)
grep -rn 'tokio::spawn' crates/*/src/ --include='*.rs' | grep -v test | wc -l

# 5. Integer overflow risk casts in changed code
git diff HEAD~1 -- '*.rs' | grep '+.*as u32\|+.*as i32\|+.*as usize' | wc -l

# 6. Secrets in changed code
git diff HEAD~1 | grep -iE '(password|secret|api_key|token).*=.*"[^"]{8,}"' | grep -v test | grep -v example

# 7. Hardcoded IPs in new code (except config/README)
git diff HEAD~1 -- '*.rs' '*.ts' | grep -E '\+.*192\.168\.[0-9]+\.[0-9]+' | grep -v config | grep -v README | grep -v CLAUDE

# 8. Run cargo clippy (if available)
cargo clippy --all-targets 2>&1 | grep "^error" | head -10

# 9. Check dependencies for known vulnerabilities
cargo audit 2>&1 | grep -E "^(ID|Crate|Version|Warning)" | head -20
```

**Zero tolerance on:** format! SQL, secrets in code, unwrap in production.
**Review required on:** untracked spawn, integer casts, hardcoded IPs.

### Phase 2 Gate
- [ ] `cargo test` passes (all 3 crates)
- [ ] No `.unwrap()` in new production Rust code
- [ ] No `any` in new TypeScript code
- [ ] `.bat` files: ASCII + CRLF + goto labels (no parentheses)
- [ ] Cascade update completed (all consumers updated)
- [ ] Cross-boundary serialization verified (if applicable)
- [ ] Pre-commit hooks pass (no credential leaks)
- [ ] Security gate passes: `node comms-link/test/security-check.js`
- [ ] `touch build.rs` after new commits (SR-DEPLOY-006)
- [ ] Mechanical self-audit passed (no format! SQL, no secrets, no unwrap)

**If any test fails → Phase D (Debug) with Phase 2 context**

---

## Phase 3: VERIFY

**Goal:** Prove the code works — not just compiles, but actually functions as intended.

### 3.1 — The Verification Hierarchy

```
Level 1: Compilation        cargo check / tsc           NECESSARY but NOT SUFFICIENT
Level 2: Unit Tests         cargo test                  Proves structure, not function
Level 3: Contract Tests     comms-link test suite       Proves interfaces
Level 4: Integration Tests  Live daemon tests           Proves system interaction
Level 5: E2E Verification   Live round-trip             Proves user-facing behavior
Level 6: Visual Check       Human eyes on screen        Proves customer experience
Level 7: Cross-Machine      From non-server browser     Proves real deployment
```

**Every change must be verified at the HIGHEST applicable level.** Compilation alone has been wrong 9 times.

### 3.2 — Exact Behavior Path Verification (MANDATORY)
After every fix/change, test the EXACT data flow that was affected:

```
input → transform → parse → decision → action → output
```

Do NOT substitute proxy metrics:
- Health endpoint OK ≠ bug fixed
- Build ID matches ≠ bug fixed
- `cargo test` passes ≠ bug fixed

**Rules activated:**
- **SR-TESTING-001:** Verify the EXACT behavior path, not proxies
- **SR-DEBUGGING-004:** "Shipped" means works for the user — runtime verification, not compilation

### 3.3 — Domain-Matched Verification
The verification domain MUST match the change domain:

| Change Domain | Required Verification |
|---------------|----------------------|
| Display/UI/Blanking | Visual check — ask user "are screens correct?" |
| Billing | Real billing session test |
| Network/WebSocket | Real connection from remote machine |
| Frontend | Verify from non-server browser (POS, James) |
| Game launch | Trigger launch + verify INI config on pod |
| Process guard | Check first scan result (not "everything" or "nothing") |

**Rules activated:**
- **SR-ULTIMATE-002:** Visual verification for display-affecting deploys
- **SR-DEBUGGING-004:** Frontend: verify from user's browser, not server
- **SR-PROCESS-006:** First-run verification after enabling any guard/filter/blocklist

### 3.4 — Canary Deployment (Pod 8)
ALL pod changes go to Pod 8 first:
1. Deploy to Pod 8 only
2. Verify build_id matches
3. Verify the EXACT fix (not just health)
4. Visual check if display-affecting
5. Wait for stability (no crash in 5 min)
6. THEN deploy to remaining pods

**Rules activated:**
- **SR-DEPLOY-005:** Test before upload — Pod 8 canary first
- **SR-TESTING-007:** Test display changes on ONE pod before fleet-wide

### 3.5 — Multi-Machine Verification
After frontend deploys:
1. Verify from server (localhost) — baseline
2. Verify from James's browser (192.168.31.23:3200) — LAN
3. Verify from POS (192.168.31.20) — different machine
4. Check `_next/static/` URL returns 200, not 404
5. WebSocket connects (not just REST)

**Rules activated:**
- **SR-DEBUGGING-004:** Frontend verified from non-server browser
- `NEXT_PUBLIC_` env vars baked at build time — rebuild with correct LAN IP

### 3.6 — Regression Testing
After every fix:
1. Identify adjacent functionality (what else uses the changed code?)
2. Test each adjacent area
3. Run existing test suites
4. Added regression test prevents recurrence

### 3.7 — Stability Testing
For intermittent bugs: test multiple times (50+), zero failures required.
For race conditions: add random delays, run 1000 times.
"It seems more stable" = NOT verified.

### 3.8 — Multi-Model AI Code Audit [M:targeted]

**Tiered approach** — don't run 5 models on every change. Escalate based on risk.

#### Tier A: Lightweight Diff Audit (DEFAULT — every change)
**1-2 models, diff-only, ~$0.05-0.20, ~3-5 min**
```bash
export OPENROUTER_KEY="sk-or-v1-..."

# Fast scan with cheapest model on changed files only
MODEL="qwen/qwen3-235b-a22b-2507" DIFF_ONLY=true node scripts/multi-model-audit.js

# If any P1 found, escalate to Tier B
```

**Blocking:** Consensus P1 findings only.
**Non-blocking:** Everything else (logged for triage at Ship).

#### Tier B: Targeted Multi-Model Audit (RISK-TRIGGERED)
**3 models, affected batch only, ~$0.50-1.50, ~10 min**

**Triggers for Tier B** (any one = run):
- Change tagged as risk-sensitive in Phase 1.6
- Tier A found any P1 or repeated P2 pattern
- PR touches >10 files or crosses crate boundaries
- Change modifies auth, billing, exec, deploy, or process guard

```bash
# Run 3 models in parallel on affected batch
MODEL="qwen/qwen3-235b-a22b-2507" BATCH="02" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-chat-v3-0324" BATCH="02" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-r1-0528" BATCH="02" node scripts/multi-model-audit.js &
wait

# Cross-reference
node scripts/cross-model-analysis.js --batch 02
```

**Blocking:** 2+ models agree on P1 = BLOCK until resolved.
**Soft-gate:** 2-model P1 or consensus P2 = requires explicit triage comment (accept/reject/suppress).
**Non-blocking:** Single-model findings = informational, logged for Ship gate.

#### Tier C: Full 5-Model Audit (MILESTONE/MAJOR RELEASE)
**5 OpenRouter models + Opus review, all 7 batches, ~$3-5, ~30 min**

**Triggers for Tier C:**
- Before shipping any milestone (v-numbered release)
- After security incident
- Monthly maintenance schedule
- New crate or service added

```bash
# Run all 5 OpenRouter models in parallel
MODEL="qwen/qwen3-235b-a22b-2507" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-chat-v3-0324" node scripts/multi-model-audit.js &
MODEL="xiaomi/mimo-v2-pro" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js &
MODEL="google/gemini-2.5-pro-preview-03-25" node scripts/multi-model-audit.js &
wait

# Full cross-model analysis
node scripts/cross-model-analysis.js

# Opus review of CROSS-MODEL-REPORT.md (manual)
```

**All consensus findings must be resolved. All two-model findings must be triaged.**

#### OpenRouter Model Stack (5 models, ~$3-5 total)

| Slot | Model | OpenRouter ID | Role | Cost/Audit |
|---|---|---|---|---|
| **Scanner** | Qwen3 235B | `qwen/qwen3-235b-a22b-2507` | Ultra-cheap exhaustive enumeration | ~$0.05 |
| **Code Expert** | DeepSeek V3 | `deepseek/deepseek-chat-v3-0324` | Deep code pattern matching | ~$0.16 |
| **Reasoner** | DeepSeek R1 | `deepseek/deepseek-r1-0528` | Absence detection, logic bugs, state machines | ~$0.43 |
| **SRE** | MiMo v2 Pro | `xiaomi/mimo-v2-pro` | Operational gaps, stuck states, timeouts | ~$0.77 |
| **Security** | Gemini 2.5 Pro | `google/gemini-2.5-pro-preview-03-25` | Security checklists, credential scanning | ~$1.65 |
| **Reviewer** | Opus 4.6 | Claude Code subscription | Cross-system architecture, false positive filtering | $0 |

#### Model Strengths (which model catches what)

| Bug Class | Best Model | Why |
|---|---|---|
| Hardcoded credentials | Gemini | Security checklist training |
| Auth gaps on endpoints | Gemini, MiMo | Pattern matching on routes |
| SQL injection | Gemini, V3 | Code pattern detection |
| Missing DB transactions | Gemini, MiMo, V3 | Financial code pattern |
| Serde silent drops | **Opus only** | Requires cross-boundary knowledge |
| State machine stuck states | R1, MiMo | Reasoning about transitions |
| Absence-based bugs | R1 | Chain-of-thought "what should be here" |
| Recovery system conflicts | **Opus only** | Multi-system architecture knowledge |
| Windows Session 0/1 | V3 | OS-specific code pattern |
| Timing/race conditions | R1, V3 | Reasoning + code analysis |
| Operational completeness | MiMo | SRE "what breaks at 3am" thinking |
| Volume coverage | Qwen3 | Cheapest, produces most findings |

#### Consensus Logic (Deterministic Gate Rules)

| Finding Type | Definition | Action | Blocking? |
|---|---|---|---|
| **Consensus** | 3+ models agree on same issue | Fix immediately | **YES** |
| **Two-model** | 2 models corroborate | Verify against code, triage | **YES for P1, soft-gate for P2** |
| **Unique** | 1 model only | Opus review for false positive filtering | No (informational) |

#### False Positive Handling

Maintain `audit/suppress.json` with time-bound suppressions:
```json
{
  "suppressions": [
    {
      "pattern": "ws:// should be wss://",
      "reason": "Tailscale already encrypts the tunnel",
      "expires": "2026-06-27",
      "added_by": "james"
    }
  ]
}
```

**Known false positives by model:**
- Gemini: "Rust edition 2024 invalid" (stale training), LAN-only endpoints flagged as critical
- Qwen3: Duplicate findings (same bug, different wording) — dedupe by file:line
- All models: "ws:// should be wss://" on Tailscale, "ALLOWED_BINARIES has dangerous commands"
- R1: Over-detailed reasoning restating the obvious
- MiMo: "Missing health check" when one exists — verify with grep

### Phase 3 Gate
- [ ] Exact behavior path tested (not proxies)
- [ ] Domain-matched verification completed
- [ ] Pod 8 canary verified (if pod change)
- [ ] Multi-machine verification (if frontend change)
- [ ] Regression tests pass
- [ ] Stability confirmed (if intermittent bug)
- [ ] Visual verification (if display-affecting) — user confirmed screens correct
- [ ] Multi-model audit completed (Tier A minimum, Tier B/C if risk-triggered)
- [ ] All consensus P1 findings resolved
- [ ] All two-model P1 findings triaged

**If verification fails → Phase D (Debug) with Phase 3 context**

---

## Phase 4: DEPLOY

**Goal:** Get verified code onto all target machines safely and reversibly.

### 4.1 — Pre-Deploy Checks
```bash
# Security gate
cd C:/Users/bono/racingpoint/comms-link && node test/security-check.js

# Release manifest (if OTA)
test -f deploy-staging/release-manifest.toml

# No active billing sessions on target pods
# Check via fleet health — pods with active sessions defer

# OTA sentinel clear
test ! -f C:/RacingPoint/OTA_DEPLOYING
```

**Rules activated:**
- **SR-SEC-003:** Security gate passes before any deploy
- **SR-SEC-005:** Deploy scripts enforce security gate
- **SR-OTA-002:** Never deploy without signed manifest
- **SR-OTA-003:** Billing sessions must drain before swap
- **SR-OTA-004:** OTA sentinel protocol

### 4.2 — Build & Stage
```bash
# Touch build.rs to force fresh GIT_HASH
touch crates/rc-agent/build.rs crates/racecontrol/build.rs

# Build
cargo build --release --bin rc-agent
cargo build --release --bin racecontrol

# Record expected build_id BEFORE staging
EXPECTED_BUILD=$(git rev-parse --short HEAD)
echo "Expected: $EXPECTED_BUILD"

# Copy to staging
cp target/release/rc-agent.exe deploy-staging/
cp target/release/racecontrol.exe deploy-staging/

# Start HTTP server for pod downloads
# python -m http.server 9998 --directory deploy-staging
```

**Rules activated:**
- **SR-DEPLOY-006:** Touch build.rs before release builds
- **SR-DEPLOY-010:** Deploy staging path is `C:\Users\bono\racingpoint\deploy-staging`

### 4.3 — Pod Deploy Sequence (rc-agent)
**Pod 8 canary first, then remaining pods.**

For each pod:
1. Download: `curl.exe -s -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe`
2. Preserve previous: rename current to `rc-agent-prev.exe`
3. Trigger restart: write `RCAGENT_SELF_RESTART` sentinel
4. Wait for restart (rc-agent calls `relaunch_self()` → bat swaps + starts new)
5. Verify build_id: `curl -s http://<pod_ip>:8090/health | jq '.build_id'`
6. Verify the EXACT fix
7. Also deploy current `start-rcagent.bat` + `start-rcsentry.bat`

**NEVER:**
- `taskkill /F /IM rc-agent.exe` + `start` in same exec chain (SR-DEPLOY-002)
- Run pod binaries on James's PC (SR-DEPLOY-004)
- Deploy without bat file sync (regression prevention)

**Rules activated:**
- **SR-DEPLOY-001:** Remote deploy via RCAGENT_SELF_RESTART sentinel
- **SR-DEPLOY-002:** Never taskkill + start in same chain
- **SR-DEPLOY-005:** Pod 8 canary first
- **SR-DEPLOY-007:** Smallest reversible fix first
- **SR-DEPLOY-014:** Single-binary-tier policy — same binary on all pods
- **SR-OTA-001:** Preserve previous binary
- **SR-OTA-006:** Rollback window: 72 hours minimum
- **Regression Prevention:** Deploy bat files alongside binaries
- **Process multiplication:** Kill-all before start-one in bat files

### 4.4 — Server Deploy Sequence (racecontrol)
7 steps, no shortcuts:
1. Record expected build_id: `git rev-parse --short HEAD`
2. Download first (while old process still serves :8090)
3. SSH kill+swap: rename trick (`ren` running exe, not `move /Y`)
4. Start via `schtasks /Run /TN StartRCTemp`
5. Verify build_id matches
6. Verify the EXACT fix (not just health)
7. If fail → recover via SCP + schtasks

**Rules activated:**
- **SR-DEPLOY-003:** Server deploy 7-step procedure
- **SR-DEPLOY-013:** Server binary swap via rename, never overwrite running exe
- **SR-DEPLOY-009:** Tailscale SSH fallback for recovery

### 4.5 — Cloud Deploy (Bono VPS)
```bash
# Via comms-link relay
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"template":"deploy-bono"}'
# OR manual steps:
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"git_pull","reason":"deploy"}'
```

**Rules activated:**
- **SR-COMMS-003:** Bono VPS exec via relay, not SSH

### 4.6 — Post-Deploy Verification
Per pod:
- [ ] Build_id matches expected
- [ ] rc-agent in Session 1 (Console, not Services)
- [ ] If display-affecting: trigger `RCAGENT_BLANK_SCREEN` → verify `edge_process_count > 0` at `:18924/debug`
- [ ] If game-related: trigger test launch → verify INI config content on pod
- [ ] `start-rcagent.bat` matches repo version

### Phase 4 Gate
- [ ] Security gate passed pre-deploy
- [ ] Pod 8 canary deployed + verified
- [ ] All target pods deployed + verified
- [ ] Server deployed + verified (if applicable)
- [ ] Cloud deployed + verified (if applicable)
- [ ] Bat files synced to all pods
- [ ] Previous binaries preserved
- [ ] OTA sentinel cleared

**If deploy fails → Phase D (Debug) with Phase 4 context. NEVER force through.**

---

## Phase 5: SHIP

**Goal:** Confirm everything works end-to-end and mark as shipped. This is the Ultimate Rule.

### 5.1 — Quality Gate (Automated Tests)
```bash
cd C:/Users/bono/racingpoint/comms-link && COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" bash test/run-all.sh
```
- Suite 0: Gate check
- Suite 1: Contract tests (15)
- Suite 2: Integration tests (4, live daemon)
- Suite 3: Syntax check (35 files)
- Suite 4: Security check (31 assertions)

**Exit 0 = PASS. Any other exit = BLOCKED.**

### 5.2 — E2E Live Round-Trip
```bash
# Single exec
curl -s -X POST http://localhost:8766/relay/exec/run -d '{"command":"node_version"}'

# Chain
curl -s -X POST http://localhost:8766/relay/chain/run -d '{"steps":[{"command":"node_version"}]}'

# Health + connection mode
curl -s http://localhost:8766/relay/health
```

All three must return valid responses with REALTIME connection mode.

### 5.3 — Standing Rules Compliance
- [ ] Auto-push clean (no unpushed commits)
- [ ] Bono notified (INBOX.md + WS message)
- [ ] Watchdog running (process alive + task registered)
- [ ] Rules categorized in registry
- [ ] Standing rules synced to Bono if CLAUDE.md changed

**Rules activated:**
- **SR-ULTIMATE-001:** Three verification layers
- **SR-COMMS-002:** Auto-push + notify atomic sequence
- **SR-COMMS-004:** Standing rules sync after CLAUDE.md changes

### 5.4 — Multi-Model Audit Gate [M:gate] (4th Shipping Gate)
**This is the 4th mandatory shipping gate. The Ultimate Rule now has FOUR layers.**

For milestone ships (v-numbered releases):
```bash
# If Tier C audit was not run at Phase 3, run it now
ls audit/results/cross-model-report-$(date +%Y-%m-%d)/ 2>/dev/null || {
  echo "No audit today — running full 5-model audit..."
  export OPENROUTER_KEY="sk-or-v1-..."
  MODEL="qwen/qwen3-235b-a22b-2507" node scripts/multi-model-audit.js &
  MODEL="deepseek/deepseek-chat-v3-0324" node scripts/multi-model-audit.js &
  MODEL="xiaomi/mimo-v2-pro" node scripts/multi-model-audit.js &
  MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js &
  MODEL="google/gemini-2.5-pro-preview-03-25" node scripts/multi-model-audit.js &
  wait
  node scripts/cross-model-analysis.js
}
```

**Triage requirements before shipping:**

| Finding Type | Triage Requirement | Ship Blocker? |
|---|---|---|
| Consensus P1 (3+ models, critical) | Must be FIXED | **YES — absolute block** |
| Consensus P2 (3+ models, important) | Must be FIXED or RISK-ACCEPTED with justification | **YES** |
| Two-model P1 | Must be FIXED or verified FALSE POSITIVE | **YES** |
| Two-model P2 | Must be TRIAGED (accept/reject/suppress) | Soft block |
| Unique P1 | Opus review required | Soft block |
| Unique P2/P3 | Logged, tracked | No |

**Override protocol (emergency only):**
- Record: who overrode, which findings, why, linked ticket
- Maximum override duration: 72 hours
- Must be closed by addressing findings or revising policy
- Log in LOGBOOK.md with `[AUDIT-OVERRIDE]` tag

**Metrics to track:**
- AI-found bugs that would have escaped without audit
- False positive rate per model (feeds back into suppress.json)
- Time-to-triage per finding category
- Override rate and resolution time

### 5.5 — Visual Verification (if display-affecting)
- [ ] Screens showing correctly on all affected pods?
- [ ] No flicker/misalignment/rendering issues?
- [ ] User physically verified?

**Rules activated:**
- **SR-ULTIMATE-002:** Visual verification for display-affecting deploys

### 5.6 — "Shipped Means Works For User" Checklist
- [ ] Binary running (not just compiled)
- [ ] API endpoints return correct data (not just HTTP 200)
- [ ] UI pages render and are interactive
- [ ] Frontend verified from non-server browser
- [ ] All `NEXT_PUBLIC_` vars have values
- [ ] `_next/static/` returns 200 (not 404)
- [ ] Hardware integrations tested with live data

### 5.7 — Cascade Audit
After the session's changes:
1. List EVERY change made
2. For each, identify all downstream consumers
3. Test each consumer — not just "running" but "correct output"
4. Document pending restarts and their impact

**Rules activated:**
- **SR-TESTING-018:** Cascade-audit before closing

### 5.8 — Commit & Communicate
```bash
# LOGBOOK entry (MANDATORY per SR-PROCESS-011)
# Append: | timestamp IST | James | hash | summary |

# Git push
git push

# Notify Bono (dual channel)
cd C:/Users/bono/racingpoint/comms-link
COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" COMMS_URL="ws://srv1422716.hstgr.cloud:8765" node send-message.js "Shipped: <summary>"
# + append to INBOX.md + git push
```

**Rules activated:**
- **SR-PROCESS-011:** LOGBOOK entry after every commit
- **SR-COMMS-001:** Bono INBOX.md dual channel
- **SR-COMMS-002:** Auto-push + notify atomic

### Phase 5 Gate (ULTIMATE RULE v2.0 — FOUR LAYERS — NO EXCEPTIONS)

**The Ultimate Rule now requires FOUR verification layers:**

| Layer | Gate | Tool | Blocking? |
|---|---|---|---|
| **1. Quality Gate** | Automated tests (contract + integration + syntax + security) | `run-all.sh` | YES |
| **2. E2E** | Live round-trip (exec + chain + health) | `curl` commands | YES |
| **3. Standing Rules** | Auto-push, Bono synced, watchdog, rules categorized | Manual checklist | YES |
| **4. Multi-Model AI Audit** | Cross-model consensus findings triaged | OpenRouter 5-model stack | YES (for milestones) |

- [ ] **Layer 1:** Quality Gate passed (run-all.sh exit 0)
- [ ] **Layer 2:** E2E round-trip verified (exec + chain + health)
- [ ] **Layer 3:** Standing rules compliance confirmed
- [ ] **Layer 4:** Multi-model audit — all consensus/two-model P1s resolved, all P2s triaged
- [ ] Visual verification (if display-affecting)
- [ ] "Works for user" checklist complete
- [ ] Cascade audit done
- [ ] LOGBOOK updated
- [ ] Git pushed + Bono notified

**All checks pass → SHIPPED. Any fail → DO NOT SHIP. Fix first.**

_Why Layer 4: v18.0 shipped with 8 integration bugs that 135 unit tests missed. Multi-model audit on 2026-03-27 found 48 additional bugs across the entire codebase — 7 critical P1s that no single model (including Opus) caught independently. The multi-model diversity catches what homogeneous testing cannot._

---

## Phase D: DEBUG

**Goal:** Find root cause through scientific method, fix it, and return to the failing lifecycle phase.

### Entry Point
Phase D is entered when any lifecycle phase fails. Record:
- **Failing phase:** Which phase triggered debug? (0-5)
- **Symptom:** What exactly failed?
- **Context:** What was the system state?

### D.0 — Cognitive Discipline (ALWAYS ACTIVE)

#### Philosophy
- User = Reporter, Claude = Investigator
- Treat your own code as foreign — read as if someone else wrote it
- Your implementation decisions are hypotheses, not facts
- The code's behavior is truth; your mental model is a guess
- "I implemented this wrong" — not "requirements were unclear"

#### Bias Guards

| Bias | Trap | Antidote |
|------|------|----------|
| **Confirmation** | Only seek supporting evidence | "What would prove me WRONG?" |
| **Anchoring** | First explanation becomes anchor | Generate 3+ hypotheses before investigating ANY |
| **Availability** | Recent bugs → assume similar | Treat each bug as novel until evidence says otherwise |
| **Sunk Cost** | 2 hours on one path, keep going | Every 30 min: "Starting fresh, would I still choose this?" |

#### Restart Conditions
Start over when:
1. 2+ hours with no progress
2. 3+ "fixes" that didn't work
3. Can't explain the current behavior
4. Debugging the debugger
5. Fix works but you don't know why

**Restart protocol:** Close all → write what you know for certain → write what's ruled out → list NEW hypotheses → begin fresh from D.1.

### D.1 — 4-Tier Debug Order (WHERE to look)

| Tier | Method | When | Action |
|------|--------|------|--------|
| **1** | **Deterministic** | Always first | Stale sockets, game cleanup, temp files, WerFault, MAINTENANCE_MODE sentinel — apply without LLM |
| **2** | **Memory** | After Tier 1 fails | Check LOGBOOK.md + commit history + knowledge base for identical past incident |
| **3** | **Local Ollama** | After Tier 2 fails | Query qwen2.5:3b at James .27:11434 |
| **4** | **Cloud Claude** | Last resort | Escalate — NOT auto-triggered |

**Tier 1 checklist (deterministic):**
- [ ] MAINTENANCE_MODE sentinel present? → Clear it
- [ ] OTA_DEPLOYING sentinel present? → Clear or investigate
- [ ] Stale sockets/connections? → Restart service
- [ ] Game cleanup needed? → Kill orphan processes
- [ ] WerFault dialogs? → Dismiss + investigate crash dump
- [ ] Temp files blocking? → Clean temp dirs
- [ ] Session 0 vs Session 1? → Check with `tasklist /V /FO CSV | findstr rc-agent`
- [ ] Edge process count zero with blanking state? → Session context wrong

### D.2 — 5-Step Cause Elimination (HOW to reason)

**MANDATORY for all non-trivial bugs. Never jump from symptom to fix.**

#### Step 1: Reproduce & Document Symptom
- What EXACTLY happened? (user's words, screenshot, error message)
- When? What action triggered it? System state?
- Can you reproduce it now?

#### Step 2: Hypothesize (list ALL possible causes)
Write down EVERY plausible cause:
- Software bug (logic error, state management, serialization)
- Hardware (USB, display, network cable, RAM)
- Configuration (TOML, env vars, registry, bat files)
- Network (Tailscale down, WebSocket dropped, port blocked)
- User error (wrong command, wrong pod, wrong sequence)
- Interaction between systems (recovery cascade, WoL + MAINTENANCE_MODE)

**Minimum 3 hypotheses. Each must be SPECIFIC and FALSIFIABLE.**

Bad: "Something is wrong with the state"
Good: "User state resets because component remounts when route changes"

#### Step 3: Test & Eliminate (one at a time)
For each hypothesis:
1. **Prediction:** If H is true, I will observe X
2. **Test setup:** What do I need to do?
3. **Measurement:** What exactly am I measuring?
4. **Success criteria:** What confirms? What refutes?
5. **Run:** Execute ONE test
6. **Observe:** Record what actually happened
7. **Conclude:** Support or refute?

Cross off eliminated causes with EVIDENCE, not assumptions.
"Found a crash dump" ≠ "found the cause" — correlation is not causation.

#### Step 4: Fix & Verify
- Apply fix for the CONFIRMED cause
- Reproduce the ORIGINAL trigger
- Verify the bug is actually gone
- Visual verification for UI/display issues

#### Step 5: Log
Record in LOGBOOK.md:
```
| timestamp IST | James | commit_hash | Symptom: X. Hypotheses: A (eliminated: evidence), B (eliminated: evidence), C (confirmed: evidence). Fix: Y. Verified: Z. |
```

### D.3 — Investigation Techniques (9 methods)

Select based on situation:

| Situation | Technique | Key Action |
|-----------|-----------|------------|
| Large codebase, many files | **Binary Search** | Cut problem space in half repeatedly |
| Stuck, confused | **Rubber Duck** | Explain problem in complete detail — spot assumptions |
| Complex system, many parts | **Minimal Reproduction** | Strip away everything until bug is obvious |
| Know desired output | **Working Backwards** | Start from output, trace backwards through call stack |
| Used to work, now doesn't | **Differential Debugging** | What changed? Code, env, data, config? |
| Feature worked, broke at unknown commit | **Git Bisect** | Binary search through git history (~7 tests for 100 commits) |
| Many possible causes | **Comment Out Everything** | Comment body, uncomment one piece at a time |
| Before any fix attempt | **Observability First** | Add logging BEFORE changing behavior |
| Paths/URLs/keys from variables | **Follow the Indirection** | Verify producer and consumer agree on resolved value |

**Combining techniques** (common sequence):
1. Differential debugging → identify what changed
2. Binary search → narrow down where in code
3. Observability first → add logging at that point
4. Rubber duck → articulate what you're seeing
5. Minimal reproduction → isolate just that behavior
6. Working backwards → find the root cause

### D.4 — Evidence Quality

**Strong evidence (act on this):**
- Directly observable ("I see in logs that X happens")
- Repeatable ("This fails every time I do Y")
- Unambiguous ("The value is definitely null, not undefined")
- Independent ("Happens even in fresh environment")

**Weak evidence (investigate more):**
- Hearsay ("I think I saw this fail once")
- Non-repeatable ("It failed that one time")
- Ambiguous ("Something seems off")
- Confounded ("Works after restart AND cache clear AND update")

**Decision point — act when ALL are YES:**
1. Understand the mechanism? (not just "what fails" but "why")
2. Reproduce reliably? (always, or understand trigger conditions)
3. Have evidence, not just theory?
4. Ruled out alternatives?

### D.5 — Hypothesis Testing Pitfalls

| Pitfall | Problem | Solution |
|---------|---------|----------|
| Multiple hypotheses at once | Can't tell which change fixed it | One hypothesis, one test at a time |
| Confirmation bias | Only looking for supporting evidence | Actively seek DIS-confirming evidence |
| Acting on weak evidence | "It seems like maybe..." | Wait for strong, unambiguous evidence |
| Not documenting results | Repeat experiments | Write down each hypothesis + result |
| Abandoning rigor under pressure | "Let me just try this..." | Double down on method when pressure increases |

### D.6 — Research vs Reasoning

```
Is this an error I don't recognize?
├─ YES → Web search exact error message
└─ NO ↓

Is this library/framework behavior I don't understand?
├─ YES → Check official docs
└─ NO ↓

Is this code I/we wrote?
├─ YES → Reason through it (logging, tracing, hypothesis testing)
└─ NO ↓

Is this a platform/environment difference?
├─ YES → Research platform-specific behavior
└─ NO ↓

Can I observe the behavior directly?
├─ YES → Add observability and reason through it
└─ NO → Research the domain/concept first, then reason
```

### D.7 — Verification Patterns

A fix is verified when ALL are true:
1. Original issue no longer occurs (exact reproduction steps → correct behavior)
2. You understand WHY the fix works (can explain the mechanism)
3. Related functionality still works (regression tests pass)
4. Fix works across environments (not just your machine)
5. Fix is stable (works consistently, not "worked once")

**Red flags:** "It seems to work", "I think it's fixed", "Looks good to me"
**Trust phrases:** "Verified 50 times — zero failures", "Root cause was X, fix addresses X directly"

### D.8 — Racing Point-Specific Debug Checks

These are ADDITIONAL checks specific to our fleet, applied on top of the general methodology:

- [ ] **MAINTENANCE_MODE:** Clear on all affected pods before debugging restarts
- [ ] **Session context:** rc-agent in Session 1 (Console), not Session 0 (Services)
- [ ] **Blanking behavioral test:** Trigger `RCAGENT_BLANK_SCREEN` → `edge_process_count > 0` at `:18924/debug`
- [ ] **Game launch:** `ok: true` ≠ agent received command — check WS delivery
- [ ] **UTC→IST:** Convert ALL timestamps before counting events or diagnosing timing
- [ ] **Process guard:** Violation count increasing = stale allowlist, not real violations
- [ ] **cmd.exe quoting:** Use PID targeting or batch files, avoid cmd string interpretation
- [ ] **`.spawn().is_ok()`:** Does NOT mean child started — verify alive after spawn
- [ ] **Non-interactive context:** Session 0 cannot launch GUI processes (Edge, games, overlays)
- [ ] **Crash loop:** >3 startups in 5 min with uptime < 30s → reboot first, investigate second
- [ ] **Explorer restart:** NEVER on NVIDIA Surround pods (collapses triple monitors)
- [ ] **Screenshot verification:** Triggers taskbar auto-hide — verify physically instead

### D.9 — Persistent Debug State

All investigations use the debug file protocol:
```
.planning/debug/{slug}.md          # Active sessions
.planning/debug/resolved/{slug}.md  # Resolved sessions
.planning/debug/knowledge-base.md   # Known patterns for future matching
```

**File sections:**
- **Current Focus:** OVERWRITE on each update — reflects NOW
- **Symptoms:** Written during gathering, then IMMUTABLE
- **Eliminated:** APPEND only — prevents re-investigating
- **Evidence:** APPEND only — facts discovered
- **Resolution:** OVERWRITE as understanding evolves

**Critical:** Update file BEFORE taking action. If context resets, file shows what was happening.

### D.9.1 — Post-Incident Multi-Model Audit [M:post-incident]

After resolving any **production incident** (customer-facing failure, security breach, crash loop), run a targeted multi-model audit on adjacent modules to find near-miss patterns:

```bash
# Targeted audit on the crate where the incident occurred + adjacent crates
export OPENROUTER_KEY="sk-or-v1-..."
MODEL="deepseek/deepseek-r1-0528" BATCH="<affected_batch>" node scripts/multi-model-audit.js &
MODEL="xiaomi/mimo-v2-pro" BATCH="<affected_batch>" node scripts/multi-model-audit.js &
wait
```

**Why R1 + MiMo for incidents:**
- R1 excels at absence-based reasoning ("what timeout should be here but isn't?")
- MiMo thinks like an SRE ("what else breaks at 3am in this module?")

**Output:** Near-miss findings added to knowledge base, fed into next Phase 1 risk tagging.

### Phase D Exit Gate
- [ ] Root cause confirmed with evidence (not just theory)
- [ ] Fix applied (smallest reversible change)
- [ ] Original reproduction steps → correct behavior
- [ ] Mechanism understood (can explain WHY)
- [ ] Regression tests pass
- [ ] LOGBOOK entry written
- [ ] Knowledge base updated (if new pattern)
- [ ] Post-incident multi-model audit run (if production incident)
- [ ] Return to failing lifecycle phase and continue

---

## Phase A: POST-SHIP AUDIT

**Goal:** After shipping, run comprehensive fleet audit + full multi-model code audit to catch anything the lifecycle phases missed. This is the safety net.

### A.1 — Fleet Audit (68-Phase AUDIT-PROTOCOL)
Run the full automated fleet audit across all infrastructure:
```bash
cd C:/Users/bono/racingpoint/racecontrol
AUDIT_PIN=261121 bash audit/audit.sh --mode full --auto-fix --notify --commit
```

**What this covers:**
- Tier 1: Infrastructure (fleet inventory, config, network, processes, self-heal, meta-monitors)
- Tier 2: Core Services (API, WebSocket, exec, sentry, preflight)
- Tier 3: Display & UX (lock screen, overlays, resolution, kiosk)
- Tier 4: Billing (pricing, wallet, reservations, accounting)
- Tier 5: Games & Hardware (catalog, launch E2E, hardware)

**Parallel engine:** 4-concurrent-connection cap, 200ms stagger, file-based semaphore locking.
**Intelligence:** Delta tracking (6 categories), suppress.json with expiry, dual Markdown+JSON reports.
**Auto-fix:** `is_pod_idle()` billing gate, approved-fixes whitelist, per-fix audit trail.
**Notifications:** Bono dual-channel (WS + INBOX.md), WhatsApp to Uday via Evolution API.

### A.2 — Full Multi-Model Code Audit [M:full]
If not already run at Phase 3/5, run the complete 5-model audit:
```bash
export OPENROUTER_KEY="sk-or-v1-..."

# All 5 OpenRouter models in parallel across all 7 batches
MODEL="qwen/qwen3-235b-a22b-2507" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-chat-v3-0324" node scripts/multi-model-audit.js &
MODEL="xiaomi/mimo-v2-pro" node scripts/multi-model-audit.js &
MODEL="deepseek/deepseek-r1-0528" node scripts/multi-model-audit.js &
MODEL="google/gemini-2.5-pro-preview-03-25" node scripts/multi-model-audit.js &
wait

# Cross-model analysis
node scripts/cross-model-analysis.js

# Opus review
echo "Review: audit/results/cross-model-report-$(date +%Y-%m-%d)/CROSS-MODEL-REPORT.md"
```

**The 7 Audit Batches:**

| Batch | Scope | Key Files | Focus |
|---|---|---|---|
| 01 | Racecontrol Server | `crates/racecontrol/src/*.rs` | Route auth, SQL injection, billing logic, game state |
| 02 | RC-Agent | `crates/rc-agent/src/*.rs` | Exec injection, process guard, Session 0/1, Windows |
| 03 | Sentry/Watchdog/Common | `crates/rc-sentry/`, `rc-watchdog/`, `rc-common/` | Restart loops, MAINTENANCE_MODE, recovery |
| 04 | Comms-Link | `comms-link/shared/`, `james/`, `bono/` | PSK auth, exec injection, chain orchestration |
| 05 | Audit/Healing Pipeline | `audit/lib/`, `audit/phases/`, `scripts/detectors/` | Race conditions, billing gate, notifications |
| 06 | Deploy/Infra | `scripts/deploy/`, `Cargo.toml`, `.cargo/config.toml` | Pipeline integrity, credentials, binary verification |
| 07 | Standing Rules | `CLAUDE.md` (both repos) | Rule conflicts, gaps, stale references |

### A.3 — Audit Gap Analysis
After running both audits, check the gap report for systemic issues:
```bash
# Review the meta-audit for protocol-level gaps
cat audit/AUDIT-GAP-REPORT-2026-03-27.md | head -100
```

**8 gap categories to monitor:**
1. Audit Trust & Integrity (circular dependency, credential exposure, self-test)
2. Security Blind Spots (physical, rate limiting, network segmentation, supply chain)
3. Resilience & Chaos (failure mode testing, power recovery, incident drills)
4. Data & Backup (backup verification, DB integrity, privacy compliance)
5. Performance & Capacity (baselines, disk, load testing, dependency health)
6. Protocol Consistency (undocumented phases, numbering, duplicates)
7. Code Audit Gaps (frontend not in batches, no calibration, no defect tracking)
8. Operations & Business (NTP, SLOs, asset inventory, change management)

### A.4 — Bono Coordination
Push results to git for Bono's review:
```bash
git add audit/results/
git commit -m "audit: post-ship fleet + multi-model audit results"
git push

# Notify Bono
cd C:/Users/bono/racingpoint/comms-link
COMMS_PSK="85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0" COMMS_URL="ws://srv1422716.hstgr.cloud:8765" node send-message.js "Post-ship audit complete. Cross-model report in audit/results/. Please review and add Bono-perspective findings."
```

**Bono reviews** via Perplexity MCP (`pplx_smart_query` for follow-up research on findings).

### Phase A Gate
- [ ] Fleet audit completed (68 phases, all PASS or QUIET with justification)
- [ ] Multi-model code audit completed (5 models, 7 batches)
- [ ] Cross-model report reviewed by Opus
- [ ] All consensus findings (3+ models) resolved
- [ ] All two-model P1 findings resolved
- [ ] Gap analysis reviewed — no new critical gaps
- [ ] Results pushed to git
- [ ] Bono notified for review
- [ ] LOGBOOK updated with audit summary

### When to Run Phase A

| Trigger | Fleet Audit? | Multi-Model? | Models |
|---|---|---|---|
| After milestone ship | YES | YES (full) | All 5 + Opus |
| After security incident | Targeted tiers | YES (affected batches) | Gemini + R1 |
| Monthly maintenance | YES | YES (full) | All 5 + Opus |
| After dependency update | Tier 1 + 2 | Batch 06 only | Any 1 + mechanical |
| New crate/service added | Tier 1 | YES (new batch) | V3 + R1 + MiMo |
| Quick pre-deploy check | No | Batch for changed crate | Qwen3 ($0.05) |

---

## Phase M: MULTI-MODEL AI AUDIT PROTOCOL (Reference)

**Full reference:** `audit/MULTI-MODEL-AUDIT-PROTOCOL.md`

This section summarizes how the Multi-Model AI Audit Protocol integrates into the Unified Protocol. The full protocol document contains additional detail on infrastructure setup, OpenRouter account management, and model retirement criteria.

### Activation Points Across Lifecycle

| Phase | Label | Audit Type | Blocking? | Cost | Time |
|---|---|---|---|---|---|
| **0: Session Start** | Freshness check | Age check only | Only if > 30 days | $0 | 10s |
| **1: Plan** | [M:advisory] | Single model, single batch | No | ~$0.05 | 3 min |
| **2: Create** | [M:mechanical] | Grep-based checks | Yes (zero tolerance) | $0 | 30s |
| **3: Verify** | [M:targeted] | Tier A/B/C based on risk | Yes (consensus P1) | $0.05-5.00 | 3-30 min |
| **5: Ship** | [M:gate] | 4th Ultimate Rule layer | Yes (milestones) | ~$3-5 | 30 min |
| **D: Debug** | [M:post-incident] | Targeted R1 + MiMo | No | ~$1.20 | 10 min |
| **A: Post-Ship** | [M:full] | All 5 models, 7 batches | Creates tickets | ~$3-5 | 30 min |

### Cost Summary

| Audit Scope | Cost | When |
|---|---|---|
| Mechanical self-audit | $0 | Every change |
| Tier A (1 model, diff-only) | ~$0.05 | Every change |
| Tier B (3 models, targeted) | ~$0.50-1.50 | Risk-triggered |
| Tier C (5 models, full) | ~$3-5 | Milestones, monthly |
| Post-incident (2 models) | ~$1.20 | After incidents |
| **Monthly total (estimate)** | **~$10-15** | With 2-3 milestones |

**Compare:** Opus-only equivalent would cost ~$187 per full audit and still miss 48 bugs.

### James/Bono Coordination

- **James** owns the OpenRouter account, runs all 5-model automated audits from James PC (.27)
- **Bono** reviews cross-model findings from git + adds domain review via Perplexity MCP
- Results stored in `audit/results/` in racecontrol repo — shared access via git
- **NEVER commit OpenRouter API keys to git** — share via WS or env vars only

---

## Appendix A: Standing Rules Registry Quick Reference

### By Category

| Category | Count | Key Rules |
|----------|-------|-----------|
| Ultimate | 2 | SR-ULTIMATE-001 (4-layer gate v2.0), SR-ULTIMATE-002 (visual verify) |
| Deploy | 14 | SR-DEPLOY-001–014 (sentinel restart, 7-step server, canary, binary swap) |
| Comms | 5 | SR-COMMS-001–005 (dual channel, auto-push, relay default, rules sync) |
| Code Quality | 9 | SR-QUALITY-001–009 (no unwrap, no any, bat ASCII, SSH piping) |
| Process | 11 | SR-PROCESS-001–011 (refactor second, cross-process, cascade, LOGBOOK) |
| Testing | 21 | SR-TESTING-001–021 (exact path, customer view, anomalies, session verify) |
| Debugging | 6 | SR-DEBUGGING-001–006 (recovery awareness, cause elimination, lifecycle logging) |
| Security | 6 | SR-SEC-001–006 (auth patterns, security gate, pre-commit, deploy enforcement) |
| OTA Pipeline | 6 | SR-OTA-001–006 (prev binary, manifest, billing drain, sentinel, config push, rollback) |
| **Total** | **80** | |

### By Automation Level

| Type | Count | Meaning |
|------|-------|---------|
| AUTO | 15 | Machine-checkable via `check_command` |
| HUMAN-CONFIRM | 17 | Requires human verification checklist |
| INFORMATIONAL | 48 | Knowledge rules — applied by judgment |

### By Lifecycle Phase

| Phase | Standing Rules Applied |
|-------|----------------------|
| **0: Session Start** | TESTING-016, TESTING-017, DEPLOY-012, TESTING-011, TESTING-013 |
| **1: Plan** | PROCESS-008, PROCESS-009, PROCESS-010, DEBUGGING-001, DEPLOY-008, OTA-001 |
| **2: Create** | QUALITY-001–009, PROCESS-001–003, PROCESS-007, SEC-003–006, DEPLOY-006, DEBUGGING-005 |
| **3: Verify** | TESTING-001–021, ULTIMATE-002, DEBUGGING-004, PROCESS-006 |
| **4: Deploy** | DEPLOY-001–014, OTA-001–006, COMMS-003 |
| **5: Ship** | ULTIMATE-001 (4-layer), ULTIMATE-002, COMMS-001–002, COMMS-004, PROCESS-011, TESTING-018 |
| **D: Debug** | DEBUGGING-001–006, PROCESS-010, all TESTING rules as verification |
| **A: Post-Ship** | Fleet audit (68 phases) + Multi-model code audit (5 models, 7 batches) |

---

## Appendix B: Audit Protocol Integration

The 68-phase AUDIT-PROTOCOL.md maps into this unified protocol as a **periodic Phase 3 (Verify)** run across the entire fleet:

| Audit Tier | Lifecycle Phase | What It Verifies |
|------------|----------------|------------------|
| Tier 1: Infrastructure (10 phases) | Phase 0 + Phase 3 | Fleet inventory, config, network, processes, self-heal |
| Tier 2: Core Services (6 phases) | Phase 3 | API, WebSocket, exec, sentry, preflight |
| Tier 3: Display & UX (6 phases) | Phase 3 + Visual | Lock screen, overlays, resolution, kiosk |
| Tier 4: Billing (5 phases) | Phase 3 | Pricing, wallet, reservations, accounting |
| Tier 5: Games & Hardware (4+ phases) | Phase 3 | Catalog, launch E2E, hardware |

Run the full audit via: `AUDIT_PIN=261121 bash audit/audit.sh --mode full --auto-fix --notify --commit`

---

## Appendix C: GSD Debugger Integration

The GSD debug system (`/gsd:debug` command + `gsd-debugger` agent) implements Phase D as an automated subagent:

1. **Orchestrator** (`gsd:debug`) gathers symptoms, spawns debugger
2. **Debugger agent** (`gsd-debugger`) runs Phase D autonomously:
   - Knowledge base check (D.1 Tier 2)
   - Evidence gathering (D.2 Step 1)
   - Hypothesis formation (D.2 Step 2) with falsifiability requirement
   - One-at-a-time testing (D.2 Step 3) with experimental design framework
   - Fix & verify (D.2 Steps 4-5)
3. **Checkpoints** when human input needed (visual verify, auth, decision)
4. **Knowledge base update** on resolution (feeds future D.1 Tier 2)
5. **LOGBOOK entry** on completion (D.2 Step 5)

**Session persistence:** Debug state survives context resets via `.planning/debug/{slug}.md`

---

## Appendix D: Incident Response

For production incidents (pod down, customer-facing failure), use `/rp-incident` which follows the 4-Tier Debug Order with auto-logging. This is Phase D optimized for urgency:

1. **Triage:** Is this affecting customers RIGHT NOW?
2. **Tier 1 (Deterministic):** Clear sentinels, kill orphans, restart service
3. **Stabilize:** Get service running (even if root cause unknown)
4. **Root Cause:** Full Phase D investigation after stabilization
5. **Log:** LOGBOOK entry with full incident timeline

**Rule:** Stabilize FIRST, investigate SECOND. A running system with an unknown root cause is better than a down system with perfect diagnosis.

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-03-27 | Initial unified protocol — 147+ rules mapped to 6 lifecycle phases |
| 2.0 | 2026-03-27 | Multi-Model AI Audit Protocol integrated across all phases. Ultimate Rule upgraded to 4 layers. Phase A (Post-Ship Audit) added. Phase M reference added. OpenRouter 5-model stack (Qwen3, DeepSeek V3, DeepSeek R1, MiMo v2 Pro, Gemini 2.5 Pro) with tiered activation (mechanical → lightweight → targeted → full). Audit Gap Analysis from 48 cross-model findings integrated. |
