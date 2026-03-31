use serde::Deserialize;
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub venue: VenueConfig,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    #[serde(default)]
    pub cloud: CloudConfig,
    #[serde(default)]
    pub pods: PodsConfig,
    #[serde(default)]
    pub branding: BrandingConfig,
    #[serde(default)]
    pub integrations: IntegrationsConfig,
    #[serde(default)]
    pub ai_debugger: AiDebuggerConfig,
    #[serde(default)]
    pub ac_server: AcServerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub watchdog: WatchdogConfig,
    #[serde(default)]
    pub bono: BonoConfig,
    #[serde(default)]
    pub gmail: GmailConfig,
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    #[serde(default)]
    pub alerting: AlertingConfig,
    #[serde(default)]
    pub process_guard: ProcessGuardConfig,
    #[serde(default)]
    pub cafe: CafeConfig,
    #[serde(default)]
    pub billing: BillingConfig,
    #[serde(default)]
    pub mma: MmaConfig,
}

/// MMA-First Protocol config (v29.0+) — 30-day AI training period settings
#[derive(Clone, Debug, Default, Deserialize)]
pub struct MmaConfig {
    #[serde(default)]
    pub training_mode: bool,
    pub training_start: Option<String>,
    pub training_end: Option<String>,
    #[serde(default = "default_daily_budget_pod")]
    pub daily_budget_pod: f64,
    #[serde(default = "default_daily_budget_server")]
    pub daily_budget_server: f64,
    #[serde(default = "default_daily_budget_pos")]
    pub daily_budget_pos: f64,
}

fn default_daily_budget_pod() -> f64 { 15.0 }
fn default_daily_budget_server() -> f64 { 25.0 }
fn default_daily_budget_pos() -> f64 { 8.0 }

/// Gmail API config for sending notification emails (track record beaten, etc.)
/// Uses OAuth2 refresh_token flow — no external script needed.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct GmailConfig {
    #[serde(default)]
    pub enabled: bool,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    #[serde(default = "default_gmail_from")]
    pub from_email: String,
}

