# Project Research Summary

**Project:** RaceControl Reliability & Connection Hardening
**Domain:** Rust async process supervision, WebSocket resilience, deployment reliability (Windows gaming venue)
**Researched:** 2026-03-13
**Confidence:** HIGH

## Executive Summary

RaceControl is a bespoke 8-pod sim racing venue management system built on Rust/Axum (rc-core server) and a Windows Rust agent (rc-agent) deployed to gaming PCs. The reliability problems being addressed are well-understood in the process supervision domain: crash loops caused by fixed cooldowns, silent failures from unverified restarts, and WebSocket instability during high-CPU game launch events. The recommended approach mirrors patterns from production supervisors (systemd, supervisord) adapted to the venue's specific constraints — Windows SYSTEM session isolation, billing-aware restart guards, and a 3-tier supervision stack that already exists but lacks coordination.

The entire implementation uses zero new dependencies. Every reliability primitive needed (backoff timers, async tasks, channel coordination, structured logging) is already present in the locked workspace (`tokio 1`, `axum 0.8`, `tokio-tungstenite 0.26`, `tracing 0.1`). Two modules are already complete as of research date: `rc-common/watchdog.rs` (EscalatingBackoff with 14 tests) and `rc-core/email_alerts.rs` (EmailAlerter with 10 tests). The remaining work is wiring these into AppState and modifying pod_monitor, pod_healer, and ws/mod.rs to use them. This is integration work, not design work.

The primary risks are operational rather than technical: (1) Node.js availability on Racing-Point-Server (.23) must be verified before email alerting can be deployed — the entire alert path depends on it; (2) Windows Session 0 isolation means any restart issued by pod-agent (SYSTEM) produces a running process with no visible GUI, which must be classified as partial recovery rather than failure to avoid false alerts; (3) concurrent restart commands from pod_monitor and pod_healer racing on the same pod is the most common failure mode and must be resolved by assigning exclusive restart ownership to pod_monitor before any other work.

## Key Findings

### Recommended Stack

The project constraint is explicit: no new dependencies. All reliability primitives live in the existing locked workspace. The critical insight from STACK.md is that the two already-completed modules (EscalatingBackoff, EmailAlerter) are the correct foundation — the remaining work is wiring them into AppState and call sites in pod_monitor and pod_healer.

**Core technologies:**
- `tokio 1` (full features): async runtime for timers, task spawning, process management, and channels — `tokio::time::sleep`, `tokio::spawn`, `tokio::process::Command` cover every reliability primitive needed
- `tokio-tungstenite 0.26`: WebSocket client in rc-agent — provides `connect_async`, message framing, ping/pong at protocol level
- `axum 0.8` (ws feature): WebSocket server in rc-core — handler exposes tungstenite Message type directly, no adapter layer needed
- `tokio::process::Command`: shell-out to `send_email.js` for Gmail alerts — reuses established OAuth2 credentials, no new SMTP/OAuth infrastructure required
- `chrono 0.4` (serde feature): timestamps for EscalatingBackoff state — add to rc-common Cargo.toml (already a workspace dep, safe addition)

**One caution:** The email path shells out to `send_email.js` which requires Node.js on Racing-Point-Server (.23). This is unverified. Verify with `node --version` on `.23` before deploying email alerting. Do not architect around this — install Node.js if absent.

### Expected Features

**Must have (table stakes for this milestone):**
- `EscalatingBackoff` in rc-common (30s → 2m → 10m → 30m per pod) — eliminates crash loop restart spam; the foundation everything else depends on
- Shared backoff state in AppState — coordinates pod_monitor and pod_healer, prevents concurrent double-restarts
- Post-restart health verification (60s window, checks at 5s/15s/30s/60s) — process alive + WebSocket reconnected + lock screen responsive; catches silent startup failures
- Email alert on verification failure or backoff exhaustion, rate-limited (30min per-pod, 5min venue-wide) — Uday gets notified when automation has given up
- Config validation fail-fast at rc-agent startup — exit code 1 on missing/invalid fields; eliminates silent billing-zero deploys
- WebSocket keepalive tuning (ping/pong in ws/mod.rs) + kiosk disconnect debounce (15s threshold) — eliminates "Disconnected" flash during game launch

**Should have (add after v1 validation):**
- Aggregated multi-pod email alerts — single consolidated email when 3+ pods fail within 30s window
- Partial recovery classification (FullRecovery / PartialRecovery(Session0) / Failed) — prevents false alerts from Session 0 restarts
- Deployment dry-run mode — validates config and binary compatibility before committing to 8-pod rollout

