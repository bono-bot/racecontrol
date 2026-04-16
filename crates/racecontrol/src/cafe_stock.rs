//! Cafe stock management — restock and image upload handlers.
//!
//! Extracted from cafe.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::cafe::CafeItem;
use crate::state::AppState;

// ─── Inventory Types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RestockRequest {
    pub quantity: i64,
}

// ─── Inventory Handlers ──────────────────────────────────────────────────────

pub async fn restock_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RestockRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.quantity <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check item exists and is_countable
    let item_check = sqlx::query_as::<_, (String, bool)>(
        "SELECT id, is_countable FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("restock_cafe_item fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item_check {
        None => return Err(StatusCode::NOT_FOUND),
        Some((_, false)) => {
            return Ok(Json(serde_json::json!({ "error": "item is not countable" })));
        }
        Some(_) => {}
    }

    sqlx::query(
        "UPDATE cafe_items SET stock_quantity = stock_quantity + ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(req.quantity)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("restock_cafe_item update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check new stock level and fire or reset low-stock alerts accordingly
    let new_stock_row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT stock_quantity, low_stock_threshold FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((new_stock, threshold)) = new_stock_row {
        if new_stock > threshold {
            // Restocked above threshold: reset cooldown so next breach re-alerts
            crate::cafe_alerts::reset_alert_cooldown(&state.db, &id).await;
        } else {
            // Still at or below threshold: check and possibly fire alert
            crate::cafe_alerts::check_low_stock_alerts(&state.db, &state.config, &id).await;
        }
    }

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
        tracing::warn!("restock_cafe_item fetch updated error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item {
        Some(i) => Ok(Json(serde_json::json!(i))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn upload_item_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Verify item exists and get old image_path
    let old_image: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT image_path FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("upload_item_image fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let mut image_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("upload_item_image multipart error: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        if field.name() == Some("file") {
            image_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| {
                        tracing::warn!("upload_item_image read bytes error: {}", e);
                        StatusCode::BAD_REQUEST
                    })?
                    .to_vec(),
            );
        }
    }

    let bytes = image_bytes.ok_or(StatusCode::BAD_REQUEST)?;

    // Decode and conditionally resize image
    let img = image::load_from_memory(&bytes).map_err(|e| {
        tracing::warn!("upload_item_image decode error: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    let resized = if img.width() > 800 {
        img.resize(800, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| {
            tracing::warn!("upload_item_image encode error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let jpeg_bytes = cursor.into_inner();

    // Save to disk
    let filename = format!("{}.jpg", Uuid::new_v4());
    tokio::fs::create_dir_all("./data/cafe-images")
        .await
        .map_err(|e| {
            tracing::warn!("upload_item_image mkdir error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tokio::fs::write(format!("./data/cafe-images/{}", filename), &jpeg_bytes)
        .await
        .map_err(|e| {
            tracing::warn!("upload_item_image write error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Delete old image if present (ignore errors)
    if let Some(old_path) = old_image
        && !old_path.is_empty() {
            let _ = tokio::fs::remove_file(format!("./data/cafe-images/{}", old_path)).await;
        }

    // Update DB
    sqlx::query(
        "UPDATE cafe_items SET image_path = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&filename)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("upload_item_image db update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        serde_json::json!({ "image_url": format!("/static/cafe-images/{}", filename) }),
    ))
}
