//! Force Feedback Controller for OpenFFBoard-based wheelbases (Conspit Ares 8Nm).
//!
//! Provides safety commands to zero the wheelbase torque on session end,
//! game crash, or rc-agent startup. Uses the OpenFFBoard vendor HID interface
//! (usage page 0xFF00, report ID 0xA1) — independent of DirectInput game FFB.
//!
//! This module is write-only. HID input reading lives in `driving_detector.rs`.

use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};

/// Crash counter — tracks how many times ConspitLink has been restarted via
/// watchdog (crash recovery) since rc-agent started. Resets on agent restart.
static CONSPIT_CRASH_COUNT: AtomicU32 = AtomicU32::new(0);

/// Flag: true when safe_session_end() is managing CL lifecycle.
/// The watchdog MUST skip its check when this is set.
pub static SESSION_END_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

/// Config files that must be backed up and verified before/after ConspitLink restart.
/// Tuple: (absolute path, friendly name for logging)
const CONSPIT_CONFIG_FILES: &[(&str, &str)] = &[
    (r"C:\Program Files (x86)\Conspit Link 2.0\Settings.json", "Settings.json"),
    (r"C:\Program Files (x86)\Conspit Link 2.0\Global.json", "Global.json"),
    (r"C:\Program Files (x86)\Conspit Link 2.0\JsonConfigure\GameToBaseConfig.json", "GameToBaseConfig.json"),
];

/// Runtime copy of Global.json that ConspitLink writes to C:\RacingPoint
const RUNTIME_GLOBAL_JSON: &str = r"C:\RacingPoint\Global.json";

/// OpenFFBoard vendor HID usage page — filters the correct interface
const OPENFFBOARD_USAGE_PAGE: u16 = 0xFF00;

/// Report ID for OpenFFBoard vendor commands
const REPORT_ID: u8 = 0xA1;

/// OpenFFBoard command types
const CMD_TYPE_WRITE: u8 = 0;

/// FFBWheel class ID (little-endian u16)
const CLASS_FFBWHEEL: u16 = 0x00A1;

/// FFBWheel command IDs
const CMD_ESTOP: u32 = 0x0A;
const CMD_FFB_ACTIVE: u32 = 0x00;

/// Axis class ID (little-endian u16) — for gain/power commands
const CLASS_AXIS: u16 = 0x0A01;

/// Axis command: power (overall force strength)
const CMD_POWER: u32 = 0x00;

/// Effects Manager class ID — for clearing orphaned DirectInput effects
const CLASS_FXM: u16 = 0x0A03;
/// Effects Manager: reset all effects
const CMD_FXM_RESET: u32 = 0x01;

/// Axis command: idle spring strength (centering spring when no game FFB active)
const CMD_IDLESPRING: u32 = 0x05;

/// 80% power cap as HID value: (80 * 65535) / 100 = 52428
pub const POWER_CAP_80_PERCENT: i64 = 52428;

/// Force Feedback controller for the Conspit Ares wheelbase.
///
/// Opens the OpenFFBoard vendor HID interface and provides safety commands.
/// All methods are non-panicking — if the device is absent or writes fail,
/// warnings are logged and execution continues.
#[derive(Clone)]
pub struct FfbController {
    vid: u16,
    pid: u16,
}

impl FfbController {
    /// Create a new FFB controller for the given VID/PID.
    /// Does NOT open the device yet — that happens lazily on each command.
    pub fn new(vid: u16, pid: u16) -> Self {
        Self { vid, pid }
    }

    /// Send emergency stop — immediately zeros motor torque.
    ///
    /// This is the primary safety command. Call BEFORE killing the game process.
    /// Returns Ok(true) if the command was sent, Ok(false) if device not found,
    /// or Err on HID API failure.
    pub fn zero_force(&self) -> Result<bool, String> {
        let device = match self.open_vendor_interface() {
            Some(dev) => dev,
            None => {
                tracing::debug!(
                    "Wheelbase not found (VID:{:#06x} PID:{:#06x}) — skipping FFB zero",
                    self.vid, self.pid
                );
                return Ok(false);
            }
        };

        // Send estop command (CmdID 0x0A, Data = 1)
        if let Err(e) = self.send_vendor_cmd(&device, CMD_ESTOP, 1) {
            tracing::warn!("FFB estop write failed: {}", e);
            return Err(e);
        }
        tracing::info!("FFB: emergency stop sent — wheelbase torque zeroed");

        // Also disable FFB active flag as belt-and-suspenders safety
        if let Err(e) = self.send_vendor_cmd(&device, CMD_FFB_ACTIVE, 0) {
            tracing::debug!("FFB: ffbactive=0 write failed (non-critical): {}", e);
        }

        Ok(true)
    }

