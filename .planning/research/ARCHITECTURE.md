# Architecture Research

**Domain:** Feature flag registry, OTA pipeline, and config push for RaceControl fleet
**Researched:** 2026-03-23
**Confidence:** HIGH (all integration points verified against live source code)

---

## System Overview

The v22.0 additions layer onto the existing WebSocket gateway without introducing new transports. The
feature registry lives in racecontrol SQLite, config push uses the existing `CoreToAgentMessage` channel,
and the OTA pipeline wraps the existing `deploy.rs` swap-script mechanism with canary gating and
automated rollback. No new ports or protocols are required.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                          Admin Dashboard (Next.js :3200)                      │
│   Feature toggle UI  │  OTA release trigger  │  Standing-rule gate display   │
└────────────┬─────────────────────┬────────────────────────┬───────────────────┘
             │ REST                │ REST                   │ REST
             ▼                    ▼                        ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│                    racecontrol server (Rust/Axum :8080)                       │
│                                                                               │
│  ┌─────────────────────┐   ┌──────────────────────┐   ┌───────────────────┐  │
│  │  feature_registry   │   │   ota_pipeline        │   │  standing_rules   │  │
│  │  (SQLite-backed)    │   │   (wraps deploy.rs)   │   │  (enforcement     │  │
│  │  per-pod overrides  │   │   manifest + canary   │   │  gate module)     │  │
│  └──────────┬──────────┘   └──────────┬───────────┘   └────────┬──────────┘  │
│             │                         │                         │             │
│  ┌──────────▼─────────────────────────▼─────────────────────────▼──────────┐  │
│  │                         AppState (Arc<AppState>)                         │  │
│  │  agent_senders RwLock<HashMap<pod_id, mpsc::Sender<CoreToAgentMessage>>> │  │
│  │  feature_flags RwLock<HashMap<(scope, key), FlagValue>>    (NEW)         │  │
│  │  pending_config_pushes RwLock<HashMap<pod_id, ConfigBundle>> (NEW)       │  │
│  │  ota_release_state RwLock<Option<OtaReleaseState>>          (NEW)        │  │
│  └──────────────────────────────────────┬───────────────────────────────────┘  │
│                                         │  existing mpsc channel                │
│                           CoreToAgentMessage enum                               │
└─────────────────────────────────────────┬────────────────────────────────────┘
                                          │  WebSocket (existing :8080/ws/agent)
                    ┌─────────────────────┴─────────────────────┐
                    ▼                                           ▼
         ┌──────────────────────┐                   ┌──────────────────────┐
         │  rc-agent (pod 1-7)  │                   │  rc-agent (pod 8     │
         │  ConfigApplied ack   │                   │  canary — first      │
         │  feature_flags map   │                   │  to receive OTA)     │
         │  hot-reload handler  │                   └──────────────────────┘
         └──────────────────────┘

         ┌──────────────────────────────────────────────┐
         │  rc-sentry-ai (James .27 :8096)              │
         │  Receives OTA binary push via HTTP (existing) │
         │  Feature flags via new REST endpoint          │
         └──────────────────────────────────────────────┘
