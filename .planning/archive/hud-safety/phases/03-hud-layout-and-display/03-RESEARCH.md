# Phase 3: HUD Layout and Display — Research

**Researched:** 2026-03-12
**Domain:** Win32 GDI overlay layout, AC Essentials-style HUD, monospace font rendering, RPM bar color zones, sector/lap time display
**Confidence:** HIGH

## Summary

Phase 3 redesigns the overlay from the current 6-section horizontal bar (880px centered) into an AC Essentials-inspired centered layout with large gear indicator (60-80pt), full-width RPM bar (8-12px), and organized timing data. The foundation is already solid: Phase 2 delivered `GdiResources` caching (13 handles), `HudComponent` trait with 7 implementations, `HudRenderer` dispatcher, `TempBrush` RAII, `SectionRect` layout system, `compute_layout()` pure function, and characterization tests. Phase 3 modifies the existing component implementations and layout computation -- no new architectural patterns needed.

The primary challenge is layout arithmetic: repositioning elements from a 96px-tall bar to a centered Essentials-style arrangement where gear is prominent (60-80pt), speed is below gear, RPM bar spans full width at 8-12px height, and timing data (laps, sectors, session timer) is arranged without overlap. All numeric values must use Consolas monospace font to prevent layout jitter. The RPM bar already works with dynamic `max_rpm` from AC shared memory -- only the height needs adjustment (4px to 8-12px). Sector time coloring (purple/green/yellow) already works via `sector_color()`.

**Primary recommendation:** Modify `compute_layout()` to return the Essentials layout geometry, update each `HudComponent` implementation to use Consolas for numbers and larger font sizes, expand the RPM bar height. No new crates, no new modules, no architectural changes.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| HUD-01 | Redesign to AC Essentials centered layout -- large gear (60-80pt), speed below gear, data organized around center | Existing `compute_layout()` returns `Vec<SectionRect>` -- modify to produce Essentials geometry. Existing `GearSpeedSection` component is the target for 60-80pt gear font. |
| HUD-02 | Full-width RPM bar (8-12px), fills L-to-R with green/yellow/amber/red zones, dynamic `max_rpm` from shared memory | Existing `RpmBarSection` already reads `data.max_rpm` and paints full-width with color zones. Change height from 4px to 8-12px. Color thresholds already implemented (green <50%, yellow 50-75%, amber 75-90%, red >90%). |
| HUD-03 | Display current lap time (`iCurrentTime`), previous lap (`iLastTime`), best (`iBestTime`) | Already implemented: `CurrentLapSection` shows current, `PrevLapSection` shows previous, `BestLapSection` shows best. Format via `format_lap_time()`. Reposition for Essentials layout. |
| HUD-04 | Sector times S1/S2/S3 with F1 color coding (purple/green/yellow) | Already implemented in `CurrentLapSection` and `PrevLapSection` via `sector_color()`. Uses purple=#A855F7, green=#22C55E, yellow=#F59E0B. |
| HUD-05 | Session timer from AC `sessionTimeLeft` field | Current overlay shows billing `remaining_seconds` via `SessionTimerSection`. AC's `sessionTimeLeft` (offset 152 in acpmf_graphics) is available in the shared memory but NOT currently plumbed to overlay. See "session timer source" in architecture section. |
| HUD-06 | Lap counter (current lap number) | Already implemented in `LapCounterSection`. Reads `data.current_lap_number`. Reposition for Essentials layout. |
| HUD-07 | Invalid lap indicator | Already implemented: `CurrentLapSection` draws red bar + red-tinted lap time on invalid; `LapCounterSection` draws "INV" badge. Reposition for Essentials layout. |
| HUD-08 | Speed display in KM/H, repositioned for Essentials layout | Already in `GearSpeedSection`. Speed formatted as integer, "KM/H" label below. Reposition per Essentials geometry. |
| HUD-09 | Consolas monospace font for all numeric values to prevent layout jitter | Currently all fonts are "Segoe UI" (proportional). Change all numeric-value fonts in `GdiResources::new()` from "Segoe UI" to "Consolas". Keep labels as "Segoe UI". |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| winapi | 0.3 | Win32 GDI API (CreateFontW, TextOutW, FillRect, BitBlt) | Already in use, stable, no alternative needed for GDI |
| Rust std | 1.93 | Core language, formatting, string handling | Workspace standard |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| rc-common | workspace | TelemetryFrame, LapData types | Already imported by overlay.rs |
| tracing | workspace | Logging (info/warn/debug) | Already in use for GDI handle monitoring |

