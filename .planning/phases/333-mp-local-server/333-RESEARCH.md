# Phase 333: MP Local Server + Sync Lobby - Research

**Researched:** 2026-04-07
**Domain:** Assetto Corsa dedicated server management, multiplayer client connection, lobby synchronization
**Confidence:** HIGH

## Summary

The existing codebase already has approximately 90% of the infrastructure needed for this phase. The `ac_server.rs` module in the racecontrol crate contains a fully functional `AcServerManager` with `generate_server_cfg_ini()`, `generate_entry_list_ini()`, `start_ac_server()`, `stop_ac_server()`, port allocation, continuous mode, health monitoring, preset management, and result collection. The `AcLanSessionConfig` struct in `rc-common/types.rs` already models session blocks (Practice/Qualify/Race), entry slots, weather, dynamic track, and server settings.

The critical gap is the **client-side connection path**: the current MP launch flow uses Content Manager's `acmanager://race/online?ip=...&httpPort=...` URI protocol, which depends on CM being installed and is fragile. The direct-connect alternative -- writing `race.ini` with `[REMOTE] ACTIVE=1, SERVER_IP=..., SERVER_PORT=...` and launching `acs.exe` directly -- is already partially implemented (the `write_remote_section()` function exists and is tested) but the MP launch path still routes through CM or the bat file. For SP, direct `acs.exe` launch already works (commit `d616ee10`).

The second gap is a **sync lobby** -- holding all pods at "Waiting for players" until N pods have connected. The stock acServer PICKUP_MODE allows clients to join anytime, but there is no built-in "hold start until N players" mechanism. This must be implemented server-side by monitoring acServer's HTTP API (`/INFO` returns connected car count) and using WAIT_TIME on session blocks to control progression.

**Primary recommendation:** Eliminate the Content Manager dependency for MP by using direct `acs.exe` launch with pre-written `race.ini [REMOTE]` section (same pattern as SP launch). Use the already-implemented server infrastructure in `ac_server.rs`. Add a lobby sync coordinator that polls acServer HTTP API to track pod connections.

## Standard Stack

### Core (Already in Codebase)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `ac_server.rs` | current | AC dedicated server lifecycle | Already generates configs, spawns acServer, manages ports |
| `ac_launcher.rs` | current | Client-side AC launch | Already writes race.ini with REMOTE section |
| `port_allocator.rs` | current | Dynamic port allocation for sessions | 4-min TIME_WAIT cooldown, bind-test verification |
| `rc-common::types` | current | `AcLanSessionConfig`, `AcEntrySlot`, `AcSessionBlock` | All structs defined and serializable |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `reqwest` | existing | Poll acServer HTTP API for lobby status | Already in deps |
| `tokio` | existing | Async lobby monitor task | Already used throughout |

### No New Dependencies Needed
The entire feature can be implemented using existing crate dependencies. No new Cargo additions required.

## Architecture Patterns

### Existing Infrastructure Map

```
Server (.23) - racecontrol binary
  ac_server.rs
    generate_server_cfg_ini()     -- DONE: generates full server_cfg.ini
    generate_entry_list_ini()     -- DONE: generates entry_list.ini with named or pickup slots
    start_ac_server()             -- DONE: port alloc, config write, spawn acServer, send LaunchGame to pods
    stop_ac_server()              -- DONE: kill process, release ports, send StopGame to pods
    monitor_continuous_session()  -- DONE: auto-restart race when billing still active
    check_ac_server_health()      -- DONE: poll process alive, broadcast dead sessions
    retry_pod_join()              -- DONE: re-send LaunchGame to a single pod
    save/load/list/delete_preset()-- DONE: preset CRUD in SQLite
    collect_results()             -- DONE: parse race results from acServer output
    parse_ac_results()            -- DONE: read results JSON files

  port_allocator.rs
    allocate()                    -- DONE: unique UDP/TCP/HTTP triple per session
    release()                     -- DONE: 4-min cooldown

  multiplayer.rs
    find_adjacent_idle_pods()     -- DONE: picks N consecutive idle pods
    book_multiplayer()            -- DONE: reservation + AC server + billing

Pod (each) - rc-agent binary
  ac_launcher.rs
    write_remote_section()        -- DONE: [REMOTE] ACTIVE=1, SERVER_IP, SERVER_PORT
    launch_ac()                   -- DONE (SP direct), NEEDS CHANGE (MP still uses CM bat)
    build_race_ini_string()       -- DONE: full race.ini generation including REMOTE
```

