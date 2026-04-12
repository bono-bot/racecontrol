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

// Pod Debug System
// ═══════════════════════════════════════════════════════════════════════════════

// ─── Pod Activity Log ────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct ActivityQuery {
    limit: Option<i64>,
}

pub(crate) async fn global_activity(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ActivityQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(100).min(500);
    let rows: Vec<(String, String, i64, String, String, String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, pod_id, pod_number, timestamp, category, action, details, source, entry_hash, previous_hash
         FROM pod_activity_log ORDER BY timestamp DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.0, "pod_id": r.1, "pod_number": r.2, "timestamp": r.3,
        "category": r.4, "action": r.5, "details": r.6, "source": r.7,
        "entry_hash": r.8, "previous_hash": r.9,
    })).collect();

    Json(json!(entries))
}

pub(crate) async fn pod_activity(
    State(state): State<Arc<AppState>>,
    Path(pod_id): Path<String>,
    Query(q): Query<ActivityQuery>,
) -> Json<Value> {
    let limit = q.limit.unwrap_or(100).min(500);
    let rows: Vec<(String, String, i64, String, String, String, String, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, pod_id, pod_number, timestamp, category, action, details, source, entry_hash, previous_hash
         FROM pod_activity_log WHERE pod_id = ? ORDER BY timestamp DESC LIMIT ?"
    )
    .bind(&pod_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|r| json!({
        "id": r.0, "pod_id": r.1, "pod_number": r.2, "timestamp": r.3,
        "category": r.4, "action": r.5, "details": r.6, "source": r.7,
        "entry_hash": r.8, "previous_hash": r.9,
    })).collect();

    Json(json!(entries))
}

// ─── Phase 307: Audit chain verify ────────────────────────────────────────

/// GET /api/v1/audit/verify — Walk the hash chain and report integrity status.
///
/// Returns:
/// - chain_valid: true if no tampered entries found
/// - total_entries: count of hashed entries in chain
/// - verified_entries: entries whose hash matched
/// - tampered_entries: entries whose hash did not match
/// - tampered_at: ID of first tampered entry (null if chain is valid)
pub(crate) async fn audit_verify(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Fetch all hashed entries in chronological order
    let rows: Vec<(String, String, String, String, String, String, String, String, String)> =
        sqlx::query_as(
            "SELECT id, timestamp, category, action, details, source, entry_hash, previous_hash, pod_id
             FROM pod_activity_log
             WHERE entry_hash IS NOT NULL AND previous_hash IS NOT NULL
             ORDER BY timestamp ASC",
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    let total_entries = rows.len();
    let mut verified_entries = 0usize;
    let mut tampered_entries = 0usize;
    let mut tampered_at: Option<String> = None;
    let mut first_genesis: Option<String> = None;
    let mut latest_hash = String::new();

    for row in &rows {
        let (id, timestamp, category, action, details, source, entry_hash, previous_hash, _pod_id) =
            (row.0.as_str(), row.1.as_str(), row.2.as_str(), row.3.as_str(),
             row.4.as_str(), row.5.as_str(), row.6.as_str(), row.7.as_str(), row.8.as_str());

        if first_genesis.is_none() {
            first_genesis = Some(timestamp.to_string());
        }

        let computed = crate::activity_log::compute_activity_hash(
            id, timestamp, category, action, details, source, previous_hash,
        );

        if computed == entry_hash {
            verified_entries += 1;
        } else {
            tampered_entries += 1;
            if tampered_at.is_none() {
                tampered_at = Some(id.to_string());
                tracing::warn!(
                    "AUDIT-02: Tamper detected in pod_activity_log entry {} (timestamp={})",
                    id, timestamp
                );
            }
        }
        latest_hash = entry_hash.to_string();
    }

    let chain_valid = tampered_entries == 0;

    Json(json!({
        "chain_valid": chain_valid,
        "total_entries": total_entries,
        "verified_entries": verified_entries,
        "tampered_entries": tampered_entries,
        "first_genesis": first_genesis,
        "latest_hash": if latest_hash.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(latest_hash) },
        "tampered_at": tampered_at,
    }))
}

// ─── Server Logs ─────────────────────────────────────────────────────────

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct LogsQuery {
    lines: Option<usize>,
    level: Option<String>,
}

// GET /logs — Tail the racecontrol log file
pub(crate) async fn get_server_logs(Query(q): Query<LogsQuery>) -> Json<Value> {
    let max_lines = q.lines.unwrap_or(200).min(2000);
    let level_filter = q.level.as_deref().unwrap_or("");

    // Find the most recent log file in ./logs/
    let log_dir = std::path::Path::new("logs");
    let log_file = match std::fs::read_dir(log_dir) {
        Ok(entries) => {
            let mut files: Vec<_> = entries
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with("racecontrol"))
                        .unwrap_or(false)
                })
                .collect();
            files.sort_by_key(|e| std::cmp::Reverse(e.metadata().and_then(|m| m.modified()).ok()));
            files.first().map(|e| e.path())
        }
        Err(_) => None,
    };

    let path = match log_file {
        Some(p) => p,
        None => return Json(json!({ "lines": [], "file": null, "total": 0 })),
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => return Json(json!({ "error": format!("Failed to read log: {}", e) })),
    };

    let all_lines: Vec<&str> = content.lines().collect();

    // Filter by level if requested
    let filtered: Vec<&str> = if level_filter.is_empty() {
        all_lines.clone()
    } else {
        let upper = level_filter.to_uppercase();
        all_lines
            .iter()
            .filter(|line| line.to_uppercase().contains(&upper))
            .copied()
            .collect()
    };

    // Take last N lines
    let start = filtered.len().saturating_sub(max_lines);
    let tail: Vec<&str> = filtered[start..].to_vec();

    Json(json!({
        "lines": tail,
        "file": path.file_name().and_then(|n| n.to_str()),
        "total": all_lines.len(),
        "filtered": filtered.len(),
    }))
}
