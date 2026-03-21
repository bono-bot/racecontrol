# Architecture Research

**Domain:** Pre-flight session checks integration into rc-agent
**Researched:** 2026-03-21
**Confidence:** HIGH — based on direct source code inspection of all named files

---

## System Overview

### Current rc-agent Architecture (Baseline)

```
main.rs (startup + reconnect loop)
    │
    ├── self_heal.rs          [startup only — config/script/registry repair]
    ├── self_test.rs          [startup only — 22 probes, SelfTestReport]
    ├── LockScreenManager     [HTTP server on :18923, Edge kiosk window]
    │       └── LockScreenState enum (13 states)
    │
    └── event_loop::run()     [per-connection lifetime]
            │
            ├── ConnectionState   [reset on each WS reconnect]
            └── tokio::select! loop
                    │
                    ├── ws_rx.next()  ──→  ws_handler::handle_ws_message()
                    │                           │
                    │                           ├── BillingStarted ──→ [GATE GOES HERE]
                    │                           ├── BillingTick
                    │                           ├── BillingStopped
                    │                           ├── SessionEnded
                    │                           ├── LaunchGame
                    │                           ├── SwitchController
                    │                           └── ExecRequest
                    │
                    ├── heartbeat_interval.tick()
                    ├── telemetry_interval.tick()
                    ├── game_check_interval.tick()
                    ├── kiosk_interval.tick()
                    ├── overlay_topmost_interval.tick()
                    ├── blank_timer (sleep)
                    ├── lock_event_rx.recv()
                    └── signal_rx.recv()
```

### Proposed Architecture with Pre-Flight Gate

```
BillingStarted received (ws_handler.rs)
    │
    ▼
pre_flight::run(&state, &config)          ← NEW MODULE
    │
    ├── Check: WS connected               (HeartbeatStatus.ws_connected)
    ├── Check: UDP heartbeat alive        (HeartbeatStatus)
    ├── Check: HID wheelbase present      (reuse self_test probe_hid logic)
    ├── Check: ConspitLink running        (process list scan)
    ├── Check: No orphaned game PIDs      (game_process::find_game_pid)
    ├── Check: AC content accessible      (content_scanner or path stat)
    ├── Check: No stuck billing session   (AppState billing state)
    ├── Check: Disk > 1GB free            (reuse self_test probe_disk logic)
    ├── Check: Memory > 2GB free          (reuse self_test probe_memory logic)
    ├── Check: Lock screen visible/centered (LockScreenState check)
    │
    ├── For each FAIL → auto_fix::try_fix(check)  ← NEW (or extend self_heal)
    │       ├── Restart ConspitLink
    │       ├── Kill orphaned game
    │       └── Re-check after fix attempt
    │
    ├── All pass (or fixed) → PreFlightResult::Ok
    │       └── ws_handler continues with existing BillingStarted logic
    │               (lock_screen.show_pin_screen, overlay.activate, etc.)
    │
    └── Any unfixable → PreFlightResult::MaintenanceRequired { reasons }
            ├── lock_screen.show_maintenance_required(reasons)  ← NEW STATE
            ├── send AgentMessage::PreFlightFailed to server    ← NEW PROTOCOL MSG
            └── return HandleResult::Continue (do NOT process BillingStarted further)
```

---

## Component Responsibilities

| Component | Responsibility | Status |
|-----------|----------------|--------|
| `ws_handler.rs` | Dispatch `CoreToAgentMessage` variants — BillingStarted triggers gate | MODIFY |
| `pre_flight.rs` | Run all checks, attempt auto-fixes, return pass/fail verdict | NEW |
| `lock_screen.rs` | Add `MaintenanceRequired` state to `LockScreenState` enum | MODIFY |
| `self_test.rs` | Source of reusable probe logic (HID, disk, memory, process scan) | UNCHANGED |
| `app_state.rs` | Source of truth for HID state, billing state, game state | UNCHANGED |
| `failure_monitor.rs` | Background failure detection — unaffected by pre-flight | UNCHANGED |
| `rc_common/protocol.rs` | Add `PreFlightFailed` / `PreFlightPassed` `AgentMessage` variants | MODIFY |

