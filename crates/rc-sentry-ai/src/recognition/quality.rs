use crate::detection::types::DetectedFace;
use crate::recognition::types::RejectReason;

/// Quality gate filter chain for face detections.
///
/// Rejects faces that are too small, too blurry, or at excessive yaw angles
/// before expensive ArcFace inference.
pub struct QualityGates {
    pub min_face_size: u32,
    pub min_laplacian_var: f64,
    pub max_yaw_degrees: f64,
}

impl Default for QualityGates {
    fn default() -> Self {
        Self {
            min_face_size: 80,
            min_laplacian_var: 100.0,
            max_yaw_degrees: 45.0,
        }
    }
}

impl QualityGates {
    /// Run all quality gates in sequence. Returns first failure.
    pub fn check(
        &self,
        face: &DetectedFace,
        frame_gray: &[u8],
        frame_w: u32,
        frame_h: u32,
    ) -> Result<(), RejectReason> {
        self.check_size(face)?;
        self.check_blur(face, frame_gray, frame_w, frame_h)?;
        self.check_pose(face)?;
        Ok(())
    }

    /// Reject faces with bounding box smaller than minimum size.
    fn check_size(&self, face: &DetectedFace) -> Result<(), RejectReason> {
        let width = (face.bbox[2] - face.bbox[0]) as u32;
        let height = (face.bbox[3] - face.bbox[1]) as u32;
        if width < self.min_face_size || height < self.min_face_size {
            return Err(RejectReason::TooSmall { width, height });
        }
        Ok(())
    }

    /// Reject faces with low Laplacian variance (blurry).
    fn check_blur(
        &self,
        face: &DetectedFace,
        frame_gray: &[u8],
        frame_w: u32,
        frame_h: u32,
    ) -> Result<(), RejectReason> {
        // Extract face crop coordinates, clamped to frame bounds
        let x1 = (face.bbox[0] as u32).min(frame_w.saturating_sub(1));
        let y1 = (face.bbox[1] as u32).min(frame_h.saturating_sub(1));
        let x2 = (face.bbox[2] as u32).min(frame_w);
        let y2 = (face.bbox[3] as u32).min(frame_h);

        let crop_w = x2.saturating_sub(x1);
        let crop_h = y2.saturating_sub(y1);

        if crop_w < 3 || crop_h < 3 {
            return Err(RejectReason::TooBlurry { laplacian_var: 0.0 });
        }

        // Extract face crop from grayscale frame
        let mut crop = Vec::with_capacity((crop_w * crop_h) as usize);
        for y in y1..y2 {
            let row_start = (y * frame_w + x1) as usize;
            let row_end = (y * frame_w + x2) as usize;
            crop.extend_from_slice(&frame_gray[row_start..row_end]);
        }

        let var = laplacian_variance(&crop, crop_w, crop_h);
        if var < self.min_laplacian_var {
            return Err(RejectReason::TooBlurry { laplacian_var: var });
        }
        Ok(())
    }

    /// Reject faces with excessive yaw angle (side profile).
    fn check_pose(&self, face: &DetectedFace) -> Result<(), RejectReason> {
        let yaw = estimate_yaw(&face.landmarks);
        if yaw > self.max_yaw_degrees {
            return Err(RejectReason::ExcessiveYaw { estimated_yaw: yaw });
        }
        Ok(())
    }
}

/// Compute Laplacian variance as a blur metric on a grayscale face crop.
///
/// The Laplacian highlights edges. High variance = sharp image. Low variance = blurry.
/// Threshold of 100.0 is standard for surveillance-quality cameras.
pub fn laplacian_variance(gray: &[u8], width: u32, height: u32) -> f64 {
    if width < 3 || height < 3 {
        return 0.0;
    }

    let w = width as usize;
    let h = height as usize;
    let mut sum = 0.0_f64;
    let mut sum_sq = 0.0_f64;
    let mut count = 0u64;

    // 3x3 Laplacian kernel: [0, 1, 0; 1, -4, 1; 0, 1, 0]
    for y in 1..(h - 1) {
        for x in 1..(w - 1) {
            let val = gray[y * w + x] as f64 * -4.0
                + gray[(y - 1) * w + x] as f64
                + gray[(y + 1) * w + x] as f64
                + gray[y * w + (x - 1)] as f64
                + gray[y * w + (x + 1)] as f64;
            sum += val;
            sum_sq += val * val;
            count += 1;
        }
    }

    if count == 0 {
        return 0.0;
    }

    let mean = sum / count as f64;
    (sum_sq / count as f64) - (mean * mean) // variance
}

