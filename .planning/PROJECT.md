# Racing Point Operations (Unified)

## Current State

**Shipped:** v1.0 through v5.5 (HUD, Kiosk, Leaderboards, Self-Healing, Bot Expansion, Credits), v7.0 E2E Test Suite, v8.0 Bot Autonomy, v11.0 Agent & Sentry Hardening (2026-03-13 to 2026-03-21)

The pod management stack is reliable and well-structured: rc-sentry is a hardened 6-endpoint fallback tool with timeout/truncation/concurrency safety; rc-agent main.rs is decomposed into 5 focused modules (config, app_state, ws_handler, event_loop); rc-common provides shared exec primitives with feature-gated tokio boundary; 67+ tests cover billing, failure detection, and FFB safety. 55+ phases shipped across 10 milestones.

## Current Milestone: v16.1 Camera Dashboard Pro

**Goal:** Transform the basic 13-camera snapshot grid into a professional NVR dashboard inspired by DMSS HD — hybrid streaming (cached snapshots for grid + WebRTC for fullscreen), configurable layouts (1/4/9/16), camera naming, drag-to-rearrange, and dual deployment (rc-sentry-ai + server web dashboard).

**Target features:**
- Hybrid streaming: snapshot grid (all 13 cameras, background-cached) + WebRTC fullscreen (single camera, sub-second latency via go2rtc)
- Layout modes: 1x1, 2x2, 3x3, 4x4 split-screen with smooth transitions
- Camera naming: persistent friendly names (e.g. "Pod Area", "Cashier") stored in config
- Drag-to-rearrange: reorder cameras in the grid, persist layout
- Click-to-fullscreen with WebRTC upgrade for smooth live video
- Dual deploy: embedded in rc-sentry-ai (:8096) + standalone page accessible from server web dashboard

**Constraints:**
- rc-sentry-ai must be built with dynamic CRT (RUSTFLAGS="-C target-feature=-crt-static") due to ONNX Runtime
- go2rtc needs all 13 cameras added (currently only 3)
- WebRTC requires go2rtc's built-in WebRTC relay (no TURN server needed on LAN)
- Camera names and layout preferences stored in rc-sentry-ai.toml or a separate JSON file
- NVR auth: admin/Admin@123 — snapshot proxy must not expose credentials to browser

## Current Milestone: v17.0 AI Debugger Autonomy & Self-Healing

**Goal:** Close 6 architectural gaps that prevented the system from self-healing when Edge died/stacked on pods. Make pre-flight continuous, add browser watchdog, give AI debugger execution capability for safe actions, and let the pod healer relaunch Edge.

**Target features:**
- Periodic idle-state health checks (Edge alive + window rect + HTTP server) every 60s when no billing session
- Browser watchdog in LockScreenManager — poll browser_process, detect stacking (>5 Edge), auto-relaunch
- AI debugger structured action parsing — safe-action whitelist (kill_edge, relaunch_lock_screen, restart_rcagent) for Tier 3/4 responses
- Pod healer HealAction::RelaunchLockScreen — taskkill Edge + WS ForceRelaunchBrowser message
- Proactive WARN log scanner in healer cycle with threshold-based AI escalation

**Constraints:**
- rc-agent changes (browser watchdog, idle health, action whitelist) require pod binary rebuild + fleet deploy
- racecontrol changes (healer, WARN scanner) require server binary rebuild
- Must not break existing billing, lock screen, or session management
- AI action whitelist must be conservative — only pre-approved safe actions, never arbitrary shell

**Incident trigger:** 2026-03-22 — Pod 6/7 had 25 stacked Edge processes (uncentered blanking), Pod 1 showed Instagram. System detected issues but couldn't self-heal.

## Current Milestone: v18.2 Debugging & Quality Gates

**Goal:** Fix the root cause of bugs slipping through GSD execution — 135 unit tests caught 0/8 integration bugs in v18.0. Reorganize bloated standing rules, build an integration test script that starts real daemons and verifies round-trip message flow, and wire it into GSD as a post-execution gate.

**Target features:**
- CLAUDE.md standing rules reorganized into categories (process, deploy, comms, code quality) with pruning of obsolete/duplicate rules
- Comms-link integration test script: start daemon → send real WS exec/chain/delegation → verify results
- Cross-platform syntax check (node --check on both James + Bono) as pre-deploy gate
- Contract tests for caller/callee parameter agreements (chainId passthrough, from field, message types)
- Wire integration tests into GSD execute-phase as automatic post-execution verification

