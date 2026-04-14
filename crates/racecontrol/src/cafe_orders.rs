//! Cafe order processing — place orders, receipts, order history.
//!
//! Extracted from cafe.rs (Phase 385, v49.0 Architecture Completion).
//! Contains order types, place_cafe_order (the core transaction),
//! WhatsApp receipt, thermal receipt, and customer order listing.

#[path = "cafe_order_types.rs"]
mod cafe_order_types;
pub use cafe_order_types::*;

#[path = "cafe_order_receipts.rs"]
mod cafe_order_receipts;
pub use cafe_order_receipts::*;

use std::sync::Arc;
use axum::{Json, extract::State};
use axum::http::StatusCode;
use uuid::Uuid;

use crate::state::AppState;

// ─── Order Handlers ───────────────────────────────────────────────────────────

/// Core order logic shared between staff and customer routes.
pub async fn place_cafe_order_inner(
    state: &Arc<AppState>,
    req: PlaceOrderRequest,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // ── Validation (before transaction) ──────────────────────────────────────
    if req.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "items must not be empty" })),
        ));
    }
    if req.driver_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "driver_id must not be empty" })),
        ));
    }
    for item in &req.items {
        if item.quantity < 1 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "each item quantity must be >= 1" })),
            ));
        }
    }

    // ── Step A: Acquire raw connection and BEGIN IMMEDIATE ────────────────────
    let mut conn = state.db.acquire().await.map_err(|e| {
        tracing::warn!("place_cafe_order: failed to acquire connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database unavailable" })),
        )
    })?;

    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: BEGIN IMMEDIATE failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Could not acquire write lock" })),
            )
        })?;

    // ── Step B: Validate all items exist, are available, check stock ──────────
    let mut verified_items: Vec<VerifiedOrderItem> = Vec::new();
    for req_item in &req.items {
        let row: Option<(String, String, i64, bool, i64, bool)> = sqlx::query_as(
            "SELECT id, name, selling_price_paise, is_countable, stock_quantity, is_available
             FROM cafe_items WHERE id = ?",
        )
        .bind(&req_item.item_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: item lookup error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error during item lookup" })),
            )
        })?;

        match row {
            None => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Item not found or unavailable: {}", req_item.item_id)
                    })),
                ));
            }
            Some((id, name, price, is_countable, stock_qty, is_available)) => {
                if !is_available {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": format!("Item not found or unavailable: {}", name)
                        })),
                    ));
                }
                if is_countable && stock_qty < req_item.quantity {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": format!(
                                "Out of stock: {} (available: {}, requested: {})",
                                name, stock_qty, req_item.quantity
                            )
                        })),
                    ));
                }
                verified_items.push(VerifiedOrderItem {
                    item_id: id,
                    name,
                    quantity: req_item.quantity,
                    unit_price_paise: price,
                    is_countable,
                });
            }
        }
    }

    // ── Step C: Calculate total and build OrderItemDetail list ────────────────
    let mut total_paise: i64 = 0;
    let mut order_item_details: Vec<OrderItemDetail> = Vec::new();
    for item in &verified_items {
        let line_total = item.unit_price_paise * item.quantity;
        total_paise += line_total;
        order_item_details.push(OrderItemDetail {
            item_id: item.item_id.clone(),
            name: item.name.clone(),
            quantity: item.quantity,
            unit_price_paise: item.unit_price_paise,
            line_total_paise: line_total,
        });
    }

    // ── Step C2: Evaluate promos and apply best discount ─────────────────────
    // Fetch currently active promos from DB (outside transaction — read-only, non-blocking)
    let active_promos: Vec<crate::cafe_promos::ActivePromo> = {
        let now_ist = {
            let now_utc = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let ist_secs = now_utc + 19800;
            let h = (ist_secs / 3600) % 24;
            let m = (ist_secs % 3600) / 60;
            format!("{:02}:{:02}", h, m)
        };
        sqlx::query_as::<_, crate::cafe_promos::CafePromo>(
            "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
             FROM cafe_promos WHERE is_active = 1",
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default() // promo fetch failure must NOT block the order
        .into_iter()
        .filter_map(|p| {
            if let (Some(start), Some(end)) = (&p.start_time, &p.end_time) {
                // APP-02 fix: use time_in_window which handles overnight wrap (e.g. 23:00-01:00)
                if !crate::cafe_promos::time_in_window(now_ist.as_str(), start.as_str(), end.as_str()) {
                    return None;
                }
            }
            let config = serde_json::from_str(&p.config).unwrap_or_default();
            Some(crate::cafe_promos::ActivePromo {
                id: p.id,
                name: p.name,
                promo_type: p.promo_type,
                config,
                stacking_group: p.stacking_group,
                time_label: None,
            })
        })
        .collect()
    };

    let cart_items: Vec<(String, i64)> = verified_items
        .iter()
        .map(|v| (v.item_id.clone(), v.quantity))
        .collect();

    let promo_result =
        crate::cafe_promos::evaluate_promos(&cart_items, &active_promos, total_paise);
    let discount_paise = promo_result.discount_paise.min(total_paise); // discount cannot exceed total
    let final_total_paise = total_paise - discount_paise;

    // ── Step D: Decrement stock for countable items (with race check) ─────────
    for item in &verified_items {
        if item.is_countable {
            let result = sqlx::query(
                "UPDATE cafe_items SET stock_quantity = stock_quantity - ?, updated_at = datetime('now')
                 WHERE id = ? AND is_countable = 1 AND stock_quantity >= ?",
            )
            .bind(item.quantity)
            .bind(&item.item_id)
            .bind(item.quantity)
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                tracing::warn!("place_cafe_order: stock decrement error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Stock update failed" })),
                )
            })?;

            if result.rows_affected() == 0 {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err((
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "Stock changed during order, please retry"
                    })),
                ));
            }
        }
    }

    // ── Step E: Generate receipt number ──────────────────────────────────────
    let today_prefix = chrono::Utc::now().format("%Y%m%d").to_string();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cafe_orders WHERE receipt_number LIKE ?",
    )
    .bind(format!("RP-{}-%%", today_prefix))
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        tracing::warn!("place_cafe_order: receipt count error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Receipt generation failed" })),
        )
    })?;

    let receipt_number = format!("RP-{}-{:04}", today_prefix, count + 1);

    // ── Step F: Generate order_id ─────────────────────────────────────────────
    let order_id = Uuid::new_v4().to_string();

    // ── Step G: COMMIT transaction (stock decremented, receipt reserved) ──────
    sqlx::query("COMMIT")
        .execute(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: COMMIT failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Transaction commit failed" })),
            )
        })?;

    // Drop raw connection — pool is available again
    drop(conn);

    // ── Step H: Wallet debit (outside raw transaction, uses pool internally) ──
    let order_id_for_log = order_id.clone();
    let debit_result = crate::wallet::debit(
        state,
        &req.driver_id,
        final_total_paise,
        "cafe_order",
        Some(&order_id),
        Some(&format!("Cafe order {}", receipt_number)),
    )
    .await;

    let (new_balance, wallet_txn_id) = match debit_result {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("place_cafe_order: wallet debit failed for order {}: {}", order_id_for_log, e);
            // APP-01 fix (MMA iter2: tracks per-item success to prevent double-rollback)
            rollback_stock(state, &verified_items, &order_id_for_log).await;
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            ));
        }
    };

    // ── Step I: Insert order record ───────────────────────────────────────────
    let items_json = serde_json::to_string(&order_item_details).unwrap_or_else(|_| "[]".to_string());
    if let Err(e) = sqlx::query(
        "INSERT INTO cafe_orders (id, receipt_number, driver_id, items, total_paise, discount_paise, applied_promo_id, wallet_txn_id, status, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'confirmed', ?)",
    )
    .bind(&order_id)
    .bind(&receipt_number)
    .bind(&req.driver_id)
    .bind(&items_json)
    .bind(final_total_paise)
    .bind(discount_paise)
    .bind(&promo_result.applied_promo_id)
    .bind(&wallet_txn_id)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    {
        // APP-03 fix: compensating wallet refund if order INSERT fails after debit
        // MMA iter2: log refund failures as CRITICAL instead of silently discarding
        tracing::error!("place_cafe_order: order insert failed for {}: {} — issuing compensating refund", order_id, e);
        match crate::wallet::refund(
            state,
            &req.driver_id,
            final_total_paise,
            None, // MMA iter2: no order_id ref (order doesn't exist) — prevents orphaned refs
            Some(&format!("COMPENSATING REFUND: cafe order {} insert failed", order_id)),
        )
        .await
        {
            Ok(_) => tracing::info!("place_cafe_order: compensating refund issued for failed order {}", order_id),
            Err(refund_err) => {
                // CRITICAL: customer charged with no order AND refund failed
                tracing::error!("CRITICAL: place_cafe_order: refund FAILED for order {} ({}p charged to {}): {}",
                    order_id, final_total_paise, req.driver_id, refund_err);
                // Fire-and-forget WhatsApp alert to staff
                let config_clone = state.config.clone();
                let alert_msg = format!("CRITICAL: Cafe refund failed! Order {} — {}p stuck on driver {}. Manual refund required.",
                    order_id, final_total_paise, req.driver_id);
                tokio::spawn(async move {
                    crate::whatsapp_alerter::send_whatsapp(&config_clone, &alert_msg).await;
                });
            }
        }
        // Rollback stock with retry (same as debit-failure path)
        rollback_stock(state, &verified_items, &order_id).await;
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to record order — payment refunded" })),
        ));
    }

    // ── Step J: Fire low-stock alerts (non-blocking) ──────────────────────────
    for item in &verified_items {
        if item.is_countable {
            crate::cafe_alerts::check_low_stock_alerts(&state.db, &state.config, &item.item_id).await;
        }
    }

    // ── Step L: Send WhatsApp receipt (fire-and-forget) ───────────────────────
    {
        let state_l = state.clone();
        let driver_id = req.driver_id.clone();
        let receipt_number_l = receipt_number.clone();
        let items_for_wa = order_item_details.clone();
        let total = final_total_paise;
        let balance = new_balance;
        tokio::spawn(async move {
            send_order_receipt_whatsapp(&state_l, &driver_id, &receipt_number_l, &items_for_wa, total, balance).await;
        });
    }

    // ── Step M: Print thermal receipt (fire-and-forget) ──────────────────────
    {
        let state_m = state.clone();
        let receipt_number_m = receipt_number.clone();
        let items_for_print = order_item_details.clone();
        let total = final_total_paise;
        // Fetch customer name best-effort — empty string is acceptable
        let customer_name = sqlx::query_scalar::<_, String>("SELECT COALESCE(name, '') FROM drivers WHERE id = ?")
            .bind(&req.driver_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        tokio::spawn(async move {
            print_thermal_receipt(&state_m, &receipt_number_m, &items_for_print, total, &customer_name).await;
        });
    }

    // ── Step K: Return response ───────────────────────────────────────────────
    tracing::info!(
        "Cafe order placed: {} receipt={} driver={} gross={}p discount={}p final={}p promo={:?}",
        order_id,
        receipt_number,
        req.driver_id,
        total_paise,
        discount_paise,
        final_total_paise,
        promo_result.applied_promo_id
    );

    Ok(Json(serde_json::to_value(PlaceOrderResponse {
        order_id,
        receipt_number,
        wallet_txn_id,
        total_paise: final_total_paise,
        discount_paise,
        applied_promo_id: promo_result.applied_promo_id,
        applied_promo_name: promo_result.promo_name,
        new_balance_paise: new_balance,
        items: order_item_details,
    })
    .unwrap_or_default()))
}

