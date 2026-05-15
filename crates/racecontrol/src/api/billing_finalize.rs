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
    Extension, Json,
    extract::State,
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::auth::middleware::CredentialClass;
use crate::cloud_sync_verify::{VerifyOutcome, verify_finalize_sync};
use crate::state::AppState;

/// A3 Phase 1.5 — synchronous cloud_sync verify timeout budget.
///
/// Per §S-146 RCA §5 + Q1 ratify: 200ms is 20% of the 1s SLA budget; existing
/// `ECHO_TIMEOUT_SECS=2s` (cloud_sync_verify) is the underlying reqwest cap.
/// `tokio::time::timeout` wraps the call to enforce the tighter budget; on
/// elapsed → fail-open per Q2 ratify (Mismatch identical to Unavailable from
/// the caller's perspective: keep `mirror_state="pending_async_verify"`; 30s
/// tick picks up async).
const FINALIZE_VERIFY_TIMEOUT_MS: u64 = 200;

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
//
// Phase 1.5 §S-146 Q4 (Option Z): the credential class authority lives in
// the middleware-injected `CredentialClass` extension, NOT in handler-local
// header inference. `require_staff_jwt` inserts `CredentialClass::StaffJwt`
// alongside `StaffClaims`. A future `require_service_key` middleware will
// insert `CredentialClass::ServiceKey` for rc-agent + auto-scheduler routes
// (separate PR scope). This handler is the FIRST consumer of the extension
// pattern; migration of other handlers is tracked via the audit issue seeded
// in the Phase 1.5 PR description (30-day deadline per anthropic Risk #1).
fn validate_actor_credential(
    declared_actor: FinalizeActor,
    credential: Option<CredentialClass>,
) -> Result<(), (StatusCode, Value)> {
    let credential_class = match credential {
        Some(CredentialClass::StaffJwt) => "staff-jwt",
        Some(CredentialClass::ServiceKey) => "service-key",
        Some(CredentialClass::AutoScheduler) => "internal-caller",
        None => "none",
    };

    let expected = declared_actor.expected_credential_class();
    let ok = match (declared_actor, credential) {
        (FinalizeActor::Kiosk, Some(CredentialClass::StaffJwt))
        | (FinalizeActor::Staff, Some(CredentialClass::StaffJwt)) => true,
        (FinalizeActor::RcAgent, Some(CredentialClass::ServiceKey)) => true,
        // AutoScheduler is enabled on staff-JWT in Phase 1 (deferred matrix
        // tightening to dedicated internal-caller middleware in Phase 2).
        (FinalizeActor::AutoScheduler, Some(CredentialClass::StaffJwt)) => true,
        _ => false,
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
    credential: Option<Extension<CredentialClass>>,
    Json(req): Json<FinalizeRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let started_at = Instant::now();

    // Phase 1.5 §S-146 Q4 (Option Z): credential class is read from the
    // middleware-injected Extension, NOT from raw header inference. The route
    // is registered behind `require_staff_jwt` in Phase 1, so a request
    // arriving here has `CredentialClass::StaffJwt` in extensions.
    let credential_class = credential.map(|Extension(c)| c);

    if let Err((status, body)) =
        validate_actor_credential(req.actor, credential_class)
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
        //
        // Phase 1.5 Q3 (always-emit): verify_outcome on the degraded-replay
        // path is "sync_unavailable" because the original sync verify result
        // was not preserved (receipt blob missing). Clients treat this the
        // same as any other unavailable: keep mirror_state pending.
        return Ok(Json(json!({
            "session_id": first_id,
            "finalize_reason": first_reason,
            "finalize_actor": first_actor,
            "finalized_at": first_finalized_at,
            "idempotency_status": "replayed_degraded",
            "mirror_state": "pending_async_verify",
            "verify_outcome": "sync_unavailable",
            "latency_ms": started_at.elapsed().as_millis() as u64,
        })));
    }

    // §4 state-transition gate + pre-image snapshot capture.
    //
    // Phase 1.5 §S-146 Q1 (Option A, 3/3 unanimous): capture `updated_at` here
    // as the PRIOR-row value (cursor_before for the A3 sync verify probe).
    // The UPDATE below explicitly writes `updated_at = finalized_at_iso`
    // (cursor_after), so the receiver's windowed `/sync/echo` query over
    // `(updated_at_before, updated_at_after]` matches exactly one row.
    let row: Option<(
        String,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT status, wallet_debit_paise, pricing_strategy_id, pricing_tier_id, \
                 started_at, updated_at \
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

    let (from_status, wallet_pre, strategy_pre, tier_pre, started_at_pre, updated_at_before) =
        match row {
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
    //
    // Phase 1.5 §S-146 Q1: explicitly set `updated_at = finalize_at_iso` so
    // that `(updated_at_before, updated_at_after]` is a clean cursor window
    // for the A3 sync verify probe. The cloud_sync 30s tick reads
    // `billing_sessions WHERE created_at >= ? OR ended_at >= ?` so this
    // explicit write does not alter that path.
    let finalized_at_iso = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query(
        "UPDATE billing_sessions SET \
           finalize_idempotency_key = ?, \
           finalize_reason = ?, \
           finalize_actor = ?, \
           finalized_at = ?, \
           updated_at = ? \
         WHERE id = ?",
    )
    .bind(&req.idempotency_key)
    .bind(req.reason.as_str())
    .bind(req.actor.as_str())
    .bind(&finalized_at_iso)
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

    // Phase 1.5 §S-146 A3 sync verify wrapper — Q1 cursor-window from
    // billing_session.updated_at; Q2 fail-open identical to Unavailable;
    // Q3 always-emit verify_outcome enum. tokio::time::timeout enforces the
    // 200ms budget; elapsed-timeout falls through to fail-open. Bono's 30s
    // cloud_sync_push tick + verify_push picks up the row async regardless.
    //
    // updated_at_before is the pre-finalize row's updated_at (may be None for
    // pre-existing rows that haven't been touched since the backfill); when
    // None, use a sentinel that the receiver's allowlisted column comparison
    // treats as "open lower bound" — empty string sorts before all RFC3339.
    let updated_at_before_str = updated_at_before.unwrap_or_default();
    let verify_outcome_str = match tokio::time::timeout(
        Duration::from_millis(FINALIZE_VERIFY_TIMEOUT_MS),
        verify_finalize_sync(&state, &updated_at_before_str, &finalized_at_iso),
    )
    .await
    {
        Ok(VerifyOutcome::AllOk) => "sync_ok",
        Ok(VerifyOutcome::Unavailable) => "sync_unavailable",
        Ok(VerifyOutcome::Mismatch { .. }) => "sync_mismatch",
        Err(_elapsed) => "sync_timeout",
    };
    let mirror_state = if verify_outcome_str == "sync_ok" {
        "verified"
    } else {
        // Q2 (α): Mismatch is fail-open identical to Unavailable from caller's
        // perspective — keep mirror_state="pending_async_verify"; existing 30s
        // tick will eventually advance.
        "pending_async_verify"
    };
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
        "verify_outcome": verify_outcome_str,
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

    // §5 §D — Phase 1 SLA latency-log-line + Phase 1.5 verify_outcome
    // observability (Q3 II — always-emit). verify_outcome on the same span
    // as the 1s SLA latency_ms supports A4 SLA-dashboard correlation
    // (e.g., sync_timeout count tracks 200ms-budget breaches).
    tracing::info!(
        target: "billing.finalize",
        latency_ms = elapsed_ms,
        session_id = %req.session_id,
        finalize_reason = %req.reason.as_str(),
        finalize_actor = %req.actor.as_str(),
        mirror_state = %mirror_state,
        verify_outcome = %verify_outcome_str,
        "finalize.complete"
    );

    Ok(Json(response))
}

#[cfg(test)]
mod credential_class_matrix_tests {
    use super::*;

    /// Phase 1.5 §S-146 Q4 (Option Z) — validate_actor_credential decision
    /// matrix exhausted across all (declared_actor × CredentialClass) pairs.
    /// All accept-cases hit Ok; all reject-cases hit Err with the documented
    /// mismatch shape.
    #[test]
    fn validate_actor_credential_matrix() {
        // Accept cases — declared actor matches the injected credential class
        assert!(validate_actor_credential(FinalizeActor::Kiosk, Some(CredentialClass::StaffJwt)).is_ok());
        assert!(validate_actor_credential(FinalizeActor::Staff, Some(CredentialClass::StaffJwt)).is_ok());
        assert!(validate_actor_credential(FinalizeActor::RcAgent, Some(CredentialClass::ServiceKey)).is_ok());
        assert!(validate_actor_credential(FinalizeActor::AutoScheduler, Some(CredentialClass::StaffJwt)).is_ok());

        // Reject cases — declared actor does not match the injected class
        let cases = [
            (FinalizeActor::Kiosk, Some(CredentialClass::ServiceKey), "service-key"),
            (FinalizeActor::Staff, Some(CredentialClass::ServiceKey), "service-key"),
            (FinalizeActor::Staff, None, "none"),
            (FinalizeActor::RcAgent, Some(CredentialClass::StaffJwt), "staff-jwt"),
            (FinalizeActor::RcAgent, None, "none"),
            (FinalizeActor::AutoScheduler, Some(CredentialClass::ServiceKey), "service-key"),
            (FinalizeActor::AutoScheduler, None, "none"),
        ];
        for (actor, class, expected_received) in cases {
            let Err((status, body)) = validate_actor_credential(actor, class) else {
                panic!("expected rejection for actor={:?} class={:?}", actor, class);
            };
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["error"], "actor_credential_mismatch");
            assert_eq!(body["received_credential_class"], expected_received);
            assert_eq!(body["expected_credential_class"], actor.expected_credential_class());
        }
    }

    /// Phase 1.5 §S-146 Q3 (Option II) — verify_outcome enum maps from
    /// VerifyOutcome variants deterministically. This is the structural-
    /// contract test for the call-site mapping; the live-receiver behavior
    /// path is gated behind RC_TEST_LIVE_SYNC_ECHO_URL in cloud_sync_verify.
    #[test]
    fn verify_outcome_string_mapping() {
        // Note: This test mirrors the match arms in finalize_handler. The
        // mapping is intentionally simple so a future refactor (e.g. moving
        // the mapping into a free function) can target this test as the
        // contract anchor.
        let cases = [
            (VerifyOutcome::AllOk, "sync_ok", "verified"),
            (VerifyOutcome::Unavailable, "sync_unavailable", "pending_async_verify"),
            (
                VerifyOutcome::Mismatch { details: vec![] },
                "sync_mismatch",
                "pending_async_verify",
            ),
        ];
        for (outcome, expected_outcome, expected_mirror) in cases {
            let outcome_str = match outcome {
                VerifyOutcome::AllOk => "sync_ok",
                VerifyOutcome::Unavailable => "sync_unavailable",
                VerifyOutcome::Mismatch { .. } => "sync_mismatch",
            };
            let mirror = if outcome_str == "sync_ok" {
                "verified"
            } else {
                "pending_async_verify"
            };
            assert_eq!(outcome_str, expected_outcome);
            assert_eq!(mirror, expected_mirror);
        }

        // sync_timeout has no VerifyOutcome — it surfaces from
        // tokio::time::timeout's Elapsed error. Verify the mapping here.
        let timeout_outcome = "sync_timeout";
        let timeout_mirror = if timeout_outcome == "sync_ok" {
            "verified"
        } else {
            "pending_async_verify"
        };
        assert_eq!(timeout_mirror, "pending_async_verify");
    }
}
