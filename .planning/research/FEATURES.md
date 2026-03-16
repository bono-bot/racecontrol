# Feature Research — RC Bot Expansion (v5.0)

**Domain:** Deterministic auto-fix bot for sim racing venue pod management
**Researched:** 2026-03-16
**Confidence:** HIGH (based on direct codebase analysis of ai_debugger.rs, game_process.rs, driving_detector.rs, lap_tracker.rs, billing types, protocol.rs, and UDP heartbeat)

---

## How to Read This Document

Each failure class has three sections:

- **Table Stakes** — fix patterns the bot must have to be useful. Without these, staff still need to intervene constantly.
- **Differentiators** — fix patterns that make the bot smarter than a simple watchdog. Not required on day one.
- **Anti-Features** — approaches that look useful but create more problems than they solve.

The implementation table at the end maps every pattern to complexity and dependencies on existing code.

---

## Failure Class 1: Pod Crash / Hang

**What goes wrong:** Game process freezes with no UDP output (telemetry stops), rc-agent becomes unresponsive, or acs.exe exits unexpectedly with a non-zero exit code.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| UDP silence timeout | No UDP packet on any monitored port for N seconds while `game_state == Running` | Set `game_state = Error`, send `GameCrashed` to racecontrol | N = 30s for AC (active game), already tracks `last_udp_packet` in `DrivingDetector`. Distinguishes "in menu" from "frozen" by cross-checking `AcStatus`. |
| Orphaned process kill + relaunch | Game PID alive but exit code captured (non-zero), OR game PID gone unexpectedly | Call `fix_kill_stale_game()` (already exists), then send `LaunchGame` command back to self | Already partially implemented. Need: condition check before auto-relaunch — only if billing is active and session is live. |
| WerFault dismiss | `WerFault.exe` in tasklist within 10s of game exit | Call `fix_kill_error_dialogs()` (already exists) | Already implemented in `try_auto_fix`. Extend: check after every detected crash, not just AI suggestion path. |
| rc-agent liveness self-check | Detect own event loop stalled (watchdog ping timeout on internal channel) | Log + trigger graceful restart via service manager | rc-watchdog already monitors rc-agent PID. Self-check = separate internal channel ping every 30s. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Crash pattern classification | Exit code from `game_process.last_exit_code` matched against known codes (0xC0000005 = access violation, 0xC0000409 = stack overflow, -1 = generic crash) | Annotate `GameCrashed` message with `crash_class` field for richer dashboard display | Exit code already captured in `GameProcess.last_exit_code`. Pattern table is small and static. |
| Freeze vs crash discrimination | Process still alive (PID responds to `OpenProcess`) but zero UDP for >30s | Label as "freeze" not "crash" in `GameCrashed`. Apply taskkill instead of assuming self-exit. | `is_process_alive()` already exists. Combine with UDP silence check. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Auto-relaunch game without billing check | If billing ended before crash, relaunching starts a new session the customer did not pay for | Always verify `billing_active` and session status before any auto-relaunch |
| Killing rc-agent.exe on hang detection | rc-agent is on `PROTECTED_PROCESSES` list for good reason — killing it destroys the session | Let rc-watchdog handle rc-agent restart; bot handles only game processes |
| Looping relaunch on repeated crash | If AC crashes on the same track/car 3x, infinite auto-relaunch wastes the customer session time | Apply max 2 auto-relaunch attempts per billing session; escalate to staff alert on 3rd |

---

## Failure Class 2: Billing Edges

