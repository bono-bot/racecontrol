use image::RgbImage;
use imageproc::geometric_transformations::{self, Interpolation, Projection};

/// ArcFace reference landmarks for 112x112 aligned face.
/// Order: left_eye, right_eye, nose, left_mouth, right_mouth
pub const ARCFACE_REF: [[f32; 2]; 5] = [
    [38.2946, 51.6963],
    [73.5318, 51.5014],
    [56.0252, 71.7366],
    [41.5493, 92.3655],
    [70.7299, 92.2041],
];

/// Estimate a 2D similarity transform from `src` landmarks to `ARCFACE_REF`.
///
/// Returns affine parameters `[a, -b, tx, b, a, ty]` such that:
///   dst_x = a * src_x - b * src_y + tx
///   dst_y = b * src_x + a * src_y + ty
///
/// Uses least-squares over all 5 point pairs (10 equations, 4 unknowns).
pub fn estimate_similarity_transform(src: &[[f32; 2]; 5]) -> [f32; 6] {
    let dst = &ARCFACE_REF;

    // Build normal equations: A^T*A (4x4) and A^T*b (4x1)
    // Variables: [a, b, tx, ty]
    // For each point pair:
    //   x-equation row: [src_x, -src_y, 1, 0]  rhs = dst_x
    //   y-equation row: [src_y,  src_x, 0, 1]  rhs = dst_y
    let mut ata = [[0.0_f64; 4]; 4];
    let mut atb = [0.0_f64; 4];

    for i in 0..5 {
        let sx = src[i][0] as f64;
        let sy = src[i][1] as f64;
        let dx = dst[i][0] as f64;
        let dy = dst[i][1] as f64;

        let row_x: [f64; 4] = [sx, -sy, 1.0, 0.0];
        let row_y: [f64; 4] = [sy, sx, 0.0, 1.0];

        for r in 0..4 {
            for c in 0..4 {
                ata[r][c] += row_x[r] * row_x[c];
                ata[r][c] += row_y[r] * row_y[c];
            }
            atb[r] += row_x[r] * dx;
            atb[r] += row_y[r] * dy;
        }
    }

    let params = solve_4x4(&ata, &atb);
    let a = params[0] as f32;
    let b = params[1] as f32;
    let tx = params[2] as f32;
    let ty = params[3] as f32;

    [a, -b, tx, b, a, ty]
}

