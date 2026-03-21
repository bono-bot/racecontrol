mod config;
mod frame;

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

    let _frame_buf = FrameBuffer::new();

    // Stream tasks spawned in Task 2

    // Health endpoint added in Plan 03

    tracing::info!("rc-sentry-ai running, press Ctrl+C to stop");
    tokio::signal::ctrl_c().await?;
    tracing::info!("rc-sentry-ai shutting down");

    Ok(())
}