/// Staff endpoint — driver_id is provided in request body.
pub async fn place_cafe_order(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlaceOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    place_cafe_order_inner(&state, req).await
}

/// Customer endpoint — driver_id is extracted from Authorization JWT (prevents spoofing).
pub async fn place_cafe_order_customer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(mut req): Json<PlaceOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract driver_id from JWT — ignore any driver_id in body
    let driver_id = crate::auth::verify_jwt(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or(""),
        &state.config.auth.jwt_secret,
    )
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e })),
        )
    })?;

    req.driver_id = driver_id;
    place_cafe_order_inner(&state, req).await
}

/// GET /customer/cafe/orders/history
/// Returns the authenticated customer's cafe order history as JSON.
pub async fn list_customer_orders(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let driver_id = crate::auth::verify_jwt(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or(""),
        &state.config.auth.jwt_secret,
    )
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e })),
        )
    })?;

    let rows: Vec<(String, String, String, i64, String, String)> = sqlx::query_as(
        "SELECT id, receipt_number, items, total_paise, status, created_at
         FROM cafe_orders
         WHERE driver_id = ?
         ORDER BY created_at DESC",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!(target: "cafe", "list_customer_orders DB error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to fetch orders" })),
        )
    })?;

    let orders: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, receipt_number, items_json, total_paise, status, created_at)| {
            let items: Vec<OrderItemDetail> = serde_json::from_str(&items_json)
                .unwrap_or_else(|e| {
                    tracing::warn!(target: "cafe", "Failed to parse items for order {}: {}", id, e);
                    Vec::new()
                });
            serde_json::json!({
                "id": id,
                "receipt_number": receipt_number,
                "items": items,
                "total_paise": total_paise,
                "status": status,
                "created_at": created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "orders": orders })))
}
