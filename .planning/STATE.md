---
gsd_state_version: 1.0
milestone: v7.0
milestone_name: E2E Test Suite
status: Phase 200 plan 01 shipped — combo reliability scoring foundation
stopped_at: Completed 201-frontend-integration-type-sync/201-01-PLAN.md
last_updated: "2026-03-26T10:57:52.772Z"
last_activity: 2026-03-26 — Phase 200-01 complete (combo_reliability table, query/update functions, launch warning injection, max_auto_relaunch tuning)
progress:
  total_phases: 172
  completed_phases: 145
  total_plans: 349
  completed_plans: 343
  percent: 98
---

## Current Position

Phase: 200-self-improving-intelligence
Plan: 01 complete
Status: Phase 200 plan 01 shipped — combo reliability scoring foundation
Last activity: 2026-03-26 — Phase 200-01 complete (combo_reliability table, query/update functions, launch warning injection, max_auto_relaunch tuning)

Progress: [██████████] 98%

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-26)

**Core value:** Fully autonomous infrastructure health — detect, fix, cascade, and notify without human intervention
**Current focus:** v26.0 Autonomous Bug Detection & Self-Healing — Phase 211: Safe Scheduling Foundation

## Accumulated Context

### Roadmap Evolution

- Phase 211.1 inserted after Phase 211: Venue Shutdown Button (URGENT) — staff-initiated safe shutdown of all venue infrastructure with pre-shutdown audit gate, Bono verification, and James boot-time fix implementation

### Decisions

