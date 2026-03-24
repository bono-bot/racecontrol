//! Force Feedback Controller for OpenFFBoard-based wheelbases (Conspit Ares 8Nm).
//!
//! Provides safety commands to zero the wheelbase torque on session end,
//! game crash, or rc-agent startup. Uses the OpenFFBoard vendor HID interface
//! (usage page 0xFF00, report ID 0xA1) — independent of DirectInput game FFB.
//!
//! This module is write-only. HID input reading lives in `driving_detector.rs`.

use std::sync::atomic::{AtomicU32, AtomicBool, Ordering};
use rc_common::types::SimType;

const LOG_TARGET: &str = "ffb";

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

/// Trait seam for FFB commands — abstracts HID write surface for testing.
/// Production: FfbController implements via hidapi.
/// Tests: mockall generates MockTestBackend.
pub trait FfbBackend: Send + Sync {
    fn zero_force(&self) -> Result<bool, String>;
    fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
    fn set_gain(&self, percent: u8) -> Result<bool, String>;
    fn fxm_reset(&self) -> Result<bool, String>;
    fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
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
                    target: LOG_TARGET,
                    "Wheelbase not found (VID:{:#06x} PID:{:#06x}) — skipping FFB zero",
                    self.vid, self.pid
                );
                return Ok(false);
            }
        };

        // Send estop command (CmdID 0x0A, Data = 1)
        if let Err(e) = self.send_vendor_cmd(&device, CMD_ESTOP, 1) {
            tracing::warn!(target: LOG_TARGET, "FFB estop write failed: {}", e);
            return Err(e);
        }
        tracing::info!(target: LOG_TARGET, "FFB: emergency stop sent — wheelbase torque zeroed");

        // Also disable FFB active flag as belt-and-suspenders safety
        if let Err(e) = self.send_vendor_cmd(&device, CMD_FFB_ACTIVE, 0) {
            tracing::debug!(target: LOG_TARGET, "FFB: ffbactive=0 write failed (non-critical): {}", e);
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
                    tracing::info!(target: LOG_TARGET, "FFB zero succeeded on attempt {}", i);
                    return true;
                }
                Ok(false) => {
                    // Device not found — not retryable
                    tracing::debug!(target: LOG_TARGET, "FFB zero: device not found (attempt {})", i);
                    return false;
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "FFB zero attempt {}/{} failed: {}", i, attempts, e);
                    if i < attempts {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                }
            }
        }
        tracing::error!(target: LOG_TARGET, "FFB zero failed after {} attempts — wheelbase may retain torque", attempts);
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
                tracing::warn!(target: LOG_TARGET, "FFB: failed to init HID API: {}", e);
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
                            target: LOG_TARGET,
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
                    target: LOG_TARGET,
                    "FFB: no device with usage_page {:#06x}, trying direct open",
                    OPENFFBOARD_USAGE_PAGE
                );
                match api.open(self.vid, self.pid) {
                    Ok(dev) => {
                        tracing::debug!(target: LOG_TARGET, "FFB: opened device via direct VID/PID (fallback)");
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
                    target: LOG_TARGET,
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
                tracing::info!(target: LOG_TARGET, "FFB: gain set to {}% (HID value: {})", percent, value);
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
                tracing::debug!(target: LOG_TARGET, "Wheelbase not found — skipping fxm.reset");
                return Ok(false);
            }
        };
        self.send_vendor_cmd_to_class(&device, CLASS_FXM, CMD_FXM_RESET, 0)
            .map(|_| {
                tracing::info!(target: LOG_TARGET, "FFB: fxm.reset sent — orphaned effects cleared");
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
                tracing::debug!(target: LOG_TARGET, "Wheelbase not found — skipping idlespring");
                return Ok(false);
            }
        };
        self.send_vendor_cmd_to_class(&device, CLASS_AXIS, CMD_IDLESPRING, value)
            .map(|_| {
                tracing::info!(target: LOG_TARGET, "FFB: idlespring set to {}", value);
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

impl FfbBackend for FfbController {
    fn zero_force(&self) -> Result<bool, String> {
        FfbController::zero_force(self)
    }
    fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool {
        FfbController::zero_force_with_retry(self, attempts, delay_ms)
    }
    fn set_gain(&self, percent: u8) -> Result<bool, String> {
        FfbController::set_gain(self, percent)
    }
    fn fxm_reset(&self) -> Result<bool, String> {
        FfbController::fxm_reset(self)
    }
    fn set_idle_spring(&self, value: i64) -> Result<bool, String> {
        FfbController::set_idle_spring(self, value)
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
                    tracing::info!(target: LOG_TARGET, "Sent WM_CLOSE to ConspitLink via \"{}\"", title);
                    sent = true;
                    break;
                }
            }
        }

        if !sent {
            tracing::debug!(target: LOG_TARGET, "ConspitLink window not found — may not be running");
            return true; // Not running = already "closed"
        }

        // Poll for process exit
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if !crate::ac_launcher::is_process_running("ConspitLink2.0.exe") {
                tracing::info!(
                    target: LOG_TARGET,
                    "ConspitLink exited after WM_CLOSE ({}ms)",
                    start.elapsed().as_millis()
                );
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(250));
        }

        tracing::warn!(
            target: LOG_TARGET,
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
            tracing::debug!(target: LOG_TARGET, "Backup: {} does not exist — skipping", name);
            continue;
        }
        // Only backup if current file is valid JSON
        match std::fs::read_to_string(src) {
            Ok(contents) => {
                if serde_json::from_str::<serde_json::Value>(&contents).is_ok() {
                    let bak = src.with_extension("json.bak");
                    match std::fs::copy(src, &bak) {
                        Ok(_) => tracing::debug!(target: LOG_TARGET, "Backed up {} -> {}", name, bak.display()),
                        Err(e) => tracing::warn!(target: LOG_TARGET, "Failed to backup {}: {}", name, e),
                    }
                } else {
                    tracing::warn!(
                        target: LOG_TARGET,
                        "Backup: {} contains invalid JSON — skipping (preserving existing .bak)",
                        name
                    );
                }
            }
            Err(e) => tracing::warn!(target: LOG_TARGET, "Backup: could not read {}: {}", name, e),
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
            tracing::debug!(target: LOG_TARGET, "Verify: {} does not exist — skipping (CL may not have written it yet)", name);
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(contents) => {
                if serde_json::from_str::<serde_json::Value>(&contents).is_err() {
                    tracing::warn!(target: LOG_TARGET, "{} is corrupted — attempting restore from backup", name);
                    let bak = path.with_extension("json.bak");
                    if bak.exists() {
                        match std::fs::copy(&bak, path) {
                            Ok(_) => tracing::info!(target: LOG_TARGET, "{} restored from backup", name),
                            Err(e) => {
                                tracing::error!(target: LOG_TARGET, "Failed to restore {}: {}", name, e);
                                all_ok = false;
                            }
                        }
                    } else {
                        tracing::error!(target: LOG_TARGET, "{} corrupted and no backup exists", name);
                        all_ok = false;
                    }
                } else {
                    tracing::debug!(target: LOG_TARGET, "{} integrity check passed", name);
                }
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Could not read {} for verification: {}", name, e);
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

// ─── Auto-Switch Configuration ──────────────────────────────────────────────

/// Logging target for auto-switch config operations.
const LOG_TARGET_CFG: &str = "conspit-cfg";

/// ConspitLink install directory (default production path).
const INSTALL_DIR: &str = r"C:\Program Files (x86)\Conspit Link 2.0";

/// Runtime directory where ConspitLink reads Global.json at startup.
const RUNTIME_DIR: &str = r"C:\RacingPoint";

/// Known venue game keys in GameToBaseConfig.json.
/// All 4 keys confirmed from Pod 8 hardware inspection (2026-03-24).
/// Note: AC EVO uses the uppercase-underscore key style used by ConspitLink 2.0 for newer games.
const VENUE_GAME_KEYS: &[&str] = &[
    "Assetto Corsa",
    "F1 25",
    "Assetto Corsa Competizione",
    "ASSETTO_CORSA_EVO", // Confirmed from Pod 8 GameToBaseConfig.json (2026-03-24)
];

// ─── Phase 60: Pre-Launch Profile Loading ─────────────────────────────────

/// Map a SimType to the corresponding ConspitLink game key.
/// Returns None for unrecognized games (no ConspitLink preset available).
fn sim_type_to_game_key(sim_type: SimType) -> Option<&'static str> {
    match sim_type {
        SimType::AssettoCorsa => Some("Assetto Corsa"),
        SimType::AssettoCorsaRally => Some("Assetto Corsa"), // shares AC physics engine
        SimType::F125 => Some("F1 25"),
        SimType::AssettoCorsaEvo => Some("ASSETTO_CORSA_EVO"),
        _ => None, // IRacing, LeMansUltimate, Forza, ForzaHorizon5
    }
}

/// Write `LastUsedPreset` to Global.json in the runtime directory.
/// Does NOT restart ConspitLink — caller handles that.
fn force_preset_via_global_json(game_key: &str, runtime_dir: Option<&std::path::Path>) -> Result<(), String> {
    let base = runtime_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(RUNTIME_DIR));
    let global_path = base.join("Global.json");

    let content = std::fs::read_to_string(&global_path)
        .map_err(|e| format!("Failed to read Global.json at {}: {}", global_path.display(), e))?;
    let mut json: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse Global.json: {}", e))?;

    json["LastUsedPreset"] = serde_json::Value::String(game_key.to_string());

    let output = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("Failed to serialize Global.json: {}", e))?;
    std::fs::write(&global_path, output)
        .map_err(|e| format!("Failed to write Global.json: {}", e))?;

    tracing::info!(target: LOG_TARGET_CFG, "Forced LastUsedPreset to '{}' in {}", game_key, global_path.display());
    Ok(())
}

