//! Native Win32 lock screen — replaces Edge browser for lock screen display.
//!
//! Creates a fullscreen WS_POPUP | WS_EX_TOPMOST window spanning the entire
//! virtual desktop (including NVIDIA Surround multi-monitor). Renders lock
//! screen states via GDI double-buffered painting with embedded Montserrat fonts.
//!
//! Architecture:
//! - `font.rs` — Font embedding + GDI resource cache (LockGdiResources)
//! - `window.rs` — Win32 window creation, message loop thread, cross-thread PostMessage API
//! - `painter.rs` — State-driven GDI paint dispatch (ScreenBlanked, StartupConnecting, etc.)

pub mod font;
pub mod painter;
pub mod window;

pub use font::LockGdiResources;

use crate::lock_screen::LockScreenState;
use std::sync::{Arc, Mutex};

const LOG_TARGET: &str = "native-lock";

/// Native Win32 lock screen window, replacing Edge browser.
///
/// Manages a dedicated Win32 message loop thread. The window spans the full
/// virtual desktop and renders lock screen states via GDI.
pub struct NativeLockScreen {
    /// Window handle slot — shared with the window thread.
    /// `Some(hwnd)` when the window is alive, `None` before creation or after destroy.
    hwnd: Arc<Mutex<Option<isize>>>,
    /// Join handle for the window thread. Stored for cleanup.
    window_thread: Option<std::thread::JoinHandle<()>>,
}

impl NativeLockScreen {
    /// Create a new NativeLockScreen with no window.
    pub fn new() -> Self {
        Self {
            hwnd: Arc::new(Mutex::new(None)),
            window_thread: None,
        }
    }

    /// Show the lock screen window. If no window exists, spawns the window thread.
    /// If a window already exists, triggers a repaint.
    pub fn show(&mut self, state: Arc<Mutex<LockScreenState>>) {
        let hwnd_val = {
            let slot = self.hwnd.lock().unwrap_or_else(|e| e.into_inner());
            *slot
        };

        if hwnd_val.is_some() {
            // Window already exists — just repaint
            self.request_repaint();
            return;
        }

        // Spawn a new window thread
        let hwnd_slot = self.hwnd.clone();
        let handle = std::thread::Builder::new()
            .name("lock-screen-window".to_string())
            .spawn(move || {
                window::spawn_lock_window(state, hwnd_slot);
            })
            .expect("failed to spawn lock screen window thread");

        self.window_thread = Some(handle);
        tracing::info!(target: LOG_TARGET, "Lock screen window thread spawned");
    }

    /// Hide the lock screen window (SW_HIDE). Window stays alive for fast re-show.
    #[cfg(windows)]
    pub fn hide(&self) {
        if let Some(hwnd) = self.get_hwnd() {
            // WM_APP + 1 = hide
            unsafe {
                winapi::um::winuser::PostMessageW(
                    hwnd as winapi::shared::windef::HWND,
                    winapi::um::winuser::WM_APP + 1,
                    0,
                    0,
                );
            }
        }
    }

    #[cfg(not(windows))]
    pub fn hide(&self) {}

    /// Destroy the lock screen window. Sends WM_CLOSE which triggers WM_DESTROY -> PostQuitMessage.
    #[cfg(windows)]
    pub fn destroy(&self) {
        if let Some(hwnd) = self.get_hwnd() {
            unsafe {
                winapi::um::winuser::PostMessageW(
                    hwnd as winapi::shared::windef::HWND,
                    winapi::um::winuser::WM_CLOSE,
                    0,
                    0,
                );
            }
            tracing::info!(target: LOG_TARGET, "Sent WM_CLOSE to lock screen window");
        }
    }

    #[cfg(not(windows))]
    pub fn destroy(&self) {}

    /// Check if the window is alive (HWND slot is Some).
    pub fn is_alive(&self) -> bool {
        let slot = self.hwnd.lock().unwrap_or_else(|e| e.into_inner());
        slot.is_some()
    }

    /// Request a repaint of the lock screen window.
    #[cfg(windows)]
    pub fn request_repaint(&self) {
        if let Some(hwnd) = self.get_hwnd() {
            // WM_APP + 2 = repaint
            unsafe {
                winapi::um::winuser::PostMessageW(
                    hwnd as winapi::shared::windef::HWND,
                    winapi::um::winuser::WM_APP + 2,
                    0,
                    0,
                );
            }
        }
    }

    #[cfg(not(windows))]
    pub fn request_repaint(&self) {}

    /// Request the window to show itself and come to foreground.
    #[cfg(windows)]
    pub fn request_show(&self) {
        if let Some(hwnd) = self.get_hwnd() {
            // WM_APP + 3 = show + foreground
            unsafe {
                winapi::um::winuser::PostMessageW(
                    hwnd as winapi::shared::windef::HWND,
                    winapi::um::winuser::WM_APP + 3,
                    0,
                    0,
                );
            }
        }
    }

    #[cfg(not(windows))]
    pub fn request_show(&self) {}

    /// Get the current HWND value.
    fn get_hwnd(&self) -> Option<isize> {
        let slot = self.hwnd.lock().unwrap_or_else(|e| e.into_inner());
        *slot
    }
}
