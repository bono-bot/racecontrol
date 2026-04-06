//! Screen blanking verification via Windows GDI pixel sampling.
//!
//! MON-05: After rc-agent restart, verify the blanking screen (#1A1A1A) is
//! actually displayed by sampling 9 points on the virtual screen.
//! Uses GetPixel() -- synchronous, ~1us per call, no bitmap allocation.
//!
//! Safe to call from Session 1 context (rc-sentry always runs in Session 1).
//! Do NOT call from a polling loop -- cap at once per restart cycle.

#[allow(dead_code)]
const LOG_TARGET: &str = "screen-verify";

// Blanking color: #1A1A1A = Asphalt Black (brand identity)
const EXPECTED_R: u8 = 26;
const EXPECTED_G: u8 = 26;
const EXPECTED_B: u8 = 26;
const TOLERANCE: u8 = 10;
const BLANKED_THRESHOLD_PCT: usize = 80; // 80% of points must match

/// CLR_INVALID is returned by GetPixel when the point is outside the DC bounds.
const CLR_INVALID: u32 = 0xFFFF_FFFF;

/// 9 sample points on a 1920x1080 screen.
/// Distributed in a 3x3 grid avoiding edges (taskbar, window chrome).
const SAMPLE_POINTS: &[(i32, i32)] = &[
    (100, 100),  (960, 100),  (1820, 100),   // top row
    (100, 540),  (960, 540),  (1820, 540),   // middle row
    (100, 980),  (960, 980),  (1820, 980),   // bottom row
];

#[derive(Debug, Clone, PartialEq)]
pub enum BlankingStatus {
    /// Screen appears blanked -- >= 80% of sample points match expected color.
    Blanked { matching_points: usize, total_points: usize },
    /// Screen is NOT blanked -- < 80% of sample points match.
    NotBlanked { matching_points: usize, total_points: usize },
    /// Could not determine (GDI failure, wrong session, etc.)
    Unknown(String),
}

/// Check if a pixel color (COLORREF format: 0x00BBGGRR) matches the expected
/// blanking color within tolerance.
fn color_matches(colorref: u32) -> bool {
    let r = (colorref & 0xFF) as u8;
    let g = ((colorref >> 8) & 0xFF) as u8;
    let b = ((colorref >> 16) & 0xFF) as u8;

    let r_diff = if r > EXPECTED_R { r - EXPECTED_R } else { EXPECTED_R - r };
    let g_diff = if g > EXPECTED_G { g - EXPECTED_G } else { EXPECTED_G - g };
    let b_diff = if b > EXPECTED_B { b - EXPECTED_B } else { EXPECTED_B - b };

    r_diff <= TOLERANCE && g_diff <= TOLERANCE && b_diff <= TOLERANCE
}

/// Evaluate blanking status from a set of pixel results.
/// Each entry is (valid: bool, matches: bool).
/// Points that are invalid (out of bounds, CLR_INVALID) are excluded from totals.
fn evaluate_results(results: &[(bool, bool)]) -> BlankingStatus {
    let mut valid_total: usize = 0;
    let mut matching: usize = 0;

    for &(valid, matches) in results {
        if valid {
            valid_total += 1;
            if matches {
                matching += 1;
            }
        }
    }

    if valid_total == 0 {
        return BlankingStatus::Unknown("No valid sample points".to_string());
    }

    let pct = matching * 100 / valid_total;
    if pct >= BLANKED_THRESHOLD_PCT {
        BlankingStatus::Blanked { matching_points: matching, total_points: valid_total }
    } else {
        BlankingStatus::NotBlanked { matching_points: matching, total_points: valid_total }
    }
}

