# Technology Stack: AC Launch & Session Management

**Project:** AC Launcher -- Assetto Corsa Launch System for Racing Point Pods
**Researched:** 2026-03-13
**Overall confidence:** HIGH (existing codebase verified + AC ecosystem well-documented)

## Context: What Already Exists

The rc-agent codebase already has substantial AC integration. This research documents the complete AC tooling ecosystem so the roadmap knows what to build ON TOP OF versus what to REPLACE.

### Already Implemented (in racecontrol repo)
| Component | File | Status |
|-----------|------|--------|
| race.ini generation | `rc-agent/src/ac_launcher.rs` | Working -- single-player practice only |
| assists.ini generation | `rc-agent/src/ac_launcher.rs` | Working |
| FFB gain control | `rc-agent/src/ac_launcher.rs` | Working (light/medium/strong presets) |
| Content Manager URI launch | `rc-agent/src/ac_launcher.rs` | Working for multiplayer via `acmanager://race/online` |
| Direct acs.exe launch | `rc-agent/src/ac_launcher.rs` | Working with fallback from CM |
| Shared memory telemetry | `rc-agent/src/sims/assetto_corsa.rs` | Working (physics + graphics + static) |
| AC dedicated server management | `rc-core/src/ac_server.rs` | Working (INI generation, process lifecycle) |
| Entry list generation | `rc-core/src/ac_server.rs` | Working |
| Port allocation | `rc-core/src/port_allocator.rs` | Working (dynamic UDP/TCP/HTTP, cooldown) |
| Multiplayer orchestration | `rc-core/src/multiplayer.rs` | Working (group sessions, wallet, PIN) |
| Car/track catalog | `rc-core/src/catalog.rs` | Working (36 tracks, 325 cars) |
| Difficulty presets | `rc-core/src/catalog.rs` | Working (easy/medium/hard) |
| Game process management | `rc-agent/src/game_process.rs` | Working (PID tracking, orphan cleanup) |
| Driving detector | `rc-agent/src/driving_detector.rs` | Working (HID + UDP hysteresis) |
| Conspit Link management | `rc-agent/src/ac_launcher.rs` | Working (restart, minimize) |
| Lock screen / cleanup | `rc-agent/src/ac_launcher.rs` | Working (enforce_safe_state) |

---

## Assetto Corsa Configuration File Ecosystem

**Confidence: HIGH** (verified against existing codebase + official AC documentation + community tools)

### File Locations on Pods

All AC configuration files live under the user's Documents folder:
```
%USERPROFILE%\Documents\Assetto Corsa\cfg\
    race.ini          -- Session/car/track/assists configuration
    assists.ini       -- Assist overrides (CSP/CM may read this instead of race.ini)
    controls.ini      -- FFB settings, controller mappings
    apps-default.ini  -- HUD app layout
    video.ini         -- Graphics settings (not our concern)
```

AC installation directory (read-only, game data):
```
C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\
    acs.exe           -- Game executable
    content\cars\     -- Car folders (folder name = car ID)
    content\tracks\   -- Track folders (folder name = track ID)
```

CSP configuration (Custom Shaders Patch):
```
%USERPROFILE%\Documents\Assetto Corsa\cfg\extension\
    gui.ini           -- FORCE_START=1, HIDE_MAIN_MENU=1 (already configured)
```

### race.ini -- The Core Configuration File

**What it controls:** Everything about the session -- car, track, assists, AI, session type, remote server.

**Session TYPE values** (used in `[SESSION_0]` blocks):
| Value | Session Type | Notes |
|-------|-------------|-------|
| 1 | Practice | Free driving, no AI interaction required |
| 2 | Qualifying | Timed laps, grid position determined |
| 3 | Race | Grid start, lap-counted or time-limited |

**Confidence: HIGH** -- verified from existing `ac_launcher.rs` which uses `TYPE=1` for practice, and from the AC server manager source code (JustaPenguin/assetto-server-manager `config_ini.go`).

**Key sections for our use case:**

