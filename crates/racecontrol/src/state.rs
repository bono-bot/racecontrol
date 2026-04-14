use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::api::survival::LeaseManager;
use crate::flags::FeatureFlagRow;

use crate::ac_camera::CameraController;
use crate::ac_server::AcServerManager;
use crate::billing::BillingManager;
use crate::cascade_guard::CascadeGuard;
use crate::config::Config;
use crate::crypto::encryption::FieldCipher;
use crate::email_alerts::EmailAlerter;
use crate::fleet_health::{FleetHealthStore, ViolationStore};
use crate::recovery::{RecoveryEventStore, RecoveryIntentStore};
use crate::game_launcher::GameManager;
use crate::port_allocator::PortAllocator;
use rc_common::protocol::{AiChannelMessage, CoreMessage, CoreToAgentMessage, DashboardEvent};
use rc_common::recovery::ProcessOwnership;
use rc_common::types::{ContentManifest, DeployState, PodInfo};
use rc_common::watchdog::EscalatingBackoff;

// ── Extracted submodules ─────────────────────────────────────────────────

#[path = "state_types.rs"]
mod types;
pub use types::*;

#[path = "state_methods.rs"]
mod methods;

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;

// ── AppState ─────────────────────────────────────────────────────────────

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
    /// Map of pod_id -> sender for pushing commands to specific agents.
    /// Phase 312: Changed from Sender<CoreToAgentMessage> to Sender<CoreMessage> so callers
    /// control the command_id for ACK correlation.
    pub agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreMessage>>>,
    /// Map of pod_id -> connection ID (monotonic counter) to detect stale disconnects
    pub agent_conn_ids: RwLock<HashMap<String, u64>>,
    /// v22.0 Phase 177: In-memory feature flag cache. Populated from DB at startup.
    pub feature_flags: RwLock<HashMap<String, FeatureFlagRow>>,
    /// v22.0 Phase 177: Monotonic sequence counter for config push queue entries.
    /// Initialized from MAX(seq_num) in config_push_queue + 1 on startup.
    pub config_push_seq: AtomicU64,
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
    /// Phase 304: Active fleet deploy session (deploy_id, waves, rollback events).
    /// In-memory only — does not persist across server restarts (per locked decision).
    /// Arc so the background task can hold a reference independently of the full AppState.
    pub fleet_deploy_session: std::sync::Arc<RwLock<Option<crate::fleet_deploy::FleetDeploySession>>>,
    /// Per-pod cached assist state (abs, tc, auto_shifter, ffb_percent).
    /// Updated by WS handlers on AssistChanged/FfbGainChanged/AssistState messages.
    /// Read by GET /pods/{pod_id}/assist-state to return real values immediately.
    pub assist_cache: RwLock<HashMap<String, CachedAssistState>>,
    /// Pending WebSocket command responses: request_id -> oneshot sender.
    /// Populated by ws_exec_on_pod(), resolved by ExecResult handler in ws/mod.rs.
    /// Cleaned up on agent disconnect (entries prefixed with pod_id).
    pub pending_ws_execs: RwLock<HashMap<String, tokio::sync::oneshot::Sender<WsExecResult>>>,
    /// Phase 312: Pending command ACK responses: command_id -> oneshot sender.
    /// Populated by launch_game/stop_game, resolved by CommandAck handler in ws/mod.rs.
    pub pending_command_acks: RwLock<HashMap<String, tokio::sync::oneshot::Sender<CommandAckResult>>>,
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
    /// Phase 104: Per-pod process violation history. Keyed by pod_id.
    /// Capped at 100 entries per pod. Never cleared on disconnect.
    pub pod_violations: RwLock<HashMap<String, ViolationStore>>,
    // Services health is sourced from app_health_monitor::get_current_health() — no AppState field needed.
    /// Latest venue config snapshot from on-premise server (via comms-link sync_push).
    /// None until first config_snapshot received.
    pub venue_config: RwLock<Option<VenueConfigSnapshot>>,
    /// Field-level encryption cipher for PII columns (AES-256-GCM + HMAC-SHA256).
    /// Loaded from RACECONTROL_ENCRYPTION_KEY and RACECONTROL_HMAC_KEY env vars at startup.
    pub field_cipher: FieldCipher,
    /// Phase 141: Last time the WARN log scanner triggered an AI escalation.
    /// Used to enforce cooldown (10 min) between escalations while threshold stays breached.
    pub warn_scanner_last_escalated: RwLock<Option<chrono::DateTime<chrono::Utc>>>,
    /// Phase 159: Anti-cascade guard — counts recovery actions in 60s sliding window.
    /// Pauses all automated recovery if 3+ different systems act within the window.
    /// WhatsApp alert fires to Uday when threshold is crossed.
    pub cascade_guard: std::sync::Arc<std::sync::Mutex<CascadeGuard>>,
    /// Phase 183: In-memory ring buffer for recovery events (COORD-04).
    /// All recovery authorities POST events here; pod_healer/rc-sentry query before acting.
    pub recovery_events: std::sync::Mutex<RecoveryEventStore>,
    /// Phase 185: Process ownership registry (COORD-01).
    /// Maps process names to the single recovery authority that owns them.
    /// rc-agent.exe is owned by RcSentry; pod-level WoL actions are owned by PodHealer.
    pub process_ownership: std::sync::Mutex<ProcessOwnership>,
    /// Phase 185: Recovery intent store with 2-minute TTL (COORD-02).
    /// An authority registers its intent before acting; others check before acting.
    /// Prevents simultaneous recovery by two authorities on the same pod+process.
    pub recovery_intents: std::sync::Mutex<RecoveryIntentStore>,
    /// FSM-02: Phantom billing detector — tracks when billing=active + game=Idle condition started.
    /// Key: pod_id. Value: Instant when the phantom billing condition was first detected.
    /// Cleared when the condition resolves (game starts running or billing ends).
    pub phantom_billing_start: RwLock<HashMap<String, Instant>>,
    /// Phase 255: Display machine heartbeats (display_id -> last_ping_instant, uptime_s).
    /// Updated by POST /api/v1/kiosk/ping from leaderboard display pages.
    pub display_heartbeats: RwLock<HashMap<String, (Instant, u64)>>,
    /// v29.0 Phase 27: Pod availability map for kiosk/PWA/POS.
    /// Updated by anomaly scanner via self-healing orchestration.
    pub pod_availability: crate::self_healing::PodAvailabilityMap,
    /// Phase 253: Sender for driver rating computation requests.
    /// None until rating worker is spawned in main.rs.
    pub rating_tx: Option<mpsc::Sender<crate::driver_rating::RatingRequest>>,
    /// Phase 251: Sender for telemetry frames → TelemetryWriter background task.
    /// None if telemetry persistence is not initialized.
    pub telemetry_writer_tx: Option<mpsc::Sender<rc_common::types::TelemetryFrame>>,
    /// Phase 251: Separate SqlitePool for telemetry.db (read queries).
    /// None if telemetry DB is not initialized.
    pub telemetry_db: Option<sqlx::SqlitePool>,
    /// Phase 267-02: Server-arbitrated heal lease manager (SF-02).
    /// Grants exclusive healing access per pod to one survival layer at a time.
    /// Expired leases are auto-freed on next request. Never hold lock across .await.
    pub lease_manager: std::sync::Arc<LeaseManager>,
    /// Phase 274: Tier 5 WhatsApp escalation handler.
    /// Receives EscalationRequest from pods, deduplicates, sends via Bono relay.
    pub whatsapp_escalation: std::sync::Arc<crate::whatsapp_escalation::WhatsAppEscalation>,
    /// Phase 283: Nonce store for billing replay protection (session_id -> nonce + TTL).
    pub billing_nonce_store: std::sync::Arc<crate::billing_replay::NonceStore>,
    /// Phase 300: Backup pipeline status — updated each tick for downstream API consumers.
    pub backup_status: RwLock<BackupStatus>,
    /// Phase 307: Last hash in the pod_activity_log SHA-256 chain.
    /// Initialized from DB at startup (most recent entry_hash), or "GENESIS" if no hashed entries.
    /// Held briefly inside tokio::spawn to serialize hash chain writes. Never held across .await.
    pub audit_last_hash: std::sync::Mutex<String>,
    /// Phase 317 (LAUNCH-04): Consecutive launch failure tracker per pod+SimType.
    /// Key: "{pod_id}:{sim_type:?}". Resets on success or 10-min window expiry.
    /// Never held across .await — clone/snapshot before async work.
    pub chain_failure_tracker: RwLock<HashMap<String, ChainFailureState>>,
    /// Phase 334: Race weekend session progression manager.
    /// Tracks active weekends (Practice → Qualifying → Race) with background polling.
    pub weekend: crate::weekend::WeekendManager,
    /// Phase 335: Track outline cache for spectator circuit viewer.
    /// Parsed from AC fast_lane.ai files, normalized to 0-1 range.
    pub track_outlines: crate::track_outline::TrackOutlineCache,
    /// Phase 368: In-memory launch status card store.
    /// Tracks the lifecycle of every game launch from /games/launch entry to "playable".
    /// State machine transitions emit DashboardEvent::LaunchStatusChanged to kiosk subscribers.
    /// No DB persistence for active state — durable history is in launch_timeline_spans.events_json.
    pub launch_state_machine: std::sync::Arc<crate::launch_state::LaunchStateMachine>,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool, field_cipher: FieldCipher) -> Self {
        let (dashboard_tx, _) = broadcast::channel(1024);
        let (bono_event_tx, _) = broadcast::channel(256);
        // Extract email alert config before config is moved into the struct
        let email_recipient = config.watchdog.email_recipient.clone();
        let email_script_path = config.watchdog.email_script_path.clone();
        let email_enabled = config.watchdog.email_enabled;
        // Build shared HTTP client for use in AppState and cascade_guard
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");
        // Extract alert config for cascade_guard before config is moved into the struct
        let cascade_alert_cfg = crate::cascade_guard::CascadeAlertConfig::from_config(&config);
        let cascade_guard_inner = CascadeGuard::new(cascade_alert_cfg, http_client.clone());
        // Phase 274: WhatsApp escalation handler — create before http_client is moved
        let whatsapp_escalation_handler = std::sync::Arc::new(
            crate::whatsapp_escalation::WhatsAppEscalation::new(http_client.clone())
        );
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
            feature_flags: RwLock::new(HashMap::new()),
            config_push_seq: AtomicU64::new(1),
            http_client,
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
            fleet_deploy_session: std::sync::Arc::new(RwLock::new(None)),
            assist_cache: RwLock::new(HashMap::new()),
            pending_ws_execs: RwLock::new(HashMap::new()),
            pending_command_acks: RwLock::new(HashMap::new()),
            pending_self_tests: RwLock::new(HashMap::new()),
            relay_available: AtomicBool::new(false),
            pod_fleet_health: RwLock::new(HashMap::new()),
            pod_violations: RwLock::new(HashMap::new()),
            venue_config: RwLock::new(None),
            field_cipher,
            warn_scanner_last_escalated: RwLock::new(None),
            cascade_guard: std::sync::Arc::new(std::sync::Mutex::new(cascade_guard_inner)),
            recovery_events: std::sync::Mutex::new(RecoveryEventStore::new()),
            process_ownership: {
                let mut ownership = ProcessOwnership::new();
                // rc-agent.exe is owned by RcSentry (pod-side authority).
                // PodHealer must not restart rc-agent.exe directly; it uses WoL (machine-level).
                let _ = ownership.register("rc-agent.exe", rc_common::recovery::RecoveryAuthority::RcSentry);
                std::sync::Mutex::new(ownership)
            },
            recovery_intents: std::sync::Mutex::new(RecoveryIntentStore::new()),
            phantom_billing_start: RwLock::new(HashMap::new()),
            display_heartbeats: RwLock::new(HashMap::new()),
            pod_availability: crate::self_healing::new_availability_map(),
            rating_tx: None,           // Initialized after AppState::new() in main.rs
            telemetry_writer_tx: None, // Initialized after AppState::new() in main.rs
            telemetry_db: None,        // Initialized after AppState::new() in main.rs
            lease_manager: std::sync::Arc::new(LeaseManager::new()),
            whatsapp_escalation: whatsapp_escalation_handler,
            billing_nonce_store: std::sync::Arc::new(crate::billing_replay::NonceStore::new()),
            backup_status: RwLock::new(BackupStatus::default()),
            // Phase 307: Initialized to GENESIS; main.rs loads actual last hash from DB after init_db()
            audit_last_hash: std::sync::Mutex::new("GENESIS".to_string()),
            chain_failure_tracker: RwLock::new(HashMap::new()),
            weekend: crate::weekend::WeekendManager::new(),
            track_outlines: crate::track_outline::TrackOutlineCache::new(
                std::path::PathBuf::from("data/track_outlines")
            ),
            launch_state_machine: std::sync::Arc::new(crate::launch_state::LaunchStateMachine::new()),
        }
    }

    /// Build an HTTP request to a rc-sentry protected endpoint, including the
    /// X-Service-Key header when `pods.sentry_service_key` is configured.
    pub fn sentry_post(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.http_client.post(url);
        if let Some(key) = &self.config.pods.sentry_service_key {
            req = req.header("X-Service-Key", key);
        }
        req
    }

    /// Build a GET request to a rc-sentry protected endpoint with auth header.
    pub fn sentry_get(&self, url: &str) -> reqwest::RequestBuilder {
        let mut req = self.http_client.get(url);
        if let Some(key) = &self.config.pods.sentry_service_key {
            req = req.header("X-Service-Key", key);
        }
        req
    }
}

// ── Initializer helpers ──────────────────────────────────────────────────

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
