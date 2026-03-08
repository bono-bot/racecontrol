use std::sync::Arc;

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::billing;
use crate::pod_reservation;
use crate::state::AppState;
use rc_common::protocol::{CoreToAgentMessage, DashboardEvent};
use rc_common::types::AuthTokenInfo;

// ─── JWT Claims ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // driver_id
    pub exp: usize,
    pub iat: usize,
}

// ─── Create Auth Token ─────────────────────────────────────────────────────

pub async fn create_auth_token(
    state: &Arc<AppState>,
    pod_id: String,
    driver_id: String,
    pricing_tier_id: String,
    auth_type: String,
    custom_price_paise: Option<u32>,
    custom_duration_minutes: Option<u32>,
    experience_id: Option<String>,
    custom_launch_args: Option<String>,
) -> Result<AuthTokenInfo, String> {
    // Cancel any existing pending token for this pod
    let existing = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM auth_tokens WHERE pod_id = ? AND status = 'pending'",
    )
    .bind(&pod_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    for (id,) in existing {
        let _ = cancel_auth_token(state, id).await;
    }

    // Verify driver exists and get name
    let driver = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Driver {} not found", driver_id))?;

    let driver_name = driver.1;

    // Verify pricing tier exists
    let tier = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT id, name, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| format!("Pricing tier {} not found", pricing_tier_id))?;

    let pricing_tier_name = tier.1;
    let duration_minutes = custom_duration_minutes.unwrap_or(tier.2 as u32);
    let allocated_seconds = duration_minutes * 60;

    // Generate token
    let token = match auth_type.as_str() {
        "pin" => {
            let pin: u32 = rand::thread_rng().gen_range(1000..=9999);
            format!("{:04}", pin)
        }
        "qr" => Uuid::new_v4().to_string(),
        _ => return Err("auth_type must be 'pin' or 'qr'".to_string()),
    };

    let token_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.config.auth.pin_expiry_secs as i64);

    // Insert into DB
    sqlx::query(
        "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?)",
    )
    .bind(&token_id)
    .bind(&pod_id)
    .bind(&driver_id)
    .bind(&pricing_tier_id)
    .bind(&auth_type)
    .bind(&token)
    .bind(custom_price_paise.map(|p| p as i64))
    .bind(custom_duration_minutes.map(|m| m as i64))
    .bind(&experience_id)
    .bind(&custom_launch_args)
    .bind(now.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB insert error: {}", e))?;

    let info = AuthTokenInfo {
        id: token_id.clone(),
        pod_id: pod_id.clone(),
        driver_id: driver_id.clone(),
        driver_name: driver_name.clone(),
        pricing_tier_id: pricing_tier_id.clone(),
        pricing_tier_name: pricing_tier_name.clone(),
        auth_type: auth_type.clone(),
        token: token.clone(),
        status: "pending".to_string(),
        allocated_seconds,
        custom_price_paise,
        custom_duration_minutes,
        created_at: now.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
    };

    // Send lock screen to agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let msg = match auth_type.as_str() {
            "pin" => CoreToAgentMessage::ShowPinLockScreen {
                token_id: token_id.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: pricing_tier_name.clone(),
                allocated_seconds,
            },
            _ => CoreToAgentMessage::ShowQrLockScreen {
                token_id: token_id.clone(),
                qr_payload: token.clone(),
                driver_name: driver_name.clone(),
                pricing_tier_name: pricing_tier_name.clone(),
                allocated_seconds,
            },
        };
        let _ = sender.send(msg).await;
    }
    drop(agent_senders);

    // Broadcast to dashboards
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCreated(info.clone()));

    tracing::info!(
        "Auth token created: {} ({}) for {} on pod {} (expires in {}s)",
        token_id,
        auth_type,
        driver_name,
        pod_id,
        state.config.auth.pin_expiry_secs
    );

    Ok(info)
}

// ─── Games That Need Staff Assistance (no auto-spawn) ─────────────────────

fn is_manual_launch_game(game: &str) -> bool {
    matches!(game, "f1_25" | "f1")
}

