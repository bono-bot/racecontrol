//! Content Drift Detector -- Phase 366 GLD-F-03
//!
//! Polls each pod's live disk inventory (via GET :8090/debug/content-dirs)
//! every 60 minutes and compares against the expected TOML inventory.
//! Fires ContentDriftDetected WS events and writes to content_drift_events table.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::state::AppState;
use rc_common::inventory_types::ContentDirsResponse;

const LOG_TARGET: &str = "content-drift";
const POLL_INTERVAL_SECS: u64 = 3600; // 60 minutes per D-07

/// Spawn the background content drift polling task.
/// Called from main.rs after AppState is initialized.
pub fn spawn_content_drift_task(state: Arc<AppState>) {
    tokio::spawn(async move {
        tracing::info!(
            target: LOG_TARGET,
            "Content drift detector started (interval={}s)",
            POLL_INTERVAL_SECS
        );
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
        // First tick fires immediately -- skip to avoid running at startup before pods connect.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            tracing::debug!(target: LOG_TARGET, "Starting content drift poll for all pods");
            check_all_pods_drift(&state).await;
        }
    });
}

/// Check content drift for all registered pods.
async fn check_all_pods_drift(state: &Arc<AppState>) {
    let config_dir = state.config.server.config_dir_path();
    let sentry_key = state
        .config
        .pods
        .sentry_service_key
        .clone()
        .unwrap_or_default();

    // Snapshot pod info (id -> ip) to avoid holding lock across network calls.
    let pod_targets: Vec<(String, String, u32)> = {
        let pods = state.pods.read().await;
        pods.values()
            .map(|p| (p.id.clone(), p.ip_address.clone(), p.number))
            .collect()
    };

    for (pod_id, pod_ip, pod_number) in pod_targets {
        // Load expected inventory from TOML (ground truth)
        let expected_inventory =
            match crate::api::pods::load_pod_inventory(pod_number, &config_dir) {
                Ok(inv) => inv,
                Err((_, msg)) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        pod_id = %pod_id,
                        "Failed to load TOML inventory: {}",
                        msg
                    );
                    continue;
                }
            };

        // Probe rc-agent for live disk content
        let url = format!("http://{}:8090/debug/content-dirs", pod_ip);
        let resp = reqwest::Client::new()
            .get(&url)
            .header("X-Service-Key", &sentry_key)
            .timeout(Duration::from_secs(10))
            .send()
            .await;

        let live_inventory: ContentDirsResponse = match resp {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(data) => data,
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        pod_id = %pod_id,
                        "Failed to parse content-dirs response: {}",
                        e
                    );
                    continue;
                }
            },
            Ok(r) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    pod_id = %pod_id,
                    "content-dirs returned {}",
                    r.status()
                );
                continue;
            }
            Err(_) => {
                // Pod offline -- skip silently per D-08 pitfall #5
                tracing::debug!(
                    target: LOG_TARGET,
                    pod_id = %pod_id,
                    "Pod unreachable -- skipping drift check"
                );
                continue;
            }
        };

        // Build live disk map: game_key -> (cars_on_disk, tracks_on_disk)
        let live_map: std::collections::HashMap<String, (HashSet<String>, HashSet<String>)> =
            live_inventory
                .games
                .iter()
                .map(|g| {
                    (
                        g.game_key.clone(),
                        (
                            g.cars_on_disk.iter().cloned().collect(),
                            g.tracks_on_disk.iter().cloned().collect(),
                        ),
                    )
                })
                .collect();

        let mut drifts: Vec<(String, String, String)> = Vec::new(); // (game_key, delta_type, item)

        // Compare expected TOML games vs live disk
        for game in &expected_inventory.games {
            let game_key = &game.key;
            match live_map.get(game_key) {
                None => {
                    // Whole game missing from live disk
                    drifts.push((
                        game_key.clone(),
                        "game_removed".to_string(),
                        game_key.clone(),
                    ));
                }
                Some((live_cars, live_tracks)) => {
                    let expected_cars: HashSet<String> = game.cars.iter().cloned().collect();
                    let expected_tracks: HashSet<String> =
                        game.tracks.iter().cloned().collect();

                    // Cars in TOML but not on disk -> removed
                    for car in expected_cars.difference(live_cars) {
                        if !car.is_empty() {
                            drifts.push((
                                game_key.clone(),
                                "car_removed".to_string(),
                                car.clone(),
                            ));
                        }
                    }
                    // Cars on disk but not in TOML -> added (unexpected)
                    for car in live_cars.difference(&expected_cars) {
                        if !car.is_empty() {
                            drifts.push((
                                game_key.clone(),
                                "car_added".to_string(),
                                car.clone(),
                            ));
                        }
                    }
                    // Tracks in TOML but not on disk -> removed
                    for track in expected_tracks.difference(live_tracks) {
                        if !track.is_empty() {
                            drifts.push((
                                game_key.clone(),
                                "track_removed".to_string(),
                                track.clone(),
                            ));
                        }
                    }
                    // Tracks on disk but not in TOML -> added
                    for track in live_tracks.difference(&expected_tracks) {
                        if !track.is_empty() {
                            drifts.push((
                                game_key.clone(),
                                "track_added".to_string(),
                                track.clone(),
                            ));
                        }
                    }
                }
            }
        }

        // Check for games on disk not in TOML -> game_added
        for game_dir in &live_inventory.games {
            if !expected_inventory
                .games
                .iter()
                .any(|g| g.key == game_dir.game_key)
            {
                drifts.push((
                    game_dir.game_key.clone(),
                    "game_added".to_string(),
                    game_dir.game_key.clone(),
                ));
            }
        }

        if drifts.is_empty() {
            tracing::debug!(
                target: LOG_TARGET,
                pod_id = %pod_id,
                "No content drift detected"
            );
            continue;
        }

        // Record and emit drifts
        emit_drift_events(state, &pod_id, &drifts).await;
    }
}

