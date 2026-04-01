# Racing Point Operations (Unified)

## Current State

**Shipped:** v1.0 through v5.5, v7.0-v8.0, v11.0, v16.0-v16.1, v18.0-v18.2, v21.0 (2026-03-13 to 2026-03-23)

The pod management stack is reliable and well-structured: rc-sentry is a hardened 6-endpoint fallback tool; rc-agent decomposed into 5 focused modules; rc-common provides shared exec primitives; 67+ tests cover billing, failure detection, and FFB safety. 61+ phases shipped across 11+ milestones. v21.0 added cross-project sync: shared TypeScript types, OpenAPI specs, contract tests with CI, unified deploy scripts, standing rules across all repos, and 231-test E2E framework.

## Shipped Milestone: v16.1 Camera Dashboard Pro (2026-03-22)

**Delivered:** Professional NVR camera dashboard with 13 cameras, DMSS HD-inspired UI. Hybrid streaming (cached snapshots for grid + WebRTC fullscreen via go2rtc), 4 layout modes (1×1/2×2/3×3/4×4), drag-to-rearrange, zone grouping, camera naming, layout persistence via server-side JSON, and dual deployment (rc-sentry-ai :8096 + web dashboard :3200). 936-line cameras.html + 849-line React page.tsx, 21 requirements, 4 phases, 7 plans.

## Shipped Milestone: v21.0 Cross-Project Sync & Stabilization (2026-03-23)

**Delivered:** Full audit of 30 repos — 3 dead repos archived, 16 repos normalized (git config + .gitignore), 7 npm high vulns fixed, shared TypeScript types package (`packages/shared-types/`), OpenAPI 3.0 spec (66 endpoints), Vitest contract tests with GitHub Actions CI, unified deploy scripts (`deploy.sh` + `check-health.sh`), deployment runbook, standing rules synced to all repos + Bono VPS, 231-test E2E framework. 6 phases, 18 plans, 71 commits, 20K+ lines changed across 94 files.

**Known gaps (server offline):** 8 requirements deferred — auto-seed deploy, bat deploy to pods, process guard scan, E2E test execution. All code complete, verification pending when server comes online.
- Full contract layer: shared TypeScript types between racecontrol/kiosk/admin, OpenAPI specs for all APIs, contract tests that break on drift, CI checks
- Unified deployment: clean deploy-staging (714 dirty files), unified deploy scripts for all services, deployment runbook

**Constraints:**
- Cross-repo work — touches racecontrol, kiosk, racingpoint-admin, comms-link, deploy-staging, pod-agent
- Bug fixes require pods to be online for verification
- E2E tests require both POS (:3200) and Kiosk (:8000/:3300) running
- Contract tests must not break existing APIs — additive only

## Shipped Milestone: v22.0 Feature Management & OTA Pipeline (2026-03-25)

**Delivered:** Feature flags (SQLite+cache+WS sync), OTA pipeline (canary→staged→health), admin /flags + /ota pages. Standing rules gate (76 rules classified), gate-check.sh, 7 phases (176-182), 48 requirements.

## Shipped Milestone: v23.0 Audit Protocol v4.0 — Automated Fleet Audit System (2026-03-25)

**Delivered:** Single-command fleet audit runner (`audit.sh --mode full --auto-fix --notify --commit`). 60 phases across 18 tiers, parallel engine (4-concurrent), delta tracking, auto-fix with whitelist, Bono/WhatsApp notifications. Pure bash + jq. 42 requirements, 16 plans, 5 phases (189-193).

## Current Milestone: v34.0 Time-Series Metrics & Operational Dashboards

**Goal:** Give the venue time-series depth so operators can answer "what happened last Tuesday at 8pm" without grepping JSONL logs — making autonomous action loops observable and queryable.

