//! State-driven GDI painter for the native lock screen.
//!
//! Dispatches painting based on `LockScreenState`, using double-buffered GDI
//! rendering (CreateCompatibleDC + CreateCompatibleBitmap + BitBlt) to prevent
//! flicker. Follows the exact pattern from `overlay.rs` paint_hud().

#![allow(unsafe_op_in_unsafe_fn)]

use crate::lock_screen::LockScreenState;
use crate::native_lock::font::LockGdiResources;

/// Paint the lock screen based on current state.
/// Uses double-buffered GDI rendering to prevent flicker.
///
/// `pin_dots` is the dot display string from PinInputState (only used for PinEntry).
/// `pin_count` is the number of digits entered (for placeholder rendering).
///
/// # Safety
/// Must be called from the window thread during WM_PAINT handling.
#[cfg(windows)]
pub unsafe fn paint_lock_screen(
    hwnd: winapi::shared::windef::HWND,
    state: &LockScreenState,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
    pin_dots: &str,
    pin_count: usize,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    if hdc.is_null() {
        return;
    }

    // Double-buffer: create off-screen DC + bitmap
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, screen_w, screen_h);
    let old_bmp = SelectObject(mem_dc, mem_bmp as *mut _);

    let full_rect = RECT {
        left: 0,
        top: 0,
        right: screen_w,
        bottom: screen_h,
    };

    // Dispatch based on state
    match state {
        LockScreenState::ScreenBlanked => {
            // Pure black — nothing else rendered
            FillRect(mem_dc, &full_rect, res.brush_black);
        }

        LockScreenState::StartupConnecting => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_branding(mem_dc, res, screen_w, screen_h);

            let col_white = rgb(255, 255, 255);
            draw_centered_text(mem_dc, "Connecting to server...", res.font_heading, col_white, screen_h / 2 + 20, screen_w);

            let col_grey = rgb(128, 128, 128);
            draw_centered_text(mem_dc, "Please wait", res.font_small, col_grey, screen_h / 2 + 70, screen_w);
        }

        LockScreenState::Disconnected => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_branding(mem_dc, res, screen_w, screen_h);

            let col_red = rgb(225, 6, 0);
            draw_centered_text(mem_dc, "Connection lost", res.font_heading, col_red, screen_h / 2 + 20, screen_w);

            let col_grey = rgb(128, 128, 128);
            draw_centered_text(mem_dc, "Reconnecting...", res.font_small, col_grey, screen_h / 2 + 70, screen_w);
        }

        LockScreenState::PinEntry {
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            pin_error,
            ..
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_pin_entry(
                mem_dc, res, screen_w, screen_h,
                driver_name, pricing_tier_name, *allocated_seconds,
                pin_dots, pin_count, pin_error,
            );
        }

        LockScreenState::QrDisplay {
            qr_payload,
            driver_name,
            pricing_tier_name,
            allocated_seconds,
            ..
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_qr_display(
                mem_dc, res, screen_w, screen_h,
                driver_name, qr_payload, pricing_tier_name, *allocated_seconds,
            );
        }

        LockScreenState::ActiveSession {
            driver_name,
            remaining_seconds,
            allocated_seconds,
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_active_session(
                mem_dc, res, screen_w, screen_h,
                driver_name, *remaining_seconds, *allocated_seconds,
            );
        }

        // All other states: branded placeholder (Plan 03+ add real paint functions)
        _ => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_branding(mem_dc, res, screen_w, screen_h);
        }
    }

    // Blit double buffer to screen
    BitBlt(hdc, 0, 0, screen_w, screen_h, mem_dc, 0, 0, SRCCOPY);

    // Cleanup double buffer
    SelectObject(mem_dc, old_bmp);
    DeleteObject(mem_bmp as *mut _);
    DeleteDC(mem_dc);

    EndPaint(hwnd, &ps);
}

// ---- PIN Entry Painter ----

