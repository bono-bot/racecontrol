#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Path, State},
};
use serde_json::{Value, json};
use std::sync::Arc;

use super::driver_routes::{mask_phone, mask_email, should_mask_pii};
use crate::state::AppState;

/// GET /drivers/{id}/full-profile — comprehensive driver profile for admin
pub(crate) async fn get_driver_full_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    claims: Option<axum::Extension<crate::auth::middleware::StaffClaims>>,
) -> Json<Value> {
    let mask = should_mask_pii(&claims);
    // Core driver info (10 fields)
    let core = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64, Option<String>, Option<String>, bool, Option<String>)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms,
                customer_id, nickname, COALESCE(has_used_trial, 0), dob
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let c = match core {
        Ok(Some(c)) => c,
        Ok(None) => return Json(json!({ "error": "Driver not found" })),
        Err(e) => return Json(json!({ "error": format!("DB error: {}", e) })),
    };

    // Waiver fields (separate query to stay under tuple limit)
    let waiver = sqlx::query_as::<_, (bool, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, bool)>(
        "SELECT COALESCE(waiver_signed, 0), waiver_signed_at, waiver_version,
                guardian_name, guardian_phone, signature_data,
                COALESCE(show_nickname_on_leaderboard, 0)
         FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .unwrap_or((false, None, None, None, None, None, false));

    let is_minor = c.9.as_ref().is_some_and(|dob| {
        chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d")
            .map(|date| (chrono::Utc::now().date_naive() - date).num_days() / 365 < 18)
            .unwrap_or(false)
    });

    // SEC-09: mask PII for cashier role
    let email = c.2.as_deref().map(|e| if mask { mask_email(e) } else { e.to_string() });
    let phone = c.3.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });
    let guardian_phone = waiver.4.as_deref().map(|p| if mask { mask_phone(p) } else { p.to_string() });

    let driver_json = json!({
        "id": c.0, "name": c.1, "email": email, "phone": phone,
        "total_laps": c.4, "total_time_ms": c.5,
        "customer_id": c.6, "nickname": c.7, "has_used_trial": c.8,
        "dob": c.9,
        "waiver_signed": waiver.0, "waiver_signed_at": waiver.1,
        "waiver_version": waiver.2, "guardian_name": waiver.3,
        "guardian_phone": guardian_phone, "has_signature": waiver.5.is_some(),
        "show_nickname_on_leaderboard": waiver.6, "is_minor": is_minor,
    });

    // Wallet
    let wallet = sqlx::query_as::<_, (i64, i64, i64, Option<String>)>(
        "SELECT balance_paise, total_credited_paise, total_debited_paise, updated_at FROM wallets WHERE driver_id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|w| json!({
        "balance_paise": w.0, "total_credited_paise": w.1,
        "total_debited_paise": w.2, "updated_at": w.3,
    }));

    // Recent wallet transactions (last 20)
    let txns = sqlx::query_as::<_, (String, i64, i64, String, Option<String>, Option<String>, Option<String>)>(
        "SELECT id, amount_paise, balance_after_paise, txn_type, reference_id, notes, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|t| json!({
        "id": t.0, "amount_paise": t.1, "balance_after_paise": t.2,
        "txn_type": t.3, "reference_id": t.4, "notes": t.5, "created_at": t.6,
    }))
    .collect::<Vec<_>>();

    // Billing sessions (last 20)
    let sessions = sqlx::query_as::<_, (String, String, i64, i64, String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status,
                bs.wallet_debit_paise, bs.started_at, bs.ended_at, pt.name
         FROM billing_sessions bs
         LEFT JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.driver_id = ?
         ORDER BY bs.started_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|s| json!({
        "id": s.0, "pod_id": s.1, "allocated_seconds": s.2,
        "driving_seconds": s.3, "status": s.4, "wallet_debit_paise": s.5,
        "started_at": s.6, "ended_at": s.7, "pricing_tier_name": s.8,
    }))
    .collect::<Vec<_>>();

    // Personal bests
    let pbs = sqlx::query_as::<_, (String, String, i64, Option<String>)>(
        "SELECT track, car, best_lap_ms, achieved_at FROM personal_bests WHERE driver_id = ? ORDER BY achieved_at DESC LIMIT 20"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|p| json!({ "track": p.0, "car": p.1, "best_lap_ms": p.2, "achieved_at": p.3 }))
    .collect::<Vec<_>>();

    // Referral info
    let referral = sqlx::query_as::<_, (String,)>(
        "SELECT code FROM referrals WHERE referrer_id = ? AND code IS NOT NULL LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0);

    let referral_count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM referrals WHERE referrer_id = ? AND status = 'completed'"
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    // Membership
    let membership = sqlx::query_as::<_, (String, String, f64, f64, String, bool, String)>(
        "SELECT m.id, mt.name, m.hours_used_minutes, mt.hours_included, m.expires_at, m.auto_renew, m.status
         FROM memberships m JOIN membership_tiers mt ON m.tier_id = mt.id
         WHERE m.driver_id = ? AND m.status = 'active' LIMIT 1"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|m| json!({
        "id": m.0, "tier_name": m.1, "hours_used": m.2,
        "hours_included": m.3, "expires_at": m.4, "auto_renew": m.5, "status": m.6,
    }));

    // Refunds
    let refunds = sqlx::query_as::<_, (String, i64, String, String, Option<String>, String)>(
        "SELECT r.billing_session_id, r.amount_paise, r.method, r.reason, r.notes, r.created_at
         FROM refunds r WHERE r.driver_id = ? ORDER BY r.created_at DESC LIMIT 10"
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .iter()
    .map(|r| json!({
        "billing_session_id": r.0, "amount_paise": r.1, "method": r.2,
        "reason": r.3, "notes": r.4, "created_at": r.5,
    }))
    .collect::<Vec<_>>();

    Json(json!({
        "driver": driver_json,
        "wallet": wallet,
        "transactions": txns,
        "sessions": sessions,
        "personal_bests": pbs,
        "referral_code": referral,
        "referral_count": referral_count,
        "membership": membership,
        "refunds": refunds,
    }))
}
