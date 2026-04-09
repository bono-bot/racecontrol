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
            // Asphalt background with centered Racing Point logo
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_blanking_logo(mem_dc, res, screen_w, screen_h);
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

        LockScreenState::Hidden => {
            // Window should be SW_HIDE, but paint black as safety
            FillRect(mem_dc, &full_rect, res.brush_black);
        }

        LockScreenState::SessionSummary {
            driver_name,
            total_laps,
            best_lap_ms,
            driving_seconds,
            top_speed_kmh,
            race_position,
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_session_summary(
                mem_dc, res, screen_w, screen_h,
                driver_name, *total_laps, *best_lap_ms, *driving_seconds,
                *top_speed_kmh, *race_position,
            );
        }

        LockScreenState::BetweenSessions {
            driver_name,
            total_laps,
            best_lap_ms,
            driving_seconds,
            wallet_balance_paise,
            current_split_number,
            total_splits,
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_between_sessions(
                mem_dc, res, screen_w, screen_h,
                driver_name, *total_laps, *best_lap_ms, *driving_seconds,
                *wallet_balance_paise, *current_split_number, *total_splits,
            );
        }

        LockScreenState::LaunchSplash {
            driver_name,
            message,
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_launch_splash(mem_dc, res, screen_w, screen_h, driver_name, message);
        }

        LockScreenState::AwaitingAssistance {
            driver_name,
            message,
        } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_awaiting_assistance(mem_dc, res, screen_w, screen_h, driver_name, message);
        }

        LockScreenState::ConfigError { .. } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_config_error(mem_dc, res, screen_w, screen_h);
        }

        LockScreenState::Lockdown { message } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_lockdown(mem_dc, res, screen_w, screen_h, message);
        }

        LockScreenState::MaintenanceRequired { failures } => {
            FillRect(mem_dc, &full_rect, res.brush_asphalt);
            paint_maintenance_required(mem_dc, res, screen_w, screen_h, failures);
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

// ---- Session Summary Painter ----

/// Format a lap time in milliseconds as M:SS.mmm or "N/A".
fn format_lap_time(ms: Option<u32>) -> String {
    match ms {
        Some(ms) => {
            let mins = ms / 60000;
            let secs = (ms % 60000) / 1000;
            let millis = ms % 1000;
            format!("{}:{:02}.{:03}", mins, secs, millis)
        }
        None => "N/A".to_string(),
    }
}

/// Render the session summary screen: branding, stats card with laps/time/speed/position.
#[cfg(windows)]
unsafe fn paint_session_summary(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    top_speed_kmh: Option<f32>,
    race_position: Option<u32>,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::winuser::FillRect;

    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title
    let title_y = h / 2 - 280;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Session Complete" heading
    draw_centered_text(hdc, "Session Complete", res.font_heading, col_white, title_y + 70, w);

    // Card background
    let card_w = 900.min(w - 100);
    let card_h = 360;
    let card_x = (w - card_w) / 2;
    let card_y = h / 2 - 140;
    let card_rect = RECT {
        left: card_x,
        top: card_y,
        right: card_x + card_w,
        bottom: card_y + card_h,
    };
    FillRect(hdc, &card_rect, res.brush_card);

    // Driver name inside card
    let name_text = format!("Great drive, {}!", driver_name);
    draw_centered_text(hdc, &name_text, res.font_heading, col_white, card_y + 30, w);

    // Stats grid: 2 columns, 3 rows
    let col1_x = card_x + card_w / 4;
    let col2_x = card_x + 3 * card_w / 4;
    let row_start = card_y + 100;
    let row_gap = 80;

    // Row 1: Total Laps | Best Lap
    draw_centered_text(hdc, &total_laps.to_string(), res.font_heading, col_white, row_start, col1_x * 2);
    draw_centered_text(hdc, "Total Laps", res.font_small, col_grey, row_start + 40, col1_x * 2);

    let best_lap = format_lap_time(best_lap_ms);
    // For col2: draw at offset so it centers around col2_x
    draw_stat_at(hdc, res, &best_lap, "Best Lap", col_white, col_grey, col2_x, row_start, w);

    // Row 2: Drive Time | Top Speed
    let mins = driving_seconds / 60;
    let secs = driving_seconds % 60;
    let time_text = format!("{}m {:02}s", mins, secs);
    draw_stat_at(hdc, res, &time_text, "Drive Time", col_white, col_grey, col1_x, row_start + row_gap, w);

    let speed_text = match top_speed_kmh {
        Some(spd) if spd > 0.0 => format!("{:.0} km/h", spd),
        _ => "N/A".to_string(),
    };
    draw_stat_at(hdc, res, &speed_text, "Top Speed", col_white, col_grey, col2_x, row_start + row_gap, w);

    // Row 3: Position (left col only)
    let pos_text = match race_position {
        Some(pos) => format!("P{}", pos),
        None => "N/A".to_string(),
    };
    draw_stat_at(hdc, res, &pos_text, "Position", col_white, col_grey, col1_x, row_start + row_gap * 2, w);
}

// ---- Between Sessions Painter ----

/// Render the between-sessions screen: stats, wallet balance, split progress.
#[cfg(windows)]
unsafe fn paint_between_sessions(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    total_laps: u32,
    best_lap_ms: Option<u32>,
    driving_seconds: u32,
    wallet_balance_paise: i64,
    split_num: u32,
    total_splits: u32,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::winuser::FillRect;

    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title
    let title_y = h / 2 - 280;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Ready for Next Race" heading
    draw_centered_text(hdc, "Ready for Next Race", res.font_heading, col_white, title_y + 70, w);

    // Driver name
    let name_text = format!("Great driving, {}!", driver_name);
    draw_centered_text(hdc, &name_text, res.font_body, col_grey, title_y + 120, w);

    // Card for stats
    let card_w = 800.min(w - 100);
    let card_h = 280;
    let card_x = (w - card_w) / 2;
    let card_y = h / 2 - 80;
    let card_rect = RECT {
        left: card_x,
        top: card_y,
        right: card_x + card_w,
        bottom: card_y + card_h,
    };
    FillRect(hdc, &card_rect, res.brush_card);

    // Stats row: Laps | Best Lap | Drive Time
    let col_count = 3;
    let col_spacing = card_w / col_count;
    let stats_y = card_y + 30;

    draw_stat_at(hdc, res, &total_laps.to_string(), "Laps", col_white, col_grey,
        card_x + col_spacing / 2, stats_y, w);

    let best_lap = format_lap_time(best_lap_ms);
    draw_stat_at(hdc, res, &best_lap, "Best Lap", col_white, col_grey,
        card_x + col_spacing + col_spacing / 2, stats_y, w);

    let mins = driving_seconds / 60;
    let secs = driving_seconds % 60;
    let time_text = format!("{}m {:02}s", mins, secs);
    draw_stat_at(hdc, res, &time_text, "Drive Time", col_white, col_grey,
        card_x + 2 * col_spacing + col_spacing / 2, stats_y, w);

    // Wallet balance
    let rupees = wallet_balance_paise / 100;
    let paise = (wallet_balance_paise % 100).unsigned_abs();
    let balance_text = format!("Balance: Rs. {}.{:02}", rupees, paise);
    draw_centered_text(hdc, &balance_text, res.font_body, col_white, card_y + 160, w);

    // Split progress
    let split_text = format!("Race {} of {}", split_num, total_splits);
    draw_centered_text(hdc, &split_text, res.font_small, col_grey, card_y + 200, w);

    // Hint
    draw_centered_text(hdc, "Select your next race on the kiosk", res.font_small, col_grey, card_y + card_h + 20, w);
}

// ---- Launch Splash Painter ----

/// Render the launch splash screen: branding, animated dots, driver name, message.
#[cfg(windows)]
unsafe fn paint_launch_splash(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    message: &str,
) {
    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Title
    let title_y = h / 2 - 120;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Loading..." with animated dots
    let tick = winapi::um::sysinfoapi::GetTickCount();
    let dot_count = ((tick / 500) % 3) + 1;
    let dots: String = ".".repeat(dot_count as usize);
    let loading_text = format!("Loading{}", dots);
    draw_centered_text(hdc, &loading_text, res.font_heading, col_white, title_y + 70, w);

    // Driver name
    draw_centered_text(hdc, driver_name, res.font_heading, col_white, title_y + 130, w);

    // Message (e.g., "Launching Assetto Corsa...")
    draw_centered_text(hdc, message, res.font_body, col_grey, title_y + 180, w);
}

// ---- Awaiting Assistance Painter ----

/// Render the awaiting assistance screen: branding, yellow heading, driver name, message.
#[cfg(windows)]
unsafe fn paint_awaiting_assistance(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    driver_name: &str,
    message: &str,
) {
    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);
    let col_yellow = rgb(255, 200, 0);

    // Title
    let title_y = h / 2 - 120;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Staff Assistance Required" in yellow
    draw_centered_text(hdc, "Staff Assistance Required", res.font_heading, col_yellow, title_y + 70, w);

    // Driver name + message
    draw_centered_text(hdc, driver_name, res.font_heading, col_white, title_y + 130, w);
    draw_centered_text(hdc, message, res.font_body, col_white, title_y + 180, w);

    // Footer
    draw_centered_text(hdc, "A staff member will be with you shortly", res.font_small, col_grey, title_y + 240, w);
}

