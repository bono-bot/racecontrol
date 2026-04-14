//! AC Server lifecycle utilities — orphan cleanup, process management, and session control.
//!
//! Extracted from `ac_server.rs` for module size compliance (<500 lines).

use std::path::Path;
use std::sync::Arc;
use sqlx::SqlitePool;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::*;

use crate::ac_server_results::detect_lan_ip;
use crate::state::AppState;

// ─── Orphaned Process Cleanup ─────────────────────────────────────────────────

/// On startup, find ac_sessions rows that are still 'starting' or 'running' (left over
/// from a previous racecontrol instance) and kill their processes if still alive.  This
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

// ─── Platform-specific process utilities ─────────────────────────────────────

/// Platform-specific process alive check
#[cfg(target_os = "windows")]
pub(crate) fn is_process_alive(pid: u32) -> bool {
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
pub(crate) fn is_process_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

/// Platform-specific process kill
#[cfg(target_os = "windows")]
pub(crate) fn kill_process_by_pid(pid: u32) -> anyhow::Result<()> {
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
pub(crate) fn kill_process_by_pid(pid: u32) -> anyhow::Result<()> {
    let output = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("kill -9 failed: {}", stderr.trim());
    }
    Ok(())
}

// ─── Session Management ─────────────────────────────────────────────────────

/// GROUP-02: Enable or disable continuous mode on a running AC server session.
/// When enabled, the server auto-restarts a new race when the current one ends.
pub async fn set_continuous_mode(
    state: &Arc<AppState>,
    session_id: &str,
    enabled: bool,
    group_session_id: Option<String>,
) -> anyhow::Result<()> {
    let mut instances = state.ac_server.instances.write().await;
    let inst = instances.get_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("AC session {} not found", session_id))?;
    inst.continuous_mode = enabled;
    if group_session_id.is_some() {
        inst.group_session_id = group_session_id;
    }
    tracing::info!(
        "Continuous mode {} for AC session {} (group: {:?})",
        if enabled { "ENABLED" } else { "DISABLED" },
        session_id, inst.group_session_id
    );

    // Broadcast status update
    let info = inst.to_info();
    let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));
    Ok(())
}

/// GROUP-04: Update track/car config on an active continuous-mode session.
/// Takes effect on the next race restart (monitor_continuous_session reads current config).
pub async fn update_session_config(
    state: &Arc<AppState>,
    session_id: &str,
    track: Option<String>,
    track_config: Option<String>,
    cars: Option<Vec<String>>,
) -> anyhow::Result<()> {
    let mut instances = state.ac_server.instances.write().await;
    let inst = instances.get_mut(session_id)
        .ok_or_else(|| anyhow::anyhow!("AC session {} not found", session_id))?;

    if !inst.continuous_mode {
        anyhow::bail!("Config update only allowed in continuous mode (session {})", session_id);
    }

    if let Some(t) = track {
        tracing::info!("GROUP-04: Track changed from '{}' to '{}' on session {}", inst.config.track, t, session_id);
        inst.config.track = t;
    }
    if let Some(tc) = track_config {
        inst.config.track_config = tc;
    }
    if let Some(c) = cars {
        tracing::info!("GROUP-04: Cars changed to {:?} on session {}", c, session_id);
        inst.config.cars = c;
    }

    // Broadcast updated config
    let info = inst.to_info();
    let _ = state.dashboard_tx.send(DashboardEvent::AcServerUpdate(info));

    Ok(())
}

/// GROUP-03: Re-send LaunchGame to a single pod that failed to join the AC server.
/// The server is already running — this just tells the pod to try connecting again.
pub async fn retry_pod_join(
    state: &Arc<AppState>,
    session_id: &str,
    pod_id: &str,
) -> anyhow::Result<()> {
    let (config, lan_ip) = {
        let instances = state.ac_server.instances.read().await;
        let inst = instances.get(session_id)
            .ok_or_else(|| anyhow::anyhow!("AC session {} not found", session_id))?;

        if !matches!(inst.status, AcServerStatus::Running | AcServerStatus::Starting) {
            anyhow::bail!("AC server {} is not running (status: {:?})", session_id, inst.status);
        }

        if !inst.assigned_pods.contains(&pod_id.to_string()) {
            anyhow::bail!("Pod {} is not assigned to session {}", pod_id, session_id);
        }

        let lip = state.config.ac_server.lan_ip.clone().unwrap_or_else(detect_lan_ip);
        (inst.config.clone(), lip)
    };

    // Build the same launch_args JSON as start_ac_server does
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

    // First send StopGame to kill any stuck process, then re-launch
    {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let _ = sender.send(CoreMessage::wrap(CoreToAgentMessage::StopGame)).await;
        } else {
            anyhow::bail!("Pod {} is not connected", pod_id);
        }
    }

    // Brief delay to let the old process die
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    {
        let agent_senders = state.agent_senders.read().await;
        if let Some(sender) = agent_senders.get(pod_id) {
            let cmd = CoreToAgentMessage::LaunchGame {
                sim_type: rc_common::types::SimType::AssettoCorsa,
                launch_args: Some(launch_json.to_string()),
                force_clean: false,
                duration_minutes: None,
                launch_id: None,
            };
            let _ = sender.send(CoreMessage::wrap(cmd)).await;
            tracing::info!(
                "GROUP-03: Re-sent LaunchGame to pod {} for session {} (retry join)",
                pod_id, session_id
            );
        }
    }

    // Update game tracker to Launching (clears error state on dashboard)
    {
        let mut games = state.game_launcher.active_games.write().await;
        if let Some(tracker) = games.get_mut(pod_id) {
            tracker.game_state = rc_common::types::GameState::Launching;
            tracker.error_message = None;
            let info = tracker.to_info();
            let _ = state.dashboard_tx.send(DashboardEvent::GameStateChanged(info));
        }
    }

    Ok(())
}
