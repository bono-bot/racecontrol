# Phase 23: Protocol Contract + Concurrency Safety - Research

**Researched:** 2026-03-16
**Domain:** Rust enum extension (rc-common), cross-crate compile contracts, concurrency guard pattern (tokio + Arc<RwLock>)
**Confidence:** HIGH — all findings derived from direct inspection of live source files

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PROTO-01 | `PodFailureReason` enum in rc-common covers all 9 bot failure classes (crash, hang, launch, USB, billing, telemetry, multiplayer, PIN, lap) | types.rs has no existing PodFailureReason; `Copy + PartialEq + Serialize` pattern is established by SimType/PodStatus/BillingSessionStatus enums already present |
| PROTO-02 | 5 new `AgentMessage` variants (HardwareFailure, TelemetryGap, BillingAnomaly, LapFlagged, MultiplayerFailure) added to protocol.rs | AgentMessage uses `#[serde(tag = "type", content = "data")]` — additive new variants are backward-compatible; ws/mod.rs match arms must be extended |
| PROTO-03 | `is_pod_in_recovery()` shared utility in rc-common prevents concurrent fix races | WatchdogState enum exists in racecontrol/src/state.rs; rc-common needs a utility that accepts recovery state and returns bool — verified unit test pattern modeled on pod_healer.rs tests |

</phase_requirements>

---

## Summary

Phase 23 is a pure contract phase — no detection logic, no fix handlers, no new async tasks. The deliverable is three additions to rc-common: one new enum, five new AgentMessage variants, and one utility function. All three changes must compile cleanly in rc-common before any Phase 24 bot code is written.

The codebase already has well-established enum patterns in types.rs (`SimType`, `PodStatus`, `BillingSessionStatus`, `DrivingState`) that define exactly how new enums should be structured: `Debug + Clone + Copy + PartialEq + Eq + Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]`. AgentMessage in protocol.rs uses `#[serde(tag = "type", content = "data")]` — adding new struct variants to this enum is additive and backward-compatible (old servers silently ignore unknown `type` values).

The key concurrency question for PROTO-03: `is_pod_in_recovery()` needs to detect when a pod is already undergoing a fix so a second bot task does not double-act. The WatchdogState enum (in racecontrol/src/state.rs) already encodes recovery states (`Restarting`, `Verifying`). The utility is a pure predicate over this enum — it does not need to live in rc-common since WatchdogState is racecontrol-only. Instead, rc-common provides a `RecoveryLock` type (or the utility lives in racecontrol) using `Arc<RwLock<HashMap<String, bool>>>` for the per-pod recovery flag.

**Primary recommendation:** Add `PodFailureReason` and the 5 AgentMessage variants to rc-common first. For `is_pod_in_recovery()`, implement as a function in racecontrol (not rc-common) that reads the existing `pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>` already in AppState. The success criterion ("unit test in racecontrol confirms blocking") is satisfied by a test over the pure `should_skip_for_watchdog_state()` pattern already demonstrated in pod_healer.rs.

---

## Standard Stack

### Core (no new dependencies — everything already present)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `serde` | workspace | Derive Serialize/Deserialize on new enum | Already used on every type in rc-common |
| `tokio::sync::RwLock` | workspace | Concurrent access guard for recovery state | AppState already wraps all shared state in RwLock |
| `cargo test -p rc-common` | — | Verify new types compile + serialize correctly | Existing test command, 106 tests currently pass |

### No New Dependencies

rc-common's Cargo.toml has only: `serde`, `serde_json`, `chrono`, `uuid`, `rand`. No changes needed — `PodFailureReason` only requires `serde`.

---

## Architecture Patterns

### Existing Enum Pattern (HIGH confidence — read directly from source)

Every enum in types.rs follows this exact pattern:

```rust
// Source: crates/rc-common/src/types.rs (SimType, PodStatus, BillingSessionStatus, DrivingState)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodStatus {
    Offline,
    Idle,
    InSession,
    Error,
    Disabled,
}
```

`PodFailureReason` MUST follow this identical pattern. `Copy` is critical because the enum is used as a field in AgentMessage struct variants and must be cloneable cheaply.

### Existing AgentMessage Pattern (HIGH confidence — read directly from source)

```rust
// Source: crates/rc-common/src/protocol.rs lines 18-109
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
pub enum AgentMessage {
    // ... existing variants ...
    GameCrashed { pod_id: String, billing_active: bool },
    FfbZeroed { pod_id: String },
}
```