/// After billing starts, link reservation_id + wallet fields to the billing session.
async fn link_reservation_to_billing(
    state: &Arc<AppState>,
    billing_session_id: &str,
    driver_id: &str,
) {
    // Find active reservation for this driver
    if let Some(reservation) = pod_reservation::get_active_reservation_for_driver(state, driver_id).await {
        let _ = sqlx::query(
            "UPDATE billing_sessions SET reservation_id = ? WHERE id = ?",
        )
        .bind(&reservation.id)
        .bind(billing_session_id)
        .execute(&state.db)
        .await;

        // Touch reservation activity
        pod_reservation::touch_reservation(state, &reservation.id).await;
    }
}

/// Auto-launch game or show assistance screen depending on game type.
/// Returns the game name if an experience was linked.
pub(crate) async fn launch_or_assist(
    state: &Arc<AppState>,
    pod_id: &str,
    billing_session_id: &str,
    experience_id: &Option<String>,
    custom_launch_args: &Option<String>,
    driver_name: &str,
) -> Option<String> {
    // Determine game/track/car from either custom launch args or experience
    let (game, track, car, launch_args_json) = if let Some(custom_args) = custom_launch_args {
        // Custom booking — parse the stored launch_args JSON
        let parsed: serde_json::Value = serde_json::from_str(custom_args).ok()?;
        let game = parsed["game"].as_str().unwrap_or("assetto_corsa").to_string();
        let track = parsed["track"].as_str().unwrap_or("").to_string();
        let car = parsed["car"].as_str().unwrap_or("").to_string();
        (game, track, car, custom_args.clone())
    } else if let Some(exp_id) = experience_id.as_ref() {
        // Pre-defined experience
        let exp = sqlx::query_as::<_, (String, String, String)>(
            "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
        )
        .bind(exp_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()?;
        let launch_args = serde_json::json!({
            "car": exp.2,
            "track": exp.1,
            "driver": driver_name,
            "transmission": "auto",
            "aids": { "abs": 1, "tc": 1, "stability": 1, "autoclutch": 1, "ideal_line": 1 },
            "conditions": { "damage": 0 }
        })
        .to_string();
        (exp.0, exp.1, exp.2, launch_args)
    } else {
        return None;
    };

    // Look up billing session duration and inject into launch args
    let duration_minutes: u32 = sqlx::query_as::<_, (i64,)>(
        "SELECT allocated_seconds FROM billing_sessions WHERE id = ?",
    )
    .bind(billing_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(secs,)| (secs as u32) / 60)
    .unwrap_or(60);

    // Inject duration_minutes into launch_args JSON
    let launch_args_json = {
        let mut parsed: serde_json::Value = serde_json::from_str(&launch_args_json).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);
        parsed.to_string()
    };

    let sim_type = match game.as_str() {
        "assetto_corsa" | "ac" => rc_common::types::SimType::AssettoCorsa,
        "iracing" => rc_common::types::SimType::IRacing,
        "f1_25" | "f1" => rc_common::types::SimType::F125,
        "le_mans_ultimate" | "lmu" => rc_common::types::SimType::LeMansUltimate,
        "forza" => rc_common::types::SimType::Forza,
        _ => rc_common::types::SimType::AssettoCorsa,
    };

    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(pod_id) {
        if is_manual_launch_game(&game) {
            // F1 25 etc — show assistance screen, don't auto-launch
            let _ = sender
                .send(CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_name.to_string(),
                    message: "A team member is on the way to set up your session".to_string(),
                })
                .await;

            // Broadcast assistance needed to kiosk dashboards
            let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                pod_id: pod_id.to_string(),
                driver_name: driver_name.to_string(),
                game: game.clone(),
                reason: format!("{} requires manual launch", game),
            });

            tracing::info!(
                "Assistance needed for {} on pod {} (driver: {})",
                game, pod_id, driver_name
            );
        } else {
            // Auto-spawn game
            let _ = sender
                .send(CoreToAgentMessage::LaunchGame {
                    sim_type,
                    launch_args: Some(launch_args_json),
                })
                .await;

            tracing::info!(
                "Auto-launching {} on pod {} (car: {}, track: {})",
                game, pod_id, car, track
            );
        }
    }
    drop(agent_senders);

    // Update billing session with experience info
    let exp_id = experience_id.as_deref().unwrap_or("");
    let _ = sqlx::query(
        "UPDATE billing_sessions SET experience_id = ?, car = ?, track = ?, sim_type = ? WHERE id = ?",
    )
    .bind(exp_id)
    .bind(&car)
    .bind(&track)
    .bind(&game)
    .bind(billing_session_id)
    .execute(&state.db)
    .await;

    Some(game)
}

