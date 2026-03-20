# Phase 57: Session-End Safety - Research

**Researched:** 2026-03-20
**Domain:** OpenFFBoard HID safety commands, ConspitLink process management, Rust async/sync session-end orchestration
**Confidence:** HIGH (verified against OpenFFBoard wiki, existing rc-agent source, and ConspitLink filesystem)

## Summary

Phase 57 replaces the current ESTOP-only session-end sequence with a proper close-ConspitLink-then-HID-commands approach that safely centers the wheelbase. The core technical challenge is three-fold: (1) sending the correct HID commands (`fxm.reset` + `axis.idlespring`) to clear orphaned DirectInput effects and apply a gentle centering spring, (2) closing ConspitLink before HID commands to avoid P-20 contention, and (3) restarting ConspitLink afterward with verified config integrity.

The existing codebase has strong foundations: `FfbController` already handles HID open/write with the 26-byte vendor report format, `send_vendor_cmd_to_class()` is a generic command sender that just needs new constants, and overlay.rs has a reusable `WM_CLOSE` pattern via `PostMessageW`. There are 9 session-end call sites in main.rs (plus 2 startup sites and 1 panic hook) that all follow the same pattern: `spawn_blocking(ffb.zero_force())` then `enforce_safe_state()`. Replacing `zero_force()` with a new `safe_session_end()` function is the central refactor.

