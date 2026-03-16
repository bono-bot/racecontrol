//! Kiosk mode security for gaming PCs.
//!
//! Prevents customers from accessing system files, desktop, taskbar,
//! and other unauthorized applications while using the sim rig.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use sysinfo::System;
use tracing;

/// Processes that are always allowed to run (case-insensitive basenames).
const ALLOWED_PROCESSES: &[&str] = &[
    // System essentials
    "system",
    "system idle process",
    "svchost.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "lsass.exe",
    "services.exe",
    "smss.exe",
    "dwm.exe",
    "fontdrvhost.exe",
    "sihost.exe",
    "taskhostw.exe",
    "runtimebroker.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "textinputhost.exe",
    "shellexperiencehost.exe",
    "ctfmon.exe",
    "conhost.exe",
    "dllhost.exe",
    "wudfhost.exe",
    "audiodg.exe",
    "searchindexer.exe",
    "securityhealthservice.exe",
    "securityhealthsystray.exe",
    "sgrmbroker.exe",
    "spoolsv.exe",
    "msiexec.exe",
    "registry",
    "memory compression",
    "dashost.exe",
    "wmiprvse.exe",
    "lsaiso.exe",
    "wlms.exe",
    "unsecapp.exe",
    "settingsynchost.exe",
    "backgroundtaskhost.exe",

    // Windows shell
    "explorer.exe",

    // GPU / Display
    "nvcontainer.exe",
    "nvdisplay.container.exe",
    "nvspcaps64.exe",
    "nvidia share.exe",
    "nvidia web helper.exe",
    "nvidia overlay.exe",

    // RaceControl services
    "rc-agent.exe",
    "rc-agent",
    "pod-agent.exe",
    "pod-agent",

    // Windows shell tools (used by watchdog, minimize, etc.)
    "cmd.exe",
    "powershell.exe",
    "curl.exe",
    "wscript.exe",
    "cscript.exe",
    "schtasks.exe",
    "tasklist.exe",
    "taskkill.exe",
    "findstr.exe",
    "find.exe",

    // Wheelbase telemetry
    "conspitlink2.0.exe",

    // Ollama (local LLM)
    "ollama.exe",
    "ollama_llama_server.exe",

    // Pod watchdog — hybrid connectivity agent (fallback :9091 + CLOSE_WAIT auto-restart)
    "pod_watchdog.exe",
    "pod_watchdog",

    // Node.js services
    "node.exe",

    // Edge WebView2 (for lock screen UI)
    "msedge.exe",
    "msedgewebview2.exe",

    // Steam and game launchers
    "steam.exe",
    "steamservice.exe",
    "steamwebhelper.exe",
    "steamclient.exe",
    "gameoverlayui.exe",
    "gameoverlayrenderer.exe",

    // Sim racing games
    "acs.exe",                     // Assetto Corsa
    "acserver.exe",                // AC dedicated server
    // NOTE: "content manager.exe" is NOT here — only allowed in employee debug mode
    "assettocorsa2.exe",           // Assetto Corsa Evo
    "ac2-win64-shipping.exe",      // AC Evo (Unreal Engine shipping build)
    "iracing.exe",                 // iRacing
    "iracingservice.exe",
    "iracingsim64dx11.exe",
    "f1_25.exe",                   // F1 25
    "lemansultimate.exe",          // LMU
    "forzahorizon5.exe",           // Forza
    "forzamotorsport.exe",

    // Audio
    "realtek audio console.exe",

    // Networking
    "networkmanager.exe",
];

/// Kiosk mode manager.
pub struct KioskManager {
    active: Arc<AtomicBool>,
    debug_mode: bool,
    allowed_extra: HashSet<String>,
}

