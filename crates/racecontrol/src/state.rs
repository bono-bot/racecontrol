use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::ac_camera::CameraController;
use crate::ac_server::AcServerManager;
use crate::billing::BillingManager;
use crate::config::Config;
use crate::crypto::encryption::FieldCipher;
use crate::email_alerts::EmailAlerter;
use crate::fleet_health::FleetHealthStore;
use crate::game_launcher::GameManager;
use crate::port_allocator::PortAllocator;
use rc_common::protocol::{AiChannelMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::types::{ContentManifest, DeployState, PodInfo};
use rc_common::watchdog::EscalatingBackoff;

/// Watchdog recovery state for a single pod.
///
/// Tracks where the watchdog is in the restart/verify cycle so
/// pod_monitor and pod_healer can coordinate without racing.
#[derive(Debug, Clone, PartialEq)]
pub enum WatchdogState {
    /// Pod heartbeat is current — no action needed.
    Healthy,
    /// Watchdog sent a restart command; waiting for rc-agent to come back.
    Restarting { attempt: u32, started_at: DateTime<Utc> },
    /// Restart command sent; now running post-restart verification checks.
    Verifying { attempt: u32, started_at: DateTime<Utc> },
    /// All restart attempts exhausted; manual intervention required.
    RecoveryFailed { attempt: u32, failed_at: DateTime<Utc> },
}

/// Cached assist state for a pod (abs, tc, auto_shifter, ffb_percent).
/// Populated by WebSocket handlers when agent reports assist changes or state queries.
#[derive(Clone, Debug, serde::Serialize)]
pub struct CachedAssistState {
    pub abs: u8,
    pub tc: u8,
    pub auto_shifter: bool,
    pub ffb_percent: u8,
}

impl Default for CachedAssistState {
    fn default() -> Self {
        Self {
            abs: 0,
            tc: 0,
            auto_shifter: true,
            ffb_percent: 70,
        }
    }
}

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

/// Result of a WebSocket command sent to a pod agent.
/// Stored in a oneshot channel and resolved when ExecResult arrives.
#[derive(Debug)]
pub struct WsExecResult {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Venue config snapshot received from James via comms-link sync_push.
/// Stores the latest sanitized config from the on-premise server.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct VenueConfigSnapshot {
    pub venue_name: String,
    pub venue_location: String,
    pub venue_timezone: String,
    pub pod_count: u64,
    pub pod_discovery: bool,
    pub pod_healer_enabled: bool,
    pub pod_healer_interval_secs: u64,
    pub branding_primary_color: String,
    pub branding_theme: String,
    pub source: String,
    pub pushed_at: u64,
    pub config_hash: String,
    pub received_at: chrono::DateTime<chrono::Utc>,
}

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub pods: RwLock<HashMap<String, PodInfo>>,
    pub dashboard_tx: broadcast::Sender<DashboardEvent>,
    /// Broadcast channel for emitting events to Bono's VPS relay.
    /// Other modules send here; bono_relay::spawn() subscribes and pushes to webhook.
    pub bono_event_tx: broadcast::Sender<crate::bono_relay::BonoEvent>,
    pub billing: BillingManager,
    pub game_launcher: GameManager,
    pub ac_server: AcServerManager,
    pub port_allocator: PortAllocator,
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
    /// Per-pod customer PIN failure counter (PIN-01).
    /// Keyed by pod_id. Incremented on wrong customer PIN; reset on success.
    /// Counter is separate from staff_pin_failures — they NEVER share state (PIN-02).
    pub customer_pin_failures: RwLock<HashMap<String, u32>>,
    /// Per-pod staff/employee PIN failure counter (PIN-01).
    /// Keyed by pod_id. Has NO lockout ceiling — staff can always unlock (PIN-02).
    pub staff_pin_failures: RwLock<HashMap<String, u32>>,
    /// Sender for pushing messages to AI peer via WebSocket (None if not connected)
    pub ai_peer_tx: RwLock<Option<mpsc::Sender<AiChannelMessage>>>,
    /// Per-endpoint API error counts (endpoint -> count), reset every 5 min by error aggregator
    pub api_error_counts: Mutex<HashMap<String, AtomicU32>>,
    /// When the API error counts were last reset
    pub api_error_counts_reset: Mutex<Instant>,
    /// Per-pod escalating backoff state (shared between pod_monitor and pod_healer)
    pub pod_backoffs: RwLock<HashMap<String, EscalatingBackoff>>,
    /// Email alerter for watchdog notifications (behind RwLock for async mutation)
    pub email_alerter: RwLock<EmailAlerter>,
    /// Per-pod watchdog FSM state (shared between pod_monitor and pod_healer)
    pub pod_watchdog_states: RwLock<HashMap<String, WatchdogState>>,
    /// Per-pod restart flag — set by pod_monitor, cleared by pod_healer on recovery
    pub pod_needs_restart: RwLock<HashMap<String, bool>>,
    /// Per-pod content manifest cache (populated when agent sends ContentManifest after Register)
    pub pod_manifests: RwLock<HashMap<String, ContentManifest>>,
    /// Per-pod deploy lifecycle state (active deploy blocks watchdog restart)
    pub pod_deploy_states: RwLock<HashMap<String, DeployState>>,
    /// Per-pod pending deploy binary URLs (set when pod has active billing at rolling deploy time)
    /// When a billing session ends, the deploy is triggered automatically.
    pub pending_deploys: RwLock<HashMap<String, String>>,
    /// Per-pod cached assist state (abs, tc, auto_shifter, ffb_percent).
    /// Updated by WS handlers on AssistChanged/FfbGainChanged/AssistState messages.
    /// Read by GET /pods/{pod_id}/assist-state to return real values immediately.
    pub assist_cache: RwLock<HashMap<String, CachedAssistState>>,
    /// Pending WebSocket command responses: request_id -> oneshot sender.
    /// Populated by ws_exec_on_pod(), resolved by ExecResult handler in ws/mod.rs.
    /// Cleaned up on agent disconnect (entries prefixed with pod_id).
    pub pending_ws_execs: RwLock<HashMap<String, tokio::sync::oneshot::Sender<WsExecResult>>>,
    /// Phase 50: Pending self-test responses: request_id -> (pod_id, oneshot sender).
    /// Populated by pod_self_test handler, resolved by SelfTestResult in ws/mod.rs.
    /// Cleaned up on agent disconnect (entries matching pod_id).
    pub pending_self_tests: RwLock<HashMap<String, (String, tokio::sync::oneshot::Sender<serde_json::Value>)>>,
    /// Shared relay availability flag — written by cloud_sync (with hysteresis),
    /// read by action_queue. Avoids duplicate health-check loops.
    pub relay_available: AtomicBool,
    /// Per-pod fleet health state: version, agent_started_at, http_reachable.
    /// Written by WS StartupReport/Disconnect handlers and background probe loop.
    pub pod_fleet_health: RwLock<HashMap<String, FleetHealthStore>>,
    /// Latest venue config snapshot from on-premise server (via comms-link sync_push).
    /// None until first config_snapshot received.
    pub venue_config: RwLock<Option<VenueConfigSnapshot>>,
    /// Field-level encryption cipher for PII columns (AES-256-GCM + HMAC-SHA256).
    /// Loaded from RACECONTROL_ENCRYPTION_KEY and RACECONTROL_HMAC_KEY env vars at startup.
    pub field_cipher: FieldCipher,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool, field_cipher: FieldCipher) -> Self {
        let (dashboard_tx, _) = broadcast::channel(1024);
        let (bono_event_tx, _) = broadcast::channel(256);
        // Extract email alert config before config is moved into the struct
        let email_recipient = config.watchdog.email_recipient.clone();
        let email_script_path = config.watchdog.email_script_path.clone();
        let email_enabled = config.watchdog.email_enabled;
        Self {
            config,
            db,
            pods: RwLock::new(HashMap::new()),
            dashboard_tx,
            bono_event_tx,
            billing: BillingManager::new(),
            game_launcher: GameManager::new(),
            ac_server: AcServerManager::new(),
            port_allocator: PortAllocator::new(9600, 8081, 16),
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
            customer_pin_failures: RwLock::new(HashMap::new()),
            staff_pin_failures: RwLock::new(HashMap::new()),
            ai_peer_tx: RwLock::new(None),
            api_error_counts: Mutex::new(HashMap::new()),
            api_error_counts_reset: Mutex::new(Instant::now()),
            pod_backoffs: RwLock::new(create_initial_backoffs()),
            email_alerter: RwLock::new(EmailAlerter::new(
                email_recipient,
                email_script_path,
                email_enabled,
            )),
            pod_watchdog_states: RwLock::new(create_initial_watchdog_states()),
            pod_needs_restart: RwLock::new(create_initial_needs_restart()),
            pod_manifests: RwLock::new(HashMap::new()),
            pod_deploy_states: RwLock::new(create_initial_deploy_states()),
            pending_deploys: RwLock::new(HashMap::new()),
            assist_cache: RwLock::new(HashMap::new()),
            pending_ws_execs: RwLock::new(HashMap::new()),
            pending_self_tests: RwLock::new(HashMap::new()),
            relay_available: AtomicBool::new(false),
            pod_fleet_health: RwLock::new(HashMap::new()),
            venue_config: RwLock::new(None),
            field_cipher,
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

/// Creates the initial pod_backoffs HashMap pre-populated for pods 1–8.
/// Extracted for testability.
pub fn create_initial_backoffs() -> HashMap<String, EscalatingBackoff> {
    let mut backoffs = HashMap::new();
    for pod_num in 1u32..=8 {
        backoffs.insert(format!("pod_{}", pod_num), EscalatingBackoff::new());
    }
    backoffs
}

/// Creates the initial pod_watchdog_states HashMap pre-populated for pods 1–8.
/// All pods start Healthy — watchdog FSM transitions from there.
pub fn create_initial_watchdog_states() -> HashMap<String, WatchdogState> {
    let mut states = HashMap::new();
    for pod_num in 1u32..=8 {
        states.insert(format!("pod_{}", pod_num), WatchdogState::Healthy);
    }
    states
}

/// Creates the initial pod_needs_restart HashMap pre-populated for pods 1–8.
/// All pods start as false — no restart needed until heartbeat goes stale.
pub fn create_initial_needs_restart() -> HashMap<String, bool> {
    let mut needs = HashMap::new();
    for pod_num in 1u32..=8 {
        needs.insert(format!("pod_{}", pod_num), false);
    }
    needs
}

/// Creates the initial pod_deploy_states HashMap pre-populated for pods 1-8.
/// All pods start Idle -- no deploy in progress.
pub fn create_initial_deploy_states() -> HashMap<String, DeployState> {
    let mut states = HashMap::new();
    for pod_num in 1u32..=8 {
        states.insert(format!("pod_{}", pod_num), DeployState::Idle);
    }
    states
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── WatchdogState tests ──────────────────────────────────────────────────

    #[test]
    fn watchdog_state_healthy_is_default_for_all_8_pods() {
        let states = create_initial_watchdog_states();
        assert_eq!(states.len(), 8);
        for i in 1u32..=8 {
            let key = format!("pod_{}", i);
            assert!(
                matches!(states.get(&key), Some(WatchdogState::Healthy)),
                "pod_{} should default to WatchdogState::Healthy",
                i
            );
        }
    }

    #[test]
    fn watchdog_state_restarting_has_attempt_and_started_at() {
        let now = chrono::Utc::now();
        let s = WatchdogState::Restarting { attempt: 2, started_at: now };
        match s {
            WatchdogState::Restarting { attempt, started_at } => {
                assert_eq!(attempt, 2);
                assert_eq!(started_at, now);
            }
            _ => panic!("Expected Restarting variant"),
        }
    }

    #[test]
    fn watchdog_state_verifying_has_attempt_and_started_at() {
        let now = chrono::Utc::now();
        let s = WatchdogState::Verifying { attempt: 1, started_at: now };
        match s {
            WatchdogState::Verifying { attempt, started_at } => {
                assert_eq!(attempt, 1);
                assert_eq!(started_at, now);
            }
            _ => panic!("Expected Verifying variant"),
        }
    }

    #[test]
    fn watchdog_state_recovery_failed_has_attempt_and_failed_at() {
        let now = chrono::Utc::now();
        let s = WatchdogState::RecoveryFailed { attempt: 4, failed_at: now };
        match s {
            WatchdogState::RecoveryFailed { attempt, failed_at } => {
                assert_eq!(attempt, 4);
                assert_eq!(failed_at, now);
            }
            _ => panic!("Expected RecoveryFailed variant"),
        }
    }

    #[test]
    fn pod_needs_restart_pre_populated_false_for_8_pods() {
        let needs = create_initial_needs_restart();
        assert_eq!(needs.len(), 8);
        for i in 1u32..=8 {
            let key = format!("pod_{}", i);
            assert_eq!(
                needs.get(&key),
                Some(&false),
                "pod_{} should default to false",
                i
            );
        }
    }

    // ── Deploy state tests ───────────────────────────────────────────────────

    #[test]
    fn create_initial_deploy_states_has_8_entries_all_idle() {
        let states = create_initial_deploy_states();
        assert_eq!(states.len(), 8);
        for i in 1u32..=8 {
            let key = format!("pod_{}", i);
            assert_eq!(
                states.get(&key),
                Some(&DeployState::Idle),
                "pod_{} should default to DeployState::Idle",
                i
            );
        }
    }

    // ── Backoff tests (existing) ─────────────────────────────────────────────

    #[test]
    fn create_initial_backoffs_has_exactly_8_entries() {
        let backoffs = create_initial_backoffs();
        assert_eq!(backoffs.len(), 8, "Expected exactly 8 pod backoff entries");
    }

    #[test]
    fn create_initial_backoffs_keyed_pod_1_through_pod_8() {
        let backoffs = create_initial_backoffs();
        for i in 1u32..=8 {
            let key = format!("pod_{}", i);
            assert!(backoffs.contains_key(&key), "Missing key: {}", key);
        }
    }

    #[test]
    fn create_initial_backoffs_each_entry_starts_at_attempt_zero() {
        let backoffs = create_initial_backoffs();
        for i in 1u32..=8 {
            let key = format!("pod_{}", i);
            let backoff = backoffs.get(&key).unwrap();
            // Use public API: attempt() method and ready() — a fresh backoff is always ready
            assert_eq!(backoff.attempt(), 0, "pod_{} should start at attempt 0", i);
            assert!(backoff.ready(chrono::Utc::now()), "pod_{} should have no prior attempt (always ready)", i);
        }
    }

    #[test]
    fn create_initial_backoffs_pod_5_is_some() {
        let backoffs = create_initial_backoffs();
        assert!(backoffs.get("pod_5").is_some(), "pod_5 should be pre-populated");
    }

    #[test]
    fn or_insert_with_returns_existing_entry_not_duplicate() {
        let mut backoffs = create_initial_backoffs();
        // Simulate what pod_monitor does: entry().or_insert_with()
        // Record an attempt on the pre-existing entry
        {
            let entry = backoffs.entry("pod_3".to_string()).or_insert_with(EscalatingBackoff::new);
            entry.record_attempt(chrono::Utc::now());
        }
        // Re-access — should still be at attempt 1 (existing entry was mutated, not replaced)
        let val = backoffs.get("pod_3").unwrap();
        assert_eq!(val.attempt(), 1, "or_insert_with should return pre-existing entry, not a new one");
    }
}
