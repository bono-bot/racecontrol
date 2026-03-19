# Phase 49: Session Lifecycle Autonomy - Research

**Researched:** 2026-03-19
**Domain:** Rust/Tokio async state machines, rc-agent billing lifecycle, WebSocket reconnect, E2E shell testing
**Confidence:** HIGH

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Orphan Detection & Auto-End**
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

**Post-Session Reset Flow**
- All session-ends (normal + auto) now reset pod to PinEntry state instead of ScreenBlanked
- Same 30s timing for both normal and orphan auto-end paths
- Normal end: show session summary → 30s later → PinEntry
- Orphan auto-end: skip summary → cleanup (FFB zero, game stop) → PinEntry within 30s
- Change existing blank_timer target from `show_blank_screen()` to `show_pin_entry()`

**Crash/Billing Pause Strategy**
- On game crash during billing: rc-agent immediately pauses billing locally (within 5s), notifies server via API
- rc-agent attempts relaunch automatically
- "Failed relaunch" = game PID not detected within 60s of launch attempt (reuses CRASH-02 threshold from failure_monitor.rs)
- After 2 failed relaunches: auto-end session (same path as orphan auto-end)
- Replace existing crash_recovery_timer (30s → force-reset) entirely with new pause+relaunch state machine
- Total max recovery time: ~2.5min (crash → pause → relaunch 1 (60s) → relaunch 2 (60s) → auto-end)
- Customer sees overlay message: "Game crashed — relaunching..." via existing overlay system
- After 2nd failed relaunch, overlay changes to "Session ending"
- On successful relaunch: resume billing, dismiss overlay

**WS Fast-Reconnect Window**
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

