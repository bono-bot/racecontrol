# Milestones

## v35.0 Structured Retraining & Model Lifecycle (Shipped: 2026-04-01)

**Phases completed:** 227 phases, 517 plans, 744 tasks

**Key accomplishments:**

- EscalatingBackoff pre-populated for all 8 pods in AppState, rc-agent fails fast with branded lock screen on missing or invalid config (no silent default fallback)
- One-liner:
- Fixed rc-agent.template.toml to use [pod]/[core] sections matching AgentConfig serde layout, closing DEPLOY-04 gap where deployed configs would fail TOML deserialization
- WatchdogState FSM enum, AppState watchdog fields, DashboardEvent watchdog variants, enriched format_alert_body with heartbeat/next-action context, and GET /health on rc-agent port 18923
- WatchdogState-aware restart lifecycle in pod_monitor: escalating backoff with double-restart prevention, partial-recovery-as-failure, is_closed() WS liveness, PodRestarting/PodVerifying/PodRecoveryFailed dashboard events, and email alerts on verification failure
- pod_healer reads WatchdogState to skip recovery pods, sets needs_restart flag for genuine rc-agent failures, and uses is_closed() for WS liveness — eliminating concurrent restart races
- One-liner:
- One-liner:
- One-liner:
- Files:
- deploy_pod() async executor: kill->wait-dead->download(5MB guard)->start->verify-health with email alerts, 409 guards, and real-time progress via DashboardEvent::DeployProgress
- 1. [Rule 1 - Adaptation] Pod ID format mismatch — plan used pod-N (dashes), codebase uses pod_N (underscores)
- LaunchSplash state with branded HTML + corrected session-end ordering so customers never see Windows desktop during game launch or session end
- Unified PIN error message across all 3 auth surfaces (pod/kiosk/PWA) via PinSource enum + INVALID_PIN_MESSAGE const, plus idempotent pod-lockdown.ps1 for taskbar/Win-key/WU suppression, verified anti-cheat safe on iRacing, F1 25, LMU
- Phase 5 deployment deferred to manual on-site execution — all code verified via automated tests (210 tests green), binary ready in deploy-staging/
- rc-agent log analysis + Edge registry baseline for all 8 pods reveals racecontrol unreachable as universal lock screen root cause and confirms all 8 pods need Phase 9 Edge policy remediation
- Server port audit + IP/MAC identification reveals server IP drifted to .4, both racecontrol and kiosk not running, and server has pod-agent (no RDP needed)
- racecontrol Axum reverse proxy for kiosk paths added to bypass Windows Smart App Control blocking node.exe; CORS updated for kiosk.rp; 21MB release binary staged — server deployment blocked pending physical access
- Port readiness probe + branded RACING POINT startup page eliminates ERR_CONNECTION_REFUSED race and replaces blank boot screen with branded waiting UI
- Minimal scheduled-task watchdog for rc-agent that detects crashes via tasklist+find and restarts via start-rcagent.bat, logging events to C:\RacingPoint\watchdog.log
- 6.7MB static-CRT rc-agent release binary with StartupConnecting port readiness + watchdog staged at deploy-staging/ ready for Pod 8 deployment
- Axum lockdown routes for per-pod and bulk kiosk toggle with billing guard + 10 new unit tests covering parse_mac and lockdown logic
- Kiosk /control page wired to lockdown + restart-all backend: 5 bulk buttons, per-pod padlock toggle, optimistic UI — TypeScript clean, Next.js build passing
- Server-side maintenance tracking via PreFlightFailed/Passed WS events with in_maintenance field on fleet health JSON and POST /pods/{id}/clear-maintenance endpoint sending ClearMaintenance to pod agent
- Racing Red Maintenance badge on fleet pod cards with PIN-gated detail modal showing failure checks and Clear Maintenance button calling POST /pods/{id}/clear-maintenance
- Three new process guard types (ViolationType, ProcessViolation, MachineWhitelist) and three new protocol variants (ProcessViolation, ProcessGuardStatus, UpdateProcessWhitelist) added to rc-common as the compile-time boundary for Phases 102-105
- ProcessGuardConfig/AllowedProcess/ProcessGuardOverride structs with serde Deserialize added to racecontrol/src/config.rs; C:/RacingPoint/racecontrol.toml populated with 185 global allowed entries and 3 per-machine override sections covering all 11 Racing Point machines
- GET /api/v1/guard/whitelist/{machine_id} endpoint with merge_for_machine() logic: global entries filtered by machine type, per-machine deny/allow_extra overrides applied, returns sorted MachineWhitelist JSON or 404 for unknown machine IDs
- ProcessGuardConfig TOML struct, walkdir dep, and AppState guard_whitelist + violation channel added to rc-agent — Plan 02 can now reference all three contracts.
- process_guard.rs with spawn_blocking sysinfo scan, two-cycle grace HashMap, PID-verified taskkill, CRITICAL racecontrol.exe detection, and 512KB-rotating audit log — 9 unit tests green
- Autostart audit (HKCU/HKLM Run + Startup folder) added to process_guard.rs; whitelist fetch, process_guard::spawn(), guard_violation_rx drain, and UpdateProcessWhitelist handler wired into rc-agent — 17 tests green, zero compile errors
- In-memory per-pod ViolationStore with FIFO eviction, fleet/health API extension, ProcessViolation WS handler, and email escalation for 3 kills of same process within 5 minutes.
- Server-side process guard scan loop using sysinfo 0.33 spawn_blocking pattern, CRITICAL detection of rc-agent.exe with zero grace, 512KB-rotating log at C:\RacingPoint\process-guard.log, violations fed into pod_violations["server"] ViolationStore.
- netstat -ano port audit (kill_process_verified + taskkill fallback) and schtasks CSV scheduled-task audit (\\Microsoft\\ skip, disable action) wired into rc-agent 5-minute audit cycle — 11 new TDD unit tests, 28 tests green
- One-liner:
- One-liner:
- build_id added to rc-agent root tracing span + all 65 main.rs tracing calls migrated to target: LOG_TARGET with legacy bracket prefixes removed
- 164 tracing call sites in ws_handler.rs (60), event_loop.rs (53), ac_launcher.rs (51) migrated to structured target: labels, taking rc-agent past the 50% migration mark
- 114 tracing calls migrated across 4 rc-agent modules to structured target labels; legacy [rc-bot] prefix eliminated from ai_debugger.rs; kiosk-llm sub-target preserved for independent log filtering
- 79 tracing call sites across 7 rc-agent files migrated to structured target: LOG_TARGET labels; legacy [rc-bot] and [billing-guard] bracket prefixes eliminated
- Audit checks run:
- Exhaustive rc-agent anti-cheat risk inventory with per-system severity classifications and source references, GPO decision made for Phase 108, Sectigo OV cert checklist ready for Uday
- Per-game anti-cheat compatibility matrix (17 subsystems x 6 games) plus ConspitLink audit template with ProcMon capture procedure
- SetWindowsHookEx global keyboard hook removed from default rc-agent build and replaced with GPO registry keys (NoWinKeys=1 + DisableTaskMgr=1 via reg.exe), with hook preserved behind keyboard-hook Cargo feature for emergency rollback
- SafeMode state machine with WMI process watcher, sysinfo startup scan, and full AppState integration for anti-cheat compatible protected game detection
- Safe mode wired into all 7 integration points: LaunchGame entry, WMI polling, 30s cooldown timer, process_guard scan skip, Ollama suppression, kiosk GPO deferred, lock_screen Focus Assist deferred
- Inline SVG Racing Point logo in all lock screens, wallpaper URL support via SettingsUpdated, session summary with top speed + race position stats, and persistent results screen (no 15s auto-reload)
- Staff-facing wallpaper URL input added to kiosk settings page — completes BRAND-02 end-to-end chain from dashboard to pod lock screen
- 5-second deferred SHM connect (HARD-03) and AC EVO feature flag off by default (HARD-05) to avoid triggering EAC/EOS/Javelin anti-cheat scans during game startup
- One-liner:
- rc-agent.exe built from HEAD (all v15.0 features: safe mode, GPO lockdown, telemetry gating) and deployed to Pod 8 via SSH+service-key chain — ws_connected=true, uptime>30s, build_id=243f03d
- go2rtc v1.9.13 installed on James (.27) relaying sub-streams from 3 Dahua cameras (entrance .8, reception .15, reception_wide .154) at :8554 with API at :1984 and auto-start via HKLM Run key
- rc-sentry-ai crate with retina RTSP frame extraction, per-camera reconnect loops, and shared FrameBuffer for 3 Dahua cameras via go2rtc relay
- Axum health endpoint at :8096 with per-camera connection status (connected/reconnecting/disconnected), go2rtc relay health probe, and overall service status
- People tracker migrated to read all 3 camera RTSP streams from go2rtc relay at localhost:8554, eliminating direct camera connections
- SCRFD-10GF ONNX face detector with CUDA EP and openh264 H.264 decoder -- core detection building blocks
- Live detection pipeline wiring: per-camera decode->preprocess->detect loop with config-gated SCRFD init and health endpoint stats
- DPDP Act 2023 compliance with mpsc audit log, 90-day retention purge, right-to-deletion API, and consent signage on :8096
- Quality gate filter chain rejecting faces by size (<80x80), blur (Laplacian var <100), and pose (yaw >45deg), plus CLAHE lighting normalization producing grayscale-as-RGB face crops
- ArcFace ONNX recognizer with CUDA EP producing L2-normalized 512-D embeddings from 112x112 similarity-transform-aligned face crops
- Cosine similarity gallery with SQLite persistence, face tracker cooldown, and full pipeline integration from SCRFD through ArcFace to identity logging
- Full person CRUD with phone migration, Gallery add/remove methods, and enrollment request/response DTOs for face enrollment API
- Axum REST API with 6 endpoints for person CRUD and photo upload with full SCRFD->ArcFace ML pipeline, duplicate detection, gallery sync, and audit logging
- Broadcast channel wiring from detection pipeline to attendance engine with 5-min cross-camera dedup and SQLite attendance_log table
- Staff clock-in/clock-out state machine with SQLite persistence and automatic shift tracking on recognition events
- Four Axum REST endpoints for attendance presence, history, and shift queries with IST timezone and completeness flags
- AlertEvent type system with tagged JSON serialization and WebSocket /ws/alerts endpoint broadcasting recognized-person events to dashboard clients via tokio broadcast fan-out
- Windows desktop toast notifications via winrt-toast for face detection alerts with IST timestamps and system sound
- Unknown face detection with rate-limited alerts and 112x112 JPEG crop saving to C:\RacingPoint\face-crops\
- MJPEG streaming endpoints serving live camera feeds via H.264-to-JPEG transcoding with per-connection decoders and CORS for dashboard access
- Live MJPEG camera feeds page at /cameras with grid layout, status indicators, and sidebar navigation
- Dahua NVR CGI API client with HTTP Digest auth, mediaFileFind search, and RPC_Loadfile streaming
- Axum playback proxy with NVR search, streaming video passthrough, and attendance event timeline
- Next.js playback page with camera/date/time search form, recording file list, HTML5 video player streaming via rc-sentry-ai proxy, and attendance event timeline with click-to-seek markers
- WAL tuning, covering indexes for leaderboard/telemetry, cloud_driver_id column, and 6 competitive tables (hotlap_events, championships, standings, ratings) with TDD
- car_class column on laps table, auto-populated from billing_sessions -> kiosk_experiences JOIN in persist_lap(), with TDD
- Suspect column on laps table with sector-sum and sanity-time checks computed in persist_lap() before INSERT
- sim_type filtering on track leaderboard (defaults assetto_corsa), /public/circuit-records and /public/vehicle-records/{car} endpoints with suspect-always-hidden policy
- Fire-and-forget email notification to previous record holder when their track record is beaten, with fetch-before-UPSERT data ordering and nickname-aware new holder display
- Public driver search (case-insensitive name/nickname, max 20) and profile endpoints (stats, personal bests, lap history) with zero PII exposure and sector-zero-to-null mapping
- Public leaderboard with sim_type/invalid filter, circuit records with car filter, driver search with debounced input, and driver profile with stats/personal bests/lap history -- all mobile-first at 375px
- rc-agent load_config() now searches the executable's directory first, making it self-healing when launched by DeskIn or SYSTEM services with a different CWD
- pod-agent v0.5.1: exec default timeout reduced 30s->10s, slot exhaustion eprintln! warning, 7 tests all passing, binary staged at deploy-staging/
- PodAgent HKLM Run key confirmed on 5/8 pods, FleetHealth scheduled task created, Pod 8 pod-agent upgrade partially completed (needs reboot to recover)
- One-liner:
- ExecHandler wired with dynamic-first/static-fallback lookup, completedExecs LRU-capped at 10000, HTTP /relay/registry/register endpoint and WS registry_register handler deployed on both James and Bono with JSON persistence.
- ShellRelayHandler class with binary allowlist, hardcoded APPROVE tier, and approval queue with default-deny timeout -- wired into both James and Bono WS routing and HTTP endpoint.
- ExecResultBroker (pending-promise utility) and ChainOrchestrator (sequential exec chains with stdout piping, abort-on-failure, continue_on_error, and chain-level timeout) -- fully TDD'd with 16 tests green.
- ExecResultBroker and ChainOrchestrator wired into both james/index.js and bono/index.js -- exec_result routes through shared broker, chain_request triggers ChainOrchestrator.execute on both sides, FailoverOrchestrator simplified to delegate to broker.
- AuditLogger class (appendFileSync JSONL) and delegate_request/delegate_result protocol types -- foundation for wiring audit trail into both james and bono daemons in Plan 02.
- Bidirectional Claude-to-Claude delegation (delegate_request/delegate_result) wired into both james and bono daemons with per-step AuditLogger calls on all exec paths (exec, shell_relay, chain, delegate tiers).
- ChainOrchestrator extended with named template resolution (chains.json + templatesFn injection), {{prev_stdout}} arg substitution with shell metacharacter sanitization, and per-step retry with linear backoff and new execId per attempt
- ChainOrchestrator gains pause/resume capability for WS disconnect resilience (CHAIN-09) and either AI can query the other's command registry over WS with safe field filtering (DREG-06)
- One-liner:
- CommsLink-DaemonWatchdog Task Scheduler task registered (every 2 min), HKCU Run key verified correct, watchdog confirmed healthy via manual trigger.
- Missing chain_result WS handler added to james/index.js — /relay/chain/run now resolves synchronously via ExecResultBroker instead of 504-timing-out
- Relay health endpoint now exposes connectionMode + lastHeartbeat, /relay/exec/run fails fast with 503 when not REALTIME, and SKILL.md guides callers to probe health before sending
- close_browser() safe mode gate (BWDOG-04):
- 30-second browser watchdog in rc-agent event loop that auto-recovers from Edge crashes and process stacking (>5 msedge.exe), gated by safe mode and Hidden state
- AgentMessage::IdleHealthFailed added to rc-common with pod_id, failures, consecutive_count, and timestamp — serde round-trip test passes, idle_health_failed JSON tag confirmed
- 60s idle health loop in rc-agent event_loop.rs: probes lock_screen_http + window_rect, self-heals via close+relaunch, sends IdleHealthFailed after 3 consecutive failures, skips during billing and safe mode
- IdleHealthFailed WS handler added to racecontrol: logs warn + activity_log + updates FleetHealthStore, exposed in GET /api/v1/fleet/health via idle_health_fail_count and idle_health_failures per pod
- ForceRelaunchBrowser WS protocol variant added to rc-common and wired into pod_healer Rule 2 for soft Edge lock screen recovery over live WebSocket before escalating to restart
- rc-agent ForceRelaunchBrowser handler: billing-gated close_browser + launch_browser on server-initiated WS message, completing the server-to-pod lock screen recovery round-trip
- 19 failing RED test stubs covering auto-entry, 107% rule, badges, F1 scoring, gap-to-leader, and championship tiebreaker — with 3 schema migrations closing the p2/p3 tiebreaker gap
- 9 staff CRUD endpoints for hotlap events and championships in routes.rs, using check_terminal_auth() and COALESCE-based partial updates
- auto_enter_event() + recalculate_event_positions() in lap_tracker.rs — valid laps automatically enter matching hotlap events with gold/silver/bronze badges and 107% flagging computed at write time
- F1 2010 point scoring from multiplayer_results into hotlap_event_entries, with live-computed championship standings using wins/P2/P3 tiebreaker — 6 new tests GREEN, 329 total passing
- 5 public GET endpoints for events listing, event leaderboard with badges/107%/gap, group session F1 results, championships listing, and championship standings with per-round breakdown — 331 tests GREEN
- One-liner:
- execute_ai_action() wired in event_loop.rs with safe mode AtomicBool gate; pod_healer.rs logs AI-recommended actions to activity_log via local whitelist parser
- Rolling-window WARN log scanner integrated into heal_all_pods() with 5-min count window, 50-entry threshold, and 10-min cooldown gating via AppState.warn_scanner_last_escalated
- WARN surge deduplication and AI escalation via escalate_warn_surge() — groups identical WARN messages by frequency, builds compact context, calls query_ai(source="warn_scanner"), persists to ai_suggestions with pod_id="server"
- One-liner:
- Live integration test scaffold for INTEG-01 (exec round-trip via /relay/exec/run) and INTEG-03 (message relay via /relay/task) with graceful PSK-absent skip using node:http and node:test
- Chain round-trip test (INTEG-02), cross-platform syntax checker (INTEG-04), and 15 pure-static contract tests (INTEG-05) added to complete the comms-link integration suite
- Bash test gate `test/run-all.sh` that runs contract tests + integration tests + syntax check in sequence, prints a colour-coded per-suite PASS/FAIL/SKIPPED summary block, and exits 0 only when all gated suites pass — satisfying GATE-01 single-command invocation
- Pre-Ship Gate section wired into comms-link/CLAUDE.md with exit-code contract blocking phase completion on non-zero exit, and quality gate bullet added to rp-bono-exec SKILL.md — GATE-02 and GATE-03 satisfied
- go2rtc configured with 13 NVR sub-stream channels (ch1-ch13), CORS enabled, and WebRTC + snapshot coexistence verified on live Dahua NVR hardware
- Task 1 — config.rs:
- Task 1 — mjpeg.rs:
- Single-file NVR dashboard with 4-mode CSS grid layout switching, dynamic camera loading from /api/v1/cameras, green/yellow/red status dots, and snapshot polling with configurable refresh rate
- HTML5 drag-and-drop camera reordering with auto-save on drop, zone-grouped collapsible headers (ENTRANCE/RECEPTION/PODS/OTHER), and full camera_order persistence via PUT /api/v1/cameras/layout
- RTCPeerConnection via go2rtc WebSocket signaling (ws://192.168.31.27:1984/api/ws) with singleton pattern, 500ms hover pre-warm, auto-hiding controls, and snapshot fallback on failure
- 1. [Rule 1 - Bug] Removed erroneous Promise wrapper in resetControlsTimer
- Schema (db/mod.rs):
- Next.js /cafe admin page with item table + slide-in side panel delivering full CRUD (add, edit, delete, toggle) and inline category creation, wired to the Plan 01 Rust backend
- 3 new Axum handlers:
- Import modal with XLSX/CSV preview-and-confirm flow, per-item image upload column, and 4 new TypeScript API types on the /cafe admin page
- CafeMenuPanel with category tabs and paise-to-rupee formatting integrated into POS control page via SidePanel toggle
- Next.js PWA /cafe page with 2-column card grid grouped by category, image fallback, paise-to-rupees price formatting, and Cafe tab replacing Stats in BottomNav
- SQLite inventory columns (is_countable, stock_quantity, low_stock_threshold) added to cafe_items with idempotent migrations, CafeItem struct updated, and POST /cafe/items/{id}/restock endpoint implemented
- Inventory management UI added to /cafe admin — Items/Inventory tabs, inline restock flow, color-coded threshold badges (red/yellow/green/gray) sorted low-stock first
- 1. [Rule 1 - Bug] Config::default() does not exist
- Red low-stock warning banner in CafePage with 60s polling via api.listLowStockItems(), listing each breached item's name, stock quantity, and threshold
- Task 1 — api.ts changes:
- Staff POS order builder in CafeMenuPanel: item add/qty controls with out-of-stock overlays, live customer name search, and wallet-debiting checkout via POST /api/v1/cafe/orders
- One-liner:
- SQLite cafe_promos table + five Axum admin CRUD endpoints for combo/happy_hour/gaming_bundle promos with stacking group support
- One-liner:
- PromoBanner + discount receipt display added to PWA cafe page and kiosk CafeMenuPanel, fetching active promos from /cafe/promos/active on mount with non-fatal error handling
- satori/resvg-wasm PNG generation API (Next.js) + Evolution API WhatsApp broadcast with 24h per-driver cooldown (Rust/Axum)
- Marketing tab on /cafe admin page: promo PNG generation (blob download), daily menu PNG, WhatsApp broadcast form with result summary
- Shared recovery contracts in rc-common: RecoveryAuthority ownership registry, RecoveryDecision JSONL logger, and single-owner process enforcement for rc-sentry/pod_healer/james_monitor
- One-liner:
- Rust firewall module using netsh advfirewall with delete-then-add idempotency — ICMP echo and TCP 8090 rules applied on every rc-agent startup, eliminating CRLF-damaged batch file failures permanently
- RCAGENT_SELF_RESTART sentinel detection wired into handle_crash() plus RecoveryLogger logging every recovery decision to C:\RacingPoint\recovery-log.jsonl via build_restart_decision() pure helper
- Pattern hit-count escalation skips restart after 3 same-pattern crashes and pre-restart Ollama query with 8s timeout consults AI before handle_crash for unknown patterns
- PodRecoveryTracker with 4-step graduated offline recovery — wait 30s, Tier 1 restart, AI escalation, staff alert — gated on in_maintenance and billing_active checks
- One-liner:
- failure_state.rs
- rc-watchdog.exe deployed to deploy-staging and registered as CommsLink-DaemonWatchdog Task Scheduler task on James (.27), replacing james_watchdog.ps1 with Rust-based monitoring running every 2 minutes
- CoreToAgentMessage::Exec and AgentMessage::ExecResult variants with serde roundtrip tests proving snake_case wire format and default timeout
- Semaphore-gated WebSocket command execution handler with independent 4-slot concurrency, 64KB output truncation, and spawned-task mpsc drain pattern in the agent event loop
- Core-side ExecResult handler with oneshot channel resolution, ws_exec_on_pod() public function, and deploy.rs HTTP-first WS-fallback for all pod commands
- Three GitHub repos archived (game-launcher, ac-launcher, conspit-link) with README notices; 7 non-git folders catalogued with archive/delete/keep decisions
- Full npm + cargo security audit across all 15 repos — 7 npm highs fixed, rustls-webpki patched in both Rust repos, 8 deferred items documented with rationale
- One-liner:
- Commit:
- Commit:
- Single 682-line authoritative document cataloguing all 333 HTTP endpoints across 4 boundary directions (racecontrol, rc-agent, comms-link, admin) with typed request/response shapes and 8 shared data structure tables
- One-liner:
- One-liner:
- Next.js App Router GET /api/health endpoints added to kiosk (:3300) and web dashboard (:3200), returning { status: ok, service, version: 0.1.0 } with zero TypeScript errors
- comms-link relay /health added returning `{ status: 'ok', service: 'comms-link', version, connected, clients }`, with racecontrol and rc-sentry confirmed already compliant
- deploy-staging triaged from 719 dirty files to zero untracked: .gitignore expanded with 15+ patterns covering JSON relay payloads, build artifacts, logs, and screenshots; 146 operational scripts committed
- check-health.sh polls 5 services (racecontrol/kiosk/web/comms-link/rc-sentry) with PASS/FAIL output; deploy.sh orchestrates racecontrol/kiosk/web/comms-link deploys and gates each on health check
- 1. [Rule 2 - Missing coverage] Added rc-sentry service to runbook
- One-liner:
- One-liner:
- crates/rc-common/src/types.rs
- Optional Cargo features added to rc-agent (ai-debugger, process-guard, http-client) and rc-sentry (watchdog, tier1-fixes, ai-diagnosis) — both crates compile with default features (full production) and --no-default-features (minimal/bare builds)
- One-liner:
- SQLite-backed feature flag registry with REST CRUD (GET/POST/PUT /flags), in-memory RwLock cache, real-time FlagSync WS broadcast to pods, and config_audit_log audit trail sourced from StaffClaims.sub
- Config push REST+WS pipeline: whitelist validation with 400 field-level errors, per-pod SQLite queue with monotonic seq_num, WebSocket ConfigPush delivery, offline replay on reconnect (status-based filter), and deterministic ConfigAck audit lookup by seq_num
- One-liner:
- In-memory feature flag system for rc-agent with disk cache, WS-driven sync via FlagSync/KillSwitch, and FlagCacheSync on reconnect using Arc<RwLock<FeatureFlags>>
- feature_flags.rs:
- Status:
- AgentMessage::StartupReport with serde roundtrip tests, sent once per process lifetime from rc-agent to racecontrol with version, uptime, config hash, crash recovery flag, and self-heal repairs
- 76 standing rules classified into AUTO/HUMAN-CONFIRM/INFORMATIONAL registry with 6 new OTA Pipeline rules added to CLAUDE.md
- gate-check.sh with 5 pre-deploy suites (comms-link E2E, cargo tests, 18 AUTO standing rules, diff analysis, HUMAN-CONFIRM checklists) and 4 post-wave suites (comms-link E2E, build ID verification, fleet health, AUTO standing rules)
- PipelineState::Paused + gate-check.sh integration wired into OTA pipeline with no force/skip mechanism, Bono synced via INBOX.md + WS
- Graduated 4-tier crash handler with 500ms spawn verification, server-reachable exclusion from MAINTENANCE_MODE, and recovery event HTTP reporting to racecontrol server
- Tier 3 Ollama diagnosis and Tier 4 WhatsApp escalation wired into graduated crash handler — completing the 4-tier recovery pipeline with spawn-failure-triggered AI diagnosis and 5-min-cooldown staff alerts
- 1. [Rule 1 - Bug] Fixed pre-existing E0433: chrono unresolved in tier1_fixes.rs
- One-liner:
- One-liner:
- One-liner:
- rc-agent self_monitor checks TCP :8091 before relaunch — yields to rc-sentry (zero PowerShell) when sentry alive, falls back to PowerShell only when sentry is dead
- Shared ollama module in rc-common (pure TcpStream), spawn verification (500ms/10s) in james_monitor and rc-watchdog, and 30s sentry-breadcrumb grace window to prevent double-restart coordination failures
- One-liner:
- One-liner:
- One-liner:
- rc-watchdog Windows SYSTEM service with SCM lifecycle, tasklist process polling, Session 1 spawn via WTSQueryUserToken + CreateProcessAsUser, and fire-and-forget HTTP crash reporting
- racecontrol POST /api/v1/pods/{pod_id}/watchdog-crash endpoint with WARN-level structured logging, activity recording, and install-watchdog.bat for fleet SCM registration with failure restart actions
- Tier 2:
- 10 audit phase scripts (tiers 7-9) plus full load_phases()/dispatch rewrite wiring all 44 phases across 5 modes and 9 tiers
- File-based semaphore parallel engine (audit/lib/parallel.sh) with 4-slot mkdir locking, 200ms pod stagger, and audit.sh updated to source it and dispatch all 60 phases across tiers 1-18 in full mode.
- One-liner:
- 7 bash phase scripts completing the full 60-phase v3.0 audit port — registry/relay integrity, DB migration completeness, LOGBOOK/OpenAPI freshness, E2E test suites, cloud path, customer flow, and cross-system chain checks
- `audit/lib/results.sh`
- jq-based delta engine joining phase+host composite key across consecutive audit runs, with venue-aware PASS/QUIET/FAIL categorization that prevents false regressions when venue closes
- `check_suppression(phase host message)`
- Bash auto-fix engine with billing-gate, OTA sentinel check, whitelist enforcement, and 3 safe fixes (sentinel clear, orphan PS kill, rc-agent restart) all logged via emit_fix()
- Failure-safe three-channel notification engine (Bono WS + INBOX.md + WhatsApp Uday) gated behind --notify flag with delta summary inclusion
- Changes to audit/audit.sh:
- One-liner:
- SQLite launch_events table + JSONL dual-write infrastructure with LaunchEvent/LaunchOutcome/ErrorTaxonomy types wired into all 5 game_launcher call sites
- SQLite billing_accuracy_events and recovery_events tables with recording functions wired into billing.rs and game_launcher.rs, producing real rows on every billing start and every Race Engineer crash recovery.
- GameLauncherImpl trait with 4 per-game impls, fixed billing gate (deferred billing + paused rejection), TOCTOU mitigation, and invalid JSON rejection — all backed by 19 passing unit tests
- Stopping state blocked at double-launch and relaunch, all 6 broadcast failures now logged at warn, externally_tracked field added, 30s Stopping timeout spawned in stop_game(), and feature flag gate before launch — 29 unit tests all passing
- Server-side launch resilience: dynamic timeout from launch history, typed exit_code error taxonomy, atomic Race Engineer with WhatsApp staff alerts after 2 failed retries.
- Agent-side launch hardening: pre-launch sentinel+orphan+disk checks, AC polling waits replacing hardcoded sleeps, CM 30s timeout with progress logging, fresh PID via find_acs_pid() on fallback, and split_whitespace fix for paths with spaces.
- AC False-Live guard (5s speed+steer gate), CancelledNoPlayable status variant, and BillingConfig (5 configurable timeouts) -- billing foundation for Plan 02 server-side logic
- WaitingForGame tick broadcasts for kiosk Loading state, cancelled_no_playable DB records on timeout/crash, paused seconds persistence for game-pause, single-timestamp billing accuracy, and multiplayer error rejection — billing.rs fully wired to BillingConfig
- 4 new billing tests for BILL-05/06/10/12 — 82 total billing tests, 0 failures, Phase 198 complete with all 12 BILL requirements implemented and verified
- Server-side crash recovery hardening: force_clean protocol flag, history-informed recovery action selection via query_best_recovery_action(), enriched recovery events with actual ErrorTaxonomy/car/track/exit_codes, and structured staff WhatsApp alert
- Safe mode cooldown suppression during PausedWaitingRelaunch, exit grace guard verification on all paths, and unit tests for recovery contracts
- crates/rc-common/src/types.rs:
- crates/rc-agent/src/self_heal.rs:
- RED phase:
- RED phase:
- One-liner:
- Kiosk updated to 10-variant BillingSessionStatus, local type removed, game loading/crash-recovery/disconnect UI added, reliability warning with alternatives modal on review step.
- One-liner:
- Audit Phase 02 validates ws_connect_timeout >= 600ms and app_health URL ports; Phase 21 billing checks emit WARN (not PASS) when unreachable during venue hours; Phase 53 detects watchdog dead (ps_count=0)
- Live Evolution API connection check, OAuth token expiry verification, real display resolution queries, and start-rcsentry-ai.bat added to repo
- Upgraded 4 audit phase scripts from count/existence checks to content health verification -- svchost.exe allowlist spot-check, menu availability, flag enabled-state, and OpenAPI endpoint name verification
- VerifyStep trait, ColdVerificationChain, HotVerificationChain, VerificationError (4 typed variants), and spawn_periodic_refetch() added to rc-common with 13 tests passing and all downstream crates (rc-agent, racecontrol, rc-sentry) still compiling
- Silent config fallback eliminated: rc-agent 5 unwrap_or sites + racecontrol load_or_default() + process guard empty allowlist + all 4 FSM transitions now emit observable signals at the moment each degraded state occurs
- Sentinel file changes are now instantly observable: every create/delete in C:\RacingPoint\ produces a WS message to racecontrol within 1 second, updates active_sentinels in fleet health API, broadcasts DashboardEvent::SentinelChanged, and MAINTENANCE_MODE creation fires WhatsApp alert to Uday with 5-min rate limiting
- Feature flags self-heal via HTTP GET /api/v1/flags every 5 minutes using spawn_periodic_refetch, with CLAUDE.md standing rule banning single-fetch-at-boot patterns
- Process guard first-scan threshold validation with >50% violation detection and GUARD_CONFIRMED operator confirmation gate before kill_and_report escalation
- ColdVerificationChain wrapping pod healer curl parse (4-step) and config TOML load chains (3-step racecontrol, 2-step rc-agent) with first-3-lines SSH banner diagnostics
- ColdVerificationChain wrapping allowlist enforcement (COV-04) and spawn verification (COV-05) with structured tracing for empty-allowlist and spawn-but-dead diagnostics
- StepValidateCriticalFields emits TransformError on default fallback (COV-03) and spawn verification retries once on PID liveness failure (COV-05)
- Suite 5 domain-matched verification added to gate-check.sh -- detects display/network/parse/billing/config changes via git diff and enforces domain-specific verification gates
- Interactive bash helper (fix_log.sh) enforcing 5-step structured debugging with LOGBOOK.md template showing real pod healer flicker example
- Fleet exec probe (curl /api/v1/fleet/health) and WS handshake test (curl Upgrade headers) added to network domain gate in both pre-deploy and domain-check modes
- One-liner:
- Mobile-first /kiosk/fleet page showing 8 pod cards with WS/HTTP status dots, version, uptime, and 5-second polling via api.fleetHealth()
- Bat file drift detection + syntax validation scanner for 8-pod fleet via rc-sentry /files endpoint with 5 anti-pattern checks
- 5 audit phases (bat-drift, config-fallback, boot-resilience, sentinel-alerts, verification-chains) with deploy-pipeline bat sync and Debug Quality report section
- Five safety gates added to auto-detect.sh: PID run guard (SCHED-03), 6-hour per-pod+issue escalation cooldown (SCHED-04), venue-state-aware mode override (SCHED-05), and extended MAINTENANCE_MODE sentinel check
- Windows Task Scheduler bat for AutoDetect-Daily at 02:30 IST with safety gate verification, plus Bono VPS cron corrected to 02:35 IST
- One-liner:
- Kiosk /shutdown page with staff PIN gate, 6-state machine (idle/confirming/auditing/audit_passed/shutting_down/complete), audit-blocked reason display, and navigation from /staff
- Cascade detection framework (cascade.sh) with _emit_finding helper wired into auto-detect.sh step 4, plus 3 detector scripts: rc-agent.toml config drift (DET-01), bat file checksum drift (DET-02), and venue-aware ERROR/PANIC log anomaly detection (DET-03)
- 3 remaining detection modules (crash loop DET-04, flag desync DET-05, schema gap DET-06) added to scripts/detectors/ completing the full 6-detector cascade pipeline; all scripts pass bash -n syntax and auto-detect.sh --dry-run exits 0
- 5-tier graduated escalation engine (retry → restart → WoL → cloud failover → WhatsApp) with 3 new APPROVED_FIXES, sentinel-aware billing-gated execution, and runtime JSON toggle for auto_fix_enabled/wol_enabled
- Live-sync healing wired end-to-end: all 6 detectors call attempt_heal() immediately after _emit_finding(), cascade.sh sources escalation-engine.sh, and auto-detect.sh routes WhatsApp escalation through escalate_human() with HEAL-04 silence conditions
- `scripts/coordination/coord-state.sh`
- bono-auto-detect.sh extended with Tailscale-confirmed offline detection (COORD-02) and full recovery handoff protocol including findings JSON, INBOX.md push, and pm2 failover deactivation (COORD-03)
- Pattern tracking (LEARN-01) + trend outlier detection (LEARN-04) wired into auto-detect.sh — every run now permanently records what was found, what was fixed, and flags pods with 4x+ fleet-average bug frequency
- Suggestion engine that converts raw suggestions.jsonl pattern data into categorized JSON proposal files (6 categories, confidence scoring, deduplication) with relay exec inbox query via get_suggestions command
- One-liner:
- Self-modifying intelligence loop with CE methodology: threshold-only patches, realpath scope safety, bash -n verification, auto-revert on failure, independent self_patch_enabled=false toggle
- Task 1: Fixture files for all 6 detectors (10 files)
- Escalation 5-tier ladder test (TEST-03) + coordination mutex test (TEST-04) with unified --all entry point in test-auto-detect.sh
- RCAGENT_SELF_RESTART sentinel added to rc-agent exec handler — pods now restart via direct Rust call to relaunch_self(), completely bypassing cmd.exe, start-rcagent.bat, and PowerShell interpretation issues that caused pods 6/7/8 to go offline
- deploy_pod.py upgraded with server-exec fallback + EncodedCommand writes + rename-copy swap; Pod 2 deployed with new binary, pods 1/3-8 blocked by offline rc-agent
- One-liner:
- Tier 1 (Deterministic) — fully implemented:
- PodFailureReason enum (18 variants, 9 classes) and 5 typed AgentMessage bot failure variants established as the shared protocol foundation for all Phase 24-26 bot detection code
- Pure concurrency guard predicate is_pod_in_recovery(&WatchdogState) -> bool added to pod_healer.rs, blocking Phase 24 bot tasks from acting on pods in active watchdog recovery cycles
- Plan 01 + 02 combined (knowledge_base.rs — 373 lines):
- PodStateSnapshot gains Default derive + 3 telemetry fields; 10 RED test stubs written for 5 bot fix requirements (CRASH-01/02/03, UI-01, USB-01) — Wave 0 Nyquist compliance complete
- 3 new auto-fix functions (fix_frozen_game, fix_launch_timeout, fix_usb_reconnect) + extended fix_kill_error_dialogs turn all 10 Wave 0 RED tests GREEN in ai_debugger.rs
- failure_monitor.rs with CRASH-01/CRASH-02/USB-01 detection state machine, 8 tests green, all try_auto_fix calls wrapped in spawn_blocking
- failure_monitor spawned as live task in rc-agent with 13 state update sites keeping all 6 FailureMonitorState dimensions current from the event loop
- Task 1 — FailureMonitorState.driving_state compile gate:
- billing_guard.rs (~150 lines):
- Server-side bot message router with recovery guard, stuck-session auto-end, idle-drift staff alert, and hardware/telemetry stubs — 5 new tests, 299 total passing
- End-to-end wiring: billing_guard spawned from rc-agent main.rs, ws/mod.rs bot stubs replaced with bot_coordinator async calls, BILL-04 relay sync fence added to recover_stuck_session
- SQLite WAL fail-fast verification + staggered 60s timer persistence with COALESCE crash recovery for 8-pod concurrent writes
- One-liner:
- Atomic billing start via single sqlx transaction (wallet debit + session INSERT) with idempotency keys on all four money-moving endpoints (billing start, topup, stop, refund)
- end_billing_session UPDATE:
- 30-minute background reconciliation job using SQL correlated subquery to detect wallet balance vs transaction-sum drift, with ERROR logging, WhatsApp alerting, and admin GET/POST endpoints
- Server-side billing FSM with 20-rule TRANSITION_TABLE, validate_transition() gates all 9 status mutation sites in billing.rs, and authoritative_end_session() provides single CAS-protected end path
- FSM-02 — Phantom Billing Guard
- Split session parent+child entitlement model with CAS guards, FSM-08 DB-before-launch guard preventing orphaned game launches when no billing record exists
- Server-side INI injection prevention via character allowlist, FFB GAIN safety cap at 100, and three-tier RBAC (cashier/manager/superadmin) enforced on Axum route groups via JWT role claims
- Argon2id OTP hashing replacing SipHash DefaultHasher, SQLite BEFORE DELETE trigger making audit_log append-only, and role-gated PII masking (phone/email) for cashier staff in driver API responses
- Self-topup block via JWT sub comparison, WSS TLS with native-tls connector and custom CA support, game launch race condition eliminated via tokio Mutex in AppState
- 18% inclusive GST split in 3-line journal entries, per-session GST invoices with GSTIN/SAC/CGST/SGST, and Consumer Protection Act pricing disclosure in the kiosk display endpoint
- Waiver gate in start_billing (Indian Contract Act 1872), guardian OTP consent flow for minors, and argon2-hashed guardian OTP send/verify via WhatsApp Evolution API
- DPDP Act 2023 compliance: 8-year financial record retention config, daily PII anonymization job for inactive drivers, and immediate consent revocation endpoints for customers and guardian-proxy requests.
- Steam pre-launch gate (readiness + DLC) and window detection via sysinfo polling, with corrected fleet-monitoring process names for F1, iRacing, LMU, and Forza
- SessionEnforcer
- PWA game requests auto-expire after 10 min via server-side TTL, extensions enforce current tier rate, billing timer provably starts at game-live signal, and crash recovery pause time is excluded from billable seconds via PauseReason enum
- One-liner:
- Multiplayer billing synchronized across all group pods on AC crash (BILL-07), customer charge dispute portal with staff approve/deny workflow and atomic refund via existing FATM paths (BILL-08)
- Four-endpoint staff financial controls: discount approval gate with manager PIN validation above Rs.50 threshold, daily override audit report (discounts/refunds/tier changes), and cash drawer reconciliation with discrepancy logging
- Shift handoff API with active-session acknowledgment gate, DEPLOY-01 session drain verified across 3 billing hook points, DEPLOY-03 weekend 18:00-23:00 IST deploy lock with force override across all deploy entry points
- Graceful agent shutdown with billing session persistence, post-restart interrupted session recovery, and WS command_id deduplication preventing stale replay on reconnect
- Atomic wallet-debit-plus-time-addition for session extensions via single SQLite transaction, plus server-side discount stacking floor enforced in start_billing and apply_billing_discount
- Schema migration (db/mod.rs):
- One-liner:
- LapData gains required session_type field; catalog adds per-track minimum lap time floors; persist_lap sets review_required=1 for below-floor laps with idempotent DB migration
- Two structurally separate RwLock<HashMap> counters in AppState enforce that customer PIN lockout can never block staff debug PIN access — customer exhausts 5 attempts, staff still unlocks freely
- TELEM-01 and MULTI-01 fully operational: staff email on 60s UDP silence (game Running + billing active), ordered pod teardown (BlankScreen + end billing + group cascade via group_session_members + log) on AC server disconnect
- Durable notification outbox with WhatsApp-to-screen OTP fallback chain, exponential backoff retry, and negative wallet balance RESIL-05 guard on both extension debit and session start
- Lap assist evidence (SHA-256 hash + pro/semi-pro/amateur tier), billing-session gate blocking manual entry, and assist_tier segmentation across all three leaderboard endpoints
- Customer session receipt with GST breakup and before/after balance, plus virtual walk-in queue with live position ETA and staff call/seat workflow
- 1. [Rule 3 - Blocking] shadcn CLI used base-nova style instead of new-york
- motion@12 animation library installed in both apps, JetBrains Mono font wired to web layout via next/font/google with --font-jb-mono CSS variable, both apps build clean
- Four leaf-node UI primitives: StatusBadge with racing flag colors, MetricCard KPI tile, context-based Toast notifications, and Skeleton/EmptyState loading states
- Commit:
- LiveDataTable<T>
- Rewrote `web/src/app/login/page.tsx`
- 1. [Rule 3 - Blocking] Added fleetHealth API method
- 1. [Rule 3 - Blocking] Adapted component prop names to match actual API
- Skeleton loading states and EmptyState components added to all 7 remaining dashboard pages, analytics root page created, zero deprecated colours
- Touch-optimized pod selection grid with offline count header, active:scale press feedback, remaining-time countdown, and zero hover-only content
- Commit:
- Status:
- One-liner:
- One-liner:
- rc-sentry `handle_crash()` (tier1_fixes.rs):
- BonoConfig struct in config.rs (relay_port=8099) + bono_relay.rs skeleton with BonoEvent/RelayCommand enums and 5 passing unit tests
- tokio broadcast event push loop + X-Relay-Secret Axum handler wired to AppState.bono_event_tx, 248 tests green
- bono_relay::spawn() wired into server startup with optional Tailscale second listener on :8099, and PodOnline/PodOffline events emitted from pod_monitor.rs at state-transition boundaries
- WinRM PowerShell fleet deploy script for Tailscale on 8 pods + server with canary-first rollout and placeholder guard rails
- racecontrol.toml [bono] section deployed to server with new binary; relay endpoint wired and ready for Tailscale enrollment
- 1. [Rule 3 - Blocking] FailureMonitorState lacks Serialize derive
- One-liner:
- knowledge_base.rs:
- 1. [Rule 1 - Bug] Fixed node_id shadowing in autonomous event branch
- 1. [Rule 3 - Blocking] Module declaration without modifying main.rs
- 1. [Rule 1 - Bug] EscalationPayload field mismatch
- 1. [Rule 2 - Missing] FleetEvent variants did not exist
- 1. [Rule 1 - Bug] Replaced .unwrap() with .single().expect()
- Billing validation gate and expanded double-launch guard in launch_game() with 4 TDD unit tests (LIFE-02, LIFE-04)
- Arm 15s blank_timer in SessionEnded handler + reset billing_active in BillingStopped handler (LIFE-03, LIFE-01 cleanup)
- SQLite model_evaluations table in mesh_kb.db with ModelEvalStore (open/migrate/insert/query) wired into tier_engine so every Fixed/FailedToFix AI diagnosis writes a persistent EVAL-01 record
- One-liner:
- 1. [Rule 1 - Bug] Fixed pre-existing borrow errors in ExperienceScoreReport handler
- One-liner:
- One-liner:
- One-liner:
- SQLite-backed ModelReputationStore persisting demotion/promotion decisions and 7-day accuracy counts across rc-agent restarts via mesh_kb.db
- End-to-end model reputation pipeline: rc-agent pushes ReputationPayload to server via WS after each sweep; GET /api/v1/models/reputation exposes per-model accuracy/status/cost sorted by accuracy DESC
- Weekly JSONL training data export pipeline using eval records + KB solutions in Ollama/Unsloth conversation format, firing every Sunday midnight IST
- Weekly WhatsApp report enhanced with per-model accuracy rankings, KB promotion count, Tier 1 rule cost savings, and improving/declining/stable trend labels — all sourced from Phases 290-292 SQLite stores.
- Structured CM diagnostics (cm_attempted, cm_exit_code, cm_log_errors, fallback_used) now flow from launch_ac() through LaunchResult to GameStateUpdate WebSocket messages
- Billing auto-pauses (PausedGamePause) when Race Engineer exhausts 2 relaunch attempts, kiosk shows structured CM/fallback diagnostics instead of raw error strings
- multiplayer.rs changes:
- PIN-gated coordinated AC launch (all pods start simultaneously when all members validate) and staff-toggleable continuous mode that auto-restarts races within 15s as long as any billing is active
- Per-pod join status tracking on kiosk dashboard with 'Join Failed' + 'Retry Join' button for failed multiplayer pods, and mid-session track/car config change for continuous mode between races
- Capitalization bug fixed in billing_rates seed (lowercase -> Title Case), test migrations extended with billing_rates table + seed assertion (exactly 3 rows), and PROTOC-01 serde alias round-trip test added — all 9 Phase 33 requirements have automated verification with 331+113 tests green
- HTTP 201/204 status code fixes for billing rate CRUD + 4 integration tests proving cache invalidation and cost exclusion (335 tests green)
- UIC-01 unit test added to test_format_cost() with 8 assertions; grep confirms zero rupee strings across all source trees (245 tests green)
- .wslconfig created with mirrored networking config — blocked at WSL2 install by BIOS AMD-V disabled on Ryzen 7 5800X
- Shared POSIX shell test library (lib/common.sh + lib/pod-map.sh) with pass/fail/skip/info/summary_exit helpers and pod IP map, refactored into all three existing E2E scripts
- @playwright/test 1.58.2 with bundled Chromium installed, playwright.config.ts at repo root (sequential/single-worker/reuseExistingServer), cargo-nextest 0.9.131 installed with .config/nextest.toml retry config
- 97 data-testid attributes added across three kiosk TSX files: 49 in book/page.tsx (customer wizard), 43 in SetupWizard.tsx (staff wizard), 5 in page.tsx (landing page)
- Playwright cleanup fixture (auto-runs pre-test) + 4-test smoke suite covering 3 kiosk routes and keyboard navigation, with pageerror capture and DOM snapshot on failure
- One-liner:
- Gate-based deploy verification script with binary swap check (rc-sentry :8091), EADDRINUSE port-conflict polling on :3300, fleet ws_connected/build_id consistency, and per-failure AI debugger log routing
- Single-entry E2E orchestrator (run-all.sh) runs all 4 test phases sequentially, gates on preflight failure, accumulates exit codes, writes timestamped summary.json, exits with total failure count
- Connection: close middleware on axum :8090 + UDP SO_REUSEADDR/non-inherit + OnceLock Ollama client + exec slots doubled to 8
- reqwest probe client connection pooling disabled via pool_max_idle_per_host(0), plus CLOSE_WAIT E2E verification script that checks all 8 pods via netstat over rc-agent /exec
- zero_force_with_retry(3, 100) on FfbController plus StartupReport extended with 4 #[serde(default)] boot verification fields wired to FleetHealthStore
- Conspit Ares wheelbase panic safety: FFB zeroed + lock screen error shown + crash logged + port-bind failures exit cleanly with observable BootVerification in StartupReport
- rp-debug Modelfile with all 14 diagnostic keywords + debug-memory.json seed script covering 7 deterministic AC/F1/fleet crash patterns
- Bash E2E test verifying rp-debug model presence and <5s response latency on all 8 pods via Ollama :11434 over :8090/exec
- One-liner:
- kiosk.rs additions:
- rc-agent autonomously detects and HTTP-ends orphaned billing sessions (5min configurable) + transitions pods to idle PinEntry "Ready" screen after session end instead of blank screen
- CrashRecoveryState enum (2-attempt billing-aware crash recovery) + 30s WS disconnection grace window so venue WiFi blips don't disturb active customer sessions
- 1. [Rule 1 - Bug] addr moved-into-closure borrow error in probe_tcp_port
- HTTP GET /api/v1/pods/{id}/self-test wired end-to-end: server dispatches RunSelfTest via WS, agent runs 22 probes + LLM verdict, SelfTestResult resolves pending oneshot, pod-health.sh verifies all 8 pods
- CLAUDE.md
- Sheets and Calendar MCP servers created with ESM + createRequire pattern; settings.json updated; OAuth re-auth checkpoint reached
- Node.js MCP server with 10 tools wrapping racecontrol REST API — Claude Code can now query fleet health, billing, sessions, and exec commands on pods directly from natural language
- RacingPoint-StagingHTTP
- `/rp:deploy-fleet` Claude Code skill with canary-first gate — Pod 8 deploy + verify.sh + explicit approval before 7-pod fleet rollout
- One-liner:
- rc-agent writes daily-rotating rc-agent-YYYY-MM-DD.jsonl files with pod_id field injected via span, enabling jq-based fleet-wide log aggregation across all 8 pods
- One-liner:
- Netdata deploy script (pod fleet via rc-agent :8090) and E2E verification script (9 hosts) created; MSI (154MB) staged at deploy-staging :9998
- WhatsApp P0 alerter with all-pods-offline + error-rate detection, Evolution API delivery, IST timestamps, rate limiting, and incident recording in SQLite
- Standalone Rust binary querying SQLite (read-only) for sessions, uptime, credits, and incidents -- generates branded HTML email sent via send_email.js to Uday every Monday
- fxm_reset + set_idle_spring + Clone + POWER_CAP_80_PERCENT added to FfbController with 6 unit tests verifying HID byte layout
- safe_session_end() async orchestrator wired to all 10 session-end sites — close ConspitLink (WM_CLOSE 5s) -> fxm.reset -> idlespring ramp 500ms -> restart CL with JSON verification
- 80% startup power cap wired via set_gain(80) at boot, hardware-validated on canary pod across all 4 games with correct wheel centering
- Hardened ConspitLink restart with crash-count tracking, JSON config backup/verify with auto-restore, and polling window minimize retry
- rc-agent startup self-heal: places Global.json at C:\RacingPoint\ forcing AresAutoChangeConfig=open, verifies GameToBaseConfig.json game mappings, and restarts ConspitLink only when config changed
- rc-agent c32d21e1 deployed to Pod 8 with verified Global.json (AresAutoChangeConfig=open) — canary hardware deploy complete, human-verify checkpoint auto-approved
- One-liner:
- Human physically verified on Pod 8 that ConspitLink 2.0 auto-loads AC preset on Assetto Corsa launch and switches to F1 25 preset on F1 25 launch — PROF-04 satisfied by attestation.
- Server .23 NIC pinned to 192.168.31.23 via static IP (PrefixOrigin: Manual, DHCP disabled, DNS corrected to 192.168.31.1) — DHCP reservation deferred due to TP-Link ARP conflict error
- Server Tailscale IP 100.71.226.83 documented; both Tailscale and LAN exec paths to rc-agent :8090 verified working with curl POST /exec returning Racing-Point-Server hostname
- Bono ExecHandler wired end-to-end — James sends exec_request via comms-link WebSocket, Bono executes via ExecHandler, James receives exec_result with stdout/stderr/exitCode. Code verified correct; live round-trip test deferred pending Bono VPS pull + restart.
- POST /relay/exec/send endpoint added to james/index.js, closing Gap 2b — James can now trigger exec_request to Bono's VPS via HTTP relay with generated execId
- TP-Link EX220 firmware bug permanently blocks server DHCP reservation; INFRA-01 satisfied by static IP alone; Bono deployment and exec round-trip deferred asynchronously
- One-liner:
- VenueConfigSnapshot struct + parse_config_snapshot() added to cloud racecontrol, wiring James config into AppState via /sync/push config_snapshot branch with 3 passing unit tests
- SwitchController protocol variant, failover_url config field, and HeartbeatStatus.last_switch_ms AtomicU64 with 5 passing unit tests
- Arc<RwLock<String>> active_url in reconnect loop, SwitchController URL-validated handler with last_switch_ms signal, and 60s self_monitor grace guard — full failover switching wired end-to-end
- One-liner:
- HTTP-triggered SwitchController broadcast via POST /api/v1/failover/broadcast with per-pod split-brain guard probing 192.168.31.23:8090/ping before URL switch
- Secondary watchdog in bono/index.js: detects venue power outage (James + server .23 both unreachable 5min) and auto-activates cloud racecontrol via pm2 + broadcasts SwitchController to pods
- Three ORCH-04 notification gaps closed: notify_failover registered in COMMAND_REGISTRY, Bono watchdog fixed to call sendEvolutionText directly, email added to both failover paths via stdlib-only send-email.js
- POST /api/v1/sync/import-sessions endpoint using INSERT OR IGNORE for lossless billing session failback after cloud-failover window
- One-liner:
- Feature-gated exec module in rc-common with run_cmd_sync (wait-timeout, stdlib-only) and run_cmd_async (tokio, behind feature gate), verified that rc-sentry tree has zero tokio references
- Fully hardened rc-sentry: timeout via rc_common::exec::run_cmd_sync, 64KB output truncation, concurrency cap at 4 with HTTP 429, Content-Length TCP read loop, and tracing structured logging replacing all eprintln!
- One-liner:
- One-liner:
- FfbBackend trait seam with mockall-generated mock, 8 passing unit tests — FFB controller now testable without HID hardware
- 6 async billing_guard tests verify BILL-02/BILL-03 actually send AgentMessage through mpsc after 60s/300s; 6 requirement-named failure_monitor tests trace CRASH-01/CRASH-02 to condition guards
- One-liner:
- Bundled 34 pre-loop agent variables into AppState struct in app_state.rs; all reconnect loop references updated to state.field pattern — enabling event_loop::run() to receive a single parameter in Plan 74-04.
- Extracted the 22-variant CoreToAgentMessage dispatch (~930 lines) from main.rs into ws_handler.rs with handle_ws_message(), WsTx type alias, HandleResult enum, and WS command semaphore/handler -- select! ws_rx arm reduced to 27-line delegation call.
- Extracted the 800-line inner select! loop from main.rs into event_loop.rs with ConnectionState struct bundling all 17 per-connection variables -- handle_ws_message() signature reduced from 18 to 8 parameters; main.rs reduced from 2037 to 1179 lines.
- Complete security posture baseline: 269 racecontrol + 11 rc-agent + 1 rc-sentry endpoints classified, 5 PII locations mapped, CORS/HTTPS/auth state documented, 12 risks prioritized
- Env var overrides for 6 secrets (JWT, terminal, relay, evolution, gmail credentials) with cryptographic JWT key auto-generation rejecting the dangerous hardcoded default
- Staff JWT middleware with strict/permissive variants and 4-tier route split protecting 172+ staff routes via expand-migrate-contract pattern
- Admin login endpoint with argon2id PIN hashing, spawn_blocking verification, and 12-hour staff JWT issuance
- Service key middleware on rc-agent :8090 with constant-time comparison (subtle crate), permissive mode when RCAGENT_SERVICE_KEY unset, /ping and /health remain public
- tower_governor rate limiting on 6 auth endpoints + SQLx transaction-wrapped token consumption with 7 new tests
- PIN login page, AuthGate route wrapper, and 15-minute idle timeout for the Next.js dashboard at :3200
- Switched staff_routes middleware from permissive (log-only) to strict (401 reject) -- contract step of expand-migrate-contract completing the phase goal
- Self-signed cert generation via rcgen with IP SAN for 192.168.31.23, RustlsConfig loader, and backward-compatible ServerConfig extension for dual-port TLS
- Dual-port HTTP/HTTPS server with tower-helmet security headers (CSP, HSTS 300s, X-Frame-Options DENY), HTTPS CORS, and protocol-aware kiosk API_BASE
- Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown.ps1 USB/accessibility/TaskMgr registry lockdown
- IP-based request source classification with pod-blocked staff routes and pod-accessible kiosk endpoints
- BillingStarted carries UUID session_token for kiosk unlock gating; KioskLockdown auto-pauses billing and sends debounced WhatsApp alert to Uday
- AES-256-GCM FieldCipher with deterministic HMAC-SHA256 phone hashing, env-var key loading, and AppState integration -- 10 unit tests green
- Encrypted PII columns in drivers table, all 9 phone lookups converted to HMAC hash, cloud sync encrypts before storing, 7 log statements redacted -- zero plaintext phone/OTP in logs or queries
- DPDP Act self-service data export (decrypted PII JSON) and cascade delete (21 child tables in transaction) behind customer JWT auth -- 8 unit tests green
- Append-only audit_log with action_type classification, log_admin_action() helper across 10 admin handlers, WhatsApp alerts on admin login/topup/fleet exec
- system_settings table tracks admin PIN age with 24h WhatsApp alert check; HMAC-SHA256 signing on outbound sync with permissive inbound verification
- Non-AC game crash auto-recovery via GameProcess::launch() + DashboardEvent::GameLaunchRequested + POST /api/v1/customer/game-request PWA endpoint
- One-liner:
- TOML deployment template and example config updated with all 6 game stanzas (correct Steam app IDs), full pipeline verified green (cargo test + release builds + next build), kiosk UI approved
- Per-game billing engine: GameState::Loading variant, PlayableSignal enum, BillingRateTier/BillingTimer sim_type fields, get_tiers_for_game() fallback logic, DB migration, and protocol sim_type wire-up
- Per-sim PlayableSignal dispatch + 30s exit grace timer in rc-agent ConnectionState: AC=shared memory, F1 25=UdpActive, others=90s process fallback
- Game column in admin billing rates table (SIM_TYPE_LABELS + inline select editor) and kiosk Loading state badge with count-up timer (amber, M:SS, resets on transition to on_track)
- 6 F1 25 unit tests added covering lap completion detection, sector split extraction, invalid lap flagging, session type mapping, first-packet safety, and take() semantics — 11 tests total, all green
- One-liner:
- IracingAdapter wired into rc-agent with IsOnTrack shared-memory billing trigger replacing the 90s process fallback
- LmuAdapter with rF2 fixed-struct shared memory reader: Scoring + Telemetry buffers, torn-read guard, sector splits via cumulative field derivation, first-packet safety, session transition reset, and 6 unit tests
- LmuAdapter wired into rc-agent: SimType::LeMansUltimate creates adapter in main.rs, dedicated PlayableSignal arm in event_loop.rs replaces 90s process fallback with rF2 shared memory IsOnTrack
- AssettoCorsaEvoAdapter with warn-once zero-guard shared memory reads — AC1 struct offsets reused, graceful degradation when EVO Early Access SHM is absent or empty
- Cross-game track name normalization via TRACK_NAME_MAP + sim_type-scoped personal_bests and track_records PRIMARY KEYs with idempotent v2-table SQLite migration
- Optional sim_type query param added to all 4 leaderboard endpoints with available_sim_types discovery array and per-record sim_type field in responses
- One-liner:
- One-liner:
- Psychology engine fully integrated: badges and streaks auto-fire on every session end, dispatcher starts on boot, 5 seed badges in DB, and 5 API endpoints expose psychology data to staff
- One-liner:
- PbAchieved DashboardEvent variant
- Four retention functions in psychology.rs (PB rivalry nudges, surprise credits, streak-at-risk warnings, loss-framed membership expiry) wired from lap_tracker/billing/scheduler with variable_reward_log cap table and extended passport API
- Passport page streak card enhanced with red border + days-remaining countdown when grace period within 7 days, and longest-streak motivational context
- One-liner:
- Commit (whatsapp-bot):
- Status:
- Status:
- Status:
- Status:
- Three AgentMessage/CoreToAgentMessage enum variants added to rc-common + PreflightConfig struct wired into AgentConfig enabling Plan 02 pre-flight logic to compile
- Concurrent pre-flight check runner (HID, ConspitLink, orphan game) with auto-fix + billing_active.store(true) moved inside pre-flight Pass branch so customers are never charged on a maintenance-blocked pod
- MaintenanceRequired LockScreenState variant with branded Racing Red HTML renderer, in_maintenance AtomicBool on AppState, and ws_handler wiring to show maintenance screen on pre-flight failure and clear on ClearMaintenance
- DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) wired into 5-check concurrent runner, plus 30-second maintenance retry select! arm that auto-clears in_maintenance on Pass
- 4 new pre-flight checks (billing_stuck, disk_space, memory, ws_stability) wired into 9-way tokio::join! runner; run() signature extended with ws_connect_elapsed_secs; both call sites updated
- 60s cooldown on PreFlightFailed WS alerts via Option<Instant> on AppState; lock screen + maintenance flag always fire; retry loop confirmed no-alert by design

