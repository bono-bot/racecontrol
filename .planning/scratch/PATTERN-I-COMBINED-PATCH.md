# PATTERN-I-COMBINED-PATCH — Implementation Blueprint

Generated: 2026-04-19 IST by architect agent session `ac326656396839907`
Updated: 2026-04-19 03:30 IST by James with deploy-gap finding.
Scope: rc-agent WS half-open detection (Issue 1), HTTP launch fallback (Issue 2), agent_senders lock-across-await audit (Issue 3)

---

## Decisive context (2026-04-19 03:30 IST finding)

Pattern I Issue 1 is **already fixed in git at `424ca3dc`** ("feat(rc-agent): Pattern I part 2 — client-side WS liveness check in event_loop"). Introduces:

- `WS_LIVENESS_TIMEOUT_SECS = 90` at `event_loop.rs:32`
- `liveness_check_interval` in `ConnectionState` — fires every 15s
- `last_server_frame_at` refreshed on every Ok WS frame
- `select!` liveness arm at lines 2421-2441 that breaks the inner loop after 90s without a server frame

Pod 6's current binary is `e7e01ae3` which is **2 commits older** than `424ca3dc`. `git merge-base --is-ancestor 424ca3dc e7e01ae3` returns false. Commits between them:
```
424ca3dc feat(rc-agent): Pattern I part 2 — client-side WS liveness check
2bb47593 docs(SWAPLOG+CLAUDE): between-session server-swap audit log
e7e01ae3 feat(racecontrol): --build-id / --version introspection
```

**Therefore Issue 1 requires no new code — it requires a fleet deploy of HEAD.**