/// Solve a 4x4 linear system using Gaussian elimination with partial pivoting.
fn solve_4x4(ata: &[[f64; 4]; 4], atb: &[f64; 4]) -> [f64; 4] {
    let mut aug = [[0.0_f64; 5]; 4];
    for i in 0..4 {
        for j in 0..4 {
            aug[i][j] = ata[i][j];
        }
        aug[i][4] = atb[i];
    }

    // Forward elimination with partial pivoting
    for col in 0..4 {
        let mut max_row = col;
        let mut max_val = aug[col][col].abs();
        for row in (col + 1)..4 {
            let val = aug[row][col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }

        if max_row != col {
            aug.swap(col, max_row);
        }

        let pivot = aug[col][col];
        if pivot.abs() < 1e-12 {
            return [0.0; 4];
        }

        for row in (col + 1)..4 {
            let factor = aug[row][col] / pivot;
            for j in col..5 {
                aug[row][j] -= factor * aug[col][j];
            }
        }
    }

    // Back substitution
    let mut result = [0.0_f64; 4];
    for i in (0..4).rev() {
        let mut sum = aug[i][4];
        for j in (i + 1)..4 {
            sum -= aug[i][j] * result[j];
        }
        result[i] = sum / aug[i][i];
    }

    result
}

/// Align a face from a full RGB frame to a 112x112 crop using 5-point landmarks.
///
/// 1. Computes similarity transform from landmarks to ArcFace reference points.
/// 2. Builds inverse projection (destination-to-source) for `imageproc::warp`.
/// 3. Warps the full frame and crops to 112x112.
pub fn align_face(
    frame_rgb: &[u8],
    frame_w: u32,
    frame_h: u32,
    landmarks: &[[f32; 2]; 5],
) -> RgbImage {
    let params = estimate_similarity_transform(landmarks);
    let a = params[0] as f64;
    let b = params[3] as f64; // b from [a, -b, tx, b, a, ty]
    let tx = params[2] as f64;
    let ty = params[5] as f64;

    // Forward transform matrix M:
    //   [[a, -b, tx],
    //    [b,  a, ty],
    //    [0,  0,  1]]
    //
    // Projection needs INVERSE (maps output pixel to input pixel).
    // For similarity transform, inverse is:
    //   (1/det) * [[a, b, -(a*tx + b*ty)],
    //              [-b, a, (b*tx - a*ty)],
    //              [0,  0,      det      ]]
    // where det = a^2 + b^2

    let det = a * a + b * b;
    if det.abs() < 1e-12 {
        return RgbImage::new(112, 112);
    }

    let inv_a = a / det;
    let inv_b = b / det;
    let inv_tx = -(a * tx + b * ty) / det;
    let inv_ty = (b * tx - a * ty) / det;

    // Projection::from_matrix takes row-major [f32; 9]
    // Maps (dst_x, dst_y) -> (src_x, src_y)
    let proj_matrix: [f32; 9] = [
        inv_a as f32,
        inv_b as f32,
        inv_tx as f32,
        -inv_b as f32,
        inv_a as f32,
        inv_ty as f32,
        0.0,
        0.0,
        1.0,
    ];

    let projection =
        Projection::from_matrix(proj_matrix).expect("valid projection matrix");

    let frame_img = RgbImage::from_raw(frame_w, frame_h, frame_rgb.to_vec())
        .expect("RGB buffer size must match width * height * 3");

    let warped = geometric_transformations::warp(
        &frame_img,
        &projection,
        Interpolation::Bilinear,
        image::Rgb([0, 0, 0]),
    );

    // Crop to 112x112 from top-left corner
    image::imageops::crop_imm(&warped, 0, 0, 112, 112).to_image()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_transform() {
        // When source points == ARCFACE_REF, the transform should be near-identity
        let result = estimate_similarity_transform(&ARCFACE_REF);
        let a = result[0];
        let neg_b = result[1];
        let tx = result[2];
        let b = result[3];
        let ty = result[5];

        assert!((a - 1.0).abs() < 1e-4, "a should be ~1.0, got {a}");
        assert!(neg_b.abs() < 1e-4, "-b should be ~0.0, got {neg_b}");
        assert!(tx.abs() < 1e-4, "tx should be ~0.0, got {tx}");
        assert!(b.abs() < 1e-4, "b should be ~0.0, got {b}");
        assert!(ty.abs() < 1e-4, "ty should be ~0.0, got {ty}");
    }

    #[test]
    fn test_scaled_transform() {
        // When source points are 2x ARCFACE_REF, the transform should have scale ~0.5
        let scaled: [[f32; 2]; 5] = [
            [ARCFACE_REF[0][0] * 2.0, ARCFACE_REF[0][1] * 2.0],
            [ARCFACE_REF[1][0] * 2.0, ARCFACE_REF[1][1] * 2.0],
            [ARCFACE_REF[2][0] * 2.0, ARCFACE_REF[2][1] * 2.0],
            [ARCFACE_REF[3][0] * 2.0, ARCFACE_REF[3][1] * 2.0],
            [ARCFACE_REF[4][0] * 2.0, ARCFACE_REF[4][1] * 2.0],
        ];

        let result = estimate_similarity_transform(&scaled);
        let a = result[0];
        let b = result[3];

        let scale = (a * a + b * b).sqrt();
        assert!(
            (scale - 0.5).abs() < 1e-3,
            "scale should be ~0.5, got {scale}"
        );
    }

    #[test]
    fn test_align_face_output_size() {
        let img = RgbImage::from_fn(640, 480, |x, y| {
            image::Rgb([
                (x % 256) as u8,
                (y % 256) as u8,
                ((x + y) % 256) as u8,
            ])
        });

        // Scale ARCFACE_REF to place face within 640x480 image
        let scale = 3.0_f32;
        let offset_x = 100.0_f32;
        let offset_y = 50.0_f32;
        let landmarks: [[f32; 2]; 5] = [
            [ARCFACE_REF[0][0] * scale + offset_x, ARCFACE_REF[0][1] * scale + offset_y],
            [ARCFACE_REF[1][0] * scale + offset_x, ARCFACE_REF[1][1] * scale + offset_y],
            [ARCFACE_REF[2][0] * scale + offset_x, ARCFACE_REF[2][1] * scale + offset_y],
            [ARCFACE_REF[3][0] * scale + offset_x, ARCFACE_REF[3][1] * scale + offset_y],
            [ARCFACE_REF[4][0] * scale + offset_x, ARCFACE_REF[4][1] * scale + offset_y],
        ];

        let aligned = align_face(img.as_raw(), 640, 480, &landmarks);

        assert_eq!(aligned.width(), 112, "aligned width should be 112");
        assert_eq!(aligned.height(), 112, "aligned height should be 112");
    }

    #[test]
    fn test_solve_4x4_identity() {
        let ata = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let atb = [1.0, 2.0, 3.0, 4.0];

        let result = solve_4x4(&ata, &atb);

        assert!((result[0] - 1.0).abs() < 1e-10, "x[0] should be 1.0, got {}", result[0]);
        assert!((result[1] - 2.0).abs() < 1e-10, "x[1] should be 2.0, got {}", result[1]);
        assert!((result[2] - 3.0).abs() < 1e-10, "x[2] should be 3.0, got {}", result[2]);
        assert!((result[3] - 4.0).abs() < 1e-10, "x[3] should be 4.0, got {}", result[3]);
    }
}