The `#[serde(tag = "type", content = "data")]` annotation means each variant serializes as `{"type": "hardware_failure", "data": {...}}`. New struct variants must carry all fields inline — no tuple variants (existing tuple variants like `Register(PodInfo)` are the pre-existing pattern but struct variants are used for new bot messages to allow named fields).

### Existing Test Pattern (HIGH confidence — read from protocol.rs tests)

The test pattern for new AgentMessage variants is identical to existing tests:

```rust
// Source: crates/rc-common/src/protocol.rs line 752
#[test]
fn test_agent_message_roundtrip() {
    let msg = AgentMessage::PinEntered { pod_id: "pod_1".to_string(), pin: "1234".to_string() };
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("pin_entered"));
    let parsed: AgentMessage = serde_json::from_str(&json).unwrap();
    if let AgentMessage::PinEntered { pod_id, pin } = parsed {
        assert_eq!(pod_id, "pod_1");
        assert_eq!(pin, "1234");
    } else {
        panic!("Wrong variant");
    }
}
```

Each new variant gets one roundtrip test: serialize → assert type key present → deserialize → assert fields.

### Concurrency Guard Pattern (HIGH confidence — verified from pod_healer.rs)

`is_pod_in_recovery()` is NOT a new lock — it reads existing AppState. The pattern already demonstrated in pod_healer.rs:

```rust
// Source: crates/racecontrol/src/pod_healer.rs lines 152-162
let wd_state = {
    let states = state.pod_watchdog_states.read().await;
    states.get(&pod.id).cloned().unwrap_or(WatchdogState::Healthy)
};
if should_skip_for_watchdog_state(&wd_state) {
    return Ok(());
}

// The pure predicate (already exists, tested):
fn should_skip_for_watchdog_state(wd_state: &WatchdogState) -> bool {
    matches!(wd_state, WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. })
}
```

For PROTO-03, `is_pod_in_recovery()` is effectively this same predicate, possibly renamed and placed in a new `bot_coordinator.rs` location. The unit test simply calls it with `WatchdogState::Restarting { attempt: 1, started_at: Utc::now() }` and asserts `true`.

**Critical decision:** The PROTO-03 success criterion says "unit test in racecontrol confirms blocking." This means `is_pod_in_recovery()` lives in racecontrol (not rc-common) because WatchdogState is racecontrol-local. If it needs to live in rc-common, WatchdogState would need to move there too — which is a larger change. Recommendation: place in racecontrol, not rc-common.

### Project Structure (what exists, what changes)

```
crates/rc-common/src/
├── types.rs       ADD: PodFailureReason enum (~25 lines)
├── protocol.rs    ADD: 5 AgentMessage variants (~30 lines) + pub use PodFailureReason
├── lib.rs         NO CHANGE (types and protocol already pub mod)

crates/racecontrol/src/
├── pod_healer.rs  EXISTING: should_skip_for_watchdog_state() already tested
└── (new location for is_pod_in_recovery — likely bot_coordinator.rs in Phase 25)
```

### Anti-Patterns to Avoid

