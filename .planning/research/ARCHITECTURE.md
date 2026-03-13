# Architecture Research

**Domain:** Pod supervision, connection resilience, deployment reliability
**Researched:** 2026-03-13
**Confidence:** HIGH — derived from direct codebase inspection of all relevant modules

## Standard Architecture

### System Overview

```
Racing-Point-Server (.23) — rc-core (port 8080)
┌──────────────────────────────────────────────────────────────┐
│  AppState (Arc<AppState>)                                     │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────┐  │
│  │  pod_monitor   │  │  pod_healer    │  │  email_alerts  │  │
│  │  (10s loop)    │  │  (120s loop)   │  │  EmailAlerter  │  │
│  └───────┬────────┘  └───────┬────────┘  └───────┬────────┘  │
│          │                   │                   │           │
│  ┌───────▼───────────────────▼───────────────────▼────────┐  │
│  │  Shared state: agent_senders, pod_backoffs,             │  │
│  │  udp_heartbeat timestamps, billing state, DB            │  │
│  └───────────────────────────────────────────────────────-─┘  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │  udp_heartbeat.rs — receives UDP from all pods (6s TTL)  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────┬────────────────────────────────────────┘
                      │ WebSocket (ws://<pod>:8080)
                      │ HTTP (pod-agent :8090)
                      │ UDP heartbeat (server :9996/20777/etc)
          ────────────┼─────────────────────────
          │           │           │           │
    Pod 1 (.89)  Pod 2 (.33)  ...        Pod 8 (.91)
    ┌──────────────────────────────────────────┐
    │  rc-agent (port 18923 lock screen HTTP)  │
    │  pod-agent (port 8090 exec HTTP)         │
    │  watchdog.bat / HKLM Run key             │
    └──────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Owned By |
|-----------|----------------|----------|
| `pod_monitor.rs` | Detects heartbeat staleness, owns restart decisions, calls pod-agent /exec | rc-core |
| `pod_healer.rs` | Deep diagnostics (disk, mem, zombie procs), rule-based auto-fix, AI escalation | rc-core |
| `email_alerts.rs` | Rate-limited email notifications via send_email.js shell-out | rc-core |
| `rc-common/watchdog.rs` | `EscalatingBackoff` struct shared between monitor and healer | rc-common |
| `udp_heartbeat.rs` (core) | Receives UDP from pods, updates last-seen map in AppState | rc-core |
| `udp_heartbeat.rs` (agent) | Sends UDP heartbeat to rc-core every 2s | rc-agent |
| `pod-agent` (Node.js, :8090) | Remote exec endpoint, local process watchdog, HTTP file download | each pod |
| `watchdog.bat` + HKLM Run | Local process revival on pod, Session 1 startup guarantee | each pod |
| `ws/mod.rs` | WebSocket server, maintains `agent_senders` map by pod_id | rc-core |

## Recommended Project Structure

Changes relative to current codebase:

```
crates/rc-common/src/
    watchdog.rs          DONE — EscalatingBackoff struct + 14 tests

crates/rc-core/src/
    email_alerts.rs      DONE — EmailAlerter + 10 tests
    pod_monitor.rs       MODIFY — use EscalatingBackoff, add post-restart verification,
                                  share backoff state via AppState
    pod_healer.rs        MODIFY — use shared EscalatingBackoff, remove restart ownership
                                  (defer to pod_monitor), keep diagnostics/healing
    config.rs            MODIFY — add WatchdogConfig with email fields, backoff tuning
    state.rs             MODIFY — add pod_backoffs: HashMap<PodId, EscalatingBackoff>
                                  and email_alerter: Arc<Mutex<EmailAlerter>>
    ws/mod.rs            MODIFY — WebSocket ping/pong keepalive to prevent drop during
                                  game launch / CPU spikes