---

## Where the Gate Goes

**The gate belongs in `ws_handler.rs`, inside the `BillingStarted` match arm.**

Rationale:
- `ws_handler.rs` already owns all `CoreToAgentMessage` dispatch
- `BillingStarted` is the single correct trigger point — it fires exactly once per session start
- `event_loop.rs` should not contain business logic; it is the select! dispatch layer only
- A new file `pre_flight.rs` keeps the check logic isolated and testable

The gate is NOT placed in `event_loop.rs` because event_loop is structural (select! wiring), not behavioral.

**Code location in ws_handler.rs:**

```rust
CoreToAgentMessage::BillingStarted {
    billing_session_id, driver_name, allocated_seconds, ..
} => {
    // NEW: Run pre-flight before any session setup
    let pf_result = pre_flight::run(state, &state.config).await;
    match pf_result {
        PreFlightResult::Ok => {
            // Existing BillingStarted logic continues unchanged
            state.lock_screen.show_pin_screen(...);
            ...
        }
        PreFlightResult::MaintenanceRequired { reasons } => {
            state.lock_screen.show_maintenance_required(reasons.clone());
            let msg = AgentMessage::PreFlightFailed {
                pod_id: state.pod_id.clone(),
                reasons,
            };
            let _ = ws_tx.send(Message::Text(serde_json::to_string(&msg)?.into())).await;
            // Return HandleResult::Continue — do NOT break the WS loop
        }
    }
}
```

---

## LockScreenState: New MaintenanceRequired State

**File: `lock_screen.rs`**

Add one variant to the existing `LockScreenState` enum:

```rust
/// Pod cannot start a session — hardware or system failure detected by pre-flight.
/// Shows reasons to staff. Only cleared by server-sent ClearMaintenance message
/// or manual rc-agent restart.
MaintenanceRequired {
    reasons: Vec<String>,
},
```

**Why this fits the existing pattern:**
- All other states (`Lockdown`, `ConfigError`, `Disconnected`) follow the same shape: an enum variant with a message field, rendered by the existing HTTP server as an HTML page
- `show_maintenance_required()` method follows the same pattern as `show_lockdown()` and `show_config_error()`
- The existing 3-second browser auto-reload picks up the state change automatically — no new mechanism needed
- Unlike `Lockdown` (cleared by PIN), `MaintenanceRequired` is cleared only by `ClearMaintenance` server command or restart

**New method on LockScreenManager:**

```rust
pub fn show_maintenance_required(&mut self, reasons: Vec<String>) {
    let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
    *state = LockScreenState::MaintenanceRequired { reasons };
    // Do NOT call self.launch_browser() — browser already running
}
```

---

## New Module: pre_flight.rs

**Location:** `crates/rc-agent/src/pre_flight.rs`

**Public interface:**

```rust
pub enum PreFlightResult {
    Ok,
    MaintenanceRequired { reasons: Vec<String> },
}

pub async fn run(state: &mut AppState, config: &AgentConfig) -> PreFlightResult;
```

**Internals:**

```rust
struct CheckResult {
    name: &'static str,
    passed: bool,
    auto_fix_applied: bool,
    detail: String,
}

async fn check_ws_connected(state: &AppState) -> CheckResult;
async fn check_udp_alive(state: &AppState) -> CheckResult;
async fn check_hid_present() -> CheckResult;           // reuses self_test probe_hid logic
async fn check_conspitlink_running() -> CheckResult;   // process scan via sysinfo or tasklist
async fn check_no_orphaned_game(state: &AppState) -> CheckResult;
async fn check_ac_content(config: &AgentConfig) -> CheckResult;
async fn check_no_stuck_billing(state: &AppState) -> CheckResult;
async fn check_disk_space() -> CheckResult;            // reuses self_test probe_disk logic
async fn check_memory() -> CheckResult;                // reuses self_test probe_memory logic
async fn check_lock_screen_visible(state: &AppState) -> CheckResult;

async fn try_fix_conspitlink() -> bool;   // spawn ConspitLink.exe, verify process appears
async fn try_fix_orphaned_game() -> bool; // game_process::kill_orphaned_game()
```

