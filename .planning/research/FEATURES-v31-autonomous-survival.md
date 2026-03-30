# Feature Research: v31.0 Autonomous Survival System (3-Layer MI Independence)

**Domain:** Multi-layer autonomous fleet healing — Windows edge-compute fleet (8 gaming pods + server)
**Researched:** 2026-03-30
**Confidence:** HIGH (grounded in 6+ months of operational incident history, MESHED-INTELLIGENCE.md design doc, CLAUDE.md standing rules, and deployed MI v26.0 infrastructure)

---

## Context: What Already Exists (Do Not Rebuild)

MI v26.0 is SHIPPED. The following are available building blocks:

| Component | Location | Capability |
|-----------|----------|------------|
| 5-tier diagnosis engine | rc-agent (each pod) | Deterministic → KB → single-model → 4-model → human |
| SQLite knowledge base | rc-agent per node | Solutions + experiments + confidence scoring |
| Gossip protocol | rc-agent WS | Solution propagation across fleet |
| Budget manager | rc-agent | $10/day/pod hard cap, OpenRouter cost tracking |
| 4 OpenRouter models | rc-agent | Qwen3, DeepSeek R1, MiMo v2 Pro, Gemini 2.5 Pro |
| rc-watchdog | Windows service | Session 1 spawn via WTSQueryUserToken, health polling |
| Binary deploy pipeline | deploy-server.sh v3.0 | Hash-based naming, rc-agent-prev.exe rollback |
| rc-sentry | Each pod :8091 | 6-endpoint fallback tool, post-crash log analysis |
| WhatsApp alerting | Bono VPS | Evolution API, staff + customer channels |
| SSH fleet access | Tailscale + LAN | ssh pod1..pod8, ssh server |
| Fleet health endpoint | Server :8080 | WS + HTTP, per-pod status |

v31.0 adds THREE NEW LAYERS on top of this foundation. Each layer is distinct in scope and placement.

---

## The Three Layers

```
Layer 3: External Guardian (Bono VPS — watches James + Server from outside)
           └── knows server is sick even when server can't self-report

Layer 2: Server Fleet Healer (Server .23 — watches all 8 pods + POS remotely)
           └── SSH diagnostics, fleet-wide MMA, pattern detection, remote autonomous fix

Layer 1: Smart Watchdog (each pod — watches itself, validates its own binary)
           └── binary SHA256 validation, rollback, MMA diagnosis, direct HTTP reporting
```

---

## Feature Landscape

### Layer 1: Smart Watchdog (Pod-Level Self-Survival)

**What "done" looks like:** A pod can survive a bad deploy, diagnose its own startup failures, roll back to a known-good binary, and report its status directly to the server via HTTP even when WS gossip is down.

#### Table Stakes (Layer 1)

Features the layer MUST have to function. Missing any = Layer 1 is not a survival layer, it's just the current watchdog.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Binary SHA256 validation on startup | Corrupt/truncated downloads from HTTP staging server are a real failure mode (confirmed 2026-03-29: 335-byte HTML 404 downloaded instead of 15MB binary) | LOW | Validate hash against release-manifest.toml before exec |
| Automatic rollback to rc-agent-prev.exe on validation failure | If new binary fails SHA256 or health poll within 30s, prev.exe exists for exactly this | LOW | rc-agent-prev.exe already preserved by deploy pipeline; watchdog just needs to rename+restart |
| Direct HTTP reporting to server when WS gossip is down | WS gossip is the normal path; HTTP POST to /api/v1/fleet/pod-report is the fallback | LOW | Server already has fleet health endpoint; add a POST variant |
| Startup health poll loop (3 attempts, 10s apart) before declaring self healthy | rc-sentry already does spawn verification with 500ms/10s poll; watchdog needs same pattern | LOW | Reuse existing poll logic from rc-sentry |
| MAINTENANCE_MODE auto-clear on confirmed clean binary + clean health | MAINTENANCE_MODE is a silent pod killer if not auto-cleared after a successful validated start | LOW | Already has 30-min auto-clear; add explicit clear on validated startup |
| MMA diagnosis on repeated startup failure (>2 fails in 10 min) | Without MMA, the watchdog can only restart blindly — same problem repeats | MEDIUM | Invoke Tier 3 (Qwen3) with startup log context via existing OpenRouter integration |
| Auto-download retry with exponential backoff | Single-probe HTTP server assumption fails if staging server is still starting | LOW | Already in FIX-DEPLOY-PROTOCOL.md; needs code enforcement |
| Post-rollback WhatsApp alert | Rollback is a signal that something serious went wrong; must notify staff | LOW | Reuse existing WhatsApp alerting path |

