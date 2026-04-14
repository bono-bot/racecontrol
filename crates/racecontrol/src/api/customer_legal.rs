#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use super::routes::anonymize_driver_pii;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

// Re-export dispute handlers (BILL-08) — extracted to customer_disputes.rs
pub(crate) use super::customer_disputes::{
    create_dispute_handler,
    list_disputes_handler,
    dispute_details_handler,
    resolve_dispute_handler,
};

// Re-export data retention job (LEGAL-08) — extracted to customer_data_retention.rs
pub use super::customer_data_retention::spawn_data_retention_job;
pub(crate) use super::customer_data_retention::run_pii_anonymization_cycle;

/// GET /api/v1/customer/data-export
/// Returns a JSON dump of all customer data with decrypted PII fields.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_export(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<(axum::http::StatusCode, Json<Value>), (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    let driver = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, name_enc, email_enc, phone_enc, total_laps, total_time_ms FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    let d = match driver {
        Ok(Some(row)) => row,
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_export DB error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Decrypt PII fields; fallback to plaintext columns if decryption fails or enc is NULL
    let name = d.4.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or_else(|| Some(d.1.clone()));
    let email = d.5.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.2.clone());
    let phone = d.6.as_deref()
        .and_then(|enc| state.field_cipher.decrypt_field(enc).ok())
        .or(d.3.clone());

    // Fetch nickname
    let nickname: Option<String> = sqlx::query_as::<_, (Option<String>,)>(
        "SELECT nickname FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .and_then(|r| r.0);

    // Fetch wallet balance
    let wallet_balance: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COALESCE(balance, 0) FROM wallets WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .map(|r| r.0)
    .unwrap_or(0);

    let exported_at = chrono::Utc::now().to_rfc3339();
    tracing::info!("Data export requested by driver {}", driver_id);

    Ok((
        axum::http::StatusCode::OK,
        Json(json!({
            "driver_id": d.0,
            "name": name,
            "email": email,
            "phone": phone,
            "nickname": nickname,
            "total_laps": d.7,
            "total_time_ms": d.8,
            "wallet_balance": wallet_balance,
            "exported_at": exported_at,
        })),
    ))
}

/// DELETE /api/v1/customer/data-delete
/// Cascades deletion to all child tables and the driver record in a single transaction.
/// Requires valid customer JWT.
pub(crate) async fn customer_data_delete(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<axum::http::StatusCode, (axum::http::StatusCode, Json<Value>)> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => {
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({ "error": e })),
            ))
        }
    };

    // Verify driver exists
    let exists = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match exists {
        Ok(None) => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                Json(json!({ "error": "Driver not found" })),
            ))
        }
        Err(e) => {
            tracing::error!("data_delete lookup error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
        Ok(Some(_)) => {}
    }

    // Begin transaction -- cascade delete all child tables
    let mut tx = match state.db.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            tracing::error!("data_delete transaction start error for driver {}: {}", driver_id, e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Database error" })),
            ));
        }
    };

    // Delete from child tables (children first, then parent)
    // wallet_transactions before wallets (wallet_transactions references wallets)
    let _ = sqlx::query("DELETE FROM wallet_transactions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM wallets WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM billing_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM laps WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM customer_sessions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friend_requests WHERE sender_id = ? OR receiver_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM friendships WHERE driver_a_id = ? OR driver_b_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM group_session_members WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM tournament_registrations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM pod_reservations WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM auth_tokens WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM personal_bests WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM event_entries WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_feedback WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM coupon_redemptions WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM memberships WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM referrals WHERE referrer_id = ? OR referee_id = ?").bind(&driver_id).bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM session_highlights WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM review_nudges WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM multiplayer_results WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;
    let _ = sqlx::query("DELETE FROM driver_ratings WHERE driver_id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Delete the driver record itself
    let _ = sqlx::query("DELETE FROM drivers WHERE id = ?").bind(&driver_id).execute(&mut *tx).await;

    // Commit transaction
    if let Err(e) = tx.commit().await {
        tracing::error!("data_delete commit error for driver {}: {}", driver_id, e);
        return Err((
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Database error" })),
        ));
    }

    tracing::info!("Customer {} deleted their data (DPDP compliance)", driver_id);
    Ok(axum::http::StatusCode::NO_CONTENT)
}

// ─── LEGAL-09: Consent revocation (customer-initiated via PWA) ───────────────
/// POST /api/v1/customer/revoke-consent
///
/// Allows a driver (or guardian acting on behalf of a minor) to invoke the DPDP Act
/// right of erasure. Anonymizes PII immediately. Financial records (journal entries,
/// invoices, billing_sessions) are NOT deleted — they must be retained for 8 years
/// per the Income Tax Act.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("customer_request");
    anonymize_driver_pii(&state, &driver_id, reason, None).await
}

// ─── LEGAL-09: Consent revocation (staff-initiated for guardian requests) ────
/// POST /api/v1/drivers/{id}/revoke-consent
///
/// Staff endpoint for guardian-initiated revocation — guardian calls the venue,
/// staff (cashier+) processes the data deletion request.
///
/// Body: `{ "reason": "optional reason string" }`
pub(crate) async fn staff_revoke_consent_handler(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let reason = body
        .get("reason")
        .and_then(|v| v.as_str())
        .unwrap_or("guardian_request");
    anonymize_driver_pii(&state, &driver_id, reason, Some("staff")).await
}