// ---- Config Error Painter ----

/// Render the config error screen: branding, red heading, generic message.
/// Does NOT show technical details (per LockScreenState::ConfigError docs).
#[cfg(windows)]
unsafe fn paint_config_error(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
) {
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);
    let col_white = rgb(255, 255, 255);

    // Title
    let title_y = h / 2 - 100;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Configuration Error" in red
    draw_centered_text(hdc, "Configuration Error", res.font_heading, col_red, title_y + 70, w);

    // Generic message
    draw_centered_text(hdc, "Please contact staff", res.font_body, col_white, title_y + 130, w);

    // Footer
    draw_centered_text(hdc, "Error details have been logged", res.font_small, col_grey, title_y + 180, w);
}

// ---- Lockdown Painter ----

/// Render the lockdown screen: red strip, heading, message, footer.
#[cfg(windows)]
unsafe fn paint_lockdown(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    message: &str,
) {
    use winapi::shared::windef::RECT;
    use winapi::um::winuser::FillRect;

    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);

    // Red warning strip at top (8px tall)
    let strip_rect = RECT {
        left: 0,
        top: 0,
        right: w,
        bottom: 8,
    };
    FillRect(hdc, &strip_rect, res.brush_red);

    // "KIOSK LOCKDOWN" heading in red
    let title_y = h / 2 - 60;
    draw_centered_text(hdc, "KIOSK LOCKDOWN", res.font_title, col_red, title_y, w);

    // Message
    draw_centered_text(hdc, message, res.font_body, col_white, title_y + 70, w);

    // Footer
    draw_centered_text(hdc, "Contact staff to unlock", res.font_small, col_grey, title_y + 120, w);
}

