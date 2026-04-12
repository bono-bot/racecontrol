#![allow(unused_imports)]
use super::billing_coupon::{validate_and_calc_coupon, record_coupon_redemption, reserve_coupon, redeem_coupon, restore_coupon_on_cancel, CouponDiscount};
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Billing ────────────────────────────────────────────────────────────────

pub(crate) async fn start_billing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;

    // Phase 366 GLD-F-04: Pre-check concurrent session guard — returns HTTP 409.
    let pod_id_check = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    if !pod_id_check.is_empty() {
        let pod_id = normalize_pod_id(pod_id_check).unwrap_or_else(|_| pod_id_check.to_string());
        {
            let timers = state.billing.active_timers.read().await;
            if let Some(timer) = timers.get(pod_id.as_str()) {
                let active_session_id = timer.session_id.clone();
                return (
                    axum::http::StatusCode::CONFLICT,
                    Json(json!({
                        "error": "pod_already_active",
                        "active_session_id": active_session_id,
                        "pod_id": pod_id
                    })),
                ).into_response();
            }
        }
        {
            let waiting = state.billing.waiting_for_game.read().await;
            if waiting.contains_key(pod_id.as_str()) {
                return (
                    axum::http::StatusCode::CONFLICT,
                    Json(json!({
                        "error": "pod_already_active",
                        "active_session_id": null,
                        "pod_id": pod_id,
                        "detail": "pod has a billing session waiting for game start"
                    })),
                ).into_response();
            }
        }
    }

    start_billing_inner(State(state), Json(body)).await.into_response()
}

