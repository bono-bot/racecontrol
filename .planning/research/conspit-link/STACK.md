# Technology Stack

**Project:** Conspit Link -- Full Capability Unlock
**Researched:** 2026-03-20
**Overall confidence:** HIGH (most findings verified against local filesystem + OpenFFBoard wiki + existing rc-agent code)

## Recommended Stack

This project is NOT a new application build. It is a configuration, integration, and safety-hardening project layered onto the existing rc-agent (Rust) codebase. The "stack" is the set of protocols, file formats, and HID commands that rc-agent must understand to manage Conspit Link and the Ares wheelbases.

### Core: Conspit Link 2.0 Configuration Layer

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| Conspit Link 2.0 | v1.1.2 (installed) | Wheelbase driver, FFB engine, telemetry dashboard, game detection | Mandatory -- this IS the driver for Ares wheelbases. Cannot be replaced. |
| `.Base` preset files (JSON) | N/A | Per-game FFB tuning profiles | Conspit's native format. Plain JSON, fully readable/writable. Each file controls all FFB parameters for one game. |
| `Global.json` | N/A | Auto-switch toggle, UDP ports, last-used presets per device type | Controls `AresAutoChangeConfig` flag and telemetry port routing. |
| `GameToBaseConfig.json` | N/A | Maps game keys (e.g., `ASSETTO_CORSA`) to `.Base` file paths | The auto-switch lookup table. When Conspit Link detects a game, it reads this to find the preset path. |
| `GameMatchSteamGame.json` | N/A | Maps internal game keys to Steam game display names | Used by Conspit Link for process detection via Steam library. |
| `GameSettingCenter.json` | N/A | Per-game telemetry field enable/disable (RPM, speed, flags, etc.) | Controls which dashboard elements appear on the wheel display. |
| `Settings.json` | N/A | UDP port, language, exit behavior | Minimal. Key field: `UdpPort` (currently 20778). |

### Core: OpenFFBoard HID Protocol

| Technology | Version | Purpose | Why |
|------------|---------|---------|-----|
| OpenFFBoard firmware | Unknown (Conspit fork) | Motor control, FFB effect processing, HID device interface | Runs on the Ares wheelbase MCU. VID: 0x1209, PID: 0xFFB0. Conspit uses OpenFFBoard as their firmware base. |
| HID vendor interface | Usage page 0xFF00, Report ID 0xA1 | Direct wheelbase control bypassing DirectInput | Independent of game FFB stack. Allows rc-agent to send safety commands (estop, gain, idle spring) regardless of what Conspit Link or the game is doing. |

**Verified HID Report Format (26 bytes):**

```
Byte 0:     Report ID (0xA1)
Byte 1:     Type (0=write, 1=request, 10=ACK, 13=notFound, 15=err)
Bytes 2-3:  ClassID (u16 LE)
Byte 4:     Instance (usually 0)
Bytes 5-8:  CmdID (u32 LE)
Bytes 9-16: Data (i64 LE)
Bytes 17-24: Addr (i64 LE, usually 0)
```

**Critical Class IDs (verified against OpenFFBoard wiki + existing ffb_controller.rs):**

| Class | ID | Purpose |
|-------|-----|---------|
| System | 0x0 | Error reporting, firmware info |
| FFB Wheel | 0x1 | Emergency stop, FFB active status |
| Axis | 0xA01 | Power/gain, idle spring, damper, friction, inertia, position readback |
| Effects | 0xA02 | Spring/damper/friction/inertia gain scaling |
| Effects Manager | 0xA03 | Effect lifecycle management |

**Critical Commands for This Project:**

| Command | Class | CmdID | R/W | What It Does |
|---------|-------|-------|-----|--------------|
| `estop` | FFBWheel (0x1) | 0x0A | R/W | Emergency stop -- zeros motor torque immediately. Already implemented in rc-agent. |
| `ffbactive` | FFBWheel (0x1) | 0x00 | R | Read FFB active status (is a game currently sending effects?) |
| `power` | Axis (0xA01) | 0x00 | R/W | Overall force strength (0-65535). Already implemented as `set_gain()`. |
| `idlespring` | Axis (0xA01) | 0x05 | R/W | **KEY FOR STUCK BUG FIX.** Sets centering spring strength when no FFB effects are active. |
| `axisdamper` | Axis (0xA01) | 0x06 | R/W | Independent damper effect (firmware-level, not game-driven). |
| `zeroenc` | Axis (0xA01) | 0x03 | R | Zero/center the axis encoder. |
| `curpos` | Axis (0xA01) | 0x0E | R | Read current wheel position. Useful for detecting stuck state. |
| `spring` | Effects (0xA02) | 0x03 | R/W/I | Spring gain scaling for game-driven spring effects. |
| `damper` | Effects (0xA02) | 0x05 | R/W/I | Damper gain scaling. |