**Primary recommendation:** Add `fxm_reset()`, `set_idle_spring()`, and `read_position()` methods to `FfbController`. Create a new `safe_session_end()` async function that orchestrates: WM_CLOSE ConspitLink (5s timeout) -> fxm.reset -> idlespring ramp -> restart ConspitLink. Replace all 9 session-end call sites with this function. Keep the panic hook using ESTOP (sync-safe).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- Fleet is primarily **Ares 12Nm** (7 pods) with one **Ares 8Nm** as hot-swap spare
- Both use OpenFFBoard firmware (VID: 0x1209, PID: 0xFFB0) -- same HID protocol
- Power cap: **80% for both** (9.6Nm on 12Nm, 6.4Nm on 8Nm)
- Shutdown sequence: Close ConspitLink (WM_CLOSE, 5s timeout) -> skip if timeout (don't force-kill) -> send HID commands (fxm.reset then axis.idlespring) -> restart ConspitLink immediately -> block main loop 2-3s
- Centering spring ramps from 0 to target over **500ms minimum**
- ESTOP reserved for: panic hook, USB disconnect, manual trigger, escalation if WM_CLOSE + HID both fail
- Canary pod: whichever currently has the 8Nm
- Validation: manual test all 4 games (AC, F1 25, ACC/AC EVO, AC Rally)

### Claude's Discretion
- Exact idlespring value/range (needs empirical testing on hardware)
- Whether `fxm.reset` is available on Conspit's OpenFFBoard fork (test first)
- Ramp implementation (single command vs stepped writes)
- Per-game session-end differences (start universal)
- ESTOP recovery behavior (try gentle centering after, or stay limp)
- Exact WM_CLOSE implementation for ConspitLink window (reuse overlay.rs pattern)

### Deferred Ideas (OUT OF SCOPE)
- Per-game session-end customization -- Phase 61 (FFB Preset Tuning)
- `axis.curpos` position verification -- nice to have but not blocking
- Crash-count tracking for ConspitLink -- Phase 58 (Process Hardening)
- Config file backup/verification -- Phase 58 (Process Hardening)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SAFE-01 | Wheelbase returns to center within 2 seconds of game session ending | `fxm.reset` clears orphaned effects, `axis.idlespring` applies centering spring. 500ms ramp + 1500ms settle = fits 2s budget. |
| SAFE-02 | Session-end uses fxm.reset + axis.idlespring (NOT estop) | New HID constants: CLASS_FXM (0xA03) CMD_RESET (0x1), CLASS_AXIS (0xA01) CMD_IDLESPRING (0x5). Both verified on OpenFFBoard wiki. |
| SAFE-03 | Force ramp-up is gradual (500ms minimum) | Stepped HID writes: 5 steps at 100ms intervals, incrementing idlespring value from 0 to target. |
| SAFE-04 | Venue power capped at safe maximum via axis.power | Already implemented as `set_gain()` in ffb_controller.rs. 80% = value 52428 (0.8 * 65535). Call at startup. |
| SAFE-05 | ESTOP reserved for emergencies only (separate code path) | Panic hook at line 401 keeps `zero_force_with_retry()`. New `safe_session_end()` is a distinct function for routine ends. |
| SAFE-06 | ConspitLink gracefully closed (WM_CLOSE) before HID commands | Reuse `FindWindowW` + `PostMessageW(WM_CLOSE)` pattern from overlay.rs. 5s process-exit poll. |
| SAFE-07 | ConspitLink restarted after safety sequence, JSON integrity verified | Reuse `ensure_conspit_link_running()` restart pattern. Add JSON parse check of Global.json after restart. |
</phase_requirements>

## Standard Stack

### Core (already in rc-agent)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `hidapi` | 2.x | HID device open/write for OpenFFBoard vendor commands | Already in Cargo.toml, used by ffb_controller.rs |
| `winapi` | 0.3 | FindWindowW, PostMessageW (WM_CLOSE), ShowWindow, process management | Already in Cargo.toml, used by overlay.rs and ac_launcher.rs |
| `tokio` | workspace | spawn_blocking for sync HID ops, sleep for ramp timing | Already the async runtime |
| `serde_json` | workspace | JSON parse check for ConspitLink config integrity verification | Already in deps |

### New Crates Needed

None. All required functionality is available through existing dependencies.

### NOT Needed

| Technology | Why Not |
|------------|---------|
| `notify` crate | Filesystem watching is Phase 58+, not Phase 57 |
| `shared_memory` | Telemetry reading is Phase 64, not Phase 57 |
| DirectInput API | rc-agent uses vendor HID (0xFF00), not DirectInput |

## Architecture Patterns

### Recommended Changes

```
crates/rc-agent/src/
  ffb_controller.rs   # ADD: fxm_reset(), set_idle_spring(), read_position()
                      # ADD: safe_session_end() orchestrator function
                      # ADD: close_conspit_link() helper
                      # KEEP: zero_force(), zero_force_with_retry() for ESTOP path
  ac_launcher.rs      # MODIFY: enforce_safe_state() — remove ensure_conspit_link_running()
                      #         (ConspitLink restart is now handled by safe_session_end)
  main.rs             # MODIFY: 9 session-end sites — replace zero_force() with safe_session_end()
                      # KEEP: panic hook using zero_force_with_retry() (ESTOP)
                      # KEEP: startup sites using zero_force_with_retry()
```

### Pattern 1: New HID Command Constants

**What:** Add constants for fxm.reset, axis.idlespring, and axis.curpos alongside existing ESTOP/power constants.
**When to use:** All new HID commands follow this pattern.
**Example:**
```rust
// Source: https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands

/// Effects Manager class ID
const CLASS_FXM: u16 = 0x0A03;
/// Effects Manager: reset all effects
const CMD_FXM_RESET: u32 = 0x01;

/// Axis command: idle spring strength
const CMD_IDLESPRING: u32 = 0x05;
/// Axis command: read current position
const CMD_CURPOS: u32 = 0x0E;

/// HID command type: request/read
const CMD_TYPE_REQUEST: u8 = 1;
```

**CRITICAL NOTE on Class IDs:** The existing code uses `CLASS_FFBWHEEL: u16 = 0x00A1` which works in production for ESTOP. The upstream OpenFFBoard wiki documents FFBWheel as class 0x1, not 0xA1. The OpenFFBoard configurator also uses 0x1. Conspit's fork may use different class IDs. Since the existing ESTOP works with 0x00A1, there are two possibilities:
1. Conspit's fork genuinely uses 0xA1 as the FFBWheel class (their firmware is modified)
2. There is a byte-level coincidence where 0xA1 in the class field happens to work

For new commands (fxm.reset on CLASS_FXM, idlespring on CLASS_AXIS), we should **try upstream class IDs first** (0xA03, 0xA01) since `set_gain()` already uses `CLASS_AXIS: u16 = 0x0A01` successfully. If fxm.reset fails with 0x0A03, try 0x0A02 (which the configurator maps to effects management).

### Pattern 2: safe_session_end() Orchestrator

**What:** A single async function that replaces all 9 session-end zero_force() calls.
**When to use:** Every routine session end (game exit, billing stop, server command, disconnect cleanup).
**Example:**
```rust
// In ffb_controller.rs (or a new session_safety.rs module)

/// Routine session-end safety sequence.
/// Blocks for ~2-3s. NOT suitable for panic hook (use zero_force_with_retry instead).
///
/// Sequence:
/// 1. Close ConspitLink (WM_CLOSE, 5s timeout, skip on failure)
/// 2. fxm.reset (clear orphaned DirectInput effects)
/// 3. Ramp idlespring from 0 to target over 500ms (5 steps)
/// 4. Restart ConspitLink
pub async fn safe_session_end(ffb: &FfbController) {
    // Step 1: Close ConspitLink (sync, in spawn_blocking)
    let closed = tokio::task::spawn_blocking(|| {
        close_conspit_link(Duration::from_secs(5))
    }).await.unwrap_or(false);

    if !closed {
        tracing::warn!("ConspitLink did not close within 5s — proceeding with HID commands anyway (P-20 risk accepted)");
    }

    // Step 2: fxm.reset + idlespring ramp (sync, in spawn_blocking)
    let ffb_clone = ffb.clone(); // FfbController must impl Clone (it's just vid/pid)
    tokio::task::spawn_blocking(move || {
        // Clear all orphaned effects
        if let Err(e) = ffb_clone.fxm_reset() {
            tracing::warn!("fxm.reset failed: {} — continuing with idlespring", e);
        }
        std::thread::sleep(Duration::from_millis(50));

        // Ramp idlespring: 5 steps over 500ms
        let target = 2000i64; // Empirical — test on hardware
        for step in 1..=5 {
            let value = (target * step) / 5;
            let _ = ffb_clone.set_idle_spring(value);
            std::thread::sleep(Duration::from_millis(100));
        }
    }).await.ok();

    // Step 3: Restart ConspitLink (non-blocking thread)
    tokio::task::spawn_blocking(|| {
        restart_conspit_link();
    });

    tracing::info!("Session-end safety sequence complete — wheel centering with spring");
}
```

### Pattern 3: WM_CLOSE for ConspitLink

**What:** Find ConspitLink window by title, send WM_CLOSE, poll for process exit.
**When to use:** Step 1 of safe_session_end().
**Example:**
```rust
// Reuse the FindWindowW pattern from minimize_conspit_window() in ac_launcher.rs
fn close_conspit_link(timeout: Duration) -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        fn wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }

        let titles = ["Conspit Link 2.0", "ConspitLink2.0", "Conspit Link", "ConspitLink"];
        let mut sent = false;
        for title in &titles {
            unsafe {
                let title_wide = wide(title);
                let hwnd = winapi::um::winuser::FindWindowW(std::ptr::null(), title_wide.as_ptr());
                if !hwnd.is_null() {
                    winapi::um::winuser::PostMessageW(hwnd, winapi::um::winuser::WM_CLOSE, 0, 0);
                    tracing::info!("Sent WM_CLOSE to ConspitLink via \"{}\"", title);
                    sent = true;
                    break;
                }
            }
        }

        if !sent {
            tracing::debug!("ConspitLink window not found — may not be running");
            return true; // Not running = already "closed"
        }

        // Poll for process exit
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !is_process_running("ConspitLink2.0.exe") {
                tracing::info!("ConspitLink exited after WM_CLOSE ({}ms)", start.elapsed().as_millis());
                return true;
            }
            std::thread::sleep(Duration::from_millis(250));
        }

        tracing::warn!("ConspitLink still running after {}s WM_CLOSE timeout", timeout.as_secs());
        false
    }
    #[cfg(not(windows))]
    { true }
}
```

### Anti-Patterns to Avoid

- **Never taskkill /F ConspitLink** -- corrupts JSON config files, breaks preset state. WM_CLOSE only, skip on timeout.
- **Never send HID commands while ConspitLink is running** (for safety-critical commands) -- P-20 race condition. Close CL first.
- **Never use ESTOP for routine session ends** -- ESTOP zeros ALL torque including the centering spring you are about to apply.
- **Never block the panic hook on async** -- panic hook must be sync-only (thread::sleep, not tokio::sleep).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Window finding | EnumWindows callback | FindWindowW with title array | Already proven in minimize_conspit_window(), handles multiple title variants |
| Process exit detection | tasklist parsing | is_process_running() polling | Already exists in ac_launcher.rs, handles CSV parsing |
| HID report construction | Manual byte packing | send_vendor_cmd_to_class() | Already handles the 26-byte format with proper endianness |
| ConspitLink restart | New restart logic | Reuse ensure_conspit_link_running() internals | Already handles cmd /c start + 4s delay + minimize |

## Common Pitfalls

### Pitfall 1: Class ID Mismatch on Conspit's OpenFFBoard Fork
**What goes wrong:** fxm.reset or idlespring HID writes silently fail because Conspit's firmware uses different class IDs than upstream OpenFFBoard.
**Why it happens:** Conspit forks OpenFFBoard. The existing code uses CLASS_FFBWHEEL = 0x00A1 (upstream = 0x1), suggesting class IDs may differ. CLASS_AXIS = 0x0A01 works for set_gain() so it matches upstream.
**How to avoid:** Test fxm.reset (class 0x0A03) on the canary pod FIRST. If it fails (HID write succeeds but no effect), try class 0x0A02. Add tracing logs for each HID write result. The implementer has Claude's Discretion to determine whether fxm.reset works.
**Warning signs:** HID write returns Ok but wheel behavior does not change. Check the 'type' byte in the response (10=ACK, 13=notFound, 15=err).

### Pitfall 2: Idlespring Value Range Unknown
**What goes wrong:** Setting idlespring too high causes snap-back (defeating SAFE-03). Setting it too low means wheel does not center.
**Why it happens:** OpenFFBoard wiki does not document the value range for idlespring. It could be 0-65535 (like power), 0-10000, or 0-255.
**How to avoid:** Start with a low value (e.g., 500) and increment during empirical testing on the canary pod. The ramp ensures even wrong-ish values are applied gradually. Document the tested safe range.
**Warning signs:** Wheel snaps to center suddenly, or does not move at all after session end.

### Pitfall 3: WM_CLOSE Blocks on ConspitLink Save Dialog
**What goes wrong:** ConspitLink shows a "save changes?" dialog on WM_CLOSE, preventing graceful exit. The 5s timeout fires and we proceed without closing.
**Why it happens:** Qt applications often intercept WM_CLOSE to prompt for unsaved state.
**How to avoid:** This is handled by the CONTEXT.md decision: "skip and send HID commands anyway (accept P-20 risk, don't force-kill)". The 5s timeout is the fallback. If this becomes frequent, Phase 58 can add dialog dismissal.
**Warning signs:** ConspitLink consistently fails to close within 5s.

### Pitfall 4: enforce_safe_state() Restarts ConspitLink Before HID Commands Complete
**What goes wrong:** The current code calls `enforce_safe_state()` after `zero_force()`. enforce_safe_state() calls `ensure_conspit_link_running()` which restarts ConspitLink. If ConspitLink starts before idlespring is applied, P-20 contention occurs.
**Why it happens:** enforce_safe_state() is not aware of the new session-end sequence.
**How to avoid:** Remove the `ensure_conspit_link_running()` call from enforce_safe_state() for the session-end code path, OR restructure so safe_session_end() handles the full lifecycle including ConspitLink restart and enforce_safe_state() skips ConspitLink management.
**Warning signs:** ConspitLink restarts during the idlespring ramp, overwriting the centering spring.

### Pitfall 5: Timing Budget Overrun
**What goes wrong:** WM_CLOSE (5s max) + fxm.reset (50ms) + idlespring ramp (500ms) + CL restart (4s) = 9.5s worst case. SAFE-01 requires 2 seconds.
**Why it happens:** The 2-second requirement applies to wheel centering, not the full sequence.
**How to avoid:** Clarify: the 2s requirement is "wheel starts returning to center within 2s". The sequence is: close CL (non-blocking once WM_CLOSE sent) -> fxm.reset at T+0 -> idlespring ramp at T+50ms -> wheel starts centering immediately -> CL restart happens in background. The blocking window in main loop is ~2-3s (steps 1-2), not the full 9.5s. CL restart is fire-and-forget in a background thread.
**Warning signs:** Main loop blocks for more than 3s during session end.

## Code Examples

### Exact HID Bytes for New Commands

Verified from OpenFFBoard wiki Commands page:

```rust
// fxm.reset — Clear all orphaned DirectInput effects
// Class: 0x0A03 (Effects Manager), Cmd: 0x01, Data: 0 (reset all)
// Bytes: [0xA1, 0x00, 0x03, 0x0A, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00..0x00, 0x00..0x00]
//         ReportID Write  ClassLE   Inst  CmdID_LE            Data(0)         Addr(0)
self.send_vendor_cmd_to_class(&device, 0x0A03, 0x01, 0)

// axis.idlespring — Set centering spring strength
// Class: 0x0A01 (Axis), Cmd: 0x05, Data: spring_value (range TBD, likely 0-65535)
self.send_vendor_cmd_to_class(&device, 0x0A01, 0x05, spring_value)

// axis.power — Set overall force cap (SAFE-04)
// Class: 0x0A01 (Axis), Cmd: 0x00, Data: 0-65535
// 80% = 52428
self.send_vendor_cmd_to_class(&device, 0x0A01, 0x00, 52428)

// axis.curpos — Read current wheel position (optional, deferred)
// Class: 0x0A01 (Axis), Cmd: 0x0E, Type: REQUEST (1), Data: 0
// Requires CMD_TYPE_REQUEST (0x01) instead of CMD_TYPE_WRITE (0x00)
// Read response from device after sending request
```

### Current Session-End Pattern (to be replaced)

```rust
// CURRENT (9 sites in main.rs, all identical pattern):
{ let f = ffb.clone(); tokio::task::spawn_blocking(move || { f.zero_force().ok(); }).await.ok(); }
tokio::time::sleep(Duration::from_millis(500)).await;
// ... game cleanup ...
tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });

// NEW (replacement):
safe_session_end(&ffb).await;
// ... game cleanup ...
// enforce_safe_state() no longer needs to manage ConspitLink
tokio::task::spawn_blocking(|| { ac_launcher::enforce_safe_state(); });
```

### Call Site Inventory (12 total, 9 need replacement)

| Line | Context | Action |
|------|---------|--------|
| 401 | Panic hook | KEEP zero_force_with_retry() -- ESTOP path, sync-only |
| 605 | Startup HID detection | KEEP zero_force_with_retry() -- probe, not session-end |
| 745 | Startup safe state (8s delay) | REPLACE with safe_session_end() -- pod boot cleanup |
| 1310 | Game crashed during billing | REPLACE -- crash recovery FFB zero |
| 1342 | Game exited, no billing | REPLACE -- routine session end |
| 1532 | Session summary timeout reset | REPLACE -- delayed cleanup |
| 1702 | BillingStopped (orphan auto-end) | REPLACE -- billing lifecycle |
| 1838 | SessionEnded (no summary) | REPLACE -- server-initiated end |
| 1879 | SessionEnded (with summary) | REPLACE -- server-initiated end |
| 2278 | StopGame from server | REPLACE -- admin stop |
| 2359 | SubSessionEnded (between splits) | REPLACE -- split transition |
| 2722 | WS disconnect, no billing | REPLACE -- disconnect cleanup |

Total: 9 sites to replace (lines 745, 1310, 1342, 1532, 1702, 1838, 1879, 2278, 2359, 2722 -- actually 10).

**Note:** Line 745 (startup cleanup) should also use the new sequence since it runs 8s after boot when the pod may have a stuck wheel from a previous crash.

### FfbController Clone Derivation

The current `FfbController` struct holds only `vid: u16` and `pid: u16`. It does NOT derive Clone but is trivially cloneable. Add `#[derive(Clone)]` to the struct.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| ESTOP only on session end | fxm.reset + idlespring centering | Phase 57 (this phase) | Eliminates stuck-rotation bug, no snap-back |
| HID commands while CL running | Close CL before HID | Phase 57 (this phase) | Eliminates P-20 contention |
| enforce_safe_state() manages CL | safe_session_end() manages CL lifecycle | Phase 57 (this phase) | Unified session-end orchestration |

## Open Questions

1. **Exact idlespring value range**
   - What we know: OpenFFBoard wiki says "idle spring strength" with R/W flags. axis.power uses 0-65535.
   - What's unclear: Does idlespring also use 0-65535? Or a smaller range? What value produces a "gentle but noticeable" centering feel?
   - Recommendation: Empirical testing on canary pod. Start at 500, test 1000, 2000, 5000. Document what feels right.

2. **fxm.reset availability on Conspit's firmware**
   - What we know: Upstream OpenFFBoard has class 0xA03 with cmd 0x01. Existing code uses class IDs that partially match upstream (0x0A01 works, 0x00A1 differs from upstream 0x1).
   - What's unclear: Does Conspit's fork implement the Effects Manager class at all?
   - Recommendation: Send the command on canary pod. If HID write succeeds but no ACK (or notFound response), the class may not exist. Fall back to: idlespring-only (skip fxm.reset) if unavailable.

3. **Whether ConspitLink has a save dialog on WM_CLOSE**
   - What we know: Qt apps can intercept WM_CLOSE. ConspitLink is Qt5-based.
   - What's unclear: Does ConspitLink show a dialog or close silently?
   - Recommendation: Test manually on a pod. If it shows a dialog, WM_CLOSE may not work and we accept the P-20 fallback path.

4. **Reading HID responses for verification**
   - What we know: send_vendor_cmd_to_class() is write-only. The device sends ACK/notFound/err responses.
   - What's unclear: Does hidapi's `read()` with a timeout reliably receive the response? Is the response on the same report ID?
   - Recommendation: Optional -- implement a `read_response()` method to check for ACK after fxm.reset. If too complex, skip verification and rely on behavioral testing (wheel centers = it worked).

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | Cargo.toml workspace |
| Quick run command | `cargo test -p rc-agent -- --test-threads=1` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SAFE-01 | Wheel centers within 2s | manual-only | Manual: end session on canary pod, time with stopwatch | N/A |
| SAFE-02 | fxm.reset + idlespring HID bytes correct | unit | `cargo test -p rc-agent -- fxm_reset_buffer` | Wave 0 |
| SAFE-03 | Force ramp is gradual (500ms, 5 steps) | unit | `cargo test -p rc-agent -- idlespring_ramp` | Wave 0 |
| SAFE-04 | Power cap at 80% | unit | `cargo test -p rc-agent -- power_cap_80_percent` | Exists (test_set_gain_buffer_format) |
| SAFE-05 | ESTOP separate from session-end | unit | `cargo test -p rc-agent -- estop_path_separate` | Wave 0 |
| SAFE-06 | ConspitLink WM_CLOSE before HID | manual-only | Manual: verify CL closes before HID log entries | N/A |
| SAFE-07 | ConspitLink restarts with valid JSON | manual-only | Manual: verify CL running + Global.json parseable after sequence | N/A |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent -- --test-threads=1`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent && cargo test -p racecontrol`
- **Phase gate:** Full suite green + manual test on canary pod (all 4 games)

### Wave 0 Gaps
- [ ] `ffb_controller.rs` tests: `test_fxm_reset_buffer_format` -- verify class 0x0A03, cmd 0x01, data 0
- [ ] `ffb_controller.rs` tests: `test_idlespring_buffer_format` -- verify class 0x0A01, cmd 0x05, data range
- [ ] `ffb_controller.rs` tests: `test_idlespring_ramp_values` -- verify 5-step ramp produces correct increments
- [ ] `ffb_controller.rs` tests: `test_estop_still_uses_ffbwheel_class` -- regression test for existing ESTOP

## Sources

### Primary (HIGH confidence)
- [OpenFFBoard Wiki: Commands](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands) -- Class IDs (0xA03 for fxm, 0xA01 for axis), command IDs (0x01 for reset, 0x05 for idlespring, 0x0E for curpos), HID report type values (0=write, 1=request)
- [OpenFFBoard Configurator source](https://github.com/Ultrawipf/OpenFFBoard-configurator/blob/master/main.py) -- Confirmed class IDs: 0x1/0x2/0x3 for FFB, 0xA01 for axis, 0xA02 for effects
- `crates/rc-agent/src/ffb_controller.rs` -- Existing HID implementation: 26-byte vendor report, send_vendor_cmd_to_class(), CLASS_AXIS=0x0A01 works
- `crates/rc-agent/src/main.rs` -- 12 zero_force call sites identified and categorized
- `crates/rc-agent/src/ac_launcher.rs` -- enforce_safe_state(), ensure_conspit_link_running(), minimize_conspit_window(), is_process_running()
- `crates/rc-agent/src/overlay.rs` lines 1037-1056 -- PostMessageW(WM_CLOSE) reusable pattern

### Secondary (MEDIUM confidence)
- `.planning/research/conspit-link/STACK.md` -- HID report format, config file locations, DirectInput lifecycle
- `.planning/research/conspit-link/PITFALLS.md` -- P-20 contention, orphaned effects, snap-back risk
- `.planning/research/conspit-link/ARCHITECTURE.md` -- Component boundaries, session lifecycle flow

### Tertiary (LOW confidence)
- Idlespring value range -- not documented in OpenFFBoard wiki; needs empirical testing
- fxm.reset on Conspit's firmware -- upstream class 0xA03 may not be present in Conspit's fork
- ConspitLink WM_CLOSE behavior -- Qt app behavior not verified, may show save dialog

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new deps
- Architecture: HIGH -- patterns directly derived from existing code; only new code is HID constants and orchestration function
- HID commands: MEDIUM -- class IDs and cmd IDs verified against upstream OpenFFBoard wiki, but Conspit's fork may differ (existing code shows one mismatch: FFBWheel 0x00A1 vs upstream 0x1)
- Pitfalls: HIGH -- well-documented in research files and observed in production

**Research date:** 2026-03-20
**Valid until:** 2026-04-20 (stable domain, firmware/hardware unchanged)
