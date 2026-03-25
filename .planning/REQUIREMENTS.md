# Requirements: v24.0 Game Launch & Billing Rework

**Defined:** 2026-03-26
**Core Value:** Games launch flawlessly and billing starts precisely when the customer is on-track — every launch makes the system smarter.

**Audit basis:** Full code audit of game_launcher.rs (988 lines), billing.rs (4500+ lines), ws_handler.rs, event_loop.rs, ac_launcher.rs, game_process.rs, billing_guard.rs. 22 bugs, 11 logic gaps, 3 race conditions, 9 silent error swallows, 15+ hardcoded magic numbers identified.

## v24.0 Requirements

### Metrics Foundation (METRICS)

- [ ] **METRICS-01**: Every game launch attempt is recorded in SQLite `launch_events` table with: pod_id, sim_type, car, track, session_type, timestamp, outcome (success/timeout/crash/error), error_taxonomy, duration_to_playable_ms, error_details, launch_args_hash, attempt_number
- [ ] **METRICS-02**: Every game launch attempt is ALSO appended to JSONL file (`launch-events.jsonl`) with full context for raw audit trail — dual storage ensures queryability (SQLite) and immutable audit (JSONL)
- [ ] **METRICS-03**: Every billing session event (start, pause, resume, end, discrepancy) is recorded with timing accuracy data: `launch_command_at`, `playable_signal_at`, `billing_start_at`, `delta_ms` (gap between launch and billing start)
- [ ] **METRICS-04**: Every crash recovery attempt is recorded: failure_mode (taxonomy), recovery_action_tried, recovery_outcome, recovery_duration_ms, pod_id, sim_type, car, track
- [ ] **METRICS-05**: Historical launch data is queryable per game/car/track/pod combo via `GET /api/v1/metrics/launch-stats?pod=X&game=Y&car=Z&track=W` returning: success_rate, avg_time_to_track_ms, p95_time_to_track_ms, common_failure_modes, total_launches, last_30d_trend
- [ ] **METRICS-06**: Billing accuracy metrics available via `GET /api/v1/metrics/billing-accuracy` returning: avg_delta_ms (launch-to-billing), max_delta_ms, sessions_with_zero_delta, sessions_where_billing_never_started, false_playable_signals
- [ ] **METRICS-07**: `log_game_event()` (game_launcher.rs:561-582) must NOT silently swallow DB insert errors — log error + write to JSONL fallback if DB insert fails. Add `created_at` timestamp explicitly (not rely on DB DEFAULT)

### Pod ID Normalization (PODID)

- [ ] **PODID-01**: Create `normalize_pod_id(raw: &str) -> String` helper that canonicalizes all pod ID formats (`pod-1`, `pod_1`, `POD_1`, `Pod-1`) to a single canonical form — used at EVERY entry point (API handlers, WS handlers, billing lookups)
- [ ] **PODID-02**: Replace ALL 5+ inconsistent pod ID lookups in game_launcher.rs (lines 95-106, 111-112, 142-176, 210, 395-400) with `normalize_pod_id()` — eliminate the `billing_alt_id` pattern entirely
- [ ] **PODID-03**: Replace ALL inconsistent pod ID lookups in billing.rs and agent_senders with `normalize_pod_id()` — one canonical format everywhere

### Game Launcher Rework (LAUNCH)