#### Differentiators (Layer 1)

Features that make this layer meaningfully autonomous, not just resilient.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| MMA consensus on startup failure root cause (Tier 4, 4 models) | The 5-tier diagnosis already exists in MI v26.0; applying it specifically to startup-failure context catches absence bugs and config mismatch that Tier 3 single-model misses | MEDIUM | Startup failure has structured log context: exit code, last log lines, binary hash, Windows Event Log entry |
| Manifest-driven health expectations (expected endpoints, expected build_id) | The watchdog knows exactly what "healthy" looks like because the manifest says so — not just "HTTP 200" but "build_id must match this hash" | LOW | release-manifest.toml already has build_id; watchdog reads it |
| Canary validation before fleet-wide rollout signal | Pod 8 (canary) reporting healthy via Layer 1 before Layer 2 proceeds with fleet-wide push | MEDIUM | Layer 1 and Layer 2 need a handshake protocol |
| Binary age monitoring (alert if binary > 7 days old without re-deploy) | Stale binaries are a known problem (kiosk was 14 days stale with 72 bug fixes undeployed) | LOW | Compare manifest.toml timestamp to current date |

#### Anti-Features (Layer 1)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Watchdog as Windows Service (re-registering itself) | "Services are more reliable" | Session 0 isolation breaks ALL GUI operations — this is the exact bug that took down all 8 pods on 2026-03-26 | Keep rc-watchdog as service that spawns rc-agent in Session 1 via WTSQueryUserToken, never move rc-agent to Session 0 |
| Aggressive restart loops (restart on any failure immediately) | Fast recovery instinct | >3 restarts in 10 min → MAINTENANCE_MODE → permanent silent death. Also crashes fight each other with WoL + self_monitor + rc-sentry | Use the existing 3-restart-in-10-min MAINTENANCE_MODE gate; Smart Watchdog adds MMA escalation at that gate, not more restarts |
| Binary self-patching (download and replace own binary) | Eliminate deploy step | rc-agent cannot replace its own exe while running on Windows (file lock). Also introduces race with active billing sessions | OTA_DEPLOYING sentinel already enforces this. Smart Watchdog validates + rollbacks; Layer 2 handles remote push |
| Storing OpenRouter API key in the binary/manifest | "Simpler key management" | Credentials in git = P1 security violation. Pre-commit hooks block it. | Env var OPENROUTER_API_KEY already set on all pods from v26.0 deploy |
| Calling all 4 models on every startup | "More diagnosis is better" | $3.01/call × multiple pods × multiple failures = budget exhaustion before real incidents. Training period goal is data quality, not model hammering | Use Tier 3 (single model, $0.05) for first failure; Tier 4 only after MAINTENANCE_MODE gate is reached |

---

### Layer 2: Server Fleet Healer (Remote Fleet Surgery)

**What "done" looks like:** The server can SSH into any pod, run structured diagnostics, identify what's wrong, apply a fix autonomously if the fix type is deterministic or KB-matched, and coordinate a fleet-wide MMA session when a systemic pattern is detected.