// ---- Production implementation (Windows GDI) ----
#[cfg(all(windows, not(test)))]
pub fn verify_blanking() -> BlankingStatus {
    use std::ptr;

    // RAII guard for the device context
    struct DcGuard {
        hdc: winapi::shared::windef::HDC,
    }

    impl Drop for DcGuard {
        fn drop(&mut self) {
            unsafe {
                winapi::um::winuser::ReleaseDC(ptr::null_mut(), self.hdc);
            }
        }
    }

    unsafe {
        // Get actual screen dimensions
        let screen_w = winapi::um::winuser::GetSystemMetrics(
            winapi::um::winuser::SM_CXVIRTUALSCREEN
        );
        let screen_h = winapi::um::winuser::GetSystemMetrics(
            winapi::um::winuser::SM_CYVIRTUALSCREEN
        );

        if screen_w <= 0 || screen_h <= 0 {
            return BlankingStatus::Unknown(
                format!("GetSystemMetrics returned invalid dimensions: {}x{}", screen_w, screen_h)
            );
        }

        // Get the virtual screen DC (NULL hwnd = entire virtual screen)
        let hdc = winapi::um::winuser::GetDC(ptr::null_mut());
        if hdc.is_null() {
            return BlankingStatus::Unknown("GetDC failed (null HDC)".to_string());
        }

        // RAII guard ensures ReleaseDC is called
        let _guard = DcGuard { hdc };

        let mut results = Vec::with_capacity(SAMPLE_POINTS.len());

        for &(x, y) in SAMPLE_POINTS {
            // Skip points outside screen bounds
            if x >= screen_w || y >= screen_h || x < 0 || y < 0 {
                results.push((false, false));
                continue;
            }

            let pixel = winapi::um::wingdi::GetPixel(hdc, x, y);
            if pixel == CLR_INVALID {
                results.push((false, false));
                continue;
            }

            results.push((true, color_matches(pixel)));
        }

        evaluate_results(&results)
    }
}

// ---- Non-Windows stub (for cross-compilation / CI) ----
#[cfg(all(not(windows), not(test)))]
pub fn verify_blanking() -> BlankingStatus {
    BlankingStatus::Unknown("Screen verification not available on non-Windows platforms".to_string())
}

// ---- Test implementation using thread-local mock ----
#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    /// Mock pixel data: Vec of (x, y, colorref). Use CLR_INVALID for invalid points.
    static MOCK_PIXELS: RefCell<Vec<(i32, i32, u32)>> = RefCell::new(Vec::new());
    /// Mock screen dimensions (width, height). Defaults to 1920x1080.
    static MOCK_SCREEN_SIZE: RefCell<(i32, i32)> = RefCell::new((1920, 1080));
}

#[cfg(test)]
fn set_mock_pixels(pixels: Vec<(i32, i32, u32)>) {
    MOCK_PIXELS.with(|p| {
        *p.borrow_mut() = pixels;
    });
}

#[cfg(test)]
pub fn verify_blanking() -> BlankingStatus {
    let screen_size = MOCK_SCREEN_SIZE.with(|s| *s.borrow());
    let (screen_w, screen_h) = screen_size;

    if screen_w <= 0 || screen_h <= 0 {
        return BlankingStatus::Unknown(
            format!("GetSystemMetrics returned invalid dimensions: {}x{}", screen_w, screen_h)
        );
    }

    let mock_data = MOCK_PIXELS.with(|p| p.borrow().clone());

    let mut results = Vec::with_capacity(SAMPLE_POINTS.len());

    for &(x, y) in SAMPLE_POINTS {
        // Skip points outside screen bounds
        if x >= screen_w || y >= screen_h || x < 0 || y < 0 {
            results.push((false, false));
            continue;
        }

        // Look up mock pixel value
        let pixel = mock_data.iter()
            .find(|&&(mx, my, _)| mx == x && my == y)
            .map(|&(_, _, c)| c);

        match pixel {
            Some(c) if c == CLR_INVALID => {
                results.push((false, false));
            }
            Some(c) => {
                results.push((true, color_matches(c)));
            }
            None => {
                // No mock data for this point -- treat as invalid
                results.push((false, false));
            }
        }
    }

    evaluate_results(&results)
}