**Constraints:**
- Integration tests must run against real comms-link daemons (not mocks)
- Must not require manual intervention — fully automated gate
- Rules cleanup must preserve all still-valid rules, just reorganize

## Shipped Milestone: v18.1 Seamless Execution Hardening

**Goal:** Fix the 3 critical reliability gaps found in v18.0 deployment: James daemon has no auto-recovery (crash/reboot = permanent relay outage), chain HTTP endpoint broken (chain_result not routed to broker), and no visibility when relay is down.

**Target features:**
- James comms-link daemon auto-recovery via Task Scheduler watchdog + HKLM Run key boot start
- Fix /relay/chain/run HTTP endpoint — route chain_result WS message through ExecResultBroker
- Graceful degradation visibility — health probe in skills, connection status in relay responses

**Constraints:**
- All implementation in comms-link repo (C:/Users/bono/racingpoint/comms-link)
- Must not break existing v18.0 features (135 tests must stay green)
- James-side daemon management uses Windows Task Scheduler (not PM2 — Windows)

## Shipped Milestone: v18.0 Seamless Execution

**Goal:** Enable full bidirectional dynamic execution between James (on-site AI, Windows 11) and Bono (VPS AI, Linux) — when either AI needs something done on the other's machine, it delegates the task, the remote side executes, and results flow back seamlessly. Evolves the static 13-command exec registry into a dynamic, chainable execution protocol.

**Target features:**
- Dynamic command registration — either side can register new exec commands at runtime without redeploying
- Bidirectional task chain — James sends a task to Bono, Bono executes and returns structured results (and vice versa)
- Shell relay with approval gates — arbitrary shell commands between machines with APPROVE tier security
- Execution chain orchestration — multi-step tasks where step N+1 depends on step N's output
- Seamless Claude-to-Claude coordination — when a user asks James something that requires Bono, James auto-delegates and integrates the response
- Audit trail for all cross-machine execution (who requested, what ran, exit codes, duration)

**Constraints:**
- Must extend existing comms-link WebSocket + exec-protocol infrastructure (no new transport)
- Security model must retain approval tiers (AUTO/NOTIFY/APPROVE) — no unaudited shell access
- Must work over both direct WebSocket and Tailscale paths
- Backward compatible with existing 13 static commands

## Current Milestone: v11.2 RC Sentry AI Debugger

**Goal:** Move crash diagnostics from rc-agent (where they die with the patient) to rc-sentry (external survivor). When rc-agent crashes, rc-sentry reads local crash logs, runs Tier 1 deterministic fixes, and restarts rc-agent with context — instead of blind restarts.