### What Needs to Change

```
1. AGENT SIDE: Direct acs.exe launch for MP (eliminate CM dependency)
   ac_launcher.rs: change `let use_direct_launch = !is_mp;` to `true` always
   (race.ini [REMOTE] section already provides server connection info)

2. SERVER SIDE: Lobby sync coordinator (NEW)
   ac_server.rs: add lobby_monitor() that polls acServer HTTP API
   Track: expected pods vs connected pods
   Signal "all ready" when count matches
   Dashboard event for lobby status

3. SERVER CONFIG: Adjust WAIT_TIME for lobby sync
   Longer WAIT_TIME on first session block = natural hold point
   Server transitions when WAIT_TIME expires OR manual trigger
```

### Pattern: Direct Client Connection (Eliminating CM)

**What:** AC client (`acs.exe`) reads `race.ini` at launch. When `[REMOTE] ACTIVE=1` with `SERVER_IP` and `SERVER_PORT` set, AC connects to that server on startup. This is the same mechanism CM uses internally -- CM just writes race.ini and launches acs.exe.

**Evidence from codebase:**
- `write_remote_section()` already writes the correct format (line 1174)
- Test `test_write_race_ini_multi_remote_active()` proves the section is generated correctly
- `launch-ac.bat` line 54-57: MP fallback path already launches acs.exe directly when CM not found
- SP direct launch (line 538) already uses the same `acs.exe` spawn with `DETACHED_PROCESS`

**Key insight:** The `[REMOTE]` section uses `SERVER_PORT` (UDP port, default 9600), NOT `HTTP_PORT`. The current code sends `server_http_port` to the agent, which stores it in `AcLaunchParams.server_http_port`. But `write_remote_section()` writes `SERVER_PORT={params.server_port}` -- the UDP port. The server's `start_ac_server()` sends `server_http_port` but NOT `server_port` (UDP) in the launch JSON (line 616). This needs fixing: the launch JSON must also include `server_port` (= `config.udp_port` from allocated ports).

**Code example:**
```rust
// Server sends launch command (ac_server.rs line 610-618)
let launch_json = serde_json::json!({
    "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
    "track": &config.track,
    "track_config": &config.track_config,
    "game_mode": "multi",
    "server_ip": &lan_ip,
    "server_port": config.udp_port,      // <-- MISSING, must add
    "server_http_port": config.http_port,
    "server_password": &config.password,
    "session_type": "race",
});
```

### Pattern: acServer HTTP API for Lobby Monitoring

**What:** Stock acServer exposes an HTTP API on `HTTP_PORT` (default 8081). `GET /INFO` returns server status including connected car count, track, cars, session info. `GET /ENTRY_LIST` returns current entry list with driver names and GUIDs.

**Use for lobby sync:** Poll `GET http://<server_ip>:<http_port>/INFO` every 2-3 seconds. Parse `clients` field to track how many pods have connected. When `clients == expected_count`, signal "lobby ready."

**Code example:**
```rust
async fn poll_lobby_status(http_port: u16, expected_pods: usize) -> bool {
    let url = format!("http://127.0.0.1:{}/INFO", http_port);
    if let Ok(resp) = reqwest::get(&url).await {
        if let Ok(info) = resp.json::<serde_json::Value>().await {
            let clients = info["clients"].as_u64().unwrap_or(0) as usize;
            return clients >= expected_pods;
        }
    }
    false
}
```

### Pattern: Session Progression

**What:** acServer supports automatic Practice -> Qualify -> Race progression. Each session block in `server_cfg.ini` has `WAIT_TIME` (seconds between sessions). When one session ends (time expires or all laps done), the server auto-transitions to the next after WAIT_TIME seconds.

**Config for Racing Point:**
- PICKUP_MODE_ENABLED=1 (open join, no booking)
- LOOP_MODE=1 (restart from first session when race ends)
- Session blocks: [PRACTICE] -> [QUALIFY] -> [RACE] with appropriate WAIT_TIMEs
- REGISTER_TO_LOBBY=0 (LAN only, no Kunos lobby registration)

