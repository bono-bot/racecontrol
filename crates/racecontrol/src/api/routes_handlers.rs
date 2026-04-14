//! Handler functions that live alongside routes.rs.
//! Extracted from routes.rs for ARCH-03 (<500 line target).

use axum::{Json, extract::{Path, Query, State}};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::accounting;
use crate::api::customer_auth::extract_driver_id;
use crate::state::AppState;

// ─── Coaching: Lap Comparison ────────────────────────────────────────────────

#[derive(Deserialize)]
pub(super) struct CompareLapsQuery {
    track: String,
    car: String,
    compare_to: Option<String>, // "record" or driver_id
}

pub(super) async fn customer_compare_laps(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<CompareLapsQuery>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Get driver's laps on this track+car
    let my_laps = sqlx::query_as::<_, (i64, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid
         FROM laps WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY lap_time_ms ASC LIMIT 20",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if my_laps.is_empty() {
        return Json(json!({ "error": "No laps found on this track/car" }));
    }

    let my_best = &my_laps[0];

    // Get comparison target
    let compare_to = params.compare_to.as_deref().unwrap_or("record");

    let reference_lap: Option<(String, i64, Option<i64>, Option<i64>, Option<i64>)> = if compare_to == "record" {
        // Compare to track record
        sqlx::query_as(
            "SELECT d.name, tr.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM track_records tr
             JOIN drivers d ON tr.driver_id = d.id
             LEFT JOIN laps l ON tr.lap_id = l.id
             WHERE tr.track = ? AND tr.car = ?",
        )
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    } else {
        // Compare to specific driver's best
        sqlx::query_as(
            "SELECT d.name, pb.best_lap_ms, l.sector1_ms, l.sector2_ms, l.sector3_ms
             FROM personal_bests pb
             JOIN drivers d ON pb.driver_id = d.id
             LEFT JOIN laps l ON pb.lap_id = l.id
             WHERE pb.driver_id = ? AND pb.track = ? AND pb.car = ?",
        )
        .bind(compare_to)
        .bind(&params.track)
        .bind(&params.car)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
    };

    // Compute sector deltas
    let sector_analysis = if let Some(ref_lap) = &reference_lap {
        let s1_delta = match (my_best.2, ref_lap.2) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s2_delta = match (my_best.3, ref_lap.3) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };
        let s3_delta = match (my_best.4, ref_lap.4) {
            (Some(mine), Some(theirs)) => Some(mine - theirs),
            _ => None,
        };

        let weakest = [
            s1_delta.map(|d| ("S1", d)),
            s2_delta.map(|d| ("S2", d)),
            s3_delta.map(|d| ("S3", d)),
        ]
        .iter()
        .filter_map(|x| *x)
        .max_by_key(|(_, d)| *d);

        Some(json!({
            "s1_delta_ms": s1_delta,
            "s2_delta_ms": s2_delta,
            "s3_delta_ms": s3_delta,
            "weakest_sector": weakest.map(|(s, d)| format!("{} (+{}ms)", s, d)),
            "total_delta_ms": my_best.1 - ref_lap.1,
        }))
    } else {
        None
    };

    // Consistency trend (last 10 laps chronologically)
    let recent_laps = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM laps
         WHERE driver_id = ? AND track = ? AND car = ? AND valid = 1
         ORDER BY created_at DESC LIMIT 10",
    )
    .bind(&driver_id)
    .bind(&params.track)
    .bind(&params.car)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let trend: Vec<i64> = recent_laps.iter().rev().map(|l| l.0).collect();
    let improving = if trend.len() >= 3 {
        let first_half: f64 = trend[..trend.len()/2].iter().map(|&t| t as f64).sum::<f64>() / (trend.len()/2) as f64;
        let second_half: f64 = trend[trend.len()/2..].iter().map(|&t| t as f64).sum::<f64>() / (trend.len() - trend.len()/2) as f64;
        Some(second_half < first_half)
    } else {
        None
    };

    Json(json!({
        "track": params.track,
        "car": params.car,
        "my_best": {
            "time_ms": my_best.1,
            "s1_ms": my_best.2,
            "s2_ms": my_best.3,
            "s3_ms": my_best.4,
        },
        "reference": reference_lap.as_ref().map(|r| json!({
            "driver": r.0,
            "time_ms": r.1,
            "s1_ms": r.2,
            "s2_ms": r.3,
            "s3_ms": r.4,
        })),
        "sector_analysis": sector_analysis,
        "recent_trend": trend,
        "improving": improving,
        "tip": sector_analysis.as_ref().and_then(|sa| {
            sa.get("weakest_sector").and_then(|w| w.as_str()).map(|w| {
                format!("Focus on {} — that is where you lose the most time vs the reference lap.", w)
            })
        }),
    }))
}

// ─── Customer Multiplayer Results ─────────────────────────────────────────────