**Target features:**
- Health endpoint polling (localhost:8090/health every 5s) for rc-agent crash detection — anti-cheat safe (no process inspection)
- Post-crash log analysis: read startup_log, stderr capture, panic output to understand WHY rc-agent crashed
- Tier 1 deterministic fixes before restart: clean stale sockets, kill zombie processes, repair config, clear shader cache
- Crash pattern memory (debug-memory.json) for instant replay of known fixes
- Tier 3 Ollama query (James .27:11434) for unknown crash patterns
- Crash diagnostics reported to server via fleet API
- Escalation decision: restart vs alert staff vs block pod
- Game-crash debugging stays in rc-agent (it's alive for those)

**Constraints:**
- Must NOT trigger F1 25 (Easy Anti-Cheat) or iRacing anti-cheat — no process inspection, no debug APIs, health polling only
- Extend existing rc-sentry binary (v11.0, 6 endpoints) — no new binary to deploy
- Same Rust/Axum stack, no new crate dependencies

## Current Milestone: v11.1 Pre-Flight Session Checks

**Goal:** Run automated pre-flight checks before every customer session (on BillingStarted). Auto-fix failures (restart ConspitLink, kill orphaned games, etc.), alert staff only if auto-fix fails. Block pod with "Maintenance Required" screen when unfixable.

**Target features:**
- Pre-flight check framework in rc-agent triggered by BillingStarted
- Display checks: lock screen centered/visible, overlay renders correctly
- Hardware checks: wheelbase HID connected, ConspitLink running with valid config
- Network checks: WebSocket connected, UDP heartbeat alive
- Game checks: no orphaned game processes, AC content accessible
- Billing checks: no stuck session from previous customer
- System checks: disk space > 1GB, memory > 2GB free
- Auto-fix on failure before alerting staff
- "Maintenance Required" lock screen state when pre-flight fails
- Staff notification via WS + kiosk dashboard badge

## Current Milestone: v12.1 E2E Process Guard

**Goal:** Continuous process monitor on every machine (James, server, all 8 pods) that enforces a whitelist of approved processes, ports, and auto-start entries. Auto-kills violations and alerts staff. Triggered by oversight where Steam, Leaderboard kiosk, and voice assistant watchdog were missed during manual audit.

**Target features:**
- Central whitelist in racecontrol.toml with per-machine overrides (James allows Ollama, pods don't allow Steam, etc.)
- Continuous monitoring daemon on every machine (rc-agent module on pods, racecontrol module on server, standalone on James)
- Process audit: running processes vs. approved whitelist, flag + auto-kill non-whitelisted
- Auto-start audit: HKCU/HKLM Run keys, Startup folder, Scheduled Tasks — remove non-whitelisted entries
- Port audit: listening ports vs. approved port list
- Pod binary guard: detect rc-agent/racecontrol/pod-agent running on wrong machine (standing rule #2)
- Alert on violation: WS notification to staff kiosk + email escalation
- Audit log: all violations logged with timestamp, machine, process, action taken

## Current Milestone: v15.0 AntiCheat Compatibility

**Goal:** Audit and harden all pod-side RaceControl behaviors so that rc-agent, rc-sentry, and kiosk software never trigger anti-cheat detection in F1 25 (EAC), iRacing, LMU, AC EVO, or EA WRC — preventing customer account bans.

**Target features:**
- Full pod-side behavior audit: keyboard hooks, process monitoring/killing, shared memory telemetry, USB lockdown, registry modifications, unsigned binaries, port listeners
- Risk classification matrix per anti-cheat system (EAC, iRacing AC, rF2/LMU AC, Kunos AC, EA AC)
- Auto safe mode in rc-agent: detect protected game launch, automatically disable risky subsystems (hooks, process killing, allowlist enforcement) until game exits
- Replace low-level keyboard hook (Phase 78) with anti-cheat safe alternative (policy-based, no SetWindowsHookEx)
- Gate shared memory telemetry readers to avoid triggering memory inspection detection
- Code sign rc-agent.exe and rc-sentry.exe with real code signing certificate
- Per-game anti-cheat compatibility validation via test sessions on Pod 8
- Anti-cheat compatibility matrix documentation for ops reference

**Constraints:**
- Must NOT break existing billing, lock screen, or session management while in safe mode
- Must complete BEFORE v13.0 Multi-Game Launcher deploys to customers
- Same Rust/Axum stack, minimize new crate dependencies
- Pod 8 canary-first testing for all changes

## Active Milestone: v10.0 Connectivity & Redundancy

**Goal:** Make James (.27) ↔ Server (.23) connectivity bulletproof and give Bono (cloud VPS) full failover capability so the venue keeps running even when the local server goes down.

## Active Milestone: v9.0 Tooling & Automation Research

**Goal:** Research and evaluate tools, skills, and plugins to improve Racing Point Operations — Claude Code skills, MCP servers, deployment automation, and monitoring/alerting.

## Planned Milestone: v13.0 Multi-Game Launcher

**Goal:** Launch games other than Assetto Corsa (F1 25, iRacing, AC EVO, EA WRC, LMU) from kiosk/PWA with PlayableSignal-gated billing, per-game telemetry capture, and lap times feeding into the existing leaderboard. Extends existing SimAdapter trait, GameProcess, and BillingGuard — no new crate dependencies.

**Target features:**
- Game launch profiles (TOML config) for 5 games with crash recovery and fleet dashboard visibility
- PlayableSignal billing — charges start when game is playable, not during loading/shader compilation
- Per-game telemetry adapters: F1 25 (UDP), iRacing (shared memory), LMU (rF2 shared memory), AC EVO (best-effort, feature-flagged), EA WRC (JSON UDP)
- Track name normalization and multi-game leaderboard integration

## Planned Milestone: v14.0 HR & Marketing Psychology

**Goal:** Embed 12 behavioral psychology frameworks into RacingPoint's HR and Marketing systems — customer progression (driving passport, badges), peak-end session design, retention loops (streaks, variable rewards, loss-framed notifications), community rituals (Discord weekly drops), pricing psychology (anchoring, scarcity), staff gamification (opt-in leaderboards, badges, challenges), and HR/hiring enhancements (SJTs, Cialdini campaign templates).

**Phases:** 89–96 (8 phases, 38 requirements)
**Owner:** Bono (cloud — psychology.rs, PWA, admin, bots) + James (venue — deploy rebuilt racecontrol binary after schema changes)
**Research + Phase 89 plans:** Complete (2026-03-21)

## Paused Milestone: v6.0 Salt Fleet Management

**Goal:** Replace the custom pod-agent/remote_ops HTTP endpoint with SaltStack for fleet management. Blocked at BIOS AMD-V gate for WSL2.

## What This Is

Unified project encompassing all Racing Point eSports operations:
- **RaceControl:** 8 sim racing pods managed from a central server (racecontrol, rc-agent), staff kiosk, cloud PWA. Captures lap times, sector splits, and telemetry from Assetto Corsa and F1 25 with leaderboards and driver profiles.
- **Comms Link:** Persistent James-Bono AI communication — WebSocket, watchdog, LOGBOOK sync, alerts. (Shipped, archived)
- **AC Launcher:** Purpose-built Assetto Corsa session management — difficulty tiers, billing sync, safety enforcement, multiplayer orchestration. (Shipped, archived)

## Core Value

Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.

## Requirements

### Validated (v1.0 + v2.0 — Shipped)

- ✓ WebSocket connection between racecontrol and rc-agent — existing
- ✓ Pod-agent HTTP exec endpoint for remote commands — existing
- ✓ Game launch from staff kiosk — existing
- ✓ UDP heartbeat for pod liveness detection (6s timeout) — existing
- ✓ Pod monitoring and healing (pod_monitor.rs, pod_healer.rs) — existing
- ✓ Lock screen with PIN auth — existing
- ✓ Billing lifecycle (start/stop/idle) — existing
- ✓ Escalating watchdog backoff (WD-01)
- ✓ Shared backoff state in AppState (WD-02)
- ✓ Post-restart verification (WD-03)
- ✓ Backoff reset on recovery (WD-04)
- ✓ WebSocket keepalive ping/pong (CONN-01)
- ✓ Kiosk disconnect debounce (CONN-02)
- ✓ Auto-reconnect with backoff (CONN-03)
- ✓ Config validation at startup (DEPLOY-01)
- ✓ Safe deploy sequence (DEPLOY-02)
- ✓ Honest exec status codes (DEPLOY-03)
- ✓ Stale config cleanup (DEPLOY-04)
- ✓ Rolling deploy without session disruption (DEPLOY-05)
- ✓ Email alerts on failure (ALERT-01)
- ✓ Rate-limited alerts (ALERT-02)
- ✓ Clean branded screens (SCREEN-01, SCREEN-02, SCREEN-03)
- ✓ PIN auth unification (AUTH-01)
- ✓ Performance targets met (PERF-01 through PERF-04)

- ✓ Server IP pinning and DHCP reservation (HOST-01 through HOST-04) — v2.0
- ✓ Pod lock screen hardening with startup connecting state (LOCK-01 through LOCK-03) — v2.0
- ✓ Edge browser hardening (EDGE-01 through EDGE-03) — v2.0
- ✓ Staff dashboard lockdown and power controls (KIOSK-01, KIOSK-02, PWR-01 through PWR-06) — v2.0
- ✓ Customer experience branding and session results (BRAND-01 through BRAND-03, SESS-01 through SESS-03) — v2.0

### Completed (v4.0 — shipped 2026-03-16)

- ✓ rc-agent as Windows Service with auto-restart on crash (SVC-01 through SVC-04)
- ✓ WebSocket-based remote exec for pod management when HTTP blocked (WSEX-01 through WSEX-04)
- ✓ Firewall auto-configuration in Rust on startup (FW-01 through FW-03)
- ✓ Startup error capture and reporting to racecontrol (HEAL-01 through HEAL-04)
- ✓ Self-healing config (detect and repair missing toml/bat/registry) (HEAL-01 through HEAL-04)
- ✓ Deploy resilience (verify, rollback, handle partial failures) (DEPL-01 through DEPL-05)
- ✓ Fleet health dashboard for Uday (real-time pod status) (FLEET-01 through FLEET-03)

### Completed (v5.0 — shipped 2026-03-17)

- ✓ Bot handles pod crash/hang — detect + auto-kill/restart game or rc-agent without staff (Phase 24)
- ✓ Bot handles billing edge cases — stuck sessions, idle drift, cloud sync failures (Phase 25)
- ✓ Bot handles network/connection drops — WS loss, server unreachable, IP drift (Phase 23)
- ✓ Bot handles USB hardware failures — wheelbase disconnect/reconnect, FFB fault (Phase 24)
- ✓ Bot handles game launch failures — CM hang, AC timeout, launch auto-retry (Phase 24)
- ✓ Bot handles telemetry gaps — detect missing UDP data, alert on persistent drop (Phase 26)
- ✓ Bot handles multiplayer issues — desync detection, safe teardown or auto-rejoin (Phase 26)
- ✓ Bot handles kiosk PIN failures — validation errors, staff unlock, session recovery (Phase 26)
- ✓ Bot handles lap filtering — auto-flag invalid laps, separate hotlap vs practice (Phase 26)

### Completed (v5.5 — shipped 2026-03-17)

- ✓ Credits replace INR in all user-facing UI — overlay, kiosk, billing history, admin
- ✓ `billing_rates` DB table with 3 configurable tiers (non-retroactive)
- ✓ BillingManager holds in-memory rate cache refreshed at startup and every 60s
- ✓ `compute_session_cost()` rewritten with non-retroactive additive algorithm, accepts tiers param
- ✓ Admin panel Per-Minute Rates table with inline editing
- ✓ billing_rates added to SYNC_TABLES for cloud replication

### Completed (v7.0 — shipped 2026-03-18)

- ✓ Playwright browser tests for kiosk wizard per-game flow
- ✓ API pipeline tests (billing, launch, game state lifecycle)
- ✓ Deploy verification (binary swap, port conflicts, service health)
- ✓ Per-game launch validation (AC, F1 25, EVO, Rally, iRacing)
- ✓ Self-healing test runner with auto-cleanup and retry
- ✓ Kiosk frontend smoke (page load, SSR errors, wizard correctness)
- ✓ Master E2E script reusable for other services

### Completed (v8.0 — shipped 2026-03-19)

- ✓ CLOSE_WAIT socket leak fix + connection hygiene
- ✓ Crash safety panic hook + FFB zero + startup health verification
- ✓ Local LLM (Ollama + qwen3:0.6b + rp-debug) deployed to all 8 pods
- ✓ Dynamic kiosk allowlist with LLM-based process classifier
- ✓ Session lifecycle autonomy (orphan auto-end, crash recovery, fast reconnect)
- ✓ LLM self-test with 22 probes + deterministic verdict + auto-fix patterns 8-14

### Completed (v11.0 — shipped 2026-03-21)

- ✓ rc-common exec.rs — shared sync/async exec primitive with feature-gated tokio boundary
- ✓ rc-sentry hardened — timeout enforcement, 64KB truncation, 4-slot concurrency cap, TCP read fix, tracing
- ✓ rc-sentry expanded to 6 endpoints — /health, /version, /files, /processes + 7 integration tests + graceful shutdown
- ✓ FfbBackend trait seam — mockall-based FFB tests without real HID hardware
- ✓ billing_guard + failure_monitor tests — 12 async/sync tests covering BILL-02/03 and CRASH-01/02
- ✓ rc-agent decomposed — main.rs split into config.rs, app_state.rs, ws_handler.rs, event_loop.rs

### Active (v10.0)

- [ ] Fix server .23 DHCP drift permanently
- [ ] Establish remote exec from James to Server via Tailscale SSH
- [ ] James health-monitors server .23 continuously
- [ ] Config sync: venue racecontrol config pushed to Bono
- [ ] Auto-failover: pods switch to Bono when .23 goes down
- [ ] Failover notifications to Uday
- [ ] Failback: pods return to .23 when it recovers

### Active (v9.0)

- [ ] Evaluate Claude Code custom skills and automation hooks
- [ ] Research MCP servers for Google Workspace, monitoring, venue ops
- [ ] Investigate deployment automation tools for fleet management
- [ ] Evaluate monitoring and alerting stack options
- [ ] Produce actionable recommendations with adoption plan

### Planned (v13.0)

- [ ] Staff can launch F1 25, iRacing, AC EVO, EA WRC, or LMU from kiosk with safe defaults
- [ ] Customer can request game launch from PWA/QR, staff confirms
- [ ] Per-game TOML launch profiles (exe path, args, defaults)
- [ ] Game crash/hang detection, auto-cleanup, crash recovery
- [ ] Game state visible in kiosk and fleet health dashboard
- [ ] PlayableSignal billing — starts when game is playable, not during loading
- [ ] Per-game billing rates configurable in billing_rates table
- [ ] Auto-stop billing on game exit/crash
- [ ] F1 25 telemetry: UDP lap times and sector splits
- [ ] iRacing telemetry: shared memory with session transition handling
- [ ] LMU telemetry: rFactor 2 shared memory plugin
- [ ] AC EVO telemetry: best-effort, feature-flagged (Early Access)
- [ ] EA WRC telemetry: JSON-configured UDP with stage-to-lap mapping
- [ ] Lap/stage times stored in laps table with sim_type field
- [ ] Track name normalization across games
- [ ] Leaderboard endpoints serve multi-game data

### Planned (v15.0 — AntiCheat Compatibility)

- [ ] Audit all pod-side behaviors for anti-cheat risk (keyboard hooks, process monitoring, shared memory, registry, unsigned binaries)
- [ ] Classify each behavior by risk level per anti-cheat system (EAC, iRacing, rF2/LMU, Kunos/EVO, EA/WRC)
- [ ] Implement auto safe mode: rc-agent detects protected game launch and disables risky subsystems
- [ ] Replace low-level keyboard hook with anti-cheat safe kiosk lockdown
- [ ] Gate shared memory telemetry readers behind safe mode (defer reads until anti-cheat allows)
- [ ] Code sign rc-agent.exe and rc-sentry.exe with real certificate
- [ ] Validate anti-cheat compatibility per game via test sessions
- [ ] Document anti-cheat compatibility matrix for all 5 games

### Planned (v14.0 — Psychology Integration)

- [ ] Psychology engine module (psychology.rs) with badge evaluation, streak tracking, notification dispatch (FOUND-01–05)
- [ ] Customer driving passport with track/car completion progress and badge showcase (PROG-01–05)
- [ ] PB confetti celebrations and peak-end session reports (SESS-01–04)
- [ ] Visit streaks, PB-beaten notifications, variable rewards, loss-framed membership nudges (RET-01–06)
- [ ] Discord weekly rituals, record alerts, "RacingPoint Driver" tribal identity (COMM-01–04)
- [ ] Anchoring/decoy pricing display, real-time pod scarcity, commitment ladder, social proof (PRICE-01–04)
- [ ] Opt-in staff leaderboard, skill badges, team challenges, peer kudos (STAFF-01–05)
- [ ] Hiring bot SJTs, Cialdini campaign templates, review nudge optimization, employee recognition (HR-01–05)

### Paused (v6.0 — blocked at BIOS AMD-V)

- [ ] Salt master on WSL2 (James .27) managing fleet
- [ ] Salt minion on all 8 pods + server (.23)
- [ ] remote_ops.rs removed from rc-agent (port 8090 eliminated)
- [ ] Deploy workflow via Salt replaces HTTP server + curl pipeline

### Paused (v3.0 — resume after v5.0)

- [ ] Hotlap events with staff creation and car class rankings
- [ ] Group event results with F1-style auto-scoring
- [ ] Multi-round championship system
- [ ] Driver profiles with class rating and lap history
- [ ] Telemetry visualization with speed trace, lap comparison, and track map
- [ ] Driver skill rating system
- [ ] Public access to all competitive data (no login required)

### Out of Scope

- HUD overlay features — deferred (archived in .planning/archive/hud-safety/)
- FFB safety — deferred (archived research available)
- New game integrations — current sims only (AC, F1 25)
- Real-time chat or messaging between drivers
- Mobile native app — PWA only
- External payment/wallet top-up changes — only billing rate calculation changes, not payment provider integration
- Venue kiosk changes — v3.0 targets cloud PWA only

## Context

- **Venue:** 8 gaming pods (192.168.31.x subnet), 1 server (.23), 1 James workstation (.27)
- **Stack:** Rust/Axum (racecontrol port 8080, rc-agent per-pod), Salt (fleet management), Next.js (kiosk + PWA)
- **Crates:** rc-common (shared types/protocol/exec), racecontrol (server), rc-agent (pod client — decomposed: config.rs, app_state.rs, ws_handler.rs, event_loop.rs), rc-sentry (hardened fallback ops tool)
- **Cloud:** app.racingpoint.cloud (72.60.101.58, Bono's VPS) — existing cloud_sync pushes laps, track records, driver stats
- **Existing data foundations:** laps table (sector1/2/3_ms, valid flag), personal_bests, track_records, telemetry_samples, group_sessions, friendships, drivers (total_laps, total_time_ms)
- **Existing API endpoints:** /leaderboard/{track}, /public/leaderboard, /public/laps/{id}/telemetry, /sessions, /laps
- **PWA scaffolds exist:** leaderboard, telemetry, coaching, tournaments pages (mostly empty)
- **Inspiration:** rps.racecentres.com — Track of the Month, Group Events, Championships, Circuit/Vehicle Records, Driver Data

## Constraints

- **Rust/Axum:** racecontrol and rc-agent must stay Rust — no language change
- **Fleet management:** Salt (SaltStack) replaces pod-agent/remote_ops — salt-master on WSL2, salt-minion on pods
- **No new dependencies:** Use existing crate deps where possible (tokio, reqwest, serde, chrono, tracing)
- **Email via send_email.js:** Reuse existing Gmail auth, don't add SMTP crate
- **Windows:** All pods run Windows 11, Session 1 requirement for GUI processes
- **Backward compat:** Changes must not break existing billing, game launch, or lock screen

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Archive HUD project, start reliability-first | Can't add features on fragile base | Shipped v1.0 |
| Reuse watchdog hardening research | Research already done, high confidence | Shipped v1.0 |
| EscalatingBackoff in rc-common | Shared between core and agent | Shipped v1.0 |
| Email alerts via send_email.js shell-out | Reuses existing Gmail OAuth, no new deps | Shipped v1.0 |
| Pod 8 canary-first deployment | Catch issues on one pod before rolling to all | Shipped v1.0 |
| Lock screen before game kill | Prevents desktop flash during session end | Shipped v1.0 |
| Registry-based pod lockdown | Survives rc-agent restarts, one-time apply | Shipped v1.0 |

## Planned: v8.0 RC Bot Autonomy (Phases 45–49)

**Goal:** Raise rc-agent autonomy from 6/10 to 8/10. Fix live CLOSE_WAIT socket leak (5/8 pods), install crash safety (panic hook + FFB zero), deploy local LLM to all pods, add dynamic kiosk allowlist (eliminates #1 manual intervention), auto-end orphaned sessions, auto-reset pods after billing. Can proceed in parallel with v7.0.

**Evidence:** Audit of git log (80+ commits), live pod logs (CLOSE_WAIT on pods 1/2/3/6/8, 3 fleet-wide WS disconnects, Pod 8 port binding conflicts), and code analysis (no panic hook, 6 unhandled startup failures, billing guard sends alerts but never acts).

## Future Milestone Candidates

- HUD overlay with live sector times and telemetry
- FFB safety (zero wheelbase torque on session boundary)
- Cloud dashboard for remote monitoring
- On-site deployment automation improvements
- Kiosk spectator leaderboard display (venue TV screens)

| Merge pod-agent into rc-agent | Eliminates 2-process dependency, simplifies deploy | ✓ Good — deployed all 8 pods |
| HKLM Run key for Session 1 GUI | Ensures rc-agent starts in user session at login | ⚠️ Revisit — no crash restart, needs Windows Service |
| Batch file firewall rules | netsh in .bat scripts for port 8090 | ⚠️ Revisit — CRLF bug silently breaks rules, move to Rust |

---
*Last updated: 2026-03-21 after milestone v15.0 AntiCheat Compatibility started*