**Target features:**
- SQLite metrics TSDB with 1-min resolution, 7-day raw retention, hourly/daily rollups (90-day)
- Metrics query API — query by name/time range, list all metric names, current snapshot
- Next.js /metrics dashboard with sparkline charts, pod selector, time range picker, 30s auto-refresh
- Prometheus exposition format endpoint (zero-cost future option, no Prometheus server deployed)
- TOML-configured alert thresholds evaluated every 60s against TSDB, firing to WhatsApp alerter

**Constraints:**
- SQLite WAL mode (existing pattern) — no new database dependencies
- Metrics captured: CPU, GPU temp, FPS, billing, revenue, WS connections
- Replaces aspirational Prometheus/Grafana with venue-scale SQLite + custom Next.js
- Extends existing alert_engine.rs — not a new alerting system
- Dashboard goes in racingpoint-admin (admin app, port 3201)
- Must not increase server memory footprint significantly (ring buffer, not unbounded)

## Paused Milestone: v32.0 Autonomous Meshed Intelligence

**Goal:** Close all action loops in Meshed Intelligence so the venue self-heals end-to-end: diagnose → fix → permanent fix → cascade to fleet → never debug the same issue twice.

**Target features:**
- Autonomous game launch fix + cascade — diagnose launch failure, apply fix, auto-retry (2x with clean state reset), encode permanent fix as Tier 1 deterministic check (KB hardening), cascade solution via mesh gossip to all pods + POS
- Predictive alert → action pipeline — connect predictive_maintenance alerts to diagnostic engine → tier engine (currently log-only, no action taken)
- Experience scoring integration — wire experience_score.rs into main loop, feed scores to fleet health API, auto-flag/remove low-scoring pods
- Tier 5 WhatsApp escalation — complete the stub via Bono VPS Evolution API
- Enhanced night ops + MMA — full MMA diagnostic step, auto-fix application, morning readiness report to Uday
- Model reputation auto-demotion — promote from log-only warning to actual roster removal when accuracy < 30%
- Revenue protection triggers — game running without billing, session ended but game active, pod down during peak hours
- Weekly fleet intelligence report — automated report to Uday (auto-resolution rate, MTTR, top issues, budget, KB growth)
- KB hardening pipeline — solutions succeeding 3+ times across 2+ pods auto-promote to Tier 1 deterministic checks ($0 forever)

**Constraints:**
- All modules already exist as files — this is wiring + enhancement, not greenfield
- v31.0 built MMA engine, tier engine, mesh gossip, KB — this milestone makes them fully autonomous
- Budget controls ($5-20/day per node) already enforced — no new cost risks
- WhatsApp escalation goes through Bono VPS Evolution API (not direct)
- Cascade is recursive per standing rules — fix one pod → gossip to fleet → verify on each pod
- KB promotion lifecycle exists (Discovered → Candidate → Fleet-Verified → Hardened) but "Hardened" doesn't generate Tier 1 code yet

## Current Milestone: v23.1 Audit Protocol v5.0 — Cross-Service Validation & Gap Closure

**Goal:** Close 19 gaps found in audit protocol v4.0 where checks passed but user-visible systems were broken. Five gap patterns: Wrong Layer (checking infrastructure not the consuming service), Count vs Health (counting items without verifying they work), Missing Config Validation (env vars/credentials unchecked), Missing Dashboard/UI Check (backend passes but user page untested), Missing Cross-Service Dependency (services checked independently but dependency chain unverified).

**Target features:**
- Fix Phase 19 display resolution (currently a no-op — hardcoded 1920x1080 assumption)
- Fix Phase 30 WhatsApp Evolution API live connection state verification
- Fix Phase 10 AI healer Ollama model test query (qwen2.5:3b must be functional, not just installed)
- Fix Phase 20 kiosk static file serving verification from pod perspective
- Fix Phase 21 billing endpoint unreachable→PASS false positive
- Fix Phase 44 rc-sentry-ai cross-service camera check (face-audit.jsonl recency, not just existence)
- Fix Phase 07 allowlist spot-check (verify known-good process in list, not just count)
- Fix Phase 02 config value validation (ws_connect_timeout >= 600ms, app_health URLs correct)
- Fix Phase 09 self-monitor liveness beyond uptime proxy
- Add go2rtc startup warmup to start-rcsentry-ai.bat (prevents NVR RTSP flood)
- Cross-service dependency checks for phases 35+36 (sync timestamp delta), 38+46 (relay E2E)

