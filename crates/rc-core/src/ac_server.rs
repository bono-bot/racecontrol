use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::SqlitePool;

use crate::state::AppState;
use rc_common::protocol::{CoreToAgentMessage, DashboardCommand, DashboardEvent};
use rc_common::types::*;

pub struct AcServerInstance {
    pub session_id: String,
    pub config: AcLanSessionConfig,
    pub status: AcServerStatus,
    pub child: Option<Child>,
    pub pid: Option<u32>,
    pub started_at: Option<chrono::DateTime<Utc>>,
    pub join_url: String,
    pub assigned_pods: Vec<String>,
    pub connected_pods: Vec<String>,
    pub error_message: Option<String>,
    pub server_dir: PathBuf,
}

impl AcServerInstance {
    pub fn to_info(&self) -> AcServerInfo {
        AcServerInfo {
            session_id: self.session_id.clone(),
            config: self.config.clone(),
            status: self.status,
            pid: self.pid,
            started_at: self.started_at,
            join_url: self.join_url.clone(),
            connected_pods: self.connected_pods.clone(),
            error_message: self.error_message.clone(),
        }
    }
}

pub struct AcServerManager {
    pub instances: RwLock<HashMap<String, AcServerInstance>>,
}

impl AcServerManager {
    pub fn new() -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
        }
    }
}

// ─── Orphaned Process Cleanup ─────────────────────────────────────────────────

/// On startup, find ac_sessions rows that are still 'starting' or 'running' (left over
/// from a previous rc-core instance) and kill their processes if still alive.  This
/// prevents orphaned acServer processes from holding ports and blocking new sessions.
/// Also adds orphaned ports to the PortAllocator cooldown to avoid TIME_WAIT collisions.
pub async fn cleanup_orphaned_sessions(
    db: &SqlitePool,
    port_allocator: &crate::port_allocator::PortAllocator,
) -> anyhow::Result<u32> {
    // Read ports from dedicated columns first, falling back to json_extract for pre-migration rows
    let rows = sqlx::query_as::<_, (String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT id, pid, \
                COALESCE(udp_port, json_extract(config_json, '$.udp_port')), \
                COALESCE(tcp_port, json_extract(config_json, '$.tcp_port')), \
                COALESCE(http_port, json_extract(config_json, '$.http_port')) \
         FROM ac_sessions WHERE status IN ('starting', 'running')"
    )
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        return Ok(0);
    }

    tracing::info!("Found {} orphaned ac_sessions from previous run", rows.len());
    let mut cleaned = 0u32;

    for (id, pid, udp_port, tcp_port, http_port) in &rows {
        if let Some(pid) = pid {
            let pid = *pid as u32;
            if is_process_alive(pid) {
                tracing::warn!(
                    pid,
                    session_id = %id,
                    "Killing orphaned acServer process on startup"
                );
                if let Err(e) = kill_process_by_pid(pid) {
                    tracing::error!(pid, session_id = %id, "Failed to kill orphaned process: {}", e);
                }
            } else {
                tracing::info!(
                    pid,
                    session_id = %id,
                    "Orphaned session PID {} is no longer alive — marking as error",
                    pid
                );
            }
        } else {
            tracing::info!(
                session_id = %id,
                "Orphaned session has no PID — marking as error"
            );
        }

        // Add orphaned ports to cooldown so they aren't reused during TIME_WAIT
        if let (Some(udp), Some(tcp), Some(http)) = (udp_port, tcp_port, http_port) {
            port_allocator
                .add_to_cooldown(crate::port_allocator::AllocatedPorts {
                    udp_port: *udp as u16,
                    tcp_port: *tcp as u16,
                    http_port: *http as u16,
                })
                .await;
        }

        // Mark session as error regardless
        let _ = sqlx::query(
            "UPDATE ac_sessions SET status = 'error', ended_at = datetime('now') WHERE id = ?"
        )
        .bind(id)
        .execute(db)
        .await;

        cleaned += 1;

        tracing::info!(
            session_id = %id,
            udp_port = ?udp_port,
            tcp_port = ?tcp_port,
            http_port = ?http_port,
            "Cleaned up orphaned session — ports added to cooldown"
        );
    }

    tracing::info!("Cleaned up {} orphaned ac_sessions on startup", cleaned);
    Ok(cleaned)
}