// ─── Validate PIN ──────────────────────────────────────────────────────────

pub async fn validate_pin(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
) -> Result<String, String> {
    // Check employee debug PIN first (4-digit daily rotating PIN)
    let daily_pin = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin == daily_pin {
        return validate_employee_pin(state, pod_id, pin).await;
    }

    // Atomically find and consume pending token (prevents double-spend race condition)
    let row = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&pod_id)
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid PIN or no pending assignment for this pod".to_string())?;

    let token_id = row.0;
    let driver_id = row.1;
    let pricing_tier_id = row.2;
    let custom_price_paise = row.3.map(|p| p as u32);
    let custom_duration_minutes = row.4.map(|m| m as u32);
    let experience_id = row.5;
    let custom_launch_args = row.6;

    // Start billing session
    let billing_session_id = billing::start_billing_session(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer PIN auth
    )
    .await?;

    // Finalize token as consumed with billing session ID
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to mark token {} as consumed: {}", token_id, e);
    }

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent (unless it'll be replaced by assistance screen)
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
    }
    drop(agent_senders);

    // Link reservation to billing session
    link_reservation_to_billing(state, &billing_session_id, &driver_id).await;

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!("PIN validated on pod {}, billing session {} started", pod_id, billing_session_id);

    Ok(billing_session_id)
}

// ─── Validate QR ───────────────────────────────────────────────────────────

pub async fn validate_qr(
    state: &Arc<AppState>,
    qr_token: String,
    driver_id: String,
) -> Result<String, String> {
    // Atomically find and consume pending QR token (prevents double-spend)
    let row = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE token = ? AND auth_type = 'qr' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&qr_token)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid or expired QR token".to_string())?;

    let token_id = row.0;
    let pod_id = row.1;
    let token_driver_id = row.2;
    let pricing_tier_id = row.3;
    let custom_price_paise = row.4.map(|p| p as u32);
    let custom_duration_minutes = row.5.map(|m| m as u32);
    let experience_id = row.6;
    let custom_launch_args = row.7;

    // Verify driver matches the assignment
    if token_driver_id != driver_id {
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ?")
            .bind(&token_id).execute(&state.db).await;
        return Err("QR token is not assigned to this customer".to_string());
    }

    // Start billing session
    let billing_session_id = billing::start_billing_session(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer QR auth
    )
    .await?;

    // Finalize token as consumed with billing session ID
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to mark token {} as consumed: {}", token_id, e);
    }

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
    }
    drop(agent_senders);

    // Link reservation to billing session
    link_reservation_to_billing(state, &billing_session_id, &driver_id).await;

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!("QR validated on pod {}, billing session {} started", pod_id, billing_session_id);

    Ok(billing_session_id)
}

// ─── Start Now (Staff Override) ───────────────────────────────────────────

/// Atomically consume a pending auth token and start billing without requiring PIN/QR.
/// Used by the kiosk "Start Now" button as a staff override.
pub async fn start_now(
    state: &Arc<AppState>,
    token_id: String,
) -> Result<String, String> {
    // Atomically find and consume the pending token (prevents double-spend)
    let row = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = ? AND status = 'pending'
         RETURNING pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&token_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Token not found or not pending".to_string())?;

    let pod_id = row.0;
    let driver_id = row.1;
    let pricing_tier_id = row.2;
    let custom_price_paise = row.3.map(|p| p as u32);
    let custom_duration_minutes = row.4.map(|m| m as u32);
    let experience_id = row.5;
    let custom_launch_args = row.6;

    // Start billing session
    let billing_session_id = billing::start_billing_session(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // PWA token auth
    )
    .await?;

    // Finalize token as consumed with billing session ID
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to mark token {} as consumed: {}", token_id, e);
    }

    // Get driver name for assistance screen
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Clear lock screen on agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
    }
    drop(agent_senders);

    // Link reservation to billing session
    link_reservation_to_billing(state, &billing_session_id, &driver_id).await;

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!("Start Now on pod {}: token {} consumed, billing session {} started", pod_id, token_id, billing_session_id);

    Ok(billing_session_id)
}

