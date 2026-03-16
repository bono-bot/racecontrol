# Architecture Research

**Domain:** AI auto-fix bot expansion — RC Bot v5.0 (9 new failure classes)
**Researched:** 2026-03-16
**Confidence:** HIGH — based on direct inspection of ai_debugger.rs, self_monitor.rs, game_process.rs, udp_heartbeat.rs, protocol.rs, types.rs, pod_monitor.rs, and PROJECT.md

---

## System Overview

### Current Bot Architecture (what exists)

```
rc-agent (per pod, Windows 11)
  |
  |-- ai_debugger.rs
  |     analyze_crash()         — Ollama/OpenRouter query, returns AiDebugSuggestion
  |     try_auto_fix()          — keyword match on suggestion text -> fix handler
  |     DebugMemory             — persisted pattern memory (C:\RacingPoint\debug-memory.json)
  |     PodStateSnapshot        — pod context at crash time
  |
  |-- self_monitor.rs           — background health check every 60s
  |     count_close_wait_on_8090()  — netstat CLOSE_WAIT detection
  |     ws_dead_secs check          — reconnect-loop exhaustion detection
  |     relaunch_self()             — detached PowerShell process restart
  |
  |-- game_process.rs           — game PID lifecycle, orphan cleanup
  |-- driving_detector.rs       — HID wheelbase input polling, DrivingState
  |-- udp_heartbeat.rs          — UDP ping/pong, HeartbeatStatus atomics
  |
  WebSocket (AgentMessage::AiDebugResult) --> racecontrol
```

**Existing fix handlers (all in ai_debugger.rs):**

| Fix Type | Trigger Keywords | What It Does |
|----------|-----------------|-------------|
| `clear_stale_sockets` | "close_wait" OR "zombie" OR "stale socket" | PowerShell kills PIDs with CLOSE_WAIT on ports 18923/18924/18925/8090 |
| `kill_error_dialogs` | "werfault" OR "error dialog" OR "crash dialog" | `taskkill /IM WerFault.exe /F` |
| `kill_stale_game` | "relaunch" + "game" OR "restart" + game exe | `taskkill /IM` for all known game EXEs |
| `clean_temp` | "disk space" OR "temp files" OR "clean temp" | PowerShell removes `$env:TEMP\*` |

**Pattern memory key:** `"{SimType}:{exit_code}"` — e.g. `"AssettoCorsa:-1"`. Instant re-apply on match.

---

## Recommended Architecture for v5.0

### System Diagram After Expansion