**Confidence:** HIGH -- class IDs and command IDs verified against OpenFFBoard wiki Commands page and cross-referenced with existing `ffb_controller.rs` in rc-agent (which already uses 0x1/0x0A for estop and 0xA01/0x00 for power).

### Core: Telemetry Protocols

| Protocol | Games | Port | Format | How Conspit Link Reads It |
|----------|-------|------|--------|--------------------------|
| UDP | F1 25 | 20777 (game) -> 20778 (Conspit Link `Udp_Get_Port`) | Codemasters 2025 packet format, binary structs with header | Conspit Link listens on `Udp_Get_Port` (20777 in Global.json). Game sends to 127.0.0.1:20777. |
| Shared Memory | AC, ACC, AC EVO, AC Rally | N/A (memory-mapped files) | Windows named file mappings: `Local\acpmf_physics`, `Local\acpmf_graphics`, `Local\acpmf_static` | Conspit Link reads shared memory directly. No port config needed. AC provides ~333Hz physics updates. |
| Shared Memory | ACC | N/A | Similar to AC but ACC-specific structs | Conspit Link has native ACC support via shared memory. |
| AutoRpm | AC, ACC, iRacing, LMU | N/A | Encrypted `.conspit` files in `material/AutoRpm/` | Pre-configured shift light/RPM configs. Not editable -- use as-is. |

**UDP Port Chain (verified from Global.json):**
- F1 25 game sends UDP to `127.0.0.1:20777`
- `Global.json` field `Udp_Get_Port`: `"20777"` -- Conspit Link receives here
- `Global.json` field `UdpPort`: `"20778"` -- Conspit Link forwards here (for rc-agent/external tools)
- rc-agent already monitors port 20777 for F1 telemetry in `driving_detector.rs`

### Core: Rust Crates (already in rc-agent)

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `hidapi` | 2.x | HID device enumeration, vendor interface read/write | Already in rc-agent Cargo.toml. Used by ffb_controller.rs and driving_detector.rs. |
| `serde_json` | workspace | JSON parsing for `.Base` presets, Global.json, GameToBaseConfig.json | Already in rc-agent. Perfect for reading/writing Conspit config files. |
| `tokio` | workspace | Async runtime for UDP listeners, timers, file watchers | Already in rc-agent. |
| `winapi` | 0.3 | Windows API (process management, window manipulation) | Already in rc-agent for ConspitLink window minimization, process detection. |

### New Crates Needed

| Crate | Version | Purpose | Why This One |
|-------|---------|---------|--------------|
| `notify` | 7.x | Filesystem watcher for `.Base` and JSON config file changes | Detect when Conspit Link writes config changes (e.g., user changes preset via GUI). Lightweight, cross-platform, well-maintained. |
| `shared_memory` or raw `winapi` MapViewOfFile | -- | Read AC/ACC shared memory telemetry | For future telemetry dashboard features. NOT needed for Phase 1 (stuck bug fix). Can use raw winapi already in deps. |

### NOT Needed

| Technology | Why Not |
|------------|---------|
| SimHub | Out of scope per PROJECT.md. Conspit Link handles dashboard/shift lights natively. |
| vJoy / ViGEm | Not emulating controllers. Ares is the real device. |
| DirectInput API from rc-agent | rc-agent should NOT send DirectInput FFB effects. That is the game's job. rc-agent uses the vendor HID interface (0xFF00) which is independent. |
| Conspit Link modification/patching | Constraint: configure only, never modify the exe. |
| Custom firmware flash | Ares uses Conspit's OpenFFBoard fork. Reflashing voids warranty and loses Conspit-specific features. |

## Conspit Link 2.0 Configuration Architecture

### File Locations (verified on disk)

