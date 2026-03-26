---
phase: 206-observable-state-transitions
plan: 02
subsystem: fleet-ops
tags: [rust, sentinel-watcher, fleet-health, websocket, whatsapp, notify, observable-state]

# Dependency graph
requires:
  - phase: 206-01
    provides: Config fallback warn! logging foundation — tracing target:'state' pattern established

provides:
  - AgentMessage::SentinelChange variant in rc-common/src/protocol.rs
  - DashboardEvent::SentinelChanged variant with active_sentinels field
  - sentinel_watcher.rs using notify 8.2.0 RecommendedWatcher watching C:\RacingPoint\ for 4 sentinel files
  - active_sentinels: Vec<String> in FleetHealthStore + PodFleetStatus (GET /api/v1/fleet/health)
  - SentinelChange WS handler in racecontrol ws/mod.rs
  - DashboardEvent::SentinelChanged broadcast on every sentinel change
  - MAINTENANCE_MODE creation triggers WhatsApp alert to Uday (rate-limited 5 min per sentinel per pod)
  - SENTINEL_ALERT_COOLDOWN static in ws/mod.rs

affects: [fleet-health-api, dashboard-ws, whatsapp-alerter, 207, 208, 210, debugging-silent-failures]

# Tech tracking
tech-stack:
  added:
    - "notify 8.2.0 (rc-agent Cargo.toml) — ReadDirectoryChangesW-based file system watcher"
  patterns:
    - "sentinel_watcher uses std::thread::spawn (not tokio) — notify RecommendedWatcher requires sync context"
    - "SentinelChange routed via ws_exec_result_tx channel (same path as exec results) — no new channel needed"
    - "SENTINEL_ALERT_COOLDOWN: std::sync::LazyLock<Mutex<HashMap>> — per sentinel+pod rate limiting"
    - "active_sentinels persists across WS disconnect (sentinel files stay on disk)"

key-files:
  created:
    - crates/rc-agent/src/sentinel_watcher.rs
  modified:
    - crates/rc-common/src/protocol.rs
    - crates/rc-agent/Cargo.toml
    - crates/rc-agent/src/main.rs
    - crates/racecontrol/src/fleet_health.rs
    - crates/racecontrol/src/ws/mod.rs

key-decisions:
  - "sentinel_watcher uses std::thread::spawn not tokio::spawn — notify RecommendedWatcher requires a sync recv loop; blocking_send bridges to async tokio channel"
  - "SentinelChange messages routed via ws_exec_result_tx (existing AgentMessage mpsc channel) — avoids new channel, consistent with how exec results are forwarded"
  - "active_sentinels NOT cleared on WS disconnect — sentinel files persist on disk; they will re-sync when agent reconnects and sentinel_watcher detects existing files"
  - "DashboardEvent::SentinelChanged added as new variant (not reusing PodUpdate) — carries sentinel-specific fields (file, action, active_sentinels) for dashboard without requiring PodInfo rebuild"
  - "SENTINEL_ALERT_COOLDOWN uses std::sync::LazyLock (stable Rust 1.80+) matching existing SECURITY_ALERT_DEBOUNCE pattern in whatsapp_alerter.rs"
  - "pod_number extracted via strip_prefix('pod_') — consistent with existing normalize_pod_id pattern"

patterns-established:
  - "Pattern: sentinel watcher thread — std::thread::spawn + notify RecommendedWatcher + blocking_send to tokio mpsc"
  - "Pattern: fleet health extension — add field to FleetHealthStore + PodFleetStatus + update helper + handler arm + fleet_health_handler population"

requirements-completed: [OBS-04, OBS-01]

# Metrics
duration: 55min
completed: 2026-03-26
---

# Phase 206 Plan 02: Observable State Transitions — Sentinel File Watcher Summary

**Sentinel file changes are now instantly observable: every create/delete in C:\RacingPoint\ produces a WS message to racecontrol within 1 second, updates active_sentinels in fleet health API, broadcasts DashboardEvent::SentinelChanged, and MAINTENANCE_MODE creation fires WhatsApp alert to Uday with 5-min rate limiting**

## Performance

- **Duration:** ~55 min
- **Started:** 2026-03-26T05:00:00Z (estimated)
- **Completed:** 2026-03-26T05:55:00Z (estimated)
- **Tasks:** 2
- **Files modified:** 5 (+ 1 created)

## Accomplishments