- [ ] **LAUNCH-01**: Game launcher handles AC, F1 25, and iRacing with per-game launch sequences that are structurally separate — extract from monolithic `launch_game()` into `AcLauncher`, `F1Launcher`, `IRacingLauncher` trait implementations with shared `GameLauncher` interface
- [ ] **LAUNCH-02**: Fix billing gate (game_launcher.rs:95-106) — check BOTH `active_timers` AND `waiting_for_game` maps. Also verify `timer.status == Active` (not just key presence). A paused/cancelled session must NOT pass the gate
- [ ] **LAUNCH-03**: Fix billing gate TOCTOU race — hold read lock through the entire launch validation (billing check + double-launch guard + content validation) rather than releasing between checks. Billing session expiring mid-validation must be caught
- [ ] **LAUNCH-04**: Fix double-launch guard (game_launcher.rs:108-116) — also block launch when game_state is `Stopping` (currently allows launches while previous game is shutting down, causing collision)
- [ ] **LAUNCH-05**: Fix silent JSON parsing failure (game_launcher.rs:80-90) — if `serde_json::from_str` fails on launch_args, return `Err` immediately instead of silently skipping content validation. Invalid JSON must never bypass checks
- [ ] **LAUNCH-06**: Fix silent agent send failure (game_launcher.rs:154-156) — if `tx.send(LaunchGame)` fails (agent disconnected), transition tracker to `GameState::Error` immediately instead of leaving it stuck in `Launching` for 120s. Broadcast error to dashboard
- [ ] **LAUNCH-07**: Fix feature flag silent block (ws_handler.rs:284-293) — if `game_launch` flag is disabled, send explicit `GameStateUpdate { state: Error, message: "game_launch feature disabled" }` back to server instead of silent no-op. Server must know the launch was rejected
- [ ] **LAUNCH-08**: Pre-launch health check runs before every launch — verifies: no orphan game processes (13 known exe names from game_process.rs:58-69), disk space > 1GB, ConspitLink running for AC, no MAINTENANCE_MODE sentinel, no OTA_DEPLOYING sentinel. Checks informed by historical failure causes for that pod from METRICS-01 data
- [ ] **LAUNCH-09**: Launch timeout is dynamic — query `launch_events` for median time-to-playable for this game/car/track/pod combo, add 2σ buffer. Minimum 60s, maximum 300s. Default to 120s for AC, 90s for F1/iRacing when no history exists. Replace hardcoded 120s/60s at game_launcher.rs:514-517
- [ ] **LAUNCH-10**: On launch failure, system performs clean state reset: kill ALL game-related processes (13 exe names), clear `C:\RaceControl\game.pid`, clear AC shared memory adapter, reset game_process state to Idle, clear temp shader cache if relevant — then auto-retry up to 2 times before alerting staff
- [ ] **LAUNCH-11**: Game crash during launch produces structured error with taxonomy enum: `ShaderCompilationFail`, `OutOfMemory`, `AntiCheatKick`, `ConfigCorrupt`, `ProcessCrash(exit_code)`, `LaunchTimeout`, `ContentManagerHang`, `MissingDependency`, `Unknown`. Replaces generic "game crashed" string (game_launcher.rs:526-551)
- [ ] **LAUNCH-12**: Launch failures do NOT trigger MAINTENANCE_MODE — separate launch crash counter from pod health crash counter. MAINTENANCE_MODE sentinel only written by pod health monitor, never by game launcher. Launch retries use their own 2-attempt limit independent of the 3-in-10-min MAINTENANCE_MODE threshold
- [ ] **LAUNCH-13**: Staff receives actionable alert on final launch failure — includes: error taxonomy, crash count, pod state, last 3 exit codes, suggested action based on historical recovery data for this failure mode. Alert sent via dashboard broadcast + WhatsApp to Uday
- [ ] **LAUNCH-14**: Fix `relaunch_game()` (game_launcher.rs:190-252) — check billing with BOTH pod ID formats (currently only checks one). Also handle case where `launch_args` is None (tracker created by `handle_game_state_update`, not `launch_game`) — reject relaunch with clear error instead of sending LaunchGame with empty args
- [ ] **LAUNCH-15**: Fix Race Engineer counter race condition (game_launcher.rs:403-419) — use single atomic read-check-increment operation under one write lock instead of separate read lock → write lock. Prevents duplicate relaunch spawns when rapid errors arrive on same pod
- [ ] **LAUNCH-16**: Fix timeout not triggering auto-relaunch (game_launcher.rs:526-551) — when `check_game_health()` fires timeout, call Race Engineer logic directly instead of only setting Error state. Currently games time out and sit in Error with no recovery despite active billing
- [ ] **LAUNCH-17**: Fix `stop_game()` (game_launcher.rs:254-286) — log sim_type in game event (currently logs empty string ""). Validate current state before transitioning to Stopping (don't transition Error→Stopping). Set timeout for Stopping state (if agent never confirms stop, transition to Error after 30s)
- [ ] **LAUNCH-18**: Add `error_at` timestamp to GameTracker — currently Error state has no timestamp, can't distinguish "error 1 second ago" from "error 5 minutes ago". Add `last_relaunch_attempt_at` to prevent duplicate relaunch spawns
- [ ] **LAUNCH-19**: Fix game_process.rs arg parsing — replace `split_whitespace()` (line 191) with proper Windows command-line parser that handles paths with spaces (e.g., `C:\Program Files\Steam\...`). Use array-based arg passing instead of string splitting

### AC Launcher Hardening (AC)

- [ ] **AC-01**: Replace hardcoded sleeps in ac_launcher.rs with polling-based waits: (a) post-kill 2s sleep → poll for acs.exe absence (max 5s), (b) AC load 8s sleep → poll for AC window handle (max 30s), (c) minimize 2s sleep → poll for foreground window change (max 5s)
- [ ] **AC-02**: Fix CM fallback PID — when Content Manager fails and direct acs.exe launches (ac_launcher.rs:346-350), call `find_game_pid()` to get fresh PID instead of using potentially stale PID from failed CM attempt
- [ ] **AC-03**: Increase CM launch timeout from 15s to 30s for slow pods, with progress logging every 5s showing what CM is doing (checking CM process, checking acs.exe spawned, checking WerFault)
- [ ] **AC-04**: Safety verification `verify_safety_settings()` must run AFTER race.ini write AND before game process spawn — abort launch with clear error if DAMAGE!=0 or SESSION_START!=100. Log verification result to launch_events

### PlayableSignal Rework — On-Track Billing (BILL)

- [ ] **BILL-01**: Billing starts ONLY when PlayableSignal confirms car is on-track and controllable — not during loading screens, shader compilation, or game menus. This is the existing intent but must be verified per game with stricter signals
- [ ] **BILL-02**: AC PlayableSignal rework — use shared memory `AcStatus::Live` (existing) PLUS verify `speedKmh > 0` OR `steerAngle != 0` within 5 seconds of Live signal. If neither within 5s, treat as menu/replay (not billable). Eliminates false-Live from AC pause screen
- [ ] **BILL-03**: F1 25 PlayableSignal rework — use UDP telemetry packet (port 20777) with `m_sessionType > 0` AND `m_playerCarIndex` present AND packet contains non-zero `m_speed`. Confirms player is in active session on track, not in menu/loading
- [ ] **BILL-04**: iRacing PlayableSignal rework — use shared memory `IsOnTrack=true` AND `IsOnTrackCar=true` (existing, already correct). No change needed, just verify and document
- [ ] **BILL-05**: Fix 90-second fallback (event_loop.rs:696-713) for EVO/WRC/Forza — guard with `game.is_running()` check before emitting AcStatus::Live. If game crashed before 90s, emit Error instead of false Live signal. This currently bills customers for crashed games
- [ ] **BILL-06**: Billing timer on kiosk/dashboard shows "Loading..." state (not counting down) from launch command until PlayableSignal fires — customer sees they're not being charged during load. Requires new `BillingSessionStatus::WaitingForGame` state broadcast to dashboards
- [ ] **BILL-07**: If PlayableSignal never fires within the dynamic timeout window, billing does NOT start — end the WaitingForGameEntry, alert staff, customer is not charged. Replace hardcoded 180s launch timeout (billing.rs:398) with dynamic timeout from LAUNCH-09
- [ ] **BILL-08**: Billing pauses automatically on game crash (AcStatus::Pause already works) and resumes ONLY when relaunch reaches on-track PlayableSignal state — crash recovery time is never billed. Verify this flow is correct for all 3 games (AC, F1, iRacing)
- [ ] **BILL-09**: Fix AC timer sync (game_launcher.rs:367-391) — replace hardcoded 120-second threshold with dynamic value from launch metrics (median time-to-Running for this combo). Fix double `Utc::now()` call (memory vs DB timestamp mismatch). Fix missing billing_alt_id lookup. Add error logging for failed DB UPDATE (currently `let _ = sqlx::query(...)`)
- [ ] **BILL-10**: Fix multiplayer billing silent downgrade (billing.rs:486) — if `group_session_members` DB query fails, log error and REJECT billing start instead of silently treating multiplayer as single-player. `.unwrap_or_default()` must become `.map_err()` with logging
- [ ] **BILL-11**: Fix WaitingForGameEntry orphan (billing.rs:620-636) — after multiplayer timeout evicts non-connected pods, clean up their WaitingForGameEntry from the map. Currently pods that come online after timeout get re-billed as solo sessions silently
- [ ] **BILL-12**: Replace ALL hardcoded billing timeouts with configurable values in `racecontrol.toml`: multiplayer_wait_timeout (default 60s), pause_auto_end_timeout (default 600s), launch_timeout_per_attempt (default 180s), idle_drift_threshold (default 300s), offline_grace_before_autoend (default 300s)

### Crash Recovery (RECOVER)

- [ ] **RECOVER-01**: On game crash (during launch or mid-session), system kills all game-related processes (13 exe names from game_process.rs:58-69), clears game.pid, clears shared memory adapter, resets launch state — full clean slate within 10s. Then relaunches within 60s total
- [ ] **RECOVER-02**: Recovery action selection informed by historical data — query `launch_events` for most successful recovery action for this failure taxonomy + pod + game combo. If "kill + clean + relaunch" works >80% for this combo, use it as Tier 1. If <50%, try alternative (different car, skip shader warmup, etc.)
- [ ] **RECOVER-03**: Auto-relaunch after crash preserves original launch_args (same car, track, session type) — customer doesn't re-select. Fix stale launch_args bug: if launch_args is None (tracker created by agent status update, not launch command), reject auto-relaunch with clear error instead of sending empty args
- [ ] **RECOVER-04**: After 2 failed auto-retries, staff is alerted with full crash context: error taxonomy, exit codes, pod state, recovery actions tried, plus suggestion of known-good alternative combo (from INTEL-03 data) if available. Alert via dashboard + WhatsApp
- [ ] **RECOVER-05**: Fix Race Engineer billing pause notification (game_launcher.rs:473-496) — when relaunch limit reached and billing is paused, broadcast `DashboardEvent::BillingPaused` to kiosk/dashboard AND send WhatsApp alert. Currently customer sees unexpected charge reduction with no explanation
- [ ] **RECOVER-06**: Fix exit grace timer interaction with crash recovery (event_loop.rs:606-620) — don't arm 30s exit grace timer if crash recovery is active (`crash_recovery != Idle`). Currently grace timer can fire during attempt 2 relaunch window, causing premature AcStatus::Off
- [ ] **RECOVER-07**: Fix safe mode cooldown interaction with crash recovery (event_loop.rs:1071-1076) — don't deactivate safe mode if game process is still detected or crash recovery is active. Currently safe mode can expire mid-recovery, re-enabling process guard scans during game relaunch

### Self-Improving Intelligence (INTEL)

- [ ] **INTEL-01**: System maintains per-combo reliability scores (game + car + track + pod) in SQLite `combo_reliability` table — updated after every launch. Fields: combo_hash, success_rate, avg_time_to_track_ms, p95_time_to_track_ms, total_launches, last_updated, common_failure_modes (JSON array)
- [ ] **INTEL-02**: When a selected combo has <70% success rate (from historical data, minimum 5 launches), kiosk launch response includes `warning: "This car/track combination has a {X}% success rate on this pod"` — staff sees warning before confirming launch
- [ ] **INTEL-03**: System suggests alternative combos via `GET /api/v1/games/alternatives?game=X&car=Y&track=Z&pod=N` — returns top 3 combos with >90% success rate on the same game, same pod, sorted by similarity (same track different car, same car different track)
- [ ] **INTEL-04**: Admin dashboard shows launch reliability matrix via `GET /api/v1/admin/launch-matrix` — sortable by game, pod, combo. Shows: success_rate, avg_time_to_track, failure_mode_distribution, trend (improving/degrading). Flagged combos (<70%) highlighted red
- [ ] **INTEL-05**: Dynamic timeouts (LAUNCH-09), pre-launch checks (LAUNCH-08), and recovery actions (RECOVER-02) automatically improve as more launch data accumulates — system uses rolling 30-day window for calculations, minimum 5 launches before overriding defaults. No manual threshold tuning needed

### Consistency & State Machine Fixes (STATE)

- [ ] **STATE-01**: Add `GameState::Stopping` to double-launch guard (game_launcher.rs:108-116) — prevent new launches while previous game is still shutting down
- [ ] **STATE-02**: Add `Stopping` state timeout (30s) — if agent never confirms stop, transition to Error. Currently Stopping can persist forever if agent is disconnected
- [ ] **STATE-03**: Fix tracker creation without launch_args (game_launcher.rs:319) — when agent spontaneously reports Running (after server restart), create tracker with launch_args=None but mark it as `externally_tracked` so auto-relaunch knows it can't retry
- [ ] **STATE-04**: Fix PID merge logic (game_launcher.rs:303) — always use `info.pid` when present instead of `info.pid.or(tracker.pid)`. Stale PIDs from previous launches must not persist
- [ ] **STATE-05**: Fix inconsistent state between game_launcher tracker and pod info (game_launcher.rs:328-339) — update both under a single lock scope or use a transactional update pattern. Currently a reader between the two updates sees inconsistent state
- [ ] **STATE-06**: Add `broadcast_error` to all dashboard_tx sends — replace `let _ = state.dashboard_tx.send(...)` with logged error if channel is full/broken. At minimum `warn!` on failure, consider retry for critical events (billing state changes)

## v25.0+ Requirements (Future)

### Extended Self-Improving Processes

- **FUTURE-01**: Pre-flight checks become self-improving — track per-pod failure patterns, predict failures, run proactive maintenance
- **FUTURE-02**: Recovery system becomes self-improving — track which fix works for which failure class, auto-escalate tier selection
- **FUTURE-03**: Billing accuracy reporting — weekly email showing billing accuracy metrics (avg delta between launch and on-track, outliers)
- **FUTURE-04**: Multi-game launcher for AC EVO, EA WRC, LMU when customer demand appears
- **FUTURE-05**: Cancel pending auto-relaunch tasks when manual relaunch triggered (idempotency fix)
- **FUTURE-06**: Add timeout bounds to all I/O operations in game launcher (agent send, billing fetch, DB sync) — prevent entire handler blocking on hung operation

## Out of Scope

| Feature | Reason |
|---------|--------|
| AC EVO / EA WRC / LMU / Forza launchers | No customer demand yet — 90s fallback fixed to not false-signal on crash (BILL-05) |
| Real-time billing rate adjustment based on game type | Billing rates already configurable per-game in billing_rates table |
| Kiosk UI redesign | Only adding "Loading..." state (BILL-06) and combo warnings (INTEL-02) |
| ac_launcher.rs full rewrite | Refactor sleeps and error handling (AC-01 through AC-04), don't rewrite 35K lines |
| Machine learning / AI-based prediction | Simple statistical aggregation from launch_events table is sufficient |
| Multiplayer billing redesign | Fix specific bugs (BILL-10, BILL-11) but don't rearchitect multiplayer flow |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| PODID-01 | Phase 194 | Pending |
| PODID-02 | Phase 194 | Pending |
| PODID-03 | Phase 194 | Pending |
| METRICS-01 | Phase 195 | Pending |
| METRICS-02 | Phase 195 | Pending |
| METRICS-03 | Phase 195 | Pending |
| METRICS-04 | Phase 195 | Pending |
| METRICS-05 | Phase 195 | Pending |
| METRICS-06 | Phase 195 | Pending |
| METRICS-07 | Phase 195 | Pending |
| LAUNCH-01 | Phase 196 | Pending |
| LAUNCH-02 | Phase 196 | Pending |
| LAUNCH-03 | Phase 196 | Pending |
| LAUNCH-04 | Phase 196 | Pending |
| LAUNCH-05 | Phase 196 | Pending |
| LAUNCH-06 | Phase 196 | Pending |
| LAUNCH-07 | Phase 196 | Pending |
| STATE-01 | Phase 196 | Pending |
| STATE-02 | Phase 196 | Pending |
| STATE-03 | Phase 196 | Pending |
| STATE-04 | Phase 196 | Pending |
| STATE-05 | Phase 196 | Pending |
| STATE-06 | Phase 196 | Pending |
| LAUNCH-08 | Phase 197 | Pending |
| LAUNCH-09 | Phase 197 | Pending |
| LAUNCH-10 | Phase 197 | Pending |
| LAUNCH-11 | Phase 197 | Pending |
| LAUNCH-12 | Phase 197 | Pending |
| LAUNCH-13 | Phase 197 | Pending |
| LAUNCH-14 | Phase 197 | Pending |
| LAUNCH-15 | Phase 197 | Pending |
| LAUNCH-16 | Phase 197 | Pending |
| LAUNCH-17 | Phase 197 | Pending |
| LAUNCH-18 | Phase 197 | Pending |
| LAUNCH-19 | Phase 197 | Pending |
| AC-01 | Phase 197 | Pending |
| AC-02 | Phase 197 | Pending |
| AC-03 | Phase 197 | Pending |
| AC-04 | Phase 197 | Pending |
| BILL-01 | Phase 198 | Pending |
| BILL-02 | Phase 198 | Pending |
| BILL-03 | Phase 198 | Pending |
| BILL-04 | Phase 198 | Pending |
| BILL-05 | Phase 198 | Pending |
| BILL-06 | Phase 198 | Pending |
| BILL-07 | Phase 198 | Pending |
| BILL-08 | Phase 198 | Pending |
| BILL-09 | Phase 198 | Pending |
| BILL-10 | Phase 198 | Pending |
| BILL-11 | Phase 198 | Pending |
| BILL-12 | Phase 198 | Pending |
| RECOVER-01 | Phase 199 | Pending |
| RECOVER-02 | Phase 199 | Pending |
| RECOVER-03 | Phase 199 | Pending |
| RECOVER-04 | Phase 199 | Pending |
| RECOVER-05 | Phase 199 | Pending |
| RECOVER-06 | Phase 199 | Pending |
| RECOVER-07 | Phase 199 | Pending |
| INTEL-01 | Phase 200 | Pending |
| INTEL-02 | Phase 200 | Pending |
| INTEL-03 | Phase 200 | Pending |
| INTEL-04 | Phase 200 | Pending |
| INTEL-05 | Phase 200 | Pending |

**Coverage:**
- v24.0 requirements: 63 total (corrected from initial estimate of 52)
- Categories: METRICS (7), PODID (3), LAUNCH (19), AC (4), BILL (12), RECOVER (7), INTEL (5), STATE (6)
- Mapped to phases: 63/63
- Unmapped: 0

---
*Requirements defined: 2026-03-26*
*Audit basis: game_launcher.rs, billing.rs, ws_handler.rs, event_loop.rs, ac_launcher.rs, game_process.rs, billing_guard.rs*
*Last updated: 2026-03-26 -- roadmap created, all requirements mapped to phases 194-200*
