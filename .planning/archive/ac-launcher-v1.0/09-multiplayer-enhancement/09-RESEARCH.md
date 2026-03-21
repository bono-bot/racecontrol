# Phase 9: Multiplayer Enhancement - Research

**Researched:** 2026-03-14
**Domain:** AC multiplayer server orchestration, synchronized billing, lobby UI
**Confidence:** HIGH (codebase analysis) / MEDIUM (AI-on-server approach)

## Summary

Phase 9 builds on extensive existing infrastructure. The codebase already has ~70% of the multiplayer orchestration: `multiplayer.rs` handles booking, pod reservation, shared PIN generation, wallet debit, and member validation. `ac_server.rs` manages the full server lifecycle (INI generation, process spawning, port allocation, result collection). The PWA has a 3-step booking wizard (`/book/multiplayer`) and a lobby page (`/book/group`) with status polling.

The primary gaps are: (1) AI grid fillers on the AC dedicated server, (2) synchronized billing start tied to all players being on-track, (3) enriching the lobby UI with track/car/AI info, and (4) fixing the `LaunchGame` message format so the agent correctly launches in multiplayer mode.

**Primary recommendation:** Use AssettoServer (custom AC server replacement, already well-known in AC community) instead of vanilla acServer.exe for AI opponent support. The vanilla acServer does NOT support AI. If AssettoServer is not acceptable, skip AI fillers entirely and have players race each other only (still satisfies MULT-01, MULT-03, MULT-04, MULT-05, MULT-06 without MULT-02).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **AI Grid Fillers - Same car as players:** All AI drive the same car model the host chose
- **AI Grid Fillers - Auto-fill to track max:** AI fills all remaining grid spots up to track's pit count minus human players
- **AI Grid Fillers - Host's difficulty setting:** AI fillers use AI_LEVEL from host's chosen difficulty tier
- **AI Grid Fillers - Entry list INI approach:** Add AI entries in entry_list.ini with empty GUIDs, use existing 60-name AI driver pool
- **Synchronized Billing - Starts when last player is on-track:** Billing starts for ALL players only after every participant's AC STATUS=LIVE
- **Synchronized Billing - Disconnected player billing stops individually:** Individual billing stop on disconnect
- **Synchronized Billing - Race ends naturally:** Billing stops when AC race finishes (laps/time)
- **Synchronized Billing - Each player pays their own:** Independent wallet debit (host pays on book, invitees on accept)
- **Lobby - Auto-start when all checked in:** Race launches on all pods when every invited player's status is 'validated'
- **Lobby - Show track, car, and AI count:** Display session config in lobby alongside member list
- **Lobby - Status text only:** No countdown timer, text-based status progression
- **Lobby - Show pod number per player:** Already partially implemented (m.pod_number)
- **Server-to-Pod - Content Manager URI auto-join:** Launch via acmanager:// URI
- **Server-to-Pod - 60-second connection timeout:** Pod fails if no connection within 60s
- **Server-to-Pod - AC server on Racing-Point-Server (.23):** Dedicated server on central machine
- **Server-to-Pod - All same car:** Host picks car, everyone drives it

### Claude's Discretion
- WebSocket message flow for multiplayer state coordination between rc-core and rc-agent
- How to detect "all players on-track" for synchronized billing start (polling vs event-driven)
- acmanager:// URI format and parameters for server auto-join
- How to coordinate the "all validated -> launch on all pods" transition
- Error handling for partial server starts or mid-session failures

