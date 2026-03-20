# Architecture Patterns

**Domain:** Fleet wheelbase management system (sim racing venue)
**Researched:** 2026-03-20

## System Overview

The Conspit Link fleet management system has three tiers: a per-pod wheelbase controller (Conspit Link 2.0), a per-pod fleet agent (rc-agent), and a central server (racecontrol). The architecture is NOT a clean greenfield design -- it wraps a closed-source Qt application (ConspitLink2.0.exe) that cannot be modified, only configured via JSON files and controlled via OpenFFBoard HID commands.

```
                          racecontrol (server .23:8080)
                          +--------------------------+
                          | Fleet config store       |
                          | Config push orchestrator |
                          | Session lifecycle mgr    |
                          | Pod health dashboard     |
                          +------------|-------------+
                                       | WebSocket (per-pod)
                    +------------------+------------------+
                    |                  |                  |
              rc-agent (pod 1)   rc-agent (pod 2)  ... rc-agent (pod 8)
              +-------------+    +-------------+       +-------------+
              | FFB ctrl    |    | FFB ctrl    |       | FFB ctrl    |
              | Config sync |    | Config sync |       | Config sync |
              | Game detect |    | Game detect |       | Game detect |
              | Session mgr |    | Session mgr |       | Session mgr |
              +------+------+    +------+------+       +------+------+
                     |                  |                      |
              ConspitLink 2.0    ConspitLink 2.0        ConspitLink 2.0
              (closed-source)    (closed-source)        (closed-source)
                     |                  |                      |
              Ares 8Nm HID       Ares 8Nm HID           Ares 8Nm HID
              (OpenFFBoard)      (OpenFFBoard)          (OpenFFBoard)
```

## Component Boundaries

| Component | Responsibility | Communicates With | Runs On |
|-----------|---------------|-------------------|---------|
| **racecontrol** | Fleet config authority, session lifecycle orchestration, push configs to pods | rc-agent (WebSocket) | Server .23 |
| **rc-agent** | Per-pod automation: game detection, FFB safety, config file management, Conspit Link watchdog | racecontrol (WebSocket), ConspitLink (process mgmt), OpenFFBoard (HID), game processes (process monitoring) | Each pod |
| **ConspitLink 2.0** | Wheelbase configuration GUI, game telemetry reader, dashboard/LED driver, auto game-profile switching | OpenFFBoard (HID/USB), game processes (UDP/shared mem), JSON config files (filesystem) | Each pod |
| **OpenFFBoard firmware** | Motor control, DirectInput FFB processing, vendor HID command interface | ConspitLink (HID), rc-agent (HID), game (DirectInput) | Ares wheelbase MCU |
| **Game process** | Sim racing game, emits telemetry, sends DirectInput FFB effects | OpenFFBoard (DirectInput), ConspitLink (UDP/shared mem) | Each pod |

### Critical Boundary: rc-agent vs ConspitLink

rc-agent and ConspitLink both talk to the same wheelbase via HID. This creates a contention boundary:

- **rc-agent** uses the OpenFFBoard **vendor HID interface** (usage page 0xFF00, report ID 0xA1) for safety commands: estop (0x0A), FFB active flag (0x00), gain/power control.
- **ConspitLink** uses the same vendor HID interface for configuration (angle, damping, spring, etc.) and also opens the **gamepad HID interface** for input display.
- **Game** uses the **DirectInput interface** for FFB effects (constant force, spring, damper, etc.).

**Known conflict (P-20):** ConspitLink may overwrite rc-agent's zero-force command. When rc-agent sends estop, ConspitLink can immediately re-apply its own FFB parameters, negating the safety stop.

## Data Flow

### 1. Configuration Distribution Flow

```
racecontrol (config authority)
    |
    | WebSocket: PushConfig { files: [...] }
    v
rc-agent (config sync module)
    |
    | Write files to disk:
    |   Global.json        -> C:\Program Files (x86)\Conspit Link 2.0\Global.json
    |   Settings.json      -> C:\Program Files (x86)\Conspit Link 2.0\Settings.json
    |   GameToBaseConfig.json -> ...\JsonConfigure\GameToBaseConfig.json
    |   *.Base presets     -> ...\material\DeviceGameConfig\AresToGameConfig\{game}\*.Base
    |   #My Presets/*.Base -> ...\AresToGameConfig\#My Presets\*.Base
    |
    | ALSO: copy Global.json -> C:\RacingPoint\Global.json  *** CRITICAL ***
    v
ConspitLink 2.0 (reads on startup + game detection events)
    |
    | Applies preset to wheelbase via HID
    v
Ares 8Nm (new FFB parameters active)
```

