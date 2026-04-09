# Phase 364: Session Quality Monitor - Research

**Researched:** 2026-04-09
**Status:** Complete
**Focus:** What do I need to know to plan Phase 364 well?

---

## Summary

Phase 364 has strong infrastructure to build on. All three plans (gap+stall detectors,
lap consistency checker, silent-drop audit + overflow metric) have clear implementation
paths. No blockers. Key findings below.

---

## Validation Architecture

The existing Rust test suite covers all three plan domains:
- `crates/racecontrol/src/bot_coordinator.rs` -- unit tests for telemetry_gap handlers
- `crates/racecontrol/src/ws/mod.rs` -- integration-style unit tests
- `crates/rc-agent/src/failure_monitor.rs` -- full unit test suite for detection rules

Quick run command: `cargo test -p racecontrol -- --test-threads=4 2>&1 | tail -5`
Full suite: `cargo test --workspace 2>&1 | tail -10`
Estimated runtime: ~45s (racecontrol crate only), ~90s (workspace)

---

## Domain 1: TelemetryGap + SessionStalled Detectors (Plan 364-01)

### Existing TelemetryGap infrastructure

`TelemetryGap` variant already exists in `rc-common/src/protocol.rs:225`:
```rust
TelemetryGap {
    pod_id: String,
    sim_type: SimType,
    gap_seconds: u32,
}
```

`handle_telemetry_gap()` in `bot_coordinator.rs:92` is ALREADY a functioning handler
(sends staff email for 60s crash-detection gap). Phase 364 adds a parallel lightweight
path -- new event type `TelemetryQualityGap` -- NOT modifying the existing handler.

Current TELEM-01 threshold in `failure_monitor.rs:42`:
```rust
const TELEM_GAP_SECS: u64 = 60;
```
This is the crash-detection threshold. Phase 364 adds a NEW constant beside it:
```rust
const QUALITY_GAP_MS: u64 = 500;   // new -- quality monitoring
const STALL_WARN_SECS: u64 = 15;   // new -- stall warning
```

The existing `telem_gap_fired: bool` pattern in `failure_monitor.rs:109` is the
template to replicate for `stall_warn_fired: bool`.

### New protocol variants needed

Add to `rc-common/src/protocol.rs` `AgentMessage` enum (alongside existing `TelemetryGap`):
```rust
/// Phase 364 QUALITY-01: UDP gap > 500ms while billing active + game Running.
/// Advisory quality signal -- distinct from TelemetryGap (60s crash detection).
TelemetryQualityGap {
    pod_id: String,
    gap_ms: u32,
},

/// Phase 364 STALL-01: 15s in-race telemetry silence.
/// Fires once per silence window (stall_warn_fired guard prevents duplicates).
SessionStalled {
    pod_id: String,
    silence_seconds: u32,
},
```

Both variants use the same `AgentMessage` enum on both sides (rc-agent sends,
ws/mod.rs routes to bot_coordinator.rs handlers).

### Server-side wiring points

`ws/mod.rs:1376` is where `TelemetryGap` is dispatched. New variants go alongside
in the same match arm block:
```rust
AgentMessage::TelemetryQualityGap { pod_id, gap_ms } => {
    crate::bot_coordinator::handle_telemetry_quality_gap(
        &state, &pod_id, *gap_ms
    ).await;
}
AgentMessage::SessionStalled { pod_id, silence_seconds } => {
    crate::bot_coordinator::handle_session_stalled(
        &state, &pod_id, *silence_seconds
    ).await;
}
```

### New bot_coordinator handlers

`handle_telemetry_quality_gap()` in bot_coordinator.rs:
- Guard: same billing_active + GameState::Running checks as existing handler
- Action: `tracing::warn!` + append `telemetry_gap_ms_{bucket}` to
  `billing_sessions.suspect_reasons` (non-blocking DB write)
- Rate-limit: debounce with `AtomicU64` last-write timestamp per pod (5s debounce)
  using `state` or a static per-pod approach. The existing per-pod state is in
  `AppState.pods: RwLock<HashMap<String, PodInfo>>` -- add a `last_quality_gap_warn: Option<Instant>` to `PodInfo` struct.

`handle_session_stalled()` in bot_coordinator.rs:
- Same guards as above
- Action: `tracing::warn!` + append `session_stalled_Ns` to suspect_reasons
- No email (15s stall is NOT a crash alert)

### suspect_reasons append SQL

