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

// ─── Customer Passport + Badges (PWA) ────────────────────────────────────────

/// GET /customer/passport — returns driving passport with tiered track/car collections.
/// Lazy backfill: if driver has laps but no passport entries, backfills from laps table first.
pub(crate) async fn customer_passport(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // Lazy backfill: check if driver has passport entries
    let passport_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM driving_passport WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if passport_count == 0 {
        psychology::backfill_driving_passport(&state, &driver_id).await;
    }

    // Fetch all passport entries for this driver
    let entries: Vec<(String, String, Option<String>, Option<i64>, i64)> = sqlx::query_as(
        "SELECT track, car, first_driven_at, best_lap_ms, lap_count FROM driving_passport WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Build driven sets for quick lookup
    let driven_tracks: std::collections::HashSet<String> = entries.iter().map(|(t, _, _, _, _)| t.clone()).collect();
    let driven_cars: std::collections::HashSet<String> = entries.iter().map(|(_, c, _, _, _)| c.clone()).collect();

    // Build lookup maps for passport data (aggregate per track and per car)
    let mut track_data: std::collections::HashMap<String, (i64, i64, Option<String>)> = std::collections::HashMap::new();
    let mut car_data: std::collections::HashMap<String, (i64, i64, Option<String>)> = std::collections::HashMap::new();
    for (track, car, first_driven, best_lap, lap_count) in &entries {
        let te = track_data.entry(track.clone()).or_insert((0, i64::MAX, first_driven.clone()));
        te.0 += lap_count;
        if let Some(bl) = best_lap && *bl < te.1 { te.1 = *bl; }
        let ce = car_data.entry(car.clone()).or_insert((0, i64::MAX, first_driven.clone()));
        ce.0 += lap_count;
        if let Some(bl) = best_lap && *bl < ce.1 { ce.1 = *bl; }
    }

    // Get featured catalog data
    let featured_tracks = catalog::get_featured_tracks_for_passport();
    let featured_cars = catalog::get_featured_cars_for_passport();

    // Tier boundaries: Starter=0..6, Explorer=6..15, Legend=15..end
    let tier_boundaries: &[(usize, usize, &str, &str)] = &[
        (0, 6, "Starter Circuits", "Starter Garage"),
        (6, 15, "Explorer Circuits", "Explorer Garage"),
        (15, usize::MAX, "Legend Circuits", "Legend Garage"),
    ];

    // Build track tiers
    let mut track_tiers = Vec::new();
    for &(start, end, track_label, _) in tier_boundaries {
        let tier_items: Vec<Value> = featured_tracks.iter()
            .filter_map(|t| {
                let sort = t.get("sort_order")?.as_u64()? as usize;
                if sort >= start && sort < end { Some(t) } else { None }
            })
            .map(|t| {
                let tid = t.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let driven = driven_tracks.contains(tid);
                let (lap_count, best_lap, first_driven) = track_data.get(tid).cloned().unwrap_or((0, 0, None));
                json!({
                    "id": tid,
                    "name": t.get("name").and_then(|v| v.as_str()).unwrap_or(tid),
                    "category": t.get("category").and_then(|v| v.as_str()).unwrap_or(""),
                    "country": t.get("country").and_then(|v| v.as_str()).unwrap_or(""),
                    "driven": driven,
                    "lap_count": if driven { lap_count } else { 0 },
                    "best_lap_ms": if driven && best_lap < i64::MAX { Some(best_lap) } else { None::<i64> },
                    "first_driven_at": first_driven
                })
            })
            .collect();
        let driven_count = tier_items.iter().filter(|i| i.get("driven").and_then(|v| v.as_bool()).unwrap_or(false)).count();
        track_tiers.push(json!({
            "name": track_label,
            "target": tier_items.len(),
            "driven_count": driven_count,
            "items": tier_items
        }));
    }

    // Build car tiers
    let mut car_tiers = Vec::new();
    for &(start, end, _, car_label) in tier_boundaries {
        let tier_items: Vec<Value> = featured_cars.iter()
            .filter_map(|c| {
                let sort = c.get("sort_order")?.as_u64()? as usize;
                if sort >= start && sort < end { Some(c) } else { None }
            })
            .map(|c| {
                let cid = c.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let driven = driven_cars.contains(cid);
                let (lap_count, best_lap, first_driven) = car_data.get(cid).cloned().unwrap_or((0, 0, None));
                json!({
                    "id": cid,
                    "name": c.get("name").and_then(|v| v.as_str()).unwrap_or(cid),
                    "category": c.get("category").and_then(|v| v.as_str()).unwrap_or(""),
                    "driven": driven,
                    "lap_count": if driven { lap_count } else { 0 },
                    "best_lap_ms": if driven && best_lap < i64::MAX { Some(best_lap) } else { None::<i64> },
                    "first_driven_at": first_driven
                })
            })
            .collect();
        let driven_count = tier_items.iter().filter(|i| i.get("driven").and_then(|v| v.as_bool()).unwrap_or(false)).count();
        car_tiers.push(json!({
            "name": car_label,
            "target": tier_items.len(),
            "driven_count": driven_count,
            "items": tier_items
        }));
    }

    // Non-featured (other) tracks and cars
    let featured_track_ids: std::collections::HashSet<&str> = featured_tracks.iter()
        .filter_map(|t| t.get("id")?.as_str())
        .collect();
    let featured_car_ids: std::collections::HashSet<&str> = featured_cars.iter()
        .filter_map(|c| c.get("id")?.as_str())
        .collect();

    let other_tracks: Vec<Value> = entries.iter()
        .filter(|(t, _, _, _, _)| !featured_track_ids.contains(t.as_str()))
        .map(|(t, _, first_driven, best_lap, lap_count)| {
            let display_name = catalog::id_to_display_name(t);
            json!({
                "id": t,
                "name": display_name,
                "driven": true,
                "lap_count": lap_count,
                "best_lap_ms": best_lap,
                "first_driven_at": first_driven
            })
        })
        .collect();

    let other_cars: Vec<Value> = entries.iter()
        .filter(|(_, c, _, _, _)| !featured_car_ids.contains(c.as_str()))
        .map(|(_, c, first_driven, best_lap, lap_count)| {
            let display_name = catalog::id_to_display_name(c);
            json!({
                "id": c,
                "name": display_name,
                "driven": true,
                "lap_count": lap_count,
                "best_lap_ms": best_lap,
                "first_driven_at": first_driven
            })
        })
        .collect();

    // Summary stats
    let total_laps: i64 = entries.iter().map(|(_, _, _, _, lc)| lc).sum();
    let streak_data: Option<(i64, i64, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT current_streak, longest_streak, last_visit_date, grace_expires_date
         FROM streaks WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (streak_weeks, longest_streak, last_visit_date, grace_expires_date) = streak_data
        .unwrap_or((0, 0, None, None));

    Json(json!({
        "passport": {
            "tracks": {
                "total_driven": driven_tracks.len(),
                "total_available": featured_tracks.len(),
                "tiers": {
                    "starter": track_tiers.first(),
                    "explorer": track_tiers.get(1),
                    "legend": track_tiers.get(2)
                },
                "other": other_tracks
            },
            "cars": {
                "total_driven": driven_cars.len(),
                "total_available": featured_cars.len(),
                "tiers": {
                    "starter": car_tiers.first(),
                    "explorer": car_tiers.get(1),
                    "legend": car_tiers.get(2)
                },
                "other": other_cars
            },
            "summary": {
                "unique_tracks": driven_tracks.len(),
                "unique_cars": driven_cars.len(),
                "total_laps": total_laps,
                "streak_weeks": streak_weeks,
                "longest_streak": longest_streak,
                "last_visit_date": last_visit_date,
                "grace_expires_date": grace_expires_date
            }
        }
    }))
}

/// GET /customer/badges — returns earned + available badges for the authenticated customer.
/// Earned badges include earned_at timestamp. Available (not yet earned) badges include
/// progress toward the target metric.
pub(crate) async fn customer_badges(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Json<Value> {
    let driver_id = match extract_driver_id(&state, &headers) {
        Ok(id) => id,
        Err(e) => return Json(json!({ "error": e })),
    };

    // All active badge definitions — column is badge_icon, NOT icon
    let all_badges: Vec<(String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, name, description, category, badge_icon, criteria_json FROM achievements WHERE is_active = 1 ORDER BY sort_order"
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    // Earned badges for this driver — table is driver_achievements, column is achievement_id
    let earned_map: std::collections::HashMap<String, String> = sqlx::query_as::<_, (String, String)>(
        "SELECT achievement_id, earned_at FROM driver_achievements WHERE driver_id = ?"
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .collect();

    // Driver metrics for progress calculation
    let total_laps: i64 = sqlx::query_scalar("SELECT COALESCE(total_laps, 0) FROM drivers WHERE id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let unique_tracks: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT track) FROM driving_passport WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let unique_cars: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT car) FROM driving_passport WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let pb_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM personal_bests WHERE driver_id = ?")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);
    let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM billing_sessions WHERE driver_id = ? AND status = 'completed'")
        .bind(&driver_id).fetch_one(&state.db).await.unwrap_or(0);

    let mut earned_list = Vec::new();
    let mut available_list = Vec::new();

    for (badge_id, name, description, category, badge_icon, criteria_json) in &all_badges {
        if let Some(earned_at) = earned_map.get(badge_id) {
            earned_list.push(json!({
                "id": badge_id,
                "name": name,
                "description": description,
                "category": category,
                "icon": badge_icon,
                "earned_at": earned_at,
                "earned": true
            }));
        } else {
            let (progress, target) = parse_badge_progress(criteria_json, total_laps, unique_tracks, unique_cars, pb_count, session_count);
            available_list.push(json!({
                "id": badge_id,
                "name": name,
                "description": description,
                "category": category,
                "icon": badge_icon,
                "progress": progress,
                "target": target,
                "earned": false
            }));
        }
    }

    let total_available = all_badges.len();
    Json(json!({
        "badges": {
            "earned": earned_list,
            "available": available_list,
            "total_earned": earned_list.len(),
            "total_available": total_available
        }
    }))
}

/// Parse badge criteria JSON to extract progress/target for display.
/// Returns (current_progress, target_value).
/// IMPORTANT: criteria_json keys are "type" and "value" (NOT "metric"/"threshold").
/// Example: {"type":"total_laps","operator":">=","value":100}
pub(crate) fn parse_badge_progress(criteria_json: &str, total_laps: i64, unique_tracks: i64, unique_cars: i64, pb_count: i64, session_count: i64) -> (i64, i64) {
    let parsed: Result<Value, _> = serde_json::from_str(criteria_json);
    match parsed {
        Ok(criteria) => {
            let metric = criteria.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let threshold = criteria.get("value").and_then(|v| v.as_i64()).unwrap_or(1);
            let progress = match metric {
                "total_laps" => total_laps,
                "unique_tracks" => unique_tracks,
                "unique_cars" => unique_cars,
                "pb_count" => pb_count,
                "session_count" => session_count,
                "first_lap" => if total_laps > 0 { 1 } else { 0 },
                "streak_weeks" => 0, // streak handled separately, not in simple metrics
                _ => 0,
            };
            (progress.min(threshold), threshold)
        }
        Err(_) => (0, 1),
    }
}