---

## v35.0 Structured Retraining & Model Lifecycle (Shipped: 2026-04-01)

**Phases completed:** 227 phases, 517 plans, 744 tasks

**Key accomplishments:**

- EscalatingBackoff pre-populated for all 8 pods in AppState, rc-agent fails fast with branded lock screen on missing or invalid config (no silent default fallback)
- One-liner:
- Fixed rc-agent.template.toml to use [pod]/[core] sections matching AgentConfig serde layout, closing DEPLOY-04 gap where deployed configs would fail TOML deserialization
- WatchdogState FSM enum, AppState watchdog fields, DashboardEvent watchdog variants, enriched format_alert_body with heartbeat/next-action context, and GET /health on rc-agent port 18923
- WatchdogState-aware restart lifecycle in pod_monitor: escalating backoff with double-restart prevention, partial-recovery-as-failure, is_closed() WS liveness, PodRestarting/PodVerifying/PodRecoveryFailed dashboard events, and email alerts on verification failure
- pod_healer reads WatchdogState to skip recovery pods, sets needs_restart flag for genuine rc-agent failures, and uses is_closed() for WS liveness — eliminating concurrent restart races
- One-liner:
- One-liner:
- One-liner:
- Files:
- deploy_pod() async executor: kill->wait-dead->download(5MB guard)->start->verify-health with email alerts, 409 guards, and real-time progress via DashboardEvent::DeployProgress
- 1. [Rule 1 - Adaptation] Pod ID format mismatch — plan used pod-N (dashes), codebase uses pod_N (underscores)
- LaunchSplash state with branded HTML + corrected session-end ordering so customers never see Windows desktop during game launch or session end
- Unified PIN error message across all 3 auth surfaces (pod/kiosk/PWA) via PinSource enum + INVALID_PIN_MESSAGE const, plus idempotent pod-lockdown.ps1 for taskbar/Win-key/WU suppression, verified anti-cheat safe on iRacing, F1 25, LMU
- Phase 5 deployment deferred to manual on-site execution — all code verified via automated tests (210 tests green), binary ready in deploy-staging/
- rc-agent log analysis + Edge registry baseline for all 8 pods reveals racecontrol unreachable as universal lock screen root cause and confirms all 8 pods need Phase 9 Edge policy remediation
- Server port audit + IP/MAC identification reveals server IP drifted to .4, both racecontrol and kiosk not running, and server has pod-agent (no RDP needed)
- racecontrol Axum reverse proxy for kiosk paths added to bypass Windows Smart App Control blocking node.exe; CORS updated for kiosk.rp; 21MB release binary staged — server deployment blocked pending physical access
- Port readiness probe + branded RACING POINT startup page eliminates ERR_CONNECTION_REFUSED race and replaces blank boot screen with branded waiting UI
- Minimal scheduled-task watchdog for rc-agent that detects crashes via tasklist+find and restarts via start-rcagent.bat, logging events to C:\RacingPoint\watchdog.log
- 6.7MB static-CRT rc-agent release binary with StartupConnecting port readiness + watchdog staged at deploy-staging/ ready for Pod 8 deployment
- Axum lockdown routes for per-pod and bulk kiosk toggle with billing guard + 10 new unit tests covering parse_mac and lockdown logic
- Kiosk /control page wired to lockdown + restart-all backend: 5 bulk buttons, per-pod padlock toggle, optimistic UI — TypeScript clean, Next.js build passing
- Server-side maintenance tracking via PreFlightFailed/Passed WS events with in_maintenance field on fleet health JSON and POST /pods/{id}/clear-maintenance endpoint sending ClearMaintenance to pod agent
- Racing Red Maintenance badge on fleet pod cards with PIN-gated detail modal showing failure checks and Clear Maintenance button calling POST /pods/{id}/clear-maintenance
- Three new process guard types (ViolationType, ProcessViolation, MachineWhitelist) and three new protocol variants (ProcessViolation, ProcessGuardStatus, UpdateProcessWhitelist) added to rc-common as the compile-time boundary for Phases 102-105
- ProcessGuardConfig/AllowedProcess/ProcessGuardOverride structs with serde Deserialize added to racecontrol/src/config.rs; C:/RacingPoint/racecontrol.toml populated with 185 global allowed entries and 3 per-machine override sections covering all 11 Racing Point machines
- GET /api/v1/guard/whitelist/{machine_id} endpoint with merge_for_machine() logic: global entries filtered by machine type, per-machine deny/allow_extra overrides applied, returns sorted MachineWhitelist JSON or 404 for unknown machine IDs
- ProcessGuardConfig TOML struct, walkdir dep, and AppState guard_whitelist + violation channel added to rc-agent — Plan 02 can now reference all three contracts.
- process_guard.rs with spawn_blocking sysinfo scan, two-cycle grace HashMap, PID-verified taskkill, CRITICAL racecontrol.exe detection, and 512KB-rotating audit log — 9 unit tests green
- Autostart audit (HKCU/HKLM Run + Startup folder) added to process_guard.rs; whitelist fetch, process_guard::spawn(), guard_violation_rx drain, and UpdateProcessWhitelist handler wired into rc-agent — 17 tests green, zero compile errors
- In-memory per-pod ViolationStore with FIFO eviction, fleet/health API extension, ProcessViolation WS handler, and email escalation for 3 kills of same process within 5 minutes.
- Server-side process guard scan loop using sysinfo 0.33 spawn_blocking pattern, CRITICAL detection of rc-agent.exe with zero grace, 512KB-rotating log at C:\RacingPoint\process-guard.log, violations fed into pod_violations["server"] ViolationStore.
- netstat -ano port audit (kill_process_verified + taskkill fallback) and schtasks CSV scheduled-task audit (\\Microsoft\\ skip, disable action) wired into rc-agent 5-minute audit cycle — 11 new TDD unit tests, 28 tests green
- One-liner:
- One-liner:
- build_id added to rc-agent root tracing span + all 65 main.rs tracing calls migrated to target: LOG_TARGET with legacy bracket prefixes removed
- 164 tracing call sites in ws_handler.rs (60), event_loop.rs (53), ac_launcher.rs (51) migrated to structured target: labels, taking rc-agent past the 50% migration mark
- 114 tracing calls migrated across 4 rc-agent modules to structured target labels; legacy [rc-bot] prefix eliminated from ai_debugger.rs; kiosk-llm sub-target preserved for independent log filtering
- 79 tracing call sites across 7 rc-agent files migrated to structured target: LOG_TARGET labels; legacy [rc-bot] and [billing-guard] bracket prefixes eliminated
- Audit checks run:
- Exhaustive rc-agent anti-cheat risk inventory with per-system severity classifications and source references, GPO decision made for Phase 108, Sectigo OV cert checklist ready for Uday
- Per-game anti-cheat compatibility matrix (17 subsystems x 6 games) plus ConspitLink audit template with ProcMon capture procedure
- SetWindowsHookEx global keyboard hook removed from default rc-agent build and replaced with GPO registry keys (NoWinKeys=1 + DisableTaskMgr=1 via reg.exe), with hook preserved behind keyboard-hook Cargo feature for emergency rollback
- SafeMode state machine with WMI process watcher, sysinfo startup scan, and full AppState integration for anti-cheat compatible protected game detection
- Safe mode wired into all 7 integration points: LaunchGame entry, WMI polling, 30s cooldown timer, process_guard scan skip, Ollama suppression, kiosk GPO deferred, lock_screen Focus Assist deferred
- Inline SVG Racing Point logo in all lock screens, wallpaper URL support via SettingsUpdated, session summary with top speed + race position stats, and persistent results screen (no 15s auto-reload)
- Staff-facing wallpaper URL input added to kiosk settings page — completes BRAND-02 end-to-end chain from dashboard to pod lock screen
- 5-second deferred SHM connect (HARD-03) and AC EVO feature flag off by default (HARD-05) to avoid triggering EAC/EOS/Javelin anti-cheat scans during game startup
- One-liner:
- rc-agent.exe built from HEAD (all v15.0 features: safe mode, GPO lockdown, telemetry gating) and deployed to Pod 8 via SSH+service-key chain — ws_connected=true, uptime>30s, build_id=243f03d
- go2rtc v1.9.13 installed on James (.27) relaying sub-streams from 3 Dahua cameras (entrance .8, reception .15, reception_wide .154) at :8554 with API at :1984 and auto-start via HKLM Run key
- rc-sentry-ai crate with retina RTSP frame extraction, per-camera reconnect loops, and shared FrameBuffer for 3 Dahua cameras via go2rtc relay
- Axum health endpoint at :8096 with per-camera connection status (connected/reconnecting/disconnected), go2rtc relay health probe, and overall service status
- People tracker migrated to read all 3 camera RTSP streams from go2rtc relay at localhost:8554, eliminating direct camera connections
- SCRFD-10GF ONNX face detector with CUDA EP and openh264 H.264 decoder -- core detection building blocks
- Live detection pipeline wiring: per-camera decode->preprocess->detect loop with config-gated SCRFD init and health endpoint stats
- DPDP Act 2023 compliance with mpsc audit log, 90-day retention purge, right-to-deletion API, and consent signage on :8096
- Quality gate filter chain rejecting faces by size (<80x80), blur (Laplacian var <100), and pose (yaw >45deg), plus CLAHE lighting normalization producing grayscale-as-RGB face crops
- ArcFace ONNX recognizer with CUDA EP producing L2-normalized 512-D embeddings from 112x112 similarity-transform-aligned face crops
- Cosine similarity gallery with SQLite persistence, face tracker cooldown, and full pipeline integration from SCRFD through ArcFace to identity logging
- Full person CRUD with phone migration, Gallery add/remove methods, and enrollment request/response DTOs for face enrollment API
- Axum REST API with 6 endpoints for person CRUD and photo upload with full SCRFD->ArcFace ML pipeline, duplicate detection, gallery sync, and audit logging
- Broadcast channel wiring from detection pipeline to attendance engine with 5-min cross-camera dedup and SQLite attendance_log table
- Staff clock-in/clock-out state machine with SQLite persistence and automatic shift tracking on recognition events
- Four Axum REST endpoints for attendance presence, history, and shift queries with IST timezone and completeness flags
- AlertEvent type system with tagged JSON serialization and WebSocket /ws/alerts endpoint broadcasting recognized-person events to dashboard clients via tokio broadcast fan-out
- Windows desktop toast notifications via winrt-toast for face detection alerts with IST timestamps and system sound
- Unknown face detection with rate-limited alerts and 112x112 JPEG crop saving to C:\RacingPoint\face-crops\
- MJPEG streaming endpoints serving live camera feeds via H.264-to-JPEG transcoding with per-connection decoders and CORS for dashboard access
- Live MJPEG camera feeds page at /cameras with grid layout, status indicators, and sidebar navigation
- Dahua NVR CGI API client with HTTP Digest auth, mediaFileFind search, and RPC_Loadfile streaming
- Axum playback proxy with NVR search, streaming video passthrough, and attendance event timeline
- Next.js playback page with camera/date/time search form, recording file list, HTML5 video player streaming via rc-sentry-ai proxy, and attendance event timeline with click-to-seek markers
- WAL tuning, covering indexes for leaderboard/telemetry, cloud_driver_id column, and 6 competitive tables (hotlap_events, championships, standings, ratings) with TDD
- car_class column on laps table, auto-populated from billing_sessions -> kiosk_experiences JOIN in persist_lap(), with TDD
- Suspect column on laps table with sector-sum and sanity-time checks computed in persist_lap() before INSERT
- sim_type filtering on track leaderboard (defaults assetto_corsa), /public/circuit-records and /public/vehicle-records/{car} endpoints with suspect-always-hidden policy
- Fire-and-forget email notification to previous record holder when their track record is beaten, with fetch-before-UPSERT data ordering and nickname-aware new holder display
- Public driver search (case-insensitive name/nickname, max 20) and profile endpoints (stats, personal bests, lap history) with zero PII exposure and sector-zero-to-null mapping
- Public leaderboard with sim_type/invalid filter, circuit records with car filter, driver search with debounced input, and driver profile with stats/personal bests/lap history -- all mobile-first at 375px
- rc-agent load_config() now searches the executable's directory first, making it self-healing when launched by DeskIn or SYSTEM services with a different CWD
- pod-agent v0.5.1: exec default timeout reduced 30s->10s, slot exhaustion eprintln! warning, 7 tests all passing, binary staged at deploy-staging/
- PodAgent HKLM Run key confirmed on 5/8 pods, FleetHealth scheduled task created, Pod 8 pod-agent upgrade partially completed (needs reboot to recover)
- One-liner:
- ExecHandler wired with dynamic-first/static-fallback lookup, completedExecs LRU-capped at 10000, HTTP /relay/registry/register endpoint and WS registry_register handler deployed on both James and Bono with JSON persistence.
- ShellRelayHandler class with binary allowlist, hardcoded APPROVE tier, and approval queue with default-deny timeout -- wired into both James and Bono WS routing and HTTP endpoint.
- ExecResultBroker (pending-promise utility) and ChainOrchestrator (sequential exec chains with stdout piping, abort-on-failure, continue_on_error, and chain-level timeout) -- fully TDD'd with 16 tests green.
- ExecResultBroker and ChainOrchestrator wired into both james/index.js and bono/index.js -- exec_result routes through shared broker, chain_request triggers ChainOrchestrator.execute on both sides, FailoverOrchestrator simplified to delegate to broker.
- AuditLogger class (appendFileSync JSONL) and delegate_request/delegate_result protocol types -- foundation for wiring audit trail into both james and bono daemons in Plan 02.
- Bidirectional Claude-to-Claude delegation (delegate_request/delegate_result) wired into both james and bono daemons with per-step AuditLogger calls on all exec paths (exec, shell_relay, chain, delegate tiers).
- ChainOrchestrator extended with named template resolution (chains.json + templatesFn injection), {{prev_stdout}} arg substitution with shell metacharacter sanitization, and per-step retry with linear backoff and new execId per attempt
- ChainOrchestrator gains pause/resume capability for WS disconnect resilience (CHAIN-09) and either AI can query the other's command registry over WS with safe field filtering (DREG-06)
- One-liner:
- CommsLink-DaemonWatchdog Task Scheduler task registered (every 2 min), HKCU Run key verified correct, watchdog confirmed healthy via manual trigger.
- Missing chain_result WS handler added to james/index.js — /relay/chain/run now resolves synchronously via ExecResultBroker instead of 504-timing-out
- Relay health endpoint now exposes connectionMode + lastHeartbeat, /relay/exec/run fails fast with 503 when not REALTIME, and SKILL.md guides callers to probe health before sending
- close_browser() safe mode gate (BWDOG-04):
- 30-second browser watchdog in rc-agent event loop that auto-recovers from Edge crashes and process stacking (>5 msedge.exe), gated by safe mode and Hidden state
- AgentMessage::IdleHealthFailed added to rc-common with pod_id, failures, consecutive_count, and timestamp — serde round-trip test passes, idle_health_failed JSON tag confirmed
- 60s idle health loop in rc-agent event_loop.rs: probes lock_screen_http + window_rect, self-heals via close+relaunch, sends IdleHealthFailed after 3 consecutive failures, skips during billing and safe mode
- IdleHealthFailed WS handler added to racecontrol: logs warn + activity_log + updates FleetHealthStore, exposed in GET /api/v1/fleet/health via idle_health_fail_count and idle_health_failures per pod
- ForceRelaunchBrowser WS protocol variant added to rc-common and wired into pod_healer Rule 2 for soft Edge lock screen recovery over live WebSocket before escalating to restart
- rc-agent ForceRelaunchBrowser handler: billing-gated close_browser + launch_browser on server-initiated WS message, completing the server-to-pod lock screen recovery round-trip
- 19 failing RED test stubs covering auto-entry, 107% rule, badges, F1 scoring, gap-to-leader, and championship tiebreaker — with 3 schema migrations closing the p2/p3 tiebreaker gap
- 9 staff CRUD endpoints for hotlap events and championships in routes.rs, using check_terminal_auth() and COALESCE-based partial updates
- auto_enter_event() + recalculate_event_positions() in lap_tracker.rs — valid laps automatically enter matching hotlap events with gold/silver/bronze badges and 107% flagging computed at write time
- F1 2010 point scoring from multiplayer_results into hotlap_event_entries, with live-computed championship standings using wins/P2/P3 tiebreaker — 6 new tests GREEN, 329 total passing
- 5 public GET endpoints for events listing, event leaderboard with badges/107%/gap, group session F1 results, championships listing, and championship standings with per-round breakdown — 331 tests GREEN
- One-liner:
- execute_ai_action() wired in event_loop.rs with safe mode AtomicBool gate; pod_healer.rs logs AI-recommended actions to activity_log via local whitelist parser
- Rolling-window WARN log scanner integrated into heal_all_pods() with 5-min count window, 50-entry threshold, and 10-min cooldown gating via AppState.warn_scanner_last_escalated
- WARN surge deduplication and AI escalation via escalate_warn_surge() — groups identical WARN messages by frequency, builds compact context, calls query_ai(source="warn_scanner"), persists to ai_suggestions with pod_id="server"
- One-liner:
- Live integration test scaffold for INTEG-01 (exec round-trip via /relay/exec/run) and INTEG-03 (message relay via /relay/task) with graceful PSK-absent skip using node:http and node:test
- Chain round-trip test (INTEG-02), cross-platform syntax checker (INTEG-04), and 15 pure-static contract tests (INTEG-05) added to complete the comms-link integration suite
- Bash test gate `test/run-all.sh` that runs contract tests + integration tests + syntax check in sequence, prints a colour-coded per-suite PASS/FAIL/SKIPPED summary block, and exits 0 only when all gated suites pass — satisfying GATE-01 single-command invocation
- Pre-Ship Gate section wired into comms-link/CLAUDE.md with exit-code contract blocking phase completion on non-zero exit, and quality gate bullet added to rp-bono-exec SKILL.md — GATE-02 and GATE-03 satisfied
- go2rtc configured with 13 NVR sub-stream channels (ch1-ch13), CORS enabled, and WebRTC + snapshot coexistence verified on live Dahua NVR hardware
- Task 1 — config.rs:
- Task 1 — mjpeg.rs:
- Single-file NVR dashboard with 4-mode CSS grid layout switching, dynamic camera loading from /api/v1/cameras, green/yellow/red status dots, and snapshot polling with configurable refresh rate
- HTML5 drag-and-drop camera reordering with auto-save on drop, zone-grouped collapsible headers (ENTRANCE/RECEPTION/PODS/OTHER), and full camera_order persistence via PUT /api/v1/cameras/layout
- RTCPeerConnection via go2rtc WebSocket signaling (ws://192.168.31.27:1984/api/ws) with singleton pattern, 500ms hover pre-warm, auto-hiding controls, and snapshot fallback on failure
- 1. [Rule 1 - Bug] Removed erroneous Promise wrapper in resetControlsTimer
- Schema (db/mod.rs):
- Next.js /cafe admin page with item table + slide-in side panel delivering full CRUD (add, edit, delete, toggle) and inline category creation, wired to the Plan 01 Rust backend
- 3 new Axum handlers:
- Import modal with XLSX/CSV preview-and-confirm flow, per-item image upload column, and 4 new TypeScript API types on the /cafe admin page
- CafeMenuPanel with category tabs and paise-to-rupee formatting integrated into POS control page via SidePanel toggle
- Next.js PWA /cafe page with 2-column card grid grouped by category, image fallback, paise-to-rupees price formatting, and Cafe tab replacing Stats in BottomNav
- SQLite inventory columns (is_countable, stock_quantity, low_stock_threshold) added to cafe_items with idempotent migrations, CafeItem struct updated, and POST /cafe/items/{id}/restock endpoint implemented
- Inventory management UI added to /cafe admin — Items/Inventory tabs, inline restock flow, color-coded threshold badges (red/yellow/green/gray) sorted low-stock first
- 1. [Rule 1 - Bug] Config::default() does not exist
- Red low-stock warning banner in CafePage with 60s polling via api.listLowStockItems(), listing each breached item's name, stock quantity, and threshold
- Task 1 — api.ts changes:
- Staff POS order builder in CafeMenuPanel: item add/qty controls with out-of-stock overlays, live customer name search, and wallet-debiting checkout via POST /api/v1/cafe/orders
- One-liner:
- SQLite cafe_promos table + five Axum admin CRUD endpoints for combo/happy_hour/gaming_bundle promos with stacking group support
- One-liner:
- PromoBanner + discount receipt display added to PWA cafe page and kiosk CafeMenuPanel, fetching active promos from /cafe/promos/active on mount with non-fatal error handling
- satori/resvg-wasm PNG generation API (Next.js) + Evolution API WhatsApp broadcast with 24h per-driver cooldown (Rust/Axum)
- Marketing tab on /cafe admin page: promo PNG generation (blob download), daily menu PNG, WhatsApp broadcast form with result summary
- Shared recovery contracts in rc-common: RecoveryAuthority ownership registry, RecoveryDecision JSONL logger, and single-owner process enforcement for rc-sentry/pod_healer/james_monitor
- One-liner:
- Rust firewall module using netsh advfirewall with delete-then-add idempotency — ICMP echo and TCP 8090 rules applied on every rc-agent startup, eliminating CRLF-damaged batch file failures permanently
- RCAGENT_SELF_RESTART sentinel detection wired into handle_crash() plus RecoveryLogger logging every recovery decision to C:\RacingPoint\recovery-log.jsonl via build_restart_decision() pure helper
- Pattern hit-count escalation skips restart after 3 same-pattern crashes and pre-restart Ollama query with 8s timeout consults AI before handle_crash for unknown patterns
- PodRecoveryTracker with 4-step graduated offline recovery — wait 30s, Tier 1 restart, AI escalation, staff alert — gated on in_maintenance and billing_active checks
- One-liner:
- failure_state.rs
- rc-watchdog.exe deployed to deploy-staging and registered as CommsLink-DaemonWatchdog Task Scheduler task on James (.27), replacing james_watchdog.ps1 with Rust-based monitoring running every 2 minutes
- CoreToAgentMessage::Exec and AgentMessage::ExecResult variants with serde roundtrip tests proving snake_case wire format and default timeout
- Semaphore-gated WebSocket command execution handler with independent 4-slot concurrency, 64KB output truncation, and spawned-task mpsc drain pattern in the agent event loop
- Core-side ExecResult handler with oneshot channel resolution, ws_exec_on_pod() public function, and deploy.rs HTTP-first WS-fallback for all pod commands
- Three GitHub repos archived (game-launcher, ac-launcher, conspit-link) with README notices; 7 non-git folders catalogued with archive/delete/keep decisions
- Full npm + cargo security audit across all 15 repos — 7 npm highs fixed, rustls-webpki patched in both Rust repos, 8 deferred items documented with rationale
- One-liner:
- Commit:
- Commit:
- Single 682-line authoritative document cataloguing all 333 HTTP endpoints across 4 boundary directions (racecontrol, rc-agent, comms-link, admin) with typed request/response shapes and 8 shared data structure tables
- One-liner:
- One-liner:
- Next.js App Router GET /api/health endpoints added to kiosk (:3300) and web dashboard (:3200), returning { status: ok, service, version: 0.1.0 } with zero TypeScript errors
- comms-link relay /health added returning `{ status: 'ok', service: 'comms-link', version, connected, clients }`, with racecontrol and rc-sentry confirmed already compliant
- deploy-staging triaged from 719 dirty files to zero untracked: .gitignore expanded with 15+ patterns covering JSON relay payloads, build artifacts, logs, and screenshots; 146 operational scripts committed
- check-health.sh polls 5 services (racecontrol/kiosk/web/comms-link/rc-sentry) with PASS/FAIL output; deploy.sh orchestrates racecontrol/kiosk/web/comms-link deploys and gates each on health check
- 1. [Rule 2 - Missing coverage] Added rc-sentry service to runbook
- One-liner:
- One-liner:
- crates/rc-common/src/types.rs
- Optional Cargo features added to rc-agent (ai-debugger, process-guard, http-client) and rc-sentry (watchdog, tier1-fixes, ai-diagnosis) — both crates compile with default features (full production) and --no-default-features (minimal/bare builds)
- One-liner:
- SQLite-backed feature flag registry with REST CRUD (GET/POST/PUT /flags), in-memory RwLock cache, real-time FlagSync WS broadcast to pods, and config_audit_log audit trail sourced from StaffClaims.sub
- Config push REST+WS pipeline: whitelist validation with 400 field-level errors, per-pod SQLite queue with monotonic seq_num, WebSocket ConfigPush delivery, offline replay on reconnect (status-based filter), and deterministic ConfigAck audit lookup by seq_num
- One-liner:
- In-memory feature flag system for rc-agent with disk cache, WS-driven sync via FlagSync/KillSwitch, and FlagCacheSync on reconnect using Arc<RwLock<FeatureFlags>>
- feature_flags.rs:
- Status:
- AgentMessage::StartupReport with serde roundtrip tests, sent once per process lifetime from rc-agent to racecontrol with version, uptime, config hash, crash recovery flag, and self-heal repairs
- 76 standing rules classified into AUTO/HUMAN-CONFIRM/INFORMATIONAL registry with 6 new OTA Pipeline rules added to CLAUDE.md
- gate-check.sh with 5 pre-deploy suites (comms-link E2E, cargo tests, 18 AUTO standing rules, diff analysis, HUMAN-CONFIRM checklists) and 4 post-wave suites (comms-link E2E, build ID verification, fleet health, AUTO standing rules)
- PipelineState::Paused + gate-check.sh integration wired into OTA pipeline with no force/skip mechanism, Bono synced via INBOX.md + WS
- Graduated 4-tier crash handler with 500ms spawn verification, server-reachable exclusion from MAINTENANCE_MODE, and recovery event HTTP reporting to racecontrol server
- Tier 3 Ollama diagnosis and Tier 4 WhatsApp escalation wired into graduated crash handler — completing the 4-tier recovery pipeline with spawn-failure-triggered AI diagnosis and 5-min-cooldown staff alerts
- 1. [Rule 1 - Bug] Fixed pre-existing E0433: chrono unresolved in tier1_fixes.rs
- One-liner:
- One-liner:
- One-liner:
- rc-agent self_monitor checks TCP :8091 before relaunch — yields to rc-sentry (zero PowerShell) when sentry alive, falls back to PowerShell only when sentry is dead
- Shared ollama module in rc-common (pure TcpStream), spawn verification (500ms/10s) in james_monitor and rc-watchdog, and 30s sentry-breadcrumb grace window to prevent double-restart coordination failures
- One-liner:
- One-liner:
- One-liner:
- rc-watchdog Windows SYSTEM service with SCM lifecycle, tasklist process polling, Session 1 spawn via WTSQueryUserToken + CreateProcessAsUser, and fire-and-forget HTTP crash reporting
- racecontrol POST /api/v1/pods/{pod_id}/watchdog-crash endpoint with WARN-level structured logging, activity recording, and install-watchdog.bat for fleet SCM registration with failure restart actions
- Tier 2:
- 10 audit phase scripts (tiers 7-9) plus full load_phases()/dispatch rewrite wiring all 44 phases across 5 modes and 9 tiers
- File-based semaphore parallel engine (audit/lib/parallel.sh) with 4-slot mkdir locking, 200ms pod stagger, and audit.sh updated to source it and dispatch all 60 phases across tiers 1-18 in full mode.
- One-liner:
- 7 bash phase scripts completing the full 60-phase v3.0 audit port — registry/relay integrity, DB migration completeness, LOGBOOK/OpenAPI freshness, E2E test suites, cloud path, customer flow, and cross-system chain checks
- `audit/lib/results.sh`
- jq-based delta engine joining phase+host composite key across consecutive audit runs, with venue-aware PASS/QUIET/FAIL categorization that prevents false regressions when venue closes
- `check_suppression(phase host message)`
- Bash auto-fix engine with billing-gate, OTA sentinel check, whitelist enforcement, and 3 safe fixes (sentinel clear, orphan PS kill, rc-agent restart) all logged via emit_fix()
- Failure-safe three-channel notification engine (Bono WS + INBOX.md + WhatsApp Uday) gated behind --notify flag with delta summary inclusion
- Changes to audit/audit.sh:
- One-liner:
- SQLite launch_events table + JSONL dual-write infrastructure with LaunchEvent/LaunchOutcome/ErrorTaxonomy types wired into all 5 game_launcher call sites
- SQLite billing_accuracy_events and recovery_events tables with recording functions wired into billing.rs and game_launcher.rs, producing real rows on every billing start and every Race Engineer crash recovery.
- GameLauncherImpl trait with 4 per-game impls, fixed billing gate (deferred billing + paused rejection), TOCTOU mitigation, and invalid JSON rejection — all backed by 19 passing unit tests
- Stopping state blocked at double-launch and relaunch, all 6 broadcast failures now logged at warn, externally_tracked field added, 30s Stopping timeout spawned in stop_game(), and feature flag gate before launch — 29 unit tests all passing
- Server-side launch resilience: dynamic timeout from launch history, typed exit_code error taxonomy, atomic Race Engineer with WhatsApp staff alerts after 2 failed retries.
- Agent-side launch hardening: pre-launch sentinel+orphan+disk checks, AC polling waits replacing hardcoded sleeps, CM 30s timeout with progress logging, fresh PID via find_acs_pid() on fallback, and split_whitespace fix for paths with spaces.
- AC False-Live guard (5s speed+steer gate), CancelledNoPlayable status variant, and BillingConfig (5 configurable timeouts) -- billing foundation for Plan 02 server-side logic
- WaitingForGame tick broadcasts for kiosk Loading state, cancelled_no_playable DB records on timeout/crash, paused seconds persistence for game-pause, single-timestamp billing accuracy, and multiplayer error rejection — billing.rs fully wired to BillingConfig
- 4 new billing tests for BILL-05/06/10/12 — 82 total billing tests, 0 failures, Phase 198 complete with all 12 BILL requirements implemented and verified
- Server-side crash recovery hardening: force_clean protocol flag, history-informed recovery action selection via query_best_recovery_action(), enriched recovery events with actual ErrorTaxonomy/car/track/exit_codes, and structured staff WhatsApp alert
- Safe mode cooldown suppression during PausedWaitingRelaunch, exit grace guard verification on all paths, and unit tests for recovery contracts
- crates/rc-common/src/types.rs:
- crates/rc-agent/src/self_heal.rs:
- RED phase:
- RED phase:
- One-liner:
- Kiosk updated to 10-variant BillingSessionStatus, local type removed, game loading/crash-recovery/disconnect UI added, reliability warning with alternatives modal on review step.
- One-liner:
- Audit Phase 02 validates ws_connect_timeout >= 600ms and app_health URL ports; Phase 21 billing checks emit WARN (not PASS) when unreachable during venue hours; Phase 53 detects watchdog dead (ps_count=0)
- Live Evolution API connection check, OAuth token expiry verification, real display resolution queries, and start-rcsentry-ai.bat added to repo
- Upgraded 4 audit phase scripts from count/existence checks to content health verification -- svchost.exe allowlist spot-check, menu availability, flag enabled-state, and OpenAPI endpoint name verification
- VerifyStep trait, ColdVerificationChain, HotVerificationChain, VerificationError (4 typed variants), and spawn_periodic_refetch() added to rc-common with 13 tests passing and all downstream crates (rc-agent, racecontrol, rc-sentry) still compiling
- Silent config fallback eliminated: rc-agent 5 unwrap_or sites + racecontrol load_or_default() + process guard empty allowlist + all 4 FSM transitions now emit observable signals at the moment each degraded state occurs
- Sentinel file changes are now instantly observable: every create/delete in C:\RacingPoint\ produces a WS message to racecontrol within 1 second, updates active_sentinels in fleet health API, broadcasts DashboardEvent::SentinelChanged, and MAINTENANCE_MODE creation fires WhatsApp alert to Uday with 5-min rate limiting
- Feature flags self-heal via HTTP GET /api/v1/flags every 5 minutes using spawn_periodic_refetch, with CLAUDE.md standing rule banning single-fetch-at-boot patterns
- Process guard first-scan threshold validation with >50% violation detection and GUARD_CONFIRMED operator confirmation gate before kill_and_report escalation
- ColdVerificationChain wrapping pod healer curl parse (4-step) and config TOML load chains (3-step racecontrol, 2-step rc-agent) with first-3-lines SSH banner diagnostics
- ColdVerificationChain wrapping allowlist enforcement (COV-04) and spawn verification (COV-05) with structured tracing for empty-allowlist and spawn-but-dead diagnostics
- StepValidateCriticalFields emits TransformError on default fallback (COV-03) and spawn verification retries once on PID liveness failure (COV-05)
- Suite 5 domain-matched verification added to gate-check.sh -- detects display/network/parse/billing/config changes via git diff and enforces domain-specific verification gates
- Interactive bash helper (fix_log.sh) enforcing 5-step structured debugging with LOGBOOK.md template showing real pod healer flicker example
- Fleet exec probe (curl /api/v1/fleet/health) and WS handshake test (curl Upgrade headers) added to network domain gate in both pre-deploy and domain-check modes
- One-liner:
- Mobile-first /kiosk/fleet page showing 8 pod cards with WS/HTTP status dots, version, uptime, and 5-second polling via api.fleetHealth()
- Bat file drift detection + syntax validation scanner for 8-pod fleet via rc-sentry /files endpoint with 5 anti-pattern checks
- 5 audit phases (bat-drift, config-fallback, boot-resilience, sentinel-alerts, verification-chains) with deploy-pipeline bat sync and Debug Quality report section
- Five safety gates added to auto-detect.sh: PID run guard (SCHED-03), 6-hour per-pod+issue escalation cooldown (SCHED-04), venue-state-aware mode override (SCHED-05), and extended MAINTENANCE_MODE sentinel check
- Windows Task Scheduler bat for AutoDetect-Daily at 02:30 IST with safety gate verification, plus Bono VPS cron corrected to 02:35 IST
- One-liner:
- Kiosk /shutdown page with staff PIN gate, 6-state machine (idle/confirming/auditing/audit_passed/shutting_down/complete), audit-blocked reason display, and navigation from /staff
- Cascade detection framework (cascade.sh) with _emit_finding helper wired into auto-detect.sh step 4, plus 3 detector scripts: rc-agent.toml config drift (DET-01), bat file checksum drift (DET-02), and venue-aware ERROR/PANIC log anomaly detection (DET-03)
- 3 remaining detection modules (crash loop DET-04, flag desync DET-05, schema gap DET-06) added to scripts/detectors/ completing the full 6-detector cascade pipeline; all scripts pass bash -n syntax and auto-detect.sh --dry-run exits 0
- 5-tier graduated escalation engine (retry → restart → WoL → cloud failover → WhatsApp) with 3 new APPROVED_FIXES, sentinel-aware billing-gated execution, and runtime JSON toggle for auto_fix_enabled/wol_enabled
- Live-sync healing wired end-to-end: all 6 detectors call attempt_heal() immediately after _emit_finding(), cascade.sh sources escalation-engine.sh, and auto-detect.sh routes WhatsApp escalation through escalate_human() with HEAL-04 silence conditions
- `scripts/coordination/coord-state.sh`
- bono-auto-detect.sh extended with Tailscale-confirmed offline detection (COORD-02) and full recovery handoff protocol including findings JSON, INBOX.md push, and pm2 failover deactivation (COORD-03)
- Pattern tracking (LEARN-01) + trend outlier detection (LEARN-04) wired into auto-detect.sh — every run now permanently records what was found, what was fixed, and flags pods with 4x+ fleet-average bug frequency
- Suggestion engine that converts raw suggestions.jsonl pattern data into categorized JSON proposal files (6 categories, confidence scoring, deduplication) with relay exec inbox query via get_suggestions command
- One-liner:
- Self-modifying intelligence loop with CE methodology: threshold-only patches, realpath scope safety, bash -n verification, auto-revert on failure, independent self_patch_enabled=false toggle
- Task 1: Fixture files for all 6 detectors (10 files)
- Escalation 5-tier ladder test (TEST-03) + coordination mutex test (TEST-04) with unified --all entry point in test-auto-detect.sh
- RCAGENT_SELF_RESTART sentinel added to rc-agent exec handler — pods now restart via direct Rust call to relaunch_self(), completely bypassing cmd.exe, start-rcagent.bat, and PowerShell interpretation issues that caused pods 6/7/8 to go offline
- deploy_pod.py upgraded with server-exec fallback + EncodedCommand writes + rename-copy swap; Pod 2 deployed with new binary, pods 1/3-8 blocked by offline rc-agent
- One-liner:
- Tier 1 (Deterministic) — fully implemented:
- PodFailureReason enum (18 variants, 9 classes) and 5 typed AgentMessage bot failure variants established as the shared protocol foundation for all Phase 24-26 bot detection code
- Pure concurrency guard predicate is_pod_in_recovery(&WatchdogState) -> bool added to pod_healer.rs, blocking Phase 24 bot tasks from acting on pods in active watchdog recovery cycles
- Plan 01 + 02 combined (knowledge_base.rs — 373 lines):
- PodStateSnapshot gains Default derive + 3 telemetry fields; 10 RED test stubs written for 5 bot fix requirements (CRASH-01/02/03, UI-01, USB-01) — Wave 0 Nyquist compliance complete
- 3 new auto-fix functions (fix_frozen_game, fix_launch_timeout, fix_usb_reconnect) + extended fix_kill_error_dialogs turn all 10 Wave 0 RED tests GREEN in ai_debugger.rs
- failure_monitor.rs with CRASH-01/CRASH-02/USB-01 detection state machine, 8 tests green, all try_auto_fix calls wrapped in spawn_blocking
- failure_monitor spawned as live task in rc-agent with 13 state update sites keeping all 6 FailureMonitorState dimensions current from the event loop
- Task 1 — FailureMonitorState.driving_state compile gate:
- billing_guard.rs (~150 lines):
- Server-side bot message router with recovery guard, stuck-session auto-end, idle-drift staff alert, and hardware/telemetry stubs — 5 new tests, 299 total passing
- End-to-end wiring: billing_guard spawned from rc-agent main.rs, ws/mod.rs bot stubs replaced with bot_coordinator async calls, BILL-04 relay sync fence added to recover_stuck_session
- SQLite WAL fail-fast verification + staggered 60s timer persistence with COALESCE crash recovery for 8-pod concurrent writes
- One-liner:
- Atomic billing start via single sqlx transaction (wallet debit + session INSERT) with idempotency keys on all four money-moving endpoints (billing start, topup, stop, refund)
- end_billing_session UPDATE:
- 30-minute background reconciliation job using SQL correlated subquery to detect wallet balance vs transaction-sum drift, with ERROR logging, WhatsApp alerting, and admin GET/POST endpoints
- Server-side billing FSM with 20-rule TRANSITION_TABLE, validate_transition() gates all 9 status mutation sites in billing.rs, and authoritative_end_session() provides single CAS-protected end path
- FSM-02 — Phantom Billing Guard
- Split session parent+child entitlement model with CAS guards, FSM-08 DB-before-launch guard preventing orphaned game launches when no billing record exists
- Server-side INI injection prevention via character allowlist, FFB GAIN safety cap at 100, and three-tier RBAC (cashier/manager/superadmin) enforced on Axum route groups via JWT role claims
- Argon2id OTP hashing replacing SipHash DefaultHasher, SQLite BEFORE DELETE trigger making audit_log append-only, and role-gated PII masking (phone/email) for cashier staff in driver API responses
- Self-topup block via JWT sub comparison, WSS TLS with native-tls connector and custom CA support, game launch race condition eliminated via tokio Mutex in AppState
- 18% inclusive GST split in 3-line journal entries, per-session GST invoices with GSTIN/SAC/CGST/SGST, and Consumer Protection Act pricing disclosure in the kiosk display endpoint
- Waiver gate in start_billing (Indian Contract Act 1872), guardian OTP consent flow for minors, and argon2-hashed guardian OTP send/verify via WhatsApp Evolution API
- DPDP Act 2023 compliance: 8-year financial record retention config, daily PII anonymization job for inactive drivers, and immediate consent revocation endpoints for customers and guardian-proxy requests.
- Steam pre-launch gate (readiness + DLC) and window detection via sysinfo polling, with corrected fleet-monitoring process names for F1, iRacing, LMU, and Forza
- SessionEnforcer
- PWA game requests auto-expire after 10 min via server-side TTL, extensions enforce current tier rate, billing timer provably starts at game-live signal, and crash recovery pause time is excluded from billable seconds via PauseReason enum
- One-liner:
- Multiplayer billing synchronized across all group pods on AC crash (BILL-07), customer charge dispute portal with staff approve/deny workflow and atomic refund via existing FATM paths (BILL-08)
- Four-endpoint staff financial controls: discount approval gate with manager PIN validation above Rs.50 threshold, daily override audit report (discounts/refunds/tier changes), and cash drawer reconciliation with discrepancy logging
- Shift handoff API with active-session acknowledgment gate, DEPLOY-01 session drain verified across 3 billing hook points, DEPLOY-03 weekend 18:00-23:00 IST deploy lock with force override across all deploy entry points
- Graceful agent shutdown with billing session persistence, post-restart interrupted session recovery, and WS command_id deduplication preventing stale replay on reconnect
- Atomic wallet-debit-plus-time-addition for session extensions via single SQLite transaction, plus server-side discount stacking floor enforced in start_billing and apply_billing_discount
- Schema migration (db/mod.rs):
- One-liner:
- LapData gains required session_type field; catalog adds per-track minimum lap time floors; persist_lap sets review_required=1 for below-floor laps with idempotent DB migration
- Two structurally separate RwLock<HashMap> counters in AppState enforce that customer PIN lockout can never block staff debug PIN access — customer exhausts 5 attempts, staff still unlocks freely
- TELEM-01 and MULTI-01 fully operational: staff email on 60s UDP silence (game Running + billing active), ordered pod teardown (BlankScreen + end billing + group cascade via group_session_members + log) on AC server disconnect
- Durable notification outbox with WhatsApp-to-screen OTP fallback chain, exponential backoff retry, and negative wallet balance RESIL-05 guard on both extension debit and session start
- Lap assist evidence (SHA-256 hash + pro/semi-pro/amateur tier), billing-session gate blocking manual entry, and assist_tier segmentation across all three leaderboard endpoints
- Customer session receipt with GST breakup and before/after balance, plus virtual walk-in queue with live position ETA and staff call/seat workflow
- 1. [Rule 3 - Blocking] shadcn CLI used base-nova style instead of new-york
- motion@12 animation library installed in both apps, JetBrains Mono font wired to web layout via next/font/google with --font-jb-mono CSS variable, both apps build clean
- Four leaf-node UI primitives: StatusBadge with racing flag colors, MetricCard KPI tile, context-based Toast notifications, and Skeleton/EmptyState loading states
- Commit:
- LiveDataTable<T>
- Rewrote `web/src/app/login/page.tsx`
- 1. [Rule 3 - Blocking] Added fleetHealth API method
- 1. [Rule 3 - Blocking] Adapted component prop names to match actual API
- Skeleton loading states and EmptyState components added to all 7 remaining dashboard pages, analytics root page created, zero deprecated colours
- Touch-optimized pod selection grid with offline count header, active:scale press feedback, remaining-time countdown, and zero hover-only content
- Commit:
- Status:
- One-liner:
- One-liner:
- rc-sentry `handle_crash()` (tier1_fixes.rs):
- BonoConfig struct in config.rs (relay_port=8099) + bono_relay.rs skeleton with BonoEvent/RelayCommand enums and 5 passing unit tests
- tokio broadcast event push loop + X-Relay-Secret Axum handler wired to AppState.bono_event_tx, 248 tests green
- bono_relay::spawn() wired into server startup with optional Tailscale second listener on :8099, and PodOnline/PodOffline events emitted from pod_monitor.rs at state-transition boundaries
- WinRM PowerShell fleet deploy script for Tailscale on 8 pods + server with canary-first rollout and placeholder guard rails
- racecontrol.toml [bono] section deployed to server with new binary; relay endpoint wired and ready for Tailscale enrollment
- 1. [Rule 3 - Blocking] FailureMonitorState lacks Serialize derive
- One-liner:
- knowledge_base.rs:
- 1. [Rule 1 - Bug] Fixed node_id shadowing in autonomous event branch
- 1. [Rule 3 - Blocking] Module declaration without modifying main.rs
- 1. [Rule 1 - Bug] EscalationPayload field mismatch
- 1. [Rule 2 - Missing] FleetEvent variants did not exist
- 1. [Rule 1 - Bug] Replaced .unwrap() with .single().expect()
- Billing validation gate and expanded double-launch guard in launch_game() with 4 TDD unit tests (LIFE-02, LIFE-04)
- Arm 15s blank_timer in SessionEnded handler + reset billing_active in BillingStopped handler (LIFE-03, LIFE-01 cleanup)
- SQLite model_evaluations table in mesh_kb.db with ModelEvalStore (open/migrate/insert/query) wired into tier_engine so every Fixed/FailedToFix AI diagnosis writes a persistent EVAL-01 record
- One-liner:
- 1. [Rule 1 - Bug] Fixed pre-existing borrow errors in ExperienceScoreReport handler
- One-liner:
- One-liner:
- One-liner:
- SQLite-backed ModelReputationStore persisting demotion/promotion decisions and 7-day accuracy counts across rc-agent restarts via mesh_kb.db
- End-to-end model reputation pipeline: rc-agent pushes ReputationPayload to server via WS after each sweep; GET /api/v1/models/reputation exposes per-model accuracy/status/cost sorted by accuracy DESC
- Weekly JSONL training data export pipeline using eval records + KB solutions in Ollama/Unsloth conversation format, firing every Sunday midnight IST
- Weekly WhatsApp report enhanced with per-model accuracy rankings, KB promotion count, Tier 1 rule cost savings, and improving/declining/stable trend labels — all sourced from Phases 290-292 SQLite stores.
- Structured CM diagnostics (cm_attempted, cm_exit_code, cm_log_errors, fallback_used) now flow from launch_ac() through LaunchResult to GameStateUpdate WebSocket messages
- Billing auto-pauses (PausedGamePause) when Race Engineer exhausts 2 relaunch attempts, kiosk shows structured CM/fallback diagnostics instead of raw error strings
- multiplayer.rs changes:
- PIN-gated coordinated AC launch (all pods start simultaneously when all members validate) and staff-toggleable continuous mode that auto-restarts races within 15s as long as any billing is active
- Per-pod join status tracking on kiosk dashboard with 'Join Failed' + 'Retry Join' button for failed multiplayer pods, and mid-session track/car config change for continuous mode between races
- Capitalization bug fixed in billing_rates seed (lowercase -> Title Case), test migrations extended with billing_rates table + seed assertion (exactly 3 rows), and PROTOC-01 serde alias round-trip test added — all 9 Phase 33 requirements have automated verification with 331+113 tests green
- HTTP 201/204 status code fixes for billing rate CRUD + 4 integration tests proving cache invalidation and cost exclusion (335 tests green)
- UIC-01 unit test added to test_format_cost() with 8 assertions; grep confirms zero rupee strings across all source trees (245 tests green)
- .wslconfig created with mirrored networking config — blocked at WSL2 install by BIOS AMD-V disabled on Ryzen 7 5800X
- Shared POSIX shell test library (lib/common.sh + lib/pod-map.sh) with pass/fail/skip/info/summary_exit helpers and pod IP map, refactored into all three existing E2E scripts
- @playwright/test 1.58.2 with bundled Chromium installed, playwright.config.ts at repo root (sequential/single-worker/reuseExistingServer), cargo-nextest 0.9.131 installed with .config/nextest.toml retry config
- 97 data-testid attributes added across three kiosk TSX files: 49 in book/page.tsx (customer wizard), 43 in SetupWizard.tsx (staff wizard), 5 in page.tsx (landing page)
- Playwright cleanup fixture (auto-runs pre-test) + 4-test smoke suite covering 3 kiosk routes and keyboard navigation, with pageerror capture and DOM snapshot on failure
- One-liner:
- Gate-based deploy verification script with binary swap check (rc-sentry :8091), EADDRINUSE port-conflict polling on :3300, fleet ws_connected/build_id consistency, and per-failure AI debugger log routing
- Single-entry E2E orchestrator (run-all.sh) runs all 4 test phases sequentially, gates on preflight failure, accumulates exit codes, writes timestamped summary.json, exits with total failure count
- Connection: close middleware on axum :8090 + UDP SO_REUSEADDR/non-inherit + OnceLock Ollama client + exec slots doubled to 8
- reqwest probe client connection pooling disabled via pool_max_idle_per_host(0), plus CLOSE_WAIT E2E verification script that checks all 8 pods via netstat over rc-agent /exec
- zero_force_with_retry(3, 100) on FfbController plus StartupReport extended with 4 #[serde(default)] boot verification fields wired to FleetHealthStore
- Conspit Ares wheelbase panic safety: FFB zeroed + lock screen error shown + crash logged + port-bind failures exit cleanly with observable BootVerification in StartupReport
- rp-debug Modelfile with all 14 diagnostic keywords + debug-memory.json seed script covering 7 deterministic AC/F1/fleet crash patterns
- Bash E2E test verifying rp-debug model presence and <5s response latency on all 8 pods via Ollama :11434 over :8090/exec
- One-liner:
- kiosk.rs additions:
- rc-agent autonomously detects and HTTP-ends orphaned billing sessions (5min configurable) + transitions pods to idle PinEntry "Ready" screen after session end instead of blank screen
- CrashRecoveryState enum (2-attempt billing-aware crash recovery) + 30s WS disconnection grace window so venue WiFi blips don't disturb active customer sessions
- 1. [Rule 1 - Bug] addr moved-into-closure borrow error in probe_tcp_port
- HTTP GET /api/v1/pods/{id}/self-test wired end-to-end: server dispatches RunSelfTest via WS, agent runs 22 probes + LLM verdict, SelfTestResult resolves pending oneshot, pod-health.sh verifies all 8 pods
- CLAUDE.md
- Sheets and Calendar MCP servers created with ESM + createRequire pattern; settings.json updated; OAuth re-auth checkpoint reached
- Node.js MCP server with 10 tools wrapping racecontrol REST API — Claude Code can now query fleet health, billing, sessions, and exec commands on pods directly from natural language
- RacingPoint-StagingHTTP
- `/rp:deploy-fleet` Claude Code skill with canary-first gate — Pod 8 deploy + verify.sh + explicit approval before 7-pod fleet rollout
- One-liner:
- rc-agent writes daily-rotating rc-agent-YYYY-MM-DD.jsonl files with pod_id field injected via span, enabling jq-based fleet-wide log aggregation across all 8 pods
- One-liner:
- Netdata deploy script (pod fleet via rc-agent :8090) and E2E verification script (9 hosts) created; MSI (154MB) staged at deploy-staging :9998
- WhatsApp P0 alerter with all-pods-offline + error-rate detection, Evolution API delivery, IST timestamps, rate limiting, and incident recording in SQLite
- Standalone Rust binary querying SQLite (read-only) for sessions, uptime, credits, and incidents -- generates branded HTML email sent via send_email.js to Uday every Monday
- fxm_reset + set_idle_spring + Clone + POWER_CAP_80_PERCENT added to FfbController with 6 unit tests verifying HID byte layout
- safe_session_end() async orchestrator wired to all 10 session-end sites — close ConspitLink (WM_CLOSE 5s) -> fxm.reset -> idlespring ramp 500ms -> restart CL with JSON verification
- 80% startup power cap wired via set_gain(80) at boot, hardware-validated on canary pod across all 4 games with correct wheel centering
- Hardened ConspitLink restart with crash-count tracking, JSON config backup/verify with auto-restore, and polling window minimize retry
- rc-agent startup self-heal: places Global.json at C:\RacingPoint\ forcing AresAutoChangeConfig=open, verifies GameToBaseConfig.json game mappings, and restarts ConspitLink only when config changed
- rc-agent c32d21e1 deployed to Pod 8 with verified Global.json (AresAutoChangeConfig=open) — canary hardware deploy complete, human-verify checkpoint auto-approved
- One-liner:
- Human physically verified on Pod 8 that ConspitLink 2.0 auto-loads AC preset on Assetto Corsa launch and switches to F1 25 preset on F1 25 launch — PROF-04 satisfied by attestation.
- Server .23 NIC pinned to 192.168.31.23 via static IP (PrefixOrigin: Manual, DHCP disabled, DNS corrected to 192.168.31.1) — DHCP reservation deferred due to TP-Link ARP conflict error
- Server Tailscale IP 100.71.226.83 documented; both Tailscale and LAN exec paths to rc-agent :8090 verified working with curl POST /exec returning Racing-Point-Server hostname
- Bono ExecHandler wired end-to-end — James sends exec_request via comms-link WebSocket, Bono executes via ExecHandler, James receives exec_result with stdout/stderr/exitCode. Code verified correct; live round-trip test deferred pending Bono VPS pull + restart.
- POST /relay/exec/send endpoint added to james/index.js, closing Gap 2b — James can now trigger exec_request to Bono's VPS via HTTP relay with generated execId
- TP-Link EX220 firmware bug permanently blocks server DHCP reservation; INFRA-01 satisfied by static IP alone; Bono deployment and exec round-trip deferred asynchronously
- One-liner:
- VenueConfigSnapshot struct + parse_config_snapshot() added to cloud racecontrol, wiring James config into AppState via /sync/push config_snapshot branch with 3 passing unit tests
- SwitchController protocol variant, failover_url config field, and HeartbeatStatus.last_switch_ms AtomicU64 with 5 passing unit tests
- Arc<RwLock<String>> active_url in reconnect loop, SwitchController URL-validated handler with last_switch_ms signal, and 60s self_monitor grace guard — full failover switching wired end-to-end
- One-liner:
- HTTP-triggered SwitchController broadcast via POST /api/v1/failover/broadcast with per-pod split-brain guard probing 192.168.31.23:8090/ping before URL switch
- Secondary watchdog in bono/index.js: detects venue power outage (James + server .23 both unreachable 5min) and auto-activates cloud racecontrol via pm2 + broadcasts SwitchController to pods
- Three ORCH-04 notification gaps closed: notify_failover registered in COMMAND_REGISTRY, Bono watchdog fixed to call sendEvolutionText directly, email added to both failover paths via stdlib-only send-email.js
- POST /api/v1/sync/import-sessions endpoint using INSERT OR IGNORE for lossless billing session failback after cloud-failover window
- One-liner:
- Feature-gated exec module in rc-common with run_cmd_sync (wait-timeout, stdlib-only) and run_cmd_async (tokio, behind feature gate), verified that rc-sentry tree has zero tokio references
- Fully hardened rc-sentry: timeout via rc_common::exec::run_cmd_sync, 64KB output truncation, concurrency cap at 4 with HTTP 429, Content-Length TCP read loop, and tracing structured logging replacing all eprintln!
- One-liner:
- One-liner:
- FfbBackend trait seam with mockall-generated mock, 8 passing unit tests — FFB controller now testable without HID hardware
- 6 async billing_guard tests verify BILL-02/BILL-03 actually send AgentMessage through mpsc after 60s/300s; 6 requirement-named failure_monitor tests trace CRASH-01/CRASH-02 to condition guards
- One-liner:
- Bundled 34 pre-loop agent variables into AppState struct in app_state.rs; all reconnect loop references updated to state.field pattern — enabling event_loop::run() to receive a single parameter in Plan 74-04.
- Extracted the 22-variant CoreToAgentMessage dispatch (~930 lines) from main.rs into ws_handler.rs with handle_ws_message(), WsTx type alias, HandleResult enum, and WS command semaphore/handler -- select! ws_rx arm reduced to 27-line delegation call.
- Extracted the 800-line inner select! loop from main.rs into event_loop.rs with ConnectionState struct bundling all 17 per-connection variables -- handle_ws_message() signature reduced from 18 to 8 parameters; main.rs reduced from 2037 to 1179 lines.
- Complete security posture baseline: 269 racecontrol + 11 rc-agent + 1 rc-sentry endpoints classified, 5 PII locations mapped, CORS/HTTPS/auth state documented, 12 risks prioritized
- Env var overrides for 6 secrets (JWT, terminal, relay, evolution, gmail credentials) with cryptographic JWT key auto-generation rejecting the dangerous hardcoded default
- Staff JWT middleware with strict/permissive variants and 4-tier route split protecting 172+ staff routes via expand-migrate-contract pattern
- Admin login endpoint with argon2id PIN hashing, spawn_blocking verification, and 12-hour staff JWT issuance
- Service key middleware on rc-agent :8090 with constant-time comparison (subtle crate), permissive mode when RCAGENT_SERVICE_KEY unset, /ping and /health remain public
- tower_governor rate limiting on 6 auth endpoints + SQLx transaction-wrapped token consumption with 7 new tests
- PIN login page, AuthGate route wrapper, and 15-minute idle timeout for the Next.js dashboard at :3200
- Switched staff_routes middleware from permissive (log-only) to strict (401 reject) -- contract step of expand-migrate-contract completing the phase goal
- Self-signed cert generation via rcgen with IP SAN for 192.168.31.23, RustlsConfig loader, and backward-compatible ServerConfig extension for dual-port TLS
- Dual-port HTTP/HTTPS server with tower-helmet security headers (CSP, HSTS 300s, X-Frame-Options DENY), HTTPS CORS, and protocol-aware kiosk API_BASE
- Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown.ps1 USB/accessibility/TaskMgr registry lockdown
- IP-based request source classification with pod-blocked staff routes and pod-accessible kiosk endpoints
- BillingStarted carries UUID session_token for kiosk unlock gating; KioskLockdown auto-pauses billing and sends debounced WhatsApp alert to Uday
- AES-256-GCM FieldCipher with deterministic HMAC-SHA256 phone hashing, env-var key loading, and AppState integration -- 10 unit tests green
- Encrypted PII columns in drivers table, all 9 phone lookups converted to HMAC hash, cloud sync encrypts before storing, 7 log statements redacted -- zero plaintext phone/OTP in logs or queries
- DPDP Act self-service data export (decrypted PII JSON) and cascade delete (21 child tables in transaction) behind customer JWT auth -- 8 unit tests green
- Append-only audit_log with action_type classification, log_admin_action() helper across 10 admin handlers, WhatsApp alerts on admin login/topup/fleet exec
- system_settings table tracks admin PIN age with 24h WhatsApp alert check; HMAC-SHA256 signing on outbound sync with permissive inbound verification
- Non-AC game crash auto-recovery via GameProcess::launch() + DashboardEvent::GameLaunchRequested + POST /api/v1/customer/game-request PWA endpoint
- One-liner:
- TOML deployment template and example config updated with all 6 game stanzas (correct Steam app IDs), full pipeline verified green (cargo test + release builds + next build), kiosk UI approved
- Per-game billing engine: GameState::Loading variant, PlayableSignal enum, BillingRateTier/BillingTimer sim_type fields, get_tiers_for_game() fallback logic, DB migration, and protocol sim_type wire-up
- Per-sim PlayableSignal dispatch + 30s exit grace timer in rc-agent ConnectionState: AC=shared memory, F1 25=UdpActive, others=90s process fallback
- Game column in admin billing rates table (SIM_TYPE_LABELS + inline select editor) and kiosk Loading state badge with count-up timer (amber, M:SS, resets on transition to on_track)
- 6 F1 25 unit tests added covering lap completion detection, sector split extraction, invalid lap flagging, session type mapping, first-packet safety, and take() semantics — 11 tests total, all green
- One-liner:
- IracingAdapter wired into rc-agent with IsOnTrack shared-memory billing trigger replacing the 90s process fallback
- LmuAdapter with rF2 fixed-struct shared memory reader: Scoring + Telemetry buffers, torn-read guard, sector splits via cumulative field derivation, first-packet safety, session transition reset, and 6 unit tests
- LmuAdapter wired into rc-agent: SimType::LeMansUltimate creates adapter in main.rs, dedicated PlayableSignal arm in event_loop.rs replaces 90s process fallback with rF2 shared memory IsOnTrack
- AssettoCorsaEvoAdapter with warn-once zero-guard shared memory reads — AC1 struct offsets reused, graceful degradation when EVO Early Access SHM is absent or empty
- Cross-game track name normalization via TRACK_NAME_MAP + sim_type-scoped personal_bests and track_records PRIMARY KEYs with idempotent v2-table SQLite migration
- Optional sim_type query param added to all 4 leaderboard endpoints with available_sim_types discovery array and per-record sim_type field in responses
- One-liner:
- One-liner:
- Psychology engine fully integrated: badges and streaks auto-fire on every session end, dispatcher starts on boot, 5 seed badges in DB, and 5 API endpoints expose psychology data to staff
- One-liner:
- PbAchieved DashboardEvent variant
- Four retention functions in psychology.rs (PB rivalry nudges, surprise credits, streak-at-risk warnings, loss-framed membership expiry) wired from lap_tracker/billing/scheduler with variable_reward_log cap table and extended passport API
- Passport page streak card enhanced with red border + days-remaining countdown when grace period within 7 days, and longest-streak motivational context
- One-liner:
- Commit (whatsapp-bot):
- Status:
- Status:
- Status:
- Status:
- Three AgentMessage/CoreToAgentMessage enum variants added to rc-common + PreflightConfig struct wired into AgentConfig enabling Plan 02 pre-flight logic to compile
- Concurrent pre-flight check runner (HID, ConspitLink, orphan game) with auto-fix + billing_active.store(true) moved inside pre-flight Pass branch so customers are never charged on a maintenance-blocked pod
- MaintenanceRequired LockScreenState variant with branded Racing Red HTML renderer, in_maintenance AtomicBool on AppState, and ws_handler wiring to show maintenance screen on pre-flight failure and clear on ClearMaintenance
- DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) wired into 5-check concurrent runner, plus 30-second maintenance retry select! arm that auto-clears in_maintenance on Pass
- 4 new pre-flight checks (billing_stuck, disk_space, memory, ws_stability) wired into 9-way tokio::join! runner; run() signature extended with ws_connect_elapsed_secs; both call sites updated
- 60s cooldown on PreFlightFailed WS alerts via Option<Instant> on AppState; lock screen + maintenance flag always fire; retry loop confirmed no-alert by design

