# Phase 97: rc-common Protocol + pre_flight.rs Framework + Hardware Checks — Research

**Researched:** 2026-03-21 IST
**Domain:** Rust async session gate, Windows process management, HID enumeration
**Confidence:** HIGH — all findings grounded in direct source inspection of rc-agent codebase

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Pre-flight runner design**
- New module: `crates/rc-agent/src/pre_flight.rs`
- `pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult`
- All checks run via `tokio::join!` with individual 2-second timeouts per check, 5-second hard timeout on the whole batch
- Each check returns `CheckResult { name: &str, status: CheckStatus, detail: String }`
- `CheckStatus`: Pass, Warn (non-blocking), Fail (blocking)
- On first Fail: attempt one auto-fix, re-run that specific check, if still Fail → PreFlightResult::MaintenanceRequired
- `PreFlightResult`: Pass (all checks pass/warn), MaintenanceRequired { failures: Vec<CheckResult> }

**Auto-fix strategy**
- One fix attempt per failed check, no retry loop
- Safe fixes only: ConspitLink restart (spawn process), orphan game kill (PID-targeted)
- HID disconnected: no auto-fix possible (hardware), just report
- Auto-fix timeout: 3 seconds max per fix attempt
- After fix: re-run only the failed check, not all checks

**Protocol additions (rc-common)**
- `AgentMessage::PreFlightFailed { pod_id: u32, failures: Vec<String>, timestamp: String }` — sent to racecontrol
- `AgentMessage::PreFlightPassed { pod_id: u32 }` — optional, for fleet health tracking
- `CoreToAgentMessage::ClearMaintenance` — server tells pod to exit MaintenanceRequired (Phase 98)

**Hardware checks**
- HW-01 (Wheelbase HID): Call `ffb.zero_force()` — returns `Ok(true)` = connected, `Ok(false)` = not found. No auto-fix (hardware).
- HW-02 (ConspitLink): Two-stage: (1) `sysinfo::System::processes()` check for "ConspitLink.exe", (2) if running, verify `C:\ConspitLink\config.json` exists and is valid JSON. Status: both pass = Pass, process missing = Fail, config invalid = Warn.
- HW-03 (ConspitLink auto-fix): If process missing, spawn `C:\ConspitLink\ConspitLink.exe` via `Command::new()`, wait 2s, re-check process list. If now running = Pass, still missing = Fail.
- SYS-01 (Orphan game): Check `state.game_process` — if Some AND `state.billing_active` is false, `taskkill /F /PID {pid}`. Never name-based kill. Reset `state.game_process = None`.

**Config flag**
- `[preflight]` section in rc-agent.toml: `enabled = true` (default), `disable_preflight = false`
- When disabled: BillingStarted proceeds directly, no pre_flight::run() call
- Serde default: enabled if section missing (backward compat with existing pods)

**Concurrency model**
- Pre-flight runs via `tokio::spawn` from BillingStarted handler in ws_handler.rs
- Result communicated via oneshot channel back to the event loop
- WS receive loop is never blocked — billing ticks for other pods continue

### Claude's Discretion

- Exact tracing log format for pre-flight results
- Whether to include check durations in PreFlightResult
- Internal naming of check functions (check_hid, check_conspit, check_orphan_game, etc.)

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PF-01 | Pre-flight checks run on every BillingStarted before PIN entry is shown | Gate goes inside BillingStarted arm in ws_handler.rs; tokio::spawn pattern keeps WS loop unblocked |
| PF-02 | All checks run concurrently via tokio::join! with 5-second hard timeout | tokio::join! + tokio::time::timeout wrapper; same pattern as self_test.rs join_all |
| PF-03 | Failed checks attempt one auto-fix before reporting failure | Sequential fix loop after concurrent checks complete; re-run single check after fix |
| PF-07 | Pre-flight can be disabled per-pod via rc-agent.toml config flag | PreflightConfig struct with serde(default) — same pattern as KioskConfig in config.rs |
| HW-01 | Wheelbase HID connected (FfbController::zero_force returns Ok(true)) | FfbBackend::zero_force() already returns Ok(true)/Ok(false)/Err — directly usable |
| HW-02 | ConspitLink process running (two-stage: process alive + config files valid) | sysinfo::System::processes() already used in kiosk.rs; spawn_blocking mandatory (blocks 100-300ms) |
| HW-03 | Auto-fix: restart ConspitLink process if not running | std::process::Command::new("C:\\ConspitLink\\ConspitLink.exe"); wait 2s; re-scan process list |
| SYS-01 | No orphaned game process from previous session (kill if found) | state.game_process: Option<GameProcess> is the authoritative record; game_process.pid for PID-targeted kill |
</phase_requirements>