### Deferred Ideas (OUT OF SCOPE)
- Race Weekend multiplayer (group Practice -> Qualify -> Race sequence) -- AMLT-01, v2
- Spectator mode for waiting customers -- AMLT-02, v2
- Custom livery selection per customer -- AMLT-03, v2
- AI grid size slider in multiplayer config -- decided against (auto-fill to max)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| MULT-01 | Multiple customers on different pods can race together on AC dedicated server | Existing: `start_ac_lan_for_group()`, `start_ac_server()`, `acmanager://` URI. Gap: `LaunchGame` sends raw URI instead of JSON with `game_mode: "multi"` |
| MULT-02 | AI fills remaining grid spots in multiplayer races | **CRITICAL**: Vanilla acServer does NOT support AI. Needs AssettoServer or skip. See AI Grid Filler Architecture section |
| MULT-03 | Cross-pod billing synchronized | Existing: per-pod `defer_billing_start()` + `handle_game_status_update()`. Gap: no multi-pod synchronization -- billing starts independently per pod on STATUS=LIVE |
| MULT-04 | Multiplayer lobby/waiting UI shows who's joined and race status | Existing: `/book/group` has member list + status polling. Gap: no track/car/AI info displayed |
| MULT-05 | Uses existing ac_server.rs infrastructure | `generate_server_cfg_ini`, `generate_entry_list_ini`, `start_ac_server` all exist and work |
| MULT-06 | Entry list includes real driver names and GUIDs | Existing: `get_driver_entry_info()` resolves name + steam_guid from DB |
</phase_requirements>

## Standard Stack

### Core (Already in Use)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rc-core | 0.1.0 | Server orchestration (multiplayer.rs, ac_server.rs, billing.rs) | Existing Rust/Axum codebase |
| rc-agent | 0.1.0 | Pod-side game launcher (ac_launcher.rs) | Existing agent with CM multiplayer support |
| rc-common | 0.1.0 | Shared types and protocol | AcEntrySlot, GroupSessionInfo, etc. |
| Next.js PWA | - | Mobile lobby UI | Existing /book/multiplayer and /book/group pages |
| SQLite (sqlx) | - | State persistence | group_sessions, group_session_members, ac_sessions, multiplayer_results tables exist |

### For AI Support (If Pursuing MULT-02)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| AssettoServer | latest | Custom AC server with AI support | Replace vanilla acServer.exe for multiplayer sessions with AI |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| AssettoServer | Vanilla acServer + no AI | Simple, zero setup. Satisfies all requirements except MULT-02 |
| AssettoServer | CSP server plugins | Uncertain compatibility, complex setup |
| Synchronized billing via "all LIVE" | Independent billing per pod | Simpler but violates MULT-03 lock |

## Architecture Patterns

### Existing Flow (What Works Today)

```
1. Host creates session:     PWA /book/multiplayer -> POST /api/multiplayer/book
2. Invites sent:             multiplayer::book_multiplayer() -> group_session_members rows
3. Friends accept:           PWA /book/group -> POST /api/multiplayer/:id/accept
4. Each player validates PIN: Lock screen -> PinEntered -> on_member_validated()
5. All validated trigger:     on_member_validated() -> start_ac_lan_for_group()
6. Server starts:            start_ac_server() generates INI, spawns acServer, allocates ports
7. Game launches on pods:    LaunchGame -> agent -> ac_launcher (via acmanager:// URI)
```

### Gap Analysis

**Gap 1: LaunchGame message format mismatch**
`start_ac_server()` sends `launch_args: Some(join_url.clone())` where join_url is `acmanager://race/online/join?ip=X&httpPort=Y`. But the agent's `LaunchGame` handler tries to parse launch_args as JSON (`AcLaunchParams`). When JSON parsing fails, it falls back to defaults with empty `game_mode` (NOT "multi"), so the agent launches acs.exe directly instead of via Content Manager.

**Fix:** `start_ac_server()` should send JSON launch_args with `game_mode: "multi"`, `server_ip`, `server_http_port`, and `server_password` fields. The agent already handles `game_mode == "multi"` correctly via `launch_via_cm()`.

**Gap 2: No AI entries in entry list**
`start_ac_lan_for_group()` only creates human player entries. No AI entries are added. Even if AI were supported by the server, the entry list would only have human slots.

**Gap 3: No synchronized billing**
Currently, `defer_billing_start()` + `handle_game_status_update()` starts billing independently per pod when each pod's AC reaches STATUS=LIVE. There's no coordination to wait for ALL pods to be LIVE before starting billing for ANY pod.

**Gap 4: Lobby lacks session config info**
The `/book/group` page only shows `experience_name`, `host_name`, `pricing_tier_name`, and member list. It does not show: track name, car model, AI opponent count, or difficulty tier.

