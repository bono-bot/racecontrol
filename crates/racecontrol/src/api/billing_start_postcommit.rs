//! Billing start — post-commit side effects (non-critical).
//!
//! Extracted from `billing_start.rs` (Phase 385 split) to keep each module under 500 lines.
//! All operations here run AFTER the billing transaction has committed successfully.
//! Failures are logged but do NOT roll back the session.

use std::sync::Arc;

use crate::accounting;
use crate::billing;
use crate::state::AppState;

use super::billing_coupon::{record_coupon_redemption, redeem_coupon};

// ─── Post-commit side effects ────────────────────────────────────────────────

/// Parameters for post-commit processing.
pub(crate) struct PostCommitParams<'a> {
    pub session_id: &'a str,
    pub driver_id: &'a str,
    pub driver_name: String,
    pub pod_id: String,
    pub pricing_tier_id: &'a str,
    pub staff_id: Option<String>,
    pub wallet_debit_paise: Option<i64>,
    pub applied_discount_paise: i64,
    pub original_price_paise: i64,
    pub applied_discount_reason: Option<String>,
    pub applied_coupon_id: Option<String>,
    pub allocated_seconds: u32,
    pub final_split_count: u32,
    pub split_duration_minutes: Option<u32>,
    pub now: chrono::DateTime<chrono::Utc>,
    pub tier_name: String,
    pub tier_billing_mode: String,
    pub tier_rate_per_min: Option<i64>,
    pub tier_hold: Option<i64>,
    pub tier_low_warn: Option<i64>,
    pub wallet_owner_id: String,
}

/// Run all post-commit side effects. Returns the billing nonce for the response.
pub(crate) async fn run_post_commit(
    state: &Arc<AppState>,
    params: PostCommitParams<'_>,
) -> String {
    // LEGAL-08: Update last_activity_at
    let _ = sqlx::query(
        "UPDATE drivers SET last_activity_at = datetime('now') WHERE id = ?",
    )
    .bind(params.driver_id)
    .execute(&state.db)
    .await;

    // Record coupon redemption + mark redeemed (FATM-08)
    if let Some(ref cid) = params.applied_coupon_id {
        record_coupon_redemption(state, cid, params.driver_id, params.session_id, params.applied_discount_paise).await;
        let _ = redeem_coupon(&state.db, cid).await;
    }

    // Audit log for discounts
    if params.applied_discount_paise > 0 {
        accounting::log_audit(
            state,
            "billing_sessions",
            params.session_id,
            "discount",
            None,
            Some(&serde_json::json!({
                "discount_paise": params.applied_discount_paise,
                "original_price_paise": params.original_price_paise,
                "reason": params.applied_discount_reason,
                "coupon_id": params.applied_coupon_id,
            }).to_string()),
            params.staff_id.as_deref(),
        )
        .await;
    }

    // GST invoice (LEGAL-02)
    if let Some(debit_paise) = params.wallet_debit_paise {
        match accounting::post_session_debit_gst(state, params.driver_id, debit_paise, params.session_id).await {
            Ok((_entry_id, net_paise, gst_paise)) => {
                if let Err(e) = accounting::generate_invoice(
                    state,
                    params.session_id,
                    params.driver_id,
                    &params.driver_name,
                    debit_paise,
                    net_paise,
                    gst_paise,
                )
                .await
                {
                    tracing::warn!(
                        session_id = %params.session_id,
                        "Invoice generation failed (non-critical): {}",
                        e
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    session_id = %params.session_id,
                    "GST journal entry failed (non-critical): {}",
                    e
                );
            }
        }
    }

    // BILL-13: Defer timer activation until game reaches AcStatus::Live
    let pod_id_for_defer = params.pod_id.clone();
    let billing_pod_id_clone = pod_id_for_defer.clone();
    let pod_id_for_audit = pod_id_for_defer.clone();
    billing::defer_billing_with_precommitted_session(state, pod_id_for_defer, billing::BillingStartData {
        session_id: params.session_id.to_string(),
        driver_id: params.driver_id.to_string(),
        driver_name: params.driver_name,
        pod_id: params.pod_id,
        pricing_tier_name: params.tier_name,
        allocated_seconds: params.allocated_seconds,
        split_count: params.final_split_count,
        split_duration_minutes: params.split_duration_minutes,
        started_at: params.now,
        billing_mode: params.tier_billing_mode.clone(),
        rate_paise_per_minute: params.tier_rate_per_min.unwrap_or(0) as u32,
        hold_paise: if params.tier_billing_mode == "per_minute" { params.tier_hold.unwrap_or(10000) as u32 } else { 0 },
        wallet_owner_id: params.wallet_owner_id,
        low_balance_warning_paise: params.tier_low_warn.unwrap_or(5000) as u32,
    }).await;

    // Phase 307 AUDIT-03: Log billing session start for hash chain coverage
    crate::activity_log::log_pod_activity(
        state,
        &billing_pod_id_clone,
        "billing",
        "Session Started",
        &format!("session_id={} driver={} tier={}", params.session_id, params.driver_id, params.pricing_tier_id),
        "core",
        Some(params.session_id),
    );

    // Phase 283: Generate nonce for replay protection
    let billing_nonce = state.billing_nonce_store.generate(params.session_id).await;

    // Phase 283: Audit log — billing session started
    crate::billing_replay::insert_audit_log(
        &state.db,
        params.session_id,
        &pod_id_for_audit,
        "billing_start",
        "none",
        "waiting_for_game",
        Some(&billing_nonce),
        params.staff_id.as_deref().unwrap_or("system"),
        &state.config.venue.venue_id,
    )
    .await;

    billing_nonce
}
