# Architecture Research

**Domain:** Rust/Axum fleet management — verification, observability, boot resilience layer (v25.0)
**Researched:** 2026-03-26
**Confidence:** HIGH (based on direct codebase analysis of all 4 crates)

## Standard Architecture

### System Overview

```
┌────────────────────────────────────────────────────────────────────────────┐
│                     racecontrol (server :8080)                              │
│                                                                              │
│  pod_monitor.rs   pod_healer.rs   fleet_health.rs   app_health_monitor.rs  │
│  flags.rs         recovery.rs     cloud_sync.rs      ota_pipeline.rs        │
│       │                │                │                    │              │
│       └────────────────┴────────────────┴────────────────────┘              │
│                          state.rs (AppState)                                 │
│         WatchdogState | RecoveryIntentStore | FeatureFlagRow                 │
└──────────────────────────────────┬─────────────────────────────────────────┘
                                   │ WebSocket (AgentMessage / CoreToAgentMessage)
┌──────────────────────────────────┴─────────────────────────────────────────┐
│                      rc-agent (each pod :8090)                               │
│                                                                              │
│  app_state.rs     event_loop.rs    ws_handler.rs    billing_guard.rs        │
│  self_monitor.rs  pre_flight.rs    process_guard.rs  startup_log.rs         │
│  safe_mode.rs     feature_flags.rs failure_monitor.rs game_process.rs       │
│                                                                              │
│  AppState (survives WS reconnections — flags, guard_whitelist, safe_mode)   │
│  ConnectionState (reset per WS connect — intervals, LaunchState, etc.)      │
└──────────────────────────────────┬─────────────────────────────────────────┘
                         ┌─────────┴─────────┐
                         │ UDP telemetry      │ sentinel files / startup logs
                         │                   │
┌────────────────────────┴───────────────────┴────────────────────────────────┐
│                 rc-sentry (each pod :8091) — NO TOKIO, std::net              │
│                                                                              │
│  main.rs (4-slot concurrency cap)    watchdog.rs (FSM health polling)       │
│  tier1_fixes.rs   debug_memory.rs    session1_spawn.rs                      │
│                                                                              │
│  WatchdogState: Healthy → Suspect(N) → Crashed                              │
│  Reads: rc-agent-startup.log, rc-agent-stderr.log after crash               │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                        rc-common (shared lib, no binary)                     │
│                                                                              │
│  protocol.rs (AgentMessage / CoreToAgentMessage enums)                       │
│  types.rs    recovery.rs (RecoveryAuthority, ProcessOwnership, Logger)       │
│  exec.rs     watchdog.rs (EscalatingBackoff)   ollama.rs   udp_protocol.rs  │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Key State |
|-----------|---------------|-----------|
| `racecontrol/state.rs` | Fleet AppState — server-side shared state hub | WatchdogState, RecoveryIntentStore, FleetHealthStore |
| `racecontrol/pod_monitor.rs` | Detects pod liveness failures, advances EscalatingBackoff | Owns backoff; does NOT heal |
| `racecontrol/pod_healer.rs` | Rule-based fixes + AI escalation; reads backoff but does NOT advance it | RecoveryAuthority::PodHealer |
| `racecontrol/fleet_health.rs` | Aggregates PodFleetStatus, ViolationStore | Serves /api/v1/fleet/health |
| `racecontrol/app_health_monitor.rs` | Polls admin/kiosk/web health endpoints every 30s; WhatsApp alert on failure | — |
| `rc-agent/app_state.rs` | Pod AppState — survives WS reconnections | flags, guard_whitelist, safe_mode, heartbeat_status |
| `rc-agent/event_loop.rs` | Per-connection state machine; ConnectionState reset on each WS connect | LaunchState, CrashRecoveryState, all interval timers |
| `rc-agent/ws_handler.rs` | Dispatches CoreToAgentMessage to handlers, returns HandleResult | Stateless dispatch |
| `rc-agent/self_monitor.rs` | Background tokio::spawn — CLOSE_WAIT flood + WS dead detection | Runs every 60s, owns ws_last_connected |
| `rc-agent/pre_flight.rs` | 3 concurrent checks (HID/ConspitLink/orphan) on BillingStarted | Returns Pass or MaintenanceRequired |
| `rc-agent/startup_log.rs` | Phased startup log → `rc-agent-startup.log` | AtomicBool LOG_INITIALIZED (truncate-first) |
| `rc-agent/process_guard.rs` | Periodic scan vs MachineWhitelist; sends ProcessViolation | guard_whitelist Arc<RwLock> in AppState |
| `rc-agent/feature_flags.rs` | In-memory FeatureFlags cache; updated via FlagSync WS message | Persisted to disk on every update |
| `rc-sentry/watchdog.rs` | FSM polls rc-agent :8090 every 5s; 3-poll hysteresis; reads crash logs | WatchdogState::Healthy/Suspect(N)/Crashed |
| `rc-sentry/tier1_fixes.rs` | Deterministic fixes: stale sockets, zombie kill, config repair | Applied before restart attempt |
| `rc-sentry/debug_memory.rs` | JSON crash pattern memory — instant replay of known fixes | debug-memory.json on disk |
| `rc-common/recovery.rs` | RecoveryAuthority enum, ProcessOwnership registry, RecoveryLogger, JSONL log | Shared by all 3 executables |
| `rc-common/exec.rs` | Shared sync/async exec primitive (feature-gated tokio boundary) | Used by rc-sentry + rc-agent |

---

## Recommended Project Structure for v25.0

New features slot into the existing 4-crate structure. No new crate is needed.

```
crates/
├── rc-common/src/
│   ├── verification.rs        NEW — VerificationChain trait + VerificationStep + Verdict
│   └── ... (existing unchanged)
│
├── rc-agent/src/
│   ├── boot_resilience.rs     NEW — generic periodic re-fetch scheduler
│   ├── observable_state.rs    NEW — StateTransitionKind enum + emit_transition()
│   ├── startup_log.rs         MODIFY — add VerificationStep hooks after write_phase() calls
│   ├── event_loop.rs          MODIFY — call emit_transition() before each sentinel write
│   ├── pre_flight.rs          MODIFY — emit observable event on MaintenanceRequired result
│   ├── self_monitor.rs        MODIFY — add lifecycle logs (start / first-decision / exit)
│   ├── process_guard.rs       MODIFY — emit observable event on empty allowlist fetch
│   └── feature_flags.rs       MODIFY — emit observable event on fallback to compiled-in defaults
│
├── racecontrol/src/
│   ├── verification_gate.rs   NEW — pre-ship domain-matched gate runner
│   ├── config.rs              MODIFY — emit observable event on load_or_default() fallback
│   ├── pod_monitor.rs         MODIFY — emit observable events on WatchdogState transitions
│   ├── pod_healer.rs          MODIFY — wrap curl-output parse in VerificationChain
│   └── fleet_health.rs        MODIFY — surface verification chain failures in health response
│
└── rc-sentry/src/
    └── watchdog.rs            MODIFY — append to RecoveryLogger on every FSM transition
