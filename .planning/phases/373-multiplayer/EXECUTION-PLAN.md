# Phase 373: Multiplayer — Execution Plan

**Created:** 2026-04-12
**Author:** Bono (architect) for James (executor)
**Status:** Ready for execution
**Depends on:** Phase 372 (Billing — Arcade Model)

## 1. Current State Analysis

### What Already Works (DO NOT REWRITE)

The multiplayer system is **~85% complete**. The following code is live and tested:

| Component | File | Lines | Status |
|-----------|------|-------|--------|
| Group session booking (PWA + Staff + Kiosk) | `multiplayer.rs` | 1,749 | LIVE |
| AC dedicated server lifecycle | `ac_server.rs` | 2,185 | LIVE |
| Port allocation (16 concurrent sessions) | `port_allocator.rs` | 280 | LIVE |
| INI generation (server_cfg, entry_list, extra_cfg) | `ac_server.rs:203-411` | 208 | LIVE |
| Atomic multi-wallet debit with rollback | `multiplayer.rs:772-843` | 71 | LIVE |
| Lobby sync monitor (poll acServer /INFO) | `ac_server.rs:930-1110` | 180 | LIVE |
| Continuous mode (auto-restart races) | `ac_server.rs:1115-1233` | 118 | LIVE |
| Result collection from acServer JSON | `ac_server.rs:1400-1691` | 291 | LIVE |
| AI grid fillers (AssettoServer AI=fixed) | `multiplayer.rs:1063-1139` | 76 | LIVE |
| Shared PIN and lock screen coordination | `multiplayer.rs:197-273` | 76 | LIVE |
| Coordinated billing wait (multiplayer_waiting) | `billing.rs:648-667` | 19 | LIVE |
| Launch via JSON args (game_mode: "multi") | `ac_server.rs:609-629` | 20 | LIVE |
| rc-agent MP launch (race.ini [REMOTE]) | `ac_launcher.rs:1398-1408` | 10 | LIVE |
| E2E test script | `tests/e2e/api/multiplayer.sh` | 574 | LIVE |

### What Phase 373 Must Deliver (the gaps)

The four success criteria from ROADMAP.md:

1. **MULT-01**: Staff launches AC multiplayer — games start on 2+ pods simultaneously
2. **MULT-02**: All participants' laps appear on leaderboard during AND after the session
3. **MULT-03**: No participant dropped or orphaned mid-race
4. **MULT-04**: If any wallet debit fails, no participants are charged, session does not launch

**Gap analysis against success criteria:**

| Criterion | Current Status | Gap |
|-----------|---------------|-----|
| MULT-01 (simultaneous launch) | Partially done — `start_ac_server` sends LaunchGame to all pods but sequentially via WS. Agent launches are fire-and-forget. No readiness gate before race countdown. | Need: synchronized launch gate — all pods must have AC loaded before race starts |
| MULT-02 (lap recording in MP) | **BIG GAP** — rc-agent telemetry reader sends `LapCompleted` events only during single-player. In MP, the acServer hosts the session but each pod's AC still writes to shared memory / UDP. No code verifies that `LapCompleted` events from rc-agent are attributed to the correct driver in a group session context. | Need: verify per-pod telemetry attribution in MP, persist to leaderboard |
| MULT-03 (no orphaned participants) | Partially done — lobby sync monitor tracks connected clients. But: no reconnect logic if a pod drops mid-race; no billing pause on disconnect. | Need: disconnect detection + billing pause + rejoin path |
| MULT-04 (atomic billing) | **DONE** — `atomic_multi_debit()` validates all balances first, debits sequentially, rolls back on failure. Staff path uses this. | Verified working, no changes needed |

---

## 2. VMS Architecture Reference (How the Competitors Do It)

VMS (Virtual Motorsport Simulator) uses a different approach than ours:

- **40 acServer instances** (`acServer01.exe` through `acServer40.exe`) — pre-configured, each bound to fixed ports
- **VMS Connect** on each pod writes race.ini `[REMOTE]` section and launches AC via Content Manager URI
- **VMS Plugin** (AC shared memory plugin) on each pod reports lap times independently to the VMS server
- **No centralized telemetry** — each pod is responsible for its own lap recording

**Our approach is architecturally superior** because:
- Dynamic port allocation (PortAllocator) instead of 40 fixed configs
- Centralized acServer lifecycle management instead of pre-spawned instances
- Per-pod rc-agent telemetry reader already handles lap attribution