**CRITICAL DISCOVERY:** ConspitLink log shows it reads `C:\RacingPoint\Global.json`, NOT from its own install directory. This is almost certainly why AresAutoChangeConfig is broken on pods -- the file exists in the install dir but ConspitLink looks for it in `C:\RacingPoint\`. The install-dir copy is likely a startup default; runtime reads go to `C:\RacingPoint\`.

### 2. Config Files and Their Roles

| File | Location | Purpose | Who Reads | Who Writes |
|------|----------|---------|-----------|------------|
| `Global.json` | Install dir + `C:\RacingPoint\` | Master settings: AresAutoChangeConfig, UDP ports, Steam path, last-used presets per device | ConspitLink (from `C:\RacingPoint\`) | ConspitLink GUI, rc-agent (fleet sync) |
| `Settings.json` | Install dir | App settings: language, UDP port, exit behavior | ConspitLink | ConspitLink GUI, rc-agent |
| `GameToBaseConfig.json` | `JsonConfigure\` | Maps game ID to .Base preset file path | ConspitLink (auto-switch) | ConspitLink GUI, rc-agent |
| `GameMatchSteamGame.json` | `JsonConfigure\` | Maps game ID to Steam window title (for process detection) | ConspitLink (game detection) | Conspit (shipped) |
| `GameSettingCenter.json` | `JsonConfigure\` | Per-game telemetry field availability (what data each game provides) | ConspitLink (dashboard) | Conspit (shipped) |
| `*.Base` presets | `material\DeviceGameConfig\AresToGameConfig\{GAME}\` | FFB parameters per game | ConspitLink (applies to wheelbase) | Conspit (shipped), rc-agent (custom presets) |
| `*.conspit` | `material\AutoRpm\` | Encrypted per-game RPM LED configs | ConspitLink (shift lights) | Conspit (shipped, encrypted) |

### 3. Game Auto-Detection Flow (AresAutoChangeConfig)

```
ConspitLink 2.0 (background polling)
    |
    | Poll running processes every N seconds
    | Match against GameMatchSteamGame.json window titles:
    |   "Assetto Corsa" -> ASSETTO_CORSA
    |   "F1 25"         -> F1_25
    |   etc.
    |
    | If match found AND AresAutoChangeConfig == "open":
    |   Look up GameToBaseConfig.json[matched_game_id]
    |   Load the .Base preset file
    |   Apply FFB parameters to wheelbase via HID
    |   Load matching AutoRpm config if .conspit exists
    |   Load matching dashboard/telemetry config from GameSettingCenter.json
    |
    v
Wheelbase now configured for detected game
```

**Why it is currently broken:** ConspitLink reads Global.json from `C:\RacingPoint\Global.json` at runtime. That path does not exist on pods (it is the server's config path). The file only exists in the install directory. Fix: copy/symlink Global.json to `C:\RacingPoint\` on each pod.

### 4. Telemetry Data Flow

```
Game Process
    |
    +-- UDP (F1 25: port 20777 -> ConspitLink receives on 20778)
    |   ConspitLink forwards/processes for dashboard + shift LEDs
    |
    +-- Shared Memory (AC, ACC, AC EVO)
    |   ConspitLink reads directly from mapped memory
    |
    +-- DirectInput FFB effects
        Sent to OpenFFBoard gamepad interface
        Wheelbase renders forces to motor
```

ConspitLink receives telemetry and drives:
- **Wheel display dashboard:** RPM, speed, gear, temps, flags, lap time, position
- **Shift light LEDs:** RPM-triggered, per-game config in .conspit files
- **RGB rim lighting:** Tied to telemetry events (flags, RPM zones)

rc-agent also monitors UDP telemetry (ports 9996, 20777, 5300, 6789, 5555) independently for driving state detection and freeze detection. rc-agent does NOT depend on ConspitLink for telemetry -- it listens directly.

### 5. Session Lifecycle Flow

```
[IDLE] Kiosk lock screen visible, ConspitLink running minimized
    |
    | racecontrol: LaunchGame command via WebSocket
    v