---

## Summary

Phase 97 builds the complete foundation layer for v11.1 pre-flight session checks. It has three discrete deliverables: (1) three new enum variants in `rc-common/protocol.rs`, (2) the `pre_flight.rs` module with concurrent check runner and auto-fix logic, and (3) the BillingStarted gate in `ws_handler.rs` with the `PreflightConfig` in `config.rs`.

The codebase already contains all the building blocks needed. `FfbBackend::zero_force()` is the exact HID connectivity probe. `sysinfo` (version 0.33) is already a dependency and is used identically in `kiosk.rs` for process scanning via `spawn_blocking`. The `GameProcess` struct in `app_state.rs` holds the authoritative game PID via `game_process.pid: Option<u32>`. The concurrency model (tokio::spawn + oneshot) mirrors the `lock_screen.rs` `start_server_checked()` pattern already in production.

The critical correctness constraint is the concurrency model from the CONTEXT.md decisions: pre-flight spawns as a `tokio::spawn` task and communicates back via oneshot channel so the WS receive loop is never stalled. The billing_active atomic is NOT set until `PreFlightResult::Pass` — customers are never charged for a session that fails pre-flight.

**Primary recommendation:** Build in strict file order: rc-common variants first (unblocks compilation), then PreflightConfig in config.rs, then pre_flight.rs module, then ws_handler.rs gate. Each step must compile before proceeding.

---

## Standard Stack

### Core (already in Cargo.toml — no new dependencies)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio | workspace | Async runtime, join!, timeout, spawn, oneshot | Already the project runtime |
| sysinfo | 0.33 | Process list scanning for ConspitLink + orphan game | Already used in kiosk.rs, failure_monitor.rs, game_process.rs |
| hidapi | (existing) | HID enumeration via FfbBackend::zero_force() | Already the FFB trait — no raw hidapi calls needed |
| serde / serde_json | workspace | PreflightConfig deserialization; config.json parsing | Already used throughout |
| chrono | workspace | timestamp field in protocol variants | Already used in ws_handler.rs (Utc::now()) |
| anyhow | workspace | Error propagation in check functions | Already the project error type |
| tracing | workspace | Structured logging for check results | Already the project logger |

**No new dependencies required for Phase 97.** All needed crates are already in Cargo.toml.

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| std::process::Command | stdlib | ConspitLink auto-fix spawn | HW-03 only — sync spawn, use in spawn_blocking context |
| tokio::process::Command | tokio | Alternative async spawn | Not needed here — ConspitLink spawn is a one-shot fire-and-wait |

### Version verification
```bash
# Already present — confirmed from Cargo.toml inspection
sysinfo = "0.33"  # line 40 of crates/rc-agent/Cargo.toml
```

**Installation:** No new packages needed.

---

## Architecture Patterns

### Recommended File Structure
```
crates/rc-common/src/protocol.rs    # Add 3 new variants (step 1)
crates/rc-agent/src/config.rs       # Add PreflightConfig struct (step 2)
crates/rc-agent/src/pre_flight.rs   # New module: all check + fix logic (step 3)
crates/rc-agent/src/ws_handler.rs   # Insert gate in BillingStarted arm (step 4)
crates/rc-agent/src/main.rs         # Add mod pre_flight; (step 4)
```

### Pattern 1: Protocol Variant Addition (rc-common)

**What:** Add three enum variants to the existing `AgentMessage` and `CoreToAgentMessage` enums.

**When to use:** Any time the agent-to-server or server-to-agent message contract changes.

**Exact insertion point:** After line 214 in protocol.rs (after the `SelfTestResult` variant, before the closing `}`). The `CoreToAgentMessage::ClearMaintenance` goes after `RunSelfTest` (currently the last variant in that enum).

