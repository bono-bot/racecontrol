mod config;
mod frame;
mod stream;

use config::Config;
use frame::FrameBuffer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load config from CLI arg or default path
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\RacingPoint\rc-sentry-ai.toml".to_string());

    let config = Config::load(&config_path)?;

    tracing::info!(
        cameras = config.cameras.len(),
        relay_rtsp = %config.relay.rtsp_base,
        relay_api = %config.relay.api_url,
        port = config.service.port,
        "rc-sentry-ai starting"
    );

    let frame_buf = FrameBuffer::new();

    // Spawn one task per camera for independent RTSP streaming
    for camera in config.cameras.iter() {
        let cam = camera.clone();
        let rtsp_base = config.relay.rtsp_base.clone();
        let buf = frame_buf.clone();
        tokio::spawn(async move {
            stream::camera_loop(cam, rtsp_base, buf).await;
        });
    }

    // Health endpoint added in Plan 03

    tracing::info!("rc-sentry-ai running, press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    tracing::info!("rc-sentry-ai shutting down");

    Ok(())
}
