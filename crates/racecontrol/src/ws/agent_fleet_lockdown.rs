use std::sync::Arc;
use crate::activity_log::log_pod_activity;
use crate::state::AppState;

/// Handle AgentMessage::KioskLockdown.
pub(crate) async fn handle_kiosk_lockdown(
    state: &Arc<AppState>,
    pod_id: &str,
    reason: &str,
) {
    tracing::warn!("[kiosk] Pod {} LOCKDOWN: {}", pod_id, reason);
    log_pod_activity(state, pod_id, "kiosk", "Kiosk Lockdown", reason, "rc-bot", None);

    let pause_result: Result<Option<(String,)>, _> = sqlx::query_as(
        "SELECT id FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1"
    )
    .bind(pod_id)
    .fetch_optional(&state.db)
    .await;

    if let Ok(Some((session_id,))) = pause_result {
        let pause_reason = format!("Security anomaly: {}", reason);
        let _ = sqlx::query(
            "UPDATE billing_sessions SET status = 'paused_manual', pause_count = pause_count + 1 WHERE id = ? AND status = 'active'"
        )
        .bind(&session_id)
        .execute(&state.db)
        .await;
        tracing::warn!("[kiosk] Billing session {} auto-paused due to lockdown on pod {}", session_id, pod_id);
        log_pod_activity(state, pod_id, "billing", "Auto-Pause", &pause_reason, "rc-bot", None);
    }

    let alert_msg = format!(
        "SECURITY ALERT -- Pod {} LOCKDOWN\nReason: {}\nBilling auto-paused. Check admin dashboard.",
        pod_id, reason
    );
    crate::whatsapp_alerter::send_security_alert(&state.config, pod_id, &alert_msg).await;
}
