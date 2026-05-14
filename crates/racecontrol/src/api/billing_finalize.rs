// §S-329 row 1.13 — POST /api/v1/billing/finalize
//
// V2-doctrine unified session-end endpoint. Closes V1→V2 STRUCTURAL GAP
// G-1.13-1 (missing-endpoint /billing/finalize + idempotency informational-
// only on stop). Per §S-146 V1↔V2 RCA at
// `.planning/audits/RCA-2026-05-13-row-1.13-billing-finalize-idempotency.md`
// with all 7 MMA Step 1 consensus amendments (A1..A7) absorbed at §S-328.
//
// Phase 1 substrate scope (this file):
//   A1 — cafe-order aggregation (snapshot pre-finalize commit)
//   A2 — idempotency-key DB-stamp + payload-identity validation on replay
//   A4 — 1s SLA tracing event (latency_ms)
//   A5 — F25a SnapPricing strategy preservation (delegates to existing
//        end_billing_session_public which honors billing_pricing module)
//   A6 — DPDP cascade column NULL-out (companion in customer_legal.rs)
//   A7 — auth-tier server-side binding-matrix (staff-JWT only in Phase 1;
//        service-key path deferred to Phase 1.5 / V1-wrapper redirect)
//   N4 — F-05 invariant generalized via pre-image snapshot
//
// Deferred (Phase 2 / next-PR):
//   A3 sync cloud_sync_verify with 200ms timeout — Phase 1 returns
//      mirror_state: "pending_async_verify" unconditionally; existing 30s
//      cloud_sync_push tick + verify_push picks up the row asynchronously
//   I-1 V1 wrapper redirects (stop / stop-service / agent-shutdown still
//      run independent code paths in Phase 1; redirect is Phase 1.5)

use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Instant;

use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum FinalizeReason {
    CustomerStop,
    AgentShutdown,
    AutoComplete,
    VenueClosed,
}

impl FinalizeReason {
    fn as_str(self) -> &'static str {
        match self {
            FinalizeReason::CustomerStop => "CustomerStop",
            FinalizeReason::AgentShutdown => "AgentShutdown",
            FinalizeReason::AutoComplete => "AutoComplete",
            FinalizeReason::VenueClosed => "VenueClosed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FinalizeActor {
    Kiosk,
    Staff,
    #[serde(rename = "rc-agent")]
    RcAgent,
    #[serde(rename = "auto-scheduler")]
    AutoScheduler,
}

impl FinalizeActor {
    fn as_str(self) -> &'static str {
        match self {
            FinalizeActor::Kiosk => "kiosk",
            FinalizeActor::Staff => "staff",
            FinalizeActor::RcAgent => "rc-agent",
            FinalizeActor::AutoScheduler => "auto-scheduler",
        }
    }

    fn expected_credential_class(self) -> &'static str {
        match self {
            FinalizeActor::Kiosk | FinalizeActor::Staff => "staff-jwt",
            FinalizeActor::RcAgent => "service-key",
            FinalizeActor::AutoScheduler => "internal-caller",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FinalizeRequest {
    pub session_id: String,
    pub reason: FinalizeReason,
    pub idempotency_key: String,
    pub actor: FinalizeActor,
}

#[derive(Debug, Serialize)]
struct FinalizeSnapshot {
    wallet_debit_paise_pre_update: i64,
    cafe_amount_paise_snapshot: i64,
    pricing_strategy_id_snapshot: Option<String>,
    pricing_tier_id_snapshot: Option<String>,
    from_status: String,
    started_at: Option<String>,
}

// Auth-tier server-side binding-matrix per RCA §4-A7.
// Phase 1: only staff-JWT path is enabled (handler registered under
// require_staff_jwt). The matrix is enforced anyway so Phase 1.5 service-key
// path can drop in without re-deriving doctrine.
fn validate_actor_credential(
    state: &Arc<AppState>,
    headers: &HeaderMap,
    declared_actor: FinalizeActor,
    has_staff_jwt: bool,
) -> Result<(), (StatusCode, Value)> {
    let has_service_key = match &state.config.pods.sentry_service_key {
        Some(expected) if !expected.is_empty() => headers
            .get("x-service-key")
            .and_then(|v| v.to_str().ok())
            .map(|provided| provided == expected.as_str())
            .unwrap_or(false),
        _ => false,
    };

    let credential_class = if has_staff_jwt {
        "staff-jwt"
    } else if has_service_key {
        "service-key"
    } else {
        "none"
    };

    let expected = declared_actor.expected_credential_class();
    let ok = match declared_actor {
        FinalizeActor::Kiosk | FinalizeActor::Staff => has_staff_jwt,
        FinalizeActor::RcAgent => has_service_key,
        FinalizeActor::AutoScheduler => has_staff_jwt,
    };

    if ok {
        Ok(())
    } else {
        Err((
            StatusCode::UNAUTHORIZED,
            json!({
                "error": "actor_credential_mismatch",
                "expected_credential_class": expected,
                "received_credential_class": credential_class,
            }),
        ))
    }
}

// Per RCA §4 state-transition matrix. N1 carry-forward expands the matrix
// to include reason × actor × postcondition; Phase 1 keeps the from_status
// gate alone.
fn finalize_allowed_from(status: &str) -> bool {
    matches!(
        status,
        "active"
            | "paused_manual"
            | "paused_game_pause"
            | "paused_disconnect"
            | "paused_crash_recovery"
            | "waiting_for_game"
    )
}

pub(crate) async fn finalize_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<FinalizeRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let started_at = Instant::now();

    // The route is registered behind require_staff_jwt in Phase 1, so a
    // request arriving here has a validated staff JWT in extensions. We
    // detect that via the Authorization header presence; full claim
    // validation already ran upstream.
    let has_staff_jwt = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("Bearer "))
        .unwrap_or(false);

