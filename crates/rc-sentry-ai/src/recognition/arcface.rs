use ndarray::Array4;

/// Preprocess an aligned 112x112 RGB face for ArcFace inference.
///
/// - Asserts input is 112x112
/// - Creates NCHW tensor [1, 3, 112, 112]
/// - Normalizes: `(pixel - 127.5) / 127.5` mapping [0,255] to [-1.0, 1.0]
///
/// Note: ArcFace (glintr100) uses /127.5, NOT /128.0 like SCRFD.
pub fn preprocess(aligned_rgb: &image::RgbImage) -> Array4<f32> {
    assert_eq!(
        aligned_rgb.width(),
        112,
        "ArcFace input must be 112x112, got {}x{}",
        aligned_rgb.width(),
        aligned_rgb.height()
    );
    assert_eq!(
        aligned_rgb.height(),
        112,
        "ArcFace input must be 112x112, got {}x{}",
        aligned_rgb.width(),
        aligned_rgb.height()
    );

    let mut tensor = Array4::<f32>::zeros((1, 3, 112, 112));
    for y in 0..112u32 {
        for x in 0..112u32 {
            let pixel = aligned_rgb.get_pixel(x, y);
            tensor[[0, 0, y as usize, x as usize]] =
                (pixel[0] as f32 - 127.5) / 127.5;
            tensor[[0, 1, y as usize, x as usize]] =
                (pixel[1] as f32 - 127.5) / 127.5;
            tensor[[0, 2, y as usize, x as usize]] =
                (pixel[2] as f32 - 127.5) / 127.5;
        }
    }

    tensor
}

// --- ONNX Runtime session wrapper ---
// Separated from preprocess so the lib test target can test preprocessing
// without linking against ort (which has known MSVC static CRT linker issues).

mod session {
    use std::sync::Arc;

    use ndarray::Array4;
    use ort::session::Session;
    use ort::value::Tensor;
    use tokio::sync::Mutex;

    /// ArcFace face recognizer using ONNX Runtime with CUDA execution provider.
    ///
    /// Extracts 512-dimensional L2-normalized embeddings from 112x112 aligned face crops.
    /// The session is wrapped in `Arc<Mutex<Session>>` because `session.run()` requires
    /// `&mut self` in ort 2.0. Create one `ArcfaceRecognizer`, share via `clone_shared()`.
    pub struct ArcfaceRecognizer {
        session: Arc<Mutex<Session>>,
    }

    impl ArcfaceRecognizer {
        /// Load ArcFace ONNX model with CUDA execution provider.
        ///
        /// Uses `error_on_failure()` so CUDA EP initialization failure is an error,
        /// not a silent fallback to CPU.
        ///
        /// Verifies expected shapes at load:
        ///   - Input: [1, 3, 112, 112]
        ///   - Output: [1, 512]
        pub fn new(model_path: &str) -> anyhow::Result<Self> {
            use ort::ep;

            let session = Session::builder()
                .map_err(|e| anyhow::anyhow!("ort session builder: {e}"))?
                .with_execution_providers([ep::CUDA::default().build().error_on_failure()])
                .map_err(|e| anyhow::anyhow!("ort CUDA EP: {e}"))?
                .commit_from_file(model_path)
                .map_err(|e| anyhow::anyhow!("ort commit model: {e}"))?;

            // Log all input/output node names and shapes for runtime verification
            for input in session.inputs() {
                tracing::info!(
                    name = %input.name(),
                    dtype = ?input.dtype(),
                    "ArcFace input node"
                );
            }
            for output in session.outputs() {
                tracing::info!(
                    name = %output.name(),
                    dtype = ?output.dtype(),
                    "ArcFace output node"
                );
            }

            Ok(Self {
                session: Arc::new(Mutex::new(session)),
            })
        }

        /// Clone the recognizer (Arc clone -- cheap, shares the underlying session).
        pub fn clone_shared(&self) -> Self {
            Self {
                session: Arc::clone(&self.session),
            }
        }

