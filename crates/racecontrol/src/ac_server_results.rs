//! AC server results collection, preset management, and dashboard commands.
//! Extracted from ac_server.rs (Phase 385, v49.0 Architecture Completion).

use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use chrono::Utc;
use rc_common::protocol::DashboardEvent;
use serde::{Deserialize, Serialize};
use crate::state::AppState;
use crate::ac_server::{AcServerManager, start_ac_server, stop_ac_server};
use rc_common::protocol::DashboardCommand;
use rc_common::types::*;

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
        DashboardCommand::StartAcSession { config, pod_ids, ai_level } => {
            match start_ac_server(state, config, pod_ids, ai_level).await {
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

    // Check session dir first, then acServer's own dir (CWD changed to acServer dir in v24)
    let mut results = parse_ac_results(&server_dir);
    if results.is_empty() {
        let acserver_dir = Path::new(&state.config.ac_server.acserver_path)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        results = parse_ac_results(acserver_dir);
    }
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
            "INSERT INTO multiplayer_results (id, group_session_id, ac_session_id, driver_id, position, best_lap_ms, total_time_ms, laps_completed, dnf, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
        .bind(&state.config.venue.venue_id)
        .execute(&state.db)
        .await;
    }

    tracing::info!(
        "Collected {} results for AC session {} (group {})",
        results.len(),
        session_id,
        group_session_id,
    );

    // Phase 365 GLD-E-01: Collect AI behavior samples
    {
        let config_json: Option<String> = sqlx::query_scalar(
            "SELECT config_json FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await
        .ok()
        .flatten();

        let ai_level_for_session = config_json
            .as_deref()
            .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
            .and_then(|v| v.get("ai_level").and_then(|a| a.as_u64()))
            .map(|v| v as u32);

        if let Some(ai_lvl) = ai_level_for_session {
            // Get car and track from the AC server instance config
            let (config_car, config_track) = {
                let instances = state.ac_server.instances.read().await;
                instances.get(session_id).map(|inst| {
                    let car = inst.config.cars.first().cloned().unwrap_or_default();
                    let track = inst.config.track.clone();
                    (car, track)
                }).unwrap_or_default()
            };

            if !config_car.is_empty() && !config_track.is_empty() {
                let flags_snapshot = {
                    state.feature_flags.read().await.clone()
                };
                // Use first assigned pod_id from members, or group_session_id as fallback
                let pod_id = members.first().map(|(_, p)| p.clone().unwrap_or_default()).unwrap_or_default();
                crate::ai_behavior_batch::collect_ai_behavior_samples(
                    &state.db,
                    session_id,
                    &pod_id,
                    &config_car,
                    &config_track,
                    ai_lvl,
                    &results,
                    &flags_snapshot,
                ).await;

                // Phase 365 GLD-E-04: Check for AI behavior anomaly
                let ai_laps: Vec<i64> = results
                    .iter()
                    .filter(|e| e.guid.is_empty() && e.best_lap_ms.unwrap_or(0) > 0 && e.laps_completed >= 3)
                    .filter_map(|e| e.best_lap_ms)
                    .collect();
                if !ai_laps.is_empty() {
                    let mut sorted = ai_laps.clone();
                    sorted.sort_unstable();
                    let mid = sorted.len() / 2;
                    let median = if sorted.len() % 2 == 0 {
                        (sorted[mid - 1] + sorted[mid]) / 2
                    } else {
                        sorted[mid]
                    };
                    crate::ai_behavior_batch::check_and_broadcast_anomaly(
                        state,
                        session_id,
                        &pod_id,
                        &config_car,
                        &config_track,
                        ai_lvl,
                        median,
                        ai_laps.len() as u32,
                    ).await;
                }
            }
        }
    }

    Ok(results)
}

// ─── Helpers ────────────────────────────────────────────────────────────────

pub fn detect_lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

