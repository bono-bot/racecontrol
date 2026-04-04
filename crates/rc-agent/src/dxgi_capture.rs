//! DXGI Desktop Duplication API screenshot capture.
//!
//! Captures the actual GPU framebuffer output, including Direct3D
//! exclusive fullscreen games that bypass GDI (CopyFromScreen).
//!
//! Used by the `/screenshot?method=dxgi` endpoint in remote_ops.

#[cfg(windows)]
use windows::{
    core::Interface,
    Win32::Foundation::*,
    Win32::Graphics::Direct3D::*,
    Win32::Graphics::Direct3D11::*,
    Win32::Graphics::Dxgi::*,
    Win32::Graphics::Dxgi::Common::*,
};

/// Raw BGRA pixel buffer from a DXGI frame capture.
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    /// BGRA8 pixels, row-major, stride may include padding.
    pub data: Vec<u8>,
    pub stride: u32,
}

/// Capture a single frame from the primary display using DXGI Desktop Duplication.
#[cfg(windows)]
pub fn capture_frame() -> Result<CapturedFrame, String> {
    unsafe {
        // 1. Create D3D11 device
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;

        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| format!("D3D11CreateDevice failed: {}", e))?;

        let device = device.ok_or("D3D11 device is None")?;
        let context = context.ok_or("D3D11 context is None")?;

        // 2. Get DXGI adapter and output
        let dxgi_device: IDXGIDevice = device
            .cast::<IDXGIDevice>()
            .map_err(|e| format!("Cast to IDXGIDevice failed: {}", e))?;

        let adapter: IDXGIAdapter = dxgi_device
            .GetAdapter()
            .map_err(|e| format!("GetAdapter failed: {}", e))?;

        let output: IDXGIOutput = adapter
            .EnumOutputs(0)
            .map_err(|e| format!("EnumOutputs(0) failed: {}", e))?;

        let output1: IDXGIOutput1 = output
            .cast::<IDXGIOutput1>()
            .map_err(|e| format!("Cast to IDXGIOutput1 failed: {}", e))?;

        // 3. Create desktop duplication
        let duplication: IDXGIOutputDuplication = output1
            .DuplicateOutput(&device)
            .map_err(|e| format!("DuplicateOutput failed: {}", e))?;

        // 4. Get output description for dimensions
        let desc = duplication.GetDesc();
        let width = desc.ModeDesc.Width;
        let height = desc.ModeDesc.Height;

        // 5. Acquire next frame (500ms timeout)
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;

        duplication
            .AcquireNextFrame(500, &mut frame_info, &mut resource)
            .map_err(|e| format!("AcquireNextFrame failed: {}", e))?;

        let resource = resource.ok_or("Frame resource is None")?;

        // 6. Get the texture from the frame
        let texture: ID3D11Texture2D = resource
            .cast::<ID3D11Texture2D>()
            .map_err(|e| format!("Cast to ID3D11Texture2D failed: {}", e))?;

        // 7. Create a CPU-readable staging texture
        let mut tex_desc = D3D11_TEXTURE2D_DESC::default();
        texture.GetDesc(&mut tex_desc);
        tex_desc.Usage = D3D11_USAGE_STAGING;
        tex_desc.BindFlags = 0;
        tex_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;
        tex_desc.MiscFlags = 0;

        let mut staging: Option<ID3D11Texture2D> = None;
        device
            .CreateTexture2D(&tex_desc, None, Some(&mut staging))
            .map_err(|e| format!("CreateTexture2D (staging) failed: {}", e))?;
        let staging = staging.ok_or("Staging texture is None")?;

        // 8. Copy GPU texture to staging
        context.CopyResource(&staging, &texture);

        // 9. Map staging texture to read pixels
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| format!("Map failed: {}", e))?;

        let stride = mapped.RowPitch;
        let data_size = (stride * height) as usize;
        let src_slice = std::slice::from_raw_parts(mapped.pData as *const u8, data_size);
        let data = src_slice.to_vec();

        context.Unmap(&staging, 0);

        // 10. Release frame
        duplication
            .ReleaseFrame()
            .map_err(|e| format!("ReleaseFrame failed: {}", e))?;

        Ok(CapturedFrame {
            width,
            height,
            data,
            stride,
        })
    }
}

