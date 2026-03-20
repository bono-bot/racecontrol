# Phase 68: Pod SwitchController - Context

**Gathered:** 2026-03-20
**Status:** Ready for planning

<domain>
## Phase Boundary

Any rc-agent pod can switch its WebSocket target from .23 to Bono's VPS (and back) at runtime without a process restart. A new `SwitchController` message triggers the switch, and `self_monitor.rs` will not fight the intentional URL change. Pod 8 canary first, then fleet deploy.

</domain>

<decisions>
## Implementation Decisions

### Failover URL Configuration
- Add `failover_url` field to `[core]` section of `rc-agent.toml` on all pods
- Value: `ws://100.70.177.44:8080/ws/agent` (Bono VPS Tailscale IP, discovered in Phase 66)
- `core.url` remains the primary (LAN server .23): `ws://192.168.31.23:8080/ws/agent`
- `failover_url` is optional with `#[serde(default)]` — missing field means no failover capability (backward compatible)
- Deploy via pendrive update to all 8 pods (update `rc-agent.toml` on each pod's `C:\RacingPoint\rc-agent.toml`)

### Runtime URL Switching (Arc<RwLock<String>>)
- Refactor `config.core.url` from a startup-read `String` to `Arc<RwLock<String>>` shared across the WS reconnect loop
- The reconnect loop reads the current URL from the RwLock on each iteration (not cached)
- When `SwitchController` is received, write the new URL to the RwLock — the next reconnect iteration picks it up
- No need to forcefully close the current WS connection — the reconnect loop naturally retries on disconnect
- Store both `primary_url` (from config) and `failover_url` (from config) as immutable references — SwitchController toggles between them

### SwitchController Message
- Add `SwitchController { target_url: String }` variant to `CoreToAgentMessage` enum in `rc-common/protocol.rs`
- Server (racecontrol) sends this to individual pods or broadcasts to all connected agents
- rc-agent handler: validates URL starts with `ws://` or `wss://`, writes to the shared `Arc<RwLock<String>>`, logs the switch, triggers a graceful WS close to force immediate reconnect to new URL
- If `target_url` doesn't match either `primary_url` or `failover_url`, reject with warning log (safety guard)

### Self-Monitor Suppression
- Add `last_switch_time: Option<Instant>` to self_monitor's state (or pass via `Arc<AtomicU64>` for lock-free access)
- After `SwitchController` is received, set `last_switch_time = Some(Instant::now())`
- In self_monitor's WS-dead check: if `last_switch_time` is Some AND elapsed < 60s, skip the relaunch — the pod is intentionally reconnecting to a new URL
- After 60s, clear `last_switch_time` — normal monitoring resumes
- If the new URL is ALSO unreachable after 60s, self_monitor's normal WS_DEAD_SECS (300s) threshold kicks in and will eventually relaunch

### Fleet Rollout
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### rc-agent Core
- `crates/rc-agent/src/main.rs` — CoreConfig struct (line ~176), WS reconnect loop (line ~933), config.core.url references throughout
- `crates/rc-agent/src/self_monitor.rs` — WS_DEAD_SECS, relaunch_self(), CLOSE_WAIT strike counter
- `crates/rc-agent/src/remote_ops.rs` — RCAGENT_SELF_RESTART sentinel pattern (line ~489)

### Protocol
- `crates/rc-common/src/protocol.rs` — CoreToAgentMessage enum (line ~216), AgentMessage enum (line ~22) — SwitchController goes into CoreToAgentMessage

### Research
- `.planning/research/ARCHITECTURE.md` — Integration points, failover mechanics, SwitchController design notes
- `.planning/research/PITFALLS.md` — self_monitor conflict with failover switches, false positive risks
- `.planning/research/FEATURES.md` — Hysteresis pattern, failover trigger mechanics

### Phase 66 Discovery
- Server Tailscale IP: 100.71.226.83 (racing-point-server)
- Bono VPS Tailscale IP: 100.70.177.44 (srv1422716)
- All 8 pods have Tailscale IPs (100.92-127.x.x range)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `CoreConfig` struct in main.rs — already has `url: String` field, add `failover_url: Option<String>` alongside
- `validate_config()` function (line ~2808) — already validates `core.url` starts with ws:// — extend for failover_url
- `relaunch_self()` in self_monitor.rs — the function SwitchController must NOT trigger

### Established Patterns
- WS reconnect loop at line ~933: `loop { connect_async(&config.core.url) ... }` — this is where Arc<RwLock> replaces the direct string read
- Self-monitor checks `ws_connected` flag every 30s — the last_switch_time guard integrates into this existing check
- `CoreToAgentMessage` uses serde adjacently-tagged format — SwitchController follows the same pattern

### Integration Points
- main.rs reconnect loop: `config.core.url` → `active_url.read()` (the key refactor point)
- self_monitor.rs: add last_switch_time check before WS_DEAD_SECS relaunch
- rc-common/protocol.rs: add SwitchController variant to CoreToAgentMessage
- racecontrol/routes.rs or ws_handler: add ability to send SwitchController to specific pods or broadcast

</code_context>

<specifics>
## Specific Ideas

- Pod 8 is the sole canary target — never test SwitchController on pods 1-7 first (may have live customer sessions)
- The SwitchController message should carry the target URL string, not an enum like "primary"/"failover" — this keeps it flexible for future URL changes
- Safety: if target_url doesn't match either configured URL, log a warning and ignore — prevents accidental misdirection

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 68-pod-switchcontroller*
*Context gathered: 2026-03-20*