This blueprint's Issue 1 work is reduced to optional hardening:
- Add `Message::Ping` on heartbeat tick for defense-in-depth (architect's original recommendation; 1-line).
- Validate liveness guard actually fires on Pod 6 by observing a half-open event post-deploy.

---

## Issue 1 — Status after finding

| Item | Status | Action owner |
|---|---|---|
| Liveness guard code | DONE at `424ca3dc` | — |
| Fleet deploy of `424ca3dc` or later | PENDING | user sign-off, then James deploy script |
| Defense-in-depth: explicit Ping on heartbeat tick | OPTIONAL | low-risk 1-line add to event_loop.rs |
| Test: unit test with `#[cfg(test)]` constant override | OPTIONAL | same patch as above |
| TCP keepalive | NON-GOAL | tokio-tungstenite doesn't expose socket post-handshake |

---

## Issue 2 — HTTP launch fallback (new endpoint on pod :8090)

Orthogonal to Issue 1. Value: covers the 30-90s reconnect window when rc-agent is re-handshaking after a liveness break, during which a server→pod launch command would still drop.

### New endpoint on rc-agent

File: `crates/rc-agent/src/remote_ops.rs`. Add to protected_routes:
```
.route("/launch", post(launch_game_http))
```

Request body (new struct, same file or new `launch_types.rs`):
```rust
#[derive(Deserialize)]
pub struct HttpLaunchRequest {
    pub idempotency_key: String,  // session_id from server
    pub sim_type: String,         // "assetto_corsa", "f1_25", etc.
    pub launch_args: Option<String>, // JSON string, same as WS payload
    pub force_clean: bool,
}
```

Response body:
```rust
#[derive(Serialize)]
pub struct HttpLaunchResponse {
    pub ok: bool,
    pub idempotent: bool,   // true if this key was already processed
    pub error: Option<String>,
}
```

Idempotency: in-process `Mutex<HashSet<String>>` (max 16 entries, ring behavior).

### Channel-based dispatch (recommended)

Add `launch_cmd_tx: mpsc::Sender<LaunchGameCmd>` to `AppState`. HTTP handler sends a `LaunchGameCmd` onto the channel. Event loop's existing `select!` picks it up and dispatches via the same path as WS `LaunchGame`. This avoids threading `ws_tx`/`conn` into the HTTP context.

New file: `crates/rc-agent/src/launch_channel.rs` — defines `LaunchGameCmd` and helpers.

### Server-side fallback

File: `crates/racecontrol/src/game_launcher_ops.rs:318-328`. Existing loop attempts WS send twice; after both fail (`send_ok == false`), try HTTP:

```rust
if !send_ok {
    let pod_ip = {
        let pods = state.pods.read().await;
        pods.get(pod_id).map(|p| p.ip_address.clone())
    }; // lock dropped
    if let Some(ip) = pod_ip {
        let result = http_launch_on_pod(&state, &ip, &launch_msg, pod_id, session_id).await;
        send_ok = result.is_ok();
    }
}
```

`http_launch_on_pod`: `POST http://{ip}:8090/launch` with `X-Service-Key: {state.config.pods.sentry_service_key}`, serialized `HttpLaunchRequest`, 5s timeout.

### Tests

- `crates/rc-agent/tests/http_launch_test.rs` — POST /launch, assert 200 + LaunchGameCmd on channel.
- `crates/rc-agent/tests/http_launch_idempotent.rs` — POST twice with same key, second returns `idempotent:true`.
- `crates/racecontrol/tests/launch_http_fallback_test.rs` — empty `agent_senders`, mock HTTP server on localhost, call `send_launch_to_pod`, assert mock received exactly one POST.

---

## Issue 3 — agent_senders lock-across-await audit

Grep surfaced 60 `agent_senders.(read|write)` sites across 25 files.

### Confirmed GREEN (already use clone-snapshot-drop)

| File | Lines | Pattern |
|---|---|---|
| `ac_server.rs` | 252-258 | explicit snapshot Vec with comment "Clone senders, drop lock, then send" |
| `billing_session_end.rs` | 267-270, 464-467 | snapshot before `.await` |
| `billing_timer_expiry.rs` | 31-36, 160-163, 256-259 | `.cloned()` then drop |
| `config_push_handlers.rs` | 72 | `.cloned()` inline |
| `cloud_sync_pull.rs` | 173 | `.len()` only, no `.await` |

### Confirmed RED (lock held across `.await`) — FIX REQUIRED

| Priority | File | Lines | Fix |
|---|---|---|---|
| P0 | `game_launcher_ops.rs` | 320-326 | Clone `tx` inside lock scope; drop guard; await clone |
| P1 | `api/game_pod_controls.rs` | 23-28 | Same pattern |
| P1 | `api/game_pod_controls.rs` | 47-52 | Same pattern |
| P1 | `api/game_pod_controls.rs` | 67-72 | Same pattern |
| P1 | `api/game_pod_controls.rs` | 101-107 | Same pattern |
| P1 | `api/game_pod_controls.rs` | 130-133 | Same pattern |

### Fix template

Before:
```rust
let senders = state.agent_senders.read().await;
if let Some(tx) = senders.get(&pod_id) {
    let _ = tx.send(msg).await;  // lock held across .await
}
```

After:
```rust
let sender = {
    let senders = state.agent_senders.read().await;
    senders.get(&pod_id).cloned()
}; // guard dropped
if let Some(tx) = sender {
    let _ = tx.send(msg).await;
}
```

### TBD (need second-pass audit)

~25 sites across `auth/`, `billing_session_start.rs`, `billing_timer.rs`, `billing_timer_expiry_timeout.rs`, `bot_coordinator_recovery.rs`, `api/admin_tools.rs`, `api/customer_booking_continue.rs`, `api/debug_fixes.rs`, `api/debug_incidents.rs`, `api/pod_exec.rs`, `api/pod_mgmt.rs`, `api/pod_mgmt_bulk.rs`, `api/sync_failover.rs`, `config_push_full.rs`, `deploy.rs`, `deploy_awareness_fleet.rs`, `fleet_anomaly_detection.rs`, `fleet_health_api.rs`, `game_launcher_state.rs`, `game_launcher_support.rs`, `game_launcher_ops_relaunch.rs`, `ac_server_lifecycle.rs`. Some reads may be `.len()` only (GREEN by inspection), others may match the RED pattern. Deferred to Phase 2.

---

## Build sequence (bisectable commits)

1. **Commit 1 — Issue 3 P0+P1 fixes only** (6 confirmed RED sites). No behavior change. Safest commit. Can deploy independently.
2. **Commit 2 — Issue 1 defense-in-depth** (explicit `Message::Ping` on heartbeat tick + `#[cfg(test)]` constant override + unit test). Optional; not required if `424ca3dc` already deployed.
3. **Commit 3 — Issue 2 HTTP launch fallback** (new endpoint + channel + server-side fallback + tests).
4. **Commit 4 — Issue 3 TBD audit** (complete the remaining ~25 sites).

---

## Risk table

| Change | What could break | Mitigation |
|---|---|---|
| clone-snapshot-drop (Issue 3) | Any site that relied on lock being held to prevent concurrent map mutation between `get()` and `send()` | agent_senders entries are only written on WS connect/disconnect (main.rs), not on send. Concurrent get/send is safe without holding the lock. |
| Message::Ping in heartbeat (Issue 1 DiD) | Server rejects Ping frame | tungstenite RFC 6455 requires auto-pong; server uses tungstenite which complies. Canary one pod before fleet. |
| HTTP /launch endpoint (Issue 2) | Duplicate launches | Idempotency key + mutually-exclusive with WS (HTTP only fires when WS send returned Err) |
| HTTP /launch endpoint (Issue 2) | Billing guard bypass | Channel-based dispatch routes through same event loop path — billing_guard applied identically |

---

## Non-goals

1. Does NOT redesign the WS protocol — no new `CoreToAgentMessage` variants.
2. Does NOT fix concurrent launches on a single pod (existing `LaunchState` handles this).
3. Does NOT add WS Pong tracking as a new AtomicInstant — existing `last_server_frame_at` serves.
4. Does NOT implement TCP keepalive — tokio-tungstenite doesn't expose post-handshake socket.
5. Does NOT change server-side WS handling — tungstenite auto-Pongs.
6. Does NOT solve Gap 4 `RCAGENT_SERVICE_KEY` mismatch — separate deploy concern.
7. Does NOT deploy anything — blueprint only.

---

## Deploy note

Pattern I is definitively CLOSED when:

1. Fleet shipped with `424ca3dc` or later.
2. A future half-open event on ANY pod produces log line `WS liveness: no server frame in Ns — forcing reconnect` within ~105s.
3. `GET http://<pod>:8090/debug/ws-state` shows `phase: "connected"` and `recent_failures: []` for 24h.
4. `OPEN-PATTERNS.md` Pattern I row updated to CLOSED with the observed liveness-break timestamp.

---

## Appendix — files read during architect session

- `crates/rc-agent/src/event_loop.rs` (lines 24-32, 82-94, 2421-2441, 2444-2451, heartbeat tick near 385)
- `crates/rc-agent/src/self_monitor.rs` (lines 40-150)
- `crates/rc-agent/src/main.rs` (lines 2060-2400)
- `crates/rc-agent/src/remote_ops.rs` (protected_routes block, approx lines 256-274)
- `crates/rc-agent/src/udp_heartbeat.rs` (lines 1-200)
- `crates/racecontrol/src/game_launcher_ops.rs` (lines 318-328)
- `crates/racecontrol/src/api/game_pod_controls.rs` (lines 23, 47, 67, 101, 130)
- `crates/racecontrol/src/api/game_launch.rs` (lines 55-270)
- `crates/racecontrol/src/state.rs` (line 45, `pods: RwLock<HashMap<String, PodInfo>>`)
- `crates/racecontrol/src/billing_session_end.rs` (lines 267-270, 464-467 — canonical GREEN pattern)
- `crates/racecontrol/src/ac_server.rs` (lines 240-270 — canonical GREEN pattern)
- `docs/ARCHITECTURE.md` (section 17.1 Launch Chain)