We do NOT need to copy VMS's approach. We need to close the gaps in our existing system.

---

## 3. Detailed Plan by Component

### Plan 373-01: Synchronized Launch Gate

**Goal:** All pods have AC loaded and connected to the acServer before the race countdown starts.

**Current flow:**
```
start_ac_server() -> spawn acServer.exe -> send LaunchGame to each pod
                                           -> monitor_lobby_sync polls /INFO
                                           -> when clients == expected, broadcast LobbyPhase::Active
```

**Problem:** The acServer race starts immediately. Pods that load slowly miss the start.

**Fix:** Use acServer's `WAIT_TIME` parameter in the `[RACE]` session block. Currently hardcoded to `10` seconds in `start_ac_lan_for_group()` at `multiplayer.rs:1154`. This is the time the server waits after all clients connect before starting the race.

**Changes:**

1. **`multiplayer.rs:1154`** — Change `wait_time_secs: 10` to `wait_time_secs: 60` (gives all pods 60s to load)
2. **`ac_server.rs:930-1110`** (monitor_lobby_sync) — When all pods connect (`clients >= expected_pod_count`), broadcast `LobbyPhase::AllReady`. The 60s `WAIT_TIME` in the server config handles the actual countdown — no code needed.
3. **`ac_server.rs:425-430`** — Remove the single-session guard. Currently `start_ac_server` checks `if matches!(inst.status, Starting | Running)` and bails. This prevents concurrent MP sessions (e.g., two groups of 4 racing different tracks). The `PortAllocator` already handles port isolation. Remove this check.

**Line count estimate:** ~15 lines changed

**Files touched:**
- `crates/racecontrol/src/multiplayer.rs` — 1 line (wait_time_secs)
- `crates/racecontrol/src/ac_server.rs` — ~14 lines (remove single-session guard, no functional changes to lobby monitor)

---

### Plan 373-02: Lap Recording in Multiplayer

**Goal:** Every participant's laps appear on the leaderboard during and after a multiplayer session.

**Current lap recording flow (single-player):**
```
rc-agent reads AC shared memory -> detects lap completion -> sends LapCompleted {
    pod_id, driver_id, game, track, car, lap_time_ms, sector_times_ms, is_valid
} to racecontrol server -> server inserts into `laps` table -> leaderboard queries `laps`
```

**Key insight:** In multiplayer, AC on each pod still writes to local shared memory. The rc-agent telemetry reader on each pod reads that shared memory and sends `LapCompleted` to the server. The driver_id is set when billing starts (from the auth token). This means **lap recording already works in MP** — each pod independently reports its driver's laps.

**However, there are two gaps:**

**Gap A: driver_id attribution during MP billing wait**

When `defer_billing_start()` is called with `group_session_id: Some(id)`, the billing timer doesn't start until ALL group members reach LIVE. During the wait period, rc-agent may send `LapCompleted` events. The server needs to know which driver is on which pod even before billing starts.

Currently `LapCompleted.driver_id` is populated by rc-agent from `self.current_driver_id` which is set when the agent receives a `SetDriverContext` message (sent during billing start). In MP, billing is deferred — so `current_driver_id` may be unset during the loading/practice phase.

**Fix:** Send `SetDriverContext` to each pod IMMEDIATELY when `start_ac_lan_for_group()` runs (before billing starts). This is already partially done — the `LaunchGame` message includes `launch_args` with driver info, but the agent needs an explicit `SetDriverContext` for telemetry attribution.

**Changes:**

1. **`multiplayer.rs:1182-1200`** — After `start_ac_server` succeeds, iterate over `members` and send `CoreToAgentMessage::SetDriverContext { driver_id, driver_name }` to each pod. This ensures rc-agent has `current_driver_id` set before any laps are recorded.

**Gap B: Leaderboard query doesn't filter by session type**

The leaderboard queries `laps` table. MP laps and SP laps are in the same table. No filtering issue here — laps are laps regardless of session type. But we should tag MP laps with the `group_session_id` for post-session analysis.

**Fix:** Add `group_session_id` column to the `laps` table insert path.

**Changes:**

