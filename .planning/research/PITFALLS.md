# Pitfalls Research

**Domain:** Pre-flight session health gates added to an existing kiosk/agent system (rc-agent on Windows gaming pods)
**Researched:** 2026-03-21
**Confidence:** HIGH — all pitfalls grounded in the actual rc-agent codebase (ws_handler.rs, self_test.rs, lock_screen.rs, billing_guard.rs, failure_monitor.rs)

---

## Critical Pitfalls

### Pitfall 1: Blocking BillingStarted with Synchronous Checks

**What goes wrong:**
Pre-flight checks are awaited synchronously inside the `BillingStarted` arm of `handle_ws_message`. Each check takes up to 1-2 seconds (HID enumeration, ConspitLink process scan, disk space query). Total: 5-10 seconds of blocking on the critical path. The WebSocket receive loop is stalled. BillingTick messages queue up or are dropped. The lock screen never transitions from QrDisplay/PinEntry to ActiveSession. The customer sits at the PIN screen while the staff see billing as "started."

**Why it happens:**
The `BillingStarted` handler in ws_handler.rs currently does fast in-memory work only (set atomics, send to failure_monitor_tx, call lock_screen.show_active_session). Adding `await` calls for I/O in-line feels natural but the handler runs on the single WS receive task — there is no separate executor for it.

**How to avoid:**
Spawn the pre-flight check as a `tokio::spawn` from within the `BillingStarted` arm. The handler returns immediately; the check runs concurrently. Result is communicated back via a channel (the existing `ws_exec_result_tx` pattern or a new `preflight_result_tx`). Lock screen shows a transitional state ("Preparing your session...") during the check window. If checks pass within 3 seconds, transition to ActiveSession. If they fail, transition to MaintenanceRequired.

The `self_test::run_all_probes()` already uses `join_all` with individual timeouts — that pattern must be reused, not a serial await chain.

**Warning signs:**
- Pre-flight check code is written as `let result = run_checks().await;` inside the `BillingStarted` match arm
- No `tokio::spawn` or channel around the check invocation
- BillingTick processing stops during the check window

**Phase to address:**
Phase 1 (Pre-flight framework) — the concurrency model must be established before any individual checks are written.

---

### Pitfall 2: Auto-Fix Kills a Game on a Pod That Is Mid-Session

**What goes wrong:**
The "no orphaned game processes" check finds `acs.exe` running and auto-kills it. But the pod is between splits (BetweenSessions state) where the customer is still billed and expects to continue. The game kill ends their session. The customer loses their in-progress time. Staff have to issue a refund.

More subtle: the check runs during BillingStarted for pod N. Pod N's billing session just started. But the game kill logic uses `taskkill /F /IM acs.exe` without confirming the PID belongs to this pod's session. On a shared network segment, if there is any shared state, wrong assumptions compound.

**Why it happens:**
Orphan detection in billing_guard.rs correctly checks `billing_active=true + game_pid=None` before killing. But the pre-flight check runs at BillingStarted — billing IS active (billing_active just became true). The new code doesn't have access to the same suppression logic yet. It sees a game PID and calls it an orphan.