/// Render the PIN entry screen: branding, driver info card, PIN dot display, error text.
#[cfg(windows)]
unsafe fn paint_pin_entry(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    pricing_tier: &str,
    alloc_secs: u32,
    pin_dots: &str,
    pin_count: usize,
    pin_error: &Option<String>,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::wingdi::*;
    use winapi::um::winuser::FillRect;

    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title: "RACING POINT" in red
    let title_y = h / 2 - 220;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // Card background (centered, ~800px wide, ~400px tall)
    let card_w = 800.min(w - 100);
    let card_h = 400;
    let card_x = (w - card_w) / 2;
    let card_y = h / 2 - 140;
    let card_rect = RECT {
        left: card_x,
        top: card_y,
        right: card_x + card_w,
        bottom: card_y + card_h,
    };
    FillRect(hdc, &card_rect, res.brush_card);

    // "Welcome, {driver_name}" inside card
    let welcome_text = format!("Welcome, {}", driver_name);
    draw_centered_text(hdc, &welcome_text, res.font_heading, col_white, card_y + 30, w);

    // Pricing tier + allocated minutes
    let minutes = alloc_secs / 60;
    let tier_text = format!("{} \u{2014} {} minutes", pricing_tier, minutes);
    draw_centered_text(hdc, &tier_text, res.font_body, col_grey, card_y + 80, w);

    // "Enter your PIN" label
    draw_centered_text(hdc, "Enter your PIN", res.font_small, col_grey, card_y + 130, w);

    // PIN placeholder circles (6 total, filled for entered digits)
    let dot_spacing = 60;
    let max_dots = 6usize;
    let total_dot_width = (max_dots as i32 - 1) * dot_spacing;
    let dot_start_x = (w - total_dot_width) / 2;
    let dot_y = card_y + 190;
    let dot_radius = 18;

    for i in 0..max_dots {
        let cx = dot_start_x + (i as i32) * dot_spacing;
        if i < pin_count {
            // Filled dot — white
            let brush = CreateSolidBrush(rgb(255, 255, 255));
            let pen = CreatePen(PS_SOLID as i32, 1, rgb(255, 255, 255));
            let old_brush = SelectObject(hdc, brush as *mut _);
            let old_pen = SelectObject(hdc, pen as *mut _);
            Ellipse(hdc, cx - dot_radius, dot_y - dot_radius, cx + dot_radius, dot_y + dot_radius);
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(brush as *mut _);
            DeleteObject(pen as *mut _);
        } else {
            // Empty circle — grey outline
            let brush = GetStockObject(NULL_BRUSH as i32);
            let pen = CreatePen(PS_SOLID as i32, 2, rgb(90, 90, 90));
            let old_brush = SelectObject(hdc, brush);
            let old_pen = SelectObject(hdc, pen as *mut _);
            Ellipse(hdc, cx - dot_radius, dot_y - dot_radius, cx + dot_radius, dot_y + dot_radius);
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(pen as *mut _);
        }
    }

    // Error text in red (if present)
    if let Some(err_msg) = pin_error {
        draw_centered_text(hdc, err_msg, res.font_small, col_red, card_y + 260, w);
    }

    // Bottom help text
    draw_centered_text(hdc, "Need help? Ask a staff member", res.font_small, col_grey, card_y + card_h + 20, w);
}

// ---- QR Display Painter ----

/// Render the QR code display screen: branding, driver info, QR code, scan hint.
#[cfg(windows)]
unsafe fn paint_qr_display(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    qr_payload: &str,
    pricing_tier: &str,
    alloc_secs: u32,
) {
    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title: "RACING POINT" in red
    let title_y = h / 2 - 280;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // Driver name + tier info
    let welcome_text = format!("Welcome, {}", driver_name);
    draw_centered_text(hdc, &welcome_text, res.font_heading, col_white, title_y + 70, w);

    let minutes = alloc_secs / 60;
    let tier_text = format!("{} \u{2014} {} minutes", pricing_tier, minutes);
    draw_centered_text(hdc, &tier_text, res.font_body, col_grey, title_y + 120, w);

    // QR code centered in the middle
    // module_size: responsive scaling based on screen height
    let module_size = (h / 80).clamp(4, 8);
    let qr_center_y = h / 2 + 20;
    crate::native_lock::qr::paint_qr_code(hdc, qr_payload, w / 2, qr_center_y, module_size);

    // "Scan to check in" below QR
    draw_centered_text(hdc, "Scan to check in", res.font_body, col_white, qr_center_y + module_size * 20 + 20, w);

    // "Or enter PIN manually" hint
    draw_centered_text(hdc, "Or enter PIN manually", res.font_small, col_grey, qr_center_y + module_size * 20 + 60, w);
}