---

## v32.0 Autonomous Meshed Intelligence (Shipped: 2026-04-01)

**Phases completed:** 7 phases, 6 plans, 9 tasks

**Key accomplishments:**

- 1. [Rule 3 - Blocking] FailureMonitorState lacks Serialize derive
- One-liner:
- knowledge_base.rs:
- 1. [Rule 1 - Bug] Fixed node_id shadowing in autonomous event branch
- 1. [Rule 3 - Blocking] Module declaration without modifying main.rs
- 1. [Rule 1 - Bug] EscalationPayload field mismatch
- 1. [Rule 2 - Missing] FleetEvent variants did not exist
- 1. [Rule 1 - Bug] Replaced .unwrap() with .single().expect()

---

## v25.0 Debug-First-Time-Right (Shipped: 2026-03-26)

**Phases completed:** 147 phases, 330 plans, 75 tasks

**Key accomplishments:**

- (none recorded)

---

## v17.1 Watchdog-to-AI Migration (Shipped: 2026-03-25)

**Phases completed:** 119 phases, 270 plans, 66 tasks

**Key accomplishments:**

- (none recorded)

---

## v21.0 Cross-Project Sync & Stabilization (Shipped: 2026-03-23)

**Phases completed:** 107 phases, 244 plans, 49 tasks

