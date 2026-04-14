//! Multiplayer helpers — AC LAN setup, group session management, kiosk booking.
//!
//! Internal helpers for group session lifecycle (AC LAN start, response tracking,
//! session info building, driver lookup), stale invite cleanup, and kiosk self-service
//! multiplayer booking flow.
//!
//! Extracted from multiplayer.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;

use crate::auth;
use crate::multiplayer::find_adjacent_idle_pods;
use crate::pod_reservation;
use crate::state::AppState;
use crate::wallet;
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

/// Cleanup stale group session invites.
/// Cancels group_sessions with status 'forming' that are older than 5 minutes.
/// Pending members get status 'timeout', pods are released.
/// If no accepted members remain, the session is cancelled.
pub async fn cleanup_stale_invites(state: &Arc<AppState>) {
    // Find stale forming sessions (older than 5 minutes)
    let stale_sessions: Vec<(String,)> = match sqlx::query_as(
        "SELECT id FROM group_sessions
         WHERE status = 'forming'
           AND created_at < datetime('now', '-5 minutes')",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!("[cleanup_stale_invites] DB error: {}", e);
            return;
        }
    };

    if stale_sessions.is_empty() {
        return;
    }

    tracing::info!(
        "[cleanup_stale_invites] Found {} stale forming sessions",
        stale_sessions.len()
    );

    for (session_id,) in &stale_sessions {
        // Timeout pending members
        let _ = sqlx::query(
            "UPDATE group_session_members SET status = 'timeout'
             WHERE group_session_id = ? AND status = 'pending'",
        )
        .bind(session_id)
        .execute(&state.db)
        .await;

        // Check if any members have accepted/validated status
        let accepted_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM group_session_members
             WHERE group_session_id = ? AND status IN ('accepted', 'validated')",
        )
        .bind(session_id)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);

        if accepted_count == 0 {
            // No accepted members — cancel the session
            let _ = sqlx::query(
                "UPDATE group_sessions SET status = 'cancelled' WHERE id = ?",
            )
            .bind(session_id)
            .execute(&state.db)
            .await;

            tracing::info!(
                "[cleanup_stale_invites] Cancelled stale session {} (no accepted members)",
                session_id
            );
        } else {
            tracing::info!(
                "[cleanup_stale_invites] Session {} has {} accepted members — keeping active",
                session_id,
                accepted_count
            );
        }

        // Release pods for timed-out members by clearing pod_id
        let _ = sqlx::query(
            "UPDATE group_session_members SET pod_id = NULL
             WHERE group_session_id = ? AND status = 'timeout'",
        )
        .bind(session_id)
        .execute(&state.db)
        .await;
    }
}

/// Result returned from kiosk self-service multiplayer booking.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KioskMultiplayerResult {
    pub group_session_id: String,
    pub experience_name: String,
    pub tier_name: String,
    pub allocated_seconds: u32,
    pub assignments: Vec<KioskMultiplayerAssignment>,
}

/// Per-pod assignment in a kiosk multiplayer booking.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KioskMultiplayerAssignment {
    pub pin: String,
    pub pod_id: String,
    pub pod_number: u32,
    pub role: String,
}

