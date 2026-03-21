# Feature Research: Pre-Flight Session Checks

**Domain:** Pre-session health gate for kiosk/sim racing pod agent (rc-agent)
**Researched:** 2026-03-21
**Confidence:** HIGH (based on direct codebase audit + domain analysis of kiosk/arcade/sim racing health-gate patterns)

---

## Feature Landscape

### Table Stakes (Staff Expect These)

Features that a pre-flight gate must have. Missing any of these means the gate cannot do its job — it either blocks valid sessions, misses real failures, or produces noise that staff learn to ignore.

| # | Feature | Why Expected | Complexity | Existing Dependency |
|---|---------|--------------|------------|---------------------|
| TS-1 | **Triggered on BillingStarted, runs before session begins** | The gate must intercept the session lifecycle at the right point. A gate that runs at startup or on a cron is not a pre-flight check — it's a background monitor. | LOW | `ws_handler.rs` BillingStarted handler — the hook point already exists. Insert pre-flight call before `state.lock_screen.show_active_session()`. |
| TS-2 | **WebSocket connected to racecontrol** | If the WS is down when a session starts, billing ticks cannot arrive, overlay will not update, and BillingStopped will be missed. A pod with a dead WS should not enter a customer session. | LOW | `HeartbeatStatus.ws_connected` atomic already exists. Read it. |
| TS-3 | **UDP heartbeat alive (last packet < threshold)** | Telemetry silence means no lap times are being captured. For a sim racing pod, no telemetry = no laps = no leaderboard. The customer paid for nothing. | LOW | `HeartbeatStatus` already tracks UDP. `FailureMonitorState.last_udp_secs_ago` exists. Check it (warn if > 30s, but this is soft-fail since no session is running yet). |
| TS-4 | **Wheelbase HID connected (Conspit Ares VID:0x1209 PID:0xFFB0)** | Customer cannot steer without the wheelbase. If HID is missing at session start, they get an unusable pod. Auto-fix: rescan HID. If still missing after rescan, alert staff and block. | LOW | `FailureMonitorState.hid_connected` exists. Auto-fix: re-enumerate HID via `ffb_controller`. |
| TS-5 | **No orphaned game process from previous session** | A lingering `acs.exe` or `F12025.exe` from a crashed previous session means the new session will either fail to launch or the customer will walk into someone else's in-progress game. | LOW | `game_process.rs` has process detection. Auto-fix: `kill_orphaned_game()` — this pattern already exists in `ai_debugger.rs`. |
| TS-6 | **Billing clear — no active session from previous customer** | If the previous session was not cleanly ended (billing_guard missed the orphan or the HTTP call failed), starting a new session on top of an active one corrupts billing state. | LOW | `FailureMonitorState.billing_active` and `active_billing_session_id` exist. Auto-fix: attempt HTTP orphan-end with existing `attempt_orphan_end()` logic. |
| TS-7 | **Disk space > 1GB free on system drive** | AC replays, log files, and temp files accumulate. If disk is full, AC cannot write session data, Windows Update may fail mid-session, and game launch can fail silently. 1GB is the minimum safe floor. | LOW | `self_test.rs` already has disk probe logic — reuse it. |
| TS-8 | **Memory > 2GB free** | AC + CSP + overlay + rc-agent together consume ~4-6GB. Less than 2GB free at session start means the customer will hit frame drops or OOM crashes mid-session. | LOW | `self_test.rs` already has memory probe logic — reuse it. |
| TS-9 | **ConspitLink process running** | ConspitLink is the bridge between the Conspit Ares wheelbase firmware and AC. Without it, FFB is dead even if HID is connected. | LOW | `self_test.rs` already has process detection. Auto-fix: spawn ConspitLink. |
| TS-10 | **Auto-fix failures before alerting** | A gate that alerts staff on every transient failure is noise. Staff will disable it or ignore it. Auto-fix first — restart the process, kill the orphan, rescan HID — then only alert if auto-fix cannot resolve. | MEDIUM | `ai_debugger.rs` `try_auto_fix()` with canonical keywords. Pattern already established. |
| TS-11 | **"Maintenance Required" lock screen when unfixable** | If auto-fix fails, the pod should show a clear "Maintenance Required" screen and block the session. The customer should not be handed a broken pod. Staff need a visible signal. | LOW | `LockScreen` already has states for special displays. New state: `MaintenanceRequired` with failure summary text. |
| TS-12 | **Staff notification via WebSocket on unresolved failure** | Staff at the kiosk dashboard need to know which pod blocked and why without walking to the pod. The WS channel is the established mechanism. | LOW | `AgentMessage` enum — add `PreFlightFailed` variant. Server receives, routes to kiosk dashboard badge. |