fn default_gmail_from() -> String { "james@racingpoint.in".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct VenueConfig {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
    /// GST Identification Number (15-char alphanumeric). Used for invoice generation.
    #[serde(default = "default_venue_gstin")]
    pub venue_gstin: String,
}

fn default_venue_gstin() -> String {
    "36PLACEHOLDER0Z0".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// HTTPS port. When set, enables TLS listener alongside HTTP.
    #[serde(default)]
    pub tls_port: Option<u16>,
    /// Path to TLS certificate PEM file. Auto-generated if missing.
    #[serde(default)]
    pub cert_path: Option<String>,
    /// Path to TLS private key PEM file. Auto-generated if missing.
    #[serde(default)]
    pub key_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct CloudConfig {
    #[serde(default)]
    pub enabled: bool,
    pub turso_url: Option<String>,
    /// Base URL for the cloud racecontrol API (e.g., "https://app.racingpoint.cloud/api/v1")
    pub api_url: Option<String>,
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
    /// How often to poll for cloud actions (default: 3 seconds)
    #[serde(default = "default_action_poll_interval")]
    pub action_poll_interval_secs: u64,
    /// Shared secret for terminal command access
    pub terminal_secret: Option<String>,
    /// PIN for terminal web UI authentication (only Uday knows this)
    pub terminal_pin: Option<String>,
    /// Localhost URL for the comms-link relay (e.g., "http://localhost:8765" on cloud,
    /// "http://localhost:8766" on venue). When set, sync routes through the relay for
    /// real-time 2s sync instead of 30s HTTP polling.
    #[serde(default)]
    pub comms_link_url: Option<String>,
    /// HMAC-SHA256 key for signing cloud sync payloads (AUTH-07).
    /// When set, outbound sync requests include x-sync-signature, x-sync-timestamp,
    /// x-sync-nonce headers. Inbound requests are verified (permissive mode initially).
    #[serde(default)]
    pub sync_hmac_key: Option<String>,
    /// Identity of this racecontrol instance for sync origin tagging.
    /// Set to "local" on venue server, "cloud" on VPS. Prevents sync loops.
    #[serde(default = "default_origin_local")]
    pub origin_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PodsConfig {
    #[serde(default = "default_pod_count")]
    pub count: u32,
    #[serde(default = "default_true")]
    pub discovery: bool,
    #[serde(default, rename = "static")]
    pub static_pods: Vec<StaticPodConfig>,
    #[serde(default = "default_true")]
    pub healer_enabled: bool,
    #[serde(default = "default_healer_interval")]
    pub healer_interval_secs: u32,
    /// Service key for authenticating with rc-sentry :8091/exec on pods.
    /// Must match the RCSENTRY_SERVICE_KEY env var set on each pod.
    #[serde(default)]
    pub sentry_service_key: Option<String>,
}

impl Default for PodsConfig {
    fn default() -> Self {
        Self {
            count: 16,
            discovery: true,
            static_pods: Vec::new(),
            healer_enabled: true,
            healer_interval_secs: default_healer_interval(),
            sentry_service_key: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct StaticPodConfig {
    pub number: u32,
    pub name: String,
    pub ip: String,
    pub sim: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct BrandingConfig {
    pub logo: Option<String>,
    #[serde(default = "default_color")]
    pub primary_color: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct IntegrationsConfig {
    #[serde(default)]
    pub discord: DiscordConfig,
    #[serde(default)]
    pub whatsapp: WhatsAppConfig,
    /// HMAC secret for payment gateway webhook signature verification.
    /// When set, /webhooks/payment-gateway requires X-Webhook-Signature header.
    #[serde(default)]
    pub payment_webhook_secret: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: Option<String>,
    pub results_channel: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    pub contact: Option<String>,
    /// Evolution API URL for marketing/promotional messages (broadcasts, nudges, campaigns).
    /// Should point directly to Bono VPS (e.g., "http://100.70.177.44:53622").
    /// Falls back to auth.evolution_url if not set.
    pub marketing_url: Option<String>,
    /// API key for the marketing Evolution instance. Falls back to auth.evolution_api_key.
    pub marketing_api_key: Option<String>,
    /// Instance name for marketing messages. Falls back to auth.evolution_instance.
    pub marketing_instance: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AiDebuggerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub claude_cli_enabled: bool,
    #[serde(default = "default_claude_cli_timeout")]
    pub claude_cli_timeout_secs: u32,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_ollama_model")]
    pub ollama_model: String,
    pub anthropic_api_key: Option<String>,
    #[serde(default = "default_anthropic_model")]
    pub anthropic_model: String,
    #[serde(default = "default_true")]
    pub chat_enabled: bool,
    #[serde(default = "default_true")]
    pub proactive_analysis: bool,
}

impl Default for AiDebuggerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            claude_cli_enabled: true,
            claude_cli_timeout_secs: default_claude_cli_timeout(),
            ollama_url: default_ollama_url(),
            ollama_model: default_ollama_model(),
            anthropic_api_key: None,
            anthropic_model: default_anthropic_model(),
            chat_enabled: true,
            proactive_analysis: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AcServerConfig {
    #[serde(default = "default_acserver_path")]
    pub acserver_path: String,
    #[serde(default = "default_ac_data_dir")]
    pub data_dir: String,
    pub lan_ip: Option<String>,
}

impl Default for AcServerConfig {
    fn default() -> Self {
        Self {
            acserver_path: default_acserver_path(),
            data_dir: default_ac_data_dir(),
            lan_ip: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_jwt_secret")]
    pub jwt_secret: String,
    #[serde(default = "default_pin_expiry")]
    pub pin_expiry_secs: u64,
    #[serde(default = "default_otp_expiry")]
    pub otp_expiry_secs: u64,
    pub evolution_url: Option<String>,
    pub evolution_api_key: Option<String>,
    pub evolution_instance: Option<String>,
    /// MMA-P3: Previous JWT secret for rotation grace period.
    /// When rotating jwt_secret, set this to the OLD secret so existing tokens
    /// remain valid until they expire naturally. Remove after 24h.
    #[serde(default)]
    pub jwt_secret_previous: Option<String>,
    /// Argon2id hash of the admin PIN. When set, enables the admin login endpoint.
    /// Set via config file or RACECONTROL_ADMIN_PIN_HASH env var.
    #[serde(default)]
    pub admin_pin_hash: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            jwt_secret_previous: None,
            pin_expiry_secs: default_pin_expiry(),
            otp_expiry_secs: default_otp_expiry(),
            evolution_url: None,
            evolution_api_key: None,
            evolution_instance: None,
            admin_pin_hash: None,
        }
    }
}

/// Message category determines which Evolution API instance to route through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatsAppCategory {
    /// OTP, receipts, alerts, operational notifications — venue tunnel OK
    Operational,
    /// Broadcasts, promotions, deals, nudges, campaigns — must use Bono VPS
    Marketing,
}

/// Resolved Evolution API credentials for a given message category.
pub struct EvolutionCredentials {
    pub url: String,
    pub api_key: String,
    pub instance: String,
}

impl Config {
    /// Resolve Evolution API credentials by message category.
    /// Marketing messages use `integrations.whatsapp.marketing_*` if configured,
    /// falling back to `auth.evolution_*`. This ensures marketing can be routed
    /// through Bono VPS while operational messages stay on the venue tunnel.
    pub fn evolution_for(&self, category: WhatsAppCategory) -> Option<EvolutionCredentials> {
        match category {
            WhatsAppCategory::Operational => {
                Some(EvolutionCredentials {
                    url: self.auth.evolution_url.clone()?,
                    api_key: self.auth.evolution_api_key.clone()?,
                    instance: self.auth.evolution_instance.clone()?,
                })
            }
            WhatsAppCategory::Marketing => {
                let wa = &self.integrations.whatsapp;
                // Use dedicated marketing config if set, otherwise fall back to operational
                let url = wa.marketing_url.as_ref()
                    .or(self.auth.evolution_url.as_ref())?.clone();
                let key = wa.marketing_api_key.as_ref()
                    .or(self.auth.evolution_api_key.as_ref())?.clone();
                let inst = wa.marketing_instance.as_ref()
                    .or(self.auth.evolution_instance.as_ref())?.clone();
                Some(EvolutionCredentials { url, api_key: key, instance: inst })
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WatchdogConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_watchdog_interval")]
    pub check_interval_secs: u64,
    #[serde(default = "default_heartbeat_timeout")]
    pub heartbeat_timeout_secs: i64,
    #[serde(default = "default_restart_cooldown")]
    pub restart_cooldown_secs: i64,
    #[serde(default = "default_false")]
    pub email_enabled: bool,
    #[serde(default = "default_email_recipient")]
    pub email_recipient: String,
    #[serde(default = "default_email_script_path")]
    pub email_script_path: String,
    #[serde(default = "default_email_pod_cooldown")]
    pub email_pod_cooldown_secs: i64,
    #[serde(default = "default_email_venue_cooldown")]
    pub email_venue_cooldown_secs: i64,
    #[serde(default)]
    pub escalation_steps_secs: Vec<u64>,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            check_interval_secs: default_watchdog_interval(),
            heartbeat_timeout_secs: default_heartbeat_timeout(),
            restart_cooldown_secs: default_restart_cooldown(),
            email_enabled: false,
            email_recipient: default_email_recipient(),
            email_script_path: default_email_script_path(),
            email_pod_cooldown_secs: default_email_pod_cooldown(),
            email_venue_cooldown_secs: default_email_venue_cooldown(),
            escalation_steps_secs: Vec::new(),
        }
    }
}

/// Configuration for server-side monitoring and alerting.
#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    /// Number of ERROR events in window that triggers alert (default: 5)
    #[serde(default = "default_error_rate_threshold")]
    pub error_rate_threshold: usize,
    /// Sliding window duration in seconds (default: 60)
    #[serde(default = "default_error_rate_window_secs")]
    pub error_rate_window_secs: u64,
    /// Cooldown between error rate alerts in seconds (default: 1800 = 30 min)
    #[serde(default = "default_error_rate_cooldown_secs")]
    pub error_rate_cooldown_secs: u64,
    /// Enable error rate email alerting (default: false)
    #[serde(default)]
    pub error_rate_email_enabled: bool,
}

fn default_error_rate_threshold() -> usize { 5 }
fn default_error_rate_window_secs() -> u64 { 60 }
fn default_error_rate_cooldown_secs() -> u64 { 1800 }

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            error_rate_threshold: 5,
            error_rate_window_secs: 60,
            error_rate_cooldown_secs: 1800,
            error_rate_email_enabled: false,
        }
    }
}

/// Configuration for WhatsApp P0 alerting to Uday.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AlertingConfig {
    /// Enable WhatsApp P0 alerting (default: false)
    #[serde(default)]
    pub enabled: bool,
    /// Uday's WhatsApp number in Evolution API format (e.g., "919876543210")
    pub uday_phone: Option<String>,
    /// Cooldown between same-type P0 alerts in seconds (default: 1800 = 30 min)
    #[serde(default = "default_alert_cooldown")]
    pub cooldown_secs: u64,
}

fn default_alert_cooldown() -> u64 { 1800 }

// ─── Cafe Config ─────────────────────────────────────────────────────────────

/// Configuration for cafe-related features.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CafeConfig {
    /// Path to the Node.js thermal receipt print script.
    /// If None, thermal printing is silently skipped.
    #[serde(default)]
    pub print_script_path: Option<String>,
}

// ─── Billing Config ───────────────────────────────────────────────────────────

/// Configurable timeouts for the billing system (BILL-12).
/// All fields have serde defaults so adding [billing] to racecontrol.toml is optional.
#[derive(Debug, Clone, Deserialize)]
pub struct BillingConfig {
    /// How long to wait for multiplayer pods to all reach LIVE before evicting (seconds). Default: 60.
    #[serde(default = "default_multiplayer_wait_timeout")]
    pub multiplayer_wait_timeout_secs: u64,
    /// How long a game-pause can last before billing session auto-ends (seconds). Default: 600.
    #[serde(default = "default_pause_auto_end_timeout")]
    pub pause_auto_end_timeout_secs: u32,
    /// Per-attempt timeout waiting for PlayableSignal (seconds). 2 attempts = 2x this. Default: 180.
    #[serde(default = "default_launch_timeout_per_attempt")]
    pub launch_timeout_per_attempt_secs: u64,
    /// Seconds of no driving input before billing anomaly flagged. Default: 300.
    #[serde(default = "default_idle_drift_threshold")]
    pub idle_drift_threshold_secs: u64,
    /// Grace period before auto-ending session when pod goes offline (seconds). Default: 300.
    #[serde(default = "default_offline_grace")]
    pub offline_grace_secs: u64,
}

fn default_multiplayer_wait_timeout() -> u64 { 60 }
fn default_pause_auto_end_timeout() -> u32 { 600 }
fn default_launch_timeout_per_attempt() -> u64 { 180 }
fn default_idle_drift_threshold() -> u64 { 300 }
fn default_offline_grace() -> u64 { 300 }

impl Default for BillingConfig {
    fn default() -> Self {
        Self {
            multiplayer_wait_timeout_secs: default_multiplayer_wait_timeout(),
            pause_auto_end_timeout_secs: default_pause_auto_end_timeout(),
            launch_timeout_per_attempt_secs: default_launch_timeout_per_attempt(),
            idle_drift_threshold_secs: default_idle_drift_threshold(),
            offline_grace_secs: default_offline_grace(),
        }
    }
}

// ─── Process Guard Config ──────────────────────────────────────────────────

/// A single allowed process entry in the whitelist.
#[derive(Debug, Clone, Deserialize)]
pub struct AllowedProcess {
    /// Process name (exact match, case-insensitive). Supports simple * wildcard prefix/suffix.
    pub name: String,
    /// Category tag: "system", "racecontrol", "game", "peripheral", "ollama", "development", "monitoring"
    pub category: String,
    /// Which machine types this entry applies to. Values: "all", "pod", "james", "server".
    #[serde(default)]
    pub machines: Vec<String>,
}

/// Per-machine process guard overrides (additive allow + deny lists).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProcessGuardOverride {
    /// Process names to allow in addition to the global list.
    #[serde(default)]
    pub allow_extra_processes: Vec<String>,
    /// Port numbers to allow in addition to the global list.
    #[serde(default)]
    pub allow_extra_ports: Vec<u16>,
    /// Autostart key names to allow in addition to the global list.
    #[serde(default)]
    pub allow_extra_autostart: Vec<String>,
    /// Process names explicitly denied even if they appear in the global list.
    #[serde(default)]
    pub deny_processes: Vec<String>,
}

/// Top-level [process_guard] configuration section.
#[derive(Debug, Clone, Deserialize)]
pub struct ProcessGuardConfig {
    /// Enable the process guard. Default: false (safe rollout).
    #[serde(default)]
    pub enabled: bool,
    /// Process scan interval in seconds. Default: 60.
    #[serde(default = "default_poll_interval_secs")]
    pub poll_interval_secs: u64,
    /// Enforcement mode: "report_only" or "kill_and_report". Default: "report_only".
    #[serde(default = "default_violation_action")]
    pub violation_action: String,
    /// If true, only warn on first consecutive sighting — kill on second. Default: true.
    #[serde(default = "default_true")]
    pub warn_before_kill: bool,
    /// Global allowed process list (applies to all machines unless overridden).
    #[serde(default)]
    pub allowed: Vec<AllowedProcess>,
    /// Per-machine overrides. Keys: "james", "pod", "server".
    #[serde(default)]
    pub overrides: std::collections::HashMap<String, ProcessGuardOverride>,
    /// Shared secret for POST /api/v1/guard/report from rc-process-guard (James).
    /// If None, accepts all requests (dev mode). Always set in production.
    #[serde(default)]
    pub report_secret: Option<String>,
}

impl Default for ProcessGuardConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_secs: default_poll_interval_secs(),
            violation_action: default_violation_action(),
            warn_before_kill: true,
            allowed: Vec::new(),
            overrides: std::collections::HashMap::new(),
            report_secret: None,
        }
    }
}

