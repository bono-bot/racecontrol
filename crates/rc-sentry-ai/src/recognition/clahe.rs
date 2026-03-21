/// Apply CLAHE (Contrast Limited Adaptive Histogram Equalization) to a face crop.
///
/// Converts the input RGB image to grayscale, applies CLAHE for lighting normalization,
/// then replicates the grayscale result to 3 channels (R=G=B) for ArcFace input.
///
/// This ensures consistent face appearance under varying entrance lighting conditions.
pub fn apply_clahe(face_crop: &image::RgbImage) -> image::RgbImage {
    todo!("implement CLAHE normalization")
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};

    #[test]
    fn test_clahe_changes_dark_uniform_image() {
        // Dark uniform 80x80 image -- CLAHE should redistribute histogram
        let input = RgbImage::from_fn(80, 80, |_, _| Rgb([30, 30, 30]));
        let output = apply_clahe(&input);

        // Output should NOT be all the same as input (CLAHE redistributes)
        let all_same = output.pixels().all(|p| p[0] == 30 && p[1] == 30 && p[2] == 30);
        assert!(!all_same, "CLAHE should change pixel values of uniform dark image");
    }

    #[test]
    fn test_clahe_preserves_dimensions() {
        let input = RgbImage::from_fn(80, 80, |_, _| Rgb([30, 30, 30]));
        let output = apply_clahe(&input);
        assert_eq!(output.width(), 80);
        assert_eq!(output.height(), 80);
    }

    #[test]
    fn test_clahe_output_is_grayscale_as_rgb() {
        // Output should have R=G=B for every pixel (grayscale replicated to 3 channels)
        let input = RgbImage::from_fn(80, 80, |x, y| {
            let v = ((x * 3 + y * 7) % 256) as u8;
            Rgb([v, v.wrapping_add(10), v.wrapping_add(20)])
        });
        let output = apply_clahe(&input);

        for pixel in output.pixels() {
            assert_eq!(
                pixel[0], pixel[1],
                "R should equal G, got R={} G={}",
                pixel[0], pixel[1]
            );
            assert_eq!(
                pixel[1], pixel[2],
                "G should equal B, got G={} B={}",
                pixel[1], pixel[2]
            );
        }
    }
}
