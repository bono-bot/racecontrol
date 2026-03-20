# Phase 73: Critical Business Tests - Research

**Researched:** 2026-03-20
**Domain:** Rust unit testing — tokio async, watch channel injection, trait seam abstraction
**Confidence:** HIGH

## Summary

Phase 73 adds unit test coverage to three safety-critical modules before Phase 74 decomposes rc-agent. The "Refactor Second" standing rule is non-negotiable: tests must be green before any structural changes.

The good news is that most of the work is already done in spirit. `billing_guard.rs` and `failure_monitor.rs` already have `#[cfg(test)]` blocks with condition-level tests. What Phase 73 must add is **time-based state machine tests** — specifically, that the debounce timers fire anomaly messages through the actual watch channel after the threshold elapses. The existing tests only assert on boolean conditions, not on the timer logic or the channel send.

For `ffb_controller.rs`, no trait seam exists yet. The module calls `hidapi::HidApi::new()` directly inside `open_vendor_interface()`, which is a private method. The trait seam must be introduced by extracting the HID write path behind a `FfbBackend` trait, then injecting a `MockFfbBackend` in tests. This is the only module requiring new production-code changes to enable testing.

**Primary recommendation:** Add `mockall = "0.13"` to dev-dependencies, introduce `FfbBackend` trait in `ffb_controller.rs`, then write `#[tokio::test]` tests for `billing_guard` and `#[test]` condition tests for `failure_monitor` state machine gaps.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| TEST-01 | billing_guard unit tests cover stuck session detection (BILL-02) and idle drift (BILL-03) | billing_guard.rs has existing condition tests but lacks timer-elapsed + channel-send verification; needs tokio::time::pause() + watch injection |
| TEST-02 | failure_monitor unit tests cover game freeze (CRASH-01) and launch timeout (CRASH-02) | failure_monitor.rs has condition tests for CRASH-02 and threshold logic for CRASH-01; CRASH-01 is blocked by is_game_process_hung() (Windows HWnd call) — must test the condition logic path separately |
| TEST-03 | ffb_controller tests via FfbBackend trait seam (no real HID access in tests) | FfbBackend trait does not exist yet; open_vendor_interface() calls hidapi directly; trait must be extracted and injected before any FFB test can run |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| tokio (full) | 1.x (workspace) | Async runtime for `#[tokio::test]` | Already in workspace; billing_guard uses tokio::sync::watch |
| mockall | 0.13 | Auto-generate mock structs from trait definitions | Mentioned explicitly in STATE.md decisions; MSRV 1.77 — project at 1.93.1 |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio::time::pause() | built-in | Freeze virtual time in tests | Use when testing timer-elapsed logic WITHOUT real sleep; pairs with advance() |
| tokio::sync::watch::channel | built-in | Inject state into billing_guard::spawn() | The production spawn() already accepts watch::Receiver — inject directly in test |
| tokio::sync::mpsc::channel | built-in | Capture AgentMessage output from billing_guard | Collect sent messages to assert anomaly was fired |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| mockall | manual mock struct | manual is simpler for 1-2 methods but loses auto-generation; mockall adds clean `expect_*` API |
| tokio::time::pause() | real sleep with short timeouts | real sleep makes tests flaky and slow; pause()/advance() is deterministic |
| FfbBackend trait | #[cfg(test)] stub module | trait is cleaner seam, avoids dead code in production; STATE.md decision already locked this |

**Installation:**
```bash
# Add to crates/rc-agent/Cargo.toml [dev-dependencies]
mockall = "0.13"
```

**Version verification:** mockall 0.13.x is current as of early 2026. The `0.13` semver range covers all 0.13.x patch releases. Project Rust is 1.93.1; mockall 0.13 MSRV is 1.77 — compatible.

## Architecture Patterns

### Recommended Project Structure
```
crates/rc-agent/src/
├── billing_guard.rs     # Add timer tests using tokio::time::pause() + watch injection
├── failure_monitor.rs   # Add state machine condition tests (no async needed for most)
└── ffb_controller.rs    # Extract FfbBackend trait, inject MockFfbBackend in test module
```

All tests live in `#[cfg(test)] mod tests { ... }` within the same file. No separate test files.