fn default_poll_interval_secs() -> u64 { 60 }
fn default_violation_action() -> String { "report_only".to_string() }

/// Configuration for the Bono relay: event push to Bono's VPS over Tailscale mesh,
/// and inbound relay endpoint for commands from Bono's cloud.
#[derive(Debug, Clone, Deserialize)]
pub struct BonoConfig {
    /// Set to true to enable Bono event push and relay endpoint.
    #[serde(default)]
    pub enabled: bool,
    /// Bono's VPS webhook URL on the Tailscale mesh (e.g. "http://100.x.x.x/webhooks/racecontrol").
    /// Leave None until Bono's Tailscale IP is known.
    pub webhook_url: Option<String>,
    /// Server's own Tailscale IP to bind relay endpoint on (e.g. "100.y.y.y").
    pub tailscale_bind_ip: Option<String>,
    /// Port for Bono relay endpoint. Must NOT be in the AC server HTTP port range (8081-8096).
    #[serde(default = "default_relay_port")]
    pub relay_port: u16,
    /// Shared secret Bono sends in X-Relay-Secret header for inbound command auth.
    pub relay_secret: Option<String>,
}

fn default_relay_port() -> u16 { 8099 }

impl Default for BonoConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: None,
            tailscale_bind_ip: None,
            relay_port: default_relay_port(),
            relay_secret: None,
        }
    }
}