### Deferred Ideas (OUT OF SCOPE)
- Dashboard notifications/alerts for auto-ended sessions (push notification to Uday's phone) — separate phase
- Configurable relaunch attempt count (currently hardcoded to 2) — add if needed later
- Customer-facing PWA notification when their session is auto-ended — separate phase
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SESSION-01 | Orphaned billing auto-end: billing_active=true + no game_pid for `auto_end_orphan_session_secs` (default 300s) → rc-agent calls server API to end, sends SessionAutoEnded WS message, resets pod to PinEntry | billing_guard.rs BILL-02 pattern extends naturally; reqwest HTTP call to `/api/v1/billing/{id}/end`; existing AgentMessage enum extended |
| SESSION-02 | Post-session reset: 30s after ANY session end (normal or orphan), pod returns to PinEntry not ScreenBlanked; blank_timer target changed | blank_timer + blank_timer_armed pattern in main.rs already exists; only target changes from show_blank_screen() to show_pin_entry() |
| SESSION-03 | Crash/billing pause+relaunch: crash → pause billing (5s) → relaunch attempts (2x60s each) → auto-end on 2nd failure; overlay shows status throughout | crash_recovery_timer replaced with CrashRecoveryState enum; billing_paused field in FailureMonitorState |
| SESSION-04 | WS fast-reconnect: 30s grace window before showing Disconnected or triggering relaunch; reconnect within 30s sends PodStateSnapshot for reconciliation | ws_disconnected_at Instant added to reconnect loop outer scope; ws_connected flag already exists in HeartbeatStatus |
</phase_requirements>

---

## Summary

Phase 49 adds autonomous session lifecycle management to rc-agent. All four requirements build on existing patterns already established in the codebase — no new frameworks or major architectural changes are needed.

The most complex change is SESSION-03 (crash recovery state machine), which replaces the blunt 30s crash_recovery_timer with a structured state machine that pauses billing, attempts up to 2 game relaunches each with a 60s window, and auto-ends only after exhausting relaunches. The billing_guard.rs extension for SESSION-01 is straightforward: the existing `game_gone_since` timer already tracks the condition; the new code adds a second escalation tier at 5 minutes that calls the server API directly instead of just sending an anomaly.

SESSION-02 is a one-line change (blank_timer target) plus a 30s reset instead of 15s. SESSION-04 requires tracking when WS disconnected in the outer reconnect loop scope (not inside the inner event loop) and suppressing `show_disconnected()` during the 30s grace window. All four features must also add the `end_reason` column to `billing_sessions` (schema migration) and new protocol message variants.

**Primary recommendation:** Implement in two plans — Plan 01 covers SESSION-01 + SESSION-02 + schema/protocol changes (low-risk, read-only from server's perspective). Plan 02 covers SESSION-03 + SESSION-04 (higher-risk state machine surgery on main.rs event loop).

---

## Standard Stack

### Core (already in use — no new dependencies needed)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x | Async runtime, timers (Sleep), channels | Already the rc-agent runtime |
| tokio::sync::watch | 1.x | FailureMonitorState broadcast | Established pattern for shared state |
| tokio::sync::mpsc | 1.x | AgentMessage channel to WS sender | Established pattern across all monitors |
| reqwest | 0.12.x | HTTP client for orphan auto-end API call | Already used in self_monitor for Ollama |
| serde_json | 1.x | Protocol serialization | Already used everywhere |
| sqlx | 0.8.x | DB migration for end_reason column | Already used in racecontrol crate |

### No New Dependencies

All functionality uses existing crate dependencies. The reqwest client for orphan auto-end should follow the `OnceLock<reqwest::Client>` pattern from `self_monitor.rs` — static client, no per-call construction.

**Installation:** No new packages. No `Cargo.toml` changes needed.

---

## Architecture Patterns

### Recommended File Touch Map

```
crates/rc-common/src/protocol.rs       — Add 3 new AgentMessage variants
crates/rc-agent/src/billing_guard.rs   — Add SESSION-01 orphan auto-end escalation
crates/rc-agent/src/failure_monitor.rs — Add billing_paused field to FailureMonitorState
crates/rc-agent/src/main.rs            — SESSION-02 blank_timer target + SESSION-03 state machine + SESSION-04 WS grace
crates/racecontrol/src/billing.rs      — Add end_reason param to end_billing_session_public()
crates/racecontrol/src/migrations/     — Add end_reason column to billing_sessions
crates/racecontrol/src/cloud_sync.rs   — Include end_reason in sync_push query
tests/e2e/api/session-lifecycle.sh     — New E2E test script
```

### Pattern 1: Orphan Auto-End Escalation (SESSION-01)

**What:** billing_guard.rs already has a `game_gone_since` timer that fires BILL-02 at 60s. Add a second phase that fires at `auto_end_orphan_session_secs` (300s default) and makes an HTTP POST to end the session directly.

**Key insight:** The billing session ID is currently "unknown" in BILL-02 because the guard doesn't know it — for auto-end, rc-agent must resolve the session ID by querying `GET /api/v1/billing/active?pod_id={pod_id}` first, then POST to `/api/v1/billing/{id}/end`.

**State to add to billing_guard.rs:**
```rust
// Task-local (no struct change needed in billing_guard — these are loop-local)
let mut orphan_fired = false;           // prevents duplicate auto-end
// Uses existing game_gone_since timer, just adds second threshold check
```

**How it calls the server:**
```rust
// HTTP client — static OnceLock like self_monitor does
static ORPHAN_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

async fn attempt_orphan_end(core_base_url: &str, pod_id: &str) -> bool {
    // Step 1: GET /api/v1/billing/active?pod_id=pod_X → extract session_id
    // Step 2: POST /api/v1/billing/{session_id}/end → server ends it
    // Returns true if succeeded
}
```

**Retry logic (Claude's discretion — recommended):** 3 attempts, delays [5s, 15s, 30s]. On all-fail: force-local-reset and queue deferred-end as `Option<String>` (the session_id) in billing_guard task-local state, checked every poll when WS is confirmed alive.

**billing_guard.rs spawn signature change:** Must receive `core_base_url: String` and `pod_id: String` (pod_id already there, base_url is new) to construct HTTP URLs.

### Pattern 2: blank_timer Target Change (SESSION-02)

**Current code (main.rs ~line 1292):**
```rust
lock_screen.show_blank_screen();
```

**Changed to:**
```rust
lock_screen.show_pin_entry(/* token_id, driver_name, tier, secs — need defaults or stored values */);
```

**Critical detail:** `show_pin_entry()` requires a `LockScreenState::PinEntry` with `token_id`, `driver_name`, `pricing_tier_name`, `allocated_seconds`, `pin_error`. After session end there is no active booking token. The correct approach is to call `show_pin_entry_idle()` or use `lock_screen.show_blank_screen()` and then `show_pin_entry(...)` with the NEXT queued booking if one exists — OR define a new `PinEntry` state that allows empty/idle parameters.

**Recommended (Claude's discretion):** Read lock_screen.rs — `show_pin_entry()` takes `LockScreenState::PinEntry` with all those fields. The simplest path is to add a `show_idle_pin_entry()` helper that shows the PIN screen without a specific booking context (suitable for a freshly reset pod waiting for the next customer). Alternatively, check if lock_screen already has such a state.

**Timer duration change:** blank_timer currently arms at 15s after SessionEnded (line 1520). Change to 30s for both normal and orphan paths per locked decisions.

### Pattern 3: Crash Recovery State Machine (SESSION-03)

**What:** Replace the single `crash_recovery_armed` bool + `crash_recovery_timer` Sleep with a structured enum that tracks relaunch attempt count.

**Recommended state enum (Claude's discretion):**
```rust
#[derive(Debug)]
enum CrashRecoveryState {
    Idle,
    PausedWaitingRelaunch {
        attempt: u8,           // 1 or 2
        launched_at: tokio::time::Instant,
        last_sim_type: SimType,
        last_launch_args: Option<String>,
    },
    AutoEndPending,            // 2nd attempt failed → triggering auto-end
}
```

**Flow:**
1. Game crash detected (existing code at main.rs ~line 1132) → set `billing_paused=true` in FailureMonitorState, send `AgentMessage::BillingPaused` to WS, show overlay "Game crashed — relaunching...", set `CrashRecoveryState::PausedWaitingRelaunch { attempt: 1, ... }`
2. Poll every 5s (or use a 60s Sleep timer): if `game_pid` appears within 60s → send `AgentMessage::BillingResumed` to WS, dismiss overlay, set `billing_paused=false`, back to `Idle`
3. After 60s no PID → attempt 2: attempt same relaunch, `attempt: 2`, new 60s timer
4. After attempt 2 fails → `AutoEndPending` → same path as orphan auto-end (HTTP POST to server, show "Session ending" overlay, go to PinEntry)

**Existing code to remove:** The `crash_recovery_timer` Sleep + `crash_recovery_armed` bool. The 30s force-reset path is entirely replaced.

**billing_paused field choice (Claude's discretion — recommended):** Add `billing_paused: bool` to `FailureMonitorState`. Simpler than a separate channel; billing_guard can read it to suppress BILL-02/BILL-03 anomalies during crash recovery, since billing is legitimately paused.

### Pattern 4: WS Grace Window (SESSION-04)

**Current reconnect loop structure:** The outer `loop {}` at main.rs ~line 735 runs connect → event loop → on disconnect: break → retry. Currently `show_disconnected()` is called immediately on connect failure (lines 751, 759).

**Change required:** Track when WS first disconnected in the outer loop scope (not inside the inner event loop):

```rust
// OUTSIDE the reconnect loop, declared once:
let mut ws_disconnected_at: Option<std::time::Instant> = None;

// Inside the reconnect loop, on FAILED connect:
let disconnected_for = ws_disconnected_at
    .get_or_insert_with(std::time::Instant::now)
    .elapsed();

if disconnected_for > Duration::from_secs(30) {
    lock_screen.show_disconnected();
}
// else: silent reconnect — customer doesn't see "Disconnected"

// On SUCCESSFUL connect: reset the tracker
ws_disconnected_at = None;
```

**On reconnect within 30s:** After successful `Register`, send `AgentMessage::PodStateSnapshot(...)` — but wait, PodStateSnapshot is for the AI debugger, not for WS protocol. The locked decision says "send full PodStateSnapshot to server for state reconciliation." This means: send the existing `AgentMessage::Heartbeat(pod_info)` with current state — the Register message at lines 768-776 already does this. Verify this is sufficient or add a dedicated reconciliation message.

**Self-monitor interaction:** `self_monitor.rs` uses `ws_last_connected` (its own Instant) to decide when to relaunch at WS_DEAD_SECS=300s. The 30s grace window in the reconnect loop is a lower layer — it only affects when the lock screen shows "Disconnected". The 300s relaunch by self_monitor is unaffected, which is correct per locked decisions (0-30s silent, 30s-5min reconnect continues, 5min+ full relaunch).

### Anti-Patterns to Avoid

- **Don't use `async` inside billing_guard loop directly for HTTP:** The guard is a `tokio::spawn` async task so `.await` is fine — but keep the HTTP client static (OnceLock) to avoid creating a new client on every poll tick.
- **Don't skip the BILL-02 escalation path when orphan_fired=true:** They serve different purposes. BILL-02 fires at 60s to warn the server. Orphan auto-end fires at 300s to act. Both should fire; orphan_fired only prevents a second auto-end attempt.
- **Don't block the main.rs event loop for HTTP calls:** The orphan auto-end HTTP call from billing_guard runs in its own `tokio::spawn` task — it's not in main.rs. This is correct and avoids blocking the select! loop.
- **Don't hardcode session_id in auto-end:** Must resolve via `GET /api/v1/billing/active` before POSTing end. "unknown" is only acceptable for anomaly notifications, not for actual end calls.
- **Don't change self_monitor WS_DEAD_SECS:** The 300s relaunch threshold stays. Only the lock screen behavior changes during the 30s grace window.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client for orphan end | Custom TCP socket | `reqwest` with `OnceLock<Client>` | Self_monitor already has the exact pattern |
| Crash relaunch logic | New game launcher | Existing `ac_launcher`, game_process spawn path | Reuse exactly what LaunchGame handler does |
| Billing pause notification | Custom protocol | New `AgentMessage::BillingPaused` + existing WS pipeline | Pattern matches all other AgentMessage sends |
| Timer in state machine | std::thread::sleep | `tokio::time::Sleep` with `as_mut().reset()` | Existing blank_timer and crash_recovery_timer use this pattern |
| DB migration | Manual SQL in code | sqlx migration file in migrations/ | Existing racecontrol migration pattern |

**Key insight:** Every new capability in this phase has an existing code pattern to follow. The implementation is wiring, not invention.

---

## Common Pitfalls

### Pitfall 1: Resolving the Billing Session ID in billing_guard

**What goes wrong:** billing_guard sends "unknown" as billing_session_id for BILL-02 (this is acceptable for anomaly notifications). For auto-end it MUST have the real session ID or the server rejects the call.

**Why it happens:** billing_guard doesn't have the session_id — it only sees FailureMonitorState which doesn't track the session_id. The session_id lives in racecontrol's active_timers.

**How to avoid:** billing_guard must call `GET /api/v1/billing/active` (or a dedicated `GET /api/v1/pods/{pod_id}/session`) to discover the session_id before calling end. This adds one extra HTTP round trip. Alternative: add `active_billing_session_id: Option<String>` to FailureMonitorState so main.rs sets it on BillingStarted and billing_guard reads it directly — avoids the extra HTTP call and is the cleaner approach.

**Warning signs:** 404 on POST to `/api/v1/billing/unknown/end`

### Pitfall 2: show_pin_entry() Requires Booking Context

**What goes wrong:** After a session ends (especially orphan auto-end), there is no active booking token to populate `LockScreenState::PinEntry { token_id, driver_name, ... }`. Calling show_pin_entry with empty fields may render incorrectly or panic if the HTML template expects non-empty values.

**Why it happens:** PinEntry state was designed for when a booking already exists and the customer is authenticating.

**How to avoid:** Add a `show_idle_pin_entry()` method to `LockScreenManager` that renders a clean "Ready — please scan QR" state using an empty/placeholder PinEntry, OR check if there is a queued booking for this pod and show that, OR use a new `LockScreenState::Idle` variant (overkill for this phase). The simplest path: read lock_screen.rs fully to see what HTML the server renders for PinEntry — if it handles empty token_id gracefully, just pass empty strings.

**Warning signs:** Lock screen shows broken UI after session end, or JS errors in Edge's kiosk overlay.

### Pitfall 3: crash_recovery_armed Still True When CrashRecoveryState Changes

**What goes wrong:** If code is migrated incrementally and the old `crash_recovery_armed` bool isn't removed at the same time as the new CrashRecoveryState enum, both can be active simultaneously causing double-resets.

**Why it happens:** Main.rs event loop has many sites that set/clear crash_recovery_armed (lines 1143, 1302, 1482). All must be updated atomically.

**How to avoid:** Remove `crash_recovery_armed` bool and `crash_recovery_timer` Sleep in the same commit that adds CrashRecoveryState. Do a `grep -n crash_recovery` across main.rs to find all sites before starting.

**Warning signs:** cargo build errors (unused variable warning on crash_recovery_armed) or test failures in session flow.

### Pitfall 4: billing_guard HTTP Call Blocks on Server Unreachable

**What goes wrong:** reqwest default timeout is quite long. If the server is unreachable and billing_guard's HTTP call is `await`ed without a timeout, the guard poll loop stalls at 300s+ intervals instead of every 5s.

**Why it happens:** async tasks can still block on awaiting futures with long timeouts.

**How to avoid:** Use `.timeout(Duration::from_secs(10))` on the reqwest client builder (like self_monitor's ollama client does) OR wrap each call in `tokio::time::timeout(Duration::from_secs(10), ...)`. The 3-retry backoff (5s, 15s, 30s) means even a slow path resolves within ~60s total, which is acceptable within the 300s orphan window.

### Pitfall 5: WS Grace Window Interacts With UDP Heartbeat

**What goes wrong:** The UDP heartbeat's `CoreDead` event (3 missed pongs = 6s) triggers `break` in the WS event loop (main.rs line 1373), which exits to the reconnect loop. If the reconnect loop immediately shows Disconnected based on the old logic, the 30s grace window is bypassed.

**Why it happens:** The grace window logic must be in the reconnect loop (outer), not the WS event loop (inner). Currently `show_disconnected()` is called from inside the reconnect loop's connect-failure paths (lines 751, 759). On UDP-triggered disconnects, the break causes a reconnect attempt — the connect fails and hits one of those paths.

**How to avoid:** The `ws_disconnected_at` Instant must be set at the outer loop boundary (when the inner event loop exits via `break`), not inside the connect-failure paths. Then all three paths (connect error, connect timeout, inner loop break) correctly consult the same Instant.

---

## Code Examples

Verified patterns from existing source:

### OnceLock reqwest client (from self_monitor.rs)
```rust
// Source: crates/rc-agent/src/self_monitor.rs lines 168-177
static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("ollama HTTP client build failed")
    })
}
```
Apply same pattern for `ORPHAN_CLIENT` in billing_guard.rs with 10s timeout.

### Armed timer pattern (from main.rs lines 845-852)
```rust
// Source: crates/rc-agent/src/main.rs lines 845-852
let mut blank_timer: std::pin::Pin<Box<tokio::time::Sleep>> =
    Box::pin(tokio::time::sleep(Duration::from_secs(u64::MAX)));
let mut blank_timer_armed = false;

let mut crash_recovery_timer: std::pin::Pin<Box<tokio::time::Sleep>> =
    Box::pin(tokio::time::sleep(Duration::from_secs(u64::MAX)));
// crash_recovery_armed follows same pattern
```
New CrashRecoveryState uses the same `tokio::time::Sleep` pin — create one Sleep for the relaunch wait timer.

### watch channel send_modify (from main.rs line 1405)
```rust
// Source: crates/rc-agent/src/main.rs line 1405
let _ = failure_monitor_tx.send_modify(|s| {
    s.billing_active = true;
});
// Use same pattern to set s.billing_paused = true on crash
```

### AgentMessage enum variant (from protocol.rs)
```rust
// Source: crates/rc-common/src/protocol.rs lines 64-65
// Existing pattern to follow:
GameCrashed { pod_id: String, billing_active: bool },
// New variants follow same shape:
SessionAutoEnded { pod_id: String, billing_session_id: String, reason: String },
BillingPaused { pod_id: String, billing_session_id: String },
BillingResumed { pod_id: String, billing_session_id: String },
```

### billing_guard task-local escalation pattern (from billing_guard.rs lines 29-32)
```rust
// Source: crates/rc-agent/src/billing_guard.rs lines 29-32
let mut stuck_fired = false;
let mut game_gone_since: Option<std::time::Instant> = None;
// Add alongside:
let mut orphan_fired = false;
// orphan_fired prevents duplicate auto-end calls for same orphan window
```

### E2E test script structure (from billing.sh)
```bash
# Source: tests/e2e/api/billing.sh — pattern for session-lifecycle.sh
#!/bin/bash
set -uo pipefail
BASE_URL="${RC_BASE_URL:-http://192.168.31.23:8080/api/v1}"
POD_ID="${TEST_POD_ID:-pod-8}"
SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd)
source "$SCRIPT_DIR/../lib/common.sh"
source "$SCRIPT_DIR/../lib/pod-map.sh"
# Gates 0-N with pass/fail/skip/info + summary_exit
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| 30s crash_recovery_timer → force-reset | pause+relaunch state machine (this phase) | Phase 49 | Customer gets auto-recovery not force-wipe |
| show_blank_screen() after session | show_pin_entry() after session (this phase) | Phase 49 | Pod immediately ready for next customer |
| show_disconnected() immediately on WS drop | 30s grace window before showing disconnected (this phase) | Phase 49 | Venue flaky WiFi drops don't disturb sessions |
| No orphan detection action | auto-end via HTTP + SessionAutoEnded WS notify (this phase) | Phase 49 | Orphaned billing resolves without staff |

**Deprecated by this phase:**
- `crash_recovery_armed: bool` — replaced by `CrashRecoveryState` enum
- `crash_recovery_timer: Pin<Box<Sleep>>` — replaced by relaunch timer inside state machine
- `blank_timer` target `show_blank_screen()` — changed to `show_pin_entry()` / `show_idle_pin_entry()`

---

## Open Questions

1. **How does lock_screen handle PinEntry without a booking token?**
   - What we know: `show_pin_entry()` requires `LockScreenState::PinEntry { token_id, driver_name, ... }`. After orphan auto-end there's no active booking.
   - What's unclear: Whether the HTML template renders a "waiting for customer" state gracefully with empty fields.
   - Recommendation: Read lock_screen.rs fully in Plan 01. If empty fields cause issues, add `show_idle_pin_entry()` helper that constructs a safe PinEntry with placeholder values.

2. **Does billing.rs end_billing_session_public() need to accept an end_reason parameter?**
   - What we know: Currently takes `session_id` and `BillingSessionStatus`. The `end_reason` column must be populated for SESSION-01's audit trail.
   - What's unclear: Whether to add `end_reason: &str` to the public function signature or update the DB separately after calling end.
   - Recommendation: Add `end_reason: Option<&str>` to `end_billing_session_public()` signature so the UPDATE query includes it atomically. Existing callers pass `None` (backward compat).

3. **How does billing_guard get the billing_session_id for auto-end?**
   - What we know: It currently uses "unknown". For auto-end it needs the real ID.
   - What's unclear: Whether to add `active_billing_session_id: Option<String>` to FailureMonitorState (cleaner) or HTTP-fetch it (extra round trip).
   - Recommendation: Add to FailureMonitorState — main.rs sets it on BillingStarted, clears on SessionEnded. No extra HTTP call needed.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | bash + lib/common.sh (existing E2E shell infrastructure) |
| Config file | none — scripts are self-contained |
| Quick run command | `bash tests/e2e/api/session-lifecycle.sh` |
| Full suite command | `bash tests/e2e/run-all.sh` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SESSION-01 | billing create → orphan timeout → auto-end → pod idle via API | integration | `bash tests/e2e/api/session-lifecycle.sh` | ❌ Wave 0 |
| SESSION-02 | session end → 30s → pod in PinEntry (not ScreenBlanked) | integration | included in session-lifecycle.sh Gate 4 | ❌ Wave 0 |
| SESSION-03 | crash → billing paused → relaunch → resume or auto-end | manual-only (requires real game crash) | Manual verification; unit tests for state machine logic | N/A |
| SESSION-04 | WS drop < 30s → no Disconnected screen, no self-relaunch | manual-only (requires network disruption) | Manual at Pod 8; unit test for ws_disconnected_at logic | N/A |

**SESSION-03/04 manual justification:** These require controlled game crashes and network disruptions on live hardware. The E2E test suite validates the observable server-side outcomes (billing ended, pod idle) rather than the internal rc-agent state transitions. Unit tests cover the state machine logic.

### Unit Test Coverage (cargo nextest)

The planner MUST add unit tests for new logic in:
- `billing_guard.rs` — orphan threshold at 300s, two-tier (60s warn + 300s act), orphan_fired deduplication
- `failure_monitor.rs` — billing_paused field default, watch channel propagation
- `main.rs` / inline tests — CrashRecoveryState transitions (can be `#[cfg(test)]` inline tests mimicking existing pattern)

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent` (unit tests only, fast)
- **Per wave merge:** `bash tests/e2e/api/session-lifecycle.sh` + `bash tests/e2e/api/billing.sh`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `tests/e2e/api/session-lifecycle.sh` — covers SESSION-01 + SESSION-02 (Gates 0-5)
- [ ] No framework install needed — bash + python3 already available (matching billing.sh pattern)

---

## Sources

### Primary (HIGH confidence)
- Direct code read: `crates/rc-agent/src/billing_guard.rs` — BILL-02 pattern, game_gone_since timer, two-tier structure
- Direct code read: `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState struct, existing fields, watch channel pattern
- Direct code read: `crates/rc-agent/src/self_monitor.rs` — OnceLock reqwest client pattern, WS_DEAD_SECS=300, relaunch_self()
- Direct code read: `crates/rc-agent/src/main.rs` — blank_timer (line 845), crash_recovery_timer (line 850), SessionEnded handler (line 1474), crash detection (line 1132), reconnect loop (line 735), show_disconnected() calls (lines 751, 759)
- Direct code read: `crates/rc-common/src/protocol.rs` — AgentMessage enum, adjacently-tagged serde, existing variant shapes
- Direct code read: `crates/racecontrol/src/billing.rs` — end_billing_session_public() signature, billing_sessions UPDATE query
- Direct code read: `tests/e2e/api/billing.sh` — E2E test pattern (gates, curl, python3 parsing, summary_exit)
- Direct code read: `tests/e2e/lib/common.sh` — pass/fail/skip/info helpers, summary_exit
- Direct code read: `.planning/phases/49-session-lifecycle-autonomy/49-CONTEXT.md` — all locked decisions

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` — accumulated decisions from previous phases confirming timer patterns and test conventions

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies already in use, no new packages
- Architecture: HIGH — all patterns verified in existing code; state machine structure is Claude's discretion but follows established tokio patterns
- Pitfalls: HIGH — identified from direct code inspection of the exact files being modified
- E2E test structure: HIGH — verified against billing.sh which is the direct template

**Research date:** 2026-03-19 IST
**Valid until:** 2026-04-19 (stable Rust/Tokio ecosystem; rc-agent patterns won't change within 30 days)