**What goes wrong:** Session stays `Active` after game process exits (stuck billing), `idle_time_ms` accumulates because DrivingState never fires (idle drift), or cloud sync failure leaves venue and cloud out of sync.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Stuck session cleanup | `billing_session.status == Active` but `game_state == Idle` for >60s and no `DrivingState::Active` | Send `StopSession` + `BillingStopped` to close the session at the last known active time | Billing FSM lives in racecontrol. Trigger from pod_monitor detecting game gone + billing still active. |
| Idle drift guard | `driving_state == Idle` for entire session duration (no Active ever recorded) | Flag session as `suspect` in DB; do not charge for drive time; alert staff | DrivingState already tracked per-pod. If `drive_time_ms == 0` at session end, auto-refund or zero-charge. |
| Cloud sync failure recovery | `cloud_sync` HTTP POST returns non-200 for >3 consecutive cycles (90s) | Log `CLOUD_SYNC_FAIL` with timestamp, continue venue operations, retry on next cycle | Cloud sync already runs every 30s. Missing: structured failure counting. Venue must never block on cloud. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Billing tick gap detection | `BillingTick` message from server stops arriving for >10s while session is `Active` | rc-agent locally pauses drive time accumulation, not cloud sync | BillingTick is sent every second. If gap detected, pause local drive time counter to avoid overcharging during WS outage. |
| Orphaned session on rc-agent restart | On agent startup, check if `billing_session_id` is set in config/state but no matching active timer on server | Send `SessionEvent::SessionEnded` to close the dangling session | `StartupReport` already sent on reconnect. Extend it to carry `billing_session_id` if one was active at crash. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Auto-refund on any sync failure | Cloud sync fails for network reasons unrelated to the session; auto-refunding creates false negatives | Only refund if session `drive_time_ms == 0` AND game never reached `AcStatus::Live` |
| Extending sessions automatically on crash | If a customer session ends early due to crash, auto-extending without staff approval bypasses pricing rules | Alert staff + send `GameCrashed { billing_active: true }` to dashboard; staff initiates extension |

---

## Failure Class 3: Network Repair

**What goes wrong:** WebSocket between rc-agent and racecontrol drops silently (half-open TCP), racecontrol HTTP endpoint unreachable due to DHCP drift on server IP, or server IP changes overnight.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| WS drop repair | UDP heartbeat `HeartbeatEvent::CoreDead` fires (no pong for `DEAD_TIMEOUT_SECS`) | Force WebSocket reconnect with escalating backoff | Already implemented via `HeartbeatEvent::CoreDead` to main loop reconnect. Verify this path is wired in main.rs. |
| HTTP endpoint retry | `reqwest` POST to `/billing/session/end` or `/laps` returns `ConnectionRefused` or timeout | Retry with 3x exponential backoff (1s, 2s, 4s); queue the payload if all retries fail | `cloud_sync.rs` already has retry logic. rc-agent HTTP calls need same pattern. |
| IP drift detection | DNS/static IP lookup fails; UDP heartbeat to configured IP gets no response for >60s | Log `SERVER_IP_DRIFT` warning; try fallback IPs (.51, .4, .23) from config if present | Server DHCP history shows drift: .51 to .23 to .4 to .23. Multi-IP fallback list in config is the pragmatic fix. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Heartbeat sequence gap alerting | UDP heartbeat `sequence` counter shows jumps >5 (packets lost) | Log sequence gap, escalate to email alert if gap >20 in 60s | Sequence already tracked in `HeartbeatPing`. Analysis is pure arithmetic — no extra state needed. |
| WS reconnect storm prevention | Multiple pods reconnect simultaneously after server restart | Add jitter to reconnect delay: `base_delay + random(0..pod_number * 500ms)` | Without jitter, all 8 pods hammer the server WS endpoint simultaneously. Pod number is available from config. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| mDNS/Bonjour discovery for server IP | Adds dependency on network service that may not be running; DHCP drift is already solved by reservation | Fix the root cause: DHCP reservation for server MAC `BC:FC:E7:2C:F2:CE` on the router (HOST-01 done in v2.0) |
| Automatic network stack reset (netsh int ip reset) | Resets ALL network interfaces including the pod own connection — drops all sessions on all pods | Clear only CLOSE_WAIT sockets on specific ports (already done in `fix_stale_sockets`) |

---

## Failure Class 4: USB Hardware (Wheelbase)

**What goes wrong:** Conspit Ares wheelbase (VID:0x1209 PID:0xFFB0) disconnects mid-session, HID device disappears from Windows device tree, or FFB torque remains engaged after game exit (safety hazard).

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| USB disconnect detection | `DrivingDetector` emits `DetectorSignal::HidDisconnected` (HID open fails or `hidapi` returns error) | Set `wheelbase_connected = false` in `PodStateSnapshot`, send staff alert via racecontrol | Already detected: `DrivingDetector.is_hid_connected()` tracks this. Missing: alert path when it flips false mid-session. |
| FFB zero on session end | Session ends normally or game crashes | Call `FfbController::zero_force()` before or concurrent with game kill | `FfbController` already implemented with `CMD_ESTOP`. Protocol message `FfbZeroed` already exists. Needs wiring into session-end path. |
| FFB zero on game crash | `GameCrashed` event fires | Call `FfbController::zero_force()` immediately, before any process cleanup | Safety requirement: high torque left engaged with no game feedback is physically dangerous. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| USB device re-enumeration probe | Disconnect detected, game still running, billing active | Wait 10s then re-scan HID bus; if device reappears, log recovery and continue without staff | HID rescan = `hidapi::HidApi::new()` re-enumerate. ~10s is typical Windows USB re-enum delay. |
| Consecutive disconnect counting | Same wheelbase disconnects >2x in one billing session | Add `disconnect_count` to session telemetry; alert staff after 2nd disconnect | Repeated disconnects indicate physical USB issue (cable/port) that bot cannot fix — needs staff intervention. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Sending USB power cycle command via software | Windows cannot reliably power-cycle USB from software without device-specific drivers; may corrupt USB hub state | Alert staff to physically reseat the USB cable; log the pod number and port |
| Disabling FFB entirely after disconnect | Customer loses FFB for rest of session even if wheelbase reconnects | Zero torque on disconnect, restore to last `ffb_percent` value when reconnect detected |

