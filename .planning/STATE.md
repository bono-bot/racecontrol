---
gsd_state_version: 1.0
milestone: v7.0
milestone_name: E2E Test Suite
status: executing
stopped_at: Completed 209-pre-ship-gate-and-process-tooling/209-03-PLAN.md
last_updated: "2026-03-26T06:34:38.318Z"
last_activity: 2026-03-26 — Phase 209-03 complete (gap closure — GATE-03 network gate fleet probe + WS handshake test)
progress:
  total_phases: 172
  completed_phases: 135
  total_plans: 325
  completed_plans: 318
  percent: 98
---

## Current Position

Phase: 209-pre-ship-gate-and-process-tooling
Plan: 03 complete
Status: Phase 209 complete (all 3 plans shipped, gap closure done)
Last activity: 2026-03-26 — Phase 209-03 complete (gap closure — GATE-03 network gate fleet probe + WS handshake test)

Progress: [██████████] 98%

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-26)

**Core value:** Fully autonomous infrastructure health — detect, fix, cascade, and notify without human intervention
**Current focus:** v26.0 Autonomous Bug Detection & Self-Healing — Phase 211: Safe Scheduling Foundation

## Accumulated Context

### Roadmap Evolution

- Phase 211.1 inserted after Phase 211: Venue Shutdown Button (URGENT) — staff-initiated safe shutdown of all venue infrastructure with pre-shutdown audit gate, Bono verification, and James boot-time fix implementation

### Decisions

- 6 phases derived from 37 requirements across 5 natural categories (SCHED, DET, HEAL, COORD, LEARN, TEST)
- Phase numbering starts at 211 (v23.1 occupies 202-204, v25.0 occupies 205-210)
- Phase 211 (safety gates) must ship before any scheduled execution fires on live infrastructure — no "add safety later" option at 2:30 AM
- Phase 212 (detection) inherits Phase 211 safety infrastructure via source-module composition
- Phase 213 (healing) grouped with HEAL-06/07/08 (Audit Protocol methodology, live-sync, toggle) — they modify the fix engine architecture, same phase
- Phase 214 (Bono coordination) gates on Phase 213 — coordination is only needed when both agents have fix capability
- Phase 215 (self-patch loop LEARN-07/08/09) depends on detection + fixing being stable — placed after Phase 213+214
- Phase 216 (tests) is last — tests should validate stable behavior, not planned behavior
- Foundation scripts already exist: auto-detect.sh (6-step pipeline), bono-auto-detect.sh (Bono-side failover)
- Config drift (DET-01) has upstream Rust API dependency — plan Phase 212 to decide: build GET /api/v1/config/health-params endpoint OR use SCP fallback with first-line TOML validation
- Log anomaly thresholds (DET-03) should use pattern-based triggers for Phase 212 launch — rate-based thresholds need 7-day calibration window
- Bono cron schedule needs verification before Phase 211 work: current may be 0 2 * * * (UTC 02:00 = IST 07:30) but target is IST 02:35 (UTC 21:05)
- WoL escalation tier (HEAL-01/02) requires manual test on at least 2 pods before autonomous activation — not blocking Phase 213 but must happen before WoL tier is enabled in whitelist
- Previous milestone context preserved:
  - [Phase 202]: ws_connect_timeout threshold at 600ms, billing checks venue-state-aware, ps_count=0 is WARN (watchdog dead)
  - [Phase 205-verification-chain-foundation]: verification.rs not feature-gated — VerificationError and VerifyStep needed by all crates including rc-sentry
  - [Phase 205-verification-chain-foundation]: boot_resilience.rs feature-gated behind tokio — rc-sentry has no async runtime
  - [Phase 203-02]: Content sub-checks added alongside existing count checks (not replacing them)
  - [Phase 206]: All config fallback sites in rc-agent main.rs are after tracing init — use tracing::warn! directly without pre-init buffer
  - [Phase 206]: Empty allowlist detection writes override directly to MachineWhitelist under write lock — all downstream scan paths see report_only
  - [Phase 206]: RecoveryLogger for FSM transitions created inside watchdog thread pointing to RECOVERY_LOG_POD — safe JSONL append without coordination
  - [Phase 196-01]: validate_args called before billing gate — invalid JSON rejected before touching shared state
  - [Phase 196-01]: launcher_for() returns static dyn ref (ZST impls) — no heap allocation in hot launch path
  - [Phase 196-01]: Billing gate checks both active_timers AND waiting_for_game — deferred billing sessions now pass launch gate
  - [Phase 206-02]: sentinel_watcher uses std::thread::spawn (not tokio) — notify RecommendedWatcher requires sync recv loop; blocking_send bridges to async tokio channel
  - [Phase 206-02]: SentinelChange routed via ws_exec_result_tx (existing AgentMessage mpsc) — no new channel needed
  - [Phase 206-02]: active_sentinels NOT cleared on WS disconnect — sentinel files persist on disk; clear would cause stale "no sentinels" until next change event
  - [Phase 206-02]: DashboardEvent::SentinelChanged is a new dedicated variant (not PodUpdate reuse) — carries sentinel-specific fields for dashboard real-time reaction
  - [Phase 196-02]: Stopping timeout tested via check_game_health() — tokio::time::pause() breaks SQLite pool timeout in make_state()
  - [Phase 196-02]: Feature flag 'game_launch' defaults to enabled (unwrap_or(true)) when missing — prevents Pitfall 6 regression
  - [Phase 207-01]: fetch_from_server feature-gated behind #[cfg(feature = "http-client")] to match existing reqwest gating; rc-common tokio feature enabled in rc-agent Cargo.toml for boot_resilience access