/// Resolve JWT signing secret: env var > config value > auto-generate.
/// The dangerous default "racingpoint-jwt-change-me-in-production" is treated as unset.
fn resolve_jwt_secret(config_value: &str) -> String {
    // 1. Environment variable takes priority
    if let Ok(key) = std::env::var("RACECONTROL_JWT_SECRET") {
        if !key.is_empty() {
            tracing::info!("Using JWT secret from RACECONTROL_JWT_SECRET env var");
            return key;
        }
    }
    // 2. Config file value (if not the dangerous default and not empty)
    if config_value != "racingpoint-jwt-change-me-in-production" && !config_value.is_empty() {
        return config_value.to_string();
    }
    // 3. Generate random 256-bit key
    use rand::Rng;
    let key_bytes: [u8; 32] = rand::thread_rng().r#gen();
    let hex_key: String = key_bytes.iter().map(|b| format!("{:02x}", b)).collect();
    tracing::warn!(
        "No JWT secret configured — generated random key. \
         Tokens will be invalidated on restart. \
         Set RACECONTROL_JWT_SECRET env var for persistence."
    );
    hex_key
}

// ─── Verification chain steps for config TOML load (COV-03) ──────────────────

struct StepConfigFileReadable;
impl VerifyStep for StepConfigFileReadable {
    type Input = String;   // file path
    type Output = (String, String);  // (file content, path)
    fn name(&self) -> &str { "file_readable" }
    fn run(&self, input: String) -> Result<(String, String), VerificationError> {
        std::fs::read_to_string(&input)
            .map(|content| (content, input.clone()))
            .map_err(|e| VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("path={} error={}", input, e),
            })
    }
}

struct StepConfigTomlParse;
impl VerifyStep for StepConfigTomlParse {
    type Input = (String, String);  // (content, path)
    type Output = Config;
    fn name(&self) -> &str { "toml_parse" }
    fn run(&self, input: (String, String)) -> Result<Config, VerificationError> {
        let (content, path) = input;
        toml::from_str::<Config>(&content).map_err(|e| {
            // COV-03: Log first 3 lines to help diagnose SSH banner corruption
            let first_3_lines: String = content.lines().take(3).collect::<Vec<_>>().join(" | ");
            VerificationError::InputParseError {
                step: self.name().to_string(),
                raw_value: format!("path={} error={} first_3_lines=[{}]", path, e, first_3_lines),
            }
        })
    }
}