/// Insert drift events into DB, broadcast WS events, fire WhatsApp for game_removed.
async fn emit_drift_events(
    state: &Arc<AppState>,
    pod_id: &str,
    drifts: &[(String, String, String)],
) {
    let now_ist = (Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30))
        .to_rfc3339();

    for (game_key, delta_type, item) in drifts {
        let id = Uuid::new_v4().to_string();

        // Insert to DB
        let db_result = sqlx::query(
            "INSERT INTO content_drift_events (id, pod_id, detected_at, game_key, delta_type, item) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(pod_id)
        .bind(&now_ist)
        .bind(game_key)
        .bind(delta_type)
        .bind(item)
        .execute(&state.db)
        .await;

        if let Err(e) = db_result {
            tracing::error!(
                target: LOG_TARGET,
                pod_id = %pod_id,
                "Failed to insert content_drift_event: {}",
                e
            );
        }

        // Broadcast ContentDriftDetected to all admin dashboard clients
        let _ = state.dashboard_tx.send(
            rc_common::protocol::DashboardEvent::ContentDriftDetected {
                pod_id: pod_id.to_string(),
                game_key: game_key.clone(),
                delta_type: delta_type.clone(),
                item: item.clone(),
                detected_at: now_ist.clone(),
            },
        );
        tracing::warn!(
            target: LOG_TARGET,
            pod_id = %pod_id,
            delta_type = %delta_type,
            item = %item,
            "Content drift detected"
        );

        // WhatsApp alert for game_removed only (per D-09 -- game removal is P2-10 class)
        if delta_type == "game_removed" {
            let alert_msg = format!(
                "CONTENT DRIFT: {} - game '{}' no longer on disk (not in TOML expected inventory). Check pod immediately.",
                pod_id, item
            );
            crate::whatsapp_alerter::send_admin_alert(
                &state.config,
                "ContentDrift",
                &alert_msg,
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::sqlite::SqlitePoolOptions;
    use sqlx::SqlitePool;

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .connect(":memory:")
            .await
            .expect("in-memory sqlite");
        sqlx::query(
            "CREATE TABLE content_drift_events (
                id TEXT PRIMARY KEY, pod_id TEXT, detected_at TEXT,
                game_key TEXT, delta_type TEXT, item TEXT,
                resolved_at TEXT, resolution_note TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("create table");
        pool
    }

    #[tokio::test]
    async fn test_content_drift_events_table_writable() {
        let pool = test_pool().await;
        let result = sqlx::query(
            "INSERT INTO content_drift_events (id, pod_id, detected_at, game_key, delta_type, item) VALUES ('id1', 'pod-1', '2026-04-10T00:00:00+05:30', 'assetto_corsa', 'game_removed', 'assetto_corsa')",
        )
        .execute(&pool)
        .await;
        assert!(
            result.is_ok(),
            "Insert to content_drift_events failed: {:?}",
            result.err()
        );
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM content_drift_events")
                .fetch_one(&pool)
                .await
                .expect("count");
        assert_eq!(count.0, 1);
    }
}
