//! Billing start — input parsing, lock acquisition, and pre-transaction validation.
//!
//! Extracted from `billing_start.rs` (Phase 385 split) to keep each module under 500 lines.
//! This module validates all preconditions before the billing transaction begins.

use std::sync::Arc;

use axum::Json;
use serde_json::{Value, json};
use tokio::sync::OwnedMutexGuard;

use crate::state::AppState;
use crate::wallet;
use rc_common::pod_id::normalize_pod_id;

// ─── Intermediate types ─────────────────────────────────────────────────────

/// Raw fields parsed from the JSON request body.
pub(crate) struct BillingStartInput {
    pub pod_id: String,
    pub driver_id: String,
    pub pricing_tier_id: String,
    pub custom_price_paise: Option<u32>,
    pub custom_duration_minutes: Option<u32>,
    pub staff_id: Option<String>,
    pub split_count: Option<u32>,
    pub split_duration_minutes: Option<u32>,
    pub idempotency_key: Option<String>,
    pub coupon_code: Option<String>,
    pub staff_discount_paise: Option<i64>,
    pub discount_reason: Option<String>,
    pub guardian_present_flag: bool,
}

/// Pricing tier info fetched from DB.
pub(crate) struct TierInfo {
    pub name: String,
    pub duration_minutes: i64,
    pub price_paise: i64,
    pub is_trial: bool,
    pub billing_mode: String,
    pub rate_per_min: Option<i64>,
    pub hold: Option<i64>,
    pub low_warn: Option<i64>,
}

/// Driver info fetched from DB.
pub(crate) struct DriverInfo {
    pub name: String,
    pub has_used_trial: bool,
    pub unlimited_trials: bool,
    pub is_minor: bool,
}

/// Locks held for the duration of the billing start operation.
/// These must remain alive until the transaction completes.
#[allow(dead_code)]
pub(crate) struct BillingStartLocks {
    pub driver_lock: OwnedMutexGuard<()>,
    pub pod_lock: OwnedMutexGuard<()>,
}

// ─── Parsing ─────────────────────────────────────────────────────────────────

/// Parse and normalize the JSON request body into a `BillingStartInput`.
pub(crate) fn parse_billing_input(body: &Value) -> Result<BillingStartInput, Json<Value>> {
    let pod_id_raw = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let pricing_tier_id = body.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or("");

    let pod_id = normalize_pod_id(pod_id_raw).unwrap_or_else(|_| pod_id_raw.to_string());

    if pod_id.is_empty() || driver_id.is_empty() || pricing_tier_id.is_empty() {
        return Err(Json(json!({ "error": "pod_id, driver_id, and pricing_tier_id are required" })));
    }

    Ok(BillingStartInput {
        pod_id,
        driver_id: driver_id.to_string(),
        pricing_tier_id: pricing_tier_id.to_string(),
        custom_price_paise: body.get("custom_price_paise").and_then(|v| v.as_u64()).map(|v| v as u32),
        custom_duration_minutes: body.get("custom_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32),
        staff_id: body.get("staff_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
        split_count: body.get("split_count").and_then(|v| v.as_u64()).map(|v| v as u32),
        split_duration_minutes: body.get("split_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32),
        idempotency_key: body.get("idempotency_key").and_then(|v| v.as_str()).map(|s| s.to_string()),
        coupon_code: body.get("coupon_code").and_then(|v| v.as_str()).map(|s| s.to_string()),
        staff_discount_paise: body.get("staff_discount_paise").and_then(|v| v.as_i64()),
        discount_reason: body.get("discount_reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
        guardian_present_flag: body.get("guardian_present").and_then(|v| v.as_bool()).unwrap_or(false),
    })
}

// ─── Lock acquisition ────────────────────────────────────────────────────────

/// Acquire per-driver and per-pod billing locks.
/// CONC-01: Driver lock first, then pod lock — consistent ordering prevents deadlocks.
pub(crate) async fn acquire_billing_locks(
    state: &Arc<AppState>,
    driver_id: &str,
    pod_id: &str,
) -> Result<BillingStartLocks, Json<Value>> {
    // CONC-01: Per-driver lock FIRST
    let driver_lock_arc = state.billing.get_driver_billing_lock(driver_id);
    let driver_lock = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        driver_lock_arc.lock_owned(),
    ).await {
        Ok(guard) => guard,
        Err(_) => {
            return Err(Json(json!({ "error": "Billing request timed out for driver — another operation is in progress" })));
        }
    };

    // BATOM-01: Per-pod lock to serialize concurrent start_billing calls
    let billing_lock_arc = state.billing.get_billing_start_lock(pod_id);
    let pod_lock = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        billing_lock_arc.lock_owned(),
    ).await {
        Ok(guard) => guard,
        Err(_) => {
            tracing::error!(
                "Per-pod billing lock timeout (30s) for pod {} — another billing operation may be stuck",
                pod_id
            );
            state.record_api_error("billing/start");
            return Err(Json(json!({ "error": format!("Billing request timed out for pod {} — another operation is in progress", pod_id) })));
        }
    };

    Ok(BillingStartLocks { driver_lock, pod_lock })
}

// ─── Idempotency ─────────────────────────────────────────────────────────────

/// FATM-02: Check if an idempotency key was already processed; return the original result if so.
pub(crate) async fn check_idempotency(
    state: &Arc<AppState>,
    key: &str,
) -> Option<Json<Value>> {
    let existing = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT id, wallet_debit_paise FROM billing_sessions WHERE idempotency_key = ?",
    )
    .bind(key)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    existing.map(|(existing_id, existing_debit)| {
        Json(json!({
            "ok": true,
            "billing_session_id": existing_id,
            "wallet_debit_paise": existing_debit,
            "idempotent_replay": true,
        }))
    })
}

// ─── In-memory pre-validation ────────────────────────────────────────────────

/// BATOM-02: Check both in-memory maps for active/waiting sessions on this pod.
pub(crate) async fn check_pod_not_active(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), Json<Value>> {
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(pod_id) {
            return Err(Json(json!({ "error": format!("Pod {} already has an active billing session", pod_id) })));
        }
    }
    {
        let waiting = state.billing.waiting_for_game.read().await;
        if waiting.contains_key(pod_id) {
            return Err(Json(json!({ "error": format!("Pod {} already has a billing session waiting for game", pod_id) })));
        }
    }
    Ok(())
}

