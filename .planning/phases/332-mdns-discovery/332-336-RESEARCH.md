# Phases 332-336: VMS Architecture Integration (Final 5) - Research

**Researched:** 2026-04-07
**Domain:** mDNS, AC dedicated server multiplayer, lobby sync, session progression, track visualization, deploy verification
**Confidence:** HIGH (332-333, 336), MEDIUM (334-335)

## Summary

Phases 332-336 complete the v44.0 VMS Architecture Integration milestone. Research reveals that Phases 332 (mDNS) and 333 (MP Local Server) are 80-90% implemented already -- the core infrastructure exists and the work is about filling specific gaps. Phase 334 (Session Progression) requires understanding acServer's automatic practice/quali/race session flow, which is partially modeled but needs a new "weekend mode" coordinator. Phase 335 (Circuit Viewer) is the most novel -- it requires extracting track outlines from AC's `fast_lane.ai` binary files and rendering them with live `normalized_car_position` dots via WebSocket. Phase 336 (Deploy Verification) has extensive existing scripts but critical behavioral gaps that caused real incidents.

**Primary recommendation:** Ship 332 first (small, isolated), then 333+334 together (they share MP infrastructure), then 335 (frontend-heavy, independent), then 336 (automation, depends on all others being deployable).

---

## Phase 332: mDNS Auto-Discovery

### Status: 90% DONE -- gap is reconnect re-discovery

**What exists:**
- Server advertiser: `crates/racecontrol/src/mdns.rs` -- `start_advertiser()` registers `_racecontrol._tcp.local.` with TXT records (build_id, venue_id)
- Agent browser: `crates/rc-agent/src/mdns_discovery.rs` -- `discover_server()` browses with 5s timeout, returns `ws://IP:PORT/ws/agent`
- Both wired into `main.rs`, gated by `mdns_enabled` config (default: true)
- Library: `mdns-sd = "0.12"` (latest: 0.19, but 0.12 is working)

**What's missing (the actual Phase 332 work):**
1. **mDNS re-discovery on WS disconnect** -- `discover_server()` runs once at startup. If server IP changes, agent retries stale IP forever. Fix: after every 5 failed reconnects, re-run mDNS discovery in `spawn_blocking`.
2. **Pre-flight mDNS check** -- diagnostic that tests mDNS availability at startup, logs whether UDP 5353 is reachable.
3. Optional: mdns-sd upgrade (0.12 -> 0.19). LOW priority -- 0.12 works, upgrade introduces risk from 0.17's loopback-by-default.

**Effort:** 1-2 hours. ~50 lines of Rust added to the reconnect loop in `rc-agent/src/main.rs`.

**Key pitfalls:**
- Windows mDNS daemon conflict (dnscache on UDP 5353) -- mitigated by SO_REUSEADDR in mdns-sd
- `spawn_blocking` is required -- mDNS uses sync channels, blocking tokio runtime without it
- Windows Firewall on "Public" profile blocks UDP 5353 -- all pods should be on "Private" profile

---

## Phase 333: MP Local Server + Sync Lobby

### Status: 80% DONE -- gap is direct client launch + lobby HTTP polling

**What exists (ac_server.rs -- 1050+ lines):**
- `AcServerManager` with `start_ac_server()`, `stop_ac_server()`, health monitoring, orphan cleanup
- `generate_server_cfg_ini()` -- full INI with session blocks, weather, dynamic track, driving aids
- `generate_entry_list_ini()` -- named entries + pickup mode slots
- `PortAllocator` -- dynamic UDP/TCP/HTTP port allocation with 4-min cooldown
- `LobbyManager` (`lobby.rs`) -- Forming/AllReady/Starting/Active/Cancelled phases
- `multiplayer.rs` -- `find_adjacent_idle_pods()`, `book_multiplayer()` with wallet debit + reservations
- `monitor_continuous_session()` -- auto-restart when billing active
- Config: `acserver_path = "C:/RacingPoint/ac-server/acServer.exe"` on server .23

**What exists on the agent side (ac_launcher.rs):**
- `write_remote_section()` -- writes `[REMOTE] ACTIVE=1, SERVER_IP=..., SERVER_PORT=...` to race.ini
- Direct acs.exe launch for SP (DETACHED_PROCESS flag, no bat file)
- Test: `test_write_race_ini_multi_remote_active()` proves [REMOTE] section works