```

---

## Architectural Patterns

### Pattern 1: VerificationChain — Wrap Existing Parse/Transform Paths

**What:** A typed chain that records each step's input, transform, output, and verdict. Not a new execution path — it wraps existing ones at the call site.

**When to use:** Around any path where a silent wrong value causes downstream failure: curl output parsing, config loading, spawn verification, billing guard decisions.

**Trade-offs:** One struct alloc per step. Worth it because the chain produces a structured record readable by rc-sentry's debug_memory and loggable before tracing is fully initialized.

**Example (wrapping existing curl parse in pod_healer):**

```rust
// rc-common/src/verification.rs
pub enum Verdict {
    Pass,
    Fail(String),
}

pub struct VerificationStep {
    pub name: &'static str,
    pub input: String,
    pub output: String,
    pub verdict: Verdict,
}

pub struct VerificationChain {
    steps: Vec<VerificationStep>,
}

impl VerificationChain {
    pub fn record(&mut self, name: &'static str, input: &str, output: &str, verdict: Verdict) {
        self.steps.push(VerificationStep {
            name,
            input: input.into(),
            output: output.into(),
            verdict,
        });
    }
    pub fn passed(&self) -> bool {
        self.steps.iter().all(|s| matches!(s.verdict, Verdict::Pass))
    }
    pub fn first_failure(&self) -> Option<&VerificationStep> {
        self.steps.iter().find(|s| matches!(s.verdict, Verdict::Fail(_)))
    }
}
```

The chain is populated inline at the call site and discarded after logging. No global state.

---

### Pattern 2: Observable State Transitions — Emit-Then-Act

**What:** Before writing any sentinel file or changing a critical shared flag, emit an observable event: `eprintln!` (pre-tracing-init safety) + `tracing::warn!` + optional `DashboardEvent` WS broadcast. The sentinel write happens after the emit.

**When to use:** MAINTENANCE_MODE writes, GRACEFUL_RELAUNCH creation, OTA_DEPLOYING, config fallback, empty allowlist fetch, feature flag default fallback.

**Trade-offs:** One extra tracing call per transition. No shared state required — emit is fire-and-forget.

**Integration point:** The existing `rc-agent/event_loop.rs` already writes sentinel files via `fs::write()`. The modification is one line before each write: `observable_state::emit_transition(kind, details)`. The sentinel protocol itself does not change — rc-sentry and pod_monitor continue reading files as before.

```rust
// rc-agent/src/observable_state.rs
pub enum StateTransitionKind {
    MaintenanceModeEntered,
    MaintenanceModeCleared,
    GracefulRelaunchSentinel,
    ConfigFallbackActivated,
    EmptyAllowlistFetched,
    FeatureFlagDefaultFallback,
    OtaDeployingStarted,
}