### No New Dependencies
Phase 3 requires **zero new crate dependencies**. All rendering uses existing winapi GDI functions. The "Consolas" font is a system font shipped with every Windows installation since Vista.

## Architecture Patterns

### Current Overlay Architecture (from Phase 2)

```
overlay.rs (~1,324 lines)
  Types:
    OverlayData         -- shared state (Arc<Mutex<>>), written by main loop, read by paint
    LapRecord           -- completed lap with sector times + valid flag
    SectionRect         -- layout rectangle {x, y, w, h}
    GdiResources        -- 13 cached GDI handles (9 fonts, 1 pen, 3 brushes)
    TempBrush           -- RAII wrapper for per-frame dynamic brushes
    WindowState         -- data + res + renderer + gdi_baseline + timer_tick
    HudRenderer         -- dispatcher with Vec<Box<dyn HudComponent>>

  Components (6 sections + 1 full-width):
    SessionTimerSection  -- billing countdown timer
    CurrentLapSection    -- current lap time + live S1/S2/S3
    GearSpeedSection     -- gear (32px), speed (16px), RPM number
    PrevLapSection       -- previous lap time + sector times
    BestLapSection       -- best lap time + sector times (purple)
    LapCounterSection    -- lap number + INV badge
    RpmBarSection        -- full-width bar (4px tall, NOT a HudComponent)

  Layout:
    compute_layout(window_width) -> Vec<SectionRect>
    Section widths: [120, 200, 100, 200, 200, 60] = 880px total, centered
    Window: 1920x96 px, top of screen

  Paint:
    paint_hud(hwnd, data, res, renderer)
      -> double buffer (mem_dc + BitBlt)
      -> bg fill -> renderer.paint_all()
      -> renderer: rpm_bar -> red borders -> dividers -> section loop
```

### Phase 3 Target Layout (Essentials Style)

The Essentials layout keeps the same components but changes geometry and font sizing. The window height needs to increase to accommodate the 60-80pt gear indicator and additional timing rows.

**Target window dimensions:** 1920 x 140 px (increased from 96px to fit larger gear + timing rows below)

**Target visual arrangement:**

```
+-----------------------------------------------------------------------+
| [RPM BAR - full width, 10px tall, fills L->R with color zones]        |
| [RED ACCENT - 2px]                                                    |
|                                                                       |
|   SESSION       CURRENT LAP        [GEAR]        PREV        BEST    |
|   01:30         1:23.456           [  3  ]       1:24.789    1:22.100 |
|                 S1  S2  S3         [247 KM/H]    S1  S2  S3  S1 S2 S3|
|                 32.1 28.4 22.1                   32.5 29.1 23.2      |
|                                    LAP 5 [INV]                        |
|                                                                       |
| [RED ACCENT - 2px]                                                    |
+-----------------------------------------------------------------------+
```

**Key changes from current layout:**
1. **BAR_HEIGHT:** 96 -> 140 (accommodate 60-80pt gear + rows)
2. **RPM bar:** 4px -> 10px height
3. **Gear font:** 32px -> 72px (centered in its section)
4. **Speed:** positioned below gear, larger (16px -> 20px)
5. **All numeric fonts:** "Segoe UI" -> "Consolas"
6. **Section widths:** Adjust for Essentials proportions (gear section wider)
7. **Lap counter:** Moved below gear/speed area

### Pattern: Modify compute_layout() for New Geometry

```rust
fn compute_layout(window_width: i32) -> Vec<SectionRect> {
    // Essentials layout: wider gear section, adjusted proportions
    let section_widths: [i32; 6] = [140, 260, 160, 240, 240, 0];
    // Lap counter (index 5) is now integrated into gear section
    // Total = 1040px (still centered)
    let total_content: i32 = section_widths.iter().sum();
    let start_x = (window_width - total_content).max(0) / 2;
    // ... build rects
}
```

### Pattern: Font Changes in GdiResources