/// Platform-specific process alive check
#[cfg(target_os = "windows")]
fn is_process_alive(pid: u32) -> bool {
    let output = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(&pid.to_string())
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Platform-specific process kill
#[cfg(target_os = "windows")]
fn kill_process_by_pid(pid: u32) -> anyhow::Result<()> {
    let output = std::process::Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("taskkill failed: {}", stderr.trim());
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn kill_process_by_pid(pid: u32) -> anyhow::Result<()> {
    let output = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("kill -9 failed: {}", stderr.trim());
    }
    Ok(())
}

// ─── INI Generation ──────────────────────────────────────────────────────────

pub fn generate_server_cfg_ini(config: &AcLanSessionConfig) -> String {
    let cars_str = config.cars.join(";");

    let track_value = if config.min_csp_version > 0 {
        format!("csp/{}/../{}", config.min_csp_version, config.track)
    } else {
        config.track.clone()
    };

    let mut ini = format!(
        "[SERVER]\n\
         NAME={}\n\
         CARS={}\n\
         TRACK={}\n\
         CONFIG_TRACK={}\n\
         MAX_CLIENTS={}\n\
         PASSWORD={}\n\
         ADMIN_PASSWORD=racingpoint\n\
         UDP_PORT={}\n\
         TCP_PORT={}\n\
         HTTP_PORT={}\n\
         REGISTER_TO_LOBBY=0\n\
         PICKUP_MODE_ENABLED={}\n\
         LOCKED_ENTRY_LIST=0\n\
         LOOP_MODE=1\n\
         SLEEP_TIME=1\n\
         CLIENT_SEND_INTERVAL_HZ=18\n\
         NUM_THREADS=2\n\
         ALLOWED_TYRES_OUT=2\n\
         MAX_BALLAST_KG=300\n\
         QUALIFY_MAX_WAIT_PERC=120\n\
         RACE_PIT_WINDOW_START=0\n\
         RACE_PIT_WINDOW_END=0\n\
         REVERSED_GRID_RACE_POSITIONS=0\n\
         RACE_OVER_TIME=60\n\
         RESULT_SCREEN_TIME=5\n\
         ABS_ALLOWED={}\n\
         TC_ALLOWED={}\n\
         AUTOCLUTCH_ALLOWED={}\n\
         TYRE_BLANKETS_ALLOWED={}\n\
         STABILITY_ALLOWED={}\n\
         FORCE_VIRTUAL_MIRROR={}\n\
         DAMAGE_MULTIPLIER={}\n\
         FUEL_RATE={}\n\
         TYRE_WEAR_RATE={}\n\
         LEGAL_TYRES=\n",
        config.name,
        cars_str,
        track_value,
        config.track_config,
        config.max_clients,
        config.password,
        config.udp_port,
        config.tcp_port,
        config.http_port,
        if config.pickup_mode { 1 } else { 0 },
        config.abs_allowed,
        config.tc_allowed,
        if config.autoclutch_allowed { 1 } else { 0 },
        if config.tyre_blankets_allowed { 1 } else { 0 },
        if config.stability_allowed { 1 } else { 0 },
        if config.force_virtual_mirror { 1 } else { 0 },
        0, // SAFETY: damage always 0, ignoring config.damage_multiplier
        config.fuel_rate,
        config.tyre_wear_rate,
    );

    // Session blocks
    for session in &config.sessions {
        match session.session_type {
            SessionType::Practice => {
                ini.push_str(&format!(
                    "\n[PRACTICE]\n\
                     NAME={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.duration_minutes, session.wait_time_secs
                ));
            }
            SessionType::Qualifying => {
                ini.push_str(&format!(
                    "\n[QUALIFY]\n\
                     NAME={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.duration_minutes, session.wait_time_secs
                ));
            }
            SessionType::Race => {
                ini.push_str(&format!(
                    "\n[RACE]\n\
                     NAME={}\n\
                     LAPS={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.laps, session.duration_minutes, session.wait_time_secs
                ));
            }
            _ => {}
        }
    }

    // Dynamic track
    let dt = &config.dynamic_track;
    ini.push_str(&format!(
        "\n[DYNAMIC_TRACK]\n\
         SESSION_START={}\n\
         RANDOMNESS={}\n\
         SESSION_TRANSFER={}\n\
         LAP_GAIN={}\n",
        100, // SAFETY: grip always 100%, ignoring dt.session_start
        dt.randomness, dt.session_transfer, dt.lap_gain
    ));

    // Weather
    for (i, w) in config.weather.iter().enumerate() {
        ini.push_str(&format!(
            "\n[WEATHER_{}]\n\
             GRAPHICS={}\n\
             BASE_TEMPERATURE_AMBIENT={}\n\
             BASE_TEMPERATURE_ROAD={}\n\
             VARIATION_AMBIENT={}\n\
             VARIATION_ROAD={}\n\
             WIND_BASE_SPEED_MIN={}\n\
             WIND_BASE_SPEED_MAX={}\n\
             WIND_BASE_DIRECTION={}\n\
             WIND_VARIATION_DIRECTION={}\n",
            i, w.graphics,
            w.base_temperature_ambient, w.base_temperature_road,
            w.variation_ambient, w.variation_road,
            w.wind_base_speed_min, w.wind_base_speed_max,
            w.wind_base_direction, w.wind_variation_direction,
        ));
    }

    ini
}

pub fn generate_entry_list_ini(config: &AcLanSessionConfig) -> String {
    let mut ini = String::new();

    if !config.entries.is_empty() {
        for (i, entry) in config.entries.iter().enumerate() {
            ini.push_str(&format!(
                "[CAR_{}]\n\
                 MODEL={}\n\
                 SKIN={}\n\
                 DRIVERNAME={}\n\
                 GUID={}\n\
                 BALLAST={}\n\
                 RESTRICTOR={}\n\
                 SPECTATOR_MODE=0\n",
                i, entry.car_model, entry.skin, entry.driver_name, entry.guid,
                entry.ballast, entry.restrictor,
            ));
            // AssettoServer AI entry: AI=fixed for AI opponents
            if let Some(ref ai) = entry.ai_mode {
                ini.push_str(&format!("AI={}\n", ai));
            }
            ini.push('\n');
        }
    } else if config.pickup_mode && !config.cars.is_empty() {
        // Generate empty slots alternating across allowed cars
        let num_slots = config.max_clients as usize;
        for i in 0..num_slots {
            let car = &config.cars[i % config.cars.len()];
            ini.push_str(&format!(
                "[CAR_{}]\n\
                 MODEL={}\n\
                 SKIN=\n\
                 DRIVERNAME=\n\
                 GUID=\n\
                 BALLAST=0\n\
                 RESTRICTOR=0\n\
                 SPECTATOR_MODE=0\n\n",
                i, car,
            ));
        }
    }

    ini
}

/// Generate extra_cfg.yml for AssettoServer with AI configuration.
/// Returns empty string if no AI entries are present.
/// When AI entries exist, generates EnableAi: true and optionally AiAggression
/// mapped from the 0-100 AI_LEVEL to a 0.0-1.0 float.
pub fn generate_extra_cfg_yml(config: &AcLanSessionConfig, ai_level: Option<u32>) -> String {
    let has_ai = config.entries.iter().any(|e| e.ai_mode.is_some());
    if !has_ai {
        return String::new();
    }

    let mut yml = String::new();
    yml.push_str("EnableAi: true\n");
    yml.push_str("AiParams:\n");
    yml.push_str("  MaxAiTargetOccupancy: 1.0\n");

    if let Some(level) = ai_level {
        let fraction = level as f64 / 100.0;
        yml.push_str(&format!("  AiAggression: {:.2}\n", fraction));
    }

    yml
}

// ─── Server Lifecycle ────────────────────────────────────────────────────────

pub async fn start_ac_server(
    state: &Arc<AppState>,
    config: AcLanSessionConfig,
    pod_ids: Vec<String>,
    ai_level: Option<u32>,
) -> anyhow::Result<String> {
    let session_id = uuid::Uuid::new_v4().to_string();

    // Check if another session is already running
    {
        let instances = state.ac_server.instances.read().await;
        for inst in instances.values() {
            if matches!(inst.status, AcServerStatus::Starting | AcServerStatus::Running) {
                anyhow::bail!("An AC server session is already running: {}", inst.session_id);
            }
        }
    }

    // Allocate dynamic ports for this session
    let allocated = state.port_allocator.allocate(&session_id).await?;
    let mut config = config;
    config.udp_port = allocated.udp_port;
    config.tcp_port = allocated.tcp_port;
    config.http_port = allocated.http_port;

    tracing::info!(
        session_id = %session_id,
        udp_port = allocated.udp_port,
        tcp_port = allocated.tcp_port,
        http_port = allocated.http_port,
        "Dynamically allocated ports for AC session"
    );

    // Create server directory
    let data_dir = &state.config.ac_server.data_dir;
    let server_dir = PathBuf::from(data_dir).join(&session_id);
    let cfg_dir = server_dir.join("cfg");
    std::fs::create_dir_all(&cfg_dir)?;

    // Generate and write config files (now using dynamically allocated ports)
    let server_cfg = generate_server_cfg_ini(&config);
    let entry_list = generate_entry_list_ini(&config);
    std::fs::write(cfg_dir.join("server_cfg.ini"), &server_cfg)?;
    std::fs::write(cfg_dir.join("entry_list.ini"), &entry_list)?;

    // Write extra_cfg.yml for AssettoServer AI configuration (if AI entries exist).
    // Uses caller-provided AI level from host's difficulty tier, falls back to SemiPro (87).
    let extra_cfg = generate_extra_cfg_yml(&config, ai_level);
    if !extra_cfg.is_empty() {
        // extra_cfg.yml goes in server_dir root (AssettoServer reads from working directory)
        std::fs::write(server_dir.join("extra_cfg.yml"), &extra_cfg)?;
        tracing::info!("Wrote extra_cfg.yml with AI configuration for session {}", session_id);
    }

    // Always write csp_extra_options.ini — CSP reads this from the server.
    // Custom content takes priority; otherwise generate a baseline that ensures
    // CSP properly handles audio reinit and session transitions.
    let csp_opts = config.csp_extra_options.clone().unwrap_or_else(|| {
        "[EXTRA_RULES]\n\
         FORCE_MINIMUM_CSP=1\n"
            .to_string()
    });
    std::fs::write(cfg_dir.join("csp_extra_options.ini"), &csp_opts)?;

    // Determine LAN IP
    let lan_ip = state.config.ac_server.lan_ip.clone()
        .unwrap_or_else(detect_lan_ip);

    // Build join URL
    let join_url = format!(
        "acmanager://race/online/join?ip={}&httpPort={}",
        lan_ip, config.http_port
    );

    // Spawn acServer process
    let acserver_path = &state.config.ac_server.acserver_path;
    let (child, pid) = if Path::new(acserver_path).exists() {
        let child = std::process::Command::new(acserver_path)
            .current_dir(&server_dir)
            .spawn()?;
        let pid = child.id();
        (Some(child), Some(pid))
    } else {
        tracing::warn!("acServer not found at {}. Running in dry-run mode.", acserver_path);
        (None, None)
    };

    let now = Utc::now();

    // Store instance
    {
        let mut instances = state.ac_server.instances.write().await;
        instances.insert(
            session_id.clone(),
            AcServerInstance {
                session_id: session_id.clone(),
                config: config.clone(),
                status: AcServerStatus::Starting,
                child,
                pid,
                started_at: Some(now),
                join_url: join_url.clone(),
                assigned_pods: pod_ids.clone(),
                connected_pods: Vec::new(),
                error_message: None,
                server_dir,
            },
        );
    }

    // Broadcast initial status
    let info = {
        let instances = state.ac_server.instances.read().await;
        instances.get(&session_id).map(|i| i.to_info())
    };
    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
    }

    // Log to DB (including dynamically allocated ports for orphan recovery)
    let config_json = serde_json::to_string(&config).unwrap_or_default();
    let pod_ids_json = serde_json::to_string(&pod_ids).unwrap_or_default();
    let _ = sqlx::query(
        "INSERT INTO ac_sessions (id, config_json, status, pod_ids, pid, join_url, udp_port, tcp_port, http_port, started_at, created_at) \
         VALUES (?, ?, 'starting', ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&session_id)
    .bind(&config_json)
    .bind(&pod_ids_json)
    .bind(pid.map(|p| p as i64))
    .bind(&join_url)
    .bind(allocated.udp_port as i64)
    .bind(allocated.tcp_port as i64)
    .bind(allocated.http_port as i64)
    .execute(&state.db)
    .await;

    // Wait briefly for server to start
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Update to Running
    {
        let mut instances = state.ac_server.instances.write().await;
        if let Some(inst) = instances.get_mut(&session_id) {
            inst.status = AcServerStatus::Running;
        }
    }

    // Broadcast running status
    let info = {
        let instances = state.ac_server.instances.read().await;
        instances.get(&session_id).map(|i| i.to_info())
    };
    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
    }

    // Launch Content Manager on each selected pod with JSON launch_args
    // (fixes: agent expects JSON with game_mode "multi", not raw acmanager:// URI)
    let agent_senders = state.agent_senders.read().await;
    for pod_id in &pod_ids {
        if let Some(sender) = agent_senders.get(pod_id) {
            let launch_json = serde_json::json!({
                "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
                "track": &config.track,
                "track_config": &config.track_config,
                "game_mode": "multi",
                "server_ip": &lan_ip,
                "server_http_port": config.http_port,
                "server_password": &config.password,
                "session_type": "race",
            });
            let cmd = CoreToAgentMessage::LaunchGame {
                sim_type: SimType::AssettoCorsa,
                launch_args: Some(launch_json.to_string()),
            };
            let _ = sender.send(cmd).await;
            tracing::info!("Sent AC multiplayer join command to pod {} (JSON launch_args)", pod_id);
        } else {
            tracing::warn!("Pod {} not connected, skipping launch", pod_id);
        }
    }

    // Update DB status
    let _ = sqlx::query("UPDATE ac_sessions SET status = 'running' WHERE id = ?")
        .bind(&session_id)
        .execute(&state.db)
        .await;

    tracing::info!("AC LAN session started: {} (join: {})", session_id, join_url);
    Ok(session_id)
}

