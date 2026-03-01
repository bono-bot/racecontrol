use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::{get, post, put, delete},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::sync::Arc;

use crate::billing;
use crate::state::AppState;
use rc_common::types::*;

pub fn api_routes() -> Router<Arc<AppState>> {
    Router::new()
        // Health
        .route("/health", get(health))
        // Pods
        .route("/pods", get(list_pods))
        .route("/pods/{id}", get(get_pod))
        // Drivers
        .route("/drivers", get(list_drivers).post(create_driver))
        .route("/drivers/{id}", get(get_driver))
        // Sessions
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{id}", get(get_session))
        // Laps
        .route("/laps", get(list_laps))
        .route("/sessions/{id}/laps", get(session_laps))
        // Leaderboard
        .route("/leaderboard/{track}", get(track_leaderboard))
        // Events
        .route("/events", get(list_events).post(create_event))
        // Bookings
        .route("/bookings", get(list_bookings).post(create_booking))
        // Pricing
        .route("/pricing", get(list_pricing_tiers).post(create_pricing_tier))
        .route("/pricing/{id}", put(update_pricing_tier).delete(delete_pricing_tier))
        // Billing
        .route("/billing/start", post(start_billing))
        .route("/billing/active", get(active_billing_sessions))
        .route("/billing/sessions", get(list_billing_sessions))
        .route("/billing/sessions/{id}", get(get_billing_session))
        .route("/billing/sessions/{id}/events", get(billing_session_events))
        .route("/billing/{id}/stop", post(stop_billing))
        .route("/billing/{id}/pause", post(pause_billing))
        .route("/billing/{id}/resume", post(resume_billing))
        .route("/billing/{id}/extend", post(extend_billing))
        .route("/billing/report/daily", get(daily_billing_report))
        // Venue info
        .route("/venue", get(venue_info))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "service": "racecontrol",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

async fn venue_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "name": state.config.venue.name,
        "location": state.config.venue.location,
        "timezone": state.config.venue.timezone,
        "pods": state.config.pods.count,
    }))
}

async fn list_pods(State(state): State<Arc<AppState>>) -> Json<Value> {
    let pods = state.pods.read().await;
    let pod_list: Vec<&PodInfo> = pods.values().collect();
    Json(json!({ "pods": pod_list }))
}

async fn get_pod(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let pods = state.pods.read().await;
    match pods.get(&id) {
        Some(pod) => Json(json!({ "pod": pod })),
        None => Json(json!({ "error": "Pod not found" })),
    }
}

