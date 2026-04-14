//! Billing rate tier CRUD handlers — extracted from pricing_routes.rs.

use super::auth_staff::venue_authority_guard;
use axum::{
    Json,
    extract::{Path, State},
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::accounting;
use crate::state::AppState;

// ─── Billing Rate Tiers (per-minute rates) ──────────────────────────────────

pub(crate) async fn list_billing_rates(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, i64, i64, bool, Option<String>)>(
        "SELECT id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, is_active, sim_type
         FROM billing_rates ORDER BY tier_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(rates) => {
            let list: Vec<Value> = rates
                .iter()
                .map(|r| {
                    json!({
                        "id": r.0, "tier_order": r.1, "tier_name": r.2,
                        "threshold_minutes": r.3, "rate_per_min_paise": r.4,
                        "is_active": r.5, "sim_type": r.6,
                    })
                })
                .collect();
            Json(json!({ "rates": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn create_billing_rate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let id = uuid::Uuid::new_v4().to_string();
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64()).unwrap_or(1);
    let tier_name = body.get("tier_name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64()).unwrap_or(0);
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64()).unwrap_or(2500);

    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).map(|s| s.to_string());

    let result = sqlx::query(
        "INSERT INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise, sim_type)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(tier_order)
    .bind(tier_name)
    .bind(threshold_minutes)
    .bind(rate_per_min_paise)
    .bind(&sim_type)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            (axum::http::StatusCode::CREATED, Json(json!({ "id": id, "tier_name": tier_name }))).into_response()
        }
        Err(e) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "error": e.to_string() }))).into_response(),
    }
}

pub(crate) async fn update_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    let tier_name = body.get("tier_name").and_then(|v| v.as_str());
    let tier_order = body.get("tier_order").and_then(|v| v.as_i64());
    let threshold_minutes = body.get("threshold_minutes").and_then(|v| v.as_i64());
    let rate_per_min_paise = body.get("rate_per_min_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());
    // sim_type: present in body = update (even if null to clear); absent = don't touch
    let sim_type_in_body = body.get("sim_type").map(|v| v.as_str().map(|s| s.to_string()));

    // SAFETY: Column names are hardcoded string literals below — not from user input.
    // All values use bind parameters (?). No SQL injection risk.
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = tier_name {
        updates.push("tier_name = ?");
        binds.push(n.to_string());
    }
    if let Some(o) = tier_order {
        updates.push("tier_order = ?");
        binds.push(o.to_string());
    }
    if let Some(t) = threshold_minutes {
        updates.push("threshold_minutes = ?");
        binds.push(t.to_string());
    }
    if let Some(r) = rate_per_min_paise {
        updates.push("rate_per_min_paise = ?");
        binds.push(r.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }
    let sim_type_val: Option<String> = if let Some(opt_s) = sim_type_in_body {
        updates.push("sim_type = ?");
        binds.push(opt_s.clone().unwrap_or_default());
        opt_s
    } else {
        None
    };
    let _ = sim_type_val; // used via binds above

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" })).into_response();
    }

    updates.push("updated_at = datetime('now')");
    let query = format!("UPDATE billing_rates SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            let new_values = serde_json::to_string(&body).ok();
            accounting::log_audit(
                &state, "billing_rates", &id, "update",
                old_snapshot.as_deref(), new_values.as_deref(), None,
            ).await;
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => Json(json!({ "error": e.to_string() })).into_response(),
    }
}

pub(crate) async fn delete_billing_rate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Phase 349: Guard — cloud instance rejects writes to venue-authoritative tables
    if let Some(rejection) = venue_authority_guard(&state, "billing_rates") {
        return rejection.into_response();
    }
    let old_snapshot = accounting::snapshot_row(&state, "billing_rates", &id).await;

    match sqlx::query(
        "UPDATE billing_rates SET is_active = 0, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    {
        Ok(_) => {
            crate::billing::refresh_rate_tiers(&state).await;
            accounting::log_audit(
                &state,
                "billing_rates",
                &id,
                "delete",
                old_snapshot.as_deref(),
                Some("{\"is_active\":false}"),
                None,
            )
            .await;
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            tracing::error!("delete_billing_rate DB error for {}: {}", id, e);
            axum::http::StatusCode::NO_CONTENT.into_response()
        }
    }
}
