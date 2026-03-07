//! Cloud → rc-core Action Queue
//!
//! Polls the cloud for pending actions every N seconds (default: 3s).
//! Processes each action (booking, wallet top-up, QR confirm, etc.)
//! and ACKs back to cloud with status.
//!
//! Same pattern as remote_terminal.rs but for structured business actions.

use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::state::AppState;
use rc_common::protocol::{CloudAction, PendingCloudAction};

#[derive(Deserialize)]
struct PendingActionsResponse {
    actions: Option<Vec<PendingCloudAction>>,
}

/// Spawn the action queue background task.
/// Only starts if cloud.enabled = true and cloud.api_url is set.
pub fn spawn(state: Arc<AppState>) {
    let cloud = &state.config.cloud;
    if !cloud.enabled {
        return;
    }

    let api_url = match &cloud.api_url {
        Some(url) => url.clone(),
        None => return,
    };

    let secret = cloud.terminal_secret.clone().unwrap_or_default();
    let interval_secs = cloud.action_poll_interval_secs;

    tracing::info!(
        "Action queue enabled: polling every {}s at {}",
        interval_secs,
        api_url
    );

    tokio::spawn(async move {
        // Wait 5s on startup before first poll
        tokio::time::sleep(Duration::from_secs(5)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            interval.tick().await;
            if let Err(e) = poll_actions(&state, &api_url, &secret).await {
                tracing::debug!("Action queue poll: {}", e);
            }
        }
    });
}

/// Poll cloud for pending actions, process each, and ACK.
async fn poll_actions(
    state: &Arc<AppState>,
    cloud_url: &str,
    secret: &str,
) -> anyhow::Result<()> {
    let url = format!("{}/actions/pending", cloud_url);

    let resp = state
        .http_client
        .get(&url)
        .header("x-terminal-secret", secret)
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Cloud returned status {}", resp.status());
    }

    let body: PendingActionsResponse = resp.json().await?;

    let actions = match body.actions {
        Some(a) if !a.is_empty() => a,
        _ => return Ok(()),
    };

    tracing::info!("Action queue: processing {} actions", actions.len());

    for pending in actions {
        let result = process_action(state, &pending.action).await;
        let (status, error) = match &result {
            Ok(()) => ("completed", None),
            Err(e) => ("failed", Some(e.to_string())),
        };

        // ACK back to cloud
        ack_action(state, cloud_url, secret, &pending.id, status, error.as_deref()).await;

        if let Err(e) = &result {
            tracing::warn!(
                "Action {} ({:?}) failed: {}",
                pending.id,
                std::mem::discriminant(&pending.action),
                e
            );
        }
    }

    Ok(())
}

/// Process a single cloud action by dispatching to the appropriate module.
async fn process_action(state: &Arc<AppState>, action: &CloudAction) -> anyhow::Result<()> {
    match action {
        CloudAction::BookingCreated {
            booking_id,
            driver_id,
            pricing_tier_id,
            experience_id,
            pod_id,
        } => {
            tracing::info!(
                "Action: BookingCreated booking={} driver={} tier={} pod={:?}",
                booking_id,
                driver_id,
                pricing_tier_id,
                pod_id
            );

            // Log the booking event for dashboard visibility
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO sync_log (id, table_name, row_id, operation, payload, synced, created_at) \
                 VALUES (?, 'cloud_actions', ?, 'insert', ?, 0, datetime('now'))",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(booking_id)
            .bind(serde_json::to_string(action).unwrap_or_default())
            .execute(&state.db)
            .await;

            // Broadcast to dashboard so staff sees the new booking
            let _ = state.dashboard_tx.send(
                rc_common::protocol::DashboardEvent::AiMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: "cloud".to_string(),
                    recipient: "dashboard".to_string(),
                    content: format!(
                        "New booking from PWA: driver={}, tier={}, experience={:?}",
                        driver_id,
                        pricing_tier_id,
                        experience_id
                    ),
                    message_type: "booking_notification".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );

            Ok(())
        }

        CloudAction::WalletTopUp {
            driver_id,
            amount_paise,
            transaction_id,
        } => {
            tracing::info!(
                "Action: WalletTopUp driver={} amount={} txn={}",
                driver_id,
                amount_paise,
                transaction_id
            );

            // Credit the local wallet
            let _ = sqlx::query(
                "UPDATE wallets SET credits_paise = credits_paise + ?, updated_at = datetime('now') WHERE driver_id = ?",
            )
            .bind(amount_paise)
            .bind(driver_id)
            .execute(&state.db)
            .await;

            // Log the transaction
            let _ = sqlx::query(
                "INSERT OR IGNORE INTO wallet_transactions (id, driver_id, amount_paise, type, reference_id, created_at) \
                 VALUES (?, ?, ?, 'topup', ?, datetime('now'))",
            )
            .bind(transaction_id)
            .bind(driver_id)
            .bind(amount_paise)
            .bind(transaction_id)
            .execute(&state.db)
            .await;

            Ok(())
        }

        CloudAction::BookingCancelled { booking_id } => {
            tracing::info!("Action: BookingCancelled booking={}", booking_id);

            // Cancel any pending auth tokens for this booking
            let _ = sqlx::query(
                "UPDATE auth_tokens SET status = 'cancelled' WHERE id = ? AND status = 'pending'",
            )
            .bind(booking_id)
            .execute(&state.db)
            .await;

            Ok(())
        }

        CloudAction::QrConfirmed {
            token_id,
            driver_id,
        } => {
            tracing::info!(
                "Action: QrConfirmed token={} driver={}",
                token_id,
                driver_id
            );

            // Mark the QR auth token as consuming — billing will be triggered
            let _ = sqlx::query(
                "UPDATE auth_tokens SET status = 'consuming' WHERE id = ? AND status = 'pending'",
            )
            .bind(token_id)
            .execute(&state.db)
            .await;

            Ok(())
        }

        CloudAction::SettingsChanged { key, value } => {
            tracing::info!("Action: SettingsChanged {}={}", key, value);

            // Update local kiosk_settings
            let _ = sqlx::query(
                "INSERT OR REPLACE INTO kiosk_settings (key, value, updated_at) VALUES (?, ?, datetime('now'))",
            )
            .bind(key)
            .bind(value)
            .execute(&state.db)
            .await;

            Ok(())
        }

        CloudAction::Notification {
            title,
            body,
            target,
        } => {
            tracing::info!("Action: Notification '{}' → {}", title, target);

            // Broadcast to dashboard
            let _ = state.dashboard_tx.send(
                rc_common::protocol::DashboardEvent::AiMessage {
                    id: uuid::Uuid::new_v4().to_string(),
                    sender: "cloud".to_string(),
                    recipient: target.clone(),
                    content: format!("{}: {}", title, body),
                    message_type: "notification".to_string(),
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            );

            Ok(())
        }
    }
}

/// ACK a processed action back to cloud.
async fn ack_action(
    state: &Arc<AppState>,
    cloud_url: &str,
    secret: &str,
    action_id: &str,
    status: &str,
    error: Option<&str>,
) {
    let url = format!("{}/actions/{}/ack", cloud_url, action_id);
    let _ = state
        .http_client
        .post(&url)
        .header("x-terminal-secret", secret)
        .json(&json!({
            "status": status,
            "error": error,
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await;
}
