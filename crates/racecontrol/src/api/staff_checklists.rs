//! Digital Staff Operations — Checklists API (Phase 391).
//!
//! Replaces paper checklists with a tracked digital audit trail.
//! Morning opening and evening closing checklists enforced and tracked.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::state::AppState;

// ─── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: String,
    pub label: String,
    pub category: String,
}

#[derive(Debug, Deserialize)]
pub struct StartChecklistRequest {
    pub staff_id: String,
    pub staff_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CompleteItemRequest {
    pub item_id: String,
    pub notes: Option<String>,
}

// ─── GET /staff/checklists — list all active checklists ─────────────────────

pub(crate) async fn list_checklists(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let rows: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, checklist_type, items_json FROM staff_checklists WHERE is_active = 1 ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let checklists: Vec<Value> = rows.iter().map(|(id, name, ctype, items_json)| {
        let items: Value = serde_json::from_str(items_json).unwrap_or(json!([]));
        json!({
            "id": id,
            "name": name,
            "type": ctype,
            "items": items,
            "item_count": items.as_array().map(|a| a.len()).unwrap_or(0),
        })
    }).collect();

    Json(json!({ "checklists": checklists }))
}

// ─── POST /staff/checklists/{id}/start — begin a checklist session ──────────

pub(crate) async fn start_checklist(
    State(state): State<Arc<AppState>>,
    Path(checklist_id): Path<String>,
    Json(req): Json<StartChecklistRequest>,
) -> Json<Value> {
    // Get checklist template
    let checklist: Option<(String, String)> = sqlx::query_as(
        "SELECT items_json, name FROM staff_checklists WHERE id = ? AND is_active = 1"
    )
    .bind(&checklist_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let Some((items_json, name)) = checklist else {
        return Json(json!({"error": "Checklist not found or inactive"}));
    };

    let items: Value = serde_json::from_str(&items_json).unwrap_or(json!([]));
    let total = items.as_array().map(|a| a.len()).unwrap_or(0) as i32;

    // Check if already started today
    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM staff_checklist_completions
         WHERE checklist_id = ? AND staff_id = ? AND date = date('now') AND completed_at IS NULL"
    )
    .bind(&checklist_id)
    .bind(&req.staff_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    if let Some((existing_id,)) = existing {
        return Json(json!({
            "ok": true,
            "completion_id": existing_id,
            "message": "Resuming existing checklist session",
            "checklist_name": name,
        }));
    }

    let result = sqlx::query(
        "INSERT INTO staff_checklist_completions (checklist_id, staff_id, staff_name, total_items, date)
         VALUES (?, ?, ?, ?, date('now'))"
    )
    .bind(&checklist_id)
    .bind(&req.staff_id)
    .bind(&req.staff_name)
    .bind(total)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) => Json(json!({
            "ok": true,
            "completion_id": r.last_insert_rowid(),
            "checklist_name": name,
            "total_items": total,
        })),
        Err(e) => {
            tracing::error!("Failed to start checklist: {}", e);
            Json(json!({"error": "Failed to start checklist"}))
        }
    }
}

// ─── POST /staff/checklists/completions/{id}/check — mark item done ─────────

pub(crate) async fn check_item(
    State(state): State<Arc<AppState>>,
    Path(completion_id): Path<i64>,
    Json(req): Json<CompleteItemRequest>,
) -> Json<Value> {
    // Get current completed items
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT completed_items_json FROM staff_checklist_completions WHERE id = ? AND completed_at IS NULL"
    )
    .bind(completion_id)
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let Some((current_json,)) = row else {
        return Json(json!({"error": "Completion not found or already finished"}));
    };

    let mut items: Vec<Value> = serde_json::from_str(&current_json).unwrap_or_default();

    // Add the checked item (idempotent — skip if already present)
    let already_checked = items.iter().any(|i| i.get("id").and_then(|v| v.as_str()) == Some(&req.item_id));
    if !already_checked {
        items.push(json!({
            "id": req.item_id,
            "checked_at": chrono::Utc::now().to_rfc3339(),
            "notes": req.notes,
        }));
    }

    let new_json = serde_json::to_string(&items).unwrap_or_default();
    let count = items.len() as i32;

    let _ = sqlx::query(
        "UPDATE staff_checklist_completions SET completed_items_json = ?, completed_count = ? WHERE id = ?"
    )
    .bind(&new_json)
    .bind(count)
    .bind(completion_id)
    .execute(&state.db)
    .await;

    Json(json!({ "ok": true, "completed_count": count, "item_id": req.item_id }))
}

// ─── POST /staff/checklists/completions/{id}/finish — finalize checklist ────

pub(crate) async fn finish_checklist(
    State(state): State<Arc<AppState>>,
    Path(completion_id): Path<i64>,
) -> Json<Value> {
    let result = sqlx::query(
        "UPDATE staff_checklist_completions SET completed_at = datetime('now') WHERE id = ? AND completed_at IS NULL"
    )
    .bind(completion_id)
    .execute(&state.db)
    .await;

    match result {
        Ok(r) if r.rows_affected() > 0 => Json(json!({ "ok": true, "completion_id": completion_id, "finished": true })),
        Ok(_) => Json(json!({"error": "Already finished or not found"})),
        Err(e) => {
            tracing::error!("Failed to finish checklist: {}", e);
            Json(json!({"error": "Failed to finish checklist"}))
        }
    }
}

// ─── GET /staff/checklists/compliance — admin compliance view ───────────────

pub(crate) async fn checklist_compliance(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // Last 7 days of compliance data
    let rows: Vec<(String, String, String, Option<String>, i32, i32, Option<String>)> = sqlx::query_as(
        "SELECT sc.date, sc.staff_id, COALESCE(sc.staff_name, sc.staff_id), cl.name,
                sc.total_items, sc.completed_count, sc.completed_at
         FROM staff_checklist_completions sc
         JOIN staff_checklists cl ON cl.id = sc.checklist_id
         WHERE sc.date >= date('now', '-7 days')
         ORDER BY sc.date DESC, cl.name"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let entries: Vec<Value> = rows.iter().map(|(date, staff_id, staff_name, checklist_name, total, completed, completed_at)| {
        json!({
            "date": date,
            "staff_id": staff_id,
            "staff_name": staff_name,
            "checklist": checklist_name,
            "total_items": total,
            "completed_items": completed,
            "completion_pct": if *total > 0 { (*completed as f64 / *total as f64 * 100.0).round() } else { 0.0 },
            "finished": completed_at.is_some(),
            "finished_at": completed_at,
        })
    }).collect();

    // Count gaps: days where opening/closing wasn't completed
    let gap_count: Option<(i32,)> = sqlx::query_as(
        "SELECT COUNT(DISTINCT date) FROM (
            SELECT date('now', '-' || n || ' days') as d, n FROM (
                SELECT 0 as n UNION SELECT 1 UNION SELECT 2 UNION SELECT 3
                UNION SELECT 4 UNION SELECT 5 UNION SELECT 6
            )
         ) dates
         LEFT JOIN staff_checklist_completions sc ON sc.date = dates.d AND sc.completed_at IS NOT NULL
         WHERE sc.id IS NULL"
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    Json(json!({
        "entries": entries,
        "days_with_gaps": gap_count.map(|(c,)| c).unwrap_or(7),
        "period": "last_7_days",
    }))
}