#### Table Stakes (Layer 2)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| SSH diagnostic runner (structured commands → structured output) | Server already has Tailscale SSH access to all pods. Without structured SSH diagnostics, Layer 2 can only probe HTTP — it cannot see process state, session context, Event Log, or file system | MEDIUM | Run predefined diagnostic scripts via SSH, parse output into structured JSON |
| Fleet-pattern detection (N pods reporting same symptom in M minutes) | MI v26.0 already has pattern detection logic in server coordinator; Layer 2 needs to ACT on the pattern, not just detect it | LOW | Reuse pattern_detector from MI v26.0; add autonomous action dispatch |
| Autonomous deterministic fix dispatch (Tier 1 fixes via SSH) | The fleet KB already knows how to fix the top-N issues. Layer 2 must be able to apply them remotely without human | MEDIUM | Tier 1 fix_action JSON already stored in solutions table; Layer 2 reads + executes via SSH |
| MMA session coordination for fleet-wide unknown issues | When multiple pods have same unknown issue, one MMA session covers all — prevents duplicate model spend (gossip first-responder rule) | MEDIUM | Extend existing experiment ledger / first-responder rule to fleet-level |
| Repair confidence gate before autonomous fix | Applying an unverified fix to 8 pods simultaneously is the "bad fix fleet-wide" critical risk | LOW | Only dispatch autonomous fix if confidence >= 0.8 AND fix_type is deterministic or config (never code_change) |
| Post-fix verification (poll health endpoint + behavioral check) | `.spawn().is_ok()` does NOT mean the child started — already a standing rule | LOW | After dispatch, poll /health for build_id match AND /debug for edge_process_count > 0 (standing rule) |
| Pod isolation before risky repair (mark pod as MAINTENANCE before touching it) | Prevents kiosk from routing customers to a pod mid-repair | LOW | Write MAINTENANCE_MODE sentinel via rc-sentry exec before repair, clear after verification |

#### Differentiators (Layer 2)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| SSH diagnostic fingerprinting (build structured symptom JSON from raw SSH output) | MMA models need structured context to be effective — raw SSH stdout is noise | MEDIUM | Map known output patterns (tasklist, netstat, Event Log entries) to symptom JSON schema matching existing solutions table |
| Graduated repair scope (single pod → pod class → fleet) | Applying to all pods simultaneously risks cascading failure. Pod class (same GPU/driver combo) is the right intermediate step | MEDIUM | Pod metadata already has hardware_class in environment_fingerprint schema |
| Repair audit trail (every SSH command + response logged to incident_log) | Legal/financial traceability for "why was billing interrupted" inquiries | LOW | Append to existing incident_log table (fleet_solutions schema already has this) |
| Night-ops autonomous maintenance window (midnight cycle) | MI v26.0 section 7.7 designs this; Layer 2 makes it real — full fleet SSH sweep + Tier 1 apply during venue-closed hours | HIGH | Requires safe-time detection (is venue closed? no active billing sessions?) + scheduler |
| Predictive repair trigger (trending metric crosses threshold → pre-emptive action) | MI v26.0 section 7.1 designs this — ConspitLink reconnection rate → USB dying alert | MEDIUM | Reuse existing threshold-based anomaly detection; add Layer 2 SSH action for pre-emptive restart |
| Cross-pod fix comparison (Pod 3 fixed by X, Pod 7 has same symptoms — use X) | This is exactly the gossip protocol benefit materialized at Layer 2 | LOW | Server coordinator already has fleet KB; Layer 2 reads it before spawning new MMA session |

#### Anti-Features (Layer 2)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Fleet-wide simultaneous fix application | Fast, efficient | Bad fix applied fleet-wide = all 8 pods down simultaneously. "Bad fix propagated fleet-wide" is in MI v26.0 critical risk list | Canary first (Pod 8), wait for verification, then gradual rollout (3 pods, then remaining 5) |
| Applying staff-triggered fixes via fleet broadcast path | "Same fix should propagate" | Standing rule violation: staff-triggered fixes must NOT reuse autonomous broadcast paths (blast radius difference). Documented in CLAUDE.md 2026-03-29 | Gate fleet broadcast to Tier 2+ KB-sourced solutions only (already the rule) |
| SSH with root/SYSTEM context for all repairs | "More permissions = more fix options" | SYSTEM-context process spawns land in Session 0. Breaks GUI. Same root cause as the 2026-03-26 all-pods-down incident | SSH as `User` (current config), use schtasks for restart ops — same as current deploy pipeline |
| Storing SSH private key in racecontrol binary/config | "Zero-friction SSH" | Credentials in binary = audit/security failure. RCSENTRY_SERVICE_KEY incident (2026-03-28) shows key rotation is hard when hardcoded | Use ~/.ssh/config on server (already configured per CLAUDE.md) |
| Layer 2 deciding on code_change fixes autonomously | "Full autonomy" | Code changes require rebuild + redeploy + E2E verification + MMA audit. No autonomous system should apply code_change without human review | fix_type = code_change → escalate to Uday via WhatsApp only, never auto-apply |

