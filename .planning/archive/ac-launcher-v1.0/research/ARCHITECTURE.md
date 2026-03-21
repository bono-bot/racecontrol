# Architecture Patterns

**Domain:** Assetto Corsa launch, session management, multiplayer orchestration, and billing sync for Racing Point eSports (8 sim pods)
**Researched:** 2026-03-13
**Confidence:** HIGH (based entirely on existing codebase analysis -- no speculation)

## Executive Summary

The existing racecontrol codebase already has a well-structured three-tier architecture (rc-agent on pods, rc-core on server, dashboards via WebSocket). The AC launch, multiplayer, billing, and telemetry subsystems are **already implemented** with substantial code. This milestone is not greenfield -- it is about filling gaps, fixing known issues (billing-starts-during-loading, invalid combo filtering, difficulty tiers), and wiring together pieces that exist but are not yet integrated end-to-end.

The architecture does NOT need restructuring. The existing component boundaries, message protocol, and data flow patterns are sound. What is needed is:

1. **Billing sync improvement** -- billing resets when GameState::Running, but DirectX init delay means Running != actually driving
2. **Session config enrichment** -- difficulty tiers, AI opponents, valid combo filtering on top of existing AcLaunchParams
3. **Multiplayer orchestration completion** -- the flow exists (book_multiplayer -> on_member_validated -> start_ac_lan_for_group) but needs testing and the PWA/kiosk UX around it
4. **Mid-session controls** -- transmission and FFB already have set_transmission()/set_ffb(), need to wire to the overlay/PWA UI

## System Architecture (As-Is)

```
+------------------+     +------------------+     +------------------+
|  Customer PWA    |     |  Staff Kiosk     |     |  Admin Dashboard |
|  (Next.js)       |     |  (Next.js)       |     |  (Next.js)       |
+--------+---------+     +--------+---------+     +--------+---------+
         |                         |                         |
         |  HTTP/WS (cloud sync)   |  WS (dashboard)        |  WS (dashboard)
         v                         v                         v
+--------+--------------------------------------------------------+---------+
|                           rc-core (Server .23, port 8080)                  |
|  +-------------+ +------------+ +----------------+ +-----------+          |
|  | GameManager | | Billing    | | AcServerMgr    | | Multiplayer|          |
|  | (game_      | | Manager    | | (ac_server.rs) | | (multi-   |          |
|  |  launcher)  | | (billing)  | |                | |  player)  |          |
|  +------+------+ +------+-----+ +-------+--------+ +-----+----+          |
|         |               |                |                |               |
|  +------+------+ +------+-----+ +-------+--------+ +-----+-----+        |
|  | PortAlloc   | | CloudSync  | | Catalog        | | PodMonitor |        |
|  +-------------+ +------------+ +----------------+ +-----------+         |
+-----------+-------------------------------------------+------------------+
            | WebSocket (agent protocol)                |
            v                                           v
+-----------+-----------+               +---------------+-----------+
|  rc-agent (Pod 1-8)   |               |  AC Dedicated Server     |
|  port 8080 each       |               |  (.23, dynamic ports)    |
|  +-----------------+  |               |  acServer.exe            |
|  | AcLauncher      |  |               |  (server_cfg.ini +       |
|  | (ac_launcher.rs) | |               |   entry_list.ini)        |
|  +-----------------+  |               +--------------------------+
|  | GameProcess      |  |
|  | (game_process.rs)|  |
|  +-----------------+  |
|  | SimAdapter (AC)  |  |     +---------------------------+
|  | (sims/assetto_   |  |     | pod-agent (port 8090)     |
|  |  corsa.rs)       |  |     | Remote exec/deploy only   |
|  +-----------------+  |     +---------------------------+
|  | DrivingDetector  |  |
|  | (HID+UDP)        |  |
|  +-----------------+  |
|  | Overlay (Edge)   |  |
|  | Lock Screen      |  |
|  +-----------------+  |
+-----------------------+
```

### Component Boundaries

