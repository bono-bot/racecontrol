//! Font embedding and GDI resource cache for the native lock screen.
//!
//! Loads Montserrat TTF via `include_bytes!` + `AddFontResourceExW(FR_PRIVATE)`,
//! with automatic fallback to Segoe UI if font installation fails.
//! GDI handles are cached in `LockGdiResources` and freed on Drop.

#![allow(unsafe_op_in_unsafe_fn)]

#[cfg(windows)]
use winapi::shared::windef::{HBRUSH, HFONT};

#[cfg(windows)]
use winapi::shared::windef::HBITMAP;

const LOG_TARGET: &str = "lock-screen-font";

/// Embedded Racing Point logo PNG (~47KB).
const LOGO_PNG: &[u8] = include_bytes!("../../assets/logo/rp-logo-blanking.png");

/// Embedded Montserrat Regular TTF (~300KB, SIL Open Font License).
const MONTSERRAT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Montserrat-Regular.ttf");

/// Embedded Montserrat Bold TTF (~300KB, SIL Open Font License).
const MONTSERRAT_BOLD: &[u8] = include_bytes!("../../assets/fonts/Montserrat-Bold.ttf");

/// Write embedded font bytes to a temp file and install it as a private font resource.
/// Returns the temp file path on success so it can be uninstalled later.
#[cfg(windows)]
fn install_font(data: &[u8], filename: &str) -> Option<String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::io::Write;

    let temp_dir = std::env::temp_dir();
    let path = temp_dir.join(filename);
    let path_str = path.to_string_lossy().to_string();

    // Write TTF to temp file
    match std::fs::File::create(&path) {
        Ok(mut f) => {
            if let Err(e) = f.write_all(data) {
                tracing::warn!(target: LOG_TARGET, "Failed to write font {}: {}", filename, e);
                return None;
            }
        }
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to create font file {}: {}", filename, e);
            return None;
        }
    }

    // AddFontResourceExW with FR_PRIVATE (0x10) — only visible to this process
    let wide_path: Vec<u16> = OsStr::new(&path_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let added = unsafe {
        winapi::um::wingdi::AddFontResourceExW(wide_path.as_ptr(), 0x10, std::ptr::null_mut())
    };

    if added > 0 {
        tracing::info!(target: LOG_TARGET, "Installed font {} ({} resources)", filename, added);
        Some(path_str)
    } else {
        tracing::warn!(target: LOG_TARGET, "AddFontResourceExW failed for {} — will use system font fallback", filename);
        // Clean up the temp file since we failed
        let _ = std::fs::remove_file(&path);
        None
    }
}

/// Temp file paths for installed fonts, used by uninstall.
#[cfg(windows)]
static FONT_PATHS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Install both embedded Montserrat fonts as private font resources.
/// Returns true if at least one font was installed successfully.
/// Must be called from the window thread (or before CreateFont calls).
#[cfg(windows)]
pub fn install_embedded_fonts() -> bool {
    let mut installed = 0;
    let mut paths = Vec::new();

    if let Some(p) = install_font(MONTSERRAT_REGULAR, "rp-montserrat-regular.ttf") {
        paths.push(p);
        installed += 1;
    }
    if let Some(p) = install_font(MONTSERRAT_BOLD, "rp-montserrat-bold.ttf") {
        paths.push(p);
        installed += 1;
    }

    if let Ok(mut stored) = FONT_PATHS.lock() {
        *stored = paths;
    }

    tracing::info!(target: LOG_TARGET, "Installed {}/2 Montserrat fonts", installed);
    installed > 0
}

/// Uninstall embedded fonts and delete temp files.
#[cfg(windows)]
pub fn uninstall_embedded_fonts() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let paths = match FONT_PATHS.lock() {
        Ok(mut p) => std::mem::take(&mut *p),
        Err(e) => std::mem::take(&mut *e.into_inner()),
    };

    for path_str in &paths {
        let wide_path: Vec<u16> = OsStr::new(path_str)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            winapi::um::wingdi::RemoveFontResourceExW(wide_path.as_ptr(), 0x10, std::ptr::null_mut());
        }
        let _ = std::fs::remove_file(path_str);
        tracing::info!(target: LOG_TARGET, "Uninstalled font: {}", path_str);
    }
}

