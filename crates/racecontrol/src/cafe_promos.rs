use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::state::AppState;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Serializable active promo returned by the public endpoint and used in evaluate_promos.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActivePromo {
    pub id: String,
    pub name: String,
    pub promo_type: String,
    pub config: serde_json::Value,
    pub stacking_group: Option<String>,
    pub time_label: Option<String>, // e.g. "Active until 6:00 PM" or None
}

/// Result from evaluate_promos — best discount to apply.
#[derive(Debug, Default, Clone)]
pub struct PromoEvalResult {
    pub applied_promo_id: Option<String>,
    pub promo_name: Option<String>,
    pub discount_paise: i64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafePromo {
    pub id: String,
    pub name: String,
    pub promo_type: String,
    pub config: String,
    pub is_active: bool,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafePromoRequest {
    pub name: String,
    pub promo_type: String,
    pub config: serde_json::Value,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCafePromoRequest {
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
    pub is_active: Option<bool>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub stacking_group: Option<String>,
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn validate_promo_type(promo_type: &str) -> bool {
    matches!(promo_type, "combo" | "happy_hour" | "gaming_bundle")
}

/// Returns current IST time as "HH:MM" string.
fn ist_now_hhmm() -> String {
    // IST = UTC+5:30 = UTC + 19800 seconds
    let now_utc = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let ist_secs = now_utc + 19800;
    let hours = (ist_secs / 3600) % 24;
    let minutes = (ist_secs % 3600) / 60;
    format!("{:02}:{:02}", hours, minutes)
}

/// Returns true if `now` (HH:MM) is within [start, end) window.
/// Handles overnight wrap (e.g. 22:00 to 02:00) correctly.
pub fn time_in_window(now: &str, start: &str, end: &str) -> bool {
    if start == end {
        // MMA iter2: start==end means "no window restriction" — always active
        return true;
    }
    if start < end {
        now >= start && now < end
    } else {
        // overnight: e.g. 22:00 to 02:00
        now >= start || now < end
    }
}

/// Format "15:00" as "3:00 PM".
fn fmt_hhmm(hhmm: &str) -> String {
    let parts: Vec<&str> = hhmm.splitn(2, ':').collect();
    if parts.len() != 2 {
        return hhmm.to_string();
    }
    let h: u32 = parts[0].parse().unwrap_or(0);
    let m = parts[1];
    let period = if h < 12 { "AM" } else { "PM" };
    let h12 = match h % 12 {
        0 => 12,
        v => v,
    };
    format!("{}:{} {}", h12, m, period)
}

/// Evaluate which promo (if any) applies to this cart and return the best discount.
/// cart_items: Vec<(item_id, quantity)>
/// active_promos: slice of currently-active promos (time-filtered)
/// total_paise: gross cart total before any discount (needed for happy_hour %)
pub fn evaluate_promos(
    cart_items: &[(String, i64)],
    active_promos: &[ActivePromo],
    total_paise: i64,
) -> PromoEvalResult {
    let cart_map: std::collections::HashMap<&str, i64> =
        cart_items.iter().map(|(id, qty)| (id.as_str(), *qty)).collect();

    // group_key -> (discount_paise, promo_id, promo_name)
    let mut group_best: std::collections::HashMap<String, (i64, String, String)> =
        std::collections::HashMap::new();

    for promo in active_promos {
        let discount = calc_promo_discount(promo, &cart_map, total_paise);
        if discount <= 0 {
            continue;
        }
        let key = promo
            .stacking_group
            .clone()
            .unwrap_or_else(|| promo.id.clone());
        let entry = group_best
            .entry(key)
            .or_insert((0, promo.id.clone(), promo.name.clone()));
        if discount > entry.0 {
            *entry = (discount, promo.id.clone(), promo.name.clone());
        }
    }

    // Pick the single largest discount across all stacking groups (v1 simplification)
    if let Some((discount, id, name)) = group_best.values().max_by_key(|(d, _, _)| *d) {
        PromoEvalResult {
            applied_promo_id: Some(id.clone()),
            promo_name: Some(name.clone()),
            discount_paise: *discount,
        }
    } else {
        PromoEvalResult::default()
    }
}

/// Calculate the discount in paise that a single promo gives for this cart.
/// Returns 0 if promo conditions are not met.
fn calc_promo_discount(
    promo: &ActivePromo,
    cart_map: &std::collections::HashMap<&str, i64>,
    total_paise: i64,
) -> i64 {
    match promo.promo_type.as_str() {
        "combo" => {
            let items = match promo.config.get("items").and_then(|v| v.as_array()) {
                Some(arr) => arr.clone(),
                None => return 0,
            };
            let mut gross: i64 = 0;
            for req in &items {
                let item_id = match req.get("item_id").and_then(|v| v.as_str()) {
                    Some(id) => id,
                    None => return 0,
                };
                let req_qty = req.get("quantity").and_then(|v| v.as_i64()).unwrap_or(1);
                if cart_map.get(item_id).copied().unwrap_or(0) < req_qty {
                    return 0; // condition not met
                }
                let unit_price = req
                    .get("unit_price_paise")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                gross += unit_price * req_qty;
            }
            if let Some(bundle_price) = promo
                .config
                .get("bundle_price_paise")
                .and_then(|v| v.as_i64())
            {
                return (gross - bundle_price).max(0);
            }
            if let Some(pct) = promo
                .config
                .get("discount_percent")
                .and_then(|v| v.as_i64())
            {
                return (gross * pct / 100).max(0);
            }
            0
        }
        "happy_hour" => {
            let pct = match promo
                .config
                .get("discount_percent")
                .and_then(|v| v.as_i64())
            {
                Some(p) if p > 0 && p <= 100 => p,
                _ => return 0,
            };
            (total_paise * pct / 100).max(0)
        }
        _ => 0, // gaming_bundle: display only, no auto-apply in v1
    }
}

// ─── Handlers ────────────────────────────────────────────────────────────────

/// Public endpoint — returns active promos filtered to current IST time window.
/// No auth required (frontend displays to customers before ordering).
pub async fn list_active_promos(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<Vec<ActivePromo>>), (StatusCode, Json<serde_json::Value>)> {
    let promos = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE is_active = 1 ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let now_ist = ist_now_hhmm();
    let active: Vec<ActivePromo> = promos
        .into_iter()
        .filter_map(|p| {
            // Check time window: if both start and end are set, must be within window
            if let (Some(start), Some(end)) = (&p.start_time, &p.end_time) {
                if !time_in_window(&now_ist, start, end) {
                    return None;
                }
            }
            let config: serde_json::Value = serde_json::from_str(&p.config)
                .unwrap_or_else(|_| serde_json::Value::Object(Default::default()));
            let time_label = match (&p.start_time, &p.end_time) {
                (Some(_), Some(end)) => Some(format!("Active until {}", fmt_hhmm(end))),
                _ => None,
            };
            Some(ActivePromo {
                id: p.id,
                name: p.name,
                promo_type: p.promo_type,
                config,
                stacking_group: p.stacking_group,
                time_label,
            })
        })
        .collect();

    Ok((StatusCode::OK, Json(active)))
}

pub async fn list_cafe_promos(
    State(state): State<Arc<AppState>>,
) -> Result<(StatusCode, Json<Vec<CafePromo>>), (StatusCode, Json<serde_json::Value>)> {
    let promos = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promos)))
}

pub async fn create_cafe_promo(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafePromoRequest>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    if !validate_promo_type(&req.promo_type) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid promo_type; must be 'combo', 'happy_hour', or 'gaming_bundle'"})),
        ));
    }

    // Validate time window: if both start and end are provided, start must be before end
    // (overnight promos like 22:00-02:00 are allowed — only reject identical times)
    if let (Some(start), Some(end)) = (&req.start_time, &req.end_time) {
        if start == end {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "start_time and end_time must be different"})),
            ));
        }
    }

    let id = Uuid::new_v4().to_string();
    let config_str = serde_json::to_string(&req.config).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;
    let is_active = req.is_active.unwrap_or(false);

    sqlx::query(
        "INSERT INTO cafe_promos (id, name, promo_type, config, is_active, start_time, end_time, stacking_group)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(&req.promo_type)
    .bind(&config_str)
    .bind(is_active)
    .bind(&req.start_time)
    .bind(&req.end_time)
    .bind(&req.stacking_group)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::CREATED, Json(promo)))
}

