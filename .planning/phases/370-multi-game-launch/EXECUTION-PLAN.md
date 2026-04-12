# Phase 370 — Multi-Game Launch (F1 25, iRacing, LMU)

**Status:** Planning  
**Depends on:** Phase 369 (AC launcher rewrite using launch contract)  
**Executor:** James (on-site, Pod 8 canary)  
**Estimated LOC:** ~1,200 total (~400 per sim)

---

## 1. SimLauncher Trait Definition

Phase 369 refactors the AC launcher behind a new `SimLauncher` trait. Phase 370 adds
implementations for F1 25, iRacing, and LMU. The trait lives in rc-agent (not rc-common)
because it deals with process spawning, which is agent-side only.

```rust
// crates/rc-agent/src/sim_launcher.rs (new file, created by Phase 369)

use anyhow::Result;
use rc_common::launch_contract::{LaunchRequest, LaunchResult, LaunchOutcome, GameConfig};

/// Every sim implements this trait to handle the launch→spawn→detect lifecycle.
/// The trait is ONLY about launching. Telemetry reading is handled by SimAdapter (sims/mod.rs).
///
/// Design principle (from VMS analysis): launcher EXITS after spawning.
/// No monitoring, no healing in the launch path. That is SimAdapter + event_loop's job.
pub trait SimLauncher: Send + Sync {
    /// Human-readable name for logging.
    fn name(&self) -> &'static str;

    /// Pre-launch validation specific to this sim.
    /// Examples: check game is installed, check required plugins exist,
    /// check subscription active (iRacing), check shared memory plugin loaded (LMU).
    /// Returns Ok(()) to proceed, Err(reason) to abort with a user-visible error.
    fn pre_validate(&self, request: &LaunchRequest) -> Result<()>;

    /// Write any config files the game needs before spawning.
    /// AC: writes race.ini, assists.ini. F1 25: nothing (no pre-config).
    /// iRacing: nothing (session configured via iRacing UI/website).
    /// LMU: optionally write player.json for name injection.
    fn write_config(&self, request: &LaunchRequest) -> Result<()>;

    /// Spawn the game process. Returns immediately after CreateProcess succeeds.
    /// The PID may be None for Steam URL launches (game appears later).
    /// MUST use DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP on Windows.
    /// MUST NOT block waiting for the game to load.
    fn spawn(&self, request: &LaunchRequest) -> Result<LaunchResult>;

    /// Process name(s) to scan for when detecting if the game is running.
    /// Used by find_game_pid() when spawn() returned pid=None (Steam launches).
    fn process_names(&self) -> &[&'static str];

    /// Window title substring to identify the game window (for focus detection).
    /// Used by off-track blanking and overlay positioning.
    fn window_title_hint(&self) -> &'static str;
}
```

**Key design decisions:**

1. `SimLauncher` is separate from `SimAdapter`. Launcher = spawn process. Adapter = read telemetry.
   They have different lifecycles: launcher runs once, adapter runs continuously.

2. `pre_validate` replaces the per-sim checks scattered in ws_handler.rs (GAME-01 through GAME-07).
   Each sim's validation logic moves INTO the sim's launcher implementation.

3. `write_config` is the AC-specific step that other sims mostly skip.
   F1 25 and iRacing have no config files we control. LMU has optional player.json.

4. `spawn` returns `LaunchResult` from the contract — `Spawned { pid }` or `Failed { reason }`.

---

## 2. Per-Sim Implementation Plan

### 2.1 F1 25 (`F125Launcher`)

**File:** `crates/rc-agent/src/launchers/f1_25.rs` (~350 lines)

#### How to launch the exe

- **Process:** `F1_25.exe` (standalone EA launcher, no Steam DRM on our install)
- **Working directory:** The F1 25 install directory (from `config.games.f1_25.exe_path` parent)
- **Spawn flags:** `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` (same as AC)
- **Steam:** `GameType::F125.steam_app_id()` returns `None` — direct exe launch, not Steam URL.
  If a venue changes to Steam, the `GameExeConfig.use_steam` flag handles it via `GameProcess::launch`.
