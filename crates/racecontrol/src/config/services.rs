//! Service configuration types: integrations, auth, billing, watchdog, monitoring,
//! process guard, backup, event archive, and other operational configs.

use serde::Deserialize;

use super::{default_true, default_false};

// ─── Integrations ────────────────────────────────────────────────────────────

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
    /// OpenRouter model for debug diagnostics (used before Ollama fallback).
    /// Key read from OPENROUTER_KEY env var or data/openrouter-mma-key.txt.
    #[serde(default = "default_openrouter_model")]
    pub openrouter_model: String,
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
            openrouter_model: default_openrouter_model(),
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
    /// Phase 348: Pre-shared secret for break-glass emergency access.
    /// When set, enables POST /api/v1/auth/break-glass which issues a 1-hour
    /// superadmin JWT. Every use triggers a WhatsApp alert + audit log.
    /// Set via RACECONTROL_BREAK_GLASS_SECRET env var or config file.
    #[serde(default)]
    pub break_glass_secret: Option<String>,
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
            break_glass_secret: None,
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

// ─── Watchdog & Monitoring ───────────────────────────────────────────────────

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

// ─── Cafe & Billing ─────────────────────────────────────────────────────────

/// Configuration for cafe-related features.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CafeConfig {
    /// Path to the Node.js thermal receipt print script.
    /// If None, thermal printing is silently skipped.
    #[serde(default)]
    pub print_script_path: Option<String>,
}

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
    /// Phase 283: HMAC secret for billing replay protection. If empty, a random secret is generated at startup.
    #[serde(default)]
    pub hmac_secret: String,
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
            hmac_secret: String::new(),
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

// ─── Bono Relay ──────────────────────────────────────────────────────────────

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


// ─── Default functions ───────────────────────────────────────────────────────

fn default_claude_cli_timeout() -> u32 { 30 }
fn default_ollama_url() -> String { "http://192.168.31.27:11434".to_string() }
fn default_ollama_model() -> String { "qwen2.5:3b".to_string() }
fn default_anthropic_model() -> String { "claude-sonnet-4-20250514".to_string() }
fn default_openrouter_model() -> String { "deepseek/deepseek-chat-v3-0324".to_string() }
fn default_acserver_path() -> String { "C:/RacingPoint/ac-server/acServer.exe".to_string() }
fn default_ac_data_dir() -> String { "./data/ac_servers".to_string() }
// v47.0 Phase 345-03 (Phase 343 C5): return empty so the dangerous default literal
// is no longer compiled into the binary. resolve_jwt_secret treats empty as unset
// and auto-generates or reads from env.
fn default_jwt_secret() -> String { String::new() }
fn default_pin_expiry() -> u64 { 600 }
fn default_otp_expiry() -> u64 { 300 }
fn default_watchdog_interval() -> u64 { 10 }
fn default_heartbeat_timeout() -> i64 { 30 }
fn default_restart_cooldown() -> i64 { 120 }
fn default_email_recipient() -> String { "usingh@racingpoint.in".to_string() }
fn default_email_script_path() -> String { "send_email.js".to_string() }
fn default_email_pod_cooldown() -> i64 { 1800 }
fn default_email_venue_cooldown() -> i64 { 300 }