```
┌──────────────────────────────────────────────────────────────────┐
│                  rc-agent (per pod, Windows 11)                   │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  failure_monitor.rs  (NEW — detection loop for 9 classes)   │  │
│  │  Polls every 5-30s. Reads shared HeartbeatStatus + module   │  │
│  │  state. Constructs synthetic suggestion strings, calls       │  │
│  │  try_auto_fix(). Sends AgentMessage variants to server.      │  │
│  └─────────────────────────────────────────────────────────────┘  │
│          |                          |                              │
│          v                          v                              │
│  ┌────────────────┐    ┌────────────────────────────────────────┐ │
│  │ ai_debugger.rs │    │ billing_guard.rs  (NEW)                │ │
│  │ EXTEND:        │    │ Agent-side billing anomaly detection.  │ │
│  │ - PodStateSnap │    │ Idle drift, stuck-WaitingForGame,      │ │
│  │   shot fields  │    │ game-dead-billing-alive. Reports       │ │
│  │ - try_auto_fix │    │ AgentMessage::BillingAnomaly.          │ │
│  │   new arms     │    └────────────────────────────────────────┘ │
│  │ - new fix fns  │                                               │
│  └────────────────┘    ┌────────────────────────────────────────┐ │
│                         │ lap_filter.rs  (NEW)                   │ │
│  ┌─────────────────┐   │ Validates laps at capture time.        │ │
│  │ self_monitor.rs │   │ Cuts, invalid speed, spin detection.   │ │
│  │ EXISTING (keep) │   │ Emits valid=false + LapFlagged msg.    │ │
│  └─────────────────┘   └────────────────────────────────────────┘ │
│                                                                    │
│  Shared state (HeartbeatStatus atomics):                           │
│  ws_connected, game_running, billing_active, driving_active        │
│  + NEW: udp_receiving, last_telemetry_ts, wheelbase_hid_error     │
└─────────────────────────────┬────────────────────────────────────┘
                               | WebSocket (AgentMessage enum)
                               v
┌──────────────────────────────────────────────────────────────────┐
│                  racecontrol (server, port 8080)                   │
│                                                                    │
│  ┌─────────────────────────────────────────────────────────────┐  │
│  │  bot_coordinator.rs  (NEW — fleet-level bot decisions)       │  │
│  │  Receives: HardwareFailure, TelemetryGap, BillingAnomaly,   │  │
│  │  LapFlagged, MultiplayerFailure, PinBotEvent                │  │
│  │  Decides: StopSession vs email alert vs no-op               │  │
│  └─────────────────────────────────────────────────────────────┘  │
│                                                                    │
│  pod_monitor.rs (EXISTING — unchanged)                             │
│  billing.rs     (EXISTING — add stuck session recovery)           │
│  multiplayer.rs (EXISTING — add desync detection hook)            │
└──────────────────────────────────────────────────────────────────┘
          |
          v
┌──────────────────────────────────────────────────────────────────┐
│                rc-common (shared types + protocol)                 │
│  EXTEND types.rs:     PodFailureReason enum (all 9 classes)       │
│  EXTEND protocol.rs:  AgentMessage + 5 new variants               │
└──────────────────────────────────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Crate | Status |
|-----------|----------------|-------|--------|
| `ai_debugger.rs` | Keyword-dispatch fix handlers, DebugMemory pattern learning, Ollama/OpenRouter queries | rc-agent | MODIFY |
| `self_monitor.rs` | CLOSE_WAIT flood + WS dead -> relaunch | rc-agent | KEEP (no change) |
| `failure_monitor.rs` | Detection loop for all 9 new failure classes. Single place for all timing-based checks. | rc-agent | NEW |
| `billing_guard.rs` | Agent-side billing anomaly detection (idle drift, stuck WaitingForGame, game-dead-billing-alive) | rc-agent | NEW |
| `lap_filter.rs` | Invalid lap detection at UDP capture time (cuts, speed, spin) | rc-agent | NEW |
| `bot_coordinator.rs` | Server-side fleet decisions: billing recovery, multiplayer teardown, PIN unlock, sync failure alerts | racecontrol | NEW |
| `pod_monitor.rs` | Heartbeat checker, WatchdogState FSM, escalating backoff | racecontrol | EXISTING (no change) |
| `billing.rs` | BillingSession lifecycle — add `recover_stuck_session()` helper | racecontrol | MINOR MODIFY |
| `protocol.rs` | Wire format — add 5 new AgentMessage variants + PodFailureReason | rc-common | MODIFY |
| `types.rs` | Domain types — add PodFailureReason enum | rc-common | MODIFY |

---

## Recommended Project Structure Changes

```
crates/rc-common/src/
├── types.rs            MODIFY: add PodFailureReason enum
├── protocol.rs         MODIFY: add AgentMessage variants (5 new)
└── lib.rs              no change

crates/rc-agent/src/
├── ai_debugger.rs      MODIFY: extend PodStateSnapshot + try_auto_fix() arms + fix fns
├── self_monitor.rs     no change (existing WS/CLOSE_WAIT logic stays)
├── game_process.rs     MINOR: expose launch_elapsed_secs to failure_monitor
├── driving_detector.rs MINOR: expose last_hid_error: Option<String>
├── failure_monitor.rs  NEW: detection loop (7 agent-side failure classes)
├── billing_guard.rs    NEW: billing anomaly detection (3 billing classes)
├── lap_filter.rs       NEW: invalid lap detection
└── main.rs             MODIFY: spawn failure_monitor, billing_guard tasks