pub fn emit_transition(kind: StateTransitionKind, details: &str) {
    // eprintln! fires even before tracing subscriber is initialized
    eprintln!("[OBSERVABLE-STATE] {:?}: {}", kind, details);
    tracing::warn!(target: "observable-state", kind = ?kind, %details, "State transition");
}
```

---

### Pattern 3: Boot Resilience — Startup Fetch + Periodic Re-fetch

**What:** Any resource fetched once at startup (allowlist, config, feature flags) must also be re-fetched on a periodic `tokio::spawn` background loop. The existing `process_guard` allowlist re-fetch (every 300s, implemented in commit `821c3031`) is the reference pattern.

**When to use:** Any `OnceLock` or startup-time fetch that has no retry path. Resources where a transient server-down at boot time would leave the pod in permanent degraded state.

**Trade-offs:** Slightly wider stale window (up to interval_secs) vs permanent failure from a single boot miss. The tradeoff is explicit and correct: 5-minute stale allowlist is better than permanent empty allowlist.

**Integration:** The existing `self_monitor.rs` and the allowlist re-fetch in `process_guard.rs` both use the `tokio::spawn` + `interval` pattern. New `boot_resilience.rs` extracts the shared scaffold and adds the mandatory lifecycle logging (start / first-success / exit).

```rust
// rc-agent/src/boot_resilience.rs
pub fn spawn_periodic_refetch<F, Fut>(name: &'static str, interval_secs: u64, fetch_fn: F)
where
    F: Fn() -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    tokio::spawn(async move {
        tracing::info!(target: "boot-resilience", "Periodic re-fetch started: {}", name);
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        let mut first_complete = false;
        loop {
            interval.tick().await;
            fetch_fn().await;
            if !first_complete {
                tracing::info!(target: "boot-resilience", "First re-fetch complete: {}", name);
                first_complete = true;
            }
        }
    });
}
```

---

### Pattern 4: Startup Enforcement Audit — Bat File Invariant Scanner

**What:** A bash audit script that reads each `start-rcagent.bat` (via fleet exec or staging copy) and verifies that every known manual fix has a corresponding enforcement line. Not a Rust change — a standalone script in `audit/startup/`.

**When to use:** After any session that applies manual fixes to pods. Wired into pre-ship gate.

**Integration:** The v23.0 `audit/` directory already runs 60 phase scripts across 18 tiers via `audit.sh`. The startup enforcement audit is a new tier that produces pass/fail output consumed by the existing `report.sh` + delta tracker. No new tooling needed.

---

## Data Flow for Verification Chain

### How Verification Results Propagate

```
rc-agent (pod)
    │
    ├─ VerificationChain populated inline at parse/transform site
    │         │
    │         ├─ On failure: tracing::error! + chain details written to startup_log.rs
    │         │
    │         └─ AgentMessage::AiDebugResult (existing WS path) →
    │                  racecontrol ws_handler →
    │                  RecoveryEventStore (ring buffer, fleet alert)
    │
    └─ ObservableState emitted before sentinel file writes
              │
              ├─ tracing::warn! (captured in racecontrol-*.jsonl rolling log)
              │
              └─ DashboardEvent::PodActivity (existing WS broadcast path) →
                       kiosk :3300 (staff Control Room)
                       admin :3201 (Control Room page)