```ini
[RACE]
AI_LEVEL=95          ; 0-100, percentage skill (maps to difficulty tier)
CARS=8               ; Total cars in session (1 = solo, >1 = player + AI)
CONFIG_TRACK=        ; Track configuration/layout variant (empty = default)
DRIFT_MODE=0         ; 0=normal, 1=drift scoring
FIXED_SETUP=0        ; 0=free setup, 1=fixed (we want 0 for customer flexibility)
JUMP_START_PENALTY=0 ; 0=no penalty for jump starts
MODEL=ferrari_488_gt3 ; Player car model (folder name)
PENALTIES=1          ; 1=enforce penalties (cutting, etc.)
RACE_LAPS=10         ; Laps for race session (0=unlimited/time-based)
TRACK=monza          ; Track folder name

[CAR_0]              ; Player car (always index 0)
MODEL=ferrari_488_gt3
SKIN=00_default
DRIVER_NAME=Player
NATIONALITY=IND
SETUP=               ; Empty = default setup

[CAR_1]              ; AI opponent 1 (only when CARS > 1)
MODEL=ferrari_488_gt3
SKIN=01_red
DRIVER_NAME=AI Driver 1
AI_LEVEL=95          ; Per-car AI level override
AI_AGGRESSION=0      ; 0.0-1.0, how aggressive AI is (passed separately)

[SESSION_0]
NAME=Race
DURATION_MINUTES=0   ; 0=lap-based, >0=time-based
SPAWN_SET=START      ; START=grid, PIT=pit lane (practice)
TYPE=3               ; 1=Practice, 2=Qualifying, 3=Race
LAPS=10
STARTING_POSITION=1  ; Player grid position (1=pole)

[AUTOSPAWN]
ACTIVE=1             ; Skip menus, go straight to track

[REMOTE]
ACTIVE=0             ; 0=offline, 1=connect to server
SERVER_IP=
SERVER_PORT=0
PASSWORD=
```

**What's missing in the current implementation:**
- Multiple `[CAR_N]` blocks for AI opponents (current code only writes `[CAR_0]`)
- `SESSION_0` always hard-codes `TYPE=1` (Practice) -- need Race and Qualifying
- No AI_LEVEL per-car configuration
- No RACE_LAPS/STARTING_POSITION for race mode
- No multi-session support (e.g., Practice + Qualifying + Race weekend)

### assists.ini -- Assist Overrides

**Why it exists:** CSP and Content Manager may read assists from this file instead of race.ini's `[ASSISTS]` section. The existing code already writes both (correct approach).

**Values for our difficulty tiers:**

| Setting | Rookie | Amateur | Semi-Pro | Pro | Alien |
|---------|--------|---------|----------|-----|-------|
| ABS | 1 | 1 | 1 | 0 | 0 |
| TC | 1 | 1 | 1 | 0 | 0 |
| STABILITY | 1 | 1 | 0 | 0 | 0 |
| AUTO_CLUTCH | 1 | 1 | 1 | 0 | 0 |
| IDEAL_LINE | 1 | 0 | 0 | 0 | 0 |
| AUTO_SHIFTER | 1 | 1 | 0 | 0 | 0 |
| DAMAGE | 0 | 0 | 0 | 0 | 0 |

**DAMAGE is always 0** -- non-negotiable safety constraint. Prevents hardware damage from crashes and keeps customers happy.

**Confidence: HIGH** -- values verified against existing `catalog.rs` difficulty presets (easy/medium/hard), extended to 5 tiers.

### controls.ini -- FFB Configuration

**Key section:**
```ini
[FF]
GAIN=70              ; 0-100, force feedback strength
FILTER=0             ; 0-1.0, low-pass filter
DAMPER_GAIN=0        ; 0-100, damper effect
DAMPER_MIN_LEVEL=0
ENABLE_GYRO=0
MIN_FORCE=0
```

**Existing preset mapping** (from `ac_launcher.rs`):
- light = GAIN=40
- medium = GAIN=70
- strong = GAIN=100

**Confidence: HIGH** -- verified in codebase.

---

## Content Manager (CM) Integration

**Confidence: MEDIUM** (CM is closed-source; URI protocol reverse-engineered from actools GitHub repo)

### acmanager:// URI Protocol

Content Manager registers the `acmanager://` URI scheme handler on Windows. The existing code uses this for multiplayer joins.

**Known URI patterns:**

| URI | Purpose | Used By Us |
|-----|---------|-----------|
| `acmanager://race/config` | Launch using current race.ini | Not used (fails without CM Quick Drive preset) |
| `acmanager://race/online?ip=X&httpPort=Y` | Join online server | YES -- multiplayer |
| `acmanager://race/online/join?ip=X&httpPort=Y` | Alternative join format | YES -- in ac_server.rs |
| `acmanager://race/csp` | Launch CSP-specific mode | Not needed |
| `acmanager://race/quickdrive` | Launch Quick Drive | Not reliable on pods |