#[cfg(not(windows))]
pub fn install_embedded_fonts() -> bool { false }

#[cfg(not(windows))]
pub fn uninstall_embedded_fonts() {}

// ---- GDI Resource Cache ----

/// Cached GDI handles for lock screen rendering.
/// Created once on the window thread, freed on Drop.
/// Follows the exact pattern from overlay.rs GdiResources.
#[cfg(windows)]
pub struct LockGdiResources {
    /// Montserrat Bold 48pt — "RACING POINT" header
    pub font_title: HFONT,
    /// Montserrat Bold 36pt — state headings ("Connecting...")
    pub font_heading: HFONT,
    /// Montserrat Regular 24pt — body text, driver names
    pub font_body: HFONT,
    /// Montserrat Bold 72pt — PIN dot display
    pub font_pin: HFONT,
    /// Montserrat Bold 96pt — countdown timer
    pub font_timer: HFONT,
    /// Montserrat Regular 16pt — labels, help text
    pub font_small: HFONT,
    /// RGB(0, 0, 0) — ScreenBlanked background
    pub brush_black: HBRUSH,
    /// RGB(26, 26, 26) = #1A1A1A — standard background (Asphalt Black)
    pub brush_asphalt: HBRUSH,
    /// RGB(225, 6, 0) = #E10600 — Racing Red
    pub brush_red: HBRUSH,
    /// RGB(34, 34, 34) = #222222 — card backgrounds
    pub brush_card: HBRUSH,
    /// RGB(90, 90, 90) = #5A5A5A — Gunmetal Grey
    pub brush_grey: HBRUSH,
    /// Embedded Racing Point logo as a GDI DIB section (HBITMAP).
    /// `None` if logo failed to decode (fallback: text-only branding).
    pub logo_bitmap: Option<HBITMAP>,
    /// Logo width in pixels (0 if no logo).
    pub logo_w: i32,
    /// Logo height in pixels (0 if no logo).
    pub logo_h: i32,
}