**Constraints:**
- All fixes are bash edits to existing phase scripts in audit/phases/ — no compiled dependencies
- rc-sentry-ai staggered startup requires ONNX build env (deferred or bat-based workaround)
- Must not increase audit runtime beyond 5 minutes (parallel execution preserved)
- Must maintain 346+ PASS baseline (no regressions)

## Current Milestone: v25.0 Debug-First-Time-Right — Systematic Debugging Quality

**Goal:** Eliminate the pattern of multi-attempt debugging by building verification frameworks, observable state transitions, boot resilience, and enforced startup verification into the Rust codebase and operational tooling. Born from a retrospective audit of 11 multi-attempt bugs (avg 2.4 attempts each) revealing 7 root cause categories.

**Target features:**
- Chain-of-verification framework in Rust — verify each step: input → transform → parse → decision → action (not just endpoints)
- Observable state transitions — alerts on MAINTENANCE_MODE writes, config fallbacks, sentinel file creation, empty allowlist
- Boot resilience patterns — periodic re-fetch loops for anything fetched once at startup (allowlist, config, feature flags)
- Startup enforcement audit — scan all bat files across 8 pods, ensure every manual fix has code enforcement
- Pre-ship verification gate — domain-matched verification (visual changes = visual checks, network = connection checks)
- Cause Elimination Process enforcement — structured template before declaring any non-trivial bug fixed
- Silent failure elimination — eprintln! for errors before logging init, explicit alerts for state machine transitions

**Constraints:**
- Rust/Axum stack — changes to rc-agent + racecontrol require binary rebuild + fleet deploy
- Must not break existing billing, lock screen, session management, or recovery systems
- Bat file changes must be deployed alongside binaries (standing rule: bat sync)
- Pod 8 canary-first testing for all changes
- Backward compatible with existing kiosk/PWA/admin flows

**Audit evidence (2026-03-26):**
- 8+ incidents: proxy verification (build_id/health OK ≠ bug fixed)
- 5+ incidents: manual fixes without code enforcement (regress on reboot)
- 3+ incidents: incomplete root cause analysis (piecemeal fixes)
- 6+ incidents: silent failures (no observable state transitions)
- 2+ incidents: boot-time transient failures (no periodic retry)
- 2+ incidents: context/semantic mismatch (dev vs production)

## Current Milestone: v24.0 Game Launch & Billing Rework — Self-Improving Launch Engine

**Goal:** Rework the game launcher and billing system to be resilient, accurate, and self-improving. Games must launch flawlessly, billing must start precisely when the car is on-track and controllable (not during loading), and every launch/billing event feeds a metrics foundation that makes the system smarter over time.

**Target features:**
- Resilient game launcher for AC, F1 25, and iRacing with structured error taxonomy and fast crash recovery (<60s)
- On-track-only billing — PlayableSignal reworked to detect car-controllable state, not just process alive
- Self-improving metrics foundation — every launch, crash, billing event recorded (SQLite + JSONL)
- Dynamic timeouts tuned from historical launch data per game/car/track/pod combo
- Pre-launch health checks informed by past failure causes per pod
- Combo reliability scores visible in admin dashboard
- Auto-suggest alternatives when a car/track combo has low success rate
- Auto-retry (2x) on launch failure with clean state reset, then staff alert
- Crash pattern learning — track what fails, what fix works, adapt recovery actions
- Launch gate / deferred billing bug fix (active_timers vs waiting_for_game mismatch)

**Constraints:**
- Rust/Axum stack — changes to rc-agent + racecontrol require binary rebuild + fleet deploy
- Must not break existing billing, lock screen, or session management
- PlayableSignal rework must cover AC (UDP telemetry), F1 25 (UDP), iRacing (shared memory)
- Metrics foundation must be extensible for future processes (recovery, pre-flight) in later milestones
- Pod 8 canary-first testing for all changes
- Backward compatible with existing kiosk/PWA launch flow

