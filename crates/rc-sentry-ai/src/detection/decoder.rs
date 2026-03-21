use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

/// Decoded RGB frame from H.264 NAL unit(s).
pub struct DecodedFrame {
    pub rgb: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

/// Stateful H.264 decoder wrapping openh264.
///
/// CRITICAL: One `FrameDecoder` per camera stream -- H.264 is stateful
/// (P-frames depend on previous frames). Creating a new decoder per frame
/// will only decode I-frames correctly.
pub struct FrameDecoder {
    decoder: Decoder,
}

impl FrameDecoder {
    pub fn new() -> anyhow::Result<Self> {
        let decoder = Decoder::new()?;
        Ok(Self { decoder })
    }

    /// Decode H.264 NAL unit(s) to RGB pixels.
    /// Returns `None` if frame cannot be decoded (e.g., waiting for keyframe).
    pub fn decode(&mut self, nal_data: &[u8]) -> anyhow::Result<Option<DecodedFrame>> {
        let yuv = match self.decoder.decode(nal_data)? {
            Some(yuv) => yuv,
            None => return Ok(None),
        };
        let (width, height) = yuv.dimensions();
        let mut rgb = vec![0u8; width * height * 3];
        yuv.write_rgb8(&mut rgb);
        Ok(Some(DecodedFrame {
            rgb,
            width: width as u32,
            height: height as u32,
        }))
    }
}
