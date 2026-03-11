//! Racing HUD overlay displayed at the top of the screen during active billing sessions.
//!
//! Renders a native Win32 popup window with GDI text — no browser, no HTTP server.
//! The window is created borderless and topmost from birth, so there is no
//! timing-dependent title-bar stripping. A dedicated thread runs the Win32
//! message loop and repaints the HUD every 200 ms.

#![allow(unsafe_op_in_unsafe_fn)]

use std::sync::{Arc, Mutex};
use rc_common::types::{LapData, TelemetryFrame};

/// Height of the HUD bar (px).
const BAR_HEIGHT: i32 = 96;
/// Maximum bar width.
const BAR_WIDTH: i32 = 1920;
/// Repaint interval (ms) — matches the old HTTP polling rate.
const REPAINT_INTERVAL_MS: u32 = 200;
/// Timer ID for WM_TIMER.
const TIMER_ID: usize = 1;

// ─── Types ───────────────────────────────────────────────────────────────────

/// A completed lap record for overlay display.
#[derive(Debug, Clone)]
struct LapRecord {
    lap_time_ms: u32,
    sector1_ms: Option<u32>,
    sector2_ms: Option<u32>,
    sector3_ms: Option<u32>,
    valid: bool,
}

/// Shared state written by the public API, read by the paint routine.
#[derive(Debug, Clone)]
struct OverlayData {
    active: bool,
    driver_name: String,
    remaining_seconds: u32,
    allocated_seconds: u32,
    current_lap_number: u32,
    current_lap_time_ms: u32,
    current_sector: u8,
    current_lap_invalid: bool,
    live_sector1_ms: Option<u32>,
    live_sector2_ms: Option<u32>,
    live_sector3_ms: Option<u32>,
    speed_kmh: f32,
    gear: i8,
    rpm: u32,
    car: String,
    track: String,
    previous_lap: Option<LapRecord>,
    best_lap: Option<LapRecord>,
}

impl Default for OverlayData {
    fn default() -> Self {
        Self {
            active: false,
            driver_name: String::new(),
            remaining_seconds: 0,
            allocated_seconds: 0,
            current_lap_number: 0,
            current_lap_time_ms: 0,
            current_sector: 0,
            current_lap_invalid: false,
            live_sector1_ms: None,
            live_sector2_ms: None,
            live_sector3_ms: None,
            speed_kmh: 0.0,
            gear: 0,
            rpm: 0,
            car: String::new(),
            track: String::new(),
            previous_lap: None,
            best_lap: None,
        }
    }
}

// ─── GDI Resource Cache ─────────────────────────────────────────────────────

/// Layout rectangle for a HUD section.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SectionRect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

/// Compute section layout rectangles for the given window width.
/// Returns 6 rects, one per HUD section, horizontally centered.
fn compute_layout(window_width: i32) -> Vec<SectionRect> {
    let section_widths: [i32; 6] = [120, 200, 100, 200, 200, 60];
    let total_content: i32 = section_widths.iter().sum(); // 880
    let start_x = (window_width - total_content).max(0) / 2;
    let mut rects = Vec::with_capacity(6);
    let mut sx = start_x;
    for &w in &section_widths {
        rects.push(SectionRect { x: sx, y: 12, w, h: BAR_HEIGHT });
        sx += w;
    }
    rects
}

/// Cached GDI handles — created once at WM_CREATE, freed at WM_DESTROY via Drop.
#[cfg(windows)]
struct GdiResources {
    font_label: winapi::shared::windef::HFONT,        // 11px bold
    font_value: winapi::shared::windef::HFONT,        // 22px bold
    font_gear: winapi::shared::windef::HFONT,         // 32px bold
    font_speed: winapi::shared::windef::HFONT,        // 16px bold
    font_lap: winapi::shared::windef::HFONT,          // 18px bold
    font_sector: winapi::shared::windef::HFONT,       // 12px bold
    font_sector_label: winapi::shared::windef::HFONT, // 10px normal
    font_unit: winapi::shared::windef::HFONT,         // 9px normal
    font_badge: winapi::shared::windef::HFONT,        // 9px bold
    pen_divider: winapi::shared::windef::HPEN,        // 1px solid #282828
    brush_bg: winapi::shared::windef::HBRUSH,         // #121212
    brush_rpm_bg: winapi::shared::windef::HBRUSH,     // #1E1E1E
    brush_red: winapi::shared::windef::HBRUSH,        // #E10600
}

#[cfg(windows)]
impl GdiResources {
    /// Create all cached GDI handles. Must be called from the window thread.
    unsafe fn new() -> Self {
        use winapi::um::wingdi::*;

        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }

        let null_hdc = std::ptr::null_mut();
        Self {
            font_label: create_font(null_hdc, "Segoe UI", 11, true),
            font_value: create_font(null_hdc, "Segoe UI", 22, true),
            font_gear: create_font(null_hdc, "Segoe UI", 32, true),
            font_speed: create_font(null_hdc, "Segoe UI", 16, true),
            font_lap: create_font(null_hdc, "Segoe UI", 18, true),
            font_sector: create_font(null_hdc, "Segoe UI", 12, true),
            font_sector_label: create_font(null_hdc, "Segoe UI", 10, false),
            font_unit: create_font(null_hdc, "Segoe UI", 9, false),
            font_badge: create_font(null_hdc, "Segoe UI", 9, true),
            pen_divider: CreatePen(PS_SOLID as i32, 1, rgb(40, 40, 40)),
            brush_bg: CreateSolidBrush(rgb(18, 18, 18)),
            brush_rpm_bg: CreateSolidBrush(rgb(30, 30, 30)),
            brush_red: CreateSolidBrush(rgb(225, 6, 0)),
        }
    }
}

#[cfg(windows)]
impl Drop for GdiResources {
    fn drop(&mut self) {
        unsafe {
            use winapi::um::wingdi::DeleteObject;
            DeleteObject(self.font_label as *mut _);
            DeleteObject(self.font_value as *mut _);
            DeleteObject(self.font_gear as *mut _);
            DeleteObject(self.font_speed as *mut _);
            DeleteObject(self.font_lap as *mut _);
            DeleteObject(self.font_sector as *mut _);
            DeleteObject(self.font_sector_label as *mut _);
            DeleteObject(self.font_unit as *mut _);
            DeleteObject(self.font_badge as *mut _);
            DeleteObject(self.pen_divider as *mut _);
            DeleteObject(self.brush_bg as *mut _);
            DeleteObject(self.brush_rpm_bg as *mut _);
            DeleteObject(self.brush_red as *mut _);
        }
    }
}

/// RAII wrapper for dynamic (per-frame) GDI brushes.
#[cfg(windows)]
struct TempBrush(winapi::shared::windef::HBRUSH);

#[cfg(windows)]
impl TempBrush {
    fn new(color: u32) -> Self {
        Self(unsafe { winapi::um::wingdi::CreateSolidBrush(color) })
    }
    fn handle(&self) -> winapi::shared::windef::HBRUSH {
        self.0
    }
}

#[cfg(windows)]
impl Drop for TempBrush {
    fn drop(&mut self) {
        unsafe { winapi::um::wingdi::DeleteObject(self.0 as *mut _); }
    }
}

// ─── HUD Component System ───────────────────────────────────────────────────

/// Trait for HUD section components. Each section knows how to paint itself.
#[cfg(windows)]
trait HudComponent {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    );
}

/// Session timer section (match arm 0).
#[cfg(windows)]
struct SessionTimerSection;

#[cfg(windows)]
impl HudComponent for SessionTimerSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_grey: u32 = rgb(85, 85, 85);
        let col_white: u32 = rgb(255, 255, 255);
        let col_red: u32 = rgb(225, 6, 0);
        let col_amber: u32 = rgb(245, 158, 11);

        unsafe {
            draw_text_at(hdc, res.font_label, col_grey, rect.x + 12, rect.y, "SESSION");

            let timer_str = format_timer(data.remaining_seconds);
            let timer_col = if data.remaining_seconds <= 10 {
                col_red
            } else if data.remaining_seconds <= 60 {
                col_amber
            } else {
                col_white
            };
            draw_text_at(hdc, res.font_value, timer_col, rect.x + 12, 28, &timer_str);
        }
    }
}

/// Current lap section with live sector times (match arm 1).
#[cfg(windows)]
struct CurrentLapSection;

#[cfg(windows)]
impl HudComponent for CurrentLapSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_grey: u32 = rgb(85, 85, 85);
        let col_white: u32 = rgb(255, 255, 255);
        let col_red: u32 = rgb(225, 6, 0);
        let col_purple: u32 = rgb(168, 85, 247);
        let col_dim: u32 = rgb(68, 68, 68);
        let col_sector_grey: u32 = rgb(160, 160, 160);

        unsafe {
            use winapi::shared::windef::RECT;
            use winapi::um::winuser::FillRect;

            draw_text_at(hdc, res.font_label, col_grey, rect.x + 12, rect.y, "CURRENT LAP");

            let lap_str = format_lap_time(data.current_lap_time_ms);
            let lap_col = if data.current_lap_invalid { rgb(255, 138, 132) } else { col_white };
            if data.current_lap_invalid {
                let inv_brush = TempBrush::new(col_red);
                let inv_rect = RECT {
                    left: rect.x + 8,
                    top: 28 - 2,
                    right: rect.x + 11,
                    bottom: 28 + 24,
                };
                FillRect(hdc, &inv_rect, inv_brush.handle());
            }
            draw_text_at(hdc, res.font_value, lap_col, rect.x + 16, 28, &lap_str);

            // Live sector times
            let mut sector_x = rect.x + 12;
            let sectors: [(&str, Option<u32>, Option<u32>, Option<u32>); 3] = [
                ("S1", data.live_sector1_ms,
                 data.previous_lap.as_ref().and_then(|p| p.sector1_ms),
                 data.best_lap.as_ref().and_then(|b| b.sector1_ms)),
                ("S2", data.live_sector2_ms,
                 data.previous_lap.as_ref().and_then(|p| p.sector2_ms),
                 data.best_lap.as_ref().and_then(|b| b.sector2_ms)),
                ("S3", data.live_sector3_ms,
                 data.previous_lap.as_ref().and_then(|p| p.sector3_ms),
                 data.best_lap.as_ref().and_then(|b| b.sector3_ms)),
            ];
            for (idx, (label, ms, prev_ms, best_ms)) in sectors.iter().enumerate() {
                let is_active = data.current_sector == idx as u8 && ms.is_none();
                let label_col = if is_active { col_white } else { col_dim };
                draw_text_at(hdc, res.font_sector_label, label_col, sector_x, 56, label);
                sector_x += 16;

                if ms.is_some() {
                    let col = sector_color(
                        *ms, *prev_ms, *best_ms,
                        col_sector_grey, col_purple, rgb(34, 197, 94), rgb(245, 158, 11),
                    );
                    draw_text_at(hdc, res.font_sector, col, sector_x, 55, &format_sector(*ms));
                } else {
                    let dash_col = if is_active { col_white } else { col_dim };
                    draw_text_at(hdc, res.font_sector, dash_col, sector_x, 55, "--.-");
                }
                sector_x += 46;
            }
        }
    }
}

