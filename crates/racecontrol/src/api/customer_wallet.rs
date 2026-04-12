#![allow(unused_imports)]
use super::customer_auth::extract_driver_id;
use rand::Rng;
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::ac_server;
use crate::accounting;
use crate::fleet_alert;
use crate::recovery;
use crate::cafe;
use crate::config_push;
use crate::flags;
use crate::policy_engine;
use crate::preset_library;
use crate::cafe_alerts;
use crate::cafe_marketing;
use crate::cafe_promos;
use crate::auth;
use crate::whatsapp_alerter;
use crate::psychology;
use crate::auth::middleware::{require_staff_jwt, require_role_manager, require_role_superadmin};
use crate::network_source::require_non_pod_source;
use crate::billing;
use crate::catalog;
use crate::cloud_sync;
use crate::fleet_health;
use crate::fleet_intelligence;
use crate::process_guard;
use crate::friends;
use crate::game_launcher;
use crate::multiplayer;
use crate::pod_reservation;
use crate::reservation;
use crate::scheduler;
use crate::wallet;
use crate::weekend;
use crate::maintenance_store;
use crate::state::{AppState, VenueConfigSnapshot};
use crate::venue_shutdown;
use crate::wol;
use rc_common::pod_id::normalize_pod_id;
use rc_common::types::*;
use rc_common::protocol::{CloudAction, CoreMessage, CoreToAgentMessage, DashboardEvent};

// ─── Customer Wallet ────────────────────────────────────────────────────────

pub(crate) async fn customer_wallet(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    match wallet::get_wallet_info(&state, &driver_id).await {
        Ok(Some(info)) => Json(json!({ "wallet": info })),
        Ok(None) => Json(json!({ "wallet": {
            "driver_id": driver_id,
            "balance_credits": 0,
            "total_credited": 0,
            "total_spent": 0,
            "rupee_deposited": 0,
            "rupee_refunded": 0,
            "bonus_credited": 0,
            "max_cash_refund": 0,
            "transactions_count": 0,
            "updated_at": null,
        } })),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn customer_wallet_transactions(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    let limit = params.get("limit").and_then(|l| l.parse().ok()).unwrap_or(50i64);
    let offset = params.get("offset").and_then(|o| o.parse().ok()).unwrap_or(0i64);

    let total: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM wallet_transactions WHERE driver_id = ?",
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .map(|r| r.0)
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, driver_id, amount_paise, balance_after_paise, txn_type, reference_id, notes, staff_id, created_at
         FROM wallet_transactions WHERE driver_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
    )
    .bind(&driver_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let txns: Vec<Value> = rows.iter().map(|r| {
        json!({
            "id": r.0, "driver_id": r.1, "amount_paise": r.2,
            "balance_after_paise": r.3, "txn_type": r.4,
            "reference_id": r.5, "notes": r.6, "staff_id": r.7,
            "created_at": r.8,
        })
    }).collect();

    Json(json!({ "transactions": txns, "total": total }))
}

// ─── Customer Experiences ───────────────────────────────────────────────────

pub(crate) async fn customer_experiences(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>, i64, String, i64)>(
        "SELECT e.id, e.name, e.game, e.track, e.car, e.car_class, e.duration_minutes, e.start_type, e.sort_order
         FROM kiosk_experiences e WHERE e.is_active = 1 ORDER BY e.sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    // Also fetch pricing tiers for the client
    let tiers = sqlx::query_as::<_, (String, String, i64, i64, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, sort_order
         FROM pricing_tiers WHERE is_active = 1 ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match (rows, tiers) {
        (Ok(experiences), Ok(tiers)) => {
            let exp_list: Vec<Value> = experiences.iter().map(|e| json!({
                "id": e.0, "name": e.1, "game": e.2, "track": e.3,
                "car": e.4, "car_class": e.5, "duration_minutes": e.6,
                "start_type": e.7, "sort_order": e.8,
            })).collect();

            let tier_list: Vec<Value> = tiers.iter().map(|t| json!({
                "id": t.0, "name": t.1, "duration_minutes": t.2,
                "price_paise": t.3, "is_trial": t.4, "sort_order": t.5,
            })).collect();

            Json(json!({ "experiences": exp_list, "pricing_tiers": tier_list }))
        }
        _ => Json(json!({ "error": "Failed to load experiences" })),
    }
}

// ─── AC Catalog ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct CatalogQuery {
    pod_id: Option<String>,
}

pub(crate) async fn customer_ac_catalog(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CatalogQuery>,
) -> Json<Value> {
    let manifest = if let Some(ref pod_id) = query.pod_id {
        state.pod_manifests.read().await.get(pod_id).cloned()
    } else {
        None
    };
    Json(catalog::get_filtered_catalog(manifest.as_ref()))
}
