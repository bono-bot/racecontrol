# Phase 2 Research: HUD Infrastructure — GDI Resource Cache and Component System

**Date:** 2026-03-11
**Scope:** INFRA-01 (GDI font cache), INFRA-02 (component-based paint system)
**Key file:** `crates/rc-agent/src/overlay.rs` (848 lines)

---

## 1. Current GDI Leak Sources — Full Audit

### 1.1 Font Creation (overlay.rs:482-489)

Every `WM_PAINT` cycle (every 200ms per `REPAINT_INTERVAL_MS`), `paint_hud()` creates **8 fonts** via `create_font()`:

| Variable | Face | Size | Bold | Line |
|----------|------|------|------|------|
| `font_label` | Segoe UI | 11 | yes | 482 |
| `font_value` | Segoe UI | 22 | yes | 483 |
| `font_gear` | Segoe UI | 32 | yes | 484 |
| `font_speed` | Segoe UI | 16 | yes | 485 |
| `font_lap` | Segoe UI | 18 | yes | 486 |
| `font_sector` | Segoe UI | 12 | yes | 487 |
| `font_sector_label` | Segoe UI | 10 | no | 488 |
| `font_unit` | Segoe UI | 9 | no | 489 |

All 8 are deleted at the end of `paint_hud()` at lines 712-719. **No active leak** — but wasteful: `CreateFontW` involves GDI kernel allocation per call. At 5 paints/second, that is **40 kernel alloc/free cycles per second**.

### 1.2 Conditional Font (overlay.rs:698)

A **9th font** (`font_badge`) is created **conditionally** inside the lap counter section:

```rust
let font_badge = create_font(mem_dc, "Segoe UI", 9, true);
draw_text_at(mem_dc, font_badge, col_white, badge_x + 4, badge_y + 1, "INV");
DeleteObject(font_badge as *mut _);
```

Correctly paired with `DeleteObject`. No leak, but fragile pattern.

### 1.3 Brush Creation (overlay.rs:444-477, 548-551, 689-697)

Per-paint brushes — all correctly paired:

| Brush | Created | Deleted | Conditional? |
|-------|---------|---------|--------------|
| `bg_brush` | 444 | 447 | No |
| `rpm_brush` | 462 | 468 | No |
| `rpm_bg_brush` | 463 | 469 | No |
| `red_brush` | 472 | 477 | No |
| `inv_brush` | 548 | 551 | Yes (invalid lap) |
| `badge_brush` | 689 | 697 | Yes (invalid lap) |

### 1.4 Pen, Double Buffer

- 1 pen (`divider_pen`) per paint — correctly paired (lines 492, 711)
- Memory DC + bitmap — correctly paired (lines 439-441, 725-727)

### 1.5 Summary

**No active GDI leaks exist.** Every `Create*` paired with `DeleteObject`. But **~15-17 GDI objects allocated/freed every 200ms** (80 kernel calls/sec). The fix is caching, not leak plugging. More critically, the monolithic function makes it easy to introduce leaks in Phase 3.

### 1.6 What Needs Caching

**Cache once (WM_CREATE), destroy once (WM_DESTROY):**
- 9 fonts (8 base + 1 badge) = **9 HFONT handles**
- 1 divider pen = **1 HPEN handle**
- 4 constant-color brushes (bg, rpm_bg, red, border) = **4 HBRUSH handles**

**Keep per-paint (dynamic color):**
- `rpm_brush` — color depends on RPM percentage each frame

---

## 2. Font Inventory — Cache Struct Design

### 2.1 Complete Font Table