/// Gear + speed + RPM number section (match arm 2).
#[cfg(windows)]
struct GearSpeedSection;

#[cfg(windows)]
impl HudComponent for GearSpeedSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_white: u32 = rgb(255, 255, 255);
        let col_dim: u32 = rgb(68, 68, 68);

        unsafe {
            let gear_str = match data.gear {
                0 => "N".to_string(),
                g if g < 0 => "R".to_string(),
                g => g.to_string(),
            };
            draw_text_at(hdc, res.font_gear, col_white, rect.x + 12, 14, &gear_str);

            let speed_str = if data.speed_kmh > 0.0 {
                format!("{}", data.speed_kmh.round() as i32)
            } else {
                "---".to_string()
            };
            draw_text_at(hdc, res.font_speed, rgb(187, 187, 187), rect.x + 52, 18, &speed_str);
            draw_text_at(hdc, res.font_unit, col_dim, rect.x + 52, 38, "KM/H");

            if data.rpm > 0 {
                let rpm_str = format!("{}", data.rpm);
                draw_text_at(hdc, res.font_sector_label, col_dim, rect.x + 52, 56, &rpm_str);
            }
        }
    }
}

/// Previous lap section with sector times (match arm 3).
#[cfg(windows)]
struct PrevLapSection;

#[cfg(windows)]
impl HudComponent for PrevLapSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_grey: u32 = rgb(85, 85, 85);
        let col_light_grey: u32 = rgb(229, 231, 235);
        let col_purple: u32 = rgb(168, 85, 247);
        let col_dim: u32 = rgb(68, 68, 68);
        let col_sector_grey: u32 = rgb(160, 160, 160);

        unsafe {
            draw_text_at(hdc, res.font_label, col_grey, rect.x + 12, rect.y, "PREV");

            if let Some(ref prev) = data.previous_lap {
                let prev_str = format_lap_time(prev.lap_time_ms);
                draw_text_at(hdc, res.font_lap, col_light_grey, rect.x + 12, 28, &prev_str);

                let mut sector_x = rect.x + 12;
                for (label, ms, best_ms) in [
                    ("S1", prev.sector1_ms, data.best_lap.as_ref().and_then(|b| b.sector1_ms)),
                    ("S2", prev.sector2_ms, data.best_lap.as_ref().and_then(|b| b.sector2_ms)),
                    ("S3", prev.sector3_ms, data.best_lap.as_ref().and_then(|b| b.sector3_ms)),
                ] {
                    draw_text_at(hdc, res.font_sector_label, col_dim, sector_x, 56, label);
                    sector_x += 16;
                    let val_str = format_sector(ms);
                    let col = sector_color(
                        ms, None, best_ms,
                        col_sector_grey, col_purple, rgb(34, 197, 94), rgb(245, 158, 11),
                    );
                    draw_text_at(hdc, res.font_sector, col, sector_x, 55, &val_str);
                    sector_x += 46;
                }
            } else {
                draw_text_at(hdc, res.font_lap, rgb(51, 51, 51), rect.x + 12, 28, "--:--.---");
            }
        }
    }
}

/// Best lap section with sector times (match arm 4).
#[cfg(windows)]
struct BestLapSection;

