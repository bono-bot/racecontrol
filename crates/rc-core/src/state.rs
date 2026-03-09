use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::ac_camera::CameraController;
use crate::ac_server::AcServerManager;
use crate::billing::BillingManager;
use crate::config::Config;
use crate::game_launcher::GameManager;
use rc_common::protocol::{AiChannelMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::PodInfo;

/// Tracks OTP request count and window start per phone number
pub struct OtpRateLimit {
    pub count: u32,
    pub window_start: Instant,
}

/// Tracks failed OTP verification attempts per phone number
pub struct OtpFailedAttempts {
    pub count: u32,
    pub locked_until: Option<Instant>,
}

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub pods: RwLock<HashMap<String, PodInfo>>,
    pub dashboard_tx: broadcast::Sender<DashboardEvent>,
    pub billing: BillingManager,
    pub game_launcher: GameManager,
    pub ac_server: AcServerManager,
    pub camera: CameraController,
    /// Map of pod_id -> sender for pushing commands to specific agents
    pub agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>,
    /// Map of pod_id -> connection ID (monotonic counter) to detect stale disconnects
    pub agent_conn_ids: RwLock<HashMap<String, u64>>,
    /// Shared HTTP client for outbound requests (cloud sync, etc.)
    pub http_client: reqwest::Client,
    /// Active terminal PIN sessions (token -> expiry)
    pub terminal_sessions: RwLock<HashMap<String, chrono::DateTime<chrono::Utc>>>,
    /// OTP send rate limiting: phone -> (count, window_start)
    pub otp_rate_limits: Mutex<HashMap<String, OtpRateLimit>>,
    /// OTP verification failed attempts: phone -> (count, locked_until)
    pub otp_failed_attempts: Mutex<HashMap<String, OtpFailedAttempts>>,
    /// Sender for pushing messages to AI peer via WebSocket (None if not connected)
    pub ai_peer_tx: RwLock<Option<mpsc::Sender<AiChannelMessage>>>,
    /// Per-endpoint API error counts (endpoint -> count), reset every 5 min by error aggregator
    pub api_error_counts: Mutex<HashMap<String, AtomicU32>>,
    /// When the API error counts were last reset
    pub api_error_counts_reset: Mutex<Instant>,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Self {
        let (dashboard_tx, _) = broadcast::channel(1024);
        Self {
            config,
            db,
            pods: RwLock::new(HashMap::new()),
            dashboard_tx,
            billing: BillingManager::new(),
            game_launcher: GameManager::new(),
            ac_server: AcServerManager::new(),
            camera: CameraController::new(),
            agent_senders: RwLock::new(HashMap::new()),
            agent_conn_ids: RwLock::new(HashMap::new()),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
            terminal_sessions: RwLock::new(HashMap::new()),
            otp_rate_limits: Mutex::new(HashMap::new()),
            otp_failed_attempts: Mutex::new(HashMap::new()),
            ai_peer_tx: RwLock::new(None),
            api_error_counts: Mutex::new(HashMap::new()),
            api_error_counts_reset: Mutex::new(Instant::now()),
        }
    }

    /// Broadcast settings to all agents, applying per-pod screen_blanking override.
    /// If `screen_blanking_pods` is set (comma-separated pod numbers), only those pods
    /// get `screen_blanking_enabled=true`; all others get `false`.
    pub async fn broadcast_settings(&self, settings: &HashMap<String, String>) {
        let blanking_pods = settings.get("screen_blanking_pods")
            .or_else(|| None) // check DB value below
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
                if let Some(ref pod_list) = blanking_pods {
                    if !pod_list.trim().is_empty() {
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
            }

            let msg = CoreToAgentMessage::SettingsUpdated { settings: pod_settings };
            let _ = sender.send(msg).await;
        }
    }

    /// Get settings for a specific pod, applying per-pod screen_blanking override.
    pub async fn settings_for_pod(&self, settings: &HashMap<String, String>, pod_number: u32) -> HashMap<String, String> {
        let mut pod_settings = settings.clone();

        let blanking_pods = settings.get("screen_blanking_pods").cloned()
            .or_else(|| {
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

        if let Some(ref pod_list) = blanking_pods {
            if !pod_list.trim().is_empty() {
                let is_blanking_pod = pod_list.split(',')
                    .any(|s| s.trim().parse::<u32>().ok() == Some(pod_number));

                if blanking_enabled && !is_blanking_pod {
                    pod_settings.insert("screen_blanking_enabled".to_string(), "false".to_string());
                }
            }
        }

        pod_settings
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
