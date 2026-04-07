# Phase 329 Plan 03: Remaining Painters + Edge Removal - Research

**Researched:** 2026-04-07
**Domain:** Win32 GDI lock screen completion, Edge browser code removal
**Confidence:** HIGH

## Summary

Plans 329-01 and 329-02 have already completed the vast majority of 329-03's planned work. After thorough code inspection, **all 14 LockScreenState painters are already implemented** in `native_lock/painter.rs` (882 lines). The `lock_screen.rs` file (1275 lines) has already been cleaned to use native Win32 windows exclusively -- no Edge spawn code, no HTML template generation, no SNSS cleanup, no `render_page_public`, no `page_shell_with_bg` functions exist. The HTTP server already serves JSON only.

The remaining work for 329-03 is significantly smaller than the original plan anticipated:

1. **Legacy API shim methods** (`launch_browser`, `close_browser`, `is_browser_alive`, `is_browser_expected`) exist as thin wrappers in `lock_screen.rs` (lines 712-730). These are called from `event_loop.rs` and `ws_handler.rs`. The plan says to delete these, but they already delegate to native window methods. The question is whether to rename call sites or keep the shims.

2. **Edge process kill code** in `event_loop.rs` (lines 2060-2063) kills `msedge.exe` and `msedgewebview2.exe` as part of a ForceRelaunchBrowser handler -- this is lock-screen-specific Edge cleanup that should be removed or updated.

3. **Pod 8 visual verification** -- the canary deploy and visual check.

**Primary recommendation:** The 329-03 plan's Task 1 is ~80% already done. The remaining work is: (a) optionally rename legacy browser API methods to native window terminology across callers, (b) remove the msedge.exe kill in the ForceRelaunchBrowser handler in event_loop.rs, and (c) visual verification on Pod 8.

## Current State of Implementation

### What Already Exists (from 329-01 and 329-02)

**All 14 state painters implemented in `native_lock/painter.rs` (882 lines):**

| State | Painter Function | Status | Implemented In |
|-------|-----------------|--------|----------------|
| ScreenBlanked | `FillRect(brush_black)` (inline) | DONE | Plan 01 |
| StartupConnecting | `paint_branding` + text (inline) | DONE | Plan 01 |
| Disconnected | `paint_branding` + red text (inline) | DONE | Plan 01 |
| Hidden | `FillRect(brush_black)` (inline) | DONE | Plan 01 |
| PinEntry | `paint_pin_entry()` | DONE | Plan 02 |
| QrDisplay | `paint_qr_display()` | DONE | Plan 02 |
| ActiveSession | `paint_active_session()` | DONE | Plan 02 |
| SessionSummary | `paint_session_summary()` | DONE | Already in code |
| BetweenSessions | `paint_between_sessions()` | DONE | Already in code |
| LaunchSplash | `paint_launch_splash()` | DONE | Already in code |
| AwaitingAssistance | `paint_awaiting_assistance()` | DONE | Already in code |
| ConfigError | `paint_config_error()` | DONE | Already in code |
| Lockdown | `paint_lockdown()` | DONE | Already in code |
| MaintenanceRequired | `paint_maintenance_required()` | DONE | Already in code |

**No wildcard `_ =>` match arm** -- every variant has an explicit match arm in `paint_lock_screen()`.

**The `lock_screen.rs` file (1275 lines) is already clean:**
- No `render_page_public()` function
- No `page_shell_with_bg()` function
- No HTML template generation
- No Edge spawn/launch code
- No SNSS cleanup
- No `count_edge_processes()`
- No `enforce_kiosk_foreground()` (Edge-specific version)
- No `wallpaper_url` field
- No `browser_process` field
- HTTP server serves JSON only (`/health`, `/state`, `/countdown-warning`)
- Module doc comment already says "native Win32 fullscreen window via the `native_lock` module"

### What Still Has Edge References (needs attention)

#### 1. Legacy API Shim Methods in `lock_screen.rs` (lines 708-730)

