---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 105-02-PLAN.md
last_updated: "2026-03-21T11:50:09.892Z"
progress:
  total_phases: 68
  completed_phases: 51
  total_plans: 129
  completed_plans: 127
  percent: 92
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 93-02-PLAN.md
last_updated: "2026-03-21T09:35:00.648Z"
progress:
  [█████████░] 92%
  completed_phases: 48
  total_plans: 117
  completed_plans: 115
  percent: 98
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 93-02-PLAN.md
last_updated: "2026-03-21T09:07:11.301Z"
progress:
  [██████████] 98%
  completed_phases: 84
  total_plans: 209
  completed_plans: 206
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 100-02-PLAN.md
last_updated: "2026-03-21T06:24:44Z"
last_activity: "2026-03-21 -- 100-02 complete: Fleet page Racing Red Maintenance badge, PIN-gated modal with failure list, Clear Maintenance button calling POST /pods/{id}/clear-maintenance (STAFF-01, STAFF-02)"
progress:
  [██████████] 96%
  total_phases: 65
  completed_phases: 41
  total_plans: 108
  completed_plans: 104
  percent: 96
decisions:

  - "PIN gate accepts any 4-digit input for maintenance modal — casual venue TV protection; actual security is JWT-protected clear-maintenance endpoint"
  - "maintenance check runs first in statusBorder/statusLabel/statusLabelColor so in_maintenance=true always overrides WS/HTTP status visuals"

---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: in_progress
stopped_at: Completed 85-01-PLAN.md
last_updated: "2026-03-21T07:00:00.000Z"
last_activity: "2026-03-21 -- 85-01 complete: LmuAdapter with rF2 shared memory (Scoring + Telemetry), torn-read guard, sector splits (cumulative derivation), first-packet safety, session transition reset, 6 unit tests (TEL-LMU-01, TEL-LMU-02, TEL-LMU-03)"
progress:
  [██████████] 96%
  completed_phases: 40
  total_plans: 107
  completed_plans: 103
  percent: 96
decisions:

  - "clear_on_disconnect() clears in_maintenance=false because offline pods are not in maintenance from the server's perspective"
  - "Optimistic server-side clear on clear_maintenance_pod() for instant staff visual feedback without waiting for PreFlightPassed roundtrip"
  - "sector_times_ms() uses .round() not truncation — (42.3-20.1)*1000 = 22199.99 would truncate to 22199 instead of 22200"
  - "pub mod lmu registered in Plan 01 (not Plan 02) — required for cargo test sims::lmu compilation"

---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 84-01-PLAN.md
last_updated: "2026-03-21T05:27:15.878Z"
last_activity: "2026-03-21 -- 84-01 complete: IracingAdapter with shared memory, dynamic variable lookup, double-buffer tick-lock, session transition detection, pre-flight app.ini check, 8 unit tests (TEL-IR-01, TEL-IR-02, TEL-IR-03, TEL-IR-04)"
progress:
  total_phases: 65
  completed_phases: 39
  total_plans: 104
  completed_plans: 100
  percent: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 99-02-PLAN.md