```

### Structure Rationale

- **EscalatingBackoff in rc-common:** Both pod_monitor and pod_healer need it. Placing it in rc-common avoids a circular dep between the two rc-core modules and makes it unit-testable in isolation.
- **EmailAlerter in rc-core:** Owns Gmail shell-out. Lives in rc-core because only rc-core has access to AppState and pod health data. rc-agent never sends emails directly.
- **Shared backoff in AppState:** The single source of truth for per-pod restart state. Prevents pod_monitor and pod_healer from making simultaneous restart decisions.

## Architectural Patterns

### Pattern 1: Shared Escalating Backoff via AppState

**What:** `AppState` holds a `HashMap<String, EscalatingBackoff>` (keyed by pod_id). Both `pod_monitor` and `pod_healer` access it through the same `Arc<RwLock<...>>`. Only `pod_monitor` may call `record_attempt`. `pod_healer` reads the state to decide whether to defer.

**When to use:** Any restart decision in either monitor or healer tier.

**Trade-offs:** Adds a lock acquisition on every monitor cycle (10s), but the map has at most 8 entries and the critical section is microseconds.

**Example:**
```rust
// In state.rs
pub struct AppState {
    // ... existing fields ...
    pub pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>,
    pub email_alerter: Mutex<EmailAlerter>,
}

// In pod_monitor.rs — restart decision
let mut backoffs = state.pod_backoffs.write().await;
let backoff = backoffs.entry(pod_id.clone()).or_insert_with(EscalatingBackoff::new);
if backoff.ready(now) {
    backoff.record_attempt(now);
    // issue restart
} else {
    tracing::debug!("Pod {} restart cooldown not elapsed", pod_id);
}

// In pod_healer.rs — respect monitor ownership
let backoffs = state.pod_backoffs.read().await;
if let Some(b) = backoffs.get(pod_id) {
    if b.exhausted() {
        // pod_monitor owns restart; healer focuses on diagnostics
    }
}
```

### Pattern 2: Post-Restart Verification as Detached Task

**What:** After sending a restart command, `tokio::spawn` a verification task that does NOT hold any locks. It checks at 5s, 15s, 30s, 60s. Uses three signals: (a) `tasklist` via pod-agent, (b) `state.agent_senders.contains_key`, (c) lock screen HTTP 200 at :18923. Reports partial recovery (Session 0: process + WS but no lock screen) vs full recovery vs failure.

**When to use:** After every restart attempt in pod_monitor. Never block the 10s loop.

**Trade-offs:** Verification happens in the background. The monitor loop may run 1-2 more cycles before verification completes — that is correct. The cooldown timer already prevents duplicate restarts during verification.

**Example:**
```rust
// After restart command issued in pod_monitor:
let state_clone = Arc::clone(&state);
let pod_id_clone = pod_id.clone();
let pod_ip_clone = pod_ip.clone();
tokio::spawn(async move {
    verify_restart_health(state_clone, pod_id_clone, pod_ip_clone).await;
});

// verify_restart_health checks at intervals and:
// - On success: calls backoff.reset() via state.pod_backoffs
// - On partial (Session 0): logs known limitation, does NOT trigger email
// - On failure: sends email alert via state.email_alerter
```

### Pattern 3: WebSocket Keepalive for Connection Resilience

**What:** The WebSocket handler in `ws/mod.rs` sends ping frames on a timer (every 30s). The rc-agent WebSocket client responds with pong. On the server side, a pong-watchdog closes the connection if no pong is received within 10s of a ping. rc-agent's reconnect loop (already present) then re-establishes within 5s.

**When to use:** Prevents the "disconnected" flash in the kiosk during game launch. Game launch causes a ~5s CPU spike; without pings the OS TCP stack may buffer and the WebSocket appears dead.

**Trade-offs:** Adds 30s-periodic overhead across 8 concurrent WS connections — negligible on the server.

### Pattern 4: Config Validation Fail-Fast at Startup

**What:** rc-agent validates all required config fields in `main.rs` before spawning any async tasks. Missing fields emit a clear error and exit with code 1. This prevents the current failure mode where rc-agent starts silently and crashes later during game launch.

**When to use:** In rc-agent's main.rs immediately after `Config::load()`.

**Trade-offs:** Makes misconfigured deploys fail loudly, which is always preferable to silent partial operation.

## Data Flow

### Supervision Signal Flow

```
rc-agent (pod)
    │
    ├─── UDP heartbeat every 2s ─────────────────► udp_heartbeat.rs (core)
    │                                                    │ updates last_seen map
    │                                                    ▼
    │                                              pod_monitor.rs (10s loop)
    │                                                    │ stale > 30s?
    │                                                    ▼
    │                                              EscalatingBackoff.ready()?
    │                                                    │ YES
    │                                                    ▼
    │◄─── POST /exec restart cmd ──────────────── pod-agent (:8090)
    │                                                    │
    │                                              tokio::spawn verify_restart
    │                                                    │ 5s/15s/30s/60s checks
    ├─── WebSocket connected? ────────────────────► state.agent_senders
    ├─── lock screen HTTP? ───────────────────────► pod-agent /exec powershell
    │                                                    │ result
    │                                                    ▼
    │                                              backoff.reset()  OR
    │                                              EmailAlerter.send_alert()
