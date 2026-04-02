use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, MissedTickBehavior};

// Phase 206 (OBS-04): Rate-limit sentinel WhatsApp alerts to 1 per sentinel type per pod per 5 min.
// Key format: "sentinel_{file}_{pod_number}". Prevents alert storms during restart storms.
static SENTINEL_ALERT_COOLDOWN: std::sync::LazyLock<Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// Check and update sentinel alert cooldown. Returns true if the alert should fire.
/// Cooldown key: `sentinel_{file}_{pod_number}`. Cooldown period: 300s (5 minutes).
fn check_sentinel_cooldown(key: &str) -> bool {
    const COOLDOWN_SECS: u64 = 300;
    let mut map = SENTINEL_ALERT_COOLDOWN.lock().unwrap_or_else(|p| p.into_inner());
    let now = Instant::now();
    if let Some(last) = map.get(key) {
        if now.duration_since(*last).as_secs() < COOLDOWN_SECS {
            return false;
        }
    }
    map.insert(key.to_string(), now);
    true
}

use crate::ac_camera;
use crate::ac_server;
use crate::activity_log::log_pod_activity;
use crate::auth;
use crate::billing;
use crate::event_archive;
use crate::game_launcher;
use crate::state::{AppState, CachedAssistState};

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
use rc_common::pod_id::normalize_pod_id;
use rc_common::protocol::{
    AgentMessage, AiChannelMessage, CoreMessage, CoreToAgentMessage, DashboardCommand, DashboardEvent,
};
use rc_common::types::{BillingSessionStatus, GameState};
use sqlx;

/// Query parameters for WS authentication
#[derive(serde::Deserialize, Default)]
pub struct WsAuthParams {
    /// PSK bootstrap token — must match config.cloud.terminal_secret
    #[serde(default)]
    token: Option<String>,
    /// Per-pod JWT token — issued by server after first PSK auth (Phase 306)
    #[serde(default)]
    jwt: Option<String>,
}

/// WS authentication result for the agent endpoint (Phase 306).
enum AgentAuthResult {
    PskAuthenticated,
    JwtAuthenticated { pod_id: String, pod_number: u32 },
}

/// Validate WebSocket token against terminal_secret (if configured).
/// Returns true if: no secret configured (dev mode), or token matches.
fn verify_ws_token(state: &AppState, token: &Option<String>) -> bool {
    match &state.config.cloud.terminal_secret {
        None => true, // dev mode — no auth required
        Some(secret) if secret.is_empty() => true,
        Some(secret) => token.as_deref() == Some(secret.as_str()),
    }
}

/// Phase 306: Authenticate a pod WS connection.
/// Tries JWT first (steady-state), then PSK (bootstrap).
fn authenticate_agent_ws(state: &AppState, params: &WsAuthParams) -> Result<AgentAuthResult, String> {
    if let Some(ref jwt_token) = params.jwt {
        if !jwt_token.is_empty() {
            let prev_secret = state.config.auth.jwt_secret_previous.as_deref();
            match crate::auth::middleware::decode_pod_jwt(
                jwt_token,
                &state.config.auth.jwt_secret,
                prev_secret,
            ) {
                Ok(claims) => {
                    return Ok(AgentAuthResult::JwtAuthenticated {
                        pod_id: claims.pod_id,
                        pod_number: claims.pod_number,
                    });
                }
                Err(e) => return Err(format!("Invalid pod JWT: {}", e)),
            }
        }
    }
    let psk_ok = match &state.config.cloud.terminal_secret {
        None => true,
        Some(s) if s.is_empty() => true,
        Some(secret) => params.token.as_deref() == Some(secret.as_str()),
    };
    if psk_ok { Ok(AgentAuthResult::PskAuthenticated) }
    else { Err("Invalid or missing PSK token".to_string()) }
}