**How to avoid:**
Pre-flight game checks must use `AppState.game_process` (the agent's authoritative game PID record) not a raw process list scan. If `state.game_process` is `Some(_)`, the game is agent-managed and must NOT be killed. Only kill processes not tracked by `state.game_process`. Cross-check: game PID from `game_process.pid()` must differ from any PID found by the orphan scan.

Also: re-use `billing_guard`'s `recovery_in_progress` suppression gate. If `failure_monitor_tx.borrow().recovery_in_progress == true`, pre-flight auto-fix must be suppressed entirely.

**Warning signs:**
- Pre-flight orphan check uses `tasklist | find "acs.exe"` without checking `state.game_process`
- Auto-fix kills processes by name without PID verification
- No check for `BetweenSessions` lock screen state before killing

**Phase to address:**
Phase 1 (Pre-flight framework) — safe-kill rules must be written before any auto-fix logic ships.

---

### Pitfall 3: "Maintenance Required" State Has No Exit Path

**What goes wrong:**
A new `MaintenanceRequired` lock screen state is added. A check fails, the pod enters MaintenanceRequired. Staff fix the issue (reconnect the wheelbase, restart ConspitLink). But MaintenanceRequired has no automatic recovery path. The pod stays in maintenance mode until staff find the rc-agent restart option in the kiosk dashboard, or until rc-agent is manually restarted. Customers cannot use the pod for the rest of the shift even though the hardware is fine.

**Why it happens:**
The lock screen state machine (13 states in `LockScreenState`) currently transitions on explicit server commands (`BillingStarted`, `SessionEnded`, `BillingStopped`) or on local events (PinEntered, blank timer). Adding a state without adding exit transitions is easy to miss — Rust's match blocks will compile with `_ => {}` fallthrough.

**How to avoid:**
MaintenanceRequired must have two explicit exit transitions:
1. **Staff clearance:** A new `CoreToAgentMessage::ClearMaintenance` command from racecontrol (triggered by staff pressing "Clear Maintenance" on the kiosk dashboard). This transitions to `StartupConnecting` or `Hidden` and re-enables billing.
2. **Auto-retry:** A background task re-runs the failed checks every 30 seconds. If all checks pass, the pod self-clears MaintenanceRequired and sends `AgentMessage::MaintenanceCleared` to racecontrol. Staff are notified.

The auto-retry approach is critical — manual staff clearance adds toil and defeats the purpose of automated pre-flight.

**Warning signs:**
- MaintenanceRequired is added to the `LockScreenState` enum without a corresponding `ClearMaintenance` message in the protocol
- No background task scheduled to re-probe the failed check
- The kiosk dashboard shows the maintenance badge but has no "Clear" button

**Phase to address:**
Phase 2 (Lock screen integration) — exit paths must be designed alongside the state addition, not as a follow-up.

---

### Pitfall 4: Self-Test Probes Designed for Cold Boot Fail on Warm System

**What goes wrong:**
Several probes in `self_test.rs` assume a cold-start context. The `udp_port_AC` probe (Probe 6) checks whether the AC telemetry port is bound. At startup, nothing binds it — pass means "game not running yet." Between sessions, if AC crashed without cleanup, the port may still be bound by a zombie process — now the probe gives a false pass (port bound = must be OK) when the socket is actually dead. The CLOSE_WAIT probe checks `:8090` — between sessions, legitimate CLOSE_WAIT sockets accumulate during normal operation and do not indicate a problem.

**Why it happens:**
`self_test.rs` was written for startup verification after rc-agent starts — a cold system context. Between-session use is a "warm system" context with different baseline state. The probes were not designed with that context in mind.

**How to avoid:**
Pre-flight checks must be a separate module from `self_test.rs`, designed for warm-system semantics:
- UDP port checks: between sessions, a bound telemetry port means an orphaned game socket. The probe must invert its success condition: bound = fail (orphan), unbound = pass.
- CLOSE_WAIT: use a higher threshold for between-session checks (normal warm-system accumulation) vs. startup checks.
- Disk/memory: these are context-agnostic and safe to reuse.
- WS connected: safe to reuse — this is a live check regardless of context.

Do not call `self_test::run_all_probes()` from pre-flight. Create `preflight::run_checks()` that implements context-appropriate versions.

**Warning signs:**
- Pre-flight imports and calls `self_test::run_all_probes()` without filtering or adapting probes
- UDP port check reports pass when a game is not expected to be running
- CLOSE_WAIT threshold is the same 20-socket limit used at startup

**Phase to address:**
Phase 1 (Pre-flight framework) — the probe semantics decision (reuse vs. new module) must be made upfront.

---

### Pitfall 5: Staff Notification Flood on Repeated Check Failures

**What goes wrong:**
BillingStarted fires 8 times in 10 minutes as staff tests pods before opening. Each fires a pre-flight check. Checks fail (wheelbase cable was knocked). Each failure sends a WhatsApp/WS alert. Uday receives 8 identical "Pod 3 Maintenance Required" messages within 60 seconds. He ignores them as spam. The real failure that happens 2 hours later is also ignored.

The existing `email_alerts.rs` has rate limiting (`ALERT-02`) but staff notifications via WS dashboard badge and WhatsApp alerter do not share that rate limiter.

**Why it happens:**
Each pre-flight check is a new event — the alert system sees each as a distinct trigger, not as a repeated condition. There is no "is the same alert already active?" check.

**How to avoid:**
Pre-flight alerts must use a per-pod, per-failure-type deduplication window. Pattern from `failure_monitor.rs`: use `stuck_fired` / `idle_fired` boolean guards in the task-local state. For pre-flight: maintain a `HashMap<(pod_id, check_name), Instant>` of last-alerted times. Re-alert only after the pod has recovered and failed again, or after a 30-minute silence window.

The `MaintenanceRequired` state itself is the natural deduplication gate: only alert when entering MaintenanceRequired, not on every re-check failure.

**Warning signs:**
- Pre-flight failure handler calls `send_staff_alert()` directly without checking last-alert time
- No shared state between successive BillingStarted invocations for alert deduplication
- WhatsApp alerter receives multiple identical pod+failure messages within 60 seconds

**Phase to address:**
Phase 3 (Staff notification) — rate limiting logic must be specified before any notification call site is written.

---

### Pitfall 6: ConspitLink "Running" Check Passes but Process Is Hung

**What goes wrong:**
The ConspitLink check finds `ConspitLink.exe` in the process list via `tasklist`. Check passes. Session starts. Customer sits down. FFB wheel does not respond because ConspitLink's internal state is corrupted — it is running but not processing USB events. The customer complains, staff manually restart ConspitLink, session time is lost.

**Why it happens:**
Process existence check (`tasklist | find "ConspitLink.exe"`) is a necessary but not sufficient health check. A hung process appears alive in the process list. The existing `kiosk.rs` already distinguishes between allowed processes and process health, but ConspitLink specifically has no liveness probe beyond presence.

**How to avoid:**
ConspitLink health check must be two-stage:
1. Process exists (`tasklist`)
2. HID device is responding: enumerate HID devices (already in `self_test::probe_hid()`) and confirm OpenFFBoard VID:0x1209 PID:0xFFB0 is present

If both pass, the combination is a reasonable (not perfect) proxy for ConspitLink health. If the process exists but HID is absent, classify as ConspitLink hung — kill and restart it.

Note: do NOT open the HID device for a write/read test — `probe_hid()` explicitly enumerates only. Opening the HID device during a live session can cause ConspitLink to lose its handle. Enumerate only.

**Warning signs:**
- ConspitLink check is `tasklist | find "ConspitLink.exe"` and nothing more
- No HID cross-check for ConspitLink liveness
- Auto-fix restarts ConspitLink even when `state.game_process` is `Some(_)` (mid-session)

**Phase to address:**
Phase 1 (Hardware checks) — liveness definition for ConspitLink must be established before the check is coded.

---

### Pitfall 7: Pre-Flight Check Takes >5 Seconds and Customer Sees Blank Lock Screen

**What goes wrong:**
A ConspitLink restart (auto-fix) takes 3 seconds. HID re-enumeration after restart takes 2 more seconds. Disk space check via `std::fs::metadata` on a spinning drive takes 1 second. Total: 6 seconds. The lock screen transitions to a loading state but the HTTP server serving it has no "checking..." page — it shows the last state (PinEntry or SessionSummary). The customer sees a stale screen with no indication anything is happening.

**Why it happens:**
The lock screen HTML pages are static per-state. There is no "loading" sub-state for PinEntry. The 5-second hard budget from the milestone context is not enforced by any timeout in the framework.

**How to avoid:**
1. **Enforce the budget:** wrap the entire `preflight::run_checks()` in `tokio::time::timeout(Duration::from_secs(5), ...)`. If it times out, fail fast with `PreflightResult::Timeout` and let the session proceed (timeout = probably fine, not definitely broken).
2. **Show a transitional state:** Add `LockScreenState::PreflightChecking { driver_name }` or reuse `LaunchSplash` as the holding state during the 0-5 second window. The lock screen HTTP server already handles all states from a shared `Arc<Mutex<LockScreenState>>` — a state transition during the check window is safe.
3. **Time-box individual checks:** HID enum ≤ 1s, process scan ≤ 0.5s, ConspitLink restart ≤ 2s, disk check ≤ 0.2s. Any check exceeding its sub-budget is skipped and logged as "check skipped: timeout."

**Warning signs:**
- No `tokio::time::timeout` wrapping the overall pre-flight call
- Lock screen state is not updated before checks begin
- Individual checks have no per-check timeout (they rely on OS-level timeouts which may be 30+ seconds)

**Phase to address:**
Phase 1 (Pre-flight framework) — timeouts and the transitional lock screen state are foundational, not bolt-ons.

---

### Pitfall 8: Billing Check Detects "Stuck Session" Because Cleanup Hasn't Finished

**What goes wrong:**
BillingStarted fires. Pre-flight billing check queries the database: "is there an active billing session for this pod?" It finds one — from the session that just ended 500ms ago. The `BillingStopped` processing (session cleanup, database update) is asynchronous on the server side. The pre-flight check races the cleanup and sees stale state. It classifies the pod as having a stuck session and blocks the new session.

**Why it happens:**
The new session's BillingStarted is sent by racecontrol only after it has started the new session, but the previous session's database row may not yet be marked complete (async commit). The pre-flight check hits the same database 100ms later and finds two "active" sessions.

**How to avoid:**
Billing stuck-session check must use the agent's local state (`state.heartbeat_status.billing_active` atomic) not an HTTP query to the server. The agent knows when its own billing started — the `billing_active` atomic is set in `BillingStarted` before pre-flight runs. A stuck session from a previous customer would be caught by `billing_guard.rs` (already in production) before BillingStarted fires for a new session.

If a server-side check is required, add a grace period: only flag stuck if the prior session has been active for more than 10 seconds at the time of BillingStarted. Sessions that ended < 5 seconds ago are not stuck.

**Warning signs:**
- Pre-flight billing check makes an HTTP call to `/api/v1/billing/active?pod_id=X` rather than reading local atomic state
- No grace period between `BillingStopped` and the stuck-session threshold
- Test shows false positives when two sessions start within 2 seconds of each other

**Phase to address:**
Phase 1 (Billing checks) — use local state first, server state only as secondary with a time-grace.

---

### Pitfall 9: "Maintenance Required" Breaks State Machine Transitions for Server

**What goes wrong:**
Pod enters MaintenanceRequired. Racecontrol still has the pod marked as "available" in its pod reservation system. Staff try to book the pod — racecontrol sends BillingStarted. Ws_handler receives BillingStarted while in MaintenanceRequired state. The BillingStarted handler runs anyway (ws_handler has no state guard for MaintenanceRequired). Lock screen tries to transition from MaintenanceRequired to ActiveSession — the HTML page for that combination is not defined. Browser shows a blank page or the wrong state.

**Why it happens:**
The lock screen state machine in `lock_screen.rs` does not currently guard against illegal transitions. States transition whenever `set_state()` is called — there is no "transition allowed?" check. Adding MaintenanceRequired without adding transition guards leaves illegal paths open.

**How to avoid:**
Two defenses:
1. **Server-side gate:** When a pod sends `AgentMessage::MaintenanceRequired`, racecontrol's pod reservation system must mark the pod as unavailable. BillingStarted for a maintenance pod must be rejected at the server. Kiosk dashboard shows the maintenance badge and disables "Start Session" for that pod.
2. **Agent-side guard:** In `ws_handler`'s BillingStarted arm, check if `state.lock_screen` is in MaintenanceRequired state. If so, send `AgentMessage::MaintenanceActive { pod_id }` back to racecontrol and return without starting the session.

Both defenses are needed — the server guard prevents the race, the agent guard is the safety net.

**Warning signs:**
- No `AgentMessage::MaintenanceRequired` type defined in `rc_common::protocol`
- racecontrol's pod_reservation.rs is not updated to handle maintenance state
- BillingStarted handler in ws_handler.rs has no early-return guard for MaintenanceRequired

**Phase to address:**
Phase 2 (Lock screen integration) and Phase 3 (Server-side pod state) must be sequenced together — the server pod state must be updated in the same phase as the lock screen state.

---

### Pitfall 10: Screenshot-Based Display Validation Is Unreliable Between Sessions

**What goes wrong:**
A display validation check takes a screenshot and checks whether the lock screen is centered and visible. Between sessions, the previous customer may have changed display scaling (via Windows Display Settings — possible if kiosk lockdown is incomplete). The screenshot-based check uses pixel coordinates that assume 1920x1080 at 100% DPI. At 125% scaling, coordinates shift. The check reports "lock screen off-center" and blocks the session. In reality, the lock screen is fine.

Additional failure mode: taking a screenshot on Windows requires GDI+ API calls from a process running in Session 1 (the user session). rc-agent runs as a Windows Service in Session 0. Session 0 cannot capture Session 1 screenshots without additional WTS API calls.

**Why it happens:**
Screenshots seem like the obvious way to verify "the lock screen is showing." But the Session 0/1 boundary and DPI scaling make it unreliable. The existing `lock_screen.rs` uses `GetSystemMetrics` to handle multi-monitor bounds — it already handles this complexity for positioning, but not for validation.

**How to avoid:**
Skip screenshot-based validation entirely. The lock screen HTTP server at `127.0.0.1:18923` is a reliable liveness signal. The display check should be:
1. TCP connect to `127.0.0.1:18923` — confirms the HTTP server is running
2. HTTP GET `/` — confirms it returns 200 and the expected state HTML
3. Check that the Edge browser process hosting the lock screen is alive (`tasklist | find "msedge.exe"` with the `--kiosk` flag in its command line)

These three checks together confirm "lock screen is serving and browser is showing it" without requiring a screenshot.

**Warning signs:**
- Display check code imports a screenshot crate or uses `BitBlt`/GDI calls
- Check assumes fixed pixel coordinates (1920, 1080)
- No fallback when screenshot fails (returns false negative)

**Phase to address:**
Phase 2 (Display checks) — define the check methodology before coding. HTTP probe is correct, screenshot is wrong.

---

### Pitfall 11: Auto-Fix ConspitLink Restart Mid-Session When Another Billing Is Active

**What goes wrong:**
Pod 3 pre-flight check detects ConspitLink is not running (it crashed between sessions). Auto-fix restarts it. Pod 4 happens to have an active session — ConspitLink on Pod 4 is unaffected (it's a per-pod process). But if the restart script uses a machine-wide ConspitLink path and the USB device is shared (unlikely but possible with a USB hub), the restart on Pod 3 causes a brief USB enumeration that disrupts Pod 4's active FFB feedback. The Pod 4 customer loses force feedback for 2-3 seconds.

More realistic version: the restart script kills all `ConspitLink.exe` instances on the pod (`taskkill /F /IM ConspitLink.exe`) instead of the specific PID. If multiple instances are running (from a previous failed start), it kills the wrong one.

**How to avoid:**
ConspitLink restart must be PID-targeted, not name-targeted:
1. Get the PID of the non-responsive ConspitLink from `tasklist /FO CSV`
2. Kill only that PID: `taskkill /F /PID <pid>`
3. Start a new ConspitLink with full path from config

Auto-fix must also check `state.heartbeat_status.billing_active` before restarting ConspitLink. If billing is active (another customer is in-session on this pod — unlikely given pre-flight fires at BillingStarted, but possible if a stuck session persists), do not restart ConspitLink. Send MaintenanceRequired instead.

**Warning signs:**
- Auto-fix uses `taskkill /F /IM ConspitLink.exe` (name-based, kills all instances)
- No billing_active check before the ConspitLink restart
- ConspitLink path is hardcoded rather than read from config

**Phase to address:**
Phase 1 (Hardware checks + auto-fix) — PID-targeted kill must be the default pattern for all auto-fix process restarts.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Reuse `self_test::run_all_probes()` for pre-flight | No new code | Cold-boot probes give wrong results in warm-system context (UDP port semantics inverted) | Never — create separate `preflight::run_checks()` |
| Inline await in BillingStarted handler | Simple code | Blocks WS receive loop, customer waits at lock screen | Never — always spawn |
| Name-based process kill (`taskkill /IM`) | Simpler command | Kills wrong instance, disrupts active sessions | Never for ConspitLink auto-fix; acceptable for game process cleanup when no session is active |
| HTTP query to server for billing state check | Authoritative server data | Races async cleanup, causes false stuck-session positives | Acceptable only with a 5+ second grace period gate |
| Screenshot for display validation | Intuitive approach | Session 0 cannot capture Session 1 display; DPI scaling breaks coordinates | Never — use HTTP probe to lock screen server instead |
| Single alert per failure (no deduplication) | Simple alert code | Notification flood on repeated failures | Never in production — add deduplication from day one |

---

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| `ws_handler` + pre-flight spawn | Borrowing `state` into the spawned task (not `Send`-safe) | Clone only the fields needed by the check (Arc refs, config values); do not move `state` into the spawn |
| `LockScreenManager.set_state()` from spawned task | `LockScreenManager` holds `Arc<Mutex<>>` internally so it IS Send; but calling it from a spawn requires the manager reference to be cloned before spawn | Clone the `Arc` ref to the lock screen manager before spawning the pre-flight task |
| `failure_monitor_tx` watch channel + pre-flight | Pre-flight reading `billing_active` via `failure_monitor_tx.borrow()` races with BillingStarted setting it | Read the `heartbeat_status.billing_active` atomic directly — it is set synchronously in BillingStarted before the spawn |
| racecontrol `pod_reservation.rs` + MaintenanceRequired | Server has no maintenance pod concept; adding it requires changes to pod state enum, kiosk API, and dashboard | Server-side pod state update must be in scope for the lock screen integration phase, not deferred |
| `whatsapp_alerter` + pre-flight failures | alerter has no per-pod cooldown for maintenance alerts | Add maintenance alert cooldown at the call site; do not rely on the alerter to deduplicate |
| HID enumeration + open session | `hidapi::HidApi::new()` is safe (enumerate only); but if any code path calls `device.open()` on the HID device during a session, ConspitLink loses its handle | Never open the HID device; enumerate only. Add a code review rule. |

---

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Serial pre-flight checks | Total check time = sum of all check times (5-15s) | Run all checks concurrently with `tokio::join!` or `join_all`, same as `self_test::run_all_probes()` | Immediately — even 3 serial 2s checks exceed the 5s budget |
| `netstat -ano` called multiple times | Each call takes 200-500ms on Windows; calling it for UDP check AND CLOSE_WAIT check means 1s+ overhead | Call `netstat -ano` once, cache output, parse for all checks that need it | First time — the overhead is immediate |
| HID enumeration on every pre-flight | `HidApi::new()` enumerates all USB devices, takes 200-500ms | Cache result in `AppState.hid_detected` (already exists); only re-enumerate if the cached result is negative | Always — unnecessary if cached state is fresh |
| ConspitLink process scan with full `tasklist` | `tasklist` outputs all processes; parsing takes 50-200ms | Use `tasklist /FI "IMAGENAME eq ConspitLink.exe" /FO CSV` to filter at the OS level | At 100+ running processes — common on gaming pods |
| Disk space check on spinning HDD | `std::fs::metadata("C:")` blocks the thread; HDD seek time adds 20-100ms | Run in `tokio::task::spawn_blocking`; it already IS blocking I/O | Always on HDD pods (check if pods have SSDs) |

---

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No lock screen state during check window | Customer sees stale PIN entry screen or session summary for 2-5 seconds; confusing | Add `LaunchSplash` or a "Preparing your session..." message immediately on BillingStarted, before checks begin |
| MaintenanceRequired shows technical error details | Customer sees "ConspitLink.exe PID 4821 not found" — confusing and unprofessional | Show only branded message: "Pod maintenance in progress. Staff have been notified." Technical details go to logs only. |
| Staff notified before auto-fix is attempted | Staff rush to the pod before the agent has had a chance to fix it | Attempt auto-fix first; notify staff only if auto-fix fails (same pattern as existing `failure_monitor` → `ai_debugger` → alert flow) |
| Maintenance badge visible to customers on kiosk dashboard TV | Kiosk dashboard is sometimes on a TV visible to customers in the venue | Maintenance details (pod number, failure reason) should require staff PIN to view; public view shows only "Pod unavailable" |

---

## "Looks Done But Isn't" Checklist

- [ ] **Timing budget:** Run a stopwatch test. Book a session on a healthy pod. Measure time from BillingStarted to lock screen showing "ActiveSession." Must be under 5 seconds wall-clock.
- [ ] **Concurrency:** Check that BillingTick messages are processed normally while pre-flight is running. Look for dropped ticks in logs.
- [ ] **MaintenanceRequired exit:** Manually trigger a failure on Pod 8, confirm MaintenanceRequired state. Then fix the issue. Verify the pod auto-clears within 60 seconds without a manual restart.
- [ ] **False positive test:** Run pre-flight on a fully healthy pod 20 times consecutively. Zero should report failure.
- [ ] **Safe-kill verification:** Start an active session on Pod 8. Manually set `game_process = None` in a test. Verify pre-flight does NOT kill the game that is actually running.
- [ ] **Notification deduplication:** Trigger a failure that persists. Confirm only one staff notification is sent, not one per BillingStarted attempt.
- [ ] **Server pod state:** After MaintenanceRequired, verify racecontrol marks the pod unavailable. Try booking the pod from the kiosk — it must be blocked.
- [ ] **Warm-system UDP probe:** Run pre-flight immediately after a session ends (game just closed). The UDP telemetry port should NOT be bound; if it is, it's an orphan. Verify the check catches this correctly.
- [ ] **ConspitLink restart race:** Kill ConspitLink manually on Pod 8. Start a new session. Verify ConspitLink is restarted and working before the lock screen shows ActiveSession.
- [ ] **Check module separation:** Verify `preflight::run_checks()` is a different function from `self_test::run_all_probes()`. No shared call path.

---

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| BillingStarted blocks WS loop | HIGH | Roll back pre-flight to a pass-through (disable checks temporarily). The `disable_preflight` config flag should exist from day one for exactly this scenario. |
| MaintenanceRequired with no exit | MEDIUM | Restart rc-agent on the pod (racecontrol fleet exec → `sc stop rc-agent && sc start rc-agent`). Pod comes back in StartupConnecting state. |
| False positive blocks healthy pod | MEDIUM | Staff uses "Clear Maintenance" button in kiosk dashboard. Temporarily raise check thresholds in config until false positive source is identified. |
| ConspitLink restart kills active session | HIGH | Immediately issue BillingPause for the affected pod. Apply manual FFB restart via rc-sentry `/exec` endpoint. Session time compensation is manual. |
| Notification flood | LOW | Add a `preflight_alert_cooldown_secs` config field. Set to 1800 (30 minutes) in production. Restart racecontrol to apply. |
| Pre-flight timeout causes all checks to skip | LOW | Increase `preflight_timeout_secs` in config. Checks revert to pass-through until timeout is resolved. No customer impact. |

---

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Blocking BillingStarted with sync checks | Phase 1: Framework | BillingTick messages arrive normally during pre-flight; no dropped ticks in log |
| Auto-fix kills game mid-session | Phase 1: Framework | Safe-kill checklist test: game not killed when `state.game_process` is Some |
| MaintenanceRequired has no exit path | Phase 2: Lock screen integration | Manual test: fix a failure, pod auto-clears within 60s without restart |
| Self-test probes wrong in warm context | Phase 1: Framework | Create separate `preflight::run_checks()`; no call to `self_test::run_all_probes()` |
| Staff notification flood | Phase 3: Notifications | 20 consecutive failures → 1 staff notification, not 20 |
| ConspitLink check passes but process hung | Phase 1: Hardware checks | HID liveness cross-check code present; code review confirms no `device.open()` |
| >5s check blocks customer at lock screen | Phase 1: Framework | Stopwatch test: BillingStarted to ActiveSession under 5s on healthy pod |
| Billing check races session cleanup | Phase 1: Billing checks | Local atomic used for billing state; no HTTP query to server |
| MaintenanceRequired breaks server pod state | Phase 2 + Protocol | racecontrol marks pod unavailable on `AgentMessage::MaintenanceRequired`; kiosk blocks booking |
| Screenshot-based display validation | Phase 2: Display checks | HTTP probe to :18923 used; no screenshot/GDI calls in codebase |
| ConspitLink restart kills wrong process | Phase 1: Hardware checks | PID-targeted kill in all auto-fix code; name-based kill only for confirmed orphan games |

---

## Sources

- `crates/rc-agent/src/ws_handler.rs` — BillingStarted handler structure, critical path analysis
- `crates/rc-agent/src/self_test.rs` — existing probe implementations, timeout patterns
- `crates/rc-agent/src/lock_screen.rs` — LockScreenState enum (13 states), state machine structure
- `crates/rc-agent/src/billing_guard.rs` — suppression gate patterns (recovery_in_progress, billing_paused)
- `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState, debounce patterns (stuck_fired), CRASH-01/02 detection rules
- `crates/rc-agent/src/app_state.rs` — AppState structure, game_process field, hid_detected field
- `.planning/PROJECT.md` — v11.1 milestone goals, constraint list, existing system capabilities
- `CLAUDE.md` — Windows Service context, Session 0/1 boundary, ConspitLink VID:PID, deploy rules

---
*Pitfalls research for: Pre-flight session health gates added to existing kiosk/agent system*
*Researched: 2026-03-21*