// ─── Cancel Auth Token ─────────────────────────────────────────────────────

pub async fn cancel_auth_token(
    state: &Arc<AppState>,
    token_id: String,
) -> Result<(), String> {
    // Get pod_id before updating
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT pod_id FROM auth_tokens WHERE id = ? AND status = 'pending'",
    )
    .bind(&token_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Token not found or not pending".to_string())?;

    let pod_id = row.0;

    // Update status
    sqlx::query("UPDATE auth_tokens SET status = 'cancelled' WHERE id = ?")
        .bind(&token_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

    // Clear lock screen on agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
    }
    drop(agent_senders);

    // Broadcast cleared event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCleared {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        reason: "cancelled".to_string(),
    });

    tracing::info!("Auth token {} cancelled for pod {}", token_id, pod_id);
    Ok(())
}

// ─── Expire Stale Tokens ───────────────────────────────────────────────────

pub async fn expire_stale_tokens(state: &Arc<AppState>) {
    let expired = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pod_id FROM auth_tokens WHERE status = 'pending' AND expires_at <= datetime('now')",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if expired.is_empty() {
        return;
    }

    for (token_id, pod_id) in &expired {
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'expired' WHERE id = ?")
            .bind(token_id)
            .execute(&state.db)
            .await;

        // Clear lock screen
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
        }
        drop(agent_senders);

        let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenCleared {
            token_id: token_id.clone(),
            pod_id: pod_id.clone(),
            reason: "expired".to_string(),
        });
    }

    tracing::info!("Expired {} stale auth tokens", expired.len());
}

// ─── Get Pending Tokens ────────────────────────────────────────────────────

pub async fn get_pending_tokens(state: &Arc<AppState>) -> Vec<AuthTokenInfo> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, Option<i64>, Option<i64>, String, String)>(
        "SELECT at.id, at.pod_id, at.driver_id, d.name, at.pricing_tier_id, pt.name, at.auth_type, at.token, at.custom_price_paise, at.custom_duration_minutes, at.created_at, at.expires_at
         FROM auth_tokens at
         JOIN drivers d ON at.driver_id = d.id
         JOIN pricing_tiers pt ON at.pricing_tier_id = pt.id
         WHERE at.status = 'pending' AND at.expires_at > datetime('now')
         ORDER BY at.created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let duration_query = "SELECT duration_minutes FROM pricing_tiers WHERE id = ?";

    let mut tokens = Vec::new();
    for r in rows {
        let duration_minutes = r.9.unwrap_or_else(|| {
            // Can't do async here, use a default
            0
        });

        let tier_duration = sqlx::query_as::<_, (i64,)>(duration_query)
            .bind(&r.4)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .map(|t| t.0 as u32)
            .unwrap_or(0);

        let actual_minutes = if duration_minutes > 0 {
            duration_minutes as u32
        } else {
            tier_duration
        };

        tokens.push(AuthTokenInfo {
            id: r.0,
            pod_id: r.1,
            driver_id: r.2,
            driver_name: r.3,
            pricing_tier_id: r.4,
            pricing_tier_name: r.5,
            auth_type: r.6,
            token: r.7,
            status: "pending".to_string(),
            allocated_seconds: actual_minutes * 60,
            custom_price_paise: r.8.map(|p| p as u32),
            custom_duration_minutes: r.9.map(|m| m as u32),
            created_at: r.10,
            expires_at: r.11,
        });
    }

    tokens
}

// ─── JWT Helpers ───────────────────────────────────────────────────────────