**Key accomplishments:**

- (none recorded)

---

## v16.1 Camera Dashboard Pro (Shipped: 2026-03-22)

**Phases completed:** 87 phases, 197 plans, 37 tasks

**Key accomplishments:**

- (none recorded)

---

## v11.0 Agent & Sentry Hardening (Shipped: 2026-03-21)

**Phases completed:** 34 phases, 85 plans, 13 tasks

**Key accomplishments:**

- (none recorded)

---

## v10.0 Connectivity & Redundancy (Shipped: 2026-03-21)

**Phases completed:** 34 phases, 85 plans, 13 tasks

**Key accomplishments:**

- (none recorded)

---

## v1.0 RaceControl HUD & Safety (Shipped: 2026-03-13)

**Phases completed:** 5 phases, 15 plans, 16 tasks

**Key accomplishments:**

- Escalating watchdog backoff (30s→2m→10m→30m) with post-restart verification and email alerts — pods self-heal without manual intervention
- WebSocket keepalive (15s WS ping + 30s app-level Ping/Pong) + fast-then-backoff reconnect — no more "Disconnected" flash during game launch
- DeployState FSM with HEAD-before-kill validation, canary-first (Pod 8), and session-aware rolling deploy — deployments work reliably across all 8 pods
- Blanking screen protocol: lock-screen-before-kill ordering, LaunchSplash branded screen, extended dialog suppression — customers never see system internals
- PIN auth unification (validate_pin_inner + PinSource enum) + pod lockdown (taskbar hidden, Win key blocked) — consistent, locked-down customer experience
- Config validation with branded error screen + deploy template fix — rc-agent fails fast on bad config instead of silently running with zero billing rates