```rust
// ---- Legacy API compatibility ----
// These methods maintain the old public API so callers in event_loop.rs,
// ai_debugger.rs etc. continue to work without changes.

pub fn launch_browser(&mut self) {
    self.show_native_window();
}

pub fn close_browser(&mut self) {
    self.hide_native_window();
}

pub fn is_browser_alive(&self) -> bool {
    self.is_window_alive()
}

pub fn is_browser_expected(&self) -> bool {
    self.is_window_expected()
}
```

**Callers of these shims:**

| File | Line | Call | Context |
|------|------|------|---------|
| `event_loop.rs` | 465 | `close_browser()` | Session end: hide lock screen |
| `event_loop.rs` | 945 | `close_browser()` | Game launch: hide lock screen |
| `event_loop.rs` | 965 | `close_browser()` | Game launch variant |
| `event_loop.rs` | 987 | `close_browser()` | Game launch variant |
| `event_loop.rs` | 1016 | `close_browser()` | Game launch variant |
| `event_loop.rs` | 1520-1524 | `is_browser_expected()` + `is_browser_alive()` + `launch_browser()` | Window watchdog: respawn if dead |
| `event_loop.rs` | 2069-2070 | `close_browser()` + `launch_browser()` | ForceRelaunchBrowser handler |
| `ws_handler.rs` | 698 | `close_browser()` | Session transition |
| `ws_handler.rs` | 1767-1768 | `close_browser()` + `launch_browser()` | ForceRelaunchBrowser handler |

**Decision needed:** Either (a) rename all call sites to `hide_native_window()` / `show_native_window()` / `is_window_alive()` / `is_window_expected()` OR (b) keep the shims with a comment that they are legacy names. Option (a) is cleaner but touches more files; option (b) is lower risk.

#### 2. msedge.exe Kill in ForceRelaunchBrowser (event_loop.rs:2055-2070)

```rust
// Lines ~2055-2070 in event_loop.rs:
// Kills msedge.exe and msedgewebview2.exe, then calls close_browser + launch_browser
```

This code explicitly kills Edge processes as part of force-relaunching the "browser". Since the lock screen no longer uses Edge, this kill is unnecessary. However, the `close_browser()` + `launch_browser()` pattern (which now maps to hide/show native window) is still valid.

**IMPORTANT DISTINCTION:** The `msedge.exe` references in these OTHER files are NOT lock-screen related and must be KEPT:
- `tier_engine.rs` -- POS kiosk Edge browser management (billing dashboard)
- `diagnostic_engine.rs` -- POS health check (is Edge running for billing?)
- `kiosk.rs` -- Process allowlist (Edge is allowed on pods/POS)
- `ac_launcher.rs` -- PowerShell script allowlist for AC game
- `ai_debugger.rs` -- Kill Edge crash reporter dialogs (still valid for POS)
- `mma_engine.rs` -- Health check heuristic (legacy, may need update)
- `openrouter.rs` -- Training data / prompt text (cosmetic, not functional)
- `cognitive_gate.rs` -- Diagnostic text (cosmetic)

#### 3. Module Location: `native_lock/` vs `lock_screen/`

The 329-03 PLAN references files at `crates/rc-agent/src/lock_screen/painter.rs` etc., but the actual code is at `crates/rc-agent/src/native_lock/`. This is a known discrepancy documented in both 329-01 and 329-02 summaries. The plan from 329-01 noted: "Plan 03 will rename when lock_screen.rs is converted to lock_screen/mod.rs."

**This rename is complex and risky:**
- `lock_screen.rs` contains the `LockScreenState` enum, `LockScreenManager`, HTTP server, tests
- `native_lock/` contains the Win32 window, painter, font, keyboard, QR modules
- Merging them into a `lock_screen/` directory would require:
  - Renaming `lock_screen.rs` to `lock_screen/mod.rs`
  - Moving `native_lock/*` into `lock_screen/`
  - Updating all `use crate::native_lock::` to `use crate::lock_screen::`
  - Updating all `use crate::lock_screen::` (which currently refers to the file)
  - Touching main.rs module declarations