crates/racecontrol/src/
├── bot_coordinator.rs  NEW: handles new AgentMessage variants
├── ws/mod.rs           MODIFY: route new AgentMessage variants to bot_coordinator
├── billing.rs          MINOR: add recover_stuck_session()
└── multiplayer.rs      MINOR: expose desync state to bot_coordinator
```

---

## Architectural Patterns

### Pattern 1: PodFailureReason in rc-common — the Central Taxonomy

**What:** Add `PodFailureReason` enum to `rc-common/src/types.rs` as the shared vocabulary for all failure classes. Both detection code (agent) and handling code (server) reference the same variants.

**When to use:** Every new failure class gets a variant here first, before any detection or fix code is written.

**Trade-offs:** Forces deliberate naming before implementation. rc-common compiles first — any change here breaks both consuming crates until they handle new variants. Accept this: it's the right forcing function to keep the taxonomy stable.

**Recommended enum:**

```rust
// rc-common/src/types.rs — ADD
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodFailureReason {
    // Crash/hang class (agent-side fix)
    GameFrozen,
    AgentStuck,
    ProcessHung,
    // Billing edge cases (server-side recovery)
    SessionStuckWaitingForGame,
    IdleBillingDrift,
    CreditSyncFailed,
    // Network/connection (self_monitor handles WS dead already)
    WsLost,          // reserved — self_monitor.rs handles this already
    IpDrifted,       // server-side diagnosis only
    // Hardware (agent-side fix)
    WheelbaseDisconnected,
    FfbFault,
    // Game launch (agent-side fix)
    ContentManagerHang,
    LaunchTimeout,
    // Telemetry (agent reports, server alerts)
    UdpDataMissing,
    TelemetryInvalid,
    // Multiplayer (server-side coordination)
    MultiplayerDesync,
    MultiplayerServerDisconnect,
    // Kiosk/PIN (server routes to agent)
    PinValidationFailed,
    StaffUnlockNeeded,
    // Lap filtering (agent flags, server stores)
    LapCut,
    LapInvalidSpeed,
    LapSpin,
}
```

### Pattern 2: New AgentMessage Variants (5 needed)

**What:** Add variants to `AgentMessage` in `protocol.rs` for each agent-to-server signal that doesn't fit existing variants. Use existing `AiDebugResult` for crash/hang results. Only add what's new.

**When to use:** When the agent needs to tell the server about a failure class that has no existing representation.

**Trade-offs:** Adding variants with `#[serde(tag = "type")]` is backward compatible — old server ignores unknown `type` values. New server + old agent just means the variant is never sent. This is safe for rolling deploy.

**New variants to add:**

```rust
// rc-common/src/protocol.rs — ADD to AgentMessage

/// Agent detected a hardware failure (USB disconnect, FFB fault)
HardwareFailure {
    pod_id: String,
    reason: PodFailureReason,
    detail: String,
},

/// Agent detected telemetry gap (no UDP data for N seconds while game running)
TelemetryGap {
    pod_id: String,
    sim_type: SimType,
    gap_seconds: u32,
},

/// Agent detected billing anomaly (stuck session, idle drift, game dead + billing alive)
BillingAnomaly {
    pod_id: String,
    billing_session_id: String,
    reason: PodFailureReason,
    detail: String,
},

/// Agent flagged an invalid lap at capture time
LapFlagged {
    pod_id: String,
    lap_id: String,
    reason: PodFailureReason,
    detail: String,
},

/// Agent detected multiplayer session issue (desync or server disconnect)
MultiplayerFailure {
    pod_id: String,
    reason: PodFailureReason,
    session_id: Option<String>,
},
```

### Pattern 3: try_auto_fix() Extension — Arms Only, No Restructure

**What:** The existing `try_auto_fix(suggestion: &str, snapshot: &PodStateSnapshot)` is a keyword-dispatch function. New failure classes add new match arms. The function signature does not change.

**When to use:** Every pod-side fix that can be triggered by a keyword in the AI suggestion OR a synthetic suggestion string from `failure_monitor.rs`.

**Trade-offs:** The `suggestion: &str` indirection exists so that DebugMemory learning works for both AI-triggered and detection-triggered fixes. Keeping this consistent means pattern memory learns from both sources. Do not add a parallel dispatch path that bypasses suggestion text.

**How to add new arms (order matters — put specific before general):**

