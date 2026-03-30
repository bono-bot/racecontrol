//! Cloud -> racecontrol Action Queue
//!
//! Supports dual-mode operation:
//! - **Relay mode**: Actions are PUSHED to the venue via comms-link relay (sub-second delivery).
//!   Polling is disabled since actions arrive via POST /actions/process.
//! - **HTTP fallback**: Polls the cloud for pending actions every N seconds (default: 3s).
//!
//! On reconnect transition (fallback -> relay), runs one final poll to drain
//! any actions queued during the outage before disabling polling.

use std::sync::Arc;
use std::sync::atomic::Ordering;
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
///
/// When comms-link relay is available, polling is skipped (actions arrive via push).
/// When relay is unavailable, falls back to HTTP polling at action_poll_interval_secs.
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
    let has_relay = cloud.comms_link_url.is_some();

    tracing::info!(
        "Action queue enabled: polling every {}s at {} (relay: {})",
        interval_secs,
        api_url,
        if has_relay { "configured" } else { "not configured" }
    );

    tokio::spawn(async move {
        // Wait 5s on startup before first poll
        tokio::time::sleep(Duration::from_secs(5)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        let mut prev_relay_available = false;

        loop {
            interval.tick().await;

            if has_relay {
                // Read the shared relay flag (written by cloud_sync with hysteresis)
                let relay_now = state.relay_available.load(Ordering::Relaxed);

                if !prev_relay_available && relay_now {
                    // Transitioning fallback -> relay: run one final poll to drain
                    // actions created while relay was down. They weren't pushed via relay
                    // (it was down) and won't be polled after this tick (polling is about
                    // to be disabled).
                    tracing::info!(
                        "Relay reconnected -- running final poll to drain actions queued during outage"
                    );
                    if let Err(e) = poll_actions(&state, &api_url, &secret).await {
                        tracing::debug!("Action queue final drain poll: {}", e);
                    }
                } else if relay_now {
                    // Steady-state relay mode: skip polling, actions arrive via
                    // POST /actions/process push endpoint
                    tracing::debug!("Action queue: relay active, skipping poll (actions arrive via push)");
                } else {
                    // Fallback mode: relay unavailable, poll normally
                    if let Err(e) = poll_actions(&state, &api_url, &secret).await {
                        tracing::debug!("Action queue poll: {}", e);
                    }
                }

                prev_relay_available = relay_now;
            } else {
                // No relay configured: always poll
                if let Err(e) = poll_actions(&state, &api_url, &secret).await {
                    tracing::debug!("Action queue poll: {}", e);
                }
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
/// Public within the crate so routes.rs can call it from the /actions/process endpoint.
pub(crate) async fn process_action(state: &Arc<AppState>, action: &CloudAction) -> anyhow::Result<()> {
    match action {
        CloudAction::BookingCreated {
            booking_id,
            driver_id,
            pricing_tier_id,
            experience_id,
            pod_id,
        } => {
            // Bug #19: Redact driver_id in logs
            let redacted_driver = if driver_id.len() > 4 { format!("{}***", &driver_id[..4]) } else { "***".to_string() };
            tracing::info!(
                "Action: BookingCreated booking={} driver={} tier={} pod={:?}",
                booking_id,
                redacted_driver,
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
            // Bug #19: Redact driver_id in logs
            let redacted_driver = if driver_id.len() > 4 { format!("{}***", &driver_id[..4]) } else { "***".to_string() };
            tracing::info!(
                "Action: WalletTopUp driver={} amount={} txn={}",
                redacted_driver,
                amount_paise,
                transaction_id
            );

            // Bug #10: Idempotency check — skip if this transaction already exists
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM wallet_transactions WHERE id = ? AND type = 'topup'",
            )
            .bind(transaction_id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);

            if existing.is_some() {
                tracing::info!("WalletTopUp txn={} already processed — skipping (idempotent)", transaction_id);
                return Ok(());
            }

            // MMA-GLM5-FIX: Wrap wallet credit + transaction log in a DB transaction
            // to prevent balance increase without a recorded transaction entry.
            let mut tx = state.db.begin().await
                .map_err(|e| anyhow::anyhow!("DB transaction start failed: {}", e))?;

            sqlx::query(
                "UPDATE wallets SET balance_paise = balance_paise + ?, updated_at = datetime('now') WHERE driver_id = ?",
            )
            .bind(amount_paise)
            .bind(driver_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Wallet credit failed: {}", e))?;

            sqlx::query(
                "INSERT OR IGNORE INTO wallet_transactions (id, driver_id, amount_paise, type, reference_id, created_at) \
                 VALUES (?, ?, ?, 'topup', ?, datetime('now'))",
            )
            .bind(transaction_id)
            .bind(driver_id)
            .bind(amount_paise)
            .bind(transaction_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!("Transaction log failed: {}", e))?;

            tx.commit().await
                .map_err(|e| anyhow::anyhow!("Wallet topup commit failed: {}", e))?;

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
            // Bug #19: Redact driver_id in logs
            let redacted_driver = if driver_id.len() > 4 { format!("{}***", &driver_id[..4]) } else { "***".to_string() };
            tracing::info!(
                "Action: QrConfirmed token={} driver={}",
                token_id,
                redacted_driver
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
            tracing::info!("Action: Notification '{}' -> {}", title, target);

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