pub async fn stop_ac_server(state: &Arc<AppState>, session_id: &str) -> anyhow::Result<()> {
    // Collect results BEFORE killing the process
    let _ = collect_results(state, session_id).await;

    let assigned_pods;
    let mut killed_via_fallback = false;

    // Update instance
    {
        let mut instances = state.ac_server.instances.write().await;
        if let Some(inst) = instances.get_mut(session_id) {
            inst.status = AcServerStatus::Stopping;

            // Kill the process via child handle
            if let Some(ref mut child) = inst.child {
                let _ = child.kill();
                let _ = child.wait();
            } else if let Some(pid) = inst.pid {
                // Child handle lost (e.g. after restart) but PID still in memory
                tracing::info!(pid, "Stopping acServer via PID fallback (in-memory, no child handle)");
                if is_process_alive(pid) {
                    let _ = kill_process_by_pid(pid);
                }
                killed_via_fallback = true;
            }

            assigned_pods = inst.assigned_pods.clone();
            inst.status = AcServerStatus::Stopped;
            inst.child = None;
            inst.pid = None;
        } else {
            // Session not in memory — try PID fallback from DB (post-restart scenario)
            let row = sqlx::query_as::<_, (Option<i64>, Option<String>)>(
                "SELECT pid, pod_ids FROM ac_sessions WHERE id = ? AND status IN ('starting', 'running')"
            )
            .bind(session_id)
            .fetch_optional(&state.db)
            .await?;

            if let Some((pid_opt, pod_ids_json)) = row {
                if let Some(pid) = pid_opt {
                    let pid = pid as u32;
                    if is_process_alive(pid) {
                        tracing::info!(pid, session_id, "Stopping acServer via PID fallback from DB (no child handle)");
                        if let Err(e) = kill_process_by_pid(pid) {
                            tracing::error!(pid, session_id, "Failed to kill via PID fallback: {}", e);
                        }
                    }
                    killed_via_fallback = true;
                } else {
                    // No PID in DB — last resort: kill all acServer processes by name
                    #[cfg(target_os = "windows")]
                    {
                        tracing::warn!(session_id, "No PID available — killing ALL acServer.exe instances as last resort");
                        let _ = std::process::Command::new("taskkill")
                            .args(["/F", "/IM", "acServer.exe"])
                            .output();
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        tracing::warn!(session_id, "No PID available — killing all acServer processes as last resort");
                        let _ = std::process::Command::new("killall")
                            .args(["-9", "acServer"])
                            .output();
                    }
                    killed_via_fallback = true;
                }

                // Parse pod_ids from DB for StopGame broadcast
                assigned_pods = pod_ids_json
                    .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
                    .unwrap_or_default();
            } else {
                anyhow::bail!("AC session {} not found in memory or database", session_id);
            }
        }
    }

    if killed_via_fallback {
        tracing::info!(session_id, "Session stopped via PID fallback path");
    }

    // Release dynamically allocated ports (enters cooldown)
    state.port_allocator.release(session_id).await;

    // Broadcast stopped status
    let info = {
        let instances = state.ac_server.instances.read().await;
        instances.get(session_id).map(|i| i.to_info())
    };
    if let Some(info) = info {
        let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
    }

    // Send StopGame to all assigned pods
    let agent_senders = state.agent_senders.read().await;
    for pod_id in &assigned_pods {
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreToAgentMessage::StopGame).await;
        }
    }

    // Remove from active instances
    state.ac_server.instances.write().await.remove(session_id);

    // Update DB
    let _ = sqlx::query("UPDATE ac_sessions SET status = 'stopped', ended_at = datetime('now') WHERE id = ?")
        .bind(session_id)
        .execute(&state.db)
        .await;

    tracing::info!("AC LAN session stopped: {}", session_id);
    Ok(())
}