| Name | Face | Size (px) | Weight | Usage |
|------|------|----------|--------|-------|
| `label` | Segoe UI | 11 | 700 (bold) | Section headers |
| `value` | Segoe UI | 22 | 700 (bold) | Primary values (timer, lap#) |
| `gear` | Segoe UI | 32 | 700 (bold) | Gear indicator |
| `speed` | Segoe UI | 16 | 700 (bold) | Speed value |
| `lap` | Segoe UI | 18 | 700 (bold) | Lap times |
| `sector` | Segoe UI | 12 | 700 (bold) | Sector time values |
| `sector_label` | Segoe UI | 10 | 400 (normal) | "S1"/"S2"/"S3" labels, RPM |
| `unit` | Segoe UI | 9 | 400 (normal) | "KM/H" unit text |
| `badge` | Segoe UI | 9 | 700 (bold) | "INV" badge text |

### 2.2 Proposed GdiResources Struct

```rust
struct GdiResources {
    // Fonts (9 handles)
    font_label: HFONT,        // 11px bold
    font_value: HFONT,        // 22px bold
    font_gear: HFONT,         // 32px bold
    font_speed: HFONT,        // 16px bold
    font_lap: HFONT,          // 18px bold
    font_sector: HFONT,       // 12px bold
    font_sector_label: HFONT, // 10px normal
    font_unit: HFONT,         // 9px normal
    font_badge: HFONT,        // 9px bold

    // Pens (1 handle)
    pen_divider: HPEN,        // 1px solid #282828

    // Brushes (constant-color, 4 handles)
    brush_bg: HBRUSH,         // #121212
    brush_rpm_bg: HBRUSH,     // #1E1E1E
    brush_red: HBRUSH,        // #E10600
}

impl Drop for GdiResources {
    fn drop(&mut self) {
        unsafe { /* DeleteObject all 14 handles */ }
    }
}
```

**Total cached handles: 14** (9 fonts + 1 pen + 4 brushes).

---

## 3. Component Decomposition

### 3.1 Current Section Loop (overlay.rs:518-707)

`for (i, &sec_w) in section_widths.iter().enumerate()` with `match i`:

| Index | Component | Width | Data Fields |
|-------|-----------|-------|-------------|
| 0 | Session Timer | 120px | `remaining_seconds` |
| 1 | Current Lap | 200px | `current_lap_time_ms`, `current_lap_invalid`, `current_sector`, sectors, prev/best lap |
| 2 | Gear + Speed | 100px | `gear`, `speed_kmh`, `rpm` |
| 3 | Previous Lap | 200px | `previous_lap`, `best_lap` |
| 4 | Best Lap | 200px | `best_lap` |
| 5 | Lap Counter | 60px | `current_lap_number`, `current_lap_invalid` |

Plus full-width: **RPM Bar** (449-469), **Accent Borders** (472-477).

Every component reads `&OverlayData` immutably. Clean for shared reference.

---

## 4. Trait Design

### 4.1 HudComponent Trait

```rust
trait HudComponent {
    fn paint(&self, hdc: HDC, rect: &SectionRect, data: &OverlayData, res: &GdiResources);
}

#[derive(Debug, Clone, Copy)]
struct SectionRect { x: i32, y: i32, w: i32, h: i32 }
```

**Recommendation: Trait objects (`Box<dyn HudComponent>`).** Matches the requirement "implementing one trait and registering it." Vtable overhead negligible at 6 components × 5 Hz.

### 4.2 Registration

```rust
struct HudRenderer {
    components: Vec<(Box<dyn HudComponent>, i32)>,  // (component, width)
    rpm_bar: RpmBarComponent,
}

impl HudRenderer {
    fn new() -> Self {
        let mut r = Self { components: Vec::new(), rpm_bar: RpmBarComponent };
        r.register(Box::new(SessionTimerComponent), 120);
        r.register(Box::new(CurrentLapComponent), 200);
        // ...
        r
    }
}
```

Adding a new component = implement trait + one `register()` call. No `paint_hud()` changes.

---

## 5. GDI Resource Lifecycle

### 5.1 Current vs. Proposed

| Event | Current | Proposed |
|-------|---------|----------|
| WM_CREATE | Store `Arc<Mutex<OverlayData>>` pointer | Create `WindowState { data, res: GdiResources, renderer: HudRenderer }` |
| WM_PAINT | Create 15+ GDI objects, paint, delete all | Use cached `res`, only create dynamic RPM brush |
| WM_DESTROY | Free data pointer | `res.destroy()` (14 handles), free WindowState |

### 5.2 WindowState Struct

```rust
struct WindowState {
    data: Arc<Mutex<OverlayData>>,
    res: GdiResources,
    renderer: HudRenderer,
    gdi_baseline: u32,  // for leak detection
}
```

Stored via `SetWindowLongPtrW(GWLP_USERDATA)` — only accessed from window thread. Not `Send`/`Sync`, which is fine.

**Note:** `CreateFontW` does not need an HDC parameter (the existing `create_font` prefixes it with `_`). `GdiResources::new()` can call `CreateFontW` directly.

---

## 6. Validation Architecture

### 6.1 Runtime GDI Handle Monitoring

```rust
fn gdi_handle_count() -> u32 {
    unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) }
}
```

- Log baseline at WM_CREATE
- Check every 60s in WM_TIMER
- Warn if count drifts >5 from baseline

### 6.2 Characterization Tests (before refactor)

Pure functions testable without Win32:
- `format_timer(90)` → `"01:30"`
- `format_lap_time(0)` → `"--:--.---"`
- `format_sector(Some(32100))` → `"32.1"`
- `sector_color(...)` → correct color
- `compute_layout(1920, 96)` → correct SectionRects

### 6.3 Visual Regression (Pod 8)

1. Screenshot current HUD before refactor
2. Deploy refactored code to Pod 8
3. Screenshot and compare visually
4. Run 30-min session, verify GDI handle count stable

### 6.4 TempBrush RAII Wrapper

```rust
struct TempBrush(HBRUSH);
impl TempBrush {
    fn new(color: u32) -> Self { Self(unsafe { CreateSolidBrush(color) }) }
    fn handle(&self) -> HBRUSH { self.0 }
}
impl Drop for TempBrush {
    fn drop(&mut self) { unsafe { DeleteObject(self.0 as *mut _); } }
}
```

Guarantees cleanup for dynamic brushes even on early returns.

---

## 7. Risk Analysis

| Risk | Probability | Mitigation |
|------|------------|------------|
| Flicker during refactor | Medium | Preserve double-buffer pattern (mem_dc → BitBlt) |
| Wrong teardown order | Low | Use `draw_text_at` helper (handles select/restore) |
| Missed DeleteObject for dynamic brushes | Medium | TempBrush RAII wrapper + GDI counter |
| Thread safety of GdiResources | Low | Store in WindowState (window thread only) |
| HFONT not Send/Sync | Certain | WindowState via raw pointer, not in Arc<Mutex> |
| Visual regression | Medium | Screenshot before/after, characterization tests |
| Layout coordinate drift | Low | `compute_layout()` characterization test |

---

## 8. Build Order

1. **Write characterization tests** — format_timer, format_lap_time, format_sector, sector_color, layout math
2. **Extract GdiResources** — cache 14 handles, implement Drop
3. **Extract compute_layout()** — pure function with test
4. **Define HudComponent trait + implement 6 components** — dispatcher loop replaces match
5. **Add GDI handle counter** — runtime leak detection

---

## 9. Files to Modify

Phase 2 touches **one file**: `crates/rc-agent/src/overlay.rs`. No API changes. No cross-crate changes.

---

*Phase: 02-hud-infrastructure*
*Research completed: 2026-03-11*