rc-sentry (pod)
    │
    └─ FSM transition → rc-common RecoveryLogger.append()
              │
              └─ JSONL written to C:\RacingPoint\recovery-log.jsonl
                       (already read by racecontrol/recovery.rs)
```

The critical constraint: verification results travel over the **existing AgentMessage WebSocket channel** and the **existing RecoveryLogger JSONL path**. No new transport. No new protocol message variants unless an existing variant cannot carry the data.

---

## Component Modification Map

### New Modules (additive, no existing code changed)

| Module | Crate | Responsibility | Size Estimate |
|--------|-------|---------------|---------------|
| `rc-common/verification.rs` | rc-common | VerificationChain, VerificationStep, Verdict types | ~100 LOC |
| `rc-agent/observable_state.rs` | rc-agent | StateTransitionKind enum + emit_transition() | ~80 LOC |
| `rc-agent/boot_resilience.rs` | rc-agent | Generic periodic re-fetch scheduler with lifecycle logging | ~60 LOC |
| `racecontrol/verification_gate.rs` | racecontrol | Pre-ship domain-matched gate runner | ~150 LOC |

### Modified Modules (additive changes only — no existing behavior removed)

| Module | Crate | Change | Risk |
|--------|-------|--------|------|
| `rc-agent/startup_log.rs` | rc-agent | Add VerificationStep hooks after write_phase() calls | LOW — tracing/logging only |
| `rc-agent/event_loop.rs` | rc-agent | Call emit_transition() before sentinel file writes | LOW — one line per sentinel |
| `rc-agent/pre_flight.rs` | rc-agent | Emit observable event on MaintenanceRequired result | LOW — after existing result construction |
| `rc-agent/self_monitor.rs` | rc-agent | Add lifecycle logs: task started / first decision / exit | LOW — tracing calls only |
| `rc-agent/process_guard.rs` | rc-agent | Emit observable event when fetched whitelist is empty | LOW — one conditional check |
| `rc-agent/feature_flags.rs` | rc-agent | Emit observable event on fallback to compiled-in defaults | LOW — one conditional check |
| `rc-sentry/watchdog.rs` | rc-sentry | Append to RecoveryLogger on every FSM state change (not just Crashed) | LOW — uses existing RecoveryLogger API |
| `racecontrol/pod_monitor.rs` | racecontrol | Emit observable event on WatchdogState transitions | LOW — one call per transition |
| `racecontrol/pod_healer.rs` | racecontrol | Wrap curl-output-to-u32 parse in VerificationChain | LOW-MEDIUM — wraps existing parse, not replacing it |
| `racecontrol/config.rs` | racecontrol | Emit observable event when load_or_default() falls back | LOW — one check at parse time |
| `racecontrol/fleet_health.rs` | racecontrol | Optionally surface recent verification failures in health response | LOW — additive field |

---

## Build Order

Dependencies must resolve in this order:

**Wave 1 — rc-common (no upstream Rust dependencies)**
1. `rc-common/verification.rs` — VerificationChain, VerificationStep, Verdict
   - Blocks: all crates that instrument parse paths

**Wave 2 — rc-agent + rc-sentry (depend on rc-common)**
2. `rc-agent/observable_state.rs` — StateTransitionKind, emit_transition()
   - Blocks: event_loop, pre_flight, process_guard, feature_flags modifications
3. `rc-agent/boot_resilience.rs` — generic periodic re-fetch scheduler
   - Blocks: wiring into existing allowlist/flags/config re-fetch callsites
4. Modify rc-agent modules (event_loop, pre_flight, process_guard, feature_flags, self_monitor, startup_log)
   - All consume items from steps 2-3
5. Modify `rc-sentry/watchdog.rs`
   - Consumes rc-common RecoveryLogger (already exists) — can run in parallel with step 4

**Wave 3 — racecontrol (depends on rc-common, observes rc-agent over WS)**
6. `racecontrol/verification_gate.rs` — pre-ship gate runner
   - Consumes VerificationChain from Wave 1
7. Modify racecontrol modules (pod_healer, pod_monitor, config, fleet_health)
   - All consume VerificationChain from Wave 1

**Wave 4 — Operational tooling (zero Rust compile dependency)**
8. `audit/startup/` enforcement audit scripts
9. Cause Elimination Process template integration into gate-check.sh
   - Can be developed in parallel with Waves 2-3

**Rationale:** rc-common types must stabilize before rc-agent/rc-sentry consume them. Server-side changes (Wave 3) are lower risk — they instrument the server's own state machines rather than pod-side session logic. Bat file / tooling changes (Wave 4) have no compile dependency and can parallelize freely.

---

## Integration with Existing 4-Tier Recovery

```
Tier 1 — self_monitor.rs (rc-agent)
    EXISTING: detects CLOSE_WAIT flood, WS dead time, triggers relaunch
    ADDS: emit_transition(GracefulRelaunchSentinel) before relaunch write
    ADDS: VerificationChain wraps CLOSE_WAIT netstat parse