/// Kiosk self-service multiplayer booking.
/// Unlike book_multiplayer(), this doesn't require friends to be pre-registered.
/// Host pays for all pods. Each participant gets a unique PIN.
///
/// Returns a list of (pin, pod_number) pairs — one per participant.
pub async fn book_multiplayer_kiosk(
    state: &Arc<AppState>,
    host_id: &str,
    pricing_tier_id: &str,
    pod_count: usize,
    experience_id: Option<&str>,
    custom: Option<(String, String, String)>,
) -> Result<KioskMultiplayerResult, String> {
    if pod_count < 2 || pod_count > 8 {
        return Err("Pod count must be between 2 and 8".to_string());
    }

    // Verify pricing tier
    let tier = sqlx::query_as::<_, (String, i64, i64)>(
        "SELECT name, price_paise, duration_minutes FROM pricing_tiers WHERE id = ? AND is_active = 1",
    )
    .bind(pricing_tier_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?
    .ok_or("Pricing tier not found")?;

    let (tier_name, price_per_pod_paise, duration_minutes) = tier;
    let total_price = price_per_pod_paise * pod_count as i64;

    // Resolve experience
    let (experience_id_resolved, experience_name) = if let Some(eid) = experience_id {
        let exp = sqlx::query_as::<_, (String,)>(
            "SELECT name FROM kiosk_experiences WHERE id = ?",
        )
        .bind(eid)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?
        .ok_or("Experience not found")?;
        (eid.to_string(), exp.0)
    } else if let Some((ref game, ref track, ref car)) = custom {
        let adhoc_id = uuid::Uuid::new_v4().to_string();
        let adhoc_name = format!("Custom: {} @ {}", car, track);
        sqlx::query(
            "INSERT INTO kiosk_experiences (id, name, game, track, car, duration_minutes, start_type, sort_order, is_active, created_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, 'race', 9999, 0, datetime('now'), ?)",
        )
        .bind(&adhoc_id)
        .bind(&adhoc_name)
        .bind(game)
        .bind(track)
        .bind(car)
        .bind(duration_minutes)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;
        (adhoc_id, adhoc_name)
    } else {
        return Err("Must provide experience_id or custom booking payload".to_string());
    };

    // Validate host wallet
    wallet::ensure_wallet(state, host_id).await?;
    let host_balance = wallet::get_balance(state, host_id).await?;
    if host_balance < total_price {
        return Err(format!(
            "Insufficient wallet balance: have {}p, need {}p ({}p x {} pods)",
            host_balance, total_price, price_per_pod_paise, pod_count
        ));
    }

    // Find adjacent idle pods
    let pod_ids = find_adjacent_idle_pods(state, pod_count).await?;

    // Debit host wallet for all pods
    wallet::ensure_wallet(state, host_id).await?;
    let (_, wallet_txn_id) = wallet::debit(
        state,
        host_id,
        total_price,
        "debit_session",
        None,
        Some(&format!("Kiosk multiplayer: {} x {} pods", experience_name, pod_count)),
    )
    .await?;

    // Ensure ac_session_id column exists
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN ac_session_id TEXT")
        .execute(&state.db)
        .await;

    // Create group session
    let group_session_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO group_sessions (id, host_driver_id, experience_id, pricing_tier_id, shared_pin, status, total_members, created_at, venue_id)
         VALUES (?, ?, ?, ?, ?, 'active', ?, datetime('now'), ?)",
    )
    .bind(&group_session_id)
    .bind(host_id)
    .bind(&experience_id_resolved)
    .bind(pricing_tier_id)
    .bind("0000") // Placeholder — each participant gets unique PIN
    .bind(pod_count as i64)
    .bind(&state.config.venue.venue_id)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error: {}", e))?;

    // Create auth token + reservation for each pod (each gets unique PIN)
    let mut assignments: Vec<KioskMultiplayerAssignment> = Vec::new();
    let host_name = get_driver_name(state, host_id).await;

    for (i, pod_id) in pod_ids.iter().enumerate() {
        let role = if i == 0 { "host" } else { "invitee" };

        // Create reservation
        let reservation_id = pod_reservation::create_reservation(state, host_id, pod_id).await?;

        // Create auth token (generates unique PIN)
        let token = auth::create_auth_token(
            state,
            pod_id.clone(),
            host_id.to_string(),
            pricing_tier_id.to_string(),
            "pin".to_string(),
            None,
            Some(duration_minutes as u32),
            Some(experience_id_resolved.clone()),
            None,
        )
        .await?;

        // The token field IS the PIN for pin-type auth tokens
        let pin = token.token.clone();

        // Get pod number
        let pod_number = {
            let pods = state.pods.read().await;
            pods.get(pod_id).map(|p| p.number).unwrap_or(0)
        };

        // Create group member record
        let member_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO group_session_members (id, group_session_id, driver_id, role, status, pod_id, reservation_id, auth_token_id, wallet_txn_id, invited_at, accepted_at, venue_id)
             VALUES (?, ?, ?, ?, 'accepted', ?, ?, ?, ?, datetime('now'), datetime('now'), ?)",
        )
        .bind(&member_id)
        .bind(&group_session_id)
        .bind(host_id)
        .bind(role)
        .bind(pod_id)
        .bind(&reservation_id)
        .bind(&token.id)
        .bind(&wallet_txn_id)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await
        .map_err(|e| format!("DB error: {}", e))?;

        // Send lock screen to pod
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender
                .send(CoreMessage::wrap(CoreToAgentMessage::ShowPinLockScreen {
                    token_id: token.id.clone(),
                    driver_name: host_name.clone(),
                    pricing_tier_name: tier_name.clone(),
                    allocated_seconds: duration_minutes as u32 * 60,
                }))
                .await;
        }

        assignments.push(KioskMultiplayerAssignment {
            pin,
            pod_id: pod_id.clone(),
            pod_number,
            role: role.to_string(),
        });
    }

    // GROUP-01: AC server start deferred to on_member_validated() -> start_ac_lan_for_group()
    // Server starts only when ALL members have validated their PINs (coordinated launch).
    // The ac_session_id will be set on group_sessions by start_ac_lan_for_group().

    // Broadcast to dashboard
    if let Ok(info) = build_group_session_info(state, &group_session_id).await {
        let _ = state.dashboard_tx.send(DashboardEvent::GroupSessionCreated(info));
    }

    tracing::info!(
        "Kiosk multiplayer group {} created: {} pods, host {}",
        group_session_id, pod_count, host_id
    );

    Ok(KioskMultiplayerResult {
        group_session_id,
        experience_name,
        tier_name,
        allocated_seconds: duration_minutes as u32 * 60,
        assignments,
    })
}