**Critical finding:** The existing code already discovered that `acmanager://race/config` fails with "Settings are not specified" if CM's Quick Drive was never configured on the pod. This is why single-player uses direct `acs.exe` launch instead.

**Recommendation: Use direct acs.exe launch for single-player, CM URI for multiplayer only.** This is already the approach in the codebase and it's correct.

**Why not CM for everything:**
1. CM Quick Drive state varies per pod (some never configured)
2. CM can show error dialogs that block automation
3. Direct acs.exe with pre-written race.ini is deterministic and testable
4. CM adds ~5-10 seconds of overhead (launches CM, then CM launches acs.exe)
5. CM for multiplayer is necessary because it handles the server join handshake that acs.exe alone cannot do reliably

**Confidence: HIGH** -- empirically verified on pods, documented in code comments.

---

## AC Dedicated Server Configuration

**Confidence: HIGH** (verified against existing `ac_server.rs` which already generates these files)

### server_cfg.ini Parameters

The existing `generate_server_cfg_ini()` in `ac_server.rs` already handles all essential parameters. Key additions needed for this project:

**Session blocks** (already partially implemented):
```ini
[PRACTICE]
NAME=Practice
TIME=10              ; Duration in minutes
IS_OPEN=1            ; Allow joining
WAIT_TIME=30         ; Seconds between session transitions

[QUALIFY]
NAME=Qualifying
TIME=10
IS_OPEN=1
WAIT_TIME=60

[RACE]
NAME=Race
LAPS=10              ; 0=time-based
TIME=0               ; 0=lap-based
IS_OPEN=1
WAIT_TIME=60
```

**Safety enforcement on server:**
```ini
DAMAGE_MULTIPLIER=0           ; Always 0 -- hardware protection
TYRE_WEAR_RATE=100            ; Realistic wear
FUEL_RATE=100                 ; Realistic fuel
ABS_ALLOWED=1                 ; 0=off, 1=factory, 2=on -- tier-dependent
TC_ALLOWED=1                  ; Same scale
STABILITY_ALLOWED=1           ; 0=off, 1=on
AUTOCLUTCH_ALLOWED=1          ; 0=off, 1=on
```

**CSP integration** (already implemented):
```
cfg/csp_extra_options.ini     ; Placed in server session directory
```

The `min_csp_version` field in `AcLanSessionConfig` encodes into the track name:
```
TRACK=csp/2144/../monza       ; Requires CSP 2144+
```
This forces clients to have CSP installed, which fixes audio restart issues on session transitions.

**Confidence: HIGH** -- all verified in existing `ac_server.rs` source.

### entry_list.ini

Already fully implemented in `generate_entry_list_ini()`. Supports:
- Named entries with GUID, car model, skin, ballast, restrictor
- Auto-generated pickup mode entries (empty slots alternating across allowed cars)

---

## Game State Detection Methods

**Confidence: HIGH** (verified against existing `assetto_corsa.rs` shared memory implementation)

### Method 1: Shared Memory (PRIMARY -- already implemented)

AC exposes three memory-mapped files:

| Name | Update Rate | Purpose |
|------|------------|---------|
| `Local\acpmf_physics` | Every frame (~60Hz) | Throttle, brake, steering, speed, RPM, gear |
| `Local\acpmf_graphics` | ~10Hz | Lap times, sectors, position, session status |
| `Local\acpmf_static` | Once per session | Car model, track, driver name, max RPM |

**AC_STATUS field** (graphics offset 4, i32):
| Value | Name | Meaning |
|-------|------|---------|
| 0 | AC_OFF | Game not loaded / in menu |
| 1 | AC_REPLAY | Watching replay |
| 2 | AC_LIVE | On track, actively driving |
| 3 | AC_PAUSE | Game paused |

**This is the key field for billing sync.** When AC_STATUS transitions from 0 to 2, the customer is actually on track and driving. Billing should start at this transition, not when acs.exe is launched.

**Additional on-track detection fields:**
- `isInPit` (offset 160): 1 = in pit lane (still "live" but not on track)
- `normalizedCarPosition` (offset 248): 0.0-1.0, track progress (changes = actually moving)
- `speedKmh` (physics offset 28): > 0 confirms movement
- `completedLaps` (offset 132): increments = actively completing laps