// ---- Maintenance Required Painter ----

/// Render the maintenance required screen: branding, yellow heading, failure list.
#[cfg(windows)]
unsafe fn paint_maintenance_required(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    w: i32,
    h: i32,
    failures: &[String],
) {
    let col_white = rgb(255, 255, 255);
    let col_grey = rgb(128, 128, 128);
    let col_red = rgb(225, 6, 0);
    let col_yellow = rgb(255, 200, 0);

    // Title
    let title_y = h / 2 - 180;
    draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, title_y, w);

    // "Maintenance Required" heading in yellow
    draw_centered_text(hdc, "Maintenance Required", res.font_heading, col_yellow, title_y + 70, w);

    // Failure list (up to 5 items)
    let max_items = 5;
    let list_y = title_y + 130;
    let line_gap = 35;
    let display_count = failures.len().min(max_items);

    for (i, failure) in failures.iter().take(display_count).enumerate() {
        let text = format!("- {}", failure);
        draw_centered_text(hdc, &text, res.font_body, col_white, list_y + (i as i32) * line_gap, w);
    }

    if failures.len() > max_items {
        let more_text = format!("...and {} more", failures.len() - max_items);
        draw_centered_text(hdc, &more_text, res.font_small, col_grey, list_y + (display_count as i32) * line_gap, w);
    }

    // Footer
    let footer_y = list_y + (display_count as i32 + 1) * line_gap;
    draw_centered_text(hdc, "Staff has been notified", res.font_small, col_grey, footer_y, w);
}

// ---- Stat Helper ----