#[cfg(windows)]
impl HudComponent for BestLapSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_purple: u32 = rgb(168, 85, 247);

        unsafe {
            draw_text_at(hdc, res.font_label, col_purple, rect.x + 12, rect.y, "BEST");

            if let Some(ref best) = data.best_lap {
                let best_str = format_lap_time(best.lap_time_ms);
                draw_text_at(hdc, res.font_lap, col_purple, rect.x + 12, 28, &best_str);

                let mut sector_x = rect.x + 12;
                for (label, ms) in [
                    ("S1", best.sector1_ms),
                    ("S2", best.sector2_ms),
                    ("S3", best.sector3_ms),
                ] {
                    draw_text_at(hdc, res.font_sector_label, rgb(120, 60, 180), sector_x, 56, label);
                    sector_x += 16;
                    draw_text_at(hdc, res.font_sector, col_purple, sector_x, 55, &format_sector(ms));
                    sector_x += 46;
                }
            } else {
                draw_text_at(hdc, res.font_lap, rgb(51, 51, 51), rect.x + 12, 28, "--:--.---");
            }
        }
    }
}

/// Lap counter section with INV badge (match arm 5).
#[cfg(windows)]
struct LapCounterSection;

#[cfg(windows)]
impl HudComponent for LapCounterSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        rect: &SectionRect,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }
        let col_grey: u32 = rgb(85, 85, 85);
        let col_white: u32 = rgb(255, 255, 255);
        let col_red: u32 = rgb(225, 6, 0);

        unsafe {
            use winapi::shared::windef::RECT;
            use winapi::um::winuser::FillRect;

            draw_text_at(hdc, res.font_label, col_grey, rect.x + 12, rect.y, "LAP");

            let lap_num_str = if data.current_lap_number > 0 {
                data.current_lap_number.to_string()
            } else {
                "-".to_string()
            };
            draw_text_at(hdc, res.font_value, col_white, rect.x + 12, 28, &lap_num_str);

            // INV badge
            if data.current_lap_invalid {
                let badge_x = rect.x + 42;
                let badge_y = 28 + 2;
                let badge_brush = TempBrush::new(col_red);
                let badge_rect = RECT {
                    left: badge_x,
                    top: badge_y,
                    right: badge_x + 30,
                    bottom: badge_y + 16,
                };
                FillRect(hdc, &badge_rect, badge_brush.handle());
                draw_text_at(hdc, res.font_badge, col_white, badge_x + 4, badge_y + 1, "INV");
            }
        }
    }
}

/// Full-width RPM color bar at the top of the HUD.
#[cfg(windows)]
struct RpmBarSection;

#[cfg(windows)]
impl RpmBarSection {
    fn paint(
        &self,
        hdc: winapi::shared::windef::HDC,
        window_width: i32,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }

        unsafe {
            use winapi::shared::windef::RECT;
            use winapi::um::winuser::FillRect;

            let rpm_pct = if data.rpm > 0 { (data.rpm as f32 / 18000.0).min(1.0) } else { 0.0 };
            let rpm_bar_w = (rpm_pct * window_width as f32) as i32;
            let rpm_col = if rpm_pct > 0.90 {
                rgb(225, 6, 0)
            } else if rpm_pct > 0.75 {
                rgb(245, 158, 11)
            } else if rpm_pct > 0.50 {
                rgb(234, 179, 8)
            } else {
                rgb(34, 197, 94)
            };
            let rpm_brush = TempBrush::new(rpm_col);
            let rpm_bar_rect = RECT { left: 0, top: 0, right: window_width, bottom: 4 };
            FillRect(hdc, &rpm_bar_rect, res.brush_rpm_bg);
            let rpm_fill_rect = RECT { left: 0, top: 0, right: rpm_bar_w, bottom: 4 };
            FillRect(hdc, &rpm_fill_rect, rpm_brush.handle());
        }
    }
}

/// Dispatches paint calls to registered HUD components.
#[cfg(windows)]
struct HudRenderer {
    sections: Vec<Box<dyn HudComponent>>,
    rpm_bar: RpmBarSection,
}

#[cfg(windows)]
impl HudRenderer {
    fn new() -> Self {
        Self {
            sections: vec![
                Box::new(SessionTimerSection),
                Box::new(CurrentLapSection),
                Box::new(GearSpeedSection),
                Box::new(PrevLapSection),
                Box::new(BestLapSection),
                Box::new(LapCounterSection),
            ],
            rpm_bar: RpmBarSection,
        }
    }