### Pattern 1: Watch Channel Injection for billing_guard

**What:** `billing_guard::spawn()` already accepts `watch::Receiver<FailureMonitorState>`. In tests, create the channel pair, push state via the `Sender`, advance time, then assert on `mpsc::Receiver<AgentMessage>`.

**When to use:** Testing timer-based detection (BILL-02 at 60s, BILL-03 at 300s) where the actual channel message matters, not just the boolean condition.

**Example:**
```rust
// Source: tokio::time docs — https://docs.rs/tokio/latest/tokio/time/fn.pause.html
#[tokio::test]
async fn bill02_stuck_session_fires_after_60s() {
    tokio::time::pause(); // freeze virtual clock

    let (state_tx, state_rx) = tokio::sync::watch::channel(FailureMonitorState {
        billing_active: true,
        game_pid: None,
        ..FailureMonitorState::default()
    });
    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel(16);

    billing_guard::spawn(
        state_rx,
        msg_tx,
        "pod_test".to_string(),
        "http://unused".to_string(),
        300,
    );

    // Advance past the 5s poll interval + 60s threshold
    tokio::time::advance(Duration::from_secs(70)).await;
    tokio::task::yield_now().await; // let the spawned task process

    let msg = msg_rx.try_recv().expect("BillingAnomaly must be sent");
    assert!(matches!(msg, AgentMessage::BillingAnomaly { reason: PodFailureReason::SessionStuckWaitingForGame, .. }));
}
```

**Key detail:** `tokio::time::pause()` only works when the tokio runtime uses the test scheduler. `#[tokio::test]` provides this by default. `billing_guard::spawn()` uses `tokio::time::interval()` which respects the paused clock.

### Pattern 2: State Machine Condition Tests (no async)

**What:** For conditions that don't require timing — just asserting the boolean logic is correct given a state snapshot — use plain `#[test]` with `make_state()` helper already present in both files.

**When to use:** CRASH-01 UDP silence threshold logic, CRASH-02 launch timeout elapsed check, recovery suppression logic.

**Example:**
```rust
#[test]
fn crash01_udp_silence_triggers_freeze_check() {
    let state = make_state(|s| {
        s.game_pid = Some(1234);
        s.last_udp_secs_ago = Some(35); // > 30s FREEZE_UDP_SILENCE_SECS
        s.recovery_in_progress = false;
    });
    let udp_silent = state.last_udp_secs_ago
        .map(|s| s >= FREEZE_UDP_SILENCE_SECS)
        .unwrap_or(false);
    assert!(state.game_pid.is_some() && udp_silent);
}
```

### Pattern 3: FfbBackend Trait Seam

**What:** Extract the HID write surface into a trait. The production `FfbController` implements it by calling hidapi. The test mock implements it by recording calls.

**When to use:** Whenever a unit must be tested without calling real hardware APIs.

**Trait definition pattern:**
```rust
// In ffb_controller.rs

pub trait FfbBackend: Send + Sync {
    fn zero_force(&self) -> Result<bool, String>;
    fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
    fn set_gain(&self, percent: u8) -> Result<bool, String>;
    fn fxm_reset(&self) -> Result<bool, String>;
    fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
}

impl FfbBackend for FfbController { ... } // delegates to existing hidapi methods

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub FfbBackendMock {}
        impl FfbBackend for FfbBackendMock {
            fn zero_force(&self) -> Result<bool, String>;
            fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
            fn set_gain(&self, percent: u8) -> Result<bool, String>;
            fn fxm_reset(&self) -> Result<bool, String>;
            fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
        }
    }

    #[test]
    fn zero_force_called_on_session_end() {
        let mut mock = MockFfbBackendMock::new();
        mock.expect_zero_force()
            .returning(|| Ok(true))
            .times(1);
        let result = mock.zero_force();
        assert_eq!(result, Ok(true));
    }
}
```

**Scope decision:** The trait seam is for the high-level FFB commands (zero_force, set_gain, etc.) — NOT the low-level HID packet construction. The internal `send_vendor_cmd` and `open_vendor_interface` do NOT need to be on the trait. This keeps the seam minimal and production code unchanged except for the trait addition.

### Anti-Patterns to Avoid