2. **`crates/racecontrol/src/billing.rs`** (or wherever `LapCompleted` is handled on the server) — When inserting a lap, check if the pod has an active billing timer with a `group_session_id`. If so, include it in the `laps` INSERT. This requires an `ALTER TABLE laps ADD COLUMN group_session_id TEXT` migration.

3. **DB migration** — Add to `migrations.rs` or the startup migration block:
```sql
ALTER TABLE laps ADD COLUMN group_session_id TEXT;
CREATE INDEX IF NOT EXISTS idx_laps_group_session ON laps(group_session_id);
```

**Line count estimate:** ~40 lines added

**Files touched:**
- `crates/racecontrol/src/multiplayer.rs` — ~15 lines (SetDriverContext sends)
- `crates/racecontrol/src/billing.rs` or lap handler — ~15 lines (group_session_id tagging)
- `crates/racecontrol/src/main.rs` — ~10 lines (migration)

---

### Plan 373-03: Disconnect Handling and Rejoin

**Goal:** No participant dropped or orphaned mid-race.

**Current disconnect detection:**
- `monitor_lobby_sync` polls acServer `/INFO` and tracks connected client count
- When a pod's AC exits, rc-agent sends `GameEvent::Ended` or `GameEvent::Crashed`
- `handle_game_status_update` receives `AcStatus::Off` and ends billing for that pod

**Gaps:**
1. No automatic reconnect attempt when a pod disconnects mid-race
2. Billing for disconnected pod ends permanently — no pause-and-resume
3. Other pods are not notified that a participant dropped
4. No mechanism to rejoin the same acServer session after a crash

**Fix — three-layer approach:**

**Layer 1: Billing pause on disconnect (not end)**