last_updated: "2026-03-21T05:25:55.665Z"
last_activity: "2026-03-21 -- 83-01 complete: 6 F1 25 unit tests added (lap completion, sector splits, invalid lap flag, session type mapping, first-packet safety, take semantics) — TEL-F1-01, TEL-F1-02, TEL-F1-03 verified"
progress:
  [██████████] 96%
  completed_phases: 39
  total_plans: 104
  completed_plans: 99
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 99-01-PLAN.md
last_updated: "2026-03-21T05:20:01.351Z"
last_activity: "2026-03-21 -- 83-01 complete: 6 F1 25 unit tests added (lap completion, sector splits, invalid lap flag, session type mapping, first-packet safety, take semantics) — TEL-F1-01, TEL-F1-02, TEL-F1-03 verified"
progress:
  [██████████] 95%
  completed_phases: 38
  total_plans: 104
  completed_plans: 98
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 98-02-PLAN.md
last_updated: "2026-03-21T04:56:25.218Z"
last_activity: "2026-03-21 -- 98-02 complete: DISP-01 HTTP probe (127.0.0.1:18923) + DISP-02 GetWindowRect (Chrome_WidgetWin_1) in pre_flight.rs (5 concurrent checks) + 30s maintenance retry select! arm in event_loop.rs (PF-06, DISP-01, DISP-02)"
progress:
  total_phases: 65
  completed_phases: 38
  total_plans: 102
  completed_plans: 97
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 98-01-PLAN.md
last_updated: "2026-03-21T04:47:06.000Z"
last_activity: "2026-03-21 -- 98-01 complete: MaintenanceRequired LockScreenState variant + in_maintenance AtomicBool on AppState + ClearMaintenance ws_handler (PF-04, PF-05)"
progress:
  [██████████] 95%
  completed_phases: 37
  total_plans: 100
  completed_plans: 96
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 83-01-PLAN.md
last_updated: "2026-03-21T04:32:00.084Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 65
  completed_phases: 37
  total_plans: 98
  completed_plans: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-02-PLAN.md