**Design rules:**
- All checks run concurrently with `tokio::join!` (no sequential dependencies for read-only checks)
- Auto-fix attempts run sequentially after all checks complete (to avoid race between concurrent fixes)
- Re-check after each fix to verify it worked before moving on
- Total time budget: 5 seconds (10s would block the customer UX too long)
- Non-fatal failures (overlay render, shader cache) are WARN-level only, never block

---

## Data Flow: Pre-Flight Integration

### Pass Path

```
BillingStarted (WS) → ws_handler.rs
    → pre_flight::run() [~1-5s]
        → all checks pass (or auto-fixed)
    → PreFlightResult::Ok
    → existing BillingStarted logic unchanged:
        state.overlay.activate_v2(driver_name)
        state.lock_screen.show_active_session(driver_name, allocated_seconds)
        failure_monitor_tx.send_modify(billing_active = true)
        heartbeat_status.billing_active.store(true)
```

### Fail Path

```
BillingStarted (WS) → ws_handler.rs
    → pre_flight::run() [~1-5s]
        → check(s) fail, auto-fix attempted, still failed
    → PreFlightResult::MaintenanceRequired { reasons }
    → state.lock_screen.show_maintenance_required(reasons)
    → AgentMessage::PreFlightFailed { pod_id, reasons } → WS to racecontrol
    → racecontrol → kiosk dashboard badge (existing alert pathway)
    → ws_handler returns HandleResult::Continue (no panic, no state corruption)
    → billing_active stays FALSE — customer NOT charged
```

### Server Protocol Changes (rc-common)

Two new `AgentMessage` variants (agent to server):

```rust
PreFlightFailed {
    pod_id: String,
    reasons: Vec<String>,
    timestamp: DateTime<Utc>,
},

PreFlightPassed {
    pod_id: String,
    timestamp: DateTime<Utc>,
    checks_ran: u8,
},
```

One new `CoreToAgentMessage` variant (server to agent, optional):

```rust
ClearMaintenance {
    pod_id: String,
},
```

`ClearMaintenance` allows staff to clear the maintenance screen from the kiosk dashboard without requiring a pod restart. Handle in ws_handler.rs by transitioning lock screen to `Hidden` or `StartupConnecting`.

---

## Reuse from self_test.rs

`pre_flight.rs` should NOT call `self_test::run()` directly — `self_test::run()` is designed for on-demand diagnostic runs (triggered by server command), not low-latency per-session gates. Instead:

- Extract shared probe logic into free functions in `self_test.rs` marked `pub(crate)`
- `pre_flight.rs` calls those functions directly with its own timeout budget
- This keeps `self_test.rs` as the single source of probe implementations

Specific reuse candidates:

| Pre-flight check | self_test.rs source |
|-----------------|---------------------|
| HID wheelbase | `probe_hid()` — extract HID enumerate as `pub(crate) fn check_hid_device()` |
| Disk space | `probe_disk()` — extract sysinfo call as `pub(crate) fn available_disk_gb()` |
| Memory | `probe_memory()` — extract as `pub(crate) fn available_memory_gb()` |
| Process scan | Pattern already in kiosk.rs (allowlist scan) — reuse via `sysinfo::System::processes()` |

---

## Architectural Patterns

### Pattern 1: Gate Inside Handler, Not in Loop

**What:** The pre-flight gate lives inside `ws_handler::handle_ws_message()`, not in `event_loop::run()`.

**When to use:** When a check must run at a specific message boundary (BillingStarted) rather than on a timer.

**Trade-off:** Adds async await inside the WS message handler (currently synchronous-style dispatch). Acceptable because BillingStarted is a rare event (once per customer session), never on a hot path.

### Pattern 2: Enum-Driven Lock Screen State