pub async fn check_ac_server_health(state: &Arc<AppState>) {
    let mut dead_sessions = Vec::new();

    {
        let mut instances = state.ac_server.instances.write().await;
        for (id, inst) in instances.iter_mut() {
            if !matches!(inst.status, AcServerStatus::Running | AcServerStatus::Starting) {
                continue;
            }

            // Check if acServer process is still alive
            if let Some(ref mut child) = inst.child {
                match child.try_wait() {
                    Ok(Some(_)) => {
                        // Process exited
                        inst.status = AcServerStatus::Error;
                        inst.error_message = Some("acServer process exited unexpectedly".to_string());
                        dead_sessions.push(id.clone());
                    }
                    Ok(None) => {} // Still running
                    Err(e) => {
                        inst.status = AcServerStatus::Error;
                        inst.error_message = Some(format!("Failed to check process: {}", e));
                        dead_sessions.push(id.clone());
                    }
                }
            }
        }
    }

    // Broadcast updates for dead sessions
    for id in dead_sessions {
        let info = {
            let instances = state.ac_server.instances.read().await;
            instances.get(&id).map(|i| i.to_info())
        };
        if let Some(info) = info {
            let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
        }
    }
}

// ─── Preset Management ──────────────────────────────────────────────────────

pub async fn save_preset(
    state: &Arc<AppState>,
    name: &str,
    config: &AcLanSessionConfig,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let config_json = serde_json::to_string(config)?;

    sqlx::query(
        "INSERT INTO ac_presets (id, name, config_json, created_at) VALUES (?, ?, ?, datetime('now'))"
    )
    .bind(&id)
    .bind(name)
    .bind(&config_json)
    .execute(&state.db)
    .await?;

    tracing::info!("Saved AC preset: {} ({})", name, id);
    Ok(id)
}

