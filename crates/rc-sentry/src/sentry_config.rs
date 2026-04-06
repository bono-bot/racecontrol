//! Sentry configuration — optional TOML file to override watchdog target.
//!
//! Without a config file, rc-sentry defaults to monitoring rc-agent on :8090
//! (backwards compatible with all pod deployments).
//!
//! With `rc-sentry.toml`, it can monitor any HTTP service (e.g. racecontrol on :8080).

use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<SentryConfig> = OnceLock::new();

/// Watchdog target configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct SentryConfig {
    /// Display name for the monitored service (used in logs).
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Health endpoint address (host:port format, no path).
    #[serde(default = "default_health_addr")]
    pub health_addr: String,

    /// Health endpoint path.
    #[serde(default = "default_health_path")]
    pub health_path: String,

    /// Port the service listens on (for TIME_WAIT cleanup).
    #[serde(default = "default_service_port")]
    pub service_port: u16,

    /// Process name to kill on crash (e.g. "rc-agent.exe" or "racecontrol.exe").
    #[serde(default = "default_process_name")]
    pub process_name: String,

    /// Bat script to restart the service.
    #[serde(default = "default_start_script")]
    pub start_script: String,

    /// TOML config file that must exist for the service (preflight check).
    #[serde(default = "default_service_toml")]
    pub service_toml: String,

    /// Startup log path (for crash context).
    #[serde(default = "default_startup_log")]
    pub startup_log: String,

    /// Stderr log path (for crash context).
    #[serde(default = "default_stderr_log")]
    pub stderr_log: String,

    /// Mesh connectivity configuration.
    #[serde(default)]
    pub mesh: MeshConfig,

    /// Direct WhatsApp alert configuration (MON-04).
    #[serde(default)]
    pub alert_config: AlertConfig,
}

fn default_service_name() -> String { "rc-agent".to_string() }
fn default_health_addr() -> String { "127.0.0.1:8090".to_string() }
fn default_health_path() -> String { "/health".to_string() }
fn default_service_port() -> u16 { 8090 }
fn default_process_name() -> String { "rc-agent.exe".to_string() }
fn default_start_script() -> String { r"C:\RacingPoint\start-rcagent.bat".to_string() }
fn default_service_toml() -> String { r"C:\RacingPoint\rc-agent.toml".to_string() }
fn default_startup_log() -> String { r"C:\RacingPoint\rc-agent-startup.log".to_string() }
fn default_stderr_log() -> String { r"C:\RacingPoint\rc-agent-stderr.log".to_string() }

/// Mesh configuration — connects rc-sentry to Bono comms-link hub via Tailscale.
/// P1-03: Manual Debug impl redacts PSK to prevent credential leaks in logs.
#[derive(Clone, Deserialize)]
pub struct MeshConfig {
    /// Enable mesh connectivity (default: false until configured)
    #[serde(default)]
    pub enabled: bool,

    /// Node identifier (e.g. "pod-8", "pos-1")
    #[serde(default = "default_mesh_node_id")]
    pub node_id: String,

    /// Role: "pod" or "pos" — controls which commands are allowed
    #[serde(default = "default_mesh_role")]
    pub role: String,

    /// Bono hub WebSocket URL (Tailscale IP)
    #[serde(default = "default_mesh_hub_url")]
    pub hub_url: String,

    /// Pre-shared key for HMAC auth (same as COMMS_PSK)
    #[serde(default)]
    pub psk: String,

    /// Heartbeat interval in seconds
    #[serde(default = "default_mesh_heartbeat_secs")]
    pub heartbeat_secs: u64,
}

fn default_mesh_node_id() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string())
}
fn default_mesh_role() -> String { "pod".to_string() }
fn default_mesh_hub_url() -> String { "ws://100.70.177.44:8765".to_string() }
fn default_mesh_heartbeat_secs() -> u64 { 15 }

impl std::fmt::Debug for MeshConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshConfig")
            .field("enabled", &self.enabled)
            .field("node_id", &self.node_id)
            .field("role", &self.role)
            .field("hub_url", &self.hub_url)
            .field("psk", &if self.psk.is_empty() { "(empty)" } else { "[REDACTED]" })
            .field("heartbeat_secs", &self.heartbeat_secs)
            .finish()
    }
}

impl Default for MeshConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            node_id: default_mesh_node_id(),
            role: default_mesh_role(),
            hub_url: default_mesh_hub_url(),
            psk: String::new(),
            heartbeat_secs: default_mesh_heartbeat_secs(),
        }
    }
}

impl Default for SentryConfig {
    fn default() -> Self {
        Self {
            service_name: default_service_name(),
            health_addr: default_health_addr(),
            health_path: default_health_path(),
            service_port: default_service_port(),
            process_name: default_process_name(),
            start_script: default_start_script(),
            service_toml: default_service_toml(),
            startup_log: default_startup_log(),
            stderr_log: default_stderr_log(),
            mesh: MeshConfig::default(),
            alert_config: AlertConfig::default(),
        }
    }
}

/// Direct WhatsApp alert configuration for MON-04.
/// Bypasses server relay -- alerts fire even when server is down.
#[derive(Clone, Deserialize)]
pub struct AlertConfig {
    /// Whether direct WhatsApp alerts are enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Evolution API base URL (e.g. "http://srv1422716.hstgr.cloud:8080")
    #[serde(default)]
    pub whatsapp_url: String,
    /// Evolution API instance name (e.g. "racing-point-staff")
    #[serde(default)]
    pub whatsapp_instance: String,
    /// Evolution API key (apikey header value)
    #[serde(default)]
    pub whatsapp_api_key: String,
    /// Phone number to alert (E.164 without +, e.g. "917075778180")
    #[serde(default)]
    pub whatsapp_number: String,
    /// COMMS_PSK value (stored alongside alert config for future mesh use)
    #[serde(default)]
    pub comms_psk: String,
}

