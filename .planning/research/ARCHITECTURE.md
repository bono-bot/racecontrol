# Architecture Research — Racing HUD Redesign & FFB Safety

**Date:** 2026-03-11
**Scope:** Subsequent milestone layered on top of the existing rc-agent codebase.
**Files studied:** overlay.rs (848 lines), driving_detector.rs, main.rs, ac_launcher.rs,
assetto_corsa.rs, sims/mod.rs, rc-common/types.rs

---

## 1. Overlay Architecture

### 1.1 Current layout — what exists today

`overlay.rs` renders a single full-width horizontal bar (1920×96 px) at the top of
the screen. The `paint_hud` function is monolithic: one unsafe GDI function (~330
lines) that hard-codes six sequential sections separated by vertical dividers.

```
Section widths (px):  [120, 200, 100, 200, 200, 60]
Sections (left→right): SESSION | CURRENT LAP | GEAR/SPEED | PREV | BEST | LAP#
```

The section widths are fixed constants embedded inside `paint_hud`. Layout is
computed once at the top of the function via:

```rust
let total_content: i32 = section_widths.iter().sum();  // 880px
let start_x = (w - total_content).max(0) / 2;          // centred horizontally
```

Painting then walks `sx` (left edge) forward by `section_widths[i]` after each
section, calling `draw_text_at` with absolute pixel coordinates.

### 1.2 Problem with the monolithic approach

All layout math is baked into one function body. Adding a new section, changing a
width, or switching from horizontal-bar to centered-Essentials requires touching
dozens of coordinate literals. Refactoring is error-prone and the function will
exceed ~500 lines if new content is added naively.

### 1.3 Recommended: component-based painting

Split `paint_hud` into a dispatcher that drives an array of component closures or
trait objects. Each component owns its own layout rectangle and paints itself.

**Proposed component boundary:**

```
paint_hud(hwnd, data)
  ├── compute_layout(screen_w, screen_h) → Vec<SectionRect>
  └── for each SectionRect:
        component.paint(mem_dc, rect, data)

Components:
  SessionTimerComponent   → draws remaining/allocated time, colour-coded
  CurrentLapComponent     → lap time + live S1/S2/S3 with F1 colour coding
  GearSpeedComponent      → gear (large), speed, RPM
  PrevLapComponent        → last completed lap + sector splits
  BestLapComponent        → personal best + sector splits (purple)
  LapCounterComponent     → lap number + INV badge
  RpmBarComponent         → full-width colour bar at top (not a section, painted first)
```

A `SectionRect` is simply:

```rust
struct SectionRect {
    x: i32, y: i32,
    w: i32, h: i32,
}
```

`compute_layout` is the only function that knows about screen size and section
weights. Changing from horizontal bar to "centered Essentials" (e.g. only showing
SESSION + LAP + GEAR in the center of the screen when in a different display mode)
is then a matter of returning a different `Vec<SectionRect>` from `compute_layout`
rather than editing painting code.

### 1.4 Layout calculation approach for "centered Essentials"

The Essentials layout is a subset of sections displayed in the screen center,
rather than as a full-width bar. Proposed approach:

```
Normal bar:   [SESSION | CUR_LAP | GEAR/SPEED | PREV | BEST | LAP#]  ← full width
Essentials:              [CUR_LAP | GEAR/SPEED | SESSION]              ← centered, narrower
```

`compute_layout` accepts a `LayoutMode` enum:

```rust
enum LayoutMode {
    FullBar,        // current behaviour
    Essentials,     // narrower centered strip (e.g. 480px wide)
}
```

`compute_layout(mode, screen_w, screen_h)` returns the appropriate `Vec<SectionRect>`.
The component array is always the same; components that have no rect in a given mode
simply skip painting.

### 1.5 GDI resource management

Current code creates 8 font handles per frame and deletes them inside the same
`paint_hud` call. This is correct but wasteful — `CreateFontW` is not free. The
refactoring should cache font handles in a `GdiResources` struct stored on the
window user-data alongside `OverlayData`:

```rust
struct WindowState {
    data: Arc<Mutex<OverlayData>>,
    fonts: GdiResources,   // created once in WM_CREATE, destroyed in WM_DESTROY
}
```

`GdiResources` holds all `HFONT` and shared `HPEN`/`HBRUSH` values. Components
receive `&GdiResources` alongside the HDC. This eliminates per-frame alloc/free
cycles for the 8 fonts.

---

## 2. FFB Integration Architecture