- **Args:** None needed for basic launch. F1 25 uses its own in-game menu for session selection.
  The `LaunchRequest.config.F125 { track }` field is informational (for telemetry attribution),
  not written to any config file.

#### How to detect game is running

- **Process names:** `["F1_25.exe", "F1_2025.exe"]` (already in `game_process::process_names`)
- **Window title:** `"F1"` (F1 25 uses "F1 25" as window title in windowed mode,
  and the DX11 renderer window in fullscreen — title varies by build)
- **Liveness signal:** The F125Adapter sends `DetectorSignal::UdpReachable` as soon as ANY
  valid F1 25 UDP packet is received on port 20777 (ADAPTER-SWAP-03). This fires within
  30-60 seconds of game reaching the main menu.

#### How to read telemetry

- **Method:** UDP broadcast on port 20777 (already implemented in `sims/f1_25.rs`)
- **Adapter:** `F125Adapter` binds `127.0.0.1:20777` with `SO_REUSEADDR` (coexists with ConspitLink)
- **Packets parsed:** Session (1), LapData (2), Participants (4), CarTelemetry (6), CarStatus (7)
- **No changes needed** — telemetry adapter is complete (1,041 lines, fully tested)

#### Known quirks (from existing code + VMS analysis)

1. **Long load time:** EA SPORTS splash -> main menu -> session select -> loading -> grid -> countdown.
   Can take 3+ minutes from exe spawn to first telemetry packet with speed > 0.
   `UdpReachable` (menu packets) fires at ~60s; `UdpActive` (on-track, speed > 0) at ~180-300s.
   The launch verifier must use `UdpReachable` for launch confirmation, NOT `UdpActive`.
   This is already fixed (LAUNCH-PLAYABLE-SPLIT, 2026-04-12).

2. **ConspitLink port conflict:** ConspitLink2.0 binds `127.0.0.1:20777` for FFB.
   Both must coexist via `SO_REUSEADDR`. Already handled in F125Adapter.connect().

3. **No config file injection:** Unlike AC, F1 25 has no `race.ini` equivalent we can write.
   Track/car selection happens in-game. The customer picks their session after launch.
   `write_config()` is a no-op.

4. **Packet format version:** Header byte 0-1 must be `2025` (u16 LE). If EA releases F1 26,
   the format version will change and packets will be silently dropped until adapter is updated.

#### pre_validate checks

- Game exe exists at configured path
- F1 25 is NOT already running (orphan check — handled by generic pre_launch_checks)
- No additional sim-specific validation needed (no subscription, no plugins)

#### Estimated: ~350 lines

- `pre_validate`: ~30 lines (exe existence check)
- `write_config`: ~5 lines (no-op)
- `spawn`: ~80 lines (direct exe launch with DETACHED_PROCESS)
- `process_names` + `window_title_hint`: ~10 lines
- Helpers, imports, tests: ~225 lines

---

### 2.2 iRacing (`IRacingLauncher`)

**File:** `crates/rc-agent/src/launchers/iracing.rs` (~400 lines)

#### How to launch the exe

- **Process:** `iRacingSim64DX11.exe` (the actual sim binary)
- **BUT:** iRacing uses a multi-process launch chain:
  1. `iRacingUI.exe` (launcher/updater UI) — may need to be launched first
  2. `iRacingService64.exe` (background service) — starts automatically
  3. `iRacingSim64DX11.exe` (the actual sim) — spawned by the service when user joins a session
- **Our approach:** Launch `iRacingUI.exe` (the launcher) directly. The customer then selects
  a session (practice, race, etc.) through iRacing's own UI. The sim binary starts automatically.
  Alternatively, if `iRacingSim64DX11.exe` is configured as exe_path, spawn it directly
  (for cases where the customer has already configured a session via the website).
- **Working directory:** iRacing install dir (typically `C:\Program Files (x86)\iRacing`)
- **Spawn flags:** `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`
- **VMS pattern (from EXAMPLE iracing.ini):** VMS monitors `iRacingSim64DX11.exe` and uses
  window title `"iRacing.com Simulator"` for automation. We don't need automation (no mouse
  clicks for session management), just process detection.

#### How to detect game is running