---

### Layer 3: External Guardian (Off-Site Watchdog)

**What "done" looks like:** Bono VPS monitors the server's health from outside the venue network. When the server goes down (or is unresponsive), the Guardian can trigger a server restart via the existing Tailscale SSH path, and escalate to Uday via WhatsApp when restart fails or is unsafe.

#### Table Stakes (Layer 3)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Server health polling from Bono VPS (HTTP probe every 60s) | Server is the single point of coordination for all 8 pods. If server is sick, pods become islands. No internal watcher can catch this — server is both the watcher and the watched | LOW | Bono VPS already has Tailscale + SSH to server. Add health probe loop to comms-link or standalone guardian service |
| Server dead-man detection (missed N consecutive polls = declare dead) | Single failed probe = network blip. Three consecutive failures = real issue | LOW | 3-miss threshold with 60s interval = 3-minute detection window |
| Server restart via Tailscale SSH | rc-watchdog equivalent for the server. Bono VPS executes `schtasks /Run /TN StartRCTemp` via SSH after dead-man trigger | LOW | SSH path already proven (deploy-server.sh v3.0 uses it). Tailscale IP: 100.125.108.37 |
| Restart safety check (is billing session active? is OTA_DEPLOYING set?) | Restarting server during active billing session corrupts transactions (standing rule). OTA_DEPLOYING sentinel must be checked | LOW | Check /api/v1/fleet/health for active_billing_sessions before restart |
| WhatsApp escalation when restart fails or is unsafe | If Guardian can't fix it, Uday must know immediately | LOW | Existing WhatsApp alerting path via Evolution API on Bono VPS |
| Status distinction: server dead vs server busy vs server unreachable | Rebooting a server that is busy (long MMA session) is wrong; rebooting an unreachable server is right | LOW | HTTP 200 with response time > threshold = busy; connection refused = dead; timeout = network issue |
| Guardian health reporting to James via comms-link | James needs to know Guardian is alive and watching | LOW | Append heartbeat to INBOX.md every 6h OR on any triggered event |

#### Differentiators (Layer 3)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Venue-hours-aware restart decision | Restarting the server at 2pm on a Saturday (peak hours) when 6 customers are racing is different from 2am | LOW | Server exposes /api/v1/venue/status (venue open/closed, active sessions count). Guardian checks before restart |
| Graduated restart (soft → hard → report-only) | `schtasks /Run /TN StartRCDirect` (soft) first; if fails, `taskkill /F` + start (hard); if fails, report-only | LOW | Mirrors deploy-server.sh v3.0 fallback chain |
| Watchdog-of-watchdog: Guardian confirms rc-watchdog service is running on server | Guardian SSH can check `sc query RCWatchdog` — catches the case where watchdog died before racecontrol did | LOW | Single SSH command, low cost |
| Guardian availability metric (uptime of external guardian itself) | A guardian that's down provides false security. Bono VPS comms-link watchdog already handles this | LOW | Existing `CommsLink-DaemonWatchdog` task on James covers comms-link; Guardian needs same |
| Cross-region knowledge sync trigger (Guardian detects server KB diverged from cloud) | When server has been down and comes back, its fleet KB may be behind Bono VPS cloud KB | MEDIUM | After successful server restart, Guardian triggers KB sync pull |