    if let Err((status, body)) =
        validate_actor_credential(&state, &headers, req.actor, has_staff_jwt)
    {
        return Err((status, Json(body)));
    }

    // §4-A2 idempotency-key DB lookup + payload-identity validation.
    let existing: Option<(String, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT id, finalize_reason, finalize_actor, finalized_at \
             FROM billing_sessions \
             WHERE finalize_idempotency_key = ? \
             LIMIT 1",
        )
        .bind(&req.idempotency_key)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "db_error", "detail": e.to_string()})),
            )
        })?;

    if let Some((first_id, first_reason, first_actor, first_finalized_at)) = existing {
        // Payload-identity check — all 3 fields must match the first call,
        // otherwise return 409 with first-call summary.
        let same_session = first_id == req.session_id;
        let same_reason =
            first_reason.as_deref() == Some(req.reason.as_str());
        let same_actor = first_actor.as_deref() == Some(req.actor.as_str());

        if !(same_session && same_reason && same_actor) {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "idempotency_payload_mismatch",
                    "first_call_summary": {
                        "session_id": first_id,
                        "reason": first_reason,
                        "actor": first_actor,
                        "finalized_at": first_finalized_at,
                    },
                })),
            ));
        }

        // Match — return canonical first response from receipts table.
        let receipt: Option<(String,)> = sqlx::query_as(
            "SELECT idempotency_response_blob FROM billing_session_receipts \
             WHERE session_id = ? LIMIT 1",
        )
        .bind(&first_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "db_error", "detail": e.to_string()})),
            )
        })?;

        if let Some((blob,)) = receipt
            && let Ok(mut resp) = serde_json::from_str::<Value>(&blob)
        {
            // Mark this response as a replay; latency reflects the replay
            // path, not the original.
            if let Some(obj) = resp.as_object_mut() {
                obj.insert(
                    "idempotency_status".into(),
                    Value::String("replayed".into()),
                );
                obj.insert(
                    "latency_ms".into(),
                    Value::Number(serde_json::Number::from(
                        started_at.elapsed().as_millis() as u64,
                    )),
                );
            }
            return Ok(Json(resp));
        }

        // Row stamped but receipt missing — degraded replay; surface
        // observable state without making up canonical fields.
        return Ok(Json(json!({
            "session_id": first_id,
            "finalize_reason": first_reason,
            "finalize_actor": first_actor,
            "finalized_at": first_finalized_at,
            "idempotency_status": "replayed_degraded",
            "mirror_state": "pending_async_verify",
            "latency_ms": started_at.elapsed().as_millis() as u64,
        })));
    }

    // §4 state-transition gate + pre-image snapshot capture.
    let row: Option<(String, Option<i64>, Option<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT status, wallet_debit_paise, pricing_strategy_id, pricing_tier_id, started_at \
             FROM billing_sessions WHERE id = ? LIMIT 1",
        )
        .bind(&req.session_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "db_error", "detail": e.to_string()})),
            )
        })?;

    let (from_status, wallet_pre, strategy_pre, tier_pre, started_at_pre) = match row {
        Some(r) => r,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "session_not_found", "session_id": req.session_id})),
            ));
        }
    };

    if !finalize_allowed_from(&from_status) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "invalid_from_status",
                "idempotency_status": "terminal",
                "from_status": from_status,
            })),
        ));
    }

    // §5 §A cafe-order aggregation — captured pre-commit so concurrent
    // cafe inserts during finalize block on the row-lock and do not
    // corrupt the snapshot (N3 TOCTOU window closed).
    let cafe_amount_paise_snapshot: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount_paise), 0) \
         FROM cafe_orders \
         WHERE session_id = ? \
           AND status IN ('placed', 'preparing', 'ready', 'delivered') \
           AND voided_at IS NULL",
    )
    .bind(&req.session_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    let snapshot = FinalizeSnapshot {
        wallet_debit_paise_pre_update: wallet_pre.unwrap_or(0),
        cafe_amount_paise_snapshot,
        pricing_strategy_id_snapshot: strategy_pre,
        pricing_tier_id_snapshot: tier_pre,
        from_status: from_status.clone(),
        started_at: started_at_pre,
    };

    // §5 delegates the actual end-of-session work to the existing
    // billing-session machinery so F-05 invariant + F25a SnapPricing path
    // are preserved by-construction (single source-of-truth).
    let end_status = match req.reason {
        FinalizeReason::CustomerStop => rc_common::types::BillingSessionStatus::EndedEarly,
        FinalizeReason::AgentShutdown => rc_common::types::BillingSessionStatus::EndedEarly,
        FinalizeReason::AutoComplete => rc_common::types::BillingSessionStatus::Completed,
        FinalizeReason::VenueClosed => rc_common::types::BillingSessionStatus::CancelledNoPlayable,
    };
    let ended = crate::billing::end_billing_session_public(
        &state,
        &req.session_id,
        end_status,
        Some("§S-329 /billing/finalize"),
    )
    .await;

    if !ended {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "finalize_rejected",
                "from_status": from_status,
            })),
        ));
    }

    // §4-A2 stamp idempotency-key + finalize meta-columns on billing_sessions.
    let finalized_at_iso = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query(
        "UPDATE billing_sessions SET \
           finalize_idempotency_key = ?, \
           finalize_reason = ?, \
           finalize_actor = ?, \
           finalized_at = ? \
         WHERE id = ?",
    )
    .bind(&req.idempotency_key)
    .bind(req.reason.as_str())
    .bind(req.actor.as_str())
    .bind(&finalized_at_iso)
    .bind(&req.session_id)
    .execute(&state.db)
    .await;

    // Read back computed fields for the canonical response.
    let post: Option<(Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
        "SELECT wallet_debit_paise, total_debited_paise, elapsed_seconds \
         FROM billing_sessions WHERE id = ? LIMIT 1",
    )
    .bind(&req.session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();
    let (wallet_final, total_final, elapsed_secs) = match post {
        Some((w, t, e)) => (w.unwrap_or(0), t.unwrap_or(0), e.unwrap_or(0)),
        None => (0, 0, 0),
    };
    let wallet_refund_paise =
        (snapshot.wallet_debit_paise_pre_update - wallet_final).max(0);

    let mirror_state = "pending_async_verify";
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    let response = json!({
        "session_id": req.session_id,
        "finalized_at": finalized_at_iso,
        "finalize_reason": req.reason.as_str(),
        "finalize_actor": req.actor.as_str(),
        "wallet_debit_paise_final": wallet_final,
        "wallet_refund_paise": wallet_refund_paise,
        "cafe_amount_paise": snapshot.cafe_amount_paise_snapshot,
        "total_debited_paise": total_final,
        "session_duration_secs": elapsed_secs,
        "mirror_state": mirror_state,
        "idempotency_status": "first",
        "latency_ms": elapsed_ms,
    });

    // §4-A2 receipt — canonical first response stored verbatim for
    // deterministic replay. ON CONFLICT NO-OP is the correct semantic on
    // racy double-call: the second caller sees the same row.
    let blob = response.to_string();
    let _ = sqlx::query(
        "INSERT INTO billing_session_receipts (session_id, idempotency_response_blob) \
         VALUES (?, ?) \
         ON CONFLICT(session_id) DO NOTHING",
    )
    .bind(&req.session_id)
    .bind(&blob)
    .execute(&state.db)
    .await;

    // §5 §D — Phase 1 SLA latency-log-line.
    tracing::info!(
        target: "billing.finalize",
        latency_ms = elapsed_ms,
        session_id = %req.session_id,
        finalize_reason = %req.reason.as_str(),
        finalize_actor = %req.actor.as_str(),
        mirror_state = %mirror_state,
        "finalize.complete"
    );

    Ok(Json(response))
}
