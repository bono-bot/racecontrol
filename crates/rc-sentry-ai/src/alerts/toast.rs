use tokio::sync::broadcast;

use crate::alerts::types::AlertEvent;

/// Windows toast notification engine.
///
/// Subscribes to the alert broadcast channel and displays a Windows 10/11 toast
/// notification with system sound for every face detection event.
/// Windows plays the default notification sound automatically for each toast.
#[cfg(target_os = "windows")]
pub async fn run(mut alert_rx: broadcast::Receiver<AlertEvent>) {
    tracing::info!("toast notification engine started");

    let ist = chrono::FixedOffset::east_opt(5 * 3600 + 1800)
        .expect("valid IST offset");

    loop {
        match alert_rx.recv().await {
            Ok(event) => {
                let (title, line1, line2) = format_event(&event, &ist);
                // winrt-toast COM calls are synchronous — run in blocking thread
                tokio::task::spawn_blocking(move || {
                    if let Err(e) = show_toast(&title, &line1, &line2) {
                        tracing::warn!(error = %e, "failed to show toast notification");
                    }
                });
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!(skipped = n, "toast receiver lagged, dropped events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::info!("toast broadcast channel closed, shutting down");
                break;
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn format_event(
    event: &AlertEvent,
    ist: &chrono::FixedOffset,
) -> (String, String, String) {
    use chrono::TimeZone;

    match event {
        AlertEvent::Recognized {
            person_name,
            camera,
            timestamp,
            ..
        } => {
            let ts_ist = ist.from_utc_datetime(&timestamp.naive_utc());
            let ts_str = ts_ist.format("%d-%b %H:%M:%S IST").to_string();
            (
                "Person Detected".to_string(),
                person_name.clone(),
                format!("{camera} | {ts_str}"),
            )
        }
        AlertEvent::UnknownPerson {
            camera, timestamp, ..
        } => {
            let ts_ist = ist.from_utc_datetime(&timestamp.naive_utc());
            let ts_str = ts_ist.format("%d-%b %H:%M:%S IST").to_string();
            (
                "Unknown Person".to_string(),
                "Unrecognized face detected".to_string(),
                format!("{camera} | {ts_str}"),
            )
        }
    }
}

#[cfg(target_os = "windows")]
fn show_toast(title: &str, line1: &str, line2: &str) -> anyhow::Result<()> {
    use winrt_toast::content::text::TextPlacement;
    use winrt_toast::{Header, Text, Toast, ToastManager};

    let mut toast = Toast::new();

    toast
        .header(Header::new("sentry", "Racing Point Sentry", ""))
        .text1(Text::new(title))
        .text2(Text::new(line1))
        .text3(Text::new(line2).with_placement(TextPlacement::Attribution));

    // Windows plays the default notification sound automatically for each toast.
    // winrt-toast 0.1 does not expose an audio API, but the system default sound
    // is enabled by default in the Windows toast XML schema.

    let manager = ToastManager::new("RacingPoint.Sentry");
    manager.show(&toast)?;

    Ok(())
}

/// No-op stub for non-Windows platforms -- drains the channel to avoid backpressure.
#[cfg(not(target_os = "windows"))]
pub async fn run(mut alert_rx: broadcast::Receiver<AlertEvent>) {
    tracing::info!("toast notifications not available on this platform");
    loop {
        match alert_rx.recv().await {
            Err(broadcast::error::RecvError::Closed) => break,
            _ => continue,
        }
    }
}
