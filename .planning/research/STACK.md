# Stack Research

**Domain:** Rust async — WebSocket resilience, process supervision, deployment reliability
**Researched:** 2026-03-13
**Confidence:** HIGH

## Context

This is a stack dimension research for an existing Rust/Axum + Node.js pod management system.
The project constraint is explicit: **no new dependencies** where existing crates cover the need.
All recommendations below respect that constraint. Where a new crate is genuinely required it is
flagged and justified.

Existing locked versions (from workspace Cargo.toml):

| Crate | Locked version |
|-------|---------------|
| tokio | 1 (full features) |
| axum | 0.8 (ws + macros) |
| tokio-tungstenite | 0.26 |
| futures-util | 0.3 |
| reqwest | 0.12 |
| serde / serde_json | 1 |
| chrono | 0.4 |
| tracing | 0.1 |
| anyhow | 1 |
| thiserror | 2 |
| toml | 0.8 |

---

## Recommended Stack

### Core Technologies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| tokio | 1 (full) | Async runtime — timers, task spawning, process management, channels | Already the runtime. `tokio::time::sleep`, `tokio::spawn`, `tokio::process::Command` cover every reliability primitive needed without adding dependencies. |
| tokio-tungstenite | 0.26 | WebSocket client in rc-agent (connects to rc-core) | Already used. 0.26 is the current stable release as of early 2026. Provides `connect_async`, message framing, ping/pong at the protocol level. |
| axum | 0.8 | WebSocket server in rc-core (accepts agent connections) | Already used with `features = ["ws"]`. axum 0.8's WebSocket handler exposes the underlying tungstenite Message type directly — no adapter layer needed. |
| futures-util | 0.3 | Stream/sink combinators for WebSocket message loops | Already used. `StreamExt::next()` + `SinkExt::send()` is the standard non-blocking WebSocket read/write pattern in tokio-tungstenite. |

### Supporting Libraries — Reliability Primitives

All of these are already in the workspace. No new crates needed.

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::time | (in tokio 1) | Reconnect timers, health check poll intervals, backoff delays | Use `tokio::time::sleep(duration).await` inside reconnect loops. Use `tokio::time::timeout(duration, future)` to bound health checks that could hang. |
| tokio::process::Command | (in tokio 1) | Shell out to `send_email.js` for Gmail alerts | Use only for email notifications. `kill_on_drop(true)` ensures the Node process is cleaned up if rc-core exits. |
| tokio::sync::mpsc | (in tokio 1) | Channel between WebSocket read task and rest of rc-agent | Use unbounded channel for low-volume control messages (heartbeat, commands). The existing pattern in rc-agent (mpsc for lock screen events) is correct — extend it, don't replace it. |
| chrono | 0.4 | Timestamps for backoff state (`last_attempt_at: Option<DateTime<Utc>>`) | Use `Utc::now()` in `EscalatingBackoff`. Serialize with `serde` feature (already enabled) for persistence if needed. |
| tracing | 0.1 | Structured event logging for all watchdog state transitions | Use `tracing::warn!` for recoverable failures, `tracing::error!` for escalated alerts. Include pod_id as a field: `tracing::warn!(pod_id = %pod_id, "Restart verification failed")`. |
| anyhow | 1 | Error propagation in health check async fns | Use `anyhow::Result<bool>` in verification functions. The existing `check_rc_agent_health` pattern in pod_healer.rs is correct — return `Ok(true)` on pod-agent exec failure (safe default). |

### Development Tools

| Tool | Purpose | Notes |
|------|---------|-------|
| cargo test | Unit test framework for all reliability logic | Run `cargo test -p rc-common -- watchdog` for backoff unit tests. Run `cargo test -p rc-core -- email_alerts` for alerter tests. Full suite: `cargo test --workspace`. |
| cargo clippy | Lint — catches common async pitfalls | Pay attention to `clippy::await_holding_lock` — the pod monitor holds no locks across await points, but verify this when adding shared backoff state to AppState. |

---

## Installation

No new crates. All reliability work uses the existing workspace.