## Current Milestone: v26.0 Meshed Intelligence — Self-Healing AI Fleet

**Goal:** Every node (8 pods + server) autonomously diagnoses and heals itself using the Unified Protocol + 4 OpenRouter models. Solutions propagate via gossip mesh — solve once, apply everywhere. No issue debugged twice. 4-day sprint with James (on-site, pods) + Bono (VPS 24/7, server/cloud/dashboard).

**Target features:**
- Diagnostic Engine per node: 5-tier autonomous diagnosis (deterministic → KB → single model → 4-model → human)
- Local SQLite Knowledge Base per node: solutions + experiments + confidence scoring
- OpenRouter Integration: 4 models (Qwen3, DeepSeek R1, DeepSeek V3, Gemini 2.5 Pro) embedded in rc-agent
- Budget Manager: $10/day/pod hard cap, $20/day/server, cost tracking + mechanical fallback at ceiling
- Mesh Gossip Protocol: solution propagation via existing WebSocket (announce → verify → promote → harden)
- Server Coordinator: fleet KB aggregation, canary promotion (3+ successes → fleet-verified), pattern detection
- Admin Dashboard: mesh intelligence page (solution feed, budget tracker, model performance, KB browser)
- Predictive Maintenance: threshold-based anomaly detection (USB, GPU temp, disk, error rate)
- Customer Experience Scoring: per-pod quality score with auto-rotation at <50%
- Night Operations: autonomous midnight maintenance cycle (audit + diagnose + fix + report)
- Multi-Venue Cloud KB: solutions sync across venues via Bono VPS
- Fleet Intelligence Reports: weekly automated WhatsApp report to Uday

**Owner split:**
- **James (on-site):** Phases 229-230 (Engine+KB), 231-232 (OpenRouter+Budget), 233 (Gossip), 236-237 (Predict+Score), 238 (Night Ops)
- **Bono (VPS, 24/7):** Phases 234-235 (Coordinator+Dashboard), 239-240 (Multi-Venue+Reports)

**Budget:** $10/day/pod + $20/day/server = $100/day fleet. Monthly ~$3,000.

**Foundation:** Unified Protocol v3.0 (A-/A grade), existing rc-agent WS, SQLite, fleet exec, audit.sh, multi-model audit scripts.

**Design doc:** .planning/MESHED-INTELLIGENCE.md

**Constraints:**
- Rust/Axum stack — rc-agent changes require binary rebuild + fleet deploy
- Must not break existing billing, lock screen, session management, or recovery systems
- OpenRouter API key via env var, NEVER in code or git
- Budget hard caps enforced in code — ceiling hit = mechanical fallback, never blocks ops
- Pod 8 canary-first for all pod-side changes
- Shared types in rc-common for solution schema + gossip messages
- Backward compatible with existing kiosk/PWA/admin flows

## Current Milestone: v27.0 Workflow Integrity & Compliance Hardening

**Goal:** Fix all 72 MMA audit findings (22 P0, 30 P1, 20 P2) across the entire customer-to-session pipeline — financial atomicity, state machine integrity, security, safety, legal compliance (India), game-specific flows, deployment safety, staff controls, and UX.