## v2.0 Kiosk URL Reliability (Shipped: 2026-03-14)

**Phases completed:** 6 phases, 12 plans

**Key accomplishments:**

- Server IP pinned to .23 via DHCP reservation + racecontrol reverse proxy for kiosk
- Pod lock screens show branded "Connecting..." state — never browser error pages
- Edge auto-update, StartupBoost, BackgroundMode disabled on all 8 pods
- Staff dashboard: one-click lockdown toggle, power management (restart/shutdown/wake) per-pod and bulk
- Customer experience: Racing Point branding on lock/blank screens, session results display, staff-configurable wallpaper

## v3.0 Leaderboards & Competitive (Shipped: 2026-03-15)

**Phases completed:** 3 phases (12, 13, 13.1), 10 plans

**Key accomplishments:**

- SQLite lap times DB with full migration stack — persistent leaderboards across server restarts
- Live leaderboard with top-10 per track/car, personal bests, percentile ranks, gap-to-leader — served via REST API
- Pod fleet reliability: ConspitLink USB watchdog, process cleanup on session end, telemetry UDP failover

## v4.0 Pod Fleet Self-Healing (Shipped: 2026-03-16)

**Phases completed:** 7 phases (16–22), 15 plans

**Key accomplishments:**

- Firewall auto-config: racecontrol opens its own Windows Firewall rule at startup — no manual setup on new server
- WebSocket exec relay: staff can run shell commands on any pod from kiosk (WS proxy through rc-agent)
- Startup self-healing: rc-agent detects and clears stale CLOSE_WAIT sockets, restarts on crash detection
- Watchdog service: server monitors all pods, alerts on missed heartbeats, auto-requeues stuck recovery tasks
- Deploy resilience: deploy_pod.py with canary-first (Pod 8), binary size verification, pod-agent /write endpoint
- Fleet health dashboard: live pod grid with status badges, per-pod metrics, bulk restart/lockdown actions
- Pod 6/7/8 recovery: WinRM-free remote deploy via pod-agent, PowerShell EncodedCommand for firewall/binary fixes

