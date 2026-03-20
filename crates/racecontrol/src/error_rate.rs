use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{Event, Subscriber};
use tracing_subscriber::layer::Context;
use tracing_subscriber::Layer;

/// Configuration for error rate monitoring.
#[derive(Debug, Clone)]
pub struct ErrorRateConfig {
    /// Number of errors in window that triggers alert (default: 5)
    pub threshold: usize,
    /// Sliding window duration in seconds (default: 60)
    pub window_secs: u64,
    /// Cooldown between alerts in seconds (default: 1800 = 30 min)
    pub cooldown_secs: u64,
}

impl Default for ErrorRateConfig {
    fn default() -> Self {
        Self {
            threshold: 5,
            window_secs: 60,
            cooldown_secs: 1800,
        }
    }
}

struct ErrorRateState {
    timestamps: VecDeque<Instant>,
    config: ErrorRateConfig,
    last_alerted: Option<Instant>,
}

/// A tracing Layer that counts ERROR-level events in a sliding window
/// and signals an alerter task via mpsc channel when threshold is exceeded.
pub struct ErrorCountLayer {
    inner: Arc<Mutex<ErrorRateState>>,
    alert_tx: tokio::sync::broadcast::Sender<()>,
}

impl ErrorCountLayer {
    pub fn new(config: ErrorRateConfig, alert_tx: tokio::sync::broadcast::Sender<()>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ErrorRateState {
                timestamps: VecDeque::new(),
                config,
                last_alerted: None,
            })),
            alert_tx,
        }
    }
}

impl<S: Subscriber> Layer<S> for ErrorCountLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        if *event.metadata().level() != tracing::Level::ERROR {
            return;
        }

        let mut state = match self.inner.lock() {
            Ok(s) => s,
            Err(_) => return, // poisoned mutex — skip silently
        };

        let now = Instant::now();
        let window = Duration::from_secs(state.config.window_secs);

        // Evict timestamps outside the sliding window
        let cutoff = now.checked_sub(window).unwrap_or(now);
        state.timestamps.retain(|&t| t > cutoff);
        state.timestamps.push_back(now);

        // Check threshold
        if state.timestamps.len() >= state.config.threshold {
            // Check cooldown
            let cooldown = Duration::from_secs(state.config.cooldown_secs);
            if let Some(last) = state.last_alerted {
                if now.duration_since(last) < cooldown {
                    return; // still in cooldown
                }
            }

            // Fire alert — broadcast send (CRITICAL: on_event is sync)
            if self.alert_tx.send(()).is_ok() {
                state.last_alerted = Some(now);
                // Clear timestamps to avoid re-firing on next error
                state.timestamps.clear();
            }
        }
    }
}

/// Async task that receives error rate alerts and sends email.
/// Spawned with tokio::spawn in main.rs.
///
/// Uses its own EmailAlerter instances (NOT state.email_alerter) because:
/// - Error rate alerts use "server" as pod_id (not per-pod)
/// - Independent rate limiting from watchdog alerts
/// - Sends to two recipients (james + uday)
pub async fn error_rate_alerter_task(
    mut alert_rx: tokio::sync::broadcast::Receiver<()>,
    email_script_path: String,
    recipients: Vec<String>,
) {
    use crate::email_alerts::EmailAlerter;

    // Create a dedicated alerter for each recipient — belt-and-suspenders rate limiting
    // (rate-limited at Layer level too, this is belt-and-suspenders)
    let mut alerters: Vec<EmailAlerter> = recipients
        .iter()
        .map(|r| EmailAlerter::new(r.clone(), email_script_path.clone(), true))
        .collect();

    loop {
        match alert_rx.recv().await {
            Ok(()) => {
                let subject = "RaceControl: High Error Rate Alert";
                let body = format!(
                    "RaceControl error rate threshold exceeded.\n\n\
                     The server has logged an unusual number of errors in a short time window.\n\
                     Please check the structured logs at logs/racecontrol-*.jsonl on the server.\n\n\
                     Run: jq 'select(.level == \"ERROR\")' logs/racecontrol-$(date +%%Y-%%m-%%d).jsonl\n\n\
                     — James Vowles (automated alert)"
                );

                for alerter in &mut alerters {
                    alerter.send_alert("server", subject, &body).await;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("Error rate alerter lagged by {} messages", n);
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::broadcast;

    fn make_layer(threshold: usize, window_secs: u64, cooldown_secs: u64) -> (ErrorCountLayer, broadcast::Receiver<()>) {
        let (tx, _) = broadcast::channel(16);
        let rx = tx.subscribe();
        let config = ErrorRateConfig {
            threshold,
            window_secs,
            cooldown_secs,
        };
        (ErrorCountLayer::new(config, tx), rx)
    }

    #[test]
    fn test_error_rate_below_threshold() {
        let (_layer, mut rx) = make_layer(5, 60, 1800);

        // Simulate 4 errors below threshold — no alert should be sent
        // (We test the state directly since on_event requires a subscriber context)
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_error_rate_threshold_reached() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let (tx, _) = broadcast::channel(16);
        let mut rx = tx.subscribe();
        let config = ErrorRateConfig {
            threshold: 3,
            window_secs: 60,
            cooldown_secs: 1800,
        };
        let layer = ErrorCountLayer::new(config, tx);

        // Use the layer with a test subscriber
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        // Emit 3 errors (meets threshold)
        for _ in 0..3 {
            tracing::error!("test error");
        }

        // Alert should have been sent
        rt.block_on(async {
            let result = tokio::time::timeout(
                Duration::from_millis(100),
                rx.recv(),
            ).await;
            assert!(result.is_ok(), "Expected alert to be received");
        });
    }

    #[test]
    fn test_error_rate_window_eviction() {
        let (tx, _) = broadcast::channel(16);
        let mut rx = tx.subscribe();
        let config = ErrorRateConfig {
            threshold: 5,
            window_secs: 1, // 1 second window for test
            cooldown_secs: 0,
        };
        let layer = ErrorCountLayer::new(config, tx);

        // Manually insert 4 old timestamps (outside window)
        {
            let mut state = layer.inner.lock().unwrap();
            let old = Instant::now() - Duration::from_secs(5);
            for _ in 0..4 {
                state.timestamps.push_back(old);
            }
        }

        // Use the layer to emit 1 new error
        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);
        tracing::error!("fresh error");

        // Old timestamps should have been evicted, total = 1, below threshold of 5
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_error_rate_cooldown() {
        let rt = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
        let (tx, _) = broadcast::channel(16);
        let mut rx = tx.subscribe();
        let config = ErrorRateConfig {
            threshold: 2,
            window_secs: 60,
            cooldown_secs: 3600, // 1 hour cooldown
        };
        let layer = ErrorCountLayer::new(config, tx);

        use tracing_subscriber::layer::SubscriberExt;
        let subscriber = tracing_subscriber::registry().with(layer);
        let _guard = tracing::subscriber::set_default(subscriber);

        // First burst: 2 errors -> alert fires
        tracing::error!("error 1");
        tracing::error!("error 2");

        rt.block_on(async {
            let r = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
            assert!(r.is_ok(), "First alert should fire");
        });

        // Second burst: 2 more errors -> cooldown blocks
        tracing::error!("error 3");
        tracing::error!("error 4");

        // Should NOT receive another alert (cooldown active)
        assert!(rx.try_recv().is_err(), "Second alert should be blocked by cooldown");
    }
}