/// Estimate yaw angle from 5-point facial landmarks.
///
/// Uses the ratio of left-eye-to-nose vs. nose-to-right-eye horizontal distances.
/// Returns estimated absolute yaw in degrees (0 = frontal, 90 = full profile).
pub fn estimate_yaw(landmarks: &[[f32; 2]; 5]) -> f64 {
    let left_eye = landmarks[0];
    let right_eye = landmarks[1];
    let nose = landmarks[2];

    let left_dist = (nose[0] - left_eye[0]).abs();
    let right_dist = (right_eye[0] - nose[0]).abs();

    if left_dist + right_dist < 1.0 {
        return 90.0; // degenerate case
    }

    // Ratio: 1.0 = frontal, 0.0 = full profile
    let ratio = left_dist.min(right_dist) / left_dist.max(right_dist);

    // Map ratio to approximate yaw angle
    // ratio ~1.0 -> 0 degrees, ratio ~0.0 -> 90 degrees
    ((1.0 - ratio) as f64) * 90.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detection::types::DetectedFace;

    fn make_face(x1: f32, y1: f32, x2: f32, y2: f32, landmarks: [[f32; 2]; 5]) -> DetectedFace {
        DetectedFace {
            bbox: [x1, y1, x2, y2],
            confidence: 0.99,
            landmarks,
        }
    }

    // Frontal face landmarks (symmetric eyes around nose)
    fn frontal_landmarks() -> [[f32; 2]; 5] {
        [
            [30.0, 40.0],  // left_eye
            [70.0, 40.0],  // right_eye
            [50.0, 55.0],  // nose
            [35.0, 70.0],  // left_mouth
            [65.0, 70.0],  // right_mouth
        ]
    }

    // Side profile landmarks (one eye very close to nose)
    fn side_profile_landmarks() -> [[f32; 2]; 5] {
        [
            [48.0, 40.0],  // left_eye (compressed toward nose)
            [70.0, 40.0],  // right_eye
            [50.0, 55.0],  // nose
            [48.0, 70.0],  // left_mouth
            [65.0, 70.0],  // right_mouth
        ]
    }

    #[test]
    fn test_size_reject_too_small() {
        let gates = QualityGates::default();
        // 60x60 face -- below 80x80 minimum
        let face = make_face(10.0, 10.0, 70.0, 70.0, frontal_landmarks());
        let gray = vec![128u8; 200 * 200];
        let result = gates.check(&face, &gray, 200, 200);
        assert!(result.is_err());
        match result.unwrap_err() {
            RejectReason::TooSmall { width, height } => {
                assert_eq!(width, 60);
                assert_eq!(height, 60);
            }
            other => panic!("expected TooSmall, got {:?}", other),
        }
    }

    #[test]
    fn test_size_accept_large_enough() {
        let gates = QualityGates::default();
        // 100x100 face -- above 80x80 minimum
        let face = make_face(10.0, 10.0, 110.0, 110.0, frontal_landmarks());
        // Need a sharp enough image to pass blur check too
        let w = 200u32;
        let h = 200u32;
        let mut gray = vec![128u8; (w * h) as usize];
        // Create checkerboard pattern for high Laplacian variance
        for y in 0..h {
            for x in 0..w {
                gray[(y * w + x) as usize] = if (x + y) % 2 == 0 { 0 } else { 255 };
            }
        }
        let result = gates.check(&face, &gray, w, h);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn test_blur_reject_uniform() {
        // Uniform gray image has Laplacian variance ~0.0
        let var = laplacian_variance(&vec![128u8; 100 * 100], 100, 100);
        assert!(var < 1.0, "uniform image should have near-zero Laplacian variance, got {}", var);
    }

    #[test]
    fn test_blur_accept_checkerboard() {
        // High-contrast checkerboard should have high Laplacian variance
        let w = 100u32;
        let h = 100u32;
        let mut gray = vec![0u8; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                gray[(y * w + x) as usize] = if (x + y) % 2 == 0 { 0 } else { 255 };
            }
        }
        let var = laplacian_variance(&gray, w, h);
        assert!(var > 100.0, "checkerboard should have high Laplacian variance, got {}", var);
    }

    #[test]
    fn test_pose_accept_frontal() {
        let yaw = estimate_yaw(&frontal_landmarks());
        assert!(yaw < 45.0, "frontal face should have low yaw, got {}", yaw);
    }

    #[test]
    fn test_pose_reject_side_profile() {
        let yaw = estimate_yaw(&side_profile_landmarks());
        assert!(yaw > 45.0, "side profile should have high yaw, got {}", yaw);
    }

    #[test]
    fn test_check_runs_all_gates_returns_first_failure() {
        let gates = QualityGates::default();
        // Small face (should fail on size first, never reach blur/pose)
        let face = make_face(10.0, 10.0, 50.0, 50.0, frontal_landmarks());
        let gray = vec![128u8; 200 * 200];
        let result = gates.check(&face, &gray, 200, 200);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RejectReason::TooSmall { .. }));
    }
}
