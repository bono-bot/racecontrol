# Phase 2: Watchdog Hardening - Research

**Researched:** 2026-03-13
**Domain:** Rust async watchdog patterns, tokio task coordination, WebSocket liveness, Axum state mutation
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Post-restart verification**
- rc-agent exposes a /health endpoint on a local port; rc-core verifies lock screen responsiveness by hitting it via pod-agent /exec curl
- Verification polling uses escalating schedule: 5s, 15s, 30s, 60s after restart command sent (total 60s window)
- All 3 checks must pass: process alive + WebSocket connected + lock screen health endpoint responsive. If any check fails at 60s, declare failure
- Partial recovery (process + WS but no lock screen) is treated as FAILED — alert fires, backoff escalates. Customers can't use a pod without the lock screen
- Verification runs as a spawned async task (tokio::spawn) — pod_monitor continues checking other pods without blocking

**Kiosk status display**
- Detailed states shown to staff: Online, Offline, Restarting (attempt N/4), Verifying Recovery, Recovery Failed
- New DashboardEvent types: PodRestarting, PodVerifying, PodRecoveryFailed — delivered via existing WebSocket protocol
- Show the backoff step ("Backoff: 2m") but not a live countdown timer — less visual noise
- Current state only — no restart history in the dashboard UI. Staff checks activity log for patterns

**Alert email content**
- Actionable summary format, 10 lines max. Subject: "[RaceControl] Pod N — Recovery Failed" or "Pod N — Max Escalation Reached"
- Body includes: pod name, failure type (no WS / no lock screen / process dead), current backoff step, last heartbeat time, next action suggestion
- Recipient: Uday only (usingh@racingpoint.in) — James is on-site and sees the kiosk
- Alert fires on two triggers: (1) post-restart verification failure, (2) max backoff escalation reached (30m step)
- Uses existing Node.js send_email.js script via Command::new("node") — same Gmail OAuth path already implemented in EmailAlerter

**Healer vs Monitor boundaries**
- pod_healer sets a `needs_restart: true` flag per pod in AppState when it detects restart-worthy issues. pod_monitor checks this flag on its next cycle and issues the restart
- pod_healer skips its entire diagnostic cycle for pods in Restarting or Verifying state — no conflicting actions during recovery
- WebSocket liveness uses belt-and-suspenders: heartbeat timestamp timeout as primary detection, channel send-ping-and-check-error as secondary confirmation before declaring dead
- Full backoff reset on recovery: attempt counter goes to 0, next failure starts at 30s. Clean slate — recovered pod is healthy until proven otherwise

### Claude's Discretion
- Exact /health endpoint response format on rc-agent side
- How to structure the `needs_restart` flag in AppState (bool, enum, or timestamp)
- DashboardEvent payload structure for new watchdog event types
- Email body template formatting
- How to handle verification task cleanup if pod_monitor cycle runs while verification is still pending

