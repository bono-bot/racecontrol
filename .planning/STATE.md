---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: Roadmap ready, awaiting plan-phase
stopped_at: Completed 56-01-PLAN.md (WhatsApp P0 alerter)
last_updated: "2026-03-20T10:48:48.175Z"
last_activity: 2026-03-20 — Milestone v11.0 Agent & Sentry Hardening started
progress:
  total_phases: 35
  completed_phases: 15
  total_plans: 44
  completed_plans: 38
  percent: 86
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: Roadmap ready, awaiting plan-phase
stopped_at: "Phase 66-03: Task 1 done (wired Bono ExecHandler). Task 2 checkpoint awaiting Bono pull+restart + round-trip verification."
last_updated: "2026-03-20T10:40:52.085Z"
last_activity: 2026-03-20 — v10.0 Connectivity & Redundancy roadmap created, 5 phases (66-70), 22 requirements mapped
progress:
  [█████████░] 86%
  completed_phases: 15
  total_plans: 44
  completed_plans: 37
  percent: 84
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: Roadmap ready, awaiting plan-phase
stopped_at: "Phase 55: server .23 Netdata installed (v2.9.0). Pods offline — deploy pending next venue open."
last_updated: "2026-03-20T10:15:55.382Z"
last_activity: 2026-03-20 — v10.0 Connectivity & Redundancy roadmap created, 5 phases (66-70), 22 requirements mapped
progress:
  [████████░░] 84%
  completed_phases: 15
  total_plans: 39
  completed_plans: 36
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v11.0 Agent & Sentry Hardening — defining requirements

## Current Position

Phase: Not started (defining requirements)
Plan: —
Status: Defining requirements
Last activity: 2026-03-20 — Milestone v11.0 Agent & Sentry Hardening started

Progress: [░░░░░░░░░░] 0% (0/5 phases)

## Phase Map — v10.0 Connectivity & Redundancy

| Phase | Name | Requirements | Access | Status |
|-------|------|--------------|--------|--------|
| 66 | Infrastructure Foundations | INFRA-01, INFRA-02, INFRA-03 | Router web UI + Tailscale | Not started |
| 67 | Config Sync | SYNC-01, SYNC-02, SYNC-03 | Server .23 (Rust changes) | Not started |
| 68 | Pod SwitchController | FAIL-01, FAIL-02, FAIL-03, FAIL-04 | Pods (rc-agent Rust changes) | Not started |
| 69 | Health Monitor & Failover Orchestration | HLTH-01..04, ORCH-01..04 | Server + pods (Rust changes) | Not started |
| 70 | Failback & Data Reconciliation | BACK-01, BACK-02, BACK-03, BACK-04 | Server + pods (Rust changes) | Not started |

**Phase 66:** Router config + Tailscale verification — no code changes, manual network work.
**Phase 67:** Rust changes to racecontrol on .23 — sha2 hash watcher + comms-link sync_push.
**Phase 68:** Rust changes to rc-agent — Arc<RwLock<String>> refactor + SwitchController message + self_monitor guard. Pod 8 canary required before fleet deploy.
**Phase 69:** Rust changes to racecontrol (health probe loop, hysteresis FSM, ORCH broadcast) + comms-link task_request to Bono. Bono VPS changes coordinated via comms-link.
**Phase 70:** Rust changes to racecontrol (DB merge) + failback broadcast. Most complex data integrity phase.

## Phase Map — Active Parallel Work (v9.0 + v10.0 Conspit Link)

| Phase | Name | Status |
|-------|------|--------|
| 55 | Netdata Fleet Deploy | In Progress (55-01 Task 1 done, waiting for server .23 install) |
| 56 | WhatsApp Alerting + Weekly Report | Not started |
| 58 | ConspitLink Process Hardening | In Progress (58-01 done) |
| 59 | Auto-Switch Configuration | Not started |
| 60 | Pre-Launch Profile Loading | Not started |
| 61 | FFB Preset Tuning | Not started |
| 62 | Fleet Config Distribution | Not started |
| 63 | Fleet Monitoring | Not started |
| 64 | Telemetry Dashboards | Not started |
| 65 | Shift Lights & RGB Lighting | Not started |

## Performance Metrics

**Velocity:**
- Total plans completed: 3 (v7.0 milestone)
- Average duration: 7 min
- Total execution time: 0.33 hours

