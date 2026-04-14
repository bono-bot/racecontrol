//! AI escalation and staff alerting recovery steps for offline pods.
//!
//! Extracted from pod_healer_recovery.rs (v49.0 Architecture Completion).
//! Implements Steps 4-5 of the graduated recovery ladder:
//!   Step 4 (AiEscalation): Escalate to AI for root cause analysis
//!   Step 5+ (AlertStaff): Repeated staff alerts every 15 minutes

use std::sync::Arc;

use crate::activity_log::log_pod_activity;
use crate::pod_healer::{PodRecoveryStep, PodRecoveryTracker};
use crate::state::AppState;
use rc_common::recovery::{
    RecoveryAction, RecoveryAuthority, RecoveryDecision, RecoveryLogger, RECOVERY_LOG_SERVER,
};
use rc_common::types::PodInfo;

/// Execute the AI escalation recovery step (Step 4 of graduated recovery).
///
/// Sends pod context to AI for root cause analysis and logs the suggestion.
pub(crate) async fn run_ai_escalation_step(
    state: &Arc<AppState>,
    pod: &PodInfo,
    tracker: &mut PodRecoveryTracker,
) {
    tracing::info!(
        target: "pod_healer",
        "Pod {} — step 3: AI escalation",
        pod.id
    );
    let decision = RecoveryDecision::new(
        "server",
        "rc-agent.exe",
        RecoveryAuthority::PodHealer,
        RecoveryAction::EscalateToAi,
        "graduated_step3_ai_escalation",
    );
    let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

    let context = format!(
        "Pod {} is offline. Tier 1 restart was attempted and pod remains offline. \
         Last seen: {:?}. Please suggest root cause and next steps.",
        pod.id, pod.last_seen
    );
    let messages = vec![
        serde_json::json!({
            "role": "system",
            "content": "You are a sim racing venue technician. A pod has failed to recover \
                        after an automated restart. Provide a brief root cause and specific \
                        manual steps. Keep under 150 words."
        }),
        serde_json::json!({ "role": "user", "content": context.clone() }),
    ];
    match crate::ai::query_ai(
        &state.config.ai_debugger,
        &messages,
        Some(&state.db),
        Some("healer_graduated"),
    )
    .await
    {
        Ok((suggestion, model)) => {
            tracing::info!(
                target: "pod_healer",
                "Pod {} AI suggestion ({}): {}",
                pod.id,
                model,
                suggestion.chars().take(100).collect::<String>()
            );
            log_pod_activity(
                state,
                &pod.id,
                "race_engineer",
                "AI Escalation",
                &format!(
                    "AI suggestion ({}): {}",
                    model,
                    suggestion.chars().take(200).collect::<String>()
                ),
                "race_engineer",
                None,
            );
        }
        Err(e) => {
            tracing::warn!(
                target: "pod_healer",
                "Pod {} AI escalation failed: {}",
                pod.id,
                e
            );
        }
    }
    tracker.step = PodRecoveryStep::AlertStaff;
}

/// Execute the staff alerting recovery step (Step 5 of graduated recovery).
///
/// Sends email alert to staff and re-alerts every 15 minutes until resolved.
pub(crate) async fn run_alert_staff_step(
    state: &Arc<AppState>,
    pod: &PodInfo,
    tracker: &mut PodRecoveryTracker,
) {
    // CONN-RESIL: Re-alert every 15 minutes instead of every 2-minute cycle.
    const RE_ALERT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(15 * 60);
    let should_alert = tracker
        .last_staff_alert_at
        .map(|t| t.elapsed() >= RE_ALERT_INTERVAL)
        .unwrap_or(true); // First alert always fires

    if !should_alert {
        tracing::info!(
            target: "pod_healer",
            "Pod {} — still at AlertStaff, re-alert suppressed (next in {}s)",
            pod.id,
            RE_ALERT_INTERVAL.as_secs().saturating_sub(
                tracker
                    .last_staff_alert_at
                    .map(|t| t.elapsed().as_secs())
                    .unwrap_or(0)
            )
        );
        return;
    }

    tracing::warn!(
        target: "pod_healer",
        "Pod {} — step 4: alerting staff (re-alert every 15min)",
        pod.id
    );
    let decision = RecoveryDecision::new(
        "server",
        "rc-agent.exe",
        RecoveryAuthority::PodHealer,
        RecoveryAction::AlertStaff,
        "graduated_step4_staff_alert",
    );
    let _ = RecoveryLogger::new(RECOVERY_LOG_SERVER).log(&decision);

    let offline_duration = tracker
        .first_detected_at
        .map(|t| t.elapsed())
        .unwrap_or_default();
    let body = format!(
        "Pod {} has failed all automated recovery steps.\n\
         Tier 1 restart attempted. AI escalated. Pod still offline.\n\
         Offline for: {}min {}s\n\
         Last seen: {:?}\n\
         Manual intervention required.\n\
         (This alert repeats every 15 minutes until resolved.)",
        pod.id,
        offline_duration.as_secs() / 60,
        offline_duration.as_secs() % 60,
        pod.last_seen
    );
    let subject = format!(
        "[RaceControl] Pod {} — Manual Intervention Required ({}min offline)",
        pod.id,
        offline_duration.as_secs() / 60
    );
    state
        .email_alerter
        .write()
        .await
        .send_alert(&pod.id, &subject, &body)
        .await;
    tracker.last_staff_alert_at = Some(std::time::Instant::now());
    log_pod_activity(
        state,
        &pod.id,
        "race_engineer",
        "Staff Alert Sent",
        &format!(
            "All automated recovery steps exhausted — staff alerted (offline {}min)",
            offline_duration.as_secs() / 60
        ),
        "race_engineer",
        None,
    );
}