pub async fn delete_preset(state: &Arc<AppState>, preset_id: &str) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM ac_presets WHERE id = ?")
        .bind(preset_id)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn load_preset(
    state: &Arc<AppState>,
    preset_id: &str,
) -> anyhow::Result<(String, AcLanSessionConfig)> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT name, config_json FROM ac_presets WHERE id = ?"
    )
    .bind(preset_id)
    .fetch_one(&state.db)
    .await?;

    let config: AcLanSessionConfig = serde_json::from_str(&row.1)?;
    Ok((row.0, config))
}

pub async fn list_presets(state: &Arc<AppState>) -> anyhow::Result<Vec<AcPresetSummary>> {
    let rows = sqlx::query_as::<_, (String, String, String, Option<String>)>(
        "SELECT id, name, config_json, created_at FROM ac_presets ORDER BY created_at DESC"
    )
    .fetch_all(&state.db)
    .await?;

    let mut presets = Vec::new();
    for (id, name, config_json, created_at) in rows {
        if let Ok(config) = serde_json::from_str::<AcLanSessionConfig>(&config_json) {
            presets.push(AcPresetSummary {
                id,
                name,
                track: config.track,
                track_config: config.track_config,
                cars: config.cars,
                max_clients: config.max_clients,
                created_at: created_at
                    .and_then(|s| chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok())
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(Utc::now),
                updated_at: None,
            });
        }
    }

    Ok(presets)
}

// ─── Dashboard Command Handler ──────────────────────────────────────────────

pub async fn handle_dashboard_command(state: &Arc<AppState>, cmd: DashboardCommand) {
    match cmd {
        DashboardCommand::StartAcSession { config, pod_ids } => {
            match start_ac_server(state, config, pod_ids, None).await {
                Ok(id) => tracing::info!("AC session started: {}", id),
                Err(e) => tracing::error!("Failed to start AC session: {}", e),
            }
        }
        DashboardCommand::StopAcSession { session_id } => {
            if let Err(e) = stop_ac_server(state, &session_id).await {
                tracing::error!("Failed to stop AC session: {}", e);
            }
        }
        DashboardCommand::SaveAcPreset { name, config } => {
            match save_preset(state, &name, &config).await {
                Ok(_) => {
                    // Send updated preset list
                    if let Ok(presets) = list_presets(state).await {
                        let _ = state.dashboard_tx.send(DashboardEvent::AcPresetList(presets));
                    }
                }
                Err(e) => tracing::error!("Failed to save preset: {}", e),
            }
        }
        DashboardCommand::DeleteAcPreset { preset_id } => {
            if let Err(e) = delete_preset(state, &preset_id).await {
                tracing::error!("Failed to delete preset: {}", e);
            } else if let Ok(presets) = list_presets(state).await {
                let _ = state.dashboard_tx.send(DashboardEvent::AcPresetList(presets));
            }
        }
        DashboardCommand::LoadAcPreset { preset_id } => {
            match load_preset(state, &preset_id).await {
                Ok((_name, config)) => {
                    let _ = state.dashboard_tx.send(DashboardEvent::AcPresetLoaded {
                        preset_id,
                        config,
                    });
                }
                Err(e) => tracing::error!("Failed to load preset: {}", e),
            }
        }
        _ => {}
    }
}

// ─── Result Collection ──────────────────────────────────────────────────────

/// A single driver's result from an AC dedicated server session.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MultiplayerResult {
    pub position: u32,
    pub driver_name: String,
    pub guid: String,
    pub best_lap_ms: Option<i64>,
    pub total_time_ms: Option<i64>,
    pub laps_completed: u32,
}

