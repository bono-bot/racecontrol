use serde::Deserialize;
use std::path::PathBuf;

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
}

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
    #[serde(default = "default_sync_interval")]
    pub sync_interval_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct PodsConfig {
    #[serde(default = "default_pod_count")]
    pub count: u32,
    #[serde(default = "default_true")]
    pub discovery: bool,
    #[serde(default, rename = "static")]
    pub static_pods: Vec<StaticPodConfig>,
}

impl Default for PodsConfig {
    fn default() -> Self {
        Self {
            count: 16,
            discovery: true,
            static_pods: Vec::new(),
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

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
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
        }
    }
}

fn default_host() -> String { "0.0.0.0".to_string() }
fn default_port() -> u16 { 8080 }
fn default_db_path() -> String { "./data/racecontrol.db".to_string() }
fn default_location() -> String { "Bandlaguda, Hyderabad".to_string() }
fn default_timezone() -> String { "Asia/Kolkata".to_string() }
fn default_sync_interval() -> u64 { 30 }
fn default_pod_count() -> u32 { 16 }
fn default_true() -> bool { true }
fn default_color() -> String { "#FF4400".to_string() }
fn default_theme() -> String { "dark".to_string() }