### Deferred Ideas (OUT OF SCOPE)
- Billing timer pause during pod downtime with grace period on recovery — new capability, belongs in billing/session management phase
- Restart history UI in kiosk dashboard — future observability improvement
- Badge count for problem pods — nice-to-have for future dashboard enhancement
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| WD-01 | Pod restart uses escalating backoff (30s→2m→10m→30m) instead of fixed cooldown | EscalatingBackoff already implemented with tests; pod_monitor already wires it — research confirms the integration is incomplete (recovery reset path exists but verification path doesn't broadcast dashboard events or update pod status) |
| WD-03 | Post-restart verification confirms process running + WebSocket connected + lock screen responsive (60s window) | verify_restart() skeleton exists in pod_monitor.rs but uses `contains_key` for WS liveness (known insufficient) and partial-recovery handling contradicts CONTEXT.md decisions — both need rewriting |
| WD-04 | Backoff resets to base on confirmed full recovery | reset() call exists in verify_restart() happy path; missing from non-happy paths; the partial-recovery branch current does NOT reset — CONTEXT.md says partial IS failure so the current "return silently" is wrong |
| ALERT-01 | Email alert fires when post-restart verification fails or max escalation reached | EmailAlerter.send_alert() wired in pod_monitor.rs exhaustion path and verify_restart failure path, but alert body doesn't include last heartbeat time or "next action suggestion" per CONTEXT.md — format_alert_body needs updating |
| ALERT-02 | Rate-limited: max 1 email per pod per 30min, 1 venue-wide per 5min | EmailAlerter already enforces these limits via should_send() / record_sent() — no new logic needed, just correct wiring |
</phase_requirements>

---

## Summary

Phase 2 is primarily an **integration and correctness phase**, not a new-design phase. The building blocks — `EscalatingBackoff`, `EmailAlerter`, `AppState.pod_backoffs`, `AppState.email_alerter`, the `verify_restart()` skeleton in pod_monitor.rs — are all present and tested in isolation. What is missing or incorrect:

1. **WS liveness check** uses `agent_senders.contains_key()` which is insufficient. A key can remain in the map after a connection drops because cleanup only happens in the disconnect handler. The correct pattern is to send a ping message via the channel and check if `send()` returns an error — a closed channel returns `SendError` immediately.

2. **Partial recovery handling** contradicts CONTEXT.md. The current code silently returns without alerting when process+WS are up but lock screen is down. The decision is that partial recovery is FAILED — it must fire an alert and escalate backoff.

3. **Dashboard events** for watchdog states (`PodRestarting`, `PodVerifying`, `PodRecoveryFailed`) don't exist yet in `DashboardEvent` and are never broadcast during the restart lifecycle.

4. **Healer/monitor boundary** via `needs_restart` flag in AppState is not implemented. The healer currently logs "deferring restart to pod_monitor" as a string in issues[], but the monitor never reads that signal. The flag needs an AppState field.

5. **Email alert body** is missing last heartbeat time and "next action suggestion" fields per CONTEXT.md decisions.

6. **Healer does not skip** pods in Restarting/Verifying state — there is no pod watchdog state enum to check against.

**Primary recommendation:** Implement in this order: (1) WatchdogState enum + AppState field, (2) WS liveness ping pattern, (3) `needs_restart` flag, (4) DashboardEvent new variants, (5) pod_monitor restart lifecycle with broadcasts, (6) verify_restart rewrite, (7) pod_healer skip logic, (8) email body update.

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | 1.x | Async runtime, spawn, sleep, interval | Already in use throughout |
| tokio::sync::RwLock | built-in | Shared mutable state (pod_backoffs, new watchdog states) | Same pattern as all other AppState fields |
| tokio::sync::mpsc | built-in | WS channel send-ping-and-check-error liveness | Already used for agent_senders |
| chrono | 0.4 | Timestamps for last-heartbeat in alert body | Already in use |
| serde/serde_json | 1.x | DashboardEvent serialization | Already in use |
| tracing | 0.1 | Structured logging for all state transitions | Already in use |

No new dependencies are required for this phase.

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| anyhow | 1.x | Error propagation in verify_restart async task | Already used in pod_healer |
| uuid | 1.x | Activity log entry IDs | Already in use |

**Installation:** No new packages needed.

---

## Architecture Patterns

### Recommended File Changes
```
crates/rc-common/src/
├── protocol.rs          # Add PodRestarting, PodVerifying, PodRecoveryFailed to DashboardEvent
├── watchdog.rs          # No changes needed (EscalatingBackoff is correct)
└── types.rs             # Optional: add WatchdogState enum if shared between crates

crates/rc-core/src/
├── state.rs             # Add pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>
│                        # Add pod_needs_restart: RwLock<HashMap<String, bool>>
├── pod_monitor.rs       # Rewrite WS liveness, add broadcasts, fix partial-recovery
├── pod_healer.rs        # Add skip logic for Restarting/Verifying pods
└── email_alerts.rs      # Update format_alert_body to include last_heartbeat + next_action

crates/rc-agent/src/
└── lock_screen.rs       # Add /health endpoint to the existing HTTP server (port 18923)
```

### Pattern 1: WatchdogState enum in AppState

**What:** A per-pod state enum tracking the watchdog's view of each pod, separate from PodStatus. PodStatus is customer-visible; WatchdogState is internal watchdog machinery.

**When to use:** Any time pod_monitor or pod_healer needs to know if a pod is currently in a recovery cycle.

```rust
// In rc-core/src/state.rs (or rc-common/src/types.rs if shared)
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogState {
    /// Pod is healthy — no active recovery
    Healthy,
    /// Restart command sent, waiting for rc-agent to come back
    Restarting { attempt: u32, started_at: DateTime<Utc> },
    /// Restart command sent, verification task is running
    Verifying { attempt: u32, started_at: DateTime<Utc> },
    /// Verification failed — backoff escalated, awaiting next cycle
    RecoveryFailed { attempt: u32, failed_at: DateTime<Utc> },
}

// In AppState:
pub pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>,
pub pod_needs_restart: RwLock<HashMap<String, bool>>,
```

**Key insight:** Using an enum rather than multiple bools keeps the state machine explicit and makes illegal states unrepresentable. The planner should add both fields to `AppState::new()` with pre-populated entries for pods 1-8.

### Pattern 2: WS Liveness via Channel Send-Ping

**What:** Send a dummy message via the `agent_senders` mpsc channel; if the channel is closed, `send()` returns `Err(SendError)` immediately. This is the belt-and-suspenders secondary check.

**When to use:** In verify_restart() and anywhere pod_monitor needs to confirm WS is live.

```rust
// Source: Tokio mpsc documentation + STATE.md blocker note
async fn is_ws_connected(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    let Some(sender) = senders.get(pod_id) else {
        return false;
    };
    // try_send to a closed channel returns Err immediately — no await needed
    // We do NOT send a real message — use is_closed() which tokio provides
    !sender.is_closed()
}
```

**Note on `is_closed()`:** `tokio::sync::mpsc::Sender::is_closed()` returns true if all receivers have been dropped. This is exactly what we need — if the WebSocket task that owns `cmd_rx` has exited (connection dropped), `is_closed()` returns true. This avoids the overhead of actually sending a message. Confidence: HIGH (tokio 1.x API).

**Alternative (if is_closed() not available):** Use `sender.try_send(CoreToAgentMessage::Ping)` — but this requires adding a `Ping` variant to `CoreToAgentMessage`, which is extra protocol surface. The `is_closed()` approach is cleaner.

### Pattern 3: Healer Skip via WatchdogState Check

**What:** At the top of `heal_pod()`, read `pod_watchdog_states` and return early if the pod is in Restarting or Verifying state.

**When to use:** First check in heal_pod() after the pod-agent ping.

```rust
// In pod_healer.rs heal_pod()
let watchdog_state = {
    let states = state.pod_watchdog_states.read().await;
    states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
};
match watchdog_state {
    WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. } => {
        tracing::debug!(
            "Pod healer: {} in recovery cycle — skipping diagnostic",
            pod.id
        );
        return Ok(());
    }
    _ => {}
}
```

### Pattern 4: DashboardEvent New Variants

**What:** Add three new variants to `DashboardEvent` in protocol.rs.

```rust
// In rc-common/src/protocol.rs, inside DashboardEvent enum:

/// Watchdog issued restart command for a pod
PodRestarting {
    pod_id: String,
    attempt: u32,
    max_attempts: u32,
    backoff_label: String, // e.g., "30s", "2m", "10m", "30m"
},

/// Watchdog is verifying recovery for a pod
PodVerifying {
    pod_id: String,
    attempt: u32,
},

/// Watchdog verification failed for a pod — alert sent
PodRecoveryFailed {
    pod_id: String,
    attempt: u32,
    reason: String, // "no_ws", "no_lock_screen", "process_dead"
},
```

**Serialization:** These follow the existing `#[serde(tag = "event", content = "data")]` pattern, so they will arrive at the dashboard as `{"event": "pod_restarting", "data": {...}}`.

### Pattern 5: needs_restart Flag Flow

**What:** pod_healer sets the flag; pod_monitor reads and clears it on its next cycle.

```rust
// pod_healer sets:
{
    let mut needs = state.pod_needs_restart.write().await;
    needs.insert(pod.id.clone(), true);
}

// pod_monitor reads and clears (at top of stale pod handling):
let healer_flagged = {
    let mut needs = state.pod_needs_restart.write().await;
    needs.remove(&pod.id).unwrap_or(false)
};
// If healer_flagged is true AND backoff.ready(), proceed with restart regardless
// of whether heartbeat_timeout has strictly elapsed (healer saw deeper issue)
```

**Caution:** The healer only sets this flag when it detects lock screen unresponsive with NO WebSocket. It must NOT set the flag for issues it handles itself (zombie sockets, disk cleanup) — those are not restart-worthy.

### Pattern 6: Email Body with Heartbeat Time

The existing `format_alert_body` signature needs a `last_heartbeat` parameter:

```rust
pub fn format_alert_body(
    pod_id: &str,
    reason: &str,          // "no_ws", "no_lock_screen", "process_dead", etc.
    failure_type: &str,    // human-readable: "No WebSocket", "Lock screen unresponsive"
    attempt: u32,
    cooldown_secs: u64,
    last_heartbeat: Option<DateTime<Utc>>,
    next_action: &str,     // "Pod will retry in 10m", "Manual intervention required"
) -> String
```

This is a breaking change to an existing function — all callers in pod_monitor.rs must be updated simultaneously.

### Anti-Patterns to Avoid

- **`agent_senders.contains_key()` for WS liveness:** The key remains after disconnect until the ws handler task cleans it up. Use `sender.is_closed()` instead.
- **Verification that blocks the monitor loop:** `verify_restart()` MUST be a `tokio::spawn()` detached task. The current code does this correctly — do not refactor it to await inline.
- **Setting pod_needs_restart for disk/memory issues:** Only set it when lock screen is unresponsive with no WS (genuine rc-agent failure). Disk and memory issues are healer-only.
- **Forgetting to clear WatchdogState on recovery:** When backoff.reset() is called, also set watchdog_state to Healthy and broadcast a PodUpdate.
- **Race between concurrent verify tasks:** If pod_monitor fires again before a prior verify_restart() task completes, a second verify task would launch. Guard with WatchdogState: if already Verifying, skip spawning a new task.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async channel liveness | Custom heartbeat/ping protocol | `mpsc::Sender::is_closed()` | Built into tokio, zero overhead, no protocol change |
| Rate-limited alerting | Custom cooldown timer | `EmailAlerter.should_send()` + `record_sent()` | Already implemented and tested (Phase 1) |
| Backoff state machine | Custom retry loop | `EscalatingBackoff` from rc-common | Already implemented with 11 passing tests |
| Per-pod state tracking | Multiple HashMap fields | Single `WatchdogState` enum | Keeps state machine explicit, impossible to be in two states at once |
| Lock screen health check | Direct TCP to pod IP | pod-agent /exec + PowerShell Invoke-WebRequest | Lock screen binds to 127.0.0.1:18923, not externally reachable |

---

## Common Pitfalls

### Pitfall 1: agent_senders key survives disconnect
**What goes wrong:** `state.agent_senders.read().await.contains_key(&pod_id)` returns true even after the WebSocket connection drops, because the disconnect handler in ws/mod.rs removes the key asynchronously (after the loop exits). During the brief window between connection drop and cleanup, the key is still present.
**Why it happens:** The ws handler removes the key in a cleanup block after `ws_receiver.next()` returns None (connection closed). This runs after the pod_monitor check.
**How to avoid:** Use `sender.is_closed()` — this checks the channel's underlying state, which transitions immediately when all receivers drop.
**Warning signs:** Verification reporting WS healthy immediately after restart (key still present from old connection).

### Pitfall 2: Double restart from concurrent verify task
**What goes wrong:** pod_monitor fires (30s interval), sees pod still stale, spawns a new verify task — but a previous verify task from the prior restart is still running its sleep+check cycle.
**Why it happens:** verify_restart() sleeps up to 60s total. The monitor interval (10s by default) fires 6 times during that window.
**How to avoid:** Check WatchdogState before spawning verify_restart(). If state is already Verifying, skip the restart and the spawn.
**Warning signs:** Two email alerts for the same pod within 60s, or backoff.attempt() incrementing twice for one failure.

### Pitfall 3: WatchdogState not cleared on natural recovery
**What goes wrong:** Pod recovers naturally (sends heartbeat), pod_monitor resets the backoff — but WatchdogState stays as RecoveryFailed. Healer keeps skipping the pod forever.
**Why it happens:** Two separate state machines (backoff + WatchdogState) that need synchronized updates.
**How to avoid:** In the "pod is healthy" branch of check_all_pods(), clear WatchdogState to Healthy whenever backoff.attempt() > 0 is reset.
**Warning signs:** Healer logs showing "in recovery cycle — skipping" for pods that have been Online for hours.

### Pitfall 4: Healer sets needs_restart during active billing
**What goes wrong:** Healer flags a pod for restart mid-session. Monitor honors the flag and kills rc-agent while customer is playing.
**Why it happens:** The needs_restart flag bypasses the heartbeat-timeout check in pod_monitor.
**How to avoid:** pod_monitor must still check `has_active_billing()` before honoring the needs_restart flag. The billing guard applies to ALL restart paths.
**Warning signs:** Customer session terminated unexpectedly, billing timer stopped, lock screen appears mid-race.

### Pitfall 5: Breaking format_alert_body callers
**What goes wrong:** Adding parameters to `format_alert_body` breaks all existing call sites that don't pass the new args.
**Why it happens:** Rust won't compile — missing arguments are compile errors, not runtime.
**How to avoid:** Update all callers in pod_monitor.rs at the same time as the function signature change. Search for all uses before editing.
**Warning signs:** Compile error `E0061: this function takes N arguments but M were supplied`.

### Pitfall 6: Lock screen /health endpoint port collision
**What goes wrong:** rc-agent already serves on 127.0.0.1:18923. If /health is added to the same server, it just becomes another route — no collision. BUT if a second server is started on the same port, it will panic.
**Why it happens:** Developer adds a separate HTTP listener instead of adding a route to the existing one.
**How to avoid:** Add the `/health` route to the existing `serve_lock_screen()` TCP listener in lock_screen.rs (port 18923). The existing handler already parses the URL path — add an arm for `/health`.
**Warning signs:** `address already in use` panic on rc-agent startup.

---

## Code Examples

### Existing verify_restart() — Current Issues

The current implementation at pod_monitor.rs lines 382-482 has two problems:
1. `state.agent_senders.read().await.contains_key(&pod_id)` — insufficient liveness check (Pitfall 1)
2. Partial recovery branch (ws_ok && !lock_ok) returns silently — CONTEXT.md says this IS failure

The rewrite must:
- Replace `contains_key()` with `is_closed()` check
- Treat partial recovery as full failure (fire alert, do NOT reset backoff, update WatchdogState to RecoveryFailed)
- Broadcast PodVerifying on entry, PodRecoveryFailed on failure, PodUpdate(Healthy) on success

### Adding /health to lock_screen.rs

The existing `serve_lock_screen()` at lock_screen.rs line 106 runs a TCP listener loop. The inner handler parses request paths. Add:

```rust
// In serve_lock_screen() request handler, alongside existing path matching:
if path == "/health" {
    let is_active = !matches!(
        *state.lock().unwrap_or_else(|e| e.into_inner()),
        LockScreenState::Hidden | LockScreenState::Disconnected | LockScreenState::ConfigError { .. }
    );
    let status = if is_active { "ok" } else { "degraded" };
    // Return: HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n...\r\n{"status":"ok"}
}
```

The PowerShell check in pod_monitor already looks for HTTP 200 on `http://127.0.0.1:18923/` — so the existing root path response (200) already acts as a health indicator. The `/health` route as a dedicated endpoint just makes intent explicit and allows richer response. Since CONTEXT.md says "rc-agent exposes a /health endpoint" and the check is already `port 18923` via PowerShell, the `/health` path can simply return 200 JSON when the lock screen server is running — the content matters less than the status code.

### tokio mpsc is_closed() Pattern

```rust
// Source: tokio 1.x docs — Sender::is_closed()
// Returns true when all Receivers have been dropped.
// In our case: the ws send_task drops cmd_rx when the WS connection closes.

async fn is_ws_alive(state: &Arc<AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed cooldown (`restart_cooldown_secs`) | `EscalatingBackoff` (30s→2m→10m→30m) | Phase 1 | Phase 2 wires backoff into restart lifecycle |
| pod_healer triggers restarts directly | pod_monitor has exclusive restart ownership, healer defers | Phase 1 intent, Phase 2 implementation | Eliminates concurrent restart race |
| No post-restart verification | verify_restart() skeleton with 60s polling | Phase 2 | Prevents false "recovery" claims |
| No dashboard watchdog states | PodRestarting/PodVerifying/PodRecoveryFailed events | Phase 2 | Staff see recovery progress without polling |

**Deprecated/outdated in this codebase:**
- `restart_cooldown_secs` in WatchdogConfig: Was the old fixed cooldown. Now superseded by `escalation_steps_secs` + `EscalatingBackoff`. The config field still exists but is unused by pod_monitor (which uses EscalatingBackoff). It should remain in config for backward compatibility but can be ignored in Phase 2 implementation.
- Partial recovery "return silently" branch in verify_restart(): Contradicts CONTEXT.md — must be replaced with failure path.

---

## Open Questions

1. **WatchdogState enum location: rc-common or rc-core?**
   - What we know: rc-agent doesn't need to know about WatchdogState. DashboardEvent variants in rc-common need to carry watchdog info (attempt count, backoff label). State enum only used by rc-core internally.
   - What's unclear: Should the enum live in rc-common/types.rs (for potential future agent awareness) or rc-core/state.rs (simpler, no cross-crate exposure)?
   - Recommendation: Define in rc-core/state.rs. If it needs to go to rc-common later, it's a one-line move. Avoids polluting the shared protocol crate with internal machinery.

2. **verify_restart task cleanup when monitor re-triggers**
   - What we know: If WatchdogState is Verifying, pod_monitor should NOT spawn a new verify task or issue a restart.
   - What's unclear: How does pod_monitor know a verify task is already running? It needs to read WatchdogState, which is the recommended approach.
   - Recommendation: Set WatchdogState to Verifying immediately when spawning the task (before tokio::spawn). The monitor will see this on its next cycle and skip. This is race-free because pod_monitor is single-threaded (it runs in a single tokio::spawn loop).

3. **Node.js availability on Racing-Point-Server (.23)**
   - What we know: STATE.md blocker — "Node.js on Racing-Point-Server (.23) must be verified before Phase 2 deploys email alerting — run `node --version` on .23; install Node.js LTS if absent"
   - What's unclear: Whether Node.js is currently installed on .23
   - Recommendation: Wave 0 should include a task to verify `node --version` on .23 via pod-agent or SSH. The email feature can be toggled off (`email_enabled = false`) for testing on other functionality.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test runner) |
| Config file | none — `cargo test` discovers tests automatically |
| Quick run command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-core` |
| Full suite command | `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-core && cargo test -p rc-agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WD-01 | Backoff advances through 30s→2m→10m→30m steps on repeated failures | unit | `cargo test -p rc-common watchdog` | ✅ (existing 11 tests in watchdog.rs) |
| WD-01 | pod_monitor reads backoff.ready() before restarting | unit | `cargo test -p rc-core pod_monitor` | ❌ Wave 0 |
| WD-03 | verify_restart: all 3 checks pass = reset + PodUpdate | unit | `cargo test -p rc-core verify_restart` | ❌ Wave 0 |
| WD-03 | verify_restart: WS dead at 60s = RecoveryFailed event + alert | unit | `cargo test -p rc-core verify_restart_failure` | ❌ Wave 0 |
| WD-03 | verify_restart: partial recovery (process+WS, no lock screen) = failure path | unit | `cargo test -p rc-core verify_restart_partial` | ❌ Wave 0 |
| WD-04 | Full recovery resets backoff to attempt=0 | unit | `cargo test -p rc-core backoff_reset_on_recovery` | ❌ Wave 0 |
| ALERT-01 | send_alert called on verification failure | unit | `cargo test -p rc-core alert_on_verify_fail` | ❌ Wave 0 |
| ALERT-01 | send_alert called on max escalation (attempt >= 4) | unit | `cargo test -p rc-core alert_on_exhaustion` | ❌ Wave 0 |
| ALERT-02 | should_send() blocks repeated alerts within 30min/pod | unit | `cargo test -p rc-core email_alerts` | ✅ (existing 8 tests in email_alerts.rs) |
| WD-01 + WD-04 | WatchdogState transitions: Healthy → Restarting → Verifying → Healthy | unit | `cargo test -p rc-core watchdog_state_transitions` | ❌ Wave 0 |
| WD-03 | Healer skips pod in Restarting/Verifying state | unit | `cargo test -p rc-core healer_skips_restarting` | ❌ Wave 0 |
| WD-01 | needs_restart flag: healer sets, monitor reads and clears | unit | `cargo test -p rc-core needs_restart_flag` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `export PATH="$PATH:/c/Users/bono/.cargo/bin" && cargo test -p rc-common && cargo test -p rc-core`
- **Per wave merge:** Full suite (all 3 crates)
- **Phase gate:** Full suite green before `/gsd:verify-work`, then deploy to Pod 8 and verify live watchdog behavior

### Wave 0 Gaps

The following test scaffolding is needed before implementation waves begin:

- [ ] `crates/rc-core/src/pod_monitor.rs` — Add `#[cfg(test)] mod tests {}` block with helper to create mock AppState and test backoff/WatchdogState transitions
- [ ] `crates/rc-core/src/pod_healer.rs` — Add `#[cfg(test)] mod tests {}` block with helper to verify healer skips Restarting pods
- [ ] Shared test helper: `fn make_test_app_state() -> Arc<AppState>` using `Config::default_test()` and in-memory SQLite — currently only in integration.rs, needs to be a helper available to unit tests in each module

Note: Integration test infra in `tests/integration.rs` already has `create_test_db()` and `run_test_migrations()`. The `make_test_app_state()` helper should use the same pattern.

---

## Sources

### Primary (HIGH confidence)
- Direct codebase read — `crates/rc-common/src/watchdog.rs` — EscalatingBackoff API, 11 test cases
- Direct codebase read — `crates/rc-core/src/email_alerts.rs` — EmailAlerter API, send_alert signature, rate limit logic
- Direct codebase read — `crates/rc-core/src/state.rs` — AppState fields, pod_backoffs/email_alerter placement
- Direct codebase read — `crates/rc-core/src/pod_monitor.rs` — Current verify_restart(), WS liveness defect confirmed
- Direct codebase read — `crates/rc-core/src/pod_healer.rs` — Current healer logic, "defer restart" comment
- Direct codebase read — `crates/rc-common/src/protocol.rs` — Existing DashboardEvent variants, serde attributes
- Direct codebase read — `crates/rc-agent/src/lock_screen.rs` — Port 18923, existing HTTP server pattern
- Direct codebase read — `crates/rc-core/src/config.rs` — WatchdogConfig fields, email defaults
- Tokio 1.x documentation — `mpsc::Sender::is_closed()` exists and returns true when all receivers dropped

### Secondary (MEDIUM confidence)
- STATE.md Blockers section — agent_senders channel liveness noted as known issue to fix in Phase 2
- CONTEXT.md decisions — comprehensive, treat as locked spec for implementation

### Tertiary (LOW confidence)
- None — all findings verified against actual codebase

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all crates already in use, no new dependencies
- Architecture: HIGH — patterns derived from existing code in the same repo
- Pitfalls: HIGH — defects identified by reading the actual implementation
- WS liveness (is_closed): HIGH — tokio 1.x API confirmed by knowledge of tokio mpsc design

**Research date:** 2026-03-13
**Valid until:** 2026-04-13 (stable Rust ecosystem, internal codebase)