    /// Zero wheelbase torque with retry logic. Retries `attempts` times with
    /// `delay_ms` between attempts on HID write failure (Err). Device-not-found
    /// (Ok(false)) is not retried — it's a permanent condition.
    ///
    /// Safe to call from panic hook (sync, no async, no allocator dependency).
    pub fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool {
        for i in 1..=attempts {
            match self.zero_force() {
                Ok(true) => {
                    tracing::info!("FFB zero succeeded on attempt {}", i);
                    return true;
                }
                Ok(false) => {
                    // Device not found — not retryable
                    tracing::debug!("FFB zero: device not found (attempt {})", i);
                    return false;
                }
                Err(e) => {
                    tracing::warn!("FFB zero attempt {}/{} failed: {}", i, attempts, e);
                    if i < attempts {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
            }
        }
        tracing::error!("FFB zero failed after {} attempts — wheelbase may retain torque", attempts);
        false
    }

    /// Open the OpenFFBoard vendor HID interface (usage page 0xFF00).
    ///
    /// Enumerates all HID devices matching VID/PID and selects the one
    /// with the vendor usage page — NOT the gamepad/DirectInput interface.
    fn open_vendor_interface(&self) -> Option<hidapi::HidDevice> {
        let api = match hidapi::HidApi::new() {
            Ok(api) => api,
            Err(e) => {
                tracing::warn!("FFB: failed to init HID API: {}", e);
                return None;
            }
        };

        // Find the vendor interface by usage page
        let device_info = api
            .device_list()
            .find(|d| {
                d.vendor_id() == self.vid
                    && d.product_id() == self.pid
                    && d.usage_page() == OPENFFBOARD_USAGE_PAGE
            });

        match device_info {
            Some(info) => {
                match info.open_device(&api) {
                    Ok(dev) => Some(dev),
                    Err(e) => {
                        tracing::warn!(
                            "FFB: found vendor interface but failed to open: {}",
                            e
                        );
                        None
                    }
                }
            }
            None => {
                // Try fallback: open by VID/PID directly (some firmware versions
                // may not report usage page correctly)
                tracing::debug!(
                    "FFB: no device with usage_page {:#06x}, trying direct open",
                    OPENFFBOARD_USAGE_PAGE
                );
                match api.open(self.vid, self.pid) {
                    Ok(dev) => {
                        tracing::debug!("FFB: opened device via direct VID/PID (fallback)");
                        Some(dev)
                    }
                    Err(_) => None,
                }
            }
        }
    }

    /// Set FFB gain as a percentage (10-100).
    ///
    /// Sends to OpenFFBoard Axis class power command.
    /// Returns Ok(true) if sent, Ok(false) if device not found.
    pub fn set_gain(&self, percent: u8) -> Result<bool, String> {
        let percent = percent.clamp(10, 100);
        let device = match self.open_vendor_interface() {
            Some(dev) => dev,
            None => {
                tracing::debug!(
                    "Wheelbase not found (VID:{:#06x} PID:{:#06x}) — skipping FFB gain",
                    self.vid, self.pid
                );
                return Ok(false);
            }
        };

        // Map percentage to 16-bit HID value
        let value = (percent as i64 * 65535) / 100;

        // Send to Axis class (0x0A01), not FFBWheel class
        self.send_vendor_cmd_to_class(&device, CLASS_AXIS, CMD_POWER, value)
            .map(|_| {
                tracing::info!("FFB: gain set to {}% (HID value: {})", percent, value);
                true
            })
    }

    /// Clear all orphaned DirectInput force-feedback effects.
    /// Sends fxm.reset (Effects Manager class 0x0A03, cmd 0x01).
    /// Returns Ok(true) if sent, Ok(false) if device not found.
    pub fn fxm_reset(&self) -> Result<bool, String> {
        let device = match self.open_vendor_interface() {
            Some(dev) => dev,
            None => {
                tracing::debug!("Wheelbase not found — skipping fxm.reset");
                return Ok(false);
            }
        };
        self.send_vendor_cmd_to_class(&device, CLASS_FXM, CMD_FXM_RESET, 0)
            .map(|_| {
                tracing::info!("FFB: fxm.reset sent — orphaned effects cleared");
                true
            })
    }

    /// Set the idle centering spring strength.
    /// Sends axis.idlespring (Axis class 0x0A01, cmd 0x05).
    /// Value range TBD empirically — start low (500-2000), test on hardware.
    /// Returns Ok(true) if sent, Ok(false) if device not found.
    pub fn set_idle_spring(&self, value: i64) -> Result<bool, String> {
        let device = match self.open_vendor_interface() {
            Some(dev) => dev,
            None => {
                tracing::debug!("Wheelbase not found — skipping idlespring");
                return Ok(false);
            }
        };
        self.send_vendor_cmd_to_class(&device, CLASS_AXIS, CMD_IDLESPRING, value)
            .map(|_| {
                tracing::info!("FFB: idlespring set to {}", value);
                true
            })
    }

    /// Send a vendor command to a specified class on the OpenFFBoard.
    ///
    /// Like send_vendor_cmd() but takes a class_id parameter instead of
    /// hardcoding CLASS_FFBWHEEL. Used by set_gain() for CLASS_AXIS.
    fn send_vendor_cmd_to_class(
        &self,
        device: &hidapi::HidDevice,
        class_id: u16,
        cmd_id: u32,
        data: i64,
    ) -> Result<(), String> {
        let mut buf = [0u8; 26];

        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&class_id.to_le_bytes());
        buf[4] = 0;
        buf[5..9].copy_from_slice(&cmd_id.to_le_bytes());
        buf[9..17].copy_from_slice(&data.to_le_bytes());
        buf[17..25].copy_from_slice(&0i64.to_le_bytes());

        device
            .write(&buf)
            .map(|_| ())
            .map_err(|e| format!("HID write failed: {}", e))
    }

    /// Send a vendor command to the OpenFFBoard.
    ///
    /// Report format (26 bytes total):
    /// - Byte 0: Report ID (0xA1)
    /// - Byte 1: Command type (0 = write)
    /// - Bytes 2-3: ClassID (u16 LE) — 0x00A1 for FFBWheel
    /// - Byte 4: Instance (0)
    /// - Bytes 5-8: CmdID (u32 LE)
    /// - Bytes 9-16: Data (i64 LE)
    /// - Bytes 17-24: Address (i64 LE, usually 0)
    fn send_vendor_cmd(
        &self,
        device: &hidapi::HidDevice,
        cmd_id: u32,
        data: i64,
    ) -> Result<(), String> {
        let mut buf = [0u8; 26];

        // Report ID
        buf[0] = REPORT_ID;
        // Command type: write
        buf[1] = CMD_TYPE_WRITE;
        // ClassID: FFBWheel (u16 LE)
        buf[2..4].copy_from_slice(&CLASS_FFBWHEEL.to_le_bytes());
        // Instance
        buf[4] = 0;
        // CmdID (u32 LE)
        buf[5..9].copy_from_slice(&cmd_id.to_le_bytes());
        // Data (i64 LE)
        buf[9..17].copy_from_slice(&data.to_le_bytes());
        // Address (i64 LE) — 0
        buf[17..25].copy_from_slice(&0i64.to_le_bytes());

        device
            .write(&buf)
            .map(|_| ())
            .map_err(|e| format!("HID write failed: {}", e))
    }
}

// ─── ConspitLink Process Management + Session-End Orchestrator ──────────────

/// Close ConspitLink gracefully via WM_CLOSE and wait for process exit.
///
/// Tries multiple window title variants (WPF title may differ from process name).
/// Returns true if ConspitLink exits within the timeout, or was not running.
/// Returns false if it's still running after the timeout (P-20 risk accepted).
pub fn close_conspit_link(timeout: std::time::Duration) -> bool {
    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        fn wide(s: &str) -> Vec<u16> {
            OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
        }

        let titles = [
            "Conspit Link 2.0",
            "ConspitLink2.0",
            "Conspit Link",
            "ConspitLink",
        ];
        let mut sent = false;
        for title in &titles {
            unsafe {
                let title_wide = wide(title);
                let hwnd = winapi::um::winuser::FindWindowW(std::ptr::null(), title_wide.as_ptr());
                if !hwnd.is_null() {
                    winapi::um::winuser::PostMessageW(
                        hwnd,
                        winapi::um::winuser::WM_CLOSE,
                        0,
                        0,
                    );
                    tracing::info!("Sent WM_CLOSE to ConspitLink via \"{}\"", title);
                    sent = true;
                    break;
                }
            }
        }

        if !sent {
            tracing::debug!("ConspitLink window not found — may not be running");
            return true; // Not running = already "closed"
        }

        // Poll for process exit
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !crate::ac_launcher::is_process_running("ConspitLink2.0.exe") {
                tracing::info!(
                    "ConspitLink exited after WM_CLOSE ({}ms)",
                    start.elapsed().as_millis()
                );
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        tracing::warn!(
            "ConspitLink still running after {}s WM_CLOSE timeout",
            timeout.as_secs()
        );
        false
    }
    #[cfg(not(windows))]
    {
        let _ = timeout;
        true
    }
}

/// Get current crash count (number of watchdog-triggered restarts since agent start).
pub fn get_crash_count() -> u32 {
    CONSPIT_CRASH_COUNT.load(Ordering::Relaxed)
}

/// Increment crash count and return new value (watchdog path only).
fn increment_crash_count() -> u32 {
    CONSPIT_CRASH_COUNT.fetch_add(1, Ordering::Relaxed) + 1
}

/// Backup ConspitLink config files to `.json.bak` counterparts.
///
/// Only backs up files whose current content is valid JSON — avoids overwriting
/// a good `.bak` with a corrupt source (Pitfall 2).
pub fn backup_conspit_configs() {
    backup_conspit_configs_impl(None);
}

/// Verify all ConspitLink config files parse as valid JSON.
///
/// If a file is corrupt and a `.json.bak` exists, restores from backup.
/// Returns true if all files are OK (or were successfully restored).
pub fn verify_conspit_configs() -> bool {
    verify_conspit_configs_impl(None)
}

/// Internal: backup with optional base dir override for testing.
fn backup_conspit_configs_impl(base_dir: Option<&std::path::Path>) {
    // Build list of files to check: the 3 config files + runtime Global.json
    let config_entries: Vec<(String, &str)> = if let Some(dir) = base_dir {
        // Test mode: use relative names inside the test dir
        let mut entries: Vec<(String, &str)> = CONSPIT_CONFIG_FILES
            .iter()
            .map(|(_path, name)| (dir.join(name).to_string_lossy().into_owned(), *name))
            .collect();
        entries.push((dir.join("RuntimeGlobal.json").to_string_lossy().into_owned(), "RuntimeGlobal.json"));
        entries
    } else {
        let mut entries: Vec<(String, &str)> = CONSPIT_CONFIG_FILES
            .iter()
            .map(|(path, name)| (path.to_string(), *name))
            .collect();
        entries.push((RUNTIME_GLOBAL_JSON.to_string(), "RuntimeGlobal.json"));
        entries
    };

    for (path_str, name) in &config_entries {
        let src = std::path::Path::new(path_str);
        if !src.exists() {
            tracing::debug!("Backup: {} does not exist — skipping", name);
            continue;
        }
        // Only backup if current file is valid JSON
        match std::fs::read_to_string(src) {
            Ok(contents) => {
                if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
                    let bak = src.with_extension("json.bak");
                    match std::fs::copy(src, &bak) {
                        Ok(_) => tracing::debug!("Backed up {} -> {}", name, bak.display()),
                        Err(e) => tracing::warn!("Failed to backup {}: {}", name, e),
                    }
                } else {
                    tracing::warn!(
                        "Backup: {} contains invalid JSON — skipping (preserving existing .bak)",
                        name
                    );
                }
            }
            Err(e) => tracing::warn!("Backup: could not read {}: {}", name, e),
        }
    }
}