pub fn create_jwt(driver_id: &str, secret: &str) -> Result<String, String> {
    let now = Utc::now();
    let exp = now + Duration::days(30);

    let claims = Claims {
        sub: driver_id.to_string(),
        iat: now.timestamp() as usize,
        exp: exp.timestamp() as usize,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| format!("JWT encode error: {}", e))
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<String, String> {
    let data = jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| format!("JWT decode error: {}", e))?;

    Ok(data.claims.sub)
}

// ─── OTP ───────────────────────────────────────────────────────────────────

pub async fn send_otp(state: &Arc<AppState>, phone: &str) -> Result<String, String> {
    // Find or create driver by phone
    let driver = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM drivers WHERE phone = ?",
    )
    .bind(phone)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let driver_id = match driver {
        Some((id, _)) => id,
        None => {
            // Auto-create driver with phone + generate customer_id
            let id = Uuid::new_v4().to_string();

            // Get next customer_id sequence number (numeric MAX to avoid lexicographic issues)
            let max_num = sqlx::query_as::<_, (Option<i64>,)>(
                "SELECT MAX(CAST(REPLACE(customer_id, 'RP', '') AS INTEGER)) FROM drivers WHERE customer_id IS NOT NULL AND customer_id LIKE 'RP%'",
            )
            .fetch_one(&state.db)
            .await
            .ok()
            .and_then(|r| r.0)
            .unwrap_or(0) as u32;
            let customer_id = format!("RP{:03}", max_num + 1);

            sqlx::query(
                "INSERT INTO drivers (id, name, phone, customer_id, updated_at) VALUES (?, ?, ?, ?, datetime('now'))",
            )
            .bind(&id)
            .bind(format!("Customer {}", &phone[phone.len().saturating_sub(4)..]))
            .bind(phone)
            .bind(&customer_id)
            .execute(&state.db)
            .await
            .map_err(|e| format!("DB error creating driver: {}", e))?;
            tracing::info!("New customer {} assigned ID {}", id, customer_id);
            id
        }
    };

    // Generate 6-digit OTP
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // Store OTP in driver record
    sqlx::query("UPDATE drivers SET otp_code = ?, otp_expires_at = ? WHERE id = ?")
        .bind(&otp_str)
        .bind(expires_at.to_rfc3339())
        .bind(&driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error storing OTP: {}", e))?;

    // Send OTP via WhatsApp (Evolution API)
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

        let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
        let body = serde_json::json!({
            "number": wa_phone,
            "text": format!("🏎️ *RacingPoint*\n\nYour login code is: *{}*\n\nValid for {} minutes.", otp_str, state.config.auth.otp_expiry_secs / 60)
        });

        let client = reqwest::Client::new();
        match client.post(&url).header("apikey", evo_key).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("OTP sent via WhatsApp to {}", wa_phone);
            }
            Ok(resp) => {
                tracing::warn!("Evolution API returned {}: OTP for {} is {}", resp.status(), phone, otp_str);
            }
            Err(e) => {
                tracing::warn!("Failed to send OTP via WhatsApp: {}. OTP for {} is {}", e, phone, otp_str);
            }
        }
    } else {
        tracing::info!("OTP for phone {}: {} (Evolution API not configured)", phone, otp_str);
    }

    Ok(driver_id)
}

pub async fn verify_otp(state: &Arc<AppState>, phone: &str, otp: &str) -> Result<String, String> {
    let driver = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone = ?",
    )
    .bind(phone)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Phone number not found".to_string())?;

    let driver_id = driver.0;
    let stored_otp = driver.1.ok_or_else(|| "No OTP pending".to_string())?;
    let expires_at = driver.2.ok_or_else(|| "No OTP pending".to_string())?;

    // Check expiry
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map_err(|_| "Invalid expiry timestamp".to_string())?;
    if Utc::now() > expires {
        return Err("OTP has expired".to_string());
    }

    // Verify OTP
    if stored_otp != otp {
        return Err("Invalid OTP".to_string());
    }

    // Clear OTP and update login timestamp
    sqlx::query(
        "UPDATE drivers SET otp_code = NULL, otp_expires_at = NULL, phone_verified = 1, last_login_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Create JWT
    let jwt = create_jwt(&driver_id, &state.config.auth.jwt_secret)?;

    // Record customer session
    let session_id = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + Duration::days(30);

    sqlx::query(
        "INSERT INTO customer_sessions (id, driver_id, token_hash, created_at, expires_at)
         VALUES (?, ?, ?, datetime('now'), ?)",
    )
    .bind(&session_id)
    .bind(&driver_id)
    .bind(&jwt[jwt.len().saturating_sub(32)..]) // store last 32 chars as hash
    .bind(expires_at.to_rfc3339())
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating session: {}", e))?;

    tracing::info!("Customer {} verified OTP and logged in", driver_id);

    Ok(jwt)
}