```

### Restart Ownership Decision Flow

```
pod_monitor detects stale heartbeat
    │
    ├── billing active? ──► SKIP (never restart during paid session)
    │
    ├── backoff.ready()? ──► NO → log "cooldown not elapsed", continue
    │
    └── YES → record_attempt → send restart cmd → spawn verify task
                                                        │
pod_healer detects rc-agent unhealthy (120s loop)       │
    │                                                   │
    ├── backoff.exhausted()? ──► YES → diagnostics only, no restart
    │                                  email alert if not rate-limited
    │
    └── NO + backoff.ready() ──► defer to pod_monitor (do NOT restart)
                                  log "deferring restart to pod_monitor"
```

### Deployment Reliability Flow

```
James (.27) builds binary
    │
    ├── cargo test --workspace ──► all green?
    │
    └── YES → copy to deploy-staging/
               start HTTP server (:9998)
               POST /exec to Pod 8 only:
                 taskkill rc-agent → delete old → download new →
                 size check → start → verify WS reconnects
                 │
                 SUCCESS? ──► deploy to remaining 7 pods in sequence
                 FAILURE? ──► stop, investigate
```

### Email Alert Decision Flow

```
Trigger: post-restart verification failure OR backoff.exhausted()
    │
    EmailAlerter.should_send(pod_id, now)
    │
    ├── enabled? ──► NO → skip silently
    ├── per-pod cooldown elapsed (30min)? ──► NO → log "rate-limited", skip
    ├── venue-wide cooldown elapsed (5min)? ──► NO → log "rate-limited", skip
    │
    └── ALL PASS → node send_email.js usingh@racingpoint.in <subject> <body>
                    15s timeout, logs warning on failure, never panics
                    record_sent(pod_id, now) on success
```

## Build Order (Phase Dependencies)

The requirements fall into four dependency layers. Each layer must be complete before the next can be integrated and tested.

```
Layer 1 — Shared primitives (no deps on other new work)
  rc-common/watchdog.rs    DONE
  rc-core/email_alerts.rs  DONE

Layer 2 — State wiring (depends on Layer 1)
  state.rs: add pod_backoffs + email_alerter to AppState
  config.rs: add WatchdogConfig with email fields + backoff step tuning
  → Gate: cargo test -p rc-core -- config (all pass)

Layer 3 — Monitor + healer integration (depends on Layer 2)
  pod_monitor.rs: use shared backoff, add post-restart verify task
  pod_healer.rs: read shared backoff, remove restart ownership
  ws/mod.rs: WebSocket ping/pong keepalive
  → Gate: cargo test -p rc-core -- pod_monitor (all pass)
           Manual: kiosk no longer flashes "disconnected" on game launch

Layer 4 — Agent hardening (independent of Layers 2-3, depends only on Layer 1)
  rc-agent config validation fail-fast
  pod-agent idempotent deploy command
  → Gate: deploy to Pod 8, verify binary swap works cleanly
           deploy to remaining 7 pods