struct StepValidateCriticalFields;
impl VerifyStep for StepValidateCriticalFields {
    type Input = Config;
    type Output = Config;
    fn name(&self) -> &str { "validate_critical_fields" }
    fn run(&self, input: Config) -> Result<Config, VerificationError> {
        // Check that critical fields are not at their default values
        let default = Config::default_config();
        let mut fallbacks = Vec::new();
        if input.database.path == default.database.path {
            fallbacks.push("database.path");
        }
        if !fallbacks.is_empty() {
            // COV-03: Emit TransformError through chain for tracing span capture.
            // Config is still usable — caller catches this as non-fatal warning.
            // Using eprintln as well because tracing may not be initialized during config load.
            eprintln!("[config_validate] fields at default values: {:?}", fallbacks);
            return Err(VerificationError::TransformError {
                step: self.name().to_string(),
                raw_value: format!("fields_at_default={:?}", fallbacks),
            });
        }
        Ok(input)
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        // Build the search path list. Always try:
        //   1. CWD-relative (for dev / explicit cd-before-launch scenarios)
        //   2. Directory of the running executable (reliable for schtasks / HKLM Run / watchdog restarts
        //      where CWD is not guaranteed to match the install directory)
        //   3. /etc/racecontrol/ (Linux/VPS deployments)
        let mut paths: Vec<String> = vec!["racecontrol.toml".to_string()];
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let exe_cfg = exe_dir.join("racecontrol.toml");
                let exe_cfg_str = exe_cfg.to_string_lossy().into_owned();
                // Only add if different from CWD-relative (avoid duplicate on happy path)
                if exe_cfg_str != "racecontrol.toml" {
                    paths.push(exe_cfg_str);
                }
            }
        }
        paths.push("/etc/racecontrol/racecontrol.toml".to_string());

        for path in &paths {
            let chain = ColdVerificationChain::new("config_load");
            // Step 1: Check file is readable
            match chain.execute_step(&StepConfigFileReadable, path.clone()) {
                Ok((content, path_display)) => {
                    // Step 2: Parse TOML
                    match chain.execute_step(&StepConfigTomlParse, (content, path_display.clone())) {
                        Ok(mut config) => {
                            config.apply_env_overrides();
                            // Step 3: Validate critical fields (non-fatal — TransformError means config is usable but has defaults)
                            let validated_config = chain.execute_step(&StepValidateCriticalFields, config.clone());
                            match validated_config {
                                Ok(config) => {
                                    eprintln!("[config] Loaded config from {}", path_display);
                                    tracing::info!("Loaded config from {}", path_display);
                                    return config;
                                }
                                Err(e) => {
                                    // COV-03: TransformError flows through chain tracing span for structured logging.
                                    // Config is still usable — proceed with it but warn.
                                    tracing::warn!(target: "state", error = %e, path = %path_display, "config field validation detected default fallbacks — using config anyway");
                                    eprintln!("[config] Loaded config from {} (with field validation warnings)", path_display);
                                    return config;
                                }
                            }
                        }
                        Err(e) => {
                            // COV-03: VerificationError includes first 3 lines of file for SSH banner diagnosis
                            let msg = format!("[config_parse] field=config_parse source={} error={} fallback=Config::default() — config file parse failed via verification chain", path_display, e);
                            eprintln!("{}", msg);
                            tracing::warn!(target: "state", field = "config_parse", source = %path_display, error = %e, fallback = "Config::default()", "config file parse failed via verification chain — possible SSH banner corruption");
                        }
                    }
                }
                Err(_) => {
                    // File not readable — expected for most search paths, skip silently
                }
            }
        }
        // OBS-02: No config file found — emit structured warn (eprintln always, tracing if initialized)
        let msg = format!("[config_fallback] field=config_file source=racecontrol.toml fallback=Config::default() — config file not found, using defaults");
        eprintln!("{}", msg);
        tracing::warn!(target: "state", field = "config_file", source = "racecontrol.toml", fallback = "Config::default()", "config file not found, using defaults");
        Self::default_config()
    }

    /// Create a default config suitable for tests.
    pub fn default_test() -> Self {
        Self::default_config()
    }

    fn default_config() -> Self {
        Config {
            venue: VenueConfig {
                name: "RacingPoint".to_string(),
                location: default_location(),
                timezone: default_timezone(),
                venue_gstin: default_venue_gstin(),
            },
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
                tls_port: None,
                cert_path: None,
                key_path: None,
            },
            database: DatabaseConfig {
                path: default_db_path(),
            },
            cloud: CloudConfig::default(),
            pods: PodsConfig::default(),
            branding: BrandingConfig::default(),
            integrations: IntegrationsConfig::default(),
            ai_debugger: AiDebuggerConfig::default(),
            ac_server: AcServerConfig::default(),
            auth: AuthConfig::default(),
            watchdog: WatchdogConfig::default(),
            bono: BonoConfig::default(),
            gmail: GmailConfig::default(),
            monitoring: MonitoringConfig::default(),
            alerting: AlertingConfig::default(),
            process_guard: ProcessGuardConfig::default(),
            cafe: CafeConfig::default(),
            billing: BillingConfig::default(),
            mma: MmaConfig::default(),
        }
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(url) = std::env::var("OLLAMA_URL") {
            tracing::info!("Overriding ollama_url from OLLAMA_URL env var");
            self.ai_debugger.ollama_url = url;
        }
        if let Ok(model) = std::env::var("OLLAMA_MODEL") {
            tracing::info!("Overriding ollama_model from OLLAMA_MODEL env var");
            self.ai_debugger.ollama_model = model;
        }
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            tracing::info!("Overriding anthropic_api_key from ANTHROPIC_API_KEY env var");
            self.ai_debugger.anthropic_api_key = Some(key);
        }

        // --- Secret env var overrides (AUDIT-03) ---
        // JWT secret is handled specially via resolve_jwt_secret (supports auto-generation)
        self.auth.jwt_secret = resolve_jwt_secret(&self.auth.jwt_secret);

        if let Ok(val) = std::env::var("RACECONTROL_ADMIN_PIN_HASH") {
            if !val.is_empty() {
                tracing::info!("Overriding admin_pin_hash from RACECONTROL_ADMIN_PIN_HASH env var");
                self.auth.admin_pin_hash = Some(val);
            }
        }

        if let Ok(val) = std::env::var("RACECONTROL_TERMINAL_SECRET") {
            if !val.is_empty() {
                tracing::info!("Overriding terminal_secret from RACECONTROL_TERMINAL_SECRET env var");
                self.cloud.terminal_secret = Some(val);
            }
        }
        if let Ok(val) = std::env::var("RACECONTROL_RELAY_SECRET") {
            if !val.is_empty() {
                tracing::info!("Overriding relay_secret from RACECONTROL_RELAY_SECRET env var");
                self.bono.relay_secret = Some(val);
            }
        }
        if let Ok(val) = std::env::var("RACECONTROL_EVOLUTION_API_KEY") {
            if !val.is_empty() {
                tracing::info!("Overriding evolution_api_key from RACECONTROL_EVOLUTION_API_KEY env var");
                self.auth.evolution_api_key = Some(val);
            }
        }
        if let Ok(val) = std::env::var("RACECONTROL_GMAIL_CLIENT_SECRET") {
            if !val.is_empty() {
                tracing::info!("Overriding gmail.client_secret from RACECONTROL_GMAIL_CLIENT_SECRET env var");
                self.gmail.client_secret = Some(val);
            }
        }
        if let Ok(val) = std::env::var("RACECONTROL_GMAIL_REFRESH_TOKEN") {
            if !val.is_empty() {
                tracing::info!("Overriding gmail.refresh_token from RACECONTROL_GMAIL_REFRESH_TOKEN env var");
                self.gmail.refresh_token = Some(val);
            }
        }
        if let Ok(val) = std::env::var("RACECONTROL_SYNC_HMAC_KEY") {
            if !val.is_empty() {
                tracing::info!("Overriding sync_hmac_key from RACECONTROL_SYNC_HMAC_KEY env var");
                self.cloud.sync_hmac_key = Some(val);
            }
        }
    }
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_db_path() -> String { "./data/racecontrol.db".to_string() }
fn default_location() -> String { "Bandlaguda, Hyderabad".to_string() }
fn default_timezone() -> String { "Asia/Kolkata".to_string() }
fn default_sync_interval() -> u64 { 30 }
fn default_action_poll_interval() -> u64 { 3 }
fn default_origin_local() -> String { "local".to_string() }
fn default_pod_count() -> u32 { 16 }
fn default_true() -> bool { true }
fn default_color() -> String { "#E10600".to_string() }
fn default_theme() -> String { "dark".to_string() }
fn default_acserver_path() -> String { "C:/RacingPoint/ac-server/acServer.exe".to_string() }
fn default_ac_data_dir() -> String { "./data/ac_servers".to_string() }
fn default_jwt_secret() -> String { "racingpoint-jwt-change-me-in-production".to_string() }
fn default_pin_expiry() -> u64 { 600 }
fn default_otp_expiry() -> u64 { 300 }
fn default_watchdog_interval() -> u64 { 10 }
fn default_heartbeat_timeout() -> i64 { 30 }
fn default_restart_cooldown() -> i64 { 120 }
fn default_claude_cli_timeout() -> u32 { 30 }
fn default_ollama_url() -> String { "http://192.168.31.27:11434".to_string() }
fn default_ollama_model() -> String { "qwen2.5:3b".to_string() }
fn default_anthropic_model() -> String { "claude-sonnet-4-20250514".to_string() }
fn default_healer_interval() -> u32 { 120 }
fn default_false() -> bool { false }
fn default_email_recipient() -> String { "usingh@racingpoint.in".to_string() }
fn default_email_script_path() -> String { "send_email.js".to_string() }
fn default_email_pod_cooldown() -> i64 { 1800 }
fn default_email_venue_cooldown() -> i64 { 300 }

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    // SAFETY: These tests mutate environment variables which is inherently unsafe
    // in multi-threaded contexts. ENV_MUTEX serializes all env-var tests within
    // this process so parallel cargo test invocations don't race on set_var/remove_var.
    pub(crate) static ENV_MUTEX: Mutex<()> = Mutex::new(());

    macro_rules! with_env_lock {
        ($body:block) => {{
            let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
            $body
        }};
    }

    #[test]
    fn jwt_secret_from_env_var() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("RACECONTROL_JWT_SECRET", "env-secret-123"); }
        let result = resolve_jwt_secret("config-value");
        assert_eq!(result, "env-secret-123");
        unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
    }

    #[test]
    fn jwt_secret_from_config_when_no_env() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
        let result = resolve_jwt_secret("my-custom-secret");
        assert_eq!(result, "my-custom-secret");
    }

    #[test]
    fn jwt_secret_rejects_dangerous_default() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
        let result = resolve_jwt_secret("racingpoint-jwt-change-me-in-production");
        assert_ne!(result, "racingpoint-jwt-change-me-in-production");
        assert_eq!(result.len(), 64); // 32 bytes * 2 hex chars
    }

    #[test]
    fn jwt_secret_auto_generates_on_empty() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
        let result = resolve_jwt_secret("");
        assert_eq!(result.len(), 64);
        // Verify it's valid hex
        assert!(result.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn jwt_secret_auto_generate_is_random() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::remove_var("RACECONTROL_JWT_SECRET"); }
        let key1 = resolve_jwt_secret("");
        let key2 = resolve_jwt_secret("");
        assert_ne!(key1, key2, "Two auto-generated keys must differ");
    }

    #[test]
    fn env_var_overrides_terminal_secret() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("RACECONTROL_TERMINAL_SECRET", "term-secret-abc"); }
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let mut config: Config = toml::from_str(toml_str).expect("parse");
        config.apply_env_overrides();
        assert_eq!(config.cloud.terminal_secret.as_deref(), Some("term-secret-abc"));
        unsafe { std::env::remove_var("RACECONTROL_TERMINAL_SECRET"); }
    }

    #[test]
    fn env_var_overrides_relay_secret() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("RACECONTROL_RELAY_SECRET", "relay-secret-xyz"); }
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let mut config: Config = toml::from_str(toml_str).expect("parse");
        config.apply_env_overrides();
        assert_eq!(config.bono.relay_secret.as_deref(), Some("relay-secret-xyz"));
        unsafe { std::env::remove_var("RACECONTROL_RELAY_SECRET"); }
    }

    #[test]
    fn env_var_overrides_evolution_api_key() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("RACECONTROL_EVOLUTION_API_KEY", "evo-key-123"); }
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let mut config: Config = toml::from_str(toml_str).expect("parse");
        config.apply_env_overrides();
        assert_eq!(config.auth.evolution_api_key.as_deref(), Some("evo-key-123"));
        unsafe { std::env::remove_var("RACECONTROL_EVOLUTION_API_KEY"); }
    }

    #[test]
    fn env_var_overrides_gmail_secrets() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("RACECONTROL_GMAIL_CLIENT_SECRET", "gmail-cs");
            std::env::set_var("RACECONTROL_GMAIL_REFRESH_TOKEN", "gmail-rt");
        }
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let mut config: Config = toml::from_str(toml_str).expect("parse");
        config.apply_env_overrides();
        assert_eq!(config.gmail.client_secret.as_deref(), Some("gmail-cs"));
        assert_eq!(config.gmail.refresh_token.as_deref(), Some("gmail-rt"));
        unsafe {
            std::env::remove_var("RACECONTROL_GMAIL_CLIENT_SECRET");
            std::env::remove_var("RACECONTROL_GMAIL_REFRESH_TOKEN");
        }
    }

    #[test]
    fn config_fallback_preserved_when_no_env_vars() {
        let _g = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Clear all secret env vars
        unsafe {
            std::env::remove_var("RACECONTROL_JWT_SECRET");
            std::env::remove_var("RACECONTROL_TERMINAL_SECRET");
            std::env::remove_var("RACECONTROL_RELAY_SECRET");
            std::env::remove_var("RACECONTROL_EVOLUTION_API_KEY");
            std::env::remove_var("RACECONTROL_GMAIL_CLIENT_SECRET");
            std::env::remove_var("RACECONTROL_GMAIL_REFRESH_TOKEN");
        }
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
[cloud]
terminal_secret = "from-config"
[bono]
relay_secret = "from-config-relay"
[auth]
jwt_secret = "custom-jwt-from-config"
evolution_api_key = "evo-from-config"
[gmail]
client_secret = "gmail-from-config"
refresh_token = "gmail-rt-from-config"
"#;
        let mut config: Config = toml::from_str(toml_str).expect("parse");
        config.apply_env_overrides();
        assert_eq!(config.auth.jwt_secret, "custom-jwt-from-config");
        assert_eq!(config.cloud.terminal_secret.as_deref(), Some("from-config"));
        assert_eq!(config.bono.relay_secret.as_deref(), Some("from-config-relay"));
        assert_eq!(config.auth.evolution_api_key.as_deref(), Some("evo-from-config"));
        assert_eq!(config.gmail.client_secret.as_deref(), Some("gmail-from-config"));
        assert_eq!(config.gmail.refresh_token.as_deref(), Some("gmail-rt-from-config"));
    }

    #[test]
    fn server_config_tls_port_deserializes() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
