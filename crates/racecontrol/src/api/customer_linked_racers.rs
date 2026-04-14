#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::wallet;
use crate::state::AppState;

// ─── Linked Racers ──────────────────────────────────────────────────────────

pub(crate) const MAX_LINKED_RACERS: i64 = 3;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
pub(crate) struct AddRacerRequest {
    name: String,
    dob: String,
    waiver_consent: bool,
}

pub(crate) async fn customer_add_racer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(req): Json<AddRacerRequest>,
) -> Json<Value> {
    let parent_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    if !req.waiver_consent {
        return Json(json!({ "error": "Waiver consent is required" }));
    }

    let name = match crate::input_validation::validate_name(&req.name) {
        Ok(n) => n,
        Err(e) => return Json(json!({ "error": e })),
    };

    let dob = match chrono::NaiveDate::parse_from_str(&req.dob, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return Json(json!({ "error": "Invalid date format. Use YYYY-MM-DD" })),
    };

    let today = chrono::Utc::now().date_naive();
    let age = (today - dob).num_days() / 365;

    if age < 5 {
        return Json(json!({ "error": "Minimum age for racers is 5 years" }));
    }

    // Check racer cap (max 3 linked racers per account)
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM drivers WHERE linked_to = ?")
        .bind(&parent_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or((0,));

    if count.0 >= MAX_LINKED_RACERS {
        return Json(json!({ "error": format!("Maximum {} racers per account", MAX_LINKED_RACERS) }));
    }

    // Check duplicate name+DOB
    let duplicate: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM drivers WHERE name = ? AND dob = ? AND registration_completed = 1",
    )
    .bind(&name)
    .bind(&req.dob)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if duplicate.is_some() {
        return Json(json!({ "error": "A racer with this name and date of birth already exists" }));
    }

    // Get parent info for guardian fields
    let parent = sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT name, phone FROM drivers WHERE id = ?",
    )
    .bind(&parent_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let (guardian_name, guardian_phone) = match parent {
        Some((pname, pphone)) => (Some(pname), pphone),
        None => return Json(json!({ "error": "Parent account not found" })),
    };

    let racer_id = format!("drv_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap_or("x"));
    let customer_id = {
        // Same RP### format as venue_register — see that function for details.
        let max: Option<(Option<i64>,)> = sqlx::query_as(
            "SELECT MAX(CAST(SUBSTR(customer_id, 3) AS INTEGER)) FROM drivers WHERE customer_id LIKE 'RP%'",
        )
        .fetch_optional(&state.db)
        .await
        .unwrap_or(None);
        let next_num = max
            .and_then(|m| m.0)
            .map(|n| n as u64)
            .unwrap_or(0) + 1;
        format!("RP{:03}", next_num)
    };

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, dob, customer_id, linked_to, waiver_signed, waiver_signed_at, waiver_version, guardian_name, guardian_phone, registration_completed, created_at)
         VALUES (?, ?, ?, ?, ?, 1, datetime('now'), 'v1.0', ?, ?, 1, datetime('now'))",
    )
    .bind(&racer_id)
    .bind(&name)
    .bind(&req.dob)
    .bind(&customer_id)
    .bind(&parent_id)
    .bind(&guardian_name)
    .bind(&guardian_phone)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => {
            // Create empty wallet for tracking (can be imported to own account later)
            let _ = wallet::ensure_wallet(&state, &racer_id).await;

            tracing::info!("Racer {} added by parent {} (age: {})", racer_id, parent_id, age);
            Json(json!({
                "status": "ok",
                "racer_id": racer_id,
                "name": name,
                "customer_id": customer_id,
                "is_minor": age < 18,
            }))
        }
        Err(e) => Json(json!({ "error": format!("Failed to add racer: {}", e) })),
    }
}

pub(crate) async fn customer_list_racers(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let parent_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let rows = sqlx::query_as::<_, (String, String, Option<String>, String, i64, i64, bool)>(
        "SELECT id, name, dob, customer_id, total_laps, total_time_ms, COALESCE(has_used_trial, 0)
         FROM drivers WHERE linked_to = ? ORDER BY created_at",
    )
    .bind(&parent_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let racers: Vec<serde_json::Value> = rows.iter().map(|r| {
        let age = r.2.as_ref().and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|dob| (chrono::Utc::now().date_naive() - dob).num_days() / 365)
            .unwrap_or(0);
        json!({
            "id": r.0,
            "name": r.1,
            "dob": r.2,
            "customer_id": r.3,
            "total_laps": r.4,
            "total_time_ms": r.5,
            "has_used_trial": r.6,
            "age": age,
            "is_minor": age < 18,
        })
    }).collect();

    Json(json!({ "racers": racers, "max_racers": MAX_LINKED_RACERS }))
}

pub(crate) async fn customer_waiver_status(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let row = sqlx::query_as::<_, (bool, bool)>(
        "SELECT COALESCE(waiver_signed, 0), COALESCE(registration_completed, 0) FROM drivers WHERE id = ?",
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some((waiver, registered))) => Json(json!({
            "waiver_signed": waiver,
            "registration_completed": registered,
        })),
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": format!("DB error: {}", e) })),
    }
}