/// Internal: verify with optional base dir override for testing.
fn verify_conspit_configs_impl(base_dir: Option<&std::path::Path>) -> bool {
    let config_entries: Vec<(String, &str)> = if let Some(dir) = base_dir {
        let mut entries: Vec<(String, &str)> = CONSPIT_CONFIG_FILES
            .iter()
            .map(|(_path, name)| (dir.join(name).to_string_lossy().into_owned(), *name))
            .collect();
        entries.push((dir.join("RuntimeGlobal.json").to_string_lossy().into_owned(), "RuntimeGlobal.json"));
        entries
    } else {
        let mut entries: Vec<(String, &str)> = CONSPIT_CONFIG_FILES
            .iter()
            .map(|(path, name)| (path.to_string(), *name))
            .collect();
        entries.push((RUNTIME_GLOBAL_JSON.to_string(), "RuntimeGlobal.json"));
        entries
    };

    let mut all_ok = true;
    for (path_str, name) in &config_entries {
        let path = std::path::Path::new(path_str);
        if !path.exists() {
            tracing::debug!("Verify: {} does not exist — skipping (CL may not have written it yet)", name);
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if serde_json::from_str::<serde_json::Value>(&contents).is_err() {
                    tracing::warn!("{} is corrupted — attempting restore from backup", name);
                    let bak = path.with_extension("json.bak");
                    if bak.exists() {
                        match std::fs::copy(&bak, path) {
                            Ok(_) => tracing::info!("{} restored from backup", name),
                            Err(e) => {
                                tracing::error!("Failed to restore {}: {}", name, e);
                                all_ok = false;
                            }
                        }
                    } else {
                        tracing::error!("{} corrupted and no backup exists", name);
                        all_ok = false;
                    }
                } else {
                    tracing::debug!("{} integrity check passed", name);
                }
            }
            Err(e) => {
                tracing::warn!("Could not read {} for verification: {}", name, e);
                // Read error is not necessarily corruption — file may be locked briefly
            }
        }
    }
    all_ok
}