### 2.1 What "FFB write" means for the Conspit Ares

The Conspit Ares runs OpenFFBoard firmware. FFB (Force Feedback) output is driven
by the game via DirectInput/FFB host calls — the game controls force magnitude
through the standard FFB pipeline. The _wheelbase_ gain is configured via:

1. **At launch time (file write):** `ac_launcher::set_ffb()` writes `[FF] GAIN=<n>`
   to `Documents/Assetto Corsa/cfg/controls.ini`. AC reads this at session start.
2. **Emergency zero-force (runtime):** To guarantee the motor stops on session end,
   a zero-force command must be sent to the OpenFFBoard's USB HID endpoint before
   `taskkill` is called. If AC is killed first, its FFB host thread exits and the
   motor may be left at whatever force AC last commanded.

### 2.2 Where FFB write calls should live

**Do NOT add FFB write logic to `driving_detector.rs`.** That module is HID
input-only (read path). Mixing write responsibility would break the single-
responsibility principle and create shared-state hazards (both reading and writing
to the same HID device from the same thread without coordination).

**Correct home: a new `ffb_controller.rs` module** with a simple public API:

```rust
pub struct FfbController {
    vid: u16,
    pid: u16,
}

impl FfbController {
    pub fn new(vid: u16, pid: u16) -> Self { ... }

    /// Send zero-force to the wheelbase. Blocks up to 100ms.
    /// Call BEFORE taskkill to guarantee the motor stops.
    pub fn zero_force(&self) -> Result<()> { ... }

    /// Set gain level (0..100). Used for preset changes mid-session.
    pub fn set_gain(&self, gain: u8) -> Result<()> { ... }
}
```

`FfbController` is instantiated in `main.rs` alongside `DrivingDetector` and stored
in the same async scope. It does NOT run on a separate thread — it is called
synchronously (via `spawn_blocking` if needed) at specific lifecycle events:

```
BillingStarted received   → (no FFB action — gain already set in controls.ini at launch)
SessionEnded received     → zero_force() THEN enforce_safe_state() THEN taskkill
StopGame received         → zero_force() THEN game.stop()
crash_recovery_timer fires → zero_force() THEN enforce_safe_state()
```

### 2.3 Ordering guarantee: zero-force before process kill

The ordering constraint is:

```
1. zero_force()      — HID write, blocks ≤100ms
2. taskkill acs.exe  — kills FFB host in AC; motor now safe because step 1 already ran
```

This ordering is enforced by keeping both calls in the same sequential code path in
`main.rs`. Since `zero_force()` is a blocking call, it must run in `spawn_blocking`.
The existing pattern already does this for `enforce_safe_state()`:

```rust
// Pattern already used in main.rs — replicate for FFB:
tokio::task::spawn_blocking(|| {
    ffb.zero_force().ok();          // step 1 — HID write
    ac_launcher::enforce_safe_state(); // step 2 — kills game
}).await.ok();
```

`enforce_safe_state()` already calls `taskkill` for all game processes. By placing
`zero_force()` before it inside the same `spawn_blocking` closure, the ordering is
deterministic and synchronous — no channels, no timers, no race conditions.

### 2.4 HID device sharing between DrivingDetector and FfbController

Both `driving_detector` (read) and `ffb_controller` (write) access the same USB
device. On Windows, HID devices opened with `FILE_FLAG_OVERLAPPED` and no exclusive
share flag allow multiple simultaneous readers but only one writer at a time.
OpenFFBoard's HID interface (output report) is a separate endpoint from the input
report, so they do not conflict at the USB level.

To be safe, the two modules should use separate device handles opened independently:
- `driving_detector`'s HID handle is opened read-only (no output reports sent)
- `ffb_controller`'s HID handle is opened for write (output reports only)

There is no need for a shared mutex on the device handle as long as the two handles
are opened to different report types.

### 2.5 Thread model

```
tokio async runtime (main loop)
  ├── HID monitor task (spawn_blocking, loops on hidapi::read_timeout)
  │     → sends DetectorSignal via mpsc to main loop
  ├── UDP monitor task (tokio::spawn, UDP socket listener)
  │     → sends DetectorSignal via mpsc to main loop
  ├── Win32 window thread (std::thread::spawn, GetMessage loop)
  │     → reads Arc<Mutex<OverlayData>>, no write from this thread
  └── main event loop (tokio::select!)
        → FfbController called here via spawn_blocking at lifecycle events
```