---

## Failure Class 5: Game Launch Failures

**What goes wrong:** Content Manager (CM) process hangs at launch, `acs.exe` launches but exits within 5s (bad track/car combo), or CM window appears but never triggers the game process.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Launch timeout detection | `game_state == Launching` for >90s with no UDP packet received | Kill launch process, set `game_state = Error`, send `GameStateUpdate` with `error_message` | `GameLaunchInfo` has `launched_at`. Delta from now > 90s = timeout. AC typically loads in 20-40s. |
| CM hang kill | CM process (`Content Manager.exe` or `acmanager.exe`) alive >90s after launch command without spawning `acs.exe` | `taskkill /IM "Content Manager.exe" /F`, fall back to direct `acs.exe` launch | `LaunchDiagnostics.cm_attempted` already tracks this. `fallback_used` field exists for this scenario. |
| Immediate exit detection | `acs.exe` exits within 5s of spawn (exit code != 0) | Log `direct_exit_code` in `LaunchDiagnostics`, retry once with 10s delay, escalate to staff on 2nd failure | `GameProcess.last_exit_code` captures this. Already tracked. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Pre-launch dialog clearance | Before launch attempt, scan for `DIALOG_PROCESSES` (WerFault, msiexec, SystemSettings) | Kill all dialog processes from `DIALOG_PROCESSES` list (already defined in `ac_launcher.rs`) | Dialogs from previous crash can block new game window. List already exists — needs proactive sweep before each launch. |
| CM log error extraction | After CM hang/crash, read `%LOCALAPPDATA%\AcTools Content Manager\Logs\` for error lines | Include in `LaunchDiagnostics.cm_log_errors` field (already exists in type) | CM writes timestamped logs. Last 50 lines on failure gives diagnostic context without any AI. |
| race.ini validation before launch | Before writing `race.ini` for AC, verify track and car paths exist on disk | Return error immediately if `ContentScanner` shows track/car missing; skip launch entirely | `content_scanner.rs` exists. Cross-reference before writing race.ini to avoid AC crashing on load. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Relaunching game in infinite loop on immediate exit | If `acs.exe` exits immediately due to corrupted track/car, loop creates noisy logs and delays staff notice | Max 2 auto-retries per billing session; 3rd failure = staff alert with `cm_log_errors` payload |
| Modifying CSP/CM config files to fix launch | Config file mutations can corrupt the entire AC installation if written mid-launch | Only write `race.ini` (already done correctly); never touch `gui.ini`, CSP settings, or `launcher.ini` at runtime |

---

## Failure Class 6: Telemetry Gaps

**What goes wrong:** UDP port silent because game is in menu (not a bug), or game UDP silent because port is wrong, game crashed, or network config blocks the port locally.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Game-state-aware silence categorization | No UDP on port 9996 for >30s while `AcStatus == Live` (not Off/Menu/Pause) | Mark telemetry as `dropped`, send staff alert via racecontrol | `AcStatus` already tracked via shared memory. Critical distinction: silence in menu is normal; silence while `AcStatus::Live` is a problem. |
| Partial data flagging | UDP frame received but `sector1_ms == 0` AND `sector2_ms == 0` (both zero, not just one) while racing | Mark telemetry frame as `partial`, skip lap completion for that lap | Already have `suspect_flag` logic in `lap_tracker.rs`. Extend: partial frames should not trigger lap events. |
| Corrupted packet rejection | UDP frame length != expected size for sim type, or checksum mismatch (for sims that include one) | Drop packet, log `CORRUPT_PACKET` counter, alert if >10% drop rate in 60s | Each sim adapter `parse_frame()` already returns `Option` — rejected frames are implicit. Add explicit counter. |
| Port conflict detection | UDP bind on port 9996 fails at startup | Log `PORT_BIND_FAIL`, alert staff, suggest checking for other AC instances | rc-agent already spawns UDP listeners. Capture bind errors and surface them via `StartupReport`. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Persistent drop rate alerting | Corrupt or dropped packet rate exceeds 5% over 60s while game running | Email alert with pod_id, sim_type, drop rate; staff checks pod network cable | Useful signal for deteriorating hardware (bad USB-Ethernet adapter common in gaming rigs). |
| Telemetry recovery confirmation | After gap, UDP resumes — log recovery time | Include gap duration in `TelemetryFrame` metadata or separate `TelemetryRecovered` event | Useful for diagnosing intermittent issues vs sustained failures. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Inferring lap times from position data when UDP drops | Position-based lap detection is complex, sim-specific, and inaccurate without proper waypoints | Mark the lap as invalid during the gap; telemetry is either present and valid or absent |
| Restarting UDP listener to fix silence | UDP listener restart flushes any buffered packets and risks missing the lap completion event | Telemetry silence is a signal problem, not a listener problem; fix port conflicts at startup |

---

## Failure Class 7: Multiplayer Issues

**What goes wrong:** AC dedicated server (`RP_OPTIMAL` preset on .23) becomes unreachable mid-race, one pod desyncs from the server session, or pods launch at different times causing a mismatched lobby.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| AC server disconnect detection | `AcStatus` drops from `Live` to `Off` on all pods simultaneously (or majority) while billing active | Send staff alert with pod count, do NOT auto-end billing (customer deserves compensation decision by staff) | Simultaneous status change across pods = server-side issue. Single pod drop = pod-side issue. |
| Server reachability pre-check | Before multiplayer session launch, HTTP GET to AC server stracker API or `ac_server.rs` endpoint | If unreachable, block launch and alert staff rather than launching pods into an empty lobby | `ac_server.rs` exists. Add a pre-launch reachability check. |
| Single pod desync (pod-only drop) | `AcStatus` drops `Live` to `Off` on one pod while others remain `Live` | Send `StopGame` to the desynced pod, re-engage lock screen, alert staff to manually rejoin | Partial desync: one pod network or game crashed. Other pods continue; desynced pod needs manual re-entry. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Desync timeline logging | Record `AcStatus` transitions per pod with timestamps | On desync event, include timeline in staff alert (which pod dropped first, gap before others followed) | Pure timestamp comparison — no extra logic beyond what already flows through `GameStatusUpdate` messages. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Auto-rejoin desynced pod to AC server | Content Manager join URL must be re-generated with current session token; bot has no access to AC server session state | Alert staff; they re-trigger the join URL from the kiosk in 30 seconds |
| Restarting AC server process automatically | AC server manages active sessions for multiple pods; restart kills all other pods races | Alert Uday/staff via email; AC server restart is a manual, high-impact action |

---

## Failure Class 8: Kiosk PIN

**What goes wrong:** Customer enters wrong PIN 3+ times (locks them out), staff PIN conflicts with customer PIN on same day (theoretically impossible but must be handled), session token expired before customer scans QR.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| PIN fail count tracking | `PinEntered` response from server returns invalid; track consecutive failures per pod | After 3 failures: show "Contact staff" message on lock screen, send staff alert | Lock screen already has `pin_error: Option<String>` field. Failure counter is new state in rc-agent main loop. |
| Session token expiry detection | `ShowPinLockScreen` or `ShowQrLockScreen` contains `allocated_seconds` that has elapsed before PIN entry | Lock screen detects token age; send `SessionUpdate` to server requesting refresh | Token age = `Utc::now() - show_time`. If >5 min elapsed before customer scans, request new token. |
| Staff vs customer PIN audit log | Server returns "invalid" for a PIN that matches staff PIN computation for the day | Log `PIN_CONFLICT` warning with date and pod_id for audit trail | Cannot happen by design (separate auth endpoints) but log it explicitly if it somehow occurs. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| PIN lockout auto-reset | PIN lockout (3 failures) clears after 5 minutes | Auto-clear lockout counter after 5 min; no staff PIN needed for this reset | Prevents customer frustration if they mistyped once and return 5 min later. |
| QR scan soft alert | `ShowQrLockScreen` displayed for >120s with no `PinEntered` or session confirmation | Send soft alert to staff ("Customer on Pod X may need help with QR scan") | QR scanning fails on older phones or low-light conditions. Proactive staff nudge reduces wait time. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Auto-unlocking pod after repeated PIN failures | Security bypass — someone could brute-force PIN by waiting for auto-unlock | After lockout, require staff physical presence or dashboard unlock only |
| Displaying PIN hash or partial PIN in logs | Even partial hash exposure reduces PIN entropy significantly | Log only attempt count and pod_id; never log the PIN value itself |

---

## Failure Class 9: Lap Filtering

**What goes wrong:** AC sends a lap with `lap_time_ms = 0` (warm-up lap or load artifact), sector sum does not match lap total (partial telemetry), speed unrealistically high or low, or a lap is completed with a sector cut flag set.

### Table Stakes

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Zero-time lap rejection | `lap_time_ms == 0` | Drop silently; do not insert into DB | Already implemented: `if lap.lap_time_ms == 0 { return false }` in `persist_lap`. |
| Sub-minimum time rejection | `lap_time_ms < 20_000` (20 seconds) — impossibly fast for any track in the catalog | Set `suspect = true`, store but exclude from leaderboard | Already implemented: `sanity_ok = lap.lap_time_ms >= 20_000` in `persist_lap`. Needs per-track floor added. |
| Sector sum mismatch | `|sector1 + sector2 + sector3 - lap_time_ms| > 500ms` with all sectors present and non-zero | Set `suspect = true`, store but exclude from personal bests and track records | Already implemented: `sector_sum_ok` check in `persist_lap`. |
| Invalid flag from game | `current_lap_invalid == true` in `TelemetryFrame` (F1 25 sets this on track cuts) | Set `valid = false` on the `LapData` before sending `LapCompleted` to server | `TelemetryFrame.current_lap_invalid` field already exists. Needs wiring in F1 and AC sim adapters. |

### Differentiators

| Fix Pattern | Detection Trigger | Action | Notes |
|-------------|-------------------|--------|-------|
| Per-track minimum lap time | Lookup minimum expected lap time from track catalog (e.g., Monza ~80s, Nordschleife ~6min) | Reject or flag any lap below track-specific minimum as `suspect` | Requires a `min_lap_ms: Option<u32>` field in the track catalog. More precise than global 20s minimum. |
| Statistical outlier detection | Lap time > 3x the driver session average on the same track | Flag as `suspect` with reason `outlier` | Catches spun/off-track laps submitted without the invalid flag. Uses session lap history already collected. |
| Hotlap vs Practice classification | `session_type` from `SessionInfo` determines the bucket | Store laps in correct bucket (`hotlap_bests` vs `practice_bests`) when session type is known | `SessionType` enum has `Hotlap` and `Practice` variants. `LapData.session_id` links back to `SessionInfo.session_type`. |

### Anti-Features

| Anti-Feature | Why Problematic | Do This Instead |
|--------------|-----------------|-----------------|
| Rejecting laps based on speed trace analysis (telemetry comparison) | Requires complete telemetry for every lap stored; telemetry gaps make this unreliable | Use time-based rules (sector sum, per-track minimum, outlier) which work from lap data alone |
| Silently dropping suspect laps | Staff cannot review or override decisions they cannot see | Always store suspect laps with `suspect = true`; never hard-delete; expose `suspect` filter in admin dashboard |
| Retroactively invalidating posted track records | If a record is found suspect later, retroactive change causes confusion and complaints | Mark suspect, alert staff, let staff make the call — bot is advisory on existing records, authoritative on new inserts only |

---

## Feature Dependencies

```
[Class 1: Pod Crash]
    requires  game_process.is_running() + last_exit_code (EXISTS)
    requires  DrivingDetector.last_udp_packet (EXISTS)
    enhances  [Class 6: Telemetry Gaps] (UDP silence = shared signal)