```
C:\Program Files (x86)\Conspit Link 2.0\
  ConspitLink2.0.exe            -- Main application (Qt5-based)
  Global.json                    -- Global settings (auto-switch, UDP ports, last-used presets)
  Settings.json                  -- Minimal settings (UDP port, language, exit behavior)
  JsonConfigure\
    GameToBaseConfig.json        -- Game -> preset path mapping (auto-switch lookup)
    GameMatchSteamGame.json      -- Game key -> Steam display name mapping
    GameSettingCenter.json       -- Per-game telemetry field enable/disable
  material\
    DeviceGameConfig\
      AresToGameConfig\
        #My Presets\             -- User-created presets (currently only default.Base)
        ASSETTO_CORSA\
          官方预设\              -- Official presets (default, drift1, drift2)
          职业车手预设\          -- Pro driver presets (Yifei Ye, Congfu Cheng, etc.)
        F1_25\
          官方预设\              -- Official (F1 25_default.Base)
          ConfigureFile\         -- Game setup instructions
        [... per game ...]
    AutoRpm\
      ac.conspit                 -- Encrypted auto-RPM configs (shift lights)
      acc.conspit
      iracing.conspit
      lmu.conspit
    Dashboards\                  -- Dashboard themes (ConspitDash, LovelyDash, SamDash)
  ConspitGameSupportSolutions\   -- Per-game setup instructions
  log\                           -- Application logs
```

### .Base Preset File Format (verified by reading actual files)

Plain JSON. All keys are prefixed with `ui->` (matching Qt widget names in the GUI).

```json
{
    "PresetForm": "Base",
    "ui->Mechanical_Inertia": 5,
    "ui->Slider_Advanced_Effects_Gain": 200,
    "ui->Slider_Damper": 100,
    "ui->Slider_Damping": 100,
    "ui->Slider_Filter_Center": 100,
    "ui->Slider_Filter_Gain": 100,
    "ui->Slider_Mechanical_Damper": 30,
    "ui->Slider_Spring": 100,
    "ui->Slider_advanced_Friction": 20,
    "ui->Slider_angle": 900,
    "ui->Slider_speed_Limit": 0,
    "ui->Speed_switchButton": false,
    "ui->button_Group": 2,
    "ui->lihuikui": false,
    "ui->slider_Force": 0,
    "ui->slider_Friction": 100,
    "ui->slider_Max_Force": 1077,
    "ui->zhoushuru": false
}
```

**Key parameters for venue tuning:**

| Parameter | Range | What It Controls | Venue Consideration |
|-----------|-------|------------------|---------------------|
| `Slider_angle` | degrees | Steering rotation (e.g., 900 for road, 360 for F1) | Must match in-game setting or wheel fights itself |
| `slider_Max_Force` | 0-~1200 | Maximum FFB force output | 1077 = ~8Nm. Higher values clip on Ares 8Nm hardware. |
| `Slider_Spring` | 0-100 | Centering spring strength | 0 in pro presets (pure FFB). Higher for casual/new drivers. |
| `Slider_Damper` / `Slider_Damping` | 0-100 | Damping resistance | 0 in pro presets. Some damping helps prevent oscillation for novices. |
| `Slider_advanced_Friction` | 0-100 | Friction feel | Low in pro presets (16-20). Higher masks detail. |
| `Mechanical_Inertia` | 0-100 | Simulated wheel weight | 0-5 in pro presets. Higher feels heavy/sluggish. |
| `Slider_Filter_Center` | 0-200 | Center filter (deadzone-like) | 166 in Yifei Ye AC preset. Higher = more filtered center. |
| `Slider_Advanced_Effects_Gain` | 0-200 | Multiplier for game-driven effects | 200 in all presets. Maxes out effect intensity. |

**Notable format differences between presets:**
- F1 25 default has extra fields not in AC default: `Slider_FFB_Damping`, `Slider_Smooth`, `Slider_Steering_Range`, `Slider_Stop_Feel`, `Slider_Strength`, `button_Group_2`
- Yifei Ye AC preset has duplicate key `slider_Friction` (JSON technically invalid but most parsers accept last value)

### Auto-Switch Mechanism

`Global.json` contains `"AresAutoChangeConfig": "open"` which enables automatic preset switching. When enabled:

