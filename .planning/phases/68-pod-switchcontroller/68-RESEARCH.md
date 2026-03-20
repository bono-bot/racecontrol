# Phase 68: Pod SwitchController - Research

**Researched:** 2026-03-20
**Domain:** Rust async WebSocket runtime URL switching, self-monitor guard, rc-agent protocol extension
**Confidence:** HIGH

## Summary

Phase 68 adds runtime WebSocket failover switching to rc-agent. Today `config.core.url` is a plain `String` read once at startup and used directly by `connect_async()` in the outer reconnect loop at line ~933. The entire change is surgical: replace that `String` with an `Arc<RwLock<String>>` so the reconnect loop can pick up a new URL without restarting the process, add a `SwitchController` variant to `CoreToAgentMessage`, and add a `last_switch_time` guard to `self_monitor.rs` so the WS-dead watchdog does not fight the intentional disconnect that occurs during a switch.

The codebase has a clean seam for all three changes. The reconnect loop already `break`s from the inner event loop on any WS error and immediately retries with the URL in the outer loop — there is no caching of the URL inside the inner loop. `self_monitor.rs` is a standalone `tokio::spawn` task that reads only `HeartbeatStatus.ws_connected` (an `Arc<AtomicBool>`); adding a shared `Arc<AtomicU64>` (epoch-millis timestamp of last switch) threads cleanly between main.rs and self_monitor.rs without introducing any new lock types.

The only cross-crate change is adding `SwitchController { target_url: String }` to `CoreToAgentMessage` in `rc-common/protocol.rs`. racecontrol (server side) needs a new handler or route to send that message to individual pods or broadcast to all; this is an additive change to the existing pod WS broadcast infrastructure.