/// WebSocket endpoint for pod agents
pub async fn agent_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    match authenticate_agent_ws(&state, &params) {
        Ok(auth_result) => Ok(ws.on_upgrade(move |socket| handle_agent(socket, state, auth_result))),
        Err(reason) => {
            tracing::warn!("WS agent connection rejected — {}", reason);
            // WSAUTH-03: WhatsApp alert on invalid JWT (not PSK — too noisy)
            if params.jwt.as_ref().map_or(false, |j| !j.is_empty()) {
                let state_clone = state.clone();
                let reason_clone = reason.clone();
                tokio::spawn(async move {
                    crate::whatsapp_alerter::send_admin_alert(
                        &state_clone.config,
                        "ws_jwt_rejected",
                        &format!("Pod WS connection rejected: {}", reason_clone),
                    ).await;
                });
            }
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// WebSocket endpoint for dashboard clients
pub async fn dashboard_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_ws_token(&state, &params.token) {
        tracing::warn!("WS dashboard connection rejected — invalid or missing token");
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(ws.on_upgrade(|socket| handle_dashboard(socket, state)))
}

/// Phase 306: Issue a 24-hour pod JWT and queue it for sending.
fn issue_pod_jwt_to_agent(
    state: &AppState,
    pod_id: &str,
    pod_number: u32,
    cmd_tx: &mpsc::Sender<CoreToAgentMessage>,
) {
    match crate::auth::middleware::create_pod_jwt(&state.config.auth.jwt_secret, pod_id, pod_number, 24) {
        Ok(token) => {
            let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp();
            if cmd_tx.try_send(CoreToAgentMessage::IssueJwt { token, expires_at }).is_ok() {
                tracing::info!("Phase 306: JWT issued to pod {} (expires_at={})", pod_id, expires_at);
            } else {
                tracing::warn!("Phase 306: Failed to queue IssueJwt for pod {}", pod_id);
            }
        }
        Err(e) => tracing::error!("Phase 306: Failed to create pod JWT for {}: {}", pod_id, e),
    }
}

async fn handle_agent(socket: WebSocket, state: Arc<AppState>, auth_result: AgentAuthResult) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Unique ID for this connection — used to avoid stale disconnect cleanup
    static CONN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let conn_id = CONN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    tracing::info!("Pod agent connected (conn_id={}, auth={})", conn_id,
        match &auth_result { AgentAuthResult::PskAuthenticated => "psk", AgentAuthResult::JwtAuthenticated { .. } => "jwt" });

    // Create mpsc channel for sending commands back to this agent
    let (cmd_tx, mut cmd_rx) = mpsc::channel::<CoreToAgentMessage>(64);
    let mut registered_pod_id: Option<String> = None;

    // Phase 306: JWT was already issued if this is a JWT-authenticated connection
    let jwt_issued_for_conn = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        matches!(auth_result, AgentAuthResult::JwtAuthenticated { .. }),
    ));

    // Shared state for pending application-level ping measurement
    // send_task writes (id, Instant) when it sends a Ping; receive loop reads+clears it on Pong
    static PING_ID_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pending_ping: Arc<tokio::sync::Mutex<Option<(u64, Instant)>>> = Arc::new(tokio::sync::Mutex::new(None));
    let pending_ping_send = pending_ping.clone();

    // Spawn task to forward commands from mpsc to WebSocket sender.
    // Also sends WS-level keepalive ping every 15s (CONN-01) and
    // an app-level measurement Ping every 30s (PERF-03).
    let send_task = tokio::spawn(async move {
        let mut ping_interval = interval(Duration::from_secs(15));
        ping_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        let mut measure_interval = interval(Duration::from_secs(30));
        measure_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        // Consume the immediate first tick so the first real tick fires after the full interval
        ping_interval.tick().await;
        measure_interval.tick().await;

        loop {
            tokio::select! {
                cmd = cmd_rx.recv() => {
                    match cmd {
                        Some(cmd) => {
                            // DEPLOY-05: Wrap with CoreMessage to add command_id for agent deduplication.
                            // This is the single serialization point for all CoreToAgentMessage sends.
                            let wrapped = CoreMessage::wrap(cmd);
                            if let Ok(json) = serde_json::to_string(&wrapped) {
                                // MMA-P2: Log and continue on transient send failures instead
                                // of breaking the entire send loop. Only break on channel close.
                                if let Err(e) = ws_sender.send(Message::Text(json.into())).await {
                                    tracing::warn!("WS send failed (conn_id={}): {} — continuing", conn_id, e);
                                    // If the socket is truly closed, the next send will also fail
                                    // and we'll detect it on the next iteration or via ping failure
                                }
                            }
                        }
                        None => break, // Channel closed — handle_agent is exiting
                    }
                }
                _ = ping_interval.tick() => {
                    // WS-level keepalive ping to prevent TCP idle timeout during CPU spikes
                    tracing::trace!("WS ping sent (conn_id={})", conn_id);
                    if ws_sender.send(Message::Ping(vec![].into())).await.is_err() {
                        break;
                    }
                }
                _ = measure_interval.tick() => {
                    // Application-level ping for round-trip latency measurement
                    let ping_id = PING_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let msg = CoreToAgentMessage::Ping { id: ping_id };
                    if let Ok(json) = serde_json::to_string(&msg) {
                        // Record send time before sending
                        *pending_ping_send.lock().await = Some((ping_id, Instant::now()));
                        if ws_sender.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    // Phase 306 WSAUTH-02: JWT rotation — issue RefreshJwt ~1h before the 24h token expires.
    // A shared Arc<Mutex<Option<(String, u32)>>> lets the receive loop inform the rotation task
    // which pod is registered. The task spawns a one-shot 23h sleep then sends RefreshJwt.
    let jwt_rotation_pod_id: std::sync::Arc<tokio::sync::Mutex<Option<(String, u32)>>> =
        std::sync::Arc::new(tokio::sync::Mutex::new(None));
    {
        let rotation_pod_id = jwt_rotation_pod_id.clone();
        let rotation_state = state.clone();
        let rotation_cmd_tx = cmd_tx.clone();
        tokio::spawn(async move {
            // Wait up to 60s for the pod to Register and set rotation_pod_id
            tokio::time::sleep(Duration::from_secs(60)).await;
            let guard = rotation_pod_id.lock().await;
            let Some((ref pod_id, pod_number)) = *guard else { return };
            let pod_id = pod_id.clone();
            drop(guard);
            // Now wait another 23h (total ~23h after connect), then refresh
            tokio::time::sleep(Duration::from_secs(22 * 3600)).await;
            if rotation_cmd_tx.is_closed() { return; }
            match crate::auth::middleware::create_pod_jwt(&rotation_state.config.auth.jwt_secret, &pod_id, pod_number, 24) {
                Ok(token) => {
                    let expires_at = (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp();
                    let msg = CoreToAgentMessage::RefreshJwt { token, expires_at };
                    if rotation_cmd_tx.try_send(msg).is_ok() {
                        tracing::info!("Phase 306: JWT refreshed for pod {} (expires_at={})", pod_id, expires_at);
                    }
                }
                Err(e) => tracing::error!("Phase 306: JWT refresh failed for {}: {}", pod_id, e),
            }
        });
    }

    // Listen for messages from the agent
    while let Some(Ok(msg)) = ws_receiver.next().await {
        if let Message::Text(text) = msg {
            match serde_json::from_str::<AgentMessage>(&text) {
                Ok(agent_msg) => {
                    match &agent_msg {
                        AgentMessage::Register(pod_info) => {
                            // Normalize pod_id to canonical form (pod_N) at registration entry
                            let canonical_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|_| pod_info.id.clone());
                            tracing::info!("Pod {} registered (conn_id={}): {}", pod_info.number, conn_id, pod_info.name);
                            registered_pod_id = Some(canonical_id.clone());
                            // Phase 306: Tell rotation task which pod this connection serves
                            *jwt_rotation_pod_id.lock().await = Some((canonical_id.clone(), pod_info.number));
                            log_pod_activity(&state, &canonical_id, "system", "Pod Online", &format!("Pod {} connected (conn_id={})", pod_info.number, conn_id), "agent", None);
                            event_archive::append_event(&state.db, "pod.online", "ws", Some(&canonical_id), serde_json::json!({
                                "pod_number": pod_info.number,
                                "conn_id": conn_id,
                            }), &state.config.venue.venue_id);

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

                            // MMA-P1-FIX: Sync pod registration to SQLite — keeps DB in sync
                            // with in-memory state so kiosk/API queries see current pod data.
                            // - ON CONFLICT preserves 'disabled' status (MMA F-02)
                            // - Validates number matches seeded value (MMA F-03/F-06)
                            // - Awaited (not spawned) to prevent race with disconnect (MMA F-01)
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
                                        // MMA-R2: Use sqlx error type matching, not string contains
                                        let is_unique = matches!(e, sqlx::Error::Database(db_err) if db_err.code().map_or(false, |c| c == "2067"));
                                        if is_unique {
                                            tracing::error!(
                                                "Pod {} registration rejected: number {} conflicts with another pod — rolling back in-memory",
                                                canonical_id, pod_info.number
                                            );
                                            // MMA-R2-01: Roll back in-memory insert to prevent divergence
                                            state.pods.write().await.remove(&canonical_id);
                                            state.agent_senders.write().await.remove(&canonical_id);
                                            state.agent_conn_ids.write().await.remove(&canonical_id);
                                            registered_pod_id = None;
                                            continue; // Skip rest of Register handling
                                        } else {
                                            tracing::warn!("Failed to sync pod {} registration to DB: {}", canonical_id, e);
                                        }
                                    }
                                }
                            }

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::PodUpdate(pod_info.clone()));

                            // Reconcile game_launcher.active_games with pod's reported state
                            {
                                let mut games = state.game_launcher.active_games.write().await;
                                let pod_game_state = pod_info.game_state.unwrap_or(GameState::Idle);
                                match pod_game_state {
                                    GameState::Running | GameState::Launching | GameState::Loading => {
                                        // Pod reports a game is active — ensure tracker exists
                                        if let Some(tracker) = games.get_mut(&canonical_id) {
                                            tracker.game_state = pod_game_state;
                                        } else if let Some(sim) = pod_info.current_game {
                                            games.insert(
                                                canonical_id.clone(),
                                                game_launcher::GameTracker {
                                                    pod_id: canonical_id.clone(),
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
                                                },
                                            );
                                            tracing::info!("Reconciled game tracker for pod {} on reconnect ({:?})", pod_info.number, pod_game_state);
                                        }
                                    }
                                    GameState::Idle | GameState::Stopping | GameState::Error => {
                                        // Pod reports idle — remove any stale tracker
                                        if games.remove(&canonical_id).is_some() {
                                            tracing::info!("Removed stale game tracker for pod {} on reconnect", pod_info.number);
                                        }
                                    }
                                }
                            }

                            // Resync active billing session to reconnected agent
                            {
                                let resync = {
                                    let timers = state.billing.active_timers.read().await;
                                    timers.get(&canonical_id).map(|timer| (
                                        timer.session_id.clone(),
                                        timer.driver_name.clone(),
                                        timer.allocated_seconds,
                                        timer.remaining_seconds(),
                                    ))
                                };
                                if let Some((session_id, driver_name, allocated_seconds, remaining)) = resync {
                                    let _ = cmd_tx.send(CoreToAgentMessage::BillingStarted {
                                        billing_session_id: session_id.clone(),
                                        driver_name: driver_name.clone(),
                                        allocated_seconds,
                                        session_token: Some(uuid::Uuid::new_v4().to_string()),
                                    }).await;
                                    let _ = cmd_tx.send(CoreToAgentMessage::BillingTick {
                                        remaining_seconds: remaining,
                                        allocated_seconds,
                                        driver_name: driver_name.clone(),
                                        tick_seq: 0, // initial tick on reconnect — next real tick will be seq > 0
                                        elapsed_seconds: None,
                                        cost_paise: None,
                                        rate_per_min_paise: None,
                                        paused: None,
                                        minutes_to_next_tier: None,
                                        tier_name: None,
                                    }).await;
                                    // Restore pod state (agent Register overwrites with Idle)
                                    {
                                        let mut pods = state.pods.write().await;
                                        if let Some(pod) = pods.get_mut(&canonical_id) {
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
                                    let _ = cmd_tx.send(CoreToAgentMessage::SettingsUpdated { settings: pod_settings }).await;
                                    tracing::info!("Sent initial kiosk settings to pod {}", pod_info.number);
                                }
                            }

                            // Phase 306 WSAUTH-01/04: Issue JWT after PSK bootstrap.
                            if !jwt_issued_for_conn.load(std::sync::atomic::Ordering::Relaxed) {
                                issue_pod_jwt_to_agent(&state, &canonical_id, pod_info.number, &cmd_tx);
                                jwt_issued_for_conn.store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            // Phase 296 PUSH-02: Push stored full AgentConfig to pod on connect
                            if let Err(e) = crate::config_push::push_full_config_to_pod(
                                &state, &canonical_id, &cmd_tx,
                            ).await {
                                tracing::warn!("Failed to push full config to pod {} on connect: {}", canonical_id, e);
                            }
                            // Phase 298 PRESET-02: Push preset library to pod on connect
                            if let Err(e) = crate::preset_library::push_presets_to_pod(&state, &canonical_id, &cmd_tx).await {
                                tracing::warn!("Failed to push presets to pod {} on connect: {}", canonical_id, e);
                            }
                        }
                        AgentMessage::Heartbeat(pod_info) => {
                            // Merge agent-reported fields with core-managed fields
                            // (billing_session_id, current_driver, status are managed by racecontrol billing)
                            // OR-016: Normalize pod_id (same as Register handler) to prevent split state
                            let hb_pod_id = normalize_pod_id(&pod_info.id).unwrap_or_else(|_| pod_info.id.clone());
                            // Kimi-004: Verify heartbeat sender matches this connection's registered pod
                            if let Some(ref expected) = registered_pod_id {
                                if &hb_pod_id != expected {
                                    tracing::warn!("Heartbeat pod_id mismatch: conn registered as {} but sent heartbeat for {}", expected, hb_pod_id);
                                    continue; // Reject spoofed heartbeat
                                }
                            }
                            let mut pods = state.pods.write().await;
                            let updated = if let Some(existing) = pods.get_mut(&hb_pod_id) {
                                // Preserve core-managed billing state
                                existing.ip_address = pod_info.ip_address.clone();
                                let now = chrono::Utc::now();
                                existing.last_seen = Some(now);
                                // OR-007: Only accept agent-reported game_state if valid transition
                                // Prevents stale heartbeats from reverting Running→Idle etc.
                                existing.driving_state = pod_info.driving_state;
                                if let Some(new_gs) = pod_info.game_state {
                                    let accept = match (existing.game_state, new_gs) {
                                        // Never allow heartbeat to revert from Running to Idle/Launching
                                        // (only GameStateUpdate messages should do that)
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
                                // Backfill MAC address if missing (needed for WOL)
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

                            // FSM-02: Phantom billing guard — detect billing=active + game=Idle >30s.
                            // Skip check if game_state is None (old agents may not send it).
                            if let Some(reported_gs) = pod_info.game_state {
                                let has_active_billing = {
                                    let timers = state.billing.active_timers.read().await;
                                    timers.get(&hb_pod_id).map_or(false, |t| {
                                        matches!(t.status, BillingSessionStatus::Active)
                                    })
                                };
                                if has_active_billing && reported_gs == GameState::Idle {
                                    // Condition detected: billing active but game idle — start or check timer
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
                                        // Auto-pause the billing timer
                                        {
                                            let mut timers = state.billing.active_timers.write().await;
                                            if let Some(timer) = timers.get_mut(&hb_pod_id) {
                                                if timer.status == BillingSessionStatus::Active {
                                                    timer.status = BillingSessionStatus::PausedGamePause; // FSM-02: phantom guard
                                                }
                                            }
                                        }
                                        // Clear the phantom timer entry — condition resolved by pausing
                                        state.phantom_billing_start.write().await.remove(&hb_pod_id);
                                    }
                                } else {
                                    // Condition cleared: remove phantom timer entry if it exists
                                    let has_entry = state.phantom_billing_start.read().await.contains_key(&hb_pod_id);
                                    if has_entry {
                                        state.phantom_billing_start.write().await.remove(&hb_pod_id);
                                    }
                                }
                            }

                            // RESIL-08: Clock drift detection — compare agent_timestamp with server time.
                            // Drop any lock before async work. Snapshot only what is needed.
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
                                    // Update fleet health store with drift value
                                    let mut fleet = state.pod_fleet_health.write().await;
                                    let store = fleet.entry(hb_pod_id.clone()).or_default();
                                    store.clock_drift_secs = Some(drift_secs);
                                }
                            }
                        }
                        AgentMessage::Telemetry(frame) => {
                            // MMA-ITER1-#4 (8/8): Override pod_id with authenticated WS identity
                            let mut frame = frame.clone();
                            if let Some(ref expected) = registered_pod_id {
                                if frame.pod_id != *expected {
                                    tracing::warn!("Telemetry pod_id spoof: conn={} frame={} — overriding", expected, frame.pod_id);
                                    frame.pod_id = expected.clone();
                                }
                            }
                            // Feed telemetry to camera controller
                            crate::ac_camera::on_telemetry(&state, &frame).await;
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::Telemetry(frame.clone()));
                            // Phase 251: Send to telemetry writer for persistence
                            if let Some(ref tx) = state.telemetry_writer_tx {
                                // Non-blocking send — drop frame if channel is full
                                let _ = tx.try_send(frame.clone());
                            }
                        }
                        AgentMessage::LapCompleted(lap) => {
                            let mut lap = lap.clone();

                            // Resolve driver from active billing session on this pod
                            if let Some((driver_id, session_id)) =
                                crate::lap_tracker::resolve_driver_for_pod(&state, &lap.pod_id).await
                            {
                                lap.driver_id = driver_id;
                                lap.session_id = session_id;
                            }

                            tracing::info!(
                                "Lap completed: {} - {}ms on {}",
                                lap.driver_id, lap.lap_time_ms, lap.track
                            );

                            // Persist to DB and update leaderboards
                            crate::lap_tracker::persist_lap(&state, &lap).await;

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::LapCompleted(lap));
                        }
                        AgentMessage::SessionUpdate(session) => {
                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::SessionUpdate(session.clone()));
                        }
                        AgentMessage::DrivingStateUpdate { pod_id, state: driving_state } => {
                            tracing::debug!("Pod {} driving state: {:?}", pod_id, driving_state);

                            // Update pod info
                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                pod.driving_state = Some(*driving_state);
                            }

                            // Update billing timer
                            billing::update_driving_state(&state, pod_id, *driving_state).await;
                        }
                        AgentMessage::GameStateUpdate(info) => {
                            tracing::info!(
                                "Pod {} game state: {:?} ({:?})",
                                info.pod_id, info.game_state, info.sim_type
                            );
                            let gs_action = match info.game_state {
                                GameState::Running => "Game Running",
                                GameState::Loading => "Game Loading",
                                GameState::Error => "Game Crashed",
                                GameState::Idle => "Game Stopped",
                                GameState::Launching => "Game Launching",
                                GameState::Stopping => "Game Stopping",
                            };
                            let gs_details = match &info.error_message {
                                Some(err) => format!("{}: {}", info.sim_type, err),
                                None => format!("{}", info.sim_type),
                            };
                            log_pod_activity(&state, &info.pod_id, "game", gs_action, &gs_details, "agent", None);
                            game_launcher::handle_game_state_update(&state, info.clone()).await;
                        }
                        AgentMessage::AiDebugResult(suggestion) => {
                            tracing::info!(
                                "AI debug suggestion for pod {}: {}",
                                suggestion.pod_id, suggestion.model
                            );
                            // Persist to DB
                            let id = uuid::Uuid::new_v4().to_string();
                            let _ = sqlx::query(
                                "INSERT INTO ai_suggestions (id, pod_id, sim_type, error_context, suggestion, model, source) \
                                 VALUES (?, ?, ?, ?, ?, ?, 'crash')"
                            )
                            .bind(&id)
                            .bind(&suggestion.pod_id)
                            .bind(format!("{:?}", suggestion.sim_type))
                            .bind(&suggestion.error_context)
                            .bind(&suggestion.suggestion)
                            .bind(&suggestion.model)
                            .execute(&state.db)
                            .await;

                            let _ = state
                                .dashboard_tx
                                .send(DashboardEvent::AiDebugSuggestion(suggestion.clone()));
                        }
                        AgentMessage::PinEntered { pod_id, pin } => {
                            tracing::info!("PIN entered on pod {}", pod_id);
                            log_pod_activity(&state, pod_id, "auth", "PIN Entered", "", "agent", None);
                            auth::handle_pin_entered(&state, pod_id.clone(), pin.clone()).await;
                        }
                        AgentMessage::Pong { id, agent_delay_us } => {
                            // Application-level round-trip measurement response
                            let mut guard = pending_ping.lock().await;
                            if let Some((pending_id, sent_at)) = guard.take() {
                                if pending_id == *id {
                                    let elapsed_ms = sent_at.elapsed().as_millis();
                                    let fallback_label = format!("conn_{}", conn_id);
                                    let label = registered_pod_id.as_deref().unwrap_or(&fallback_label);
                                    if elapsed_ms > 600 {
                                        let agent_info = match agent_delay_us {
                                            Some(us) => format!(", agent_process={}us", us),
                                            None => String::new(),
                                        };
                                        tracing::warn!(
                                            "WS round-trip slow: {} took {}ms (threshold 600ms{})",
                                            label, elapsed_ms, agent_info
                                        );
                                    } else {
                                        tracing::debug!(
                                            "WS round-trip: {}ms ({})",
                                            elapsed_ms, label
                                        );
                                    }
                                } else {
                                    // Stale pong (id mismatch) — discard
                                    tracing::debug!(
                                        "Stale pong id={} (expected {}), discarding",
                                        id, pending_id
                                    );
                                }
                            }
                        }
                        AgentMessage::GameStatusUpdate { pod_id, ac_status, sim_type } => {
                            tracing::info!("Pod {} AC STATUS: {:?}", pod_id, ac_status);
                            log_pod_activity(&state, pod_id, "game", &format!("AC Status: {:?}", ac_status), "", "agent", None);
                            billing::handle_game_status_update(&state, pod_id, *ac_status, *sim_type, &cmd_tx).await;
                        }
                        AgentMessage::FfbZeroed { pod_id } => {
                            tracing::info!("Pod {} FFB zeroed (safety action completed)", pod_id);
                            log_pod_activity(&state, pod_id, "safety", "FFB Zeroed", "Wheelbase torque set to 0", "agent", None);
                        }
                        AgentMessage::GameCrashed { pod_id, billing_active } => {
                            tracing::warn!("Pod {} game crashed (billing_active={})", pod_id, billing_active);
                            log_pod_activity(&state, pod_id, "game", "Game Crashed", &format!("billing_active={}", billing_active), "agent", None);
                            // CRASH-02: Auto-pause billing on game crash
                            if *billing_active {
                                let mut timers = state.billing.active_timers.write().await;
                                if let Some(timer) = timers.get_mut(pod_id.as_str()) {
                                    if timer.status == BillingSessionStatus::Active {
                                        timer.status = BillingSessionStatus::PausedGamePause;
                                        timer.pause_seconds = 0;
                                        timer.pause_count += 1;
                                        tracing::info!("Billing auto-paused on crash for pod {}", pod_id);
                                    }
                                }
                            }

                            // RESIL-06: Record crash event and check if pod should be flagged for maintenance.
                            // Drop lock before any async work (standing rule: no lock across .await).
                            let crash_id = uuid::Uuid::new_v4().to_string();
                            let crash_result = sqlx::query(
                                "INSERT INTO pod_crash_events (id, pod_id, crash_type) VALUES (?, ?, 'game_crash')"
                            )
                            .bind(&crash_id)
                            .bind(pod_id)
                            .execute(&state.db)
                            .await;

                            if let Err(e) = crash_result {
                                tracing::warn!("RESIL-06: Failed to insert crash event for pod {}: {}", pod_id, e);
                            } else {
                                // Count crashes in last hour
                                let count_result: Result<(i64,), _> = sqlx::query_as(
                                    "SELECT COUNT(*) FROM pod_crash_events WHERE pod_id = ? AND created_at > datetime('now', '-1 hour')"
                                )
                                .bind(pod_id)
                                .fetch_one(&state.db)
                                .await;

                                if let Ok((count,)) = count_result {
                                    // Snapshot pod_id for async use, update fleet health store
                                    let pod_id_owned = pod_id.clone();
                                    let count_i32 = count as i32;
                                    {
                                        let mut fleet = state.pod_fleet_health.write().await;
                                        let store = fleet.entry(pod_id_owned.clone()).or_default();
                                        store.crashes_last_hour = count_i32;
                                        if count > 3 && !store.maintenance_flag {
                                            store.maintenance_flag = true;
                                            tracing::error!(
                                                "RESIL-06: Pod {} flagged for maintenance — {} crashes in 1 hour",
                                                pod_id_owned, count
                                            );
                                            // Send WhatsApp alert (after lock is dropped)
                                            let alert_msg = format!(
                                                "[MAINTENANCE] Pod {} auto-flagged: {} crashes in last hour. Check hardware. {}",
                                                pod_id_owned, count,
                                                crate::whatsapp_alerter::ist_now_string()
                                            );
                                            // drop fleet write guard before async send
                                            drop(fleet);
                                            crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
                                        }
                                    }
                                }
                            }
                        }
                        AgentMessage::AssistChanged { pod_id, assist_type, enabled, confirmed } => {
                            tracing::info!(
                                "Pod {} assist changed: {} = {} (confirmed: {})",
                                pod_id, assist_type, enabled, confirmed
                            );
                            log_pod_activity(&state, pod_id, "game", "Assist Changed",
                                &format!("{} = {} (confirmed: {})", assist_type, enabled, confirmed), "agent", None);
                            // Update assist cache with the changed value
                            {
                                let mut cache = state.assist_cache.write().await;
                                let entry = cache.entry(pod_id.clone()).or_default();
                                match assist_type.as_str() {
                                    "abs" => entry.abs = if *enabled { 1 } else { 0 },
                                    "tc" => entry.tc = if *enabled { 1 } else { 0 },
                                    "transmission" => entry.auto_shifter = *enabled,
                                    _ => {}
                                }
                            }
                        }
                        AgentMessage::FfbGainChanged { pod_id, percent } => {
                            tracing::info!("Pod {} FFB gain changed to {}%", pod_id, percent);
                            log_pod_activity(&state, pod_id, "game", "FFB Gain Changed",
                                &format!("{}%", percent), "agent", None);
                            // Update FFB percent in assist cache
                            {
                                let mut cache = state.assist_cache.write().await;
                                let entry = cache.entry(pod_id.clone()).or_default();
                                entry.ffb_percent = *percent;
                            }
                        }
                        AgentMessage::AssistState { pod_id, abs, tc, auto_shifter, ffb_percent } => {
                            tracing::info!(
                                "Pod {} assist state: ABS={} TC={} auto_shifter={} FFB={}%",
                                pod_id, abs, tc, auto_shifter, ffb_percent
                            );
                            log_pod_activity(&state, pod_id, "game", "Assist State",
                                &format!("ABS={} TC={} auto_shifter={} FFB={}%", abs, tc, auto_shifter, ffb_percent), "agent", None);
                            // Replace entire cached state for this pod with fresh data from agent
                            {
                                let mut cache = state.assist_cache.write().await;
                                cache.insert(pod_id.clone(), CachedAssistState {
                                    abs: *abs,
                                    tc: *tc,
                                    auto_shifter: *auto_shifter,
                                    ffb_percent: *ffb_percent,
                                });
                            }
                        }
                        AgentMessage::ContentManifest(manifest) => {
                            if let Some(ref pod_id) = registered_pod_id {
                                let car_count = manifest.cars.len();
                                let track_count = manifest.tracks.len();
                                tracing::info!("Pod {} content manifest: {} cars, {} tracks", pod_id, car_count, track_count);
                                log_pod_activity(&state, pod_id, "content", "Content Scanned",
                                    &format!("{} cars, {} tracks", car_count, track_count), "agent", None);
                                state.pod_manifests.write().await.insert(pod_id.clone(), manifest.clone());
                            }
                        }
                        AgentMessage::ExecResult { request_id, success, exit_code, stdout, stderr } => {
                            tracing::info!("WS command result {}: success={}", request_id, success);
                            let mut pending = state.pending_ws_execs.write().await;
                            if let Some(sender) = pending.remove(request_id) {
                                let _ = sender.send(crate::state::WsExecResult {
                                    success: *success,
                                    exit_code: *exit_code,
                                    stdout: stdout.clone(),
                                    stderr: stderr.clone(),
                                });
                            } else {
                                tracing::warn!("No pending request for request_id={}", request_id);
                            }
                        }
                        AgentMessage::StartupReport {
                            pod_id, version, uptime_secs, config_hash,
                            crash_recovery, repairs,
                            lock_screen_port_bound, remote_ops_port_bound,
                            hid_detected, udp_ports_bound,
                            ..
                        } => {
                            tracing::info!(
                                "Pod {} startup report: version={}, uptime={}s, config_hash={}, crash_recovery={}, repairs={:?}, \
                                 lock_screen={}, remote_ops={}, hid={}, udp_ports={:?}",
                                pod_id, version, uptime_secs, config_hash, crash_recovery, repairs,
                                lock_screen_port_bound, remote_ops_port_bound, hid_detected, udp_ports_bound
                            );
                            if *crash_recovery {
                                tracing::warn!("Pod {} recovered from a crash!", pod_id);
                            }
                            if !repairs.is_empty() {
                                tracing::warn!("Pod {} self-healed: {:?}", pod_id, repairs);
                            }
                            if !lock_screen_port_bound {
                                tracing::warn!("Pod {} BOOT WARNING: lock screen port 18923 NOT bound!", pod_id);
                            }
                            if !remote_ops_port_bound {
                                tracing::warn!("Pod {} BOOT WARNING: remote ops port 8090 NOT bound!", pod_id);
                            }
                            log_pod_activity(
                                &state,
                                pod_id,
                                "system",
                                "Startup Report",
                                &format!("v{} uptime={}s hash={} crash_recovery={} repairs={:?}",
                                    version, uptime_secs, config_hash, crash_recovery, repairs),
                                "agent",
                                None,
                            );
                            // Store version + uptime + boot verification for fleet health dashboard.
                            let crash_loop_just_detected = {
                                let mut fleet = state.pod_fleet_health.write().await;
                                let store = fleet.entry(pod_id.clone()).or_default();
                                let was_looping = store.crash_loop;
                                crate::fleet_health::store_startup_report(
                                    store, version, *uptime_secs, *crash_recovery,
                                    *lock_screen_port_bound, *remote_ops_port_bound,
                                    *hid_detected, udp_ports_bound,
                                );
                                // Newly detected crash loop (transition false → true)
                                !was_looping && store.crash_loop
                            };
                            // Phase 9b: WhatsApp alert on crash loop detection (once per loop)
                            if crash_loop_just_detected {
                                let alert_msg = format!(
                                    "🔴 CRASH LOOP: Pod {} is restarting every ~17s (uptime={}s). Likely OS/hardware issue. Needs reboot.",
                                    pod_id, uptime_secs
                                );
                                tracing::error!("{}", alert_msg);
                                crate::whatsapp_alerter::send_admin_alert(
                                    &state.config,
                                    "crash_loop",
                                    &alert_msg,
                                ).await;
                            }
                        }
                        AgentMessage::HardwareFailure { pod_id, reason, detail } => {
                            crate::bot_coordinator::handle_hardware_failure(&state, &pod_id, &reason, &detail).await;
                        }
                        AgentMessage::HardwareDisconnect { pod_id, device, timestamp } => {
                            tracing::error!(
                                "RESIL-04: Hardware disconnect on pod {}: {} at {}",
                                pod_id, device, timestamp
                            );
                            log_pod_activity(&state, pod_id, "hardware", "USB Disconnect",
                                &format!("{} disconnected", device), "agent", None);

                            // Pause active billing session if present
                            // Snapshot pod_id and billing state before async work
                            let has_active_billing = {
                                let timers = state.billing.active_timers.read().await;
                                timers.get(pod_id.as_str()).map_or(false, |t| {
                                    matches!(t.status, BillingSessionStatus::Active)
                                })
                            };
                            if has_active_billing {
                                let mut timers = state.billing.active_timers.write().await;
                                if let Some(timer) = timers.get_mut(pod_id.as_str()) {
                                    if timer.status == BillingSessionStatus::Active {
                                        timer.status = BillingSessionStatus::PausedGamePause;
                                        timer.pause_count += 1;
                                        tracing::info!(
                                            "RESIL-04: Billing auto-paused for pod {} — {} disconnected",
                                            pod_id, device
                                        );
                                    }
                                }
                                drop(timers);
                            }

                            // WhatsApp alert to staff
                            let alert_msg = format!(
                                "[HW ALERT] {} disconnected on Pod {}. Billing paused. {}",
                                device, pod_id,
                                crate::whatsapp_alerter::ist_now_string()
                            );
                            crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
                        }
                        AgentMessage::TelemetryGap { pod_id, sim_type: _, gap_seconds } => {
                            crate::bot_coordinator::handle_telemetry_gap(&state, &pod_id, *gap_seconds as u64).await;
                        }
                        AgentMessage::BillingAnomaly { pod_id, billing_session_id, reason, detail } => {
                            crate::bot_coordinator::handle_billing_anomaly(&state, &pod_id, &billing_session_id, *reason, &detail).await;
                        }
                        AgentMessage::LapFlagged { pod_id, lap_id, reason, detail } => {
                            tracing::info!("[bot] LapFlagged pod={} lap={} reason={:?}: {}", pod_id, lap_id, reason, detail);
                        }
                        AgentMessage::MultiplayerFailure { pod_id, reason, session_id } => {
                            crate::bot_coordinator::handle_multiplayer_failure(
                                &state,
                                &pod_id,
                                &reason,
                                session_id.as_deref(),
                            ).await;
                        }
                        AgentMessage::Disconnect { pod_id } => {
                            tracing::info!("Pod {} disconnected", pod_id);
                            log_pod_activity(&state, pod_id, "system", "Pod Offline", "Agent sent disconnect", "agent", None);
                            event_archive::append_event(&state.db, "pod.offline", "ws", Some(pod_id), serde_json::json!({ "reason": "agent_disconnect" }), &state.config.venue.venue_id);
                            let has_active_billing = state
                                .billing
                                .active_timers
                                .read()
                                .await
                                .contains_key(pod_id.as_str());

                            if let Some(pod) = state.pods.write().await.get_mut(pod_id) {
                                // Don't overwrite Disabled — admin intentionally shut it down
                                if pod.status == rc_common::types::PodStatus::Disabled {
                                    break;
                                }
                                pod.status = rc_common::types::PodStatus::Offline;
                                pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                                // Preserve game_state if billing is active — agent will resync on reconnect
                                if !has_active_billing {
                                    pod.game_state = Some(GameState::Idle);
                                    pod.current_game = None;
                                }
                                let _ = state
                                    .dashboard_tx
                                    .send(DashboardEvent::PodUpdate(pod.clone()));
                            }
                            // Update billing timer to no-device
                            billing::update_driving_state(
                                &state,
                                pod_id,
                                rc_common::types::DrivingState::NoDevice,
                            )
                            .await;
                            // MMA-P1-FIX: Sync offline status to DB — preserves disabled/maintenance.
                            if let Err(e) = sqlx::query(
                                "UPDATE pods SET status = 'offline', last_seen = datetime('now')
                                 WHERE id = ? AND status NOT IN ('disabled', 'maintenance')"
                            )
                            .bind(pod_id)
                            .execute(&state.db)
                            .await {
                                tracing::warn!("Failed to sync pod {} graceful disconnect to DB: {}", pod_id, e);
                            }
                            // Clear fleet health version/uptime on graceful disconnect.
                            {
                                let mut fleet = state.pod_fleet_health.write().await;
                                if let Some(store) = fleet.get_mut(pod_id.as_str()) {
                                    crate::fleet_health::clear_on_disconnect(store);
                                }
                            }
                            break;
                        }
                        AgentMessage::ProcessApprovalRequest { pod_id, process_name, exe_path, sighting_count } => {
                            tracing::warn!(
                                "[kiosk] Pod {} requesting approval for '{}' (path: {}, seen {} times)",
                                pod_id, process_name, exe_path, sighting_count
                            );
                            log_pod_activity(
                                &state,
                                &pod_id,
                                "kiosk",
                                "Process Approval Request",
                                &format!("Process '{}' at '{}' seen {} times — awaiting approval", process_name, exe_path, sighting_count),
                                "rc-bot",
                                None,
                            );
                            // TODO: forward to admin dashboard for approve/reject UI
                            // For now, log and let TTL handle it (auto-reject after 10min)
                        }
                        AgentMessage::KioskLockdown { pod_id, reason } => {
                            tracing::warn!("[kiosk] Pod {} LOCKDOWN: {}", pod_id, reason);
                            log_pod_activity(
                                &state,
                                &pod_id,
                                "kiosk",
                                "Kiosk Lockdown",
                                &reason,
                                "rc-bot",
                                None,
                            );

                            // Auto-pause active billing session on this pod (SESS-05)
                            let pause_result: Result<Option<(String,)>, _> = sqlx::query_as(
                                "SELECT id FROM billing_sessions WHERE pod_id = ? AND status = 'active' ORDER BY started_at DESC LIMIT 1"
                            )
                            .bind(&pod_id)
                            .fetch_optional(&state.db)
                            .await;

                            if let Ok(Some((session_id,))) = pause_result {
                                let pause_reason = format!("Security anomaly: {}", reason);
                                let _ = sqlx::query(
                                    "UPDATE billing_sessions SET status = 'paused_manual', pause_count = pause_count + 1 WHERE id = ? AND status = 'active'"
                                )
                                .bind(&session_id)
                                .execute(&state.db)
                                .await;
                                tracing::warn!("[kiosk] Billing session {} auto-paused due to lockdown on pod {}", session_id, pod_id);
                                log_pod_activity(&state, &pod_id, "billing", "Auto-Pause", &pause_reason, "rc-bot", None);
                            }

                            // WhatsApp alert with debounce (SESS-05)
                            let alert_msg = format!(
                                "SECURITY ALERT -- Pod {} LOCKDOWN\nReason: {}\nBilling auto-paused. Check admin dashboard.",
                                pod_id, reason
                            );
                            crate::whatsapp_alerter::send_security_alert(&state.config, &pod_id, &alert_msg).await;
                        }
                        // SESSION-01: Agent auto-ended an orphaned billing session.
                        // The HTTP end was already attempted by the agent (billing_guard.rs).
                        // Log for audit trail; server-side billing state already cleaned up by
                        // the HTTP call to /api/v1/billing/{id}/end from the agent.
                        AgentMessage::SessionAutoEnded { pod_id, billing_session_id, reason } => {
                            tracing::warn!(
                                "[session-auto-end] Pod {} session {} auto-ended by agent: {}",
                                pod_id, billing_session_id, reason
                            );
                            log_pod_activity(
                                &state,
                                &pod_id,
                                "billing",
                                "Session Auto-Ended",
                                &format!("session={} reason={}", billing_session_id, reason),
                                "rc-agent",
                                None,
                            );
                        }
                        // SESSION-03 + FSM-04: Billing paused during crash recovery.
                        // Agent sends this BEFORE attempting relaunch. Server must pause the
                        // billing timer to ensure customer is never charged for recovery time.
                        AgentMessage::BillingPaused { pod_id, billing_session_id } => {
                            tracing::info!(
                                "[billing] Pod {} session {} billing paused (crash recovery)",
                                pod_id, billing_session_id
                            );
                            // FSM-04: Actually pause the billing timer via FSM transition
                            let timers = &state.billing.active_timers;
                            let mut guard = timers.write().await;
                            if let Some(timer) = guard.values_mut().find(|t| t.session_id == *billing_session_id) {
                                if let Err(e) = crate::billing_fsm::validate_transition(
                                    timer.status,
                                    crate::billing_fsm::BillingEvent::CrashPause,
                                ) {
                                    tracing::warn!("[billing] FSM rejected CrashPause for session {}: {}", billing_session_id, e);
                                } else {
                                    timer.status = rc_common::types::BillingSessionStatus::PausedGamePause;
                                    // BILL-06: Mark this as crash-recovery pause so recovery_pause_seconds increments
                                    timer.pause_reason = crate::billing::PauseReason::CrashRecovery;
                                    tracing::info!("[billing] Timer paused for session {} (FSM-04 crash recovery)", billing_session_id);
                                }
                            }
                            drop(guard);
                        }
                        // SESSION-03: Billing resumed after successful game relaunch.
                        AgentMessage::BillingResumed { pod_id, billing_session_id } => {
                            tracing::info!(
                                "[billing] Pod {} session {} billing resumed",
                                pod_id, billing_session_id
                            );
                        }
                        // Phase 50: Agent returns self-test probe results.
                        AgentMessage::PreFlightPassed { pod_id } => {
                            tracing::info!("Pod {} pre-flight checks passed", pod_id);
                            log_pod_activity(&state, pod_id, "system", "Pre-flight Passed", "All checks passed before session start", "agent", None);
                            // Phase 100 (STAFF-03): Clear maintenance state — pod is healthy.
                            {
                                let mut fleet = state.pod_fleet_health.write().await;
                                let store = fleet.entry(pod_id.clone()).or_default();
                                store.in_maintenance = false;
                                store.maintenance_failures.clear();
                            }
                        }
                        AgentMessage::PreFlightFailed { pod_id, failures, .. } => {
                            tracing::warn!("Pod {} pre-flight checks failed: {:?}", pod_id, failures);
                            log_pod_activity(&state, pod_id, "system", "Pre-flight Failed", &format!("Failures: {:?}", failures), "agent", None);
                            // Phase 100 (STAFF-03): Mark pod as in maintenance with failure details.
                            {
                                let mut fleet = state.pod_fleet_health.write().await;
                                let store = fleet.entry(pod_id.clone()).or_default();
                                store.in_maintenance = true;
                                store.maintenance_failures = failures.clone();
                            }
                        }
                        AgentMessage::SelfTestResult { pod_id, request_id, report } => {
                            tracing::info!(
                                "[self-test] Pod {} returned self-test results for request_id={}",
                                pod_id, request_id
                            );
                            let mut pending = state.pending_self_tests.write().await;
                            if let Some((_pod_id, tx)) = pending.remove(request_id.as_str()) {
                                let _ = tx.send(report.clone());
                            } else {
                                tracing::warn!("[self-test] Received SelfTestResult for unknown request_id: {}", request_id);
                            }
                        }
                        AgentMessage::ProcessViolation(violation) => {
                            let machine_id = violation.machine_id.clone();
                            let name = violation.name.clone();
                            let action = violation.action_taken.clone();
                            let ts = violation.timestamp.clone();
                            let now = chrono::Utc::now();

                            // Use debug for report_only violations (no action taken).
                            // warn floods the console when whitelist isn't configured.
                            if action == "reported" || action == "report_only" {
                                tracing::debug!(
                                    "[guard] Violation on {}: {} action={} ts={}",
                                    machine_id, name, action, ts
                                );
                            } else {
                                tracing::warn!(
                                    "[guard] Violation on {}: {} action={} ts={}",
                                    machine_id, name, action, ts
                                );
                            }

                            // Prefer registered_pod_id (authoritative WS key); fall back to machine_id
                            let pod_key = if let Some(pod_id) = &registered_pod_id {
                                pod_id.clone()
                            } else {
                                machine_id.replace('-', "_")
                            };

                            // Store violation and check for repeat offender
                            let should_escalate = {
                                let mut vmap = state.pod_violations.write().await;
                                let store = vmap.entry(pod_key.clone()).or_default();
                                let escalate = store.repeat_offender_check(violation, now);
                                store.push(violation.clone());
                                escalate
                            };

                            // Email escalation: 3 kills of same process within 5 minutes
                            if should_escalate {
                                let subject = format!(
                                    "GUARD ALERT: Repeat offender on {} — {}",
                                    machine_id, name
                                );
                                let body = format!(
                                    "Process '{}' has been killed 3+ times in the last 5 minutes on machine '{}'.\n\
                                     Last action: {}\nTimestamp: {}\n\n\
                                     Check C:\\RacingPoint\\process-guard.log on the affected machine.",
                                    name, machine_id, action, ts
                                );
                                let mut alerter = state.email_alerter.write().await;
                                alerter.send_alert(&pod_key, &subject, &body).await;
                            }
                        }
                        AgentMessage::ProcessGuardStatus { pod_id, scan_count, violation_count_total, violation_count_last_scan, last_scan_at, guard_active } => {
                            tracing::info!(
                                "[guard] Status from {}: scans={}, violations_total={}, violations_last_scan={}, last_scan={}, active={}",
                                pod_id, scan_count, violation_count_total, violation_count_last_scan, last_scan_at, guard_active
                            );
                        }
                        AgentMessage::IdleHealthFailed { pod_id, failures, consecutive_count, timestamp } => {
                            tracing::warn!(
                                "Pod {} idle health failed: {:?} ({} consecutive, at {})",
                                pod_id, failures, consecutive_count, timestamp
                            );
                            log_pod_activity(
                                &state,
                                pod_id,
                                "system",
                                "Idle Health Failed",
                                &format!("Failures: {:?}, consecutive: {}", failures, consecutive_count),
                                "agent",
                                None,
                            );
                            {
                                let mut fleet = state.pod_fleet_health.write().await;
                                let store = fleet.entry(pod_id.clone()).or_default();
                                store.idle_health_fail_count = *consecutive_count;
                                store.idle_health_failures = failures.clone();
                            }
                        }
                        AgentMessage::FlagCacheSync(payload) => {
                            tracing::info!(
                                "Pod {} requests flag sync (cached_version={})",
                                payload.pod_id, payload.cached_version
                            );
                            // Get current max flag version from state.feature_flags cache
                            let max_version = {
                                let flags = state.feature_flags.read().await;
                                flags.values().map(|f| f.version).max().unwrap_or(0)
                            };
                            // If pod is stale, send full flag state with per-pod override resolution
                            if payload.cached_version < max_version as u64 {
                                let flags = state.feature_flags.read().await;
                                let pod_id = &payload.pod_id;
                                let flag_map: std::collections::HashMap<String, bool> = flags
                                    .iter()
                                    .map(|(name, row)| {
                                        let effective = serde_json::from_str::<std::collections::HashMap<String, bool>>(&row.overrides)
                                            .ok()
                                            .and_then(|ovr| ovr.get(pod_id).copied())
                                            .unwrap_or(row.enabled);
                                        (name.clone(), effective)
                                    })
                                    .collect();
                                let sync_payload = rc_common::types::FlagSyncPayload {
                                    flags: flag_map,
                                    version: max_version as u64,
                                };
                                let _ = cmd_tx
                                    .send(CoreToAgentMessage::FlagSync(sync_payload))
                                    .await;
                            }
                            // Replay pending config pushes for this pod.
                            // IMPORTANT: Do NOT pass payload.cached_version as a sequence filter.
                            // cached_version is a FLAG version counter — it has nothing to do with
                            // config push sequences. replay_pending_config_pushes uses
                            // status != 'acked' as the filter instead.
                            crate::config_push::replay_pending_config_pushes(
                                &state,
                                &payload.pod_id,
                                &cmd_tx,
                            )
                            .await;
                        }
                        AgentMessage::ConfigAck(payload) => {
                            tracing::info!(
                                "Pod {} acked config push seq={} accepted={}",
                                payload.pod_id, payload.sequence, payload.accepted
                            );
                            // Update config_push_queue: mark acked
                            let ack_result = sqlx::query(
                                "UPDATE config_push_queue SET status = 'acked', acked_at = datetime('now') \
                                 WHERE pod_id = ? AND seq_num = ?",
                            )
                            .bind(&payload.pod_id)
                            .bind(payload.sequence as i64)
                            .execute(&state.db)
                            .await;
                            if let Err(e) = ack_result {
                                tracing::warn!(
                                    "Failed to update config_push_queue ack for pod {} seq {}: {}",
                                    payload.pod_id, payload.sequence, e
                                );
                            }
                            // Update config_audit_log pods_acked — find by seq_num (deterministic lookup).
                            // IMPORTANT: Do NOT use ORDER BY id DESC LIMIT 1 — non-deterministic under
                            // concurrent pushes. The seq_num column was added specifically for this lookup.
                            let audit_lookup = sqlx::query_scalar::<_, i64>(
                                "SELECT id FROM config_audit_log WHERE entity_type = 'config' AND seq_num = ?",
                            )
                            .bind(payload.sequence as i64)
                            .fetch_optional(&state.db)
                            .await;
                            if let Ok(Some(audit_id)) = audit_lookup {
                                let current_acked: String = sqlx::query_scalar(
                                    "SELECT pods_acked FROM config_audit_log WHERE id = ?",
                                )
                                .bind(audit_id)
                                .fetch_one(&state.db)
                                .await
                                .unwrap_or_else(|_| "[]".to_string());
                                let mut acked: Vec<String> =
                                    serde_json::from_str(&current_acked).unwrap_or_default();
                                if !acked.contains(&payload.pod_id) {
                                    acked.push(payload.pod_id.clone());
                                }
                                let _ = sqlx::query(
                                    "UPDATE config_audit_log SET pods_acked = ? WHERE id = ?",
                                )
                                .bind(
                                    serde_json::to_string(&acked)
                                        .unwrap_or_else(|_| "[]".to_string()),
                                )
                                .bind(audit_id)
                                .execute(&state.db)
                                .await;
                            } else {
                                tracing::warn!(
                                    "No audit log entry found for config push seq={}",
                                    payload.sequence
                                );
                            }
                        }
                        // Phase 206 (OBS-04): Sentinel file change on pod.
                        // 1. Update active_sentinels in fleet health store.
                        // 2. Broadcast DashboardEvent::SentinelChanged for real-time visibility.
                        // 3. MAINTENANCE_MODE creation fires WhatsApp alert to Uday (rate-limited).
                        AgentMessage::SentinelChange { pod_id, file, action, timestamp } => {
                            tracing::info!(
                                target: "fleet",
                                pod = %pod_id,
                                sentinel = %file,
                                action = %action,
                                timestamp = %timestamp,
                                "sentinel file change received"
                            );

                            // Parse pod number from pod_id (e.g. "pod_3" -> 3)
                            let pod_number: u32 = pod_id.strip_prefix("pod_")
                                .and_then(|n| n.parse().ok())
                                .unwrap_or(0);

                            // Update fleet health store (active_sentinels field)
                            let updated_sentinels = {
                                let mut fleet = state.pod_fleet_health.write().await;
                                let store = fleet.entry(pod_id.clone()).or_default();
                                crate::fleet_health::update_sentinel(store, file, action);
                                store.active_sentinels.clone()
                            };

                            // REQUIRED: Broadcast to dashboard via DashboardEvent::SentinelChanged.
                            // Per plan locked decision — real-time WS broadcast, not just fleet poll.
                            let _ = state.dashboard_tx.send(
                                DashboardEvent::SentinelChanged {
                                    pod_id: pod_id.clone(),
                                    pod_number,
                                    file: file.clone(),
                                    action: action.clone(),
                                    timestamp: timestamp.clone(),
                                    active_sentinels: updated_sentinels,
                                }
                            );

                            // OBS-01: MAINTENANCE_MODE creation → WhatsApp alert to Uday
                            if file == "MAINTENANCE_MODE" && action == "created" {
                                let cooldown_key = format!("sentinel_{}_{}", file, pod_number);
                                if check_sentinel_cooldown(&cooldown_key) {
                                    let alert_msg = format!(
                                        "[ALERT] Pod {} entered MAINTENANCE_MODE at {} (IST). \
                                        Sentinel file created — all restarts are now blocked. \
                                        Check fleet health or clear sentinel to recover.",
                                        pod_number, timestamp
                                    );
                                    crate::whatsapp_alerter::send_whatsapp(&state.config, &alert_msg).await;
                                    tracing::warn!(
                                        target: "state",
                                        pod = pod_number,
                                        "MAINTENANCE_MODE WhatsApp alert sent to Uday"
                                    );
                                } else {
                                    tracing::debug!(
                                        target: "state",
                                        pod = pod_number,
                                        "MAINTENANCE_MODE WhatsApp alert suppressed (5-min cooldown active)"
                                    );
                                }
                            }
                        }
                        // ─── Staff Diagnostic Bridge (v27.0) ──────────────────────
                        AgentMessage::DiagnosticResult {
                            correlation_id,
                            tier,
                            outcome,
                            root_cause,
                            fix_action,
                            fix_type,
                            confidence,
                            fix_applied,
                            problem_hash,
                            summary,
                        } => {
                            tracing::info!(
                                target: "debug-bridge",
                                correlation_id = %correlation_id,
                                tier = tier,
                                outcome = %outcome,
                                fix_applied = fix_applied,
                                "DiagnosticResult received from pod"
                            );

                            // Store in debug_resolutions for kiosk RAG
                            if outcome == "fixed" || !root_cause.is_empty() {
                                let res_id = uuid::Uuid::new_v4().to_string();
                                let resolution_text = if !summary.is_empty() {
                                    format!("[Tier {}] {}", tier, summary)
                                } else {
                                    format!("[Tier {}] {}: {}", tier, outcome, fix_action)
                                };
                                let effectiveness = if outcome == "fixed" { 4 } else { 2 };

                                // Find the incident category using correlation_id (MMA R4-1 fix)
                                // correlation_id was passed as incident_id in DiagnosticRequest
                                let category: String = sqlx::query_scalar(
                                    "SELECT category FROM debug_incidents WHERE id = ?"
                                )
                                .bind(correlation_id.as_str())
                                .fetch_optional(&state.db)
                                .await
                                .unwrap_or(None)
                                .unwrap_or_else(|| "unknown".to_string());

                                let _ = sqlx::query(
                                    "INSERT INTO debug_resolutions (id, incident_id, category, resolution_text, effectiveness) \
                                     VALUES (?, ?, ?, ?, ?)"
                                )
                                .bind(&res_id)
                                .bind(&correlation_id)
                                .bind(&category)
                                .bind(&resolution_text)
                                .bind(effectiveness)
                                .execute(&state.db)
                                .await;

                                // Log to activity feed
                                let log_detail = format!(
                                    "Tier {} diagnosis: {} (confidence: {:.0}%, applied: {})",
                                    tier, summary, confidence * 100.0, fix_applied
                                );
                                let log_pod = registered_pod_id.as_deref().unwrap_or("unknown");
                                crate::activity_log::log_pod_activity(
                                    &state,
                                    log_pod,
                                    "race_engineer",
                                    "Pod Diagnosis Complete",
                                    &log_detail,
                                    "race_engineer",
                                    None,
                                );
                            }

                            // ─── Fleet Distribution (v27.0) ──────────────────────────
                            // Only broadcast to fleet for high-confidence KB solutions (Tier 2+).
                            // Tier 1 deterministic fixes are pod-local and should NOT be fleet-broadcast
                            // (MMA Round 1 P2: single-pod fixes must not propagate to other pods).
                            let is_fleet_worthy = *tier >= 2 && *confidence >= 0.8;
                            if outcome == "fixed" && is_fleet_worthy && !problem_hash.is_empty() {
                                tracing::info!(
                                    target: "debug-bridge",
                                    problem_hash = %problem_hash,
                                    confidence = confidence,
                                    "Broadcasting solution to fleet via MeshSolutionBroadcast"
                                );

                                let broadcast_msg = CoreToAgentMessage::MeshSolutionBroadcast {
                                    problem_hash: problem_hash.clone(),
                                    problem_key: format!("staff_{}", correlation_id),
                                    root_cause: root_cause.clone(),
                                    fix_action: fix_action.clone(),
                                    fix_type: fix_type.clone(),
                                    confidence: *confidence,
                                    source_node: registered_pod_id.clone().unwrap_or_else(|| "unknown".to_string()),
                                    promotion_status: "fleet_verified".to_string(),
                                };

                                // Send to OTHER connected pods (exclude origin) — clone senders first.
                                // If origin pod is unknown (None), skip broadcast entirely to avoid
                                // sending to ALL pods including origin (MMA OpenRouter fix: empty string matched nobody).
                                let origin_pod = match &registered_pod_id {
                                    Some(id) => id.clone(),
                                    None => {
                                        tracing::warn!(target: "debug-bridge", "Fleet broadcast skipped — origin pod not registered");
                                        String::new() // will skip the broadcast below
                                    }
                                };
                                let sender_snapshot: Vec<_> = if origin_pod.is_empty() {
                                    vec![] // don't broadcast if we don't know who sent it
                                } else {
                                    let senders = state.agent_senders.read().await;
                                    senders.iter()
                                        .filter(|(id, _)| **id != origin_pod)
                                        .map(|(id, s)| (id.clone(), s.clone()))
                                        .collect()
                                }; // lock dropped here
                                for (target_pod_id, sender) in &sender_snapshot {
                                    if let Err(e) = sender.send(broadcast_msg.clone()).await {
                                        tracing::debug!(
                                            target: "debug-bridge",
                                            pod = %target_pod_id,
                                            "Failed to broadcast solution: {}",
                                            e
                                        );
                                    }
                                }
                            }

                            // If incident was resolved, update its status using correlation_id
                            // (which equals incident_id — set in create_debug_incident, MMA R4-1 fix)
                            if outcome == "fixed" && *fix_applied {
                                match sqlx::query(
                                    "UPDATE debug_incidents SET status = 'resolved', resolved_at = datetime('now') \
                                     WHERE id = ? AND status = 'open'"
                                )
                                .bind(correlation_id.as_str())
                                .execute(&state.db)
                                .await {
                                    Ok(result) if result.rows_affected() == 0 => {
                                        tracing::debug!(
                                            target: "debug-bridge",
                                            correlation_id = %correlation_id,
                                            "Incident already closed or not found — skipping resolution"
                                        );
                                    }
                                    Ok(_) => {
                                        tracing::info!(
                                            target: "debug-bridge",
                                            correlation_id = %correlation_id,
                                            "Incident auto-resolved by tier engine diagnosis"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            target: "debug-bridge",
                                            correlation_id = %correlation_id,
                                            "Failed to resolve incident: {}",
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        // ─── BILL-01: Inactivity Alert ───────────────────────────────────────
                        AgentMessage::InactivityAlert { pod_id, idle_seconds } => {
                            // Look up driver name and session id from billing timers
                            let (driver_name, session_id) = {
                                let timers = state.billing.active_timers.read().await;
                                if let Some(timer) = timers.get(pod_id.as_str()) {
                                    (timer.driver_name.clone(), timer.session_id.clone())
                                } else {
                                    (String::from("unknown"), String::from("unknown"))
                                }
                            }; // lock dropped
                            tracing::warn!(
                                "BILL-01: Pod {} idle for {}s — staff alerted (driver={}, session={})",
                                pod_id, idle_seconds, driver_name, session_id
                            );
                            log_pod_activity(
                                &state,
                                pod_id,
                                "billing",
                                "Customer Idle",
                                &format!("No input for {}s — staff alert sent", idle_seconds),
                                "agent",
                                None,
                            );
                            // Broadcast to staff dashboard — do NOT auto-end the session
                            let _ = state.dashboard_tx.send(DashboardEvent::InactivityAlert {
                                pod_id: pod_id.clone(),
                                idle_seconds: *idle_seconds,
                                driver_name,
                                session_id,
                            });
                        }

                        // v29.0: Extended hardware telemetry for preventive maintenance
                        AgentMessage::ExtendedTelemetry {
                            pod_id,
                            gpu_temp_celsius,
                            cpu_usage_pct,
                            gpu_usage_pct,
                            memory_usage_pct,
                            disk_usage_pct,
                            process_handle_count,
                            network_latency_ms,
                            system_uptime_secs,
                            ..
                        } => {
                            tracing::debug!(
                                pod = %pod_id,
                                gpu_temp = ?gpu_temp_celsius,
                                cpu = ?cpu_usage_pct,
                                gpu = ?gpu_usage_pct,
                                mem = ?memory_usage_pct,
                                disk = ?disk_usage_pct,
                                handles = ?process_handle_count,
                                latency = ?network_latency_ms,
                                uptime = ?system_uptime_secs,
                                "v29.0: Extended telemetry received"
                            );
                            // Store in telemetry_store for trend analysis
                            crate::telemetry_store::store_extended_telemetry(
                                &state,
                                pod_id,
                                &agent_msg,
                            );
                        }

                        // ─── Meshed Intelligence gossip (v26.0) ─────────────────────
                        AgentMessage::MeshSolutionAnnounce { .. }
                        | AgentMessage::MeshSolutionRequest { .. }
                        | AgentMessage::MeshExperimentAnnounce { .. }
                        | AgentMessage::MeshHeartbeat { .. } => {
                            crate::mesh_handler::handle_mesh_message(
                                &state, &agent_msg, &cmd_tx,
                            ).await;
                        }

                        // ─── Model Evaluation Sync (EVAL-03 / Phase 290) ─────────────
                        AgentMessage::ModelEvalSync { pod_id, records } => {
                            tracing::debug!(
                                target: "racecontrol::ws",
                                pod_id = %pod_id,
                                record_count = records.len(),
                                "EVAL-03: received model eval sync from pod"
                            );
                            for rec in records.iter() {
                                if let Err(e) = crate::fleet_kb::insert_eval_record(&state.db, rec).await {
                                    tracing::warn!(
                                        target: "racecontrol::ws",
                                        error = %e,
                                        model_id = %rec.model_id,
                                        "EVAL-03: failed to insert eval record"
                                    );
                                }
                            }
                        }

                        // ─── Model Reputation Sync (MREP-04 / Phase 292) ─────────────
                        AgentMessage::ModelReputationSync { pod_id, rows } => {
                            tracing::debug!(
                                target: "racecontrol::ws",
                                pod_id = %pod_id,
                                model_count = rows.len(),
                                "MREP-04: received model reputation sync from pod"
                            );
                            for row in rows.iter() {
                                if let Err(e) = crate::fleet_kb::upsert_reputation(&state.db, row, &pod_id).await {
                                    tracing::warn!(
                                        target: "racecontrol::ws",
                                        error = %e,
                                        model_id = %row.model_id,
                                        "MREP-04: failed to upsert reputation row"
                                    );
                                }
                            }
                        }

                        // ─── Tier 5 WhatsApp escalation (v274) ──────────────────────
                        AgentMessage::EscalationRequest(payload) => {
                            tracing::warn!(
                                target: "racecontrol::ws",
                                pod_id = %payload.pod_id,
                                incident_id = %payload.incident_id,
                                severity = %payload.severity,
                                "Received Tier 5 escalation from pod"
                            );
                            let escalation = state.whatsapp_escalation.clone();
                            let payload_owned = payload.clone();
                            tokio::spawn(async move {
                                escalation.handle_escalation(payload_owned).await;
                            });
                        }

                        // ─── Experience Score Report (CX-06) ─────────────────────────
                        AgentMessage::ExperienceScoreReport {
                            pod_id,
                            total_score,
                            status,
                            ..
                        } => {
                            let pod_id_owned = pod_id.clone();
                            let score = *total_score;
                            let status_owned = status.clone();
                            tracing::debug!(
                                target: "racecontrol::ws",
                                pod_id = %pod_id,
                                score = score,
                                status = %status,
                                "Received experience score report from pod"
                            );
                            let mut fleet = state.pod_fleet_health.write().await;
                            let store = fleet.entry(pod_id_owned).or_default();
                            store.experience_score = Some(score);
                            store.experience_status = Some(status_owned);
                        }

                        // Phase 306: JwtAck — agent confirmed JWT receipt
                        AgentMessage::JwtAck { pod_id } => {
                            tracing::info!("Phase 306: JWT ack from pod {}", pod_id);
                        }

                        _ => { /* catch-all for future protocol additions */ }
                    }
                }
                Err(e) => {
                    tracing::warn!("Invalid agent message: {}", e);
                }
            }
        }
    }

    // Cleanup: only remove sender and mark offline if THIS connection is still the active one.
    // A newer connection may have already replaced us in agent_senders/agent_conn_ids,
    // in which case this is a stale zombie disconnect and we must NOT touch the pod state.
    if let Some(pod_id) = &registered_pod_id {
        let current_conn_id = state.agent_conn_ids.read().await.get(pod_id).copied();
        let is_stale = current_conn_id.is_some() && current_conn_id != Some(conn_id);

        if is_stale {
            tracing::info!(
                "Stale WebSocket cleanup for pod {} (conn_id={}, current={}). Skipping.",
                pod_id, conn_id, current_conn_id.unwrap()
            );
        } else {
            state.agent_senders.write().await.remove(pod_id);
            state.agent_conn_ids.write().await.remove(pod_id);

            // Clear fleet health version/uptime on ungraceful disconnect.
            {
                let mut fleet = state.pod_fleet_health.write().await;
                if let Some(store) = fleet.get_mut(pod_id.as_str()) {
                    crate::fleet_health::clear_on_disconnect(store);
                }
            }

            // Sweep pending WS command entries for this pod (they use "pod_X:" prefix)
            {
                let prefix = format!("{}:", pod_id);
                let mut pending = state.pending_ws_execs.write().await;
                let stale_keys: Vec<String> = pending.keys()
                    .filter(|k| k.starts_with(&prefix))
                    .cloned()
                    .collect();
                for key in &stale_keys {
                    pending.remove(key);
                }
                if !stale_keys.is_empty() {
                    tracing::info!("Cleaned {} pending WS command(s) for disconnected {}", stale_keys.len(), pod_id);
                }
            }

            // Clean up pending self-tests for disconnected pod
            {
                let mut pending = state.pending_self_tests.write().await;
                let before = pending.len();
                pending.retain(|_req_id, (pid, _tx)| pid != pod_id);
                let removed = before - pending.len();
                if removed > 0 {
                    tracing::info!("Cleaned {} pending self-test(s) for disconnected {}", removed, pod_id);
                }
            }

            let has_active_billing = state
                .billing
                .active_timers
                .read()
                .await
                .contains_key(pod_id.as_str());

            // Mark pod offline on ungraceful disconnect (WebSocket dropped without Disconnect message)
            if let Some(pod) = state.pods.write().await.get_mut(pod_id.as_str()) {
                if pod.status != rc_common::types::PodStatus::Offline
                    && pod.status != rc_common::types::PodStatus::Disabled
                {
                    tracing::warn!("Pod {} WebSocket dropped without Disconnect (conn_id={}) — marking Offline", pod_id, conn_id);
                    log_pod_activity(&state, pod_id, "system", "Pod Disconnected", &format!("WebSocket dropped unexpectedly (conn_id={})", conn_id), "core", None);
                    pod.status = rc_common::types::PodStatus::Offline;
                    pod.driving_state = Some(rc_common::types::DrivingState::NoDevice);
                    // Preserve game_state if billing is active — agent will resync on reconnect
                    if !has_active_billing {
                        pod.game_state = Some(GameState::Idle);
                        pod.current_game = None;
                    }
                    let _ = state.dashboard_tx.send(DashboardEvent::PodUpdate(pod.clone()));
                }
            }

            // MMA-P1-FIX: Sync offline status to DB — preserves disabled/maintenance
            // status (MMA F-02), awaited for ordering guarantees (MMA F-01).
            if let Err(e) = sqlx::query(
                "UPDATE pods SET status = 'offline', last_seen = datetime('now')
                 WHERE id = ? AND status NOT IN ('disabled', 'maintenance')"
            )
            .bind(pod_id)
            .execute(&state.db)
            .await {
                tracing::warn!("Failed to sync pod {} disconnect to DB: {}", pod_id, e);
            }

            billing::update_driving_state(&state, pod_id, rc_common::types::DrivingState::NoDevice)
                .await;
        }
    }

    send_task.abort();
    tracing::info!("Pod agent disconnected");
}

async fn handle_dashboard(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();

    tracing::info!("Dashboard client connected");

    // Send current pod list on connect (only physical pods 1-8, exclude POS/utility agents)
    let pods = state.pods.read().await;
    let pod_list: Vec<_> = pods.values().filter(|p| p.number >= 1 && p.number <= 8).cloned().collect();
    drop(pods);

    let init_msg = DashboardEvent::PodList(pod_list);
    if let Ok(json) = serde_json::to_string(&init_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active billing sessions on connect
    let rate_tiers = state.billing.rate_tiers.read().await;
    let timers = state.billing.active_timers.read().await;
    let billing_list: Vec<_> = timers.values().map(|t| t.to_info(&rate_tiers)).collect();
    drop(timers);
    drop(rate_tiers);

    let billing_msg = DashboardEvent::BillingSessionList(billing_list);
    if let Ok(json) = serde_json::to_string(&billing_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active game sessions on connect
    let games = state.game_launcher.active_games.read().await;
    let game_list: Vec<_> = games.values().map(|g| g.to_info()).collect();
    drop(games);

    let game_msg = DashboardEvent::GameSessionList(game_list);
    if let Ok(json) = serde_json::to_string(&game_msg) {
        let _ = sender.send(Message::Text(json.into())).await;
    }

    // Send active AC server sessions on connect
    {
        let instances = state.ac_server.instances.read().await;
        for inst in instances.values() {
            if matches!(inst.status, rc_common::types::AcServerStatus::Running | rc_common::types::AcServerStatus::Starting) {
                let msg = DashboardEvent::AcServerUpdate(inst.to_info());
                if let Ok(json) = serde_json::to_string(&msg) {
                    let _ = sender.send(Message::Text(json.into())).await;
                }
            }
        }
    }

    // Send AC preset list on connect
    if let Ok(presets) = ac_server::list_presets(&state).await {
        let msg = DashboardEvent::AcPresetList(presets);
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    // Send recent activity log on connect (last 100 entries)
    {
        let rows: Vec<(String, String, i64, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, pod_id, pod_number, timestamp, category, action, details, source
             FROM pod_activity_log ORDER BY timestamp DESC LIMIT 100"
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let entries: Vec<rc_common::types::PodActivityEntry> = rows.into_iter().map(|r| {
            rc_common::types::PodActivityEntry {
                id: r.0, pod_id: r.1, pod_number: r.2 as u32, timestamp: r.3,
                category: r.4, action: r.5, details: r.6, source: r.7,
            }
        }).collect();

        let msg = DashboardEvent::PodActivityList(entries);
        if let Ok(json) = serde_json::to_string(&msg) {
            let _ = sender.send(Message::Text(json.into())).await;
        }
    }

    // Subscribe to broadcast events
    let mut rx = state.dashboard_tx.subscribe();

    // Forward broadcast events to this dashboard client (filter non-physical pods)
    let send_task = tokio::spawn(async move {
        // Phase 254: Debounce RecordBroken broadcasts — max 1 per second per (track, sim_type)
        let mut record_debounce: HashMap<(String, String), Instant> = HashMap::new();

        while let Ok(event) = rx.recv().await {
            // Phase 254: Debounce RecordBroken events per (track, sim_type) — max 1/sec
            if let DashboardEvent::RecordBroken { ref track, ref sim_type, .. } = event {
                let key = (track.clone(), sim_type.clone());
                let now = Instant::now();
                if let Some(last) = record_debounce.get(&key) {
                    if now.duration_since(*last).as_secs() < 1 {
                        continue;
                    }
                }
                record_debounce.insert(key, now);
            }

            // Skip PodUpdate for non-physical pods (e.g. POS PC registering as pod 9)
            let filtered = match &event {
                DashboardEvent::PodUpdate(pod) if pod.number < 1 || pod.number > 8 => continue,
                DashboardEvent::PodList(pods) => {
                    let physical: Vec<_> = pods.iter().filter(|p| p.number >= 1 && p.number <= 8).cloned().collect();
                    DashboardEvent::PodList(physical)
                }
                _ => event,
            };
            if let Ok(json) = serde_json::to_string(&filtered) {
                if sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Handle incoming commands from dashboard
    let cmd_state = state.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<DashboardCommand>(&text) {
                    Ok(cmd) => match &cmd {
                        DashboardCommand::LaunchGame { .. }
                        | DashboardCommand::StopGame { .. } => {
                            let _ = game_launcher::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::StartAcSession { .. }
                        | DashboardCommand::StopAcSession { .. }
                        | DashboardCommand::SaveAcPreset { .. }
                        | DashboardCommand::DeleteAcPreset { .. }
                        | DashboardCommand::LoadAcPreset { .. } => {
                            ac_server::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::AssignCustomer { .. }
                        | DashboardCommand::CancelAssignment { .. } => {
                            auth::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                        DashboardCommand::SetCameraMode { mode, enabled } => {
                            if let Some(en) = enabled {
                                ac_camera::set_enabled(&cmd_state, *en).await;
                            }
                            if !mode.is_empty() {
                                let cam_mode = match mode.as_str() {
                                    "closest_cycle" => ac_camera::CameraMode::ClosestCycle,
                                    "leader" => ac_camera::CameraMode::Leader,
                                    "closest" => ac_camera::CameraMode::Closest,
                                    "cycle" => ac_camera::CameraMode::Cycle,
                                    "off" => ac_camera::CameraMode::Off,
                                    _ => ac_camera::CameraMode::ClosestCycle,
                                };
                                ac_camera::set_mode(&cmd_state, cam_mode).await;
                            }
                        }
                        DashboardCommand::DeployPod { pod_id, binary_url } => {
                            // Look up pod IP
                            let pod_ip = {
                                let pods = cmd_state.pods.read().await;
                                pods.get(pod_id).map(|p| p.ip_address.clone())
                            };
                            if let Some(pod_ip) = pod_ip {
                                // Check no active deploy in progress
                                let is_active = {
                                    let ds = cmd_state.pod_deploy_states.read().await;
                                    ds.get(pod_id).map(|s| s.is_active()).unwrap_or(false)
                                };
                                if !is_active {
                                    let deploy_state = Arc::clone(&cmd_state);
                                    let deploy_pod_id = pod_id.clone();
                                    let deploy_pod_ip = pod_ip;
                                    let deploy_url = binary_url.clone();
                                    tokio::spawn(async move {
                                        crate::deploy::deploy_pod(
                                            deploy_state,
                                            deploy_pod_id,
                                            deploy_pod_ip,
                                            deploy_url,
                                        )
                                        .await;
                                    });
                                } else {
                                    tracing::warn!(
                                        "DeployPod [{}]: deploy already in progress — ignoring",
                                        pod_id
                                    );
                                }
                            } else {
                                tracing::warn!("DeployPod: unknown pod_id {}", pod_id);
                            }
                        }
                        DashboardCommand::DeployRolling { binary_url } => {
                            // Rolling deploy via kiosk WebSocket command.
                            // Delegates to deploy_rolling() which handles:
                            //   - Canary-first (pod_8), halt on canary failure
                            //   - WaitingSession for pods with active billing
                            //   - Session-end hook triggers deferred deploys
                            let deploy_state = Arc::clone(&cmd_state);
                            let deploy_url = binary_url.clone();
                            tokio::spawn(async move {
                                // Dashboard-initiated deploy: no force override (DEPLOY-03), actor="dashboard"
                                if let Err(e) = crate::deploy::deploy_rolling(deploy_state, deploy_url, false, "dashboard").await {
                                    tracing::error!("Rolling deploy via dashboard failed: {}", e);
                                }
                            });
                        }
                        DashboardCommand::CancelDeploy { pod_id } => {
                            // Mark the deploy state as Failed to signal cancellation.
                            // The running deploy_pod() task checks is_cancelled() at each step
                            // and exits early if it finds a Failed state.
                            let mut deploy_states = cmd_state.pod_deploy_states.write().await;
                            if let Some(ds) = deploy_states.get(pod_id) {
                                if ds.is_active() {
                                    let cancel_state = rc_common::types::DeployState::Failed {
                                        reason: "Cancelled by staff".to_string(),
                                    };
                                    deploy_states
                                        .insert(pod_id.clone(), cancel_state.clone());
                                    let _ = cmd_state.dashboard_tx.send(
                                        rc_common::protocol::DashboardEvent::DeployProgress {
                                            pod_id: pod_id.clone(),
                                            state: cancel_state,
                                            message: "Deploy cancelled by staff".to_string(),
                                            timestamp: chrono::Utc::now().to_rfc3339(),
                                        },
                                    );
                                    tracing::info!(
                                        "Deploy [{}]: cancelled by staff via dashboard",
                                        pod_id
                                    );
                                }
                            }
                        }
                        _ => {
                            billing::handle_dashboard_command(&cmd_state, cmd).await;
                        }
                    },
                    Err(e) => {
                        tracing::debug!("Non-command dashboard message: {}", e);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
    tracing::info!("Dashboard client disconnected");
}

/// WebSocket endpoint for AI-to-AI messaging (Bono ↔ James)
pub async fn ai_ws(
    Query(params): Query<WsAuthParams>,
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    if !verify_ws_token(&state, &params.token) {
        tracing::warn!("WS AI channel connection rejected — invalid or missing token");
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(ws.on_upgrade(|socket| handle_ai(socket, state)))
}

async fn handle_ai(socket: WebSocket, state: Arc<AppState>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    tracing::info!("AI channel: connection attempt");

    // First message must be Auth
    let identity = match ws_receiver.next().await {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<AiChannelMessage>(&text) {
                Ok(AiChannelMessage::Auth { secret, identity }) => {
                    let expected = state.config.cloud.terminal_secret.as_deref();
                    if expected.is_some() && expected != Some(&secret) {
                        let fail = AiChannelMessage::AuthFailed {
                            reason: "Invalid secret".to_string(),
                        };
                        if let Ok(json) = serde_json::to_string(&fail) {
                            let _ = ws_sender.send(Message::Text(json.into())).await;
                        }
                        tracing::warn!("AI channel: auth failed for {}", identity);
                        return;
                    }
                    identity
                }
                _ => {
                    tracing::warn!("AI channel: first message was not Auth");
                    return;
                }
            }
        }
        _ => {
            tracing::warn!("AI channel: connection closed before auth");
            return;
        }
    };

    // Send AuthOk
    let auth_ok = AiChannelMessage::AuthOk {
        identity: identity.clone(),
    };
    if let Ok(json) = serde_json::to_string(&auth_ok) {
        if ws_sender.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }

    tracing::info!("AI channel: {} authenticated", identity);

    // Create mpsc channel for sending messages to this peer
    let (msg_tx, mut msg_rx) = mpsc::channel::<AiChannelMessage>(256);

    // Store sender so HTTP endpoints can push via WS
    *state.ai_peer_tx.write().await = Some(msg_tx.clone());

    // Deliver any pending messages from DB
    let pending: Vec<(String, String, String, String, Option<String>, Option<String>, String)> =
        sqlx::query_as(
            "SELECT id, sender, content, message_type, metadata, in_reply_to, created_at
             FROM ai_messages WHERE recipient = ? AND status = 'pending'
             ORDER BY created_at ASC",
        )
        .bind(&identity)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

    for (id, sender, content, msg_type, metadata, in_reply_to, created_at) in &pending {
        let msg = AiChannelMessage::Message {
            id: id.clone(),
            sender: sender.clone(),
            content: content.clone(),
            message_type: msg_type.clone(),
            metadata: metadata.as_ref().and_then(|m| serde_json::from_str(m).ok()),
            in_reply_to: in_reply_to.clone(),
            created_at: created_at.clone(),
        };
        if let Ok(json) = serde_json::to_string(&msg) {
            if ws_sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
            // Mark as delivered
            let _ = sqlx::query(
                "UPDATE ai_messages SET status = 'delivered', channel = 'ws',
                 delivered_at = datetime('now') WHERE id = ?",
            )
            .bind(id)
            .execute(&state.db)
            .await;
        }
    }

    if !pending.is_empty() {
        tracing::info!("AI channel: delivered {} pending messages to {}", pending.len(), identity);
    }

    // Spawn task to forward mpsc messages to WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    // Listen for incoming messages from peer
    while let Some(Ok(msg)) = ws_receiver.next().await {
        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<AiChannelMessage>(&text) {
                    Ok(ai_msg) => match &ai_msg {
                        AiChannelMessage::Message {
                            id,
                            sender,
                            content,
                            message_type,
                            metadata,
                            in_reply_to,
                            created_at,
                        } => {
                            let recipient = if sender == "james" { "bono" } else { "james" };
                            let meta_str = metadata.as_ref().map(|v| v.to_string());
                            let _ = sqlx::query(
                                "INSERT OR IGNORE INTO ai_messages
                                 (id, sender, recipient, content, message_type, metadata, channel, status, in_reply_to, created_at)
                                 VALUES (?, ?, ?, ?, ?, ?, 'ws', 'delivered', ?, ?)",
                            )
                            .bind(id)
                            .bind(sender)
                            .bind(recipient)
                            .bind(content)
                            .bind(message_type)
                            .bind(&meta_str)
                            .bind(in_reply_to)
                            .bind(created_at)
                            .execute(&state.db)
                            .await;

                            let _ = state.dashboard_tx.send(DashboardEvent::AiMessage {
                                id: id.clone(),
                                sender: sender.clone(),
                                recipient: recipient.to_string(),
                                content: content.clone(),
                                message_type: message_type.clone(),
                                created_at: created_at.clone(),
                            });

                            // Send Ack
                            let _ = msg_tx
                                .send(AiChannelMessage::Ack {
                                    message_id: id.clone(),
                                })
                                .await;
                        }
                        AiChannelMessage::Ack { message_id } => {
                            let _ = sqlx::query(
                                "UPDATE ai_messages SET status = 'delivered', delivered_at = datetime('now')
                                 WHERE id = ? AND status = 'pending'",
                            )
                            .bind(message_id)
                            .execute(&state.db)
                            .await;
                        }
                        AiChannelMessage::MarkRead { message_id } => {
                            let _ = sqlx::query(
                                "UPDATE ai_messages SET status = 'read', read_at = datetime('now') WHERE id = ?",
                            )
                            .bind(message_id)
                            .execute(&state.db)
                            .await;
                        }
                        AiChannelMessage::Ping => {
                            let _ = msg_tx.send(AiChannelMessage::Pong).await;
                        }
                        _ => {}
                    },
                    Err(e) => {
                        tracing::warn!("AI channel: invalid message: {}", e);
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // Cleanup
    *state.ai_peer_tx.write().await = None;
    send_task.abort();
    tracing::info!("AI channel: {} disconnected", identity);
}

/// Send a shell command to a pod agent via WebSocket and wait for the result.
///
/// Uses pod-prefixed request_id (e.g. "pod_3:uuid") so disconnect cleanup
/// can identify and remove stale entries.
///
/// Returns (success, stdout, stderr) or an error string.
pub async fn ws_exec_on_pod(
    state: &std::sync::Arc<crate::state::AppState>,
    pod_id: &str,
    cmd: &str,
    timeout_ms: u64,
) -> Result<(bool, String, String), String> {
    let request_id = format!("{}:{}", pod_id, uuid::Uuid::new_v4());
    let (tx, rx) = tokio::sync::oneshot::channel();

    // Register pending response
    state.pending_ws_execs.write().await.insert(request_id.clone(), tx);

    // Clone the sender, drop the lock immediately (avoid holding lock across await)
    let sender = {
        let senders = state.agent_senders.read().await;
        senders.get(pod_id).cloned()
            .ok_or_else(|| format!("Pod {} not connected via WebSocket", pod_id))?
    };

    // Send the command
    if sender.send(rc_common::protocol::CoreToAgentMessage::Exec {
        request_id: request_id.clone(),
        cmd: cmd.to_string(),
        timeout_ms,
    }).await.is_err() {
        state.pending_ws_execs.write().await.remove(&request_id);
        return Err(format!("Failed to send command to pod {}", pod_id));
    }

    // Wait for response with buffer timeout (command timeout + 5s for WS round trip)
    match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms + 5000),
        rx,
    ).await {
        Ok(Ok(result)) => Ok((result.success, result.stdout, result.stderr)),
        Ok(Err(_)) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err("WS response channel closed unexpectedly".to_string())
        }
        Err(_) => {
            state.pending_ws_execs.write().await.remove(&request_id);
            Err(format!("WS command timed out after {}ms", timeout_ms + 5000))
        }
    }
}