**Gap 5: `GroupSessionInfo` type missing fields**
The `GroupSessionInfo` struct has `experience_name` but no `track`, `car`, `ai_count`, or `difficulty` fields. The PWA needs these for the enhanced lobby display.

### Recommended Architecture for Synchronized Billing (MULT-03)

**Event-driven via existing WebSocket protocol:**

1. When a pod's AC reaches STATUS=LIVE, rc-core receives a `GameStatusUpdate` agent message
2. Instead of immediately starting billing, check if pod is part of a group session
3. If yes, record this pod as "on-track" in a new in-memory map: `HashMap<String, HashSet<String>>` (group_session_id -> set of on-track pod_ids)
4. When the set equals the expected member count, start billing for ALL pods simultaneously
5. Use existing `start_billing_session()` for each pod -- called from this coordinator, not from `handle_game_status_update()`

**Disconnection handling:**
- If a player disconnects (STATUS=Off) mid-race, call `end_billing_session()` for that pod only
- Other players continue racing with billing active
- Do NOT stop the AC server for remaining players

### Recommended Architecture for "All Validated -> Launch" (Already Implemented)

The `on_member_validated()` function already does this:
1. Updates member status to 'validated'
2. Checks if `validated_total >= accepted_total`
3. If all validated, calls `start_ac_lan_for_group()`
4. `start_ac_lan_for_group()` calls `start_ac_server()` which sends `LaunchGame` to all pods

This flow is complete. Only the LaunchGame message format needs fixing (Gap 1).

### Recommended acmanager:// URI Format

Already verified in codebase (`ac_server.rs` line 433 and `ac_launcher.rs` line 1045):
```
acmanager://race/online/join?ip={server_ip}&httpPort={http_port}
```
With optional password: `&password={password}`

The agent's `launch_via_cm()` function (ac_launcher.rs:1042) constructs this URI correctly. The fix is to send JSON launch_args so the agent parses them into `AcLaunchParams` and invokes `launch_via_cm()`.

## AI Grid Filler Architecture

### CRITICAL FINDING: Vanilla acServer Does NOT Support AI

**Confidence: HIGH** (verified via official Kunos documentation and multiple community sources)

The vanilla `acServer.exe` from Steam (`C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\server\acServer.exe`) does NOT support AI opponents. Empty GUID entries in `entry_list.ini` create unreserved slots for human players to join -- they do NOT become AI drivers.