last_updated: "2026-03-21T04:20:27.168Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 65
  completed_phases: 36
  total_plans: 97
  completed_plans: 94
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-02-PLAN.md
last_updated: "2026-03-21T06:33:11.251Z"
last_activity: "2026-03-21 -- 97-02 complete: pre_flight.rs concurrent check runner (HID, ConspitLink, orphan game) + ws_handler pre-flight gate with billing_active.store(true) inside Pass branch (PF-01, PF-02, PF-03, HW-01, HW-02, HW-03, SYS-01)"
progress:
  total_phases: 75
  completed_phases: 73
  total_plans: 186
  completed_plans: 185
  percent: 97
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 82-03-PLAN.md
last_updated: "2026-03-21T04:11:01.247Z"
last_activity: "2026-03-21 -- 97-01 complete: PreFlightPassed + PreFlightFailed AgentMessage variants + ClearMaintenance CoreToAgentMessage variant + PreflightConfig struct wired into AgentConfig (PF-07)"
progress:
  [██████████] 97%
  completed_phases: 29
  total_plans: 82
  completed_plans: 81
  percent: 99
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 97-01-PLAN.md
last_updated: "2026-03-21T04:06:13.920Z"
last_activity: "2026-03-21 -- 80-02 complete: PIN rotation alerting (system_settings + 24h WhatsApp check) + HMAC-SHA256 cloud sync signing/verification in permissive mode (ADMIN-06, AUTH-07)"
progress:
  [██████████] 99%
  completed_phases: 29
  total_plans: 82
  completed_plans: 80
  percent: 95
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 97 context gathered
last_updated: "2026-03-21T03:40:37.614Z"
last_activity: "2026-03-21 -- 79-02 complete: PII encryption migration, 9 phone queries use phone_hash, 7 log statements redacted, cloud sync encrypts (DATA-01, DATA-02, DATA-03)"
progress:
  [██████████] 95%
  completed_phases: 28
  total_plans: 80
  completed_plans: 77
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 79-02-PLAN.md
last_updated: "2026-03-21T03:03:57.288Z"
last_activity: 2026-03-21 — Milestone v11.1 Pre-Flight Session Checks started
progress:
  total_phases: 57
  completed_phases: 28
  total_plans: 75
  completed_plans: 76
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 82 context gathered
last_updated: "2026-03-21T02:59:19.931Z"
progress:
  [██████████] 100%
  completed_phases: 27
  total_plans: 75
  completed_plans: 75
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 79-03-PLAN.md
last_updated: "2026-03-21T02:56:24Z"
last_activity: "2026-03-21 -- 79-03 complete: DPDP data export (decrypted PII JSON) + cascade delete (21 child tables in transaction) with 8 unit tests (DATA-04, DATA-05)"
progress:
  total_phases: 57
  completed_phases: 27
  total_plans: 75
  completed_plans: 75
  percent: 100
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 89-03-PLAN.md
last_updated: "2026-03-21T02:29:20.428Z"
progress:
  [██████████] 100%
  completed_phases: 65
  total_plans: 177
  completed_plans: 170
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 81 UI-SPEC approved
last_updated: "2026-03-21T01:22:08.465Z"
last_activity: "2026-03-21 -- 78-03 complete: BillingStarted session_token + KioskLockdown auto-pause billing + debounced WhatsApp alert (SESS-04, SESS-05)"
progress:
  total_phases: 53
  completed_phases: 28
  total_plans: 80
  completed_plans: 72
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 74-01-PLAN.md
last_updated: "2026-03-21T01:06:56.526Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  total_phases: 53
  completed_phases: 27
  total_plans: 76
  completed_plans: 70
  percent: 92
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-03-PLAN.md
last_updated: "2026-03-21T01:01:53.983Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  [█████████░] 92%
  completed_phases: 27
  total_plans: 76
  completed_plans: 69
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 74-01-PLAN.md
last_updated: "2026-03-21T01:01:17.895Z"
last_activity: "2026-03-21 -- 78-01 complete: Edge kiosk hardened with 12 security flags + keyboard hook blocks (F12/Ctrl+Shift+I/J/Ctrl+L) + pod-lockdown USB/accessibility/TaskMgr lockdown (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)"
progress:
  [█████████░] 91%
  completed_phases: 26
  total_plans: 76
  completed_plans: 68
  percent: 89
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-02-PLAN.md
last_updated: "2026-03-21T01:00:37.615Z"
last_activity: "2026-03-21 -- 69-02 complete: failover_broadcast endpoint + split-brain guard in rc-agent SwitchController (ORCH-02, ORCH-03)"
progress:
  [█████████░] 89%
  completed_phases: 26
  total_plans: 76
  completed_plans: 67
  percent: 88
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Phase 81 context gathered
last_updated: "2026-03-21T00:56:28.952Z"
last_activity: "2026-03-20 -- 77-02 complete: dual-port HTTPS 8443 + tower-helmet security headers + protocol-aware kiosk API_BASE (TLS-01, TLS-03, TLS-04, KIOSK-06)"
progress:
  [█████████░] 88%
  completed_phases: 26
  total_plans: 76
  completed_plans: 66
  percent: 87
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 69-01-PLAN.md
last_updated: "2026-03-21T00:56:00.000Z"
last_activity: "2026-03-21 -- 69-01 complete: HealthMonitor FSM (12-tick/60s hysteresis) + FailoverOrchestrator (activate_failover -> exec_result -> broadcast -> notify) wired into james/index.js (HLTH-01, HLTH-02, HLTH-03, ORCH-01, ORCH-04)"
progress:
  [█████████░] 87%
  completed_phases: 26
  total_plans: 76
  completed_plans: 65
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 77-01-PLAN.md
last_updated: "2026-03-20T14:22:16.463Z"
last_activity: "2026-03-20 -- 77-01 complete: TLS foundation with rcgen cert gen, RustlsConfig loader, ServerConfig extension (TLS-02, TLS-04)"
progress:
  total_phases: 45
  completed_phases: 26
  total_plans: 70
  completed_plans: 64
  percent: 91
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 77-01-PLAN.md
last_updated: "2026-03-20T14:18:04.350Z"
last_activity: "2026-03-20 -- 77-01 complete: TLS foundation with rcgen cert gen, RustlsConfig loader, ServerConfig extension (TLS-02, TLS-04)"
progress:
  [█████████░] 91%
  completed_phases: 25
  total_plans: 70
  completed_plans: 63
  percent: 90
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 68-01-PLAN.md
last_updated: "2026-03-20T14:02:05.654Z"
last_activity: "2026-03-20 -- 76-06 complete: strict JWT enforcement on 172 staff routes (AUTH-01, AUTH-02, AUTH-03, SESS-01)"
progress:
  [█████████░] 90%
  completed_phases: 24
  total_plans: 67
  completed_plans: 61
