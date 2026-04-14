#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Query, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::billing;
use crate::lap_tracker;
use crate::state::AppState;
use rc_common::types::{LapData, SessionType, SimType};

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

            // Step 4: Parse CSV rows and insert each lap into the laps table.
            // This closes the gap where fallback laps were saved to disk but
            // never made visible to leaderboards, personal bests, or driver stats.
            let state_clone = state.clone();
            let session_id_clone = session_id.clone();
            let csv_data_clone = csv_data.clone();
            tokio::spawn(async move {
                parse_and_persist_fallback_laps(&state_clone, &session_id_clone, &csv_data_clone).await;
            });

            (StatusCode::OK, "received").into_response()
        }
        Ok(_) => (StatusCode::NOT_FOUND, "session not found").into_response(),
        Err(e) => {
            tracing::error!(error = %e, "telemetry_fallback: DB update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "db update failed").into_response()
        }
    }
}

/// Resolve the SimType for a billing session by looking up its kiosk_experience game field.
/// Falls back to AssettoCorsa if the session or experience is not found.
async fn resolve_sim_type_for_session(db: &sqlx::SqlitePool, session_id: &str) -> SimType {
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT ke.game
         FROM billing_sessions bs
         JOIN kiosk_experiences ke ON ke.id = bs.experience_id
         WHERE bs.id = ?
         LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();

    match row {
        Some((game,)) => match game.as_str() {
            "assetto_corsa" => SimType::AssettoCorsa,
            "assetto_corsa_evo" => SimType::AssettoCorsaEvo,
            "assetto_corsa_rally" => SimType::AssettoCorsaRally,
            "iracing" => SimType::IRacing,
            "le_mans_ultimate" | "lmu" => SimType::LeMansUltimate,
            "f1_25" | "f125" => SimType::F125,
            "forza" => SimType::Forza,
            "forza_horizon_5" => SimType::ForzaHorizon5,
            _ => {
                tracing::warn!(
                    session_id = %session_id, game = %game,
                    "telemetry_fallback: unknown game in experience, defaulting to AssettoCorsa"
                );
                SimType::AssettoCorsa
            }
        },
        None => {
            tracing::warn!(
                session_id = %session_id,
                "telemetry_fallback: no kiosk_experience found for session, defaulting to AssettoCorsa"
            );
            SimType::AssettoCorsa
        }
    }
}

/// Parse a session_type string from CSV (Debug-formatted enum) back into SessionType.
fn parse_session_type(s: &str) -> SessionType {
    match s.to_lowercase().as_str() {
        "practice" => SessionType::Practice,
        "qualifying" => SessionType::Qualifying,
        "race" => SessionType::Race,
        "hotlap" => SessionType::Hotlap,
        _ => SessionType::Practice,
    }
}

/// Parse CSV fallback data and persist each lap via lap_tracker::persist_lap().
///
/// CSV header (from rc-agent csv_lap_fallback.rs):
///   timestamp,driver_id,car,track,lap_time_ms,session_type,lap_number,valid,id,pod_id
///
/// Laps that already exist in the DB (duplicate id) are silently skipped
/// because persist_lap() uses INSERT which fails on PRIMARY KEY conflict.
async fn parse_and_persist_fallback_laps(
    state: &Arc<AppState>,
    session_id: &str,
    csv_data: &[u8],
) {
    let csv_str = match std::str::from_utf8(csv_data) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(
                session_id = %session_id,
                error = %e,
                "telemetry_fallback: CSV is not valid UTF-8"
            );
            return;
        }
    };

    let lines: Vec<&str> = csv_str.lines().collect();
    if lines.len() < 2 {
        tracing::info!(
            session_id = %session_id,
            "telemetry_fallback: CSV has no data rows (header only or empty)"
        );
        return;
    }

    // Resolve sim_type once for the entire session
    let sim_type = resolve_sim_type_for_session(&state.db, session_id).await;

    let mut persisted = 0u32;
    let mut skipped = 0u32;
    let mut errors = 0u32;

    // Skip header (line 0), parse each data row
    for (line_idx, line) in lines.iter().enumerate().skip(1) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Simple CSV parse — fields may be quoted (csv_escape in rc-agent wraps
        // values containing comma/quote/newline). Use a basic state machine.
        let fields = parse_csv_line(line);
        if fields.len() < 10 {
            tracing::warn!(
                session_id = %session_id,
                line = line_idx + 1,
                field_count = fields.len(),
                "telemetry_fallback: malformed CSV row, skipping"
            );
            skipped += 1;
            continue;
        }

        // Fields: 0=timestamp, 1=driver_id, 2=car, 3=track, 4=lap_time_ms,
        //         5=session_type, 6=lap_number, 7=valid, 8=id, 9=pod_id
        let lap_time_ms: u32 = match fields[4].parse() {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(
                    session_id = %session_id,
                    line = line_idx + 1,
                    raw = %fields[4],
                    "telemetry_fallback: invalid lap_time_ms, skipping"
                );
                skipped += 1;
                continue;
            }
        };

        let lap_number: u32 = match fields[6].parse() {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!(
                    session_id = %session_id,
                    line = line_idx + 1,
                    raw = %fields[6],
                    "telemetry_fallback: invalid lap_number, skipping"
                );
                skipped += 1;
                continue;
            }
        };

        let valid = fields[7] == "1";

        let lap_id = fields[8].to_string();
        let pod_id = rc_common::pod_id::normalize_pod_id(&fields[9])
            .unwrap_or_else(|_| fields[9].to_string());

        // Parse the CSV timestamp for created_at
        let created_at = chrono::DateTime::parse_from_rfc3339(&fields[0])
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        let lap = LapData {
            id: lap_id.clone(),
            session_id: session_id.to_string(),
            driver_id: fields[1].to_string(),
            pod_id,
            sim_type: sim_type.clone(),
            track: fields[3].to_string(),
            car: fields[2].to_string(),
            lap_number,
            lap_time_ms,
            sector1_ms: None, // CSV fallback does not capture sector times
            sector2_ms: None,
            sector3_ms: None,
            valid,
            session_type: parse_session_type(&fields[5]),
            created_at,
        };

        // Check for duplicate lap id before calling persist_lap to avoid
        // the error log that persist_lap emits on INSERT conflict.
        let exists = sqlx::query_as::<_, (i32,)>(
            "SELECT 1 FROM laps WHERE id = ? LIMIT 1"
        )
        .bind(&lap_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .is_some();

        if exists {
            tracing::debug!(
                session_id = %session_id,
                lap_id = %lap_id,
                "telemetry_fallback: lap already exists, skipping duplicate"
            );
            skipped += 1;
            continue;
        }

        let _is_record = lap_tracker::persist_lap(state, &lap).await;
        persisted += 1;
    }

    tracing::info!(
        session_id = %session_id,
        persisted,
        skipped,
        errors,
        total_rows = lines.len() - 1,
        "telemetry_fallback: CSV lap parsing complete"
    );
}

/// Parse a single CSV line, handling quoted fields (double-quote escaping).
/// Returns a Vec of unquoted field values.
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                // Check for escaped quote ("")
                if chars.peek() == Some(&'"') {
                    current.push('"');
                    chars.next(); // consume the second quote
                } else {
                    in_quotes = false;
                }
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            fields.push(std::mem::take(&mut current));
        } else {
            current.push(ch);
        }
    }
    fields.push(current);
    fields
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