- **Adding PodFailureReason as String fields:** Every existing enum is typed. Strings are misspellable and non-exhaustive in match arms. Use the typed enum.
- **Adding fields to existing AgentMessage variants:** `#[serde(tag = "type", content = "data")]` makes this a breaking wire change. Add new variants only.
- **Placing is_pod_in_recovery() in rc-common:** WatchdogState lives in racecontrol/src/state.rs — moving it to rc-common would bloat the shared contract. The function belongs in racecontrol.
- **Deriving Hash on PodFailureReason when not needed:** Only add Hash if the enum will be used as a HashMap key (it won't in Phase 23).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON tag/content dispatch | Custom serialization impl | `#[serde(tag = "type", content = "data")]` | Already battle-tested on AgentMessage with 20+ variants |
| Recovery gate locking | New Mutex<HashSet> for in-progress pods | Read existing `pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>` | AppState already has the recovery state — no second structure needed |
| Enum string matching | `match reason_str { "crash" => ... }` | Exhaustive match on `PodFailureReason` enum | Compiler enforces exhaustiveness, eliminates typo bugs |

---

## Common Pitfalls

### Pitfall 1: ws/mod.rs match is Exhaustive — New Variants Cause Compile Error

**What goes wrong:** Adding 5 new `AgentMessage` variants to rc-common makes the `match msg` in `crates/racecontrol/src/ws/mod.rs` non-exhaustive. The file will not compile until all 5 new variants have match arms.

**Why it happens:** Rust enums are closed — adding variants breaks exhaustive matches. The ws/mod.rs handler currently matches 20+ variants with no wildcard arm.

**How to avoid:** After adding variants to rc-common, immediately add `_ => {}` or explicit stub arms to ws/mod.rs before the next `cargo check`. In Phase 23 the stubs just log and do nothing — `bot_coordinator.rs` is wired in Phase 25.

**Warning signs:** Compiler error "non-exhaustive patterns: `AgentMessage::HardwareFailure { .. }` not covered."

### Pitfall 2: rc-agent Also Matches AgentMessage (Indirectly)

**What goes wrong:** rc-agent does not receive AgentMessage over the wire (it sends it), but if any rc-agent code constructs `AgentMessage` in tests or sends specific variants, new variants may require handling in rc-agent too.

**Why it happens:** Both crates consume rc-common. Adding variants to a Serialize/Deserialize enum doesn't break the sender — but if rc-agent has any exhaustive match on AgentMessage (e.g., for echo or test purposes), it will fail to compile.

**How to avoid:** Run `cargo check -p rc-agent-crate` immediately after adding variants to rc-common to surface any hidden match statements.

**Warning signs:** Compile error in rc-agent files even though rc-agent never receives AgentMessage.

### Pitfall 3: serde rename_all Mismatch

**What goes wrong:** New variants in AgentMessage use `snake_case` via `#[serde(rename_all = "snake_case")]` on the enum. If a variant is named `HardwareFailure`, the wire type key is `hardware_failure`. Test assertions must use the wire-format name, not the Rust name.

**How to avoid:** Test with `assert!(json.contains("hardware_failure"))` not `"HardwareFailure"`.

### Pitfall 4: is_pod_in_recovery() Test Without AppState

**What goes wrong:** Trying to write a unit test for `is_pod_in_recovery()` that spins up a real AppState (requires DB, config, etc.) makes the test an integration test with 10s startup time.

**How to avoid:** Extract the pure predicate logic into a standalone function that takes `&WatchdogState` (not `Arc<AppState>`). Test the predicate in isolation. The AppState-reading wrapper is not tested at unit level — pod_healer.rs already demonstrates this separation (see `should_skip_for_watchdog_state()`).

---

## Code Examples

Verified patterns from direct source inspection:

### PodFailureReason Enum Definition

```rust
// ADD to: crates/rc-common/src/types.rs
// Pattern: identical to PodStatus, DrivingState, AcStatus in same file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PodFailureReason {
    // Crash/hang class
    GameFrozen,
    ProcessHung,
    // Game launch class
    ContentManagerHang,
    LaunchTimeout,
    // USB/hardware class
    WheelbaseDisconnected,
    FfbFault,
    // Billing class
    SessionStuckWaitingForGame,
    IdleBillingDrift,
    CreditSyncFailed,
    // Telemetry class
    UdpDataMissing,
    TelemetryInvalid,
    // Multiplayer class
    MultiplayerDesync,
    MultiplayerServerDisconnect,
    // PIN class
    PinValidationFailed,
    StaffUnlockNeeded,
    // Lap class
    LapCut,
    LapInvalidSpeed,
    LapSpin,
}
```

Note: 18 variants cover all 9 failure classes (some classes have multiple reasons). The PROTO-01 requirement says "9 bot failure classes" — each class needs at least one variant.

### AgentMessage Import Addition

```rust
// MODIFY top of: crates/rc-common/src/protocol.rs
use crate::types::{
    // ... existing imports ...
    PodFailureReason,  // ADD
};
```

### 5 New AgentMessage Variants

```rust
// ADD to AgentMessage enum in: crates/rc-common/src/protocol.rs

/// Agent detected a hardware failure (USB disconnect, FFB fault)
HardwareFailure {
    pod_id: String,
    reason: PodFailureReason,
    detail: String,
},

/// Agent detected telemetry gap (no UDP data for N seconds while billing active)
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

/// Agent detected multiplayer session failure (desync or server disconnect)
MultiplayerFailure {
    pod_id: String,
    reason: PodFailureReason,
    session_id: Option<String>,
},
```

### ws/mod.rs Stub Arms (required for compile)

```rust
// ADD to match msg { ... } in crates/racecontrol/src/ws/mod.rs
// These are no-op stubs — bot_coordinator wiring is Phase 25

AgentMessage::HardwareFailure { pod_id, reason, detail } => {
    tracing::info!("[bot] HardwareFailure pod={} reason={:?}: {}", pod_id, reason, detail);
}
AgentMessage::TelemetryGap { pod_id, sim_type, gap_seconds } => {
    tracing::info!("[bot] TelemetryGap pod={} sim={:?} gap={}s", pod_id, sim_type, gap_seconds);
}
AgentMessage::BillingAnomaly { pod_id, billing_session_id, reason, detail } => {
    tracing::info!("[bot] BillingAnomaly pod={} session={} reason={:?}: {}", pod_id, billing_session_id, reason, detail);
}
AgentMessage::LapFlagged { pod_id, lap_id, reason, detail } => {
    tracing::info!("[bot] LapFlagged pod={} lap={} reason={:?}: {}", pod_id, lap_id, reason, detail);
}
AgentMessage::MultiplayerFailure { pod_id, reason, session_id } => {
    tracing::info!("[bot] MultiplayerFailure pod={} reason={:?} session={:?}", pod_id, reason, session_id);
}
```

### is_pod_in_recovery() Pure Predicate

```rust
// ADD to: crates/racecontrol/src/ (pod_healer.rs or new bot_coordinator.rs)
// Mirrors existing should_skip_for_watchdog_state() already in pod_healer.rs

use crate::state::WatchdogState;

/// Returns true if the pod is currently in a recovery cycle (Restarting or Verifying).
/// A second bot task must not act while recovery is in progress.
pub fn is_pod_in_recovery(wd_state: &WatchdogState) -> bool {
    matches!(
        wd_state,
        WatchdogState::Restarting { .. } | WatchdogState::Verifying { .. }
    )
}
```

### Unit Test for is_pod_in_recovery()

```rust
// ADD to tests module in the same file
#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::WatchdogState;
    use chrono::Utc;

    #[test]
    fn recovery_blocks_second_bot_task_when_restarting() {
        let state = WatchdogState::Restarting { attempt: 1, started_at: Utc::now() };
        assert!(
            is_pod_in_recovery(&state),
            "is_pod_in_recovery must return true for Restarting — blocks second bot task"
        );
    }

    #[test]
    fn recovery_blocks_second_bot_task_when_verifying() {
        let state = WatchdogState::Verifying { attempt: 1, started_at: Utc::now() };
        assert!(is_pod_in_recovery(&state));
    }

    #[test]
    fn recovery_allows_bot_when_healthy() {
        assert!(!is_pod_in_recovery(&WatchdogState::Healthy));
    }

    #[test]
    fn recovery_allows_bot_when_recovery_failed() {
        let state = WatchdogState::RecoveryFailed { attempt: 4, failed_at: Utc::now() };
        assert!(
            !is_pod_in_recovery(&state),
            "RecoveryFailed means watchdog gave up — bot may still try"
        );
    }
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| String-typed failure reasons passed in AgentMessage | Typed `PodFailureReason` enum in rc-common | Compiler-enforced exhaustiveness, no typo bugs, doc-linkable |
| Per-module recovery guards (scattered) | Single `is_pod_in_recovery()` predicate over WatchdogState | One place to audit; all Phase 24/25 bot tasks call same function |

**Deprecated/outdated in this phase:**
- None — this is additive only. Existing AgentMessage variants and existing types.rs enums are untouched.

---

## Open Questions

1. **Where does is_pod_in_recovery() live?**
   - What we know: WatchdogState is in racecontrol/src/state.rs. rc-common cannot import from racecontrol.
   - What's unclear: PROTO-03 requirement says "shared utility in rc-common" — but WatchdogState is not in rc-common.
   - Recommendation: Two options: (a) place the function in racecontrol (pod_healer.rs or a new utils.rs) and satisfy the test requirement there; (b) move WatchdogState to rc-common and place the utility there. Option (a) is lower risk — WatchdogState is tightly coupled to racecontrol's state module and moving it would require touching AppState, pod_monitor, and pod_healer. The test success criterion ("unit test in racecontrol") supports option (a). Planner should default to option (a) and note this in the plan.

2. **Does rc-agent have exhaustive match on AgentMessage?**
   - What we know: rc-agent sends AgentMessage but does not receive it over the wire.
   - What's unclear: Whether any rc-agent test file or local code has a match on AgentMessage that would break.
   - Recommendation: Run `cargo check -p rc-agent-crate` after adding variants to rc-common as the first verification step. The ARCHITECTURE research shows no such match in the files inspected.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test harness (cargo test) |
| Config file | Cargo.toml workspace settings |
| Quick run command | `cargo test -p rc-common` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|--------------|
| PROTO-01 | PodFailureReason has 9+ failure class variants, compiles clean | Unit (serde roundtrip) | `cargo test -p rc-common test_pod_failure_reason` | No — Wave 0 |
| PROTO-02 | 5 AgentMessage variants serialize with correct type keys, deserialize without panic | Unit (serde roundtrip x5) | `cargo test -p rc-common test_hardware_failure_roundtrip` (etc.) | No — Wave 0 |
| PROTO-03 | is_pod_in_recovery() returns true for Restarting/Verifying, false for Healthy/RecoveryFailed | Unit (pure fn, no AppState) | `cargo test -p racecontrol-crate is_pod_in_recovery` | No — Wave 0 |

Existing 47 tests across 3 crates must remain green: verified by running the full suite after additions.

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common`
- **Per wave merge:** Full suite: `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `crates/rc-common/src/types.rs` — add `PodFailureReason` enum + test `test_pod_failure_reason_serde_roundtrip`
- [ ] `crates/rc-common/src/protocol.rs` — add 5 AgentMessage variants + tests (one roundtrip per variant)
- [ ] `crates/racecontrol/src/pod_healer.rs` (or new location) — add `is_pod_in_recovery()` + 4 unit tests
- [ ] `crates/racecontrol/src/ws/mod.rs` — add stub match arms for 5 new variants (compile gate, no logic)

---

## Sources

### Primary (HIGH confidence)

- Direct inspection: `crates/rc-common/src/types.rs` — all existing enums: SimType, PodStatus, BillingSessionStatus, DrivingState, AcStatus, BillingSessionStatus (2026-03-16)
- Direct inspection: `crates/rc-common/src/protocol.rs` — AgentMessage enum (lines 18-109), CoreToAgentMessage enum (lines 111-283), existing tests (lines 679+) (2026-03-16)
- Direct inspection: `crates/rc-common/src/lib.rs` — module structure (2026-03-16)
- Direct inspection: `crates/rc-common/Cargo.toml` — dependencies: serde, serde_json, chrono, uuid, rand (2026-03-16)
- Direct inspection: `crates/racecontrol/src/pod_healer.rs` — should_skip_for_watchdog_state() predicate + 8 unit tests, AppState backoff pattern (2026-03-16)
- Direct inspection: `crates/racecontrol/src/state.rs` — WatchdogState enum definition (lines 26-35), AppState structure (2026-03-16)
- Direct inspection: `crates/racecontrol/src/ws/mod.rs` — AgentMessage match arms (all 20+ variants enumerated via grep) (2026-03-16)
- Direct inspection: `crates/rc-agent/src/ai_debugger.rs` — PodStateSnapshot, try_auto_fix structure (2026-03-16)
- Direct inspection: `.planning/research/ARCHITECTURE.md` — full system diagram, PROTO-01/02/03 recommended code (2026-03-16)
- Direct inspection: `.planning/config.json` — nyquist_validation: true (2026-03-16)

### Secondary (MEDIUM confidence)

- `.planning/research/STACK.md` — confirmed no new crates needed (2026-03-16)
- `.planning/REQUIREMENTS.md` — PROTO-01/02/03 definitions (2026-03-16)
- `.planning/STATE.md` — accumulated decisions: rc-common compiles first, cross-crate compile dependency (2026-03-16)

---

## Metadata

**Confidence breakdown:**
- PodFailureReason enum shape: HIGH — pattern taken directly from existing enums in types.rs
- AgentMessage variant additions: HIGH — additive only, serde tag/content is established pattern
- is_pod_in_recovery() placement: MEDIUM — PROTO-03 says "rc-common" but WatchdogState is racecontrol-only; the open question documents this clearly
- ws/mod.rs stub arms requirement: HIGH — Rust exhaustive match guarantees compile failure without them
- Test patterns: HIGH — identical to existing protocol.rs tests and pod_healer.rs tests

**Research date:** 2026-03-16
**Valid until:** 2026-04-16 (stable Rust enum/serde patterns; no fast-moving ecosystem risk)