```

---

## Component Responsibilities

| Component | Responsibility | New vs Modified |
|-----------|---------------|----------------|
| `feature_registry.rs` (racecontrol) | SQLite-backed flag store, per-pod/global scope, REST CRUD API | NEW |
| `ota_pipeline.rs` (racecontrol) | Release manifest, canary gate, staged rollout, health check, auto-rollback | NEW (wraps deploy.rs) |
| `standing_rules_gate.rs` (racecontrol) | Codify 41+ CLAUDE.md rules as executable checks, block pipeline on failure | NEW |
| `config_push.rs` (racecontrol) | Queue config bundles per pod, push on WS connect, retry on reconnect | NEW |
| `AppState` (racecontrol/state.rs) | Gains 3 new RwLock fields for flags, pending pushes, OTA state | MODIFIED |
| `CoreToAgentMessage` (rc-common/protocol.rs) | Add ConfigPush, FeatureFlagsUpdate, OtaDownload variants | MODIFIED |
| `AgentMessage` (rc-common/protocol.rs) | Add ConfigAck, FeatureFlagsAck, OtaDownloadComplete variants | MODIFIED |
| `rc-agent/event_loop.rs` | Handle new server messages, apply config hot-reload, download+self-swap OTA | MODIFIED |
| `rc-agent/config.rs` | Separate static config (TOML) from runtime config (pushed overlay) | MODIFIED |
| Admin dashboard (Next.js) | Feature toggle UI, OTA release trigger page, standing-rule status display | MODIFIED |

---

## New WebSocket Message Variants

These are the only protocol changes needed. All existing messages are untouched (additive-only).

### `CoreToAgentMessage` additions (server → pod)

```rust
/// Push a runtime config overlay to a pod. Agent merges over static TOML.
/// On reconnect, server replays the latest bundle from pending_config_pushes.
ConfigPush {
    push_id: String,        // UUID for dedup + ack matching
    config_json: String,    // serialized RuntimeConfigOverlay
    force_reload: bool,     // true = reload modules that support hot-reload
},

/// Push current feature flag state to a pod.
/// Sent on connect (full snapshot) and on any flag change (delta).
FeatureFlagsUpdate {
    flags: HashMap<String, serde_json::Value>,  // key -> value
    is_snapshot: bool,      // true = replace all, false = merge delta
},

/// Instruct pod to download new binary and self-swap.
/// Extends existing deploy.rs pattern — same HTTP staging server.
OtaDownload {
    release_id: String,
    binary_url: String,     // http://192.168.31.27:9998/rc-agent.exe
    expected_sha256: String,
    manifest_url: String,   // for frontend bundles
    rollback_on_failure: bool,
},
```

### `AgentMessage` additions (pod → server)

```rust
/// Acknowledgment that pod applied a config push.
ConfigAck {
    pod_id: String,
    push_id: String,
    success: bool,
    modules_reloaded: Vec<String>,  // which subsystems hot-reloaded
    error: Option<String>,
},

/// Acknowledgment that pod applied feature flag update.
FeatureFlagsAck {
    pod_id: String,
    flags_hash: String,     // hash of applied flags for drift detection
},

/// OTA binary download completed — pod is running new binary.
OtaComplete {
    pod_id: String,
    release_id: String,
    new_version: String,
    new_build_id: String,
},

/// OTA failed — pod rolled back to previous binary.
OtaRolledBack {
    pod_id: String,
    release_id: String,
    reason: String,
    rolled_back_to: String, // previous build_id
},
```

---

## Data Flow Diagrams

### Config Push Flow

```
Admin edits config value in dashboard
    │
    ▼ POST /api/v1/config/push  {pod_id: "all" | "pod_3", key, value}
racecontrol → config_push.rs
    │
    ├── Write to SQLite config_overrides table (persistent)
    │
    ├── Update AppState::pending_config_pushes (in-memory)
    │
    ├── For each connected pod in scope:
    │       agent_senders[pod_id].send(CoreToAgentMessage::ConfigPush{...})
    │
    ▼ WebSocket frame arrives at rc-agent
rc-agent event_loop.rs
    │
    ├── Deserialize RuntimeConfigOverlay
    ├── Merge over static TOML config (overlay wins, never replaces)
    ├── Hot-reload supported modules: process_guard, preflight, kiosk
    ├── Non-hot-reload modules: restart flagged, applied at next rc-agent start
    │
    ▼ AgentMessage::ConfigAck sent back
racecontrol ws/mod.rs
    │
    └── Update pod config_push_state → broadcast DashboardEvent::ConfigPushAck
```

### Feature Flag Push Flow

```
Admin toggles flag in dashboard (e.g. telemetry=false for pod_3)
    │
    ▼ POST /api/v1/features/{flag_key}  {scope: "pod", pod_id: "pod_3", value: false}