**Primary recommendation:** Use `Arc<AtomicU64>` (epoch millis) for `last_switch_time` — no RwLock needed, no async overhead, trivially shareable between main.rs and self_monitor.rs via the same `Arc<HeartbeatStatus>` pattern that is already in place.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Failover URL Configuration
- Add `failover_url` field to `[core]` section of `rc-agent.toml` on all pods
- Value: `ws://100.70.177.44:8080/ws/agent` (Bono VPS Tailscale IP, discovered in Phase 66)
- `core.url` remains the primary (LAN server .23): `ws://192.168.31.23:8080/ws/agent`
- `failover_url` is optional with `#[serde(default)]` — missing field means no failover capability (backward compatible)
- Deploy via pendrive update to all 8 pods (update `rc-agent.toml` on each pod's `C:\RacingPoint\rc-agent.toml`)

#### Runtime URL Switching (Arc<RwLock<String>>)
- Refactor `config.core.url` from a startup-read `String` to `Arc<RwLock<String>>` shared across the WS reconnect loop
- The reconnect loop reads the current URL from the RwLock on each iteration (not cached)
- When `SwitchController` is received, write the new URL to the RwLock — the next reconnect iteration picks it up
- No need to forcefully close the current WS connection — the reconnect loop naturally retries on disconnect
- Store both `primary_url` (from config) and `failover_url` (from config) as immutable references — SwitchController toggles between them

#### SwitchController Message
- Add `SwitchController { target_url: String }` variant to `CoreToAgentMessage` enum in `rc-common/protocol.rs`
- Server (racecontrol) sends this to individual pods or broadcasts to all connected agents
- rc-agent handler: validates URL starts with `ws://` or `wss://`, writes to the shared `Arc<RwLock<String>>`, logs the switch, triggers a graceful WS close to force immediate reconnect to new URL
- If `target_url` doesn't match either `primary_url` or `failover_url`, reject with warning log (safety guard)

#### Self-Monitor Suppression
- Add `last_switch_time: Option<Instant>` to self_monitor's state (or pass via `Arc<AtomicU64>` for lock-free access)
- After `SwitchController` is received, set `last_switch_time = Some(Instant::now())`
- In self_monitor's WS-dead check: if `last_switch_time` is Some AND elapsed < 60s, skip the relaunch — the pod is intentionally reconnecting to a new URL
- After 60s, clear `last_switch_time` — normal monitoring resumes
- If the new URL is ALSO unreachable after 60s, self_monitor's normal WS_DEAD_SECS (300s) threshold kicks in and will eventually relaunch

#### Fleet Rollout
- Pod 8 canary: update rc-agent.toml with failover_url, rebuild + deploy rc-agent.exe, test SwitchController manually
- Verify: send SwitchController to Pod 8, confirm it reconnects to Bono VPS, send SwitchController back to .23, confirm it returns
- After canary passes: pendrive deploy to all 8 pods (toml + binary)
- rc-agent :8090 exec can push the toml change remotely as alternative to pendrive

### Claude's Discretion
- Exact placement of Arc<RwLock<String>> in the main.rs struct hierarchy
- Whether to use `Arc<AtomicU64>` (epoch millis) or `Arc<RwLock<Option<Instant>>>` for last_switch_time
- Whether SwitchController triggers an immediate WS close or just writes the URL and waits for natural disconnect
- Error handling for malformed SwitchController messages
- Logging format for switch events

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FAIL-01 | rc-agent has `failover_url` in CoreConfig pointing to Bono's racecontrol via Tailscale | `CoreConfig` struct at line ~175; `#[serde(default)]` pattern already used for all fields; `validate_config()` at line ~2809 to extend |
| FAIL-02 | rc-agent WS reconnect loop uses `Arc<RwLock<String>>` for runtime URL switching | Reconnect loop at line ~931 reads `config.core.url` once per iteration — trivial swap to `active_url.read().clone()`; inner loop never caches the URL |
| FAIL-03 | New `SwitchController` AgentMessage triggers rc-agent URL switch without process restart | `CoreToAgentMessage` enum ends at line 399 (protocol.rs); `other =>` catch-all at main.rs line 2733 confirms safe addition; serde adjacently-tagged pattern (`#[serde(tag = "type", content = "data")]`) already established |
| FAIL-04 | self_monitor.rs suppresses relaunch during intentional failover (`last_switch_time` guard) | `self_monitor::spawn()` receives only `Arc<HeartbeatStatus>`; `HeartbeatStatus` holds only `AtomicBool`/`AtomicU32` fields — add `last_switch_u64: AtomicU64` for epoch-millis timestamp; WS-dead check at lines 63–86 is the exact insertion point |
</phase_requirements>

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `tokio::sync::RwLock` | (tokio already in Cargo.toml) | Async-safe URL field shared between reconnect loop and SwitchController handler | Already used in project; RwLock needed for String (not Copy type) |
| `std::sync::atomic::AtomicU64` | stdlib | Lock-free last_switch_time timestamp in HeartbeatStatus | Matches existing AtomicBool/AtomicU32 pattern in HeartbeatStatus; no new dep |
| `serde` + `toml` | already in Cargo.toml | Deserialize new `failover_url: Option<String>` field in CoreConfig | Same serde/toml stack as all existing config fields |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `tokio_tungstenite::connect_async` | already in Cargo.toml | WS connection — reads active_url per iteration | No change needed; just reads from RwLock instead of config field directly |

**No new dependencies required for this phase.**

---

## Architecture Patterns

### Recommended Project Structure

No new files required. All changes are in:
```
crates/
├── rc-common/src/protocol.rs     # Add SwitchController variant to CoreToAgentMessage
├── rc-agent/src/main.rs          # CoreConfig, reconnect loop, SwitchController handler
└── rc-agent/src/self_monitor.rs  # last_switch_time guard (reads Arc<AtomicU64>)
```

### Pattern 1: Arc<AtomicU64> for last_switch_time (recommended)

**What:** Store epoch-millis of last SwitchController in `HeartbeatStatus` as `AtomicU64`. Zero means "no recent switch." self_monitor checks: if value != 0 AND (now_millis - value) < 60_000, skip WS-dead relaunch.

**When to use:** Whenever a flag must be shared between the main async task and a monitoring task without introducing new lock types. AtomicU64 has zero blocking.

**Example (HeartbeatStatus addition):**
```rust
// In crates/rc-agent/src/udp_heartbeat.rs — HeartbeatStatus struct
pub struct HeartbeatStatus {
    pub ws_connected: AtomicBool,
    pub game_running: AtomicBool,
    pub driving_active: AtomicBool,
    pub billing_active: AtomicBool,
    pub game_id: AtomicU32,
    /// Epoch-millis of last SwitchController received. 0 = no recent switch.
    /// self_monitor suppresses WS-dead relaunch for 60s after a switch.
    pub last_switch_ms: AtomicU64,
}
```

**self_monitor check (insertion point: lines 63–86 of self_monitor.rs):**
```rust
// Before the ws_dead_secs >= WS_DEAD_SECS check:
let last_switch_ms = status.last_switch_ms.load(Ordering::Relaxed);
let now_ms = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_millis() as u64;
let since_switch_ms = now_ms.saturating_sub(last_switch_ms);
let switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000;

if ws_dead_secs >= WS_DEAD_SECS {
    if switch_grace_active {
        tracing::info!(
            "[rc-bot] WS dead {}s but SwitchController received {}ms ago — suppressing relaunch",
            ws_dead_secs, since_switch_ms
        );
    } else {
        tracing::warn!("[rc-bot] WebSocket dead {}s — relaunching", ws_dead_secs);
        log_event(&format!("RELAUNCH: ws_dead={}s", ws_dead_secs));
        relaunch_self();
        continue;
    }
}
```

### Pattern 2: Arc<RwLock<String>> for active URL

**What:** Replace direct `config.core.url` reads in the reconnect loop with a cloned `Arc<RwLock<String>>` constructed from `config.core.url` at startup. Both the reconnect loop (reader) and the SwitchController handler (writer) receive a clone of the same Arc.

**Key insight:** The reconnect loop is at the TOP of the outer `loop { }` — each iteration calls `connect_async(&url)`. The URL read happens exactly once per reconnect attempt. `RwLock::read()` is async in tokio but since the WS loop already `await`s, this is zero friction.

**Example (main.rs startup, before reconnect loop):**
```rust
// After config is loaded, before the reconnect loop at ~line 931:
let active_url: Arc<tokio::sync::RwLock<String>> =
    Arc::new(tokio::sync::RwLock::new(config.core.url.clone()));
let primary_url: Arc<String> = Arc::new(config.core.url.clone());
let failover_url: Arc<Option<String>> = Arc::new(config.core.failover_url.clone());

// In reconnect loop (replaces line ~934):
let url = active_url.read().await.clone();
tracing::info!("Connecting to RaceControl at {}...", url);
let ws_result = tokio::time::timeout(
    Duration::from_secs(10),
    connect_async(&url),
).await;
```

**SwitchController handler (inside the inner select! loop message handler):**
```rust
rc_common::protocol::CoreToAgentMessage::SwitchController { target_url } => {
    // Safety guard: only allow configured URLs
    let is_primary = target_url == *primary_url;
    let is_failover = failover_url.as_ref().map_or(false, |f| target_url == *f);
    if !is_primary && !is_failover {
        tracing::warn!(
            "[switch] Rejected SwitchController — unknown target_url: {}",
            target_url
        );
    } else {
        tracing::info!("[switch] SwitchController: switching to {}", target_url);
        *active_url.write().await = target_url.clone();
        // Record switch time so self_monitor suppresses relaunch for 60s
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        heartbeat_status.last_switch_ms.store(now_ms, Ordering::Relaxed);
        log_event(&format!("SWITCH: target={}", target_url));
        // Trigger immediate reconnect: break out of inner loop
        break;
    }
}
```

### Pattern 3: CoreConfig extension (FAIL-01)

**What:** Add `failover_url: Option<String>` to `CoreConfig` with `#[serde(default)]`.

**Example:**
```rust
#[derive(Debug, Deserialize)]
struct CoreConfig {
    #[serde(default = "default_core_url")]
    url: String,
    #[serde(default)]
    failover_url: Option<String>,
}
```

`validate_config` extension — if `failover_url` is `Some`, validate it also starts with `ws://` or `wss://`:
```rust
if let Some(ref furl) = config.core.failover_url {
    let furl = furl.trim();
    if !furl.starts_with("ws://") && !furl.starts_with("wss://") {
        errors.push(format!(
            "core.failover_url must start with ws:// or wss://, got {:?}",
            furl
        ));
    }
}
```

### Pattern 4: SwitchController in protocol.rs (FAIL-03)

**What:** Add variant at the end of `CoreToAgentMessage` (before the closing `}`).

The enum uses `#[serde(tag = "type", content = "data")]` with `#[serde(rename_all = "snake_case")]`. The new variant will serialize as `{"type": "switch_controller", "data": {"target_url": "ws://..."}}`.

```rust
/// Phase 68: Command agent to switch its WebSocket target URL at runtime.
/// Agent reconnects to target_url on the next reconnect iteration without restarting.
/// self_monitor will suppress WS-dead relaunch for 60s after receiving this.
SwitchController {
    target_url: String,
},
```

### Anti-Patterns to Avoid

- **Cloning config.core.url into a local `let url = config.core.url.clone()` before the loop:** This defeats the whole purpose — the clone happens once at startup and the new URL is never seen. Always read from the RwLock inside the loop.
- **Using `Arc<Mutex<String>>` instead of `Arc<RwLock<String>>`:** Mutex is sync-only in tokio; RwLock is the correct async primitive here. The inner loop holds `ws_tx.send().await` which means we cannot hold a sync Mutex guard across an await point.
- **Using Arc<RwLock<Option<Instant>>> for last_switch_time:** Requires async reads in self_monitor, which is a sync-style task internally (uses blocking Instant/elapsed checks). AtomicU64 epoch-millis avoids any async overhead.
- **Forcing close of current WS before writing URL:** Creates a race where the reconnect loop might read the old URL before the write completes. Write the URL first, then break — the break triggers reconnect with the updated value.
- **Sending SwitchController to pods 1-7 before Pod 8 canary passes:** CONTEXT.md explicitly requires Pod 8 canary first.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Async-safe mutable string | Custom channel or message-passing workaround | `Arc<tokio::sync::RwLock<String>>` | Already in tokio dep; correct primitive for shared async state |
| Lock-free timestamp between tasks | Custom AtomicPair or spinlock | `AtomicU64` epoch-millis | Matches existing `HeartbeatStatus` patterns; single CAS operation |
| Protocol serde | Custom JSON encoder | Existing `#[serde(tag, content)]` pattern | All existing `CoreToAgentMessage` variants use this; adding one more is zero risk |

---

## Common Pitfalls

### Pitfall 1: URL Cached Before Reconnect Loop
**What goes wrong:** If the URL is extracted as `let url = active_url.read().await.clone()` OUTSIDE the outer `loop {}`, the switch never takes effect.
**Why it happens:** Easy to write the read once above the loop for "efficiency."
**How to avoid:** The read MUST be the first statement INSIDE the outer `loop { ... }` block at line ~931, not before it.
**Warning signs:** SwitchController handler logs "switching to X" but pod still reconnects to old URL.

### Pitfall 2: self_monitor Fires Within 60s of Switch
**What goes wrong:** Pod sends SwitchController, connection drops (expected), self_monitor fires `relaunch_self()` after `WS_DEAD_SECS=300s` — but 300s is long, so the real risk is a quick sequence: switch + new URL unreachable + self_monitor fires at next 60s check interval.
**Why it happens:** self_monitor checks every 60s (`CHECK_INTERVAL_SECS=60`). A switch that takes >60s to reconnect would trigger the guard.
**How to avoid:** `last_switch_ms` guard suppresses for 60s. The 60s window covers exactly one self_monitor check cycle after a switch.
**Warning signs:** rc-bot-events.log shows `RELAUNCH: ws_dead=Xs` within 120s of a SwitchController in the tracing logs.

### Pitfall 3: failover_url Missing = Silent No-Op
**What goes wrong:** If `rc-agent.toml` is deployed without `failover_url`, SwitchController rejects all URLs (neither primary match because of URL mismatch, nor failover because it's None).
**Why it happens:** `#[serde(default)]` makes the field optional — an old toml silently omits it.
**How to avoid:** In the SwitchController handler, log a clear warning: "SwitchController received but failover_url not configured — ignoring." Don't silently drop.
**Warning signs:** SwitchController sent from server, nothing happens on pod, no log entry about switching.

### Pitfall 4: AtomicU64 Not in HeartbeatStatus::new()
**What goes wrong:** `last_switch_ms` field added to struct but `HeartbeatStatus::new()` not updated, causing compile error or defaulting to garbage.
**Why it happens:** Struct update without updating constructor.
**How to avoid:** Initialize to `AtomicU64::new(0)` in `HeartbeatStatus::new()`. 0 = "no switch ever" sentinel.

### Pitfall 5: Inner Loop Breaks Lose WS Close Frame
**What goes wrong:** A bare `break` from the select! inner loop drops `ws_tx` without sending a Close frame to the server. Server gets an ungraceful disconnect.
**Why it happens:** `break` exits immediately. Tungstenite normally sends Close on graceful shutdown.
**How to avoid:** Before breaking, send `ws_tx.send(Message::Close(None)).await` with a `let _ =` ignore on error (connection may already be dead). This is consistent with how other `break` paths in the inner loop work.

---

## Code Examples

### Verified Pattern: Existing reconnect loop (main.rs ~line 931-937)
```rust
// Source: crates/rc-agent/src/main.rs line ~931-937 (read 2026-03-20)
loop {
    tracing::info!("Connecting to RaceControl core at {}...", config.core.url);
    let ws_result = tokio::time::timeout(
        Duration::from_secs(10),
        connect_async(&config.core.url),
    ).await;
    // ... match ws_result ...
```
After Phase 68, `config.core.url` becomes `active_url.read().await.clone()`.

### Verified Pattern: Existing HeartbeatStatus (udp_heartbeat.rs ~line 30-36)
```rust
// Source: crates/rc-agent/src/udp_heartbeat.rs lines 30-36 (read 2026-03-20)
pub struct HeartbeatStatus {
    pub ws_connected: AtomicBool,
    pub game_running: AtomicBool,
    pub driving_active: AtomicBool,
    pub billing_active: AtomicBool,
    pub game_id: AtomicU32,
}
// Add: pub last_switch_ms: AtomicU64,
```

### Verified Pattern: self_monitor WS-dead check (self_monitor.rs lines 63-86)
```rust
// Source: crates/rc-agent/src/self_monitor.rs lines 63-86 (read 2026-03-20)
if ws_dead_secs >= WS_DEAD_SECS {
    tracing::warn!("[rc-bot] WebSocket dead {}s — relaunching to reestablish", ws_dead_secs);
    log_event(&format!("RELAUNCH: ws_dead={}s (threshold={}s) — no AI needed", ws_dead_secs, WS_DEAD_SECS));
    relaunch_self();
    continue;
}
// Phase 68: add last_switch_ms guard BEFORE this block.
```

### Verified Pattern: CoreToAgentMessage serde format (protocol.rs lines 213-216)
```rust
// Source: crates/rc-common/src/protocol.rs lines 213-216 (read 2026-03-20)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum CoreToAgentMessage {
// SwitchController serializes as: {"type":"switch_controller","data":{"target_url":"ws://..."}}
```

### Verified Pattern: Existing other => catch-all (main.rs line 2733)
```rust
// Source: crates/rc-agent/src/main.rs line 2733 (read 2026-03-20)
other => {
    tracing::warn!("Unhandled CoreToAgentMessage: {:?}", other);
}
// SwitchController handler goes BEFORE this arm.
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Static URL string read at startup | `Arc<RwLock<String>>` read per reconnect iteration | Phase 68 | Enables runtime URL switching without process restart |
| No failover awareness in self_monitor | `last_switch_ms` guard suppresses WS-dead relaunch for 60s | Phase 68 | Prevents false-positive process kill during intentional reconnect |
| No `failover_url` in CoreConfig | `failover_url: Option<String>` with `#[serde(default)]` | Phase 68 | Backward compatible — old tomls still work |

---

## Open Questions

1. **Server-side SwitchController dispatch: new route or existing admin endpoint?**
   - What we know: racecontrol has a fleet exec infrastructure; pod WS connections are held in an `Arc<DashMap>` or similar
   - What's unclear: whether to add a dedicated `/api/v1/fleet/switch-controller` POST endpoint or reuse the WS broadcast path
   - Recommendation: research racecontrol's ws_handler or fleet routes during planning; CONTEXT.md defers this to Claude's discretion — it's an additive server-side change

2. **Do all 8 pods need the binary update, or just the toml?**
   - What we know: the `SwitchController` variant must be in the binary (`rc-common/protocol.rs`) for the handler to compile
   - What's unclear: can a pod running an old binary (without SwitchController) safely ignore the new message?
   - Recommendation: Yes — old binary will hit the `other =>` catch-all and log a warning, not crash. But failover does not work until binary is updated. Plan for binary + toml deploy together.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` + `#[tokio::test]` |
| Config file | Cargo.toml dev-dependencies (already configured) |
| Quick run command | `cargo test -p rc-agent 2>&1 \| tail -20` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| FAIL-01 | `failover_url: Some(...)` deserializes correctly; missing field = `None` | unit | `cargo test -p rc-agent validate_config` | ❌ Wave 0 (add to existing test module in main.rs) |
| FAIL-01 | `validate_config` rejects non-ws:// `failover_url` | unit | `cargo test -p rc-agent validate_config` | ❌ Wave 0 |
| FAIL-02 | `Arc<RwLock<String>>` write is visible to next loop iteration | unit | `cargo test -p rc-agent active_url` | ❌ Wave 0 |
| FAIL-03 | `SwitchController` serializes/deserializes round-trip correctly | unit | `cargo test -p rc-common` | ❌ Wave 0 (add to rc-common) |
| FAIL-03 | Safety guard rejects unknown target_url | unit | `cargo test -p rc-agent switch_controller` | ❌ Wave 0 |
| FAIL-04 | `last_switch_ms` guard suppresses relaunch within 60s | unit | `cargo test -p rc-agent self_monitor` | ❌ Wave 0 (add to self_monitor.rs test module) |
| FAIL-04 | Guard does NOT suppress after 60s elapsed | unit | `cargo test -p rc-agent self_monitor` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent 2>&1 | tail -30`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `crates/rc-agent/src/main.rs` test module — add `validate_config_accepts_failover_url`, `validate_config_rejects_non_ws_failover_url`
- [ ] `crates/rc-common/src/protocol.rs` test module — add `switch_controller_serde_round_trip`
- [ ] `crates/rc-agent/src/self_monitor.rs` test module — add `last_switch_guard_suppresses_within_60s`, `last_switch_guard_allows_after_60s`
- [ ] `crates/rc-agent/src/udp_heartbeat.rs` — `HeartbeatStatus::new()` must initialize `last_switch_ms: AtomicU64::new(0)` (compile-time enforced, no extra test needed)

---

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/main.rs` — CoreConfig struct (lines 174-178), reconnect loop (lines 931-981), CoreToAgentMessage dispatch (lines 1817-2736), validate_config (lines 2809-2836) — read directly 2026-03-20
- `crates/rc-agent/src/self_monitor.rs` — full file read directly 2026-03-20 — WS_DEAD_SECS=300, CHECK_INTERVAL_SECS=60, relaunch_self(), WS-dead check lines 63-86
- `crates/rc-common/src/protocol.rs` — CoreToAgentMessage enum (lines 216-399), serde tag/content format — read directly 2026-03-20
- `crates/rc-agent/src/udp_heartbeat.rs` — HeartbeatStatus struct (lines 30-36) — read directly 2026-03-20
- `.planning/phases/68-pod-switchcontroller/68-CONTEXT.md` — all locked decisions — read directly 2026-03-20

### Secondary (MEDIUM confidence)
- `.planning/STATE.md` — project constraints, Rust version 1.93.1 (AtomicU64 stable since 1.0), no new dep needed
- `.planning/config.json` — nyquist_validation: true confirmed

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new deps; tokio RwLock and AtomicU64 are stable stdlib/tokio primitives already in use
- Architecture: HIGH — code read directly; insertion points confirmed by line numbers
- Pitfalls: HIGH — derived from reading the actual loop structure and self_monitor logic

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (stable codebase; no external deps changing)
