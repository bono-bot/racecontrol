//! WhatsApp PIN delivery via Evolution API — fire-and-forget.

use std::sync::Arc;

use serde_json::json;

use crate::state::AppState;

/// Send the reservation PIN to the customer via WhatsApp (Evolution API).
///
/// Fire-and-forget: logs success/failure but never propagates errors.
/// Queries the driver's phone from the database.
pub(crate) async fn send_pin_whatsapp(state: &Arc<AppState>, driver_id: &str, pin: &str) {
    // Get customer phone number
    let phone: String = match sqlx::query_as::<_, (Option<String>,)>(
        "SELECT phone FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    {
        Ok(Some((Some(p),))) if !p.is_empty() => p,
        Ok(_) => {
            tracing::warn!("No phone number for driver {} — skipping WhatsApp PIN delivery", driver_id);
            return;
        }
        Err(e) => {
            tracing::warn!("DB error fetching phone for driver {}: {} — skipping WhatsApp PIN delivery", driver_id, e);
            return;
        }
    };

    if let (Some(evo_url), Some(evo_key), Some(evo_instance)) = (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        let wa_phone = if phone.starts_with('+') {
            phone[1..].to_string()
        } else if phone.len() == 10 {
            format!("91{}", phone)
        } else {
            phone.to_string()
        };

        let message = format!(
            "Your Racing Point PIN: *{}*\n\nValid for 24 hours.\nShow this at the kiosk when you arrive.\n\nRacing Point eSports & Cafe",
            pin
        );

        let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = json!({
            "number": wa_phone,
            "text": message,
        });

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        match client.post(&url).header("apikey", evo_key).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("Reservation PIN sent via WhatsApp for driver {}", driver_id);
            }
            Ok(resp) => {
                tracing::warn!(
                    "Evolution API returned {} when sending reservation PIN for driver {}",
                    resp.status(),
                    driver_id
                );
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to send reservation PIN via WhatsApp for driver {}: {}",
                    driver_id,
                    e
                );
            }
        }
    } else {
        tracing::info!(
            "Reservation PIN for driver {} (Evolution API not configured — PIN not sent via WhatsApp)",
            driver_id
        );
    }
}