[Class 2: Billing Edges]
    requires  BillingSessionStatus FSM in racecontrol (EXISTS)
    requires  GameState + DrivingState per pod (EXISTS)
    depends-on [Class 1] crash detection (need to know game exited)

[Class 3: Network Repair]
    requires  HeartbeatEvent::CoreDead from udp_heartbeat.rs (EXISTS)
    requires  EscalatingBackoff from rc-common (EXISTS)
    enhances  [Class 2] (WS drop causes billing tick gap)

[Class 4: USB Hardware]
    requires  DrivingDetector.is_hid_connected() (EXISTS)
    requires  FfbController::zero_force() (EXISTS)
    must-precede [Class 1] game kill (zero FFB before killing game)

[Class 5: Game Launch]
    requires  LaunchDiagnostics struct (EXISTS)
    requires  DIALOG_PROCESSES list in ac_launcher.rs (EXISTS)
    requires  content_scanner.rs for pre-launch validation (EXISTS)

[Class 6: Telemetry Gaps]
    requires  AcStatus shared memory integration (EXISTS via GameStatusUpdate)
    requires  sim adapter parse_frame() returning Option (EXISTS)
    feeds     [Class 9: Lap Filtering] (gap during lap = suspect lap)

[Class 7: Multiplayer]
    requires  GameStatusUpdate per pod (EXISTS)
    requires  ac_server.rs reachability check (PARTIAL — module exists, check not wired)
    requires  [Class 3] network repair working first