**Target features:**
- Financial atomicity: Single-transaction billing start (wallet debit + session + journal), idempotency keys on all money-moving POSTs, row-level wallet locking (SELECT FOR UPDATE)
- State machine integrity: Formal FSM transition table with server-side CAS writes, cross-FSM invariants (billing vs game state), session finalization guards (single terminal write)
- Security: INI injection whitelist (car/track names), FFB GAIN cap at 100, PIN CAS redemption, RBAC roles (cashier/manager/superadmin), WSS for WebSocket, audit log append-only
- Legal compliance (India): 18% GST separation in double-entry journal, DPDP Act parental consent for under-18, enhanced waiver with guardian flow, Consumer Protection Act pricing/refund display
- Game-specific hardening: Steam pre-launch validation, process name corrections (F1/iRacing/LMU/Forza), Forza session enforcer, AC EVO config adapter, iRacing subscription check
- Deployment safety: OTA session drain, graceful agent shutdown with auto-refund, billing timer DB persistence (60s heartbeat)
- Staff controls: Self-top-up block, discount approval gates, coupon state machine (available→reserved→redeemed→released), staff abuse analytics
- Billing improvements: Tier/rate price alignment (Rs.700 → Rs.750 or rate adjustment), split session FSM, extension atomicity, inactivity detection
- UX: Session countdown warnings (5min/1min), PWA game-request timeout (10min TTL), notification outbox with retry, dispute portal
- Resilience: SQLite WAL mode, staggered billing timer writes, reconciliation jobs, orphaned session detection, hardware health heartbeats

**Key decisions:**
- GST rate is 18% (28% slab removed — confirmed by owner)
- Data localization OK — Hostinger Mumbai datacenter
- RBI PPI: Closed-system wallet = no license needed (venue-only, no transfers/withdrawals)
- Minor waiver: Indian Contract Act 1872 makes waivers unenforceable for minors — need operational solution (mandatory guardian presence + video consent + enhanced insurance)
- Tier vs rate alignment: will align `tier_30min` price to match rate calculation or vice versa

**Constraints:**
- Rust/Axum + Next.js stack — changes span racecontrol server, rc-agent, rc-common, kiosk, PWA, admin
- Must not break existing billing, session management, game launch, or recovery systems
- SQLite remains the database (no PostgreSQL migration in this milestone)
- Pod 8 canary-first for all agent-side changes
- Backward compatible with existing deployed kiosk/PWA/admin flows
- Financial changes require E2E verification: create → topup → book → launch → end → verify balance

## Shipped Milestone: v17.1 Watchdog-to-AI Migration (2026-03-25)

**Delivered:** Replaced all dumb restart-loop watchdogs with 4-tier graduated AI recovery. 6 phases, 9 plans, 21 requirements. Recovery events API (ring buffer + fleet alert), spawn verification (500ms/10s health poll), Session 1 spawn path (WTSQueryUserToken), ProcessOwnership registry + RecoveryIntentStore (2-min TTL), GRACEFUL_RELAUNCH sentinel deconfliction, context-aware WoL (skips when sentry handled), MAINTENANCE_MODE JSON payload with 30-min auto-clear + WOL_SENT immediate-clear + WhatsApp alert, self_monitor yields to sentry (eliminates orphan PowerShell), shared ollama.rs in rc-common, sentry breadcrumb grace window in rc-watchdog.

## Shipped Milestone: v17.0 AI Debugger Autonomy & Self-Healing

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

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

Last updated: 2026-04-01

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

## Current Milestone: v30.0 Racing Dashboard UI Redesign

**Goal:** Full motorsport-inspired redesign of all venue management pages with shared component system, F1 timing tower patterns, and MMA design audits.

**Target features:**
- Shared design system (extended Tailwind theme with full color token ramp, JetBrains Mono for numerics, motorsport CSS vars)
- Shared component library (AppShell, PodCard, StatusBadge, MetricCard, PinPad, LeaderboardTable, Countdown, LiveDataTable)
- Login page redesign (6-digit PIN, motorsport aesthetic, racing-control feel)
- Web Dashboard redesign (pods view, sessions, billing, fleet health, leaderboards, settings — 15+ pages)
- Kiosk redesign (customer-facing pod selection, game launch, billing — 10+ pages)
- Admin Dashboard pages (staff management, fleet overview, analytics)
- MMA design audit after each phase batch for quality assurance