- **Don't `tokio::time::sleep()` in tests:** Use `tokio::time::pause()` + `advance()`. Real sleep is non-deterministic and slow.
- **Don't test `is_game_process_hung()` directly:** It calls Windows HWnd APIs — it's `#[cfg(windows)]` and requires a real process. The existing `#[cfg(not(windows))]` stub returns `false`, which is the correct test behavior. CRASH-01 tests verify the upstream condition (UDP silence + game_pid present) that would trigger the hung check.
- **Don't put `mockall` in `[dependencies]`:** It MUST go in `[dev-dependencies]` only. mockall is a test-only dependency.
- **Don't call `billing_guard::spawn()` with a real `core_base_url`:** The orphan auto-end spawns a reqwest call. Use a garbage URL like `"http://unused"` and set `orphan_end_threshold_secs` above the test window so it never fires during timer tests.
- **Don't wrap `attempt_orphan_end` in a trait:** STATE.md decisions say "callback param (option b) is simpler, avoid trait boilerplate" for the orphan HTTP path. Leave `attempt_orphan_end` as-is; test the main anomaly path only.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Mock trait implementations | Manual structs with tracked call counts | mockall 0.13 | Manual mocks miss edge cases; mockall's `expect_*` API enforces call counts and captures args cleanly |
| Time advancement in async tests | Thread::sleep / short real timeouts | tokio::time::pause() + advance() | Real sleep causes flaky CI; pause() is deterministic |
| Watch channel clones for state injection | Custom shared-state struct | tokio::sync::watch::channel directly | Already what production code uses — no impedance mismatch |

**Key insight:** The existing `make_state()` helper pattern in both modules is exactly right. Don't replace it — build timer tests on top of it.

## Common Pitfalls

### Pitfall 1: tokio::time::pause() Scope
**What goes wrong:** `pause()` affects the entire tokio runtime in a test. If two tests run concurrently and one calls `pause()`, the other may behave incorrectly.
**Why it happens:** `#[tokio::test]` by default runs each test in its own runtime, but if tests share a runtime (e.g. via `tokio::test(flavor = "multi_thread")`), pause can interfere.
**How to avoid:** Use the default single-threaded `#[tokio::test]` (no flavor annotation). Each test gets its own isolated runtime and its own clock.
**Warning signs:** Tests pass in isolation but fail when run together.

### Pitfall 2: Task Not Yielding After Time Advance
**What goes wrong:** `tokio::time::advance()` is called but the spawned task hasn't had a chance to process the tick yet, so `msg_rx.try_recv()` returns `Empty`.
**Why it happens:** `advance()` moves the clock but doesn't yield to other tasks. The spawned billing_guard task only runs when the test task yields.
**How to avoid:** After `advance()`, call `tokio::task::yield_now().await` one or more times to let the spawned task process its interval tick.
**Warning signs:** `try_recv()` returns Err(Empty) even though the clock has advanced past the threshold.

### Pitfall 3: billing_guard Orphan Spawn During Timer Tests
**What goes wrong:** If `orphan_end_threshold_secs` is ≤ the test's advance amount, a reqwest HTTP call fires to `"http://unused"`, which may log spurious warnings or cause the test to hang.
**Why it happens:** SESSION-01 orphan auto-end is in the same `game_pid.is_none()` branch as BILL-02.
**How to avoid:** Always set `orphan_end_threshold_secs` to a value larger than the time advance in BILL-02 tests. Use `orphan_end_threshold_secs: 9999` as a safe sentinel.
**Warning signs:** Test logs show "[billing-guard] ORPHAN auto-end" during a BILL-02 test.

### Pitfall 4: FfbBackend Trait Not Send + Sync
**What goes wrong:** `FfbController` is `Clone` but the trait object `Box<dyn FfbBackend>` requires `Send + Sync` if it will be passed to tokio tasks.
**Why it happens:** Current `FfbController` is used inside `spawn_blocking` calls — if callers hold a `Box<dyn FfbBackend>`, it needs the marker traits.
**How to avoid:** Declare the trait as `pub trait FfbBackend: Send + Sync { ... }` from the start. mockall's generated mock implements `Send` automatically.
**Warning signs:** Compiler error: "the trait `Send` is not implemented for `dyn FfbBackend`".