/// Encode a CapturedFrame as JPEG bytes via BMP→PowerShell pipeline.
pub fn encode_jpeg(frame: &CapturedFrame, quality: u8) -> Result<Vec<u8>, String> {
    let tmp_bmp = std::env::temp_dir().join("rc_dxgi_frame.bmp");
    let tmp_jpg = std::env::temp_dir().join("rc_dxgi_frame.jpg");

    let row_bytes = (frame.width * 3) as usize;
    let bmp_stride = (row_bytes + 3) & !3;
    let pixel_data_size = bmp_stride * frame.height as usize;

    let mut bmp_data: Vec<u8> = Vec::with_capacity(54 + pixel_data_size);

    // BMP File Header (14 bytes)
    let file_size = 54 + pixel_data_size;
    bmp_data.extend_from_slice(b"BM");
    bmp_data.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&54u32.to_le_bytes());

    // BMP Info Header (40 bytes)
    bmp_data.extend_from_slice(&40u32.to_le_bytes());
    bmp_data.extend_from_slice(&(frame.width as i32).to_le_bytes());
    bmp_data.extend_from_slice(&(-(frame.height as i32)).to_le_bytes()); // top-down
    bmp_data.extend_from_slice(&1u16.to_le_bytes());
    bmp_data.extend_from_slice(&24u16.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&(pixel_data_size as u32).to_le_bytes());
    bmp_data.extend_from_slice(&2835u32.to_le_bytes());
    bmp_data.extend_from_slice(&2835u32.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());
    bmp_data.extend_from_slice(&0u32.to_le_bytes());

    // Pixel data: BGRA → BGR
    for y in 0..frame.height as usize {
        let src_offset = y * frame.stride as usize;
        for x in 0..frame.width as usize {
            let px = src_offset + x * 4;
            bmp_data.push(frame.data[px]);
            bmp_data.push(frame.data[px + 1]);
            bmp_data.push(frame.data[px + 2]);
        }
        for _ in 0..(bmp_stride - row_bytes) {
            bmp_data.push(0);
        }
    }

    std::fs::write(&tmp_bmp, &bmp_data)
        .map_err(|e| format!("Failed to write BMP: {}", e))?;

    let ps = format!(
        "Add-Type -AssemblyName System.Drawing; \
         $b = [System.Drawing.Image]::FromFile('{}'); \
         $enc = [System.Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | \
         Where-Object {{ $_.MimeType -eq 'image/jpeg' }}; \
         $ep = New-Object System.Drawing.Imaging.EncoderParameters(1); \
         $ep.Param[0] = New-Object System.Drawing.Imaging.EncoderParameter(\
         [System.Drawing.Imaging.Encoder]::Quality,[long]{}); \
         $b.Save('{}',$enc,$ep); $b.Dispose()",
        tmp_bmp.to_string_lossy().replace('\\', "\\\\"),
        quality,
        tmp_jpg.to_string_lossy().replace('\\', "\\\\"),
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps])
        .output()
        .map_err(|e| format!("PowerShell JPEG encode failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("JPEG encode failed: {}", stderr));
    }

    let jpg_bytes = std::fs::read(&tmp_jpg)
        .map_err(|e| format!("Failed to read JPEG: {}", e))?;

    let _ = std::fs::remove_file(&tmp_bmp);
    let _ = std::fs::remove_file(&tmp_jpg);

    Ok(jpg_bytes)
}

#[cfg(not(windows))]
pub fn capture_frame() -> Result<CapturedFrame, String> {
    Err("DXGI capture is only available on Windows".to_string())
}