| Component | Location | Responsibility | Communicates With |
|-----------|----------|---------------|-------------------|
| **rc-agent** | Each pod (Windows, port 8080) | Game launch, process monitoring, telemetry reading, lock screen, overlay, HID driving detection, FFB control | rc-core (WebSocket), AC shared memory, OpenFFBoard USB |
| **rc-core** | Server .23 (port 8080) | Central management: billing timers, game state tracking, AC server lifecycle, multiplayer coordination, auth, catalog, pod monitoring | rc-agent (WebSocket), dashboards (WebSocket), SQLite DB, cloud sync |
| **AC Dedicated Server** | Server .23 (dynamic ports via PortAllocator) | Hosts multiplayer sessions | AC clients on pods (UDP/TCP/HTTP) |
| **Kiosk/PWA** | Next.js frontends | Staff: configure sessions, manage billing. Customer: select experience, enter PIN | rc-core (WebSocket/HTTP) |
| **pod-agent** | Each pod (port 8090) | Remote command execution, binary deployment | rc-core (HTTP), James workstation |
| **Catalog** | rc-core (catalog.rs, static arrays) | 36 tracks, 325+ cars with categories | Kiosk/PWA via API |

## Data Flows

### Flow 1: Single-Player Session (Customer via PWA)

```
Customer --> PWA --> rc-core --> rc-agent --> AC

1. Customer scans QR on pod -> PWA opens
2. Customer selects: car + track + difficulty + session type
3. PWA sends booking via CloudAction::BookingCreated -> rc-core action queue
4. rc-core creates auth_token, sends ShowPinLockScreen to rc-agent
5. Customer enters 4-digit PIN on pod lock screen
6. rc-agent sends AgentMessage::PinEntered -> rc-core validates
7. rc-core starts BillingTimer (Active), sends:
   - BillingStarted to rc-agent
   - LaunchGame { sim_type: AC, launch_args: JSON } to rc-agent
8. rc-agent:
   a. Parses AcLaunchParams from launch_args JSON
   b. Kills existing AC
   c. Writes race.ini + assists.ini + apps preset
   d. Sets FFB strength
   e. Launches acs.exe directly (single-player)
   f. Waits for AC, minimizes Conspit, foregrounds game
   g. Reports GameStateUpdate { Running, pid } -> rc-core
9. rc-core: On GameState::Running for AC:
   - Resets billing driving_seconds to 0 (AC timer sync)
   - This is the BILLING START MOMENT
10. rc-agent SimAdapter (shared memory):
    - Reads telemetry (speed, throttle, brake, gear, RPM)
    - Tracks sector times and lap completions
    - Sends Telemetry + LapCompleted frames -> rc-core
11. rc-core BillingManager:
    - Ticks every 1s, counts driving_seconds
    - Sends BillingTick -> rc-agent (overlay countdown)
    - At 5min/1min remaining: sends warning
12. Session end (time up OR staff StopGame):
    - rc-core sends SessionEnded -> rc-agent
    - rc-agent calls cleanup_after_session()
    - Kills AC, minimizes windows, shows lock screen
```

### Flow 2: Multiplayer Session (Group via PWA)

```
Host --> PWA --> rc-core --> AC Server + all pod agents

1. Host books multiplayer:
   - POST /api/multiplayer/book with friend_ids + experience
   - rc-core::multiplayer::book_multiplayer():
     a. Validates friends, pricing, wallet
     b. find_adjacent_idle_pods(N) -- prefers adjacent pod numbers
     c. Creates group_sessions + group_session_members rows
     d. Generates shared 4-digit PIN
     e. Debits host wallet
     f. Creates pod reservations + auth tokens for host
     g. Shows lock screen with shared PIN on host pod
     h. Creates 'pending' invitee records

2. Invitees accept:
   - Each friend calls accept_group_invite()
   - Debits their wallet, creates reservation + auth token
   - Shows lock screen with SAME shared PIN on their pod

3. Members validate (enter shared PIN):
   - Each member enters PIN -> PinEntered -> rc-core validates
   - rc-core::multiplayer::on_member_validated()
   - If NOT all validated: shows "Waiting for friends... (2/3 checked in)"
   - If ALL validated: triggers start_ac_lan_for_group()

4. AC LAN Session Start:
   a. rc-core builds AcLanSessionConfig:
      - Track/car from experience
      - max_clients = member count
      - entries = one per validated member
      - pickup_mode = true
   b. rc-core::ac_server::start_ac_server():
      - PortAllocator assigns unique UDP/TCP/HTTP ports
      - Generates server_cfg.ini + entry_list.ini
      - Spawns acServer.exe with --config pointing to session dir
      - Stores AcServerInstance in memory + DB
   c. For each pod:
      - Sends LaunchGame { sim_type: AC, launch_args: join_url }
      - join_url = "acmanager://race/online/join?ip=X&httpPort=Y"
   d. rc-agent on each pod:
      - Detects multiplayer mode (launch_args starts with "acmanager://")
      - Writes race.ini with [REMOTE] ACTIVE=1
      - Launches via Content Manager URI (handles server join handshake)
      - Falls back to direct acs.exe if CM fails

5. During session:
   - All pods connected to same AC dedicated server
   - Each rc-agent reads local shared memory (per-pod telemetry)
   - Billing runs independently on each pod (same timer sync)

6. Session end:
   - rc-core stops AC server via stop_ac_server()
   - Kills acServer.exe process
   - Sends StopGame to all assigned pods
   - Releases ports (enter 4-min cooldown for TIME_WAIT)
```

