# Phase 49: Session Lifecycle Autonomy - Context

**Gathered:** 2026-03-19
**Status:** Ready for planning

<domain>
## Phase Boundary

rc-agent autonomously handles session end-of-life — auto-ends orphaned billing after configurable timeout, resets pod to idle after session, pauses billing on game crash with auto-resume, and fast-reconnects WebSocket without full relaunch when server blips. Depends on Phase 46 (crash safety).

</domain>

<decisions>
## Implementation Decisions

### Orphan Detection & Auto-End
- rc-agent detects orphaned sessions locally (billing_active=true + no game_pid for configurable timeout)
- rc-agent calls local server HTTP API (`POST /api/v1/billing/{id}/end`) to end session — server remains authoritative for billing
- Cloud (admin.racingpoint.cloud, app.racingpoint.cloud) syncs automatically via existing cloud_sync.rs — no special cloud handling needed
- Timeout configured via `auto_end_orphan_session_secs` in rc-agent TOML config (default: 300s / 5 minutes)
- Two-tier detection: BILL-02 anomaly at 60s (early warning to server/dashboard), orphan auto-end at 5min (escalation that acts)
- On auto-end: skip session summary (nobody's watching), go straight to cleanup + PinEntry
- If server unreachable: retry 3 times with backoff (5s, 15s, 30s), then force-local-reset and queue deferred-end (memory-only, retried on next WS connect)
- Notification: new `AgentMessage::SessionAutoEnded` sent to server via WS — server logs it, shows in fleet health dashboard
- Add `end_reason` field to `billing_sessions` table (values: 'manual', 'orphan_timeout', 'crash_limit', etc.)
- `end_reason` included in cloud_sync push payload — Bono's admin dashboard can show auto-end badges

### Post-Session Reset Flow
- All session-ends (normal + auto) now reset pod to PinEntry state instead of ScreenBlanked
- Same 30s timing for both normal and orphan auto-end paths
- Normal end: show session summary → 30s later → PinEntry
- Orphan auto-end: skip summary → cleanup (FFB zero, game stop) → PinEntry within 30s
- Change existing blank_timer target from `show_blank_screen()` to `show_pin_entry()`

### Crash/Billing Pause Strategy
- On game crash during billing: rc-agent immediately pauses billing locally (within 5s), notifies server via API
- rc-agent attempts relaunch automatically
- "Failed relaunch" = game PID not detected within 60s of launch attempt (reuses CRASH-02 threshold from failure_monitor.rs)
- After 2 failed relaunches: auto-end session (same path as orphan auto-end)
- Replace existing crash_recovery_timer (30s → force-reset) entirely with new pause+relaunch flow
- Total max recovery time: ~2.5min (crash → pause → relaunch 1 (60s) → relaunch 2 (60s) → auto-end)
- Customer sees overlay message: "Game crashed — relaunching..." via existing overlay system
- After 2nd failed relaunch, overlay changes to "Session ending"
- On successful relaunch: resume billing, dismiss overlay

### WS Fast-Reconnect Window
- Add 30s grace period to WS reconnect loop — layered on top of self_monitor's 5-min relaunch
- Tiers: 0-30s silent reconnect, 30s-5min reconnect continues with state preserved, 5min+ full relaunch (existing self_monitor behavior)
- During 30s window: game, billing, and overlay all keep running — customer doesn't notice
- On reconnect within 30s: send full PodStateSnapshot to server for state reconciliation (reuses existing struct)
- Lock screen: don't show "Disconnected" state during 30s grace window — only show after 30s if still disconnected

### Claude's Discretion
- Exact retry backoff timing for deferred-end queue
- Internal state machine structure for crash recovery flow
- Whether to add a `billing_paused` field to FailureMonitorState or use a separate channel
- E2E test script implementation details for `session-lifecycle.sh`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Session & Billing
- `crates/rc-agent/src/billing_guard.rs` — BILL-02/BILL-03 anomaly detection, current thresholds, FailureMonitorState polling pattern
- `crates/rc-agent/src/main.rs` — SessionEnded handler (line ~1474), crash_recovery_timer (line ~1300), blank_timer (line ~1286), game crash detection (line ~1128)
- `crates/racecontrol/src/bot_coordinator.rs` — BillingAnomaly routing, recover_stuck_session(), handle_billing_anomaly()
- `crates/racecontrol/src/billing.rs` — end_billing_session_public(), billing API endpoints

### Pod State & Lock Screen
- `crates/rc-agent/src/lock_screen.rs` — LockScreenState enum (PinEntry, ScreenBlanked, etc.), show_pin_entry(), show_blank_screen(), show_session_summary()
- `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState struct, CRASH-02 detection, recovery_in_progress flag

### WS & Self-Monitor
- `crates/rc-agent/src/self_monitor.rs` — WS_DEAD_SECS (300s), relaunch_self(), CLOSE_WAIT detection
- `crates/rc-agent/src/udp_heartbeat.rs` — HeartbeatStatus struct (ws_connected, billing_active atomics)

### Cloud Sync
- `crates/racecontrol/src/cloud_sync.rs` — sync_push billing_sessions query, 30s push cycle

### E2E Testing
- `tests/e2e/lib/common.sh` — Shared test helpers (pass/fail/skip/info)
- `tests/e2e/api/` — Existing API pipeline test patterns

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `billing_guard.rs`: Already has BILL-02 detection loop with `game_gone_since` timer — extend for orphan auto-end at 5min
- `FailureMonitorState` (watch channel): Shared state struct with billing_active, game_pid, recovery_in_progress — add billing_paused field
- `PodStateSnapshot`: Exists for crash debugging — reuse for WS reconnect state sync
- `overlay.rs`: Has `show_message()` / `deactivate()` — use for crash recovery UX
- `lock_screen.rs`: `show_pin_entry()` already exists — just change blank_timer target

### Established Patterns
- Watch channel pattern: FailureMonitorState broadcast via `tokio::sync::watch` — all monitors observe shared state
- AgentMessage pattern: New message types (like SessionAutoEnded) follow existing enum variants in rc_common::protocol
- Timer pattern: `Pin<Box<Sleep>>` with armed flag — used by blank_timer and crash_recovery_timer

### Integration Points
- billing_guard.rs → Add orphan auto-end escalation after BILL-02 detection
- main.rs event loop → Replace crash_recovery_timer with pause+relaunch state machine
- main.rs WS connect → Add 30s grace period before showing Disconnected
- main.rs SessionEnded handler → Change blank_timer target to PinEntry
- rc_common::protocol → Add SessionAutoEnded, BillingPaused, BillingResumed message variants
- billing_sessions schema → Add end_reason column, update cloud_sync push query

</code_context>

<specifics>
## Specific Ideas

- Orphan auto-end should feel invisible to staff — it just happens, they see a dashboard log entry
- The 30s WS grace window is critical for the venue's flaky WiFi — brief drops shouldn't disrupt customer sessions
- End_reason field enables Uday to audit how many sessions end abnormally vs normally from the admin dashboard

</specifics>

<deferred>
## Deferred Ideas

- Dashboard notifications/alerts for auto-ended sessions (push notification to Uday's phone) — separate phase
- Configurable relaunch attempt count (currently hardcoded to 2) — add if needed later
- Customer-facing PWA notification when their session is auto-ended — separate phase

</deferred>

---

*Phase: 49-session-lifecycle-autonomy*
*Context gathered: 2026-03-19*
