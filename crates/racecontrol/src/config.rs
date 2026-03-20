use serde::Deserialize;

#[derive(Debug, Deserialize)]
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
}

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

#[derive(Debug, Deserialize)]
pub struct VenueConfig {
    pub name: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_db_path")]
    pub path: String,
}

#[derive(Debug, Default, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
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
}

impl Default for PodsConfig {
    fn default() -> Self {
        Self {
            count: 16,
            discovery: true,
            static_pods: Vec::new(),
            healer_enabled: true,
            healer_interval_secs: default_healer_interval(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StaticPodConfig {
    pub number: u32,
    pub name: String,
    pub ip: String,
    pub sim: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct BrandingConfig {
    pub logo: Option<String>,
    #[serde(default = "default_color")]
    pub primary_color: String,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Default, Deserialize)]
pub struct IntegrationsConfig {
    #[serde(default)]
    pub discord: DiscordConfig,
    #[serde(default)]
    pub whatsapp: WhatsAppConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct DiscordConfig {
    pub webhook_url: Option<String>,
    pub results_channel: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    pub contact: Option<String>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: default_jwt_secret(),
            pin_expiry_secs: default_pin_expiry(),
            otp_expiry_secs: default_otp_expiry(),
            evolution_url: None,
            evolution_api_key: None,
            evolution_instance: None,
        }
    }
}

#[derive(Debug, Deserialize)]
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
#[derive(Debug, Deserialize)]
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

/// Configuration for the Bono relay: event push to Bono's VPS over Tailscale mesh,
/// and inbound relay endpoint for commands from Bono's cloud.
#[derive(Debug, Deserialize)]
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

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: Config = toml::from_str(&content)?;
        config.apply_env_overrides();
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        let paths = ["racecontrol.toml", "/etc/racecontrol/racecontrol.toml"];
        for path in paths {
            if let Ok(config) = Self::load(path) {
                tracing::info!("Loaded config from {}", path);
                return config;
            }
        }
        tracing::warn!("No config file found, using defaults");
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
            },
            server: ServerConfig {
                host: default_host(),
                port: default_port(),
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
    }
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_db_path() -> String { "./data/racecontrol.db".to_string() }
fn default_location() -> String { "Bandlaguda, Hyderabad".to_string() }
fn default_timezone() -> String { "Asia/Kolkata".to_string() }
fn default_sync_interval() -> u64 { 30 }
fn default_action_poll_interval() -> u64 { 3 }
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
fn default_ollama_url() -> String { "http://localhost:11434".to_string() }
fn default_ollama_model() -> String { "racing-point-ops".to_string() }
fn default_anthropic_model() -> String { "claude-sonnet-4-20250514".to_string() }
fn default_healer_interval() -> u32 { 120 }
fn default_false() -> bool { false }
fn default_email_recipient() -> String { "usingh@racingpoint.in".to_string() }
fn default_email_script_path() -> String { "send_email.js".to_string() }
fn default_email_pod_cooldown() -> i64 { 1800 }
fn default_email_venue_cooldown() -> i64 { 300 }

#[cfg(test)]
mod tests {
    use super::*;

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
}