**Defer to v2+:**
- Activity log persistence across restarts — touches rc-agent core substantially; own milestone
- Configurable cooldown step tuning in racecontrol.toml — defaults will be correct for this venue

**Anti-features to avoid:**
- Restart rc-agent on every WebSocket disconnect — creates restart storms; use reconnect logic with backoff instead
- Sub-second process polling — UDP heartbeat at 6s staleness is already faster than needed; polling adds cost with no benefit
- Automatic binary rollback — adds deployment complexity; deploy-to-Pod-8-first discipline catches failures before full rollout

### Architecture Approach

The architecture is a three-tier supervision system on Racing-Point-Server (.23): `pod_monitor` (10s loop) owns restart decisions via a shared `EscalatingBackoff` per pod; `pod_healer` (120s loop) owns diagnostics and cleanup but defers all restart commands to pod_monitor; `EmailAlerter` fires when backoff is exhausted or verification fails. The critical design constraint is that AppState is the single source of truth for per-pod backoff state — both supervisors access it through `Arc<RwLock<HashMap<PodId, EscalatingBackoff>>>`. Post-restart verification always runs as a detached `tokio::spawn` task to avoid blocking the 10s monitor loop across all 8 pods.

**Major components:**
1. `rc-common/watchdog.rs` — `EscalatingBackoff` struct, step table (30s/2m/10m/30m), reset on recovery; DONE with 14 tests
2. `rc-core/email_alerts.rs` — `EmailAlerter`, shell-out to `send_email.js`, per-pod + venue-wide rate limiting; DONE with 10 tests
3. `rc-core/state.rs` — ADD `pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>` and `email_alerter: Mutex<EmailAlerter>` to AppState
4. `rc-core/pod_monitor.rs` — MODIFY to use shared backoff, spawn verification task after each restart
5. `rc-core/pod_healer.rs` — MODIFY to read shared backoff, remove restart ownership, keep diagnostics
6. `rc-core/ws/mod.rs` — MODIFY to add ping/pong keepalive (30s ping, 10s pong timeout)
7. `rc-agent/src/config.rs` — ADD validate() with fail-fast on missing/zero fields

**Build order (hard dependency chain):**
- Layer 1: rc-common/watchdog.rs + rc-core/email_alerts.rs (DONE)
- Layer 2: state.rs + config.rs wiring (depends on Layer 1)
- Layer 3: pod_monitor + pod_healer + ws/mod.rs integration (depends on Layer 2)
- Layer 4: rc-agent config validation + pod-agent idempotency (independent of Layers 2-3)

### Critical Pitfalls

1. **HTTP 200 false recovery** — `pod-agent /exec` returns 200 when the restart command is delivered, not when rc-agent is healthy. `start /b rc-agent.exe` always exits immediately. Avoidance: spawn a verification task that polls at 5s/15s/30s/60s checking process alive (tasklist), WebSocket reconnected (agent_senders), and lock screen (port 18923). Never declare recovery from the restart HTTP response alone.

2. **Concurrent restart from monitor + healer** — Both run on independent tokio intervals, both detect the same unhealthy pod, both issue restart commands within seconds of each other. The pod gets killed mid-startup by the second command. Avoidance: exclusive restart ownership in pod_monitor via shared EscalatingBackoff in AppState. pod_healer reads the backoff but never calls record_attempt or issues restart commands.

3. **Session 0 GUI blindness** — pod-agent runs as SYSTEM; any restart via pod-agent spawns rc-agent in Session 0 where GUI (lock screen, overlay) is invisible to the customer. WebSocket reconnects (monitoring shows green) but the screen is blank. Avoidance: classify this as PartialRecovery(Session0) — do NOT email alert, do NOT mark as failure. Log the limitation. The HKLM Run key restores Session 1 on next login.

4. **WebSocket drop during game launch** — CPU spike from shader compilation (5-30s) starves the tokio runtime on the pod, causing missed pong deadlines and rc-core closing the connection as stale. Avoidance: (a) require 2-3 consecutive missed heartbeats before marking offline, not just one; (b) send application-level pings from rc-agent every 10s; (c) debounce kiosk status display — only show "Disconnected" after 15s of confirmed absence.

5. **Email storm on venue-wide network event** — A router reboot takes all 8 pods offline simultaneously, triggering 8 independent email alerts within 60 seconds. Avoidance: venue-level 5-minute rate limit in EmailAlerter across all pods (already implemented in email_alerts.rs). When 3+ pods fail within 30s, aggregate into a single email naming all affected pods.