| Plan | Duration | Tasks | Files |
|------|----------|-------|-------|
| 41-01 | 3 min | 2 | 5 |
| 41-02 | 7 min | 2 | 6 |
| 42-01 | 10 min | 2 | 3 |
| 42-02 | 2 min | 1 | 4 |
| 43-01 | 1 min | 1 | 1 |
| 43-02 | 2 min | 2 | 2 |
| 44-01 | 2 min | 1 | 1 |

**Recent Trend:** On track

*Updated after each plan completion*
| Phase 44-deploy-verification-master-script P02 | 5 | 1 tasks | 1 files |
| Phase 45-close-wait-fix-connection-hygiene P01 | 15 | 2 tasks | 3 files |
| Phase 45 P02 | 525640 | 1 tasks | 2 files |
| Phase 45 P02 | 8 | 1 tasks | 2 files |
| Phase 46 P01 | 18 | 2 tasks | 5 files |
| Phase 46 P02 | 6 | 2 tasks | 4 files |
| Phase 47 P02 | 1 | 1 tasks | 1 files |
| Phase 47 P01 | 4 | 2 tasks | 3 files |
| Phase 48 P01 | 18 | 2 tasks | 4 files |
| Phase 48 P02 | 564 | 2 tasks | 3 files |
| Phase 50 P02 | 5 | 1 tasks | 1 files |
| Phase 50 P01 | 8 | 2 tasks | 3 files |
| Phase 50 P03 | 25 | 2 tasks | 6 files |
| Phase 49 P02 | 12 | 2 tasks | 2 files |
| Phase 51 P01 | 193 | 2 tasks | 2 files |
| Phase 52-mcp-servers P01 | 4 | 2 tasks | 6 files |
| Phase 52-mcp-servers P02 | 3 | 2 tasks | 3 files |
| Phase 53-deployment-automation P02 | 2 | 1 tasks | 1 files |
| Phase 53-deployment-automation P01 | 513 | 2 tasks | 1 files |
| Phase 57 P01 | 2 | 1 tasks | 1 files |
| Phase 54-structured-logging-error-rate-alerting P01 | 8 | 1 tasks | 2 files |
| Phase 57 P02 | 12 | 2 tasks | 3 files |
| Phase 57 P03 | 2 | 2 tasks | 1 files |
| Phase 54-structured-logging-error-rate-alerting P02 | 12 | 1 tasks | 1 files |
| Phase 54-structured-logging-error-rate-alerting P03 | 6 | 2 tasks | 4 files |
| Phase 58 P01 | 286 | 1 tasks | 1 files |
| Phase 55 P01 | 815 | 1 tasks | 2 files |
| Phase 66-infrastructure-foundations P03 | 2 | 1 tasks | 3 files |
| Phase 56 P01 | 494 | 2 tasks | 8 files |

## Accumulated Context

### Decisions

(v7.0 E2E Test Suite — key constraints from research)
- Playwright version locked to 1.58.2 with bundled Chromium — msedge channel has documented 30s hang after headed tests
- `workers: 1` and `fullyParallel: false` are mandatory — game launch tests mutate live pod state and collide if parallelized
- `reuseExistingServer: true` is mandatory — venue kiosk is already running on :3300, Playwright must attach not restart
- data-testid attributes must be added to kiosk source (Phase 42) BEFORE any wizard spec is written (Phase 43)
- Pre-test cleanup fixture must exist BEFORE any stateful test — stale games/billing poison subsequent test runs
- Shell scripts own HTTP API verification; Playwright owns browser layer — never blur this boundary
- `/api/v1/fleet/health` is the correct endpoint for ws_connected checks (NOT `/api/v1/pods`) — confirmed pitfall
- F1 25 Steam launch ID is 3059520 (EA Anti-Cheat bootstrapper), NOT store ID 2805550 — must verify on Pod 8
- Pod 8 is the sole test target — never run launch tests on pods 1–7 (may have live customer sessions)
- run-all.sh is the final integration point — only writable once all phase scripts exist (Phase 44)

(Phase 41-01 — shell test library decisions)
- summary_exit exits with FAIL count only — skips are informational, do not cause failure
- lib/common.sh has NO set options — callers manage their own error handling (smoke.sh needs -e, game-launch.sh does not)
- pod_ip() uses hyphens (pod-1 through pod-8) matching POD_ID variable format — Python dict used underscores and silently failed
- TTY check gates ANSI colors — CI gets clean text, terminals get colors