---

---
gsd_state_version: 1.0
milestone: v6.0
milestone_name: Salt Fleet Management
status: completed
stopped_at: Completed 76-06-PLAN.md
last_updated: "2026-03-20T13:38:47.349Z"
last_activity: "2026-03-20 -- 76-06 complete: strict JWT enforcement on 172 staff routes (AUTH-01, AUTH-02, AUTH-03, SESS-01)"
progress:
  total_phases: 45
  completed_phases: 24
  total_plans: 62
  completed_plans: 60
  percent: 97
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-21)

**Core value:** Customers see their lap times, compete on leaderboards, and compare telemetry
**Current focus:** Phase 93 — Community & Tribal Identity

## Current Position

Phase: 93 (Community & Tribal Identity) — EXECUTING
Plan: 1 of 2

## Phase Map -- v11.0 Agent & Sentry Hardening

| Phase | Name | Requirements | Status |
|-------|------|--------------|--------|
| 71 | rc-common Foundation + rc-sentry Core Hardening | SHARED-01..03, SHARD-01..05 | Complete (2/2 plans done) |
| 72 | rc-sentry Endpoint Expansion + Integration Tests | SEXP-01..04, SHARD-06, TEST-04 | Complete (2/2 plans done) |
| 73 | Critical Business Tests | TEST-01, TEST-02, TEST-03 | Complete (2/2 plans done) |
| 74 | rc-agent Decomposition | DECOMP-01..04 | In Progress (3/4 plans done) |

**Phase 71:** rc-common exec.rs with feature gate (SHARED) + rc-sentry timeout, truncation, concurrency cap, partial read fix, structured logging (SHARD). No rc-agent changes. Verify `cargo tree -p rc-sentry` shows no tokio after every rc-common change.
**Phase 72:** rc-sentry endpoint expansion (/health, /version, /files, /processes, graceful shutdown) + TcpStream-based integration tests on ephemeral port.
**Phase 73:** billing_guard + failure_monitor unit tests using watch channel injection / mockall; FfbBackend trait seam. MUST complete before Phase 74 (Refactor Second rule).
**Phase 74:** config.rs -> app_state.rs -> ws_handler.rs -> event_loop.rs extraction in strict dependency order. select! dispatch body (event_loop) deferred to v12.0 if risk is too high.

## Key Constraints for This Milestone

- rc-sentry MUST stay stdlib-only -- never add tokio. Run `cargo build --bin rc-sentry` after every rc-common change
- rc-common feature gate: default = sync (rc-sentry uses this), "async-exec" = tokio (rc-agent uses this)
- wait-timeout 0.2 is the only correct stdlib-compatible child process timeout on Windows
- mockall 0.13 goes in rc-agent dev-dependencies only (MSRV 1.77, project at 1.93.1)
- event_loop.rs extraction is the highest regression risk -- protect with Phase 73 tests first
- select! variable extraction uses ConnectionState struct pattern, never Arc<Mutex<T>> for local variables

## Performance Metrics

**Velocity (recent):**

- Phase 56 P01: 494 min | Phase 56 P02: 3 min | Phase 57 P01-03: ~35 min total
- Average recent plan: ~15 min

**Updated after each plan completion**

## Accumulated Context

### Roadmap Evolution

- Phase 106 added: Structured Log Labels — Add [build_id][module] prefix to all rc-agent tracing output

### Decisions (v11.0)

