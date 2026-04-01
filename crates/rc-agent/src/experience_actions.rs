//! Experience Score Actions — threshold-triggered responses to experience scores.
//!
//! Phase 276 — CX-07 and CX-08:
//! - Score < 80%: auto-flag pod for maintenance (CX-07)
//! - Score < 50%: auto-remove from rotation + WhatsApp alert to Uday (CX-08)

use tokio::sync::mpsc;

use rc_common::protocol::{AgentMessage, EscalationPayload};

use crate::experience_score::{ExperienceScore, ScoreStatus, MAINTENANCE_THRESHOLD, REMOVAL_THRESHOLD};

const LOG_TARGET: &str = "experience-actions";

/// Evaluate the experience score and take appropriate action.
///
/// CX-07: Score < 80% → log WARNING, flag for maintenance.
/// CX-08: Score < 50% → send EscalationPayload via WS (triggers WhatsApp via Phase 274).
pub async fn evaluate_score(
    score: &ExperienceScore,
    node_id: &str,
    ws_tx: &mpsc::Sender<AgentMessage>,
) {
    match score.status {
        ScoreStatus::RemoveFromRotation => {
            tracing::warn!(
                target: LOG_TARGET,
                pod = node_id,
                score = format!("{:.1}", score.total),
                threshold = REMOVAL_THRESHOLD,
                "CX-08: Pod experience score {:.1}% < {}% — auto-removing from rotation, escalating to staff",
                score.total,
                REMOVAL_THRESHOLD,
            );

            // CX-08: Send escalation to server for WhatsApp alert
            let escalation = EscalationPayload {
                pod_id: node_id.to_string(),
                incident_id: uuid::Uuid::new_v4().to_string(),
                severity: "critical".to_string(),
                trigger: format!("CX-08: Experience score {:.1}% below {}%", score.total, REMOVAL_THRESHOLD),
                summary: format!(
                    "Pod {} experience score {:.1}% — remove from rotation. game={:.0}% session={:.0}% display={:.0}% hw={:.0}% billing={:.0}%",
                    node_id, score.total, score.game_launch, score.session_completion,
                    score.display_stability, score.hardware_responsive, score.billing_accuracy
                ),
                actions_tried: vec![
                    "Experience score calculated from 5-min diagnostic window".to_string(),
                ],
                impact: "Pod quality below acceptable threshold — customers will have degraded experience".to_string(),
                dashboard_url: format!("/status#{}", node_id),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            let msg = AgentMessage::EscalationRequest(escalation);
            if let Err(e) = ws_tx.send(msg).await {
                tracing::error!(
                    target: LOG_TARGET,
                    error = %e,
                    "CX-08: Failed to send removal escalation — WhatsApp alert will not fire"
                );
            }
        }
        ScoreStatus::Maintenance => {
            tracing::warn!(
                target: LOG_TARGET,
                pod = node_id,
                score = format!("{:.1}", score.total),
                threshold = MAINTENANCE_THRESHOLD,
                "CX-07: Pod experience score {:.1}% < {}% — flagged for maintenance",
                score.total,
                MAINTENANCE_THRESHOLD,
            );
        }
        ScoreStatus::Healthy => {
            // No action needed
        }
    }
}