- **Process names:** `["iRacingSim64DX11.exe", "iRacingService.exe", "iRacingService64.exe",
  "iRacingLauncher64.exe", "iRacingUI.exe"]` (already in `game_process::process_names`)
- **Primary detection:** `iRacingSim64DX11.exe` running = game is active
- **Window title:** `"iRacing.com Simulator"` (from VMS ini)
- **Liveness signal:** iRacing adapter connects via shared memory `Local\IRSDKMemMapFileName`.
  When the adapter successfully reads the header with status bit 1 set, the sim is in-session.
  The adapter's `read_is_on_track()` returns true when the player is on track.
  **This maps to the launch verifier's PlayableSignal for iRacing.**

#### How to read telemetry

- **Method:** Windows shared memory (`Local\IRSDKMemMapFileName`)
- **Adapter:** `IracingAdapter` (already implemented in `sims/iracing.rs`, 988 lines)
- **Variable lookup:** Dynamic via varHeader array scan (not fixed offsets)
- **YAML session info:** Track name, car name, session type parsed from iRacing's embedded YAML
- **No changes needed** — telemetry adapter is complete

#### Known quirks (from existing code + VMS configs + iracing_checks.rs)

1. **Subscription wall:** iRacing requires an active subscription. If expired, the launcher
   shows a subscription dialog instead of loading the sim. `iracing_checks::check_iracing_ready()`
   already validates this (GAME-05 in ws_handler.rs). **Move this into `pre_validate()`.**

2. **Shared memory must be enabled:** `Documents\iRacing\app.ini` must contain `irsdkEnableMem=1`.
   `iracing_checks::check_iracing_shm_enabled()` already validates this. **Move into `pre_validate()`.**

3. **Multi-process architecture:** The actual sim (`iRacingSim64DX11.exe`) is NOT what we spawn.
   We spawn the launcher UI, and the sim binary appears later when the user joins a session.
   The launch verifier must wait for `iRacingSim64DX11.exe` to appear, not just the launcher.
   `find_game_pid(SimType::IRacing)` already scans for this.

4. **Session-managed externally:** iRacing sessions are configured on the iRacing website,
   not via config files on disk. The `GameConfig::IRacing {}` struct is intentionally empty.
   `write_config()` is a no-op.

5. **Double-buffer tick-lock:** iRacing uses 4 alternating buffers for telemetry. The adapter
   already handles torn reads via `read_latest_row_offset()` with 3 retries.

6. **Stale SHM after exit:** When iRacing exits, the shared memory mapping is destroyed.
   Reading from stale pointers causes access violation. `verify_shm_alive()` already guards
   against this by probing `OpenFileMappingW` before each read.

7. **num_vars bounds guard:** Corrupted SHM headers can have absurd `num_vars` values.
   Already guarded with `num_vars > 4096` check (commit e388b5af).

#### pre_validate checks

- Game exe exists at configured path
- `irsdkEnableMem=1` in `Documents\iRacing\app.ini` (moved from iracing_checks.rs)
- iRacing subscription active (moved from GAME-05 block in ws_handler.rs)
- iRacing NOT already running (orphan check)

#### Estimated: ~400 lines

- `pre_validate`: ~100 lines (exe check, SHM enabled check, subscription check)
- `write_config`: ~5 lines (no-op)
- `spawn`: ~80 lines (launch iRacingUI.exe or direct sim exe)
- `process_names` + `window_title_hint`: ~10 lines
- Helpers (subscription check, SHM enabled check), imports, tests: ~205 lines

---

### 2.3 Le Mans Ultimate (`LmuLauncher`)

**File:** `crates/rc-agent/src/launchers/lmu.rs` (~400 lines)

#### How to launch the exe

- **Process:** `Le Mans Ultimate.exe` (note: space in filename)
- **Working directory:** LMU install dir (from `config.games.le_mans_ultimate.exe_path` parent)
- **Spawn flags:** `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`
- **Steam:** LMU may be a Steam game (app ID varies). If `use_steam` is set, launch via
  `steam://rungameid/{id}`. Otherwise, direct exe launch.