/// Matches the AC dedicated server JSON result format.
/// Uses serde(rename) for PascalCase field names and serde(default) for leniency.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct AcResultFile {
    #[serde(rename = "Result", default)]
    pub result: Vec<AcResultEntry>,
    #[serde(rename = "TrackName", default)]
    pub track_name: String,
    #[serde(rename = "TrackConfig", default)]
    pub track_config: String,
    #[serde(rename = "Type", default)]
    pub session_type: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
pub struct AcResultEntry {
    #[serde(rename = "DriverName", default)]
    pub driver_name: String,
    #[serde(rename = "DriverGuid", default)]
    pub driver_guid: String,
    #[serde(rename = "CarId", default)]
    pub car_id: u32,
    #[serde(rename = "CarModel", default)]
    pub car_model: String,
    #[serde(rename = "BestLap", default)]
    pub best_lap: i64,
    #[serde(rename = "TotalTime", default)]
    pub total_time: i64,
    #[serde(rename = "LapCount", default)]
    pub lap_count: u32,
    #[serde(rename = "HasFinished", default)]
    pub has_finished: bool,
}

/// Parse AC result files from a server session directory and return structured results.
/// Reads JSON files from `{server_dir}/results/` directory.
/// Returns empty vec if directory doesn't exist or contains no valid results.
pub fn parse_ac_results(server_dir: &Path) -> Vec<MultiplayerResult> {
    let results_dir = server_dir.join("results");
    if !results_dir.exists() {
        tracing::debug!("No results directory found at {:?}", results_dir);
        return vec![];
    }

    let mut all_results: Vec<MultiplayerResult> = Vec::new();

    let entries = match std::fs::read_dir(&results_dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::warn!("Failed to read results directory {:?}: {}", results_dir, e);
            return vec![];
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to read result file {:?}: {}", path, e);
                continue;
            }
        };

        let result_file: AcResultFile = match serde_json::from_str(&content) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Failed to parse result file {:?}: {}", path, e);
                continue;
            }
        };

        for (i, entry) in result_file.result.iter().enumerate() {
            all_results.push(MultiplayerResult {
                position: (i + 1) as u32,
                driver_name: entry.driver_name.clone(),
                guid: entry.driver_guid.clone(),
                best_lap_ms: if entry.best_lap > 0 { Some(entry.best_lap) } else { None },
                total_time_ms: if entry.total_time > 0 { Some(entry.total_time) } else { None },
                laps_completed: entry.lap_count,
            });
        }
    }

    all_results
}