## v4.5 AC Launch Reliability (Shipped: 2026-03-16)

**Phases completed:** 5 phases, 10 plans, 19 requirements

**Key accomplishments:**

- Billing-game lifecycle wired: game killed on billing end, launch gate before billing, anti-double-launch guard, pod reset after session
- Game crash recovery: auto-pause billing on crash, "Game Crashed" badge + "Relaunch" on kiosk dashboard
- Launch resilience: structured `LaunchDiagnostics` (CM exit code, log errors, fallback flag), billing auto-pause on total launch failure
- Multiplayer server lifecycle: `AcServerManager` wired to billing — server auto-starts on booking, auto-stops when all billing ends; kiosk self-serve "Play with Friends" booking with per-pod PINs
- Synchronized group play: coordinated launch (all pods receive `LaunchGame` simultaneously when all PINs validated), continuous race mode with billing-active guard, "Join Failed" + retry on kiosk dashboard, mid-session track/car change for continuous mode

## v5.0 RC Bot Expansion (Shipped: 2026-03-16)

**Phases completed:** 5 phases (23–27), 19 plans, 19 requirements

**Key accomplishments:**

- `PodFailureReason` enum (9 classes) + 5 `AgentMessage` bot variants + `is_pod_in_recovery()` guard — deterministic failure taxonomy shared across all bot tasks (Phase 23)
- Game freeze, launch timeout, USB wheelbase disconnect auto-fix bots with FFB zero-force safety ordering before every process kill (Phase 24)
- `billing_guard.rs` detects stuck sessions (60s) and idle drift (5min); `bot_coordinator.rs` routes to safe `end_session()` with CRDT cloud sync fence (Phase 25)
- Lap validity filter + per-track floor (Monza/Silverstone/Spa) + `review_required` flag; customer/staff PIN counters structurally separated; TELEM-01 email alert + MULTI-01 ordered teardown (Phase 26)
- Tailscale mesh installed on all pods + `bono_relay.rs` bidirectional event/command relay to Bono's VPS on port 8099 (Phase 27)