    /// Paint all HUD components: RPM bar, red borders, dividers, then sections.
    unsafe fn paint_all(
        &self,
        hdc: winapi::shared::windef::HDC,
        window_width: i32,
        window_height: i32,
        data: &OverlayData,
        res: &GdiResources,
    ) {
        use winapi::shared::windef::RECT;
        use winapi::um::wingdi::*;
        use winapi::um::winuser::FillRect;

        // RPM bar (full-width, top 4px)
        self.rpm_bar.paint(hdc, window_width, data, res);

        // Red accent borders (cached brush)
        let top_border = RECT { left: 0, top: 4, right: window_width, bottom: 6 };
        let bot_border = RECT { left: 0, top: window_height - 2, right: window_width, bottom: window_height };
        FillRect(hdc, &top_border, res.brush_red);
        FillRect(hdc, &bot_border, res.brush_red);

        SetBkMode(hdc, TRANSPARENT as i32);

        // Section layout
        let rects = compute_layout(window_width);
        let old_pen = SelectObject(hdc, res.pen_divider as *mut _);

        // Dividers between sections
        for (i, rect) in rects.iter().enumerate() {
            if i > 0 {
                MoveToEx(hdc, rect.x, 8, std::ptr::null_mut());
                LineTo(hdc, rect.x, window_height - 6);
            }
        }

        // Paint each section component
        for (i, component) in self.sections.iter().enumerate() {
            if let Some(rect) = rects.get(i) {
                component.paint(hdc, rect, data, res);
            }
        }

        // Restore pen
        SelectObject(hdc, old_pen as *mut _);
    }
}

// ─── GDI Handle Leak Detection ──────────────────────────────────────────────

/// How often (in WM_TIMER ticks) to check GDI handle count.
/// At 200ms/tick, 300 ticks ≈ 60 seconds.
const GDI_CHECK_INTERVAL: u32 = 300;

/// Maximum allowed drift from baseline before warning.
const GDI_DRIFT_THRESHOLD: u32 = 5;

/// Returns the current GDI object count for this process.
#[cfg(windows)]
fn gdi_handle_count() -> u32 {
    // GetGuiResources is in user32.dll. Use raw FFI since winapi may not export it.
    unsafe extern "system" {
        fn GetGuiResources(hProcess: winapi::shared::ntdef::HANDLE, uiFlags: u32) -> u32;
    }
    unsafe {
        GetGuiResources(winapi::um::processthreadsapi::GetCurrentProcess(), 0) // GR_GDIOBJECTS = 0
    }
}

/// Window-thread-local state stored via SetWindowLongPtrW(GWLP_USERDATA).
#[cfg(windows)]
struct WindowState {
    data: Arc<Mutex<OverlayData>>,
    res: GdiResources,
    renderer: HudRenderer,
    gdi_baseline: u32,
    timer_tick: u32,
}

// ─── Manager ─────────────────────────────────────────────────────────────────

/// Manages the racing HUD overlay lifecycle.
pub struct OverlayManager {
    state: Arc<Mutex<OverlayData>>,
    #[cfg(windows)]
    window_hwnd: Arc<Mutex<Option<isize>>>,
    #[cfg(windows)]
    window_thread: Option<std::thread::JoinHandle<()>>,
}

impl OverlayManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(OverlayData::default())),
            #[cfg(windows)]
            window_hwnd: Arc::new(Mutex::new(None)),
            #[cfg(windows)]
            window_thread: None,
        }
    }

    /// No-op — kept for API compatibility with main.rs.
    /// The old implementation started an HTTP server here; the native window
    /// is created on-demand in `activate()`.
    pub fn start_server(&self) {
        tracing::info!("Overlay: native Win32 mode — no HTTP server needed");
    }

    /// Activate overlay for a new billing session.
    pub fn activate(&mut self, driver_name: String, allocated_seconds: u32) {
        {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            *data = OverlayData {
                active: true,
                driver_name,
                remaining_seconds: allocated_seconds,
                allocated_seconds,
                ..OverlayData::default()
            };
            data.active = true;
        }

        #[cfg(windows)]
        self.open_window();
    }

    /// Update billing timer from BillingTick.
    pub fn update_billing(&self, remaining_seconds: u32) {
        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if data.active {
            data.remaining_seconds = remaining_seconds;
        }
    }

    /// Update telemetry data from current frame.
    pub fn update_telemetry(&self, frame: &TelemetryFrame) {
        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !data.active {
            return;
        }
        data.current_lap_number = frame.lap_number;
        data.current_lap_time_ms = frame.lap_time_ms;
        data.current_sector = frame.sector;
        data.current_lap_invalid = frame.current_lap_invalid.unwrap_or(false);
        data.speed_kmh = frame.speed_kmh;
        data.gear = frame.gear;
        data.rpm = frame.rpm;
        data.car = frame.car.clone();
        data.track = frame.track.clone();
        data.live_sector1_ms = frame.sector1_ms;
        data.live_sector2_ms = frame.sector2_ms;
        data.live_sector3_ms = frame.sector3_ms;
    }

    /// Record a completed lap — update previous_lap and possibly best_lap.
    pub fn on_lap_completed(&self, lap: &LapData) {
        let record = LapRecord {
            lap_time_ms: lap.lap_time_ms,
            sector1_ms: lap.sector1_ms,
            sector2_ms: lap.sector2_ms,
            sector3_ms: lap.sector3_ms,
            valid: lap.valid,
        };

        let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !data.active {
            return;
        }

        data.previous_lap = Some(record.clone());

        if lap.valid {
            let is_best = match &data.best_lap {
                Some(best) => lap.lap_time_ms < best.lap_time_ms,
                None => true,
            };
            if is_best {
                data.best_lap = Some(record);
            }
        }
    }

    /// Deactivate overlay — close window, restore taskbar, clear state.
    pub fn deactivate(&mut self) {
        {
            let mut data = self.state.lock().unwrap_or_else(|e| e.into_inner());
            data.active = false;
        }
        #[cfg(windows)]
        self.close_window();
        crate::kiosk::hide_taskbar(false);
    }

    /// Re-enforce HWND_TOPMOST (call periodically from main loop).
    pub fn enforce_topmost(&self) {
        let data = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if data.active {
            #[cfg(windows)]
            {
                let hwnd_guard = self.window_hwnd.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(h) = *hwnd_guard {
                    let hwnd = h as winapi::shared::windef::HWND;
                    unsafe {
                        winapi::um::winuser::SetWindowPos(
                            hwnd,
                            winapi::um::winuser::HWND_TOPMOST,
                            0,
                            0,
                            0,
                            0,
                            winapi::um::winuser::SWP_NOMOVE
                                | winapi::um::winuser::SWP_NOSIZE
                                | winapi::um::winuser::SWP_NOACTIVATE,
                        );
                    }
                }
            }
        }
    }

    // ─── Windows window management ──────────────────────────────────────────

    #[cfg(windows)]
    fn open_window(&mut self) {
        self.close_window();
        crate::kiosk::hide_taskbar(true);

        let state = self.state.clone();
        let hwnd_slot = self.window_hwnd.clone();

        let handle = std::thread::spawn(move || {
            win32_window_loop(state, hwnd_slot);
        });
        self.window_thread = Some(handle);
    }

    #[cfg(windows)]
    fn close_window(&mut self) {
        {
            let mut hwnd_guard = self.window_hwnd.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(h) = hwnd_guard.take() {
                let hwnd = h as winapi::shared::windef::HWND;
                unsafe {
                    winapi::um::winuser::PostMessageW(
                        hwnd,
                        winapi::um::winuser::WM_CLOSE,
                        0,
                        0,
                    );
                }
            }
        }
        if let Some(handle) = self.window_thread.take() {
            let _ = handle.join();
        }
        tracing::info!("Overlay window closed");
    }
}

