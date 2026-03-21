use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub relay: RelayConfig,
    pub cameras: Vec<CameraConfig>,
    #[serde(default)]
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RelayConfig {
    pub api_url: String,
    pub rtsp_base: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CameraConfig {
    pub name: String,
    pub stream_name: String,
    pub role: String,
    pub fps: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyConfig {
    #[serde(default = "default_audit_log_path")]
    pub audit_log_path: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: u64,
}

fn default_audit_log_path() -> String {
    r"C:\RacingPoint\logs\face-audit.jsonl".to_string()
}

fn default_retention_days() -> u64 {
    90
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            audit_log_path: default_audit_log_path(),
            retention_days: default_retention_days(),
        }
    }
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {path}"))?;
        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("Failed to parse config file: {path}"))?;
        Ok(config)
    }
}

impl CameraConfig {
    pub fn relay_url(&self, rtsp_base: &str) -> String {
        format!("{rtsp_base}/{}", self.stream_name)
    }
}
