//! Billing start — HTTP handler + atomic transaction core.
//!
//! Split into 3 modules (Phase 385):
//! - `billing_start_validate`   — input parsing, lock acquisition, pre-TX validation
//! - `billing_start`            — (this file) HTTP handler + atomic transaction
//! - `billing_start_postcommit` — post-commit side effects (invoice, audit, defer timer)

#![allow(unused_imports)]
use super::billing_coupon::{validate_and_calc_coupon, reserve_coupon, restore_coupon_on_cancel};
use super::billing_start_postcommit::{PostCommitParams, run_post_commit};
use super::billing_start_validate::*;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::billing;
use crate::wallet;
use crate::state::AppState;
use rc_common::pod_id::normalize_pod_id;

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

/// Inner billing start — validates, runs transaction, then post-commit effects.
pub(crate) async fn start_billing_inner(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // ─── Phase 1: Parse + Validate ───────────────────────────────────────────
    let input = match parse_billing_input(&body) {
        Ok(i) => i,
        Err(e) => return e,
    };

    let _locks = match acquire_billing_locks(&state, &input.driver_id, &input.pod_id).await {
        Ok(l) => l,
        Err(e) => return e,
    };

    // FATM-02: Idempotency check
    if let Some(ref key) = input.idempotency_key
        && let Some(replay) = check_idempotency(&state, key).await {
            return replay;
        }

    // BATOM-02: Pre-validate in-memory maps
    if let Err(e) = check_pod_not_active(&state, &input.pod_id).await {
        return e;
    }

    let tier = match lookup_tier(&state, &input.pricing_tier_id).await {
        Ok(t) => t,
        Err(e) => return e,
    };

    let driver = match lookup_and_validate_driver(&state, &input.driver_id, &input.pod_id, input.guardian_present_flag).await {
        Ok(d) => d,
        Err(e) => return e,
    };

    if let Err(e) = check_trial_eligibility(&state, &input.driver_id, driver.has_used_trial, driver.unlimited_trials, tier.is_trial).await {
        return e;
    }

    if let Err(e) = validate_pod_exists(&state, &input.pod_id).await {
        return e;
    }

    let (final_split_count, allocated_seconds) = match validate_splits_and_duration(&input, tier.duration_minutes) {
        Ok(v) => v,
        Err(e) => return e,
    };

    // ─── Phase 2: Pricing + Discounts ────────────────────────────────────────
    let mut base_price_paise = input.custom_price_paise.map(|p| p as i64).unwrap_or(tier.price_paise);

    // Group discount: if 3+ sessions active, 4th+ gets group multiplier
    let mut group_discount_paise: i64 = 0;
    let active_count = {
        let timers = state.billing.active_timers.read().await;
        timers.len()
    };
    if !tier.is_trial && active_count >= 3 {
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

    // FATM-08: Generate session_id early for coupon reservation
    let session_id = uuid::Uuid::new_v4().to_string();

    // Apply coupon or staff discount
    let mut applied_discount_paise: i64 = group_discount_paise;
    let mut applied_coupon_id: Option<String> = None;
    let mut applied_discount_reason: Option<String> = if group_discount_paise > 0 {
        Some(format!("Group {} sessions (11% off)", active_count + 1))
    } else {
        None
    };

    if let Some(ref code) = input.coupon_code {
        match validate_and_calc_coupon(&state, code, &input.driver_id, base_price_paise).await {
            Ok(cd) => {
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
    } else if let Some(staff_disc) = input.staff_discount_paise
        && staff_disc > 0 && staff_disc <= base_price_paise {
            applied_discount_paise += staff_disc;
            let staff_desc = input.discount_reason.clone().unwrap_or("Staff discount".to_string());
            applied_discount_reason = Some(match applied_discount_reason {
                Some(existing) => format!("{} + {}", existing, staff_desc),
                None => staff_desc,
            });
        }

    let original_price_paise = input.custom_price_paise.map(|p| p as i64).unwrap_or(tier.price_paise);
    let mut final_price_paise = original_price_paise - applied_discount_paise;

    // FATM-10: Enforce discount floor
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

    let wallet_owner_id = match resolve_wallet_owner(&state, &input.driver_id).await {
        Ok(id) => id,
        Err(e) => return e,
    };

    if let Err(e) = pre_check_wallet_balance(&state, &wallet_owner_id, final_price_paise, tier.is_trial).await {
        return e;
    }

    // Fetch pod number for debit notes
    let pod_num = sqlx::query_as::<_, (i64,)>("SELECT number FROM pods WHERE id = ?")
        .bind(&input.pod_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or(0);

    // ─── Phase 3: Atomic Transaction ─────────────────────────────────────────
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            if let Some(ref cid) = applied_coupon_id {
                let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
                tracing::warn!(coupon_id = %cid, session_id = %session_id,
                    "FATM-09: Restored coupon reservation after TX begin failure");
            }
            state.record_api_error("billing/start");
            return Json(json!({ "error": format!("DB error starting transaction: {}", e) }));
        }
    };

    let now = chrono::Utc::now();

    // Step 1: Debit wallet (FATM-01, FATM-03)
    let wallet_debit_paise: Option<i64> = if !tier.is_trial && final_price_paise > 0 {
        let debit_notes = if applied_discount_paise > 0 {
            format!("Session on Pod {} — {} credits discount", pod_num, applied_discount_paise / 100)
        } else {
            format!("Session on Pod {}", pod_num)
        };
        match wallet::debit_in_tx(
            &mut tx, &wallet_owner_id, final_price_paise, "debit_session",
            Some(&session_id), Some(&debit_notes),
            input.idempotency_key.as_deref(), &state.config.venue.venue_id,
        ).await {
            Ok(_) => Some(final_price_paise),
            Err(e) => {
                drop(tx);
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

    // Step 2: INSERT billing session (FATM-01)
    let dynamic_price = if input.custom_price_paise.is_none() && !tier.is_trial {
        let dp = billing::compute_dynamic_price_in_tx(&mut tx, tier.price_paise).await;
        if dp != tier.price_paise { Some(dp) } else { None }
    } else {
        input.custom_price_paise.map(|p| p as i64)
    };

    if let Err(e) = sqlx::query(
        "INSERT INTO billing_sessions \
         (id, driver_id, pod_id, pricing_tier_id, allocated_seconds, status, custom_price_paise, \
          started_at, staff_id, split_count, split_duration_minutes, \
          wallet_debit_paise, discount_paise, coupon_id, original_price_paise, discount_reason, idempotency_key, \
          guardian_present, is_minor_session, venue_id, wallet_owner_id, billing_mode, rate_paise_per_minute) \
         VALUES (?, ?, ?, ?, ?, 'waiting_for_game', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&session_id)
    .bind(&input.driver_id)
    .bind(&input.pod_id)
    .bind(&input.pricing_tier_id)
    .bind(allocated_seconds as i64)
    .bind(dynamic_price)
    .bind(now.to_rfc3339())
    .bind(&input.staff_id)
    .bind(final_split_count as i64)
    .bind(input.split_duration_minutes.map(|d| d as i64))
    .bind(wallet_debit_paise)
    .bind(applied_discount_paise)
    .bind(&applied_coupon_id)
    .bind(original_price_paise)
    .bind(&applied_discount_reason)
    .bind(input.idempotency_key.as_deref())
    .bind(input.guardian_present_flag)
    .bind(driver.is_minor)
    .bind(&state.config.venue.venue_id)
    .bind(&wallet_owner_id)
    .bind(&tier.billing_mode)
    .bind(tier.rate_per_min)
    .execute(&mut *tx)
    .await {
        drop(tx);
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after session INSERT failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Failed to create billing session: {}", e) }));
    }

    // Step 3: Log billing events
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

    // Step 4: Mark trial as used
    if tier.is_trial && !driver.unlimited_trials {
        let _ = sqlx::query("UPDATE drivers SET has_used_trial = 1, updated_at = datetime('now') WHERE id = ?")
            .bind(&input.driver_id)
            .execute(&mut *tx)
            .await;
    }

    // Commit: all-or-nothing (FATM-01)
    if let Err(e) = tx.commit().await {
        if applied_coupon_id.is_some() {
            let _ = restore_coupon_on_cancel(&state.db, &session_id).await;
            tracing::info!("FATM-09: Coupon restored after commit failure for session {}", session_id);
        }
        state.record_api_error("billing/start");
        return Json(json!({ "error": format!("Transaction commit failed: {}", e) }));
    }

    // ─── Phase 4: Post-commit side effects ───────────────────────────────────
    let billing_nonce = run_post_commit(&state, PostCommitParams {
        session_id: &session_id,
        driver_id: &input.driver_id,
        driver_name: driver.name,
        pod_id: input.pod_id,
        pricing_tier_id: &input.pricing_tier_id,
        staff_id: input.staff_id,
        wallet_debit_paise,
        applied_discount_paise,
        original_price_paise,
        applied_discount_reason: applied_discount_reason.clone(),
        applied_coupon_id,
        allocated_seconds,
        final_split_count,
        split_duration_minutes: input.split_duration_minutes,
        now,
        tier_name: tier.name,
        tier_billing_mode: tier.billing_mode,
        tier_rate_per_min: tier.rate_per_min,
        tier_hold: tier.hold,
        tier_low_warn: tier.low_warn,
        wallet_owner_id,
    }).await;

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