**What needs to change:**
1. **Direct acs.exe launch for MP** -- change `let use_direct_launch = !is_mp;` to `true` always. The bat file / Content Manager `acmanager://` URI path is fragile and depends on CM being installed.
2. **Fix missing `server_port` in launch JSON** -- `start_ac_server()` sends `server_http_port` but the [REMOTE] section needs `SERVER_PORT` (UDP port). The launch JSON at line 616 already has `"server_port": config.udp_port` (added in v44.0), so this may already be correct -- verify.
3. **Lobby sync via acServer HTTP API** -- Poll `GET http://127.0.0.1:{http_port}/INFO` every 3s. Parse `clients` field to count connected pods. When `clients >= expected_count`, transition lobby to AllReady.
4. **Firewall rules on server .23** -- Ensure ports 9600-9615 (UDP+TCP) and 8081-8096 (HTTP) are allowed for acServer.

**acServer.exe location:** Configured as `C:/RacingPoint/ac-server/acServer.exe` on server .23. Must verify this exists at deploy time. The server runs acServer.exe as a child process with CWD set to acServer's directory (so it can find `content/` for cars/tracks).

**acServer HTTP API (for lobby monitoring):**
- `GET /INFO` -- returns JSON with `clients` (connected count), `track`, `cars`, `session`, etc.
- `GET /ENTRY_LIST` -- returns current entries with driver names and GUIDs
- Available on the `http_port` configured in `server_cfg.ini` (dynamically allocated by PortAllocator)
- Accessible from localhost on server .23

**Key pitfalls:**
- Car/track name case mismatch between server config and client race.ini
- SERVER_PORT (UDP) vs HTTP_PORT confusion -- [REMOTE] section needs UDP port
- acServer needs AC content directory as CWD
- CTRL_CLOSE_EVENT from console hierarchy (fixed by DETACHED_PROCESS flag)
- Entry list slot count must match MAX_CLIENTS

---

## Phase 334: Follow-the-Server Session Progression

### Status: Partially modeled, needs "weekend mode" coordinator

**What exists:**
- `AcSessionBlock` type: `{ name, session_type, duration_minutes, laps, wait_time_secs }`
- `SessionType` enum: `Practice, Qualifying, Race, Hotlap`
- `AcSessionPhase` enum: `Practice, Qualifying, Race` (in rc-common types.rs)
- `AcLanSessionConfig.sessions: Vec<AcSessionBlock>` -- ordered list of session blocks
- `generate_server_cfg_ini()` already generates `[PRACTICE]`, `[QUALIFY]`, `[RACE]` sections with WAIT_TIME
- `continuous_mode` -- auto-restarts entire race when billing still active
- `LOOP_MODE=1` in server_cfg.ini -- restarts from first session after race ends

**How acServer session progression works:**
1. acServer reads `server_cfg.ini` with ordered session blocks
2. First session starts immediately (e.g., Practice)
3. When session time expires, WAIT_TIME countdown begins
4. After WAIT_TIME, next session starts automatically (Qualifying, then Race)
5. With `LOOP_MODE=1`, after race ends, it loops back to first session
6. `PICKUP_MODE_ENABLED=1` allows clients to join any active session

**What Phase 334 needs:**
1. **"Weekend Mode" API** -- Staff configures practice duration, quali duration, race laps in one action via admin dashboard. Server creates AcLanSessionConfig with all three blocks.
2. **Session phase tracking** -- Monitor acServer's current session phase. The HTTP API `/INFO` endpoint returns the active session type. Broadcast `AcSessionPhase` changes to dashboard.
3. **Mid-weekend join** -- Pods joining after the weekend started enter the current session. acServer's PICKUP_MODE handles this natively -- no additional work needed if the server is in the correct session.
4. **Dashboard UI** -- Show current phase (Practice/Qualifying/Race), time remaining, transition countdown.

**Key insight:** acServer handles session progression natively. The primary work is:
- A higher-level API endpoint for "start weekend" that configures Practice + Qualify + Race in one call
- Polling the acServer HTTP API to track current session phase
- Broadcasting phase changes to dashboards
- UI in admin dashboard to configure the weekend