```toml
# No additions to workspace Cargo.toml needed.
# The one module change is in rc-common/Cargo.toml if EscalatingBackoff
# needs chrono (add it there — chrono is already a workspace dep):

# crates/rc-common/Cargo.toml
[dependencies]
serde       = { workspace = true }
serde_json  = { workspace = true }
chrono      = { workspace = true }   # add this line
uuid        = { workspace = true }
```

---

## Alternatives Considered

### WebSocket Reconnection

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| Hand-rolled reconnect loop in rc-agent using `tokio::time::sleep` + `connect_async` | `tokio-retry` crate | tokio-retry adds a crate for a 15-line function. The reconnect loop is not complex: sleep → connect → if error sleep longer → repeat. The backoff sequence is fixed (not exponential), which makes a step table clearer than a retry policy object. |
| Hand-rolled reconnect loop | `backoff` crate (exponential) | Exponential backoff is overkill here. The schedule is 5s → 15s → 30s → 60s (agent reconnect) and 30s → 2m → 10m → 30m (watchdog). Fixed step tables are easier to reason about and tune. |

### Email Alerting

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| Shell out to `send_email.js` via `tokio::process::Command` | `lettre` 0.11 crate | lettre adds ~3 transitive deps and requires either an App Password (may not be available on Google Workspace) or hand-rolled OAuth2 token refresh logic. `send_email.js` already handles OAuth2 token refresh and is tested working. |
| Shell out to `send_email.js` | Direct Gmail API via `reqwest` | Would require implementing OAuth2 access-token-from-refresh-token flow: one extra HTTP call to Google's token endpoint, token caching, error handling. ~200 lines of boilerplate for functionality `send_email.js` already provides. |
| Shell out to `send_email.js` | `lettre` with XOAUTH2 mechanism | lettre supports XOAUTH2 but you still need to refresh the access token yourself before passing it to lettre. Same token-refresh problem as above. |

### Process Supervision

| Recommended | Alternative | Why Not |
|-------------|-------------|---------|
| `EscalatingBackoff` struct in rc-common (step table, reset on recovery) | External supervisor (systemd, Windows Service, NSSM) | Pod processes already have a 3-tier supervision stack (watchdog.bat → pod_monitor → pod_healer). The problem is not that supervision is missing — it's that the fixed cooldowns cause crash loops. Adding an external supervisor adds ops complexity without solving the cooldown problem. |
| Shared `EscalatingBackoff` state in `AppState` | Separate backoff state per supervision tier | pod_monitor and pod_healer both restart rc-agent. Without shared state they race and double-restart. Shared state in AppState (behind `RwLock`) prevents this with minimal overhead. |

---

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `exponential-backoff` or `tokio-retry` crates | Adds dependency for a trivial function. The reconnect and watchdog schedules are fixed step tables, not exponential curves — a Vec<Duration> is cleaner and more explicit. | Hand-rolled `EscalatingBackoff` struct in rc-common (already designed, see 05-RESEARCH.md) |
| `lettre` crate for email | Requires new SMTP/OAuth infrastructure when `send_email.js` already works. Google Workspace App Password support is unverified for racingpoint.in. | `tokio::process::Command` shell-out to existing `send_email.js` |
| `notify` crate for file watching | Not needed for this milestone — config is read at startup, binary deploy triggers a restart. | Nothing; config is validated at startup via `fail-fast` pattern |
| `sysinfo` in rc-core | rc-core runs on the server, not the pods. Remote process health is checked via pod-agent `/exec tasklist` — not local sysinfo. `sysinfo` is correctly confined to rc-agent only. | pod-agent `/exec` for remote process checks |
| Blocking `std::process::Command` for email | Blocks the async executor thread during Node.js startup (~50-200ms). In a tokio runtime this starves other tasks on that thread. | `tokio::process::Command` (async, non-blocking) |
| Fixed cooldown constants | The existing `HEAL_COOLDOWN_SECS = 600` causes infinite crash loops for a pod with a broken binary — it restarts every 10 minutes forever with no escalation or alert. | `EscalatingBackoff` with step table and exhaustion detection |

