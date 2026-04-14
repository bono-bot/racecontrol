use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

// Re-export extracted modules
pub use crate::cafe_import::*;
pub use crate::cafe_orders::*;
pub use crate::cafe_stock::*;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafeCategory {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafeItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub is_available: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub image_path: Option<String>,
    pub is_countable: bool,
    pub stock_quantity: i64,
    pub low_stock_threshold: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafeItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub is_countable: Option<bool>,
    pub stock_quantity: Option<i64>,
    pub low_stock_threshold: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCafeItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub selling_price_paise: Option<i64>,
    pub cost_price_paise: Option<i64>,
    pub is_countable: Option<bool>,
    pub stock_quantity: Option<i64>,
    pub low_stock_threshold: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafeCategoryRequest {
    pub name: String,
    pub sort_order: Option<i64>,
}


// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn list_cafe_items(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let items = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items ORDER BY name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("list_cafe_items DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total = items.len();
    // F1-SEC: Strip cost_price, stock internals from public response.
    // cost_price_paise is business intelligence — never expose to customers.
    let safe_items: Vec<serde_json::Value> = items.iter().map(|item| {
        serde_json::json!({
            "id": item.id,
            "name": item.name,
            "description": item.description,
            "category_id": item.category_id,
            "selling_price_paise": item.selling_price_paise,
            "is_available": item.is_available,
            "image_path": item.image_path,
        })
    }).collect();
    Ok(Json(serde_json::json!({ "items": safe_items, "total": total, "page": 1 })))
}

pub async fn create_cafe_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafeItemRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    // Validate inputs
    if req.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.selling_price_paise <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.cost_price_paise < 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check category exists
    let cat_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM cafe_categories WHERE id = ?",
    )
    .bind(&req.category_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_item category check error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if cat_exists == 0 {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "category_id not found" })),
        ));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO cafe_items (id, name, description, category_id, selling_price_paise, cost_price_paise, is_available, is_countable, stock_quantity, low_stock_threshold)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
    )
    .bind(&id)
    .bind(req.name.trim())
    .bind(req.description.as_deref())
    .bind(&req.category_id)
    .bind(req.selling_price_paise)
    .bind(req.cost_price_paise)
    .bind(req.is_countable.unwrap_or(false))
    .bind(req.stock_quantity.unwrap_or(0))
    .bind(req.low_stock_threshold.unwrap_or(0))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_item insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn update_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCafeItemRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // If category_id provided, verify it exists
    if let Some(ref cat_id) = req.category_id {
        let cat_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM cafe_categories WHERE id = ?",
        )
        .bind(cat_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            tracing::warn!("update_cafe_item category check error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if cat_exists == 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Build dynamic SET clause
    let mut set_clauses: Vec<String> = Vec::new();
    set_clauses.push("updated_at = datetime('now')".to_string());

    if req.name.is_some() {
        set_clauses.push("name = ?".to_string());
    }
    if req.description.is_some() {
        set_clauses.push("description = ?".to_string());
    }
    if req.category_id.is_some() {
        set_clauses.push("category_id = ?".to_string());
    }
    if req.selling_price_paise.is_some() {
        set_clauses.push("selling_price_paise = ?".to_string());
    }
    if req.cost_price_paise.is_some() {
        set_clauses.push("cost_price_paise = ?".to_string());
    }
    if req.is_countable.is_some() {
        set_clauses.push("is_countable = ?".to_string());
    }
    if req.stock_quantity.is_some() {
        set_clauses.push("stock_quantity = ?".to_string());
    }
    if req.low_stock_threshold.is_some() {
        set_clauses.push("low_stock_threshold = ?".to_string());
    }

    let query_str = format!(
        "UPDATE cafe_items SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut q = sqlx::query(&query_str);
    if let Some(ref name) = req.name {
        q = q.bind(name);
    }
    if let Some(ref desc) = req.description {
        q = q.bind(desc);
    }
    if let Some(ref cat_id) = req.category_id {
        q = q.bind(cat_id);
    }
    if let Some(price) = req.selling_price_paise {
        q = q.bind(price);
    }
    if let Some(cost) = req.cost_price_paise {
        q = q.bind(cost);
    }
    if let Some(countable) = req.is_countable {
        q = q.bind(countable);
    }
    if let Some(qty) = req.stock_quantity {
        q = q.bind(qty);
    }
    if let Some(threshold) = req.low_stock_threshold {
        q = q.bind(threshold);
    }
    q = q.bind(&id);

    q.execute(&state.db).await.map_err(|e| {
        tracing::warn!("update_cafe_item execute error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Return updated item
    let item = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("update_cafe_item fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item {
        Some(i) => Ok(Json(serde_json::json!(i))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query("DELETE FROM cafe_items WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::warn!("delete_cafe_item error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn toggle_cafe_item_availability(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query(
        "UPDATE cafe_items SET is_available = NOT is_available, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("toggle_cafe_item_availability error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Fetch new value
    let new_available = sqlx::query_scalar::<_, bool>(
        "SELECT is_available FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("toggle_cafe_item_availability fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "id": id, "is_available": new_available })))
}

pub async fn list_cafe_categories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let categories = sqlx::query_as::<_, CafeCategory>(
        "SELECT id, name, sort_order, created_at FROM cafe_categories ORDER BY sort_order ASC, name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("list_cafe_categories error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "categories": categories })))
}

pub async fn create_cafe_category(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafeCategoryRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = Uuid::new_v4().to_string();
    let sort_order = req.sort_order.unwrap_or(0);

    sqlx::query(
        "INSERT OR IGNORE INTO cafe_categories (id, name, sort_order) VALUES (?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(sort_order)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_category insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // SELECT by name to return (handles both new and existing)
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM cafe_categories WHERE name = ?",
    )
    .bind(&req.name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_category fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "id": row.0, "name": row.1 })))
}

pub async fn public_menu(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Custom row type for JOIN result
    #[derive(Debug, Serialize, sqlx::FromRow)]
    struct MenuItem {
        id: String,
        name: String,
        description: Option<String>,
        category_id: String,
        category_name: String,
        selling_price_paise: i64,
        cost_price_paise: i64,
        is_available: bool,
        created_at: Option<String>,
        updated_at: Option<String>,
        image_path: Option<String>,
        is_countable: bool,
        stock_quantity: i64,
    }

    let items = sqlx::query_as::<_, MenuItem>(
        "SELECT ci.id, ci.name, ci.description, ci.category_id,
                cc.name AS category_name,
                ci.selling_price_paise, ci.cost_price_paise,
                ci.is_available, ci.created_at, ci.updated_at,
                ci.image_path, ci.is_countable, ci.stock_quantity
         FROM cafe_items ci
         JOIN cafe_categories cc ON ci.category_id = cc.id
         WHERE ci.is_available = 1
         ORDER BY cc.sort_order ASC, ci.name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("public_menu error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut val = serde_json::to_value(item).unwrap_or_default();
            if let Some(obj) = val.as_object_mut() {
                let out_of_stock = item.is_countable && item.stock_quantity <= 0;
                obj.insert(
                    "out_of_stock".to_string(),
                    serde_json::Value::Bool(out_of_stock),
                );
            }
            val
        })
        .collect();

    let total = items_json.len();
    Ok(Json(serde_json::json!({ "items": items_json, "total": total, "page": 1 })))
}




// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "cafe_tests.rs"]
mod tests;