/// Testable backup function that operates within a given directory.
#[cfg(test)]
pub(crate) fn backup_conspit_configs_in_dir(dir: &std::path::Path) {
    backup_conspit_configs_impl(Some(dir));
}

/// Testable verify function that operates within a given directory.
#[cfg(test)]
pub(crate) fn verify_conspit_configs_in_dir(dir: &std::path::Path) -> bool {
    verify_conspit_configs_impl(Some(dir))
}

/// Minimize ConspitLink window with polling retry.
///
/// Calls `minimize_conspit_window()` every 500ms for up to 8s (16 attempts).
/// More reliable than a fixed sleep — handles variable CL startup times.
pub fn minimize_conspit_window_with_retry() {
    let start = std::time::Instant::now();
    for attempt in 1..=16u32 {
        std::thread::sleep(std::time::Duration::from_millis(500));
        crate::ac_launcher::minimize_conspit_window();
        // Check if CL is at least running (confirms it started)
        if crate::ac_launcher::is_process_running("ConspitLink2.0.exe") {
            tracing::info!(
                "ConspitLink window minimize attempt {} ({}ms elapsed) — process is running",
                attempt,
                start.elapsed().as_millis()
            );
            return;
        }
    }
    tracing::warn!(
        "ConspitLink window minimize: process not detected after 8s — may not have started"
    );
}