**Open questions:**
- Exact JSON format of acServer `/INFO` response -- needs empirical testing on server .23
- Whether WAIT_TIME can be adjusted mid-session (likely not -- fixed at config generation time)
- How `continuous_mode` interacts with weekend mode -- they should be mutually exclusive (continuous restarts the same race; weekend progresses through sessions)

---

## Phase 335: Live Circuit Viewer (Spectator)

### Status: Novel -- needs track outline extraction + WebSocket rendering

**Data flow (already working):**
1. rc-agent reads AC shared memory at ~10Hz
2. `TelemetryFrame` includes `normalized_car_position: Option<f32>` (0.0-1.0 track spline position)
3. Agent sends `AgentMessage::Telemetry(frame)` to server via WebSocket
4. Server broadcasts `DashboardEvent::Telemetry(frame)` to all dashboard clients
5. Kiosk/spectator pages receive telemetry via `useKioskSocket` hook

**The telemetry data already available per pod:**
- `normalized_car_position` -- 0.0 to 1.0 position on track spline (AC only)
- `position: Option<Position3D>` -- world coordinates { x, y, z } (AC only, from acpmf_graphics)
- `speed_kmh`, `throttle`, `brake`, `gear`, `rpm`, `sector`
- `track` -- track name string (e.g., "monza", "spa")
- `pod_id` -- which pod this is from

**Track outline data source -- two approaches:**

### Approach A: `normalized_car_position` + precomputed track polylines (RECOMMENDED)
- Extract 2D (x, z) coordinates from AC's `fast_lane.ai` binary file for each installed track
- The `fast_lane.ai` file is a binary file with a 4x4byte header (2nd value = point count), then arrays of float x, y, z coordinates forming the track centerline
- Convert to a JSON polyline: `[{x, z}, {x, z}, ...]` normalized to a 0-1 range
- At render time, `normalized_car_position` (0.0-1.0) maps to the polyline index: `pointIndex = floor(normalized * numPoints)`
- Store precomputed polylines as static JSON assets on the server, one per track

**Pros:** Works with existing telemetry (only needs `normalized_car_position`), no world-to-pixel math at render time, consistent rendering across all clients.

**Cons:** Need to run a one-time extraction script for every installed track. `normalized_car_position` is only available for AC (not F1 25, LMU, ACE).

### Approach B: World coordinates + map.ini projection
- Use `Position3D { x, y, z }` from telemetry directly
- Each AC track has a `data/map.ini` with `[PARAMETERS]` section: `X_OFFSET`, `Z_OFFSET`, `SCALE_FACTOR`, `WIDTH`, `HEIGHT`
- Formula: `pixel_x = (world_x * SCALE_FACTOR) + X_OFFSET`, `pixel_y = (world_z * SCALE_FACTOR) + Z_OFFSET`
- Also use `map.png` from track data as the background image

**Pros:** Works with actual world coordinates. `map.png` provides a real track image.