(Phase 41-02 — Playwright and nextest decisions)
- Playwright 1.58.2 with bundled Chromium — msedge channel has documented 30s hang (GitHub #22776)
- fullyParallel:false and workers:1 are non-negotiable — game launch tests mutate live pod state
- reuseExistingServer:true — venue kiosk already running on :3300, must attach not restart
- baseURL defaults to http://192.168.31.23:3300 — KIOSK_BASE_URL env var overrides for dev/CI
- playwright.config.ts at repo root — auto-discovered by npx playwright test without --config flag
- cargo-nextest per-process isolation is the default — not explicitly configured in nextest.toml
- node_modules/ was missing from root .gitignore — fixed; root node_modules was being tracked in git

(Phase 42-01 — data-testid attribute decisions)
- data-testid naming is consistent between book/page.tsx and SetupWizard.tsx for shared step names (step-select-game, step-select-plan, etc.) — Phase 43 specs work against both paths
- wizard-back-btn applies to both back button instances in SetupWizard.tsx footer (non-review and review conditional blocks both use same testid)
- pin-modal placed on PinModal component's outer fixed div inside the component function body (not on render call site in CustomerLanding)
- [Phase 42]: jsErrors array scoped to module level (not testInfo metadata) — simpler than testInfo cast approach; workers:1 means no concurrency issue
- [Phase 42]: outputDir set to ./tests/e2e/results/ — collocates screenshots with test source, not at repo root test-results/
- [Phase 42]: Keyboard nav test accepts either advanced or stayed on plan step — only asserts Tab/Enter cause no JS errors as regression guard
- [Phase 43]: AC wizard BROW-02 tests preset path only (default experienceMode=preset) — custom track/car path separate concern
- [Phase 43]: session_splits and driving_settings steps use 3s isVisible guards — test survives different tier configs on live server
- [Phase 43]: Experience DB guards: empty experience list is non-fatal — step rendered without experiences is acceptable behavior
- [Phase 43-02]: Remote exec port is 8091 — MEMORY.md says 8090 but game-launch.sh line 224 uses 8091; script evidence wins
- [Phase 43-02]: Launching state accepted as API-03 pass — Steam games take 30-90s to reach Running, matches game-launch.sh Gate 6
- [Phase 43-02]: forza_horizon_5 IS in GAMES_TO_TEST; forza (Motorsport) excluded (enabled:false in constants.ts)
- [Phase 43-02]: capture_error_screenshot fires only on launch failure to reduce noise; Steam dialog dismiss fires after every accepted launch
- [Phase 44-01]: fleet/health fetched once and reused for Gates 5/6/7 — avoids 3 redundant 10s HTTP calls per verify run
- [Phase 44-01]: build_id absent from fleet/health is skip() not fail() — older rc-agent versions predate the field; absence alone does not indicate deploy failure
- [Phase 44-01]: Gate 3 EADDRINUSE detection polls :3300 up to 30s with 5s intervals — Node.js startup can take 10-20s after deploy restart
- [Phase 44-01]: log_to_ai_debugger() format is [YYYY-MM-DD HH:MM:SS] GATE: name | FAILURE: msg — one line per failure, plain text, suitable for AI debugger parsing
- [Phase 44-deploy-verification-master-script]: run-all.sh uses PIPESTATUS[0] after tee pipe to capture phase exit codes; does not source lib/common.sh (orchestrator not test script); RESULTS_DIR exported for deploy/verify.sh AI log co-location
- [Phase 45]: bind_udp_reusable() mirrors the existing TCP pattern in remote_ops.rs — socket2 + SetHandleInformation approach
- [Phase 45]: MAX_CONCURRENT_EXECS 4->8: parallel deploys to 8 pods hit the old limit causing 429 errors during fleet operations
- [Phase 45]: Connection: close header set via axum middleware (not per-handler) to guarantee every response closes the connection
- [Phase 45]: pool_max_idle_per_host(0) on probe_client is belt-and-suspenders alongside server-side Connection: close — both ends actively close TCP after each probe
- [Phase 45]: THRESHOLD=5 matches self_monitor.rs internal threshold — consistent alerting boundary across runtime and test suite
- [Phase 46-01]: zero_force_with_retry uses thread::sleep not tokio::sleep — sync-safe for panic hook use
- [Phase 46-01]: Ok(false) from zero_force = device not found, permanent, not retried; Err = HID write error, retried
- [Phase 46-01]: StartupReport extended with 4 #[serde(default)] fields — backward compat without version negotiation
- [Phase 46-01]: rc-agent main.rs StartupReport uses false defaults for Phase 46 fields — Plan 02 wires real port-bind results
- [Phase 46-01]: Backward compat test JSON uses {"type":"startup_report","data":{...}} — adjacently-tagged serde format
- [Phase 46]: Panic hook uses try_lock not lock to update lock screen state — avoids deadlock if panic occurs while state mutex is held by another task
- [Phase 46]: remote_ops start_checked() started early so 30s retry window runs concurrently with FFB/HID init; await deferred to just before WS reconnect loop
- [Phase 47]: Wall-clock timing (date +%s%3N) for latency gates — simpler than powershell Stopwatch, matches close-wait.sh style
- [Phase 47]: Gate 2 runs even when Gate 1 fails — richer diagnostics on partial LLM deployments
- [Phase 47]: Layered timeouts: outer curl 15s, exec timeout_ms 10000, inner Ollama curl 8s — prevents hangs on unresponsive pods
- [Phase 47]: deploy-staging/Modelfile uses FROM qwen3:0.6b; Modelfile DIAGNOSTIC KEYWORDS must stay in sync with try_auto_fix() string checks; seed-debug-memory.sh uses python3 for JSON payload construction to avoid bash quoting pitfalls; patterns 8-14 in Modelfile are informational only, Phase 50 will wire auto-fix code
- [Phase 48]: BASELINE_PROCESSES const slice of 14 common system process names as UX guard — authoritative 70+ baseline lives in rc-agent, not DB
- [Phase 48]: INSERT OR IGNORE on kiosk_allowlist process_name UNIQUE; DELETE returns 204 idempotently; hardcoded_count: 70 is informational
- [Phase 48]: server_allowlist is 4th additive layer — never replaces hardcoded baseline (ALLOW-05)
- [Phase 48]: classify_process defaults to Ask on LLM failure — never auto-kills on uncertainty (ALLOW-04)
- [Phase 48]: allowlist_poll_loop fires first tick immediately at startup (ALLOW-03)
- [Phase 49-01]: auto_end_orphan_session_secs in AgentConfig (serde default 300s) — per-pod configurable without code rebuild
- [Phase 49-01]: SESSION-01 orphan check shares game_gone_since timer with BILL-02 — two-tier escalation from 60s alert to 300s auto-end
- [Phase 49-01]: SessionAutoEnded WS message sent regardless of HTTP retry outcome — server notified even if billing state is stale
- [Phase 49-01]: show_blank_screen() kept ONLY for disconnect cleanup — all post-session and post-crash paths use show_idle_pin_entry()
- [Phase 50-02]: hidden_cmd() used for all new fix functions — ensures CREATE_NO_WINDOW on pods
- [Phase 50-02]: fix_memory_pressure uses non-destructive working set trim — protected list enforced at PowerShell query level
- [Phase 50-02]: fix_dll_repair uses spawn() not output() — sfc /scannow takes 5-15 min, blocking would stall rc-agent
- [Phase 50-01]: startup self-test uses deterministic_verdict not LLM — Ollama call at startup too slow, blocks WS reconnect loop
- [Phase 50-01]: SelfTestResult.report is serde_json::Value — avoids rc-common depending on rc-agent self_test types
- [Phase 50-01]: 22 probes run via tokio::join! (parameterized TCP/UDP functions called per-port) rather than exactly 18 unique functions
- [Phase 50-01]: probe_udp_port_from_netstat_output extracted as pure testable function — no OS calls in tests
- [Phase 50]: pending_self_tests stores (pod_id, tx) tuple — pod_id needed for disconnect cleanup via retain pattern
- [Phase 50]: RunSelfTest result sent via ws_exec_result_tx channel (not ws_tx) — SplitSink is not Clone
- [Phase 50]: Fleet Health gate skipped when --skip-deploy passed — same condition as Phase 4 Deploy Verification
- [Phase 49]: CrashRecoveryState timer embedded in enum variant (not a separate armed bool) — eliminates split state, cleaner tokio::select! polling
- [Phase 49]: WS 30s grace window uses ws_disconnected_at: Option<Instant> with get_or_insert_with — billing+game continue during WiFi blips, Disconnected screen suppressed for first 30s

(v9.0 Tooling & Automation — key constraints from roadmap)
- Phase 51-53 have zero pod access requirements — can execute on James's workstation alone
- Phase 54-56 require Rust code changes deployed to server and pods — use pendrive or rc-agent :8090
- CLAUDE.md must be created in the repo root (not .planning/) so Claude Code auto-loads it on session start
- Custom skills use disable-model-invocation: true for SKILL-02 and SKILL-03 (per requirements) — deterministic deploy flows, not LLM-mediated
- racingpoint-google OAuth is the existing auth for MCP-01/02/03 — do not create new OAuth credentials
- rc-ops-mcp (MCP-04) runs on James's machine (:27), not on the server — avoids exposing server REST API externally
- tracing-appender crate already used in racecontrol; tracing-subscriber needs JSON feature flag for MON-01/02
- Error rate alerting uses the existing send_email.js shell-out pattern — no new SMTP crate (constraint in PROJECT.md)
- Netdata install on pods goes via rc-agent :8090 exec (not pendrive) — MEMORY.md confirms :8090 is deployed on all pods
- WhatsApp bot is racingpoint-whatsapp-bot (bono-bot org) — already integrated for v5.0 Phase 27 bono_relay.rs
- [Phase 51]: CLAUDE.md is the authoritative Racing Point context source; MEMORY.md holds identity + current state only, with explicit pointer to CLAUDE.md
- [Phase 51]: CLAUDE.md at 179 lines uses dense tables — well under 300-line context pressure limit
- [Phase 52-01]: createRequire CJS bridge pattern copied exactly from racingpoint-mcp-gmail — proven approach for ESM servers using CJS google libs
- [Phase 52-01]: GOOGLE_REFRESH_TOKEN set to PLACEHOLDER_REAUTH_NEEDED in settings.json — will be updated after OAuth re-auth in Task 3
- [Phase 52-01]: Same CLIENT_ID and CLIENT_SECRET as Gmail/Drive entries — all 4 Google MCP servers share one OAuth app
- [Phase 52-mcp-servers]: rc-ops-mcp runs on James's machine (.27) not on server — avoids exposing server REST API externally
- [Phase 52-mcp-servers]: Native fetch() only in rc-ops-mcp — Node 22 built-in, no axios/node-fetch dependency
- [Phase 52-mcp-servers]: Per-pod exec via /pods/{id}/exec (NOT /fleet/exec which does not exist) — 10 tools cover all priority racecontrol endpoints
- [Phase 53-02]: Use deploy_pod.py (NOT deploy-all-pods.py) — avoids hardcoded TARGET_SIZE that must be updated per build
- [Phase 53-02]: Sequential pod deploy (not parallel) — prevents RCAGENT_SELF_RESTART race conditions across pods
- [Phase 53-02]: Approval gate accepts y/yes/go/proceed only — any ambiguity cancels fleet rollout
- [Phase 53-01]: ONLOGON trigger (not ONSTART/SYSTEM) used for both tasks — matches CommsLink-Watchdog pattern; Python HTTP servers require user session
- [Phase 53-01]: Task Scheduler registration requires admin — used Start-Process RunAs elevation; schtasks /create with ONLOGON always needs UAC on non-admin sessions
- [Phase 57]: CLASS_FXM = 0x0A03, CMD_FXM_RESET = 0x01, CMD_IDLESPRING = 0x05 per upstream OpenFFBoard wiki — needs empirical validation on Conspit fork
- [Phase 57]: POWER_CAP_80_PERCENT pub const (not crate-private) so main.rs can reference at startup; Clone derive on FfbController since it only holds vid/pid
- [Phase 54-structured-logging-error-rate-alerting]: File layer uses .json() for structured JSONL; stdout layer stays plain text — no JSON on stdout per requirement
- [Phase 54-structured-logging-error-rate-alerting]: RollingFileAppender::builder() used over rolling::daily() to produce racecontrol-YYYY-MM-DD.jsonl naming with .jsonl extension
- [Phase 57]: safe_session_end() is async with spawn_blocking for sync HID — fits tokio select loop; CL restart is fire-and-forget; idlespring target=2000 (empirical starting value)
- [Phase 54-02]: Tracing init moved after config load in rc-agent so pod_id is available for info_span; pre-init messages use eprintln!
- [Phase 54-02]: rc-agent JSONL filename pattern: rc-agent-YYYY-MM-DD.jsonl (DAILY rotation, prefix=rc-agent-, suffix=jsonl)
- [Phase 54-02]: rc-agent stdout layer stays plain text; only file layer uses .json() for fleet-wide jq filtering
- [Phase 54-03]: Config loaded before tracing init in main.rs so MonitoringConfig thresholds are available at layer setup time
- [Phase 54-03]: ErrorCountLayer clears timestamps after firing alert to avoid re-triggering on next error in same burst
- [Phase 57-03]: Power cap placed after zero_force_with_retry() probe — wheelbase must be detected before set_gain can succeed
- [Phase 57-03]: Idlespring target=2000 confirmed acceptable on hardware — no tuning adjustment needed from Plan 02 default
- [Phase 58]: backup_conspit_configs() validates JSON before overwriting .bak -- prevents corrupt backup chain (Pitfall 2)
- [Phase 58]: Crash count increments only on watchdog path (is_crash_recovery=true), not session-end restarts
- [Phase 58]: Testable _impl(Option<&Path>) pattern for filesystem functions with hardcoded production paths
- [Phase 55]: msiexec timeout_ms=180000 critical — default 10s exec timeout kills Netdata install mid-run
- [Phase 55]: Netdata dashboard UI locked on free tier for Windows nodes — use /api/v1/info as canonical health check, not browser dashboard
- [Phase 55]: netdata.msi excluded from git (154MB binary artifact) — downloaded fresh each deploy from staging :9998

(v10.0 Connectivity & Redundancy — key constraints from roadmap)
- DHCP reservation is a hard prerequisite (Phase 66) — all subsequent phases depend on stable .23 IP
- Tailscale SSH does NOT work on Windows — use rc-agent :8090 over Tailscale IP for all server exec (GitHub #14942)
- rc-agent core.url is currently read once at startup — Phase 68 requires Arc<RwLock<String>> refactor before runtime URL switching is possible
- self_monitor.rs will fight intentional failover switches — Phase 68 adds last_switch_time guard to suppress relaunch during switch window
- Existing hysteresis pattern in cloud_sync.rs (3-down/2-up) should be reused in Phase 69 health probe loop
- Minimum 60s outage window before auto-failover (AC game launches cause 3-4s CPU spikes that transiently fail probes)
- Pod 8 canary testing required for SwitchController (Phase 68) before fleet deployment to all 8 pods
- Comms Link v2.0 (shipped 2026-03-20) is the coordination backbone — sync_push, task_request, exec_request protocols all available
- BACK-02 (session merge) is the highest-risk plan in the milestone — requires careful SQLite UUID reconciliation matching cloud_sync.rs pattern
- [Phase 66-infrastructure-foundations]: Bono ExecHandler imports james/exec-handler.js (symmetric reuse — same handler class, registry determines valid commands)
- [Phase 66-infrastructure-foundations]: activate_failover/deactivate_failover use pm2 app name 'racecontrol' — best guess, verify with 'pm2 list' on VPS before Phase 69
- [Phase 56]: broadcast::channel replaces mpsc for error rate alerts -- enables both email and WhatsApp alerters to subscribe independently
- [Phase 56]: P0State is internal to whatsapp_alerter (not shared with AppState) -- keeps alert state isolated
- [Phase 56]: 2-second debounce on PodOffline before counting online pods -- absorbs cascading disconnects

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- BIOS AMD-V disabled on Ryzen 7 5800X — v6.0 blocked; must enable SVM Mode before Phase 36
- Clean up .planning/update_roadmap_v9.py after v9.0 roadmap creation (can delete)

### Blockers/Concerns

- v6.0 (Phases 36–40) is blocked on BIOS AMD-V — does not affect v10.0 Connectivity & Redundancy
- Gmail OAuth tokens expired — MCP-01/02/03 (Phase 52) will need re-authorization of racingpoint-google OAuth before MCP server can connect
- Phase 66 (INFRA-01) requires physical access to router web UI at 192.168.31.1 to set DHCP reservation

## Session Continuity

Last session: 2026-03-20T10:48:48.170Z
Stopped at: Completed 56-01-PLAN.md (WhatsApp P0 alerter)
Resume file: None
Next action: Phase 66 — Infrastructure Foundations (DHCP reservation + Tailscale exec verification)
