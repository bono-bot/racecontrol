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
    use winapi::shared::windef::*;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    let class_name = wide("RacingHudOverlay");
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    // Store state pointer as raw for the window proc
    let state_ptr = Box::into_raw(Box::new(state.clone()));

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
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
                0
            }
            WM_TIMER => {
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
                0
            }
            WM_PAINT => {
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const Arc<Mutex<OverlayData>>;
                if !state_ptr.is_null() {
                    let state = &*state_ptr;
                    let data = state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    paint_hud(hwnd, &data);
                }
                0
            }
            WM_DESTROY => {
                KillTimer(hwnd, TIMER_ID);
                let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Arc<Mutex<OverlayData>>;
                if !state_ptr.is_null() {
                    drop(Box::from_raw(state_ptr));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
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
unsafe fn paint_hud(hwnd: winapi::shared::windef::HWND, data: &OverlayData) {
    use winapi::shared::windef::*;
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    fn rgb(r: u8, g: u8, b: u8) -> u32 {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }

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

    // ── Background ──
    let bg_brush = CreateSolidBrush(rgb(18, 18, 18));
    let bg_rect = RECT { left: 0, top: 0, right: w, bottom: h };
    FillRect(mem_dc, &bg_rect, bg_brush);
    DeleteObject(bg_brush as *mut _);

    // ── RPM color bar (top 4px, full width) ──
    // Green at low RPM → Yellow mid → Red near redline
    let rpm_pct = if data.rpm > 0 { (data.rpm as f32 / 18000.0).min(1.0) } else { 0.0 };
    let rpm_bar_w = (rpm_pct * w as f32) as i32;
    let rpm_col = if rpm_pct > 0.90 {
        rgb(225, 6, 0)       // Red — near redline
    } else if rpm_pct > 0.75 {
        rgb(245, 158, 11)    // Amber — high RPM
    } else if rpm_pct > 0.50 {
        rgb(234, 179, 8)     // Yellow — mid RPM
    } else {
        rgb(34, 197, 94)     // Green — low RPM
    };
    let rpm_brush = CreateSolidBrush(rpm_col);
    let rpm_bg_brush = CreateSolidBrush(rgb(30, 30, 30));
    let rpm_bar_rect = RECT { left: 0, top: 0, right: w, bottom: 4 };
    FillRect(mem_dc, &rpm_bar_rect, rpm_bg_brush);
    let rpm_fill_rect = RECT { left: 0, top: 0, right: rpm_bar_w, bottom: 4 };
    FillRect(mem_dc, &rpm_fill_rect, rpm_brush);
    DeleteObject(rpm_brush as *mut _);
    DeleteObject(rpm_bg_brush as *mut _);

    // ── Red accent border below RPM bar (1px) ──
    let red_brush = CreateSolidBrush(rgb(225, 6, 0)); // #E10600
    let top_border = RECT { left: 0, top: 4, right: w, bottom: 6 };
    let bot_border = RECT { left: 0, top: h - 2, right: w, bottom: h };
    FillRect(mem_dc, &top_border, red_brush);
    FillRect(mem_dc, &bot_border, red_brush);
    DeleteObject(red_brush as *mut _);

    SetBkMode(mem_dc, TRANSPARENT as i32);

    // ── Create fonts ──
    let font_label = create_font(mem_dc, "Segoe UI", 11, true);
    let font_value = create_font(mem_dc, "Segoe UI", 22, true);
    let font_gear = create_font(mem_dc, "Segoe UI", 32, true);
    let font_speed = create_font(mem_dc, "Segoe UI", 16, true);
    let font_lap = create_font(mem_dc, "Segoe UI", 18, true);
    let font_sector = create_font(mem_dc, "Segoe UI", 12, true);
    let font_sector_label = create_font(mem_dc, "Segoe UI", 10, false);
    let font_unit = create_font(mem_dc, "Segoe UI", 9, false);

    // ── Section divider ──
    let divider_pen = CreatePen(PS_SOLID as i32, 1, rgb(40, 40, 40));

    // ── Color constants ──
    let col_white: u32 = rgb(255, 255, 255);
    let col_grey: u32 = rgb(85, 85, 85);
    let col_light_grey: u32 = rgb(229, 231, 235); // #E5E7EB
    let col_red: u32 = rgb(225, 6, 0);
    let col_amber: u32 = rgb(245, 158, 11);
    let col_purple: u32 = rgb(168, 85, 247);
    let col_dim: u32 = rgb(68, 68, 68);
    let col_sector_grey: u32 = rgb(160, 160, 160);

    // ── Layout — divide into 6 sections ──
    // Content starts below RPM bar (6px top reserved).
    let section_widths: [i32; 6] = [130, 160, 110, 200, 200, 80]; // Session, Lap, Gear, Prev, Best, LapNum
    let total_content: i32 = section_widths.iter().sum();
    let start_x = (w - total_content).max(0) / 2;

    let mut sx = start_x;
    let label_y = 12;
    let value_y = 28;
    let sector_y = 56; // Row for sector times

    // Helper: draw a divider line at x
    let old_pen = SelectObject(mem_dc, divider_pen as *mut _);

    for (i, &sec_w) in section_widths.iter().enumerate() {
        if i > 0 {
            // Draw vertical divider
            MoveToEx(mem_dc, sx, 8, std::ptr::null_mut());
            LineTo(mem_dc, sx, h - 6);
        }

        match i {
            0 => {
                // ── Session Timer ──
                draw_text_at(mem_dc, font_label, col_grey, sx + 12, label_y, "SESSION");

                let timer_str = format_timer(data.remaining_seconds);
                let timer_col = if data.remaining_seconds <= 10 {
                    col_red
                } else if data.remaining_seconds <= 60 {
                    col_amber
                } else {
                    col_white
                };
                draw_text_at(mem_dc, font_value, timer_col, sx + 12, value_y, &timer_str);
            }
            1 => {
                // ── Current Lap ──
                draw_text_at(mem_dc, font_label, col_grey, sx + 12, label_y, "CURRENT LAP");

                let lap_str = format_lap_time(data.current_lap_time_ms);
                let lap_col = if data.current_lap_invalid { rgb(255, 138, 132) } else { col_white };
                // Red left border indicator for invalid laps
                if data.current_lap_invalid {
                    let inv_brush = CreateSolidBrush(col_red);
                    let inv_rect = RECT { left: sx + 8, top: value_y - 2, right: sx + 11, bottom: value_y + 24 };
                    FillRect(mem_dc, &inv_rect, inv_brush);
                    DeleteObject(inv_brush as *mut _);
                }
                draw_text_at(mem_dc, font_value, lap_col, sx + 16, value_y, &lap_str);

                // Live sector indicator below current lap
                let sector_label = match data.current_sector {
                    0 => "S1",
                    1 => "S2",
                    2 => "S3",
                    _ => "S1",
                };
                draw_text_at(mem_dc, font_sector_label, col_dim, sx + 16, sector_y, sector_label);
            }
            2 => {
                // ── Gear + Speed ──
                let gear_str = match data.gear {
                    0 => "N".to_string(),
                    g if g < 0 => "R".to_string(),
                    g => g.to_string(),
                };
                draw_text_at(mem_dc, font_gear, col_white, sx + 12, 14, &gear_str);

                let speed_str = if data.speed_kmh > 0.0 {
                    format!("{}", data.speed_kmh.round() as i32)
                } else {
                    "---".to_string()
                };
                draw_text_at(mem_dc, font_speed, rgb(187, 187, 187), sx + 52, 18, &speed_str);
                draw_text_at(mem_dc, font_unit, col_dim, sx + 52, 38, "KM/H");

                // RPM number below
                if data.rpm > 0 {
                    let rpm_str = format!("{}", data.rpm);
                    draw_text_at(mem_dc, font_sector_label, col_dim, sx + 52, sector_y, &rpm_str);
                }
            }
            3 => {
                // ── Previous Lap ──
                draw_text_at(mem_dc, font_label, col_grey, sx + 12, label_y, "PREV");

                if let Some(ref prev) = data.previous_lap {
                    let prev_str = format_lap_time(prev.lap_time_ms);
                    draw_text_at(mem_dc, font_lap, col_light_grey, sx + 12, value_y, &prev_str);

                    // Sector times — prominent row
                    let mut sector_x = sx + 12;
                    for (label, ms, best_ms) in [
                        ("S1", prev.sector1_ms, data.best_lap.as_ref().and_then(|b| b.sector1_ms)),
                        ("S2", prev.sector2_ms, data.best_lap.as_ref().and_then(|b| b.sector2_ms)),
                        ("S3", prev.sector3_ms, data.best_lap.as_ref().and_then(|b| b.sector3_ms)),
                    ] {
                        draw_text_at(mem_dc, font_sector_label, col_dim, sector_x, sector_y, label);
                        sector_x += 16;
                        let val_str = format_sector(ms);
                        let col = sector_color(ms, best_ms, col_sector_grey, col_purple, rgb(34, 197, 94), col_amber);
                        draw_text_at(mem_dc, font_sector, col, sector_x, sector_y - 1, &val_str);
                        sector_x += 46;
                    }
                } else {
                    draw_text_at(mem_dc, font_lap, rgb(51, 51, 51), sx + 12, value_y, "--:--.---");
                }
            }
            4 => {
                // ── Best Lap ──
                draw_text_at(mem_dc, font_label, col_purple, sx + 12, label_y, "BEST");

                if let Some(ref best) = data.best_lap {
                    let best_str = format_lap_time(best.lap_time_ms);
                    draw_text_at(mem_dc, font_lap, col_purple, sx + 12, value_y, &best_str);

                    // Sector times — always purple for best
                    let mut sector_x = sx + 12;
                    for (label, ms) in [("S1", best.sector1_ms), ("S2", best.sector2_ms), ("S3", best.sector3_ms)] {
                        draw_text_at(mem_dc, font_sector_label, rgb(120, 60, 180), sector_x, sector_y, label);
                        sector_x += 16;
                        draw_text_at(mem_dc, font_sector, col_purple, sector_x, sector_y - 1, &format_sector(ms));
                        sector_x += 46;
                    }
                } else {
                    draw_text_at(mem_dc, font_lap, rgb(51, 51, 51), sx + 12, value_y, "--:--.---");
                }
            }
            5 => {
                // ── Lap Counter ──
                draw_text_at(mem_dc, font_label, col_grey, sx + 12, label_y, "LAP");

                let lap_num_str = if data.current_lap_number > 0 {
                    data.current_lap_number.to_string()
                } else {
                    "-".to_string()
                };
                draw_text_at(mem_dc, font_value, col_white, sx + 12, value_y, &lap_num_str);

                // INV badge
                if data.current_lap_invalid {
                    let badge_x = sx + 42;
                    let badge_y = value_y + 2;
                    let badge_brush = CreateSolidBrush(col_red);
                    let badge_rect = RECT {
                        left: badge_x,
                        top: badge_y,
                        right: badge_x + 30,
                        bottom: badge_y + 16,
                    };
                    FillRect(mem_dc, &badge_rect, badge_brush);
                    DeleteObject(badge_brush as *mut _);
                    let font_badge = create_font(mem_dc, "Segoe UI", 9, true);
                    draw_text_at(mem_dc, font_badge, col_white, badge_x + 4, badge_y + 1, "INV");
                    DeleteObject(font_badge as *mut _);
                }
            }
            _ => {}
        }

        sx += sec_w;
    }

    // Cleanup GDI objects
    SelectObject(mem_dc, old_pen as *mut _);
    DeleteObject(divider_pen as *mut _);
    DeleteObject(font_label as *mut _);
    DeleteObject(font_value as *mut _);
    DeleteObject(font_gear as *mut _);
    DeleteObject(font_speed as *mut _);
    DeleteObject(font_lap as *mut _);
    DeleteObject(font_sector as *mut _);
    DeleteObject(font_sector_label as *mut _);
    DeleteObject(font_unit as *mut _);

    // Blit double buffer to screen
    BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

    // Cleanup double buffer
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

fn sector_color(prev_ms: Option<u32>, best_ms: Option<u32>, default: u32, purple: u32, green: u32, yellow: u32) -> u32 {
    match (prev_ms, best_ms) {
        (Some(p), Some(b)) if p > 0 && b > 0 => {
            if p <= b {
                purple
            } else if p.saturating_sub(b) <= 300 {
                green
            } else {
                yellow
            }
        }
        _ => default,
    }
}