#### Anti-Features (Layer 3)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Guardian making diagnostic decisions (MMA, KB lookups) | "More intelligence is better" | Guardian is an availability layer, not a diagnostic layer. Running MMA from Bono VPS during server downtime adds latency and complexity at exactly the wrong moment | Guardian's job is: detect → restart → alert. Diagnosis is Layer 2's job. Keep them separate. |
| Guardian polling every 5 seconds | "Faster detection" | 5s polling from VPS to server over Tailscale generates unnecessary traffic and creates false positives on any network blip | 60s normal poll, 15s poll if previous poll failed (adaptive polling) |
| Guardian SSH-ing into pods directly | "Skip the server, go straight to pods" | Guardian has no context about pod state, billing sessions, or the right fix. Layer 2 (server) has this context. Guardian bypassing Layer 2 creates uncoordinated dual-action | Guardian watches server only. Server (Layer 2) watches pods. |
| Guardian managing binary deploys | "Guardian can push fixes while server is down" | If server is down, OTA coordinator is down. Pushing binaries to pods without coordinator creates manifest divergence | Fix: restart server first. If server can't restart, alert Uday. Binary deploys require server coordination. |

---

## Unified MMA Protocol (Cross-Layer Feature)

This is not a new layer — it is the shared protocol used by all three layers when they escalate to model-based diagnosis.

### Table Stakes (MMA Protocol)

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| 5-model consensus (add one model vs current 4) | 4 models can deadlock 2-2. 5 models produce a clear majority. Also: MMA audits repeatedly show thinking-model variants catch execution-path bugs that non-thinking models miss | LOW | Add one thinking-variant model (e.g. DeepSeek R1 thinking mode) as 5th slot |
| N-iteration convergence (run until consensus, not fixed N rounds) | v26.0 MMA design uses fixed rounds. Real-world audits show 7-10 rounds needed for subtle bugs (v27.0 found 7 new bugs in rounds 8-10) | MEDIUM | Convergence criterion: 3 consecutive rounds with 0 new P1/P2 findings |
| Unified Protocol v3.1 integration (all 5-layer checks) | MMA sessions not using the Unified Protocol quality gate miss execution-path bugs (standing rule: 4-layer verification before ship) | LOW | MMA session must include: Phase D diagnosis + Phase 5 gate checks embedded in prompt |
| Structured finding taxonomy (P0/P1/P2, finding type, affected component) | Without taxonomy, MMA output is prose that humans must parse. Standing rules exist because findings without structure get partially actioned | LOW | Reuse existing MMA protocol taxonomy from audit/MULTI-MODEL-AUDIT-PROTOCOL.md |
| Fact-checker model in every session | Standing rule: MMA models hallucinate policy names. A fact-checker model cross-references findings against CLAUDE.md standing rules | LOW | Assign one model role as fact-checker (feeds findings to it for verification before action) |
| Cost guard (abort MMA session if budget would be exhausted) | Budget manager already enforces per-session cap. MMA protocol must check before launching 5-model × N-iteration session | LOW | Pre-flight: remaining_today × 0.8 >= estimated_session_cost or defer |

### Differentiators (MMA Protocol)

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Dual reasoning mode (non-thinking + thinking model variants in same session) | Standing rule from v27.0: non-thinking models find architecture bugs; thinking models find execution-path bugs. Both are needed — neither alone is sufficient | LOW | Already done in v27.0 — formalize as protocol requirement |
| Session reuse (MMA findings from one layer available to other layers) | Layer 1 doing MMA on a pod startup failure should publish findings to Layer 2 fleet context | MEDIUM | Write findings to incident_log table on server after session |
| Training-period model selection (high capability during 30-day window, then cost-optimize) | 30-day training period goal: collect high-quality diagnosis data. Use more capable (expensive) models. After training, swap to cheaper models with same accuracy | LOW | Flag sessions as training=true during training window; different model selection path |

### Anti-Features (MMA Protocol)

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| MMA on every health check failure | "Catch problems early" | Health check failures are often transient network blips. Running 5-model MMA on every blip exhausts budget instantly ($3.01/session × 10 blips/day × 8 pods = $241/day vs $100/day fleet budget) | Tier 1 → Tier 2 → Tier 3 → Tier 4 escalation already enforces this. MMA is Tier 4. Don't shortcut. |
| All models in same session asking the same question | "More coverage" | Homogeneous prompting produces homogeneous findings — confirmed in 14-model MMA audit. Multiple models need different ROLES (scanner, reasoner, SRE, security, fact-checker) | Already in MI v26.0 model role assignment. Extend to 5th model with fact-checker role. |
| Single model as "arbiter" that overrides consensus | "Faster decisions" | The entire point of consensus is no single model is authoritative. Gemini hallucinated 4 false positives in one week (MI v26.0 report). Arbiter model would propagate these | Majority vote (3/5). P1 consensus required: 4/5 agreement before auto-apply. |