`FfbController` is neither in the HID monitor thread nor in the Win32 window thread.
It lives in the main loop scope and is dispatched via `spawn_blocking` only when
needed (session end, crash recovery). This eliminates all contention.

---

## 3. Data Flow for Game Timer Sync

### 3.1 AC shared memory fields relevant to time

From `assetto_corsa.rs`, the graphics shared memory exposes:

| Field | Offset | Type | Meaning |
|-------|--------|------|---------|
| `I_CURRENT_TIME` | 140 | i32 | Current lap elapsed time in ms |
| `I_LAST_TIME`    | 144 | i32 | Last completed lap total time in ms |
| `I_BEST_TIME`    | 148 | i32 | Session best lap time in ms |
| `COMPLETED_LAPS` | 132 | i32 | Lap counter — increments on crossing finish line |

The `TelemetryFrame` populated by `AssettoCorsaAdapter::read_telemetry()` uses:

```rust
lap_time_ms:    i_current_time   // current lap stopwatch
session_time_ms: lap_time_ms     // NOTE: currently aliased — same field
```

`session_time_ms` is set to `lap_time_ms` (line 329 in `assetto_corsa.rs`). This
means there is **no genuine session-elapsed field** being plumbed from AC's shared
memory. AC does not expose total session elapsed time in `acpmf_graphics` by default;
it exposes only current-lap time.

### 3.2 The timer drift problem

The overlay's session countdown (`remaining_seconds`) is driven by `BillingTick`
messages from rc-core arriving over WebSocket. rc-core counts down from the
purchased allocation using wall-clock time. AC's `I_CURRENT_TIME` is the game's
own lap timer — driven by the simulation engine, which can pause (during replays,
loading screens, pit stops) independently of wall-clock time.

Drift sources:
1. **Network jitter:** `BillingTick` arrives every second. If a tick is late or
   dropped, the overlay's display lags by up to 1–2 seconds.
2. **Pause state:** AC pauses its lap timer when the game is paused or in the menu.
   Wall-clock billing continues. The overlay session timer would show a different
   value than the game's in-session display if the player pauses.
3. **Clock skew:** The overlay timer is decremented by `BillingTick` events (server
   authoritative), not by local polling. It is inherently authoritative but coarse.

### 3.3 Recommended approach: keep billing timer as source of truth

The session countdown displayed on the overlay must match billing reality. Using
`remaining_seconds` from `BillingTick` is correct. Do NOT replace it with a
locally-interpolated timer derived from `I_CURRENT_TIME`.

To eliminate jitter-induced flicker on the display, apply client-side interpolation:

```rust
// In OverlayData:
remaining_seconds: u32,          // authoritative — set by BillingTick
last_tick_wall_time: Instant,    // when the last BillingTick arrived

// In paint_hud, compute display value:
let elapsed_since_tick = last_tick_wall_time.elapsed().as_secs() as u32;
let display_seconds = remaining_seconds.saturating_sub(elapsed_since_tick);
```

This gives smooth second-by-second counting even if ticks arrive slightly late,
while keeping the server value as the source of truth. At 200ms repaint interval,
the display is always within 200ms of correct.

### 3.4 AC game timer available in TelemetryFrame

For the current-lap stopwatch shown on the overlay (not the session countdown),
`TelemetryFrame.lap_time_ms` (= `I_CURRENT_TIME`) is the right source. It is
updated every 100ms by the telemetry polling loop and reflects AC's own timer.
No drift issue here — the overlay lap timer is driven from the same source as AC's
on-screen lap timer.

**Summary: two timers, two sources:**
- Session countdown → `BillingTick.remaining_seconds` (server authoritative) + local interpolation
- Current lap stopwatch → `TelemetryFrame.lap_time_ms` (AC shared memory, 100ms poll)

### 3.5 Data flow diagram

