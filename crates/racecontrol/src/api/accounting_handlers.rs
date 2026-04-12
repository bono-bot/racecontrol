#![allow(unused_imports)]
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

// ─── Accounting & Audit Routes ─────────────────────────────────────────────

pub(crate) async fn list_accounts(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, i64, String, String, Option<String>, Option<String>, bool)>(
        "SELECT id, code, name, account_type, parent_id, description, is_active
         FROM accounts ORDER BY code",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(accts) => {
            let list: Vec<Value> = accts.iter().map(|a| json!({
                "id": a.0, "code": a.1, "name": a.2, "account_type": a.3,
                "parent_id": a.4, "description": a.5, "is_active": a.6,
            })).collect();
            Json(json!({ "accounts": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct DateRangeQuery {
    from: Option<String>,
    to: Option<String>,
}

pub(crate) async fn trial_balance(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    match accounting::get_trial_balance(&state, params.from.as_deref(), params.to.as_deref()).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn profit_loss(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    match accounting::get_profit_loss(&state, params.from.as_deref(), params.to.as_deref()).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn balance_sheet(State(state): State<Arc<AppState>>) -> Json<Value> {
    match accounting::get_balance_sheet(&state).await {
        Ok(data) => Json(data),
        Err(e) => Json(json!({ "error": e })),
    }
}

pub(crate) async fn list_journal_entries(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DateRangeQuery>,
) -> Json<Value> {
    let limit = 100i64; // default

    let mut query = String::from(
        "SELECT je.id, je.date, je.description, je.reference_type, je.reference_id, je.staff_id, je.created_at
         FROM journal_entries je WHERE 1=1"
    );

    if params.from.is_some() {
        query.push_str(" AND je.date >= ?");
    }
    if params.to.is_some() {
        query.push_str(" AND je.date <= ?");
    }
    query.push_str(" ORDER BY je.created_at DESC LIMIT ?");

    let mut q = sqlx::query_as::<_, (String, String, String, Option<String>, Option<String>, Option<String>, String)>(&query);
    if let Some(ref d) = params.from {
        q = q.bind(d);
    }
    if let Some(ref d) = params.to {
        q = q.bind(d);
    }
    q = q.bind(limit);

    let entries = match q.fetch_all(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let mut result = Vec::new();
    for entry in &entries {
        // Fetch lines for this entry
        let lines = sqlx::query_as::<_, (String, String, i64, i64)>(
            "SELECT jel.account_id, a.name, jel.debit_paise, jel.credit_paise
             FROM journal_entry_lines jel
             JOIN accounts a ON jel.account_id = a.id
             WHERE jel.journal_entry_id = ?
             ORDER BY jel.debit_paise DESC",
        )
        .bind(&entry.0)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let line_json: Vec<Value> = lines.iter().map(|l| json!({
            "account_id": l.0,
            "account_name": l.1,
            "debit_paise": l.2,
            "credit_paise": l.3,
        })).collect();

        result.push(json!({
            "id": entry.0,
            "date": entry.1,
            "description": entry.2,
            "reference_type": entry.3,
            "reference_id": entry.4,
            "staff_id": entry.5,
            "created_at": entry.6,
            "lines": line_json,
        }));
    }

    Json(json!({ "entries": result, "count": result.len() }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct AuditLogQuery {
    table_name: Option<String>,
    row_id: Option<String>,
    action: Option<String>,
    staff_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    limit: Option<i64>,
}

pub(crate) async fn query_audit_log(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AuditLogQuery>,
) -> Json<Value> {
    let limit = params.limit.unwrap_or(100).min(500);

    let mut query = String::from(
        "SELECT id, table_name, row_id, action, old_values, new_values, staff_id, ip_address, created_at
         FROM audit_log WHERE 1=1"
    );
    let mut binds: Vec<String> = Vec::new();

    if let Some(ref t) = params.table_name {
        query.push_str(" AND table_name = ?");
        binds.push(t.clone());
    }
    if let Some(ref r) = params.row_id {
        query.push_str(" AND row_id = ?");
        binds.push(r.clone());
    }
    if let Some(ref a) = params.action {
        query.push_str(" AND action = ?");
        binds.push(a.clone());
    }
    if let Some(ref s) = params.staff_id {
        query.push_str(" AND staff_id = ?");
        binds.push(s.clone());
    }
    if let Some(ref d) = params.from {
        query.push_str(" AND created_at >= ?");
        binds.push(d.clone());
    }
    if let Some(ref d) = params.to {
        query.push_str(" AND created_at <= ?");
        binds.push(d.clone());
    }

    query.push_str(" ORDER BY created_at DESC LIMIT ?");
    binds.push(limit.to_string());

    let mut q = sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>, Option<String>, Option<String>, String)>(&query);
    for b in &binds {
        q = q.bind(b);
    }

    match q.fetch_all(&state.db).await {
        Ok(rows) => {
            let entries: Vec<Value> = rows.iter().map(|r| json!({
                "id": r.0,
                "table_name": r.1,
                "row_id": r.2,
                "action": r.3,
                "old_values": r.4.as_ref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
                "new_values": r.5.as_ref().and_then(|s| serde_json::from_str::<Value>(s).ok()),
                "staff_id": r.6,
                "ip_address": r.7,
                "created_at": r.8,
            })).collect();
            Json(json!({ "entries": entries, "count": entries.len() }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