1. Conspit Link monitors running processes (via Steam library names from `GameMatchSteamGame.json`)
2. When a matching game process is detected, it looks up the game key in `GameToBaseConfig.json`
3. The mapped `.Base` file path is loaded and applied to the wheelbase
4. When the game exits, Conspit Link reverts to the last-used or default preset

**Known issue:** Auto-switch is configured ("open") but not working reliably. Possible causes:
- `GameMatchSteamGame.json` has empty strings for some games (Forza Horizon 5, Rennsport, RBR, WRC Generations)
- Games not launched through Steam may not be detected
- Process name matching may fail for non-standard game installs

### Telemetry Dashboard Configuration

`GameSettingCenter.json` controls which telemetry fields are displayed on the wheel screen per game. Verified fields: rpm, speed, gear, clutch, brake, throttle, completedlaps, bestlaptime, tyretemp, braketemp, position, watertemp, flag, abslevel, tclevel, drsavailable.

All venue games (AC, ACC, AC EVO, F1 25, AC Rally) have full telemetry fields enabled except:
- AC: `watertemp: false`
- AC EVO: `bestlaptime: false`
- F1 25: All enabled including `drsavailable: true`

## DirectInput FFB Effect Lifecycle

**What happens when a game process dies mid-FFB-effect:**

1. Game creates DirectInput device with `DISCL_EXCLUSIVE | DISCL_FOREGROUND`
2. Game downloads FFB effects (constant force, spring, damper, etc.) to the device
3. Game starts effects playing
4. **If game crashes or is killed:**
   - DirectInput runtime calls `Unacquire()` during COM cleanup
   - `Unacquire()` is supposed to stop all effects and release the device
   - **BUT:** If the process is killed abruptly (`taskkill /F`), COM cleanup may not run
   - The HID driver retains the last-sent FFB report
   - The wheelbase firmware continues executing the last effect until a new command arrives
   - **This is the root cause of the stuck-rotation bug:** a constant force or spring effect with offset remains active in firmware memory

5. **What Conspit Link does (or fails to do):**
   - Conspit Link re-acquires the device after the game releases it
   - It should send `DISFFC_RESET` to clear all effects
   - **Known issue P-20:** Conspit Link may overwrite rc-agent's zero-force command
   - If Conspit Link's cleanup races with rc-agent's estop, the last writer wins

**The fix strategy must be:**
1. rc-agent sends estop (0x0A) via vendor HID interface BEFORE killing game process
2. rc-agent sets `idlespring` (0x05 on Axis class) to a moderate centering value
3. rc-agent verifies position via `curpos` (0x0E) -- wheel should drift to center
4. Sequence: estop -> wait 100ms -> idlespring on -> verify position
5. Do NOT rely on Conspit Link's cleanup -- it is unreliable and races with rc-agent

**Confidence:** MEDIUM on Conspit Link cleanup behavior (inferred from P-20 bug report + DirectInput documentation). HIGH on the HID-based fix strategy (estop + idlespring are verified OpenFFBoard commands).

## Fleet Deployment Pattern

### Config Sync Architecture

For 8 identical pods, the pattern is:

1. **Golden config on rc-agent deploy server** (or in this repo):
   - Master copies of all `.Base` presets (venue-tuned, not Conspit defaults)
   - Master `Global.json` with correct auto-switch and UDP settings
   - Master `GameToBaseConfig.json` pointing to venue presets
   - Master `GameSettingCenter.json` with correct dashboard fields