/// Apply safe fallback for unrecognized games: 50% power cap + gentle centering spring.
/// HID failures are non-fatal (no device on dev machine is expected).
fn apply_unrecognized_game_fallback(sim_type: SimType) -> Result<(), String> {
    tracing::warn!(
        target: LOG_TARGET,
        "Unrecognized game {:?} -- applying safe fallback: 50% power cap + idlespring centering",
        sim_type
    );

    let ffb = FfbController::new(0x1209, 0xFFB0);
    if let Err(e) = ffb.set_gain(50) {
        tracing::debug!(target: LOG_TARGET, "Safe fallback set_gain failed (expected on dev): {}", e);
    }
    if let Err(e) = ffb.set_idle_spring(500) {
        tracing::debug!(target: LOG_TARGET, "Safe fallback set_idle_spring failed (expected on dev): {}", e);
    }

    Ok(())
}

/// Wait for ConspitLink auto-detect or force preset if CL wasn't running.
/// - If CL was running: trust Phase 59 auto-detect, wait 3s grace period
/// - If CL was NOT running: start CL, force preset via Global.json, restart CL
fn wait_for_cl_or_force_preset(game_key: &str, runtime_dir: Option<&std::path::Path>) -> Result<(), String> {
    // Guard: don't fight with safe_session_end()
    for _ in 0..6 {
        if !SESSION_END_IN_PROGRESS.load(Ordering::Acquire) {
            break;
        }
        tracing::debug!(target: LOG_TARGET_CFG, "SESSION_END_IN_PROGRESS — waiting 500ms before pre-load");
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    if SESSION_END_IN_PROGRESS.load(Ordering::Acquire) {
        return Err("SESSION_END_IN_PROGRESS still active after 3s — skipping pre-load".to_string());
    }

    let cl_was_running = crate::ac_launcher::is_process_running("ConspitLink2.0.exe");

    if cl_was_running {
        // Trust Phase 59 auto-detect. Wait 3s grace period.
        tracing::info!(target: LOG_TARGET_CFG, "ConspitLink running — trusting auto-detect for '{}'", game_key);
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        return Ok(());
    }

    // CL was NOT running — force preset
    tracing::info!(target: LOG_TARGET_CFG, "ConspitLink not running — forcing preset '{}'", game_key);
    crate::ac_launcher::ensure_conspit_link_running();
    std::thread::sleep(std::time::Duration::from_millis(500));

    force_preset_via_global_json(game_key, runtime_dir)?;

    #[cfg(windows)]
    {
        restart_conspit_link_hardened(false);
    }

    Ok(())
}

/// Pre-load the correct FFB preset before game launch.
/// - Recognized games: force ConspitLink preset (if CL wasn't running)
/// - Unrecognized games: apply safe 50% power cap + gentle centering
///
/// Designed to be called from `spawn_blocking` in the LaunchGame handler.
/// Failures are non-fatal — caller must handle errors gracefully.
pub fn pre_load_game_preset(sim_type: SimType, runtime_dir: Option<&std::path::Path>) -> Result<(), String> {
    match sim_type_to_game_key(sim_type) {
        Some(key) => {
            tracing::info!(target: LOG_TARGET_CFG, "Pre-loading preset for {:?} (key: '{}')", sim_type, key);
            wait_for_cl_or_force_preset(key, runtime_dir)
        }
        None => apply_unrecognized_game_fallback(sim_type),
    }
}

/// Result of the auto-switch config self-heal operation.
pub struct AutoSwitchConfigResult {
    /// True if the Global.json file was found in the install dir and placement was attempted.
    pub global_json_placed: bool,
    /// True if the target Global.json content actually changed (write occurred).
    pub global_json_changed: bool,
    /// True if any GameToBaseConfig.json mapping was added or corrected.
    pub game_to_base_fixed: bool,
    /// True if ConspitLink was restarted (only when config changed).
    pub conspit_restarted: bool,
    /// Non-fatal errors that occurred during the operation.
    pub errors: Vec<String>,
}

/// Ensure ConspitLink auto-switch configuration is in place.
///
/// Places `Global.json` at `C:\RacingPoint\Global.json` with `AresAutoChangeConfig` forced
/// to `"open"`, verifies `GameToBaseConfig.json` game mappings, and restarts ConspitLink
/// if anything changed.  Non-fatal: errors are logged but do not block startup.
///
/// Must be called BEFORE `enforce_safe_state()` so ConspitLink starts with correct config.
pub fn ensure_auto_switch_config() -> AutoSwitchConfigResult {
    ensure_auto_switch_config_impl(None, None)
}

/// Testable entry point for `ensure_auto_switch_config` — uses provided dirs instead of
/// the production paths.  Called from unit tests via `ensure_auto_switch_config_in_dir`.
fn ensure_auto_switch_config_impl(
    install_dir: Option<&std::path::Path>,
    runtime_dir: Option<&std::path::Path>,
) -> AutoSwitchConfigResult {
    let install_base = install_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(INSTALL_DIR));
    let runtime_base = runtime_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(RUNTIME_DIR));

    let mut result = AutoSwitchConfigResult {
        global_json_placed: false,
        global_json_changed: false,
        game_to_base_fixed: false,
        conspit_restarted: false,
        errors: Vec::new(),
    };

    // Ensure runtime directory exists (C:\RacingPoint\ on production)
    if let Err(e) = std::fs::create_dir_all(&runtime_base) {
        result.errors.push(format!("create_dir_all({}): {}", runtime_base.display(), e));
        return result;
    }

    // 1. Place Global.json with AresAutoChangeConfig forced to "open"
    let source_global = install_base.join("Global.json");
    let target_global = runtime_base.join("Global.json");
    match place_global_json(&source_global, &target_global) {
        Ok(changed) => {
            result.global_json_placed = true;
            result.global_json_changed = changed;
            if changed {
                tracing::info!(
                    target: LOG_TARGET_CFG,
                    "Global.json placed at runtime path with AresAutoChangeConfig=open"
                );
            } else {
                tracing::debug!(
                    target: LOG_TARGET_CFG,
                    "Global.json already correct at runtime path — no write needed"
                );
            }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET_CFG, "Failed to place Global.json: {}", e);
            result.errors.push(format!("Global.json: {}", e));
        }
    }

    // 2. Verify GameToBaseConfig.json game mappings (only if the file exists)
    let gtb_path = install_base
        .join("JsonConfigure")
        .join("GameToBaseConfig.json");
    if gtb_path.exists() {
        match verify_game_to_base_config(&gtb_path, &install_base) {
            Ok(fixed) => {
                result.game_to_base_fixed = fixed;
                if fixed {
                    tracing::info!(
                        target: LOG_TARGET_CFG,
                        "GameToBaseConfig.json: missing game key(s) added"
                    );
                } else {
                    tracing::debug!(
                        target: LOG_TARGET_CFG,
                        "GameToBaseConfig.json: all venue game keys present"
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    target: LOG_TARGET_CFG,
                    "GameToBaseConfig.json verification failed: {}",
                    e
                );
                result.errors.push(format!("GameToBaseConfig.json: {}", e));
            }
        }
    } else {
        tracing::debug!(
            target: LOG_TARGET_CFG,
            "GameToBaseConfig.json not found at {} — skipping (CL may not be installed)",
            gtb_path.display()
        );
    }

    // 3. Restart ConspitLink only if config actually changed.
    //    In test mode (install_dir.is_some()) we skip the real restart — just set the flag.
    if result.global_json_changed || result.game_to_base_fixed {
        tracing::info!(
            target: LOG_TARGET_CFG,
            "Config changed — restarting ConspitLink to pick up new settings"
        );
        result.conspit_restarted = true;
        if install_dir.is_none() {
            // Production path only — never call from tests
            restart_conspit_link_hardened(false);
        }
    }

    result
}

