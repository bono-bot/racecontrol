//! Remote booking reservation module — PIN generation, CRUD, WhatsApp delivery.
//!
//! Customers book from their phone via the PWA, receive a 6-char alphanumeric PIN,
//! and present it at the kiosk on arrival.

use std::sync::Arc;

use rand::Rng;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::auth;
use crate::billing;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};

/// Numeric-only PIN charset (0-9). All PINs across the system are 4-digit numeric.
const PIN_CHARSET: &[u8] = b"0123456789";
const PIN_LENGTH: usize = 4;

#[derive(serde::Deserialize)]
pub struct CreateReservationRequest {
    pub experience_id: String,
    pub pricing_tier_id: String,
}

// ─── PIN Generation ──────────────────────────────────────────────────────────

/// Generate a 6-char alphanumeric PIN unique among active reservations.
/// Retries up to 5 times (collision probability is negligible with 31^6 = ~887M combinations).
pub async fn generate_unique_pin(db: &sqlx::SqlitePool) -> Result<String, String> {
    for _ in 0..5 {
        // Generate PIN in a non-async block so ThreadRng doesn't cross await boundary
        let pin: String = {
            let mut rng = rand::thread_rng();
            (0..PIN_LENGTH)
                .map(|_| PIN_CHARSET[rng.gen_range(0..PIN_CHARSET.len())] as char)
                .collect()
        };
        let exists = sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM reservations WHERE pin = ? AND status IN ('pending_debit', 'confirmed')",
        )
        .bind(&pin)
        .fetch_one(db)
        .await
        .map(|r| r.0 > 0)
        .unwrap_or(true);
        if !exists {
            return Ok(pin);
        }
    }
    Err("Failed to generate unique PIN after 5 attempts".into())
}

// ─── Create Reservation ──────────────────────────────────────────────────────

