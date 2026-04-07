//! Win32 window creation and message loop for the native lock screen.
//!
//! Spawns a dedicated `std::thread` with a Win32 message loop (GetMessage/
//! TranslateMessage/DispatchMessage). The window is WS_POPUP | WS_EX_TOPMOST
//! spanning the full virtual desktop via `get_virtual_screen_bounds()`.

#![allow(unsafe_op_in_unsafe_fn)]

use crate::lock_screen::{get_virtual_screen_bounds, LockScreenState};
use crate::native_lock::font::{self, LockGdiResources};
use crate::native_lock::painter;
use std::sync::{Arc, Mutex};

const LOG_TARGET: &str = "lock-screen-window";

/// Timer ID for the 1-second repaint timer.
const TIMER_ID: usize = 100;

/// Repaint interval in milliseconds (1 second for countdown timer).
const REPAINT_INTERVAL_MS: u32 = 1000;

/// Internal state held on the window thread, stored in GWLP_USERDATA.
#[cfg(windows)]
struct LockWindowState {
    state: Arc<Mutex<LockScreenState>>,
    res: LockGdiResources,
    screen_w: i32,
    screen_h: i32,
}

/// Entry point for the lock screen window thread.
/// Called from `NativeLockScreen::show()` on a dedicated `std::thread`.
///
/// 1. Installs embedded fonts
/// 2. Registers window class
/// 3. Creates fullscreen WS_POPUP | WS_EX_TOPMOST window
/// 4. Runs Win32 message loop until WM_QUIT
/// 5. Cleans up fonts on exit
#[cfg(windows)]
pub fn spawn_lock_window(state: Arc<Mutex<LockScreenState>>, hwnd_slot: Arc<Mutex<Option<isize>>>) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::shared::minwindef::*;
    use winapi::um::libloaderapi::GetModuleHandleW;
    use winapi::um::winuser::*;

    fn wide(s: &str) -> Vec<u16> {
        OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    // Step 1: Install embedded fonts (must happen on this thread before CreateFont)
    let fonts_ok = font::install_embedded_fonts();
    if !fonts_ok {
        tracing::warn!(target: LOG_TARGET, "Font installation failed — will use system font fallback");
    }

    // Step 2: Get virtual screen bounds (covers NVIDIA Surround)
    let (vx, vy, vw, vh) = get_virtual_screen_bounds();
    tracing::info!(target: LOG_TARGET, "Virtual screen: {}x{} at ({}, {})", vw, vh, vx, vy);

    // Step 3: Create GDI resources and window state on this thread
    let window_state = unsafe {
        let res = LockGdiResources::new();
        Box::new(LockWindowState {
            state: state.clone(),
            res,
            screen_w: vw,
            screen_h: vh,
        })
    };
    let state_ptr = Box::into_raw(window_state);

    // Step 4: Register window class
    let class_name = wide("RacingPointLockScreen");
    let hinstance = unsafe { GetModuleHandleW(std::ptr::null()) };

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(lock_wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: std::ptr::null_mut(),
        hCursor: std::ptr::null_mut(), // Cursor managed per-state in WM_SETCURSOR
        hbrBackground: std::ptr::null_mut(),
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_ptr(),
        hIconSm: std::ptr::null_mut(),
    };

    unsafe {
        RegisterClassExW(&wc);
    }

    // Step 5: Create fullscreen window spanning virtual desktop
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOPMOST,
            class_name.as_ptr(),
            wide("Racing Point Lock Screen").as_ptr(),
            WS_POPUP | WS_VISIBLE,
            vx,
            vy,
            vw,
            vh,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinstance,
            state_ptr as LPVOID,
        )
    };

    if hwnd.is_null() {
        tracing::error!(target: LOG_TARGET, "CreateWindowExW failed for lock screen");
        unsafe { drop(Box::from_raw(state_ptr)); }
        font::uninstall_embedded_fonts();
        return;
    }

    // Store HWND so other threads can PostMessage to us
    {
        let mut slot = hwnd_slot.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(hwnd as isize);
    }

    // Start 1-second repaint timer (for countdown displays)
    unsafe {
        SetTimer(hwnd, TIMER_ID, REPAINT_INTERVAL_MS, None);
    }

    tracing::info!(
        target: LOG_TARGET,
        "Native lock screen window created: {}x{} at ({}, {})",
        vw, vh, vx, vy
    );

    // Step 6: Message loop
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Cleanup: clear hwnd slot
    {
        let mut slot = hwnd_slot.lock().unwrap_or_else(|e| e.into_inner());
        *slot = None;
    }

    // Uninstall fonts
    font::uninstall_embedded_fonts();
    tracing::info!(target: LOG_TARGET, "Lock screen window thread exiting");
}

#[cfg(not(windows))]
pub fn spawn_lock_window(
    _state: Arc<Mutex<LockScreenState>>,
    _hwnd_slot: Arc<Mutex<Option<isize>>>,
) {
    tracing::warn!("Native lock screen not supported on non-Windows platforms");
}

// ---- Window Procedure ----

#[cfg(windows)]
unsafe extern "system" fn lock_wnd_proc(
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
                tracing::info!(target: LOG_TARGET, "Lock screen GDI resources cached (11 handles)");
                0
            }

            WM_TIMER => {
                // 1-second repaint timer — triggers WM_PAINT
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
                0
            }

            WM_PAINT => {
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const LockWindowState;
                if !ws_ptr.is_null() {
                    let ws = &*ws_ptr;
                    let state = ws.state.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    painter::paint_lock_screen(hwnd, &state, &ws.res, ws.screen_w, ws.screen_h);
                }
                0
            }

            // WM_APP + 1: Hide window
            x if x == WM_APP + 1 => {
                ShowWindow(hwnd, SW_HIDE);
                0
            }

            // WM_APP + 2: Request repaint
            x if x == WM_APP + 2 => {
                InvalidateRect(hwnd, std::ptr::null(), FALSE);
                0
            }

            // WM_APP + 3: Show window + bring to foreground
            x if x == WM_APP + 3 => {
                ShowWindow(hwnd, SW_SHOW);
                SetForegroundWindow(hwnd);
                0
            }

            WM_SETCURSOR => {
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const LockWindowState;
                if !ws_ptr.is_null() {
                    let ws = &*ws_ptr;
                    let state = ws.state.lock().unwrap_or_else(|e| e.into_inner());
                    if matches!(*state, LockScreenState::ScreenBlanked) {
                        // Hide cursor on blanked screen
                        SetCursor(std::ptr::null_mut());
                        return TRUE as isize;
                    }
                }
                // Show normal arrow cursor for all other states
                SetCursor(LoadCursorW(std::ptr::null_mut(), IDC_ARROW));
                TRUE as isize
            }

            WM_DESTROY => {
                KillTimer(hwnd, TIMER_ID);
                let ws_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LockWindowState;
                if !ws_ptr.is_null() {
                    // Drop LockWindowState — LockGdiResources::drop() frees all cached handles
                    drop(Box::from_raw(ws_ptr));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                tracing::info!(target: LOG_TARGET, "Lock screen GDI resources released");
                PostQuitMessage(0);
                0
            }

            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