tls_port = 8443
cert_path = "/tmp/cert.pem"
key_path = "/tmp/key.pem"
[database]
"#;
        let config: Config = toml::from_str(toml_str).expect("parse with tls_port");
        assert_eq!(config.server.tls_port, Some(8443));
        assert_eq!(config.server.cert_path.as_deref(), Some("/tmp/cert.pem"));
        assert_eq!(config.server.key_path.as_deref(), Some("/tmp/key.pem"));
    }

    #[test]
    fn server_config_tls_port_defaults_to_none() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let config: Config = toml::from_str(toml_str).expect("parse without tls_port");
        assert!(config.server.tls_port.is_none());
        assert!(config.server.cert_path.is_none());
        assert!(config.server.key_path.is_none());
    }

    #[test]
    fn watchdog_config_deserializes_with_defaults() {
        let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse with defaults");
        assert!(config.watchdog.enabled);
        assert!(!config.watchdog.email_enabled);
        assert_eq!(config.watchdog.email_recipient, "usingh@racingpoint.in");
        assert_eq!(config.watchdog.email_script_path, "send_email.js");
        assert_eq!(config.watchdog.email_pod_cooldown_secs, 1800);
        assert_eq!(config.watchdog.email_venue_cooldown_secs, 300);
        assert!(config.watchdog.escalation_steps_secs.is_empty());
    }

    #[test]
    fn watchdog_config_deserializes_with_explicit_email_values() {
        let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]

