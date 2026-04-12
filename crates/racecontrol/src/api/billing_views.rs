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

pub(crate) async fn track_review_click(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> axum::response::Redirect {
    // Log the click
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, metadata, venue_id) \
         VALUES (?, ?, 'review_click', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&driver_id) // using driver_id as reference
    .bind(format!("{{\"driver_id\":\"{}\",\"type\":\"google_review\"}}", driver_id))
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    tracing::info!("Review link clicked by driver {}", driver_id);

    // TODO: Send WhatsApp to staff: "Driver X clicked review link — verify and approve"
    // Redirect to Google Maps review page (place ID to be configured)
    axum::response::Redirect::temporary("https://g.page/r/racingpoint/review")
}

/// Act 3: Track follow link click — redirects to Instagram, logs click.
pub(crate) async fn track_follow_click(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
) -> axum::response::Redirect {
    let _ = sqlx::query(
        "INSERT INTO billing_events (id, billing_session_id, event_type, metadata, venue_id) \
         VALUES (?, ?, 'follow_click', ?, ?)",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(&driver_id)
    .bind(format!("{{\"driver_id\":\"{}\",\"type\":\"instagram_follow\"}}", driver_id))
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await;

    tracing::info!("Follow link clicked by driver {}", driver_id);
    axum::response::Redirect::temporary("https://instagram.com/racingpoint.esports")
}

/// Act 3: Staff approves review/follow bonus — credits wallet.
pub(crate) async fn approve_incentive_bonus(
    State(state): State<Arc<AppState>>,
    Path(driver_id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let bonus_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let (amount_paise, flag_column) = match bonus_type {
        "review" => (5000i64, "review_bonus_claimed"),  // ₹50
        "follow" => (2500i64, "follow_bonus_claimed"),  // ₹25
        _ => return Json(json!({ "error": "type must be 'review' or 'follow'" })),
    };

    // Check if already claimed
    let already_claimed: Option<(bool,)> = sqlx::query_as(
        &format!("SELECT COALESCE({}, 0) FROM drivers WHERE id = ?", flag_column),
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((true,)) = already_claimed {
        return Json(json!({ "error": format!("{} bonus already claimed", bonus_type) }));
    }

    // Resolve wallet owner (linked racers → parent)
    let wallet_owner = crate::wallet::resolve_wallet_owner(&state, &driver_id)
        .await
        .unwrap_or_else(|_| driver_id.clone());

    // Credit wallet
    match crate::wallet::credit_wallet(
        &state.db,
        &wallet_owner,
        amount_paise,
        &format!("{}_bonus", bonus_type),
        Some(&driver_id),
        Some(&format!("{} incentive bonus", bonus_type)),
        &state.config.venue.venue_id,
    ).await {
        Ok(_) => {
            // Set flag
            let _ = sqlx::query(
                &format!("UPDATE drivers SET {} = 1 WHERE id = ?", flag_column),
            )
            .bind(&driver_id)
            .execute(&state.db)
            .await;

            tracing::info!("Incentive bonus approved: {} for driver {} ({}p)", bonus_type, driver_id, amount_paise);
            Json(json!({ "ok": true, "bonus_credits": amount_paise / 100 }))
        }
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// Act 3: Staff-accessible session receipt (for POS printing).
pub(crate) async fn staff_session_receipt(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Json<Value> {
    let session = sqlx::query_as::<_, (String, String, i64, i64, String, Option<String>, Option<String>, Option<i64>, i64, Option<String>)>(
        "SELECT d.name, bs.pricing_tier_id, bs.driving_seconds, COALESCE(bs.wallet_debit_paise, 0), bs.status, \
         bs.started_at, bs.ended_at, bs.refund_paise, bs.allocated_seconds, \
         COALESCE(bs.billing_mode, 'package') \
         FROM billing_sessions bs JOIN drivers d ON bs.driver_id = d.id WHERE bs.id = ?",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (driver_name, tier_id, driving_secs, debit, status, started_at, ended_at, refund_opt, allocated, billing_mode) = session;
    let refund = refund_opt.unwrap_or(0);
    let duration_min = driving_secs / 60;
    let cost_credits = debit / 100;
    let refund_credits = refund / 100;

    // Tier name lookup
    let tier_name: String = sqlx::query_scalar("SELECT name FROM pricing_tiers WHERE id = ?")
        .bind(&tier_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| tier_id.clone());

    // Wallet balance
    let balance: i64 = sqlx::query_scalar(
        "SELECT COALESCE(balance_paise, 0) FROM wallets WHERE driver_id = (SELECT driver_id FROM billing_sessions WHERE id = ?)",
    )
    .bind(&session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0);

    Json(json!({
        "session_id": session_id,
        "driver_name": driver_name,
        "tier_name": tier_name,
        "billing_mode": billing_mode,
        "started_at": started_at,
        "ended_at": ended_at,
        "duration_minutes": duration_min,
        "duration_seconds": driving_secs,
        "cost_credits": cost_credits,
        "cost_paise": debit,
        "refund_credits": refund_credits,
        "refund_paise": refund,
        "wallet_balance_credits": balance / 100,
        "wallet_balance_paise": balance,
        "status": status,
        "allocated_seconds": allocated,
    }))
}

/// Act 3: Visit-level receipt (cumulative — all sessions + cafe for one visit).
pub(crate) async fn visit_receipt(
    State(state): State<Arc<AppState>>,
    Path(visit_id): Path<String>,
) -> Json<Value> {
    // Visit info
    let visit = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT v.id, d.name, v.started_at, v.ended_at, v.total_sessions, v.total_spent_paise \
         FROM visits v JOIN drivers d ON v.driver_id = d.id WHERE v.id = ?",
    )
    .bind(&visit_id)
    .fetch_optional(&state.db)
    .await;

    let visit = match visit {
        Ok(Some(v)) => v,
        Ok(None) => return Json(json!({ "error": "Visit not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    let (_vid, driver_name, started, ended, total_sessions, total_spent) = visit;

    // All sessions in this visit
    let sessions: Vec<(String, String, i64, i64, String)> = sqlx::query_as(
        "SELECT bs.id, pt.name, bs.driving_seconds, COALESCE(bs.wallet_debit_paise, 0), bs.status \
         FROM billing_sessions bs \
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id \
         WHERE bs.visit_id = ? ORDER BY bs.started_at",
    )
    .bind(&visit_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let session_list: Vec<Value> = sessions.iter().map(|(id, tier, secs, cost, status)| {
        json!({
            "session_id": id,
            "tier_name": tier,
            "duration_minutes": secs / 60,
            "cost_credits": cost / 100,
            "status": status,
        })
    }).collect();

    // Wallet balance
    let balance: i64 = sqlx::query_scalar(
        "SELECT COALESCE(w.balance_paise, 0) FROM wallets w \
         JOIN visits v ON w.driver_id = v.driver_id WHERE v.id = ?",
    )
    .bind(&visit_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0);

    Json(json!({
        "visit_id": visit_id,
        "driver_name": driver_name,
        "started_at": started,
        "ended_at": ended,
        "total_sessions": total_sessions,
        "total_spent_credits": total_spent / 100,
        "total_spent_paise": total_spent,
        "wallet_balance_credits": balance / 100,
        "sessions": session_list,
    }))
}

/// Act 2: Upgrade a package billing session to a higher tier (e.g. 30min → 60min).
pub(crate) async fn upgrade_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let new_tier_id = body.get("new_tier_id").and_then(|v| v.as_str()).unwrap_or("");
    if new_tier_id.is_empty() {
        return Json(json!({ "ok": false, "error": "new_tier_id is required" }));
    }
    match billing::upgrade_billing_tier(&state, &id, new_tier_id).await {
        Ok(()) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

/// Act 3: End a customer visit — closes visit, calculates totals, triggers receipt.
pub(crate) async fn end_visit(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let end_method = body.get("end_method").and_then(|v| v.as_str()).unwrap_or("staff");
    match crate::visits::end_visit(&state, &id, end_method).await {
        Ok(summary) => Json(json!({
            "ok": true,
            "visit_id": summary.visit_id,
            "total_sessions": summary.total_sessions,
            "total_spent_paise": summary.total_spent_paise,
            "wallet_balance_paise": summary.wallet_balance_paise,
            "end_method": summary.end_method,
        })),
        Err(e) => Json(json!({ "ok": false, "error": e })),
    }
}

pub(crate) async fn active_billing_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rate_tiers = state.billing.rate_tiers.read().await;
    let timers = state.billing.active_timers.read().await;
    let sessions: Vec<_> = timers.values().map(|t| t.to_info(&rate_tiers)).collect();
    Json(json!({ "sessions": sessions }))
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub(crate) struct BillingListQuery {
    date: Option<String>,
    status: Option<String>,
}

pub(crate) async fn list_billing_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BillingListQuery>,
) -> Json<Value> {
    let mut query = String::from(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.created_at, bs.wallet_debit_paise, bs.refund_paise, bs.discount_paise
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE 1=1",
    );

    // Build parameterized query to prevent SQL injection
    let mut bind_values: Vec<String> = Vec::new();
    if let Some(date) = &params.date {
        // Validate date format (YYYY-MM-DD only)
        if date.len() == 10 && date.chars().all(|c| c.is_ascii_digit() || c == '-') {
            query.push_str(" AND date(bs.started_at) = ?");
            bind_values.push(date.clone());
        }
    }
    if let Some(status) = &params.status {
        // Validate status is alphanumeric + underscores only
        if status.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            query.push_str(" AND bs.status = ?");
            bind_values.push(status.clone());
        }
    }

    query.push_str(" ORDER BY bs.created_at DESC LIMIT 100");

    let mut q = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, String, Option<i64>, Option<i64>, Option<i64>)>(
        &query,
    );
    for val in &bind_values {
        q = q.bind(val);
    }
    let rows = q.fetch_all(&state.db).await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10, "created_at": s.11,
                        "wallet_debit_paise": s.12, "refund_paise": s.13, "discount_paise": s.14,
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn get_billing_session(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(s)) => Json(json!({
            "id": s.0, "driver_id": s.1, "driver_name": s.2,
            "pod_id": s.3, "pricing_tier_name": s.4,
            "allocated_seconds": s.5, "driving_seconds": s.6,
            "status": s.7, "price_paise": s.8,
            "started_at": s.9, "ended_at": s.10,
        })),
        Ok(None) => Json(json!({ "error": "Billing session not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn billing_session_events(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, Option<String>, String)>(
        "SELECT id, event_type, driving_seconds_at_event, metadata, created_at
         FROM billing_events WHERE billing_session_id = ? ORDER BY created_at ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(events) => {
            let list: Vec<Value> = events
                .iter()
                .map(|e| {
                    json!({
                        "id": e.0, "event_type": e.1,
                        "driving_seconds_at_event": e.2,
                        "metadata": e.3, "created_at": e.4,
                    })
                })
                .collect();
            Json(json!({ "events": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

pub(crate) async fn billing_session_summary(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Get billing session info (including discount fields)
    let session = sqlx::query_as::<_, (String, String, String, String, i64, i64, String, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, bs.allocated_seconds, bs.driving_seconds, bs.status, bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         WHERE bs.id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    let session = match session {
        Ok(Some(s)) => s,
        Ok(None) => return Json(json!({ "error": "Session not found" })),
        Err(e) => return Json(json!({ "error": e.to_string() })),
    };

    // Get discount info
    let discount_info = sqlx::query_as::<_, (Option<i64>, Option<String>, Option<i64>, Option<String>)>(
        "SELECT discount_paise, coupon_id, original_price_paise, discount_reason FROM billing_sessions WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    // Get laps for this session
    let laps = sqlx::query_as::<_, (String, i64, i64, Option<i64>, Option<i64>, Option<i64>, bool, String, String)>(
        "SELECT id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, track, car
         FROM laps WHERE session_id = ? ORDER BY lap_number ASC",
    )
    .bind(&id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let total_laps = laps.len() as u32;
    let valid_laps: Vec<_> = laps.iter().filter(|l| l.6).collect();
    let best_lap_ms = valid_laps.iter().map(|l| l.2).min();
    let avg_lap_ms = if !valid_laps.is_empty() {
        Some(valid_laps.iter().map(|l| l.2).sum::<i64>() / valid_laps.len() as i64)
    } else {
        None
    };

    // Check personal best
    let track = laps.first().map(|l| l.7.as_str()).unwrap_or("");
    let car = laps.first().map(|l| l.8.as_str()).unwrap_or("");

    let pb = sqlx::query_as::<_, (i64,)>(
        "SELECT best_lap_ms FROM personal_bests WHERE driver_id = ? AND track = ? AND car = ?",
    )
    .bind(&session.1)
    .bind(track)
    .bind(car)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let personal_best_broken = best_lap_ms.map(|b| pb.map(|p| b <= p.0).unwrap_or(true)).unwrap_or(false);

    // Check leaderboard position
    let leaderboard_position = if !track.is_empty() && !car.is_empty() {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) + 1 FROM personal_bests WHERE track = ? AND car = ? AND best_lap_ms < ?",
        )
        .bind(track)
        .bind(car)
        .bind(best_lap_ms.unwrap_or(i64::MAX))
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
    } else {
        None
    };

    let laps_json: Vec<Value> = laps
        .iter()
        .map(|l| {
            json!({
                "lap_number": l.1,
                "lap_time_ms": l.2,
                "sector1_ms": l.3,
                "sector2_ms": l.4,
                "sector3_ms": l.5,
                "valid": l.6,
            })
        })
        .collect();

    Json(json!({
        "summary": {
            "billing_session_id": session.0,
            "driver_id": session.1,
            "driver_name": session.2,
            "pod_id": session.3,
            "track": track,
            "car": car,
            "allocated_seconds": session.4,
            "driving_seconds": session.5,
            "status": session.6,
            "total_laps": total_laps,
            "best_lap_ms": best_lap_ms,
            "average_lap_ms": avg_lap_ms,
            "personal_best_broken": personal_best_broken,
            "leaderboard_position": leaderboard_position,
            "laps": laps_json,
            "discount_paise": discount_info.as_ref().and_then(|d| d.0),
            "coupon_id": discount_info.as_ref().and_then(|d| d.1.clone()),
            "original_price_paise": discount_info.as_ref().and_then(|d| d.2),
            "discount_reason": discount_info.as_ref().and_then(|d| d.3.clone()),
        }
    }))
}
