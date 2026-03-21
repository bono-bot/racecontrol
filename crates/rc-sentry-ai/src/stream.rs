use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;

use crate::config::CameraConfig;
use crate::frame::FrameBuffer;

/// Per-camera RTSP stream loop with automatic reconnection.
///
/// Each camera runs in its own tokio task. On stream error, waits 5 seconds
/// before reconnecting. On normal stream end, reconnects immediately.
pub async fn camera_loop(camera: CameraConfig, rtsp_base: String, frame_buf: FrameBuffer) {
    let mut attempt: u64 = 0;
    loop {
        attempt += 1;
        tracing::info!(
            camera = %camera.name,
            attempt,
            "connecting to RTSP stream"
        );

        match connect_and_stream(&camera, &rtsp_base, &frame_buf).await {
            Ok(()) => {
                tracing::info!(camera = %camera.name, "stream ended normally, reconnecting");
                attempt = 0;
            }
            Err(e) => {
                tracing::warn!(
                    camera = %camera.name,
                    error = %e,
                    attempt,
                    "stream error, reconnecting in 5s"
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn connect_and_stream(
    camera: &CameraConfig,
    rtsp_base: &str,
    frame_buf: &FrameBuffer,
) -> anyhow::Result<()> {
    let relay_url = camera.relay_url(rtsp_base);
    let url = url::Url::parse(&relay_url)?;

    let session_group = Arc::new(retina::client::SessionGroup::default());
    let mut session = retina::client::Session::describe(
        url,
        retina::client::SessionOptions::default().session_group(session_group),
    )
    .await
    .map_err(|e| anyhow::anyhow!("RTSP DESCRIBE failed: {e}"))?;

    session
        .setup(
            0,
            retina::client::SetupOptions::default().transport(retina::client::Transport::Tcp(
                retina::client::TcpTransportOptions::default(),
            )),
        )
        .await
        .map_err(|e| anyhow::anyhow!("RTSP SETUP failed: {e}"))?;

    let mut session = session
        .play(retina::client::PlayOptions::default())
        .await
        .map_err(|e| anyhow::anyhow!("RTSP PLAY failed: {e}"))?
        .demuxed()
        .map_err(|e| anyhow::anyhow!("demux failed: {e}"))?;

    let frame_interval = Duration::from_millis(1000 / camera.fps.max(1) as u64);

    tracing::info!(
        camera = %camera.name,
        fps = camera.fps,
        "stream connected, extracting frames"
    );

    while let Some(item) = session.next().await {
        let item = item.map_err(|e| anyhow::anyhow!("stream item error: {e}"))?;
        match item {
            retina::codec::CodecItem::VideoFrame(frame) => {
                frame_buf.update(&camera.name, frame.data().to_vec()).await;
                // Rate limit to configured FPS
                tokio::time::sleep(frame_interval).await;
            }
            _ => {
                // Ignore audio, metadata, and other codec items
            }
        }
    }

    Ok(())
}