racecontrol → feature_registry.rs
    │
    ├── Upsert SQLite features table (flag_key, scope, pod_id, value, updated_at)
    │
    ├── Recompute effective flag map for affected pods
    │   (pod-level override wins over global default)
    │
    ├── Send delta to connected pod:
    │       CoreToAgentMessage::FeatureFlagsUpdate{flags: {"telemetry": false}, is_snapshot: false}
    │
    ▼ rc-agent applies delta
    │
    ├── Updates in-memory FeatureFlagMap (Arc<RwLock<HashMap<String, Value>>>)
    ├── Modules poll flag map at natural check points — no hot-reload plumbing needed
    │
    ▼ AgentMessage::FeatureFlagsAck sent back
    │
    └── Server records ack in per-pod flag_ack table for drift detection
```

### OTA Release Flow

```
CI build produces: rc-agent.exe + racecontrol.exe + frontend bundles
    │
    ├── Compute SHA256 for each artifact
    ├── Write release manifest JSON: {release_id, version, artifacts, sha256s}
    ├── Stage on deploy-staging HTTP server (:9998)
    │
    ▼ POST /api/v1/ota/release  {manifest_url, notes}    (admin trigger)
racecontrol → ota_pipeline.rs
    │
    ├── [GATE] standing_rules_gate.rs — run all 41+ rule checks
    │   Block if any check fails → return 422 with failing rules
    │
    ├── [GATE] verify binary size >= 5MB (reuse deploy.rs validate_binary_size)
    │
    ├── [CANARY] Send OtaDownload to Pod 8 only
    │   Wait up to 120s for OtaComplete ack
    │   Run health verification: WS connected + /health build_id matches
    │
    │   ┌─ health OK ──► [STAGED ROLLOUT]
    │   │
    │   └─ health fail → send OtaRolledBack trigger → wait rollback ack → abort release
    │
    ├── [STAGED ROLLOUT] Send OtaDownload to remaining pods in batches of 2
    │   Each batch: wait OtaComplete acks → verify health → proceed or rollback
    │   Session-aware: if pod has active billing, queue OtaDownload in pending_deploys
    │   (existing AppState::pending_deploys pattern — already handles this)
    │
    ├── Persist release record to SQLite releases table on completion
    │
    └── Broadcast DashboardEvent::OtaReleaseComplete → admin sees live status
```

---

## Config Storage Strategy: SQLite + TOML Hybrid

The existing architecture uses static TOML at startup with no persistence layer for overrides. v22.0
adds a runtime overlay tier in SQLite without removing TOML.

```
Priority (highest wins):
  3. Per-pod runtime override  ← config_overrides table, pod_id="pod_3"
  2. Global runtime override   ← config_overrides table, pod_id="*"
  1. Static TOML               ← C:\RacingPoint\rc-agent.toml (never hot-reloaded)
```

**Rationale:** TOML remains the install-time baseline. SQLite overrides are additive. If the database
is wiped, pods fall back to TOML — no brick risk. The distinction maps cleanly to operational roles:
TOML is for deploy-time config (server IP, pod number, certs), SQLite overlay is for operational config
(flags, thresholds, timeouts) that staff toggle from the dashboard.

### New SQLite Tables

```sql
-- Feature flag registry
CREATE TABLE IF NOT EXISTS feature_flags (
    flag_key     TEXT NOT NULL,
    scope        TEXT NOT NULL CHECK(scope IN ('global', 'pod', 'service')),
    target_id    TEXT NOT NULL DEFAULT '*',  -- pod_id or service_name, '*' = global
    value        TEXT NOT NULL,              -- JSON-encoded value
    updated_at   TEXT NOT NULL,
    updated_by   TEXT NOT NULL DEFAULT 'admin',
    PRIMARY KEY (flag_key, scope, target_id)
);