- [Phase 200-01]: No COALESCE in PRIMARY KEY (SQLite limitation) — used UNIQUE INDEX on COALESCE(car,'') + COALESCE(track,'') with DELETE+INSERT upsert pattern
- [Phase 200-01]: FailureMode defined locally in metrics.rs to avoid circular import with api::metrics module
- [Phase 200-01]: Used crate::metrics:: prefix in routes.rs to disambiguate from local super::metrics (api::metrics)
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
- [Phase 211.1-01]: SSH from server to James for audit trigger (relay /exec/run goes to Bono VPS not James local)
- [Phase 211.1-01]: Billing drain checks both active_timers AND waiting_for_game -- deferred billing sessions must block shutdown
- [Phase 211.1-venue-shutdown-button]: Shutdown page blocks on api.venueShutdown() (up to 150s timeout) while audit runs server-side — no polling, single long request
- [Phase 211.1-venue-shutdown-button]: audit_blocked reason field drives user-facing message: billing_active shows session count, audit_failed shows truncated output, james_offline directs to Bono
- [Phase 210]: timeout /nobreak not flagged (works in HKLM Run context); ConspitLink added to bloatware skip list
- [Phase 211.1-venue-shutdown-button]: Bono fallback uses HTTP relay not SSH; both-offline returns actionable error; boot-time-fix archives findings with mv not delete
- [Phase 210]: Phase 61 bat-drift uses bat_scan_pod_json for structured audit output
- [Phase 210]: deploy-pod.sh copies bat files to BINARY_DIR to reuse single HTTP server for bat sync
- [Phase 198-on-track-billing]: AC False-Live guard: 5s window with speed>0 OR |steer|>0.02 before billing emit; process fallback crash guard gates Live on game.is_running()
- [Phase 198-on-track-billing]: BillingConfig placed after CafeConfig in Config struct; Config initializer required explicit billing: BillingConfig::default()
- [Phase 212-detection-expansion]: DET-01 uses safe_remote_exec :8090 (not SCP) to read rc-agent.toml
- [Phase 212-detection-expansion]: cascade.sh DETECTOR_FINDINGS accumulator feeds BUGS_FOUND in auto-detect.sh run_cascade_check() after all 6 detectors run
- [Phase 198-on-track-billing]: WaitingForGame broadcast uses separate loop over waiting_for_game map (not active_timers) — entries never in active_timers
- [Phase 198-on-track-billing]: BILL-10 error path drops mp lock before acquiring waiting_for_game — consistent lock ordering prevents deadlock
- [Phase 212-detection-expansion]: DET-04 reads JSONL (not startup.log) -- startup.log truncates on restart making historical restart count impossible
- [Phase 212-detection-expansion]: DET-06 cloud DB check uses SSH (not relay) -- relay dispatches to registered commands only, not raw shell exec
- [Phase 198-on-track-billing]: 4 billing tests for BILL-05/06/10/12: waiting_for_game_tick_broadcasts, cancelled_no_playable_on_timeout, multiplayer_db_query_failure_preserves_waiting_entry, configurable_billing_timeouts — all 82 billing tests pass
- [Phase 213-self-healing-escalation]: WOL_ENABLED defaults to false until manual test on 2 pods — prevents spurious WoL on online pods
- [Phase 213-self-healing-escalation]: clear_old_maintenance_mode guarded to venue=closed only — open-hours MM may be intentional staff action
- [Phase 213-self-healing-escalation]: Missing auto-detect-config.json = auto_fix_enabled (fail-safe) — prevents accidental detect-only mode
- [Phase 213-self-healing-escalation]: cascade.sh sources escalation-engine.sh after detector files and before run_all_detectors() -- ordering critical for live-sync healing
- [Phase 213-self-healing-escalation]: WhatsApp block replaced with escalate_human() -- HEAL-04 silence conditions (QUIET, venue-closed deferral, 6h cooldown) centralized in healing engine
- [Phase 199-crash-recovery]: force_clean: false on normal launches, true on Race Engineer and manual relaunch paths — backward-compatible serde default
- [Phase 199-crash-recovery]: query_best_recovery_action follows query_dynamic_timeout pattern: 3-sample minimum, unwrap_or_default, returns (action, success_rate) tuple
- [Phase 199-crash-recovery]: exit_codes pushed inside existing LAUNCH-17 write lock — atomic with relaunch counter increment, no extra lock
- [Phase 214-01]: Guard-wrap all coordination hook calls with type -t check so auto-detect.sh degrades gracefully if coord-state.sh absent
- [Phase 214-01]: Combined EXIT trap covers PID file + coord lock atomically — written after write_active_lock to ensure correct cleanup ordering
- [Phase 214-02]: BONO_DEGRADED_MODE=true (Tailscale up, relay down) disables all fixes — may be intentional maintenance
- [Phase 214-02]: tailscale ping --c 1 --timeout 5s 100.125.108.37 is authoritative for offline confirmation; icmp ping is not used
- [Phase 214-02]: write_bono_findings() pushes to INBOX.md via git to satisfy dual-channel comms standing rule
- [Phase 214-02]: Recovery check placed post-summary so Bono completes full run first, then checks if James relay came back
- [Phase 199-crash-recovery]: Safe mode cooldown suppression re-arms timer for 30s during PausedWaitingRelaunch — prevents safe mode from expiring mid-recovery (RECOVER-07)
- [Phase 199-crash-recovery]: force_clean in event_loop.rs satisfied via documentation comments — Plan 01 correctly placed implementation in ws_handler.rs; event_loop.rs delegates to ws_handler
- [Phase 215-self-improving-intelligence]: SUGGESTIONS_JSONL at audit/results/suggestions.jsonl — shared between pattern-tracker and trend-analyzer
- [Phase 215-self-improving-intelligence]: TREND_OUTLIER entries excluded from analysis input via jq select to prevent feedback loop inflating pod counts
- [Phase 215-02]: Proposal deduplication at write time — scan pending proposals before creating new to prevent duplicate accumulation across runs
- [Phase 215-02]: get_suggestions registered in pattern-tracker.sh for relay exec — ensures alias available even if suggestion-engine not yet sourced
- [Phase 215-02]: TREND_OUTLIER entries processed as threshold_tune proposals with fixed confidence 0.50 to avoid feedback loop inflating regular counts
- [Phase 215]: Threshold increment 20% round-up to ensure actual increase for small base values
- [Phase 215]: Standing rule IDs use SR-LEARNED-NNN prefix to distinguish engine-generated from manual rules
- [Phase 215]: new_audit_check/self_patch queued for 215-04 self-patch loop with status queued_for_selfpatch (not dropped)
- [Phase 215-04]: self_patch_enabled defaults to false — explicit opt-in for script self-modification, unlike auto_fix_enabled which defaults true
- [Phase 215-04]: self_patch_loop processes ONE proposal per run — blast radius limit for autonomous code modification
- [Phase 215-04]: Scope restriction via realpath comparison: only scripts/detectors/ and scripts/healing/ — never auto-detect.sh or audit/lib/
- [Phase 216-01]: grep -oP -> grep -oE portability fix applied to detect-config-drift, detect-log-anomaly, detect-crash-loop -- Perl regex fails on Git Bash Windows
- [Phase 216-01]: fixture-backed mock pattern: FIXTURE_FILE + safe_remote_exec jq -Rn rawfile -- all 6 detectors testable offline without live pods
- [Phase 200-02]: query_alternatives takes &SqlitePool directly — testable without State, consistent with Plan 01 query_ pattern
- [Phase 200-02]: Pod fallback fills remaining slots (< 3) from fleet with different pod_id to avoid duplicates
- [Phase 200-02]: launch_matrix runs two queries per pod (aggregate + failure modes) — readable and SQLite-safe
- [Phase 216-02]: CALLS_FILE file-based tracking used instead of TIER_CALLS array -- bash arrays mutated in subshell forks are lost; writing to shared file survives fork boundary
- [Phase 216-02]: TIER-SENTINEL test asserts human only -- escalate_human is called directly at tier 5 without sentinel gate; sentinel only guards tiers 1-4
- [Phase 201-01]: BillingSessionStatus uses 10 variants matching Rust enum — removed stale paused_idle and expired, added waiting_for_game, paused_disconnect, paused_game_pause, cancelled_no_playable
- [Phase 201-01]: Parity script uses indexOf + brace-counting for Rust enum parsing (not regex) — more reliable for multi-line enums with doc comments

### Pending Todos

- Verify Bono cron schedule before Phase 211: `ssh root@100.70.177.44 "crontab -l | grep auto-detect"` — target is 21:05 UTC (= 02:35 IST)
- WoL manual test on at least 2 pods before HEAL-01 WoL tier is enabled in APPROVED_FIXES
- Decide Phase 212 config-drift.sh path: build GET /api/v1/config/health-params Rust endpoint OR use SCP fallback — document decision before writing the script

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-03-26T10:57:52.751Z
Stopped at: Completed 201-frontend-integration-type-sync/201-01-PLAN.md
Resume file: None