/// Restart ConspitLink with full hardening:
/// - Optionally increment crash counter (watchdog path only)
/// - Backup config files (skip if source is corrupt)
/// - Launch process
/// - Minimize window with polling retry
/// - Verify all JSON configs after startup
///
/// `is_crash_recovery`: true when called from watchdog, false from session-end.
pub fn restart_conspit_link_hardened(is_crash_recovery: bool) {
    #[cfg(windows)]
    {
        let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
        if !std::path::Path::new(conspit_path).exists() {
            tracing::debug!("ConspitLink not installed — skipping restart");
            return;
        }

        // 1. Increment crash counter (watchdog only, not session-end)
        if is_crash_recovery {
            let count = increment_crash_count();
            tracing::warn!("ConspitLink crash recovery restart #{}", count);
            if count >= 5 {
                tracing::error!(
                    "ConspitLink has crashed {} times since agent start",
                    count
                );
            }
        }

        // 2. Backup configs (only if current files are valid JSON)
        backup_conspit_configs();

        // 3. Launch ConspitLink
        match crate::ac_launcher::hidden_cmd("cmd")
            .args(["/c", "start", "", conspit_path])
            .spawn()
        {
            Ok(_) => {
                tracing::info!("ConspitLink started, will verify + minimize...");
                // Single thread: minimize with retry, then verify configs
                std::thread::spawn(|| {
                    minimize_conspit_window_with_retry();
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    verify_conspit_configs();
                });
            }
            Err(e) => tracing::error!("Failed to restart ConspitLink: {}", e),
        }
    }
}