- [Phase 207]: guard_confirmed AtomicBool shared via AppState; GUARD_CONFIRMED intercepted in WS exec dispatch
- [Phase 211-safe-scheduling-foundation]: PID file at /tmp/auto-detect.pid -- host-local, survives git clean, no repo pollution (SCHED-03)
- [Phase 211-safe-scheduling-foundation]: Cooldown keyed per pod_ip:issue_type (not fleet-level) -- prevents storm suppression from hiding different issues on same pod (SCHED-04)
- [Phase 211-safe-scheduling-foundation]: WhatsApp send deferred to Phase 213 -- cooldown infrastructure wired now, Phase 213 adds send call only
- [Phase 208]: StepValidateCriticalFields returns Err(TransformError) on defaults — caller catches non-fatally (COV-03 gap closed)
- [Phase 208]: rc-agent load_config tries next path on parse failure instead of returning error immediately
- [Phase 208-03]: Config and all sub-structs derive Clone for ownership safety in chain execution
- [Phase 208-03]: Spawn retry uses same method (session1 or schtasks) that originally succeeded — exactly one retry, no infinite loops (COV-05 gap closed)
- [Phase 211-safe-scheduling-foundation]: Git Bash at C:\Program Files\Git\bin\bash.exe confirmed on James .27; AUDIT_PIN baked into schtasks /TR for SYSTEM context; Bono cron corrected 0 21 -> 5 21 UTC (02:30 -> 02:35 IST); relay custom_command not supported, SSH fallback used for cron correction
- [Phase 197-launch-resilience-ac-hardening]: Dynamic timeout from median+2*stdev of last 10 launches; exit_code priority over string parsing; atomic Race Engineer with single write lock
- [Phase 209]: IST timestamp via TZ=Asia/Kolkata date (portable); real pod healer flicker incident as template example
- [Phase 209]: Domain detection uses git diff --cached first (staged), falls back to HEAD~1; display/parse are blocking, billing/config informational
- [Phase 197-launch-resilience-ac-hardening]: Pre-launch checks: testable via check_sentinel_files_in_dir(dir) with injectable path; PID-stability polling for AC readiness; parse_launch_args for space-safe arg passing
- [Phase 209]: Used curl Upgrade headers for WS handshake test instead of wscat dependency

### Pending Todos

- Verify Bono cron schedule before Phase 211: `ssh root@100.70.177.44 "crontab -l | grep auto-detect"` — target is 21:05 UTC (= 02:35 IST)
- WoL manual test on at least 2 pods before HEAL-01 WoL tier is enabled in APPROVED_FIXES
- Decide Phase 212 config-drift.sh path: build GET /api/v1/config/health-params Rust endpoint OR use SCP fallback — document decision before writing the script

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-26T06:34:32.078Z
Stopped at: Completed 209-pre-ship-gate-and-process-tooling/209-03-PLAN.md
Resume file: None