2. **rc-agent config push command:**
   - Server sends "sync conspit config" command to all pods
   - rc-agent on each pod copies golden configs to `C:\Program Files (x86)\Conspit Link 2.0\`
   - rc-agent restarts Conspit Link gracefully (not taskkill /F) to pick up new configs
   - rc-agent verifies configs loaded correctly by reading back JSON files

3. **Config drift detection:**
   - rc-agent periodically checksums config files
   - Reports drift to racecontrol server
   - Auto-remediation: re-push golden config if drift detected

4. **Preset deployment path:**
   ```
   .planning/presets/         -- Venue-tuned .Base files (version controlled in this repo)
   rc-agent config push  -->  C:\Program Files (x86)\Conspit Link 2.0\material\DeviceGameConfig\
                              AresToGameConfig\#My Presets\[game]_racingpoint.Base
   GameToBaseConfig.json -->  Updated to point to #My Presets paths
   ```

### Why NOT SimHub / External Dashboard

Conspit Link already provides:
- Native wheel-screen dashboards (ConspitDash, LovelyDash, SamDash themes)
- Shift light LEDs driven by AutoRpm configs
- Per-game telemetry field configuration
- Direct integration with Ares hardware (no extra software layer)

SimHub would add another process to manage, another failure mode, and conflict with Conspit Link's telemetry ownership. Use Conspit Link's built-in capabilities first.

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| FFB safety | OpenFFBoard vendor HID (0xFF00) | DirectInput DISFFC_RESET from rc-agent | DirectInput requires exclusive access. Conspit Link holds the device. Vendor HID works independently. |
| Config format | Direct JSON file manipulation | Conspit Link GUI automation (SendKeys) | Fragile, slow, breaks on UI updates. JSON files are the source of truth. |
| Telemetry | Conspit Link native | SimHub | Extra process, conflicts with CL telemetry. CL already reads shared mem + UDP. |
| Fleet sync | rc-agent file copy + checksum | Group Policy / SCCM | Overkill for 8 machines. rc-agent already has exec + file transfer. |
| Process detection | Conspit Link auto-switch | rc-agent game process monitoring | rc-agent already does this for billing. Let CL handle preset switching. Fix its config. |
| Idle centering | `idlespring` HID command | Software spring effect via DirectInput | DI requires exclusive access. HID idlespring is firmware-level, always works. |

## Installation

No new applications to install. All work is configuration + rc-agent code changes.

```bash
# New Rust crate (if file watching needed later):
cargo add notify@7 -p rc-agent

# No other installs needed -- hidapi, serde_json, tokio, winapi already in deps
```

**Config files to create/modify (per pod):**
```
# Venue .Base presets (new files, copy to all 8 pods):
C:\Program Files (x86)\Conspit Link 2.0\material\DeviceGameConfig\AresToGameConfig\
  #My Presets\AC_RacingPoint.Base
  #My Presets\F1_25_RacingPoint.Base
  #My Presets\ACC_RacingPoint.Base
  #My Presets\ACRally_RacingPoint.Base

# Config modifications (all 8 pods):
Global.json           -- verify AresAutoChangeConfig: "open", correct UDP ports
GameToBaseConfig.json  -- point all active games to RacingPoint presets in #My Presets
GameSettingCenter.json -- verify all telemetry fields enabled for venue games
Settings.json          -- verify UdpPort: "20778"
```

## Sources

- [OpenFFBoard Wiki: Commands](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands) -- Class IDs, command IDs, HID report format (HIGH confidence)
- [OpenFFBoard Wiki: Configurator Guide](https://github.com/Ultrawipf/OpenFFBoard/wiki/Configurator-guide) -- Power scaling, endstop protection (HIGH confidence)
- [OpenFFBoard GitHub](https://github.com/Ultrawipf/OpenFFBoard) -- Firmware architecture (HIGH confidence)
- [Microsoft: SendForceFeedbackCommand](https://learn.microsoft.com/en-us/previous-versions/windows/desktop/ee417918(v=vs.85)) -- DISFFC_RESET/STOPALL behavior (HIGH confidence)
- [Microsoft: Force Feedback Device Driver Interface](https://learn.microsoft.com/en-us/previous-versions/windows/hardware/hid/force-feedback-device-driver-interface) -- Driver cleanup on process exit (MEDIUM confidence)
- [EA Forums: F1 25 UDP Specification](https://forums.ea.com/discussions/f1-25-general-discussion-en/discussion-f1%C2%AE-25-udp-specification/12187351) -- UDP telemetry format (HIGH confidence)
- [ACLIB/SharedMemory](https://github.com/ACLIB/SharedMemory) -- AC shared memory structs (HIGH confidence)
- [acc_shared_memory_rs crate](https://crates.io/crates/acc_shared_memory_rs) -- ACC shared memory for Rust (MEDIUM confidence)
- Local filesystem: `C:\Program Files (x86)\Conspit Link 2.0\` -- All config files verified by direct inspection (HIGH confidence)
- Existing rc-agent code: `ffb_controller.rs`, `driving_detector.rs`, `failure_monitor.rs` -- Verified HID implementation (HIGH confidence)