[LAUNCHING] rc-agent starts game process, monitors for PID
    |
    | Game PID detected, UDP/shared-mem telemetry begins
    | ConspitLink detects game (if AresAutoChangeConfig works)
    | ConspitLink loads game-specific .Base preset
    v
[ACTIVE] Customer driving, FFB active, telemetry flowing
    |
    | Session timer expires OR staff ends session
    | racecontrol: EndSession command via WebSocket
    v
[ENDING] rc-agent sequence:
    | 1. Send HID estop (0x0A) to zero motor torque     <-- RACE CONDITION with P-20
    | 2. Wait brief delay
    | 3. Kill game process
    | 4. Send HID ffbactive=0 as belt-and-suspenders
    | 5. Apply centering spring (NOT YET IMPLEMENTED)
    | 6. Return to kiosk lock screen
    v
[IDLE] Ready for next customer
```

**The stuck-rotation bug lives in step 1-4.** The game dies, its DirectInput FFB effects become stale (last force vector held), rc-agent's estop fires but ConspitLink may overwrite it (P-20), and no centering spring is applied. The wheel snaps to whatever the last force vector was and stays there.

### 6. HID Command Path (Safety)

```
rc-agent (FfbController)
    |
    | hidapi: open device VID:0x1209 PID:0xFFB0, usage_page 0xFF00
    | Write 26-byte vendor report:
    |   [0xA1][type][classID_LE][instance][cmdID_LE][data_LE][addr_LE]
    |
    | Commands available:
    |   FFBWheel (0x00A1):
    |     CMD_ESTOP (0x0A, data=1)     -> emergency torque zero
    |     CMD_FFB_ACTIVE (0x00, data=0) -> disable FFB processing
    |   Axis (0x0A01):
    |     CMD_POWER (0x00, data=0-65535) -> gain percentage
    |
    v
OpenFFBoard firmware (Ares wheelbase MCU)
    |
    | Processes vendor command immediately
    | Motor torque goes to zero / gain adjusted
    v