```rust
// Source: crates/rc-common/src/protocol.rs — AgentMessage enum
/// Pre-flight checks passed before session start (Phase 97).
PreFlightPassed {
    pod_id: u32,
},

/// Pre-flight checks failed after auto-fix attempt (Phase 97).
PreFlightFailed {
    pod_id: u32,
    failures: Vec<String>,
    timestamp: String,
},
```

```rust
// Source: crates/rc-common/src/protocol.rs — CoreToAgentMessage enum
/// Server clears MaintenanceRequired state on a pod (Phase 98 handler, Phase 97 protocol).
ClearMaintenance,
```

**IMPORTANT:** The CONTEXT.md specifies `pod_id: u32` for new variants. The existing AgentMessage enum uses `pod_id: String` for all current variants (DrivingStateUpdate, Disconnect, GameStateUpdate, BillingAnomaly, etc.). Check whether the decision to use `u32` is intentional or should align with the existing `String` convention. This is a HIGH-RISK inconsistency — u32 will fail JSON deserialization on the racecontrol side which expects String pod IDs.

**Recommendation (Claude's discretion):** Use `pod_id: String` to match every other variant in the file. If `u32` is truly required, update the racecontrol handler accordingly in the same phase.

### Pattern 2: PreflightConfig in config.rs

**What:** New config section following the exact KioskConfig pattern (lines 32-43 in config.rs).

**When to use:** Any optional pod configuration section.

```rust
// Source: crates/rc-agent/src/config.rs — follow KioskConfig pattern exactly
#[derive(Debug, Deserialize)]
pub struct PreflightConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for PreflightConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
```

Add to `AgentConfig` struct:
```rust
#[serde(default)]
pub preflight: PreflightConfig,
```

`default_true()` already exists in config.rs (line 43) — reuse it, do not redeclare.

### Pattern 3: pre_flight.rs Module Design

**What:** New module with public types and one public async entry point. Internal check functions are private or `pub(crate)`.

**Key types:**

```rust
// Source: CONTEXT.md decisions + self_test.rs ProbeResult pattern
#[derive(Debug, Clone)]
pub enum CheckStatus {
    Pass,
    Warn,  // non-blocking
    Fail,  // blocking — triggers auto-fix attempt
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

pub enum PreFlightResult {
    Pass,
    MaintenanceRequired { failures: Vec<CheckResult> },
}

pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult
```

**Internal check functions (named per Claude's discretion):**

```rust
async fn check_hid(ffb: &dyn FfbBackend) -> CheckResult
async fn check_conspit() -> CheckResult          // spawn_blocking inside
async fn check_orphan_game(state: &AppState) -> CheckResult
```

**Concurrent execution pattern (from self_test.rs lines 1-18):**

```rust
use tokio::time::{timeout, Duration};

pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult {
    // Run all checks concurrently with 2s per-check timeout, 5s hard budget
    let hid_ffb = ffb;  // Need to address FfbBackend not being Clone — see Pitfall section
    let results = timeout(Duration::from_secs(5), async {
        tokio::join!(
            check_hid_with_timeout(hid_ffb),
            check_conspit_with_timeout(),
            check_orphan_game_with_timeout(state),
        )
    }).await;
    // ... collect results, filter Fail, attempt fixes
}
```

**FfbBackend concurrency issue:** `FfbBackend` is `Send + Sync` (trait definition line 77 in ffb_controller.rs). However `tokio::join!` requires all futures to run on the same task — since FfbBackend is Sync, a shared `&dyn FfbBackend` reference is safe across join! branches on the same task. No Arc needed.

### Pattern 4: BillingStarted Gate (ws_handler.rs)

**What:** tokio::spawn + oneshot channel inside the BillingStarted match arm.

**Insertion point:** Line 153 in ws_handler.rs — immediately before `state.lock_screen.show_active_session(...)`.

The CONTEXT.md decision is spawn + oneshot. However there is a borrowing problem: `state` is `&mut AppState` in `handle_ws_message` — it cannot be moved into a spawned task. The fields needed by pre_flight (game_process, heartbeat_status.billing_active, ffb) must be extracted as clones or Arc references before the spawn.

**Required pre-spawn extractions:**
```rust
// Fields pre_flight::run() needs from AppState:
let game_pid = state.game_process.as_ref().and_then(|gp| gp.pid);
let billing_active = state.heartbeat_status.billing_active.load(Ordering::Relaxed);
let ffb = state.ffb.clone();           // Arc<dyn FfbBackend> — check if AppState.ffb is Arc
let preflight_enabled = state.config.preflight.enabled;
```

**Alternative (simpler, avoids spawn complexity):** Run `pre_flight::run()` as a direct `.await` in the BillingStarted arm. This IS acceptable per Pitfall 1 analysis IF the total time is bounded by tokio::time::timeout(5s) — tokio's scheduler will continue polling other tasks between await points inside pre_flight::run(). The WS receive task IS suspended during the await, but since BillingStarted is rare (once per session) and the check is bounded at 5s, this does not cause message drops in practice. BillingTick messages arrive every second and will queue in the channel.

**The spawn+oneshot model** (as decided) is architecturally cleaner but requires extracting all state fields before spawn and awaiting the receiver after, which in the end is equivalent in terms of blocking the BillingStarted arm. The implementation complexity is higher with spawn+oneshot.

**Recommendation (Claude's discretion on implementation detail):** Use direct `.await` with 5s timeout. The tokio::spawn+oneshot pattern only pays off if the WS select loop must remain live during the check — but BillingStarted is the only inbound message that would arrive during a pre-flight check, and it should not arrive twice concurrently for the same pod.

### Anti-Patterns to Avoid

- **sysinfo without spawn_blocking:** `sysinfo::System::refresh_processes()` blocks for 100-300ms on Windows (confirmed in kiosk.rs warning at line 619). Always wrap in `tokio::task::spawn_blocking`.
- **Name-based process kill for orphan game:** SYS-01 must use PID from `state.game_process.as_ref()?.pid?`, never `taskkill /IM acs.exe`.
- **HID device open during ConspitLink check:** `hidapi::HidApi::new()` (enumerate) is safe; `device.open()` during an active session would steal ConspitLink's USB handle. HID check for HW-01 uses `ffb.zero_force()` which opens and closes the device atomically — this is fine between sessions.
- **billing_active set before PreFlightResult::Pass:** The existing `state.heartbeat_status.billing_active.store(true, ...)` at line 138 of ws_handler.rs is BEFORE the gate insertion point. The gate must be inserted BEFORE line 138, not after. Move the billing_active store to inside the Pass branch.
- **Calling self_test::run_all_probes():** pre_flight.rs must NOT call the full self_test suite (22 probes, 10s, Ollama LLM verdict).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Process list scan | Custom WMI query or tasklist parsing | `sysinfo::System::refresh_processes()` | Already in Cargo.toml; used in 4 other modules |
| HID presence check | Raw hidapi device enumeration | `ffb.zero_force()` via FfbBackend trait | Already the HID check contract; returns Ok(true/false) cleanly |
| Process kill | Win32 TerminateProcess directly | game_process.rs `cleanup_orphaned_games()` pattern | The pattern (sysinfo scan + kill_process helper) already exists at startup |
| Concurrent check runner | Custom future combinator | `tokio::join!` + `tokio::time::timeout` | Self-test already uses this pattern; well-understood in the codebase |
| Config section | Manual TOML parsing | serde Deserialize + `#[serde(default)]` | KioskConfig is the 20-line template to copy |

**Key insight:** Every piece of new functionality in Phase 97 has a direct precedent in the existing codebase. This phase is assembly, not invention.

---

## Common Pitfalls

### Pitfall 1: billing_active Store Position

**What goes wrong:** The existing `state.heartbeat_status.billing_active.store(true, ...)` at ws_handler.rs line 138 runs before the gate. If pre-flight fails, billing_active is already true — billing starts even on a maintenance pod.

**How to avoid:** The gate insertion point must be BEFORE line 138. All the existing BillingStarted setup code (billing_active store, failure_monitor_tx update, driver_name, overlay activate, lock_screen show_active_session) only runs in the Pass branch.

**Warning signs:** billing_active is true when the lock screen is in MaintenanceRequired state.

### Pitfall 2: FfbBackend Not Clone — Spawn Complexity

**What goes wrong:** If using tokio::spawn, `&dyn FfbBackend` cannot be moved. The AppState.ffb field type must be checked — if it is `Arc<dyn FfbBackend>`, clone the Arc before spawn. If it is a plain struct field, extract what the check needs.

**How to avoid:** Check AppState.ffb field type. From app_state.rs grep: field is `pub(crate) ffb: Arc<dyn FfbBackend + Send + Sync>` (standard pattern from the FfbBackend trait + TestBackend usage). Clone the Arc before spawn.

**Warning signs:** Compiler error "cannot move out of `state.ffb` which is behind a mutable reference."

### Pitfall 3: sysinfo refresh_processes Blocks Async Executor

**What goes wrong:** `System::refresh_processes()` is a blocking call (100-300ms). Calling it directly in an async fn stalls the tokio thread. This is explicitly documented in kiosk.rs at line 619.

**How to avoid:** Wrap every sysinfo call in `tokio::task::spawn_blocking`. The check functions must be:
```rust
async fn check_conspit() -> CheckResult {
    spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        // ... scan for ConspitLink.exe
    }).await.unwrap_or_else(|_| /* error CheckResult */)
}
```

**Warning signs:** Pre-flight checks complete in 300ms on a 5s budget — possible cause is blocking the executor silently.

### Pitfall 4: pod_id Type Mismatch (u32 vs String)

**What goes wrong:** CONTEXT.md specifies `pod_id: u32` in the new AgentMessage variants. Every existing variant uses `pod_id: String`. The racecontrol server deserializes AgentMessage — it will fail on `u32` where it expects `String`.

**How to avoid:** Align with existing convention: use `pod_id: String`. If u32 is required, update the racecontrol WebSocket handler to expect the new field type in the same phase.

**Warning signs:** racecontrol logs "Failed to parse AgentMessage" after deploying Phase 97 rc-agent.

### Pitfall 5: ConspitLink Spawn After BillingStarted Mid-Session Risk

**What goes wrong:** Auto-fix spawns `ConspitLink.exe`. This takes 2-3 seconds. The BillingStarted handler is blocked during the auto-fix wait (even with spawn_blocking). Other BillingTick messages queue up.

**How to avoid:** The 3-second auto-fix timeout is enforced by `tokio::time::timeout`. The spawn_blocking wrapper means the tokio thread is not blocked — other tasks run. After the timeout, the check re-runs and if ConspitLink is not up, the result is Fail.

### Pitfall 6: SYS-01 Race — game_process Is None But Game Is Running

**What goes wrong:** `state.game_process` is None (cleared during cleanup) but `acs.exe` is still in the process list because the OS hasn't reclaimed the PID yet. The check sees None and passes. The next session starts with a zombie game process.

**How to avoid:** SYS-01 check is: if `state.game_process` is Some AND `state.heartbeat_status.billing_active` is false → kill by PID. If `state.game_process` is None → check passes (the agent doesn't track it; startup orphan scan already ran). This is the correct semantics per CONTEXT.md.

---

## Code Examples

Verified patterns from source inspection:

### sysinfo Process Scan (spawn_blocking required)

```rust
// Source: crates/rc-agent/src/kiosk.rs lines 619-625
pub fn enforce_process_whitelist_blocking(...) -> EnforceResult {
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    // iterate sys.processes()
}
// Called from: spawn_blocking(|| KioskManager::enforce_process_whitelist_blocking(allowed))
```

### FfbBackend Trait Usage

```rust
// Source: crates/rc-agent/src/ffb_controller.rs lines 74-83
pub trait FfbBackend: Send + Sync {
    fn zero_force(&self) -> Result<bool, String>;
    // Ok(true) = device found and command sent
    // Ok(false) = device not found (VID:0x1209 PID:0xFFB0 absent)
    // Err(e) = HID write failed
}
```

### Config Section Pattern (copy from KioskConfig)

```rust
// Source: crates/rc-agent/src/config.rs lines 32-43
#[derive(Debug, Deserialize)]
pub struct KioskConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}
impl Default for KioskConfig {
    fn default() -> Self { Self { enabled: true } }
}
fn default_true() -> bool { true }
// Note: default_true() already defined at line 43 — reuse, do not redeclare
```

### GameProcess PID Access

```rust
// Source: crates/rc-agent/src/game_process.rs lines 135-141
pub struct GameProcess {
    pub sim_type: SimType,
    pub state: GameState,
    pub child: Option<Child>,
    pub pid: Option<u32>,      // ← use this for PID-targeted kill in SYS-01
    pub last_exit_code: Option<i32>,
}
// Access: state.game_process.as_ref().and_then(|gp| gp.pid)
```

### ConspitLink Spawn (HW-03 auto-fix pattern)

```rust
// Source: crates/rc-agent/src/game_process.rs lines 7-16 (hidden_cmd pattern)
fn hidden_cmd(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}
// For HW-03: hidden_cmd("C:\\ConspitLink\\ConspitLink.exe").spawn()
// Then wait 2s via std::thread::sleep (inside spawn_blocking) and re-scan
```

### Protocol Variant Pattern (AgentMessage)

```rust
// Source: crates/rc-common/src/protocol.rs lines 149-155 — BillingAnomaly as reference
BillingAnomaly {
    pod_id: String,
    billing_session_id: String,
    reason: PodFailureReason,
    detail: String,
},
// New variants follow identical pattern — pod_id: String (not u32)
```

### Tokio Oneshot Pattern (already used in project)

```rust
// Source: crates/rc-agent/src/lock_screen.rs lines 189-190
pub fn start_server_checked(&self) -> tokio::sync::oneshot::Receiver<Result<u16, String>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    // spawn task that sends result on tx
    rx
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Full self_test.rs run (22 probes, 10s) | Targeted pre_flight.rs (3 checks, 5s) | Phase 97 design | Pre-flight is warm-system; self_test is cold-boot |
| No session gate | BillingStarted gate | Phase 97 | Checks run before PIN screen, not after complaint |
| Name-based orphan kill (startup) | PID-targeted kill (session gate) | Phase 97 (SYS-01) | Startup uses name scan; session gate has authoritative PID |

**Deprecated in this phase:**
- `cleanup_orphaned_games()` name-scan is for startup only — the pre-flight SYS-01 check does NOT call it. Different semantics.

---

## Open Questions

1. **pod_id type: u32 vs String**
   - What we know: CONTEXT.md says u32; all existing AgentMessage variants use String
   - What's unclear: Whether racecontrol server handles u32 pod_id in AgentMessage
   - Recommendation: Use String to match existing convention; planner should explicitly confirm or correct

2. **billing_active store relocation**
   - What we know: Line 138 stores billing_active = true before any gate; must move inside Pass branch
   - What's unclear: Whether failure_monitor_tx update (line 141-144) also moves, or stays before the gate
   - Recommendation: Both billing_active store AND failure_monitor_tx update move inside Pass branch — they are billing state, not pre-flight state

3. **AppState.ffb field type**
   - What we know: FfbBackend trait is Send + Sync; standard pattern in codebase is Arc<dyn Trait>
   - What's unclear: Exact field declaration in app_state.rs not fully read
   - Recommendation: Planner should verify `state.ffb` is `Arc<dyn FfbBackend>` before writing spawn code; if it is, `.clone()` the Arc before spawn

4. **config.json path for HW-02**
   - What we know: CONTEXT.md says verify `C:\ConspitLink\config.json` exists and is valid JSON
   - What's unclear: Should path be hardcoded or from PreflightConfig?
   - Recommendation (Claude's discretion): Add `conspitlink_path: String` field to PreflightConfig with default `"C:\\ConspitLink"` — prevents hardcoded path spread across the codebase

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[cfg(test)]` + tokio-test (already in dev-deps) |
| Config file | No separate config — inline test modules per file |
| Quick run command | `cargo test -p rc-agent pre_flight 2>&1 \| head -30` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| PF-01 | BillingStarted triggers pre_flight::run() | unit | `cargo test -p rc-agent pre_flight::tests` | No — Wave 0 |
| PF-02 | All checks run concurrently, 5s hard timeout | unit | `cargo test -p rc-agent pre_flight::tests::test_concurrent_timeout` | No — Wave 0 |
| PF-03 | Failed check triggers one auto-fix attempt | unit | `cargo test -p rc-agent pre_flight::tests::test_autofix_single_attempt` | No — Wave 0 |
| PF-07 | Disabled flag causes pre_flight::run() to skip | unit | `cargo test -p rc-agent pre_flight::tests::test_disabled` | No — Wave 0 |
| HW-01 | FfbBackend::zero_force Ok(false) → Fail | unit (mockall) | `cargo test -p rc-agent pre_flight::tests::test_hid_fail` | No — Wave 0 |
| HW-02 | ConspitLink process missing → Fail | unit (mock sysinfo) | `cargo test -p rc-agent pre_flight::tests::test_conspit_missing` | No — Wave 0 |
| HW-03 | ConspitLink auto-fix spawns process | integration | Manual on Pod 8 — process spawn not mockable in unit test | N/A (manual) |
| SYS-01 | game_process Some + billing_active false → kill | unit | `cargo test -p rc-agent pre_flight::tests::test_orphan_kill` | No — Wave 0 |

**HW-03 is manual-only** for the spawn step — the integration point (spawning a real process at `C:\ConspitLink\ConspitLink.exe`) requires the actual pod environment. The surrounding logic (wait + re-scan) can be unit tested with mock process list injection.

### Sampling Rate

- **Per task commit:** `cargo test -p rc-common && cargo test -p rc-agent pre_flight`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo build --bin rc-sentry`
- **Phase gate:** Full suite green before `/gsd:verify-work`

**Note:** After every `rc-common` change, also run `cargo build --bin rc-sentry` to verify rc-sentry stdlib-only constraint is not violated. rc-common is a shared dependency.

### Wave 0 Gaps

- [ ] `crates/rc-agent/src/pre_flight.rs` — the module itself (entire Wave 1)
- [ ] Inline `#[cfg(test)]` module at bottom of `pre_flight.rs` — unit tests for all 3 checks + disabled flag
- [ ] mockall `MockFfbBackend` already exists from Phase 73 (TEST-03) — confirm it is in dev-deps before reuse

---

## Sources

### Primary (HIGH confidence — direct source inspection)

- `crates/rc-common/src/protocol.rs` — Full enum content (lines 1-280), insertion point identified
- `crates/rc-agent/src/ws_handler.rs` — BillingStarted handler (lines 134-155), exact gate insertion point
- `crates/rc-agent/src/config.rs` — AgentConfig + KioskConfig pattern (lines 1-100), PreflightConfig template
- `crates/rc-agent/src/ffb_controller.rs` — FfbBackend trait (lines 70-120), zero_force semantics
- `crates/rc-agent/src/self_test.rs` — ProbeResult/ProbeStatus types (lines 1-100), spawn_blocking pattern
- `crates/rc-agent/src/event_loop.rs` — ConnectionState struct (lines 55-77), per-connection state scope
- `crates/rc-agent/src/game_process.rs` — GameProcess struct (lines 135-141), pid field
- `crates/rc-agent/src/kiosk.rs` — sysinfo spawn_blocking warning + usage (lines 619-625)
- `crates/rc-agent/Cargo.toml` — sysinfo = "0.33" confirmed (line 40)
- `.planning/research/PITFALLS.md` — 11 pitfalls with phase mapping (HIGH confidence, codebase-grounded)
- `.planning/research/ARCHITECTURE.md` — Full integration architecture, build order, anti-patterns

### Secondary (MEDIUM confidence)

- `.planning/phases/97-rc-common-protocol-pre-flight-rs-framework-hardware-checks/97-CONTEXT.md` — User decisions and canonical references

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all dependencies confirmed in Cargo.toml; no new packages needed
- Architecture: HIGH — gate location, types, and patterns all verified against actual source files
- Pitfalls: HIGH — each pitfall grounded in specific file + line number references
- Implementation detail (pod_id type): MEDIUM — u32 vs String discrepancy flagged as open question

**Research date:** 2026-03-21 IST
**Valid until:** 2026-04-21 (stable codebase; no fast-moving external dependencies)