// ---- Unit Tests ----
#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create COLORREF from RGB (COLORREF = 0x00BBGGRR)
    fn rgb(r: u8, g: u8, b: u8) -> u32 {
        (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
    }

    /// Helper: set all 9 sample points to the given color
    fn set_all_points(color: u32) {
        let pixels: Vec<(i32, i32, u32)> = SAMPLE_POINTS.iter()
            .map(|&(x, y)| (x, y, color))
            .collect();
        set_mock_pixels(pixels);
    }

    /// Helper: set N of 9 sample points to match, rest to mismatch
    fn set_n_matching(n: usize, match_color: u32, mismatch_color: u32) {
        let pixels: Vec<(i32, i32, u32)> = SAMPLE_POINTS.iter()
            .enumerate()
            .map(|(i, &(x, y))| {
                if i < n {
                    (x, y, match_color)
                } else {
                    (x, y, mismatch_color)
                }
            })
            .collect();
        set_mock_pixels(pixels);
    }

    #[test]
    fn test_blanking_all_match() {
        set_all_points(rgb(26, 26, 26));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::Blanked { matching_points: 9, total_points: 9 });
    }

    #[test]
    fn test_blanking_none_match() {
        set_all_points(rgb(255, 255, 255));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::NotBlanked { matching_points: 0, total_points: 9 });
    }

    #[test]
    fn test_blanking_below_threshold() {
        // 7 of 9 = 77.7% < 80%
        set_n_matching(7, rgb(26, 26, 26), rgb(255, 255, 255));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::NotBlanked { matching_points: 7, total_points: 9 });
    }

    #[test]
    fn test_blanking_at_threshold() {
        // 8 of 9 = 88.8% >= 80%
        set_n_matching(8, rgb(26, 26, 26), rgb(255, 255, 255));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::Blanked { matching_points: 8, total_points: 9 });
    }

    #[test]
    fn test_blanking_tolerance_boundary() {
        // RGB(36,36,36) = exactly +10 from expected (26) -- should match
        set_all_points(rgb(36, 36, 36));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::Blanked { matching_points: 9, total_points: 9 });

        // RGB(37,37,37) = +11 from expected -- should NOT match
        set_all_points(rgb(37, 37, 37));
        let result = verify_blanking();
        assert_eq!(result, BlankingStatus::NotBlanked { matching_points: 0, total_points: 9 });
    }

    #[test]
    fn test_blanking_invalid_pixel_skipped() {
        // Set 8 points to matching color, 1 point to CLR_INVALID
        let mut pixels: Vec<(i32, i32, u32)> = SAMPLE_POINTS.iter()
            .map(|&(x, y)| (x, y, rgb(26, 26, 26)))
            .collect();
        // Replace last point with CLR_INVALID
        if let Some(last) = pixels.last_mut() {
            last.2 = CLR_INVALID;
        }
        set_mock_pixels(pixels);

        let result = verify_blanking();
        // 8 valid points, all matching. CLR_INVALID excluded from total.
        assert_eq!(result, BlankingStatus::Blanked { matching_points: 8, total_points: 8 });
    }

    #[test]
    fn test_color_matches_exact() {
        assert!(color_matches(rgb(26, 26, 26)));
    }

    #[test]
    fn test_color_matches_within_tolerance() {
        assert!(color_matches(rgb(16, 26, 36))); // -10, 0, +10
        assert!(color_matches(rgb(36, 36, 36))); // +10, +10, +10
    }

    #[test]
    fn test_color_no_match_outside_tolerance() {
        assert!(!color_matches(rgb(37, 26, 26))); // R is +11
        assert!(!color_matches(rgb(26, 15, 26))); // G is -11
        assert!(!color_matches(rgb(26, 26, 37))); // B is +11
    }
}