```

## Integration Points

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| pod_monitor ↔ pod_healer | Shared `pod_backoffs` in AppState via `RwLock` | pod_monitor writes (record_attempt, reset); pod_healer reads only |
| pod_monitor / pod_healer ↔ pod-agent | HTTP POST /exec with JSON `{cmd: "..."}` | Field is `cmd` not `command` — existing pitfall |
| pod_monitor ↔ email_alerter | `state.email_alerter: Mutex<EmailAlerter>` | Lock for send_alert call; 15s timeout prevents blocking |
| rc-core ws ↔ rc-agent | WebSocket persistent connection + ping/pong | `agent_senders` map keyed by pod_id |
| rc-core ↔ rc-agent (liveness) | UDP heartbeat every 2s, 6s stale threshold | Parallel to WebSocket — heartbeat can be alive while WS is down |
| email_alerter ↔ send_email.js | tokio::process::Command shell-out | Node.js must be on Racing-Point-Server (.23); verify before deploy |

### External Dependencies

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| send_email.js (Node.js) | Shell-out via `tokio::process::Command` | Requires Node.js on .23, credentials at configured path |
| Gmail OAuth2 | Handled entirely by send_email.js | rc-core never touches OAuth2 tokens directly |
| pod-agent (:8090) | reqwest HTTP client from rc-core | Must be reachable; WoL fallback if not |

## Anti-Patterns

### Anti-Pattern 1: Restart Without Verification

**What people do:** Send restart command, check HTTP 200, declare recovery success.

**Why it's wrong:** `start /b rc-agent.exe` always returns exit 0 even if rc-agent crashes 1 second after launch (bad config, missing DLL, port conflict). The pod appears recovered but is dead.

**Do this instead:** Spawn a verification task. Confirm process alive via tasklist AND WebSocket reconnected AND lock screen responsive within 60s.

### Anti-Pattern 2: Both Monitor and Healer Restart Same Pod

**What people do:** Independent timers in pod_monitor (10s) and pod_healer (120s) both detect rc-agent down and both issue restart commands.

**Why it's wrong:** Causes double-kill of the restarting process. Activity log shows contradictory state. Hard to debug.

**Do this instead:** Share `EscalatingBackoff` in AppState. Only pod_monitor owns restart decisions. pod_healer defers: it reads the shared backoff and only escalates diagnostics/email when the backoff is exhausted.

### Anti-Pattern 3: Blocking the Monitor Loop for Verification

**What people do:** Await the post-restart health check inline in the 10s monitor loop.

**Why it's wrong:** Verification takes up to 60s. Blocking the loop means all 7 other pods go unchecked during that window. A pod that went offline at second 5 of a 60s wait gets a 65s response time.

**Do this instead:** `tokio::spawn` the verification task. The loop continues checking all 8 pods every 10s regardless.

### Anti-Pattern 4: Per-Pod Email Without Venue-Wide Aggregation

**What people do:** Emit one email per failing pod immediately.

**Why it's wrong:** A network switch reboot or power flicker takes all 8 pods offline simultaneously — 8 emails in 10 seconds.

**Do this instead:** Venue-wide cooldown (5 min) in EmailAlerter across all pods. Already implemented in `email_alerts.rs`.

### Anti-Pattern 5: Fixed Global Cooldown

**What people do:** Use a single `const HEAL_COOLDOWN_SECS: u64 = 600` across all pods.

**Why it's wrong:** A pod in a crash loop (bad binary, hardware fault) will restart every 600s indefinitely, preventing staff from noticing the problem needs manual attention.

**Do this instead:** EscalatingBackoff. After 4 attempts the cooldown reaches 30min and `exhausted()` returns true — at that point the email alert fires and staff intervene.

## Scaling Considerations

This system is fixed at 8 pods. Scaling is not a concern. The relevant reliability concern is the inverse: what happens when the system is under-loaded (all pods idle) vs overloaded (all 8 pods in session simultaneously triggering game launches).

| Scenario | Risk | Mitigation |
|----------|------|------------|
| All 8 pods game-launch simultaneously | CPU spike on server, WS drops | WebSocket ping/pong keepalive (Pattern 3) |
| Network switch reboot | All 8 pods go offline | Venue-wide email rate limiting, WoL fallback |
| rc-core restart during session | All WS connections drop | rc-agent reconnect loop re-establishes within 5s |
| Bad binary deployed to all 8 pods | All pods dead | Deploy-to-one-first protocol (Pod 8 gate) |

## Sources

- `crates/rc-common/src/watchdog.rs` — EscalatingBackoff implementation (HIGH)
- `crates/rc-core/src/email_alerts.rs` — EmailAlerter implementation (HIGH)
- `crates/rc-core/src/pod_monitor.rs` — existing PodRecoveryState, restart cmd, 10s loop (HIGH)
- `crates/rc-core/src/pod_healer.rs` — HealCooldown, check_rc_agent_health pattern (HIGH)
- `crates/rc-core/src/state.rs` — AppState shape, agent_senders map (HIGH)
- `.planning/archive/hud-safety/phases/05-watchdog-hardening/05-RESEARCH.md` — full watchdog architecture analysis, Session 0 pitfalls (HIGH)
- `.planning/PROJECT.md` — requirements scope, constraints, stack decisions (HIGH)

---
*Architecture research for: RaceControl Reliability & Connection Hardening*
*Researched: 2026-03-13*