impl std::fmt::Debug for AlertConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AlertConfig")
            .field("enabled", &self.enabled)
            .field("whatsapp_url", &self.whatsapp_url)
            .field("whatsapp_instance", &self.whatsapp_instance)
            .field("whatsapp_api_key", &if self.whatsapp_api_key.is_empty() { "(empty)" } else { "[REDACTED]" })
            .field("whatsapp_number", &self.whatsapp_number)
            .field("comms_psk", &if self.comms_psk.is_empty() { "(empty)" } else { "[REDACTED]" })
            .finish()
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            whatsapp_url: String::new(),
            whatsapp_instance: String::new(),
            whatsapp_api_key: String::new(),
            whatsapp_number: String::new(),
            comms_psk: String::new(),
        }
    }
}

/// Load config from TOML file, or use defaults.
/// The config path is read from argv[2] or defaults to `rc-sentry.toml` in CWD.
pub fn load() -> &'static SentryConfig {
    CONFIG.get_or_init(|| {
        let path = std::env::args()
            .nth(2)
            .unwrap_or_else(|| "rc-sentry.toml".to_string());

        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match toml::from_str::<SentryConfig>(&content) {
                    Ok(cfg) => {
                        tracing::info!(
                            "loaded sentry config from {path}: service={}, health={}",
                            cfg.service_name, cfg.health_addr
                        );
                        cfg
                    }
                    Err(e) => {
                        tracing::error!("failed to parse {path}: {e} — using defaults");
                        SentryConfig::default()
                    }
                }
            }
            Err(_) => {
                tracing::info!("no sentry config at {path} — using defaults (rc-agent mode)");
                SentryConfig::default()
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_config_defaults() {
        let cfg = AlertConfig::default();
        assert!(!cfg.enabled);
        assert!(cfg.whatsapp_url.is_empty());
        assert!(cfg.whatsapp_instance.is_empty());
        assert!(cfg.whatsapp_api_key.is_empty());
        assert!(cfg.whatsapp_number.is_empty());
        assert!(cfg.comms_psk.is_empty());
    }

    #[test]
    fn alert_config_deserialize_full() {
        let toml_str = r#"
service_name = "rc-agent"
health_addr = "127.0.0.1:8090"
health_path = "/health"
service_port = 8090
process_name = "rc-agent.exe"
start_script = "C:\\RacingPoint\\start-rcagent.bat"
service_toml = "C:\\RacingPoint\\rc-agent.toml"
startup_log = "C:\\RacingPoint\\rc-agent-startup.log"
stderr_log = "C:\\RacingPoint\\rc-agent-stderr.log"

[alert_config]
enabled = true
whatsapp_url = "http://srv1422716.hstgr.cloud:8080"
whatsapp_instance = "racing-point-staff"
whatsapp_api_key = "secret-api-key-123"
whatsapp_number = "917075778180"
comms_psk = "secret-psk-456"
"#;
        let cfg: SentryConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.alert_config.enabled);
        assert_eq!(cfg.alert_config.whatsapp_url, "http://srv1422716.hstgr.cloud:8080");
        assert_eq!(cfg.alert_config.whatsapp_instance, "racing-point-staff");
        assert_eq!(cfg.alert_config.whatsapp_api_key, "secret-api-key-123");
        assert_eq!(cfg.alert_config.whatsapp_number, "917075778180");
        assert_eq!(cfg.alert_config.comms_psk, "secret-psk-456");
    }

    #[test]
    fn alert_config_deserialize_missing() {
        // TOML without [alert_config] should still deserialize with defaults
        let toml_str = r#"
service_name = "rc-agent"
health_addr = "127.0.0.1:8090"
health_path = "/health"
service_port = 8090
process_name = "rc-agent.exe"
start_script = "C:\\RacingPoint\\start-rcagent.bat"
service_toml = "C:\\RacingPoint\\rc-agent.toml"
startup_log = "C:\\RacingPoint\\rc-agent-startup.log"
stderr_log = "C:\\RacingPoint\\rc-agent-stderr.log"
"#;
        let cfg: SentryConfig = toml::from_str(toml_str).unwrap();
        assert!(!cfg.alert_config.enabled);
        assert!(cfg.alert_config.whatsapp_url.is_empty());
        assert!(cfg.alert_config.whatsapp_api_key.is_empty());
    }

    #[test]
    fn alert_config_debug_redacts_secrets() {
        let cfg = AlertConfig {
            enabled: true,
            whatsapp_url: "http://example.com:8080".to_string(),
            whatsapp_instance: "test-instance".to_string(),
            whatsapp_api_key: "super-secret-key".to_string(),
            whatsapp_number: "917075778180".to_string(),
            comms_psk: "super-secret-psk".to_string(),
        };
        let debug_output = format!("{:?}", cfg);
        assert!(debug_output.contains("[REDACTED]"), "Debug output should contain [REDACTED]");
        assert!(!debug_output.contains("super-secret-key"), "Debug output must NOT contain the actual API key");
        assert!(!debug_output.contains("super-secret-psk"), "Debug output must NOT contain the actual PSK");
        assert!(debug_output.contains("917075778180"), "Debug output should show the phone number (not a secret)");
    }
}