### Pitfall 5: CRASH-01 Tests Trying to Call is_game_process_hung()
**What goes wrong:** A test attempts to verify the full CRASH-01 path including `is_game_process_hung()`, which tries to call `EnumWindows` / `IsHungAppWindow` from WinAPI. No real game window exists in test context.
**Why it happens:** The function is gated `#[cfg(windows)]` but still accessible in tests on Windows.
**How to avoid:** Test only the condition that leads TO the hung check (UDP silence + game_pid present). The hung check itself is Windows-only infrastructure tested manually on hardware. Document this scope boundary explicitly in the test comments.

### Pitfall 6: failure_monitor::spawn() Tight Dependencies
**What goes wrong:** `failure_monitor::spawn()` requires `Arc<HeartbeatStatus>` which pulls in `udp_heartbeat.rs` atomics. Test setup becomes complex.
**Why it happens:** The spawn signature is: `status: Arc<HeartbeatStatus>, state_rx, agent_msg_tx, pod_id, pod_number`.
**How to avoid:** Do NOT test `failure_monitor::spawn()` as a black box. Test the condition logic directly using the `make_state()` helper (already exists). The spawn function itself is adequately covered by the existing pattern — what's missing is the CRASH-02 elapsed condition test and the CRASH-01 UDP threshold test.

## Code Examples

### billing_guard — Timer Test with Watch Injection
```rust
// Pattern: inject watch, advance time, assert mpsc message
// Source: tokio time docs + billing_guard.rs spawn() signature

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use rc_common::types::PodFailureReason;
    use rc_common::protocol::AgentMessage;

    fn stuck_state() -> FailureMonitorState {
        FailureMonitorState {
            billing_active: true,
            game_pid: None,
            ..FailureMonitorState::default()
        }
    }

    #[tokio::test]
    async fn bill02_anomaly_fires_after_60s() {
        tokio::time::pause();
        let (state_tx, state_rx) = tokio::sync::watch::channel(stuck_state());
        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel(16);

        spawn(state_rx, msg_tx, "pod_test".into(), "http://unused".into(), 9999);

        // Advance past first poll (5s) + stuck threshold (60s)
        tokio::time::advance(Duration::from_secs(70)).await;
        tokio::task::yield_now().await;

        match msg_rx.try_recv() {
            Ok(AgentMessage::BillingAnomaly { reason, .. }) => {
                assert_eq!(reason, PodFailureReason::SessionStuckWaitingForGame);
            }
            other => panic!("Expected BillingAnomaly, got: {:?}", other),
        }
        drop(state_tx); // silence unused warning
    }

    #[tokio::test]
    async fn bill02_does_not_fire_when_recovery_in_progress() {
        tokio::time::pause();
        let mut s = stuck_state();
        s.recovery_in_progress = true;
        let (_state_tx, state_rx) = tokio::sync::watch::channel(s);
        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel(16);

        spawn(state_rx, msg_tx, "pod_test".into(), "http://unused".into(), 9999);

        tokio::time::advance(Duration::from_secs(70)).await;
        tokio::task::yield_now().await;

        assert!(msg_rx.try_recv().is_err(), "No anomaly when recovery_in_progress");
    }
}
```

### failure_monitor — CRASH-02 Condition Test (no async needed)
```rust
// Pattern: existing make_state() + direct condition assertion
// CRASH-02 condition: launch_started_at.elapsed() > 90s && game_pid.is_none()

#[test]
fn crash02_launch_timeout_fires_after_90s() {
    use std::time::Instant;
    let state = make_state(|s| {
        s.launch_started_at = Some(Instant::now() - Duration::from_secs(100));
        s.game_pid = None;
        s.recovery_in_progress = false;
    });
    let launched_at = state.launch_started_at.unwrap();
    let should_fire = launched_at.elapsed() > Duration::from_secs(LAUNCH_TIMEOUT_SECS)
        && state.game_pid.is_none();
    assert!(should_fire, "CRASH-02 must trigger after 90s with no game PID");
}
```

Note: Tests for CRASH-02 in this exact pattern already exist in the current `failure_monitor.rs` test module (lines 352-376). What is MISSING is verification that these tests actually cover the REQUIREMENTS file's CRASH-01 and CRASH-02 IDs explicitly — the planner should add tests that are named after the requirement IDs for traceability.

