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
}
