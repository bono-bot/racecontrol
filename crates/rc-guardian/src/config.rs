//! Guardian configuration — loaded from rc-guardian.toml or environment variables.

use serde::Deserialize;
use tracing::info;

/// Configuration for the external guardian.
#[derive(Debug, Clone, Deserialize)]
pub struct GuardianConfig {
    /// RaceControl server health URL (Tailscale preferred)
    #[serde(default = "default_server_url")]
    pub server_url: String,

    /// Fleet health URL for billing safety check
    #[serde(default = "default_fleet_url")]
    pub fleet_url: String,

    /// Server Tailscale IP for SSH restart
    #[serde(default = "default_tailscale_ip")]
    pub tailscale_ip: String,

    /// SSH user for server restart
    #[serde(default = "default_ssh_user")]
    pub ssh_user: String,

    /// Comms-link WebSocket URL (on Bono VPS)
    #[serde(default = "default_comms_link_url")]
    pub comms_link_url: String,

    /// Health poll interval in seconds (EG-01: 60s)
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    /// HTTP request timeout in seconds
    #[serde(default = "default_http_timeout")]
    pub http_timeout_secs: u64,

    /// Consecutive failures before dead-man trigger (EG-02: 3)
    #[serde(default = "default_dead_man_threshold")]
    pub dead_man_threshold: u32,

    /// Heartbeat interval in seconds (EG-08: 6h = 21600s)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,

    /// Threshold in ms above which a response is classified as "busy"
    #[serde(default = "default_busy_threshold_ms")]
    pub busy_threshold_ms: u64,

    /// Evolution API base URL for WhatsApp alerts
    #[serde(default = "default_evolution_url")]
    pub evolution_api_url: String,

    /// Evolution API instance name
    #[serde(default = "default_evolution_instance")]
    pub evolution_instance: String,

    /// Evolution API key
    #[serde(default)]
    pub evolution_api_key: String,

    /// WhatsApp number to send alerts to (Uday)
    #[serde(default = "default_alert_phone")]
    pub alert_phone: String,
}

fn default_server_url() -> String {
    "http://100.125.108.37:8080/api/v1/health".to_string()
}

fn default_fleet_url() -> String {
    "http://100.125.108.37:8080/api/v1/fleet/health".to_string()
}

fn default_tailscale_ip() -> String {
    "100.125.108.37".to_string()
}

fn default_ssh_user() -> String {
    "ADMIN".to_string()
}

fn default_comms_link_url() -> String {
    "ws://localhost:8765".to_string()
}

fn default_poll_interval() -> u64 {
    60
}

fn default_http_timeout() -> u64 {
    10
}

fn default_dead_man_threshold() -> u32 {
    3
}

fn default_heartbeat_interval() -> u64 {
    21600 // 6 hours
}

fn default_busy_threshold_ms() -> u64 {
    5000 // 5 seconds
}

fn default_evolution_url() -> String {
    "http://srv1422716.hstgr.cloud:53622".to_string()
}

fn default_evolution_instance() -> String {
    "RacingPoint".to_string()
}

fn default_alert_phone() -> String {
    "917981264279".to_string()
}

impl GuardianConfig {
    /// Load config from rc-guardian.toml (next to binary or /etc/racecontrol/)
    /// with environment variable overrides.
    pub fn load() -> anyhow::Result<Self> {
        // Try loading from file
        let config_paths = [
            "rc-guardian.toml",
            "/etc/racecontrol/rc-guardian.toml",
        ];

        let mut config: GuardianConfig = 'load: {
            for path in &config_paths {
                if let Ok(contents) = std::fs::read_to_string(path) {
                    info!(path, "Loading config from file");
                    match toml::from_str(&contents) {
                        Ok(c) => break 'load c,
                        Err(e) => {
                            tracing::warn!(path, error = %e, "Failed to parse config file, using defaults");
                        }
                    }
                }
            }
            info!("No config file found, using defaults with env overrides");
            GuardianConfig::default()
        };