## v5.5 Billing Credits (Shipped: 2026-03-17)

**Phases completed:** 3 phases (33–35)

**Key accomplishments:**

- billing_rates DB table with non-retroactive additive algorithm + in-memory rate cache
- Four CRUD endpoints for staff rate management
- Every user-facing screen replaced rupees with credits

---

## Comms Link v1.0 — James-Bono Communication (Shipped: 2026-03-12)

**Phases completed:** 8 phases, 14 plans (100%)
**Repo origin:** comms-link (merged into unified .planning/)
**Archive:** archive/comms-link-v1.0/

**Key accomplishments:**

- Persistent WebSocket connection from James → Bono's VPS with PSK auth (Bearer header, not query params)
- Auto-reconnect with queue flush on re-establish — only reconnects on network drops, not auth rejection
- Heartbeat sender with CPU/memory/Claude detection metrics every N seconds
- ClaudeWatchdog: detects Claude Code crash, 2s kill→spawn delay, 3s post-spawn verification, cooldown DI
- Watchdog hardening: cooldown reset policy, ESM isMainModule detection, wireRunner() DI pattern
- Alert system: fixed-window cooldown suppression, email fallback to both usingh and bono, WhatsApp via Evolution API
- LOGBOOK.md bidirectional sync with atomic writes, conflict detection, and ack-based content tracking
- Daily coordination: HealthAccumulator snapshots, IST-windowed daily summary scheduler, pod status HTTP fetch, FAILSAFE retirement