6. **Silent config mismatch** — serde #[serde(default)] silently substitutes zero/empty for missing fields. rc-agent starts cleanly, billing shows ₹0 sessions. Avoidance: validate() method at startup checking pod_id non-empty, core_url non-empty, billing rates > 0. Exit code 1 on failure with a descriptive error.

7. **File lock on binary replace (Windows)** — Deploying a new rc-agent.exe fails silently because the old process holds the file lock. Avoidance: enforce kill → wait 2-3s → verify-dead (tasklist) → download → size check → start sequence. Use `/T` flag with taskkill to kill process tree. Verify Defender exclusions are present on each pod individually.

## Implications for Roadmap

Based on the Layer 1-4 build order in ARCHITECTURE.md and the P1/P2/P3 priority matrix in FEATURES.md, four phases are natural. Two primitives are already done (Layer 1), which means Phase 1 is wiring work, not new design.

### Phase 1: State Wiring and Config Hardening

**Rationale:** Layers 1 (done) and 2 (state/config wiring) must be complete before any monitor or healer integration. This is the lowest-risk, highest-leverage work — it connects already-tested primitives to the application without changing any live supervision logic yet. Config validation is independent of Layer 2 and can be done in parallel.

**Delivers:** AppState with pod_backoffs and email_alerter wired in; WatchdogConfig struct in racecontrol.toml; rc-agent config validation fail-fast

**Features addressed:** Shared backoff state, config validation at startup, idempotent pod-agent commands

**Pitfalls avoided:** Silent config mismatch (pitfall 7), concurrent restart race setup (pitfall 5 setup)

**Research flag:** Standard patterns. No additional research needed — EscalatingBackoff and EmailAlerter are already implemented with tests.

### Phase 2: Watchdog Hardening (Monitor + Healer Integration)

**Rationale:** Once shared state is wired (Phase 1), the monitor and healer can be refactored to use it. This phase eliminates the most painful operational problems: crash loops (fixed cooldowns) and concurrent double-restarts. Post-restart verification belongs here because it depends on shared backoff state to avoid re-triggering during the verification window.

**Delivers:** pod_monitor using EscalatingBackoff with post-restart verification; pod_healer reading shared backoff and deferring restart ownership; email alerts firing on verification failure or backoff exhaustion

**Features addressed:** EscalatingBackoff cooldowns, post-restart health verification, email alerting, partial recovery classification (Session 0)

**Pitfalls avoided:** HTTP 200 false recovery (pitfall 1), concurrent restart (pitfall 5), Session 0 blindness (pitfall 3), email storm (pitfall 6)

**Research flag:** No additional research needed. Architecture is fully specified in ARCHITECTURE.md with code examples for all four patterns.

### Phase 3: WebSocket Connection Resilience

**Rationale:** WebSocket stability during game launch is the visible symptom that staff and customers see daily ("Disconnected" flash). This is independent of the watchdog refactor — it touches ws/mod.rs on the server and rc-agent's connection loop, neither of which require shared backoff state. It can proceed in parallel with Phase 2 if needed, but the kiosk debounce is a frontend change and can be sequenced last.

**Delivers:** WebSocket ping/pong keepalive in ws/mod.rs (30s ping, 10s pong timeout); rc-agent application-level pings every 10s; kiosk disconnect debounce (15s threshold before showing "Disconnected"); multi-missed-heartbeat threshold before marking pod offline

**Features addressed:** WebSocket stays connected during game launch, kiosk shows stable connection state

**Pitfalls avoided:** WebSocket drop during CPU spike (pitfall 4)

**Research flag:** No additional research needed. tokio-tungstenite ping/pong API is stable and documented; kiosk debounce is a standard React/Next.js pattern.

### Phase 4: Deployment Pipeline Hardening

**Rationale:** Deploy reliability (kill → verify-dead → download → size-check → start → verify-reconnect) is independent of Phases 1-3 and can be done last or in parallel. It is lower urgency because the existing deploy discipline (Pod 8 first) provides a manual safety net, but the file lock pitfall (Windows binary locking) makes automated deployment risky until this phase is complete.

**Delivers:** Hardened deploy sequence in pod-agent exec chain; process tree kill (/T flag); binary size verification; explicit verification of WebSocket reconnect before declaring deploy success; Defender exclusion verification check