---

## Feature Dependencies

```
[SHA256 binary validation]
    └──requires──> [release-manifest.toml with hash field]
                       └──requires──> [stage-release.sh manifest generation]
                                         (already exists in OTA pipeline)

[Layer 1 MMA on startup failure]
    └──requires──> [OpenRouter API key on pods]
                       └──already deployed: v26.0

[Layer 2 SSH diagnostics]
    └──requires──> [SSH key + ~/.ssh/config on server]
                       └──already configured: CLAUDE.md

[Layer 2 autonomous fix dispatch]
    └──requires──> [Layer 1 behavioral verification (edge_process_count check)]
                └──requires──> [MAINTENANCE_MODE sentinel protocol]
                                   └──already exists: rc-agent

[Layer 3 server health polling]
    └──requires──> [Tailscale connectivity Bono VPS → Server]
                       └──already exists: 100.70.177.44 → 100.125.108.37

[Layer 3 restart decision]
    └──requires──> [/api/v1/venue/status endpoint on server]
                       (NEW: server needs to expose active_billing_sessions count)

[Unified MMA Protocol v3.1]
    └──requires──> [5-model roster (4 existing + 1 thinking variant)]
    └──requires──> [budget_remaining check before session launch]
    └──enhances──> [all three layers]

[Night-ops autonomous cycle]
    └──requires──> [Layer 2 full function]
    └──requires──> [venue-closed detection (no active billing sessions for 30+ min)]
    └──requires──> [safe restart window (02:00–05:00 IST)]
```

### Dependency Notes

- **Layer 1 and Layer 2 are independent at startup** — Layer 1 runs on each pod, Layer 2 runs on server. They communicate via the existing fleet health WS + HTTP path.
- **Layer 3 depends on Layer 2 being healthy** — Guardian restarts the server; Layer 2 (which runs on the server) then handles pod recovery. If Layer 2 is broken, Guardian can only restart and alert.
- **Unified MMA Protocol enhances all three layers** — but none of the three layers REQUIRES the 5th model or N-iteration convergence to function. These are protocol improvements layered on top of existing MI v26.0.
- **Night-ops requires Layer 2 to be complete** — it is not a v31.0 launch feature; it is a v31.x add-on once Layer 2 is proven.

---

## MVP Definition (v31.0 Launch)

### Launch With (v31.0)

Minimum viable product — what makes this a "survival system" vs the current state.

- [ ] **Layer 1: Binary SHA256 validation + rollback** — catches the confirmed failure mode (2026-03-29: 335-byte HTML 404 downloaded as binary). Low complexity, high value.
- [ ] **Layer 1: Startup health poll loop (3 attempts before MMA escalation)** — enforces the existing standing rule in code, not just docs.
- [ ] **Layer 1: Direct HTTP reporting when WS down** — Island Mode: pod can report even without fleet gossip.
- [ ] **Layer 2: SSH diagnostic runner (structured output)** — prerequisite for all autonomous remote repair.
- [ ] **Layer 2: Fleet-pattern detection → MMA session (one session covers all pods with same symptom)** — avoids duplicate model spend.
- [ ] **Layer 2: Repair confidence gate + canary rollout** — prevents the "bad fix fleet-wide" critical risk.
- [ ] **Layer 3: Server health polling from Bono VPS (60s interval, 3-miss threshold)** — the Guardian's core function.
- [ ] **Layer 3: Server restart via Tailscale SSH with billing safety check** — Guardian's action capability.
- [ ] **Layer 3: WhatsApp escalation when restart fails** — human-in-the-loop as last resort.
- [ ] **MMA Protocol: 5th model (thinking variant) + fact-checker role assignment** — addresses the known blind spot from v27.0.