### Flow 3: Billing Timer Sync (The Core Problem)

```
CURRENT (with known issue):

  T+0s:  LaunchGame sent to agent
  T+2s:  Agent kills old AC
  T+4s:  Agent writes race.ini
  T+5s:  Agent spawns acs.exe
  T+5-20s: DirectX initialization (black screen, loading)
  T+20s: AC window appears
  T+25s: Agent reports GameState::Running
  T+25s: rc-core resets billing driving_seconds to 0  <-- SYNC POINT

  GAP: The "AC timer sync" in game_launcher.rs only fires when:
    - GameState::Running AND sim_type == AssettoCorsa
    - AND driving_seconds < 120 (initial launch only)

  PROBLEM: GameState::Running means acs.exe PID exists, NOT that the
  customer is actually on track. AC still has:
    - Splash screen / CSP loading
    - Track loading screen
    - Pit loading
  These can take 5-30 additional seconds after PID appears.

PROPOSED IMPROVEMENT:

  Use the SimAdapter (shared memory) to detect actual gameplay:
  - acpmf_graphics::STATUS == 2 (LIVE) means on-track
  - STATUS 0 = OFF, 1 = REPLAY, 3 = PAUSE

  New state machine:
  T+0s:  LaunchGame -> billing timer starts but PAUSED
  T+25s: GameState::Running -> billing stays paused
  T+30-50s: SimAdapter detects STATUS=LIVE -> NOW start billing

  This requires:
  1. New AgentMessage variant: GameplayStarted { pod_id, session_time_ms }
  2. rc-core billing sync listens for GameplayStarted instead of GameState::Running
  3. rc-agent polls shared memory STATUS field in its main loop
```

### Flow 4: Mid-Session Controls

```
EXISTING (already implemented in rc-agent):
  - set_transmission(transmission: &str) -> rewrites race.ini AUTO_SHIFTER
  - set_ffb(preset: &str) -> rewrites controls.ini [FF] GAIN

EXISTING protocol messages:
  - CoreToAgentMessage::SetTransmission { transmission }
  - CoreToAgentMessage::SetFfb { preset }

WHAT IS MISSING:
  - PWA/Overlay UI to trigger these controls
  - DashboardCommand variants for mid-session control
  - API endpoints on rc-core to forward to agent

  Data flow:
  Customer (overlay) -> rc-agent HTTP -> set_transmission()/set_ffb()
  OR
  Customer (PWA) -> rc-core API -> CoreToAgentMessage::Set* -> rc-agent
```

## Recommended Architecture Changes

### Change 1: Difficulty Tier Mapping (rc-common + rc-core + rc-agent)

**Where it lives:** rc-common::types as a new DifficultyTier enum, mapped to AC params in rc-agent.

```
DifficultyTier -> AcLaunchParams mapping:

| Tier | AI Level | AI Aggression | Aids (ABS/TC/SC) | Ideal Line |
|------|----------|---------------|-------------------|------------|
| Rookie    | 70  | 0 (calm)     | ABS=1 TC=1 SC=1  | 1 (on)     |
| Amateur   | 80  | 30           | ABS=1 TC=1 SC=0  | 0 (off)    |
| SemiPro   | 90  | 60           | ABS=1 TC=0 SC=0  | 0          |
| Pro       | 95  | 80           | ABS=0 TC=0 SC=0  | 0          |
| Alien     | 100 | 100          | ABS=0 TC=0 SC=0  | 0          |
```