        // Environment variable overrides
        if let Ok(v) = std::env::var("GUARDIAN_SERVER_URL") {
            config.server_url = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_FLEET_URL") {
            config.fleet_url = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_TAILSCALE_IP") {
            config.tailscale_ip = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_SSH_USER") {
            config.ssh_user = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_COMMS_LINK_URL") {
            config.comms_link_url = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_POLL_INTERVAL") {
            if let Ok(n) = v.parse() {
                config.poll_interval_secs = n;
            }
        }
        if let Ok(v) = std::env::var("GUARDIAN_DEAD_MAN_THRESHOLD") {
            if let Ok(n) = v.parse() {
                config.dead_man_threshold = n;
            }
        }
        if let Ok(v) = std::env::var("GUARDIAN_EVOLUTION_URL") {
            config.evolution_api_url = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_EVOLUTION_INSTANCE") {
            config.evolution_instance = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_EVOLUTION_KEY") {
            config.evolution_api_key = v;
        }
        if let Ok(v) = std::env::var("GUARDIAN_ALERT_PHONE") {
            config.alert_phone = v;
        }

        Ok(config)
    }
}

impl Default for GuardianConfig {
    fn default() -> Self {
        Self {
            server_url: default_server_url(),
            fleet_url: default_fleet_url(),
            tailscale_ip: default_tailscale_ip(),
            ssh_user: default_ssh_user(),
            comms_link_url: default_comms_link_url(),
            poll_interval_secs: default_poll_interval(),
            http_timeout_secs: default_http_timeout(),
            dead_man_threshold: default_dead_man_threshold(),
            heartbeat_interval_secs: default_heartbeat_interval(),
            busy_threshold_ms: default_busy_threshold_ms(),
            evolution_api_url: default_evolution_url(),
            evolution_instance: default_evolution_instance(),
            evolution_api_key: String::new(),
            alert_phone: default_alert_phone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = GuardianConfig::default();
        assert_eq!(config.server_url, "http://100.125.108.37:8080/api/v1/health");
        assert_eq!(config.tailscale_ip, "100.125.108.37");
        assert_eq!(config.ssh_user, "ADMIN");
        assert_eq!(config.poll_interval_secs, 60);
        assert_eq!(config.dead_man_threshold, 3);
        assert_eq!(config.http_timeout_secs, 10);
        assert_eq!(config.busy_threshold_ms, 5000);
        assert_eq!(config.comms_link_url, "ws://localhost:8765");
        assert_eq!(config.alert_phone, "917981264279");
        assert!(config.evolution_api_key.is_empty());
    }

    #[test]
    fn test_config_from_toml() {
        let toml_str = r#"
            server_url = "http://192.168.31.23:8080/api/v1/health"
            tailscale_ip = "100.92.122.89"
            ssh_user = "User"
            poll_interval_secs = 30
            dead_man_threshold = 5
        "#;
        let config: GuardianConfig = toml::from_str(toml_str).expect("parse toml");
        assert_eq!(config.server_url, "http://192.168.31.23:8080/api/v1/health");
        assert_eq!(config.tailscale_ip, "100.92.122.89");
        assert_eq!(config.ssh_user, "User");
        assert_eq!(config.poll_interval_secs, 30);
        assert_eq!(config.dead_man_threshold, 5);
        // Unspecified fields use defaults
        assert_eq!(config.http_timeout_secs, 10);
        assert_eq!(config.comms_link_url, "ws://localhost:8765");
    }

    #[test]
    fn test_config_partial_toml() {
        let toml_str = r#"
            evolution_api_key = "test-key-123"
        "#;
        let config: GuardianConfig = toml::from_str(toml_str).expect("parse toml");
        assert_eq!(config.evolution_api_key, "test-key-123");
        // All other fields should be defaults
        assert_eq!(config.server_url, "http://100.125.108.37:8080/api/v1/health");
    }

    #[test]
    fn test_config_empty_toml() {
        let config: GuardianConfig = toml::from_str("").expect("parse empty toml");
        assert_eq!(config.server_url, "http://100.125.108.37:8080/api/v1/health");
        assert_eq!(config.dead_man_threshold, 3);
    }
}
