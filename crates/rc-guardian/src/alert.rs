//! WhatsApp alert via Evolution API (EG-05).

use crate::config::GuardianConfig;
use serde::Serialize;
use tracing::{info, warn, error};

/// Evolution API send message request body.
#[derive(Serialize)]
struct EvolutionSendMessage {
    number: String,
    text: String,
}

/// Send a WhatsApp alert via Evolution API (EG-05).
///
/// Best-effort: logs errors but does not propagate them.
/// The guardian should never crash because WhatsApp delivery failed.
pub async fn send_whatsapp(client: &reqwest::Client, config: &GuardianConfig, message: &str) {
    if config.evolution_api_key.is_empty() {
        warn!("Evolution API key not configured — skipping WhatsApp alert");
        info!(message, "Would have sent WhatsApp alert");
        return;
    }

    let url = format!(
        "{}/message/sendText/{}",
        config.evolution_api_url.trim_end_matches('/'),
        config.evolution_instance
    );

    let body = EvolutionSendMessage {
        number: config.alert_phone.clone(),
        text: message.to_string(),
    };

    info!(
        phone = %config.alert_phone,
        message_len = message.len(),
        "Sending WhatsApp alert"
    );

    let result = client
        .post(&url)
        .header("apikey", &config.evolution_api_key)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await;

    match result {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                info!("WhatsApp alert sent successfully");
            } else {
                let body = resp.text().await.unwrap_or_default();
                error!(
                    status = status.as_u16(),
                    body = %body,
                    "WhatsApp alert delivery failed"
                );
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to send WhatsApp alert (network error)");
        }
    }
}