```rust
impl GdiResources {
    unsafe fn new() -> Self {
        Self {
            // Labels stay Segoe UI (proportional OK for "SESSION", "BEST", etc.)
            font_label: create_font(null_hdc, "Segoe UI", 11, true),

            // ALL numeric values -> Consolas (monospace prevents jitter)
            font_value: create_font(null_hdc, "Consolas", 24, true),    // was Segoe UI 22
            font_gear: create_font(null_hdc, "Consolas", 72, true),     // was Segoe UI 32
            font_speed: create_font(null_hdc, "Consolas", 20, true),    // was Segoe UI 16
            font_lap: create_font(null_hdc, "Consolas", 20, true),      // was Segoe UI 18
            font_sector: create_font(null_hdc, "Consolas", 13, true),   // was Segoe UI 12
            font_sector_label: create_font(null_hdc, "Segoe UI", 10, false), // labels stay
            font_unit: create_font(null_hdc, "Segoe UI", 9, false),     // "KM/H" label stays
            font_badge: create_font(null_hdc, "Segoe UI", 9, true),     // "INV" label stays
            // pens and brushes unchanged
        }
    }
}
```

### Anti-Patterns to Avoid

- **Multiple layout modes:** Do NOT implement a `LayoutMode` enum (FullBar vs Essentials) at this stage. The roadmap says "redesign" not "add option." Replace the old layout, do not maintain two. Simplifies testing and eliminates dead code.
- **New modules:** Do NOT create separate files for components. All HUD code belongs in `overlay.rs`. The component trait is internal to this file.
- **Dynamic font creation:** Do NOT create fonts per-component. All fonts are cached in `GdiResources` and shared. If a component needs a new size, add it to `GdiResources`.
- **Text measurement for centering:** Do NOT use `GetTextExtentPoint32W` inside the hot paint loop for every frame. Consolas is monospace -- calculate character width once and use arithmetic. Exception: gear digit needs centering within its section, which can use a one-time measurement at GdiResources init.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Monospace alignment | Custom character-width table | Consolas font with fixed-width property | System font, guaranteed monospace, CLEARTYPE_QUALITY |
| Color zone RPM | Complex gradient calculation | Fixed threshold checks (existing) | 4 color zones is sufficient; gradients would require per-pixel drawing |
| F1 sector colors | Custom color state machine | Existing `sector_color()` function | Already tested (5 characterization tests), handles all cases |
| Layout centering | Manual pixel arithmetic per component | `compute_layout()` returning `SectionRect` | Single source of truth for all geometry |
| Timer formatting | Custom time string building | Existing `format_timer()`, `format_lap_time()`, `format_sector()` | Already tested (15 characterization tests) |

## Common Pitfalls

