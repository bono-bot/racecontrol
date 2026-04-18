# Game Launch — End-to-End Map

> **Purpose:** single-page trace of the game-launch pipeline from kiosk button to lap-recorded, with `file:line` references at every hop. Debugging entry point — follow the arrows.
>
> **Scope:** single-player Assetto Corsa (most common). Multiplayer diverges at step 7 (Content Manager URI). F1 25 / iRacing / LMU / Forza diverge at step 8 (per-game spawner).

## Pipeline (SP/AC)

### 1. Trigger — kiosk staff page
- File: [kiosk/src/app/kiosk/staff/page.tsx](../kiosk/src/app/kiosk/staff/page.tsx)
- Action: "Launch Game" button → `fetchApi("/games/launch", POST, {pod_id, sim_type, launch_args})`
- Wizard fields: `sim_type`, `track`, `car`, `session_type`, `ai_level` (u32), `ai_cars` (Vec<AiCarSlot>)
- Known contract bug: kiosk used to send `ai_difficulty` (string) / `ai_count` — silently dropped by serde. Fixed by aligning to `ai_level` / `ai_cars`.

### 2. Server route — billing + validation
- File: [crates/racecontrol/src/api/game_launch.rs:55](../crates/racecontrol/src/api/game_launch.rs#L55) — `launch_game()`
- Trial guard: AC-only for trial sessions (line 86)
- `duration_minutes` injected from active billing session's remaining time (line 93-115)
- SEC-01 launch_args validator at [api/security.rs](../crates/racecontrol/src/api/security.rs) — rejects INI injection before WS send
- Concurrent guard: HTTP 409 `game_already_active` if GameTracker not Idle

### 3. FSM transition — GameTracker
- File: [crates/racecontrol/src/game_launcher_ops.rs:29](../crates/racecontrol/src/game_launcher_ops.rs#L29) — `launch_game()`
- State machine: `Idle → Launching → Running → Stopping → Idle` (or `Error`)
- State file: [crates/racecontrol/src/game_launcher_state.rs:13](../crates/racecontrol/src/game_launcher_state.rs#L13) — `handle_game_state_update()`
- Timeout: `Launching` auto-transitions to `Error` after 60s (prevents stuck state from WS drop)

### 4. WS dispatch — server → rc-agent
- File: [crates/racecontrol/src/ws/mod.rs:260](../crates/racecontrol/src/ws/mod.rs#L260) — `dispatch_agent_message()`
- Message: `CoreToAgentMessage::LaunchGame` ([rc-common/src/protocol.rs](../crates/rc-common/src/protocol.rs))
- Dashboard broadcast: `DashboardEvent::GameLaunching` to kiosk + admin
- **Ack path**: rc-agent returns `GameStateUpdate::Launching` — server does NOT wait for this before returning HTTP 200 (known behavior; adds fire-and-forget risk if WS drops between queue and delivery)

### 5. rc-agent receive
- File: [crates/rc-agent/src/ws_handler.rs:169](../crates/rc-agent/src/ws_handler.rs#L169) — `handle_ws_message()`
- Dispatch by `sim_type` (assetto_corsa, assetto_corsa_evo, f1_25, iracing, le_mans_ultimate, forza, forza_horizon_5, assetto_corsa_rally)
- INV-10 guard: `sim_type = None` silent abort fixed in `2c27e2fc` — now emits `DashboardEvent::CommandError`

### 6. Pre-flight
- File: [crates/rc-agent/src/pre_flight.rs](../crates/rc-agent/src/pre_flight.rs) — `run_preflight_checks()`
- Checks: AC install dir exists, `acs.exe` present, plugin file at `Documents/Assetto Corsa/cfg/python.ini` has `[RACECONTROL]` section, Steam running if required, disk space OK
- Failure class: P0 Zero Laps incident — python.ini missing `[RACECONTROL]` on 7/8 pods; fix `ac0b215e`

### 7. AC-specific launcher
- File: [crates/rc-agent/src/ac_launcher.rs:437](../crates/rc-agent/src/ac_launcher.rs#L437) — `launch_ac()`
- `build_race_ini_string()` at [ac_launcher.rs:1536](../crates/rc-agent/src/ac_launcher.rs#L1536) — serializes wizard → `race.ini`
- `parse_ini()` at [ac_launcher.rs:2625](../crates/rc-agent/src/ac_launcher.rs#L2625) — validates INI round-trip
- Writes `race.ini` + `assists.ini` to `%USERPROFILE%\Documents\Assetto Corsa\cfg\`
- **SP path**: direct `acs.exe` spawn via [crates/rc-common/src/spawn_safe.rs:37](../crates/rc-common/src/spawn_safe.rs#L37) `spawn_safe_capture()` with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`
- **MP path**: delegate to [deploy/launch-ac.bat](../deploy/launch-ac.bat) which invokes Content Manager via `acmanager://race/online?ip=…&httpPort=…&password=…` URI
- **Why SP bypasses bat**: `launch-ac.bat` creates a console chain that sends `CTRL_CLOSE_EVENT` to rc-agent on AC fullscreen transition — crashed the agent with `0xC000013A`. Fix: `d616ee10`.

### 8. Process lifecycle
- File: [crates/rc-agent/src/game_process.rs](../crates/rc-agent/src/game_process.rs)
- Monitored: `acs.exe`, `AssettoCorsa.exe`, `ContentManager.exe` (MP), `F1_25.exe`, `iRacingSim64DX11.exe`, `LMU.exe`, `ForzaHorizon5.exe`
- Co-process pollution: `WerFault.exe` (crash dump), `Variable_dump.exe` (VSD Craft), `EasyAntiCheat.exe`
- Exit event: rc-agent emits `event_type=crashed` with `error_message="Process exited unexpectedly (exit code: N)"`
- **Known misclassification (Pattern H)**: clean ALT-F4 exits report as "crashed exit 0" — pending `clean_exit_heuristic` column (exit_code==0 AND seconds_since_launch>=30 AND no WerFault child)

### 9. Plugin + telemetry
- File: [plugins/assetto_corsa/RaceControl.py](../plugins/assetto_corsa/RaceControl.py) — AC plugin entry
- Telemetry writer: [plugins/assetto_corsa/rclib/telemetry.py:34](../plugins/assetto_corsa/rclib/telemetry.py#L34) — `RcTelemetryWriter`
- UDP ports per sim: 9996 (AC), 20777 (F1 25), 6789 (iRacing), 5555 (LMU), 5300 (Forza)
- SHM (AC-only): plugin writes 400 bytes shared memory — rc-agent reads for state detection
- Driving detection: [crates/rc-agent/src/driving_detector.rs:152](../crates/rc-agent/src/driving_detector.rs#L152) — 10s idle threshold stops billing timer

### 10. Lap persistence
- File: [crates/racecontrol/src/lap_tracker.rs:68](../crates/racecontrol/src/lap_tracker.rs#L68) — `persist_lap()`
- Flow: UDP packet → rc-agent `extract_driver_id()` at [api/customer_auth.rs:125](../crates/racecontrol/src/api/customer_auth.rs#L125) → WS to server → `persist_lap()` → `laps` table + leaderboard
- Cloud sync: `sync_push()` at [api/sync_cloud_push.rs:21](../crates/racecontrol/src/api/sync_cloud_push.rs#L21) replicates to Bono VPS

### 11. Billing coupling
- File: [crates/racecontrol/src/billing_session_end.rs:36](../crates/racecontrol/src/billing_session_end.rs#L36) — `end_billing_session()`
- Idle auto-end: [crates/racecontrol/src/billing_fsm.rs:131](../crates/racecontrol/src/billing_fsm.rs#L131) — `validate_transition()`
- Refund calc: F-05 bug (overwrite before read) fixed; orphan-refund trace via `wallet_transactions` history
- `billing_session_extend.rs` + `billing_session_start.rs` bracket the launch lifecycle

## Deploy artifacts (pod-local)

| File | Role |
|---|---|
| [scripts/deploy/start-rcagent.bat](../scripts/deploy/start-rcagent.bat) | HKLM Run key — boots rc-agent in Session 1 |
| [deploy/launch-ac.bat](../deploy/launch-ac.bat) | MP Content Manager launcher (SP bypasses) |
| [deploy/install-watchdog.bat](../deploy/install-watchdog.bat) | Registers rc-watchdog service |
| [deploy/pod-lockdown.ps1](../deploy/pod-lockdown.ps1) | Kiosk lockdown during active game |
| `%USERPROFILE%\Documents\Assetto Corsa\cfg\python.ini` | Plugin enablement — `[RACECONTROL]` section |
| `%USERPROFILE%\Documents\Assetto Corsa\cfg\race.ini` | Generated per-launch — car/track/AI/session |
| `C:\RacingPoint\MAINTENANCE_MODE` (sentinel) | Blocks all restarts if present |
| `C:\RacingPoint\GRACEFUL_RELAUNCH` (sentinel) | rc-agent clean self-restart marker |
| `C:\RacingPoint\rc-agent.toml` | pod config — sentry_service_key, ac_path |

## Recovery paths

- **rc-agent crash** → RCWatchdog Windows service respawns via `WTSQueryUserToken + CreateProcessAsUser` (Session 1)
- **Game stuck in Launching** → FSM 60s timeout → auto-Error → kiosk retry enabled
- **acs.exe won't start** → `ac_launcher.rs::try_auto_fix()` at [ai_debugger.rs:930](../crates/rc-agent/src/ai_debugger.rs#L930) runs MMA Tier 1 fixes
- **Pod unreachable** → rc-sentry on :8091 exec endpoint (independent binary) — can kill+restart rc-agent

## Failure taxonomy

- `INV-1` — exit code capture (`68f4d61e`)
- `INV-10` — sim_type=None abort (`2c27e2fc`)
- `BILL-14` — billing session missing sim_type
- `F-05` — wallet_debit_paise overwrite
- `DE-1` — zombie half-socket in `ws/dashboard_handler.rs` (`2c27e2fc`)
- `DE-4` — BILL-14 silent abort, no kiosk event (`2c27e2fc`)
- `ZL-1/ZL-2` — python.ini missing `[RACECONTROL]` (`ac0b215e`)
- `GL-5/8/9/10` — Steam dialog / SQL bug (`bf8a30e4`, `40968ddc`)

## Events table

| Table | Columns | Written by |
|---|---|---|
| `game_launch_events` | pod_id, sim_type, event_type, error_message, exit_code, clean_exit_heuristic (pending) | rc-agent event_loop |
| `billing_sessions` | pod_id, driver_id, status, allocated_seconds, driving_seconds, wallet_debit_paise | server billing |
| `laps` | session_id, driver_id, pod_id, lap_time_ms, sector_times | lap_tracker.rs |
| `wallet_transactions` | driver_id, type, amount_paise, billing_session_id | wallet.rs |