impl KioskManager {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            debug_mode: false,
            allowed_extra: HashSet::new(),
        }
    }

    /// Enable kiosk mode — hides taskbar, starts process watchdog.
    pub fn activate(&self) {
        if self.active.load(Ordering::SeqCst) {
            return;
        }
        self.active.store(true, Ordering::SeqCst);
        tracing::info!("Kiosk mode ACTIVATED — blocking unauthorized access");

        #[cfg(windows)]
        {
            hide_taskbar(true);
            install_keyboard_hook();
        }
    }

    /// Disable kiosk mode — restores taskbar and normal access.
    pub fn deactivate(&self) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }
        self.active.store(false, Ordering::SeqCst);
        tracing::info!("Kiosk mode DEACTIVATED — restoring normal access");

        #[cfg(windows)]
        {
            hide_taskbar(false);
            remove_keyboard_hook();
        }
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Add a process name to the allow list (e.g., the current game).
    pub fn allow_process(&mut self, name: &str) {
        self.allowed_extra.insert(name.to_lowercase());
    }

    /// Remove a process from the extra allow list.
    pub fn disallow_process(&mut self, name: &str) {
        self.allowed_extra.remove(&name.to_lowercase());
    }

    /// Enter employee debug mode — allows Content Manager and deactivates kiosk restrictions.
    pub fn enter_debug_mode(&mut self) {
        self.debug_mode = true;
        self.deactivate();
        tracing::info!("Kiosk: EMPLOYEE DEBUG MODE — Content Manager and all apps allowed");
    }

    /// Exit employee debug mode — re-engages kiosk restrictions.
    pub fn exit_debug_mode(&mut self) {
        self.debug_mode = false;
        tracing::info!("Kiosk: exiting debug mode");
    }

    /// Check if in debug mode
    pub fn is_debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Scan running processes and kill any not on the allow list.
    /// Call this periodically (e.g., every 5 seconds).
    pub fn enforce_process_whitelist(&self) {
        if !self.active.load(Ordering::SeqCst) {
            return;
        }

        let allowed_set: HashSet<String> = ALLOWED_PROCESSES
            .iter()
            .map(|s| s.to_lowercase())
            .chain(self.allowed_extra.iter().cloned())
            .collect();

        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for (pid, process) in sys.processes() {
            let name = process.name().to_string_lossy().to_lowercase();
            if name.is_empty() {
                continue;
            }
            // Skip if allowed
            if allowed_set.contains(&name) {
                continue;
            }
            // Skip system PIDs (0, 4)
            if pid.as_u32() <= 4 {
                continue;
            }
            // Kill unauthorized process
            if process.kill() {
                tracing::warn!("Kiosk: killed unauthorized process '{}' (PID {})", name, pid);
            }
        }
    }
}

// ─── Windows-specific implementations ──────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    use std::sync::atomic::{AtomicPtr, Ordering};
    use std::ptr;
    use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
    use winapi::shared::windef::HWND;
    use winapi::um::winuser;

    static HOOK_HANDLE: AtomicPtr<winapi::shared::windef::HHOOK__> =
        AtomicPtr::new(ptr::null_mut());

    /// Low-level keyboard hook callback.
    /// Blocks: Win key, Alt+Tab, Alt+F4, Ctrl+Esc, Alt+Esc.
    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let kb = unsafe { &*(l_param as *const winuser::KBDLLHOOKSTRUCT) };
            let vk = kb.vkCode;
            let flags = kb.flags;
            let alt_down = flags & winuser::LLKHF_ALTDOWN != 0;

            // Block Win key (left and right)
            if vk == winuser::VK_LWIN as u32 || vk == winuser::VK_RWIN as u32 {
                return 1;
            }
            // Block Alt+Tab
            if alt_down && vk == winuser::VK_TAB as u32 {
                return 1;
            }
            // Block Alt+F4
            if alt_down && vk == winuser::VK_F4 as u32 {
                return 1;
            }
            // Block Alt+Esc
            if alt_down && vk == winuser::VK_ESCAPE as u32 {
                return 1;
            }
            // Block Ctrl+Esc (start menu)
            if vk == winuser::VK_ESCAPE as u32 {
                let ctrl_down =
                    unsafe { winuser::GetAsyncKeyState(winuser::VK_CONTROL) } < 0;
                if ctrl_down {
                    return 1;
                }
            }
        }
        unsafe { winuser::CallNextHookEx(ptr::null_mut(), code, w_param, l_param) }
    }

    pub fn install_keyboard_hook() {
        unsafe {
            let hook = winuser::SetWindowsHookExW(
                winuser::WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                ptr::null_mut(),
                0,
            );
            if !hook.is_null() {
                HOOK_HANDLE.store(hook, Ordering::SeqCst);
                tracing::info!("Kiosk: keyboard hook installed (Win/Alt+Tab/Alt+F4 blocked)");
            } else {
                tracing::error!("Kiosk: failed to install keyboard hook");
            }
        }
    }

    pub fn remove_keyboard_hook() {
        let hook = HOOK_HANDLE.swap(ptr::null_mut(), Ordering::SeqCst);
        if !hook.is_null() {
            unsafe {
                winuser::UnhookWindowsHookEx(hook);
            }
            tracing::info!("Kiosk: keyboard hook removed");
        }
    }

    pub fn hide_taskbar(hide: bool) {
        unsafe {
            let taskbar_class: Vec<u16> = "Shell_TrayWnd\0".encode_utf16().collect();
            let hwnd: HWND = winuser::FindWindowW(taskbar_class.as_ptr(), ptr::null());
            if !hwnd.is_null() {
                let cmd = if hide {
                    winuser::SW_HIDE
                } else {
                    winuser::SW_SHOW
                };
                winuser::ShowWindow(hwnd, cmd);
                tracing::info!(
                    "Kiosk: taskbar {}",
                    if hide { "hidden" } else { "restored" }
                );
            }
        }
    }
}

#[cfg(windows)]
pub use windows_impl::{hide_taskbar, install_keyboard_hook, remove_keyboard_hook};

// ─── Non-Windows stubs ─────────────────────────────────────────────────────

#[cfg(not(windows))]
pub fn hide_taskbar(_hide: bool) {}

#[cfg(not(windows))]
pub fn install_keyboard_hook() {}

#[cfg(not(windows))]
pub fn remove_keyboard_hook() {}
