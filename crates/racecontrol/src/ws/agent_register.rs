use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{BillingSessionStatus, GameState, PodInfo};

use crate::activity_log::log_pod_activity;
use crate::event_archive;
use crate::game_launcher;
use crate::state::AppState;

use super::agent_auth;

// Reconnect storm throttle: prevent the same pod from re-registering within 2 seconds.
// Normal reconnects take 5-10s minimum. Rapid re-registration indicates a reconnect storm
// (e.g., all 8 pods restarted simultaneously) that can crash the server.
static REGISTER_COOLDOWN: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

const REGISTER_COOLDOWN_SECS: u64 = 2;

/// Known MAC addresses for WOL — keyed by pod ID.
fn pod_mac_address(pod_id: &str) -> Option<String> {
    match pod_id {
        "pod_1" => Some("30:56:0F:05:45:88".into()),
        "pod_2" => Some("30:56:0F:05:46:53".into()),
        "pod_3" => Some("30:56:0F:05:44:B3".into()),
        "pod_4" => Some("30:56:0F:05:45:25".into()),
        "pod_5" => Some("30:56:0F:05:44:B7".into()),
        "pod_6" => Some("30:56:0F:05:45:6E".into()),
        "pod_7" => Some("30:56:0F:05:44:B4".into()),
        "pod_8" => Some("30:56:0F:05:46:C5".into()),
        _ => None,
    }
}

/// Handle AgentMessage::Register. Returns the new registered pod_id if successful, or None to skip (throttled/rejected).
pub(crate) async fn handle_register(
    state: &Arc<AppState>,
    pod_info: &PodInfo,
    registered_pod_id: &mut Option<String>,
    jwt_rotation_pod_id: &Arc<tokio::sync::Mutex<Option<(String, u32)>>>,
    jwt_issued_for_conn: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    cmd_tx: &mpsc::Sender<CoreMessage>,
    conn_id: u64,
) -> bool {
    // Normalize pod_id to canonical form (pod_N) at registration entry
    let canonical_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|_| pod_info.id.clone());

    // Reconnect storm throttle: skip re-registration if <2s since last
    {
        let mut cooldown = REGISTER_COOLDOWN.lock().unwrap_or_else(|p| p.into_inner());
        let now = Instant::now();
        if let Some(last) = cooldown.get(&canonical_id) {
            if now.duration_since(*last).as_secs() < REGISTER_COOLDOWN_SECS {
                tracing::warn!(
                    target: "fleet-health",
                    "Register throttled for {} — {}ms since last register (reconnect storm protection)",
                    canonical_id,
                    now.duration_since(*last).as_millis()
                );
                return false; // caller should `continue`
            }
        }
        cooldown.insert(canonical_id.clone(), now);
    }

    tracing::info!("Pod {} registered (conn_id={}): {}", pod_info.number, conn_id, pod_info.name);
    *registered_pod_id = Some(canonical_id.clone());
    // Phase 306: Tell rotation task which pod this connection serves
    *jwt_rotation_pod_id.lock().await = Some((canonical_id.clone(), pod_info.number));
    log_pod_activity(state, &canonical_id, "system", "Pod Online", &format!("Pod {} connected (conn_id={})", pod_info.number, conn_id), "agent", None);
    event_archive::append_event(&state.db, "pod.online", "ws", Some(&canonical_id), serde_json::json!({
        "pod_number": pod_info.number,
        "conn_id": conn_id,
    }), &state.config.venue.venue_id);

    // WS stability tracking: record reconnect for MI diagnostic detection.
    {
        let mut fleet = state.pod_fleet_health.write().await;
        let store = fleet.entry(canonical_id.clone()).or_default();
        store.ws_reconnect_count += 1;
        store.ws_reconnect_times.push(chrono::Utc::now());
        if store.ws_reconnect_times.len() > 20 {
            store.ws_reconnect_times.remove(0);
        }
    }

    // MMA-109: Scope each lock tightly — never hold across .await
    // Lock order: agent_senders → agent_conn_ids → pods (consistent)
    {
        state.agent_senders.write().await
            .insert(canonical_id.clone(), cmd_tx.clone());
    }
    {
        state.agent_conn_ids.write().await
            .insert(canonical_id.clone(), conn_id);
    }
    {
        state.pods.write().await
            .insert(canonical_id.clone(), pod_info.clone());
    }

    // MMA-P1-FIX: Sync pod registration to SQLite
    {
        let db_result = sqlx::query(
            "INSERT INTO pods (id, number, name, ip_address, sim_type, status, last_seen, venue_id)
             VALUES (?, ?, ?, ?, 'assetto_corsa', 'online', datetime('now'), ?)
             ON CONFLICT(id) DO UPDATE SET
               ip_address = excluded.ip_address,
               status = CASE WHEN pods.status IN ('disabled', 'maintenance') THEN pods.status ELSE 'online' END,
               last_seen = datetime('now')"
        )
        .bind(&canonical_id)
        .bind(pod_info.number as i64)
        .bind(&pod_info.name)
        .bind(&pod_info.ip_address)
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;

        match db_result {
            Ok(_) => {}
            Err(ref e) => {
                let is_unique = matches!(e, sqlx::Error::Database(db_err) if db_err.code().map_or(false, |c| c == "2067"));
                if is_unique {
                    tracing::error!(
                        "Pod {} registration rejected: number {} conflicts with another pod — rolling back in-memory",
                        canonical_id, pod_info.number
                    );
                    state.pods.write().await.remove(&canonical_id);
                    state.agent_senders.write().await.remove(&canonical_id);
                    state.agent_conn_ids.write().await.remove(&canonical_id);
                    *registered_pod_id = None;
                    return false; // caller should `continue`
                } else {
                    tracing::warn!("Failed to sync pod {} registration to DB: {}", canonical_id, e);
                }
            }
        }
    }

    let _ = state
        .dashboard_tx
        .send(DashboardEvent::PodUpdate(pod_info.clone()));

    // GSTATE-02: Smart reconciliation — merge pod's actual state with server tracker
    reconcile_game_state(state, &canonical_id, pod_info).await;

    // Resync active billing session to reconnected agent
    resync_billing(state, &canonical_id, pod_info, cmd_tx).await;

    // Send current kiosk settings to newly connected agent
    if let Ok(rows) = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value FROM kiosk_settings",
    )
    .fetch_all(&state.db)
    .await
    {
        if !rows.is_empty() {
            let settings: std::collections::HashMap<String, String> =
                rows.into_iter().collect();
            let pod_settings = state.settings_for_pod(&settings, pod_info.number).await;
            let _ = cmd_tx.send(CoreMessage::wrap(CoreToAgentMessage::SettingsUpdated { settings: pod_settings })).await;
            tracing::info!("Sent initial kiosk settings to pod {}", pod_info.number);
        }
    }

    // Phase 306 WSAUTH-01/04: Issue JWT after PSK bootstrap.
    if !jwt_issued_for_conn.load(std::sync::atomic::Ordering::Relaxed) {
        agent_auth::issue_pod_jwt_to_agent(state, &canonical_id, pod_info.number, cmd_tx);
        jwt_issued_for_conn.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    // Phase 296 PUSH-02: Push stored full AgentConfig to pod on connect
    if let Err(e) = crate::config_push::push_full_config_to_pod(
        state, &canonical_id, cmd_tx,
    ).await {
        tracing::warn!("Failed to push full config to pod {} on connect: {}", canonical_id, e);
    }
    // Phase 298 PRESET-02: Push preset library to pod on connect
    if let Err(e) = crate::preset_library::push_presets_to_pod(state, &canonical_id, cmd_tx).await {
        tracing::warn!("Failed to push presets to pod {} on connect: {}", canonical_id, e);
    }

    true // success
}