- rc-common/src/protocol.rs: `AgentMessage::SentinelChange { pod_id, file, action, timestamp }` variant added before Unknown catch-all
- rc-common/src/protocol.rs: `DashboardEvent::SentinelChanged { pod_id, pod_number, file, action, timestamp, active_sentinels }` variant added for real-time dashboard broadcast
- rc-agent/Cargo.toml: `notify = "8.2.0"` added under dependencies
- crates/rc-agent/src/sentinel_watcher.rs: new module (137 LOC) — watches C:\RacingPoint\ for 4 known sentinel files using notify 8.2.0 RecommendedWatcher; emits AgentMessage::SentinelChange via blocking_send; MAINTENANCE_MODE creation emits eprintln! immediately; IST timestamp via FixedOffset::east_opt (no unwrap)
- crates/rc-agent/src/main.rs: `mod sentinel_watcher;` added; `sentinel_watcher::spawn(state.ws_exec_result_tx.clone(), state.pod_id.clone())` called after process guard spawn and before reconnect loop
- crates/racecontrol/src/fleet_health.rs: `active_sentinels: Vec<String>` added to `FleetHealthStore` and `PodFleetStatus`; `update_sentinel()` helper updates list on create/delete; `get_active_sentinels()` helper; both unregistered and registered pod paths in `fleet_health_handler` include `active_sentinels`; `clear_on_disconnect()` does NOT clear sentinels (persist on disk)
- crates/racecontrol/src/ws/mod.rs: `SENTINEL_ALERT_COOLDOWN` LazyLock static + `check_sentinel_cooldown()` rate-limiter added at top; `AgentMessage::SentinelChange` handler arm added before catch-all; handler updates fleet store, broadcasts `DashboardEvent::SentinelChanged`, triggers WhatsApp alert on MAINTENANCE_MODE with cooldown check

## Task Commits

Each task was committed atomically:

1. **Task 1: SentinelChange protocol + notify dep + sentinel_watcher.rs** - `af09e863` (feat)
2. **Task 2: Server-side handler + fleet health active_sentinels + WA alert** - `8674ee58` (feat)
3. **Deviation fix: remove unwrap in sentinel_watcher IST timestamp** - `bb5897b3` (fix)

## Files Created/Modified

- `crates/rc-agent/src/sentinel_watcher.rs` - NEW: notify 8.2.0 watcher for C:\RacingPoint\; 4 known sentinel files; AgentMessage::SentinelChange via blocking_send; MAINTENANCE_MODE eprintln!
- `crates/rc-common/src/protocol.rs` - AgentMessage::SentinelChange + DashboardEvent::SentinelChanged variants added
- `crates/rc-agent/Cargo.toml` - notify = "8.2.0" dependency added
- `crates/rc-agent/src/main.rs` - mod sentinel_watcher; spawn call added after process guard block
- `crates/racecontrol/src/fleet_health.rs` - active_sentinels in FleetHealthStore + PodFleetStatus; update_sentinel() + get_active_sentinels() helpers; fleet_health_handler updated
- `crates/racecontrol/src/ws/mod.rs` - SentinelChange handler arm; SENTINEL_ALERT_COOLDOWN; DashboardEvent::SentinelChanged broadcast; MAINTENANCE_MODE WhatsApp alert

## Decisions Made

- sentinel_watcher uses `std::thread::spawn` (not `tokio::spawn`) because notify RecommendedWatcher requires a sync blocking recv loop. `blocking_send()` bridges back to the async tokio mpsc channel safely.
- SentinelChange messages routed via existing `ws_exec_result_tx` channel — same mpsc that carries exec results and guard violations. No new channel needed.
- `active_sentinels` NOT cleared on WS disconnect — sentinel files persist on disk across reconnections; clear_on_disconnect would cause the UI to show stale "no sentinels" until next change event.
- `DashboardEvent::SentinelChanged` is a new dedicated variant carrying sentinel-specific fields, not a reuse of `PodUpdate`. This allows dashboard clients to react specifically to sentinel changes without rebuilding full PodInfo.
- `SENTINEL_ALERT_COOLDOWN` uses `std::sync::LazyLock` (stable since Rust 1.80) matching the existing `SECURITY_ALERT_DEBOUNCE` pattern in whatsapp_alerter.rs.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] .unwrap() in IST timestamp construction in sentinel_watcher.rs**
- **Found during:** Pre-commit hook warning after Task 1 commit
- **Issue:** `east_opt(5h30m).unwrap_or(east_opt(0).unwrap())` — standing rule prohibits .unwrap() in production Rust
- **Fix:** Replaced with `match east_opt(IST_OFFSET_SECS) { Some(ist) => ..., None => Utc::now()... }` — east_opt(19800) always succeeds in practice, match makes the fallback explicit
- **Files modified:** crates/rc-agent/src/sentinel_watcher.rs
- **Commit:** bb5897b3

---

**Total deviations:** 1 auto-fixed (Rule 1 — unwrap in production code)
**Impact on plan:** Correctness improvement, no scope change.

## Issues Encountered

- No pre-existing test failures. All 190 rc-common tests + 66 racecontrol tests pass after changes.
- Only pre-existing warnings (dead code, unused vars in other modules). No new warnings introduced.

## Next Phase Readiness

- OBS-04 (sentinel file watcher) and OBS-01 (MAINTENANCE_MODE alert) requirements complete
- All 5 OBS requirements now complete (OBS-01, OBS-02, OBS-03, OBS-04, OBS-05)
- Phase 206 observable state transitions fully shipped
- Ready for Phase 207 (coverage or boot resilience)

---
*Phase: 206-observable-state-transitions*
*Completed: 2026-03-26*