Motor (belt-drive, 8Nm max)
```

**Both rc-agent and ConspitLink use hidapi.dll** to talk to the same USB device. They can both open it simultaneously (HID allows multiple readers). Write contention is the P-20 issue -- whoever writes last wins.

## Recommended Architecture

### Layer 1: Central Config Authority (racecontrol)

racecontrol stores the canonical config set:
- One `Global.json` template (with AresAutoChangeConfig: "open")
- One `Settings.json` template
- One `GameToBaseConfig.json` mapping Racing Point's chosen presets per game
- Racing Point custom `.Base` preset files (tuned for venue's Ares 8Nm units)
- Per-pod overrides if needed (unlikely -- fleet should be identical)

Exposes a WebSocket command: `PushConfig` that sends config files to rc-agent.

### Layer 2: Pod Agent Config Sync (rc-agent)

New module: `conspit_config.rs`
- Receives config push from racecontrol
- Writes files to correct locations:
  - `Global.json` to BOTH install dir AND `C:\RacingPoint\Global.json`
  - `Settings.json` to install dir
  - `GameToBaseConfig.json` to `JsonConfigure\`
  - `.Base` files to appropriate game preset dirs
- Validates file writes (read-back and compare)
- Reports config state hash to racecontrol for drift detection
- Triggers ConspitLink restart if config changed while running (graceful close, re-launch, minimize)

### Layer 3: Session Safety Orchestration (rc-agent)

Enhanced `ffb_controller.rs` for the stuck-rotation fix:
1. **Pre-kill sequence:** Before killing game process, send estop + wait 200ms + verify with HID read
2. **Spring centering:** After estop, apply centering spring via OpenFFBoard spring effect command
3. **ConspitLink coordination:** After game kill, tell ConspitLink to reload idle preset (via config file swap or ConspitLink restart)
4. **Post-kill verification:** Read wheel position via HID input, confirm it is near center within 5 seconds
5. **Fallback:** If wheel still offset after 5s, power-cycle USB port via devcon or restart ConspitLink

### Layer 4: Telemetry Monitoring (rc-agent, existing)

Already implemented:
- `driving_detector.rs` monitors HID input + UDP packets
- `failure_monitor.rs` detects freeze, USB disconnect, launch timeout, telemetry gaps
- ConspitLink independently reads telemetry for dashboard/LEDs

No changes needed here -- this layer is solid.

## Component Boundaries (Detailed)

### What rc-agent Owns

| Capability | How | Module |
|------------|-----|--------|
| FFB emergency stop | HID vendor cmd: estop (0x0A) | `ffb_controller.rs` |
| FFB gain control | HID vendor cmd: Axis power (0x00) | `ffb_controller.rs` |
| Centering spring (NEW) | HID vendor cmd: spring constant force | `ffb_controller.rs` |
| ConspitLink process watchdog | Process scan + restart + minimize | `ac_launcher.rs` |
| Config file distribution | Write JSON + .Base to disk | `conspit_config.rs` (NEW) |
| Config state reporting | Hash config files, report to server | `conspit_config.rs` (NEW) |
| Game process detection | Process scan by name | `ac_launcher.rs` |
| Driving state detection | HID input + UDP monitoring | `driving_detector.rs` |
| Failure detection | Freeze, USB, telemetry gaps | `failure_monitor.rs` |

### What ConspitLink Owns (cannot modify)

| Capability | How | Config |
|------------|-----|--------|
| Auto game profile switching | Process polling + GameMatchSteamGame.json | `Global.json` AresAutoChangeConfig |
| FFB preset application | Read .Base, write to wheelbase via HID | `GameToBaseConfig.json` |
| Dashboard display | Read telemetry, render to wheel display | `GameSettingCenter.json` |
| Shift light LEDs | RPM from telemetry | `*.conspit` (encrypted, shipped) |
| RGB lighting | Telemetry events | Material configs (shipped) |
| Telemetry reception | UDP listener + shared memory reader | `Settings.json` UdpPort |

### What racecontrol Owns

| Capability | How | Endpoint |
|------------|-----|----------|
| Fleet config authority | Store canonical configs | Database/filesystem |
| Config push to pods | WebSocket PushConfig command | WS per-pod |
| Config drift detection | Compare pod config hashes vs canonical | Fleet health API |
| Session lifecycle commands | LaunchGame, EndSession | WS per-pod |
| Pod health aggregation | Heartbeat + fleet status | `/api/v1/fleet/health` |

## Anti-Patterns to Avoid

### Anti-Pattern 1: Treating ConspitLink as a controllable service
**What:** Trying to send commands to ConspitLink or control it programmatically beyond process start/stop.
**Why bad:** ConspitLink is a closed-source Qt GUI app. It has no API, no IPC, no command-line flags. The only control surface is: start it, stop it, write its config files before/after restart, and minimize its window.
**Instead:** Control the wheelbase directly via OpenFFBoard HID for safety commands. Control ConspitLink indirectly by writing its config files and restarting it.

### Anti-Pattern 2: Racing HID writes between rc-agent and ConspitLink
**What:** Sending HID commands to the wheelbase while ConspitLink is actively applying a preset.
**Why bad:** Last writer wins. If ConspitLink applies a preset 50ms after rc-agent sends estop, the estop is negated. This IS the P-20 bug and likely the root cause of stuck-rotation.
**Instead:** Sequence operations: (1) estop via HID, (2) wait for ConspitLink to settle (it polls on an interval, not continuously), (3) verify wheel state via HID read. For session-end, consider briefly closing ConspitLink before estop, then restarting it.

### Anti-Pattern 3: Config files in one location only
**What:** Assuming ConspitLink reads all config from its install directory.
**Why bad:** Log evidence proves ConspitLink reads Global.json from `C:\RacingPoint\` at runtime, not from its install dir. Config sync that only writes to install dir will NOT fix AresAutoChangeConfig.
**Instead:** Write to BOTH locations. Treat `C:\RacingPoint\Global.json` as the authoritative runtime path.

### Anti-Pattern 4: Deploying presets without restarting ConspitLink
**What:** Writing new .Base files or GameToBaseConfig.json while ConspitLink is running and expecting it to pick them up.
**Why bad:** ConspitLink likely caches config at startup. Changing files on disk without restart means ConspitLink uses stale config until next restart.
**Instead:** After writing config files, gracefully restart ConspitLink (close window, wait for exit, re-launch, minimize).

## Patterns to Follow

### Pattern 1: Config-then-restart
**What:** All config changes follow: write files -> verify writes -> graceful restart ConspitLink -> minimize window -> report success.
**When:** Any config push from racecontrol, any preset change, any Global.json update.
```
1. Write new config files to disk (both locations for Global.json)
2. Read back and verify content matches
3. Gracefully close ConspitLink (WM_CLOSE, not taskkill /F)
4. Wait for process exit (max 5s, then force if needed)
5. Start ConspitLink
6. Wait 4s for GUI init
7. Minimize window (FindWindow + ShowWindow)
8. Report config hash to racecontrol
```

### Pattern 2: Estop-with-ConspitLink-awareness
**What:** Safety commands account for ConspitLink's potential to overwrite.
**When:** Session end, emergency stop, any time motor must be zeroed.
```
1. Close ConspitLink (prevents it from fighting the estop)
2. Send HID estop (0x0A, data=1)
3. Send HID ffbactive=0
4. Wait 200ms
5. Apply spring centering force (if OpenFFBoard supports it)
6. Restart ConspitLink (it will re-init with idle state)
7. Minimize ConspitLink
```

### Pattern 3: Config hash for drift detection
**What:** rc-agent computes SHA256 of all config files and reports to racecontrol periodically.
**When:** Every heartbeat (every 30s).
```rust
struct ConspitConfigState {
    global_json_hash: String,
    settings_json_hash: String,
    game_to_base_hash: String,
    preset_hashes: HashMap<String, String>, // game_id -> .Base file hash
    conspit_link_running: bool,
    conspit_link_version: String,
}
```
racecontrol compares against canonical hashes. Mismatch = config drift alert + auto-push.

## Scalability Considerations

| Concern | At 8 pods (current) | At 20 pods | At 50+ pods |
|---------|---------------------|------------|-------------|
| Config push | Sequential WS push, <1s total | Still fine, WS is per-pod | Batch push with progress tracking |
| Drift detection | Per-heartbeat hash, trivial | Same | Aggregate dashboard needed |
| HID contention | One wheelbase per pod, no contention between pods | Same | Same (HID is local) |
| Preset storage | ~20 .Base files per pod, <100KB total | Same | Same |
| Session lifecycle | Server orchestrates per-pod | Server needs queuing | Priority queue for session commands |

Fleet size is not a scaling concern for this system. The bottleneck is the HID contention between rc-agent and ConspitLink on each individual pod, which is a per-pod problem, not a fleet problem.

## Build Order (Dependencies)

```
Phase 1: Fix stuck-rotation bug
    Requires: Understanding of HID command timing
    Depends on: Nothing (existing ffb_controller.rs)
    Enables: Safe session transitions (everything else depends on this)