```rust
// In try_auto_fix() — ADD before final None return

// USB wheelbase reset
if lower.contains("wheelbase") && lower.contains("usb reset") {
    return Some(fix_usb_reset(snapshot));
}

// Content Manager hang
if lower.contains("content manager") && (lower.contains("hang") || lower.contains("kill cm")) {
    return Some(fix_kill_content_manager());
}

// Launch timeout — kill and signal for relaunch
if lower.contains("launch timeout") || (lower.contains("acs.exe") && lower.contains("timeout")) {
    return Some(fix_launch_timeout(snapshot));
}

// FFB fault reset
if lower.contains("ffb fault") || lower.contains("ffb reset") {
    return Some(fix_ffb_reset(snapshot));
}

// Telemetry gap — log only (no deterministic fix beyond what self_monitor does)
if lower.contains("telemetry gap") || lower.contains("udp missing") {
    return Some(fix_log_telemetry_gap(snapshot)); // non-destructive: log + alert
}
```

### Pattern 4: PodStateSnapshot Expansion

**What:** Add new fields to `PodStateSnapshot` in `ai_debugger.rs`. These are pod-local facts captured at detection time. Populate them in `failure_monitor.rs` before calling `try_auto_fix()`.

**When to use:** When a new fix handler needs information not currently in the snapshot (e.g., whether wheelbase HID has errored, how long since the last UDP frame, whether a multiplayer session is active).

**Trade-offs:** The snapshot is never persisted to disk in this form — it's ephemeral context passed to the AI prompt builder and fix handlers. Growing it has no migration cost. Add `#[serde(default)]` on new fields so the struct can still be constructed from partial JSON in tests.

**Fields to add:**

```rust
// In ai_debugger.rs — ADD to PodStateSnapshot
#[serde(default)]
pub udp_receiving: bool,                        // any UDP frame in last 10s
#[serde(default)]
pub last_telemetry_secs_ago: u64,               // seconds since last UDP frame
#[serde(default)]
pub wheelbase_hid_error: Option<String>,        // HID error string if disconnected
#[serde(default)]
pub game_launch_elapsed_secs: u64,              // seconds since LaunchGame received
#[serde(default)]
pub billing_status: Option<String>,             // BillingSessionStatus as string
#[serde(default)]
pub multiplayer_active: bool,                   // AC LAN session in progress
#[serde(default)]
pub consecutive_pin_failures: u32,              // failed PIN attempts on lock screen
```

### Pattern 5: Detection in failure_monitor.rs, Fixes in ai_debugger.rs

**What:** `failure_monitor.rs` is a single polling task that checks all 9 failure conditions from shared atomic state. When it detects a problem, it constructs a synthetic suggestion string with canonical keywords and calls `try_auto_fix()`. Fix implementations stay in `ai_debugger.rs`.

**When to use:** Every detection loop belongs in `failure_monitor.rs`. No detection logic scattered in other modules.

**Trade-offs:** The indirection through a synthetic suggestion string feels slightly redundant for deterministic detectors. The reason to keep it: `DebugMemory::record_fix()` and `instant_fix()` only fire when fixes go through `try_auto_fix()`. Keeping this path ensures the pattern learning loop captures detection-triggered fixes too.

**Synthetic suggestion keyword conventions (canonical — must match try_auto_fix arms):**

| Failure Class | Canonical Synthetic String |
|--------------|---------------------------|
| Game frozen | `"Game process frozen — relaunch game acs.exe"` |
| Content Manager hang | `"Content Manager hang — kill CM process"` |
| Launch timeout | `"launch timeout — acs.exe timeout exceeded"` |
| USB wheelbase disconnect | `"Wheelbase usb reset required — HID disconnected"` |
| FFB fault | `"ffb fault detected — ffb reset needed"` |
| Telemetry gap | `"telemetry gap — udp missing from running game"` |
| Disk space | `"disk space — clean temp files"` (existing) |

---

## Data Flow

### Agent-Side Fix Flow (pod-local)

```
failure_monitor.rs detects condition (polling every 5-30s)
    |
    v
Build PodStateSnapshot with current pod state
    |
    v
Construct synthetic suggestion with canonical keywords
    |
    v
ai_debugger::try_auto_fix(suggestion, &snapshot) -> Option<AutoFixResult>
    |
    +-- arm matched -> fix_handler() runs (e.g., taskkill, HID reset, etc.)
    |       |
    |       v
    |   DebugMemory::record_fix() -- saves outcome to debug-memory.json
    |   Send AgentMessage::AiDebugResult to racecontrol (for dashboard)
    |   Send AgentMessage::HardwareFailure / TelemetryGap (for specific classes)
    |
    +-- no arm matched -> log_event("[rc-bot] No auto-fix for: ...")
```

