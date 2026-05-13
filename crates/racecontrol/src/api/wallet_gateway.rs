//! FATM-11: Payment gateway webhook handler — extracted from wallet_staff.rs.
//!
//! §S-260 Atom 2 — HMAC-SHA256 verification + timestamp drift/replay guard.
//! Cluster RCA I-3 HIGH BLOCKING pre-merge fix. MMA Step 1 4/4 unanimous flagged
//! the pre-fix "secret-is-set" structural-only guard as insufficient.
//!
//! Canonical signing input (per-field concatenation; avoids JSON canonicalization
//! ambiguity flagged by MMA Kimi as "signature wrapping" vulnerability):
//!   `{transaction_id}|{driver_id}|{amount_paise}|{status}|{timestamp_unix_secs}`
//!
//! Compose with real gateways (Razorpay/Cashfree/etc.): adapt this canonical
//! string to match the chosen gateway's HMAC documentation.

use axum::{
    Json,
    extract::State,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const HMAC_MAX_DRIFT_SECS: i64 = 300; // ±5 min — replay-window beyond idempotency_key

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
    // Secret is set — require the signature + timestamp headers
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
    let provided_ts = headers
        .get("x-webhook-timestamp")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if provided_ts.is_empty() {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook rejected — missing X-Webhook-Timestamp header"
        );
        return Json(json!({ "ok": false, "error": "missing webhook timestamp" }));
    }

    // §S-260 Atom 2 — Timestamp drift guard (replay-prevention beyond idempotency_key).
    // Reject if |now - provided_ts| > 5min. Bounds the replay window for an attacker
    // who has captured a valid (signature, timestamp) pair.
    let now_unix: i64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let ts_unix: i64 = match provided_ts.parse::<i64>() {
        Ok(v) => v,
        Err(_) => {
            tracing::warn!(
                transaction_id = %req.transaction_id,
                "FATM-11: Gateway webhook rejected — X-Webhook-Timestamp must be Unix seconds integer"
            );
            return Json(json!({ "ok": false, "error": "webhook timestamp must be unix seconds" }));
        }
    };
    if ts_unix <= 0 || (now_unix - ts_unix).abs() > HMAC_MAX_DRIFT_SECS {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            now_unix = now_unix,
            ts_unix = ts_unix,
            drift_secs = (now_unix - ts_unix).abs(),
            "FATM-11: Gateway webhook rejected — timestamp drift > 5min (replay-prevention)"
        );
        return Json(json!({ "ok": false, "error": "webhook timestamp stale or out of range" }));
    }

    // §S-260 Atom 2 — Canonical signing input (per-field concatenation; avoids
    // JSON canonicalization ambiguity per MMA Kimi finding). Real gateways will
    // dictate their own format; adapt this line when integrating.
    let canonical = format!(
        "{}|{}|{}|{}|{}",
        req.transaction_id, req.driver_id, req.amount_paise, req.status, provided_ts
    );

    // §S-260 Atom 2 — Compute expected HMAC-SHA256 + constant-time compare.
    let provided_sig_bytes = match hex::decode(provided_sig) {
        Ok(b) => b,
        Err(_) => {
            tracing::warn!(
                transaction_id = %req.transaction_id,
                "FATM-11: Gateway webhook rejected — signature must be hex-encoded"
            );
            return Json(json!({ "ok": false, "error": "signature must be hex-encoded" }));
        }
    };
    let mut mac = match HmacSha256::new_from_slice(webhook_secret.as_bytes()) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(
                transaction_id = %req.transaction_id,
                "FATM-11: HMAC init failed: {}", e
            );
            return Json(json!({ "ok": false, "error": "internal HMAC init error" }));
        }
    };
    mac.update(canonical.as_bytes());
    if mac.verify_slice(&provided_sig_bytes).is_err() {
        tracing::warn!(
            transaction_id = %req.transaction_id,
            "FATM-11: Gateway webhook rejected — HMAC signature mismatch"
        );
        return Json(json!({ "ok": false, "error": "invalid webhook signature" }));
    }
    tracing::debug!(
        transaction_id = %req.transaction_id,
        "FATM-11: HMAC-SHA256 verification passed (Atom 2)"
    );

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

    // §S-281 D-PHASE-γ-1 Atom 7 — INSERT-then-handle-conflict idempotency pattern.
    //
    // Replaces the prior pre-flight SELECT-OUTSIDE-tx pattern (I-13 race class):
    // two concurrent webhooks with the same idempotency_key both used to SELECT
    // (finding no row) → both proceeded to credit_in_tx → second one's INSERT
    // failed on UNIQUE constraint and returned 500 → caller had to retry to get
    // the duplicate-response 200. Wallet balance was already race-safe via the
    // UNIQUE INDEX + rollback, but the caller-visible retry-required symptom
    // was the I-13 idempotency persistence retry-race surface.
    //
    // New pattern: BEGIN tx → credit_in_tx (which INSERTs wallet_transactions
    // with idempotency_key) → on Err detected as UNIQUE-conflict, ROLLBACK +
    // re-SELECT cached row + return duplicate-response 200 immediately (no
    // retry required, no caller-visible race).
    //
    // D-PHASE-γ-5 NO-MIGRATION — UNIQUE INDEX `idx_wallet_txn_idempotency` is
    // already present at `crates/racecontrol/src/db/migrate_billing.rs:865-868`
    // (Phase 252 FATM-02 family · james §S-274 C1 VERIFIED). Pure-code refactor.
    //
    // D-PHASE-γ-8 amount-validation — on UNIQUE-conflict + cached-row fetch,
    // reject if `req.amount_paise != cached_amount` (closes the
    // idempotency_key-reuse-with-different-amount false-positive class flagged
    // by MMA Step 1 NVIDIA Q2 and ratified in §S-280 D-PHASE-γ-8).
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
            let is_conflict = is_idempotency_conflict(&e);
            // MAOR-T1 IMPORTANT-2 — `drop(tx)` triggers ROLLBACK via sqlx::Transaction's
            // Drop impl; the pool connection is returned synchronously before the
            // post-conflict SELECT below borrows a new connection. The wallet UPDATE
            // executed inside `credit_in_tx` (step 1 before the failing INSERT) is
            // reversed by this rollback, so the loser tx leaves wallet state unchanged.
            drop(tx);

            if is_conflict {
                // D-PHASE-γ-1 conflict-handling: UNIQUE-conflict means another
                // tx committed the same idempotency_key first. Fetch cached
                // row deterministically (now guaranteed visible outside tx
                // after rollback) + amount-validate + return duplicate-response.
                //
                // MAOR-T1 POLISH-2 — propagate DB errors explicitly. Previous
                // `.ok().flatten()` swallowed sqlx::Error, masking transient DB
                // outages as misleading "cached row missing" responses.
                //
                // MMA-Step2 Edge-case-2 (nvidia+qwen 2-of-2 consensus) — also
                // SELECT driver_id from the cached row. On UNIQUE-conflict, the
                // cached row may carry a DIFFERENT driver_id (gateway integration
                // bug: same transaction_id reused across drivers). Returning the
                // cached balance would expose the wrong driver's wallet state.
                // Reject with bad-request if driver_id mismatches.
                let cached_result = sqlx::query_as::<_, (i64, i64, String)>(
                    "SELECT amount_paise, balance_after_paise, driver_id FROM wallet_transactions WHERE idempotency_key = ?",
                )
                .bind(&req.transaction_id)
                .fetch_optional(&state.db)
                .await;

                let cached = match cached_result {
                    Ok(opt) => opt,
                    Err(db_err) => {
                        tracing::error!(
                            transaction_id = %req.transaction_id,
                            "FATM-11: post-conflict cached-row SELECT failed: {}", db_err
                        );
                        return Json(json!({ "ok": false, "error": "DB error — please retry" }));
                    }
                };

                return match cached {
                    Some((cached_amount, balance_after, cached_driver_id)) => {
                        // MMA-Step2 Edge-case-2 — driver_id mismatch check
                        // (defends against gateway integration bug reusing
                        // transaction_id across drivers).
                        if cached_driver_id != req.driver_id {
                            tracing::warn!(
                                transaction_id = %req.transaction_id,
                                cached_driver_id = %cached_driver_id,
                                requested_driver_id = %req.driver_id,
                                "FATM-11: idempotency_key reused with different driver_id — rejecting (gateway integration bug?)"
                            );
                            return Json(json!({
                                "ok": false,
                                "error": "idempotency_key reused with different driver_id"
                            }));
                        }
                        // D-PHASE-γ-8 amount-validation on cached-row match.
                        if cached_amount != req.amount_paise {
                            tracing::warn!(
                                transaction_id = %req.transaction_id,
                                cached_amount = cached_amount,
                                requested_amount = req.amount_paise,
                                "FATM-11: idempotency_key reused with different amount_paise — rejecting"
                            );
                            return Json(json!({
                                "ok": false,
                                "error": "idempotency_key reused with different amount_paise"
                            }));
                        }
                        tracing::info!(
                            transaction_id = %req.transaction_id,
                            "FATM-11: Gateway webhook duplicate (via INSERT-conflict) — returning cached result"
                        );
                        Json(json!({
                            "ok": true,
                            "duplicate": true,
                            "new_balance_paise": balance_after
                        }))
                    }
                    None => {
                        // Unexpected race: UNIQUE-conflict but no row visible
                        // post-rollback. Structurally possible only via concurrent
                        // DELETE on wallet_transactions, which is not permitted by
                        // any V2 code path. Reaching here indicates server-side
                        // inconsistency requiring operator investigation, not a
                        // retryable client error.
                        //
                        // MAOR-T1 POLISH-1 — message corrected from misleading
                        // "please retry" (would infinite-loop) to operator-investigate.
                        tracing::error!(
                            transaction_id = %req.transaction_id,
                            "FATM-11: UNIQUE-conflict but cached row not found post-rollback — unexpected race, operator investigate"
                        );
                        Json(json!({
                            "ok": false,
                            "error": "idempotency conflict but cached row missing — unexpected server error, contact support"
                        }))
                    }
                };
            }

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
        "new_balance_paise": new_balance,
        "txn_id": txn_id
    }))
}