```
AC process (acs.exe)
│
│  acpmf_physics     ──→ read_f32(speed, throttle, brake, steering, rpm, gear)
│  acpmf_graphics    ──→ read_i32(I_CURRENT_TIME, COMPLETED_LAPS, CURRENT_SECTOR,
│                                  LAST_SECTOR_TIME, IS_IN_PIT, STATUS)
│  acpmf_static      ──→ read_wchar(CAR_MODEL, TRACK, PLAYER_NAME) [once on connect]
│
▼
AssettoCorsaAdapter::read_telemetry()   [100ms interval, tokio event loop]
│  ├── builds TelemetryFrame
│  └── on lap completion: builds LapData → pending_lap
│
▼
main.rs telemetry_interval tick
│  ├── overlay.update_telemetry(&frame)   → writes OverlayData.{speed,gear,rpm,lap_time_ms,...}
│  ├── adapter.poll_lap_completed()
│  │     └── overlay.on_lap_completed(&lap) → updates previous_lap, best_lap in OverlayData
│  └── AgentMessage::Telemetry(frame)    → WebSocket → rc-core
│
▼
rc-core (billing loop, every 1s)
│  └── CoreToAgentMessage::BillingTick { remaining_seconds }
│
▼
main.rs WS receive
│  └── overlay.update_billing(remaining_seconds)  → writes OverlayData.remaining_seconds
│
▼
OverlayManager::state (Arc<Mutex<OverlayData>>)
│
▼
Win32 window thread (WM_TIMER, every 200ms)
│  └── paint_hud(hwnd, data)   → GDI rendering
```

---

## 4. Build Order

The FFB safety and HUD redesign have partially independent dependency chains, but
share a sequencing constraint: the HUD component refactor must not break existing
tests before FFB is added.

### 4.1 Dependency graph

```
[A] GdiResources cache       ← no deps; can build first
[B] component trait + SectionRect ← depends on OverlayData (already exists)
[C] compute_layout() + LayoutMode ← depends on [B]
[D] Individual paint components    ← depends on [A], [B], [C], GdiResources
[E] Refactor paint_hud dispatcher  ← depends on [D]; replaces monolithic fn
    (characterization tests pass before and after this step)

[F] FfbController module     ← no deps on overlay; parallel track
[G] zero_force() wired into main.rs lifecycle ← depends on [F]
[H] set_gain() used in ac_launcher::set_ffb fallback ← depends on [F]

[I] BillingTick interpolation in OverlayData ← depends on [E] being done
[J] End-to-end test: session start → overlay active → BillingTick ticks → session end → zero_force ← depends on [G], [I]
```

### 4.2 Recommended build sequence

**Phase 1 — Characterization tests (test-first mandate)**

Write tests against the current monolithic `paint_hud` behavior BEFORE touching it.
The only practical unit-testable surface is the formatting helpers and layout math:

```rust
// Already present — verify all pass before proceeding:
format_timer(90)   → "01:30"
format_lap_time(0) → "--:--.---"
format_sector(Some(32100)) → "32.1"
sector_color(...)  → correct colour enum
```

Add a layout math test:

```rust
let widths = [120, 200, 100, 200, 200, 60]; // current
let total: i32 = widths.iter().sum();
assert_eq!(total, 880);
let start_x = (1920 - total) / 2;
assert_eq!(start_x, 520);
```

**Phase 2 — GdiResources cache [A]**

Extract font creation into a `GdiResources` struct. Store it in `WindowState` via
`SetWindowLongPtrW`. No behavioral change — just moves font alloc from paint-time
to WM_CREATE. Verify with existing repaint behavior on a pod before proceeding.

**Phase 3 — Component trait + SectionRect [B, C, D]**

Define the `HudComponent` trait:

```rust
trait HudComponent {
    fn paint(&self, hdc: HDC, rect: SectionRect, data: &OverlayData, res: &GdiResources);
}
```

Implement each existing section as a component. `compute_layout(FullBar, 1920, 96)`
must return the same rects as the old hard-coded constants:
`SectionRect { x: 520, w: 120 }`, `{ x: 640, w: 200 }`, etc.

**Phase 4 — Refactor paint_hud dispatcher [E]**

Replace the monolithic match statement with:

```rust
let rects = compute_layout(layout_mode, w, h);
for (component, rect) in COMPONENTS.iter().zip(rects.iter()) {
    component.paint(mem_dc, *rect, &data, &res);
}
```

Run characterization tests. Run `cargo test -p rc-agent`. Deploy to Pod 8 only and
verify HUD renders identically to before.

**Phase 5 — FfbController module [F]**

New file: `crates/rc-agent/src/ffb_controller.rs`. Implement `zero_force()` using
`hidapi`. No changes to `main.rs` yet. Write a unit test verifying that the function
returns `Ok(())` when no device is connected (graceful degradation — pods without a
wheelbase should not panic):

```rust
#[test]
fn zero_force_no_device_is_ok() {
    let ctrl = FfbController::new(0x1209, 0xFFB0);
    // With no device, should return Err but not panic
    let _ = ctrl.zero_force(); // either Ok or Err, never panic
}
```

**Phase 6 — Wire FFB into main.rs lifecycle [G]**

