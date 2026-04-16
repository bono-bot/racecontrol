use chrono::Utc;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::state::AppState;
use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::*;

pub use crate::ac_server_config::*;
pub use crate::ac_server_lifecycle::*;
pub use crate::ac_server_monitor::*;
pub use crate::ac_server_results::*;

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
    pub continuous_mode: bool,
    /// group_session_id for looking up billing state
    pub group_session_id: Option<String>,
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
            continuous_mode: self.continuous_mode,
        }
    }
}

pub struct AcServerManager {
    pub instances: RwLock<HashMap<String, AcServerInstance>>,
}

impl Default for AcServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AcServerManager {
    pub fn new() -> Self {
        Self {
            instances: RwLock::new(HashMap::new()),
        }
    }
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

    // Spawn acServer process — binary MUST exist (no silent dry-run)
    let acserver_path = &state.config.ac_server.acserver_path;
    if !Path::new(acserver_path).exists() {
        // Clean up allocated ports before failing
        state.port_allocator.release(&session_id).await;
        // Clean up created directory
        let _ = std::fs::remove_dir_all(&server_dir);
        anyhow::bail!(
            "acServer binary not found at '{}'. Install the AC dedicated server or update \
             [ac_server] acserver_path in racecontrol.toml. Multiplayer AC cannot start without it.",
            acserver_path
        );
    }
    // CWD must be the acServer's own directory (where content/ lives), NOT the session
    // directory. The vanilla Kunos acServer reads tracks/cars from content/ relative to CWD.
    // Copy our generated configs into acServer's cfg/ directory before starting.
    let acserver_dir = Path::new(acserver_path).parent()
        .unwrap_or_else(|| Path::new("."));
    let acserver_cfg_dir = acserver_dir.join("cfg");
    std::fs::create_dir_all(&acserver_cfg_dir)?;
    std::fs::copy(cfg_dir.join("server_cfg.ini"), acserver_cfg_dir.join("server_cfg.ini"))?;
    std::fs::copy(cfg_dir.join("entry_list.ini"), acserver_cfg_dir.join("entry_list.ini"))?;
    if cfg_dir.join("csp_extra_options.ini").exists() {
        let _ = std::fs::copy(cfg_dir.join("csp_extra_options.ini"), acserver_cfg_dir.join("csp_extra_options.ini"));
    }
    // extra_cfg.yml goes in server root (AssettoServer reads from CWD root)
    if server_dir.join("extra_cfg.yml").exists() {
        let _ = std::fs::copy(server_dir.join("extra_cfg.yml"), acserver_dir.join("extra_cfg.yml"));
    }
    let child = std::process::Command::new(acserver_path)
        .current_dir(acserver_dir)
        .spawn()?;
    let pid = child.id();
    let (child, pid) = (Some(child), Some(pid));

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
                continuous_mode: false,
                group_session_id: None,
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
    // Clone senders, drop lock, then send (standing rule: no lock across .await)
    let senders_snapshot: Vec<(String, _)> = {
        let agent_senders = state.agent_senders.read().await;
        pod_ids.iter().filter_map(|pod_id| {
            agent_senders.get(pod_id).map(|s| (pod_id.clone(), s.clone()))
        }).collect()
    };
    let missing: Vec<_> = pod_ids.iter().filter(|p| !senders_snapshot.iter().any(|(id, _)| id == *p)).collect();
    for pod_id in missing {
        tracing::warn!("Pod {} not connected, skipping launch", pod_id);
    }
    for (pod_id, sender) in &senders_snapshot {
        let launch_json = serde_json::json!({
            "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
            "track": &config.track,
            "track_config": &config.track_config,
            "game_mode": "multi",
            "server_ip": &lan_ip,
            "server_port": config.udp_port,  // v44.0: was missing — clients can't connect without UDP port
            "server_http_port": config.http_port,
            "server_password": &config.password,
            "session_type": "race",
        });
        let cmd = CoreToAgentMessage::LaunchGame {
            sim_type: SimType::AssettoCorsa,
            launch_args: Some(launch_json.to_string()),
            force_clean: false,
            duration_minutes: None,
            launch_id: None,
        };
        let _ = sender.send(CoreMessage::wrap(cmd)).await;
        tracing::info!("Sent AC multiplayer join command to pod {} (JSON launch_args)", pod_id);
    }

    // Update DB status
    let _ = sqlx::query("UPDATE ac_sessions SET status = 'running' WHERE id = ?")
        .bind(&session_id)
        .execute(&state.db)
        .await;

    // v44.0 Phase 333: Spawn lobby sync monitor to track pod connections via acServer HTTP API.
    // The monitor polls /INFO every 3s, updates connected_pods, broadcasts LobbyUpdate events,
    // and transitions to Active when all pods connect (or after 120s timeout).
    {
        let lobby_state = state.clone();
        let lobby_session_id = session_id.clone();
        let lobby_http_port = config.http_port;
        let lobby_pod_count = pod_ids.len();
        tokio::spawn(async move {
            monitor_lobby_sync(lobby_state, lobby_session_id, lobby_http_port, lobby_pod_count).await;
        });
    }

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

    // Send StopGame to all assigned pods (clone senders, drop lock first)
    let stop_senders: Vec<_> = {
        let agent_senders = state.agent_senders.read().await;
        assigned_pods.iter().filter_map(|pod_id| {
            agent_senders.get(pod_id).cloned()
        }).collect()
    };
    for sender in &stop_senders {
        let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
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

#[cfg(test)]
#[path = "ac_server_tests.rs"]
mod tests;
