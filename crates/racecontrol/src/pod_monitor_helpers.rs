/// Pure helper functions extracted for testability.

/// Check if a pod's WebSocket sender channel is still open (liveness check).
#[cfg(test)]
pub(super) async fn is_ws_alive(state: &std::sync::Arc<crate::state::AppState>, pod_id: &str) -> bool {
    let senders = state.agent_senders.read().await;
    match senders.get(pod_id) {
        Some(sender) => !sender.is_closed(),
        None => false,
    }
}

/// Convert a cooldown duration to a human-readable label ("30s", "2m", "10m", "30m").
#[cfg(test)]
pub(super) fn backoff_label(cooldown: std::time::Duration) -> String {
    let secs = cooldown.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

/// Determine the failure reason string from check results.
///
/// Used by verification logic -- extracted for testability.
pub fn determine_failure_reason(process_ok: bool, ws_ok: bool, _lock_ok: bool) -> &'static str {
    if !process_ok {
        "process_dead"
    } else if !ws_ok {
        "no_ws"
    } else {
        "no_lock_screen"
    }
}

/// Determine failure type label from reason string.
pub fn failure_type_from_reason(reason: &str) -> &'static str {
    match reason {
        "process_dead" => "Process Dead",
        "no_ws" => "No WebSocket",
        "no_lock_screen" => "Lock Screen Unresponsive",
        _ => "Unknown",
    }
}