### Server-Side Coordination Flow

```
rc-agent sends AgentMessage::{HardwareFailure|TelemetryGap|BillingAnomaly|LapFlagged|MultiplayerFailure}
    |
    v
racecontrol ws/mod.rs handle_agent() receives, routes to bot_coordinator::handle()
    |
    +-- BillingAnomaly -> billing::recover_stuck_session()
    |       -> sends CoreToAgentMessage::StopSession to agent
    |       -> agent kills game, shows lock screen, sends SessionUpdate::Finished
    |       -> then billing::end_session() fires (existing session end flow preserved)
    |
    +-- TelemetryGap -> log + DashboardEvent::BotAction (alert staff kiosk)
    |       -> no CoreToAgentMessage needed unless gap persists >10min (then StopGame)
    |
    +-- HardwareFailure -> log + email alert if billing active
    |
    +-- LapFlagged -> forward to lap_tracker.rs for valid=false storage
    |       -> DashboardEvent::LapFlagged to kiosk
    |
    +-- MultiplayerFailure -> bot_coordinator checks other pods in same session
            -> sends StopSession to affected pods if desync confirmed
```

### Billing Edge Case Flow (billing_guard.rs)

```
billing_guard.rs polls every 30s (separate from failure_monitor)
    |
    +-- BillingSessionStatus::WaitingForGame for >5min?
    |       -> AgentMessage::BillingAnomaly { reason: SessionStuckWaitingForGame }
    |
    +-- billing_active=true AND game_running=false?
    |       -> AgentMessage::BillingAnomaly { reason: SessionStuckWaitingForGame }
    |
    +-- billing_active=true AND driving_active=false for >10min?
            -> AgentMessage::BillingAnomaly { reason: IdleBillingDrift }
```

### Lap Filter Flow

```
UDP frame arrives -> sims/assetto_corsa.rs or sims/f1_25.rs detects lap completion
    |
    v
lap_filter::validate(lap: &LapData, recent_frames: &[TelemetryFrame]) -> LapValidity
    |
    +-- LapValidity::Valid
    |       -> AgentMessage::LapCompleted (valid=true, existing path)
    |
    +-- LapValidity::Invalid { reason: PodFailureReason }
            -> AgentMessage::LapCompleted (valid=false)
            -> AgentMessage::LapFlagged { reason, detail }  (new)
```

---

## Integration Points: Agent vs Server vs Common

### What Lives Where

| Failure Class | Detection | Fix/Action | Server Role | Protocol Change |
|--------------|-----------|-----------|------------|----------------|
| Game freeze / process hung | `failure_monitor.rs` polls game PID | `fix_kill_stale_game()` (existing) | Log AiDebugResult | None (existing AiDebugResult) |
| rc-agent stuck | `self_monitor.rs` (existing) | `relaunch_self()` (existing) | None | None |
| Content Manager hang | `failure_monitor.rs` polls launch_elapsed_secs + CM PID | NEW `fix_kill_content_manager()` | Receives GameStateUpdate::Error | None (existing GameStateUpdate) |
| Launch timeout | `failure_monitor.rs` polls launch_elapsed_secs | NEW `fix_launch_timeout()` | Receives GameStateUpdate::Error | None |
| USB wheelbase disconnect | `failure_monitor.rs` polls `driving_detector.last_hid_error` | NEW `fix_usb_reset()` | NEW: receives HardwareFailure | NEW: HardwareFailure variant |
| FFB fault | `failure_monitor.rs` polls HID state | NEW `fix_ffb_reset()` | NEW: receives HardwareFailure | NEW: HardwareFailure variant |
| Telemetry gap | `failure_monitor.rs` polls `last_telemetry_ts` | `fix_log_telemetry_gap()` (non-destructive) | NEW: receives TelemetryGap, alerts | NEW: TelemetryGap variant |
| Billing stuck/idle drift | `billing_guard.rs` polls BillingSessionStatus + game state | Server-side: StopSession | NEW: receives BillingAnomaly | NEW: BillingAnomaly variant |
| Cloud sync failure | `cloud_sync.rs` (server) retry exhaustion | Email alert (existing alerter) | bot_coordinator | None (server-internal) |
| Multiplayer desync | `failure_monitor.rs` polls AC server state | NEW: reports to server | bot_coordinator: StopSession | NEW: MultiplayerFailure variant |
| PIN validation failure | `lock_screen.rs` (existing PinFailed path) | Server sends CoreToAgentMessage::PinFailed (existing) | bot_coordinator: escalate after N failures | None (existing path reused) |
| Lap cut / invalid speed / spin | `lap_filter.rs` on lap completion | valid=false on LapCompleted (existing field) | NEW: receives LapFlagged, stores + alerts | NEW: LapFlagged variant |