Phase 2: Fix AresAutoChangeConfig
    Requires: Global.json at C:\RacingPoint\ on each pod
    Depends on: Phase 1 (safe to test game switching only when session-end is safe)
    Enables: Automatic per-game FFB preset loading

Phase 3: Tune per-game FFB presets
    Requires: AresAutoChangeConfig working (Phase 2)
    Depends on: Phase 2
    Enables: Good FFB feel per game

Phase 4: Config sync via rc-agent
    Requires: conspit_config.rs module, racecontrol PushConfig command
    Depends on: Phase 2-3 (must have correct configs to push)
    Enables: Fleet-wide consistency

Phase 5: Telemetry/dashboard/LEDs
    Requires: ConspitLink properly configured (Phase 2)
    Depends on: Phase 2
    Enables: Customer-facing telemetry features
    Can run in parallel with Phase 3-4
```

## Sources

- **ConspitLink 2.0 log** (`C:\Program Files (x86)\Conspit Link 2.0\log\2026-03-17_log.ConspitLog`): Reveals Global.json read path is `C:\RacingPoint\Global.json`, not install dir. HIGH confidence.
- **rc-agent source** (`ffb_controller.rs`, `driving_detector.rs`, `failure_monitor.rs`, `ac_launcher.rs`): Existing HID command structure, safety commands, process management. HIGH confidence.
- **ConspitLink config files** (Global.json, Settings.json, GameToBaseConfig.json, GameSettingCenter.json, .Base presets): File formats and game mapping structure. HIGH confidence.
- **OpenFFBoard HID protocol**: Vendor command structure (report ID 0xA1, 26-byte reports). HIGH confidence from existing rc-agent implementation.
- **ConspitLink ChangeLog**: Version 1.1.2, Qt-based application. HIGH confidence.