// ---- Active Session Painter ----

/// Render the active session screen: branding, large countdown timer, progress bar.
#[cfg(windows)]
unsafe fn paint_active_session(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    remaining_secs: u32,
    allocated_secs: u32,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::wingdi::{CreateSolidBrush, DeleteObject};
    use winapi::um::winuser::FillRect;

    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title: "RACING POINT" in red
    let title_y = h / 2 - 200;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // Large countdown timer: MM:SS
    let mins = remaining_secs / 60;
    let secs = remaining_secs % 60;
    let timer_text = format!("{:02}:{:02}", mins, secs);

    // Timer color: red when <= 60s, yellow when <= 300s, white otherwise
    let timer_y = h / 2 - 60;
    if remaining_secs <= 60 {
        draw_centered_text(hdc, &timer_text, res.font_timer, col_red, timer_y, w);
    } else if remaining_secs <= 300 {
        // Yellow warning color — temporary brush not needed for text color
        let col_yellow = rgb(255, 200, 0);
        draw_centered_text(hdc, &timer_text, res.font_timer, col_yellow, timer_y, w);
    } else {
        draw_centered_text(hdc, &timer_text, res.font_timer, col_white, timer_y, w);
    }

    // Driver name below timer
    draw_centered_text(hdc, driver_name, res.font_heading, col_white, h / 2 + 60, w);

    // Progress bar: horizontal red bar showing elapsed/total ratio
    let bar_w = 600.min(w - 200);
    let bar_h = 8;
    let bar_x = (w - bar_w) / 2;
    let bar_y = h / 2 + 120;

    // Background bar (grey)
    let bg_rect = RECT {
        left: bar_x,
        top: bar_y,
        right: bar_x + bar_w,
        bottom: bar_y + bar_h,
    };
    FillRect(hdc, &bg_rect, res.brush_grey);

    // Elapsed bar (red)
    if allocated_secs > 0 {
        let elapsed = allocated_secs.saturating_sub(remaining_secs);
        let progress_w = ((elapsed as i64 * bar_w as i64) / allocated_secs as i64) as i32;
        if progress_w > 0 {
            let progress_rect = RECT {
                left: bar_x,
                top: bar_y,
                right: bar_x + progress_w,
                bottom: bar_y + bar_h,
            };
            FillRect(hdc, &progress_rect, res.brush_red);
        }
    }

    // "Session in progress" footer
    draw_centered_text(hdc, "Session in progress", res.font_small, col_grey, bar_y + 30, w);
}

// ---- Shared Helpers ----

/// Paint the centered "RACING POINT" branding title in Racing Red.
#[cfg(windows)]
unsafe fn paint_branding(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
) {
    let col_red = rgb(225, 6, 0);
    let title_y = screen_h / 2 - 60;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, screen_w);
}

/// Draw text horizontally centered on the screen.
///
/// Uses GetTextExtentPoint32W to measure text width, then positions at
/// `(screen_w / 2 - text_w / 2, y)`.
#[cfg(windows)]
unsafe fn draw_centered_text(
    hdc: winapi::shared::windef::HDC,
    text: &str,
    font: winapi::shared::windef::HFONT,
    color: u32,
    y: i32,
    screen_w: i32,
) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::windef::SIZE;
    use winapi::um::wingdi::*;

    let old_font = SelectObject(hdc, font as *mut _);
    SetTextColor(hdc, color);
    SetBkMode(hdc, TRANSPARENT as i32);

    let wide: Vec<u16> = OsStr::new(text).encode_wide().collect();

    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, wide.as_ptr(), wide.len() as i32, &mut size);

    let x = (screen_w - size.cx) / 2;
    TextOutW(hdc, x, y, wide.as_ptr(), wide.len() as i32);

    SelectObject(hdc, old_font);
}

/// Helper: RGB color value matching Win32 COLORREF format.
#[cfg(windows)]
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

// Non-windows stub
#[cfg(not(windows))]
pub unsafe fn paint_lock_screen(
    _hwnd: *mut std::ffi::c_void,
    _state: &LockScreenState,
    _res: &LockGdiResources,
    _screen_w: i32,
    _screen_h: i32,
    _pin_dots: &str,
    _pin_count: usize,
) {}
