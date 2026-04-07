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
/// # Safety
/// Must be called from the window thread during WM_PAINT handling.
#[cfg(windows)]
pub unsafe fn paint_lock_screen(
    hwnd: winapi::shared::windef::HWND,
    state: &LockScreenState,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
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

            // "Connecting to server..." heading in white
            let col_white = rgb(255, 255, 255);
            draw_centered_text(mem_dc, "Connecting to server...", res.font_heading, col_white, screen_h / 2 + 20, screen_w);

            // "Please wait" subtext in grey
            let col_grey = rgb(128, 128, 128);
            draw_centered_text(mem_dc, "Please wait", res.font_small, col_grey, screen_h / 2 + 70, screen_w);
        }

        LockScreenState::Disconnected => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_branding(mem_dc, res, screen_w, screen_h);

            // "Connection lost" heading in red
            let col_red = rgb(225, 6, 0);
            draw_centered_text(mem_dc, "Connection lost", res.font_heading, col_red, screen_h / 2 + 20, screen_w);

            // "Reconnecting..." subtext in grey
            let col_grey = rgb(128, 128, 128);
            draw_centered_text(mem_dc, "Reconnecting...", res.font_small, col_grey, screen_h / 2 + 70, screen_w);
        }

        // All other states: branded placeholder (Plan 02/03 add real paint functions)
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

/// Paint the centered "RACING POINT" branding title in Racing Red.
#[cfg(windows)]
unsafe fn paint_branding(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
) {
    let col_red = rgb(225, 6, 0);
    // Center vertically, slightly above middle
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
) {}
