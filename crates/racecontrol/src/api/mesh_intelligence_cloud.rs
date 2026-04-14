#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::billing;
use crate::state::AppState;

// ─── GLD-C-03: CSV Telemetry Fallback Endpoint ───────────────────────────────

/// POST /api/v1/sessions/{id}/telemetry-fallback
///
/// GLD-C-03: Receive CSV telemetry fallback from rc-agent at session end (D-07/D-09).
/// Staff-authenticated via sentry_service_key (NOT public). Max body 50MB enforced
/// via DefaultBodyLimit layer at the route registration site.
///
/// On success:
/// - Writes CSV to `C:\RacingPoint\telemetry-fallback\{session_id}.csv`
/// - Updates `billing_sessions.csv_fallback_received_at = now()`
/// - Returns 200 "received"
///
/// Auth failures return 401. Path traversal attempts return 400. Missing 'csv'
/// multipart field returns 400. Billing session not found returns 404.
pub(crate) async fn telemetry_fallback_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    headers: axum::http::HeaderMap,
    mut multipart: axum::extract::Multipart,
) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    // Service key gate (inline, matching existing pattern in mesh_audit_seed_service)
    let expected = state.config.pods.sentry_service_key.as_deref().unwrap_or("");
    let provided = headers
        .get("X-Service-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if expected.is_empty() || provided.is_empty() || provided != expected {
        return (StatusCode::UNAUTHORIZED, "Invalid service key").into_response();
    }

    // Sanitize session_id — reject path traversal attempts
    if session_id.contains("..") || session_id.contains('/') || session_id.contains('\\') {
        return (StatusCode::BAD_REQUEST, "invalid session id").into_response();
    }

    // Extract the 'csv' multipart field
    let mut csv_bytes: Option<Vec<u8>> = None;
    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.name() == Some("csv") {
                    match field.bytes().await {
                        Ok(b) => { csv_bytes = Some(b.to_vec()); break; }
                        Err(e) => {
                            return (StatusCode::BAD_REQUEST, format!("read error: {e}"))
                                .into_response();
                        }
                    }
                }
                // skip fields that aren't 'csv'
            }
            Ok(None) => break,
            Err(e) => {
                return (StatusCode::BAD_REQUEST, format!("multipart error: {e}")).into_response();
            }
        }
    }
    let Some(csv_data) = csv_bytes else {
        return (StatusCode::BAD_REQUEST, "missing 'csv' field").into_response();
    };
    let csv_len = csv_data.len();

    // Write to C:\RacingPoint\telemetry-fallback\{session_id}.csv
    let dir = std::path::Path::new(r"C:\RacingPoint\telemetry-fallback");
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        tracing::error!(error = %e, "telemetry_fallback: failed to create dir");
        return (StatusCode::INTERNAL_SERVER_ERROR, "mkdir failed").into_response();
    }
    let path = dir.join(format!("{session_id}.csv"));
    if let Err(e) = tokio::fs::write(&path, &csv_data).await {
        tracing::error!(error = %e, path = %path.display(), "telemetry_fallback: write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "write failed").into_response();
    }

    // Update billing_sessions.csv_fallback_received_at
    let now_utc = chrono::Utc::now().to_rfc3339();
    let update_res = sqlx::query(
        "UPDATE billing_sessions SET csv_fallback_received_at = ? WHERE id = ?"
    )
    .bind(&now_utc)
    .bind(&session_id)
    .execute(&state.db)
    .await;

    match update_res {
        Ok(r) if r.rows_affected() > 0 => {
            tracing::info!(
                session_id = %session_id,
                bytes = csv_len,
                "telemetry_fallback: csv received and stored"
            );
            (StatusCode::OK, "received").into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, "session not found").into_response(),
        Err(e) => {
            tracing::error!(error = %e, "telemetry_fallback: DB update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "db update failed").into_response()
        }
    }
}

// ─── Cloud Mesh KB Sync (v26.0 Phase 227) ───────────────────────────────────

/// Venue pushes fleet-verified + hardened solutions to cloud KB.
/// Request body: { "venue_id": "rp-hyderabad", "solutions": [...] }
pub(crate) async fn cloud_mesh_sync(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let venue_id = body.get("venue_id").and_then(|v| v.as_str()).unwrap_or("unknown");
    let solutions = match body.get("solutions").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return Json(serde_json::json!({ "ok": false, "error": "solutions array required" })),
    };

    let mut imported = 0u32;
    let mut errors = 0u32;

    for sol_val in solutions {
        // Parse and tag with venue_id
        let mut sol: rc_common::mesh_types::MeshSolution = match serde_json::from_value(sol_val.clone()) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Cloud mesh sync: failed to parse solution: {e}");
                errors += 1;
                continue;
            }
        };
        sol.venue_id = Some(venue_id.to_string());

        if let Err(e) = crate::fleet_kb::insert_solution(&state.db, &sol).await {
            tracing::warn!("Cloud mesh sync: failed to insert solution {}: {e}", sol.id);
            errors += 1;
        } else {
            imported += 1;
        }
    }

    tracing::info!("Cloud mesh sync from venue {venue_id}: imported={imported} errors={errors}");
    Json(serde_json::json!({ "ok": true, "imported": imported, "errors": errors }))
}

/// New venue pulls the full cloud KB to seed their local fleet KB.
/// Query params: ?venue_id=xxx (optional — excludes own solutions to avoid loops)
pub(crate) async fn cloud_mesh_pull(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let requesting_venue = params.get("venue_id").map(|s| s.as_str());

    // Pull all fleet_verified + hardened solutions
    let verified = crate::fleet_kb::list_solutions(&state.db, Some("fleet_verified"), 500, 0).await.unwrap_or_default();
    let hardened = crate::fleet_kb::list_solutions(&state.db, Some("hardened"), 500, 0).await.unwrap_or_default();

    let mut all: Vec<rc_common::mesh_types::MeshSolution> = verified.into_iter().chain(hardened).collect();

    // Exclude requesting venue's own solutions (prevent sync loop)
    if let Some(vid) = requesting_venue {
        all.retain(|s| s.venue_id.as_deref() != Some(vid));
    }

    // Mark external solutions for the requesting venue
    for sol in &mut all {
        if sol.venue_id.is_some() && sol.venue_id.as_deref() != requesting_venue {
            // Tag as external — needs local verification before auto-apply
            if let Some(tags) = sol.tags.as_mut() {
                if !tags.contains(&"external".to_string()) {
                    tags.push("external".to_string());
                }
            } else {
                sol.tags = Some(vec!["external".to_string()]);
            }
        }
    }

    Json(serde_json::json!({ "solutions": all, "count": all.len() }))
}


/// GET /api/v1/reconciliation/status — returns last reconciliation run info.
pub(crate) async fn reconciliation_status() -> Json<serde_json::Value> {
    Json(billing::get_reconciliation_status())
}

/// POST /api/v1/reconciliation/run — triggers an immediate reconciliation run.
pub(crate) async fn reconciliation_run(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    billing::run_reconciliation_public(&state).await;
    Json(billing::get_reconciliation_status())
}