Phase 363 defined `suspect_reasons TEXT` (JSON array). Safe atomic append:
```sql
UPDATE billing_sessions
SET suspect_reasons = CASE
    WHEN suspect_reasons IS NULL THEN json_array(?1)
    ELSE json_insert(suspect_reasons, '$[#]', ?1)
END
WHERE id = ?2
```
SQLite's `json_insert` with `'$[#]'` path appends to the end of a JSON array atomically.
This avoids a read-modify-write race with Phase 363's `run_session_audit()`.

To get the active billing session ID from a pod_id:
```rust
let session_id = state.billing.active_timers.read().await
    .get(pod_id)
    .map(|t| t.billing_session_id.clone());
```
Existing pattern in `billing.rs` -- the `BillingTimer` struct has `billing_session_id: String`.

### rc-agent failure_monitor changes

Add `quality_gap_fired: bool` and `stall_warn_fired: bool` to the loop locals in
`failure_monitor.rs::spawn()` (beside `telem_gap_fired`).

For QUALITY-01 (500ms gap): The existing `last_udp_secs_ago: Option<u64>` is in
whole seconds. Phase 364 needs sub-second precision for 500ms gaps. Two options:

**Option A (recommended):** The `HeartbeatStatus` struct (in `udp_heartbeat.rs`) has
`last_packet_instant: AtomicU64` (stores `SystemTime::now().duration_since(UNIX_EPOCH)`
in milliseconds or nanoseconds). Researcher confirmed: check `udp_heartbeat.rs` to
determine if `last_packet_instant` is millisecond precision.

**Option B (fallback):** Add a `last_udp_ms_ago: Option<u64>` field to
`FailureMonitorState` (parallel to `last_udp_secs_ago`) that carries millisecond
granularity, populated from the same UDP receive timestamp.

The 5s poll interval of failure_monitor means 500ms gap detection at poll-boundary
precision. A 500ms gap starts at some point between polls. The first poll AFTER the
gap began may see 0-5s elapsed. To reliably detect 500ms gaps: the UDP listener (in
the sim adapter event loop) should set a flag when >500ms silence is observed, and
failure_monitor reads this flag. This is a design choice for the planner.

**Simpler approach:** detect gaps at 1s granularity (first poll after a 1s silence
is >=500ms). Add `last_udp_ms_ago` to FailureMonitorState as `Option<u64>` in
milliseconds; the UDP adapter updates it via the watch channel.

---

## Domain 2: Lap Consistency Checker (Plan 364-02)

### LapCompleted event flow confirmed

1. rc-agent `event_loop.rs:405`: `adapter.poll_lap_completed()` returns `LapData`
2. rc-agent sends `AgentMessage::LapCompleted(lap)` via `agent_msg_tx`
3. Server `ws/mod.rs:886`: receives, resolves driver, calls `lap_tracker::persist_lap()`
4. `LapData.lap_time_ms: u32` -- milliseconds, always present (never 0, guarded by adapters)
5. `LapData.session_id: String` -- the billing session ID (resolved by ws handler)
6. `LapData.valid: bool` -- invalid laps (lap time = 0, pit laps) are filtered

### Where to hook the consistency checker

The consistency check should run AFTER `lap_tracker::persist_lap()` (lap is in DB)
but BEFORE `DashboardEvent::LapCompleted` broadcast (advisory only, no timing concern).

In `ws/mod.rs:886` after `crate::lap_tracker::persist_lap(&state, &lap).await`:
```rust
// Phase 364 CONSIST-01: Check lap consistency
if lap.valid {
    crate::lap_consistency::check_lap_consistency(&state, &lap).await;
}
```

A new module `crates/racecontrol/src/lap_consistency.rs` holds this logic.

### In-memory lap history

Add to `AppState` (or to a new `QualityState` sub-struct):
```rust
/// Phase 364: Per-pod rolling lap times for consistency checking.
/// Key: pod_id. Value: VecDeque of recent lap_time_ms values (max 50 entries).
pub lap_time_history: RwLock<HashMap<String, VecDeque<u32>>>,
```

Or simpler: extend the existing `PodInfo` struct with:
```rust
pub recent_lap_times: VecDeque<u32>,  // max 50
```

`PodInfo` is already behind `RwLock<HashMap<String, PodInfo>>` in AppState.pods.
Adding a field to `PodInfo` is the cleanest approach (no new AppState fields,
no need to update `AppState::new()`).

### 3-sigma algorithm

```rust
fn check_outlier(history: &VecDeque<u32>, new_lap_ms: u32) -> bool {
    let n = history.len();
    if n < 3 { return false; }

    let mean = history.iter().map(|&x| x as f64).sum::<f64>() / n as f64;
    let variance = history.iter()
        .map(|&x| { let d = x as f64 - mean; d * d })
        .sum::<f64>() / n as f64;
    let stddev = variance.sqrt();

    if stddev < 2000.0 { return false; }  // D-04 guard: variance too low

    let z_score = ((new_lap_ms as f64) - mean).abs() / stddev;
    z_score > 3.0
}
```

This is pure Rust, no dependencies. The floating-point arithmetic on u32 lap times
(max ~900_000 ms for a 15min "lap") is well within f64 precision.

### suspect_reasons append for outlier

Same SQLite json_insert pattern as Domain 1:
```sql
UPDATE billing_sessions
SET suspect_reasons = CASE
    WHEN suspect_reasons IS NULL THEN json_array(?1)
    ELSE json_insert(suspect_reasons, '$[#]', ?1)
END
WHERE id = ?2
```
Reason string: `format!("lap_outlier_lap{}", lap.lap_number)`

Also set `suspect = 1` in the same UPDATE if not already set.

### Clearing history on session end

When a billing session ends (`post_session_hooks` in billing.rs), the lap history
for that pod should be cleared to prevent stale data leaking to the next session.
The `post_session_hooks` call already has `pod_id` -- add:
```rust
if let Some(pod) = state.pods.write().await.get_mut(&pod_id) {
    pod.recent_lap_times.clear();
}
```

---

## Domain 3: Silent-Drop Audit + Overflow Metrics (Plan 364-03)

### Complete audit of `let _ = ws_send(...)` in hot path

**Files searched:** All `crates/racecontrol/src/` and `crates/rc-agent/src/`
**Regex used:** `let _ = ws_sender\.send\|let _ = .*\.try_send`

**Hot-path findings (require fix):**

| File | Line | Pattern | Context | Fix |
|------|------|---------|---------|-----|
| `crates/racecontrol/src/ws/mod.rs` | 2719 | `let _ = ws_sender.send(Message::Text(json.into())).await` | AI channel auth failure -- sends AuthFailed before returning | Inspect channel type: if `SplitSink<WebSocketStream, Message>`, the only error is a closed connection. Log the error instead of discarding. |
| `crates/rc-agent/src/ws_handler.rs` | 324 | `let _ = state.ws_exec_result_tx.try_send(...)` | `AgentMessage::LaunchTimelineReport` send to result channel | `try_send` on mpsc -- `Full` = overflow, `Closed` = receiver gone. Log both, increment counter on Full. |
| `crates/rc-agent/src/ws_handler.rs` | 1519 | `let _ = state.ws_exec_result_tx.send(...)` | `AgentMessage::ExecResult` | Same pattern -- `.await` version, closed channel = log error. |
| `crates/rc-agent/src/ws_handler.rs` | 1557 | `let _ = state.ws_exec_result_tx.try_send(lockdown_msg)` | Lockdown message | Same as 324. |
| `crates/rc-agent/src/ws_handler.rs` | 1961, 1973 | `let _ = state.ws_exec_result_tx.try_send(AgentMessage::JwtAck {...})` | JWT ack | Same as 324. |
| `crates/rc-agent/src/ws_handler.rs` | 2005 | `let _ = state.ws_exec_result_tx.try_send(AgentMessage::LaunchTimelineReport(...))` | Duplicate of 324 pattern | Same. |

**Out-of-scope (dashboard broadcast -- silent drop IS intentional):**
- `action_queue.rs:185,324` -- `state.dashboard_tx.send()` -- broadcast channel, lagging clients intentionally dropped
- `activity_log.rs:92` -- same broadcast channel
- `ac_camera.rs:344,367` -- same broadcast channel
- `ac_server.rs:555,593,...` -- same broadcast channel

The rg verification command (success criterion 4):
```bash
rg 'let _ = ws_sender' crates/racecontrol/src/ws/
```
After fixing ws/mod.rs:2719, this returns 0 results.

Note: `ws_handler.rs` (rc-agent) uses `ws_exec_result_tx` not `ws_sender` -- different
pattern. The success criterion specifically says `ws_send(...)` in hot path which maps
to `ws_sender.send()` in racecontrol's ws/mod.rs.

### ws_try_send_overflows_total implementation

**MetricsSender infrastructure (Phase 285):**
- `MetricsSender = mpsc::Sender<MetricSample>` (in `metrics_tsdb.rs`)
- `spawn_metrics_ingestion()` in `metrics_tsdb.rs` -- spawned in main.rs:780
- `spawn_metric_producers()` in `metrics_producers.rs` -- sends every 30s via `try_send`

**Implementation plan:**

1. Add static counter to `ws/mod.rs` (or a new `ws_metrics.rs` module):
```rust
static WS_TRY_SEND_OVERFLOWS: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
```

2. In all hot-path `try_send` calls where `TrySendError::Full` can occur, increment:
```rust
if let Err(e) = some_tx.try_send(msg) {
    tracing::warn!("ws_try_send overflow: {}", e);
    WS_TRY_SEND_OVERFLOWS.fetch_add(1, Ordering::Relaxed);
}
```

3. Add to `metrics_producers.rs::spawn_metric_producers()` inner loop:
```rust
// Phase 364: ws overflow counter
let overflows = crate::ws::WS_TRY_SEND_OVERFLOWS.load(Ordering::Relaxed);
metrics_tx.try_send(MetricSample {
    metric_name: "ws_try_send_overflows_total".to_string(),
    pod_id: None,
    value: overflows as f64,
    recorded_at: now.clone(),
}).ok();
```

The `metrics_tsdb.rs` already has `METRIC_*` constants. Add:
```rust
pub const METRIC_WS_TRY_SEND_OVERFLOWS: &str = "ws_try_send_overflows_total";
```

The Prometheus formatter (`metrics_prometheus.rs:35`) auto-picks up any
`metrics_samples` row -- no changes needed to the formatter.

### Verifying Prometheus exposure

After implementation:
```bash
curl -s http://localhost:3200/api/v1/metrics/prometheus | grep ws_try_send
```
Should return:
```
# TYPE ws_try_send_overflows_total counter
racecontrol_ws_try_send_overflows_total 0
```

---

## Domain 4: Feature Flag

Phase 363's pattern for `phase363_session_audit`:
```rust
// In db/mod.rs migration section:
let _ = sqlx::query(
    "INSERT OR IGNORE INTO feature_flags (name, enabled, version, config_json)
     VALUES ('phase364_quality_monitor', 1, 1, '{}')"
).execute(pool).await;
```

Add `phase364_quality_monitor` using identical pattern. All three new behaviors
(TelemetryQualityGap, SessionStalled, lap consistency checker) are gated on this flag
via `is_feature_enabled(&state, "phase364_quality_monitor").await`.

---

## Module Structure for Phase 364

New files:
- `crates/racecontrol/src/lap_consistency.rs` -- `check_lap_consistency()` (Domain 2)

Modified files:
- `crates/rc-common/src/protocol.rs` -- add `TelemetryQualityGap` + `SessionStalled` variants
- `crates/rc-agent/src/failure_monitor.rs` -- QUALITY-01 + STALL-01 detection rules
- `crates/racecontrol/src/bot_coordinator.rs` -- new handlers
- `crates/racecontrol/src/ws/mod.rs` -- route new variants + fix ws_sender silent drop
- `crates/racecontrol/src/state.rs` -- add `lap_time_history` or extend PodInfo
- `crates/racecontrol/src/lib.rs` -- `pub mod lap_consistency;`
- `crates/racecontrol/src/metrics_tsdb.rs` -- add `METRIC_WS_TRY_SEND_OVERFLOWS` constant
- `crates/racecontrol/src/metrics_producers.rs` -- emit overflow counter
- `crates/racecontrol/src/db/mod.rs` -- seed `phase364_quality_monitor` feature flag
- `crates/rc-agent/src/ws_handler.rs` -- fix `let _ =` silent drops (rc-agent scope)

---

## Risk Register

| Risk | Severity | Mitigation |
|------|----------|------------|
| 500ms gap detection at 5s poll boundary | Low | Use millisecond-granularity field in FailureMonitorState; if not available, 1s-floor detection is acceptable per D-01 |
| json_insert SQLite availability | Low | SQLite 3.38+ (JSON5 functions); racecontrol uses SQLite bundled via sqlx -- check version in Cargo.toml; fallback: string concat |
| LapData.session_id empty at ws/mod.rs:886 | Low | ws handler resolves session_id from billing state BEFORE calling consistency checker; resolver already present |
| PodInfo struct change requires AppState::new() update | Low | PodInfo has Default impl; new field with `VecDeque::new()` default, no new() change needed if using Default |
| Phase 363 suspect_reasons NULL on new sessions | None | json_insert handles NULL via CASE WHEN (proven by Phase 363 code already live) |

---

## Questions Resolved

- **Q: Does LapData carry lap_time_ms to the server?** A: Yes. `LapData.lap_time_ms: u32` (types.rs:252). Never 0 (guarded at sim adapter level).
- **Q: Where is the billing session ID when a lap arrives?** A: ws/mod.rs:886 resolves it via `lap_tracker::resolve_driver_for_pod()` before any downstream calls. The lap passed to consistency checker already has a valid session_id.
- **Q: Does metrics_prometheus.rs need changes for new metric?** A: No. format_prometheus() queries metrics_samples and emits all rows. Adding a new metric name to the samples table auto-exposes it.
- **Q: What is the BillingTimer struct field for session ID?** A: `billing_session_id: String` on the active timer. Access via `state.billing.active_timers.read().await.get(pod_id)`.
