use tokio::sync::broadcast;

use crate::alerts::types::AlertEvent;
use crate::recognition::types::RecognitionResult;

/// Run the alert engine, subscribing to recognition events and fanning out
/// as `AlertEvent` messages on the alert broadcast channel.
pub async fn run(
    mut rx: broadcast::Receiver<RecognitionResult>,
    alert_tx: broadcast::Sender<AlertEvent>,
) {
    tracing::info!("alert engine started");

    loop {
        match rx.recv().await {
            Ok(result) => {
                let event = AlertEvent::from(result);
                // If no WS clients are listening, send returns Err — that's fine.
                let _ = alert_tx.send(event);
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "alert engine receiver lagged, dropped events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("alert engine broadcast channel closed, shutting down");
                break;
            }
        }
    }
}
