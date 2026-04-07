//! Off-track blanking overlay window (Phase 330).
//!
//! A fullscreen branded overlay that appears when the car goes off-track.
//! Separate from both the lock screen (native_lock/) and the HUD overlay (overlay.rs).
//! Uses the same Win32 GDI pattern as the lock screen: WS_POPUP + WS_EX_TOPMOST
//! spanning the full virtual desktop (7680x1440 for triple monitors).

#![allow(unsafe_op_in_unsafe_fn)]

const LOG_TARGET: &str = "off-track-blanking";

/// Handle to the off-track blanking window (if created).
/// The window is created once and shown/hidden as needed.
static BLANKING_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Show/hide messages sent to the window thread.
const WM_USER_SHOW: u32 = 0x0400 + 100;
const WM_USER_HIDE: u32 = 0x0400 + 101;

/// Manages the off-track blanking overlay lifecycle.
pub struct OffTrackBlanking {
    visible: bool,
    thread_started: bool,
}

impl OffTrackBlanking {
    pub fn new() -> Self {
        Self {
            visible: false,
            thread_started: false,
        }
    }

    /// Show the off-track blanking overlay.
    pub fn show(&mut self) {
        if self.visible {
            return;
        }
        self.visible = true;

        #[cfg(windows)]
        {
            if !self.thread_started {
                self.start_window_thread();
            }
            let hwnd = BLANKING_HWND.load(std::sync::atomic::Ordering::SeqCst);
            if hwnd != 0 {
                unsafe {
                    winapi::um::winuser::PostMessageW(
                        hwnd as winapi::shared::windef::HWND,
                        WM_USER_SHOW,
                        0,
                        0,
                    );
                }
                tracing::info!(target: LOG_TARGET, "Off-track blanking overlay SHOWN");
            } else {
                tracing::warn!(target: LOG_TARGET, "Off-track blanking window not ready yet");
            }
        }

        #[cfg(not(windows))]
        {
            tracing::info!(target: LOG_TARGET, "Off-track blanking overlay SHOWN (no-op on non-Windows)");
        }
    }

    /// Hide the off-track blanking overlay.
    pub fn hide(&mut self) {
        if !self.visible {
            return;
        }
        self.visible = false;

        #[cfg(windows)]
        {
            let hwnd = BLANKING_HWND.load(std::sync::atomic::Ordering::SeqCst);
            if hwnd != 0 {
                unsafe {
                    winapi::um::winuser::PostMessageW(
                        hwnd as winapi::shared::windef::HWND,
                        WM_USER_HIDE,
                        0,
                        0,
                    );
                }
                tracing::info!(target: LOG_TARGET, "Off-track blanking overlay HIDDEN");
            }
        }

        #[cfg(not(windows))]
        {
            tracing::info!(target: LOG_TARGET, "Off-track blanking overlay HIDDEN (no-op on non-Windows)");
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Start the dedicated window thread (called once on first show).
    #[cfg(windows)]
    fn start_window_thread(&mut self) {
        self.thread_started = true;
        std::thread::spawn(|| {
            spawn_blanking_window();
        });
        // Give the window thread time to create the window
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

/// Window thread entry point -- creates and runs the Win32 message loop.
#[cfg(windows)]
fn spawn_blanking_window() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::minwindef::*;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::*;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    // Get virtual screen bounds (covers NVIDIA Surround triple monitors)
    let (vx, vy, vw, vh) = crate::lock_screen::get_virtual_screen_bounds();
    tracing::info!(
        target: LOG_TARGET,
        "Off-track blanking virtual screen: {}x{} at ({}, {})",
        vw, vh, vx, vy
    );

    let class_name = wide("RacingPointOffTrackBlanking");
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(blanking_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(),
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };

    unsafe {
        RegisterClassExW(&wc);
    }

    // Create the window hidden initially (WS_POPUP without WS_VISIBLE)
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW, // TOOLWINDOW: no taskbar entry
            class_name.as_ptr(),
            wide("Racing Point Off-Track").as_ptr(),
            WS_POPUP, // Hidden by default (no WS_VISIBLE)
            vx,
            vy,
            vw,
            vh,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            std::ptr::null_mut(),
        )
    };

    if hwnd.is_null() {
        tracing::error!(target: LOG_TARGET, "Failed to create off-track blanking window");
        return;
    }

    // Store HWND for cross-thread access
    BLANKING_HWND.store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
    tracing::info!(target: LOG_TARGET, "Off-track blanking window created (hidden)");

    // Win32 message loop
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Window procedure for the off-track blanking overlay.
#[cfg(windows)]
unsafe extern "system" fn blanking_wnd_proc(
    hwnd: winapi::shared::windef::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    use winapi::um::wingdi::*;
    use winapi::um::winuser::*;

    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_null() {
                let mut rect: winapi::shared::windef::RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rect);

                // Racing Point Red background (#E10600)
                let bg_brush = CreateSolidBrush(rgb(0xE1, 0x06, 0x00));
                FillRect(hdc, &rect, bg_brush);
                DeleteObject(bg_brush as *mut _);

                // Draw "RACING POINT" branding text centered
                let text = "RACING POINT\0".encode_utf16().collect::<Vec<u16>>();
                SetBkMode(hdc, TRANSPARENT as i32);
                SetTextColor(hdc, rgb(0xFF, 0xFF, 0xFF)); // White text

                // Create large font for branding
                let font = CreateFontW(
                    120, 0, 0, 0,
                    FW_BOLD as i32,
                    0, 0, 0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    DEFAULT_PITCH | FF_SWISS,
                    wide_inline("Montserrat\0").as_ptr(),
                );
                let old_font = SelectObject(hdc, font as *mut _);

                // Center the text
                let mut text_rect = rect;
                DrawTextW(
                    hdc,
                    text.as_ptr(),
                    -1,
                    &mut text_rect,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );

                SelectObject(hdc, old_font);
                DeleteObject(font as *mut _);

                // Draw subtitle below
                let subtitle = "OFF TRACK\0".encode_utf16().collect::<Vec<u16>>();
                let subtitle_font = CreateFontW(
                    60, 0, 0, 0,
                    FW_NORMAL as i32,
                    0, 0, 0,
                    DEFAULT_CHARSET,
                    OUT_DEFAULT_PRECIS,
                    CLIP_DEFAULT_PRECIS,
                    CLEARTYPE_QUALITY,
                    DEFAULT_PITCH | FF_SWISS,
                    wide_inline("Montserrat\0").as_ptr(),
                );
                let old_font2 = SelectObject(hdc, subtitle_font as *mut _);

                let mut sub_rect = rect;
                sub_rect.top = (rect.top + rect.bottom) / 2 + 80;
                DrawTextW(
                    hdc,
                    subtitle.as_ptr(),
                    -1,
                    &mut sub_rect,
                    DT_CENTER | DT_TOP | DT_SINGLELINE,
                );

                SelectObject(hdc, old_font2);
                DeleteObject(subtitle_font as *mut _);

                EndPaint(hwnd, &ps);
            }
            0
        }
        _ if msg == WM_USER_SHOW => {
            ShowWindow(hwnd, SW_SHOW);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            InvalidateRect(hwnd, std::ptr::null(), 1);
            0
        }
        _ if msg == WM_USER_HIDE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

/// RGB macro for Win32 COLORREF.
#[cfg(windows)]
fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

/// Helper to create wide string inline.
#[cfg(windows)]
fn wide_inline(s: &str) -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}