### rc-common Changes Summary

**types.rs additions:**
- `PodFailureReason` enum (21 variants)

**protocol.rs additions (AgentMessage):**
- `HardwareFailure { pod_id, reason, detail }`
- `TelemetryGap { pod_id, sim_type, gap_seconds }`
- `BillingAnomaly { pod_id, billing_session_id, reason, detail }`
- `LapFlagged { pod_id, lap_id, reason, detail }`
- `MultiplayerFailure { pod_id, reason, session_id }`

**No new CoreToAgentMessage variants needed** — existing `StopSession`, `StopGame`, `PinFailed` are sufficient for all server-to-agent bot responses.

---

## Build Order (Cross-Crate Dependency Sequence)

Violating this order causes compile errors. rc-common is the shared contract; both consuming crates break until new variants are handled.

```
Phase 1 — rc-common (compile first, other crates depend on it)
  1. Add PodFailureReason enum to types.rs
  2. Add 5 new AgentMessage variants to protocol.rs
  → cargo test -p rc-common
  → Verify: all existing tests pass, serialization round-trips work

Phase 2 — rc-agent detection infrastructure (no server changes needed yet)
  3. Extend PodStateSnapshot fields in ai_debugger.rs
  4. Add new try_auto_fix() match arms + new fix handler functions
  5. Create failure_monitor.rs (7 agent-side failure class detectors)
  6. Create billing_guard.rs (3 billing edge class detectors)
  7. Create lap_filter.rs (3 lap validity detectors)
  8. Modify main.rs: spawn failure_monitor and billing_guard tasks
  → cargo test -p rc-agent-crate
  → Verify: all 47+ existing tests pass + new tests for each fix handler

Phase 3 — racecontrol bot logic (can be built after Phase 1 compiles)
  9. Create bot_coordinator.rs with handlers for 5 new AgentMessage variants
  10. Modify ws/mod.rs: route new AgentMessage variants to bot_coordinator
  11. Add billing::recover_stuck_session() helper
  12. Add multiplayer desync state to multiplayer.rs
  → cargo test -p racecontrol-crate
  → Verify: all existing tests pass + bot_coordinator unit tests
```

**Why this order:**
- Phase 1 locks the protocol contract. Without it, both Phase 2 and Phase 3 code won't compile.
- Phase 2 and Phase 3 can be developed in parallel after Phase 1, but must both be deployed before Phase 3 bot logic activates.
- Never modify protocol.rs and rc-agent in the same commit — keep protocol changes as a separate commit so the crate version history is clean.

---

## Anti-Patterns to Avoid

### Anti-Pattern 1: Detection Logic Scattered Across Existing Modules

**What people do:** Add telemetry gap detection inside `udp_heartbeat.rs`, USB polling inside `driving_detector.rs`, billing check inside the main WS loop.

**Why it's wrong:** Detection logic scattered across 8 modules is impossible to test in isolation and invisible at a glance. When a detection threshold needs tuning, you have to find it among 8 files.

**Do this instead:** All detection loops live in `failure_monitor.rs`. Existing modules expose state (atomics, Option<String> errors) via `HeartbeatStatus` or module-level fields. `failure_monitor.rs` reads state and decides. One file owns all detection policy.

### Anti-Pattern 2: Calling Fix Handlers Directly, Bypassing try_auto_fix()

**What people do:** `failure_monitor.rs` detects game freeze and calls `fix_kill_stale_game()` directly.

**Why it's wrong:** Bypasses `DebugMemory::record_fix()`. The pattern memory learning loop — which enables sub-100ms instant fix replay on recurrence — only fires when fixes go through `try_auto_fix()`. Bypass it and the bot gets dumber over time.