/// GSTATE-02: Smart reconciliation of game state on reconnect.
async fn reconcile_game_state(state: &Arc<AppState>, canonical_id: &str, pod_info: &PodInfo) {
    let mut games = state.game_launcher.active_games.write().await;
    let pod_game_state = pod_info.game_state.unwrap_or(GameState::Idle);
    let server_state = games.get(canonical_id).map(|t| t.game_state);

    match (server_state, pod_game_state) {
        // Case 1: Both agree no game running
        (None, GameState::Idle) => {}

        // Case 2: Server has no tracker, pod reports active game — create tracker
        (None, GameState::Running | GameState::Launching | GameState::Loading | GameState::InLobby) => {
            if let Some(sim) = pod_info.current_game {
                games.insert(canonical_id.to_string(), game_launcher::GameTracker {
                    pod_id: canonical_id.to_string(),
                    sim_type: sim,
                    game_state: pod_game_state,
                    pid: None,
                    launched_at: None,
                    error_message: None,
                    launch_args: None,
                    auto_relaunch_count: 0,
                    externally_tracked: true,
                    dynamic_timeout_secs: None,
                    exit_codes: Vec::new(),
                    max_auto_relaunch: 2,
                    playable_at: None,
                    ready_delay_ms: None,
                    billing_session_id: None,
                    launch_id: uuid::Uuid::new_v4().to_string(),
                });
                tracing::info!("GSTATE-02: Created game tracker for pod {} on reconnect ({:?})", pod_info.number, pod_game_state);
            }
        }

        // Case 3: Server has tracker, pod reports active — update state from pod (source of truth)
        (Some(_server_gs), GameState::Running | GameState::Launching | GameState::Loading | GameState::InLobby) => {
            if let Some(tracker) = games.get_mut(canonical_id) {
                let old_state = tracker.game_state;
                tracker.game_state = pod_game_state;
                if old_state != pod_game_state {
                    tracing::info!("GSTATE-02: Reconciled pod {} game state: {:?} -> {:?} (pod is source of truth)",
                        pod_info.number, old_state, pod_game_state);
                }
            }
        }

        // Case 4: Server has Launching tracker, pod reports Idle
        (Some(GameState::Launching), GameState::Idle) => {
            let keep = if let Some(tracker) = games.get(canonical_id) {
                match tracker.launched_at {
                    Some(launched_at) => {
                        let elapsed = chrono::Utc::now().signed_duration_since(launched_at).num_seconds();
                        elapsed < 30
                    }
                    None => true,
                }
            } else {
                false
            };
            if keep {
                tracing::info!("GSTATE-02: Keeping recent Launching tracker for pod {} despite Idle reconnect (launch may be in-flight)",
                    pod_info.number);
            } else {
                games.remove(canonical_id);
                tracing::info!("GSTATE-02: Removed stale Launching tracker for pod {} (pod reports Idle, launch >30s old)",
                    pod_info.number);
            }
        }

        // Case 5: Server has Running/Loading/Stopping/Error tracker, pod reports Idle
        (Some(server_gs), GameState::Idle) => {
            games.remove(canonical_id);
            tracing::info!("GSTATE-02: Removed stale {:?} tracker for pod {} on reconnect (pod reports Idle)",
                server_gs, pod_info.number);
        }

        // Case 6: Pod reports Stopping or Error — update existing tracker
        (Some(_), GameState::Stopping | GameState::Error) => {
            if let Some(tracker) = games.get_mut(canonical_id) {
                tracker.game_state = pod_game_state;
                tracing::info!("GSTATE-02: Updated tracker for pod {} to {:?} on reconnect", pod_info.number, pod_game_state);
            }
        }

        // Case 7: No server tracker, pod reports Stopping/Error — create transient tracker
        (None, GameState::Stopping | GameState::Error) => {
            if let Some(sim) = pod_info.current_game {
                games.insert(canonical_id.to_string(), game_launcher::GameTracker {
                    pod_id: canonical_id.to_string(),
                    sim_type: sim,
                    game_state: pod_game_state,
                    pid: None,
                    launched_at: Some(chrono::Utc::now()),
                    error_message: None,
                    launch_args: None,
                    auto_relaunch_count: 0,
                    externally_tracked: true,
                    dynamic_timeout_secs: None,
                    exit_codes: Vec::new(),
                    max_auto_relaunch: 2,
                    playable_at: None,
                    ready_delay_ms: None,
                    billing_session_id: None,
                    launch_id: uuid::Uuid::new_v4().to_string(),
                });
                tracing::info!("GSTATE-02: Created {:?} tracker for pod {} on reconnect (transient)", pod_game_state, pod_info.number);
            }
        }
    }
}

