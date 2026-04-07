# Phase 330: Native On-Track Display + Off-Track Blanking - Research

**Researched:** 2026-04-07
**Domain:** Win32 GDI overlay rendering, AC shared memory telemetry, off-track detection
**Confidence:** HIGH

## Summary

This phase adds two features to the existing rc-agent overlay system: (1) enhance the native Win32 HUD overlay with richer on-track telemetry display, and (2) implement VMS-style off-track blanking that shows Racing Point branding when the car leaves the track mid-session.

The codebase already has all the building blocks. `overlay.rs` (1370+ lines) is a fully working native Win32 GDI HUD with a 96px bar at the top showing session timer, lap times, sectors, speed/gear/RPM, and billing info. `off_track_detector.rs` (150 lines) is a complete debounced off-track state machine that reads `current_lap_invalid` from AC shared memory and produces `Show`/`Hide`/`NoChange` overlay transitions. The `native_lock/` module (from Phase 329) provides full-screen Win32 GDI rendering with branded content and double-buffered painting. However, the off_track_detector is NOT currently wired into the event loop -- it exists as a standalone module with `mod off_track_detector;` in main.rs but no usage in event_loop.rs.

**Primary recommendation:** Wire `OffTrackDetector` into the event loop's telemetry processing path, and create a new full-screen blanking window (reusing native_lock patterns) that shows branding when off-track is detected. The existing overlay HUD already displays all required on-track data. Add a feature flag `off_track_blanking` to make it configurable per session type.

## Project Constraints (from CLAUDE.md)

### Locked Decisions
- Static CRT linking: `.cargo/config.toml` has `+crt-static` -- no vcruntime dependency on pods
- rc-agent MUST run in Session 1 (interactive desktop) -- GUI operations require this
- NVIDIA Surround = single virtual desktop (7680x1440) -- `get_virtual_screen_bounds()` via `GetSystemMetrics`
- Brand colors: Racing Red `#E10600`, Asphalt Black `#1A1A1A`, Gunmetal Grey `#5A5A5A`
- Font: Montserrat (body), Enthocentric (headers)
- Visual verification mandatory for display-affecting deploys
- Deploy via SCP + reboot for display changes; Pod 8 canary first
- `winapi` 0.3.9 for all Win32 FFI (already in Cargo.toml)
- No new GUI frameworks (egui, slint, iced, winit) -- use raw Win32 GDI

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `winapi` | 0.3.9 | Win32 FFI (window creation, GDI, timers) | Already in Cargo.toml. overlay.rs + native_lock/ prove the pattern. |
| `rc-common` types | n/a | `TelemetryFrame`, `current_lap_invalid`, `speed_kmh` | Shared types crate, already has all needed fields |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Montserrat TTF (embedded) | n/a | Branded text rendering | Already embedded via Phase 329 `include_bytes!` in native_lock/font.rs |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Separate blanking window | Reuse overlay.rs window (make it full-screen) | Overlay is a 96px top bar; blanking needs full-screen. Separate window is cleaner. |
| Off-track via isValidLap | Off-track via normalized_car_position spline analysis | normalizedCarPosition is available but requires track-specific boundary data. isValidLap is AC's own detection -- simpler, proven. |
| GDI for blanking screen | Reuse native_lock window | native_lock is the lock screen with PIN entry, complex state machine. Blanking overlay should be a simpler, separate window. |

## Architecture Patterns

### Existing Code Map

```
crates/rc-agent/src/
  overlay.rs              # EXISTING: 96px HUD bar (timer, laps, sectors, speed, billing)
                          # Pattern: Arc<Mutex<OverlayData>> + dedicated std::thread + WM_TIMER repaint
  off_track_detector.rs   # EXISTING: Debounced off-track state machine (Show/Hide/NoChange)
                          # NOT wired into event_loop yet
  native_lock/            # EXISTING: Full-screen GDI rendering (Phase 329 lock screen)
    mod.rs                # NativeLockScreen struct with show/hide/destroy API
    window.rs             # spawn_lock_window() -- dedicated thread, WM_TIMER, WM_PAINT
    painter.rs            # State-driven double-buffered GDI painting
    font.rs               # Montserrat embedding + LockGdiResources cache
  event_loop.rs           # Main loop: calls state.overlay.update_telemetry(&frame)
  sims/assetto_corsa.rs   # AC shared memory reader -- produces TelemetryFrame with current_lap_invalid
  feature_flags.rs        # In-memory flag store synced from server over WS
  config.rs               # Pod agent configuration
```