/// Create a new pod-agnostic reservation with a debit_intent.
///
/// Enforces one-active-reservation per customer. Creates a debit_intent (never
/// modifies wallet directly). Spawns WhatsApp PIN delivery as fire-and-forget.
pub async fn create_reservation(
    state: &Arc<AppState>,
    driver_id: &str,
    req: &CreateReservationRequest,
) -> Result<Value, String> {
    // One-active-reservation constraint
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pin FROM reservations WHERE driver_id = ? AND status IN ('pending_debit', 'confirmed')",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    if let Some((_existing_id, existing_pin)) = existing {
        return Err(format!(
            "You already have an active reservation (PIN: {}). Cancel it first or use your existing PIN.",
            existing_pin
        ));
    }

    // Validate pricing tier
    let tier = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT id, name, duration_minutes, price_paise FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid or inactive pricing tier".to_string())?;

    let tier_name = &tier.1;
    let duration_minutes = tier.2;
    let price_paise = tier.3;

    // Validate experience
    let experience = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&req.experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid experience".to_string())?;

    let experience_name = &experience.1;

    // Optimistic wallet balance check (actual debit happens via debit_intent processing)
    let balance = wallet::get_balance(state, driver_id).await?;
    if balance < price_paise {
        return Err(format!(
            "Insufficient wallet balance. Required: {} paise, available: {} paise",
            price_paise, balance
        ));
    }

    // Generate unique PIN
    let pin = generate_unique_pin(&state.db).await?;

    // Create reservation
    let reservation_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO reservations (id, driver_id, experience_id, pin, status, expires_at, created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, 'pending_debit', datetime('now', '+24 hours'), datetime('now'), datetime('now'), ?)",
    )
    .bind(&reservation_id)
    .bind(driver_id)
    .bind(&req.experience_id)
    .bind(&pin)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating reservation: {}", e))?;

    // Create debit_intent
    let intent_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id, status, origin, created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, 'pending', 'cloud', datetime('now'), datetime('now'), ?)",
    )
    .bind(&intent_id)
    .bind(driver_id)
    .bind(price_paise)
    .bind(&reservation_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating debit intent: {}", e))?;

    // Link debit_intent to reservation
    sqlx::query(
        "UPDATE reservations SET debit_intent_id = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&intent_id)
    .bind(&reservation_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error linking debit intent: {}", e))?;

    // Get expires_at for response
    let expires_at = sqlx::query_as::<_, (String,)>(
        "SELECT expires_at FROM reservations WHERE id = ?",
    )
    .bind(&reservation_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .map_err(|e| format!("DB error: {}", e))?;

    // Fire-and-forget WhatsApp PIN delivery
    let state_clone = Arc::clone(state);
    let driver_id_owned = driver_id.to_string();
    let pin_clone = pin.clone();
    tokio::spawn(async move {
        send_pin_whatsapp(&state_clone, &driver_id_owned, &pin_clone).await;
    });

    Ok(json!({
        "reservation_id": reservation_id,
        "pin": pin,
        "status": "pending_debit",
        "experience_id": req.experience_id,
        "experience_name": experience_name,
        "pricing_tier_id": req.pricing_tier_id,
        "tier_name": tier_name,
        "duration_minutes": duration_minutes,
        "price_paise": price_paise,
        "expires_at": expires_at,
    }))
}

// ─── Get Active Reservation ──────────────────────────────────────────────────

/// Retrieve the active reservation for a customer, if any.
pub async fn get_active_reservation(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<Value, String> {
    let row = sqlx::query_as::<_, (String, String, String, String, Option<i32>, Option<String>, String, String, Option<String>)>(
        "SELECT r.id, r.experience_id, r.pin, r.status, r.pod_number, r.debit_intent_id,
                r.created_at, r.expires_at, r.updated_at
         FROM reservations r
         WHERE r.driver_id = ? AND r.status IN ('pending_debit', 'confirmed')
         ORDER BY r.created_at DESC LIMIT 1",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let row = match row {
        Some(r) => r,
        None => return Ok(json!({ "reservation": null })),
    };

    // Get experience name
    let experience_name = sqlx::query_as::<_, (String,)>(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&row.1)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .map(|r| r.0)
    .unwrap_or_else(|| "Unknown".to_string());

    // Get debit intent details for price info
    let debit_info = if let Some(ref intent_id) = row.5 {
        sqlx::query_as::<_, (i64, String)>(
            "SELECT amount_paise, status FROM debit_intents WHERE id = ?",
        )
        .bind(intent_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
    } else {
        None
    };

    let (price_paise, debit_status) = match debit_info {
        Some((amount, status)) => (Some(amount), Some(status)),
        None => (None, None),
    };

    Ok(json!({
        "reservation": {
            "id": row.0,
            "experience_id": row.1,
            "experience_name": experience_name,
            "pin": row.2,
            "status": row.3,
            "pod_number": row.4,
            "debit_intent_id": row.5,
            "price_paise": price_paise,
            "debit_status": debit_status,
            "created_at": row.6,
            "expires_at": row.7,
            "updated_at": row.8,
        }
    }))
}

// ─── Cancel Reservation ──────────────────────────────────────────────────────

/// Cancel the active reservation for a customer.
///
/// If the debit_intent is still pending, it is cancelled (no refund needed).
/// If the debit_intent was already completed, a refund debit_intent is created.
pub async fn cancel_reservation(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<Value, String> {
    // Find active reservation
    let reservation = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT id, debit_intent_id FROM reservations WHERE driver_id = ? AND status IN ('pending_debit', 'confirmed')",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "No active reservation to cancel".to_string())?;

    let (reservation_id, debit_intent_id) = reservation;
    let mut refund_paise: i64 = 0;

    if let Some(ref intent_id) = debit_intent_id {
        let intent_status = sqlx::query_as::<_, (String, i64)>(
            "SELECT status, amount_paise FROM debit_intents WHERE id = ?",
        )
        .bind(intent_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        if let Some((status, amount)) = intent_status {
            match status.as_str() {
                "pending" | "processing" => {
                    // Cancel the pending debit intent
                    sqlx::query(
                        "UPDATE debit_intents SET status = 'cancelled', updated_at = datetime('now') WHERE id = ?",
                    )
                    .bind(intent_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;
                }
                "completed" => {
                    // Create a refund debit_intent (negative amount)
                    let refund_id = Uuid::new_v4().to_string();
                    sqlx::query(
                        "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id, status, origin, created_at, updated_at, venue_id)
                         VALUES (?, ?, ?, ?, 'pending', 'cloud', datetime('now'), datetime('now'), ?)",
                    )
                    .bind(&refund_id)
                    .bind(driver_id)
                    .bind(-amount) // Negative = refund
                    .bind(&reservation_id)
                    .bind(&state.config.venue.venue_id)
                    .execute(&state.db)
                    .await
                    .map_err(|e| format!("DB error creating refund intent: {}", e))?;
                    refund_paise = amount;
                }
                _ => {
                    // failed/cancelled — nothing to do
                }
            }
        }
    }

    // Cancel the reservation
    sqlx::query(
        "UPDATE reservations SET status = 'cancelled', cancelled_at = datetime('now'), updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&reservation_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    Ok(json!({
        "status": "cancelled",
        "reservation_id": reservation_id,
        "refund_paise": refund_paise,
    }))
}

// ─── Modify Reservation ──────────────────────────────────────────────────────

/// Modify an active reservation (change experience/pricing tier).
///
/// Implements cancel-and-rebook: cancels the old reservation and creates a new one
/// with the same expires_at (preserves original TTL, does not extend it).
pub async fn modify_reservation(
    state: &Arc<AppState>,
    driver_id: &str,
    req: &CreateReservationRequest,
) -> Result<Value, String> {
    // Find active reservation and its expiry
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT id, expires_at FROM reservations WHERE driver_id = ? AND status IN ('pending_debit', 'confirmed')",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "No active reservation to modify".to_string())?;

    let original_expires_at = existing.1;

    // Cancel existing reservation
    cancel_reservation(state, driver_id).await?;

    // Validate new pricing tier
    let tier = sqlx::query_as::<_, (String, String, i64, i64)>(
        "SELECT id, name, duration_minutes, price_paise FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&req.pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid or inactive pricing tier".to_string())?;

    let tier_name = &tier.1;
    let duration_minutes = tier.2;
    let price_paise = tier.3;

    // Validate new experience
    let experience = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&req.experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or_else(|| "Invalid experience".to_string())?;

    let experience_name = &experience.1;

    // Wallet balance check (for new reservation)
    let balance = wallet::get_balance(state, driver_id).await?;
    if balance < price_paise {
        return Err(format!(
            "Insufficient wallet balance for modified reservation. Required: {} paise, available: {} paise",
            price_paise, balance
        ));
    }

    // Generate new PIN
    let pin = generate_unique_pin(&state.db).await?;

    // Create new reservation with ORIGINAL expires_at (preserves TTL)
    let reservation_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO reservations (id, driver_id, experience_id, pin, status, expires_at, created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, 'pending_debit', ?, datetime('now'), datetime('now'), ?)",
    )
    .bind(&reservation_id)
    .bind(driver_id)
    .bind(&req.experience_id)
    .bind(&pin)
    .bind(&original_expires_at) // Preserve original TTL
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating reservation: {}", e))?;

    // Create new debit_intent
    let intent_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO debit_intents (id, driver_id, amount_paise, reservation_id, status, origin, created_at, updated_at, venue_id)
         VALUES (?, ?, ?, ?, 'pending', 'cloud', datetime('now'), datetime('now'), ?)",
    )
    .bind(&intent_id)
    .bind(driver_id)
    .bind(price_paise)
    .bind(&reservation_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating debit intent: {}", e))?;

    // Link debit_intent to reservation
    sqlx::query(
        "UPDATE reservations SET debit_intent_id = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&intent_id)
    .bind(&reservation_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error linking debit intent: {}", e))?;

    // Fire-and-forget WhatsApp PIN delivery for new PIN
    let state_clone = Arc::clone(state);
    let driver_id_owned = driver_id.to_string();
    let pin_clone = pin.clone();
    tokio::spawn(async move {
        send_pin_whatsapp(&state_clone, &driver_id_owned, &pin_clone).await;
    });

    Ok(json!({
        "reservation_id": reservation_id,
        "pin": pin,
        "status": "pending_debit",
        "experience_id": req.experience_id,
        "experience_name": experience_name,
        "pricing_tier_id": req.pricing_tier_id,
        "tier_name": tier_name,
        "duration_minutes": duration_minutes,
        "price_paise": price_paise,
        "expires_at": original_expires_at,
        "modified": true,
    }))
}

// ─── Redeem PIN (Kiosk) ─────────────────────────────────────────────────────

/// Structured error for PIN redemption — allows callers to distinguish
/// customer PIN errors (should count toward lockout) from infrastructure
/// errors (should NOT punish the customer).
pub struct RedeemPinError {
    pub message: String,
    /// `true` when the customer typed a wrong/expired/invalid PIN.
    /// `false` for capacity issues, DB errors, billing failures, etc.
    pub is_pin_error: bool,
    /// Non-None when the PIN exists but payment is still processing.
    pub is_pending_debit: bool,
}

impl RedeemPinError {
    fn pin(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), is_pin_error: true, is_pending_debit: false }
    }
    fn infra(msg: impl Into<String>) -> Self {
        Self { message: msg.into(), is_pin_error: false, is_pending_debit: false }
    }
    fn pending() -> Self {
        Self {
            message: "Your booking is being processed. Please try again in a minute.".to_string(),
            is_pin_error: false,
            is_pending_debit: true,
        }
    }
}

/// Redeem a confirmed reservation PIN at the kiosk.
///
/// Flow: validate PIN -> find idle pod (with retry) -> atomic mark redeemed ->
/// assign pod -> defer billing -> launch game -> return session info.
///
/// pending_debit PINs get a distinct "being processed" message.
/// If no pods are idle, the PIN is NOT consumed.
/// Atomic UPDATE prevents double-redeem race conditions.
/// Pod assignment uses optimistic concurrency with retry to prevent TOCTOU races.
pub async fn redeem_pin(state: &Arc<AppState>, pin: &str) -> Result<Value, RedeemPinError> {
    tracing::info!("PIN redemption started for pin={}***", &pin.get(..3).unwrap_or("?"));

    // 1. Normalize PIN (numeric-only, 4 digits)
    let pin = pin.trim().to_string();
    if pin.len() != PIN_LENGTH {
        return Err(RedeemPinError::pin(format!("PIN must be exactly {} digits", PIN_LENGTH)));
    }
    if !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(RedeemPinError::pin("PIN must contain only digits (0-9)".to_string()));
    }

    // 2. Check for pending_debit status first (distinct message)
    let pending = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM reservations WHERE pin = ? AND status = 'pending_debit' AND expires_at > datetime('now') LIMIT 1",
    )
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?;

    if pending.is_some() {
        return Err(RedeemPinError::pending());
    }

    // 3. Find idle pod with retry (fixes TOCTOU race — if pod is claimed between
    //    find_idle_pod and create_reservation, retry with a different pod)
    const MAX_POD_RETRIES: usize = 3;
    let mut pod_id = pod_reservation::find_idle_pod(state)
        .await
        .ok_or_else(|| RedeemPinError::infra(
            "All pods are currently in use. Please wait a moment and try again.".to_string(),
        ))?;

    // 4. Get pod_number from in-memory state (error instead of returning Pod 0)
    let mut pod_number = {
        let pods = state.pods.read().await;
        pods.get(&pod_id)
            .map(|p| p.number)
            .ok_or_else(|| RedeemPinError::infra(format!("Pod {} not found in memory", pod_id)))?
    };

    // 5. Atomic UPDATE to prevent double-redeem
    let redeemed = sqlx::query_as::<_, (String, String, String)>(
        "UPDATE reservations SET status = 'redeemed', redeemed_at = datetime('now'),
           pod_number = ?, updated_at = datetime('now')
         WHERE id = (
           SELECT id FROM reservations WHERE pin = ? AND status = 'confirmed'
             AND expires_at > datetime('now') LIMIT 1
         ) AND status = 'confirmed'
         RETURNING id, driver_id, experience_id",
    )
    .bind(pod_number as i64)
    .bind(&pin)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?;

    let (reservation_id, driver_id, experience_id) = match redeemed {
        Some(row) => (row.0, row.1, row.2),
        None => return Err(RedeemPinError::pin("Invalid PIN or reservation not found")),
    };

    // 6. Get pricing_tier_id from kiosk_experiences (error if missing, not silent "default")
    let pricing_tier_id: String = sqlx::query_scalar(
        "SELECT pricing_tier_id FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| RedeemPinError::infra(format!("DB error: {}", e)))?
    .ok_or_else(|| {
        tracing::warn!("Experience {} has no pricing_tier_id — using default", experience_id);
        // Fallback to default but log it as a warning (B3 fix: visibility)
        RedeemPinError::infra("Experience configuration error — please see reception".to_string())
    })
    .or_else(|e| {
        // If we want to gracefully degrade rather than fail, fall back
        // For now, let it error out so staff notices the config issue
        Err(e)
    })?;

    // 7. Create pod_reservation with retry for TOCTOU race (B6 fix)
    let mut pod_claim_ok = false;
    for attempt in 0..MAX_POD_RETRIES {
        match pod_reservation::create_reservation(state, &driver_id, &pod_id).await {
            Ok(_) => {
                pod_claim_ok = true;
                break;
            }
            Err(e) => {
                tracing::warn!(
                    "Pod {} claim failed (attempt {}/{}): {} — retrying with different pod",
                    pod_id, attempt + 1, MAX_POD_RETRIES, e
                );
                // Find a different idle pod
                match pod_reservation::find_idle_pod(state).await {
                    Some(new_pod_id) => {
                        let pods = state.pods.read().await;
                        match pods.get(&new_pod_id).map(|p| p.number) {
                            Some(num) => {
                                pod_id = new_pod_id;
                                pod_number = num;
                                // Update reservation with new pod_number
                                let _ = sqlx::query(
                                    "UPDATE reservations SET pod_number = ?, updated_at = datetime('now') WHERE id = ?",
                                )
                                .bind(pod_number as i64)
                                .bind(&reservation_id)
                                .execute(&state.db)
                                .await;
                            }
                            None => continue,
                        }
                    }
                    None => break, // No more idle pods
                }
            }
        }
    }

    if !pod_claim_ok {
        // Rollback reservation status with error logging (B5 fix)
        if let Err(rollback_err) = sqlx::query(
            "UPDATE reservations SET status = 'confirmed', redeemed_at = NULL, pod_number = NULL, updated_at = datetime('now') WHERE id = ? AND status = 'redeemed'",
        )
        .bind(&reservation_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("CRITICAL: Failed to rollback reservation {}: {}", reservation_id, rollback_err);
        }
        return Err(RedeemPinError::infra("Failed to reserve pod — all pods busy or claimed"));
    }

    // 8. Create billing_session_id
    let billing_session_id = format!("deferred-{}", Uuid::new_v4());

    // 9. Defer billing start
    if let Err(e) = billing::defer_billing_start(
        state,
        pod_id.clone(),
        driver_id.clone(),
        pricing_tier_id.clone(),
        None, // custom_price_paise
        None, // custom_duration_minutes
        None, // staff_id
        None, // split_count
        None, // split_duration_minutes
        None, // group_session_id
    )
    .await
    {
        // Rollback reservation status on billing failure (B5 fix: log rollback errors)
        if let Err(rollback_err) = sqlx::query(
            "UPDATE reservations SET status = 'confirmed', redeemed_at = NULL, pod_number = NULL, updated_at = datetime('now') WHERE id = ? AND status = 'redeemed'",
        )
        .bind(&reservation_id)
        .execute(&state.db)
        .await
        {
            tracing::error!("CRITICAL: Failed to rollback reservation {} after billing error: {}", reservation_id, rollback_err);
        }
        return Err(RedeemPinError::infra(format!("Failed to start billing: {}", e)));
    }

    // 10. Get driver_name
    let driver_name: String = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Driver".to_string());

    // 11. Get pricing tier name + duration
    let tier_row = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT name, duration_minutes FROM pricing_tiers WHERE id = ?",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let tier_name = tier_row
        .as_ref()
        .map(|r| r.0.clone())
        .unwrap_or_else(|| "Session".to_string());

    let duration_minutes = tier_row
        .as_ref()
        .and_then(|r| r.1)
        .unwrap_or(30);

    // 12. Clear lock screen on pod agent
    let agent_senders = state.agent_senders.read().await;
    if let Some(sender) = agent_senders.get(&pod_id) {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::ClearLockScreen)).await;
    }
    drop(agent_senders);

    // 13. Launch game or show assistance screen
    auth::launch_or_assist(
        state,
        &pod_id,
        &billing_session_id,
        &Some(experience_id.clone()),
        &None,
        &driver_name,
    )
    .await;

    // 14. Update pod state (current_driver) and broadcast PodUpdate
    {
        let mut pods = state.pods.write().await;
        if let Some(pod) = pods.get_mut(&pod_id) {
            pod.current_driver = Some(driver_name.clone());
            let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
        }
    }

    // 15. Get experience_name
    let experience_name: String = sqlx::query_scalar(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    tracing::info!(
        "Reservation PIN redeemed on pod {} (#{}) driver {}, billing deferred",
        pod_id, pod_number, driver_name
    );

    // 16. Return JSON
    Ok(json!({
        "pod_number": pod_number,
        "pod_id": pod_id,
        "driver_name": driver_name,
        "experience_name": experience_name,
        "tier_name": tier_name,
        "allocated_seconds": duration_minutes * 60,
        "billing_session_id": billing_session_id,
    }))
}

// ─── WhatsApp PIN Delivery ───────────────────────────────────────────────────

/// Send the reservation PIN to the customer via WhatsApp (Evolution API).
///
/// Fire-and-forget: logs success/failure but never propagates errors.
/// Queries the driver's phone from the database.
pub async fn send_pin_whatsapp(state: &Arc<AppState>, driver_id: &str, pin: &str) {
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