[watchdog]
enabled = true
email_enabled = true
email_recipient = "ops@example.com"
email_script_path = "/opt/send.js"
email_pod_cooldown_secs = 3600
email_venue_cooldown_secs = 600
escalation_steps_secs = [10, 30, 60, 120]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse explicit values");
        assert!(config.watchdog.email_enabled);
        assert_eq!(config.watchdog.email_recipient, "ops@example.com");
        assert_eq!(config.watchdog.email_script_path, "/opt/send.js");
        assert_eq!(config.watchdog.email_pod_cooldown_secs, 3600);
        assert_eq!(config.watchdog.email_venue_cooldown_secs, 600);
        assert_eq!(config.watchdog.escalation_steps_secs, vec![10, 30, 60, 120]);
    }

    #[test]
    fn bono_config_defaults() {
        let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse with defaults");
        assert!(!config.bono.enabled);
        assert_eq!(config.bono.relay_port, 8099);
        assert!(config.bono.webhook_url.is_none());
        assert!(config.bono.tailscale_bind_ip.is_none());
        assert!(config.bono.relay_secret.is_none());
    }

    #[test]
    fn bono_config_explicit() {
        let toml_str = r#"
[venue]
name = "Test Venue"

[server]

[database]

[bono]
enabled = true
webhook_url = "http://100.64.0.1/webhooks/racecontrol"
tailscale_bind_ip = "100.64.0.2"
relay_port = 8099
relay_secret = "super-secret"
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse explicit bono values");
        assert!(config.bono.enabled);
        assert_eq!(config.bono.webhook_url.as_deref(), Some("http://100.64.0.1/webhooks/racecontrol"));
        assert_eq!(config.bono.tailscale_bind_ip.as_deref(), Some("100.64.0.2"));
        assert_eq!(config.bono.relay_port, 8099);
        assert_eq!(config.bono.relay_secret.as_deref(), Some("super-secret"));
    }

    // ─── ProcessGuardConfig Tests ─────────────────────────────────────────────

    #[test]
    fn process_guard_config_default_values() {
        let guard = ProcessGuardConfig::default();
        assert!(!guard.enabled);
        assert_eq!(guard.violation_action, "report_only");
        assert_eq!(guard.poll_interval_secs, 60);
        assert!(guard.warn_before_kill);
        assert!(guard.allowed.is_empty());
        assert!(guard.overrides.is_empty());
    }

    #[test]
    fn process_guard_config_deserializes_from_toml() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard]