When a pod in a group session reports `AcStatus::Off` or `AcStatus::Crash`, pause billing (don't end it). The existing `PauseReason::CrashRecovery` mechanism is perfect for this.

**Changes in `billing.rs`:**
- In `handle_game_status_update`, when `AcStatus::Off` is received for a pod that's part of a group session (check `group_session_id` on the timer), set `PauseReason::CrashRecovery` instead of ending the session.
- Existing crash recovery already pauses billing and broadcasts `BillingPaused` to dashboard.

**Layer 2: Automatic rejoin attempt**

When a pod in a group session disconnects, automatically re-send `LaunchGame` with the same MP config after a 10-second delay. This reuses the existing `retry_pod_join()` function in `ac_server.rs:838-916`.

**Changes in `billing.rs` or `multiplayer.rs`:**
- When `AcStatus::Off` or `Crashed` is received for a pod in an active group session, and the acServer is still running:
  1. Pause billing (Layer 1)
  2. Wait 10 seconds
  3. Call `ac_server::retry_pod_join(state, ac_session_id, pod_id)`
  4. If the pod reconnects (AcStatus::Live received), resume billing

**Layer 3: Dashboard notification**

Broadcast a `DashboardEvent::GroupMemberDisconnected { group_session_id, pod_id, driver_name }` when a pod drops. The existing dashboard WS infrastructure handles delivery.

**Changes:**

**In `billing.rs` — `handle_game_status_update` AcStatus::Off/Crash handler (~50 lines):**
```rust
// Check if this pod is part of a group session
let group_session_id = {
    let timers = state.billing.active_timers.read().await;
    timers.get(pod_id).and_then(|t| t.group_session_id.clone())
};

if let Some(gsid) = group_session_id {
    // Check if acServer is still running for this group
    let ac_session_id = sqlx::query_scalar::<_, String>(
        "SELECT ac_session_id FROM group_sessions WHERE id = ?"
    ).bind(&gsid).fetch_optional(&state.db).await.ok().flatten();

    if let Some(ac_sid) = ac_session_id {
        let still_running = {
            let instances = state.ac_server.instances.read().await;
            instances.get(&ac_sid).map(|i| matches!(i.status, AcServerStatus::Running)).unwrap_or(false)
        };

        if still_running {
            // PAUSE billing, don't end
            // ... set CrashRecovery pause
            // Schedule rejoin attempt
            let state_clone = state.clone();
            let pod_id_clone = pod_id.to_string();
            let ac_sid_clone = ac_sid.clone();
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                let _ = crate::ac_server::retry_pod_join(&state_clone, &ac_sid_clone, &pod_id_clone).await;
            });
            // Broadcast disconnect to dashboard
            let _ = state.dashboard_tx.send(DashboardEvent::GroupMemberDisconnected {
                group_session_id: gsid,
                pod_id: pod_id.to_string(),
            });
            return; // Don't end billing
        }
    }
    // acServer not running — end billing normally (race is over)
}
```

**New DashboardEvent variant in `rc-common/src/protocol.rs`:**
```rust
GroupMemberDisconnected {
    group_session_id: String,
    pod_id: String,
},
```

**Line count estimate:** ~70 lines added

**Files touched:**
- `crates/racecontrol/src/billing.rs` — ~50 lines (disconnect-aware billing)
- `crates/rc-common/src/protocol.rs` — ~5 lines (new event variant)
- `crates/racecontrol/src/ws/mod.rs` — ~15 lines (broadcast new event)

---

### Plan 373-04: Concurrent MP Sessions (Remove Single-Session Guard)

**Goal:** Allow multiple simultaneous multiplayer groups (e.g., Group A on Monza, Group B on Spa).

**Current limitation:** `start_ac_server()` at `ac_server.rs:424-431` iterates all instances and bails if ANY is Starting or Running:
```rust
for inst in instances.values() {
    if matches!(inst.status, AcServerStatus::Starting | AcServerStatus::Running) {
        anyhow::bail!("An AC server session is already running: {}", inst.session_id);
    }
}
```

**Fix:** Remove this check entirely. The `PortAllocator` guarantees unique ports per session. Each acServer instance runs in its own directory with its own config. There is no technical reason to limit to one session. The PortAllocator supports 16 concurrent sessions (`max_sessions: 16` from `PortAllocator::new(9600, 8081, 16)`).

**Verification:** Check that `cleanup_orphaned_sessions` handles multiple concurrent sessions correctly. Reading the code at `ac_server.rs:64-145` — it queries ALL sessions with `status IN ('starting', 'running')` and cleans each one. This is already multi-session safe.

**Line count estimate:** ~5 lines removed

**Files touched:**
- `crates/racecontrol/src/ac_server.rs` — Delete lines 424-431

---

### Plan 373-05: End-to-End Verification Tests

**Goal:** Prove all four success criteria pass with 2+ real pods.

**Extend existing `tests/e2e/api/multiplayer.sh`** with:

1. **MP-06: Lap recording verification** — After the race runs for 30 seconds, query `GET /api/v1/laps?driver_id=<test_driver>` and verify at least 1 lap exists for each participant.

2. **MP-07: Disconnect + rejoin** — Kill AC on one pod mid-race (`POST /games/stop` for one pod), wait 15 seconds, verify: (a) billing for that pod is paused, (b) other pod's billing continues, (c) the disconnected pod's game re-launches (poll `/games/active`).

3. **MP-08: Concurrent sessions** — Book two separate 2-pod MP sessions simultaneously. Verify both acServer instances are running (different ports). Stop both. Verify cleanup.

4. **MP-09: Atomic debit failure** — Create a driver with 0 balance, attempt staff_book_multiplayer with that driver + a funded driver. Verify HTTP 400, verify funded driver's balance is unchanged.

**Line count estimate:** ~200 lines added to `tests/e2e/api/multiplayer.sh`

---

## 4. File-by-File Change Summary

| File | Action | Lines Changed | Plan |
|------|--------|---------------|------|
| `crates/racecontrol/src/multiplayer.rs` | Edit | +16 | 373-01 (wait_time), 373-02 (SetDriverContext) |
| `crates/racecontrol/src/ac_server.rs` | Edit | -7, +0 | 373-04 (remove single-session guard) |
| `crates/racecontrol/src/billing.rs` | Edit | +65 | 373-02 (group_session_id tag), 373-03 (disconnect pause + rejoin) |
| `crates/rc-common/src/protocol.rs` | Edit | +5 | 373-03 (GroupMemberDisconnected event) |
| `crates/racecontrol/src/ws/mod.rs` | Edit | +15 | 373-03 (broadcast new event) |
| `crates/racecontrol/src/main.rs` | Edit | +10 | 373-02 (migration: laps.group_session_id) |
| `tests/e2e/api/multiplayer.sh` | Edit | +200 | 373-05 (new test cases MP-06 through MP-09) |
| **Total** | | **~310 lines** | |

---

## 5. Execution Order

Execute plans in this order (dependencies shown):

```
373-04 (concurrent sessions)  ─┐
373-01 (sync launch gate)      ├── Can run in parallel (independent)
373-02 (lap recording)         ─┘
         │
         v
373-03 (disconnect handling)  ── Depends on 373-02 (needs group_session_id on timers)
         │
         v
373-05 (E2E tests)            ── Depends on all above
```

**Recommended approach:** Do 373-04 + 373-01 + 373-02 in a single commit (they're small and independent). Then 373-03 in a second commit. Then 373-05 as a third commit.

---

## 6. Test Plan

### Unit Tests (run on any machine)

```bash
cargo test -p rc-common && cargo test -p racecontrol-crate --lib && cargo test -p rc-agent-crate --lib
```

New unit tests to add:

| Test | File | What It Verifies |
|------|------|-----------------|
| `test_wait_time_60s_in_race_block` | `ac_server.rs` | Generated INI has `WAIT_TIME=60` in [RACE] block |
| `test_concurrent_sessions_different_ports` | `ac_server.rs` | Two sessions get different port allocations |
| `test_group_member_disconnected_event_serde` | `protocol.rs` | New event serializes/deserializes correctly |

### Integration Tests (require 2+ pods at venue)

Run from James's machine or server (.23):

```bash
# Full MP E2E (existing + new)
RC_BASE_URL=http://192.168.31.23:8080/api/v1 bash tests/e2e/api/multiplayer.sh
```

**Test matrix (must run manually on-site):**

| Test | Pods | Steps | Pass Criteria |
|------|------|-------|---------------|
| Simultaneous launch | 2 | Book MP via staff endpoint, observe both pods | Both pods show AC loading within 5s of each other |
| Lap recording | 2 | Drive 3 laps on each pod during MP session | `GET /laps?driver_id=X` returns laps for both drivers |
| Disconnect + rejoin | 2 | Kill AC on pod A after 30s, wait 15s | Pod A's billing paused, AC re-launches on pod A, Pod B unaffected |
| Disconnect permanent | 2 | Kill AC on pod A, unplug pod A from network | Pod A's billing pauses and eventually ends (timeout), Pod B continues |
| Atomic debit failure | 2 | Driver B has 0 credits, book MP with A+B | HTTP error, Driver A balance unchanged |
| Concurrent groups | 4 | Two groups of 2, different tracks | Both acServer instances running, different ports, no interference |
| Result collection | 2 | Complete a 5-lap race, stop session | `multiplayer_results` has entries for both drivers with positions and lap times |

### Smoke Test (quick verification after deploy)

```bash
# 1. Server health
curl -s http://192.168.31.23:8080/api/v1/health | jq .build_id

# 2. Book a 2-pod MP session
curl -s -X POST http://192.168.31.23:8080/api/v1/terminal/book-multiplayer \
  -H "Content-Type: application/json" \
  -H "x-terminal-secret: rp-terminal-2026" \
  -d '{"driver_ids":["driver_test_trial","<driver2>"],"pod_ids":["pod_1","pod_2"],"pricing_tier_id":"tier_30min","game":"assetto_corsa","track":"monza","car":"ks_ferrari_488_gt3"}'

# 3. Verify both pods show AC launching
curl -s http://192.168.31.23:8080/api/v1/games/active | jq '.games[] | {pod_id, game_state, sim_type}'

# 4. After 1 minute, check laps
curl -s http://192.168.31.23:8080/api/v1/laps?limit=10 | jq '.[].driver_id'

# 5. Stop and verify cleanup
curl -s -X POST http://192.168.31.23:8080/api/v1/games/stop -H "Content-Type: application/json" -d '{"pod_id":"pod_1"}'
curl -s -X POST http://192.168.31.23:8080/api/v1/games/stop -H "Content-Type: application/json" -d '{"pod_id":"pod_2"}'
```

---

## 7. Risk Assessment

| Risk | Severity | Likelihood | Mitigation |
|------|----------|------------|------------|
| acServer.exe not installed on server .23 | HIGH | MEDIUM | Check `[ac_server] acserver_path` in racecontrol.toml. If missing, install AC dedicated server from Steam (app 302550). Phase cannot work without it. |
| AssettoServer needed for AI but not installed | MEDIUM | MEDIUM | Current code already handles AssettoServer via `extra_cfg.yml`. If only vanilla acServer is available, AI fillers won't work but human-only MP works fine. |
| Pod loads AC slowly (>60s) and misses race start | MEDIUM | LOW | `WAIT_TIME=60` in [RACE] block. If a pod takes >60s, it joins mid-race (AC allows late join with `PICKUP_MODE_ENABLED=1`, already set). |
| Billing pause on disconnect creates free-ride exploit | LOW | LOW | Billing pauses for max 60s (crash recovery timeout). If pod doesn't reconnect, billing ends. Staff can manually end via dashboard. |
| Concurrent acServer instances exhaust ports | LOW | LOW | PortAllocator has 16 slots. Max 8 pods = max 4 concurrent 2-pod sessions. 16 >> 4. |
| LapCompleted events not firing in MP mode | MEDIUM | MEDIUM | rc-agent reads AC shared memory which works identically in SP and MP. However: verify that `acpmf_physics` shared memory is populated when AC is connected to a server (not just standalone). Test on a real pod. |
| Result files not written by acServer | MEDIUM | LOW | `collect_results()` checks both `{session_dir}/results/` and `{acserver_dir}/results/`. If neither has results, it returns empty vec (no crash). Log warning. |
| Windows firewall blocks acServer ports | MEDIUM | MEDIUM | acServer needs UDP and TCP ports open. PortAllocator range is 9600-9615 (UDP/TCP) and 8081-8096 (HTTP). Add firewall rules on server .23 if not already present: `netsh advfirewall firewall add rule name="AC Server" dir=in action=allow protocol=any localport=9600-9615,8081-8096` |

---

## 8. Deploy Checklist

After all code changes are committed:

### Server (.23)
- [ ] `git pull` on server
- [ ] `cargo build --release --bin racecontrol`
- [ ] Deploy via `deploy-server.sh` (follows standing rules)
- [ ] Verify `build_id` matches
- [ ] Verify `acServer.exe` exists at configured path
- [ ] Verify firewall rules for port range 9600-9615, 8081-8096
- [ ] Run DB migration (auto on startup)

### Pods (all 8)
- [ ] No rc-agent changes needed for this phase (agent already handles MP launch)
- [ ] Verify AC is installed on all pods
- [ ] Verify Content Manager is installed on all pods (for `acmanager://` URI handling)

### Cloud (Bono VPS)
- [ ] `git pull` + `cargo build --release` + restart PM2
- [ ] Cloud racecontrol won't run acServer (no AC installed) but DB schema must be in sync

### Frontends
- [ ] No frontend changes in this phase (dashboard already shows MP state)

---

## 9. What NOT to Change

These components are working correctly and must NOT be modified:

1. **`atomic_multi_debit()`** — MULT-04 is already satisfied. Don't touch wallet/billing atomicity.
2. **`start_ac_lan_for_group()`** AI filler logic — Already generates correct entry_list.ini with AI=fixed entries.
3. **`on_member_validated()` flow** — PIN validation -> all_validated -> auto-start works correctly.
4. **`book_multiplayer_kiosk()`** — Kiosk self-service MP booking is complete and tested.
5. **`generate_server_cfg_ini()` / `generate_entry_list_ini()`** — INI generation is tested with 15 unit tests. No changes needed.
6. **PortAllocator** — Fully tested with allocation, release, cooldown, and exhaustion tests.
7. **Lobby sync monitor** — Polls acServer /INFO correctly, tracks client count, broadcasts phases.

---

## 10. Open Questions for James

1. **Is acServer.exe installed on Racing-Point-Server (.23)?** Check: `dir "C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\server\acServer.exe"`. If not, install via Steam (app 302550: "Assetto Corsa Dedicated Server").

2. **Is AssettoServer installed?** If we want AI opponents in MP races (currently coded for it), AssettoServer must replace vanilla acServer. Check if it's at the configured path. If not, download from https://assettoserver.org/ and update `acserver_path` in racecontrol.toml.

3. **Firewall rules on .23?** Run `netsh advfirewall firewall show rule name=all | findstr "9600 8081"`. If no rules match, add them per the risk assessment section.

4. **AC shared memory in MP mode?** This is the biggest unknown. When AC connects to a dedicated server, does it still write to `acpmf_physics` / `acpmf_graphics` shared memory? If yes, rc-agent telemetry works unchanged. If no, we need an alternative (acServer result files only, no real-time laps). Test by: launching a 2-pod MP session, then checking rc-agent logs for `LapCompleted` events.