[Class 8: Kiosk PIN]
    requires  PinEntered AgentMessage (EXISTS)
    requires  LockScreenState with pin_error field (EXISTS)
    requires  ShowPinLockScreen / ShowQrLockScreen messages (EXISTS)

[Class 9: Lap Filtering]
    requires  LapData.valid + suspect fields (EXISTS)
    requires  persist_lap() sanity checks (PARTIAL — 20s global minimum, sector sum done)
    requires  TelemetryFrame.current_lap_invalid (EXISTS, needs wiring)
```

### Dependency Notes

- **Class 4 must precede Class 1 in session teardown:** Zero FFB before killing game process — safety requirement.
- **Class 6 feeds Class 9:** A telemetry gap during a lap means sectors are incomplete; the lap should be flagged `suspect` automatically.
- **Class 2 depends on Class 1:** Billing edge cleanup requires knowing the game has exited. Do not close billing until crash/exit is confirmed.
- **Class 7 depends on Class 3:** Multiplayer session management assumes the WS connection to racecontrol is healthy. Fix network before adding multiplayer guards.

---

## Implementation Table

| Failure Class | Fix Pattern | Complexity | Depends On (existing) | New State Needed |
|---------------|-------------|------------|----------------------|-----------------|
| 1 — Pod Crash | UDP silence timeout | LOW | `last_udp_packet`, `game_state`, `AcStatus` | Silence timer per pod |
| 1 — Pod Crash | Orphaned process kill + relaunch | LOW | `fix_kill_stale_game()` (exists) | Retry counter per billing session |
| 1 — Pod Crash | WerFault dismiss (deterministic path) | LOW | `fix_kill_error_dialogs()` (exists) | Wire into crash path, not just AI path |
| 1 — Pod Crash | rc-agent liveness self-check | LOW | rc-watchdog PID monitoring (exists) | Internal channel ping every 30s |
| 1 — Pod Crash | Crash pattern classification | LOW | `last_exit_code` (exists) | Static exit-code lookup table |
| 1 — Pod Crash | Freeze vs crash discrimination | LOW | `is_process_alive()` + UDP silence (exists) | Combined condition check |
| 2 — Billing | Stuck session cleanup | MEDIUM | `BillingSessionStatus`, `game_state` | Timer: game-gone + billing-active duration |
| 2 — Billing | Idle drift guard | LOW | `DrivingState`, `drive_time_ms` | Flag at session-end: never-active check |
| 2 — Billing | Cloud sync failure recovery | LOW | `cloud_sync.rs` retry loop (exists) | Failure counter (consecutive 30s cycles) |
| 2 — Billing | Billing tick gap detection | LOW | `BillingTick` messages (exists) | Last-tick timestamp per pod |
| 2 — Billing | Orphaned session on restart | MEDIUM | `StartupReport` (exists) | Carry `billing_session_id` in startup report |
| 3 — Network | WS drop repair | LOW | `HeartbeatEvent::CoreDead` (exists) | Verify main.rs wiring is complete |
| 3 — Network | HTTP endpoint retry | LOW | `reqwest`, `EscalatingBackoff` (exists) | Apply existing backoff to rc-agent HTTP calls |
| 3 — Network | IP drift fallback | LOW | Config (exists) | Fallback IP list in toml config |
| 3 — Network | WS reconnect jitter | LOW | Pod number from config (exists) | Jitter formula: `base + pod_num * 500ms` |
| 4 — USB | USB disconnect alert | LOW | `DrivingDetector.is_hid_connected()` (exists) | Alert path on mid-session disconnect flip |
| 4 — USB | FFB zero on session end | LOW | `FfbController::zero_force()` (exists) | Wire into session-end handler |
| 4 — USB | FFB zero on game crash | LOW | `FfbController::zero_force()` (exists) | Wire into `GameCrashed` handler |
| 4 — USB | USB re-enumeration probe | MEDIUM | `hidapi::HidApi` (exists) | Re-scan loop with 10s delay |
| 4 — USB | Consecutive disconnect counting | LOW | USB disconnect alert (new above) | `disconnect_count` per billing session |
| 5 — Launch | Launch timeout detection | LOW | `launched_at` in `GameLaunchInfo` (exists) | 90s timeout check in game monitor loop |
| 5 — Launch | CM hang kill + fallback | LOW | `LaunchDiagnostics.cm_attempted` (exists) | CM process name scan + taskkill |
| 5 — Launch | Immediate exit detection | LOW | `last_exit_code` (exists) | 5s post-spawn exit check |
| 5 — Launch | Pre-launch dialog clearance | LOW | `DIALOG_PROCESSES` list in ac_launcher.rs (exists) | Move dialog kill before launch, not only after crash |
| 5 — Launch | CM log extraction | LOW | `LaunchDiagnostics.cm_log_errors` field (exists) | File read from known CM log path |
| 5 — Launch | race.ini pre-launch validation | MEDIUM | `content_scanner.rs` (exists) | Cross-reference track/car paths before write |
| 6 — Telemetry | Game-state-aware silence | LOW | `AcStatus` (exists) | Silence timer gated on `AcStatus::Live` |
| 6 — Telemetry | Partial data flagging | LOW | `persist_lap` suspect logic (exists) | Partial-frame detection in sim adapters |
| 6 — Telemetry | Corrupted packet counter | LOW | `parse_frame()` returns `Option` (exists) | Drop counter + rate check per pod |
| 6 — Telemetry | Port bind failure reporting | LOW | UDP socket bind (exists) | Capture error, surface via `StartupReport` |
| 7 — Multiplayer | AC server disconnect (all-pods) | LOW | `GameStatusUpdate` per pod (exists) | Cross-pod status comparison on server side |
| 7 — Multiplayer | Server reachability pre-check | LOW | `ac_server.rs` (exists) | HTTP GET before multiplayer launch |
| 7 — Multiplayer | Single pod desync detection | LOW | `AcStatus` per pod (exists) | Divergence: one pod Off, others Live |
| 7 — Multiplayer | Desync timeline logging | LOW | `GameStatusUpdate` timestamps (exists) | Timestamp comparison — no new state |
| 8 — PIN | PIN fail count tracking | LOW | `PinEntered` (exists), `pin_error` field (exists) | Failure counter per pod in agent state |
| 8 — PIN | Session token expiry detection | LOW | `ShowPinLockScreen.allocated_seconds` (exists) | Age check from display time |
| 8 — PIN | PIN lockout auto-reset | LOW | Failure counter (above) | Timer reset after 5 min |
| 8 — PIN | QR soft alert (120s) | LOW | `ShowQrLockScreen` timestamp | Duration check in lock screen state |
| 9 — Laps | Zero-time rejection | LOW | `persist_lap` check (EXISTS, done) | None |
| 9 — Laps | Sub-minimum rejection (global) | LOW | `sanity_ok` in `persist_lap` (EXISTS, done) | None |
| 9 — Laps | Sector sum mismatch | LOW | `sector_sum_ok` in `persist_lap` (EXISTS, done) | None |
| 9 — Laps | Invalid flag wiring (F1 + AC) | LOW | `TelemetryFrame.current_lap_invalid` (EXISTS) | Wire in sim adapters to set `valid = false` |
| 9 — Laps | Per-track minimum lap time | MEDIUM | Track catalog (exists) | `min_lap_ms` field in catalog entries |
| 9 — Laps | Statistical outlier detection | MEDIUM | Session lap history in DB | 3x session-average query + comparison |
| 9 — Laps | Hotlap vs Practice classification | LOW | `SessionType` enum (EXISTS), `session_id` link (EXISTS) | Route `persist_lap` based on `session_type` |

---

## MVP Definition

### Must Have for v5.0 Bot to Be Useful

These are the patterns staff must perform manually today. The bot eliminates them.

- [ ] **UDP silence timeout -> crash detection** — Without this, staff do not know a game froze until a customer complains.
- [ ] **WerFault dismiss (deterministic path)** — Currently wired only through AI path; make it fire on every crash.
- [ ] **FFB zero on session end / game crash** — Safety requirement. Physical hazard if skipped.
- [ ] **Stuck session cleanup** — Billing staying active after game exit charges customers unfairly.
- [ ] **Launch timeout + CM hang kill** — Staff currently walk to pod when launch hangs. Bot eliminates most of these.
- [ ] **PIN fail count + lockout message** — Customers stranded at lock screen without this.
- [ ] **Invalid flag from game wiring** (F1 + AC adapters) — Without this, invalid laps (cuts, crashes) enter the leaderboard.
- [ ] **Game-state-aware telemetry silence alerting** — Must use `AcStatus::Live` gate to avoid noise alerts from in-menu silence.

### Add After Validation

- [ ] **Per-track minimum lap time** — After catalog has `min_lap_ms` data populated for each track.
- [ ] **Statistical outlier detection** — After enough lap history accumulated per track (needs data volume).
- [ ] **WS reconnect jitter** — Add when pod count grows or reconnect storms observed in practice.
- [ ] **CM log extraction** — After CM launch failures are logged and the log path confirmed on all pods.
- [ ] **AC server reachability pre-check** — After multiplayer sessions become regular enough to justify the check overhead.

### Future Consideration

- [ ] **USB re-enumeration auto-reconnect** — Needs field testing; USB re-enum behavior varies by Windows build.
- [ ] **Multiplayer auto-rejoin** — Only buildable if AC server session token is accessible to rc-agent.
- [ ] **Billing tick gap local pause** — Edge case; validate real impact before building.

---

## Feature Prioritization Matrix

| Fix Pattern | Staff Relief Value | Implementation Cost | Priority |
|-------------|-------------------|---------------------|----------|
| UDP silence crash detection | HIGH | LOW | P1 |
| WerFault dismiss (deterministic path) | HIGH | LOW | P1 |
| FFB zero on session end and crash | HIGH (safety) | LOW | P1 |
| Stuck session cleanup | HIGH | MEDIUM | P1 |
| Launch timeout + CM hang kill | HIGH | LOW | P1 |
| PIN fail count + lockout | HIGH | LOW | P1 |
| Invalid flag wiring (F1 + AC) | HIGH | LOW | P1 |
| Game-state-aware telemetry silence | HIGH | LOW | P1 |
| USB disconnect alert | MEDIUM | LOW | P2 |
| Cloud sync failure recovery | MEDIUM | LOW | P2 |
| IP drift fallback config | MEDIUM | LOW | P2 |
| Pre-launch dialog clearance | MEDIUM | LOW | P2 |
| Crash pattern classification | LOW | LOW | P2 |
| Freeze vs crash discrimination | MEDIUM | LOW | P2 |
| Per-track minimum lap time | MEDIUM | MEDIUM | P2 |
| CM log extraction | LOW | LOW | P2 |
| race.ini pre-launch validation | MEDIUM | MEDIUM | P2 |
| AC server reachability pre-check | MEDIUM | LOW | P2 |
| Single pod desync detection | MEDIUM | LOW | P2 |
| Hotlap vs Practice classification | MEDIUM | LOW | P2 |
| Statistical outlier detection | LOW | MEDIUM | P3 |
| USB re-enumeration auto-reconnect | LOW | MEDIUM | P3 |
| WS reconnect storm prevention | LOW | LOW | P3 |
| Billing tick gap local pause | LOW | LOW | P3 |
| QR soft alert (120s) | LOW | LOW | P3 |
| PIN lockout auto-reset (5min) | LOW | LOW | P3 |

---

## Sources

- Direct codebase analysis: `ai_debugger.rs` (try_auto_fix, PodStateSnapshot, DebugMemory), `game_process.rs` (orphan cleanup, PID tracking, is_process_alive), `driving_detector.rs` (DetectorSignal, HID/UDP state), `lap_tracker.rs` (persist_lap, suspect logic), `ac_launcher.rs` (DIALOG_PROCESSES, DifficultyTier), `ffb_controller.rs` (CMD_ESTOP, zero_force), `lock_screen.rs` (LockScreenState, pin_error), `udp_heartbeat.rs` (HeartbeatEvent, sequence tracking), `types.rs` (BillingSessionStatus, GameState, AcStatus, LapData), `protocol.rs` (GameCrashed, FfbZeroed, PinEntered, StartupReport)
- PROJECT.md v5.0 requirements section (2026-03-16)
- ARCHITECTURE.md codebase map
- MEMORY.md: venue context (8 pods, Conspit Ares VID:0x1209/PID:0xFFB0, AC + F1 25, server DHCP history)

---
*Feature research for: RC Bot Expansion (v5.0), deterministic auto-fix patterns per failure class*
*Researched: 2026-03-16*
