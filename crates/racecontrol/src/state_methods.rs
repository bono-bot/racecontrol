//! AppState methods for settings broadcast, feature flags, and error tracking.
//!
//! Extracted from state.rs for ARCH-03 (<500 line modules).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use rc_common::protocol::{CoreMessage, CoreToAgentMessage};

use super::AppState;

impl AppState {
    /// Broadcast settings to all agents, applying per-pod screen_blanking override.
    /// If `screen_blanking_pods` is set (comma-separated pod numbers), only those pods
    /// get `screen_blanking_enabled=true`; all others get `false`.
    pub async fn broadcast_settings(&self, settings: &HashMap<String, String>) {
        let blanking_pods = settings.get("screen_blanking_pods")
            .or(None) // check DB value below
            .cloned();

        // If not in the provided settings, check DB
        let blanking_pods = if blanking_pods.is_some() {
            blanking_pods
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM kiosk_settings WHERE key = 'screen_blanking_pods'"
            )
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
        };

        let blanking_enabled = settings.get("screen_blanking_enabled")
            .map(|v| v == "true")
            .unwrap_or(false);

        let pods = self.pods.read().await;
        let agent_senders = self.agent_senders.read().await;
        let active_timers = self.billing.active_timers.read().await;

        for (pod_id, sender) in agent_senders.iter() {
            let mut pod_settings = settings.clone();

            // Pods with active billing sessions must NEVER be blanked
            if active_timers.contains_key(pod_id) {
                pod_settings.insert("screen_blanking_enabled".to_string(), "false".to_string());
            } else {
                // Apply per-pod blanking override
                if let Some(ref pod_list) = blanking_pods
                    && !pod_list.trim().is_empty() {
                        let pod_number = pods.get(pod_id).map(|p| p.number);
                        let is_blanking_pod = pod_number.map(|n| {
                            pod_list.split(',')
                                .any(|s| s.trim().parse::<u32>().ok() == Some(n))
                        }).unwrap_or(false);

                        if blanking_enabled && !is_blanking_pod {
                            pod_settings.insert("screen_blanking_enabled".to_string(), "false".to_string());
                        }
                    }
            }

            let msg = CoreToAgentMessage::SettingsUpdated { settings: pod_settings };
            let _ = sender.send(CoreMessage::wrap(msg)).await;
        }
    }

    /// Get settings for a specific pod, applying per-pod screen_blanking override.
    pub async fn settings_for_pod(&self, settings: &HashMap<String, String>, pod_number: u32) -> HashMap<String, String> {
        let mut pod_settings = settings.clone();

        let blanking_pods = settings.get("screen_blanking_pods").cloned()
            .or({
                // We can't do async in or_else, so just return None
                None
            });

        // Check DB for screen_blanking_pods if not in settings
        let blanking_pods = if blanking_pods.is_some() {
            blanking_pods
        } else {
            sqlx::query_scalar::<_, String>(
                "SELECT value FROM kiosk_settings WHERE key = 'screen_blanking_pods'"
            )
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
        };

        let blanking_enabled = settings.get("screen_blanking_enabled")
            .map(|v| v == "true")
            .unwrap_or(false);

        if let Some(ref pod_list) = blanking_pods
            && !pod_list.trim().is_empty() {
                let is_blanking_pod = pod_list.split(',')
                    .any(|s| s.trim().parse::<u32>().ok() == Some(pod_number));

                if blanking_enabled && !is_blanking_pod {
                    pod_settings.insert("screen_blanking_enabled".to_string(), "false".to_string());
                }
            }

        pod_settings
    }

    /// v22.0 Phase 177: Load feature flags from DB into the in-memory cache.
    /// Also initializes config_push_seq from MAX(seq_num) + 1 in config_push_queue.
    /// Called once at startup after AppState is constructed.
    pub async fn load_feature_flags(&self) {
        let mut loaded = false;
        for attempt in 1..=3 {
            match sqlx::query_as::<_, crate::flags::FeatureFlagRow>(
                "SELECT name, enabled, default_value, overrides, version, updated_at FROM feature_flags",
            )
            .fetch_all(&self.db)
            .await
            {
                Ok(rows) => {
                    let count = rows.len();
                    let mut cache = self.feature_flags.write().await;
                    for row in rows {
                        cache.insert(row.name.clone(), row);
                    }
                    tracing::info!("Loaded {} feature flags into cache", count);
                    loaded = true;
                    break;
                }
                Err(e) => {
                    tracing::warn!("Failed to load feature flags from DB (attempt {}/3): {}", attempt, e);
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    }
                }
            }
        }
        if !loaded {
            tracing::error!("Feature flags could not be loaded after 3 attempts — running with empty cache");
        }

        // Initialize config_push_seq from MAX(seq_num) + 1
        match sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(seq_num) FROM config_push_queue",
        )
        .fetch_one(&self.db)
        .await
        {
            Ok(max_opt) => {
                let next = (max_opt.unwrap_or(0) + 1) as u64;
                self.config_push_seq.store(next, Ordering::Relaxed);
                tracing::info!("Initialized config_push_seq to {}", next);
            }
            Err(e) => {
                tracing::warn!("Failed to read config_push_queue max seq_num: {}", e);
            }
        }
    }

    /// v22.0 Phase 177: Broadcast the current feature flag state to all connected pods.
    /// Reads the in-memory cache, resolves per-pod overrides, then sends
    /// CoreToAgentMessage::FlagSync to every connected agent with its own resolved values.
    pub async fn broadcast_flag_sync(&self) {
        use rc_common::types::FlagSyncPayload;

        let cache = self.feature_flags.read().await;
        let version = cache.values().map(|r| r.version as u64).max().unwrap_or(0);
        let agent_senders = self.agent_senders.read().await;
        for (pod_id, sender) in agent_senders.iter() {
            let flags: HashMap<String, bool> = cache
                .values()
                .map(|row| {
                    let effective = serde_json::from_str::<HashMap<String, bool>>(&row.overrides)
                        .ok()
                        .and_then(|ovr| ovr.get(pod_id).copied())
                        .unwrap_or(row.enabled);
                    (row.name.clone(), effective)
                })
                .collect();
            let payload = FlagSyncPayload { flags, version };
            if let Err(e) = sender
                .send(CoreMessage::wrap(rc_common::protocol::CoreToAgentMessage::FlagSync(payload)))
                .await
            {
                tracing::warn!("Failed to send FlagSync to pod {}: {}", pod_id, e);
            }
        }
    }

    /// Increment the API error counter for a given endpoint.
    pub fn record_api_error(&self, endpoint: &str) {
        let mut counts = self.api_error_counts.lock().unwrap();
        counts
            .entry(endpoint.to_string())
            .or_insert_with(|| AtomicU32::new(0))
            .fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of API error counts and reset them.
    pub fn drain_api_error_counts(&self) -> HashMap<String, u32> {
        let mut counts = self.api_error_counts.lock().unwrap();
        let snapshot: HashMap<String, u32> = counts
            .drain()
            .map(|(k, v)| (k, v.load(Ordering::Relaxed)))
            .filter(|(_, v)| *v > 0)
            .collect();
        *self.api_error_counts_reset.lock().unwrap() = Instant::now();
        snapshot
    }
}