-- Config push log (audit + replay on reconnect)
CREATE TABLE IF NOT EXISTS config_push_log (
    push_id      TEXT PRIMARY KEY,
    pod_id       TEXT NOT NULL,
    config_json  TEXT NOT NULL,
    pushed_at    TEXT NOT NULL,
    acked_at     TEXT,
    ack_success  INTEGER,
    error        TEXT
);

-- OTA release registry
CREATE TABLE IF NOT EXISTS ota_releases (
    release_id   TEXT PRIMARY KEY,
    version      TEXT NOT NULL,
    manifest_url TEXT NOT NULL,
    notes        TEXT,
    state        TEXT NOT NULL CHECK(state IN ('pending','canary','rolling','complete','rolled_back','failed')),
    created_at   TEXT NOT NULL,
    completed_at TEXT
);

-- Per-pod OTA deployment status
CREATE TABLE IF NOT EXISTS ota_pod_status (
    release_id   TEXT NOT NULL,
    pod_id       TEXT NOT NULL,
    state        TEXT NOT NULL CHECK(state IN ('pending','downloading','complete','rolled_back','failed')),
    build_id     TEXT,
    updated_at   TEXT NOT NULL,
    PRIMARY KEY (release_id, pod_id)
);
```

---

## Cargo Feature Gates (compile-time)

The existing rc-agent has one feature flag: `keyboard-hook = []`. v22.0 adds major-module gates.

```toml
# crates/rc-agent/Cargo.toml [features]
default = ["telemetry", "process-guard", "pre-flight"]

telemetry    = []   # UDP telemetry readers (AC, F1 25, iRacing, etc.)
process-guard = []  # Process whitelist enforcement daemon
pre-flight   = []   # Pre-session hardware checks
ai-debugger  = []   # Local Ollama crash analysis
keyboard-hook = []  # (existing) Low-level keyboard hook
```

```toml
# crates/rc-sentry-ai/Cargo.toml [features]
default = ["camera-ai"]

camera-ai    = []   # YOLOv8 face detection / people tracking
```

**Gating pattern in source:**
```rust
#[cfg(feature = "telemetry")]
mod udp_heartbeat;
#[cfg(feature = "telemetry")]
mod sims;
```

These gates enable building a stripped binary for test/canary scenarios and will serve as the
compile-time anchor for the runtime feature flag system — a runtime flag can only affect a module
that was compiled in.

---

## Hot-Reload Scope

Not all config changes can be applied without a restart. The distinction matters for deployment safety.

| Config Area | Hot-Reload | Mechanism | Notes |
|-------------|-----------|-----------|-------|
| Feature flags (all) | YES | In-memory map polled at subsystem check points | No restart needed |
| process_guard.enabled / scan_interval | YES | ProcessGuard reads config on each scan tick | Module loops naturally |
| preflight.enabled | YES | PreFlight reads config before each BillingStarted | Event-driven |
| kiosk.enabled | YES | Kiosk checks on each lock screen render | |
| pod.server_url (WebSocket target) | YES | Existing SwitchController message handles this | v10.0 feature |
| ai_debugger.ollama_url / enabled | YES | AiDebugger reads config per query | |
| pod.pod_id / pod_number | NO | Baked into startup registration | Requires restart |
| telemetry_ports | NO | UDP sockets bound at startup | Requires restart |
| games.* exe paths | NO | Used at game launch time; safe to push but only active on next launch | |
| wheelbase.* | NO | HID device opened at startup | Requires restart |

---

## Standing Rules Gate: Codification Approach

The 41+ CLAUDE.md rules are categorized into 5 enforcement tiers for the gate module.

```
Tier A — BLOCK release (hard fail, pipeline stops):
  - Binary size >= 5MB (deploy.rs already has this)
  - SHA256 match between manifest and downloaded artifact
  - No active billing sessions on canary pod at OTA start
  - Rollback binary (rc-agent-prev.exe) present on pod before swap
  - Config validation at startup passes on canary pod

Tier B — BLOCK release (process rules):
  - Auto-push: git push completed before release tagged
  - Bono synced: INBOX.md entry + WS send both completed
  - Cascade update: all linked references updated when protocol changes
  - No .unwrap() in new Rust code (cargo clippy --deny warnings)

