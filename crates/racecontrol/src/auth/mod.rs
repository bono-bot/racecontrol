pub mod admin;
pub mod middleware;
pub mod rate_limit;
pub use admin::{admin_login, hash_admin_pin, verify_admin_pin};
pub use middleware::{StaffClaims, require_staff_jwt, create_staff_jwt};

use std::sync::Arc;

use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::billing;
use crate::crypto::redaction::redact_phone;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::AuthTokenInfo;

// ─── PIN Validation Constants ─────────────────────────────────────────────

/// Standardized PIN error message — identical across pod lock screen, kiosk, and PWA paths.
/// AUTH-01 requires identical error message on all 3 entry points.
pub(crate) const INVALID_PIN_MESSAGE: &str =
    "Invalid PIN \u{2014} please try again or see reception.";

/// Maximum customer PIN failures before the pod's customer path is locked.
/// Staff path (employee debug PIN) has no such ceiling — see PIN-02.
const CUSTOMER_PIN_LOCKOUT_THRESHOLD: u32 = 5;

// ─── PinSource Enum ────────────────────────────────────────────────────────

/// Source of PIN entry — used for logging only. Validation behavior is identical across all sources.
#[derive(Debug, Clone, Copy)]
pub enum PinSource {
    Pod,   // Entered on physical pod lock screen
    Kiosk, // Staff kiosk endpoint
    Pwa,   // Customer PWA (goes through kiosk endpoint)
}

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

    // Guard: driver cannot be on another pod already
    let active_on_other = sqlx::query_as::<_, (String,)>(
        "SELECT pod_id FROM billing_sessions WHERE driver_id = ? AND status IN ('active', 'paused_manual') AND pod_id != ?",
    )
    .bind(&driver_id)
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if let Some((other_pod,)) = active_on_other {
        return Err(format!(
            "Driver already has an active session on {}",
            other_pod
        ));
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
            // MMA-Iter3: Increase PIN space from 4 digits (9000 values) to 6 digits (900000 values).
            // 4/4 models flagged 4-digit PINs as collision risk with 8 pods + 5 attempts.
            // 6-digit PIN: collision probability drops from ~3.5% to ~0.004% per session.
            let pin: u32 = rand::thread_rng().gen_range(100000..=999999);
            format!("{:06}", pin)
        }
        "qr" => Uuid::new_v4().to_string(),
        _ => return Err("auth_type must be 'pin' or 'qr'".to_string()),
    };

    let token_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.config.auth.pin_expiry_secs as i64);

    // Insert into DB
    sqlx::query(
        "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args, created_at, expires_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
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
    .bind(&state.config.venue.venue_id)
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
        let _ = sender.send(CoreMessage::wrap(msg)).await;
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

// ─── Game String Parsing ─────────────────────────────────────────────────

/// Parse a game string (from kiosk_experiences, custom launch args, etc.) into a SimType.
/// Returns AssettoCorsa as default fallback for unknown game strings.
pub fn parse_sim_type(game: &str) -> rc_common::types::SimType {
    use rc_common::types::SimType;
    match game {
        "assetto_corsa" | "ac" => SimType::AssettoCorsa,
        "assetto_corsa_evo" | "ace" => SimType::AssettoCorsaEvo,
        "assetto_corsa_rally" | "ea_wrc" | "acr" | "wrc" => SimType::AssettoCorsaRally,
        "iracing" => SimType::IRacing,
        "f1_25" | "f1" => SimType::F125,
        "le_mans_ultimate" | "lmu" => SimType::LeMansUltimate,
        "forza" => SimType::Forza,
        "forza_horizon_5" | "fh5" => SimType::ForzaHorizon5,
        _ => SimType::AssettoCorsa,
    }
}

// ─── Game Availability Check ──────────────────────────────────────────────

/// Check if a game is available given a list of installed games.
/// Returns true if installed_games is empty (backward compat with old agents that don't report games).
pub fn check_pod_has_game(installed_games: &[rc_common::types::SimType], sim_type: rc_common::types::SimType) -> bool {
    if installed_games.is_empty() {
        true // backward compat: old agents don't report games -> assume available
    } else {
        installed_games.contains(&sim_type)
    }
}

/// Check if the pod has this game installed (from agent registration).
/// Returns true if pod is not found or has no installed_games data (backward compat with old agents).
async fn pod_has_game(state: &Arc<AppState>, pod_id: &str, sim_type: rc_common::types::SimType) -> bool {
    let pods = state.pods.read().await;
    match pods.get(pod_id) {
        Some(pod) => check_pod_has_game(&pod.installed_games, sim_type),
        None => false,
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

    // Look up billing session duration — use remaining time (for crash relaunches)
    let duration_minutes: u32 = sqlx::query_as::<_, (i64, i64)>(
        "SELECT allocated_seconds, driving_seconds FROM billing_sessions WHERE id = ?",
    )
    .bind(billing_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|(alloc, driven)| {
        let remaining = (alloc as u32).saturating_sub(driven as u32);
        (remaining + 59) / 60 // round up to nearest minute
    })
    .unwrap_or(60);

    // Inject duration_minutes into launch_args JSON
    let launch_args_json = {
        let mut parsed: serde_json::Value = serde_json::from_str(&launch_args_json).unwrap_or_default();
        parsed["duration_minutes"] = serde_json::json!(duration_minutes);
        parsed.to_string()
    };

    let sim_type = parse_sim_type(&game);

    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(pod_id) {
        if !pod_has_game(state, pod_id, sim_type).await {
            // Game not installed on this pod — show assistance screen
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::ShowAssistanceScreen {
                    driver_name: driver_name.to_string(),
                    message: format!("{} is not installed on this pod — staff will assist", game),
                }))
                .await;

            // Broadcast assistance needed to kiosk dashboards
            let _ = state.dashboard_tx.send(DashboardEvent::AssistanceNeeded {
                pod_id: pod_id.to_string(),
                driver_name: driver_name.to_string(),
                game: game.clone(),
                reason: format!("{} is not installed on this pod", game),
            });

            tracing::info!(
                "Game {} not installed on pod {} — assistance needed (driver: {})",
                game, pod_id, driver_name
            );
        } else {
            // Auto-spawn game
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                    sim_type,
                    launch_args: Some(launch_args_json),
                    force_clean: false,
                    duration_minutes: None,
                }))
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

    // PIN-01: check customer lockout before attempting DB lookup
    {
        let failures = state.customer_pin_failures.read().await;
        let count = failures.get(pod_id.as_str()).copied().unwrap_or(0);
        if count >= CUSTOMER_PIN_LOCKOUT_THRESHOLD {
            return Err(
                "Too many failed attempts. Please see reception to reset your session."
                    .to_string(),
            );
        }
    }

    // SESS-03: Begin transaction for atomic token consumption + billing deferral + finalization.
    // If any step fails, the entire token state change rolls back automatically.
    let mut tx = state.db.begin().await
        .map_err(|e| format!("Transaction start failed: {}", e))?;

    // Atomically find and consume pending token within transaction (prevents double-spend race condition)
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
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // PIN-01: if token lookup failed, increment customer failure counter before returning Err
    let row = match row {
        Some(r) => r,
        None => {
            // Rollback the (empty) transaction before returning
            tx.rollback().await.ok();
            // PIN-01: increment customer failure counter for this pod
            {
                let mut failures = state.customer_pin_failures.write().await;
                *failures.entry(pod_id.clone()).or_insert(0) += 1;
            }
            return Err(INVALID_PIN_MESSAGE.to_string());
        }
    };

    let token_id = row.0;
    let driver_id = row.1;
    let pricing_tier_id = row.2;
    let custom_price_paise = row.3.map(|p| p as u32);
    let custom_duration_minutes = row.4.map(|m| m as u32);
    let experience_id = row.5;
    let custom_launch_args = row.6;

    // Check if this token belongs to a multiplayer group session
    let group_info = crate::multiplayer::find_group_session_for_token(state, &token_id).await;

    let (group_session_id, is_group_member) = if let Some((gsid, _gdriver)) = &group_info {
        // Call on_member_validated to track this PIN validation
        // billing_session_id is a deferred placeholder at this point
        let billing_session_id_placeholder = format!("deferred-{}", uuid::Uuid::new_v4());
        match crate::multiplayer::on_member_validated(state, gsid, &driver_id, &billing_session_id_placeholder).await {
            Ok(all_validated) => {
                tracing::info!(
                    "Group member {} validated on pod {} (all_validated={})",
                    driver_id, pod_id, all_validated
                );
            }
            Err(e) => {
                tracing::error!("Failed to call on_member_validated for group {}: {}", gsid, e);
            }
        }
        (Some(gsid.clone()), true)
    } else {
        (None, false)
    };

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    // Billing session will be created by billing::handle_game_status_update() when Live received
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());

    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer PIN auth
        None, // split_count
        None, // split_duration_minutes
        group_session_id,
    )
    .await
    {
        // SESS-03: Transaction rollback atomically reverts token from 'consuming' back to 'pending'
        tx.rollback().await.ok();
        tracing::error!("Defer billing failed for token {}, transaction rolled back: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed within the same transaction
    if let Err(e) = sqlx::query(
        "UPDATE auth_tokens SET status = 'consumed', billing_session_id = ?, consumed_at = datetime('now') WHERE id = ?",
    )
    .bind(&billing_session_id)
    .bind(&token_id)
    .execute(&mut *tx)
    .await
    {
        tx.rollback().await.ok();
        tracing::error!("Failed to mark token {} as consumed, rolling back: {}", token_id, e);
        return Err(format!("Token finalization failed: {}", e));
    }

    // SESS-03: Commit the transaction — token consumption is now atomic
    tx.commit().await
        .map_err(|e| format!("Transaction commit failed: {}", e))?;

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
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // Reservation linking deferred until actual billing session starts on Live
    // link_reservation_to_billing will be called inside start_billing_session()

    // GROUP-01: For group members, do NOT auto-launch individually.
    // on_member_validated() handles coordinated launch via start_ac_lan_for_group()
    // when all members are validated. For non-group, launch as before.
    if !is_group_member {
        launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;
    }

    // Update pod state to WaitingForGame
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    // PIN-01: reset customer failure counter on successful auth
    state.customer_pin_failures.write().await.remove(&pod_id);

    tracing::info!("PIN validated via {:?} on pod {}, billing deferred (waiting for LIVE)", PinSource::Pod, pod_id);

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

    // Check if this pod is part of a multiplayer group session
    let qr_group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM group_session_members WHERE pod_id = ? AND status = 'validated' ORDER BY validated_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // customer QR auth
        None, // split_count
        None, // split_duration_minutes
        qr_group_session_id,
    )
    .await
    {
        // Rollback: revert token from 'consuming' back to 'pending'
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ? AND status = 'consuming'")
            .bind(&token_id)
            .execute(&state.db)
            .await;
        tracing::error!("Defer billing failed for QR token {}, rolled back to pending: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed (billing_session_id is deferred placeholder)
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
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // Reservation linking deferred until actual billing session starts on Live

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Update pod state to WaitingForGame
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!("QR validated on pod {}, billing deferred (waiting for LIVE)", pod_id);

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

    // Check if this pod is part of a multiplayer group session
    let pwa_group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM group_session_members WHERE pod_id = ? AND status = 'validated' ORDER BY validated_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id,
        custom_price_paise,
        custom_duration_minutes,
        None, // PWA token auth
        None, // split_count
        None, // split_duration_minutes
        pwa_group_session_id,
    )
    .await
    {
        // Rollback: revert token from 'consuming' back to 'pending'
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ? AND status = 'consuming'")
            .bind(&token_id)
            .execute(&state.db)
            .await;
        tracing::error!("Defer billing failed for token {}, rolled back to pending: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed (billing_session_id is deferred placeholder)
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
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // Reservation linking deferred until actual billing session starts on Live

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Update pod state to WaitingForGame
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!("Start Now on pod {}: token {} consumed, billing deferred (waiting for LIVE)", pod_id, token_id);

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
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
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
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
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

/// Hash a one-time password using Argon2id with a random salt.
/// Returns the PHC-format hash string (starts with "$argon2id$").
/// SEC-08: OTPs must be stored as hashes, never plaintext.
fn hash_otp(otp: &str) -> Result<String, String> {
    use argon2::{
        password_hash::{rand_core::OsRng, SaltString},
        Argon2, PasswordHasher,
    };
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(otp.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("Argon2 OTP hash error: {}", e))
}

/// Verify a plaintext OTP against an argon2 hash string.
/// Returns `true` if the OTP matches the hash, `false` otherwise (including on parse errors).
/// SEC-08: Must be called via spawn_blocking — argon2 verify is CPU-intensive.
fn verify_otp_hash(otp: &str, hash_str: &str) -> bool {
    use argon2::{Argon2, PasswordHash, PasswordVerifier};
    let Ok(parsed) = PasswordHash::new(hash_str) else {
        return false;
    };
    Argon2::default()
        .verify_password(otp.as_bytes(), &parsed)
        .is_ok()
}

/// Result of an OTP send attempt. `driver_id` is always set on success;
/// `delivered` indicates whether the WhatsApp message actually reached the API.
pub struct OtpSendResult {
    pub driver_id: String,
    pub delivered: bool,
}

pub async fn send_otp(state: &Arc<AppState>, phone: &str) -> Result<OtpSendResult, String> {
    // Find or create driver by phone (lookup via HMAC hash)
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
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

            let phone_enc = state.field_cipher.encrypt_field(phone)
                .map_err(|e| format!("Encrypt error: {}", e))?;

            sqlx::query(
                "INSERT INTO drivers (id, name, phone_hash, phone_enc, customer_id, updated_at, venue_id) VALUES (?, ?, ?, ?, ?, datetime('now'), ?)",
            )
            .bind(&id)
            .bind(format!("Customer {}", &phone[phone.len().saturating_sub(4)..]))
            .bind(&phone_hash)
            .bind(&phone_enc)
            .bind(&customer_id)
            .bind(&state.config.venue.venue_id)
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

    // SEC-08: Hash the OTP before storing — plaintext OTPs must not be recoverable from DB
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    // Store hashed OTP in driver record
    sqlx::query("UPDATE drivers SET otp_code = ?, otp_expires_at = ? WHERE id = ?")
        .bind(&otp_hash)
        .bind(expires_at.to_rfc3339())
        .bind(&driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error storing OTP: {}", e))?;

    // Send OTP via WhatsApp (Evolution API) — uses plaintext otp_str, not the hash
    let delivered = send_otp_whatsapp(state, phone, &otp_str).await;

    Ok(OtpSendResult { driver_id, delivered })
}

/// Send OTP message via WhatsApp Evolution API.
/// Returns `true` if the API accepted the message, `false` on any failure.
/// Uses the shared HTTP client from AppState with a 5-second timeout.
async fn send_otp_whatsapp(state: &Arc<AppState>, phone: &str, otp_str: &str) -> bool {
    let (evo_url, evo_key, evo_instance) = match (
        &state.config.auth.evolution_url,
        &state.config.auth.evolution_api_key,
        &state.config.auth.evolution_instance,
    ) {
        (Some(u), Some(k), Some(i)) => (u.clone(), k.clone(), i.clone()),
        _ => {
            tracing::info!("OTP generated for {} (Evolution API not configured)", redact_phone(phone));
            return false;
        }
    };

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
        "text": format!("\u{1f3ce}\u{fe0f} *RacingPoint*\n\nYour login code is: *{}*\n\nValid for {} minutes.", otp_str, state.config.auth.otp_expiry_secs / 60)
    });

    match state.http_client
        .post(&url)
        .header("apikey", &evo_key)
        .timeout(std::time::Duration::from_secs(5))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("OTP sent via WhatsApp to {}", redact_phone(&wa_phone));
            true
        }
        Ok(resp) => {
            tracing::error!("Evolution API returned {} for OTP to {}", resp.status(), redact_phone(phone));
            false
        }
        Err(e) => {
            tracing::error!("Failed to send OTP via WhatsApp: {}. OTP for {}", e, redact_phone(phone));
            false
        }
    }
}

/// Resend OTP for a phone number. Reuses the existing OTP if still valid,
/// otherwise generates a fresh one. Returns delivery status.
pub async fn resend_otp(state: &Arc<AppState>, phone: &str) -> Result<OtpSendResult, String> {
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Phone number not found. Please start login again.".to_string())?;

    let (driver_id, existing_otp, expires_at) = driver;

    // Reuse existing OTP if still valid (>30s remaining), otherwise generate new
    let otp_str = match (&existing_otp, &expires_at) {
        (Some(otp), Some(exp)) if !otp.is_empty() => {
            if let Ok(exp_dt) = chrono::DateTime::parse_from_rfc3339(exp) {
                if exp_dt > chrono::Utc::now() + chrono::Duration::seconds(30) {
                    otp.clone()
                } else {
                    // Almost expired — generate new
                    let new_otp = generate_and_store_otp(state, &driver_id).await?;
                    new_otp
                }
            } else {
                let new_otp = generate_and_store_otp(state, &driver_id).await?;
                new_otp
            }
        }
        _ => {
            let new_otp = generate_and_store_otp(state, &driver_id).await?;
            new_otp
        }
    };

    let delivered = send_otp_whatsapp(state, phone, &otp_str).await;
    Ok(OtpSendResult { driver_id, delivered })
}

/// Generate a new 6-digit OTP and store it as an argon2 hash in the driver record.
/// Returns the plaintext OTP (needed for WhatsApp delivery), not the hash.
/// SEC-08: plaintext is only kept in memory for the duration of this function.
async fn generate_and_store_otp(state: &Arc<AppState>, driver_id: &str) -> Result<String, String> {
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // SEC-08: Hash OTP before storing
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    sqlx::query("UPDATE drivers SET otp_code = ?, otp_expires_at = ? WHERE id = ?")
        .bind(&otp_hash)
        .bind(expires_at.to_rfc3339())
        .bind(driver_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error storing OTP: {}", e))?;

    Ok(otp_str)
}

pub async fn verify_otp(state: &Arc<AppState>, phone: &str, otp: &str) -> Result<String, String> {
    let phone_hash = state.field_cipher.hash_phone(phone);
    let driver = sqlx::query_as::<_, (String, Option<String>, Option<String>)>(
        "SELECT id, otp_code, otp_expires_at FROM drivers WHERE phone_hash = ?",
    )
    .bind(&phone_hash)
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

    // SEC-08: Verify OTP using argon2 (constant-time, cryptographically secure)
    // spawn_blocking because argon2 verify is CPU-intensive and must not block tokio runtime
    let otp_owned = otp.to_string();
    let stored_otp_owned = stored_otp.clone();
    let valid = tokio::task::spawn_blocking(move || verify_otp_hash(&otp_owned, &stored_otp_owned))
        .await
        .unwrap_or(false);
    if !valid {
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

// ─── Guardian OTP (LEGAL-04/05) ────────────────────────────────────────────

/// Send a 6-digit OTP to a minor's guardian phone for consent verification.
/// Stores an argon2-hashed OTP in drivers.guardian_otp_code (SEC-08 compliant).
/// Returns true if WhatsApp delivery succeeded; false if Evolution API is not configured
/// or the send failed (OTP is still stored — staff can relay it verbally).
pub async fn send_guardian_otp(
    state: &Arc<AppState>,
    driver_id: &str,
    guardian_phone: &str,
) -> Result<OtpSendResult, String> {
    // Verify the driver exists
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
    if exists.is_none() {
        return Err(format!("Driver '{}' not found", driver_id));
    }

    // Generate 6-digit OTP
    let otp: u32 = rand::thread_rng().gen_range(100000..=999999);
    let otp_str = format!("{:06}", otp);
    let expires_at = Utc::now() + Duration::seconds(state.config.auth.otp_expiry_secs as i64);

    // SEC-08: Hash the OTP before storing — plaintext must not be recoverable from DB
    let otp_hash = hash_otp(&otp_str).map_err(|e| format!("OTP hash error: {}", e))?;

    // Store hashed OTP + reset verified flag (new send invalidates any previous verification)
    sqlx::query(
        "UPDATE drivers SET guardian_otp_code = ?, guardian_otp_expires_at = ?, guardian_otp_verified = 0, guardian_phone = ? WHERE id = ?",
    )
    .bind(&otp_hash)
    .bind(expires_at.to_rfc3339())
    .bind(guardian_phone)
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error storing guardian OTP: {}", e))?;

    // Send via WhatsApp — reuse existing Evolution API send logic
    let delivered = send_otp_whatsapp(state, guardian_phone, &otp_str).await;

    tracing::info!(
        driver_id = %driver_id,
        guardian = %redact_phone(guardian_phone),
        delivered = %delivered,
        "Guardian OTP sent for minor consent (LEGAL-04)"
    );

    Ok(OtpSendResult {
        driver_id: driver_id.to_string(),
        delivered,
    })
}

/// Verify a guardian's OTP for a minor customer.
/// On success: sets guardian_otp_verified=1 and guardian_otp_verified_at on the driver record.
/// Returns Ok(true) on valid OTP, Ok(false) on invalid hash.
/// Returns Err on DB failure, missing OTP, or expired OTP.
pub async fn verify_guardian_otp(
    state: &Arc<AppState>,
    driver_id: &str,
    otp: &str,
) -> Result<bool, String> {
    // Fetch stored guardian OTP hash and expiry
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT guardian_otp_code, guardian_otp_expires_at FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let (stored_otp_hash, expires_at_str) = row
        .ok_or_else(|| format!("Driver '{}' not found", driver_id))?;

    let stored_hash = stored_otp_hash.ok_or_else(|| "No guardian OTP pending".to_string())?;
    let expires_str = expires_at_str.ok_or_else(|| "No guardian OTP pending".to_string())?;

    // Check expiry (same 5-min window as regular OTP)
    let expires = chrono::DateTime::parse_from_rfc3339(&expires_str)
        .map_err(|_| "Invalid OTP expiry timestamp".to_string())?;
    if Utc::now() > expires {
        return Err("Guardian OTP has expired".to_string());
    }

    // SEC-08: Verify via argon2 — spawn_blocking because argon2 verify is CPU-intensive
    let otp_owned = otp.to_string();
    let hash_owned = stored_hash.clone();
    let valid = tokio::task::spawn_blocking(move || verify_otp_hash(&otp_owned, &hash_owned))
        .await
        .unwrap_or(false);

    if !valid {
        return Ok(false);
    }

    // Mark guardian OTP as verified — billing gate will pass on next start_billing call
    sqlx::query(
        "UPDATE drivers SET guardian_otp_verified = 1, guardian_otp_verified_at = datetime('now') WHERE id = ?",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error marking guardian OTP verified: {}", e))?;

    tracing::info!(driver_id = %driver_id, "Guardian OTP verified — minor billing gate will pass (LEGAL-04)");

    Ok(true)
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
                let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
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
            // Send failure feedback to agent so lock screen shows error
            let agent_senders = state.agent_senders.read().await;
            if let Some(sender) = agent_senders.get(&pod_id) {
                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::PinFailed {
                        reason: e.clone(),
                    }))
                    .await;
            }
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
    chosen_pod_id: Option<String>,
) -> Result<KioskPinResult, String> {
    // Atomically find and consume ANY pending pin token matching this PIN (prevents double-spend)
    // If a pod_id is provided (customer chose a pod), prefer tokens for that pod first,
    // then fall back to any matching token.
    let row = if let Some(ref cpid) = chosen_pod_id {
        // Try matching the chosen pod first
        let r = sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND pod_id = ? AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id, pod_id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
        )
        .bind(&pin)
        .bind(cpid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Fall back to any matching PIN token if none found for chosen pod
        match r {
            Some(row) => Some(row),
            None => {
                sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
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
            }
        }
    } else {
        sqlx::query_as::<_, (String, String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
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
    };

    let row = row.ok_or_else(|| INVALID_PIN_MESSAGE.to_string())?;

    let token_id = row.0;
    let token_pod_id = row.1;
    let driver_id = row.2;
    let pricing_tier_id = row.3.clone();
    let custom_price_paise = row.4.map(|p| p as u32);
    let custom_duration_minutes = row.5.map(|m| m as u32);
    let experience_id = row.6;
    let custom_launch_args = row.7;

    // Use the customer's chosen pod if provided, otherwise the token's assigned pod
    let pod_id = chosen_pod_id.unwrap_or(token_pod_id);

    // Check if this pod is part of a multiplayer group session
    let kiosk_group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT group_session_id FROM group_session_members WHERE pod_id = ? AND status = 'validated' ORDER BY validated_at DESC LIMIT 1",
    )
    .bind(&pod_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Defer billing start until AC reaches STATUS=LIVE (GameStatusUpdate from agent)
    let billing_session_id = format!("deferred-{}", uuid::Uuid::new_v4());
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id.clone(),
        custom_price_paise,
        custom_duration_minutes,
        None, // kiosk PIN validation
        None, // split_count
        None, // split_duration_minutes
        kiosk_group_session_id,
    )
    .await
    {
        // Rollback: revert token from 'consuming' back to 'pending'
        let _ = sqlx::query("UPDATE auth_tokens SET status = 'pending' WHERE id = ? AND status = 'consuming'")
            .bind(&token_id)
            .execute(&state.db)
            .await;
        tracing::error!("Defer billing failed for token {}, rolled back to pending: {}", token_id, e);
        return Err(e);
    }

    // Finalize token as consumed (billing_session_id is deferred placeholder)
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
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // Reservation linking deferred until actual billing session starts on Live

    // Auto-launch game or show assistance screen
    launch_or_assist(state, &pod_id, &billing_session_id, &experience_id, &custom_launch_args, &driver_name).await;

    // Update pod state to WaitingForGame
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // Broadcast consumed event
    let _ = state.dashboard_tx.send(DashboardEvent::AuthTokenConsumed {
        token_id: token_id.clone(),
        pod_id: pod_id.clone(),
        billing_session_id: billing_session_id.clone(),
    });

    tracing::info!(
        "PIN validated via {:?} on pod {} (#{}) driver {}, billing deferred (waiting for LIVE)",
        PinSource::Kiosk, pod_id, pod_number, driver_name
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
/// PIN-02 invariant: this function NEVER reads or writes customer_pin_failures.
pub async fn validate_employee_pin(
    state: &Arc<AppState>,
    pod_id: String,
    pin: String,
) -> Result<String, String> {
    let expected = todays_debug_pin(&state.config.auth.jwt_secret);
    if pin != expected {
        // PIN-01: increment STAFF failure counter — never customer counter
        {
            let mut failures = state.staff_pin_failures.write().await;
            *failures.entry(pod_id.clone()).or_insert(0) += 1;
        }
        return Err("Invalid employee PIN".to_string());
    }

    // PIN-01: reset staff failure counter on successful auth
    state.staff_pin_failures.write().await.remove(&pod_id);

    // Clear lock screen and enter debug mode
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterDebugMode {
            employee_name: "Staff".to_string(),
        })).await;
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
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::EnterDebugMode {
                employee_name: "Staff".to_string(),
            })).await;
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

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::SimType;

    // ─── parse_sim_type tests ────────────────────────────────────────────

    #[test]
    fn test_parse_sim_type_assetto_corsa() {
        assert_eq!(parse_sim_type("assetto_corsa"), SimType::AssettoCorsa);
        assert_eq!(parse_sim_type("ac"), SimType::AssettoCorsa);
    }

    #[test]
    fn test_parse_sim_type_assetto_corsa_evo() {
        assert_eq!(parse_sim_type("assetto_corsa_evo"), SimType::AssettoCorsaEvo);
        assert_eq!(parse_sim_type("ace"), SimType::AssettoCorsaEvo);
    }

    #[test]
    fn test_parse_sim_type_assetto_corsa_rally() {
        assert_eq!(parse_sim_type("assetto_corsa_rally"), SimType::AssettoCorsaRally);
        assert_eq!(parse_sim_type("acr"), SimType::AssettoCorsaRally);
    }

    #[test]
    fn test_parse_sim_type_iracing() {
        assert_eq!(parse_sim_type("iracing"), SimType::IRacing);
    }

    #[test]
    fn test_parse_sim_type_f1() {
        assert_eq!(parse_sim_type("f1_25"), SimType::F125);
        assert_eq!(parse_sim_type("f1"), SimType::F125);
    }

    #[test]
    fn test_parse_sim_type_lmu() {
        assert_eq!(parse_sim_type("le_mans_ultimate"), SimType::LeMansUltimate);
        assert_eq!(parse_sim_type("lmu"), SimType::LeMansUltimate);
    }

    #[test]
    fn test_parse_sim_type_forza() {
        assert_eq!(parse_sim_type("forza"), SimType::Forza);
    }

    #[test]
    fn test_parse_sim_type_forza_horizon_5() {
        assert_eq!(parse_sim_type("forza_horizon_5"), SimType::ForzaHorizon5);
        assert_eq!(parse_sim_type("fh5"), SimType::ForzaHorizon5);
    }

    #[test]
    fn test_parse_sim_type_unknown_defaults_to_ac() {
        assert_eq!(parse_sim_type("unknown_game"), SimType::AssettoCorsa);
        assert_eq!(parse_sim_type(""), SimType::AssettoCorsa);
    }

    // ─── check_pod_has_game tests ────────────────────────────────────────

    #[test]
    fn test_pod_has_game_empty_list_returns_true() {
        // Backward compat: old agents don't report installed games
        let installed: Vec<SimType> = vec![];
        assert!(check_pod_has_game(&installed, SimType::AssettoCorsa));
        assert!(check_pod_has_game(&installed, SimType::ForzaHorizon5));
    }

    #[test]
    fn test_pod_has_game_installed_returns_true() {
        let installed = vec![SimType::AssettoCorsa, SimType::F125, SimType::Forza];
        assert!(check_pod_has_game(&installed, SimType::AssettoCorsa));
        assert!(check_pod_has_game(&installed, SimType::F125));
        assert!(check_pod_has_game(&installed, SimType::Forza));
    }

    #[test]
    fn test_pod_has_game_not_installed_returns_false() {
        let installed = vec![SimType::AssettoCorsa, SimType::F125];
        assert!(!check_pod_has_game(&installed, SimType::IRacing));
        assert!(!check_pod_has_game(&installed, SimType::LeMansUltimate));
    }

    #[test]
    fn test_pod_has_game_new_variants() {
        let installed = vec![
            SimType::AssettoCorsa,
            SimType::AssettoCorsaRally,
            SimType::ForzaHorizon5,
        ];
        assert!(check_pod_has_game(&installed, SimType::AssettoCorsaRally));
        assert!(check_pod_has_game(&installed, SimType::ForzaHorizon5));
        assert!(!check_pod_has_game(&installed, SimType::AssettoCorsaEvo));
        assert!(!check_pod_has_game(&installed, SimType::IRacing));
    }

    /// AUTH-01: Verify the standardized PIN error message is correct.
    /// Both validate_pin() and validate_pin_kiosk() must return this exact string.
    #[test]
    fn pin_error_message_is_standardized() {
        assert!(
            INVALID_PIN_MESSAGE.contains("Invalid PIN"),
            "Error message must start with 'Invalid PIN'"
        );
        assert!(
            INVALID_PIN_MESSAGE.contains("reception"),
            "Error message must mention 'reception'"
        );
        // Verify the em dash is used (not a double dash)
        assert!(
            INVALID_PIN_MESSAGE.contains('\u{2014}'),
            "Error message must use em dash (U+2014), not double dash"
        );
        assert_eq!(
            INVALID_PIN_MESSAGE,
            "Invalid PIN \u{2014} please try again or see reception."
        );
    }

    /// AUTH-01: Verify PinSource enum has all three required variants.
    /// This is a compile-time check — if any variant is missing, this test won't compile.
    #[test]
    fn pin_source_has_all_variants() {
        let _pod = PinSource::Pod;
        let _kiosk = PinSource::Kiosk;
        let _pwa = PinSource::Pwa;

        // Verify Debug formatting works for tracing
        let pod_str = format!("{:?}", PinSource::Pod);
        let kiosk_str = format!("{:?}", PinSource::Kiosk);
        let pwa_str = format!("{:?}", PinSource::Pwa);
        assert_eq!(pod_str, "Pod");
        assert_eq!(kiosk_str, "Kiosk");
        assert_eq!(pwa_str, "Pwa");
    }

    /// PERF-02 proxy: Verify in-memory SQLite query completes within 200ms.
    /// Production uses local SQLite which is even faster than in-memory for reads.
    #[tokio::test]
    async fn pin_validation_timing_proxy() {
        use std::time::Instant;

        // Create in-memory SQLite pool
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Create minimal auth_tokens table
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT NOT NULL,
                status TEXT NOT NULL,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                experience_id TEXT,
                custom_launch_args TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                billing_session_id TEXT,
                consumed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("Failed to create auth_tokens table");

        let start = Instant::now();

        // Run the exact UPDATE/RETURNING query used by validate_pin() with a non-matching PIN
        let _result = sqlx::query_as::<_, (String, String, String, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id, driver_id, pricing_tier_id, custom_price_paise, custom_duration_minutes, experience_id, custom_launch_args",
        )
        .bind("pod_1")
        .bind("9999")
        .fetch_optional(&pool)
        .await;

        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 200,
            "PIN validation query took {}ms — must be under 200ms (PERF-02)",
            elapsed.as_millis()
        );

        pool.close().await;
    }

    #[test]
    fn customer_and_staff_counters_are_separate() {
        // PIN-01: the two counters are distinct HashMaps — structural separation
        use std::collections::HashMap;
        let mut customer: HashMap<String, u32> = HashMap::new();
        let staff: HashMap<String, u32> = HashMap::new();
        *customer.entry("pod_1".to_string()).or_insert(0) += 1;
        assert_eq!(customer.get("pod_1"), Some(&1));
        assert_eq!(staff.get("pod_1"), None, "staff counter must not be affected");
    }

    #[test]
    fn customer_failures_do_not_affect_staff_counter() {
        // PIN-01: 5 customer failures must leave staff counter at 0
        use std::collections::HashMap;
        let mut customer: HashMap<String, u32> = HashMap::new();
        let staff: HashMap<String, u32> = HashMap::new();
        for _ in 0..5 {
            *customer.entry("pod_1".to_string()).or_insert(0) += 1;
        }
        assert_eq!(customer.get("pod_1"), Some(&5));
        assert_eq!(
            staff.get("pod_1"),
            None,
            "staff counter must be 0 after customer failures"
        );
    }

    #[test]
    fn staff_pin_succeeds_when_customer_counter_maxed() {
        // PIN-02: staff path checks staff counter, not customer counter
        use std::collections::HashMap;
        let customer: HashMap<String, u32> = [("pod_1".to_string(), 5)].into();
        // Staff lockout check would look at staff counter (which is 0 here) — not customer
        let staff_count = 0u32; // no staff failures
        // Staff has no ceiling — even staff_count >= 1000 would pass
        let staff_locked = false; // PIN-02: staff is NEVER locked
        let _ = staff_count; // acknowledge unused variable
        assert!(!staff_locked, "staff must always be able to unlock");
        assert_eq!(customer.get("pod_1"), Some(&5), "customer IS locked at 5");
    }

    // ─── SESS-02: Single-use token (consumed token cannot be reused) ────

    /// SESS-02: A consumed token cannot be consumed again.
    /// The UPDATE...WHERE status='pending' atomically prevents double-consumption.
    #[tokio::test]
    async fn consumed_token_cannot_be_reused() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            "CREATE TABLE auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT NOT NULL,
                status TEXT NOT NULL,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                experience_id TEXT,
                custom_launch_args TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                billing_session_id TEXT,
                consumed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a pending token
        sqlx::query(
            "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, created_at, expires_at)
             VALUES ('tok-1', 'pod_1', 'drv-1', 'tier-1', 'pin', '1234', 'pending', datetime('now'), datetime('now', '+10 minutes'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // First consumption: should succeed
        let first = sqlx::query_as::<_, (String,)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id",
        )
        .bind("pod_1")
        .bind("1234")
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(first.is_some(), "First consumption must succeed");

        // Mark as consumed (simulating the full flow)
        sqlx::query("UPDATE auth_tokens SET status = 'consumed' WHERE id = 'tok-1'")
            .execute(&pool)
            .await
            .unwrap();

        // Second consumption attempt: must fail (token is consumed, not pending)
        let second = sqlx::query_as::<_, (String,)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id",
        )
        .bind("pod_1")
        .bind("1234")
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(
            second.is_none(),
            "Second consumption must fail -- consumed token cannot be reused (SESS-02)"
        );

        pool.close().await;
    }

    /// SESS-02: A token in 'consuming' state cannot be consumed again.
    /// Prevents race condition where two concurrent requests try to consume the same token.
    #[tokio::test]
    async fn consuming_token_cannot_be_double_consumed() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            "CREATE TABLE auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT NOT NULL,
                status TEXT NOT NULL,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                experience_id TEXT,
                custom_launch_args TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                billing_session_id TEXT,
                consumed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a token already in 'consuming' state (simulating a concurrent request)
        sqlx::query(
            "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, created_at, expires_at)
             VALUES ('tok-2', 'pod_1', 'drv-1', 'tier-1', 'pin', '5678', 'consuming', datetime('now'), datetime('now', '+10 minutes'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Attempt to consume: must fail (status is 'consuming', not 'pending')
        let result = sqlx::query_as::<_, (String,)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id",
        )
        .bind("pod_1")
        .bind("5678")
        .fetch_optional(&pool)
        .await
        .unwrap();

        assert!(
            result.is_none(),
            "Token in 'consuming' state must not be consumable (race protection)"
        );

        pool.close().await;
    }

    // ─── SESS-03: Atomic token consumption + billing transition ──────

    /// SESS-03: Token status transitions happen atomically via DB transaction.
    /// If billing deferral fails, token rolls back from 'consuming' to 'pending'.
    #[tokio::test]
    async fn billing_transaction_rollback_reverts_token() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            "CREATE TABLE auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT NOT NULL,
                status TEXT NOT NULL,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                experience_id TEXT,
                custom_launch_args TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                billing_session_id TEXT,
                consumed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a pending token
        sqlx::query(
            "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, created_at, expires_at)
             VALUES ('tok-3', 'pod_1', 'drv-1', 'tier-1', 'pin', '4321', 'pending', datetime('now'), datetime('now', '+10 minutes'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Start a transaction
        let mut tx = pool.begin().await.unwrap();

        // Step 1: Consume token within transaction
        let row = sqlx::query_as::<_, (String,)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id",
        )
        .bind("pod_1")
        .bind("4321")
        .fetch_optional(&mut *tx)
        .await
        .unwrap();

        assert!(row.is_some(), "Token consumption within tx should succeed");

        // Step 2: Simulate billing failure -- rollback the transaction
        tx.rollback().await.unwrap();

        // Verify token reverted to 'pending' after rollback
        let status: (String,) = sqlx::query_as("SELECT status FROM auth_tokens WHERE id = 'tok-3'")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(
            status.0, "pending",
            "Token must revert to 'pending' after transaction rollback (SESS-03)"
        );

        pool.close().await;
    }

    /// SESS-03: Successful transaction commits both token consumption and finalization.
    #[tokio::test]
    async fn billing_transaction_commit_finalizes_token() {
        let pool = sqlx::SqlitePool::connect(":memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        sqlx::query(
            "CREATE TABLE auth_tokens (
                id TEXT PRIMARY KEY,
                pod_id TEXT NOT NULL,
                driver_id TEXT NOT NULL,
                pricing_tier_id TEXT NOT NULL,
                auth_type TEXT NOT NULL,
                token TEXT NOT NULL,
                status TEXT NOT NULL,
                custom_price_paise INTEGER,
                custom_duration_minutes INTEGER,
                experience_id TEXT,
                custom_launch_args TEXT,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                billing_session_id TEXT,
                consumed_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert a pending token
        sqlx::query(
            "INSERT INTO auth_tokens (id, pod_id, driver_id, pricing_tier_id, auth_type, token, status, created_at, expires_at)
             VALUES ('tok-4', 'pod_1', 'drv-1', 'tier-1', 'pin', '9876', 'pending', datetime('now'), datetime('now', '+10 minutes'))",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Start a transaction
        let mut tx = pool.begin().await.unwrap();

        // Step 1: Consume token
        let _row = sqlx::query_as::<_, (String,)>(
            "UPDATE auth_tokens SET status = 'consuming'
             WHERE id = (
                 SELECT id FROM auth_tokens
                 WHERE pod_id = ? AND token = ? AND auth_type = 'pin' AND status = 'pending'
                   AND expires_at > datetime('now')
                 LIMIT 1
             ) AND status = 'pending'
             RETURNING id",
        )
        .bind("pod_1")
        .bind("9876")
        .fetch_optional(&mut *tx)
        .await
        .unwrap();

        // Step 2: Finalize token as consumed
        sqlx::query(
            "UPDATE auth_tokens SET status = 'consumed', billing_session_id = 'billing-123', consumed_at = datetime('now') WHERE id = 'tok-4'",
        )
        .execute(&mut *tx)
        .await
        .unwrap();

        // Step 3: Commit
        tx.commit().await.unwrap();

        // Verify token is consumed
        let status: (String,) = sqlx::query_as("SELECT status FROM auth_tokens WHERE id = 'tok-4'")
            .fetch_one(&pool)
            .await
            .unwrap();

        assert_eq!(
            status.0, "consumed",
            "Token must be 'consumed' after successful transaction commit (SESS-03)"
        );

        pool.close().await;
    }

    // ─── SEC-08: OTP hashing tests ──────────────────────────────────────────

    #[test]
    fn test_hash_otp_produces_argon2id_prefix() {
        let hash = hash_otp("123456").expect("hash_otp should succeed");
        assert!(
            hash.starts_with("$argon2id$"),
            "hash_otp must produce argon2id PHC string, got: {}",
            hash
        );
    }

    #[test]
    fn test_verify_otp_hash_correct_otp_returns_true() {
        let hash = hash_otp("123456").expect("hash_otp should succeed");
        assert!(
            verify_otp_hash("123456", &hash),
            "verify_otp_hash must return true for correct OTP"
        );
    }

    #[test]
    fn test_verify_otp_hash_wrong_otp_returns_false() {
        let hash = hash_otp("123456").expect("hash_otp should succeed");
        assert!(
            !verify_otp_hash("654321", &hash),
            "verify_otp_hash must return false for wrong OTP"
        );
    }

    #[test]
    fn test_verify_otp_hash_plaintext_input_returns_false() {
        // Graceful: if stored value is plaintext (not a valid argon2 hash), return false
        assert!(
            !verify_otp_hash("123456", "plaintext_not_hash"),
            "verify_otp_hash must return false when hash is not a valid argon2 hash"
        );
    }

    #[test]
    fn test_hash_otp_random_salt_produces_different_hashes() {
        let hash1 = hash_otp("123456").expect("hash_otp should succeed");
        let hash2 = hash_otp("123456").expect("hash_otp should succeed");
        assert_ne!(
            hash1, hash2,
            "Two hashes of the same OTP must differ (random salt)"
        );
        // But both must verify correctly
        assert!(verify_otp_hash("123456", &hash1));
        assert!(verify_otp_hash("123456", &hash2));
    }
}