// ─── Handle Dashboard Commands ─────────────────────────────────────────────

pub async fn handle_dashboard_command(
    state: &Arc<AppState>,
    cmd: rc_common::protocol::DashboardCommand,
) {
    match cmd {
        rc_common::protocol::DashboardCommand::AssignCustomer {
            pod_id,
            driver_id,
            pricing_tier_id,
            auth_type,
            custom_price_paise,
            custom_duration_minutes,
        } => {
            if let Err(e) = create_auth_token(
                state,
                pod_id,
                driver_id,
                pricing_tier_id,
                auth_type,
                custom_price_paise,
                custom_duration_minutes,
                None, // experience_id — set via REST API
                None, // custom_launch_args — set via REST API
            )
            .await
            {
                tracing::error!("Failed to assign customer: {}", e);
            }
        }
        rc_common::protocol::DashboardCommand::CancelAssignment { token_id } => {
            if let Err(e) = cancel_auth_token(state, token_id).await {
                tracing::error!("Failed to cancel assignment: {}", e);
            }
        }
        rc_common::protocol::DashboardCommand::AcknowledgeAssistance { pod_id } => {
            tracing::info!("Staff acknowledged assistance for pod {}", pod_id);
            // Clear the assistance screen on the agent
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
            }
        }
        _ => {}
    }
}

// ─── Handle Agent PIN Entry ────────────────────────────────────────────────

pub async fn handle_pin_entered(state: &Arc<AppState>, pod_id: String, pin: String) {
    match validate_pin(state, pod_id.clone(), pin).await {
        Ok(billing_session_id) => {
            tracing::info!(
                "PIN auth success on pod {}: billing session {}",
                pod_id,
                billing_session_id
            );
        }
        Err(e) => {
            tracing::warn!("PIN auth failed on pod {}: {}", pod_id, e);
        }
    }
}

// ─── Kiosk PIN Validation (no pod_id required) ───────────────────────────

#[derive(Debug, Serialize)]
pub struct KioskPinResult {
    pub billing_session_id: String,
    pub pod_id: String,
    pub pod_number: u32,
    pub driver_name: String,
    pub pricing_tier_name: String,
    pub allocated_seconds: u32,
}