### Pitfall 1: Gear Font Too Large for Window Height
**What goes wrong:** A 72pt font in GDI means 72 logical units. With CLEARTYPE at 96 DPI, this renders approximately 72 pixels of character height. The current window is only 96px tall (minus 4px RPM bar, 4px borders = 88px usable). A 72px gear digit plus speed text below it will not fit.
**Why it happens:** BAR_HEIGHT was set for the old layout with 32px gear. Not updated for Essentials.
**How to avoid:** Increase BAR_HEIGHT from 96 to 140-150px. Verify on Pod 8 that the overlay does not obscure critical game UI (AC's built-in apps default to top-left, our overlay is top-center; no overlap expected).
**Warning signs:** Gear digit truncated at bottom, speed text not visible.

### Pitfall 2: Consolas Font Width Differences from Segoe UI
**What goes wrong:** Switching from Segoe UI (proportional) to Consolas (monospace) changes the width of every character. "1:23.456" in Consolas is wider than in Segoe UI at the same point size because each digit gets the same width (including narrow digits like "1"). Section widths that fit with Segoe UI may overflow with Consolas.
**Why it happens:** Monospace fonts allocate equal width per character. The section width constants were designed for proportional fonts.
**How to avoid:** After switching to Consolas, measure the maximum-width string for each section (e.g., "59:59" for timer, "9:59.999" for lap time) at the chosen font size and ensure the section width accommodates it. Adjust `section_widths` in `compute_layout()` accordingly.
**Warning signs:** Text overflows section boundaries, overlapping with adjacent section.

### Pitfall 3: RPM Bar Height Change Shifts All Y Coordinates
**What goes wrong:** The RPM bar is currently 4px (top 0-4). Changing to 10px means all content Y coordinates shift by 6px. The red accent border at y=4 becomes y=10. Component `rect.y` starts at 12 -- if RPM bar is now 10px, the accent border (2px) puts content at y=14.
**Why it happens:** Y offsets are hardcoded in `compute_layout()` (y=12) and in component paint methods.
**How to avoid:** Define constants for RPM_BAR_HEIGHT, ACCENT_HEIGHT, and compute content_y = RPM_BAR_HEIGHT + ACCENT_HEIGHT + PADDING. Pass content_y through SectionRect.y so components use it automatically.
**Warning signs:** Content overlaps RPM bar; gap between RPM bar and content is wrong.

### Pitfall 4: Stale Layout After Window Resize
**What goes wrong:** `compute_layout()` is called every paint with `window_width`, which is correct. But BAR_HEIGHT is a compile-time constant used in `CreateWindowExW`. If BAR_HEIGHT changes but the window was already created at the old size, repaints use the new height math against the old window size.
**Why it happens:** BAR_HEIGHT constant is used both in window creation and in layout computation.
**How to avoid:** Only change BAR_HEIGHT once, before Phase 3 code is deployed. Do not try to resize the window at runtime -- just change the constant and rebuild.
**Warning signs:** Content renders outside the window bounds (invisible, but wastes GPU compositing).

### Pitfall 5: Session Timer Source Confusion
**What goes wrong:** HUD-05 says "session timer from AC `sessionTimeLeft` field." But the overlay's current `remaining_seconds` comes from `BillingTick` messages (billing countdown, server-authoritative). AC's `sessionTimeLeft` is a different timer (game-session time, pauses when game pauses). Using AC's value would show wrong billing time.
**Why it happens:** AC `sessionTimeLeft` and billing `remaining_seconds` are different concepts that look similar.
**How to avoid:** Keep billing `remaining_seconds` as the primary timer (it IS the source of truth for "how much time did the customer pay for"). If HUD-05 means showing AC's game session time as a separate display, add it as a secondary timer. Clarify with Uday. The STATE.md known issue says "Timer not synced: HUD timer starts before game launches" -- the fix is `game_live` flag which is already implemented (lines 735-738 of overlay.rs).
**Warning signs:** Timer shows wrong value, does not match billing, customer confusion about remaining time.

### Pitfall 6: Invalid Laps Not Displayed
**What goes wrong:** STATE.md says "No lap times showing: Even invalid laps should display -- show invalid laps in GREY." Currently `on_lap_completed()` only updates `best_lap` for valid laps, but `previous_lap` is always updated. The issue may be that invalid laps are not being sent by the adapter at all (the `last_lap_count > 0` guard in `assetto_corsa.rs` blocks the first lap -- Phase 4 DATA-01 fixes this). For Phase 3, ensure the display components render invalid lap times in grey/dimmed color instead of hiding them.
**Why it happens:** Display logic might use white text for valid and nothing for invalid.
**How to avoid:** In PrevLapSection, check `prev.valid` -- if false, render lap time in grey (#555555) instead of white. The "INV" badge already exists. Invalid lap times should still be visible, just visually distinct.
**Warning signs:** Previous lap section shows "--:--.---" even after completing an invalid lap.

## Code Examples

### Example 1: Updated GdiResources with Consolas Numeric Fonts

```rust
// Source: overlay.rs GdiResources::new(), verified against current implementation
unsafe fn new() -> Self {
    fn rgb(r: u8, g: u8, b: u8) -> u32 {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }
    let null_hdc = std::ptr::null_mut();
    Self {
        // Labels: proportional (Segoe UI) -- width does not matter for short labels
        font_label: create_font(null_hdc, "Segoe UI", 12, true),

        // Numeric values: monospace (Consolas) -- prevents jitter
        font_value: create_font(null_hdc, "Consolas", 24, true),     // session timer, lap counter
        font_gear: create_font(null_hdc, "Consolas", 72, true),      // gear: 60-80pt range
        font_speed: create_font(null_hdc, "Consolas", 22, true),     // speed value
        font_lap: create_font(null_hdc, "Consolas", 20, true),       // lap time values
        font_sector: create_font(null_hdc, "Consolas", 13, true),    // sector time values
        font_sector_label: create_font(null_hdc, "Segoe UI", 10, false), // "S1"/"S2"/"S3"
        font_unit: create_font(null_hdc, "Segoe UI", 10, false),     // "KM/H"
        font_badge: create_font(null_hdc, "Segoe UI", 9, true),      // "INV" badge

        pen_divider: CreatePen(PS_SOLID as i32, 1, rgb(40, 40, 40)),
        brush_bg: CreateSolidBrush(rgb(18, 18, 18)),
        brush_rpm_bg: CreateSolidBrush(rgb(30, 30, 30)),
        brush_red: CreateSolidBrush(rgb(225, 6, 0)),
    }
}
```

### Example 2: Essentials compute_layout()

```rust
/// RPM bar height for Essentials layout.
const RPM_BAR_HEIGHT: i32 = 10;
/// Red accent border height.
const ACCENT_HEIGHT: i32 = 2;
/// Content area starts after RPM bar + accent + padding.
const CONTENT_Y: i32 = RPM_BAR_HEIGHT + ACCENT_HEIGHT + 2; // = 14

fn compute_layout(window_width: i32) -> Vec<SectionRect> {
    // Essentials: wider gear section, proportional timing sections
    let section_widths: [i32; 6] = [150, 260, 180, 240, 240, 80];
    let total_content: i32 = section_widths.iter().sum(); // 1150
    let start_x = (window_width - total_content).max(0) / 2;
    let content_h = BAR_HEIGHT - CONTENT_Y - ACCENT_HEIGHT;
    let mut rects = Vec::with_capacity(6);
    let mut sx = start_x;
    for &w in &section_widths {
        rects.push(SectionRect { x: sx, y: CONTENT_Y, w, h: content_h });
        sx += w;
    }
    rects
}
```

### Example 3: RPM Bar at New Height

```rust
// In RpmBarSection::paint(), change rect height from 4 to RPM_BAR_HEIGHT
let rpm_bar_rect = RECT { left: 0, top: 0, right: window_width, bottom: RPM_BAR_HEIGHT };
FillRect(hdc, &rpm_bar_rect, res.brush_rpm_bg);
let rpm_fill_rect = RECT { left: 0, top: 0, right: rpm_bar_w, bottom: RPM_BAR_HEIGHT };
FillRect(hdc, &rpm_fill_rect, rpm_brush.handle());
```

### Example 4: Centered Gear Display

```rust
// In GearSpeedSection::paint()
// Gear is centered horizontally in its section rect
let gear_str = match data.gear {
    0 => "N".to_string(),
    g if g < 0 => "R".to_string(),
    g => g.to_string(),
};
// Consolas at 72px: each character is approximately 43px wide (72 * 0.6)
// Single digit: center at rect.x + (rect.w - 43) / 2
let char_w = 43; // approximate for Consolas 72px
let gear_x = rect.x + (rect.w - char_w) / 2;
draw_text_at(hdc, res.font_gear, col_white, gear_x, rect.y, &gear_str);

// Speed below gear
let speed_str = format!("{}", data.speed_kmh.round() as i32);
let speed_x = rect.x + (rect.w - (speed_str.len() as i32 * 13)) / 2; // 13px per char at 22px
draw_text_at(hdc, res.font_speed, rgb(187,187,187), speed_x, rect.y + 78, &speed_str);
draw_text_at(hdc, res.font_unit, col_dim, speed_x + (speed_str.len() as i32 * 13) + 4, rect.y + 82, "KM/H");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Per-frame font creation (8 CreateFontW/paint) | GdiResources cache (13 handles, created once) | Phase 2 | Enables Phase 3 font changes without per-frame overhead |
| Monolithic paint_hud() match-on-index | HudComponent trait + HudRenderer dispatcher | Phase 2 | Each section is independently modifiable |
| Hardcoded max_rpm=18000 | Dynamic from AC acpmf_static | Already implemented | RPM bar scales correctly per car |
| No characterization tests | 5 test functions (format_timer, format_lap_time, format_sector, sector_color, compute_layout) | Phase 2 | Safe to refactor layout with regression detection |

## Open Questions

1. **Session timer semantics (HUD-05)**
   - What we know: The overlay currently shows billing `remaining_seconds` from `BillingTick`. AC provides `sessionTimeLeft` in acpmf_graphics (offset 152, f32 seconds). These are different: billing is wall-clock, AC's is game-time (pauses in menus).
   - What's unclear: Does HUD-05 mean "show AC's game session time" or "keep showing billing time but label it 'SESSION'"? The STATE.md issue "Timer not synced" was about the billing timer running before game launch, which is already fixed by the `game_live` flag.
   - Recommendation: Keep billing `remaining_seconds` as the displayed timer (it answers "how much time do I have left" which is what customers care about). If Uday wants a game-elapsed timer too, add it as a secondary display. The `game_live` gating already prevents the timer from running before AC is on track.

2. **BAR_HEIGHT increase: exact value**
   - What we know: 96px is too small for 72pt gear. Need at least 140px. Maximum reasonable is ~160px before obscuring game view.
   - What's unclear: Whether 140px overlaps any AC built-in UI elements.
   - Recommendation: Start with 140px (48% increase). Test on Pod 8 -- AC's apps default to top-left corner, our overlay is centered; likely no overlap. If 140px is too much, drop gear to 60pt and use 130px.

3. **Section widths for Consolas**
   - What we know: Consolas characters are wider than Segoe UI at same point size. Max lap time string: "9:59.999" = 8 chars. At Consolas 20px, each char is approximately 12px wide = 96px total.
   - What's unclear: Exact pixel widths until rendered on actual display.
   - Recommendation: Size sections generously (260px for current/prev lap sections) and verify on Pod 8. Adjust widths in `compute_layout()` after visual verification.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` with `#[cfg(test)]` |
| Config file | None (built-in, no config needed) |
| Quick run command | `cargo test -p rc-agent-crate -- overlay::tests` |
| Full suite command | `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| HUD-01 | compute_layout returns correct SectionRects for new widths | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_compute_layout -x` | Exists -- needs update for new widths |
| HUD-02 | RPM color zones at correct thresholds | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_rpm_color_zones -x` | New -- Wave 0 |
| HUD-03 | format_lap_time produces correct strings | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_format_lap_time -x` | Exists |
| HUD-04 | sector_color returns correct F1 colors | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_sector_color -x` | Exists |
| HUD-05 | Session timer format and display logic | unit | `cargo test -p rc-agent-crate -- overlay::tests::test_format_timer -x` | Exists |
| HUD-06 | Lap counter rendering (lap number as string) | manual-only | Deploy to Pod 8, verify lap number displays | Justification: pure display, string formatting already tested |
| HUD-07 | Invalid lap indicator visible | manual-only | Drive invalid lap on Pod 8, verify red bar + "INV" badge | Justification: GDI rendering, cannot unit test |
| HUD-08 | Speed display repositioned | manual-only | Deploy to Pod 8, verify speed below gear | Justification: pixel positioning, visual verification |
| HUD-09 | Consolas font used for numerics | manual-only | Deploy to Pod 8, verify no jitter on changing digits | Justification: font rendering, visual verification |

### Sampling Rate
- **Per task commit:** `cargo test -p rc-agent-crate -- overlay::tests`
- **Per wave merge:** `cargo test -p rc-common && cargo test -p rc-agent-crate && cargo test -p racecontrol-crate`
- **Phase gate:** Full suite green + Pod 8 visual verification before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `test_rpm_color_zones` -- test that RPM percentage thresholds map to correct colors (green/yellow/amber/red)
- [ ] Update `test_compute_layout` -- change expected values to match new Essentials section widths
- [ ] `test_format_lap_time_invalid_grey` -- verify that invalid laps use grey color rendering (code path test, not GDI test)

## Sources

### Primary (HIGH confidence)
- `crates/rc-agent/src/overlay.rs` -- full read of 1,324 lines, verified all component implementations, GdiResources, formatting functions, layout system
- `.planning/phases/02-hud-infrastructure/02-RESEARCH.md` -- Phase 2 research establishing the component architecture
- `.planning/phases/02-hud-infrastructure/02-01-PLAN.md` -- Phase 2 plan confirming GdiResources, HudComponent, HudRenderer are implemented
- `.planning/research/ARCHITECTURE.md` -- Overlay architecture, data flow, component boundaries
- `.planning/research/STACK.md` -- GDI font recommendations, AC shared memory field offsets, DirectWrite migration path (deferred)
- `.planning/research/FEATURES.md` -- AC Essentials layout reference, F1 color coding convention, viewing distance readability
- `.planning/research/PITFALLS.md` -- P-01 through P-04 (GDI leaks, flicker, DPI, topmost)
- `crates/rc-agent/src/sims/assetto_corsa.rs` -- Verified max_rpm read from acpmf_static, sessionTimeLeft at offset 152
- `crates/rc-common/src/types.rs` -- TelemetryFrame fields including sector1_ms, sector2_ms, sector3_ms, current_lap_invalid

### Secondary (MEDIUM confidence)
- [CMRT Essential HUD](https://www.overtake.gg/downloads/cmrt-essential-hud.69475/) -- Third-party AC HUD reference
- [Race Essentials](https://raceessentials.wixsite.com/race-essentials) -- AC dashboard app reference
- `.planning/STATE.md` -- Known issues from Pod 8 test (timer sync, lap display, RPM bar, font sizes)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, all existing winapi/GDI
- Architecture: HIGH -- Phase 2 component system is already built and tested
- Layout geometry: MEDIUM -- exact pixel values need Pod 8 verification
- Pitfalls: HIGH -- 6 pitfalls identified from codebase analysis and prior research

**Research date:** 2026-03-12
**Valid until:** 2026-04-12 (stable domain, no external API changes expected)