- Build order: rc-common extraction -> rc-sentry hardening -> rc-agent decomposition (rc-common unblocks both sentry and agent)
- Phase 71 combines rc-common extraction + rc-sentry core hardening (they must be co-developed to validate the feature gate)
- Phase 73 (tests) precedes Phase 74 (decomposition) -- Refactor Second is a non-negotiable standing rule
- FfbBackend trait is the correct approach over #[cfg(test)] stubs -- cleaner seam, decided before writing any FFB tests
- SHARD-04 (partial TCP read fix) added to Phase 71 -- correctness issue distinct from timeout/truncation, flagged by research
- 71-01: wait-timeout = 0.2 for stdlib-only child process timeout; tokio optional dep with feature gate prevents rc-sentry contamination (SHARED-01..03 complete)
- 71-01: Truncation on Vec<u8> before String::from_utf8_lossy to prevent char boundary panics in exec.rs
- 75-01: 269 racecontrol routes classified into 5 tiers; 172 staff/admin routes have zero auth (CRITICAL); OTP plaintext in logs elevated to CRITICAL
- 75-01: rc-agent /exec and rc-sentry TCP flagged CRITICAL -- arbitrary command execution with zero auth on LAN
- 71-02: SlotGuard Drop impl ensures EXEC_SLOTS decremented even on panic -- prevents 429 lockout (SHARD-01..05 complete)
- 71-02: THREAD_COUNTER separate from EXEC_SLOTS -- EXEC_SLOTS=live connections, THREAD_COUNTER=monotonic spawn IDs
- 75-02: rand 0.8 thread_rng().r#gen() for JWT key gen (gen is Rust 2024 reserved keyword); RACECONTROL_* env var naming for all secrets
- 75-02: default_jwt_secret() kept for serde backward compat; resolve_jwt_secret() catches dangerous default at runtime
- 76-03: subtle crate for constant-time service key comparison on rc-agent; permissive mode when RCAGENT_SERVICE_KEY unset; /ping and /health remain public
- 66-05: INFRA-01 complete via static IP alone -- TP-Link EX220 firmware bug (Error 5024) persists ARP entries in NVRAM across reboots, permanently blocking DHCP reservation for server .23; reservation is "won't fix" for this router model, add if factory-reset or replaced
- 72-01: build.rs copied from rc-agent for GIT_HASH embedding; winapi 0.3 consoleapi-only for SetConsoleCtrlHandler; non-blocking accept loop polls SHUTDOWN_REQUESTED every 10ms for graceful drain (SEXP-01..04, SHARD-06 complete)
- 72-02: inline #[cfg(test)] module with incoming().take(N) for clean thread exit; ephemeral ports via 127.0.0.1:0; all 7 tests pass with zero tokio contamination (TEST-04 complete)
- 66-05: Bono deployment (exec round-trip) deferred async via INBOX.md; INFRA-03 code complete on both sides, live verification pending Bono pm2 restart
- 67-01: Allowlist approach for sanitizer (only venue/pods/branding) -- never denylist; httpPost used for relay/sync POST for consistency; RACECONTROL_TOML_PATH env var for configurable path (SYNC-01, SYNC-02 complete)
- 67-02: parse_config_snapshot extracted as pub(crate) fn for testability -- sync_push calls it rather than inlining (SYNC-03 complete)
- 67-02: config_snapshot uses total += 1 (single record semantics, not per-field) -- consistent with other upserts
- 67-02: Structured tracing on config_snapshot receipt: venue name, pod count, hash prefix (first 8 chars)
- 76-02: argon2 0.5 with Argon2id default params for admin PIN hashing; spawn_blocking for CPU-heavy verification; 503 when no hash configured; 12h JWT expiry (shift-length limit)
- 76-04: tower_governor 0.8 with PeerIpKeyExtractor for per-IP rate limiting; into_make_service_with_connect_info for ConnectInfo; SQLx transaction wraps validate_pin token lifecycle
- 76-04: Bot wallet check (AUTH-05) already existed; billing is deferred (in-memory), not DB -- TOCTOU mitigated by optimistic locking
- 73-01: FfbBackend trait uses FfbController::method(self) fully-qualified delegation to avoid infinite recursion when trait and inherent method names match; mockall mock tests added inside existing test module; tokio test-util added to dev-deps to fix pre-existing billing_guard compilation (TEST-03 complete)
- 73-02: tokio::time::Instant required (not std::time::Instant) for billing_guard debounce timers -- mock clock only controls tokio::time::* functions; yield_now x5 before first advance() lets spawned task start and register interval before clock moves (TEST-01, TEST-02 complete)
- 76-05: JWT in localStorage with client-side expiry check; AuthGate skips /login pathname to avoid redirect loop; fetchApi auto-clears token + redirects on 401; useIdleTimeout listens to 5 event types with passive listeners (ADMIN-01, ADMIN-03 complete)
- 76-06: One-line swap from require_staff_jwt_permissive to require_staff_jwt on staff sub-router; contract step of expand-migrate-contract; kept permissive variant for rollback (AUTH-01, AUTH-02, AUTH-03, SESS-01 complete)
- 68-01: SwitchController placed after RunSelfTest — additive variant, no enum reorder; failover_url: Option<String> with serde(default) for zero-friction backward compat; last_switch_ms: AtomicU64 on HeartbeatStatus for Plan 02 runtime wiring (FAIL-01, FAIL-03, FAIL-04 data contracts complete)
- 68-02: active_url Arc<RwLock<String>> read inside outer reconnect loop on each iteration — picks up new URL from SwitchController without restart; strict URL allowlist (primary+failover only); log_event made pub for cross-module SWITCH event recording; switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000 (FAIL-02, FAIL-03, FAIL-04 runtime wiring complete)
- 77-01: rcgen 0.14 generate_simple_self_signed takes Vec<String> with auto IP detection (not SanType enum); CertifiedKey has signing_key (not key_pair); backward-compat ServerConfig with Option fields (TLS-02, TLS-04)
- 77-02: HelmetLayer::blank() with selective headers (not with_defaults) -- avoids COEP/COOP/upgrade-insecure-requests that break kiosk proxy; HSTS max-age=300 for testing safety; racingpoint.cloud CORS exact match (security fix from .contains()); HTTPS listener via tokio::spawn with .into_make_service() (no ConnectInfo/rate-limiting on HTTPS port) (TLS-01, TLS-03, TLS-04, KIOSK-06)
- 69-02: failover_broadcast uses simple != for terminal_secret comparison (consistent with all existing service routes -- no subtle crate); split_brain_probe reqwest::Client created once before outer reconnect loop; guard probes :8090/ping with 2s timeout before honoring SwitchController (ORCH-02, ORCH-03)
- 69-01: ONE cycleOk boolean per 5s tick in HealthMonitor -- consecutiveFailures increments by exactly 1 per cycle, not per probe attempt; guarantees DOWN_THRESHOLD=12 = 60s sustained outage (HLTH-01, HLTH-02, HLTH-03 complete)
- 69-01: notify_failover via exec_request to Bono -- server .23 is down so James cannot use .23 email_alerts; FailoverOrchestrator delegates notification to Bono (ORCH-01, ORCH-04 complete)
- 69-03: Secondary watchdog timer: 255s after james_down (45s+255s=5min total) probes 100.71.226.83:8090/ping via Tailscale; skips if .23 reachable (not a venue outage); pm2 via execFileSync fallback restart->start; polls /health 6x before broadcast; AlertCooldown 10-min prevents repeat activations (HLTH-04 complete)
- 69-04: notify_failover tier AUTO (command itself delivers WhatsApp via Evolution API); EXEC_REASON injected as env var by ExecHandler#execute so notify_failover gets the failover reason string; buildSafeEnv() extended with Evolution API vars conditionally; notifyFn fixed to call sendEvolutionText directly; send-email.js stdlib-only with sendmail+SMTP fallback; email on both failover paths (ORCH-04 complete)
- 78-01: Defense-in-depth for DevTools: both --disable-dev-tools browser flag AND F12/Ctrl+Shift+I/J keyboard hook blocks; USBSTOR Start=4 disables mass storage only (HID unaffected); accessibility Flags 506/122/58 disable hotkeys not features (KIOSK-01, KIOSK-02, KIOSK-03, KIOSK-04)
- 74-01: AgentConfig fields all pub (not pub(crate)) for cross-module access in later extractions; load_config pub; validate_config + detect_installed_games pub(crate); billing_guard.rs required crate::config:: path fix after root extraction (DECOMP-01)
- 74-02: AppState fields all pub(crate) not pub -- crate-internal (matches config.rs pattern); crash_recovery bool renamed crash_recovery_startup to avoid collision with CrashRecoveryState inner-loop local; SelfHealResult (not HealResult) -- self_heal.rs uses that name; AiDebugSuggestion from rc_common::types (already a shared type); ws_tx/ws_rx stay loop-local (borrow conflict per RESEARCH.md Pitfall); DECOMP-02 complete
- 74-03: HandleResult::Break/Continue enum (not bool) for self-documenting loop control; anyhow::Result<HandleResult> for serde_json ? propagation; SwitchController params (primary_url/failover_url/active_url/split_brain_probe) passed separately to handle_ws_message -- outer-loop locals not in AppState; LaunchState + CrashRecoveryState made pub(crate) for ws_handler.rs cross-module access; Python file truncation deleted 972-line dead code block (lines 1699-2670); DECOMP-03 complete
- 78-03: Option<String> with #[serde(default)] for session_token -- backward compat with older agents; direct SQL UPDATE for emergency billing pause avoids circular HTTP dependency; LazyLock<Mutex<HashMap>> for per-pod security alert debounce (5min cooldown) (SESS-04, SESS-05)
- 89-02: format_wa_phone promoted to pub(crate) in billing.rs -- single phone formatting source for both billing and psychology modules; STREAK_GRACE_DAYS+7=14d total window for weekly visit streaks; send_pwa_notification uses DB-record pattern (not WebSocket), deferred to Phase 3 (FOUND-01, FOUND-02, FOUND-04)
- 89-03: psychology routes in staff_routes (JWT-protected) -- customer badge display deferred to Phase 90; evaluate_badges + update_streak called sequentially at end of post_session_hooks (already inside tokio::spawn); 5 seed badges use INSERT OR IGNORE -- idempotent across DB migrations; count extracted before into_iter().map() to avoid use-after-move (FOUND-02, FOUND-03, FOUND-04, FOUND-05 complete)
- 81-01: Non-AC crash recovery else branch: match last_sim_type to config.games field (7 variants), clone base_config, override args from last_launch_args, call GameProcess::launch() -- mirrors LaunchGame handler exactly (LAUNCH-02 complete)
- 81-01: DashboardEvent::GameLaunchRequested added at end of enum using existing SimType -- no new imports needed (LAUNCH-04 complete)
- 81-01: pwa_game_request uses extract_driver_id() in-handler (customer JWT); validates pod in state.pods + installed_games; fire-and-forget broadcast; no AppState mutation (LAUNCH-05 complete)
- 70-02: server_recovery uses prev === 'down' guard -- prevents spurious failback on degraded->healthy; only full outage recovery triggers failback sequence (BACK-01, BACK-03, BACK-04 complete)
- 70-02: sync failure does NOT block pod switchback -- sessions missed during export/import logged as syncError in Uday notify message; initiateFailback reuses same alertCooldown as initiateFailover
- 80-02: SHA-256 of admin_pin_hash stored in system_settings for change detection without duplicating sensitive hash; 24h check in alerter loop sends WhatsApp if >30 days (ADMIN-06)
- 80-02: HMAC verification in permissive mode initially -- warns but allows mismatches for deployment transition; GET signing uses reconstructed query string as body (AUTH-07)
- 97-01: pod_id: String (not u32) for PreFlightPassed/PreFlightFailed -- CONTEXT.md had u32 but RESEARCH.md identified deserialization-breaking mismatch; all existing AgentMessage variants use String
- 97-01: ClearMaintenance is a unit variant (no fields) -- CoreToAgentMessage is always routed to a specific pod via its WS connection, pod_id redundant
- 97-01: PreflightConfig follows KioskConfig serde(default) pattern exactly -- reuses existing default_true() fn (PF-07)
- 97-02: MockHidBackend defined locally in pre_flight::tests -- MockTestBackend from ffb_controller is inside private mod tests{}; local mock! avoids cross-module visibility issues
- 97-02: Orphan game state captured before AppState borrow in tokio::join! -- game_pid and has_game_process extracted as plain values to avoid lifetime issues with &AppState across await points
- 97-02: billing_active.store(true) at line 167 in ws_handler.rs -- confirmed AFTER pre_flight gate block (lines 141-165); customers on failed pod never billed (PF-01, HW-01, HW-02, HW-03, SYS-01 complete)
- 82-03: GameState union must include 'loading' for TypeScript to accept game_state === 'loading' comparisons in kiosk KioskPodCard; SIM_TYPE_LABELS + SIM_TYPE_OPTIONS module-level pattern for consistent sim_type display (BILL-03, BILL-05)
- 83-01: No production code changes needed — existing F1 25 adapter already satisfies TEL-F1-01/02/03; 6 unit tests added to prove it. adapter.connected=true set directly in session_type_mapping test to avoid binding port 20777 in unit test environment
- 98-01: failure_strings.clone() before AgentMessage send — keeps original for show_maintenance_required() in ws_handler; debug_server.rs exhaustive match needed MaintenanceRequired arm (Rule 1 auto-fix, caught immediately on first compile)
- 98-02: check_lock_screen_http_on(addr) helper for port-param testability (option b — cleaner); PreFlightPassed has only pod_id field (no timestamp) — corrected from plan snippet at compile time (Rule 1 auto-fix); Window not found returns Warn (advisory, not a blocker)
- 99-01: ws_connect_elapsed_secs passed as u64 parameter to run() — decouples pre_flight module from ConnectionState; Disk check Warn (not Fail) if C: not found — graceful on non-standard disk layouts; WS stability is Warn not Fail per NET-01 spec — advisory only (SYS-02, SYS-03, SYS-04, NET-01 complete)
- 99-02: Option<Instant> on AppState for PreFlightFailed cooldown — safe in single-threaded select! loop, no Arc/Mutex needed; retry loop does NOT send alerts (correct by design from 98-02, only logs + refreshes lock screen); reset to None on Pass ensures first failure after recovery always alerts (STAFF-04 complete)

