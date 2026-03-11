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
pub async fn cleanup_orphaned_sessions(db: &SqlitePool) -> anyhow::Result<u32> {
    let rows = sqlx::query_as::<_, (String, Option<i64>, Option<i64>, Option<i64>, Option<i64>)>(
        "SELECT id, pid, \
                json_extract(config_json, '$.udp_port') AS udp_port, \
                json_extract(config_json, '$.tcp_port') AS tcp_port, \
                json_extract(config_json, '$.http_port') AS http_port \
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
            "Cleaned up orphaned session — ports freed"
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
        config.damage_multiplier,
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
        dt.session_start, dt.randomness, dt.session_transfer, dt.lap_gain
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
                 SPECTATOR_MODE=0\n\n",
                i, entry.car_model, entry.skin, entry.driver_name, entry.guid,
                entry.ballast, entry.restrictor,
            ));
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

// ─── Server Lifecycle ────────────────────────────────────────────────────────

pub async fn start_ac_server(
    state: &Arc<AppState>,
    config: AcLanSessionConfig,
    pod_ids: Vec<String>,
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

    // Create server directory
    let data_dir = &state.config.ac_server.data_dir;
    let server_dir = PathBuf::from(data_dir).join(&session_id);
    let cfg_dir = server_dir.join("cfg");
    std::fs::create_dir_all(&cfg_dir)?;

    // Generate and write config files
    let server_cfg = generate_server_cfg_ini(&config);
    let entry_list = generate_entry_list_ini(&config);
    std::fs::write(cfg_dir.join("server_cfg.ini"), &server_cfg)?;
    std::fs::write(cfg_dir.join("entry_list.ini"), &entry_list)?;

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

    // Log to DB
    let config_json = serde_json::to_string(&config).unwrap_or_default();
    let pod_ids_json = serde_json::to_string(&pod_ids).unwrap_or_default();
    let _ = sqlx::query(
        "INSERT INTO ac_sessions (id, config_json, status, pod_ids, pid, join_url, started_at, created_at) \
         VALUES (?, ?, 'starting', ?, ?, ?, datetime('now'), datetime('now'))"
    )
    .bind(&session_id)
    .bind(&config_json)
    .bind(&pod_ids_json)
    .bind(pid.map(|p| p as i64))
    .bind(&join_url)
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

    // Launch Content Manager on each selected pod
    let agent_senders = state.agent_senders.read().await;
    for pod_id in &pod_ids {
        if let Some(sender) = agent_senders.get(pod_id) {
            let cmd = CoreToAgentMessage::LaunchGame {
                sim_type: SimType::AssettoCorsa,
                launch_args: Some(join_url.clone()),
            };
            let _ = sender.send(cmd).await;
            tracing::info!("Sent AC join command to pod {}", pod_id);
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
    let assigned_pods;

    // Update instance
    {
        let mut instances = state.ac_server.instances.write().await;
        if let Some(inst) = instances.get_mut(session_id) {
            inst.status = AcServerStatus::Stopping;

            // Kill the process
            if let Some(ref mut child) = inst.child {
                let _ = child.kill();
                let _ = child.wait();
            }

            assigned_pods = inst.assigned_pods.clone();
            inst.status = AcServerStatus::Stopped;
            inst.child = None;
            inst.pid = None;
        } else {
            anyhow::bail!("AC session {} not found", session_id);
        }
    }

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
            match start_ac_server(state, config, pod_ids).await {
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
