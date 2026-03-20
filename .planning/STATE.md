---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: Roadmap ready, awaiting plan-phase
stopped_at: Completed 53-01-PLAN.md
last_updated: "2026-03-20T08:09:25.670Z"
last_activity: 2026-03-20 — v9.0 roadmap created, 6 phases (51-56), 19 requirements mapped
progress:
  total_phases: 30
  completed_phases: 13
  total_plans: 29
  completed_plans: 28
  percent: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-20)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** v9.0 Tooling & Automation — Phase 51: CLAUDE.md + Custom Skills

## Current Position

Phase: 51 — CLAUDE.md + Custom Skills
Plan: Not started
Status: Roadmap ready, awaiting plan-phase
Last activity: 2026-03-20 — v9.0 roadmap created, 6 phases (51-56), 19 requirements mapped

Progress: [░░░░░░░░░░] 0% (0/6 phases)

## Phase Map — v9.0

| Phase | Name | Requirements | Access | Status |
|-------|------|--------------|--------|--------|
| 51 | CLAUDE.md + Custom Skills | SKILL-01 through SKILL-05 | James workstation only | Not started |
| 52 | MCP Servers | MCP-01 through MCP-04 | James workstation only | Not started |
| 53 | Deployment Automation | DEPLOY-01 through DEPLOY-03 | James workstation only | Not started |
| 54 | Structured Logging + Error Rate Alerting | MON-01 through MON-03 | Server + pods (Rust changes) | Not started |
| 55 | Netdata Fleet Deploy | MON-04 through MON-05 | Server + pods via :8090 | Not started |
| 56 | WhatsApp Alerting + Weekly Report | MON-06 through MON-07 | Server (depends on Phase 54) | Not started |

**Phases 51-53:** James workstation only, zero pod access needed.
**Phases 54-56:** Require server/pod access, deployed via pendrive or rc-agent :8090.

## Phase Map — v10.0 Conspit Link

| Phase | Name | Requirements | Access | Status |
|-------|------|--------------|--------|--------|
| 57 | Session-End Safety | SAFE-01 through SAFE-07 | Pods (rc-agent Rust changes) | Not started |
| 58 | ConspitLink Process Hardening | PROC-01 through PROC-04 | Pods (rc-agent Rust changes) | Not started |
| 59 | Auto-Switch Configuration | PROF-01, PROF-02, PROF-04 | Pods (config files) | Not started |
| 60 | Pre-Launch Profile Loading | PROF-03, PROF-05 | Pods (rc-agent Rust changes) | Not started |
| 61 | FFB Preset Tuning | FFB-01 through FFB-06 | Pods (config + hands-on tuning) | Not started |
| 62 | Fleet Config Distribution | FLEET-01 through FLEET-06 | Server + pods (Rust changes) | Not started |
| 63 | Fleet Monitoring | CLMON-01 through CLMON-04 | Server + pods (Rust changes) | Not started |
| 64 | Telemetry Dashboards | TELE-01, TELE-02, TELE-06 | Pods (config files) | Not started |
| 65 | Shift Lights & RGB Lighting | TELE-03 through TELE-05 | Pods (config files) | Not started |

**Phases 57-58:** Safety-critical rc-agent changes, deploy to Pod 8 first.
**Phase 59:** Config file fix — potentially one-file-copy for Global.json path.
**Phases 60-61:** rc-agent preset loading + hands-on FFB tuning.
**Phases 62-63:** Fleet automation (server + pod Rust changes).
**Phases 64-65:** Config-only telemetry/LED setup, no code changes.

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

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat — needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- BIOS AMD-V disabled on Ryzen 7 5800X — v6.0 blocked; must enable SVM Mode before Phase 36
- Clean up .planning/update_roadmap_v9.py after v9.0 roadmap creation (can delete)

### Blockers/Concerns

- v6.0 (Phases 36–40) is blocked on BIOS AMD-V — does not affect v9.0 (no SaltStack dependency)
- Gmail OAuth tokens expired — MCP-01/02/03 (Phase 52) will need re-authorization of racingpoint-google OAuth before MCP server can connect

## Session Continuity

Last session: 2026-03-20T08:09:25.666Z
Stopped at: Completed 53-01-PLAN.md
Resume file: None
Next action: Phase 51 Plan 01 — CLAUDE.md with project context (pod IPs, crate names, naming rules, constraints, 4-tier debug order)