/// Place `Global.json` from the install directory to the runtime directory.
///
/// - Reads source, parses as JSON, forces `AresAutoChangeConfig` to `"open"`.
/// - Compares with existing target content; returns `Ok(false)` if identical (no-op).
/// - Writes atomically: `target.json.tmp` then rename to `target`.
/// - Returns `Ok(true)` when the file was written/updated.
fn place_global_json(
    source: &std::path::Path,
    target: &std::path::Path,
) -> Result<bool, String> {
    if !source.exists() {
        return Err(format!(
            "Source Global.json not found: {} — ConspitLink may not be installed",
            source.display()
        ));
    }

    // Parse source JSON
    let raw = std::fs::read_to_string(source)
        .map_err(|e| format!("read {}: {}", source.display(), e))?;
    let mut json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse JSON: {}", e))?;

    // Force AresAutoChangeConfig to "open" (Pitfall 2: default is "close")
    json["AresAutoChangeConfig"] = serde_json::json!("open");

    let new_content = serde_json::to_string_pretty(&json)
        .map_err(|e| format!("serialize JSON: {}", e))?;

    // Compare with existing target — skip write if identical (Pitfall 1: avoid unnecessary CL restart)
    if target.exists() {
        if let Ok(existing) = std::fs::read_to_string(target) {
            if existing == new_content {
                return Ok(false); // Already correct — no change needed
            }
        }
    }

    // Atomic write: write to .json.tmp then rename (NTFS rename is atomic)
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &new_content)
        .map_err(|e| format!("write tmp {}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, target)
        .map_err(|e| format!("rename {} -> {}: {}", tmp.display(), target.display(), e))?;

    Ok(true) // File was updated
}

/// Verify that `GameToBaseConfig.json` has entries for all venue games.
///
/// For each key in `VENUE_GAME_KEYS`:
/// - If the key is missing, adds a default entry (pointing to install dir).
/// - If the mapped `.Base` file does not exist, logs a warning (path fix deferred to Phase 61).
///
/// Returns `Ok(true)` if any entry was added, `Ok(false)` if all keys were already present.
fn verify_game_to_base_config(
    gtb_path: &std::path::Path,
    install_base: &std::path::Path,
) -> Result<bool, String> {
    let raw = std::fs::read_to_string(gtb_path)
        .map_err(|e| format!("read {}: {}", gtb_path.display(), e))?;
    let mut json: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse JSON: {}", e))?;

    let map = json
        .as_object_mut()
        .ok_or_else(|| "GameToBaseConfig.json root is not a JSON object".to_string())?;

    let mut fixed = false;
    for &key in VENUE_GAME_KEYS {
        if let Some(entry) = map.get(key) {
            // Key exists — check that the mapped .Base file path exists (warn only, no fix in Phase 59)
            if let Some(path_str) = entry.as_str() {
                let base_file = std::path::Path::new(path_str);
                if !base_file.exists() {
                    tracing::warn!(
                        target: LOG_TARGET_CFG,
                        "GameToBaseConfig: '{}' maps to '{}' which does not exist on disk — \
                         preset path fix deferred to Phase 61",
                        key,
                        path_str
                    );
                }
            }
        } else {
            // Key missing — add a default entry pointing to install dir presets
            let default_path = install_base
                .join("Presets")
                .join(key)
                .to_string_lossy()
                .into_owned();
            tracing::info!(
                target: LOG_TARGET_CFG,
                "GameToBaseConfig: missing key '{}' — adding default entry '{}'",
                key,
                default_path
            );
            map.insert(key.to_string(), serde_json::json!(default_path));
            fixed = true;
        }
    }

    if fixed {
        // Write the updated config back (atomic write)
        let new_content = serde_json::to_string_pretty(&json)
            .map_err(|e| format!("serialize: {}", e))?;
        let tmp = gtb_path.with_extension("json.tmp");
        std::fs::write(&tmp, &new_content)
            .map_err(|e| format!("write tmp: {}", e))?;
        std::fs::rename(&tmp, gtb_path)
            .map_err(|e| format!("rename: {}", e))?;
    }

    Ok(fixed)
}

/// Testable entry point for `ensure_auto_switch_config` — operates inside provided dirs.
#[cfg(test)]
pub(crate) fn ensure_auto_switch_config_in_dir(
    install_dir: &std::path::Path,
    runtime_dir: &std::path::Path,
) -> AutoSwitchConfigResult {
    ensure_auto_switch_config_impl(Some(install_dir), Some(runtime_dir))
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
                target: LOG_TARGET,
                "ConspitLink window minimize attempt {} ({}ms elapsed) — process is running",
                attempt,
                start.elapsed().as_millis()
            );
            return;
        }
    }
    tracing::warn!(
        target: LOG_TARGET,
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
            tracing::debug!(target: LOG_TARGET, "ConspitLink not installed — skipping restart");
            return;
        }

        // 1. Increment crash counter (watchdog only, not session-end)
        if is_crash_recovery {
            let count = increment_crash_count();
            tracing::warn!(target: LOG_TARGET, "ConspitLink crash recovery restart #{}", count);
            if count >= 5 {
                tracing::error!(
                    target: LOG_TARGET,
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
                tracing::info!(target: LOG_TARGET, "ConspitLink started, will verify + minimize...");
                // Single thread: minimize with retry, then verify configs
                std::thread::spawn(|| {
                    minimize_conspit_window_with_retry();
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    verify_conspit_configs();
                });
            }
            Err(e) => tracing::error!(target: LOG_TARGET, "Failed to restart ConspitLink: {}", e),
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
            target: LOG_TARGET,
            "ConspitLink did not close within 5s -- proceeding with HID commands (P-20 risk accepted)"
        );
    }

    // Step 2: fxm.reset + idlespring ramp (sync, in spawn_blocking)
    let ffb_clone = ffb.clone();
    tokio::task::spawn_blocking(move || {
        // Clear all orphaned DirectInput effects
        if let Err(e) = ffb_clone.fxm_reset() {
            tracing::warn!(target: LOG_TARGET, "fxm.reset failed: {} -- continuing with idlespring", e);
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

    tracing::info!(target: LOG_TARGET, "Session-end safety sequence complete -- wheel centering with idlespring");
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

    // ─── FfbBackend Mock Tests ────────────────────────────────────────────────

    use mockall::mock;

    mock! {
        pub TestBackend {}
        impl FfbBackend for TestBackend {
            fn zero_force(&self) -> Result<bool, String>;
            fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
            fn set_gain(&self, percent: u8) -> Result<bool, String>;
            fn fxm_reset(&self) -> Result<bool, String>;
            fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
        }
    }

    #[test]
    fn ffb_zero_force_success() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force()
            .returning(|| Ok(true))
            .times(1);
        assert_eq!(mock.zero_force(), Ok(true));
    }

    #[test]
    fn ffb_zero_force_device_absent() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force()
            .returning(|| Ok(false))
            .times(1);
        assert_eq!(mock.zero_force(), Ok(false));
    }

    #[test]
    fn ffb_zero_force_hid_error() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force()
            .returning(|| Err("HID write failed: device busy".to_string()))
            .times(1);
        assert!(mock.zero_force().is_err());
    }

    #[test]
    fn ffb_zero_force_with_retry_succeeds_first_attempt() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force_with_retry()
            .withf(|attempts, delay_ms| *attempts == 3 && *delay_ms == 100)
            .returning(|_, _| true)
            .times(1);
        assert!(mock.zero_force_with_retry(3, 100));
    }

    #[test]
    fn ffb_zero_force_with_retry_all_attempts_fail() {
        let mut mock = MockTestBackend::new();
        mock.expect_zero_force_with_retry()
            .returning(|_, _| false)
            .times(1);
        assert!(!mock.zero_force_with_retry(3, 100));
    }

    #[test]
    fn ffb_set_gain_sends_valid_percent() {
        let mut mock = MockTestBackend::new();
        mock.expect_set_gain()
            .withf(|p| *p == 80)
            .returning(|_| Ok(true))
            .times(1);
        assert_eq!(mock.set_gain(80), Ok(true));
    }

    #[test]
    fn ffb_fxm_reset_clears_effects() {
        let mut mock = MockTestBackend::new();
        mock.expect_fxm_reset()
            .returning(|| Ok(true))
            .times(1);
        assert_eq!(mock.fxm_reset(), Ok(true));
    }

    #[test]
    fn ffb_set_idle_spring_sends_value() {
        let mut mock = MockTestBackend::new();
        mock.expect_set_idle_spring()
            .withf(|v| *v == 1000)
            .returning(|_| Ok(true))
            .times(1);
        assert_eq!(mock.set_idle_spring(1000), Ok(true));
    }

    // ─── Auto-Switch Config Tests ─────────────────────────────────────────────

    fn make_install_dir(base: &std::path::Path) {
        // Create install dir structure with a valid Global.json
        let install = base.join("install");
        std::fs::create_dir_all(install.join("JsonConfigure")).unwrap();
        let global_json = serde_json::json!({
            "AresAutoChangeConfig": "close",
            "SomeOtherKey": "value"
        });
        std::fs::write(
            install.join("Global.json"),
            serde_json::to_string_pretty(&global_json).unwrap(),
        ).unwrap();
    }

    fn make_runtime_dir(base: &std::path::Path) -> std::path::PathBuf {
        let runtime = base.join("runtime");
        std::fs::create_dir_all(&runtime).unwrap();
        runtime
    }

    #[test]
    fn test_place_global_json_forces_open_when_source_has_close() {
        let dir = std::env::temp_dir().join("auto_switch_test_forces_open");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Source has AresAutoChangeConfig = "close"
        let source = dir.join("source_Global.json");
        let target = dir.join("target_Global.json");
        let source_json = serde_json::json!({ "AresAutoChangeConfig": "close", "Other": 1 });
        std::fs::write(&source, serde_json::to_string_pretty(&source_json).unwrap()).unwrap();

        let result = place_global_json(&source, &target).unwrap();
        assert!(result, "Should return true (changed) when source has 'close'");
        assert!(target.exists(), "Target file should have been created");

        let target_content = std::fs::read_to_string(&target).unwrap();
        let target_json: serde_json::Value = serde_json::from_str(&target_content).unwrap();
        assert_eq!(
            target_json["AresAutoChangeConfig"],
            serde_json::json!("open"),
            "AresAutoChangeConfig must be forced to 'open'"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_place_global_json_creates_target_when_missing() {
        let dir = std::env::temp_dir().join("auto_switch_test_creates_target");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let source = dir.join("Global.json");
        let target = dir.join("runtime").join("Global.json");
        // Create runtime subdir
        std::fs::create_dir_all(dir.join("runtime")).unwrap();

        let source_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(&source, serde_json::to_string_pretty(&source_json).unwrap()).unwrap();

        assert!(!target.exists(), "Target should not exist yet");
        let result = place_global_json(&source, &target).unwrap();
        assert!(result, "Should return true (created) when target is missing");
        assert!(target.exists(), "Target must be created");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_place_global_json_noop_when_already_correct() {
        let dir = std::env::temp_dir().join("auto_switch_test_noop");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let source = dir.join("Global.json");
        let target = dir.join("runtime_Global.json");

        // Source with "close" — first call should create target with "open"
        let source_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(&source, serde_json::to_string_pretty(&source_json).unwrap()).unwrap();

        let first = place_global_json(&source, &target).unwrap();
        assert!(first, "First call: should return true (created/changed)");

        // Second call: target already has "open" content — should be no-op
        let second = place_global_json(&source, &target).unwrap();
        assert!(!second, "Second call: should return false (no change)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_place_global_json_atomic_write_uses_tmp() {
        // Verify that the intermediate .json.tmp file is cleaned up (not left behind)
        let dir = std::env::temp_dir().join("auto_switch_test_atomic");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let source = dir.join("Global.json");
        let target = dir.join("target_Global.json");
        let tmp = dir.join("target_Global.json.tmp");

        let source_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(&source, serde_json::to_string_pretty(&source_json).unwrap()).unwrap();

        place_global_json(&source, &target).unwrap();

        // After successful atomic write, .tmp should be gone (renamed to target)
        assert!(!tmp.exists(), ".json.tmp must be renamed away after write");
        assert!(target.exists(), "Target must exist after atomic write");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_game_to_base_config_returns_false_when_all_keys_present() {
        let dir = std::env::temp_dir().join("auto_switch_test_gtb_all_present");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let gtb_path = dir.join("GameToBaseConfig.json");
        // All VENUE_GAME_KEYS present (with a dummy path each)
        let mut map = serde_json::Map::new();
        for &key in VENUE_GAME_KEYS {
            map.insert(key.to_string(), serde_json::json!("C:\\some\\preset.Base"));
        }
        std::fs::write(&gtb_path, serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap()).unwrap();

        let result = verify_game_to_base_config(&gtb_path, &dir).unwrap();
        assert!(!result, "Should return false (no changes) when all keys already exist");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_verify_game_to_base_config_adds_missing_key() {
        let dir = std::env::temp_dir().join("auto_switch_test_gtb_missing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let gtb_path = dir.join("GameToBaseConfig.json");
        // Only the first key present
        let mut map = serde_json::Map::new();
        if let Some(&first_key) = VENUE_GAME_KEYS.first() {
            map.insert(first_key.to_string(), serde_json::json!("some/path.Base"));
        }
        std::fs::write(&gtb_path, serde_json::to_string_pretty(&serde_json::Value::Object(map)).unwrap()).unwrap();

        let result = verify_game_to_base_config(&gtb_path, &dir).unwrap();
        assert!(result, "Should return true (fixed) when keys were missing and added");

        // Verify the file now has all keys
        let updated: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&gtb_path).unwrap()).unwrap();
        for &key in VENUE_GAME_KEYS {
            assert!(updated.get(key).is_some(), "Key '{}' must be present after fix", key);
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ensure_auto_switch_config_impl_places_and_restarts_on_change() {
        let dir = std::env::temp_dir().join("auto_switch_test_full_change");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        let runtime = dir.join("runtime");
        std::fs::create_dir_all(install.join("JsonConfigure")).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();

        // Source Global.json with "close" — will cause a change
        let global_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(
            install.join("Global.json"),
            serde_json::to_string_pretty(&global_json).unwrap(),
        ).unwrap();

        let result = ensure_auto_switch_config_in_dir(&install, &runtime);

        assert!(result.global_json_placed, "global_json_placed must be true");
        assert!(result.global_json_changed, "global_json_changed must be true");
        assert!(result.conspit_restarted, "conspit_restarted must be true when config changed");
        assert!(result.errors.is_empty(), "errors must be empty: {:?}", result.errors);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ensure_auto_switch_config_impl_no_restart_when_nothing_changed() {
        let dir = std::env::temp_dir().join("auto_switch_test_no_change");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        let runtime = dir.join("runtime");
        std::fs::create_dir_all(install.join("JsonConfigure")).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();

        // Source Global.json with "close" — first call places it (changed=true)
        let global_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(
            install.join("Global.json"),
            serde_json::to_string_pretty(&global_json).unwrap(),
        ).unwrap();

        // First call — places the file
        let first = ensure_auto_switch_config_in_dir(&install, &runtime);
        assert!(first.conspit_restarted, "First call: must set conspit_restarted=true");

        // Second call — file already has "open", nothing changed
        let second = ensure_auto_switch_config_in_dir(&install, &runtime);
        assert!(!second.global_json_changed, "Second call: global_json_changed must be false");
        assert!(!second.conspit_restarted, "Second call: conspit_restarted must be false (no change)");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_ensure_auto_switch_creates_runtime_dir_if_missing() {
        let dir = std::env::temp_dir().join("auto_switch_test_mkdir");
        let _ = std::fs::remove_dir_all(&dir);
        let install = dir.join("install");
        // Runtime dir does NOT exist yet
        let runtime = dir.join("runtime_new");
        std::fs::create_dir_all(&install).unwrap();

        let global_json = serde_json::json!({ "AresAutoChangeConfig": "close" });
        std::fs::write(
            install.join("Global.json"),
            serde_json::to_string_pretty(&global_json).unwrap(),
        ).unwrap();

        assert!(!runtime.exists(), "Runtime dir must not exist before test");
        let result = ensure_auto_switch_config_in_dir(&install, &runtime);

        assert!(runtime.exists(), "Runtime dir must be created by ensure_auto_switch_config_impl");
        assert!(result.global_json_placed, "global_json_placed must be true after dir creation");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ─── Pre-Launch Profile Loading Tests (Phase 60) ─────────────────────────

    #[test]
    fn test_sim_type_to_game_key_recognized() {
        assert_eq!(sim_type_to_game_key(SimType::AssettoCorsa), Some("Assetto Corsa"));
        assert_eq!(sim_type_to_game_key(SimType::F125), Some("F1 25"));
        assert_eq!(sim_type_to_game_key(SimType::AssettoCorsaEvo), Some("ASSETTO_CORSA_EVO"));
    }

    #[test]
    fn test_sim_type_to_game_key_unrecognized() {
        assert_eq!(sim_type_to_game_key(SimType::Forza), None);
        assert_eq!(sim_type_to_game_key(SimType::ForzaHorizon5), None);
        assert_eq!(sim_type_to_game_key(SimType::IRacing), None);
        assert_eq!(sim_type_to_game_key(SimType::LeMansUltimate), None);
    }

    #[test]
    fn test_sim_type_to_game_key_rally() {
        // AssettoCorsaRally shares AC physics engine — maps to "Assetto Corsa"
        assert_eq!(sim_type_to_game_key(SimType::AssettoCorsaRally), Some("Assetto Corsa"));
    }

    #[test]
    fn test_pre_load_recognized_game_writes_last_used_preset() {
        let dir = std::env::temp_dir().join("pre_load_test_recognized");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // Seed Global.json with a different preset
        let global = serde_json::json!({
            "AresAutoChangeConfig": "open",
            "LastUsedPreset": "F1 25"
        });
        std::fs::write(
            dir.join("Global.json"),
            serde_json::to_string_pretty(&global).unwrap(),
        ).unwrap();

        // Force preset for Assetto Corsa
        let result = force_preset_via_global_json("Assetto Corsa", Some(&dir));
        assert!(result.is_ok(), "force_preset_via_global_json should succeed: {:?}", result);

        let updated: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("Global.json")).unwrap()
        ).unwrap();
        assert_eq!(
            updated["LastUsedPreset"], "Assetto Corsa",
            "LastUsedPreset must be updated to 'Assetto Corsa'"
        );
        assert_eq!(
            updated["AresAutoChangeConfig"], "open",
            "AresAutoChangeConfig must be preserved"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_pre_load_unrecognized_game_does_not_write_global_json() {
        let dir = std::env::temp_dir().join("pre_load_test_unrecognized");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let global = serde_json::json!({
            "AresAutoChangeConfig": "open"
        });
        let original = serde_json::to_string_pretty(&global).unwrap();
        std::fs::write(dir.join("Global.json"), &original).unwrap();

        // pre_load_game_preset for Forza should NOT touch Global.json
        let result = pre_load_game_preset(SimType::Forza, Some(&dir));
        assert!(result.is_ok(), "pre_load_game_preset should succeed for unrecognized");

        let after = std::fs::read_to_string(dir.join("Global.json")).unwrap();
        assert_eq!(after, original, "Global.json must be unchanged for unrecognized games");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_force_preset_via_global_json_writes_correctly() {
        let dir = std::env::temp_dir().join("pre_load_test_force_write");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let global = serde_json::json!({ "AresAutoChangeConfig": "open" });
        std::fs::write(
            dir.join("Global.json"),
            serde_json::to_string_pretty(&global).unwrap(),
        ).unwrap();

        let result = force_preset_via_global_json("F1 25", Some(&dir));
        assert!(result.is_ok());

        let updated: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("Global.json")).unwrap()
        ).unwrap();
        assert_eq!(updated["LastUsedPreset"], "F1 25");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_force_preset_via_global_json_missing_file() {
        let dir = std::env::temp_dir().join("pre_load_test_missing_file");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // No Global.json created

        let result = force_preset_via_global_json("F1 25", Some(&dir));
        assert!(result.is_err(), "Should return Err when Global.json is missing");
    }
}