#[cfg(windows)]
impl LockGdiResources {
    /// Create all cached GDI handles. Must be called from the window thread.
    /// Uses "Montserrat" face name — if fonts weren't installed, GDI silently
    /// falls back to the closest matching system font (Segoe UI).
    pub unsafe fn new() -> Self {
        use winapi::um::wingdi::CreateSolidBrush;

        fn rgb(r: u8, g: u8, b: u8) -> u32 {
            (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
        }

        let null_hdc = std::ptr::null_mut();
        let mut res = Self {
            font_title: create_font(null_hdc, "Montserrat", 48, true),
            font_heading: create_font(null_hdc, "Montserrat", 36, true),
            font_body: create_font(null_hdc, "Montserrat", 24, false),
            font_pin: create_font(null_hdc, "Montserrat", 72, true),
            font_timer: create_font(null_hdc, "Montserrat", 96, true),
            font_small: create_font(null_hdc, "Montserrat", 16, false),
            brush_black: CreateSolidBrush(rgb(0, 0, 0)),
            brush_asphalt: CreateSolidBrush(rgb(26, 26, 26)),
            brush_red: CreateSolidBrush(rgb(225, 6, 0)),
            brush_card: CreateSolidBrush(rgb(34, 34, 34)),
            brush_grey: CreateSolidBrush(rgb(90, 90, 90)),
            logo_bitmap: None,
            logo_w: 0,
            logo_h: 0,
        };
        // Load embedded logo PNG into a GDI DIB section
        match load_logo_bitmap() {
            Some((hbmp, w, h)) => {
                tracing::info!(target: LOG_TARGET, "Logo loaded: {}x{}", w, h);
                res.logo_bitmap = Some(hbmp);
                res.logo_w = w;
                res.logo_h = h;
            }
            None => {
                tracing::warn!(target: LOG_TARGET, "Failed to load logo — blanking will use text-only branding");
            }
        }
        res
    }
}

#[cfg(windows)]
impl Drop for LockGdiResources {
    fn drop(&mut self) {
        unsafe {
            use winapi::um::wingdi::DeleteObject;
            DeleteObject(self.font_title as *mut _);
            DeleteObject(self.font_heading as *mut _);
            DeleteObject(self.font_body as *mut _);
            DeleteObject(self.font_pin as *mut _);
            DeleteObject(self.font_timer as *mut _);
            DeleteObject(self.font_small as *mut _);
            DeleteObject(self.brush_black as *mut _);
            DeleteObject(self.brush_asphalt as *mut _);
            DeleteObject(self.brush_red as *mut _);
            DeleteObject(self.brush_card as *mut _);
            DeleteObject(self.brush_grey as *mut _);
            if let Some(bmp) = self.logo_bitmap {
                DeleteObject(bmp as *mut _);
            }
        }
    }
}

/// Create a GDI font handle, matching the overlay.rs pattern.
/// Size is in pixels (negative = character height for DPI-correct sizing).
#[cfg(windows)]
unsafe fn create_font(
    _hdc: winapi::shared::windef::HDC,
    face: &str,
    size: i32,
    bold: bool,
) -> HFONT {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use winapi::um::wingdi::CreateFontW;

    let mut face_wide = [0u16; 32];
    for (i, c) in OsStr::new(face).encode_wide().enumerate() {
        if i >= 31 { break; }
        face_wide[i] = c;
    }

    CreateFontW(
        -size,                              // height (negative = character height)
        0,                                  // width (auto)
        0,                                  // escapement
        0,                                  // orientation
        if bold { 700 } else { 400 },       // weight
        0,                                  // italic
        0,                                  // underline
        0,                                  // strikeout
        1,                                  // charset (DEFAULT_CHARSET)
        0,                                  // out precision
        0,                                  // clip precision
        5,                                  // quality (CLEARTYPE_QUALITY)
        0,                                  // pitch and family
        face_wide.as_ptr(),
    )
}

/// Decode the embedded PNG logo and create a GDI DIB section.
/// Returns (HBITMAP, width, height) or None on failure.
#[cfg(windows)]
fn load_logo_bitmap() -> Option<(HBITMAP, i32, i32)> {
    use winapi::um::wingdi::{CreateDIBSection, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};

    // Decode PNG from embedded bytes
    let img = match image::load_from_memory(LOGO_PNG) {
        Ok(img) => img.to_rgba8(),
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "Failed to decode logo PNG: {}", e);
            return None;
        }
    };

    let w = img.width() as i32;
    let h = img.height() as i32;

    // DIB sections are bottom-up by default (negative height = top-down)
    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
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
        },
        bmiColors: [unsafe { std::mem::zeroed() }],
    };

    let mut bits_ptr: *mut winapi::ctypes::c_void = std::ptr::null_mut();
    let hbmp = unsafe {
        CreateDIBSection(
            std::ptr::null_mut(), // screen DC
            &bmi,
            DIB_RGB_COLORS,
            &mut bits_ptr as *mut *mut winapi::ctypes::c_void,
            std::ptr::null_mut(), // no file mapping
            0,
        )
    };

    if hbmp.is_null() || bits_ptr.is_null() {
        tracing::warn!(target: LOG_TARGET, "CreateDIBSection failed for logo");
        return None;
    }

    // Copy RGBA pixels into the DIB section (GDI expects BGRA pre-multiplied alpha)
    let pixel_count = (w * h) as usize;
    let dst: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u8, pixel_count * 4) };
    let src = img.as_raw();

    for i in 0..pixel_count {
        let si = i * 4;
        let r = src[si];
        let g = src[si + 1];
        let b = src[si + 2];
        let a = src[si + 3];
        // GDI AlphaBlend expects pre-multiplied alpha BGRA
        dst[si] = (b as u16 * a as u16 / 255) as u8;
        dst[si + 1] = (g as u16 * a as u16 / 255) as u8;
        dst[si + 2] = (r as u16 * a as u16 / 255) as u8;
        dst[si + 3] = a;
    }

    tracing::info!(target: LOG_TARGET, "Logo DIB created: {}x{} ({} bytes)", w, h, pixel_count * 4);
    Some((hbmp, w, h))
}

// Non-windows stubs for compilation
#[cfg(not(windows))]
pub struct LockGdiResources;

#[cfg(not(windows))]
impl LockGdiResources {
    pub unsafe fn new() -> Self { Self }
}