**Do this instead:** Construct a synthetic suggestion string with canonical keywords and call `try_auto_fix(synthetic_suggestion, &snapshot)`. The keyword contract is the dispatch mechanism.

### Anti-Pattern 3: Server-Side Billing End Without Agent Confirmation

**What people do:** `bot_coordinator` receives `BillingAnomaly` and calls `billing::end_session()` directly.

**Why it's wrong:** Ends the billing record while the game is still running. Customer session terminates unexpectedly. Violates the existing invariant: "lock screen before game kill, game kill before billing end."

**Do this instead:** `bot_coordinator` sends `CoreToAgentMessage::StopSession` to the agent. Agent stops the game, shows lock screen, sends `AgentMessage::SessionUpdate { status: Finished }`. Only on receiving `SessionUpdate::Finished` does `billing::end_session()` fire. Preserves the existing session end flow.

### Anti-Pattern 4: Modifying Existing AgentMessage Variants

**What people do:** Add a `failure_reason: Option<PodFailureReason>` field to the existing `AgentMessage::GameCrashed` variant.

**Why it's wrong:** `protocol.rs` uses `#[serde(tag = "type", content = "data")]`. Adding fields to existing variants is backward-incompatible for the server if it deserializes into the old struct definition. Pods on old binary + new server = silent data loss or panic on field missing.

**Do this instead:** Add new variants. `GameCrashed` stays unchanged. The new `HardwareFailure`, `TelemetryGap` etc. are separate variants that old servers silently ignore.

### Anti-Pattern 5: PodFailureReason as a String Rather Than Enum

**What people do:** Pass `reason: String` in the new AgentMessage variants (e.g., "wheelbase_disconnected") to avoid modifying rc-common.

**Why it's wrong:** Strings are misspellable, non-exhaustive in match arms, and can't be doc-linked. The whole point of rc-common is typed shared contracts. A protocol fix that avoids touching rc-common is a protocol fix that will cause a bug.

**Do this instead:** Define `PodFailureReason` in rc-common types.rs first (Phase 1), then use it everywhere.

---

## Scaling Considerations

This venue is 8 pods and will remain small. Architecture concerns are about reliability, not scale.

| Concern | At 8 pods (now) | Notes |
|---------|-----------------|-------|
| failure_monitor polling overhead | 1 task per agent, ~100ms checks, negligible CPU | No concern |
| DebugMemory JSON on pod disk | Per-pod, ~50KB, survives agent restart | Atomic write-rename prevents corruption |
| bot_coordinator concurrency | Sequential message processing in one tokio task | Fine for 8 pods; scale only if message backlog occurs |
| billing_guard poll interval | 30s — lag before anomaly detected is acceptable | Could reduce to 10s with no cost |
| lap_filter per-lap overhead | In-memory validation on each lap completion, <1ms | No concern |

---

## Sources

- Direct inspection: `crates/rc-agent/src/ai_debugger.rs` — full file, 767 lines (2026-03-16)
- Direct inspection: `crates/rc-agent/src/self_monitor.rs` — full file, 219 lines (2026-03-16)
- Direct inspection: `crates/rc-agent/src/game_process.rs` — partial, 80 lines (2026-03-16)
- Direct inspection: `crates/rc-agent/src/udp_heartbeat.rs` — partial, 80 lines (2026-03-16)
- Direct inspection: `crates/rc-common/src/protocol.rs` — full file including all AgentMessage variants (2026-03-16)
- Direct inspection: `crates/rc-common/src/types.rs` — full file, DrivingState/GameState/BillingSessionStatus (2026-03-16)
- Direct inspection: `crates/racecontrol/src/pod_monitor.rs` — partial, WatchdogState FSM (2026-03-16)
- Direct inspection: `.planning/codebase/ARCHITECTURE.md` — full (2026-03-16)
- Direct inspection: `.planning/PROJECT.md` — v5.0 requirements (2026-03-16)
- Confidence: HIGH — all claims derived from reading actual source files, no training-data assumptions

---

*Architecture research for: RC Bot Expansion (v5.0) — ai_debugger.rs + 9 failure classes*
*Researched: 2026-03-16*