**Design direction:** F1 timing tower + pit wall + race control aesthetic. Dark OLED-optimized theme. shadcn/ui + Tailwind v4. Montserrat body + JetBrains Mono for lap times/counters. Racing Red #E10600 accents, Asphalt Black #1A1A1A backgrounds, Gunmetal Grey #5A5A5A secondary. Restrained glow (not neon), precise micro-interactions (150-250ms), status-heavy grids, timer-centric layouts.

**Constraints:**
- Must work on venue LAN (192.168.31.23), all NEXT_PUBLIC_ vars baked at build time
- Kiosk must be touch-optimized (44x44px min touch targets)
- Dark theme only — no light mode toggle needed
- Must preserve all existing API integrations (backend unchanged)
- 13 existing components to upgrade, 25+ pages across web + kiosk apps

## Previous Milestone: v31.0 Autonomous Survival System — COMPLETE (2026-03-31)

5,682 lines, 56 requirements, 6 phases. 3-layer survival system (Watchdog + Fleet Healer + Guardian) with Unified MMA Protocol (4-step convergence engine). Training period: 2026-03-31 to 2026-04-29.

## Current Milestone: v32.0 Cloud Dashboard — Remote Monitoring for Uday

**Goal:** Make MI truly autonomous by separating diagnosis and recovery across 3 independent survival layers, each with its own Unified MMA Protocol via OpenRouter, so no single system failure can kill the healing brain. Prevents every future 2am "pods are down" scenario.

**Target features:**

**Layer 1 — Smart Watchdog (per-pod, Windows service):**
- Binary validation before launch (size, PE header, SHA256 against server manifest)
- Rollback to `rc-agent-prev.exe` if new binary crashes in <30s
- Unified MMA Protocol diagnosis when restart loop detected (5 top-tier OpenRouter models, N-iteration consensus)
- Direct HTTP reporting to server (bypasses dead rc-agent)
- Auto-download correct binary from server if local is corrupted
- Runs independently of rc-agent as a Windows service — survives everything

**Layer 2 — Server Fleet Healer (inside racecontrol):**
- Watches all pods — SSH into dark pods, collect diagnostics, Unified MMA Protocol root cause analysis
- Fleet-wide pattern detection (same failure on 3+ pods = systemic issue, different response)
- Remote binary push + validated restart with session awareness
- Autonomous fix execution for known failure patterns from Knowledge Base

**Layer 3 — External Guardian (James machine / Bono VPS):**
- Watches server itself — if racecontrol dies, restart it via SSH/schtasks
- Unified MMA Protocol diagnosis for server-level failures
- WhatsApp alerts when everything else fails
- Can recreate schtasks, restart services, push binaries to server

**Unified MMA Protocol (extends Unified Protocol v3.1):**
- Incorporates ALL 4 layers of Unified Protocol v3.1 (Quality Gate, E2E, Standing Rules, Multi-Model AI Audit)
- OpenRouter management key (API Key: "Extra Usage Back Up")
- 5 unique top-tier models per diagnosis iteration with N-iteration consensus
- Diagnosis → Verification → Fix → Re-verify cycle at every layer (not just diagnosis)
- Testing phase (30 days): top-tier models to collect training data
- Post-30-days: migrate to cheaper/efficient models after ML data analysis
- SHA256 binary manifest for integrity verification at all layers
- Cross-layer communication: Watchdog → Server direct HTTP (bypasses rc-agent)

**Key architectural change:** MI moves from "brain inside the body" (dies when rc-agent dies) to "3-layer independent survival" (each layer survives the layer below it dying).

**Constraints:**
- Rust/Axum stack — rc-watchdog is already a Windows service (v17.1), extend it
- OpenRouter API key via env var on all machines, NEVER in code or git
- Must not break existing billing, lock screen, session management, or recovery systems
- Pod 8 canary-first for all watchdog changes
- Budget awareness: top-tier models during 30-day training, cheaper models after
- Backward compatible with existing MI (v26.0) — extend, don't replace
- Binary manifest must be signed/verified to prevent corrupted download attacks

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
*Last updated: 2026-03-30 after milestone v31.0 Autonomous Survival System started*