---

## Stack Patterns by Variant

**If rc-agent loses WebSocket connection to rc-core (game launch, CPU spike, network blip):**
- Use a reconnect loop with fixed step delays: 5s → 15s → 30s → 60s (cap at 60s)
- Reset attempt counter on successful reconnect
- Log at `warn!` level on each attempt, `error!` only after 3 consecutive failures
- Do NOT exit the process on disconnect — the reconnect loop IS the resilience

**If a pod's rc-agent crashes and fails to restart:**
- Use `EscalatingBackoff` (30s → 2m → 10m → 30m) — already designed in 05-RESEARCH.md
- After each restart command, spawn a verification task (non-blocking `tokio::spawn`)
- Trigger email alert when: (a) verification fails after 60s, OR (b) backoff is exhausted (attempt >= 4)
- Rate-limit emails: 1 per pod per 30min, 1 venue-wide per 5min (aggregate multi-pod failures)

**If pod-agent exec returns success but rc-agent is still unhealthy:**
- Check at 5s, 15s, 30s, 60s after restart (cumulative, not interval)
- Treat "WebSocket connected but lock screen unresponsive" as partial success — Session 0 limitation, not a failure
- Treat "process not running after 60s" as hard failure → email alert

**If deploying a new rc-agent binary to pods:**
- Explicit sequence: kill → delete stale binary → download → verify file size → start → verify connected
- Use pod-agent `/exec` for each step (idempotent commands)
- Never declare success until WebSocket reconnects from that pod (`state.agent_senders.contains_key`)

---

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| tokio-tungstenite 0.26 | tokio 1 | Requires tokio 1.x runtime. Confirmed compatible — 0.26 was released tracking tokio 1 stable. |
| axum 0.8 | tower 0.5, tower-http 0.6 | axum 0.8 dropped tower 0.4. Already correct in rc-core Cargo.toml. |
| reqwest 0.12 | tokio 1, rustls/native-tls | rc-core uses `features = ["json"]`. rc-agent uses `features = ["json"]`. No conflict. |
| thiserror 2 | anyhow 1 | Compatible. thiserror 2 is current (released 2024). rc-common uses both. |
| chrono 0.4 (serde) | serde 1 | `features = ["serde"]` already enabled in workspace. Adding chrono to rc-common Cargo.toml is safe — same version. |

---

## Sources

- **Codebase inspection** (HIGH confidence) — workspace Cargo.toml, rc-core/Cargo.toml, rc-agent/Cargo.toml, rc-common/Cargo.toml, pod_monitor.rs (existing EscalatingBackoff import confirmed at line 21)
- **05-RESEARCH.md** (HIGH confidence) — Watchdog hardening research from HUD project, 2026-03-12; patterns already validated against live codebase
- **tokio docs** (HIGH confidence) — `tokio::process::Command`, `tokio::time`, `tokio::sync::mpsc` — all stable in tokio 1.x
- **axum 0.8 changelog** (HIGH confidence) — WebSocket feature unchanged from 0.7→0.8 for handler API; `features = ["ws"]` confirmed
- **tokio-tungstenite 0.26 crates.io** (MEDIUM confidence) — version confirmed from Cargo.toml lock; 0.26 is current stable as of research date

---

## Confidence Assessment

| Area | Confidence | Reason |
|------|------------|--------|
| Core async stack | HIGH | All versions locked in existing Cargo.toml, no ambiguity |
| WebSocket resilience patterns | HIGH | Existing code (rc-agent main.rs, pod_monitor.rs) shows current implementation; gaps are clear |
| Watchdog / backoff design | HIGH | 05-RESEARCH.md is a complete prior-art analysis of this exact codebase |
| Email shell-out approach | MEDIUM | Works on James's machine (.27). Node.js availability on server (.23) is unverified — must check before implementation |
| No-new-deps claim | HIGH | Confirmed by inspecting all four Cargo.toml files; every required primitive exists in tokio 1 |

---

*Stack research for: RaceControl Reliability & Connection Hardening*
*Researched: 2026-03-13*