**Features addressed:** Clean process lifecycle for deploy, consistent binary behavior across 8 pods, idempotent pod-agent commands

**Pitfalls avoided:** File lock on binary replace (pitfall 2), partial deploy success declarations

**Research flag:** No additional research needed. The correct sequence is fully specified in PITFALLS.md and the existing MEMORY.md deployment rules.

### Phase Ordering Rationale

- Phase 1 must come first because shared AppState changes (pod_backoffs, email_alerter) are required by Phases 2 and 3 in rc-core
- Phase 2 addresses the most severe operational failures (crash loops, double-restarts, silent failures) — the highest-impact work after the foundation is set
- Phase 3 addresses the most visible user-facing problem (kiosk "Disconnected" flash) and is technically independent — can be sequenced as Phase 2b if team capacity allows parallel tracks
- Phase 4 is the safest to defer because the manual deploy protocol (Pod 8 first) provides coverage, but it should complete before the system handles fully unsupervised deploys

### Research Flags

Phases needing deeper research during planning: **None.** All four phases have well-specified implementations from direct codebase inspection + archived Phase 05 research. The architecture is detailed, the code patterns are specified with examples, and the test gates are defined.

One open question requiring operational verification (not research): **Node.js on Racing-Point-Server (.23).** Run `node --version` on the server before Phase 2 deploy. The entire email alert path depends on this. If absent, install Node.js LTS before wiring up email_alerter.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions locked in existing Cargo.toml. No new deps. Every primitive verified against actual crate APIs. |
| Features | HIGH | Derived from direct codebase inspection of pod_monitor.rs, pod_healer.rs, and archived Phase 05 research. Priority matrix is well-reasoned. |
| Architecture | HIGH | Component boundaries derived from actual code. Data flows verified against existing implementation. Two key modules (watchdog.rs, email_alerts.rs) already implemented and tested. |
| Pitfalls | HIGH | All 7 critical pitfalls identified from real failure modes in the codebase, not theoretical risks. Session 0 pitfall and concurrent restart pitfall have been encountered in production. |

**Overall confidence:** HIGH

### Gaps to Address

- **Node.js on server (.23):** Verify with `node --version` before Phase 2. If absent, install Node.js LTS. Do not block Phase 1 on this — it only affects email_alerts.rs wiring.
- **agent_senders channel liveness:** The research notes that `contains_key` in agent_senders is not sufficient to confirm an active WebSocket — the channel may be full or closed. During Phase 2, implement a send-ping-and-check-error pattern rather than just presence check.
- **tasklist text parsing brittleness:** Using `tasklist | findstr rc-agent` is fragile if another process contains "rc-agent" in its name. Acceptable for MVP; flag for replacement with PID-based check in v1.x.
- **Defender exclusion coverage:** Exclusions on all 8 pods should be verified individually before Phase 4 deploy hardening. Assumption of coverage from one pod's config has caused issues before.

## Sources

### Primary (HIGH confidence)
- Codebase inspection — `pod_monitor.rs`, `pod_healer.rs`, `email_alerts.rs`, `watchdog.rs`, `state.rs`, `ws/mod.rs`, `udp_heartbeat.rs`, `pod-agent/src/main.rs`, `rc-agent/src/main.rs`
- `.planning/archive/hud-safety/phases/05-watchdog-hardening/05-RESEARCH.md` — complete prior-art analysis of this codebase; Session 0 pitfall, flapping, email storm, concurrent restart
- `.planning/PROJECT.md` — active requirements, constraints, out-of-scope items
- `MEMORY.md` — Session 0 fix history, deploy rules, network map, Defender exclusions, pod architecture

### Secondary (MEDIUM confidence)
- tokio docs — `tokio::process::Command`, `tokio::time`, `tokio::sync::mpsc` all stable in tokio 1.x
- axum 0.8 changelog — WebSocket feature unchanged 0.7→0.8 for handler API
- tokio-tungstenite 0.26 crates.io — version confirmed from Cargo.lock; current stable as of research date
- systemd / supervisord docs — escalating backoff and post-restart health check patterns used for table stakes comparison
- Axum WebSocket GitHub discussions — no automatic reconnect in standard; backoff with jitter recommended
- Microsoft support: taskkill behavior — file lock timing after process termination, process tree kill requirement

### Tertiary (MEDIUM confidence)
- Kudu wiki: locked files during deployment — kill-before-replace pattern; lock release timing after process termination

---
*Research completed: 2026-03-13*
*Ready for roadmap: yes*