### ffb_controller — FfbBackend Trait + Mock
```rust
// Source: mockall docs — https://docs.rs/mockall/0.13/mockall/

use mockall::mock;

pub trait FfbBackend: Send + Sync {
    fn zero_force(&self) -> Result<bool, String>;
    fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
    fn set_gain(&self, percent: u8) -> Result<bool, String>;
    fn fxm_reset(&self) -> Result<bool, String>;
    fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
}

// Production implementation (impl FfbBackend for FfbController)
// delegates to the existing methods — zero new logic.

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        TestBackend {}
        impl FfbBackend for TestBackend {
            fn zero_force(&self) -> Result<bool, String>;
            fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
            fn set_gain(&self, percent: u8) -> Result<bool, String>;
            fn fxm_reset(&self) -> Result<bool, String>;
            fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
        }
    }

    #[test]
    fn zero_force_returns_true_on_success() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force().returning(|| Ok(true));
        assert_eq!(mock.zero_force(), Ok(true));
    }

    #[test]
    fn zero_force_returns_false_when_device_absent() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force().returning(|| Ok(false));
        assert_eq!(mock.zero_force(), Ok(false));
    }

    #[test]
    fn zero_force_with_retry_succeeds_on_first_attempt() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force_with_retry()
            .withf(|attempts, delay_ms| *attempts == 3 && *delay_ms == 100)
            .returning(|_, _| true);
        assert!(mock.zero_force_with_retry(3, 100));
    }

    #[test]
    fn set_gain_clamps_and_sends() {
        let mut mock = MockTestBackend::new();
        mock.expect_set_gain()
            .withf(|p| *p <= 100)
            .returning(|_| Ok(true));
        assert_eq!(mock.set_gain(80), Ok(true));
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `#[cfg(test)]` stub modules | `mockall` trait mocks | mockall 0.11+ | Auto-generated mocks reduce boilerplate; expect_* API is self-documenting |
| Real sleep in async tests | `tokio::time::pause()` + `advance()` | tokio 0.2.7+ | Deterministic, instant test execution |
| Condition-only tests | Timer + channel tests | This phase | Tests now verify the actual anomaly message is sent, not just that the condition is true |

**What already exists (do not duplicate):**
- `billing_guard.rs` lines 183-278: 10 `#[test]` tests — condition logic only, no timing
- `failure_monitor.rs` lines 316-523: 13 `#[test]` tests — condition logic, threshold checks, already covers CRASH-02 elapsed and CRASH-01 UDP silence threshold conceptually

**What is MISSING and must be added:**
- billing_guard: `#[tokio::test]` tests that verify `AgentMessage::BillingAnomaly` is actually sent through the mpsc channel after time advances past threshold
- billing_guard: Test that recovery_in_progress resets the timer (not just suppresses — verifies no delayed fire after recovery clears)
- failure_monitor: Explicit CRASH-01 and CRASH-02 named tests for requirement traceability
- ffb_controller: Entire `FfbBackend` trait + `#[cfg(test)]` mock tests (nothing exists)

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in + tokio::test (tokio 1.x, workspace dep) |
| Config file | none — inline `#[cfg(test)]` modules |
| Quick run command | `cargo test -p rc-agent-crate billing_guard` |
| Full suite command | `cargo test -p rc-agent-crate` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TEST-01 | billing_guard sends BillingAnomaly after 60s (BILL-02) | unit async | `cargo test -p rc-agent-crate billing_guard::tests::bill02` | ❌ Wave 0 |
| TEST-01 | billing_guard sends BillingAnomaly after 300s (BILL-03) | unit async | `cargo test -p rc-agent-crate billing_guard::tests::bill03` | ❌ Wave 0 |
| TEST-01 | billing_guard suppresses when recovery_in_progress | unit async | `cargo test -p rc-agent-crate billing_guard::tests::bill02_suppressed` | ❌ Wave 0 |
| TEST-02 | failure_monitor CRASH-01 UDP silence threshold logic | unit sync | `cargo test -p rc-agent-crate failure_monitor::tests::crash01` | partial (threshold test exists but not named crash01) |
| TEST-02 | failure_monitor CRASH-02 launch timeout elapsed | unit sync | `cargo test -p rc-agent-crate failure_monitor::tests::crash02` | partial (elapsed test exists at line 352) |
| TEST-03 | FfbBackend trait: zero_force returns Ok(true) | unit sync | `cargo test -p rc-agent-crate ffb` | ❌ Wave 0 |
| TEST-03 | FfbBackend trait: zero_force returns Ok(false) device absent | unit sync | `cargo test -p rc-agent-crate ffb` | ❌ Wave 0 |
| TEST-03 | FfbBackend trait: set_gain sends | unit sync | `cargo test -p rc-agent-crate ffb` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate billing_guard failure_monitor ffb`
- **Per wave merge:** `cargo test -p rc-agent-crate`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `mockall = "0.13"` in `crates/rc-agent/Cargo.toml` `[dev-dependencies]`
- [ ] `pub trait FfbBackend` in `crates/rc-agent/src/ffb_controller.rs` — zero infrastructure exists
- [ ] `impl FfbBackend for FfbController` delegating to existing methods
- [ ] `#[tokio::test]` timer tests in `billing_guard.rs` `tests` module