### Anti-Patterns to Avoid
- **Using Content Manager for MP launches:** Fragile, depends on CM installation, can show modal error dialogs, `acmanager://` URI is the only integration point. Direct acs.exe + race.ini is deterministic.
- **Using AssettoServer (community alternative):** Different binary, different config format. Stock acServer.exe is already on server via Steam. No migration needed for LAN venue.
- **Polling tasklist for lobby status:** Use acServer HTTP API instead -- it knows the actual connection state, not just process existence.
- **Booking mode:** Requires GUID reservation per pod. Use PICKUP_MODE for the venue's open-join model.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Server config generation | Custom INI writer | `generate_server_cfg_ini()` in ac_server.rs | Already handles all params, safety overrides, CSP |
| Entry list generation | Custom entry builder | `generate_entry_list_ini()` in ac_server.rs | Already supports named entries + pickup mode slots |
| Port management | Manual port tracking | `PortAllocator` in port_allocator.rs | TIME_WAIT cooldown, bind-test, concurrent session safety |
| Process lifecycle | Custom spawn/kill | `start_ac_server()`/`stop_ac_server()` | Orphan cleanup, DB logging, dashboard events, pod notification |
| Pod selection | Random pod picker | `find_adjacent_idle_pods()` in multiplayer.rs | Consecutive pod preference, reservation TOCTOU protection |

**Key insight:** Nearly everything already exists. This phase is primarily about (1) fixing the MP launch path to use direct acs.exe, (2) fixing the missing `server_port` in launch JSON, and (3) adding lobby sync monitoring.

## Common Pitfalls

### Pitfall 1: Car/Track Name Case Mismatch
**What goes wrong:** Server entry_list has `MODEL=ks_ferrari_488_gt3` but client race.ini has `MODEL=ks_ferrari_488_GT3`. Server rejects with "no available slots."
**Why it happens:** Three config sources must agree case-sensitively: server_cfg.ini CARS, entry_list.ini MODEL, client race.ini CAR.
**How to avoid:** Use the SAME car string from `AcLanSessionConfig.cars` for both server config and client launch_args. Never let the client pick a different string.
**Warning signs:** Game loads to server join screen but hangs or shows "no available slots."

### Pitfall 2: SERVER_PORT vs HTTP_PORT Confusion
**What goes wrong:** Client tries to connect on HTTP_PORT (8081) instead of UDP/TCP port (9600). Connection fails silently.
**Why it happens:** The [REMOTE] section needs `SERVER_PORT=<udp_port>` but code might use http_port.
**How to avoid:** The current launch JSON sends `server_http_port` but NOT `server_port`. The agent's `AcLaunchParams.server_port` defaults to 0. Must explicitly send the UDP port.
**Warning signs:** Race.ini shows `SERVER_PORT=0` or `SERVER_PORT=8081`.

### Pitfall 3: acServer Needs AC Content Directory as CWD
**What goes wrong:** acServer can't find cars/tracks, fails to start.
**Why it happens:** Vanilla Kunos acServer reads `content/` relative to its CWD. If started from a different directory, it can't find game data.
**How to avoid:** Already handled in `start_ac_server()` line 518-519: `.current_dir(acserver_dir)`. But config files are generated in a session directory and COPIED to acServer's `cfg/` directory (line 509-510).
**Warning signs:** acServer exits immediately with "track not found" or "car not found" in its log.

### Pitfall 4: CTRL_CLOSE_EVENT Killing rc-agent (MP Path)
**What goes wrong:** Launching AC via bat file's `cmd /C start acs.exe` creates a console hierarchy. When AC goes fullscreen, CTRL_CLOSE_EVENT propagates to rc-agent, killing it.
**Why it happens:** Console event propagation in Windows. Already fixed for SP (commit `d616ee10`, direct spawn with `DETACHED_PROCESS`).
**How to avoid:** Use the same direct launch pattern for MP. Since we're eliminating CM dependency, no bat file needed. Launch acs.exe with `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` (same as SP path).
**Warning signs:** rc-agent dies ~10-30s after AC launches, termination.log shows `CTRL_CLOSE_EVENT`.