**What:** Add `MaintenanceRequired` variant to existing `LockScreenState` enum. The existing HTTP server reads the state and renders appropriate HTML via the 3-second auto-reload loop.

**When to use:** All lock screen UI already follows this pattern — new states cost one enum variant plus one match arm in the HTML generator.

**Trade-off:** No new mechanism needed. The HTML template is embedded in the binary (see existing `ConfigError` and `Lockdown` rendering).

### Pattern 3: Concurrent Checks, Sequential Fixes

**What:** Run all pre-flight checks concurrently with `tokio::join!`. When failures are found, apply auto-fixes sequentially (one at a time, re-verify after each).

**When to use:** Concurrent reads are safe and fast. Sequential fixes prevent race conditions where two fixes interfere (e.g., killing orphaned game while also restarting ConspitLink).

**Trade-off:** Slightly longer fix time vs. parallel fixes. Correct behavior is worth the tradeoff.

### Pattern 4: Non-Blocking via HandleResult::Continue

**What:** When pre-flight fails, `ws_handler.rs` returns `HandleResult::Continue` (not `Break`, not `Err`). The select loop keeps running — the agent remains connected and responsive.

**When to use:** Always when handling a failure that should not disconnect the agent.

**Trade-off:** None. `HandleResult::Break` would disconnect the WS and trigger reconnect, which would reconnect without fixing anything.

---

## Anti-Patterns

### Anti-Pattern 1: Running Pre-Flight in event_loop.rs

**What people do:** Add the gate as a new select! branch or inject it before the reconnect loop.

**Why it's wrong:** `event_loop.rs` is structural wiring (select! dispatch), not the place for session business logic. The gate must fire on BillingStarted specifically, not on a timer or at loop entry.

**Do this instead:** Put the gate inside the `BillingStarted` match arm in `ws_handler.rs`.

### Anti-Pattern 2: Importing self_test::run() in pre_flight.rs

**What people do:** Call `self_test::run(&heartbeat_status, ...)` from pre_flight.rs to reuse probe logic.

**Why it's wrong:** `self_test::run()` runs all 22 probes with Ollama LLM verdict — that is 10+ seconds and wrong scope. Pre-flight needs 5-10 targeted checks in under 5 seconds.

**Do this instead:** Extract `pub(crate)` helper functions from self_test.rs probe implementations. Call them directly from pre_flight.rs with a tighter timeout budget.

### Anti-Pattern 3: Blocking the Customer PIN Entry UX

**What people do:** Make the lock screen show pre-flight progress, block BillingStarted until all checks complete.

**Why it's wrong:** The PIN entry screen is the customer's first touch. A visible 3-5 second delay before the screen appears degrades UX. The customer should not see the check happen.

**Do this instead:** Run pre-flight in the `BillingStarted` handler (which happens before `show_pin_screen`). If pre-flight passes, `show_pin_screen` fires immediately as today. If pre-flight fails, `show_maintenance_required` fires instead. From the customer's view: they scan QR, brief wait (same as today), then either PIN screen or maintenance screen. No visible intermediate state.

### Anti-Pattern 4: Charging Before Pre-Flight Completes

**What people do:** Set `billing_active = true` on `HeartbeatStatus` before the pre-flight gate resolves.

**Why it's wrong:** If pre-flight fails and the session is blocked, billing must not start. The customer would be charged for time on a pod that cannot deliver a session.

**Do this instead:** The existing `billing_active.store(true)` call stays exactly where it is — after the `PreFlightResult::Ok` branch. The fail branch never reaches that line.

---

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| `ws_handler.rs` to `pre_flight.rs` | Direct async fn call | `pre_flight::run(&mut state, &config)` |
| `pre_flight.rs` to `self_test.rs` | Extracted `pub(crate)` helpers | HID, disk, memory probes |
| `pre_flight.rs` to `lock_screen.rs` | Indirect via `AppState.lock_screen` | Passed via `&mut state` |
| `pre_flight.rs` to `game_process` | Direct call to find/kill orphaned PIDs | Existing `game_process::find_game_pid()` |
| `ws_handler.rs` to `rc_common::protocol` | New `AgentMessage::PreFlightFailed` variant | Requires rc-common change first |
| `racecontrol` server | Receives `PreFlightFailed`, shows badge on kiosk dashboard | Server-side work scope |