// ─── DB lookups ──────────────────────────────────────────────────────────────

/// Look up pricing tier by ID.
pub(crate) async fn lookup_tier(
    state: &Arc<AppState>,
    pricing_tier_id: &str,
) -> Result<TierInfo, Json<Value>> {
    let row = sqlx::query_as::<_, (String, i64, i64, bool, String, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT name, duration_minutes, price_paise, is_trial, \
         COALESCE(billing_mode, 'package'), rate_paise_per_minute, minimum_hold_paise, low_balance_warning_paise \
         FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((name, duration_minutes, price_paise, is_trial, billing_mode, rate_per_min, hold, low_warn)) => {
            Ok(TierInfo { name, duration_minutes, price_paise, is_trial, billing_mode, rate_per_min, hold, low_warn })
        }
        None => Err(Json(json!({ "error": format!("Pricing tier '{}' not found or inactive", pricing_tier_id) }))),
    }
}

/// Look up driver and validate waiver, minor status, and guardian consent.
pub(crate) async fn lookup_and_validate_driver(
    state: &Arc<AppState>,
    driver_id: &str,
    _pod_id: &str,
    guardian_present_flag: bool,
) -> Result<DriverInfo, Json<Value>> {
    let row = sqlx::query_as::<_, (String, bool, bool, bool, Option<String>, bool, Option<String>)>(
        "SELECT name, COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0), \
         COALESCE(waiver_signed, 0), dob, COALESCE(guardian_otp_verified, 0), guardian_name \
         FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (name, has_used_trial, unlimited_trials, waiver_signed, dob, guardian_otp_verified, guardian_name) = match row {
        Some(d) => d,
        None => return Err(Json(json!({ "error": format!("Driver '{}' not found", driver_id) }))),
    };

    // LEGAL-03: Waiver gate
    if !waiver_signed {
        return Err(Json(json!({ "error": "Waiver signing required before billing. Please complete registration." })));
    }

    // CONC-01: Per-driver concurrency guard
    let existing_driver_session = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pod_id FROM billing_sessions WHERE driver_id = ? AND status IN ('active', 'waiting_for_game')",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    if let Some((_existing_id, existing_pod)) = existing_driver_session {
        return Err(Json(json!({ "error": format!("Driver already has an active session on {}", existing_pod) })));
    }

    // LEGAL-04/05: Minor protection
    let is_minor = if let Some(ref dob_str) = dob {
        if let Ok(dob_date) = chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d") {
            use chrono::Datelike;
            let today = chrono::Utc::now().date_naive();
            let age_years = today.year() - dob_date.year()
                - if (today.month(), today.day()) < (dob_date.month(), dob_date.day()) { 1 } else { 0 };
            age_years < 18
        } else {
            false
        }
    } else {
        false
    };

    if is_minor {
        if !guardian_otp_verified {
            return Err(Json(json!({
                "error": "Minor customer: guardian OTP verification required before billing",
                "minor_flow_required": true,
                "guardian_name": guardian_name,
            })));
        }
        if !guardian_present_flag {
            return Err(Json(json!({
                "error": "Minor customer: staff must confirm guardian physical presence (guardian_present=true)",
                "minor_flow_required": true,
            })));
        }
    }

    Ok(DriverInfo { name, has_used_trial, unlimited_trials, is_minor })
}