### Pitfall 5: Entry List Slot Count
**What goes wrong:** 8 pods try to connect but entry_list.ini only has 4 slots. Extra pods can't join.
**Why it happens:** `entry_list.ini` must have at least MAX_CLIENTS entries (one per possible client).
**How to avoid:** When generating entries for N pods, set `max_clients = N` and generate N entry slots. If also adding AI, add those as additional slots.
**Warning signs:** Some pods connect, others get "server full."

### Pitfall 6: Windows Firewall Blocking acServer Ports
**What goes wrong:** acServer starts but pods can't connect. Server shows 0 clients.
**Why it happens:** Windows Firewall on server (.23) blocks the dynamically allocated UDP/TCP ports.
**How to avoid:** Add firewall rules for the port range (9600-9615 UDP+TCP, 8081-8096 HTTP). Or add acServer.exe as an allowed program.
**Warning signs:** acServer process is alive, HTTP API accessible locally, but remote pods timeout on connect.

## Code Examples

### Current MP Launch Path (to be replaced)
```rust
// ac_launcher.rs line 1491
let uri = if params.game_mode == "multi" {
    format!("acmanager://race/online?ip={}&httpPort={}", ip, port)
} else {
    "acmanager://race/config".to_string()
};
// Launched via: cmd /c start "" <uri>
```

### New MP Launch Path (direct acs.exe, same as SP)
```rust
// ac_launcher.rs - change line 537 from:
let use_direct_launch = !is_mp;
// to:
let use_direct_launch = true; // Direct launch for both SP and MP

// race.ini [REMOTE] section already written by write_remote_section():
// [REMOTE]
// ACTIVE=1
// SERVER_IP=192.168.31.23
// SERVER_PORT=9600
// PASSWORD=
// NAME=Pod1
```

### Fix: Add server_port to Launch JSON
```rust
// ac_server.rs start_ac_server() - add server_port to launch_json
let launch_json = serde_json::json!({
    "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
    "track": &config.track,
    "track_config": &config.track_config,
    "game_mode": "multi",
    "server_ip": &lan_ip,
    "server_port": config.udp_port,        // UDP port for [REMOTE] SERVER_PORT
    "server_http_port": config.http_port,   // HTTP port (not used by client directly)
    "server_password": &config.password,
    "session_type": "race",
});
```

### Lobby Sync Monitor
```rust
/// Poll acServer HTTP API to track connected clients.
/// Returns when expected_count clients have connected, or timeout.
async fn wait_for_lobby_full(
    http_port: u16,
    expected_count: usize,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() > deadline {
            return Err(format!("Lobby timeout: only {}/{} connected", /* current */, expected_count));
        }
        let url = format!("http://127.0.0.1:{}/INFO", http_port);
        if let Ok(resp) = reqwest::get(&url).await {
            if let Ok(info) = resp.json::<serde_json::Value>().await {
                let clients = info["clients"].as_u64().unwrap_or(0) as usize;
                if clients >= expected_count {
                    return Ok(());
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| CM `acmanager://` URI for MP | Direct acs.exe + race.ini [REMOTE] | This phase | Eliminates CM dependency, deterministic |
| Server on .51 (old server) | Server on .23 (current) | Already done | acServer.exe at Steam install path |
| Manual server setup | Automated via `start_ac_server()` | Already implemented | Full lifecycle management |

## Open Questions

1. **Does acs.exe reliably connect using just [REMOTE] section?**
   - What we know: The bat file's fallback path (line 54-57) already launches acs.exe directly for MP when CM is not found. The [REMOTE] section is written correctly (verified by tests). SP direct launch works perfectly.
   - What's unclear: Whether there are edge cases where acs.exe ignores [REMOTE] or needs additional handshake that CM provides.
   - Recommendation: Test on a single pod first (Pod 8 canary). Write race.ini with REMOTE, launch acs.exe, verify it connects to acServer.

2. **Does acServer HTTP API return connected client count reliably?**
   - What we know: The HTTP port is exposed and documented. `/INFO` endpoint exists.
   - What's unclear: Exact JSON response format, whether it counts mid-handshake clients.
   - Recommendation: Start acServer locally, curl `/INFO`, document the response format.