/// Draw a stat value + label centered at a given x position.
/// Used by SessionSummary and BetweenSessions for grid layout.
#[cfg(windows)]
unsafe fn draw_stat_at(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    value: &str,
    label: &str,
    value_color: u32,
    label_color: u32,
    center_x: i32,
    y: i32,
    _screen_w: i32,
) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::windef::SIZE;
    use winapi::um::wingdi::*;

    // Draw value
    let old_font = SelectObject(hdc, res.font_heading as *mut _);
    SetTextColor(hdc, value_color);
    SetBkMode(hdc, TRANSPARENT as i32);

    let wide_val: Vec<u16> = OsStr::new(value).encode_wide().collect();
    let mut size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, wide_val.as_ptr(), wide_val.len() as i32, &mut size);
    let x = center_x - size.cx / 2;
    TextOutW(hdc, x, y, wide_val.as_ptr(), wide_val.len() as i32);

    // Draw label below value
    SelectObject(hdc, res.font_small as *mut _);
    SetTextColor(hdc, label_color);

    let wide_lbl: Vec<u16> = OsStr::new(label).encode_wide().collect();
    let mut lbl_size: SIZE = std::mem::zeroed();
    GetTextExtentPoint32W(hdc, wide_lbl.as_ptr(), wide_lbl.len() as i32, &mut lbl_size);
    let lx = center_x - lbl_size.cx / 2;
    TextOutW(hdc, lx, y + 40, wide_lbl.as_ptr(), wide_lbl.len() as i32);

    SelectObject(hdc, old_font);
}

// ---- Shared Helpers ----

/// Paint the blanking screen: centered logo + "RACING POINT" text below.
/// If logo bitmap is unavailable, falls back to text-only branding.
#[cfg(windows)]
unsafe fn paint_blanking_logo(
    hdc: winapi::shared::windef::HDC,
    res: &LockGdiResources,
    screen_w: i32,
    screen_h: i32,
) {
    use winapi::um::wingdi::*;

    // Scale logo to fit screen — max 400px height, maintain aspect ratio
    let max_logo_h = 400.min(screen_h / 3);

    if let Some(logo_bmp) = res.logo_bitmap {
        if res.logo_w > 0 && res.logo_h > 0 {
            // Calculate scaled dimensions maintaining aspect ratio
            let scale = (max_logo_h as f64) / (res.logo_h as f64);
            let draw_w = (res.logo_w as f64 * scale) as i32;
            let draw_h = max_logo_h;

            // Center horizontally, place above center vertically
            let x = (screen_w - draw_w) / 2;
            let y = screen_h / 2 - draw_h / 2 - 40;

            // Create a memory DC for the logo bitmap
            let logo_dc = CreateCompatibleDC(hdc);
            let old_obj = SelectObject(logo_dc, logo_bmp as *mut _);

            // AlphaBlend for PNG transparency support
            let blend_fn = winapi::um::wingdi::BLENDFUNCTION {
                BlendOp: 0, // AC_SRC_OVER
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1, // AC_SRC_ALPHA (pre-multiplied)
            };

            // Link msimg32.lib for AlphaBlend
            #[link(name = "msimg32")]
            unsafe extern "system" {
                fn AlphaBlend(
                    hdc_dest: winapi::shared::windef::HDC,
                    x_dest: i32, y_dest: i32, w_dest: i32, h_dest: i32,
                    hdc_src: winapi::shared::windef::HDC,
                    x_src: i32, y_src: i32, w_src: i32, h_src: i32,
                    blend: winapi::um::wingdi::BLENDFUNCTION,
                ) -> i32;
            }

            AlphaBlend(
                hdc, x, y, draw_w, draw_h,
                logo_dc, 0, 0, res.logo_w, res.logo_h,
                blend_fn,
            );

            SelectObject(logo_dc, old_obj);
            DeleteDC(logo_dc);

            // "RACING POINT" text below logo
            let col_red = rgb(225, 6, 0);
            let text_y = y + draw_h + 20;
            draw_centered_text(hdc, "RACING POINT", res.font_title, col_red, text_y, screen_w);

            // "ESPORTS" subtitle
            let col_grey = rgb(128, 128, 128);
            draw_centered_text(hdc, "E S P O R T S", res.font_body, col_grey, text_y + 60, screen_w);

            return;
        }
    }

    // Fallback: text-only branding (same as other states)
    paint_branding(hdc, res, screen_w, screen_h);
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