/// §S-281 D-PHASE-γ-1 Atom 7 — detect UNIQUE-constraint violation on
/// `wallet_transactions.idempotency_key` from `wallet::credit_in_tx` error.
///
/// `credit_in_tx` returns `Result<_, String>` where the formatted error
/// embeds the sqlx-rendered SQLite error message (per `wallet.rs:190`:
/// `format!("DB error recording transaction: {}", e)`). SQLite emits
/// `UNIQUE constraint failed: wallet_transactions.idempotency_key` on the
/// idempotency_key UNIQUE INDEX (`migrate_billing.rs:865-868`).
///
/// MAOR-T1 IMPORTANT-1 nuance — the index is **partial**:
/// `WHERE idempotency_key IS NOT NULL`. NULL keys bypass the UNIQUE
/// constraint entirely and will NEVER produce this error. Callers that
/// pass `Some(non_empty_str)` (the only correct call shape for
/// `payment_gateway_webhook`) are unaffected; callers that pass `None`
/// MUST NOT rely on this helper as a deduplication signal.
///
/// We accept either the canonical sqlx message or a partial column-name
/// match for resilience against minor sqlx-error-format changes. False-
/// positive risk is negligible: no other table has `idempotency_key` in
/// the wallet_transactions namespace; the qualified column name keeps
/// this check tight.
fn is_idempotency_conflict(err_str: &str) -> bool {
    err_str.contains("UNIQUE constraint failed: wallet_transactions.idempotency_key")
        || err_str.contains("wallet_transactions.idempotency_key")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §S-281 D-PHASE-γ-1 — confirms the canonical sqlx UNIQUE-constraint
    /// error message format triggers conflict-detection.
    #[test]
    fn is_idempotency_conflict_matches_sqlx_canonical_message() {
        let canonical = "DB error recording transaction: error returned from database: \
                         UNIQUE constraint failed: wallet_transactions.idempotency_key";
        assert!(is_idempotency_conflict(canonical), "canonical sqlx UNIQUE-conflict message MUST trigger");
    }

    /// §S-281 D-PHASE-γ-1 — confirms partial column-name match works as
    /// resilience fallback (sqlx error-format minor changes don't break detection).
    #[test]
    fn is_idempotency_conflict_matches_partial_column_name() {
        let partial = "some future sqlx wrapper: wallet_transactions.idempotency_key duplicate";
        assert!(is_idempotency_conflict(partial), "partial column-name match MUST trigger");
    }

    /// §S-281 D-PHASE-γ-1 — confirms unrelated DB errors do NOT trigger
    /// conflict-detection (would cause false-positive duplicate-response
    /// instead of error propagation).
    #[test]
    fn is_idempotency_conflict_rejects_unrelated_errors() {
        let cases = [
            "DB error updating wallet: connection refused",
            "DB error reading balance: timeout",
            "UNIQUE constraint failed: wallets.driver_id", // different table
            "txn_type 'unknown' is not in the allow-list", // D-CLUSTER-9 error
            "",
        ];
        for case in cases {
            assert!(!is_idempotency_conflict(case), "unrelated error MUST NOT trigger: {}", case);
        }
    }
}