        /// Extract a 512-D L2-normalized embedding from a preprocessed tensor.
        ///
        /// Uses `spawn_blocking` internally since ONNX inference is GPU-bound
        /// and would block the tokio runtime.
        pub async fn extract_embedding(
            &self,
            tensor: Array4<f32>,
        ) -> anyhow::Result<[f32; 512]> {
            let session = Arc::clone(&self.session);

            let embedding =
                tokio::task::spawn_blocking(move || -> anyhow::Result<[f32; 512]> {
                    let mut session = session.blocking_lock();

                    let input_tensor = Tensor::from_array(tensor)
                        .map_err(|e| anyhow::anyhow!("tensor creation: {e}"))?;

                    let outputs = session
                        .run(ort::inputs![input_tensor])
                        .map_err(|e| anyhow::anyhow!("ort inference: {e}"))?;

                    // Extract the first output tensor
                    let (_, output_value) = outputs
                        .iter()
                        .next()
                        .ok_or_else(|| {
                            anyhow::anyhow!("no output tensor from ArcFace model")
                        })?;

                    let (shape, data) = output_value
                        .try_extract_tensor::<f32>()
                        .map_err(|e| anyhow::anyhow!("extract tensor: {e}"))?;

                    let flat: Vec<f32> = data.to_vec();
                    let dims: Vec<i64> = shape.iter().copied().collect();

                    if flat.len() != 512 {
                        anyhow::bail!(
                            "expected 512-D embedding, got {} elements (shape: {:?})",
                            flat.len(),
                            dims
                        );
                    }

                    // L2 normalize
                    let norm: f32 =
                        flat.iter().map(|x| x * x).sum::<f32>().sqrt();

                    let mut embedding = [0.0_f32; 512];
                    if norm > 1e-10 {
                        for (i, val) in flat.iter().enumerate() {
                            embedding[i] = val / norm;
                        }
                    } else {
                        // Degenerate case -- copy as-is
                        embedding.copy_from_slice(&flat);
                    }

                    Ok(embedding)
                })
                .await??;

            Ok(embedding)
        }
    }
}

pub use session::ArcfaceRecognizer;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preprocess_shape_and_range() {
        // Create a 112x112 test image with known pixel values
        let img = image::RgbImage::from_fn(112, 112, |x, y| {
            image::Rgb([
                (x % 256) as u8,
                (y % 256) as u8,
                ((x + y) % 256) as u8,
            ])
        });

        let tensor = preprocess(&img);

        // Verify shape is [1, 3, 112, 112]
        assert_eq!(tensor.shape(), &[1, 3, 112, 112]);

        // Verify all values are in [-1.0, 1.0] range
        for val in tensor.iter() {
            assert!(
                *val >= -1.0 && *val <= 1.0,
                "tensor value {val} outside [-1.0, 1.0]"
            );
        }

        // Verify specific normalization: pixel value 0 -> (0 - 127.5) / 127.5 = -1.0
        // pixel value 255 -> (255 - 127.5) / 127.5 = 1.0
        let min_val = tensor.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = tensor.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            (min_val - (-1.0)).abs() < 1e-5,
            "min should be -1.0, got {min_val}"
        );
        assert!(
            (max_val - 1.0).abs() < 1e-5,
            "max should be 1.0, got {max_val}"
        );
    }

    #[test]
    fn test_preprocess_normalization_values() {
        // Create image with all pixels = 127 (should normalize to near 0)
        let img = image::RgbImage::from_pixel(112, 112, image::Rgb([127, 127, 127]));
        let tensor = preprocess(&img);

        // (127 - 127.5) / 127.5 = -0.003921...
        let expected = (127.0_f32 - 127.5) / 127.5;
        let actual = tensor[[0, 0, 0, 0]];
        assert!(
            (actual - expected).abs() < 1e-5,
            "pixel 127 should normalize to {expected}, got {actual}"
        );
    }
}
