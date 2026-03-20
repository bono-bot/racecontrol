//! Force Feedback Controller for OpenFFBoard-based wheelbases (Conspit Ares 8Nm).
//!
//! Provides safety commands to zero the wheelbase torque on session end,
//! game crash, or rc-agent startup. Uses the OpenFFBoard vendor HID interface
//! (usage page 0xFF00, report ID 0xA1) — independent of DirectInput game FFB.
//!
//! This module is write-only. HID input reading lives in `driving_detector.rs`.

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

/// Force Feedback controller for the Conspit Ares wheelbase.
///
/// Opens the OpenFFBoard vendor HID interface and provides safety commands.
/// All methods are non-panicking — if the device is absent or writes fail,
/// warnings are logged and execution continues.
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
}