pub async fn update_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCafePromoRequest>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    let mut set_clauses: Vec<String> = Vec::new();
    let mut has_fields = false;

    if req.name.is_some() {
        set_clauses.push("name = ?".to_string());
        has_fields = true;
    }
    if req.config.is_some() {
        set_clauses.push("config = ?".to_string());
        has_fields = true;
    }
    if req.is_active.is_some() {
        set_clauses.push("is_active = ?".to_string());
        has_fields = true;
    }
    if req.start_time.is_some() {
        set_clauses.push("start_time = ?".to_string());
        has_fields = true;
    }
    if req.end_time.is_some() {
        set_clauses.push("end_time = ?".to_string());
        has_fields = true;
    }
    if req.stacking_group.is_some() {
        set_clauses.push("stacking_group = ?".to_string());
        has_fields = true;
    }

    if !has_fields {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no fields to update"})),
        ));
    }

    // Validate time window when both times are provided in this update
    if let (Some(start), Some(end)) = (&req.start_time, &req.end_time) {
        if start == end {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "start_time and end_time must be different"})),
            ));
        }
    }

    set_clauses.push("updated_at = datetime('now')".to_string());

    let sql = format!(
        "UPDATE cafe_promos SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql);

    if let Some(ref name) = req.name {
        query = query.bind(name);
    }
    if let Some(ref config) = req.config {
        let config_str = serde_json::to_string(config).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;
        query = query.bind(config_str);
    }
    if let Some(is_active) = req.is_active {
        query = query.bind(is_active);
    }
    if let Some(ref start_time) = req.start_time {
        query = query.bind(start_time);
    }
    if let Some(ref end_time) = req.end_time {
        query = query.bind(end_time);
    }
    if let Some(ref stacking_group) = req.stacking_group {
        query = query.bind(stacking_group);
    }

    let result = query
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promo)))
}

pub async fn delete_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query("DELETE FROM cafe_promos WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn toggle_cafe_promo(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<CafePromo>), (StatusCode, Json<serde_json::Value>)> {
    let result = sqlx::query(
        "UPDATE cafe_promos SET is_active = NOT is_active, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    if result.rows_affected() == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "promo not found"})),
        ));
    }

    let promo = sqlx::query_as::<_, CafePromo>(
        "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
         FROM cafe_promos WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        )
    })?;

    Ok((StatusCode::OK, Json(promo)))
}