AI on AC dedicated servers requires one of:
1. **AssettoServer** (https://assettoserver.org/) -- a community-built replacement server with `AI=fixed` support in entry_list.ini and `EnableAi: true` in extra_cfg.yml. Requires fast_lane.aip files per track (default tracks have these).
2. **CSP server plugins** -- uncertain compatibility, complex setup

### Recommendation for MULT-02

**Option A (Recommended): Use AssettoServer**
- Drop-in replacement for acServer.exe (same entry_list.ini and server_cfg.ini format)
- Supports AI traffic via `AI=fixed` in entry_list.ini
- Requires an `extra_cfg.yml` with `EnableAi: true`
- Change `acserver_path` in racecontrol.toml to point to AssettoServer binary
- Risk: different server behavior, needs testing

**Option B: Skip AI for v1, add in v2**
- Much simpler: just run vanilla acServer with human players only
- Satisfies MULT-01, MULT-03, MULT-04, MULT-05, MULT-06
- MULT-02 deferred to v2 with AssettoServer
- Empty grid slots stay empty (players race each other)

**Option C: AI via race.ini on agent side (workaround)**
- Instead of running AI on the server, pre-populate AI_LEVEL and AI opponents in each pod's local race.ini before server join
- This does NOT work for multiplayer -- AC ignores local AI settings when connecting to a server

**The user's locked decision says "Entry list INI approach -- no CSP server plugin dependency."** This decision was made under the assumption that vanilla acServer supports AI via empty GUIDs, which it does not. The planner should flag this for user re-evaluation.

### If AssettoServer is Adopted

Entry list format for AI entries:
```ini
[CAR_N]
MODEL=ks_ferrari_488_gt3
SKIN=
DRIVERNAME=Marco Rossi
GUID=
AI=fixed
BALLAST=0
RESTRICTOR=0
SPECTATOR_MODE=0
```

Additional config needed (`extra_cfg.yml`):
```yaml
EnableAi: true
```

Changes to `AcEntrySlot` type (rc-common/types.rs):
```rust
pub struct AcEntrySlot {
    pub car_model: String,
    pub skin: String,
    pub driver_name: String,
    pub guid: String,
    pub ballast: u32,
    pub restrictor: u32,
    pub pod_id: Option<String>,
    pub ai_mode: Option<String>,  // NEW: None for human, "fixed" for AI
}
```

Changes to `generate_entry_list_ini()`:
```rust
// For each entry, if ai_mode is Some, add AI= line
if let Some(ai) = &entry.ai_mode {
    ini.push_str(&format!("AI={}\n", ai));
}
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| AI on dedicated server | Custom AI injection | AssettoServer `AI=fixed` | Vanilla acServer has zero AI support |
| Port allocation | Manual port tracking | Existing `PortAllocator` | Already handles allocation, cooldown, and collision avoidance |
| Multi-pod billing coordination | Polling loop | Event-driven via `GameStatusUpdate` agent messages | Agent already sends AcStatus changes via WebSocket |
| Lobby status polling | WebSocket push to PWA | Existing 3-second HTTP polling in `/book/group` | PWA already polls `api.groupSession()` every 3s -- sufficient for lobby |
| acmanager:// URI construction | Hardcoded strings | Existing format in `ac_server.rs:433` | Already tested and working |
| Group session state machine | Manual status tracking | Existing DB status column | group_sessions.status: forming -> ready -> active -> all_validated -> completed |

## Common Pitfalls

### Pitfall 1: LaunchGame JSON vs URI Mismatch
**What goes wrong:** Agent receives `acmanager://...` URI as launch_args, JSON parse fails silently, agent launches with defaults (empty game_mode, wrong car/track)
**Why it happens:** `start_ac_server()` sends raw URI; agent expects JSON with `game_mode: "multi"`
**How to avoid:** Change `start_ac_server()` to send JSON launch_args: `{"car":"X","track":"Y","game_mode":"multi","server_ip":"Z","server_http_port":8081,...}`
**Warning signs:** AC launches in single-player practice mode instead of joining server

### Pitfall 2: Billing Starts Before All Players On-Track
**What goes wrong:** Player A hits STATUS=LIVE first, billing starts immediately for A. Player B is still loading (30-60s DirectX init). Player A pays for time while B loads.
**Why it happens:** Current `handle_game_status_update()` starts billing per-pod independently
**How to avoid:** Add a group-aware billing coordinator that waits for all group members to reach LIVE
**Warning signs:** Different billing start times across pods in the same session

### Pitfall 3: Race Ends But Billing Doesn't
**What goes wrong:** AC race completes (laps done), result screen shows, but billing timer keeps running because STATUS stays LIVE during result screen
**Why it happens:** AC keeps STATUS=LIVE during the post-race result screen
**How to avoid:** Watch for `AcStatus::Off` when AC exits (current behavior in `handle_game_status_update`), or add explicit "race complete" detection from server results
**Warning signs:** Billing continues 30-60s after race finish

### Pitfall 4: Vanilla acServer Appears to Work Without AI
**What goes wrong:** Server starts, players connect, grid shows empty slots. Everything seems fine but there are no AI cars.
**Why it happens:** Vanilla acServer ignores AI entries silently. Empty GUID entries become unreserved human slots.
**How to avoid:** Use AssettoServer, or explicitly document that the grid will have empty spots
**Warning signs:** Only human players visible on track despite AI entries in entry_list.ini

### Pitfall 5: Stale Group Session Blocks New Sessions
**What goes wrong:** A group session in 'all_validated' or 'active' status blocks the AC server (only one session at a time). If session doesn't complete cleanly, new multiplayer sessions can't start.
**Why it happens:** `start_ac_server()` checks for existing running sessions and bails if found
**How to avoid:** `cleanup_orphaned_sessions()` already runs on startup. Add explicit cleanup when group session is cancelled/expired.
**Warning signs:** "An AC server session is already running" error when trying to start a new multiplayer session

### Pitfall 6: Content Manager Not Installed on a Pod
**What goes wrong:** `launch_via_cm()` fails because Content Manager isn't installed. Direct acs.exe fallback launches in single-player mode with [REMOTE] ACTIVE=1 in race.ini.
**Why it happens:** CM is not standard AC -- it's a third-party app that must be installed separately
**How to avoid:** Verify CM is installed on all pods during deployment. The agent already has `find_cm_exe()` check and fallback logic with diagnostic logging (`diagnose_cm_failure()`).
**Warning signs:** "CM multiplayer launch failed" in agent logs, game opens in single-player

## Code Examples

### Example 1: Fixed LaunchGame args (JSON instead of raw URI)

```rust
// In start_ac_server(), replace:
//   launch_args: Some(join_url.clone()),
// With:
let launch_json = serde_json::json!({
    "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
    "track": config.track,
    "track_config": config.track_config,
    "game_mode": "multi",
    "server_ip": lan_ip,
    "server_http_port": config.http_port,
    "server_password": config.password,
    "session_type": "race",
    "driver": "", // filled by agent from cached driver_name
});
let cmd = CoreToAgentMessage::LaunchGame {
    sim_type: SimType::AssettoCorsa,
    launch_args: Some(launch_json.to_string()),
};
```

### Example 2: AI Entry Generation (if AssettoServer adopted)

```rust
// In start_ac_lan_for_group(), after human entries:
let ai_names = pick_ai_names(ai_count); // reuse from ac_launcher.rs
for name in ai_names {
    entry_slots.push(AcEntrySlot {
        car_model: car.clone(),
        skin: String::new(),
        driver_name: name,
        guid: String::new(),
        ballast: 0,
        restrictor: 0,
        pod_id: None,
        ai_mode: Some("fixed".to_string()),
    });
}
```

Note: `pick_ai_names()` currently lives in `rc-agent::ac_launcher`. It needs to be moved to `rc-common` or duplicated in `rc-core` so the server can use it.

### Example 3: Synchronized Billing Coordinator

```rust
// New field in BillingManager:
pub multiplayer_waiting: RwLock<HashMap<String, MultiplayerBillingWait>>,

pub struct MultiplayerBillingWait {
    pub group_session_id: String,
    pub expected_pods: HashSet<String>,
    pub live_pods: HashSet<String>,
    pub billing_params: HashMap<String, WaitingForGameEntry>,
}

// In handle_game_status_update, for AcStatus::Live:
// 1. Check if pod is part of a group session
// 2. If yes, add to live_pods set
// 3. If live_pods == expected_pods, start billing for all pods
// 4. If not, show "Waiting for X more players..." on this pod
```

### Example 4: Enhanced GroupSessionInfo for Lobby

```rust
// Add to GroupSessionInfo:
pub track: Option<String>,
pub car: Option<String>,
pub ai_count: Option<u32>,
pub difficulty_tier: Option<String>,
```

```typescript
// In /book/group/page.tsx, display session config:
{group.track && (
  <div className="grid grid-cols-3 gap-2 mb-6">
    <InfoCard label="Track" value={group.track} />
    <InfoCard label="Car" value={group.car} />
    <InfoCard label="AI Opponents" value={`${group.ai_count || 0}`} />
  </div>
)}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Vanilla acServer | AssettoServer for AI | 2022+ | Only way to get AI on AC servers |
| Raw URI launch_args | JSON launch_args | This phase | Enables proper multiplayer launch on agent |
| Independent per-pod billing | Synchronized group billing | This phase | Fair billing for all participants |

**Deprecated/outdated:**
- Using vanilla acServer for AI: never worked, community uses AssettoServer
- CSP server plugins for AI: superseded by AssettoServer's built-in AI

## Open Questions

1. **AssettoServer vs vanilla acServer for MULT-02**
   - What we know: Vanilla acServer does NOT support AI. AssettoServer does via `AI=fixed`.
   - What's unclear: Is AssettoServer already installed on Racing-Point-Server (.23)? Is it acceptable as a dependency?
   - Recommendation: Ask Uday. If not acceptable, defer MULT-02 to v2 and proceed with human-only multiplayer.

2. **AI_LEVEL in server context**
   - What we know: In single-player, `AI_LEVEL` is set in race.ini. In AssettoServer, AI behavior is configured separately.
   - What's unclear: How AssettoServer maps AI difficulty levels. May need extra_cfg.yml tuning.
   - Recommendation: Use AssettoServer's defaults for v1, tune in v2.

3. **pick_ai_names() location**
   - What we know: Currently in rc-agent. Server needs it for entry_list generation.
   - What's unclear: Whether to move to rc-common or duplicate.
   - Recommendation: Move to rc-common (shared crate). Simple refactor.

4. **Race completion detection**
   - What we know: AC STATUS goes to Off when game exits. Server writes result files.
   - What's unclear: Exact timing between race finish, result screen, and STATUS change.
   - Recommendation: Rely on STATUS=Off for billing end (current behavior). Post-race result screen time is minimal and STATUS stays LIVE.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust, built-in) |
| Config file | Cargo.toml per crate |
| Quick run command | `cargo test -p rc-common && cargo test -p rc-core --lib` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent --lib && cargo test -p rc-core --lib` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| MULT-01 | Multiple pods join same AC server | integration | Manual on-site (2+ pods needed) | N/A - manual-only |
| MULT-02 | AI fills grid spots | unit | `cargo test -p rc-core --lib -- ac_server::tests -x` | Partial (INI gen tests exist, need AI entry test) |
| MULT-03 | Synchronized billing start | unit | `cargo test -p rc-core --lib -- billing::tests -x` | Wave 0 gap (no multi-pod sync test) |
| MULT-04 | Lobby shows session info | unit | Manual PWA verification | N/A - manual-only |
| MULT-05 | Uses existing ac_server.rs | unit | `cargo test -p rc-core --lib -- ac_server::tests -x` | Existing tests cover INI generation |
| MULT-06 | Entry list has real names/GUIDs | unit | `cargo test -p rc-core --lib -- ac_server::tests -x` | Wave 0 gap (need entry list with drivers test) |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-core --lib`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent --lib && cargo test -p rc-core --lib`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Test: `generate_entry_list_ini` with AI entries (ai_mode field)
- [ ] Test: `generate_entry_list_ini` with mixed human + AI entries
- [ ] Test: LaunchGame JSON args with `game_mode: "multi"` parse correctly
- [ ] Test: `GroupSessionInfo` serialization with new fields (track, car, ai_count)
- [ ] Move `AI_DRIVER_NAMES` and `pick_ai_names()` to rc-common (or duplicate for rc-core)

## Sources

### Primary (HIGH confidence)
- **Codebase analysis** - ac_server.rs (~1100 lines), multiplayer.rs (~1300 lines), billing.rs, types.rs, ac_launcher.rs, PWA pages
- **Kunos official forum FAQ #28** - [Dedicated Server Manual](https://www.assettocorsa.net/forum/index.php?faq/dedicated-server-manual.28/) - Confirmed vanilla acServer has no AI support
- **AssettoServer docs** - [Beginner's Guide](https://assettoserver.org/docs/thebeginnersguide/) - AI configuration via `AI=fixed` and `EnableAi: true`

### Secondary (MEDIUM confidence)
- **OverTake.gg forum** - [AI cars in multiplayer](https://www.overtake.gg/threads/so-ai-cars-in-multiplayer-are-possible.160351/) - Community confirms vanilla server lacks AI, CSP/AssettoServer needed
- **GTXGaming/Shockbyte/BisectHosting** - Server hosting guides confirming `AI=auto`/`AI=fixed` entry list format

### Tertiary (LOW confidence)
- AI_LEVEL mapping to AssettoServer difficulty - no official documentation found, needs testing

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - all libraries already in use, no new dependencies except AssettoServer
- Architecture: HIGH - gaps clearly identified, solutions follow existing patterns
- AI on server: MEDIUM - vanilla acServer limitation confirmed, AssettoServer approach verified via docs but not tested locally
- Pitfalls: HIGH - identified from actual code analysis of current LaunchGame flow

**Research date:** 2026-03-14
**Valid until:** 2026-04-14 (stable domain, AC server API hasn't changed in years)