/// Inner billing start — returns Json<Value>. Phase 366 wrapper handles 409.
pub(crate) async fn start_billing_inner(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id_raw = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let pricing_tier_id = body.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or("");
    let custom_price_paise = body.get("custom_price_paise").and_then(|v| v.as_u64()).map(|v| v as u32);
    let custom_duration_minutes = body.get("custom_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);
    let staff_id = body.get("staff_id").and_then(|v| v.as_str()).map(|s| s.to_string());
    let split_count = body.get("split_count").and_then(|v| v.as_u64()).map(|v| v as u32);
    let split_duration_minutes = body.get("split_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);
    // FATM-02: Idempotency key — if present, duplicate requests return the original result
    let idempotency_key = body.get("idempotency_key").and_then(|v| v.as_str()).map(|s| s.to_string());
    // Discount params: coupon_code OR staff_discount_paise + discount_reason
    let coupon_code = body.get("coupon_code").and_then(|v| v.as_str()).map(|s| s.to_string());
    let staff_discount_paise = body.get("staff_discount_paise").and_then(|v| v.as_i64());
    let discount_reason = body.get("discount_reason").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Normalize pod_id to canonical form
    let pod_id = rc_common::pod_id::normalize_pod_id(pod_id_raw).unwrap_or_else(|_| pod_id_raw.to_string());

    if pod_id.is_empty() || driver_id.is_empty() || pricing_tier_id.is_empty() {
        return Json(json!({ "error": "pod_id, driver_id, and pricing_tier_id are required" }));
    }

    // CONC-01: Acquire per-driver lock FIRST — prevents same driver from racing
    // billing starts on different pods. Must be acquired before per-pod lock to
    // maintain consistent lock ordering (driver → pod).
    let driver_lock_arc = state.billing.get_driver_billing_lock(driver_id);
    let _driver_lock = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        driver_lock_arc.lock(),
    ).await {
        Ok(guard) => guard,
        Err(_) => {
            return Json(json!({ "error": format!("Billing request timed out for driver — another operation is in progress") }));
        }
    };

    // BATOM-01: Acquire per-pod lock to serialize concurrent start_billing calls.
    // This eliminates the TOCTOU window between pre-validation and waiting_for_game write.
    // Different pods are not blocked — only same-pod concurrent requests are serialized.
    // Timeout prevents indefinite hangs if a prior handler is stuck (e.g., slow DB).
    let billing_lock_arc = state.billing.get_billing_start_lock(&pod_id);
    let _billing_lock = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        billing_lock_arc.lock(),
    ).await {
        Ok(guard) => guard,
        Err(_) => {
            tracing::error!(
                "Per-pod billing lock timeout (30s) for pod {} — another billing operation may be stuck",
                pod_id
            );
            state.record_api_error("billing/start");
            return Json(json!({ "error": format!("Billing request timed out for pod {} — another operation is in progress", pod_id) }));
        }
    };

    // FATM-02: Idempotency check — return original result if key was already processed
    if let Some(ref key) = idempotency_key {
        let existing = sqlx::query_as::<_, (String, Option<i64>)>(
            "SELECT id, wallet_debit_paise FROM billing_sessions WHERE idempotency_key = ?",
        )
        .bind(key)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
        if let Some((existing_id, existing_debit)) = existing {
            return Json(json!({
                "ok": true,
                "billing_session_id": existing_id,
                "wallet_debit_paise": existing_debit,
                "idempotent_replay": true,
            }));
        }
    }

    // BATOM-02: Pre-validate — check BOTH in-memory maps (fast path; DB UNIQUE index is defense-in-depth)
    {
        let timers = state.billing.active_timers.read().await;
        if timers.contains_key(pod_id.as_str()) {
            return Json(json!({ "error": format!("Pod {} already has an active billing session", pod_id) }));
        }
    }
    {
        let waiting = state.billing.waiting_for_game.read().await;
        if waiting.contains_key(pod_id.as_str()) {
            return Json(json!({ "error": format!("Pod {} already has a billing session waiting for game", pod_id) }));
        }
    }

    // Look up tier (name + duration + price + trial flag + per-minute billing fields)
    let tier_info = sqlx::query_as::<_, (String, i64, i64, bool, String, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT name, duration_minutes, price_paise, is_trial, \
         COALESCE(billing_mode, 'package'), rate_paise_per_minute, minimum_hold_paise, low_balance_warning_paise \
         FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(&pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (tier_name, tier_duration_minutes, tier_price_paise, is_trial, tier_billing_mode, tier_rate_per_min, tier_hold, tier_low_warn) = match tier_info {
        Some(t) => t,
        None => return Json(json!({ "error": format!("Pricing tier '{}' not found or inactive", pricing_tier_id) })),
    };

    // Look up driver (name + trial status + waiver + DOB + guardian consent)
    let driver_info = sqlx::query_as::<_, (String, bool, bool, bool, Option<String>, bool, Option<String>)>(
        "SELECT name, COALESCE(has_used_trial, 0), COALESCE(unlimited_trials, 0), \
         COALESCE(waiver_signed, 0), dob, COALESCE(guardian_otp_verified, 0), guardian_name \
         FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (driver_name, has_used_trial, unlimited_trials, waiver_signed, dob, guardian_otp_verified, guardian_name) = match driver_info {
        Some(d) => d,
        None => return Json(json!({ "error": format!("Driver '{}' not found", driver_id) })),
    };

    // LEGAL-03: Waiver gate — billing blocked if waiver not signed
    if !waiver_signed {
        return Json(json!({ "error": "Waiver signing required before billing. Please complete registration." }));
    }

    // CONC-01: Per-driver concurrency guard — prevent same driver from having
    // simultaneous billing sessions on different pods.
    // Found by Layer 1 concurrent stress test: race condition allowed double-booking.
    let existing_driver_session = sqlx::query_as::<_, (String, String)>(
        "SELECT id, pod_id FROM billing_sessions WHERE driver_id = ? AND status IN ('active', 'waiting_for_game')",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    if let Some((_existing_id, existing_pod)) = existing_driver_session {
        return Json(json!({ "error": format!("Driver already has an active session on {}", existing_pod) }));
    }

    // LEGAL-04/05: Minor protection — check age from DOB
    let is_minor = if let Some(ref dob_str) = dob {
        if let Ok(dob_date) = chrono::NaiveDate::parse_from_str(dob_str, "%Y-%m-%d") {
            use chrono::Datelike;
            let today = chrono::Utc::now().date_naive();
            // Conservative manual age check: compare year/month/day to avoid fractional year rounding
            let age_years = today.year() - dob_date.year()
                - if (today.month(), today.day()) < (dob_date.month(), dob_date.day()) { 1 } else { 0 };
            age_years < 18
        } else {
            false // Cannot parse DOB — treat as adult
        }
    } else {
        false // No DOB on record — treat as adult
    };

    // Parse guardian_present flag from request body (staff must explicitly confirm)
    let guardian_present_flag = body.get("guardian_present").and_then(|v| v.as_bool()).unwrap_or(false);

    if is_minor {
        // LEGAL-04: Guardian OTP must be verified before billing a minor
        if !guardian_otp_verified {
            return Json(json!({
                "error": "Minor customer: guardian OTP verification required before billing",
                "minor_flow_required": true,
                "guardian_name": guardian_name,
            }));
        }
        // LEGAL-05: Staff must confirm guardian physical presence
        if !guardian_present_flag {
            return Json(json!({
                "error": "Minor customer: staff must confirm guardian physical presence (guardian_present=true)",
                "minor_flow_required": true,
            }));
        }
    }

    // Trial eligibility check — per-racer AND per-account group cap
    if is_trial && !unlimited_trials {
        if has_used_trial {
            return Json(json!({ "error": "This racer has already used their free trial" }));
        }
        // Check group trial cap: count trials used by parent + all linked racers
        let group_parent = sqlx::query_as::<_, (Option<String>,)>(
            "SELECT linked_to FROM drivers WHERE id = ?",
        )
        .bind(&driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.0);
        // The "root" of the group is either the parent (if this driver is linked) or this driver itself
        let root_id = group_parent.as_deref().unwrap_or(&driver_id);
        let group_trials: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM drivers WHERE (id = ?1 OR linked_to = ?1) AND has_used_trial = 1",
        )
        .bind(root_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));
        // Max 4 trials per group (1 parent + 3 racers)
        if group_trials.0 >= 4 {
            return Json(json!({ "error": "Maximum free trials reached for this account" }));
        }
    }

    // Validate pod exists
    let pod_exists = sqlx::query_as::<_, (String,)>("SELECT id FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();
    if pod_exists.is_none() {
        return Json(json!({ "error": format!("Pod '{}' not found", pod_id) }));
    }

    // Validate split params
    if let Some(sc) = split_count {
        if sc > 0 && split_duration_minutes.unwrap_or(1) == 0 {
            return Json(json!({ "error": "Split duration must be greater than 0 minutes" }));
        }
    }
    if let Some(dur) = custom_duration_minutes {
        if dur > 1440 { return Json(json!({ "error": "Custom duration cannot exceed 24 hours (1440 minutes)" })); }
    }
    if let Some(dur) = split_duration_minutes {
        if dur > 1440 { return Json(json!({ "error": "Split duration cannot exceed 24 hours (1440 minutes)" })); }
    }

    // Calculate allocated seconds
    let final_split_count = split_count.unwrap_or(1);
    let allocated_seconds: u32 = if let Some(split_dur) = split_duration_minutes.filter(|_| final_split_count > 1) {
        split_dur * 60
    } else {
        custom_duration_minutes
            .map(|m| m * 60)
            .unwrap_or(tier_duration_minutes as u32 * 60)
    };

    // Determine base price (custom override or tier price with optional dynamic pricing)
    let mut base_price_paise = custom_price_paise.map(|p| p as i64).unwrap_or_else(|| {
        // Dynamic pricing computed here synchronously is fine — no lock held
        tier_price_paise
    });

    // Apply group discount: if 3+ sessions already active, 4th+ gets group multiplier
    let mut group_discount_paise: i64 = 0;
    let active_count = {
        // Snapshot count before dropping lock
        let timers = state.billing.active_timers.read().await;
        timers.len()
    };
    if !is_trial && active_count >= 3 {
        let group_rule = sqlx::query_as::<_, (f64,)>(
            "SELECT multiplier FROM pricing_rules WHERE rule_type = 'group' AND is_active = 1 LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        if let Some((multiplier,)) = group_rule {
            let discounted = (base_price_paise as f64 * multiplier).round() as i64;
            group_discount_paise = base_price_paise - discounted;
            base_price_paise = discounted;
            tracing::info!(
                "Group discount applied: {} active sessions, multiplier={}, saved {}p",
                active_count + 1, multiplier, group_discount_paise
            );
        }
    }

    // FATM-08: Generate session_id early so coupon reservation can be tied to the real session ID.
    // This session_id is used in reserve_coupon and then reused in the INSERT below.
    let session_id = uuid::Uuid::new_v4().to_string();

    // Apply coupon or staff discount
    let mut applied_discount_paise: i64 = group_discount_paise;
    let mut applied_coupon_id: Option<String> = None;
    let mut applied_discount_reason: Option<String> = if group_discount_paise > 0 {
        Some(format!("Group {} sessions (11% off)", active_count + 1))
    } else {
        None
    };

    if let Some(ref code) = coupon_code {
        match validate_and_calc_coupon(&state, code, driver_id, base_price_paise).await {
            Ok(cd) => {
                // FATM-08: Reserve coupon before the billing transaction.
                // CAS UPDATE WHERE coupon_status = 'available' catches concurrent races.
                if let Err(e) = reserve_coupon(&state.db, &cd.coupon_id, &session_id).await {
                    return Json(json!({ "error": e }));
                }
                applied_discount_paise += cd.discount_paise;
                applied_coupon_id = Some(cd.coupon_id);
                let coupon_desc = format!("Coupon {}: {}", code.to_uppercase(), cd.description);
                applied_discount_reason = Some(match applied_discount_reason {
                    Some(existing) => format!("{} + {}", existing, coupon_desc),
                    None => coupon_desc,
                });
            }
            Err(e) => return Json(json!({ "error": e })),
        }
    } else if let Some(staff_disc) = staff_discount_paise {
        if staff_disc > 0 && staff_disc <= base_price_paise {
            applied_discount_paise += staff_disc;
            let staff_desc = discount_reason.unwrap_or("Staff discount".to_string());
            applied_discount_reason = Some(match applied_discount_reason {
                Some(existing) => format!("{} + {}", existing, staff_desc),
                None => staff_desc,
            });
        }
    }

    let original_price_paise = custom_price_paise.map(|p| p as i64).unwrap_or(tier_price_paise);
    let mut final_price_paise = original_price_paise - applied_discount_paise;

    // FATM-10: Enforce discount floor — combined discounts cannot reduce payable below the floor
    let discount_floor_paise = billing::DISCOUNT_FLOOR_PAISE;
    if discount_floor_paise > 0 && final_price_paise < discount_floor_paise {
        let original_total_discount = applied_discount_paise;
        applied_discount_paise = original_price_paise - discount_floor_paise;
        final_price_paise = discount_floor_paise;
        tracing::info!(
            "FATM-10: Discount floor enforced — original discount {}p capped to {}p (floor={}p, original_price={}p)",
            original_total_discount, applied_discount_paise, discount_floor_paise, original_price_paise
        );
    }

    // Resolve wallet owner: linked racers use parent's wallet
    let wallet_owner_id = match wallet::resolve_wallet_owner(&state, driver_id).await {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": format!("Wallet error: {}", e) })),
    };

    // Pre-check balance (optimistic, before acquiring tx) to return a clear error
    if !is_trial && final_price_paise > 0 {
        let balance = match wallet::get_balance(&state, &wallet_owner_id).await {
            Ok(b) => b,
            Err(e) => return Json(json!({ "error": format!("Wallet error: {}", e) })),
        };
        if balance < final_price_paise {
            return Json(json!({
                "error": format!("Insufficient credits: have {} credits, need {} credits", balance / 100, final_price_paise / 100),
                "balance_paise": balance,
                "required_paise": final_price_paise,
            }));
        }
    }

    // Fetch pod number for debit notes (before tx)
    let pod_num = sqlx::query_as::<_, (i64,)>("SELECT number FROM pods WHERE id = ?")
        .bind(&pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or(0);

    // ─── FATM-01: Single atomic transaction — wallet debit + session INSERT ───
    // If ANY step fails, the entire transaction rolls back automatically on drop.
    // No compensating refund needed — rollback is the rollback.
    // FATM-03: SQLite WAL mode with busy_timeout=5000ms handles concurrent write serialization.
    // The atomic UPDATE WHERE balance >= ? inside debit_in_tx is the overspend guard.
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            // FATM-09: If a coupon was reserved but we can't start a transaction, restore it
            if let Some(ref cid) = applied_coupon_id {
                let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
                tracing::warn!(
                    coupon_id = %cid,
                    session_id = %session_id,
                    "FATM-09: Restored coupon reservation after TX begin failure"
                );
            }
            state.record_api_error("billing/start");
            return Json(json!({ "error": format!("DB error starting transaction: {}", e) }));
        }
    };

    let now = chrono::Utc::now();

    // Step 1: Debit wallet within the transaction (FATM-01, FATM-03)
    let wallet_debit_paise: Option<i64> = if !is_trial && final_price_paise > 0 {
        let debit_notes = if applied_discount_paise > 0 {
            format!("Session on Pod {} — {} credits discount", pod_num, applied_discount_paise / 100)
        } else {
            format!("Session on Pod {}", pod_num)
        };
        match wallet::debit_in_tx(
            &mut tx,
            &wallet_owner_id,
            final_price_paise,
            "debit_session",
            Some(&session_id),
            Some(&debit_notes),
            idempotency_key.as_deref(),
            &state.config.venue.venue_id,
        ).await {
            Ok(_) => Some(final_price_paise),
            Err(e) => {
                drop(tx);
                // FATM-09: Restore any reserved coupon so it can be used again
                if applied_coupon_id.is_some() {
                    let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
                    tracing::info!("FATM-09: Coupon restored after wallet debit failure for session {}", session_id);
                }
                state.record_api_error("billing/start");
                return Json(json!({ "error": e }));
            }
        }
    } else {
        None
    };

    // Step 2: INSERT billing session within the same transaction (FATM-01)
    let dynamic_price = if custom_price_paise.is_none() && !is_trial {
        // Compute dynamic pricing inside the tx (read-only query is fine)
        let dp = billing::compute_dynamic_price_in_tx(&mut tx, tier_price_paise).await;
        if dp != tier_price_paise { Some(dp) } else { None }
    } else {
        custom_price_paise.map(|p| p as i64)
    };

    // BILL-13: Insert with 'waiting_for_game' status — timer activated on AcStatus::Live
    if let Err(e) = sqlx::query(
        "INSERT INTO billing_sessions \
         (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, \
          started_at, staff_id, split_count, split_duration_minutes, \
          wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason, idempotency_key, \
          guardian_present, is_minor_session, venue_id, wallet_owner_id, billing_mode, rate_paise_per_minute) \
         VALUES (?, ?, ?, ?, ?, 'waiting_for_game', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(driver_id)
    .bind(&pod_id)
    .bind(&pricing_tier_id)
    .bind(allocated_seconds as i64)
    .bind(dynamic_price)
    .bind(now.to_rfc3339())
    .bind(&staff_id)
    .bind(final_split_count as i64)
    .bind(split_duration_minutes.map(|d| d as i64))
    .bind(wallet_debit_paise)
    .bind(applied_discount_paise)
    .bind(&applied_coupon_id)
    .bind(original_price_paise)
    .bind(&applied_discount_reason)
    .bind(idempotency_key.as_deref())
    .bind(guardian_present_flag)
    .bind(is_minor)
    .bind(&state.config.venue.venue_id)
    .bind(&wallet_owner_id)
    .bind(&tier_billing_mode)
    .bind(tier_rate_per_min)
    .execute(&mut *tx)
    .await {
        drop(tx); // rolls back wallet debit atomically
        // FATM-09: Restore any reserved coupon so it can be used again
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after session INSERT failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Failed to create billing session: {}", e) }));
    }

    // Step 3: Log billing events within the same transaction
    for event_type in ["created", "started"] {
        let _ = sqlx::query(
            "INSERT INTO billing_events (id, billing_session_id, event_type, driving_seconds_at_event, venue_id) VALUES (?, ?, ?, 0, ?)",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&session_id)
        .bind(event_type)
        .bind(&state.config.venue.venue_id)
        .execute(&mut *tx)
        .await;
    }

    // Step 4: Mark trial as used within the same transaction
    if is_trial && !unlimited_trials {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(driver_id)
            .execute(&mut *tx)
            .await;
    }

    // ─── Commit: all-or-nothing (FATM-01) ────────────────────────────────────
    if let Err(e) = tx.commit().await {
        // FATM-09: Restore any reserved coupon so it can be used again
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after commit failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Transaction commit failed: {}", e) }));
    }

    // ─── Post-commit: LEGAL-08 activity tracking — update last_activity_at ─────
    // Non-critical: failure does NOT affect billing. Keeps active customers from being
    // anonymized by the daily data-retention background job.
    let _ = sqlx::query(
        "UPDATE drivers SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await;

    // ─── Post-commit: record coupon redemption + mark coupon redeemed (FATM-08) ─
    if let Some(ref cid) = applied_coupon_id {
        record_coupon_redemption(&state, cid, driver_id, &session_id, applied_discount_paise).await;
        // FATM-08: Transition coupon to 'redeemed' now that session is committed
        let _ = redeem_coupon(&state.db, cid).await;
    }
    if applied_discount_paise > 0 {
        accounting::log_audit(
            &state,
            "billing_sessions",
            &session_id,
            "discount",
            None,
            Some(&serde_json::json!({
                "discount_paise": applied_discount_paise,
                "original_price_paise": original_price_paise,
                "reason": applied_discount_reason,
                "coupon_id": applied_coupon_id,
            }).to_string()),
            staff_id.as_deref(),
        )
        .await;
    }

    // ─── Post-commit: generate GST invoice (LEGAL-02) ────────────────────────
    // Invoice generation is non-critical — a failure here does NOT roll back the session.
    // The journal entry is also created here using the GST-separated accounting.
    if let Some(debit_paise) = wallet_debit_paise {
        match accounting::post_session_debit_gst(&state, driver_id, debit_paise, &session_id).await {
            Ok((_entry_id, net_paise, gst_paise)) => {
                if let Err(e) = accounting::generate_invoice(
                    &state,
                    &session_id,
                    driver_id,
                    &driver_name,
                    debit_paise,
                    net_paise,
                    gst_paise,
                )
                .await
                {
                    tracing::warn!(
                        session_id = %session_id,
                        "Invoice generation failed (non-critical): {}",
                        e
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %session_id,
                    "GST journal entry failed (non-critical): {}",
                    e
                );
            }
        }
    }

    // ─── BILL-13: Defer timer activation until game reaches AcStatus::Live ─────
    // Wallet debit + DB record already committed above (FATM-01).
    // Timer starts only when PlayableSignal received — customer not charged for load screens.
    let pod_id_for_defer = pod_id.clone();
    let billing_pod_id_clone = pod_id_for_defer.clone();
    let pod_id_for_audit = pod_id_for_defer.clone();
    billing::defer_billing_with_precommitted_session(&state, pod_id_for_defer, billing::BillingStartData {
        session_id: session_id.clone(),
        driver_id: driver_id.to_string(),
        driver_name,
        pod_id,
        pricing_tier_name: tier_name,
        allocated_seconds,
        split_count: final_split_count,
        split_duration_minutes,
        started_at: now, // placeholder — overwritten to game-live time on activation
        // Per-minute billing fields from pricing tier
        billing_mode: tier_billing_mode.clone(),
        rate_paise_per_minute: tier_rate_per_min.unwrap_or(0) as u32,
        hold_paise: if tier_billing_mode == "per_minute" { tier_hold.unwrap_or(10000) as u32 } else { 0 },
        wallet_owner_id: wallet_owner_id.clone(),
        low_balance_warning_paise: tier_low_warn.unwrap_or(5000) as u32,
    }).await;

    // Phase 307 AUDIT-03: Log billing session start for hash chain coverage
    crate::activity_log::log_pod_activity(
        &state,
        &billing_pod_id_clone,
        "billing",
        "Session Started",
        &format!("session_id={} driver={} tier={}", session_id, driver_id, pricing_tier_id),
        "core",
        Some(&session_id),
    );

    // Phase 283: Generate nonce for replay protection
    let billing_nonce = state.billing_nonce_store.generate(&session_id).await;

    // Phase 283: Audit log — billing session started
    crate::billing_replay::insert_audit_log(
        &state.db,
        &session_id,
        &pod_id_for_audit,
        "billing_start",
        "none",
        "waiting_for_game",
        Some(&billing_nonce),
        staff_id.as_deref().unwrap_or("system"),
        &state.config.venue.venue_id,
    )
    .await;

    Json(json!({
        "ok": true,
        "billing_session_id": session_id,
        "wallet_debit_paise": wallet_debit_paise,
        "discount_paise": applied_discount_paise,
        "original_price_paise": original_price_paise,
        "discount_reason": applied_discount_reason,
        "discount_floor_paise": billing::DISCOUNT_FLOOR_PAISE,
        "allocated_seconds": allocated_seconds,
        "nonce": billing_nonce,
    }))
}