### Differentiators (Competitive Advantage)

Features that go beyond the basic gate and make the system genuinely better for operations. Not required for the gate to work, but high value.

| # | Feature | Value Proposition | Complexity | Notes |
|---|---------|-------------------|------------|-------|
| D-1 | **Structured pre-flight result with per-check status** | Instead of a single pass/fail, the gate returns a structured result: which checks passed, which failed, what auto-fix was attempted, what the fix outcome was. Server logs this per session. Staff can see "pod 3: HID rescan needed (auto-fixed)" rather than just "failure". | MEDIUM | Model after existing `SelfTestReport` / `ProbeResult` types in `self_test.rs`. Pre-flight result serializes to JSON, sent as `PreFlightResult` AgentMessage. |
| D-2 | **Configurable failure policy per check (hard vs soft block)** | Some checks (WS connected, HID connected, no active session) are hard blocks — do not start the session. Others (UDP heartbeat, disk space approaching limit) are soft warnings — start the session but flag the pod in the dashboard. Config-driven via `rc-agent.toml`. | MEDIUM | Adds `[preflight]` section to config. `hard_block_checks = ["ws", "hid", "billing_clear"]`, `soft_warn_checks = ["udp", "disk"]`. |
| D-3 | **Pre-flight result visible in fleet health dashboard** | Uday and staff should see a "last pre-flight" field per pod in the fleet health view — timestamp, pass/fail, which check triggered. Adds operational visibility without requiring staff to watch logs. | MEDIUM | Extend `PodFleetStatus` with `last_preflight_at`, `last_preflight_ok`, `last_preflight_detail`. Server already has `/api/v1/fleet/health`. |
| D-4 | **Kiosk dashboard badge on pre-flight failure** | When a pod's pre-flight fails, the pod card in the kiosk dashboard shows a badge (e.g. "Maintenance Required") in Racing Red (#E10600) until staff acknowledge or the pod self-clears. | MEDIUM | Requires server-side state (preflight_blocked: bool per pod) and kiosk frontend badge render. Not complex, but crosses the Rust/Next.js boundary. |
| D-5 | **Overlay renders correctly check** | Verify the overlay process is alive and responding before the session starts. A dead overlay means the customer has no HUD. Auto-fix: restart overlay process. | LOW | `self_test.rs` already has TCP probe for overlay port. Reuse it. Restart via `overlay.rs`. |
| D-6 | **AC content accessible check** | Verify the AC install directory and at least one car/track is present. A corrupted or missing content install will fail at launch, but the error message is cryptic. A pre-flight check surfaces this cleanly. | MEDIUM | Check `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content\cars` exists and is non-empty. This is a soft-fail (warn, don't block) unless content is completely absent. |
| D-7 | **CLOSE_WAIT socket cleanup before session** | `self_test.rs` already detects CLOSE_WAIT leaks. Run this as a pre-flight check and auto-clean before the session starts rather than waiting for the self-test cron. Prevents "port 8090 already in use" on session start. | LOW | Reuse existing CLOSE_WAIT detection logic. Auto-fix: `netsh int ip delete tcpconnections` or process restart. |

### Anti-Features (Do Not Build)

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| **GPU temperature check** | GPU temp varies 30-80°C legitimately during normal operation. Checking temp at BillingStarted tells you nothing useful — it will always pass after idle and always spike during gaming. Creates noise without action criteria. | Monitor GPU temperature continuously in the background if thermal management is needed (future milestone). Not a session gate. |
| **Full 22-probe self_test on every BillingStarted** | self_test.rs runs 22 probes with a 10-second per-probe timeout. Running all of them on every session start adds up to 10+ seconds of latency before a customer can play. Some probes (Ollama, Steam API ping) are irrelevant to session readiness. | Run only the 8-10 session-relevant checks. Reuse probe functions but do not invoke the full self_test pipeline. |
| **Customer-visible error messages** | The pre-flight failure screen should target staff, not customers. A customer seeing "HID device VID:0x1209 PID:0xFFB0 not found" is confusing and undermines trust in the venue. | Lock screen shows "Setting Up Your Pod — Please Wait" during pre-flight. On hard failure, show "Maintenance Required — Staff Notified." No technical details to customers. |
| **Blocking the WS send to racecontrol while pre-flight runs** | Pre-flight runs async. Blocking the WS event loop during pre-flight would prevent BillingTick, ping/pong, and other messages from arriving. This would cause the WS to drop due to keepalive timeout. | Pre-flight runs in a tokio::spawn (or spawn_blocking for sync probes). The WS event loop continues. Pre-flight completion triggers a single WS message (PreFlightResult) to the server. |
| **Per-check timeouts > 5 seconds** | A pre-flight check that takes 5+ seconds per probe will make session start feel sluggish. Customers are standing at the pod waiting. | Cap all pre-flight probes at 2 seconds. Self_test already uses 10s timeouts for startup (acceptable then, not for session start). |
| **Retry loops inside the auto-fix** | billing_guard already has a 3-attempt retry loop for orphan auto-end. Adding retry loops inside each pre-flight auto-fix creates unbounded wait time before staff are alerted. | Auto-fix attempts once. If it fails, send PreFlightFailed immediately. Do not retry in the pre-flight path — the retry logic belongs in the background monitors (failure_monitor, billing_guard). |
| **Pre-flight cancellation on new BillingStarted** | If a second BillingStarted arrives while pre-flight is running (race condition or double-click from staff), running two concurrent pre-flights creates corrupted state. | Pre-flight acquires a per-pod mutex/atomic flag. Second BillingStarted is queued or rejected until pre-flight completes. |

---

## Feature Dependencies

```
BillingStarted (ws_handler.rs)
    └──triggers──> PreFlight framework (pre_flight.rs — new)
                       ├──reads──> FailureMonitorState (hid_connected, billing_active, last_udp_secs_ago)
                       ├──reads──> HeartbeatStatus (ws_connected)
                       ├──reuses──> self_test.rs probe functions (disk, memory, tcp port)
                       ├──reuses──> game_process.rs (orphan detection)
                       ├──reuses──> ai_debugger.rs try_auto_fix() (auto-fix dispatch)
                       ├──writes──> LockScreen (MaintenanceRequired state — new state)
                       └──sends──> AgentMessage::PreFlightFailed (new variant)
                                       └──routed by server──> kiosk dashboard badge (D-4)

TS-11 (MaintenanceRequired state)
    └──requires──> lock_screen.rs show_maintenance_required() method (new)

D-3 (Fleet dashboard pre-flight field)
    └──requires──> PodFleetStatus extended with preflight fields
                       └──requires──> AgentMessage::PreFlightResult variant

D-4 (Kiosk badge)
    └──requires──> Server-side preflight_blocked state per pod
    └──requires──> D-3 (fleet status integration)
```

### Dependency Notes

- **TS-10 (auto-fix) requires TS-5, TS-6, TS-4**: Cannot auto-fix what you cannot detect. Each check must produce a structured result before auto-fix can be dispatched.
- **TS-11 (MaintenanceRequired screen) requires TS-10 (auto-fix)**: Only show the hard-blocked state after auto-fix has been attempted and failed.
- **TS-12 (staff WS notification) requires TS-11 (lock screen block)**: Notify staff only when the pod is actually blocked, not on transient failures that auto-fix resolved.
- **D-3 and D-4 are independent of each other** but both require the `PreFlightResult` AgentMessage variant to carry structured data to the server.

---

## MVP Definition

### Launch With (v11.1 core)

Minimum gate that justifies the milestone. Covers the failure modes that actually bite operations — orphan sessions, dead wheelbases, dead WS.

- [ ] **TS-1** — BillingStarted hook in ws_handler.rs calls pre_flight::run() before show_active_session()
- [ ] **TS-2** — WS connected check (hard block)
- [ ] **TS-4** — HID connected check with one HID rescan auto-fix attempt (hard block if rescan fails)
- [ ] **TS-5** — Orphan game process check with kill auto-fix (hard block if kill fails)
- [ ] **TS-6** — No active billing session check with orphan-end auto-fix (hard block if HTTP call fails)
- [ ] **TS-7** — Disk space > 1GB check (hard block)
- [ ] **TS-8** — Memory > 2GB check (soft warn, do not block)
- [ ] **TS-9** — ConspitLink process running with spawn auto-fix (hard block if spawn fails)
- [ ] **TS-10** — Auto-fix attempted before any alert
- [ ] **TS-11** — MaintenanceRequired lock screen state on hard block
- [ ] **TS-12** — PreFlightFailed AgentMessage to server on hard block

### Add After Validation (v11.1 polish)

- [ ] **TS-3** — UDP heartbeat soft-warn (add after core checks proven stable — this is low-signal before any session has run)
- [ ] **D-1** — Structured PreFlightResult with per-check status (add when staff ask "which check failed?")
- [ ] **D-5** — Overlay TCP port check with restart auto-fix

### Future Consideration (v11.2+)

- [ ] **D-2** — Configurable hard/soft policy per check via toml config
- [ ] **D-3** — Fleet dashboard pre-flight field (requires frontend work)
- [ ] **D-4** — Kiosk badge on pre-flight failure (requires server state + Next.js changes)
- [ ] **D-6** — AC content directory existence check
- [ ] **D-7** — CLOSE_WAIT pre-session cleanup

---

## Feature Prioritization Matrix

| Feature | Operational Value | Implementation Cost | Priority |
|---------|-------------------|---------------------|----------|
| TS-1 BillingStarted hook | HIGH — enables everything | LOW — one call site | P1 |
| TS-4 HID check + rescan | HIGH — #1 cause of "pod not working" complaints | LOW — state already tracked | P1 |
| TS-5 Orphan game kill | HIGH — prevents "walk into someone's game" | LOW — kill logic exists | P1 |
| TS-6 Billing clear | HIGH — prevents billing corruption | LOW — state + HTTP exist | P1 |
| TS-9 ConspitLink running | HIGH — no FFB without it | LOW — process check + spawn | P1 |
| TS-11 Maintenance screen | HIGH — staff need visible signal | LOW — new lock screen state | P1 |
| TS-12 Staff WS notification | HIGH — dashboard visibility | LOW — new AgentMessage variant | P1 |
| TS-7 Disk space | MEDIUM — rare but catastrophic when it hits | LOW — reuse self_test logic | P1 |
| TS-2 WS check | MEDIUM — WS reconnect usually self-heals | LOW — atomic read | P1 |
| TS-8 Memory check | MEDIUM — soft warn only | LOW — reuse self_test logic | P1 |
| D-1 Structured result | MEDIUM — better debugging | MEDIUM — new type + serialize | P2 |
| D-5 Overlay check | LOW — overlay rarely dies at session start | LOW — TCP probe reuse | P2 |
| TS-3 UDP check | LOW — no session running yet, false signal | LOW | P2 |
| D-2 Config-driven policy | LOW — nice to have | MEDIUM | P3 |
| D-3 Fleet dashboard field | MEDIUM — Uday visibility | MEDIUM — crosses Rust/Next.js | P3 |
| D-4 Kiosk badge | MEDIUM — staff UX | HIGH — server state + frontend | P3 |
| D-6 AC content check | LOW — content rarely disappears | MEDIUM | P3 |
| D-7 CLOSE_WAIT cleanup | LOW — background monitor handles it | LOW | P3 |

---

## Granularity Decision

The right granularity for pre-flight is **pod-state checks, not probe-everything**. The distinction:

**In scope (session-blocking conditions):**
- Process running / not running (ConspitLink, orphaned game)
- Hardware connected / not connected (HID)
- Billing state clean / dirty (active session lingering)
- Connectivity up / down (WebSocket)
- Resources available / exhausted (disk, memory)

**Out of scope (background monitoring, not session gates):**
- GPU temperature (continuous background concern)
- Ollama availability (debugging aid, not session-critical)
- Steam process (not needed for AC to run once launched)
- CLOSE_WAIT count (background leak, not a session blocker at normal counts)
- Full TCP port scan of all agent ports (only session-relevant ports matter)

Each check should complete in under 2 seconds. The full pre-flight set should complete in under 5 seconds total (concurrent execution). Anything that cannot meet this SLA should not be in the pre-flight path.

---

## Sources

- **Codebase audit (HIGH confidence):**
  - `crates/rc-agent/src/self_test.rs` — 22 probes, ProbeResult/SelfTestReport types, reusable probe functions
  - `crates/rc-agent/src/failure_monitor.rs` — FailureMonitorState, existing check conditions
  - `crates/rc-agent/src/billing_guard.rs` — attempt_orphan_end(), existing orphan detection patterns
  - `crates/rc-agent/src/ws_handler.rs` — BillingStarted hook point, existing session lifecycle
  - `crates/rc-agent/src/lock_screen.rs` — LockScreen state machine, show_active_session() call site
  - `crates/rc-agent/src/ai_debugger.rs` — try_auto_fix() pattern, canonical keyword dispatch
  - `crates/rc-agent/src/game_process.rs` — orphan game process detection
- **Domain analysis (MEDIUM confidence — derived from kiosk/arcade operational patterns):**
  - Kiosk/sim-racing health gate patterns are well-established in arcade operations: check hardware, check connectivity, check software state, block if unresolvable. No novel research needed — this is applied codebase knowledge.

---

*Feature research for: v11.1 Pre-Flight Session Checks (rc-agent)*
*Researched: 2026-03-21*