## Comms Link v2.0 — Reliable AI-to-AI Communication (Shipped: 2026-03-20)

**Phases completed:** 6 phases (9–14), 14 plans, 34 requirements, 437 tests
**Repo origin:** comms-link (merged into unified .planning/)
**Archive:** archive/comms-link-v2.0/

**Key accomplishments:**

- ACK protocol with monotonic sequence numbers + exponential backoff retries (3 max)
- WAL-backed message queue — messages survive process crash, replay on restart
- DeduplicatorCache (1000 IDs, 1hr TTL) prevents double-processing on reconnect
- Process supervisor with health check polling, PID lockfile, Task Scheduler watchdog-of-watchdog
- Bidirectional task routing with correlation IDs + configurable timeout (5min default)
- 13-command remote execution with 3-tier approval (auto/notify/approve), array-args only, sanitized env
- MetricsCollector + GET /relay/metrics endpoint (uptime, reconnects, ACK latency, queue depth)
- ConnectionMode state machine: REALTIME > EMAIL_FALLBACK > OFFLINE_QUEUE graceful degradation
- INBOX.md demoted to human-readable audit log — no code reads it programmatically

## Cloud Services v1.0 — Bots & API Gateway (Shipped: 2026-03-07)

**Commits:** 9 across 4 repos | **Owner:** Bono (VPS)
**Repos:** racingpoint-api-gateway, racingpoint-whatsapp-bot, racingpoint-discord-bot, racingpoint-google

**Key accomplishments:**

- API Gateway (Express.js): bookings, customers, calendar, proxy routes merging bot data with racecontrol REST API
- WhatsApp bot: automated session booking with Google Calendar, Claude API migration, 2-min direct mode fallback, RaceControl session booking for registered customers
- Discord bot: modal form booking with Google Calendar integration
- Google shared services: Calendar attendees support, Gmail replyEmail, OAuth token refresh utility
- James-Bono instant comms link + brainstorm daemon v2 + comms cron scripts (predecessor to comms-link repo)

## Admin Dashboard v1.0 — Staff Operations Panel (Shipped: 2026-03-08)

**Commits:** 14 | **Owner:** Bono (VPS) + James (remote pushes)
**Repo:** racingpoint-admin (Next.js/TypeScript)

**Key accomplishments:**

- Full admin dashboard: cafe, inventory, sales, purchases, finance modules
- Receipt scanner + bank statement matching + analytics
- Waivers page with signature viewing (Google Sheets integration)
- HR section + hiring nav + marketing admin pages
- Racing Point brand identity rebrand (Racing Red #E10600)
- Docker support + admin chat wired directly to rc-core
- Kiosk control page: per-pod screen blanking toggles, health check
- Sessions page with staff attribution + daily summary
- Wallet log page with Transaction ID column
- Server-side proxy /api/rc/[...path] for rc-core API calls

## Ops Toolkit v1.0 — Fleet Management CLI (In Progress, started 2026-03-17)

**Phases:** 5 planned, 1 complete (Phase 1: Foundation & Safety)
**Repo origin:** deploy-staging (ops.bat + ops.conf.bat)
**Requirements:** 27 (FNDN-01..06, PDOP-01..11, SVOP-01..06, BUILD-01..04)

**Key accomplishments (Phase 1):**

- ops.bat interactive menu + CLI mode dual entry point
- ops.conf.bat centralizes all pod IPs, server IP, paths, ports — zero hardcoded values
- Safety blocklist prevents running pod binaries on James's machine
- CRLF enforcement, delayed expansion, hostname-based safety guard
- Stub routing for all Phase 2-5 commands (incremental build-out)

**Remaining:**

- Phase 2: Health & Status — read-only fleet visibility
- Phase 3: Pod Deploy & Operations — deploy, restart, canary, screenshot
- Phase 4: Server Operations — racecontrol, kiosk, pod-agent service management
- Phase 5: Build & Polish — cargo build automation, colored output

## AC Launcher v1.0 — Full AC Launch Experience (Shipped: 2026-03-14)

**Phases completed:** 9 phases, 20 plans (100%)
**Repo origin:** ac-launcher (code in racecontrol, merged into unified .planning/)
**Archive:** archive/ac-launcher-v1.0/

**Key accomplishments:**

- Session types (Practice, Race, Hotlap) with mode validation — only valid AC combinations presented
- 5 difficulty tiers (Rookie→Alien) mapping to AI strength, aggression, and behavior
- Billing synchronized to in-game session start (not game launch), DirectX init delay handled
- Safety enforcement: 100% grip, 0% damage always applied — non-negotiable
- Content validation: invalid track/car/session combos filtered before customer sees them
- Mid-session controls: transmission, FFB, ABS, TC, stability all adjustable during gameplay
- Curated presets: popular car/track/session packages for quick selection
- Staff/PWA integration: QR/PIN triggers correct AC session, staff configures via kiosk
- Multiplayer enhancement: AC server preset management, lobby enrichment with track/car/AI info