**Recommendation:** Skip the rename in 329-03. It is not in the WIN-05 requirements and risks compilation breakage across many files. The two-module layout (`lock_screen.rs` for state/manager, `native_lock/` for Win32 rendering) is clean and well-documented.

#### 4. Test Names Reference "browser"

Two tests in `lock_screen.rs` (lines 1175, 1192):
- `test_close_browser_safe_mode`
- `test_close_browser_normal_mode`

These test the `close_browser()` shim method. If the shim is renamed, tests must update too.

## What 329-03 Actually Needs To Do

### Minimal scope (recommended):

1. **Remove msedge kill from ForceRelaunchBrowser handler** in `event_loop.rs` (lines ~2055-2065) -- the `taskkill /IM msedge.exe` and `taskkill /IM msedgewebview2.exe` lines. Keep the `close_browser()` + `launch_browser()` calls (they now just hide/show native window).

2. **Update or remove stale comments** that reference Edge in `lock_screen.rs` and `event_loop.rs`.

3. **Visual verification on Pod 8** -- deploy, verify all states render correctly on 7680x1440.

### Extended scope (optional, if time permits):

4. **Rename legacy API methods**: `launch_browser` -> `show_window` / `close_browser` -> `hide_window` etc. across all callers. This is ~15 call sites across 3 files.

5. **Update mma_engine.rs** (line 1823-1828) -- the blanking health check that counts msedge.exe processes is now wrong since native window does not spawn msedge. This check should be updated to use `is_window_alive()` instead.

6. **Update ForceRelaunchBrowser WS command** in `ws_handler.rs` -- the command name itself references "browser" but the concept (force-restart lock screen display) is still valid. Consider renaming to `ForceRelockScreen` or keeping as-is for API compatibility.

## Architecture Patterns

### Current Module Layout (correct, not what plan says)

```
crates/rc-agent/src/
  lock_screen.rs          # LockScreenState, LockScreenManager, HTTP JSON server, tests (1275 lines)
  native_lock/            # Win32 GDI rendering (NOT lock_screen/ as plan says)
    mod.rs                # NativeLockScreen struct, show/hide/destroy API (166 lines)
    font.rs               # Montserrat embedding, LockGdiResources cache (9026 bytes)
    window.rs             # Win32 message loop, WM_PAINT dispatch (13722 bytes)
    painter.rs            # All 14 state paint functions (882 lines)
    keyboard.rs           # PinInputState, digit handling (5469 bytes)
    qr.rs                 # QR code GDI renderer (3080 bytes)
```

### State Flow

```
LockScreenManager (lock_screen.rs)
  |-- state: Arc<Mutex<LockScreenState>>  (shared with native_lock)
  |-- native_window: Option<NativeLockScreen>
  |
  show_*() methods set state + call show_native_window()
  hide_native_window() hides (SW_HIDE) but keeps window alive
  |
  v
NativeLockScreen (native_lock/mod.rs)
  |-- spawns window thread
  |-- PostMessage for show/hide/repaint
  |
  v
spawn_lock_window (native_lock/window.rs)
  |-- WM_PAINT -> paint_lock_screen() 
  |-- WM_CHAR/WM_KEYDOWN -> PIN input
  |-- WM_TIMER -> repaint every 1s
  |
  v
paint_lock_screen (native_lock/painter.rs)
  |-- match on LockScreenState -> dedicated paint function per variant
```

## Common Pitfalls

### Pitfall 1: Confusing POS Edge with Lock Screen Edge
**What goes wrong:** Removing msedge references that are actually for the POS billing kiosk
**Why it happens:** msedge.exe appears in many files for different purposes
**How to avoid:** Only touch lock-screen-specific Edge code. POS Edge code in `tier_engine.rs`, `diagnostic_engine.rs`, `kiosk.rs` must be KEPT.
**Warning signs:** Any change in files outside `lock_screen.rs` / `event_loop.rs` / `ws_handler.rs` for the "remove Edge" task