**Cons:** Requires map.ini parameters per track. Position3D may have gaps (it's from a different shared memory page than normalized_car_position). Requires shipping map.png files (100-200KB each).

### Recommended implementation:

1. **Track outline extraction script** -- Python/Rust script that reads `fast_lane.ai` from AC content directory and outputs a JSON polyline. Run once on server .23 where AC content is installed.
2. **Track data API** -- `GET /api/tracks/{track_id}/outline` returns the polyline JSON. Cached on server.
3. **Spectator page** -- New page at `kiosk/src/app/spectator/circuit/page.tsx` (or enhance existing spectator page):
   - Canvas/SVG element with track outline polyline
   - Color-coded dots for each active pod, positioned using `normalized_car_position` interpolated on polyline
   - 10Hz update from WebSocket telemetry
   - Sidebar with driver names, lap times, gap to leader
4. **Display on spectator TV** -- 192.168.31.200, open `http://192.168.31.23:3300/kiosk/spectator/circuit`

**`fast_lane.ai` binary format:**
```
Header (16 bytes):
  int32: version/magic
  int32: point_count
  int32: unknown1
  int32: unknown2

Per point (varies by version, typically 4 floats = 16 bytes each):
  float32: x (world x coordinate)
  float32: y (world height)  
  float32: z (world z coordinate)
  float32: track_length_at_point

Total points: typically 500-5000 depending on track complexity
```

**Track outline extraction (one-time script):**
```python
import struct, json, sys

def parse_fast_lane(path):
    with open(path, 'rb') as f:
        header = struct.unpack('<4i', f.read(16))
        count = header[1]
        # Read x, z coordinates (skip y for 2D map)
        points = []
        for _ in range(count):
            x, y, z, length = struct.unpack('<4f', f.read(16))
            points.append({'x': x, 'z': z})
    # Normalize to 0-1 range
    xs = [p['x'] for p in points]
    zs = [p['z'] for p in points]
    x_min, x_max = min(xs), max(xs)
    z_min, z_max = min(zs), max(zs)
    x_range = x_max - x_min or 1
    z_range = z_max - z_min or 1
    scale = max(x_range, z_range)
    return [{
        'x': (p['x'] - x_min) / scale,
        'z': (p['z'] - z_min) / scale
    } for p in points]
```

**Rendering approach (Canvas, 10Hz):**
```typescript
// In spectator/circuit page.tsx
function drawCircuit(ctx: CanvasRenderingContext2D, outline: Point[], cars: CarPosition[]) {
  // Draw track outline
  ctx.beginPath();
  ctx.strokeStyle = '#333';
  ctx.lineWidth = 8;
  outline.forEach((p, i) => {
    const x = p.x * canvas.width;
    const y = p.z * canvas.height;
    i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
  });
  ctx.closePath();
  ctx.stroke();
  
  // Draw car dots
  cars.forEach(car => {
    const idx = Math.floor(car.normalizedPos * outline.length);
    const p = outline[idx % outline.length];
    ctx.beginPath();
    ctx.arc(p.x * canvas.width, p.z * canvas.height, 6, 0, Math.PI * 2);
    ctx.fillStyle = car.color;
    ctx.fill();
  });
}
```

**Key pitfalls:**
- `normalized_car_position` is AC-only (F1 25, LMU, ACE return None)
- `fast_lane.ai` format varies between AC versions -- header parsing must handle both old and new formats
- Track configs (e.g., "spa/gp" vs "spa/drift") may have different AI lines -- check both `tracks/{track}/ai/fast_lane.ai` and `tracks/{track}/{config}/ai/fast_lane.ai`
- Canvas rendering at 10Hz on a TV (spectator at 192.168.31.200) needs to be lightweight -- use `requestAnimationFrame`, avoid re-drawing the track outline every frame (cache it)
- Start/finish line position: the first point in `fast_lane.ai` corresponds to `normalized_car_position = 0.0`

**Existing spectator page (`kiosk/src/app/spectator/page.tsx`):**
- Already has: live timing table, speedometer, RPM bar, gear indicator, throttle/brake trace, activity feed
- Uses `useKioskSocket` for real-time data
- Has `latestTelemetry` map with `TelemetryFrame` per pod
- Does NOT currently display track position or circuit outline
- Phase 335 adds a new section/tab/page for circuit view

---

## Phase 336: Deploy Verification & E2E Automation

### Status: Scripts exist but critical behavioral gaps

**What exists:**
```
scripts/
  check-alive.sh        -- Multi-probe: ping + HTTP + SSH (per-target)
  pod-verify.sh         -- Behavioral: Session context + edge_process_count + blanking
  visual-verify.js      -- Screenshot capture + pixel analysis
  deploy-pod-agent.sh   -- Single-pod deploy with SHA256 verification
  deploy-server.sh      -- Server deploy via rc-sentry with rollback
  deploy-verify.sh      -- Post-deploy: hash table + page crawler + VR
  deploy-cloud.sh       -- Cloud deploy

tests/e2e/
  smoke.sh              -- API endpoint reachability
  deploy/verify.sh      -- Post-deploy: server health, WS, build_id, games
  api/billing.sh        -- Billing gate rejection + trial session create
  api/launch.sh         -- Per-game launch + state lifecycle
```

**Critical gaps that caused real incidents:**
1. **Session context (Console vs Services)** -- `pod-verify.sh` checks this but is NOT in the deploy pipeline
2. **edge_process_count > 0** -- today's blanking incident: 4 pods with `edge_process_count=0` after deploy
3. **lock_screen_state check** -- not automated after deploy
4. **Visual screenshot non-black** -- manual `--visual` flag only
5. **POS reachable + build_id** -- NOWHERE checked
6. **Cloud build_id parity** -- NOWHERE checked (deploy parity rule violation)
7. **Full billing lifecycle E2E** -- billing.sh and launch.sh are separate, no integrated flow

**What Phase 336 needs:**
1. **Merge pod-verify.sh behavioral checks into deploy/verify.sh** -- blanking, Session context, edge count
2. **Add deploy parity check** -- verify build_id matches across server, all 8 pods, POS, cloud
3. **Full lifecycle E2E test** -- wallet topup -> billing start -> game launch -> verify AC running -> stop -> verify refund
4. **Master post-deploy script** -- `tests/e2e/run-post-deploy.sh` that orchestrates all verification
5. **Integrate into deploy scripts** -- every `deploy-pod-agent.sh` and `deploy-server.sh` call auto-runs verification

**Target list for parity (from MEMORY.md):**
- Server .23 (racecontrol :8080)
- Pods 1-8 (rc-agent :8090, rc-sentry :8091)
- POS .20 (rc-agent :8090)
- Cloud / Bono VPS (racecontrol :8080 via pm2)

---

## Standard Stack (All Phases)

### Core
| Library/Tool | Version | Purpose | Phase |
|-------------|---------|---------|-------|
| mdns-sd | 0.12 (Rust crate) | mDNS service discovery | 332 |
| ac_server.rs | existing | AC server lifecycle management | 333, 334 |
| lobby.rs | existing | Sync lobby state machine | 333 |
| multiplayer.rs | existing | Pod selection + booking | 333 |
| port_allocator.rs | existing | Dynamic port allocation | 333 |
| reqwest | 0.12 (Rust crate) | HTTP API polling for acServer | 333, 334 |
| Next.js (kiosk) | 16 | Spectator circuit viewer | 335 |
| Canvas API | browser native | Track rendering at 10Hz | 335 |
| bash + curl | system | Deploy verification scripts | 336 |
| pod-verify.sh | existing | Behavioral pod verification | 336 |

### No New Dependencies Required
All phases can be implemented with existing workspace dependencies. No new crate additions needed.

## Architecture Patterns

### Project Structure for New Code
```
crates/racecontrol/src/
  mdns.rs                  -- existing (332: no changes needed)
  ac_server.rs             -- existing (333: add lobby HTTP polling, 334: add weekend mode)
  lobby.rs                 -- existing (333: minor additions for HTTP-based ready detection)
  multiplayer.rs           -- existing (333: no changes needed)

crates/rc-agent/src/
  mdns_discovery.rs        -- existing (332: add rediscover_server() with shorter timeout)
  main.rs                  -- existing (332: add re-discovery to reconnect loop)
  ac_launcher.rs           -- existing (333: change use_direct_launch to true for MP)

crates/rc-common/src/
  types.rs                 -- existing (334: add WeekendConfig type? or reuse AcLanSessionConfig)

kiosk/src/app/
  spectator/
    page.tsx               -- existing (335: add circuit view section or separate route)
    circuit/
      page.tsx             -- NEW (335: dedicated circuit viewer page)

scripts/
  extract-track-outlines.py -- NEW (335: one-time extraction from fast_lane.ai)

data/
  track-outlines/          -- NEW (335: JSON polylines per track)

tests/e2e/
  deploy/
    verify.sh              -- existing (336: merge behavioral checks)
    verify-parity.sh       -- NEW (336: cross-target build_id parity)
  api/
    full-lifecycle.sh      -- NEW (336: billing -> launch -> stop -> refund E2E)
  run-post-deploy.sh       -- NEW (336: master orchestrator)
```

### Pattern: WebSocket Telemetry to Canvas Rendering
```
Pod rc-agent              Server racecontrol          Kiosk/Spectator
  AC SharedMem (10Hz)  ->  AgentMessage::Telemetry  -> DashboardEvent::Telemetry
  normalized_car_pos       broadcast to dashboards     useKioskSocket receives
  speed, gear, etc.        (already working)           Canvas draws car dots
```

### Anti-Patterns to Avoid
- **Content Manager dependency for MP** -- use direct acs.exe launch (333)
- **mDNS on every reconnect attempt** -- only every 5th failure (332)
- **Polling tasklist for lobby status** -- use acServer HTTP API (333)
- **Re-drawing entire track outline every frame** -- cache to offscreen canvas (335)
- **Proxy-based deploy verification** -- check EXACT behavior (edge_process_count), not just health endpoint (336)

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| mDNS protocol | Raw UDP multicast | `mdns-sd` crate | RFC 6762/6763 compliance |
| AC server config | Custom INI generator | `generate_server_cfg_ini()` | Already handles all params |
| Port management | Manual tracking | `PortAllocator` | TIME_WAIT cooldown, bind-test |
| Track outline extraction | Manual coordinate entry | `fast_lane.ai` parser script | Binary file with 500-5000 points per track |
| Session progression | Custom state machine | acServer native progression | acServer handles Practice/Qualify/Race natively |
| Deploy verification | Manual SSH checks | Scripted verification pipeline | Humans forget steps |

## Common Pitfalls (Cross-Phase)

### Pitfall: acServer.exe Missing on Server
**What:** `start_ac_server()` fails with "binary not found"
**Cause:** AC dedicated server not installed at configured path
**Fix:** Verify `C:/RacingPoint/ac-server/acServer.exe` exists on .23 BEFORE planning depends on it
**Check:** `ssh server "ls 'C:/RacingPoint/ac-server/acServer.exe'" 2>/dev/null`

### Pitfall: Windows Firewall Blocks acServer Ports
**What:** acServer starts but no pods can connect. `clients: 0` in HTTP API.
**Cause:** Windows Firewall on server .23 blocks dynamically allocated UDP/TCP ports
**Fix:** Add firewall rules for port range 9600-9699 (UDP+TCP) and 8081-8099 (HTTP) for acServer.exe

### Pitfall: normalized_car_position Only for AC
**What:** Circuit viewer shows dots for AC players but not F1 25, LMU, ACE
**Cause:** Only the AC telemetry adapter reads `NORMALIZED_CAR_POSITION` from graphics shared memory
**Fix:** Phase 335 should gracefully degrade -- show "N/A" for non-AC sims, or use a placeholder "% complete" if available

### Pitfall: fast_lane.ai Binary Format Variations
**What:** Track outline extraction fails for some tracks
**Cause:** Newer AC versions and mod tracks may use different binary format with extra fields per point
**Fix:** Parse conservatively -- read header, validate point_count, skip extra bytes per point if file is larger than expected

### Pitfall: Deploy Parity Skipping Cloud
**What:** Venue works, cloud has stale build
**Cause:** Standing rule "DEPLOY PARITY" not enforced by scripts
**Fix:** Phase 336 master script must check ALL targets including Bono VPS

## Open Questions

1. **Does acServer.exe exist on server .23?**
   - Could not SSH to verify during research
   - Config says `C:/RacingPoint/ac-server/acServer.exe`
   - MUST verify before starting Phase 333
   - Recommendation: Check at phase start, install via Steam if missing

2. **acServer HTTP API `/INFO` response format?**
   - Need empirical data from running acServer instance
   - Expected fields: `clients`, `track`, `cars`, `session`, `timeofday`
   - Recommendation: Start acServer on .23, curl `/INFO`, document response

3. **fast_lane.ai exact binary format for installed tracks?**
   - General format known (header + float arrays)
   - Exact per-point byte count may vary
   - Recommendation: Write extraction script, test on monza/spa first, handle format variations

4. **Weekend mode vs continuous_mode interaction?**
   - continuous_mode restarts the SAME race repeatedly
   - Weekend mode progresses through sessions (Practice -> Quali -> Race)
   - They should be mutually exclusive
   - Recommendation: Add `weekend_mode: bool` to AcServerInstance, disable continuous_mode when weekend is active

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| mdns-sd crate | 332 | Already in Cargo.toml | 0.12 | TOML config fallback |
| acServer.exe | 333, 334 | UNVERIFIED on .23 | Stock Kunos | Must install via Steam |
| AC content (cars/tracks) | 333, 334, 335 | On server .23 | Steam install | Required |
| reqwest | 333, 334 | Already in Cargo.toml | 0.12 | N/A |
| Next.js 16 (kiosk) | 335 | Running on .23:3300 | 16 | N/A |
| Canvas API | 335 | All browsers | native | SVG fallback |
| bash/curl | 336 | James .27 + server .23 | system | N/A |
| ssh access to all targets | 336 | Via Tailscale | working | N/A |
| Spectator TV | 335 | 192.168.31.200 | WiFi connected | Any browser |

**Missing with no fallback:**
- acServer.exe on server .23 -- MUST verify/install before Phase 333

**Missing with fallback:**
- `normalized_car_position` for non-AC sims -- gracefully degrade in circuit viewer

## Dependency Graph

```
332 (mDNS)         -- independent, ship first
333 (MP Server)    -- independent, can run parallel with 332
334 (Progression)  -- depends on 333 (needs running acServer)
335 (Circuit View) -- independent (uses existing telemetry), but 330 is soft prereq
336 (Deploy E2E)   -- should be last (verifies all other phases)
```

## Effort Estimates

| Phase | Effort | Risk | Complexity |
|-------|--------|------|------------|
| 332 mDNS re-discovery | 1-2 hours | LOW | ~50 lines Rust |
| 333 MP direct launch + lobby | 4-6 hours | MEDIUM | ac_launcher.rs change + lobby HTTP polling |
| 334 Weekend mode | 3-4 hours | MEDIUM | New API endpoint + session phase tracking |
| 335 Circuit viewer | 6-8 hours | MEDIUM | Track extraction + Canvas rendering + new page |
| 336 Deploy verification | 4-6 hours | LOW | Script integration + parity checks + E2E test |

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/mdns.rs` -- server mDNS advertiser
- `crates/rc-agent/src/mdns_discovery.rs` -- agent mDNS browser
- `crates/racecontrol/src/ac_server.rs` -- AC server lifecycle (1050+ lines)
- `crates/racecontrol/src/lobby.rs` -- lobby state machine
- `crates/racecontrol/src/multiplayer.rs` -- MP booking + pod selection
- `crates/rc-agent/src/sims/assetto_corsa.rs` -- shared memory offsets including NORMALIZED_CAR_POSITION at offset 248
- `crates/rc-common/src/types.rs` -- TelemetryFrame, AcLanSessionConfig, LobbyPhase, AcSessionBlock
- `crates/rc-common/src/protocol.rs` -- AgentMessage::Telemetry, DashboardEvent::Telemetry
- `kiosk/src/app/spectator/page.tsx` -- existing spectator UI (600+ lines)
- `kiosk/src/hooks/useKioskSocket.ts` -- WebSocket data hook
- `scripts/deploy-server.sh` -- server deploy script
- `scripts/deploy-verify.sh` -- frontend deploy verification
- `scripts/pod-verify.sh` -- behavioral pod verification
- `tests/e2e/deploy/verify.sh` -- post-deploy verification

### Secondary (MEDIUM confidence)
- [Hagn's Site - map.ini](https://sites.google.com/site/hagn99/assettocorsa/modding/tracks/data/map-ini) -- AC map.ini parameter format
- [AC fast AI Line guide](https://www.overtake.gg/downloads/ac-fast-ai-line-guide.59546/) -- fast_lane.ai format overview
- [Blender Addon for AC AI files](https://github.com/leBluem/io_import_accsv) -- fast_lane.ai parser reference
- [AI Line File Format](https://www.overtake.gg/threads/ai-line-file-format.194906/) -- community format documentation
- Kunos AC Dedicated Server Manual -- server_cfg.ini parameters, session blocks, PICKUP_MODE

### Tertiary (LOW confidence)
- acServer HTTP API `/INFO` response format -- needs empirical verification
- fast_lane.ai binary format variations across track mods -- needs testing per track
- Direct acs.exe MP connect without Content Manager -- bat file fallback proves it works, but edge cases unknown

## Metadata

**Confidence breakdown:**
- Phase 332 (mDNS): HIGH -- existing code fully reviewed, gap clearly identified
- Phase 333 (MP Server): HIGH -- 80% existing, remaining work is well-scoped
- Phase 334 (Progression): MEDIUM -- acServer behavior needs empirical verification
- Phase 335 (Circuit Viewer): MEDIUM -- track outline extraction format needs testing
- Phase 336 (Deploy E2E): HIGH -- gaps identified from real incidents, fix path clear

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable domain, AC server binary hasn't changed in years)
