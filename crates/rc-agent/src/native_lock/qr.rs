//! QR code GDI renderer for the native lock screen.
//!
//! Renders QR codes as a pixel grid using GDI FillRect, with proper quiet zone
//! and centered positioning. Uses the `qrcode` crate for encoding.

#![allow(unsafe_op_in_unsafe_fn)]

/// Paint a QR code centered at (center_x, center_y) with the given module size.
///
/// Each QR "module" (pixel) is rendered as a `module_size x module_size` filled rect.
/// A white quiet zone of 1 module_size is added around the QR code.
///
/// GDI handles (brushes) are created and destroyed within this function -- no leaks.
///
/// # Safety
/// Must be called from the window thread with a valid HDC.
#[cfg(windows)]
pub unsafe fn paint_qr_code(
    hdc: winapi::shared::windef::HDC,
    qr_data: &str,
    center_x: i32,
    center_y: i32,
    module_size: i32,
) {
    use qrcode::QrCode;
    use winapi::shared::windef::RECT;
    use winapi::um::wingdi::{CreateSolidBrush, DeleteObject};
    use winapi::um::winuser::FillRect;

    let code = match QrCode::new(qr_data.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "native-lock", "QR code generation failed: {}", e);
            return;
        }
    };

    let modules = code.to_colors();
    let width = code.width() as i32;

    // quiet_zone = 1 module border on each side
    let quiet_zone = 1;
    let total_modules = width + quiet_zone * 2;
    let total_size = total_modules * module_size;

    // Top-left corner for centering
    let start_x = center_x - total_size / 2;
    let start_y = center_y - total_size / 2;

    // Create brushes -- MUST delete both before returning
    let white_brush = CreateSolidBrush(0x00FFFFFF); // RGB(255, 255, 255)
    let black_brush = CreateSolidBrush(0x00000000); // RGB(0, 0, 0)

    // Draw white background (quiet zone + QR area)
    let bg_rect = RECT {
        left: start_x,
        top: start_y,
        right: start_x + total_size,
        bottom: start_y + total_size,
    };
    FillRect(hdc, &bg_rect, white_brush);

    // Draw dark modules
    let qr_start_x = start_x + quiet_zone * module_size;
    let qr_start_y = start_y + quiet_zone * module_size;

    for row in 0..width {
        for col in 0..width {
            let idx = (row * width + col) as usize;
            if idx < modules.len() && modules[idx] == qrcode::Color::Dark {
                let rect = RECT {
                    left: qr_start_x + col * module_size,
                    top: qr_start_y + row * module_size,
                    right: qr_start_x + (col + 1) * module_size,
                    bottom: qr_start_y + (row + 1) * module_size,
                };
                FillRect(hdc, &rect, black_brush);
            }
        }
    }

    // CRITICAL: Delete brushes to prevent GDI handle leak
    DeleteObject(white_brush as *mut _);
    DeleteObject(black_brush as *mut _);
}

/// Non-windows stub for compilation.
#[cfg(not(windows))]
pub unsafe fn paint_qr_code(
    _hdc: *mut std::ffi::c_void,
    _qr_data: &str,
    _center_x: i32,
    _center_y: i32,
    _module_size: i32,
) {}