enabled = true
violation_action = "report_only"
poll_interval_secs = 30

[[process_guard.allowed]]
name = "explorer.exe"
category = "system"
machines = ["all"]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse process_guard");
        assert!(config.process_guard.enabled);
        assert_eq!(config.process_guard.violation_action, "report_only");
        assert_eq!(config.process_guard.poll_interval_secs, 30);
        assert_eq!(config.process_guard.allowed.len(), 1);
        assert_eq!(config.process_guard.allowed[0].name, "explorer.exe");
    }

    #[test]
    fn allowed_process_roundtrips() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[[process_guard.allowed]]
name = "rc-agent.exe"
category = "racecontrol"
machines = ["pod"]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse allowed entry");
        assert_eq!(config.process_guard.allowed.len(), 1);
        let entry = &config.process_guard.allowed[0];
        assert_eq!(entry.name, "rc-agent.exe");
        assert_eq!(entry.category, "racecontrol");
        assert_eq!(entry.machines, vec!["pod"]);
    }

    #[test]
    fn process_guard_override_deserializes() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard.overrides.test_machine]
allow_extra_processes = ["cargo.exe", "rustc.exe"]
allow_extra_ports = [8080, 9999]
allow_extra_autostart = ["MyService"]
deny_processes = ["steam.exe"]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse override");
        let ovr = config.process_guard.overrides.get("test_machine")
            .expect("test_machine override should exist");
        assert_eq!(ovr.allow_extra_processes, vec!["cargo.exe", "rustc.exe"]);
        assert_eq!(ovr.allow_extra_ports, vec![8080, 9999]);
        assert_eq!(ovr.allow_extra_autostart, vec!["MyService"]);
        assert_eq!(ovr.deny_processes, vec!["steam.exe"]);
    }

    #[test]
    fn process_guard_override_james_key() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]

[process_guard.overrides.james]
allow_extra_processes = ["ollama.exe"]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse james override");
        let james = config.process_guard.overrides.get("james")
            .expect("james override should exist");
        assert!(james.allow_extra_processes.contains(&"ollama.exe".to_string()));
    }

    #[test]
    fn config_without_process_guard_section_defaults() {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        let config: Config = toml::from_str(toml_str).expect("should parse without process_guard");
        assert!(!config.process_guard.enabled);
        assert_eq!(config.process_guard.violation_action, "report_only");
        assert_eq!(config.process_guard.poll_interval_secs, 60);
        assert!(config.process_guard.allowed.is_empty());
        assert!(config.process_guard.overrides.is_empty());
    }

    /// Validates the repo's racecontrol.toml parses without error and has a
    /// non-empty process guard allowlist. Catches SSH banner corruption, BOM,
    /// or missing required fields before they reach production.
    #[test]
    fn repo_toml_parses_and_has_allowlist() {
        let toml_path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../racecontrol.toml");
        let content = std::fs::read_to_string(toml_path)
            .expect("racecontrol.toml must exist at repo root");

        // Detect common corruption: SSH banners, BOM
        assert!(
            !content.starts_with("**"),
            "racecontrol.toml starts with '**' — likely SSH banner corruption"
        );
        assert!(
            !content.as_bytes().starts_with(&[0xEF, 0xBB, 0xBF]),
            "racecontrol.toml has UTF-8 BOM — TOML parsers reject this"
        );
        assert!(
            content.starts_with('['),
            "racecontrol.toml must start with a TOML section header, got: {:?}",
            &content[..content.len().min(40)]
        );

        let config: Config = toml::from_str(&content)
            .expect("racecontrol.toml must be valid TOML matching Config struct");

        assert!(
            !config.process_guard.allowed.is_empty(),
            "process_guard.allowed must not be empty — got 0 entries"
        );
        assert!(
            config.process_guard.allowed.len() >= 100,
            "process_guard.allowed has only {} entries — expected 100+, possible data loss",
            config.process_guard.allowed.len()
        );
    }
}