- **VMS pattern (from EXAMPLE lmu.ini):** VMS uses `Le Mans Ultimate.exe` as exe name.
  VMS supports up to 6 dedicated server instances (for multiplayer). Our venue uses
  single-player or peer multiplayer, so server config is not needed.

#### How to detect game is running

- **Process names:** `["LMU.exe", "Le Mans Ultimate.exe"]` (already in `game_process::process_names`)
- **Window title:** `"Le Mans Ultimate"` (standard window title)
- **Liveness signal:** LMU uses the rFactor 2 shared memory plugin (`rF2SharedMemoryMapPlugin`).
  When `$rFactor2SMMP_Scoring$` shared memory exists AND version pair is consistent AND
  `mGamePhase >= 4` (countdown or later), the game is in-session.
  The LmuAdapter's `read_is_on_track_from_shm()` returns true when player is on track.

#### How to read telemetry

- **Method:** Windows shared memory via rF2SharedMemoryMapPlugin
  - `$rFactor2SMMP_Scoring$` — 5 Hz, lap times, sector splits, session state
  - `$rFactor2SMMP_Telemetry$` — 50 Hz, vehicle inputs, speed, RPM
- **Adapter:** `LmuAdapter` (already implemented in `sims/lmu.rs`, 1,033 lines)
- **Struct layout:** Fixed C struct offsets from rF2Data.cs (not dynamic like iRacing)
- **No changes needed** — telemetry adapter is complete

#### Known quirks (from existing code + VMS configs)

1. **rF2SharedMemoryMapPlugin required:** LMU must have the rFactor 2 shared memory plugin
   installed and loaded. Without it, shared memory doesn't exist and telemetry is blind.
   `pre_validate()` should check for the plugin DLL in the LMU install directory.
   Plugin location: `<LMU_DIR>\Bin64\Plugins\rFactor2SharedMemoryMapPlugin64.dll`

2. **Spaces in exe name:** `"Le Mans Ultimate.exe"` contains spaces. The Command::new()
   call handles this correctly, but all string comparisons in process scanning must
   use the exact name with spaces. Already handled in `process_names()`.

3. **Two SHM buffers:** Unlike AC (3 buffers) or iRacing (1 buffer), LMU uses 2 separate
   memory-mapped files. Both must be opened successfully for full telemetry.
   Scoring alone provides lap times; telemetry provides speed/throttle/brake.

4. **Version pair torn-read guard:** rF2 increments `mVersionUpdateBegin` before writing
   and `mVersionUpdateEnd` after. If they differ, the read is torn (mid-write).
   Already handled with 3-retry spin loop in `read_telemetry()`.

5. **Session type mapping:** LMU's `mSession` values differ from AC's session types.
   The adapter already maps these correctly.

6. **Sector times are cumulative:** `mLastSector2` is S1+S2 (not S2 alone).
   `sector_times_ms()` already derives individual sector times correctly.

7. **VMS multiplayer support:** VMS ini shows up to 6 dedicated server instances.
   Our venue currently uses single-player or peer LAN multiplayer. If dedicated server
   support is needed later, it would be a separate phase (server config management).

8. **Stale SHM after exit:** Same pattern as iRacing. `verify_shm_alive_lmu()` probes
   `OpenFileMappingW` for `$rFactor2SMMP_Scoring$` before each read.

#### pre_validate checks