## Open Questions

1. **billing_guard timer test: how many yield_now() calls?**
   - What we know: `tokio::time::advance()` moves the clock but requires task yields to process
   - What's unclear: The billing_guard loop has multiple `.await` points (interval.tick(), try_send) — one `yield_now()` may not be enough
   - Recommendation: Start with `yield_now().await` in a small loop (up to 10 iterations) and assert non-empty. Alternatively use `tokio::time::advance()` followed by `tokio::task::yield_now().await` twice for safety.

2. **FfbBackend trait scope: include backup/verify methods?**
   - What we know: `backup_conspit_configs()` and `verify_conspit_configs()` are free functions, not `FfbController` methods. They have their own `base_dir` injection pattern for testing.
   - What's unclear: Whether TEST-03 scope includes backup/verify or only the FFB command surface.
   - Recommendation: Scope `FfbBackend` to the five FFB command methods only (`zero_force`, `zero_force_with_retry`, `set_gain`, `fxm_reset`, `set_idle_spring`). The backup/verify path already has test injection via the `base_dir: Option<&Path>` parameter pattern — no trait needed there.

3. **failure_monitor CRASH-01: is `is_game_process_hung()` testable?**
   - What we know: The function is `#[cfg(windows)]` and calls `EnumWindows`/`IsHungAppWindow` — cannot be called without a real game window. The `#[cfg(not(windows))]` stub always returns false.
   - What's unclear: Whether a mock is needed or condition testing is sufficient for TEST-02.
   - Recommendation: TEST-02 is satisfied by testing the condition guard (UDP silence + game_pid present) that gates the hung check. The hung check itself is hardware-only integration behavior. Document this explicitly in the test with a comment: "// is_game_process_hung() is Windows-only hardware behavior — not tested here".

## Sources

### Primary (HIGH confidence)
- Direct code inspection — `billing_guard.rs`, `failure_monitor.rs`, `ffb_controller.rs` (full read)
- `crates/rc-agent/Cargo.toml` — current dev-dependencies confirmed
- `.planning/STATE.md` — "Decisions (v11.0)" section confirms FfbBackend trait decision and mockall 0.13 choice
- tokio documentation — `tokio::time::pause()` API confirmed in tokio 1.x (workspace dep = version "1")

### Secondary (MEDIUM confidence)
- mockall 0.13 MSRV 1.77: from STATE.md note "MSRV 1.77, project at 1.93.1" — verified consistent with published mockall crate documentation

### Tertiary (LOW confidence)
- None — all critical findings verified from source code and project state

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — directly confirmed in source code and STATE.md decisions
- Architecture: HIGH — billing_guard.rs spawn() signature and watch injection pattern are exact; existing tests provide the pattern to extend
- Pitfalls: HIGH — time advance / yield interaction is a known tokio testing pattern; confirmed by existing lock_screen.rs `#[tokio::test]` examples in the codebase

**Research date:** 2026-03-20 IST
**Valid until:** 2026-04-20 (stable Rust testing patterns, no fast-moving ecosystem dependency)