Tier C — WARN only (degrade gracefully, don't block):
  - Health endpoint returns correct build_id after deploy
  - LOGBOOK.md entry appended after deploy
  - Standing rules CLAUDE.md updated if new rules added

Tier D — AUDIT only (logged, not blocking):
  - Deploy sequence used correct 6-step server deploy order
  - Taskkill not combined with download in same exec chain

Tier E — RUNTIME enforcement (per-operation, not per-release):
  - Session billing preserved across OTA (queue if billing active)
  - Recovery systems cannot fight each other (cascade_guard checks)
  - No .unwrap() — enforced by clippy in CI, not by gate module
```

Gate implementation: `standing_rules_gate.rs` exports a single async fn `run_gate(release: &OtaRelease, state: &AppState) -> Result<(), Vec<RuleViolation>>`. The pipeline calls it before canary, before each batch, and before marking complete.

---

## Recommended Project File Structure

```
crates/racecontrol/src/
├── feature_registry.rs   # NEW — SQLite-backed flag store, REST CRUD, push logic
├── ota_pipeline.rs       # NEW — manifest ingestion, canary, staged rollout, rollback
├── standing_rules_gate.rs # NEW — codified 41+ rule checks as executable assertions
├── config_push.rs        # NEW — overlay queue, reconnect replay, ack tracking
├── deploy.rs             # MODIFIED — OtaDownload now delegates here for swap script
├── state.rs              # MODIFIED — 3 new RwLock fields
├── api/
│   └── routes.rs         # MODIFIED — new /features, /ota, /config/push routes

crates/rc-common/src/
├── protocol.rs           # MODIFIED — 7 new message variants (additive)
├── types.rs              # MODIFIED — OtaReleaseState, FeatureFlagMap types

crates/rc-agent/src/
├── config.rs             # MODIFIED — RuntimeConfigOverlay + merge logic
├── event_loop.rs         # MODIFIED — handle ConfigPush, FeatureFlagsUpdate, OtaDownload
├── feature_flags.rs      # NEW — in-memory FeatureFlagMap with Arc<RwLock>

crates/rc-agent/Cargo.toml  # MODIFIED — telemetry/process-guard/pre-flight feature gates
crates/rc-sentry-ai/Cargo.toml # MODIFIED — camera-ai feature gate
```

---

## Integration Points: What Changes vs What Stays the Same

### Stays the Same (no changes)

| Item | Why Unchanged |
|------|--------------|
| WebSocket transport (:8080/ws/agent) | New messages ride existing connection |
| `AgentMessage`/`CoreToAgentMessage` serde format | Additive variants with `#[serde(tag = "type")]` — unknown variants ignored by older agents |
| deploy.rs swap-script mechanism | OTA pipeline reuses it; no changes to `SWAP_SCRIPT_CONTENT` |
| `AppState::agent_senders` map | Config push and feature flag push both use existing `agent_senders[pod_id].send()` |
| `AppState::pending_deploys` | OTA pipeline reuses session-aware queuing that already exists |
| TOML config files | Runtime overlay is additive; TOML remains the baseline |
| Pod 8 canary pattern | OTA pipeline formalizes what is already an informal convention |
| Static CRT constraint | All new Rust code compiles under existing `.cargo/config.toml` |

### Changes (additive only, no breaking changes)

| Item | Change |
|------|--------|
| `rc-common/protocol.rs` | +7 message variants on existing enums |
| `rc-common/types.rs` | +4 new types (OtaReleaseState, FeatureFlagMap, RuntimeConfigOverlay, RuleViolation) |
| `AppState` | +3 RwLock fields appended to struct |
| `rc-agent/config.rs` | +RuntimeConfigOverlay merged at runtime; static AgentConfig untouched |
| SQLite schema | +4 new tables; all existing tables untouched |
| Admin dashboard | +2 new pages; existing pages untouched |

---

## Offline Pod Handling

Config push and feature flag updates must handle the 8-pod fleet where pods can be offline (maintenance,
power-off, network drop). The existing `AppState::pending_deploys` pattern is the proven model.

```
Pod is offline when push is triggered:
  └── pending_config_pushes[pod_id] = latest ConfigBundle   (overwrites stale)
  └── pending_feature_flags[pod_id] = latest FlagSnapshot   (overwrites stale)

Pod reconnects (ws/mod.rs handles AgentMessage::Register):
  └── ws handler checks pending_config_pushes[pod_id]
  └── if present → send CoreToAgentMessage::ConfigPush immediately after Registered ack
  └── same for FeatureFlagsUpdate (send full snapshot, is_snapshot=true)
  └── clear pending entries after ack received
```

This is the same pattern as `pending_deploys`. The reconnect handler in `ws/mod.rs` already handles
`Register` → `Registered` → immediate command sequence. The new push logic slots into the same spot.

---

## Build Order

Dependencies between components determine build order. Changes to `rc-common` block agent and server.
Admin dashboard is fully independent (uses REST, not WS).

```
Phase 1: Protocol Foundation (rc-common)
  - Add 7 new protocol message variants
  - Add 4 new types
  BLOCKS: everything else that touches protocol
  RISK: any protocol change requires rebuilding rc-agent + racecontrol

Phase 2: Server-Side Registry (racecontrol)
  - SQLite schema (4 tables)
  - feature_registry.rs
  - config_push.rs
  - state.rs AppState additions
  - REST endpoints (/features, /config/push)
  DEPENDS ON: Phase 1
  UNBLOCKS: admin dashboard can now be built and tested

Phase 3: Agent-Side Consumer (rc-agent)
  - RuntimeConfigOverlay in config.rs
  - feature_flags.rs in-memory map
  - event_loop.rs message handlers
  - Cargo.toml feature gates
  DEPENDS ON: Phase 1
  PARALLEL WITH: Phase 2 (both depend on Phase 1, not on each other)

Phase 4: OTA Pipeline (racecontrol)
  - ota_pipeline.rs
  - standing_rules_gate.rs
  - deploy.rs integration
  - ota_releases / ota_pod_status tables
  DEPENDS ON: Phase 2 (needs AppState OTA state fields)
  DEPENDS ON: Phase 3 (OtaDownload handler must exist in rc-agent before pipeline sends it)
  NOTE: Phase 3 and Phase 4 are sequential here — canary requires agent to handle OtaDownload

Phase 5: Admin Dashboard UI
  - Feature toggle page
  - OTA release trigger page
  - Standing rules status display
  DEPENDS ON: Phase 2 REST endpoints (can be built in parallel once API contract is set)
  DEPENDS ON: Phase 4 for OTA trigger UI (can mock with stub endpoints during development)

Phase 6: Standing Rules Codification (standing_rules_gate.rs)
  - Codify Tier A/B rules as executable checks
  - Wire gate into ota_pipeline.rs pre-canary and pre-batch checks
  DEPENDS ON: Phase 4 (pipeline must exist to wire gate into)
  NOTE: Tier C/D/E rules do not block pipeline — can be added incrementally after ship
```

**Parallel opportunity:** Phase 2 (server registry) and Phase 3 (agent consumer) can both proceed
after Phase 1 lands. A two-developer team can split here. Single developer should do Phase 2 first
to get testable REST endpoints before touching the agent.

---

## Anti-Patterns

### Anti-Pattern 1: New Port for Config Push

**What people do:** Add a separate HTTP endpoint on rc-agent (:8091) for config push, parallel to WebSocket.

**Why it's wrong:** Firewall rules are tight on pods. Adding a new port requires updating HKCU firewall rules
on all 8 pods across all 8 machines. More failure surface. Standing rule DEPLOY-01 (config validation at
startup) would need to cover two transports.

**Do this instead:** Ride the existing WebSocket agent connection. `CoreToAgentMessage::ConfigPush` reaches
the agent on the same connection that already carries Exec, LaunchGame, etc. No new ports, no new firewall
rules.

### Anti-Pattern 2: Hot-Reload Everything

**What people do:** Implement hot-reload for all config changes, including pod_id, UDP socket bindings,
and HID device handles.

**Why it's wrong:** UDP sockets and HID handles are bound at startup and are not safely reacquirable
without a restart. Attempting mid-session hot-reload of these causes CLOSE_WAIT leaks (the exact leak
already diagnosed and fixed in v8.0) and HID descriptor mismatches.

**Do this instead:** Only hot-reload subsystems that poll config at natural check points (process_guard
scan tick, preflight BillingStarted event, kiosk lock screen render). Flag everything else as
`requires_restart: true` in the ConfigAck response. The agent applies the value but does not act
on it until the next startup.

### Anti-Pattern 3: Feature Flags Replace Cargo Features

**What people do:** Use runtime feature flags as a substitute for compile-time Cargo feature gates,
keeping all code compiled in and just toggling it at runtime.

**Why it's wrong:** Binary size on pods matters (static CRT + all features = large exe). Modules like
`ai-debugger` pull in reqwest + serde_json + tokio for Ollama calls. Compiling them into a stripped
"kiosk-only" build when they are never used is wasteful. More importantly, compile-time exclusion
prevents entire classes of runtime failure in those paths.

**Do this instead:** Use Cargo feature gates for major modules (whether the code ships at all).
Use runtime flags for operational control within compiled-in modules (whether a compiled-in module
is active on a specific pod). Both layers have a role; they are not interchangeable.

### Anti-Pattern 4: Blocking OTA on Active Sessions Without Queuing

**What people do:** Abort OTA for pods with active billing sessions, requiring manual retry later.

**Why it's wrong:** Racing Point operates continuous sessions. Pods are in session most of the day.
An OTA that skips in-session pods leaves a split-version fleet indefinitely.

**Do this instead:** Reuse `AppState::pending_deploys` — this mechanism already exists and already
handles deferred deploy on session-end. The OTA pipeline registers the OtaDownload message in
`pending_deploys`, and the existing session-end handler dispatches it automatically.

---

## Scalability Considerations

This is an 8-pod local fleet. The architecture choices reflect that scale.

| Concern | At 8 pods (current) | If fleet grows to 50+ pods |
|---------|---------------------|---------------------------|
| Config push fan-out | Sequential send to all connected agents is fine | Batch sends, add per-pod send queue |
| Feature flag storage | SQLite with per-pod rows is fine | SQLite remains fine up to ~500 pods |
| OTA canary → staged | 1 canary + 7 sequential is fine | Need batch size as % of fleet, parallel batches |
| Standing rules gate | All checks are local (no network) — sub-100ms | Remains fast; only audit trail needs pagination |
| Reconnect replay | Full snapshot push on reconnect is fine | Same; snapshot is small (flags + config overlay) |

---

## Sources

- Verified against live source: `crates/rc-common/src/protocol.rs` (AgentMessage, CoreToAgentMessage enums)
- Verified against live source: `crates/racecontrol/src/state.rs` (AppState struct, existing pending_deploys pattern)
- Verified against live source: `crates/racecontrol/src/deploy.rs` (swap-script mechanism, validate_binary_size, rollback script)
- Verified against live source: `crates/rc-agent/src/config.rs` (AgentConfig struct, existing runtime flag: ac_evo_telemetry_enabled)
- Verified against live source: `crates/rc-agent/Cargo.toml` (existing keyboard-hook feature gate)
- Verified against live source: `crates/racecontrol/src/ws/mod.rs` (agent_senders channel, Register → Registered → immediate command pattern)
- Standing rules source: `CLAUDE.md` (41+ rules, 7 categories)
- Architecture constraint: PROJECT.md v22.0 milestone spec

---

*Architecture research for: v22.0 Feature Management & OTA Pipeline*
*Researched: 2026-03-23*