- Game exe exists at configured path
- rF2SharedMemoryMapPlugin DLL exists in `<LMU_DIR>\Bin64\Plugins\`
- LMU NOT already running (orphan check)

#### Optional: write_config for player name

LMU reads player name from a JSON config file. If `LaunchRequest.driver_name` is provided,
write it to `<LMU_DIR>\UserData\player\player.JSON` before launch. This is a nice-to-have
(the customer can set their name in-game too). Implementation:

```rust
fn write_config(&self, request: &LaunchRequest) -> Result<()> {
    // Only write if driver_name is non-empty
    if request.driver_name.is_empty() { return Ok(()); }
    let player_json_path = self.lmu_dir.join("UserData").join("player").join("player.JSON");
    if player_json_path.exists() {
        // Read existing, update "Driver Name" field, write back
        // Non-fatal: if file doesn't exist or parse fails, skip silently
    }
    Ok(())
}
```

#### Estimated: ~400 lines

- `pre_validate`: ~80 lines (exe check, plugin DLL check)
- `write_config`: ~60 lines (player name injection, with existing file read/modify/write)
- `spawn`: ~80 lines (direct exe or Steam URL launch)
- `process_names` + `window_title_hint`: ~10 lines
- Helpers (plugin detection), imports, tests: ~170 lines

---

## 3. File-by-File Plan

### New Files

| File | Purpose | Est. Lines |
|------|---------|-----------|
| `crates/rc-agent/src/launchers/mod.rs` | Module declaration + `build_launcher()` factory | ~60 |
| `crates/rc-agent/src/launchers/f1_25.rs` | F125Launcher impl | ~350 |
| `crates/rc-agent/src/launchers/iracing.rs` | IRacingLauncher impl | ~400 |
| `crates/rc-agent/src/launchers/lmu.rs` | LmuLauncher impl | ~400 |

**Note:** `sim_launcher.rs` (trait definition) and `launchers/assetto_corsa.rs` are created
by Phase 369. Phase 370 only adds the three non-AC launchers.

### Modified Files

| File | Change | Est. Delta |
|------|--------|-----------|
| `crates/rc-agent/src/launchers/mod.rs` | Add `pub mod f1_25; pub mod iracing; pub mod lmu;` + factory entries | +15 lines |
| `crates/rc-agent/src/ws_handler.rs` | Replace per-sim `if/else` blocks in LaunchGame with `build_launcher().spawn()` dispatch | -200 / +50 lines (net -150) |
| `crates/rc-agent/src/main.rs` | Add `mod launchers;` declaration | +1 line |
| `crates/rc-agent/src/iracing_checks.rs` | Move `check_iracing_ready()` and `check_iracing_shm_enabled()` to `launchers/iracing.rs::pre_validate()` (keep as pub re-exports for backward compat) | ~10 lines changed |
| `crates/rc-agent/src/steam_checks.rs` | Move GAME-01 / GAME-06 logic into per-sim `pre_validate()` (AC, F1 25 keep Steam checks; iRacing/LMU have their own) | ~20 lines changed |
| `crates/rc-agent/src/game_process.rs` | No changes — `GameProcess::launch()` continues to handle direct/Steam exe spawning. The new `SimLauncher.spawn()` calls into it. | 0 |

### Total estimate

- **New code:** ~1,210 lines across 4 new files
- **Net reduction in ws_handler.rs:** ~150 lines removed (per-sim blocks consolidated)
- **Net delta:** ~1,060 lines added

---

## 4. Test Plan

### 4.1 Unit Tests (run on build machine — `cargo test`)

Each launcher file includes unit tests:

1. **`test_f125_pre_validate_missing_exe`** — pre_validate fails when exe_path doesn't exist
2. **`test_f125_process_names_match_contract`** — process_names() matches GameType::F125.exe_name()
3. **`test_iracing_pre_validate_shm_disabled`** — pre_validate fails when app.ini lacks irsdkEnableMem=1
4. **`test_iracing_pre_validate_shm_enabled`** — pre_validate passes with correct app.ini
5. **`test_lmu_pre_validate_missing_plugin`** — pre_validate fails when rF2SharedMemoryMapPlugin DLL is missing
6. **`test_lmu_write_config_player_name`** — write_config injects driver name into player.JSON
7. **`test_lmu_write_config_empty_name_noop`** — write_config is a no-op for empty driver_name
8. **`test_build_launcher_returns_correct_type`** — factory returns correct launcher for each GameType

Run: `cargo test -p rc-agent-crate --lib launchers`

### 4.2 Pod 8 Canary — Manual Verification (James at venue)

**Pre-requisites:**
- Pod 8 has all three games installed (F1 25, iRacing with active sub, LMU with rF2 plugin)
- Pod 8 TOML has correct `exe_path` for all three games under `[games.*]`
- Pod 8 running the new rc-agent binary

**Test sequence (all via kiosk or direct WS command):**

#### F1 25 Launch Test

1. Send `LaunchGame { sim_type: F125, launch_args: null }` to Pod 8
2. **Verify:** `launch-breadcrumb.txt` shows "LaunchGame received: sim=F125"
3. **Verify:** `F1_25.exe` appears in `tasklist` within 10 seconds
4. **Verify:** GameStateUpdate with `game_state: Launching` received by server
5. **Verify:** F125Adapter receives UDP packet (check `sim-f1` tracing logs for "F1 25 adapter listening")
6. **Verify:** After reaching main menu (~60s), `UdpReachable` signal fires (check event_loop logs)
7. **Verify:** After starting a race and reaching speed > 0, `UdpActive` signal fires
8. **Verify:** Telemetry frames appear in server's WS stream (speed, throttle, RPM)
9. **Verify:** Lap completion fires when crossing start/finish
10. Stop game: Send `StopGame` command. Verify `F1_25.exe` killed. Verify lock screen returns.

#### iRacing Launch Test

1. Ensure iRacing subscription is active and `irsdkEnableMem=1` in app.ini
2. Send `LaunchGame { sim_type: IRacing }` to Pod 8
3. **Verify:** `iRacingUI.exe` or `iRacingSim64DX11.exe` appears in `tasklist`
4. **Verify:** GameStateUpdate with `game_state: Launching` received
5. Join a practice session via iRacing UI
6. **Verify:** `IRSDKMemMapFileName` shared memory appears (check `sim-iracing` logs)
7. **Verify:** IracingAdapter connects and parses session YAML (track name, car name in logs)
8. **Verify:** `IsOnTrack` goes true when on track
9. **Verify:** Telemetry frames stream (speed, RPM, gear)
10. **Verify:** Lap completion fires with correct lap time and sector splits
11. Stop game. Verify all iRacing processes killed. Lock screen returns.

#### LMU Launch Test

1. Ensure rF2SharedMemoryMapPlugin64.dll is in `<LMU_DIR>\Bin64\Plugins\`
2. Send `LaunchGame { sim_type: LeMansUltimate }` to Pod 8
3. **Verify:** `Le Mans Ultimate.exe` appears in `tasklist`
4. **Verify:** GameStateUpdate with `game_state: Launching` received
5. Start a practice session in LMU
6. **Verify:** `$rFactor2SMMP_Scoring$` shared memory appears (check `sim-lmu` logs)
7. **Verify:** LmuAdapter connects to both Scoring and Telemetry SHM
8. **Verify:** Track name and car name read correctly from scoring buffer
9. **Verify:** mGamePhase reaches 5 (green flag) — launch verifier transitions to Live
10. **Verify:** Telemetry frames stream (speed, throttle, brake, RPM)
11. **Verify:** Lap completion fires with correct sector splits (S1, S2, S3 derived from cumulative)
12. Stop game. Verify `Le Mans Ultimate.exe` killed. Lock screen returns.

#### Cross-Sim Switch Test

1. Launch AC on Pod 8, verify telemetry
2. Stop AC
3. Launch F1 25 on Pod 8 (ADAPTER-SWAP: should rebuild F125Adapter)
4. Verify F1 25 telemetry works
5. Stop F1 25
6. Launch iRacing (ADAPTER-SWAP: should rebuild IracingAdapter)
7. Verify iRacing telemetry
8. Stop iRacing
9. **Verify:** No resource leaks (UDP socket not held, SHM handles released, no orphan processes)

#### Error Path Tests

1. **Missing exe:** Set f1_25.exe_path to a nonexistent path. Launch. Verify pre_validate
   returns error. Verify GameStateUpdate with `game_state: Error` and clear error message.
2. **iRacing SHM disabled:** Temporarily set `irsdkEnableMem=0` in app.ini. Launch iRacing.
   Verify pre_validate warns about SHM. (Non-blocking — game launches but telemetry won't work.)
3. **LMU missing plugin:** Temporarily rename the rF2 plugin DLL. Launch LMU. Verify pre_validate
   returns error about missing plugin.
4. **Orphan process:** Start F1 25 manually (not via rc-agent). Then send LaunchGame.
   Verify pre_launch_checks auto-cleans the orphan before launching.

### 4.3 Fleet Rollout (after Pod 8 passes all tests)

1. Deploy to Pods 1-7 via standard fleet deploy pipeline
2. Run one launch of each sim on Pod 1 (nearest to staff for visual verification)
3. Monitor fleet health for 24 hours — watch for:
   - Orphan process accumulation
   - SHM handle leaks (memory growth in rc-agent)
   - UDP port binding failures (ConspitLink conflict)
   - launch_verifier timeouts on non-AC sims

---

## 5. Dependencies

### Must be done first (blocking)

| Dependency | Phase | Status | Why |
|------------|-------|--------|-----|
| SimLauncher trait definition | 369 | In progress | Trait must exist before implementations |
| AC launcher refactored behind trait | 369 | In progress | Sets the pattern for other sims |
| `launchers/mod.rs` + `build_launcher()` factory | 369 | In progress | Factory dispatches by GameType |
| ws_handler LaunchGame refactored to use trait | 369 | In progress | Removes per-sim if/else blocks |

### Already done (no action needed)

| Item | Status | Evidence |
|------|--------|---------|
| F125Adapter telemetry | Complete | `sims/f1_25.rs` (1,041 lines), UDP port 20777 |
| IracingAdapter telemetry | Complete | `sims/iracing.rs` (988 lines), SHM `IRSDKMemMapFileName` |
| LmuAdapter telemetry | Complete | `sims/lmu.rs` (1,033 lines), rF2 SHM Scoring + Telemetry |
| ADAPTER-SWAP in ws_handler | Complete | Rebuilds adapter on sim_type mismatch (2026-04-12) |
| LaunchContract types | Complete | `launch_contract.rs` — GameType, LaunchRequest, LaunchResult |
| GameExeConfig in TOML | Complete | `config_schema.rs` — per-sim exe_path, working_dir, args |
| iRacing readiness checks | Complete | `iracing_checks.rs` — subscription + SHM enabled |
| Steam readiness checks | Complete | `steam_checks.rs` — GAME-01, GAME-06 |
| Process orphan cleanup | Complete | `game_process.rs` — pre_launch_checks auto-kills orphans |
| Safe mode for anti-cheat | Complete | `safe_mode.rs` — suppresses process guard during launch |

### Game installation on pods (James must verify)

| Game | Expected Install Path | Verification |
|------|----------------------|-------------|
| F1 25 | `C:\Program Files (x86)\Steam\steamapps\common\F1 25\` or `D:\Games\F1 25\` | `dir /b "<path>\F1_25.exe"` |
| iRacing | `C:\Program Files (x86)\iRacing\` | `dir /b "C:\Program Files (x86)\iRacing\iRacingSim64DX11.exe"` |
| LMU | `C:\Program Files (x86)\Steam\steamapps\common\Le Mans Ultimate\` or `D:\Games\LMU\` | `dir /b "<path>\Le Mans Ultimate.exe"` |

**TOML config required (Pod 8 first, then all pods):**
```toml
[games.f1_25]
exe_path = "D:\\Games\\F1 25\\F1_25.exe"
working_dir = "D:\\Games\\F1 25"

