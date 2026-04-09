//! Native Win32 animated blanking screen — embedded frame sequence.
//!
//! 24 frames captured from http://192.168.31.23:3300/kiosk/blanking at 960x540,
//! loaded into HBITMAPs at startup and cycled via WM_TIMER during ScreenBlanked.
//! StretchBlt upscales to target area on the lock screen.
//!
//! Captured via `scripts/diagnostics/capture-blanking-frames.js` (Playwright).
//! Re-run capture script + rebuild when the kiosk blanking page design changes.

#![cfg(windows)]

use winapi::shared::windef::HBITMAP;

/// Target frame dimensions (matches capture-blanking-frames.js WIDTH/HEIGHT).
pub const FRAME_W: i32 = 960;
pub const FRAME_H: i32 = 540;
pub const FRAME_COUNT: usize = 24;

/// Embedded PNG bytes for each frame. Order matches loop sequence.
const FRAME_PNGS: [&[u8]; FRAME_COUNT] = [
    include_bytes!("../../assets/blanking-frames/frame-00.png"),
    include_bytes!("../../assets/blanking-frames/frame-01.png"),
    include_bytes!("../../assets/blanking-frames/frame-02.png"),
    include_bytes!("../../assets/blanking-frames/frame-03.png"),
    include_bytes!("../../assets/blanking-frames/frame-04.png"),
    include_bytes!("../../assets/blanking-frames/frame-05.png"),
    include_bytes!("../../assets/blanking-frames/frame-06.png"),
    include_bytes!("../../assets/blanking-frames/frame-07.png"),
    include_bytes!("../../assets/blanking-frames/frame-08.png"),
    include_bytes!("../../assets/blanking-frames/frame-09.png"),
    include_bytes!("../../assets/blanking-frames/frame-10.png"),
    include_bytes!("../../assets/blanking-frames/frame-11.png"),
    include_bytes!("../../assets/blanking-frames/frame-12.png"),
    include_bytes!("../../assets/blanking-frames/frame-13.png"),
    include_bytes!("../../assets/blanking-frames/frame-14.png"),
    include_bytes!("../../assets/blanking-frames/frame-15.png"),
    include_bytes!("../../assets/blanking-frames/frame-16.png"),
    include_bytes!("../../assets/blanking-frames/frame-17.png"),
    include_bytes!("../../assets/blanking-frames/frame-18.png"),
    include_bytes!("../../assets/blanking-frames/frame-19.png"),
    include_bytes!("../../assets/blanking-frames/frame-20.png"),
    include_bytes!("../../assets/blanking-frames/frame-21.png"),
    include_bytes!("../../assets/blanking-frames/frame-22.png"),
    include_bytes!("../../assets/blanking-frames/frame-23.png"),
];

const LOG_TARGET: &str = "native-lock-blanking";

/// Decode all embedded PNG frames into top-down 32-bit BGRA DIB sections.
/// Returns an empty Vec on any failure — caller falls back to pure-black blanking.
pub fn load_all_frames() -> Vec<HBITMAP> {
    let mut out = Vec::with_capacity(FRAME_COUNT);
    for (i, png_bytes) in FRAME_PNGS.iter().enumerate() {
        match decode_png_to_hbitmap(png_bytes) {
            Some(hbmp) => out.push(hbmp),
            None => {
                tracing::warn!(target: LOG_TARGET, "Failed to decode blanking frame {}", i);
                // Clean up already-loaded frames to avoid HBITMAP leak
                unsafe {
                    use winapi::um::wingdi::DeleteObject;
                    for h in out.drain(..) {
                        DeleteObject(h as *mut _);
                    }
                }
                return Vec::new();
            }
        }
    }
    tracing::info!(target: LOG_TARGET, "Loaded {} blanking frames ({}x{})", out.len(), FRAME_W, FRAME_H);
    out
}

/// Decode a single PNG byte slice into a 32-bit BGRA top-down DIB section.
/// Returns None on decode/allocation failure.
fn decode_png_to_hbitmap(png_bytes: &[u8]) -> Option<HBITMAP> {
    use winapi::um::wingdi::{
        CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    };

    let img = image::load_from_memory(png_bytes).ok()?.to_rgba8();
    let (w, h) = (img.width() as i32, img.height() as i32);
    if w != FRAME_W || h != FRAME_H {
        tracing::warn!(
            target: LOG_TARGET,
            "Frame dimension mismatch: expected {}x{}, got {}x{}",
            FRAME_W, FRAME_H, w, h
        );
        return None;
    }

    // Construct BITMAPINFO via zeroed allocation to avoid RGBQUAD Default bound.
    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bits_ptr: *mut winapi::ctypes::c_void = std::ptr::null_mut();
    let hbmp = unsafe {
        CreateDIBSection(
            std::ptr::null_mut(),
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr as *mut *mut winapi::ctypes::c_void,
            std::ptr::null_mut(),
            0,
        )
    };

    if hbmp.is_null() || bits_ptr.is_null() {
        return None;
    }

    // Copy RGBA -> BGRA (no alpha pre-multiply — we render with SRCCOPY, not AlphaBlend)
    let pixel_count = (w * h) as usize;
    let src = img.as_raw();
    let dst: &mut [u8] = unsafe {
        std::slice::from_raw_parts_mut(bits_ptr as *mut u8, pixel_count * 4)
    };
    for i in 0..pixel_count {
        let si = i * 4;
        dst[si] = src[si + 2];     // B
        dst[si + 1] = src[si + 1]; // G
        dst[si + 2] = src[si];     // R
        dst[si + 3] = src[si + 3]; // A (unused for SRCCOPY)
    }

    Some(hbmp)
}