Tier 2 — rc-sentry watchdog.rs
    EXISTING: FSM Healthy → Suspect → Crashed; reads startup_log; tier1_fixes
    ADDS: RecoveryLogger.append() on Suspect AND Crashed (was Crashed-only)
    ADDS: VerificationChain wraps health poll HTTP status parse

Tier 3 — pod_monitor.rs + pod_healer.rs (racecontrol)
    EXISTING: WatchdogState machine, EscalatingBackoff, HealAction decisions
    ADDS: emit_transition on WatchdogState changes
    ADDS: VerificationChain wraps curl-output-to-u32 parse in healer
    EXISTING: RecoveryIntentStore (v17.1) deconflicts rc-sentry and pod_healer

Tier 4 — James AI watchdog (rc-watchdog.exe)
    NO CHANGE — operates at OS service level, not Rust session code
```

### Sentinel File Protocol — No Changes to Existing Behavior

The existing sentinels (`MAINTENANCE_MODE`, `GRACEFUL_RELAUNCH`, `OTA_DEPLOYING`) are read by rc-sentry, pod_monitor, and pod_healer exactly as before. The v25.0 change is additive: `emit_transition()` fires before the `fs::write()`. Existing consumers see no difference.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Post-Hoc Verification as the "Verification Framework"

**What people do:** Add health endpoint checks and build_id comparisons as the verification layer.

**Why it's wrong:** All 8 proxy verification incidents (per PROJECT.md audit) used post-hoc health checks that passed while the actual parse path failed silently. Health OK + build_id match only proves the binary is running — not that the specific transform is correct.

**Do this instead:** Inline VerificationChain at the actual parse/transform site. The chain IS the execution — same call site, same data, same instant as the work being verified.

---

### Anti-Pattern 2: Silent Fallback Without Observable Emit

**What people do:** `load_or_default()` returns empty config, code continues with degraded behavior, no WARN log.

**Why it's wrong:** The SSH banner config corruption incident ran process_guard with 0 allowed entries for 2+ hours. The fallback itself was correct behavior; the silence was the bug. Silent fallback is structurally indistinguishable from correct operation until downstream failures appear.

**Do this instead:** Every `or_default()`, `unwrap_or_default()`, or fallback on a critical config path must call `emit_transition(ConfigFallbackActivated, ...)` before continuing. The emit makes degraded state visible at the moment it occurs.

---

### Anti-Pattern 3: Boot Retry Without Periodic Re-fetch

**What people do:** Add 3-attempt retry with exponential backoff to startup fetch.

**Why it's wrong:** Boot retry succeeds on attempt 3 but then the resource is never re-fetched. The pod runs on stale/empty data until the next reboot. The allowlist incident was exactly this pattern: boot succeeded with empty allowlist, and the empty state persisted until manual restart.

**Do this instead:** Startup fetch (with retry) AND periodic re-fetch loop. The loop heals stale state automatically. The two mechanisms are complementary, not alternatives.

---

### Anti-Pattern 4: New Protocol Message Variant for Observable Events

**What people do:** Add `AgentMessage::StateTransitionEvent` to carry observable state data over WebSocket.

**Why it's wrong:** Protocol changes require simultaneous rc-agent + racecontrol binary upgrade. During the deploy window, mismatched versions drop unknown variants silently. The fleet has 8 pods that may be on different binaries during a rolling deploy.

**Do this instead:** Route observable state through existing `AgentMessage::AiDebugResult` (structured suggestions already carry free-form data) or `DashboardEvent` (already broadcast to UI). Introduce new variants only when no existing variant fits AND both sides will be upgraded atomically.

---

## Integration Points

### Internal Boundaries

| Boundary | Communication Method | Notes |
|----------|---------------------|-------|
| rc-agent → racecontrol | WebSocket (AgentMessage enum in rc-common/protocol.rs) | AiDebugResult carries VerificationChain failures |
| racecontrol → rc-agent | WebSocket (CoreToAgentMessage in rc-common/protocol.rs) | FlagSync, KillSwitch, ConfigPush unchanged |
| rc-sentry → racecontrol | HTTP POST /api/v1/recovery/events | RecoveryLogger writes JSONL locally; server reads via API |
| rc-common → all crates | Cargo lib dependency | VerificationChain, RecoveryLogger live here |
| observable_state → kiosk/admin | DashboardEvent WS broadcast from racecontrol | Staff sees transitions in Control Room page (:3201, :3300) |
| boot_resilience → AppState | Arc<RwLock<T>> (same pattern as existing guard_whitelist) | Re-fetched values update shared state in-place |

### Operational Integration Points

| System | How v25.0 Touches It | Notes |
|--------|---------------------|-------|
| `start-rcagent.bat` (8 pods) | Startup enforcement audit scans + reports missing lines | Bat changes deployed per standing rule: bat sync with binary deploy |
| `audit/` runner (v23.0) | New `audit/startup/` tier adds startup enforcement checks | Feeds existing report.sh + delta tracker |
| `gate-check.sh` (v22.0) | Pre-ship gate adds domain-match check | Visual change = visual verification required; wired into Suite 0 |
| `comms-link` relay | No changes | Verification results travel over existing rc-agent WS channel |

---

## Scalability Considerations

This system is venue-scoped: 8 pods, 1 server, fixed topology. Concerns are operational, not load-based.

| Concern | Current | With v25.0 |
|---------|---------|-----------|
| Observable event volume | 0 (all silent) | Low — fires only on state transitions, not on every tick |
| VerificationChain memory | N/A | Negligible — stack-allocated per call site, freed after log |
| Periodic re-fetch connections | 1 (allowlist, every 300s) | +2-3 more at 300s each = ~3 extra HTTP GETs per 5 min per pod |
| Recovery log growth | Unbounded JSONL | Rotate at 512KB — matches existing `rc-bot-events.log` pattern |

---

## Sources

- Direct codebase analysis: `crates/rc-agent/src/` — app_state.rs, event_loop.rs, self_monitor.rs, pre_flight.rs, startup_log.rs, process_guard.rs, feature_flags.rs, failure_monitor.rs
- Direct codebase analysis: `crates/racecontrol/src/` — state.rs, pod_monitor.rs, pod_healer.rs, flags.rs
- Direct codebase analysis: `crates/rc-sentry/src/` — main.rs, watchdog.rs
- Direct codebase analysis: `crates/rc-common/src/` — protocol.rs, recovery.rs
- `.planning/PROJECT.md` — v25.0 goal, target features, constraints, audit evidence
- `CLAUDE.md` standing rules — known failure modes, incident history, sentinel file protocol

---

*Architecture research for: v25.0 Debug-First-Time-Right*
*Researched: 2026-03-26*