[games.iracing]
exe_path = "C:\\Program Files (x86)\\iRacing\\iRacingSim64DX11.exe"
working_dir = "C:\\Program Files (x86)\\iRacing"

[games.le_mans_ultimate]
exe_path = "D:\\Games\\Le Mans Ultimate\\Le Mans Ultimate.exe"
working_dir = "D:\\Games\\Le Mans Ultimate"
```

James: verify actual install paths on Pod 8 before writing TOML. Use `where /R D:\ F1_25.exe`
or `dir /s /b D:\*F1_25.exe` to find the real location.

---

## 6. Risk Assessment

### HIGH risk

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **F1 25 UDP port conflict with ConspitLink** | No telemetry, launch verifier timeout, game killed | Already mitigated: `SO_REUSEADDR` in F125Adapter (ADAPTER-SWAP-03). Verify on Pod 8 that both ConspitLink AND F125Adapter receive packets simultaneously. |
| **iRacing subscription expired on venue account** | Launch succeeds but sim shows subscription dialog, billing starts for non-playing customer | `pre_validate()` checks subscription status. GAME-05 already exists. Move into launcher. If check cannot determine subscription state (iRacing service not running), WARN but don't block. |
| **LMU rF2 plugin not installed on pods** | No shared memory, no telemetry, no lap times | `pre_validate()` checks for plugin DLL. Clear error message: "Install rF2SharedMemoryMapPlugin". James verifies plugin on all pods before fleet deploy. |

### MEDIUM risk

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **Games not installed on all pods** | Launch fails on pods missing the game | TOML config per-pod specifies exe_path. `pre_validate()` checks file existence. Fleet content scanner (Phase 366) detects missing games proactively. |
| **Launch verifier timeout for slow-loading sims** | Game killed after 180s despite actually loading | F1 25: `UdpReachable` fires at ~60s (before speed > 0). iRacing: SHM connect at ~30s. LMU: SHM connect at ~20s. All well within 180s default timeout. If issues arise, increase `default_launch_timeout_per_attempt` per-sim. |
| **Process name mismatch (exe renamed by update)** | Game running but not detected, stuck in Launching state | Use process_names() arrays with known variants. Add tracing for "scanning for process names: [...]". If no process found within 60s, log ERROR with suggestion to check exe name. |
| **ADAPTER-SWAP race condition** | Old adapter not fully disconnected before new adapter connects | Mitigated: `old.disconnect()` called synchronously before `build_sim_adapter()`. SHM handles dropped in `disconnect()`. UDP socket dropped in `disconnect()`. |

### LOW risk

| Risk | Impact | Mitigation |
|------|--------|-----------|
| **Stale SHM after game exit (iRacing/LMU)** | Access violation crash in rc-agent | Already mitigated: `verify_shm_alive()` / `verify_shm_alive_lmu()` probes before every read. Regression test exists (`adapter_parity_tests::test_all_shm_adapters_have_liveness_guard`). |
| **Two games launched simultaneously** | Resource conflict, crash | Already mitigated: `SEC-10 game_launch_mutex` serializes LaunchGame commands. `pre_launch_checks()` auto-kills orphan processes. |
| **Windows UAC prompt blocks game launch** | Game never starts, launch timeout | All games should be pre-authorized (no UAC). If UAC fires, it's a pod config issue, not a code issue. Document in troubleshooting. |

---

## 7. Implementation Order

Execute in this exact order:

1. **Create `launchers/mod.rs`** — module declarations + factory (if not already created by Phase 369)
2. **Implement `F125Launcher`** — simplest sim (no config files, no plugins, just exe spawn)
3. **Test F1 25 on Pod 8** — full launch/telemetry/stop cycle
4. **Implement `IRacingLauncher`** — includes pre_validate migration from iracing_checks.rs
5. **Test iRacing on Pod 8** — full launch/telemetry/stop cycle
6. **Implement `LmuLauncher`** — includes plugin check and optional player name injection
7. **Test LMU on Pod 8** — full launch/telemetry/stop cycle
8. **Cross-sim switch test on Pod 8** — AC -> F1 -> iRacing -> LMU -> AC
9. **Refactor ws_handler.rs** — replace per-sim blocks with trait dispatch (if not done in 369)
10. **Fleet deploy** — Pod 8 verified, roll to remaining 7 pods
11. **Monitor 24h** — watch for regressions

Each step is independently deployable. If step 3 fails, steps 4-11 are not blocked (they
don't depend on F1 25 working). If step 5 fails (iRacing subscription issue), LMU can
still proceed at step 6.

---

## 8. Deploy Manifest

```yaml
deploy:
  rust_binary: true          # rc-agent rebuild required
  frontend_rebuild: false    # no UI changes
  config_change: true        # pod TOML needs [games.*] exe_path entries
  db_migration: false        # no DB changes
  infrastructure: false      # no new services
  data_files: false          # no data file changes
  bat_file: false            # no bat changes
  cloud_parity: true         # cloud rc-agent binary must be rebuilt (compile-only, no runtime sims)
  targets:
    - Pod 8 (canary)
    - Pods 1-7 (after canary passes)
    - Cloud (Bono VPS — binary parity only, no game launching on cloud)
```