AI Level and AI Aggression map to race.ini fields:
- `AI_LEVEL` in `[RACE]` section
- `AI_AGGRESSION` -- needs to be added to write_race_ini() (AC supports this in race.ini)

The tier should be a field in AcLaunchParams and resolved to concrete values in rc-agent before writing race.ini. NOT resolved in rc-core -- keep game-specific knowledge in the agent.

### Change 2: Session Type Support (Single-Player Race vs Practice)

**Current state:** write_race_ini() hardcodes `[SESSION_0]` as Practice with TYPE=1.

**Needed:** Support for race sessions with AI grid:
- Practice (TYPE=1): solo, no AI, timed duration
- Race (TYPE=3): grid with AI opponents, lap-based or timed
- Qualifying (TYPE=2): timed, solo on track with ghost or AI

For Race mode, race.ini needs:
- `[RACE] CARS=N` where N = player + AI count
- `[CAR_1]` through `[CAR_N-1]` sections for AI drivers
- AI_LEVEL per the difficulty tier

This is a significant change to write_race_ini() -- it goes from 1 car to N cars.

### Change 3: Valid Combo Filtering (rc-core catalog)

**Where:** rc-core::catalog.rs currently has static arrays of tracks and cars.

**Needed:** A validation layer that checks:
1. Track + car compatibility (some tracks require specific car categories)
2. Session type + mode compatibility (e.g., can't do AI race on all tracks)
3. Track + track_config validity (e.g., "spa" has configs like "spa-gp", "spa-endurance")

This should be a **server-side filter** (rc-core) so both PWA and kiosk get the same validated options. The catalog API should accept a partial selection and return only valid remaining options:
- GET /api/catalog/tracks -> all tracks
- GET /api/catalog/cars?track=spa -> cars valid for spa
- GET /api/catalog/sessions?track=spa&car=ks_ferrari_488_gt3 -> valid session types

**Approach:** AC content validation requires reading the actual content folders on the pods (or a pre-scanned manifest). Since all 8 pods have identical AC installs, scanning one pod's content/ directory once and caching the manifest in rc-core is sufficient.

### Change 4: Billing Sync via Shared Memory (rc-agent)

**Where:** rc-agent SimAdapter already reads AC shared memory.

**What to add:**
1. In the AC SimAdapter, check `graphics::STATUS` field:
   - 0 = OFF, 1 = REPLAY, 2 = LIVE (on-track), 3 = PAUSE
2. New signal from agent to core: `GameplayStarted` (STATUS transitioned to LIVE)
3. rc-core billing sync reacts to `GameplayStarted` instead of `GameState::Running`

This is the cleanest solution because:
- The shared memory is already being read (SimAdapter is connected during gameplay)
- No polling overhead -- the STATUS check happens in the existing telemetry read loop
- Handles all loading delays (DirectX, track loading, pit loading)
- Also handles race restarts / session transitions (STATUS goes PAUSE -> LIVE)

### Change 5: Preset Combos (rc-core)

**Where:** rc-core already has ac_presets table and save/load/list_presets().

**What to add:** A new `kiosk_presets` concept that is customer-facing (curated packages like "Spa GT3 Race" or "Nurburgring Hotlap") as opposed to `ac_presets` which are staff-facing server configurations.

The existing `kiosk_experiences` table already serves this role. Extend it with:
- difficulty_tier field
- session_type field (practice/race/qualifying)
- ai_count field (number of AI opponents for race mode)
- is_popular flag (for featured/promoted experiences)

## Anti-Patterns to Avoid

### Anti-Pattern 1: Game-Specific Logic in rc-core
**What:** Putting AC-specific ini generation or shared memory offsets in rc-core.
**Why bad:** rc-core should be game-agnostic. When F1/iRacing are added, rc-core changes should be zero.
**Instead:** rc-core deals with abstract concepts (GameState, SimType, launch_args JSON). rc-agent translates to game-specific actions. The existing architecture already follows this -- maintain it.

### Anti-Pattern 2: Polling AC Server HTTP API for Player Count
**What:** Using the AC dedicated server's HTTP info API to detect connected players.
**Why bad:** Unreliable (server HTTP sometimes hangs), adds network dependency, creates coupling.
**Instead:** Use the agent-reported GameState + shared memory STATUS. Each agent knows its own pod's state. rc-core aggregates.

### Anti-Pattern 3: Billing Timer in rc-agent
**What:** Running the billing countdown timer on the agent side.
**Why bad:** Agent crashes lose billing state. Multiple agents with local timers create sync issues. Cloud sync only works from rc-core.
**Instead:** Keep billing timer in rc-core (as it is now). Agent only displays what rc-core tells it via BillingTick messages. This is the correct architecture.

### Anti-Pattern 4: Single-Point-of-Failure AC Server
**What:** Running one AC dedicated server that handles all multiplayer sessions simultaneously.
**Why bad:** acServer.exe on Windows is single-instance per configuration. Port conflicts. One crash takes down all sessions.
**Instead:** Use the existing PortAllocator pattern -- each multiplayer session gets its own acServer process with unique ports. Already implemented correctly.

## Component Interaction Matrix

| Action | PWA | Kiosk | rc-core | rc-agent | AC Server |
|--------|-----|-------|---------|----------|-----------|
| Select car/track | R | R | Serves catalog API | - | - |
| Start billing | - | W | Creates timer | Displays countdown | - |
| Launch SP game | - | W | Sends LaunchGame | Writes ini, spawns acs.exe | - |
| Launch MP game | R/W | W | Starts AC server, sends LaunchGame | Joins via CM URI | Hosts session |
| Read telemetry | - | R (dashboard) | Aggregates, broadcasts | Reads shared memory | - |
| Track laps | - | R | Stores in DB | Detects via shared memory | - |
| Set transmission | R/W | W | Forwards to agent | Rewrites race.ini | - |
| Set FFB | R/W | W | Forwards to agent | Rewrites controls.ini | - |
| End session | - | W | Stops timer, sends SessionEnded | Kills AC, shows lock screen | - |
| Stop MP | - | W | Stops AC server + all pods | Kills AC | Process killed |

R = Read, W = Write/Trigger

## Scalability Considerations

| Concern | Current (8 pods) | At 16 pods | At 32+ pods |
|---------|-------------------|------------|-------------|
| WebSocket connections | 8 agents + dashboards | 16 agents | Shard rc-core or use message broker |
| Billing timer ticks | 8/sec max | 16/sec | Still fine (tick_all_timers is O(n)) |
| Telemetry bandwidth | ~400 bytes/pod * 10Hz = 32KB/s | 64KB/s | Consider sampling / aggregation |
| AC Server processes | 1-2 simultaneous (8 pods / 4 per race) | 2-4 | Server .23 has 64GB RAM, should handle 4-6 |
| PortAllocator range | 8 slots (9600-9607 UDP/TCP, 8081-8088 HTTP) | 16 slots | Extend range |
| SQLite DB | Fine for 8 pods | Fine | Consider PostgreSQL at 32+ |

## Suggested Build Order (Dependencies)

The existing codebase has all the infrastructure. New features should be built in this order based on dependency chains:

### Layer 1: Data Model + Catalog (no runtime dependencies)
1. **DifficultyTier enum** in rc-common::types
2. **Session type** extensions to AcLaunchParams (race mode, AI count)
3. **Valid combo validation** in rc-core catalog (build the filter API)
4. **Kiosk preset enrichment** (difficulty, session type, AI count fields)

### Layer 2: Agent Launch Logic (depends on Layer 1 types)
5. **write_race_ini() expansion** -- support race mode with AI grid
6. **Difficulty tier mapping** in rc-agent (tier -> AI_LEVEL + aids)
7. **Gameplay detection** via shared memory STATUS field
8. **New AgentMessage::GameplayStarted** variant

### Layer 3: Billing Sync (depends on Layer 2 gameplay detection)
9. **Billing sync to GameplayStarted** instead of GameState::Running
10. **Billing pause during loading** (optional -- could just delay start)

### Layer 4: Frontend Integration (depends on Layers 1-3)
11. **Kiosk session configuration UI** (difficulty picker, session type, car/track with filtering)
12. **PWA customer experience selection** (filtered options, difficulty)
13. **Mid-session controls** in overlay (transmission, FFB, assists)
14. **Multiplayer lobby UX** in PWA (invite friends, waiting room)

### Layer 5: Polish + Edge Cases
15. **Popular presets** (curated packages visible in PWA/kiosk)
16. **Session transition handling** (what happens when AC race finishes before billing)
17. **Error recovery** (what if AC server dies mid-multiplayer session)
18. **Multiplayer billing sync** (all pods start/end billing together)

## AC-Specific Technical Details

### race.ini Generation (Current vs Needed)

**Current** (`ac_launcher.rs::write_race_ini`):
- Single car (`[CAR_0]` only)
- Practice session only (`[SESSION_0]` TYPE=1)
- `[RACE] CARS=1`
- `[REMOTE] ACTIVE=0/1` for single/multi

**Needed for Race with AI:**
```ini
[RACE]
AI_LEVEL=90
CARS=8          ; 1 player + 7 AI
MODEL=ks_ferrari_488_gt3
TRACK=monza
...

[CAR_0]
MODEL=ks_ferrari_488_gt3
SKIN=00_default
DRIVER_NAME=Customer
AI=0            ; Human player

[CAR_1]
MODEL=ks_ferrari_488_gt3
SKIN=01_red
DRIVER_NAME=AI Driver 1
AI=1

; ... CAR_2 through CAR_7
```

### Content Manager Integration Points

| Method | When Used | Protocol |
|--------|-----------|----------|
| Direct acs.exe | Single-player (practice, race with AI) | Write race.ini, spawn acs.exe |
| CM URI acmanager://race/config | Single-player via CM (not used -- "Settings not specified" error) | URI launch |
| CM URI acmanager://race/online/join | Multiplayer (join AC server) | URI launch with ip+httpPort params |

**Decision:** Keep using direct acs.exe for single-player. CM is only needed for multiplayer server join handshake. This is already the correct approach in the codebase.

### AC Shared Memory Fields Used

| Field | Source | Offset | Purpose |
|-------|--------|--------|---------|
| STATUS | graphics | 4 | Detect LIVE (2) = on-track for billing sync |
| SPEED_KMH | physics | 28 | Telemetry display |
| GAS/BRAKE | physics | 4/8 | Driving detection backup |
| GEAR | physics | 16 | HUD display |
| RPMS | physics | 20 | RPM bar |
| COMPLETED_LAPS | graphics | 132 | Lap counting |
| I_LAST_TIME | graphics | 144 | Lap time recording |
| CURRENT_SECTOR_INDEX | graphics | 164 | Sector time tracking |
| LAST_SECTOR_TIME | graphics | 168 | Sector splits |
| CAR_MODEL | statics | 68 | Session info |
| TRACK | statics | 134 | Session info |

### Safety Enforcement

Non-negotiable presets that must ALWAYS be applied:
- `DAMAGE=0` in race.ini [ASSISTS] and assists.ini -- hardware protection
- `[DYNAMIC_TRACK] SESSION_START=100` -- full grip from start
- `DAMAGE_MULTIPLIER=0` on AC dedicated server for multiplayer
- `STABILITY_ALLOWED=0` on server (overridable per difficulty tier on client)

These are currently enforced in `write_race_ini()` and `generate_server_cfg_ini()`. Any changes to difficulty tiers must NOT touch damage -- it stays at 0.

## Sources

- **rc-agent/src/ac_launcher.rs** -- 987 lines, full AC launch sequence
- **rc-core/src/game_launcher.rs** -- 455 lines, game state management + billing sync
- **rc-core/src/multiplayer.rs** -- 1010 lines, complete multiplayer booking flow
- **rc-core/src/ac_server.rs** -- 832 lines, AC dedicated server lifecycle
- **rc-core/src/billing.rs** -- 67KB, billing timer system
- **rc-common/src/types.rs** -- 830 lines, all shared types
- **rc-common/src/protocol.rs** -- 943 lines, all WebSocket messages
- **rc-agent/src/sims/assetto_corsa.rs** -- AC shared memory reader
- **rc-agent/src/driving_detector.rs** -- HID + UDP driving state detection
- **rc-agent/src/game_process.rs** -- Generic game process management
- **rc-core/src/catalog.rs** -- 36 tracks, 325+ cars with categories
- **rc-core/src/port_allocator.rs** -- Dynamic port allocation for AC server sessions
- **AC Shared Memory Reference** -- https://www.assettocorsa.net/forum/index.php?threads/shared-memory-reference.3352/ (HIGH confidence, referenced in codebase)