Add `FfbController` to `main.rs`. Insert `zero_force()` calls at the three lifecycle
points (SessionEnded, StopGame, crash_recovery_timer) following the ordering rule:
`zero_force()` inside the same `spawn_blocking` closure, before `enforce_safe_state()`.

**Phase 7 — BillingTick interpolation [I]**

Add `last_tick_wall_time: Instant` to `OverlayData`. Update `update_billing()` to
record `Instant::now()`. Update `paint_hud` to compute display seconds via
`remaining_seconds.saturating_sub(last_tick_wall_time.elapsed().as_secs() as u32)`.

Write a deterministic test:

```rust
// Simulate a tick arriving, then 1200ms passing
data.remaining_seconds = 60;
data.last_tick_wall_time = Instant::now() - Duration::from_millis(1200);
let displayed = data.display_seconds();
assert_eq!(displayed, 59); // 60 - 1 = 59
```

**Phase 8 — Essentials layout mode [C final]**

Implement `compute_layout(Essentials, screen_w, screen_h)` returning the centered
3-section layout. Wire a `layout_mode` field into `OverlayData` so it can be set at
activate time. Test on Pod 8.

**Phase 9 — End-to-end validation [J]**

Full session walkthrough on Pod 8:
1. Session started → overlay appears, HUD renders correctly
2. BillingTick fires → countdown decrements smoothly
3. Customer drives → speed/gear/RPM/lap times update
4. Lap completed → previous_lap + best_lap panels populate
5. Session ended → `zero_force()` logged, overlay closes, lock screen shows summary
6. No motor runaway observed after taskkill (confirmed via Conspit Link UI)

---

## 5. Component Boundaries Summary

```
ac_launcher.rs
  set_ffb()       ← file-based, pre-launch; keep as-is
  enforce_safe_state() ← calls taskkill; always called AFTER zero_force()

ffb_controller.rs  [NEW]
  zero_force()    ← HID output report; blocking; called from spawn_blocking
  set_gain()      ← HID output report; optional real-time gain change

driving_detector.rs
  HID READ only   ← no write path; no change

overlay.rs
  GdiResources    ← font/pen cache; created once per window
  SectionRect     ← layout primitive
  LayoutMode      ← FullBar | Essentials
  compute_layout()← sole owner of geometry
  HudComponent    ← trait: paint(hdc, rect, data, res)
  {6 components}  ← SessionTimer, CurrentLap, GearSpeed, PrevLap, BestLap, LapCounter
  RpmBar          ← full-width; not a SectionRect component; painted before dispatch
  paint_hud()     ← dispatcher only; ~30 lines after refactor
  OverlayManager  ← public API; unchanged signatures

main.rs
  FfbController   ← instantiated alongside DrivingDetector
  lifecycle events ← zero_force() + enforce_safe_state() in correct order
  BillingTick     ← calls overlay.update_billing(); overlay interpolates internally
```

---

## 6. Key Risk Areas

**Risk: WM_DESTROY during active billing**
If the window is closed while billing is active (e.g. kiosk enforcement kills the
window), `OverlayData.active` remains true but there is no HWND to paint. The
existing `close_window()` sets the HWND slot to `None` before joining the thread.
`enforce_topmost()` already checks `hwnd_guard` for `None`. No regression expected,
but add an assertion in `activate()` that calls `close_window()` first — already
done via `open_window()` calling `self.close_window()`.

**Risk: hidapi exclusivity for FFB write**
If `hidapi` opens the OpenFFBoard device in exclusive mode, `driving_detector`'s
read loop will fail. Test with `hidapi::HidApi::open_path()` vs `open()` on the
actual pod hardware before committing the design. Fallback: serialize read and write
through a single `Arc<Mutex<HidDevice>>` owned by a new `WheelbaseManager` module
that both `DrivingDetector` and `FfbController` receive references to.

**Risk: Rust HID write on Windows requiring elevated permissions**
OpenFFBoard HID output reports may require the process to have write access to the
HID device. rc-agent currently runs without explicit elevation. Test `zero_force()`
on a real pod and verify no `AccessDenied` error before declaring Phase 5 done.

**Risk: session_time_ms aliased to lap_time_ms**
Any future feature that relies on `TelemetryFrame.session_time_ms` for total session
elapsed time will get wrong data (it currently contains current-lap time). Document
this in a code comment in `assetto_corsa.rs`. If total session elapsed is needed
(e.g. for a session-length progress bar), use `allocated_seconds - remaining_seconds`
derived from `BillingTick`, not `session_time_ms`.