**Current implementation status:** The shared memory reader is fully working in `assetto_corsa.rs`. It reads physics, graphics, and static data. Sector times, lap detection, and telemetry frames are all functional.

**What's missing for billing sync:** The current code does not expose AC_STATUS to the billing system. The billing timer reset happens on `GameState::Running` (process started), not on `AC_STATUS == AC_LIVE` (actually on track). This is the DirectX initialization gap mentioned in PROJECT.md.

### Method 2: Process Monitoring (SECONDARY -- already implemented)

Used for launch detection and crash recovery:
- `find_acs_pid()` -- polls tasklist for acs.exe
- `is_process_alive(pid)` -- checks via OpenProcess/GetExitCodeProcess
- `cleanup_orphaned_games()` -- kills stale game processes on startup

**Not suitable for billing sync** because process existence doesn't indicate on-track state.

### Method 3: UDP Telemetry (TERTIARY -- already implemented)

AC sends telemetry to UDP port 9996. The driving detector monitors this:
- Packets arriving = game is running and sending data
- No packets = game not running or not on track

**Less reliable than shared memory** because:
- UDP can be lost
- Port may conflict with other software
- Doesn't provide session state granularity

### Method 4: Log File Parsing (NOT RECOMMENDED)

AC writes logs to `%USERPROFILE%\Documents\Assetto Corsa\logs\`. Could parse for session events.

**Why not:** Slow (file I/O), unreliable (log format changes between versions), and shared memory provides the same information with better latency.

### Recommendation: Shared Memory for State, Process Monitoring for Lifecycle

| Concern | Method | Why |
|---------|--------|-----|
| Is customer on track? | Shared memory AC_STATUS | Sub-second detection, zero overhead |
| Did game crash? | Process alive check | Definitive -- no false positives |
| Has lap completed? | Shared memory completedLaps | Already implemented, works well |
| Is customer idle? | Driving detector (HID + UDP) | Hysteresis prevents flicker |
| Did DirectX finish loading? | Shared memory AC_STATUS transition 0->2 | Solves the billing gap |

---

## What NOT To Use (and Why)

### Do NOT use Content Manager for single-player launches
- CM Quick Drive state varies per pod
- CM can show error dialogs that require manual dismissal
- Direct acs.exe + race.ini is deterministic

### Do NOT use AssettoServer (community alternative)
- Different binary, different config format
- Our pods have stock acServer.exe from Steam
- Unnecessary migration complexity for a LAN venue
- Stock acServer is sufficient for 8-pod LAN races

### Do NOT use log file parsing for game state
- Shared memory is faster and more reliable
- Log format not guaranteed stable across AC versions

### Do NOT use Content Manager's internal API/RPC
- CM is closed-source .NET WPF app
- No stable API contract
- acmanager:// URI is the only supported integration point

### Do NOT attempt to modify AC configuration files while the game is running (except transmission)
- race.ini and assists.ini are read at launch only
- Changing them mid-session has no effect (confirmed by existing code comments)
- Exception: AUTO_SHIFTER changes can take effect after Ctrl+R or pit restart
- controls.ini FFB GAIN changes take effect next session

---

## Recommended Additions to Stack

### For Single-Player Race Mode (New)

Extend `write_race_ini()` to support:

1. **Multiple AI car blocks** -- `[CAR_1]` through `[CAR_N]` with configurable AI_LEVEL per car
2. **Race session type** -- `TYPE=3` instead of hardcoded `TYPE=1`
3. **Grid start** -- `SPAWN_SET=START` instead of `SPAWN_SET=PIT`
4. **Lap count** -- `RACE_LAPS=N` and `LAPS=N` in `[SESSION_0]`
5. **AI difficulty mapping** to racing-themed tiers

### AI Difficulty Tier Mapping (New)

| Tier | AI_LEVEL | AI_AGGRESSION | Player Assists |
|------|----------|---------------|----------------|
| Rookie | 70 | 0.0 | Full (ABS+TC+SC+ideal line) |
| Amateur | 80 | 0.1 | Most (ABS+TC, no ideal line) |
| Semi-Pro | 90 | 0.2 | Some (ABS+TC, no SC) |
| Pro | 95 | 0.4 | None |
| Alien | 100 | 0.7 | None |

**Confidence: MEDIUM** -- AI_LEVEL values need tuning on actual pods. The percentage mapping is based on community consensus that 70-80% is approachable for beginners, 90-95% is competitive, and 100% is very fast.

**AI_AGGRESSION** is a per-car value (0.0-1.0) that controls how aggressively AI cars defend position and overtake. Not all AC versions/CSP versions support this -- needs testing.

### For Billing Sync (Critical Gap)

The key technical change:
1. After game launches, poll shared memory `AC_STATUS` (graphics offset 4)
2. Wait for transition from 0 (OFF/loading) to 2 (LIVE/on track)
3. Signal billing system to start timer at this moment, not at process launch
4. This eliminates the 10-30 second DirectX initialization window

**Implementation approach:** Add a new state `GameState::OnTrack` between `Launching` and `Running`, triggered by shared memory AC_STATUS == 2.

### For Mid-Session Assist Changes

Already partially implemented (transmission). Need to add:
- ABS/TC/stability changes via assists.ini rewrite (takes effect after pit restart)
- FFB gain changes via controls.ini rewrite (takes effect next session or after restart)

**Limitation:** AC reads most assists at session load. Mid-session changes are limited to:
- AUTO_SHIFTER: Works after pit restart (confirmed in code)
- FFB GAIN: Works after AC restart
- ABS/TC/STABILITY: Work only if CSP's "allow assist changes" is enabled

### For Valid Option Filtering (New)

Need a validation layer that checks:
1. **Track has pit spots for N cars** -- some tracks have limited pit boxes
2. **Car has AI data** -- not all modded cars have AI line files, making AI races impossible
3. **Track+car combo** -- some cars are too fast for small tracks (safety concern at venue)
4. **Session type validity** -- drift mode only works with certain tracks/cars

**How to implement:** Scan the AC installation directory on each pod for:
- `content/tracks/{track}/ai/` -- presence = AI racing supported
- `content/tracks/{track}/data/pit_*.kn5` or pit count in `surfaces.ini` -- max grid
- `content/cars/{car}/data/ai/` -- presence = AI opponent viable

---

## Technology Decisions Summary

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Single-player launch | Direct acs.exe + race.ini | Deterministic, no CM dependency |
| Multiplayer launch | acmanager:// URI via CM | Only reliable way to handle server join handshake |
| Game state detection | Shared memory (acpmf_graphics) | Sub-second, zero overhead, already implemented |
| Billing sync trigger | AC_STATUS == 2 (LIVE) | Solves DirectX init billing gap |
| AC server management | Stock acServer.exe | Already working, no migration needed |
| Difficulty tiers | 5 racing-themed + AI_LEVEL mapping | More granular than current 3-tier system |
| FFB control | controls.ini GAIN rewrite | Already implemented, proven approach |
| Session types | race.ini SESSION_0 TYPE=1/2/3 | Standard AC mechanism |
| Option validation | Filesystem scan of AC content dirs | Only reliable source of truth for installed content |
| Mid-session assists | assists.ini rewrite | Limited effectiveness, needs CSP cooperation |

## Installation / Dependencies

No new external dependencies needed. Everything builds on:
- Existing Rust/Axum rc-agent (runs on each pod)
- Existing rc-core server (manages AC server, billing, multiplayer)
- Windows API (winapi crate) for shared memory access
- Stock AC + CSP + Content Manager (already installed on all pods)
- Stock acServer.exe (already on Racing-Point-Server .51)

## Sources

- [Content Manager actools source (URI handler)](https://github.com/gro-ove/actools/blob/master/AcManager/Tools/ArgumentsHandler.Commands.cs) -- MEDIUM confidence (code inspection)
- [AC Dedicated Server Manual (Kunos)](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/) -- HIGH confidence (official)
- [AC Shared Memory Reference](https://assettocorsamods.net/threads/doc-shared-memory-reference.58/) -- HIGH confidence (community-verified)
- [Assetto Server Manager config reference](https://github.com/JustaPenguin/assetto-server-manager/blob/master/config_ini.go) -- HIGH confidence (widely used tool)
- [CSP Server Extra Options docs](https://cup.acstuff.club/docs/csp/misc/server-extra-options) -- MEDIUM confidence (CSP documentation)
- [AssettoServer docs](https://assettoserver.org/docs/thebeginnersguide/) -- LOW confidence (alternative server, not our target)
- Existing racecontrol codebase -- HIGH confidence (running in production)