// ─── Win32 Window Implementation ─────────────────────────────────────────────

#[cfg(windows)]
fn win32_window_loop(state: Arc<Mutex<OverlayData>>, hwnd_slot: Arc<Mutex<Option<isize>>>) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::minwindef::*;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::*;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    let class_name = wide("RacingHudOverlay");
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    // Build WindowState with cached GDI resources and renderer (created on this thread)
    let window_state = unsafe {
        let res = GdiResources::new();
        let baseline = gdi_handle_count();
        Box::new(WindowState {
            data: state.clone(),
            res,
            renderer: HudRenderer::new(),
            gdi_baseline: baseline,
            timer_tick: 0,
        })
    };
    let state_ptr = Box::into_raw(window_state);

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: unsafe { LoadCursorW(std::ptr::null_mut(), IDC_ARROW) },
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };

    unsafe {
        RegisterClassExW(&wc);
    }

    // Center horizontally on screen
    let screen_w = unsafe { GetSystemMetrics(SM_CXSCREEN) };
    let bar_w = screen_w.min(BAR_WIDTH);
    let x = (screen_w - bar_w).max(0) / 2;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED,
            class_name.as_ptr(),
            wide("Racing HUD").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            x,
            0,
            bar_w,
            BAR_HEIGHT,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            state_ptr as LPVOID,
        )
    };

    if hwnd.is_null() {
        tracing::error!("Overlay: CreateWindowExW failed");
        // Clean up the leaked state pointer
        unsafe { drop(Box::from_raw(state_ptr)); }
        return;
    }

    // Set 94% opacity via layered window attributes
    // 0.94 * 255 ≈ 240
    unsafe {
        SetLayeredWindowAttributes(hwnd, 0, 240, LWA_ALPHA);
    }

    // Store HWND so other threads can PostMessage to us
    {
        let mut slot = hwnd_slot.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(hwnd as isize);
    }

    // Start repaint timer
    unsafe {
        SetTimer(hwnd, TIMER_ID, REPAINT_INTERVAL_MS, None);
    }

    tracing::info!("Overlay: native Win32 window created ({}x{} at {},0)", bar_w, BAR_HEIGHT, x);

    // Message loop
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup: the state_ptr is freed in WM_DESTROY
    {
        let mut slot = hwnd_slot.lock().unwrap_or_else(|e| e.into_inner());
        *slot = None;
    }
}

