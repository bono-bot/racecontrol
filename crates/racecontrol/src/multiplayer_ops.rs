//! Multiplayer helpers — AC LAN setup, group session management, kiosk booking.
//!
//! Internal helpers for group session lifecycle (AC LAN start, response tracking,
//! session info building, driver lookup), stale invite cleanup, and kiosk self-service
//! multiplayer booking flow.
//!
//! Extracted from multiplayer.rs (Phase 385, v49.0 Architecture Completion).

#[path = "multiplayer_ops_kiosk.rs"]
mod kiosk;
pub use kiosk::{
    book_multiplayer_kiosk, cleanup_stale_invites, KioskMultiplayerAssignment,
    KioskMultiplayerResult,
};

use std::sync::Arc;

use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{GroupMemberInfo, GroupSessionInfo};

// ─── Internal Helpers ──────────────────────────────────────────────────────

/// Auto-start AC LAN session when all group members are validated.
pub(crate) async fn start_ac_lan_for_group(
    state: &Arc<AppState>,
    group_session_id: &str,
) -> Result<(), String> {
    // Get group session details
    let session = sqlx::query_as::<_, (String,)>(
        "SELECT experience_id FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let experience_id = session.0;

    // Get experience details
    let exp = sqlx::query_as::<_, (String, String, String)>(
        "SELECT game, track, car FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Experience not found")?;

    let (game, track, car) = exp;

    // Get all validated member pod_ids
    let members: Vec<(String, String)> = sqlx::query_as(
        "SELECT driver_id, pod_id FROM group_session_members
         WHERE group_session_id = ? AND status = 'validated' AND pod_id IS NOT NULL",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let pod_ids: Vec<String> = members.iter().map(|(_, pid)| pid.clone()).collect();

    if game == "assetto_corsa" || game == "ac" {
        let human_count = members.len();

        // Query track pit count from any pod's content manifest for AI filler calculation.
        // Default to 24 if not available (reasonable for most tracks).
        let max_pits: usize = {
            let manifests = state.pod_manifests.read().await;
            manifests.values()
                .find_map(|m| {
                    m.tracks.iter()
                        .find(|t| t.id == track)
                        .and_then(|t| t.configs.first())
                        .and_then(|c| c.pit_count)
                        .map(|p| p as usize)
                })
                .unwrap_or(24)
        };

        // Calculate AI filler count: fill remaining pits, cap at 19 (AC 20-slot limit)
        let ai_count = max_pits.saturating_sub(human_count).min(19);

        // Query difficulty tier from experience for AI_LEVEL mapping
        let difficulty_tier: Option<String> = sqlx::query_scalar(
            "SELECT difficulty_tier FROM kiosk_experiences WHERE id = ?",
        )
        .bind(&experience_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        // Map difficulty tier to AI_LEVEL (Phase 2 midpoints)
        let ai_level = match difficulty_tier.as_deref() {
            Some("rookie") => 75,
            Some("amateur") => 82,
            Some("semi_pro") => 87,
            Some("pro") => 93,
            Some("alien") => 98,
            _ => 87, // Default to SemiPro midpoint
        };

        // Build human entry slots
        let mut entry_slots = Vec::new();
        for (_i, (driver_id, pod_id)) in members.iter().enumerate() {
            let (dname, dguid) = get_driver_entry_info(state, driver_id).await;
            entry_slots.push(rc_common::types::AcEntrySlot {
                car_model: car.clone(),
                skin: String::new(),
                driver_name: dname,
                guid: dguid,
                ballast: 0,
                restrictor: 0,
                pod_id: Some(pod_id.clone()),
                ai_mode: None,
            });
        }

        // Add AI fillers (same car as players, AI=fixed for AssettoServer)
        if ai_count > 0 {
            let ai_names = rc_common::ai_names::pick_ai_names(ai_count);
            for name in ai_names {
                entry_slots.push(rc_common::types::AcEntrySlot {
                    car_model: car.clone(),
                    skin: String::new(),
                    driver_name: name,
                    guid: String::new(),
                    ballast: 0,
                    restrictor: 0,
                    pod_id: None,
                    ai_mode: Some("fixed".to_string()),
                });
            }
            tracing::info!(
                "Added {} AI fillers (AI_LEVEL={}) for group session {}",
                ai_count, ai_level, group_session_id
            );
        }

        // Build AC LAN config
        let config = rc_common::types::AcLanSessionConfig {
            name: format!("Multiplayer - Group {}", &group_session_id[..8]),
            track: track.clone(),
            track_config: String::new(),
            cars: vec![car.clone()],
            max_clients: (human_count + ai_count) as u32,
            password: String::new(),
            sessions: vec![rc_common::types::AcSessionBlock {
                name: "Race".to_string(),
                session_type: rc_common::types::SessionType::Race,
                duration_minutes: 0,
                laps: 10,
                wait_time_secs: 10,
            }],
            entries: entry_slots,
            weather: vec![rc_common::types::AcWeatherConfig {
                graphics: "3_clear".to_string(),
                base_temperature_ambient: 26,
                base_temperature_road: 32,
                variation_ambient: 2,
                variation_road: 2,
                wind_base_speed_min: 0,
                wind_base_speed_max: 10,
                wind_base_direction: 0,
                wind_variation_direction: 15,
            }],
            dynamic_track: rc_common::types::AcDynamicTrackConfig {
                session_start: 90,
                randomness: 2,
                session_transfer: 90,
                lap_gain: 30,
            },
            pickup_mode: true,
            udp_port: 0,  // Dynamically assigned by PortAllocator in start_ac_server()
            tcp_port: 0,  // Dynamically assigned by PortAllocator in start_ac_server()
            http_port: 0, // Dynamically assigned by PortAllocator in start_ac_server()
            min_csp_version: 0,
            ..Default::default()
        };

        match crate::ac_server::start_ac_server(state, config, pod_ids.clone(), Some(ai_level)).await {
            Ok(ac_session_id) => {
                // Store AC session ID + track/car/ai_count for lobby enrichment
                sqlx::query("UPDATE group_sessions SET ac_session_id = ?, track = ?, car = ?, ai_count = ? WHERE id = ?")
                    .bind(&ac_session_id)
                    .bind(&track)
                    .bind(&car)
                    .bind(ai_count as i64)
                    .execute(&state.db)
                    .await
                    .map_err(|e| format!("DB error: {}", e))?;

                // Broadcast
                let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionAllValidated {
                    group_session_id: group_session_id.to_string(),
                    ac_session_id: ac_session_id.clone(),
                    pod_ids: pod_ids.clone(),
                });

                tracing::info!(
                    "AC LAN started for group session {}: ac_session={}",
                    group_session_id,
                    ac_session_id
                );
            }
            Err(e) => {
                tracing::error!("Failed to start AC LAN for group {}: {}", group_session_id, e);
                return Err(e.to_string());
            }
        }
    } else {
        // Non-AC games: launch game on each pod individually
        let sim_type = match game.as_str() {
            "iracing" => rc_common::types::SimType::IRacing,
            "f1_25" | "f1" => rc_common::types::SimType::F125,
            "le_mans_ultimate" | "lmu" => rc_common::types::SimType::LeMansUltimate,
            "forza" => rc_common::types::SimType::Forza,
            _ => rc_common::types::SimType::AssettoCorsa,
        };

        let agent_senders = state.agent_senders.read().await;
        for (driver_id, pod_id) in &members {
            if let Some(sender) = agent_senders.get(pod_id) {
                let driver_name = get_driver_name(state, driver_id).await;
                let launch_args = serde_json::json!({
                    "car": car, "track": track, "driver": driver_name
                })
                .to_string();

                let _ = sender
                    .send(CoreMessage::wrap(CoreToAgentMessage::LaunchGame {
                        sim_type: sim_type.clone(),
                        launch_args: Some(launch_args),
                        force_clean: false,
                        duration_minutes: None,
                        launch_id: None,
                    }))
                    .await;
            }
        }
        drop(agent_senders);

        // Broadcast
        let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionAllValidated {
            group_session_id: group_session_id.to_string(),
            ac_session_id: String::new(),
            pod_ids,
        });

        tracing::info!(
            "Games launched for group session {} ({})",
            group_session_id,
            game
        );
    }

    Ok(())
}

/// Check if all invitees have responded (accepted or declined). If so, set status to 'ready'.
pub(crate) async fn check_all_responded(state: &Arc<AppState>, group_session_id: &str) {
    let pending: Option<(i64,)> = sqlx::query_as(
        "SELECT COUNT(*) FROM group_session_members
         WHERE group_session_id = ? AND status = 'pending'",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if pending.map(|r| r.0).unwrap_or(1) == 0 {
        let _ = sqlx::query(
            "UPDATE group_sessions SET status = 'ready' WHERE id = ? AND status = 'forming'",
        )
        .bind(group_session_id)
        .execute(&state.db)
        .await;
    }
}

/// Build full GroupSessionInfo from DB.
pub(crate) async fn build_group_session_info(
    state: &Arc<AppState>,
    group_session_id: &str,
) -> Result<GroupSessionInfo, String> {
    let session = sqlx::query_as::<_, (String, String, String, String, String, String)>(
        "SELECT gs.id, gs.host_driver_id, gs.experience_id, gs.shared_pin, gs.status, gs.created_at
         FROM group_sessions gs WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Group session not found")?;

    let (id, host_driver_id, experience_id, shared_pin, status, created_at) = session;

    let host_name = get_driver_name(state, &host_driver_id).await;

    let experience_name: String = sqlx::query_scalar(
        "SELECT name FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Get members
    let member_rows: Vec<(String, String, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT gsm.driver_id, gsm.role, gsm.pod_id, gsm.status, d.name
         FROM group_session_members gsm
         INNER JOIN drivers d ON d.id = gsm.driver_id
         WHERE gsm.group_session_id = ?
         ORDER BY gsm.role DESC, gsm.invited_at",
    )
    .bind(group_session_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    let mut members = Vec::new();
    for (driver_id, role, pod_id, member_status, name) in member_rows {
        let customer_id = get_customer_id(state, &driver_id).await;
        let pod_number = if let Some(ref pid) = pod_id {
            get_pod_number(state, pid).await
        } else {
            None
        };

        members.push(GroupMemberInfo {
            driver_id,
            driver_name: name.unwrap_or_else(|| "Unknown".to_string()),
            customer_id,
            role,
            status: member_status,
            pod_id,
            pod_number,
        });
    }

    let pricing_tier_name: String = sqlx::query_scalar(
        "SELECT pt.name FROM pricing_tiers pt
         INNER JOIN group_sessions gs ON gs.pricing_tier_id = pt.id
         WHERE gs.id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "Unknown".to_string());

    // Query enrichment fields from group_sessions (track, car, ai_count added in Phase 9)
    let enrichment: Option<(Option<String>, Option<String>, Option<i64>)> = sqlx::query_as(
        "SELECT track, car, ai_count FROM group_sessions WHERE id = ?",
    )
    .bind(group_session_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    let (track, car, ai_count) = enrichment.unwrap_or((None, None, None));

    // Query difficulty_tier from experience (if available)
    let difficulty_tier: Option<String> = sqlx::query_scalar(
        "SELECT difficulty_tier FROM kiosk_experiences WHERE id = ?",
    )
    .bind(&experience_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    Ok(GroupSessionInfo {
        id,
        host_driver_id,
        host_name,
        experience_name,
        pricing_tier_name,
        shared_pin,
        status,
        members,
        created_at,
        track,
        car,
        ai_count: ai_count.map(|c| c as u32),
        difficulty_tier,
    })
}

pub(crate) async fn get_driver_name(state: &Arc<AppState>, driver_id: &str) -> String {
    sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "Unknown".to_string())
}

pub(crate) async fn get_customer_id(state: &Arc<AppState>, driver_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT customer_id FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten()
}

pub(crate) async fn get_pod_number(state: &Arc<AppState>, pod_id: &str) -> Option<u32> {
    let pods = state.pods.read().await;
    pods.get(pod_id).map(|p| p.number)
}

pub(crate) fn canonical_pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

/// Get driver name and steam_guid for AC entry list population.
pub(crate) async fn get_driver_entry_info(state: &Arc<AppState>, driver_id: &str) -> (String, String) {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT name, steam_guid FROM drivers WHERE id = ?",
    )
    .bind(driver_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    match row {
        Some((name, guid)) => (
            name.unwrap_or_else(|| "Driver".to_string()),
            guid.unwrap_or_default(),
        ),
        None => ("Driver".to_string(), String::new()),
    }
}