3. **WAIT_TIME behavior: does it delay session START or transition?**
   - What we know: Kunos docs say "seconds before the start of the session." Each session block has its own WAIT_TIME.
   - What's unclear: Whether WAIT_TIME acts as a countdown timer (starts when previous session ends) or as a minimum hold time.
   - Recommendation: Test with a short practice session and observe transition timing. This is already partially handled by existing code.

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| acServer.exe | AC dedicated server | Check on .23 | Stock Kunos (Steam) | N/A - required |
| AC content (cars/tracks) | acServer needs content/ dir | Check on .23 | Steam install | N/A - required |
| Windows Firewall rules | Pod connectivity to server | Check on .23 | N/A | Add rules during deploy |
| reqwest (Rust crate) | HTTP API polling | Already in Cargo.toml | existing | N/A |

**Verification needed at deploy time:**
- Confirm `acServer.exe` exists at `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\server\acServer.exe` on server .23
- Confirm firewall allows ports 9600-9615 (UDP+TCP) and 8081-8096 (HTTP) on server .23
- Confirm AC content directory has cars/tracks matching what pods have

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust) |
| Config file | Cargo.toml |
| Quick run command | `cargo test -p rc-agent -- remote` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MP-01 | Direct acs.exe launch with REMOTE section | unit | `cargo test -p rc-agent -- test_write_race_ini_multi_remote_active` | Existing |
| MP-02 | server_port included in launch JSON | unit | `cargo test -p racecontrol -- test_launch_json_includes_server_port` | Wave 0 |
| MP-03 | generate_server_cfg_ini produces valid config | unit | `cargo test -p racecontrol -- server_cfg` | Existing partial |
| MP-04 | generate_entry_list_ini with N pod entries | unit | `cargo test -p racecontrol -- entry_list` | Existing partial |
| MP-05 | Lobby sync detects N connected clients | unit | `cargo test -p racecontrol -- lobby_sync` | Wave 0 |
| MP-06 | Direct launch SP/MP parity | unit | `cargo test -p rc-agent -- test_direct_launch_mp` | Wave 0 |

### Wave 0 Gaps
- [ ] `crates/racecontrol/src/ac_server.rs` test: launch JSON includes `server_port` field
- [ ] `crates/racecontrol/src/ac_server.rs` test: lobby status polling mock
- [ ] `crates/rc-agent/src/ac_launcher.rs` test: MP mode uses direct launch (not CM)

## Sources

### Primary (HIGH confidence)
- `crates/racecontrol/src/ac_server.rs` -- existing server management code (1400+ lines)
- `crates/rc-agent/src/ac_launcher.rs` -- existing client launch code (3400+ lines)
- `crates/rc-common/src/types.rs` -- AcLanSessionConfig, AcEntrySlot, AcSessionBlock structs
- `.planning/archive/ac-launcher-v1.0/research/STACK.md` -- prior research on AC server config
- `.planning/archive/ac-launcher-v1.0/research/PITFALLS.md` -- prior pitfall documentation
- `deploy/launch-ac.bat` -- current MP launch bat file (shows CM dependency + direct fallback)
- `deploy-staging/racecontrol.toml` -- current server config with acServer path

### Secondary (MEDIUM confidence)
- [Kunos Official Forum - AC Dedicated Server Manual](https://www.assettocorsa.net/forum/index.php?faq/assetto-corsa-dedicated-server-manual.28/) -- server_cfg.ini params, ports, pickup vs booking
- `.planning/codebase/INTEGRATIONS.md` -- AC server integration notes

### Tertiary (LOW confidence)
- Direct acs.exe + [REMOTE] section connection without CM -- needs empirical validation on a pod

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- 90% already implemented in codebase
- Architecture: HIGH -- clear pattern from existing SP direct launch
- Pitfalls: HIGH -- documented from prior research and production incidents
- Lobby sync: MEDIUM -- acServer HTTP API format needs empirical verification
- Direct MP connect: MEDIUM -- bat file fallback proves it works, but edge cases unknown

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable -- AC server binary hasn't changed in years)