### Pattern 1: Off-Track Blanking Window (new)
**What:** A separate full-screen WS_POPUP | WS_EX_TOPMOST window that displays Racing Point branding when the car is off-track. Created once at session start (hidden), shown/hidden via PostMessageW on off-track transitions.
**When to use:** During active billing sessions with `off_track_blanking` feature flag enabled.
**Architecture:**
```rust
// New file: crates/rc-agent/src/blanking_overlay.rs
pub struct BlankingOverlay {
    hwnd: Arc<Mutex<Option<isize>>>,
    window_thread: Option<std::thread::JoinHandle<()>>,
}

impl BlankingOverlay {
    pub fn new() -> Self { /* ... */ }
    pub fn create_window(&mut self) { /* spawn thread, create hidden WS_POPUP */ }
    pub fn show(&self) { /* PostMessageW(WM_APP+3) */ }
    pub fn hide(&self) { /* PostMessageW(WM_APP+1) */ }
    pub fn destroy(&self) { /* PostMessageW(WM_CLOSE) */ }
}
```

### Pattern 2: Integration into Event Loop Telemetry Path
**What:** After `state.overlay.update_telemetry(&frame)` in event_loop.rs, call the off-track detector with the frame's `current_lap_invalid` and `speed_kmh`. On `OverlayChange::Show`, call `blanking_overlay.show()`. On `Hide`, call `blanking_overlay.hide()`.
**When to use:** Every telemetry frame during an active session.
**Example:**
```rust
// In event_loop.rs telemetry processing:
state.overlay.update_telemetry(&frame);

// Off-track blanking
if let Some(ref mut detector) = state.off_track_detector {
    let invalid = frame.current_lap_invalid.unwrap_or(false);
    match detector.update(invalid, frame.speed_kmh) {
        OverlayChange::Show => state.blanking_overlay.show(),
        OverlayChange::Hide => state.blanking_overlay.hide(),
        OverlayChange::NoChange => {}
    }
}
```

### Pattern 3: Feature Flag Gating
**What:** Use the existing `FeatureFlags` system to enable/disable off-track blanking. The flag `off_track_blanking` defaults to `false` (conservative). Staff can enable via admin dashboard per session type.
**When to use:** Session start -- check flag, create OffTrackDetector only if enabled.

### Anti-Patterns to Avoid
- **Do NOT create/destroy the blanking window on each off-track event.** Window creation is expensive (100ms+). Create once at session start, show/hide via SW_SHOW/SW_HIDE.
- **Do NOT block the event loop waiting for window operations.** Use PostMessageW (async, cross-thread safe).
- **Do NOT use the same window as the lock screen.** The lock screen has PIN entry, state machine, keyboard handling -- blanking overlay is much simpler (show branding, hide).
- **Do NOT remove the existing HUD overlay.** The on-track display IS the existing overlay.rs HUD. It already renders timer, lap count, position, sectors, speed.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Off-track detection logic | Custom position-based detection | `OffTrackDetector` (already exists) | Has debounce, speed filtering, edge case handling |
| Full-screen GDI rendering | Custom from scratch | Copy `native_lock/window.rs` pattern | Proven full-screen WS_POPUP with virtual desktop spanning |
| Font rendering | Custom font loader | Reuse `LockGdiResources` pattern | Montserrat already embedded, font cache pattern proven |
| Telemetry data flow | New telemetry pipeline | Existing `update_telemetry(&frame)` path | Already wired, frame has all needed fields |
| Feature flag config | Config file per-pod | `FeatureFlags` system | Server-synced, admin dashboard control, kill switch support |

## Common Pitfalls

### Pitfall 1: isValidLap Offset Uncertainty in AC Shared Memory
**What goes wrong:** The `IS_VALID_LAP` offset in `assetto_corsa.rs` is documented as "approximate -- may need correction" (line 105-107). Currently uses offset 180 but the real offset in AC's extended graphics struct is reportedly ~1408+.
**Why it happens:** AC's shared memory documentation is incomplete. The struct layout varies between AC versions and content manager patches.
**How to avoid:** The current offset 180 may read incorrect data. Test empirically: go off-track in AC, check if `current_lap_invalid` transitions to `true`. If unreliable, consider: (a) reading from the RC plugin shared memory (`rcpmf_telemetry`) which may expose a cleaner signal, or (b) using a velocity-change heuristic as fallback.
**Warning signs:** Blanking never triggers, or triggers randomly during normal driving.