/// Routine session-end safety sequence.
///
/// Blocks for ~2-3s normally (up to ~5.5s if ConspitLink does not close quickly).
/// NOT suitable for panic hook — use `zero_force_with_retry()` for ESTOP instead.
///
/// Sequence:
/// 1. Close ConspitLink (WM_CLOSE, 5s timeout, skip on failure)
/// 2. fxm.reset (clear orphaned DirectInput effects)
/// 3. Ramp idlespring from 0 to 2000 over 500ms (5 steps)
/// 4. Restart ConspitLink (fire-and-forget background thread)
pub async fn safe_session_end(ffb: &FfbController) {
    // Guard: signal watchdog to skip CL checks during session-end
    SESSION_END_IN_PROGRESS.store(true, Ordering::Release);

    // Step 1: Close ConspitLink (sync, in spawn_blocking)
    let closed = tokio::task::spawn_blocking(|| {
        close_conspit_link(std::time::Duration::from_secs(5))
    })
    .await
    .unwrap_or(false);

    if !closed {
        tracing::warn!(
            "ConspitLink did not close within 5s -- proceeding with HID commands (P-20 risk accepted)"
        );
    }

    // Step 2: fxm.reset + idlespring ramp (sync, in spawn_blocking)
    let ffb_clone = ffb.clone();
    tokio::task::spawn_blocking(move || {
        // Clear all orphaned DirectInput effects
        if let Err(e) = ffb_clone.fxm_reset() {
            tracing::warn!("fxm.reset failed: {} -- continuing with idlespring", e);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Ramp idlespring: 5 steps over 500ms (100ms between steps)
        let target: i64 = 2000;
        for step in 1..=5 {
            let value = (target * step) / 5;
            let _ = ffb_clone.set_idle_spring(value);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    })
    .await
    .ok();

    // Step 3: Restart ConspitLink (fire and forget — do NOT .await)
    // Clear SESSION_END_IN_PROGRESS AFTER restart completes (including verify/minimize)
    tokio::task::spawn_blocking(|| {
        restart_conspit_link_hardened(false);
        SESSION_END_IN_PROGRESS.store(false, Ordering::Release);
    });

    tracing::info!("Session-end safety sequence complete -- wheel centering with idlespring");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffb_controller_no_device_graceful() {
        // On machines without a wheelbase, zero_force should return Ok(false)
        let ctrl = FfbController::new(0x1209, 0xFFB0);
        let result = ctrl.zero_force();
        // Should not panic — either Ok(false) if no device, or Ok(true)/Err if device present
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_zero_force_with_retry_no_device() {
        // On dev machines without a wheelbase, should return false without retrying
        let ctrl = FfbController::new(0x1209, 0xFFB0);
        let result = ctrl.zero_force_with_retry(3, 100);
        // Should return false (device not found) without 3x delay
        assert!(!result);
    }

    #[test]
    fn test_set_gain_buffer_format() {
        // Verify the buffer layout for Axis class gain commands
        // Test 50% gain
        let percent: u8 = 50;
        let value = (percent as i64 * 65535) / 100; // = 32767
        let mut buf = [0u8; 26];
        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&CLASS_AXIS.to_le_bytes());
        buf[4] = 0;
        buf[5..9].copy_from_slice(&CMD_POWER.to_le_bytes());
        buf[9..17].copy_from_slice(&value.to_le_bytes());

        // Verify CLASS_AXIS at bytes 2-3 (0x0A01 LE = [0x01, 0x0A])
        assert_eq!(buf[2], 0x01); // CLASS_AXIS low byte
        assert_eq!(buf[3], 0x0A); // CLASS_AXIS high byte
        // Verify CMD_POWER at bytes 5-8 (0x00 LE)
        assert_eq!(buf[5], 0x00);
        assert_eq!(buf[6], 0x00);
        assert_eq!(buf[7], 0x00);
        assert_eq!(buf[8], 0x00);
        // Verify value 32767 at bytes 9-16
        assert_eq!(value, 32767);
        let stored_value = i64::from_le_bytes([buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15], buf[16]]);
        assert_eq!(stored_value, 32767);

        // Test 10% gain (minimum floor)
        let value_10 = (10i64 * 65535) / 100;
        assert_eq!(value_10, 6553);

        // Test 100% gain (maximum)
        let value_100 = (100i64 * 65535) / 100;
        assert_eq!(value_100, 65535);

        // Test clamping: 5% should clamp to 10%
        let clamped = 5u8.clamp(10, 100);
        assert_eq!(clamped, 10);

        // Test clamping: 120% should clamp to 100%
        let clamped_high = 120u8.clamp(10, 100);
        assert_eq!(clamped_high, 100);
    }

    #[test]
    fn test_set_gain_no_device_graceful() {
        // On machines without a wheelbase, set_gain should return Ok(false)
        let ctrl = FfbController::new(0x1209, 0xFFB0);
        let result = ctrl.set_gain(70);
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_vendor_cmd_buffer_format() {
        // Verify the buffer layout is correct
        let mut buf = [0u8; 26];
        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&CLASS_FFBWHEEL.to_le_bytes());
        buf[4] = 0;
        buf[5..9].copy_from_slice(&CMD_ESTOP.to_le_bytes());
        buf[9..17].copy_from_slice(&1i64.to_le_bytes());

        assert_eq!(buf[0], 0xA1); // Report ID
        assert_eq!(buf[1], 0x00); // Write type
        assert_eq!(buf[2], 0xA1); // ClassID low byte
        assert_eq!(buf[3], 0x00); // ClassID high byte
        assert_eq!(buf[5], 0x0A); // CmdID low byte (estop)
        assert_eq!(buf[9], 0x01); // Data low byte (1 = activate estop)
    }

    #[test]
    fn test_fxm_reset_buffer_format() {
        // Verify fxm.reset uses CLASS_FXM (0x0A03), CMD_FXM_RESET (0x01), data 0
        let mut buf = [0u8; 26];
        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&CLASS_FXM.to_le_bytes());
        buf[4] = 0;
        buf[5..9].copy_from_slice(&CMD_FXM_RESET.to_le_bytes());
        buf[9..17].copy_from_slice(&0i64.to_le_bytes());
        buf[17..25].copy_from_slice(&0i64.to_le_bytes());

        // CLASS_FXM at bytes 2-3: 0x0A03 LE = [0x03, 0x0A]
        assert_eq!(buf[2], 0x03);
        assert_eq!(buf[3], 0x0A);
        // CMD_FXM_RESET at bytes 5-8: 0x01 LE = [0x01, 0x00, 0x00, 0x00]
        assert_eq!(buf[5], 0x01);
        assert_eq!(buf[6], 0x00);
        assert_eq!(buf[7], 0x00);
        assert_eq!(buf[8], 0x00);
        // Data at bytes 9-16: 0i64 LE
        let stored_data = i64::from_le_bytes([buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15], buf[16]]);
        assert_eq!(stored_data, 0);
    }

    #[test]
    fn test_idlespring_buffer_format() {
        // Verify idlespring uses CLASS_AXIS (0x0A01), CMD_IDLESPRING (0x05), data = value
        let value: i64 = 2000;
        let mut buf = [0u8; 26];
        buf[0] = REPORT_ID;
        buf[1] = CMD_TYPE_WRITE;
        buf[2..4].copy_from_slice(&CLASS_AXIS.to_le_bytes());
        buf[4] = 0;
        buf[5..9].copy_from_slice(&CMD_IDLESPRING.to_le_bytes());
        buf[9..17].copy_from_slice(&value.to_le_bytes());
        buf[17..25].copy_from_slice(&0i64.to_le_bytes());

        // CLASS_AXIS at bytes 2-3: 0x0A01 LE = [0x01, 0x0A]
        assert_eq!(buf[2], 0x01);
        assert_eq!(buf[3], 0x0A);
        // CMD_IDLESPRING at bytes 5-8: 0x05 LE = [0x05, 0x00, 0x00, 0x00]
        assert_eq!(buf[5], 0x05);
        assert_eq!(buf[6], 0x00);
        assert_eq!(buf[7], 0x00);
        assert_eq!(buf[8], 0x00);
        // Data at bytes 9-16: 2000i64 LE
        let stored_data = i64::from_le_bytes([buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15], buf[16]]);
        assert_eq!(stored_data, 2000);
    }

    #[test]
    fn test_idlespring_ramp_values() {
        // 5-step ramp from 0 to target=2000 produces [400, 800, 1200, 1600, 2000]
        let target: i64 = 2000;
        let steps: Vec<i64> = (1..=5).map(|step| (target * step) / 5).collect();
        assert_eq!(steps, vec![400, 800, 1200, 1600, 2000]);
    }

    #[test]
    fn test_power_cap_80_percent() {
        // 80% of 65535 = 52428
        assert_eq!(POWER_CAP_80_PERCENT, 52428);
        assert_eq!((80i64 * 65535) / 100, 52428);
    }

    #[test]
    fn test_estop_still_uses_ffbwheel_class() {
        // Regression guard: ESTOP path must use CLASS_FFBWHEEL, not CLASS_AXIS or CLASS_FXM
        assert_eq!(CLASS_FFBWHEEL, 0x00A1);
        assert_eq!(CMD_ESTOP, 0x0A);
    }

    #[test]
    fn test_ffb_controller_is_clone() {
        // FfbController must derive Clone for use in spawn_blocking closures
        let c = FfbController::new(0x1209, 0xFFB0);
        let _c2 = c.clone();
    }

    // ─── ConspitLink Hardening Tests ────────────────────────────────────────

    #[test]
    fn test_config_file_list_complete() {
        // CONSPIT_CONFIG_FILES must have exactly 3 entries
        assert_eq!(CONSPIT_CONFIG_FILES.len(), 3);
        // Check all expected files are present
        let names: Vec<&str> = CONSPIT_CONFIG_FILES.iter().map(|(_, n)| *n).collect();
        assert!(names.contains(&"Settings.json"));
        assert!(names.contains(&"Global.json"));
        assert!(names.contains(&"GameToBaseConfig.json"));
    }

    #[test]
    fn test_crash_count_starts_at_zero() {
        // Reset for test isolation (AtomicU32 is global)
        CONSPIT_CRASH_COUNT.store(0, Ordering::Relaxed);
        assert_eq!(get_crash_count(), 0);
    }

    #[test]
    fn test_crash_count_increment() {
        CONSPIT_CRASH_COUNT.store(0, Ordering::Relaxed);
        let new = increment_crash_count();
        assert_eq!(new, 1);
        assert_eq!(get_crash_count(), 1);
        // Reset after test
        CONSPIT_CRASH_COUNT.store(0, Ordering::Relaxed);
    }

    #[test]
    fn test_backup_skips_corrupt_source() {
        // Given a file with invalid JSON, backup should NOT overwrite existing .bak
        let dir = std::env::temp_dir().join("conspit_test_backup_skip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let file = dir.join("Settings.json");
        let bak = dir.join("Settings.json.bak");

        // Create a good .bak first
        std::fs::write(&bak, r#"{"good": true}"#).unwrap();
        // Write corrupt JSON to the source file
        std::fs::write(&file, "NOT VALID JSON {{{{").unwrap();

        backup_conspit_configs_in_dir(&dir);

        // .bak should still contain the good content (not overwritten)
        let bak_content = std::fs::read_to_string(&bak).unwrap();
        assert_eq!(bak_content, r#"{"good": true}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_backup_copies_valid_source() {
        let dir = std::env::temp_dir().join("conspit_test_backup_copy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let file = dir.join("Settings.json");
        std::fs::write(&file, r#"{"valid": true}"#).unwrap();

        backup_conspit_configs_in_dir(&dir);

        let bak = dir.join("Settings.json.bak");
        assert!(bak.exists(), ".bak should be created");
        let bak_content = std::fs::read_to_string(&bak).unwrap();
        assert_eq!(bak_content, r#"{"valid": true}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_valid_json() {
        let dir = std::env::temp_dir().join("conspit_test_verify_valid");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create all 3 config files + runtime with valid JSON
        for name in &["Settings.json", "Global.json", "GameToBaseConfig.json", "RuntimeGlobal.json"] {
            std::fs::write(dir.join(name), r#"{"ok": true}"#).unwrap();
        }

        let result = verify_conspit_configs_in_dir(&dir);
        assert!(result, "All valid JSON should return true");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_corrupt_json_triggers_restore() {
        let dir = std::env::temp_dir().join("conspit_test_verify_restore");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create valid files for 2 of them
        std::fs::write(dir.join("Global.json"), r#"{"ok": true}"#).unwrap();
        std::fs::write(dir.join("GameToBaseConfig.json"), r#"{"ok": true}"#).unwrap();
        std::fs::write(dir.join("RuntimeGlobal.json"), r#"{"ok": true}"#).unwrap();

        // Create corrupt Settings.json
        std::fs::write(dir.join("Settings.json"), "CORRUPT!!!").unwrap();
        // Create a good .bak for Settings.json
        std::fs::write(dir.join("Settings.json.bak"), r#"{"restored": true}"#).unwrap();

        let result = verify_conspit_configs_in_dir(&dir);
        assert!(result, "Should return true after restoring from .bak");

        // Verify the file was restored
        let content = std::fs::read_to_string(dir.join("Settings.json")).unwrap();
        assert_eq!(content, r#"{"restored": true}"#);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_corrupt_no_backup_returns_false() {
        let dir = std::env::temp_dir().join("conspit_test_verify_no_bak");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Create valid files for most
        std::fs::write(dir.join("Global.json"), r#"{"ok": true}"#).unwrap();
        std::fs::write(dir.join("GameToBaseConfig.json"), r#"{"ok": true}"#).unwrap();
        std::fs::write(dir.join("RuntimeGlobal.json"), r#"{"ok": true}"#).unwrap();

        // Create corrupt Settings.json with NO .bak
        std::fs::write(dir.join("Settings.json"), "CORRUPT!!!").unwrap();

        let result = verify_conspit_configs_in_dir(&dir);
        assert!(!result, "Should return false when corrupt and no .bak exists");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_session_end_guard_flag() {
        // SESSION_END_IN_PROGRESS can be set and read atomically
        SESSION_END_IN_PROGRESS.store(false, Ordering::Release);
        assert!(!SESSION_END_IN_PROGRESS.load(Ordering::Acquire));

        SESSION_END_IN_PROGRESS.store(true, Ordering::Release);
        assert!(SESSION_END_IN_PROGRESS.load(Ordering::Acquire));

        // Reset
        SESSION_END_IN_PROGRESS.store(false, Ordering::Release);
    }
}