### Add After Validation (v31.x)

Features to add once core layers are proven.

- [ ] **Layer 1: MMA Tier 4 (4-model) on MAINTENANCE_MODE gate** — adds cost ($3/incident); validate Tier 3 coverage first.
- [ ] **Layer 2: Graduated repair scope (single pod → pod class → fleet)** — needs operational data on which fixes are pod-specific vs class-wide.
- [ ] **Layer 2: Predictive repair trigger (trending metric threshold)** — requires baseline data collection first.
- [ ] **Layer 3: Venue-hours-aware restart decision** — /api/v1/venue/status endpoint needed; build after server API is stable.
- [ ] **MMA Protocol: N-iteration convergence** — validate convergence criterion from 30-day training data.

### Future Consideration (v31.x+)

Features to defer until training period data informs them.

- [ ] **Night-ops autonomous maintenance window** — HIGH complexity; requires Layer 2 full stability first. 30-day training generates the data needed to define safe-action whitelist for night-ops.
- [ ] **Binary age monitoring + proactive re-deploy suggestion** — LOW priority; manual LOGBOOK process works until 30-day training period ends.
- [ ] **Cross-region knowledge sync (Guardian triggers KB sync after server restart)** — requires cloud KB to be current (Multi-Venue sync from MI v26.0 Phase 239-240).

---

## Feature Prioritization Matrix

### Layer 1: Smart Watchdog

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| SHA256 validation + rollback | HIGH (prevents bad deploy) | LOW | P1 |
| Startup health poll loop | HIGH (enforces standing rule) | LOW | P1 |
| Direct HTTP reporting when WS down | HIGH (Island Mode survival) | LOW | P1 |
| MAINTENANCE_MODE auto-clear on validated start | HIGH (prevents silent death) | LOW | P1 |
| Post-rollback WhatsApp alert | MEDIUM | LOW | P1 |
| MMA Tier 3 on startup failure | HIGH | MEDIUM | P2 |
| MMA Tier 4 on MAINTENANCE_MODE gate | MEDIUM | MEDIUM | P2 |
| Manifest-driven health expectations | MEDIUM | LOW | P2 |
| Binary age monitoring | LOW | LOW | P3 |

### Layer 2: Server Fleet Healer

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| SSH diagnostic runner (structured output) | HIGH (prerequisite) | MEDIUM | P1 |
| Fleet-pattern detection → MMA session | HIGH (cost efficiency + coordination) | LOW | P1 |
| Repair confidence gate + canary rollout | HIGH (prevents cascading failure) | LOW | P1 |
| Autonomous deterministic fix dispatch (Tier 1 via SSH) | HIGH | MEDIUM | P1 |
| Post-fix behavioral verification | HIGH (standing rule enforcement) | LOW | P1 |
| Repair audit trail in incident_log | MEDIUM | LOW | P2 |
| Graduated repair scope (single → class → fleet) | HIGH | MEDIUM | P2 |
| SSH diagnostic fingerprinting (symptom JSON) | HIGH | MEDIUM | P2 |
| Predictive repair trigger | MEDIUM | MEDIUM | P2 |
| Night-ops maintenance window | MEDIUM | HIGH | P3 |

### Layer 3: External Guardian

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Server health polling (60s, 3-miss threshold) | HIGH (external vantage point) | LOW | P1 |
| Server restart via SSH with billing safety check | HIGH | LOW | P1 |
| WhatsApp escalation on restart failure | HIGH | LOW | P1 |
| Status distinction (dead vs busy vs unreachable) | HIGH | LOW | P1 |
| Guardian heartbeat to James via comms-link | MEDIUM | LOW | P1 |
| Graduated restart (soft → hard → report-only) | MEDIUM | LOW | P2 |
| Venue-hours-aware restart | MEDIUM | LOW (needs /venue/status endpoint) | P2 |
| Cross-region KB sync after restart | LOW | MEDIUM | P3 |