### External Services

| Service | Integration | Notes |
|---------|-------------|-------|
| ConspitLink.exe | Process check (sysinfo) + spawn via `std::process::Command` | Pre-flight check + auto-fix |
| HID (OpenFFBoard) | Enumerate only — do NOT open device | Same contract as self_test probe_hid |
| AC content path | `Path::exists()` check on AC install directory | Config-driven path from AgentConfig |

---

## Suggested Build Order

Build order derived from dependency graph. Each step must compile and test before the next.

| Step | File(s) | Work | Dependencies |
|------|---------|------|--------------|
| 1 | `rc_common/protocol.rs` | Add `PreFlightFailed`, `PreFlightPassed`, `ClearMaintenance` message variants | None — rc-common has no deps on rc-agent |
| 2 | `lock_screen.rs` | Add `MaintenanceRequired` variant to `LockScreenState` enum + `show_maintenance_required()` + HTML render branch | None — lock_screen is self-contained |
| 3 | `self_test.rs` | Extract `pub(crate)` helper functions: `check_hid_device()`, `available_disk_gb()`, `available_memory_gb()` | None — preparatory refactor, no behavior change |
| 4 | `pre_flight.rs` | New module: all check functions, auto-fix attempts, `PreFlightResult` enum, `run()` | Steps 1–3 complete; AppState, game_process, config access |
| 5 | `ws_handler.rs` | Add pre-flight gate inside `BillingStarted` arm; handle `ClearMaintenance` message | Steps 1–4 complete |
| 6 | `main.rs` | Add `mod pre_flight;` declaration | Step 4 complete |
| 7 | `racecontrol` server | Handle `PreFlightFailed` message, show kiosk dashboard badge | Steps 1 + 5 complete |

**Step 1 first** because `rc_common` is a shared lib — any protocol changes must exist before rc-agent code that uses them will compile.

**Step 2 before 4** because `pre_flight.rs` calls `state.lock_screen.show_maintenance_required()` — that method must exist.

**Step 3 before 4** to avoid duplicating probe logic in the new module.

**Step 7 last and decoupled** — racecontrol can gracefully ignore unknown `AgentMessage` variants during the deployment window, so pre-flight can be deployed to pods before the server upgrade without breakage.

---

## Scalability Considerations

| Concern | Current (8 pods) | Future |
|---------|-----------------|--------|
| Pre-flight duration | ~1-5s per session start is acceptable | If check count grows past 15, parallelize with join! per category |
| Auto-fix side effects | Sequential fixes safe at 1 per session | No concurrency issue — one session starts at a time per pod |
| Staff notification | WS message to kiosk badge is sufficient | If pod count grows, aggregate fleet view already in /fleet/health |

---

## Sources

- Direct inspection: `crates/rc-agent/src/event_loop.rs` (select! loop structure, ConnectionState)
- Direct inspection: `crates/rc-agent/src/ws_handler.rs` (BillingStarted dispatch, handle_ws_message signature)
- Direct inspection: `crates/rc-agent/src/lock_screen.rs` (LockScreenState enum, all 13 states, show_* methods)
- Direct inspection: `crates/rc-agent/src/app_state.rs` (AppState 34 fields)
- Direct inspection: `crates/rc-agent/src/self_test.rs` (22 probes, ProbeResult, SelfTestReport, VerdictLevel)
- Direct inspection: `crates/rc-agent/src/self_heal.rs` (startup repair pattern)
- Direct inspection: `crates/rc-agent/src/failure_monitor.rs` (FailureMonitorState, watch channel pattern)
- Direct inspection: `crates/rc-agent/src/main.rs` (startup sequence, module declarations)

---

*Architecture research for: rc-agent v11.1 Pre-Flight Session Checks*
*Researched: 2026-03-21 IST*