### Pitfall 2: Module Path Mismatch with Plan
**What goes wrong:** Plan references `crates/rc-agent/src/lock_screen/painter.rs` but code is at `native_lock/painter.rs`
**Why it happens:** 329-01 created `native_lock/` because Rust cannot have both `lock_screen.rs` (file) and `lock_screen/` (directory)
**How to avoid:** Use actual paths from filesystem, not plan paths

### Pitfall 3: Renaming Legacy API Methods Breaks Callers
**What goes wrong:** Renaming `launch_browser()` without updating all 15+ call sites causes compilation failure
**How to avoid:** If renaming, use `cargo check` after each file change. Or keep shims.

### Pitfall 4: mma_engine Blanking Health Check is Now Wrong
**What goes wrong:** `mma_engine.rs` checks `msedge.exe` process count to verify blanking screen is working. With native window, msedge will never be running, so this check will ALWAYS fire a false alarm.
**How to avoid:** This should be updated to check `is_window_alive()` or simply removed since the native window watchdog in `event_loop.rs` already handles respawning.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | `Cargo.toml` |
| Quick run command | `cargo test -p rc-agent-crate -- lock_screen` |
| Full suite command | `cargo test -p rc-agent-crate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WIN-05 | All 14 states have painters (no wildcard) | unit | `cargo test -p rc-agent-crate -- pin_input` | Existing (10 tests) |
| WIN-05 | HTTP server JSON-only | unit | `cargo test -p rc-agent-crate -- lock_screen::tests` | Existing (14 tests) |
| WIN-05 | No Edge references in lock_screen module | manual grep | `grep -rn "msedge\|render_page_public" crates/rc-agent/src/lock_screen.rs` | N/A |
| WIN-05 | Binary compiles | build | `cargo build --release --bin rc-agent` | N/A |
| WIN-05 | Visual on Pod 8 | human | SSH + screenshot | N/A |

### Sampling Rate
- **Per task commit:** `cargo build --release --bin rc-agent && cargo test -p rc-agent-crate`
- **Per wave merge:** Full test suite
- **Phase gate:** Full suite green + Pod 8 visual verification

## Edge References Inventory (Do NOT Remove)

These msedge references are for POS/kiosk/diagnostics, NOT the lock screen:

| File | Purpose | Keep? |
|------|---------|-------|
| `tier_engine.rs` (6 refs) | POS billing Edge kiosk management | YES |
| `diagnostic_engine.rs` (4 refs) | POS health check: is Edge running? | YES |
| `kiosk.rs` (3 refs) | Process allowlist (Edge allowed on pods) | YES |
| `ac_launcher.rs` (3 refs) | PowerShell allowlist for AC game | YES |
| `ai_debugger.rs` (6 refs) | Kill Edge crash reporter, memory monitoring | YES |
| `cognitive_gate.rs` (2 refs) | Diagnostic text templates | YES |
| `openrouter.rs` (3 refs) | AI prompt training data | YES |
| `mma_engine.rs` (3 refs) | Blanking health check -- NEEDS UPDATE (false alarm risk) | UPDATE |

## Sources

### Primary (HIGH confidence)
- Direct code inspection of all files in `crates/rc-agent/src/` via Read tool
- `329-01-SUMMARY.md` -- confirmed Plan 01 deliverables
- `329-02-SUMMARY.md` -- confirmed Plan 02 deliverables
- `native_lock/painter.rs` -- all 882 lines read, all 14 painters confirmed
- `lock_screen.rs` -- all 1275 lines read, no Edge code found

## Metadata

**Confidence breakdown:**
- Painter completeness: HIGH -- read every line of painter.rs, all 14 variants matched
- Edge removal status: HIGH -- read every line of lock_screen.rs, grep confirmed no Edge spawn code
- Remaining work scope: HIGH -- grep found all msedge/browser references across codebase
- POS Edge distinction: HIGH -- read relevant functions in tier_engine.rs and diagnostic_engine.rs

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable codebase, no external dependencies changing)
