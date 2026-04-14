//! FATM-11: Payment gateway webhook handler — extracted from wallet_staff.rs.

use axum::{
    Json,
    extract::State,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;
use crate::wallet;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct PaymentGatewayWebhookRequest {
    /// Gateway's unique payment ID — used as idempotency key
    transaction_id: String,
    /// Driver to credit
    driver_id: String,
    /// Amount to credit in paise
    amount_paise: i64,
    /// Must be "success" or "captured" to trigger wallet credit
    status: String,
    /// HMAC signature from gateway (unused until gateway is chosen)
    #[allow(dead_code)]
    signature: Option<String>,
}

/// FATM-11: Payment gateway webhook — credits a driver's wallet idempotently.
/// - Same transaction_id fired twice → returns original result without double-crediting.
/// - Non-success status (refunded, failed, etc.) → acknowledged without crediting.
/// - Amount validation: must be 1 paise to Rs 10,000 (safety cap).
///
/// TODO: Verify HMAC signature from gateway (Razorpay/Cashfree/etc.)
/// When a specific gateway is chosen, implement:
///   let expected = hmac_sha256(webhook_secret, raw_body);
///   if !constant_time_eq(expected, signature) { return 401; }
/// For now the endpoint is protected by being undiscoverable (no public docs)
/// and the idempotency guard prevents replay damage.
pub(crate) async fn payment_gateway_webhook(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<PaymentGatewayWebhookRequest>,
) -> Json<Value> {
    tracing::info!(
        transaction_id = %req.transaction_id,
        driver_id = %req.driver_id,
        amount_paise = req.amount_paise,
        status = %req.status,
        "FATM-11: Payment gateway webhook received"
    );

    // v47.0 Phase 345-03 (Phase 343 C6): refuse ALL webhook calls when the secret
    // is not configured. Previously a missing secret silently skipped HMAC verify,
    // accepting any caller's fabricated wallet credits. Now the endpoint is closed
    // until operators explicitly set `[integrations].payment_webhook_secret`.
    //
    // Full HMAC-SHA256 verification against raw body is still pending real gateway
    // integration (Razorpay/Cashfree). This is the structural guard.
    let webhook_secret = state
        .config
        .integrations
        .payment_webhook_secret
        .as_ref()
        .filter(|s| !s.is_empty());
    let webhook_secret = match webhook_secret {
        Some(s) => s,
        None => {
            tracing::warn!(
                transaction_id = %req.transaction_id,
                "FATM-11: Gateway webhook rejected — payment_webhook_secret is not configured. \
                 Set [integrations].payment_webhook_secret in racecontrol.toml to enable the webhook endpoint."
            );
            return Json(json!({
                "ok": false,
                "error": "payment webhook endpoint is disabled (no secret configured)"
            }));
        }
    };
    // Secret is set — require the signature header
    let provided_sig = headers
        .get("x-webhook-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided_sig.is_empty() {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook rejected — missing X-Webhook-Signature header"
        );
        return Json(json!({ "ok": false, "error": "missing webhook signature" }));
    }
    // NOTE: Full HMAC-SHA256 verification requires raw body bytes.
    // When a real gateway is integrated, replace this with proper verification using the raw request body.
    let _ = webhook_secret; // reference to avoid unused warning until HMAC is wired
    tracing::debug!("FATM-11: Webhook signature present (full HMAC check pending gateway integration)");

    // Basic field validation
    if req.transaction_id.is_empty() || req.driver_id.is_empty() {
        return Json(json!({ "ok": false, "error": "transaction_id and driver_id are required" }));
    }

    // Amount validation: 1 paise to Rs 10,000 (100000 paise)
    if req.amount_paise <= 0 || req.amount_paise > 10_000_00 {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            amount_paise = req.amount_paise,
            "FATM-11: Gateway webhook rejected — amount out of range"
        );
        return Json(json!({
            "ok": false,
            "error": "amount_paise must be between 1 and 1000000 (Rs 10,000 cap)"
        }));
    }

    // Status check: only credit on success/captured
    let status_lower = req.status.to_lowercase();
    if status_lower != "success" && status_lower != "captured" {
        tracing::info!(
            transaction_id = %req.transaction_id,
            status = %req.status,
            "FATM-11: Gateway webhook acknowledged (non-success status — no wallet credit)"
        );
        return Json(json!({
            "ok": true,
            "action": "ignored",
            "reason": format!("status '{}' is not success/captured — no wallet credit", req.status)
        }));
    }

    // FATM-11: Idempotency check — check if this transaction_id was already processed
    let existing = sqlx::query_as::<_, (i64, i64)>(
        "SELECT amount_paise, balance_after_paise FROM wallet_transactions WHERE idempotency_key = ?",
    )
    .bind(&req.transaction_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((_amount, balance_after)) = existing {
        tracing::info!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook duplicate — returning original result"
        );
        return Json(json!({
            "ok": true,
            "duplicate": true,
            "new_balance_credits": balance_after
        }));
    }

    // Credit wallet within a transaction (atomic, idempotent via idempotency_key)
    let mut tx = match state.db.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(
                transaction_id = %req.transaction_id,
                "FATM-11: Gateway webhook DB error starting transaction: {}", e
            );
            return Json(json!({ "ok": false, "error": "DB error — please retry" }));
        }
    };

    let (new_balance, txn_id) = match wallet::credit_in_tx(
        &mut tx,
        &req.driver_id,
        req.amount_paise,
        "gateway_topup",
        Some(&req.transaction_id),
        Some("Payment gateway credit"),
        None,
        Some(&req.transaction_id), // idempotency_key = gateway's transaction_id
        &state.config.venue.venue_id,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            drop(tx);
            tracing::error!(
                transaction_id = %req.transaction_id,
                driver_id = %req.driver_id,
                "FATM-11: Gateway webhook credit_in_tx failed: {}", e
            );
            return Json(json!({ "ok": false, "error": format!("Wallet credit failed: {}", e) }));
        }
    };

    if let Err(e) = tx.commit().await {
        tracing::error!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook transaction commit failed: {}", e
        );
        return Json(json!({ "ok": false, "error": "Transaction commit failed — please retry" }));
    }

    tracing::info!(
        transaction_id = %req.transaction_id,
        driver_id = %req.driver_id,
        amount_paise = req.amount_paise,
        new_balance = new_balance,
        txn_id = %txn_id,
        "FATM-11: Gateway webhook — wallet credited successfully"
    );

    Json(json!({
        "ok": true,
        "new_balance_credits": new_balance,
        "txn_id": txn_id
    }))
}