### Pitfall 2: Window Z-Order Fight Between Blanking and HUD
**What goes wrong:** Both the blanking overlay and the HUD overlay are `WS_EX_TOPMOST`. When blanking shows, it may cover the HUD or vice versa.
**Why it happens:** Multiple TOPMOST windows compete for the top of the Z-order.
**How to avoid:** When blanking is active, the HUD should be hidden (or irrelevant since the player can't see the game). When blanking hides, the HUD should remain on top. Use `SetWindowPos` with `HWND_TOPMOST` on the blanking window's show, and re-enforce HUD topmost on blanking hide. Or simply: blanking is full-screen covering everything, HUD only needs to be visible when blanking is NOT active.
**Warning signs:** HUD visible on top of blanking screen, or HUD disappears when blanking hides.

### Pitfall 3: Blanking During Pit Lane
**What goes wrong:** AC may mark the lap as invalid when entering the pit lane, triggering unwanted blanking.
**Why it happens:** Pit lane entry can set `isValidLap = 0` in some AC versions.
**How to avoid:** `OffTrackDetector` already filters by `speed_kmh > 5.0` but pit lane speed can be >5. Additionally check `isInPit` from AC graphics shared memory (offset 160). If `isInPit == 1`, suppress blanking.
**Warning signs:** Blanking triggers every time car enters pits.

### Pitfall 4: Sub-500ms Response Time Requirement
**What goes wrong:** The success criteria require blanking within 500ms. The off_track_detector has `DEBOUNCE_ON = 1000ms` -- this is 2x the budget.
**Why it happens:** Conservative debounce to avoid flicker was set before the 500ms requirement existed.
**How to avoid:** Reduce `DEBOUNCE_ON` from 1000ms to 300ms for blanking. Keep `DEBOUNCE_OFF` at 500ms. The 200ms telemetry poll interval + 300ms debounce = 500ms total response time. Consider making debounce values configurable.
**Warning signs:** Blanking takes >500ms to appear after going off-track.

### Pitfall 5: GDI Handle Leak in Blanking Window
**What goes wrong:** Same as all GDI code -- leaked brushes/fonts crash the process.
**Why it happens:** Creating GDI resources in paint loop without cleanup.
**How to avoid:** Use `LockGdiResources` cache pattern (create once at WM_CREATE, free at WM_DESTROY via Drop). Use `TempBrush` RAII wrapper for any per-frame resources.
**Warning signs:** `gdi_handle_count()` rising over time.

## Code Examples

### Existing OffTrackDetector API
```rust
// Source: crates/rc-agent/src/off_track_detector.rs
pub struct OffTrackDetector {
    // Debounced state machine
}

impl OffTrackDetector {
    pub fn new(enabled: bool) -> Self;
    
    // Returns Show/Hide/NoChange
    pub fn update(&mut self, current_lap_invalid: bool, speed_kmh: f32) -> OverlayChange;
    
    pub fn reset(&mut self);           // Call on session end
    pub fn is_overlay_active(&self) -> bool;
    pub fn set_enabled(&mut self, enabled: bool);
}
```

### Existing Overlay Window Creation Pattern
```rust
// Source: crates/rc-agent/src/overlay.rs lines 1067-1176
fn win32_window_loop(state: Arc<Mutex<OverlayData>>, hwnd_slot: Arc<Mutex<Option<isize>>>) {
    // 1. Create GdiResources on thread
    // 2. RegisterClassExW("RacingHudOverlay")
    // 3. CreateWindowExW(WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED, 
    //                    ..., WS_POPUP | WS_VISIBLE, ...)
    // 4. SetLayeredWindowAttributes for opacity
    // 5. SetTimer for repaint interval
    // 6. GetMessageW loop
}
```

### Full-Screen Window Pattern (from native_lock)
```rust
// Source: crates/rc-agent/src/native_lock/window.rs lines 48-86
pub fn spawn_lock_window(state: Arc<Mutex<LockScreenState>>, hwnd_slot: Arc<Mutex<Option<isize>>>) {
    // 1. install_embedded_fonts()
    // 2. get_virtual_screen_bounds() -> (vx, vy, vw, vh)
    // 3. CreateWindowExW(WS_EX_TOPMOST, ..., WS_POPUP | WS_VISIBLE, vx, vy, vw, vh)
    // 4. SetTimer(TIMER_ID, 1000)
    // 5. Message loop
}
```

### TelemetryFrame Fields Available for Off-Track Detection
```rust
// Source: crates/rc-common/src/types.rs lines 172-223
pub struct TelemetryFrame {
    pub speed_kmh: f32,
    pub current_lap_invalid: Option<bool>,  // AC: isValidLap == 0
    pub normalized_car_position: Option<f32>, // 0.0-1.0 track spline
    // ... many more fields
}
```

### AC Shared Memory isValidLap Reading
```rust
// Source: crates/rc-agent/src/sims/assetto_corsa.rs lines 105-107, 597
// WARNING: IS_VALID_LAP offset 180 is approximate
pub const IS_VALID_LAP: usize = 180;
let is_valid = read_i32_buf(&graphics_buf, graphics::IS_VALID_LAP);
// TelemetryFrame: current_lap_invalid: Some(is_valid == 0)
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust) |
| Config file | Cargo.toml |
| Quick run command | `cargo test -p rc-agent-crate -- off_track` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| OTD-01 | In-session HUD renders timer, lap, position | manual (visual) | Visual deploy verification on Pod 8 | n/a |
| OTD-02 | Off-track detection via isValidLap transition | unit | `cargo test -p rc-agent-crate -- off_track` | Yes (3 tests exist) |
| OTD-03 | Blanking shows within 500ms of off-track | unit + manual | Unit: debounce timing test; Manual: stopwatch on pod | Partial (debounce tests exist, timing test needed) |
| OTD-04 | Blanking hides within 500ms on return | unit + manual | Same as OTD-03 | Partial |
| OTD-05 | Off-track blanking configurable via feature flag | unit | `cargo test -p rc-agent-crate -- feature_flag` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate -- off_track`
- **Per wave merge:** Full suite
- **Phase gate:** Full suite green + visual verification on Pod 8

### Wave 0 Gaps
- [ ] Add test for debounce timing (300ms on, 500ms off) in off_track_detector.rs
- [ ] Add test for blanking window show/hide state transitions
- [ ] Add test for feature flag gating of off_track detection
- [ ] Add test for pit lane suppression (if isInPit check added)

## Open Questions

1. **isValidLap offset accuracy**
   - What we know: Offset 180 is used, documented as "approximate -- may need correction". The real SPageFileGraphic.isValidLap in AC is deep in the struct (~1408+).
   - What's unclear: Whether offset 180 actually reads the correct field or garbage data.
   - Recommendation: Empirical test on a pod -- go off-track in AC, log `current_lap_invalid` values. If unreliable, read from RC plugin shared memory or increase the graphics shared memory buffer size to reach the real offset.

2. **RC Plugin shared memory reliability**
   - What we know: `rcpmf_telemetry` is an alternative AC shared memory source (assetto_corsa.rs line 39-46). When the RC plugin is installed, telemetry reads from it instead.
   - What's unclear: Whether the RC plugin exposes isValidLap more reliably than raw AC shared memory.
   - Recommendation: Check RC plugin source/documentation for its shared memory layout.

3. **Blanking window vs reuse of overlay window**
   - What we know: Overlay is a 96px bar. Blanking needs full-screen.
   - What's unclear: Should the overlay window resize to full-screen during blanking, or use a separate window?
   - Recommendation: Use a separate window. Resizing the HUD bar to full-screen and back would require re-creating the paint layout, and any failure leaves the HUD broken. A separate window is simpler and isolates failures.

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/overlay.rs` -- Complete working Win32 GDI HUD overlay (1370+ lines)
- `crates/rc-agent/src/off_track_detector.rs` -- Complete debounced off-track state machine (150 lines)
- `crates/rc-agent/src/native_lock/` -- Full-screen Win32 GDI rendering module (Phase 329)
- `crates/rc-agent/src/sims/assetto_corsa.rs` -- AC shared memory reader with isValidLap
- `crates/rc-common/src/types.rs` -- TelemetryFrame with current_lap_invalid field
- `.planning/phases/329-native-win32-lockscreen/329-01-SUMMARY.md` -- Phase 329 outcomes
- `.planning/phases/329-native-win32-lockscreen/329-RESEARCH.md` -- Win32 GDI patterns research

### Secondary (MEDIUM confidence)
- AC shared memory struct layout -- based on community documentation and code comments

### Tertiary (LOW confidence)
- `IS_VALID_LAP` offset 180 accuracy -- marked as approximate in source code

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- all patterns proven in existing codebase (overlay.rs, native_lock/, off_track_detector.rs)
- Pitfalls: HIGH -- documented from codebase analysis and existing Phase 329 research
- isValidLap offset: LOW -- documented as approximate, needs empirical validation

**Research date:** 2026-04-07
**Valid until:** 2026-05-07 (stable -- no external dependency changes expected)