#[cfg(windows)]
unsafe extern "system" fn wnd_proc(
    hwnd: winapi::shared::windef::HWND,
    msg: u32,
    wparam: winapi::shared::minwindef::WPARAM,
    lparam: winapi::shared::minwindef::LPARAM,
) -> winapi::shared::minwindef::LRESULT {
    use winapi::shared::minwindef::*;
    use winapi::um::winuser::*;

    unsafe {
        match msg {
            WM_CREATE => {
                let cs = &*(lparam as *const CREATESTRUCTW);
                // cs.lpCreateParams is the Box<WindowState> raw pointer
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                let ws = &*(cs.lpCreateParams as *const WindowState);
                tracing::info!(
                    "Overlay: GDI resources cached (13 handles), baseline GDI count = {}",
                    ws.gdi_baseline
                );
                0
            }
            WM_TIMER => {
                // Periodic GDI handle count check for leak detection
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !ws_ptr.is_null() {
                    let ws = &mut *ws_ptr;
                    ws.timer_tick += 1;
                    if ws.timer_tick % GDI_CHECK_INTERVAL == 0 {
                        let current = gdi_handle_count();
                        let drift = current.saturating_sub(ws.gdi_baseline);
                        if drift > GDI_DRIFT_THRESHOLD {
                            tracing::warn!(
                                "Overlay: GDI handle drift detected! baseline={}, current={}, drift=+{}",
                                ws.gdi_baseline, current, drift
                            );
                        } else {
                            tracing::debug!(
                                "Overlay: GDI handle check OK (baseline={}, current={}, drift=+{})",
                                ws.gdi_baseline, current, drift
                            );
                        }
                    }
                }
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
                0
            }
            WM_PAINT => {
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState;
                if !ws_ptr.is_null() {
                    let ws = &*ws_ptr;
                    let data = ws.data.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    paint_hud(hwnd, &data, &ws.res, &ws.renderer);
                }
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, TIMER_ID);
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
                if !ws_ptr.is_null() {
                    let ws = &*ws_ptr;
                    let final_count = gdi_handle_count();
                    tracing::info!(
                        "Overlay: closing — GDI baseline={}, final={}, drift=+{}",
                        ws.gdi_baseline, final_count,
                        final_count.saturating_sub(ws.gdi_baseline)
                    );
                    // Drop WindowState — GdiResources::drop() frees all 13 cached handles
                    drop(Box::from_raw(ws_ptr));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                tracing::info!("Overlay: GDI resources released");
                PostQuitMessage(0);
                0
            }
            WM_MOUSEACTIVATE => {
                MA_NOACTIVATE as isize
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}

// ─── GDI Painting ────────────────────────────────────────────────────────────

#[cfg(windows)]
unsafe fn paint_hud(
    hwnd: winapi::shared::windef::HWND,
    data: &OverlayData,
    res: &GdiResources,
    renderer: &HudRenderer,
) {
    use winapi::shared::windef::*;
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc.is_null() {
        return;
    }

    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;

    // Double-buffer to prevent flicker
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
    let old_bmp = SelectObject(mem_dc, mem_bmp as *mut _);

    // Background fill (cached brush)
    let bg_rect = RECT { left: 0, top: 0, right: w, bottom: h };
    FillRect(mem_dc, &bg_rect, res.brush_bg);

    // Dispatch to component system
    renderer.paint_all(mem_dc, w, h, data, res);

    // Blit double buffer to screen
    BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

    // Cleanup double buffer (per-paint, not cached)
    SelectObject(mem_dc, old_bmp);
    DeleteObject(mem_bmp as *mut _);
    DeleteDC(mem_dc);

    EndPaint(hwnd, &ps);
}

// ─── GDI Helpers ─────────────────────────────────────────────────────────────

#[cfg(windows)]
unsafe fn create_font(
    _hdc: winapi::shared::windef::HDC,
    face: &str,
    size: i32,
    bold: bool,
) -> winapi::shared::windef::HFONT {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::wingdi::*;

    let mut face_wide = [0u16; 32];
    for (i, c) in OsStr::new(face).encode_wide().enumerate() {
        if i >= 31 { break; }
        face_wide[i] = c;
    }

    CreateFontW(
        -size,                              // height (negative = character height)
        0,                                  // width (auto)
        0,                                  // escapement
        0,                                  // orientation
        if bold { 700 } else { 400 },       // weight
        0,                                  // italic
        0,                                  // underline
        0,                                  // strikeout
        1,                                  // charset (DEFAULT_CHARSET)
        0,                                  // out precision
        0,                                  // clip precision
        5,                                  // quality (CLEARTYPE_QUALITY)
        0,                                  // pitch and family
        face_wide.as_ptr(),
    )
}

#[cfg(windows)]
unsafe fn draw_text_at(
    hdc: winapi::shared::windef::HDC,
    font: winapi::shared::windef::HFONT,
    color: u32,
    x: i32,
    y: i32,
    text: &str,
) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::wingdi::*;

    let old_font = SelectObject(hdc, font as *mut _);
    SetTextColor(hdc, color);
    let wide: Vec<u16> = OsStr::new(text).encode_wide().collect();
    TextOutW(hdc, x, y, wide.as_ptr(), wide.len() as i32);
    SelectObject(hdc, old_font);
}

// ─── Formatting Helpers ──────────────────────────────────────────────────────

fn format_timer(seconds: u32) -> String {
    let m = seconds / 60;
    let s = seconds % 60;
    format!("{:02}:{:02}", m, s)
}

fn format_lap_time(ms: u32) -> String {
    if ms == 0 {
        return "--:--.---".to_string();
    }
    let m = ms / 60_000;
    let s = (ms % 60_000) / 1000;
    let ml = ms % 1000;
    format!("{}:{:02}.{:03}", m, s, ml)
}

fn format_sector(ms: Option<u32>) -> String {
    match ms {
        Some(v) if v > 0 => format!("{:.1}", v as f64 / 1000.0),
        _ => "--.--".to_string(),
    }
}

fn sector_color(
    current_ms: Option<u32>,
    prev_ms: Option<u32>,
    best_ms: Option<u32>,
    default: u32,
    purple: u32,
    green: u32,
    yellow: u32,
) -> u32 {
    match current_ms {
        Some(c) if c > 0 => {
            // Purple: new personal best (or tied)
            if let Some(b) = best_ms {
                if c <= b {
                    return purple;
                }
            }
            // Green: faster than previous lap's same sector
            if let Some(p) = prev_ms {
                if c < p {
                    return green;
                }
            }
            // Yellow: slower than or equal to previous
            // First lap (no prev/best): purple — it IS the best by definition
            if prev_ms.is_some() || best_ms.is_some() {
                yellow
            } else {
                purple
            }
        }
        _ => default,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timer() {
        assert_eq!(format_timer(0), "00:00");
        assert_eq!(format_timer(59), "00:59");
        assert_eq!(format_timer(60), "01:00");
        assert_eq!(format_timer(90), "01:30");
        assert_eq!(format_timer(3599), "59:59");
        assert_eq!(format_timer(3600), "60:00");
    }

    #[test]
    fn test_format_lap_time() {
        assert_eq!(format_lap_time(0), "--:--.---");
        assert_eq!(format_lap_time(1000), "0:01.000");
        assert_eq!(format_lap_time(61_234), "1:01.234");
        assert_eq!(format_lap_time(125_456), "2:05.456");
    }

    #[test]
    fn test_format_sector() {
        assert_eq!(format_sector(None), "--.--");
        assert_eq!(format_sector(Some(0)), "--.--");
        assert_eq!(format_sector(Some(32100)), "32.1");
        assert_eq!(format_sector(Some(1500)), "1.5");
        assert_eq!(format_sector(Some(65432)), "65.4");
    }

    #[test]
    fn test_compute_layout() {
        let rects = compute_layout(1920);
        assert_eq!(rects.len(), 6);
        // Total content = 120+200+100+200+200+60 = 880
        // start_x = (1920 - 880) / 2 = 520
        assert_eq!(rects[0].x, 520);
        assert_eq!(rects[0].w, 120);
        assert_eq!(rects[1].x, 640);  // 520 + 120
        assert_eq!(rects[1].w, 200);
        assert_eq!(rects[2].x, 840);  // 640 + 200
        assert_eq!(rects[2].w, 100);
        assert_eq!(rects[3].x, 940);  // 840 + 100
        assert_eq!(rects[3].w, 200);
        assert_eq!(rects[4].x, 1140); // 940 + 200
        assert_eq!(rects[4].w, 200);
        assert_eq!(rects[5].x, 1340); // 1140 + 200
        assert_eq!(rects[5].w, 60);

        // All rects have y=12 and h=BAR_HEIGHT
        for r in &rects {
            assert_eq!(r.y, 12);
            assert_eq!(r.h, BAR_HEIGHT);
        }

        // Narrow screen: content should start at 0 (clamped)
        let narrow = compute_layout(800);
        assert_eq!(narrow[0].x, 0); // (800-880).max(0)/2 = 0
    }

    #[test]
    fn test_sector_color() {
        let default = 100;
        let purple = 200;
        let green = 300;
        let yellow = 400;

        // No time => default
        assert_eq!(sector_color(None, None, None, default, purple, green, yellow), default);
        assert_eq!(sector_color(Some(0), None, None, default, purple, green, yellow), default);

        // First ever sector (no prev, no best) => purple (it IS the best)
        assert_eq!(sector_color(Some(30000), None, None, default, purple, green, yellow), purple);

        // Matches best => purple
        assert_eq!(sector_color(Some(30000), Some(31000), Some(30000), default, purple, green, yellow), purple);

        // Beats best => purple
        assert_eq!(sector_color(Some(29000), Some(31000), Some(30000), default, purple, green, yellow), purple);

        // Faster than prev but not best => green
        assert_eq!(sector_color(Some(30500), Some(31000), Some(30000), default, purple, green, yellow), green);

        // Slower than prev => yellow
        assert_eq!(sector_color(Some(32000), Some(31000), Some(30000), default, purple, green, yellow), yellow);

        // No prev, has best, slower => yellow
        assert_eq!(sector_color(Some(31000), None, Some(30000), default, purple, green, yellow), yellow);
    }
}