pub async fn validate_pin_kiosk(
    state: &Arc<AppState>,
    pin: String,
) -> Result<KioskPinResult, String> {
    // Atomically find and consume ANY pending pin token matching this PIN (prevents double-spend)
    let row = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
        "UPDATE auth_tokens SET status = 'consuming'
         WHERE id = (
             SELECT id FROM auth_tokens
             WHERE token = ? AND auth_type = 'pin' AND status = 'pending'
               AND expires_at > datetime('now')
             LIMIT 1
         ) AND status = 'pending'
         RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
    )
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid PIN. Please check with reception.".to_string())?;

    let token_id = row.0;
    let pod_id = row.1;
    let driver_id = row.2;
    let pricing_tier_id = row.3.clone();
    let custom_price_paise = row.4.map(|p| p as u32);
    let custom_duration_minutes = row.5.map(|m| m as u32);
    let experience_id = row.6;
    let custom_launch_args = row.7;

    // Start billing session
    let billing_session_id = billing::start_billing_session(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id.clone(),
        custom_price_paise,
        custom_duration_minutes,
        None, // kiosk PIN validation
    )
    .await?;

    // Finalize token as consumed with billing session ID
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&state.db)
    .await
    {
        tracing::error!("Failed to mark token {} as consumed: {}", token_id, e);
    }

    // Get driver name
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // Get pricing tier name and duration
    let tier_row = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT name, duration_minutes FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let pricing_tier_name = tier_row
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "Session".to_string());

    let allocated_seconds = custom_duration_minutes
        .map(|m| m * 60)
        .or_else(|| tier_row.as_ref().and_then(|r| r.1.map(|m| m as u32 * 60)))
        .unwrap_or(0);

    // Get pod number
    let pod_number: i64 = sqlx::query_scalar("SELECT number FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0);

    // Clear lock screen on agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
    }
    drop(agent_senders);

    // Link reservation to billing session
    link_reservation_to_billing(state, &billing_session_id, &driver_id).await;

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!(
        "Kiosk PIN validated: pod {} (#{}) driver {}, billing session {}",
        pod_id, pod_number, driver_name, billing_session_id
    );

    Ok(KioskPinResult {
        billing_session_id,
        pod_id,
        pod_number: pod_number as u32,
        driver_name,
        pricing_tier_name,
        allocated_seconds,
    })
}

// ─── Employee Debug PIN ──────────────────────────────────────────────────

/// Generate a deterministic 4-digit daily PIN for employees.
/// PIN = hash(secret + "YYYY-MM-DD") mod 10_000, formatted as 4 digits.
/// Changes at midnight UTC each day.
pub fn generate_daily_pin(secret: &str, date: &str) -> String {
    let input = format!("{}-employee-debug-{}", secret, date);
    // Simple hash: sum bytes with position-weighted mixing
    let mut hash: u64 = 0;
    for (i, b) in input.bytes().enumerate() {
        hash = hash.wrapping_mul(31).wrapping_add(b as u64).wrapping_add(i as u64);
    }
    // Mix further to avoid patterns
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x45d9f3b);
    hash ^= hash >> 16;
    // 4-digit PIN (1000-9999 range to avoid leading zeros confusion)
    let pin = (hash % 9000 + 1000) as u32;
    format!("{:04}", pin)
}

/// Get today's employee debug PIN
pub fn todays_debug_pin(secret: &str) -> String {
    let today = Utc::now().format("%Y-%m-%d").to_string();
    generate_daily_pin(secret, &today)
}

/// Validate an employee debug PIN on a specific pod.
/// If valid: clears lock screen, enters debug mode, no billing.
pub async fn validate_employee_pin(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
) -> Result<String, String> {
    let expected = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin != expected {
        return Err("Invalid employee PIN".to_string());
    }

    // Clear lock screen and enter debug mode
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
        let _ = sender.send(CoreToAgentMessage::EnterDebugMode {
            employee_name: "Staff".to_string(),
        }).await;
    }
    drop(agent_senders);

    tracing::info!("Employee debug PIN validated on pod {}", pod_id);

    Ok("debug_mode".to_string())
}

/// Validate employee debug PIN from kiosk (no pod_id — unlock a specific pod chosen by staff).
pub async fn validate_employee_pin_kiosk(
    state: &Arc<AppState>,
    pin: String,
    pod_id: Option<String>,
) -> Result<String, String> {
    let expected = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin != expected {
        return Err("Invalid employee PIN".to_string());
    }

    // If pod_id specified, enter debug mode on that pod
    if let Some(ref pid) = pod_id {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pid) {
            let _ = sender.send(CoreToAgentMessage::ClearLockScreen).await;
            let _ = sender.send(CoreToAgentMessage::EnterDebugMode {
                employee_name: "Staff".to_string(),
            }).await;
        }
        drop(agent_senders);
        tracing::info!("Employee debug mode on pod {} (kiosk)", pid);
    }

    Ok("debug_mode".to_string())
}

/// Check if a driver is an employee
pub async fn is_employee(state: &Arc<AppState>, driver_id: &str) -> bool {
    sqlx::query_scalar::<_, bool>("SELECT COALESCE(is_employee, 0) FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}