/// Resync active billing session to reconnected agent.
async fn resync_billing(
    state: &Arc<AppState>,
    canonical_id: &str,
    pod_info: &PodInfo,
    cmd_tx: &mpsc::Sender<CoreMessage>,
) {
    let resync = {
        let timers = state.billing.active_timers.read().await;
        timers.get(canonical_id).map(|timer| (
            timer.session_id.clone(),
            timer.driver_name.clone(),
            timer.allocated_seconds,
            timer.remaining_seconds(),
        ))
    };
    if let Some((session_id, driver_name, allocated_seconds, remaining)) = resync {
        // Resume PausedDisconnect timer — pod is back online
        {
            let mut timers = state.billing.active_timers.write().await;
            if let Some(timer) = timers.get_mut(canonical_id) {
                if timer.status == rc_common::types::BillingSessionStatus::PausedDisconnect {
                    timer.status = rc_common::types::BillingSessionStatus::Active;
                    timer.offline_since = None;
                    timer.pause_seconds = 0;
                    tracing::info!(
                        "Resumed PausedDisconnect timer for session {} on pod {} — customer is back",
                        session_id, canonical_id
                    );
                }
            }
        }
        let _ = cmd_tx.send(CoreMessage::wrap(CoreToAgentMessage::BillingStarted {
            billing_session_id: session_id.clone(),
            driver_name: driver_name.clone(),
            allocated_seconds,
            session_token: Some(uuid::Uuid::new_v4().to_string()),
        })).await;
        let _ = cmd_tx.send(CoreMessage::wrap(CoreToAgentMessage::BillingTick {
            remaining_seconds: remaining,
            allocated_seconds,
            driver_name: driver_name.clone(),
            tick_seq: 0,
            elapsed_seconds: None,
            cost_paise: None,
            rate_per_min_paise: None,
            paused: None,
            minutes_to_next_tier: None,
            tier_name: None,
        })).await;
        // Restore pod state (agent Register overwrites with Idle)
        {
            let mut pods = state.pods.write().await;
            if let Some(pod) = pods.get_mut(canonical_id) {
                pod.billing_session_id = Some(session_id.clone());
                pod.current_driver = Some(driver_name.clone());
                pod.status = rc_common::types::PodStatus::InSession;
                let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
            }
        }
        tracing::info!(
            "Resynced billing session {} to pod {} ({}s remaining)",
            session_id, pod_info.number, remaining
        );
    }
}