// ─── Trial / pod / split / wallet validation ────────────────────────────────

/// Check trial eligibility (per-racer and per-account group cap).
pub(crate) async fn check_trial_eligibility(
    state: &Arc<AppState>,
    driver_id: &str,
    has_used_trial: bool,
    unlimited_trials: bool,
    is_trial: bool,
) -> Result<(), Json<Value>> {
    if !is_trial || unlimited_trials {
        return Ok(());
    }
    if has_used_trial {
        return Err(Json(json!({ "error": "This racer has already used their free trial" })));
    }
    let group_parent = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT linked_to FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.0);
    let root_id = group_parent.as_deref().unwrap_or(driver_id);
    let group_trials: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM drivers WHERE (id = ?1 OR linked_to = ?1) AND has_used_trial = 1",
    )
    .bind(root_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or((0,));
    if group_trials.0 >= 4 {
        return Err(Json(json!({ "error": "Maximum free trials reached for this account" })));
    }
    Ok(())
}

/// Validate pod exists in DB.
pub(crate) async fn validate_pod_exists(
    state: &Arc<AppState>,
    pod_id: &str,
) -> Result<(), Json<Value>> {
    let pod_exists = sqlx::query_as::<_, (String,)>("SELECT id FROM pods WHERE id = ?")
        .bind(pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    if pod_exists.is_none() {
        return Err(Json(json!({ "error": format!("Pod '{}' not found", pod_id) })));
    }
    Ok(())
}

/// Validate split and duration params, compute allocated seconds.
pub(crate) fn validate_splits_and_duration(
    input: &BillingStartInput,
    tier_duration_minutes: i64,
) -> Result<(u32, u32), Json<Value>> {
    if let Some(sc) = input.split_count {
        if sc > 0 && input.split_duration_minutes.unwrap_or(1) == 0 {
            return Err(Json(json!({ "error": "Split duration must be greater than 0 minutes" })));
        }
    }
    if let Some(dur) = input.custom_duration_minutes {
        if dur > 1440 {
            return Err(Json(json!({ "error": "Custom duration cannot exceed 24 hours (1440 minutes)" })));
        }
    }
    if let Some(dur) = input.split_duration_minutes {
        if dur > 1440 {
            return Err(Json(json!({ "error": "Split duration cannot exceed 24 hours (1440 minutes)" })));
        }
    }

    let final_split_count = input.split_count.unwrap_or(1);
    let allocated_seconds: u32 = if let Some(split_dur) = input.split_duration_minutes.filter(|_| final_split_count > 1) {
        split_dur * 60
    } else {
        input.custom_duration_minutes
            .map(|m| m * 60)
            .unwrap_or(tier_duration_minutes as u32 * 60)
    };

    Ok((final_split_count, allocated_seconds))
}

/// Pre-check wallet balance (optimistic, before transaction).
pub(crate) async fn pre_check_wallet_balance(
    state: &Arc<AppState>,
    wallet_owner_id: &str,
    final_price_paise: i64,
    is_trial: bool,
) -> Result<(), Json<Value>> {
    if !is_trial && final_price_paise > 0 {
        let balance = match wallet::get_balance(state, wallet_owner_id).await {
            Ok(b) => b,
            Err(e) => return Err(Json(json!({ "error": format!("Wallet error: {}", e) }))),
        };
        if balance < final_price_paise {
            return Err(Json(json!({
                "error": format!("Insufficient credits: have {} credits, need {} credits", balance / 100, final_price_paise / 100),
                "balance_paise": balance,
                "required_paise": final_price_paise,
            })));
        }
    }
    Ok(())
}

/// Resolve wallet owner (linked racers use parent's wallet).
pub(crate) async fn resolve_wallet_owner(
    state: &Arc<AppState>,
    driver_id: &str,
) -> Result<String, Json<Value>> {
    wallet::resolve_wallet_owner(state, driver_id)
        .await
        .map_err(|e| Json(json!({ "error": format!("Wallet error: {}", e) })))
}