async fn list_drivers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms FROM drivers ORDER BY name"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(drivers) => {
            let list: Vec<Value> = drivers.iter().map(|d| json!({
                "id": d.0, "name": d.1, "email": d.2, "phone": d.3,
                "total_laps": d.4, "total_time_ms": d.5,
            })).collect();
            Json(json!({ "drivers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_driver(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown");
    let email = body.get("email").and_then(|v| v.as_str());
    let phone = body.get("phone").and_then(|v| v.as_str());
    let steam_guid = body.get("steam_guid").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO drivers (id, name, email, phone, steam_guid) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(&id)
    .bind(name)
    .bind(email)
    .bind(phone)
    .bind(steam_guid)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_driver(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    let row = sqlx::query_as::<_, (String, String, Option<String>, Option<String>, i64, i64)>(
        "SELECT id, name, email, phone, total_laps, total_time_ms FROM drivers WHERE id = ?"
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await;

    match row {
        Ok(Some(d)) => Json(json!({
            "id": d.0, "name": d.1, "email": d.2, "phone": d.3,
            "total_laps": d.4, "total_time_ms": d.5,
        })),
        Ok(None) => Json(json!({ "error": "Driver not found" })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, String, Option<String>)>(
        "SELECT id, type, sim_type, track, status, started_at FROM sessions ORDER BY created_at DESC LIMIT 50"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let list: Vec<Value> = sessions.iter().map(|s| json!({
                "id": s.0, "type": s.1, "sim_type": s.2,
                "track": s.3, "status": s.4, "started_at": s.5,
            })).collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let session_type = body.get("type").and_then(|v| v.as_str()).unwrap_or("practice");
    let sim_type = body.get("sim_type").and_then(|v| v.as_str()).unwrap_or("assetto_corsa");
    let track = body.get("track").and_then(|v| v.as_str()).unwrap_or("monza");
    let car_class = body.get("car_class").and_then(|v| v.as_str());

    let result = sqlx::query(
        "INSERT INTO sessions (id, type, sim_type, track, car_class, status) VALUES (?, ?, ?, ?, ?, 'pending')"
    )
    .bind(&id)
    .bind(session_type)
    .bind(sim_type)
    .bind(track)
    .bind(car_class)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "type": session_type, "track": track })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_session(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({ "todo": "get_session", "id": id }))
}

async fn list_laps(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, String, i64, Option<i64>, Option<i64>, Option<i64>, bool)>(
        "SELECT id, driver_id, track, car, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid FROM laps ORDER BY created_at DESC LIMIT 100"
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(laps) => {
            let list: Vec<Value> = laps.iter().map(|l| json!({
                "id": l.0, "driver_id": l.1, "track": l.2, "car": l.3,
                "lap_time_ms": l.4, "sector1_ms": l.5, "sector2_ms": l.6,
                "sector3_ms": l.7, "valid": l.8,
            })).collect();
            Json(json!({ "laps": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn session_laps(State(state): State<Arc<AppState>>, Path(id): Path<String>) -> Json<Value> {
    Json(json!({ "todo": "session_laps", "session_id": id }))
}

async fn track_leaderboard(State(state): State<Arc<AppState>>, Path(track): Path<String>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, String, i64, String)>(
        "SELECT tr.track, tr.car, d.name, tr.best_lap_ms, tr.achieved_at
         FROM track_records tr JOIN drivers d ON tr.driver_id = d.id
         WHERE tr.track = ? ORDER BY tr.best_lap_ms ASC"
    )
    .bind(&track)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(records) => {
            let list: Vec<Value> = records.iter().enumerate().map(|(i, r)| json!({
                "position": i + 1,
                "track": r.0, "car": r.1, "driver": r.2,
                "best_lap_ms": r.3, "achieved_at": r.4,
            })).collect();
            Json(json!({ "track": track, "records": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn list_events(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "events": [] }))
}

async fn create_event(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_event" }))
}

async fn list_bookings(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({ "bookings": [] }))
}

async fn create_booking(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Json<Value> {
    Json(json!({ "todo": "create_booking" }))
}

// ─── Pricing ────────────────────────────────────────────────────────────────

async fn list_pricing_tiers(State(state): State<Arc<AppState>>) -> Json<Value> {
    let rows = sqlx::query_as::<_, (String, String, i64, i64, bool, bool, i64)>(
        "SELECT id, name, duration_minutes, price_paise, is_trial, is_active, sort_order
         FROM pricing_tiers ORDER BY sort_order ASC",
    )
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(tiers) => {
            let list: Vec<Value> = tiers
                .iter()
                .map(|t| {
                    json!({
                        "id": t.0, "name": t.1, "duration_minutes": t.2,
                        "price_paise": t.3, "is_trial": t.4, "is_active": t.5,
                        "sort_order": t.6,
                    })
                })
                .collect();
            Json(json!({ "tiers": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn create_pricing_tier(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let id = uuid::Uuid::new_v4().to_string();
    let name = body.get("name").and_then(|v| v.as_str()).unwrap_or("Custom");
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64()).unwrap_or(30);
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64()).unwrap_or(0);
    let is_trial = body.get("is_trial").and_then(|v| v.as_bool()).unwrap_or(false);
    let sort_order = body.get("sort_order").and_then(|v| v.as_i64()).unwrap_or(10);

    let result = sqlx::query(
        "INSERT INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(duration_minutes)
    .bind(price_paise)
    .bind(is_trial)
    .bind(sort_order)
    .execute(&state.db)
    .await;

    match result {
        Ok(_) => Json(json!({ "id": id, "name": name })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn update_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let name = body.get("name").and_then(|v| v.as_str());
    let duration_minutes = body.get("duration_minutes").and_then(|v| v.as_i64());
    let price_paise = body.get("price_paise").and_then(|v| v.as_i64());
    let is_active = body.get("is_active").and_then(|v| v.as_bool());

    // Build dynamic update query
    let mut updates = Vec::new();
    let mut binds: Vec<String> = Vec::new();

    if let Some(n) = name {
        updates.push("name = ?");
        binds.push(n.to_string());
    }
    if let Some(d) = duration_minutes {
        updates.push("duration_minutes = ?");
        binds.push(d.to_string());
    }
    if let Some(p) = price_paise {
        updates.push("price_paise = ?");
        binds.push(p.to_string());
    }
    if let Some(a) = is_active {
        updates.push("is_active = ?");
        binds.push(if a { "1".to_string() } else { "0".to_string() });
    }

    if updates.is_empty() {
        return Json(json!({ "error": "No fields to update" }));
    }

    let query = format!("UPDATE pricing_tiers SET {} WHERE id = ?", updates.join(", "));

    let mut q = sqlx::query(&query);
    for b in &binds {
        q = q.bind(b);
    }
    q = q.bind(&id);

    match q.execute(&state.db).await {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn delete_pricing_tier(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    // Soft delete: set is_active = 0
    match sqlx::query("UPDATE pricing_tiers SET is_active = 0 WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
    {
        Ok(_) => Json(json!({ "ok": true })),
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

// ─── Billing ────────────────────────────────────────────────────────────────

async fn start_billing(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let pod_id = body.get("pod_id").and_then(|v| v.as_str()).unwrap_or("");
    let driver_id = body.get("driver_id").and_then(|v| v.as_str()).unwrap_or("");
    let pricing_tier_id = body.get("pricing_tier_id").and_then(|v| v.as_str()).unwrap_or("");
    let custom_price_paise = body.get("custom_price_paise").and_then(|v| v.as_u64()).map(|v| v as u32);
    let custom_duration_minutes = body.get("custom_duration_minutes").and_then(|v| v.as_u64()).map(|v| v as u32);

    if pod_id.is_empty() || driver_id.is_empty() || pricing_tier_id.is_empty() {
        return Json(json!({ "error": "pod_id, driver_id, and pricing_tier_id are required" }));
    }

    let cmd = rc_common::protocol::DashboardCommand::StartBilling {
        pod_id: pod_id.to_string(),
        driver_id: driver_id.to_string(),
        pricing_tier_id: pricing_tier_id.to_string(),
        custom_price_paise,
        custom_duration_minutes,
    };

    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn stop_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::EndBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn pause_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::PauseBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn resume_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<Value> {
    let cmd = rc_common::protocol::DashboardCommand::ResumeBilling {
        billing_session_id: id,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn extend_billing(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Json<Value> {
    let additional_seconds = body
        .get("additional_seconds")
        .and_then(|v| v.as_u64())
        .unwrap_or(600) as u32;

    let cmd = rc_common::protocol::DashboardCommand::ExtendBilling {
        billing_session_id: id,
        additional_seconds,
    };
    billing::handle_dashboard_command(&state, cmd).await;
    Json(json!({ "ok": true }))
}

async fn active_billing_sessions(State(state): State<Arc<AppState>>) -> Json<Value> {
    let timers = state.billing.active_timers.read().await;
    let sessions: Vec<_> = timers.values().map(|t| t.to_info()).collect();
    Json(json!({ "sessions": sessions }))
}

#[derive(Deserialize)]
struct BillingListQuery {
    date: Option<String>,
    status: Option<String>,
}

async fn list_billing_sessions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<BillingListQuery>,
) -> Json<Value> {
    let mut query = String::from(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at, bs.created_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE 1=1",
    );

    if let Some(date) = &params.date {
        query.push_str(&format!(" AND date(bs.started_at) = '{}'", date));
    }
    if let Some(status) = &params.status {
        query.push_str(&format!(" AND bs.status = '{}'", status));
    }

    query.push_str(" ORDER BY bs.created_at DESC LIMIT 100");

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>, String)>(
        &query,
    )
    .fetch_all(&state.db)
    .await;

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
                    })
                })
                .collect();
            Json(json!({ "sessions": list }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}

async fn get_billing_session(
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

async fn billing_session_events(
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

#[derive(Deserialize)]
struct DailyReportQuery {
    date: Option<String>,
}

async fn daily_billing_report(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DailyReportQuery>,
) -> Json<Value> {
    let date = params
        .date
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());

    let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String, i64, Option<String>, Option<String>)>(
        "SELECT bs.id, bs.driver_id, d.name, bs.pod_id, pt.name, bs.allocated_seconds,
                bs.driving_seconds, bs.status, COALESCE(bs.custom_price_paise, pt.price_paise),
                bs.started_at, bs.ended_at
         FROM billing_sessions bs
         JOIN drivers d ON bs.driver_id = d.id
         JOIN pricing_tiers pt ON bs.pricing_tier_id = pt.id
         WHERE date(bs.started_at) = ?
         ORDER BY bs.started_at ASC",
    )
    .bind(&date)
    .fetch_all(&state.db)
    .await;

    match rows {
        Ok(sessions) => {
            let total_sessions = sessions.len();
            let total_revenue_paise: i64 = sessions
                .iter()
                .filter(|s| s.7 != "cancelled")
                .map(|s| s.8)
                .sum();
            let total_driving_seconds: i64 = sessions.iter().map(|s| s.6).sum();

            let list: Vec<Value> = sessions
                .iter()
                .map(|s| {
                    json!({
                        "id": s.0, "driver_id": s.1, "driver_name": s.2,
                        "pod_id": s.3, "pricing_tier_name": s.4,
                        "allocated_seconds": s.5, "driving_seconds": s.6,
                        "status": s.7, "price_paise": s.8,
                        "started_at": s.9, "ended_at": s.10,
                    })
                })
                .collect();

            Json(json!({
                "date": date,
                "total_sessions": total_sessions,
                "total_revenue_paise": total_revenue_paise,
                "total_driving_seconds": total_driving_seconds,
                "sessions": list,
            }))
        }
        Err(e) => Json(json!({ "error": e.to_string() })),
    }
}