/// Handle AgentMessage::Heartbeat.
pub(crate) async fn handle_heartbeat(
    state: &Arc<AppState>,
    pod_info: &PodInfo,
    registered_pod_id: &Option<String>,
) {
    let hb_pod_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|_| pod_info.id.clone());
    // Kimi-004: Verify heartbeat sender matches this connection's registered pod
    if let Some(expected) = registered_pod_id {
        if &hb_pod_id != expected {
            tracing::warn!("Heartbeat pod_id mismatch: conn registered as {} but sent heartbeat for {}", expected, hb_pod_id);
            return;
        }
    }
    let mut pods = state.pods.write().await;
    let updated = if let Some(existing) = pods.get_mut(&hb_pod_id) {
        existing.ip_address = pod_info.ip_address.clone();
        let now = chrono::Utc::now();
        existing.last_seen = Some(now);
        existing.driving_state = pod_info.driving_state;
        if let Some(new_gs) = pod_info.game_state {
            let accept = match (existing.game_state, new_gs) {
                (Some(GameState::Running), GameState::Idle) => false,
                (Some(GameState::Running), GameState::Launching) => false,
                (Some(GameState::Running), GameState::Loading) => false,
                _ => true,
            };
            if accept {
                existing.game_state = pod_info.game_state;
            }
        }
        existing.current_game = pod_info.current_game;
        existing.screen_blanked = pod_info.screen_blanked;
        existing.ffb_preset = pod_info.ffb_preset.clone();
        if !pod_info.installed_games.is_empty() {
            existing.installed_games = pod_info.installed_games.clone();
        }
        if existing.mac_address.is_none() {
            existing.mac_address = pod_mac_address(&hb_pod_id);
        }
        existing.clone()
    } else {
        let mut new_pod = pod_info.clone();
        new_pod.mac_address = pod_mac_address(&hb_pod_id);
        pods.insert(hb_pod_id.clone(), new_pod.clone());
        new_pod
    };
    drop(pods);
    let _ = state
        .dashboard_tx
        .send(DashboardEvent::PodUpdate(updated));

    // FSM-02: Phantom billing guard
    if let Some(reported_gs) = pod_info.game_state {
        let has_active_billing = {
            let timers = state.billing.active_timers.read().await;
            timers.get(&hb_pod_id).map_or(false, |t| {
                matches!(t.status, BillingSessionStatus::Active)
            })
        };
        if has_active_billing && reported_gs == GameState::Idle {
            let phantom_elapsed = {
                let mut phantom = state.phantom_billing_start.write().await;
                let entry = phantom.entry(hb_pod_id.clone())
                    .or_insert_with(std::time::Instant::now);
                entry.elapsed().as_secs()
            };
            if phantom_elapsed > 30 {
                tracing::error!(
                    "PHANTOM BILLING DETECTED: pod {} has billing=active but game=Idle for {}s — auto-pausing",
                    hb_pod_id, phantom_elapsed
                );
                {
                    let mut timers = state.billing.active_timers.write().await;
                    if let Some(timer) = timers.get_mut(&hb_pod_id) {
                        if timer.status == BillingSessionStatus::Active {
                            timer.status = BillingSessionStatus::PausedGamePause;
                        }
                    }
                }
                state.phantom_billing_start.write().await.remove(&hb_pod_id);
            }
        } else {
            let has_entry = state.phantom_billing_start.read().await.contains_key(&hb_pod_id);
            if has_entry {
                state.phantom_billing_start.write().await.remove(&hb_pod_id);
            }
        }
    }

    // RESIL-08: Clock drift detection
    if let Some(ref agent_ts_str) = pod_info.agent_timestamp {
        if let Ok(agent_time) = chrono::DateTime::parse_from_rfc3339(agent_ts_str) {
            let server_time = chrono::Utc::now();
            let drift_secs = (server_time - agent_time.with_timezone(&chrono::Utc)).num_seconds();
            let abs_drift = drift_secs.unsigned_abs();
            if abs_drift > 5 {
                tracing::warn!(
                    "RESIL-08: Clock drift {}s on pod {} (server - agent)",
                    drift_secs, hb_pod_id
                );
            }
            let mut fleet = state.pod_fleet_health.write().await;
            let store = fleet.entry(hb_pod_id.clone()).or_default();
            store.clock_drift_secs = Some(drift_secs);
        }
    }
}