### Unified MMA Protocol

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| 5th model (thinking variant) + fact-checker role | HIGH (closes v27.0 blind spot) | LOW | P1 |
| Dual reasoning mode (non-thinking + thinking) | HIGH | LOW | P1 |
| Cost guard before MMA session | HIGH (budget protection) | LOW | P1 |
| Structured finding taxonomy | MEDIUM | LOW | P1 |
| N-iteration convergence | MEDIUM | MEDIUM | P2 |
| Training-period model selection flag | MEDIUM | LOW | P2 |
| Session reuse (findings published to incident_log) | MEDIUM | MEDIUM | P2 |

---

## Training Period vs Post-Training Distinction

The 30-day training period ends approximately 2026-04-30. This affects feature selection.

### During Training Period (v31.0, now → 2026-04-30)

**Goal:** Collect high-quality diagnostic data. Use capable (more expensive) models. Prioritize coverage over cost.

- Use Gemini 2.5 Pro as 5th model (despite higher false positive rate noted in weekly MI report — it catches security/config bugs others miss)
- Run Tier 3 (single model) more aggressively: trigger on first failure, not just repeated failures
- Log ALL MMA findings to incident_log with full context (training dataset)
- Do NOT optimize model selection until training data says which models have best cost/accuracy ratio on our specific issue types

### Post-Training (v31.x, after 2026-04-30)

- Rotate models based on training data: swap low-value models (per MI v26.0 section 6.2 rotation algorithm)
- Tighten Tier 3 trigger criteria (only escalate after 2+ failures, not 1)
- Implement N-iteration convergence (training data defines when sessions typically converge)
- Enable night-ops (training period established which Tier 1 fixes are safe to automate overnight)

---

## Competitor / Analogue Analysis

Direct competitors don't exist for a Windows gaming pod fleet. Closest analogues:

| Feature | Azure Arc (VM fleet) | Kubernetes (container fleet) | Our Approach |
|---------|---------------------|------------------------------|--------------|
| Binary validation | Desired state configuration | Image hash in pod spec | release-manifest.toml SHA256 + watchdog |
| Rollback | Deployment rollback (kubectl rollout undo) | Previous image tag | rc-agent-prev.exe rename (already exists) |
| External health watch | Azure Monitor | Liveness probes | Layer 3 Guardian on Bono VPS |
| Fleet repair | Azure Automation runbooks | Helm chart re-apply | Layer 2 SSH + KB-driven fix dispatch |
| MMA diagnosis | Azure Advisor + Copilot | N/A | Custom 5-model consensus via OpenRouter |
| Knowledge base | N/A | N/A | SQLite KB + gossip (unique to our system) |

Key architectural difference: Kubernetes and Azure assume containers/VMs can be killed and replaced atomically. Our pods run Windows with hardware peripherals (FFB wheelbases, SimHub dashboards), active billing sessions, and GUI requirements (Session 1). Clean replacement is often wrong — surgical repair is correct. This is why Layer 2 SSH diagnostics + Tier 1 deterministic fixes exist rather than "just restart the pod".

---

## Sources

- MESHED-INTELLIGENCE.md — MI v26.0 design doc (authoritative for existing infrastructure)
- CLAUDE.md — 200+ standing rules derived from operational incidents (HIGH confidence: primary source)
- PROJECT.md — milestone history and constraints (HIGH confidence)
- Self-Healing Infrastructure: Leveraging Reinforcement Learning for Autonomous Cloud Recovery (ResearchGate, 2025) — confirms multi-layer tiered approach, 85% MTTR reduction pattern
- Architecture strategies for self-healing and self-preservation — Microsoft Azure Well-Architected Framework (MEDIUM confidence: enterprise cloud context differs from edge Windows fleet)
- The Autonomous Stack: How Architects Are Enabling Self-Healing Systems (EAJournals, 2025) — confirms event-driven + closed-loop + observability as architectural pillars
- The Role of External Service Monitoring in SRE Practices (DEV Community) — confirms external guardian pattern as standard SRE practice for catching self-reporting blind spots

---

*Feature research for: v31.0 Autonomous Survival System (3-Layer MI Independence)*
*Researched: 2026-03-30*