/// Collect results from an AC server session, persist to multiplayer_results table.
/// Called from stop_ac_server before killing the process.
pub async fn collect_results(
    state: &Arc<AppState>,
    session_id: &str,
) -> anyhow::Result<Vec<MultiplayerResult>> {
    // Get server_dir from instance
    let server_dir = {
        let instances = state.ac_server.instances.read().await;
        instances.get(session_id).map(|i| i.server_dir.clone())
    };

    let server_dir = match server_dir {
        Some(d) => d,
        None => {
            tracing::debug!("No in-memory instance for session {} — skipping result collection", session_id);
            return Ok(vec![]);
        }
    };

    let results = parse_ac_results(&server_dir);
    if results.is_empty() {
        tracing::info!("No results to collect for AC session {}", session_id);
        return Ok(vec![]);
    }

    // Find the group_session_id linked to this ac_session_id
    let group_session_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM group_sessions WHERE ac_session_id = ?",
    )
    .bind(session_id)
    .fetch_optional(&state.db)
    .await?;

    let group_session_id = match group_session_id {
        Some(id) => id,
        None => {
            tracing::info!("AC session {} not linked to a group session — skipping result persistence", session_id);
            return Ok(results);
        }
    };

    // Get member mappings: pod_id -> driver_id (to match AC results to our drivers)
    let members: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT driver_id, pod_id FROM group_session_members WHERE group_session_id = ?",
    )
    .bind(&group_session_id)
    .fetch_all(&state.db)
    .await?;

    // Build a name->driver_id mapping for matching
    let mut name_to_driver: HashMap<String, String> = HashMap::new();
    for (driver_id, _pod_id) in &members {
        let name: Option<String> = sqlx::query_scalar("SELECT name FROM drivers WHERE id = ?")
            .bind(driver_id)
            .fetch_optional(&state.db)
            .await?;
        if let Some(name) = name {
            name_to_driver.insert(name.to_lowercase(), driver_id.clone());
        }
    }

    // Persist results
    for result in &results {
        let result_id = uuid::Uuid::new_v4().to_string();

        // Try to match driver by name (case-insensitive)
        let driver_id = name_to_driver
            .get(&result.driver_name.to_lowercase())
            .cloned()
            .unwrap_or_else(|| "unknown".to_string());

        let _ = sqlx::query(
            "INSERT INTO multiplayer_results (id, group_session_id, ac_session_id, driver_id, position, best_lap_ms, total_time_ms, laps_completed, dnf)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&result_id)
        .bind(&group_session_id)
        .bind(session_id)
        .bind(&driver_id)
        .bind(result.position as i64)
        .bind(result.best_lap_ms)
        .bind(result.total_time_ms)
        .bind(result.laps_completed as i64)
        .bind(if result.laps_completed == 0 { 1i64 } else { 0i64 })
        .execute(&state.db)
        .await;
    }

    tracing::info!(
        "Collected {} results for AC session {} (group {})",
        results.len(),
        session_id,
        group_session_id,
    );

    Ok(results)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn detect_lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ac_result_file_deserialization() {
        let json = r#"{
            "TrackName": "monza",
            "TrackConfig": "",
            "Type": "RACE",
            "Result": [
                {
                    "DriverName": "Alice",
                    "DriverGuid": "steam_123",
                    "CarId": 0,
                    "CarModel": "ks_ferrari_488_gt3",
                    "BestLap": 98500,
                    "TotalTime": 590000,
                    "LapCount": 6,
                    "HasFinished": true
                },
                {
                    "DriverName": "Bob",
                    "DriverGuid": "steam_456",
                    "CarId": 1,
                    "CarModel": "ks_ferrari_488_gt3",
                    "BestLap": 99200,
                    "TotalTime": 600000,
                    "LapCount": 6,
                    "HasFinished": true
                }
            ]
        }"#;

        let result_file: AcResultFile = serde_json::from_str(json).unwrap();
        assert_eq!(result_file.result.len(), 2);
        assert_eq!(result_file.result[0].driver_name, "Alice");
        assert_eq!(result_file.result[0].best_lap, 98500);
        assert_eq!(result_file.result[0].lap_count, 6);
        assert_eq!(result_file.result[1].driver_name, "Bob");
        assert_eq!(result_file.track_name, "monza");
        assert_eq!(result_file.session_type, "RACE");
    }

    #[test]
    fn test_ac_result_file_maps_to_multiplayer_result() {
        let json = r#"{
            "TrackName": "spa",
            "TrackConfig": "",
            "Type": "RACE",
            "Result": [
                {
                    "DriverName": "Driver1",
                    "DriverGuid": "guid_1",
                    "CarId": 0,
                    "CarModel": "car1",
                    "BestLap": 120000,
                    "TotalTime": 720000,
                    "LapCount": 5,
                    "HasFinished": true
                },
                {
                    "DriverName": "Driver2",
                    "DriverGuid": "guid_2",
                    "CarId": 1,
                    "CarModel": "car1",
                    "BestLap": 0,
                    "TotalTime": 0,
                    "LapCount": 0,
                    "HasFinished": false
                }
            ]
        }"#;

        let result_file: AcResultFile = serde_json::from_str(json).unwrap();

        // Map to MultiplayerResult
        let results: Vec<MultiplayerResult> = result_file
            .result
            .iter()
            .enumerate()
            .map(|(i, entry)| MultiplayerResult {
                position: (i + 1) as u32,
                driver_name: entry.driver_name.clone(),
                guid: entry.driver_guid.clone(),
                best_lap_ms: if entry.best_lap > 0 { Some(entry.best_lap) } else { None },
                total_time_ms: if entry.total_time > 0 { Some(entry.total_time) } else { None },
                laps_completed: entry.lap_count,
            })
            .collect();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].position, 1);
        assert_eq!(results[0].driver_name, "Driver1");
        assert_eq!(results[0].best_lap_ms, Some(120000));
        assert_eq!(results[0].total_time_ms, Some(720000));
        assert_eq!(results[0].laps_completed, 5);

        // DNF driver — zero best_lap and total_time
        assert_eq!(results[1].position, 2);
        assert_eq!(results[1].best_lap_ms, None);
        assert_eq!(results[1].total_time_ms, None);
        assert_eq!(results[1].laps_completed, 0);
    }

    #[test]
    fn test_parse_ac_results_from_directory() {
        use std::fs;

        // Create a temporary directory structure mimicking AC server output
        let temp_dir = std::env::temp_dir().join("ac_test_results");
        let results_dir = temp_dir.join("results");
        let _ = fs::remove_dir_all(&temp_dir); // clean up from previous runs
        fs::create_dir_all(&results_dir).unwrap();

        let json = r#"{
            "TrackName": "imola",
            "TrackConfig": "",
            "Type": "RACE",
            "Result": [
                {
                    "DriverName": "TestDriver",
                    "DriverGuid": "test_guid",
                    "CarId": 0,
                    "CarModel": "bmw_m3_gt2",
                    "BestLap": 95000,
                    "TotalTime": 480000,
                    "LapCount": 5,
                    "HasFinished": true
                }
            ]
        }"#;
        fs::write(results_dir.join("race_result.json"), json).unwrap();

        let results = parse_ac_results(&temp_dir);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].driver_name, "TestDriver");
        assert_eq!(results[0].guid, "test_guid");
        assert_eq!(results[0].best_lap_ms, Some(95000));

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_ac_results_empty_dir() {
        use std::fs;

        let temp_dir = std::env::temp_dir().join("ac_test_empty");
        let results_dir = temp_dir.join("results");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&results_dir).unwrap();

        let results = parse_ac_results(&temp_dir);
        assert!(results.is_empty());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_parse_ac_results_no_dir() {
        let temp_dir = std::env::temp_dir().join("ac_test_nonexistent_xyz");
        let _ = std::fs::remove_dir_all(&temp_dir);

        let results = parse_ac_results(&temp_dir);
        assert!(results.is_empty());
    }

    #[test]
    fn test_ac_result_lenient_parsing() {
        // Missing optional fields should not cause parse failure
        let json = r#"{
            "Result": [
                {
                    "DriverName": "Partial",
                    "DriverGuid": "",
                    "CarId": 0,
                    "CarModel": "",
                    "BestLap": 0,
                    "TotalTime": 0,
                    "LapCount": 0,
                    "HasFinished": false
                }
            ]
        }"#;

        let result_file: AcResultFile = serde_json::from_str(json).unwrap();
        assert_eq!(result_file.result.len(), 1);
        assert_eq!(result_file.track_name, ""); // default
        assert_eq!(result_file.session_type, ""); // default
    }

    // ── Phase 04 Plan 01: Server config safety overrides ────────────────

    #[test]
    fn test_server_cfg_damage_always_zero() {
        // SAFETY: Even when config has damage_multiplier=100, output must have DAMAGE_MULTIPLIER=0
        let mut config = AcLanSessionConfig::default();
        config.damage_multiplier = 100;
        let ini = generate_server_cfg_ini(&config);
        assert!(ini.contains("DAMAGE_MULTIPLIER=0"),
            "SAFETY: DAMAGE_MULTIPLIER must always be 0, got INI:\n{}", ini);
        assert!(!ini.contains("DAMAGE_MULTIPLIER=100"),
            "SAFETY: DAMAGE_MULTIPLIER=100 must NOT appear in output");
    }

    #[test]
    fn test_server_cfg_grip_always_100() {
        // SAFETY: Even when config has session_start=50, output must have SESSION_START=100
        let mut config = AcLanSessionConfig::default();
        config.dynamic_track.session_start = 50;
        let ini = generate_server_cfg_ini(&config);
        assert!(ini.contains("SESSION_START=100"),
            "SAFETY: SESSION_START must always be 100, got INI:\n{}", ini);
        assert!(!ini.contains("SESSION_START=50"),
            "SAFETY: SESSION_START=50 must NOT appear in output");
    }

    // ── Phase 09 Plan 01: AI entry list, extra_cfg.yml, LaunchGame JSON ───

    #[test]
    fn test_entry_list_ai_entries_have_ai_fixed() {
        let mut config = AcLanSessionConfig::default();
        config.entries = vec![
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "Marco Rossi".to_string(),
                guid: String::new(),
                ballast: 0,
                restrictor: 0,
                pod_id: None,
                ai_mode: Some("fixed".to_string()),
            },
        ];
        let ini = generate_entry_list_ini(&config);
        assert!(ini.contains("AI=fixed"), "AI entry must have AI=fixed line in INI:\n{}", ini);
        assert!(ini.contains("DRIVERNAME=Marco Rossi"), "AI entry must have driver name");
    }

    #[test]
    fn test_entry_list_mixed_human_and_ai() {
        let mut config = AcLanSessionConfig::default();
        config.entries = vec![
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "Human Driver".to_string(),
                guid: "steam_123".to_string(),
                ballast: 0,
                restrictor: 0,
                pod_id: Some("pod_1".to_string()),
                ai_mode: None,
            },
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "AI Driver".to_string(),
                guid: String::new(),
                ballast: 0,
                restrictor: 0,
                pod_id: None,
                ai_mode: Some("fixed".to_string()),
            },
        ];
        let ini = generate_entry_list_ini(&config);

        // Split into sections by [CAR_]
        let sections: Vec<&str> = ini.split("[CAR_").collect();
        // sections[0] is empty (before first [CAR_), sections[1] is CAR_0, sections[2] is CAR_1
        assert!(sections.len() >= 3, "Must have at least 2 CAR sections, got:\n{}", ini);

        let human_section = sections[1];
        let ai_section = sections[2];

        // Human entry should NOT have AI line
        assert!(!human_section.contains("AI="), "Human entry must not have AI= line:\n{}", human_section);
        assert!(human_section.contains("DRIVERNAME=Human Driver"));

        // AI entry should have AI=fixed
        assert!(ai_section.contains("AI=fixed"), "AI entry must have AI=fixed:\n{}", ai_section);
        assert!(ai_section.contains("DRIVERNAME=AI Driver"));
    }

    #[test]
    fn test_entry_list_backward_compat_no_ai_mode() {
        let mut config = AcLanSessionConfig::default();
        config.entries = vec![
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "Legacy Driver".to_string(),
                guid: "steam_789".to_string(),
                ballast: 0,
                restrictor: 0,
                pod_id: Some("pod_3".to_string()),
                ai_mode: None,
            },
        ];
        let ini = generate_entry_list_ini(&config);
        assert!(!ini.contains("AI="), "Legacy entry with ai_mode None must not have AI= line:\n{}", ini);
        assert!(ini.contains("DRIVERNAME=Legacy Driver"));
    }

    #[test]
    fn test_extra_cfg_yml_with_ai_level() {
        let mut config = AcLanSessionConfig::default();
        config.entries = vec![
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "AI Bot".to_string(),
                guid: String::new(),
                ballast: 0,
                restrictor: 0,
                pod_id: None,
                ai_mode: Some("fixed".to_string()),
            },
        ];
        let yml = generate_extra_cfg_yml(&config, Some(87));
        assert!(yml.contains("EnableAi: true"), "Must contain EnableAi: true:\n{}", yml);
        assert!(yml.contains("AiAggression: 0.87"), "AI level 87 must map to AiAggression: 0.87:\n{}", yml);
    }

    #[test]
    fn test_extra_cfg_yml_no_ai_entries_returns_empty() {
        let config = AcLanSessionConfig::default(); // no entries at all
        let yml = generate_extra_cfg_yml(&config, Some(90));
        assert!(yml.is_empty(), "No AI entries should produce empty extra_cfg.yml, got:\n{}", yml);
    }

    #[test]
    fn test_extra_cfg_yml_ai_entries_no_level() {
        let mut config = AcLanSessionConfig::default();
        config.entries = vec![
            AcEntrySlot {
                car_model: "ks_ferrari_488_gt3".to_string(),
                skin: String::new(),
                driver_name: "AI Bot".to_string(),
                guid: String::new(),
                ballast: 0,
                restrictor: 0,
                pod_id: None,
                ai_mode: Some("fixed".to_string()),
            },
        ];
        let yml = generate_extra_cfg_yml(&config, None);
        assert!(yml.contains("EnableAi: true"), "Must contain EnableAi: true:\n{}", yml);
        assert!(!yml.contains("AiAggression"), "No ai_level should omit AiAggression:\n{}", yml);
    }
}