### Blockers/Concerns

- Phase 73 billing_guard: `attempt_orphan_end` calls reqwest directly -- need to decide on trait-wrap vs callback param before writing tests. Callback param (option b) is simpler, avoid trait boilerplate.
- Phase 74 select! decomposition: enumerate all 14 mutable shared variables before first extraction step -- assign each to ConnectionState (inner loop) or ReconnectState (outer loop)
- v6.0 (Phases 36-40) still blocked on BIOS AMD-V -- does not affect v11.0
- 66-05: exec round-trip (INFRA-03) pending Bono deployment -- Bono notified via INBOX.md commits 3e4091a + 35cea4f, will self-verify once Bono pulls + restarts pm2
- 76-01: Permissive mode for initial staff JWT deploy -- logs unauthenticated requests without rejecting (expand-migrate-contract pattern)
- 76-01: StaffClaims uses role="staff" field -- customer JWTs lacking role field are auto-rejected by deserialization
- 76-01: api_routes() split into 4 tiers (public/customer/staff/service) with state parameter for middleware

- 78-02: kiosk_routes separated from staff_routes -- pods need JWT-protected kiosk endpoints (experiences GET, settings GET, pod-launch, book-multiplayer) but must not access admin routes; layer order JWT first (401) then pod source check (403) (KIOSK-07, KIOSK-05)

### Pending Todos

- Pod 3 still not verified running after fix-pod.bat -- needs physical reboot + verification
- Version string inconsistency: USB-installed pods report v0.1.0, HTTP-deployed report v0.5.2
- Clean up .planning/update_roadmap_v9.py after v9.0 (can delete)
- Clean up .planning/update_roadmap_v11.py after v11.0 roadmap creation (can delete)

## Session Continuity

Last session: 2026-03-21T11:35:09.533Z
Stopped at: Completed 105-02-PLAN.md
Resume file: None
