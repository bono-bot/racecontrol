use tokio::sync::broadcast;

use super::types::{AlertEvent, UnknownFaceEvent};

pub async fn run(
    mut unknown_rx: broadcast::Receiver<UnknownFaceEvent>,
    alert_tx: broadcast::Sender<AlertEvent>,
    face_crop_dir: String,
    face_crop_quality: u8,
    rate_limit_secs: u64,
) {
    // TODO: implemented in Task 2
    let _ = (&alert_tx, &face_crop_dir, face_crop_quality, rate_limit_secs);
    loop {
        match unknown_rx.recv().await {
            Ok(_) => {}
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "unknown face receiver lagged");
            }
        }
    }
}
