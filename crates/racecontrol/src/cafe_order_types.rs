//! Cafe order types and stock rollback helper.
//!
//! Extracted from cafe_orders.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

// ─── Order Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub driver_id: String,
    pub items: Vec<OrderItemRequest>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderItemRequest {
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    pub receipt_number: String,
    pub wallet_txn_id: String,
    pub total_paise: i64,
    pub discount_paise: i64,
    pub applied_promo_id: Option<String>,
    pub applied_promo_name: Option<String>,
    pub new_balance_paise: i64,
    pub items: Vec<OrderItemDetail>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderItemDetail {
    pub item_id: String,
    pub name: String,
    pub quantity: i64,
    pub unit_price_paise: i64,
    pub line_total_paise: i64,
}

/// Internal verified item during order processing (held between transaction and wallet debit).
#[derive(Clone)]
pub(crate) struct VerifiedOrderItem {
    pub(crate) item_id: String,
    pub(crate) name: String,
    pub(crate) quantity: i64,
    pub(crate) unit_price_paise: i64,
    pub(crate) is_countable: bool,
}

// ─── Order Helpers ───────────────────────────────────────────────────────────

/// Rollback stock for countable items with per-item tracking and 3x retry.
/// MMA iter2 fix: tracks which items succeeded to prevent double-rollback.
pub(crate) async fn rollback_stock(state: &Arc<AppState>, items: &[VerifiedOrderItem], context: &str) {
    use std::collections::HashSet;
    let mut done: HashSet<String> = HashSet::new();
    for attempt in 0..3u8 {
        let mut any_failed = false;
        for item in items {
            if !item.is_countable || done.contains(&item.item_id) {
                continue;
            }
            match sqlx::query(
                "UPDATE cafe_items SET stock_quantity = stock_quantity + ? WHERE id = ?",
            )
            .bind(item.quantity)
            .bind(&item.item_id)
            .execute(&state.db)
            .await
            {
                Ok(_) => { done.insert(item.item_id.clone()); }
                Err(e) => {
                    tracing::error!("rollback_stock: item {} failed (attempt {}/3, ctx={}): {}", item.item_id, attempt + 1, context, e);
                    any_failed = true;
                }
            }
        }
        if !any_failed { break; }
        if attempt < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1))).await;
        }
    }
}