/// GET /customer/multiplayer-results/{group_session_id} — Get race results for a group session
pub(super) async fn customer_multiplayer_results(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Path(group_session_id): Path<String>,
) -> Json<Value> {
    let _driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, i64, Option<i64>, Option<i64>, i64, i64)>(
        "SELECT mr.id, COALESCE(d.name, 'Unknown'), mr.position, mr.best_lap_ms, mr.total_time_ms,
                mr.laps_completed, mr.dnf
         FROM multiplayer_results mr
         LEFT JOIN drivers d ON d.id = mr.driver_id
         WHERE mr.group_session_id = ?
         ORDER BY mr.position ASC",
    )
    .bind(&group_session_id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(results) => {
            let results_json: Vec<Value> = results
                .iter()
                .map(|(id, name, pos, best_lap, total_time, laps, dnf)| {
                    json!({
                        "id": id,
                        "driver_name": name,
                        "position": pos,
                        "best_lap_ms": best_lap,
                        "total_time_ms": total_time,
                        "laps_completed": laps,
                        "dnf": dnf == &1,
                    })
                })
                .collect();
            Json(json!({ "results": results_json }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

/// POST /api/v1/customer/game-request
///
/// Customer requests a game launch from the PWA. Validates that the pod
/// exists and the game is installed, then broadcasts GameLaunchRequested
/// to the staff dashboard. Staff confirms via POST /api/v1/games/pod/{id}/launch.
///
/// Note: customer auth uses extract_driver_id() (customer JWT). Customer auth
/// middleware is in-handler (Phase 82+ may promote to tower middleware).

// ─── DPDP Act: Customer Data Rights (Plan 79-03) ────────────────────────────

/// Shared PII anonymization logic for both customer- and staff-initiated consent revocation.
///
/// Anonymizes all PII fields on the drivers row and sets consent_revoked = 1.
/// The driver row is retained so billing_sessions.driver_id foreign keys remain valid.
/// Financial records (journal_entries, invoices, billing_sessions, wallet_transactions)
/// are NOT touched — retained for 8 years per the Income Tax Act.
pub(crate) async fn anonymize_driver_pii(
    state: &Arc<AppState>,
    driver_id: &str,
    reason: &str,
    actor: Option<&str>,
) -> Json<Value> {
    // Check driver exists and is not already revoked
    let row = sqlx::query_as::<_, (String, bool)>(
        "SELECT id, COALESCE(consent_revoked, 0) FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(None) => return Json(json!({ "error": "Driver not found" })),
        Ok(Some((_, true))) => {
            return Json(json!({
                "ok": true,
                "message": "Consent was already revoked. Personal data has been anonymized previously."
            }));
        }
        Err(e) => {
            tracing::error!(driver_id = %driver_id, "consent_revocation DB lookup error: {}", e);
            return Json(json!({ "error": "Database error" }));
        }
        Ok(Some(_)) => {} // proceed
    }

    // Anonymize PII — same UPDATE used by the daily background job.
    // The driver row is KEPT so billing_session.driver_id FKs remain valid.
    let result = sqlx::query(
        "UPDATE drivers SET
            name = 'ANONYMIZED-' || substr(id, 1, 8),
            email = NULL,
            phone = NULL,
            phone_hash = NULL,
            guardian_name = NULL,
            guardian_phone = NULL,
            guardian_phone_hash = NULL,
            dob = NULL,
            pii_anonymized = 1,
            pii_anonymized_at = datetime('now'),
            consent_revoked = 1,
            consent_revoked_at = datetime('now')
        WHERE id = ? AND COALESCE(pii_anonymized, 0) = 0",
    )
    .bind(driver_id)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        tracing::error!(driver_id = %driver_id, "consent_revocation anonymization failed: {}", e);
        return Json(json!({ "error": "Failed to anonymize driver data" }));
    }

    // Audit log — record the revocation event
    accounting::log_audit(
        state,
        "drivers",
        driver_id,
        "consent_revocation",
        None,
        Some(&json!({ "reason": reason, "actor": actor }).to_string()),
        actor,
    )
    .await;

    tracing::info!(
        target: "legal_compliance",
        driver_id = %driver_id,
        reason = %reason,
        actor = ?actor,
        "LEGAL-09: PII anonymized via consent revocation"
    );

    Json(json!({
        "ok": true,
        "message": "Personal data has been anonymized. Financial records retained per legal requirements."
    }))
}

pub(super) async fn mesh_promote_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::FleetVerified).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "fleet_verified" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

pub(super) async fn mesh_retire_solution(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Json<serde_json::Value> {
    match crate::fleet_kb::update_status(&state.db, &id, rc_common::mesh_types::SolutionStatus::Retired).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "status": "retired" })),
        Ok(false) => Json(serde_json::json!({ "ok": false, "error": "not found" })),
        Err(e) => Json(serde_json::json!({ "ok": false, "error": e.to_string() })),
    }
}

