use anyhow::Context;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub relay: RelayConfig,
    pub cameras: Vec<CameraConfig>,
    #[serde(default)]
    pub privacy: PrivacyConfig,
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub recognition: RecognitionConfig,
    #[serde(default)]
    pub enrollment: EnrollmentConfig,
    #[serde(default)]
    pub attendance: AttendanceConfig,
    #[serde(default)]
    pub alerts: AlertsConfig,
    #[serde(default)]
    pub nvr: NvrConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub port: u16,
    pub host: String,
    /// M1-SEC: Service key for authenticating API requests.
    /// All endpoints except /health and /api/v1/privacy/consent require this.
    #[serde(default)]
    pub service_key: Option<String>,
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
    #[serde(default)]
    pub nvr_channel: Option<u32>,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub display_order: Option<u32>,
    #[serde(default = "default_zone")]
    pub zone: String,
}

fn default_zone() -> String {
    "other".to_string()
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

#[derive(Debug, Clone, Deserialize)]
pub struct DetectionConfig {
    #[serde(default = "default_detection_enabled")]
    pub enabled: bool,
    #[serde(default = "default_model_path")]
    pub model_path: String,
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,
    #[serde(default = "default_nms_threshold")]
    #[allow(dead_code)]
    pub nms_threshold: f32,
}

fn default_detection_enabled() -> bool {
    true
}

fn default_model_path() -> String {
    r"C:\RacingPoint\models\scrfd_10g_bnkps.onnx".to_string()
}

fn default_confidence() -> f32 {
    0.5
}

fn default_nms_threshold() -> f32 {
    0.4
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            enabled: default_detection_enabled(),
            model_path: default_model_path(),
            confidence_threshold: default_confidence(),
            nms_threshold: default_nms_threshold(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecognitionConfig {
    #[serde(default = "default_recognition_enabled")]
    pub enabled: bool,
    #[serde(default = "default_recognition_model_path")]
    pub model_path: String,
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
    #[serde(default = "default_min_face_size")]
    pub min_face_size: u32,
    #[serde(default = "default_min_laplacian_var")]
    pub min_laplacian_var: f64,
    #[serde(default = "default_max_yaw_degrees")]
    pub max_yaw_degrees: f64,
    #[serde(default = "default_tracker_cooldown_secs")]
    pub tracker_cooldown_secs: u64,
    #[serde(default = "default_gallery_db_path")]
    pub gallery_db_path: String,
}

fn default_recognition_enabled() -> bool {
    true
}

fn default_recognition_model_path() -> String {
    r"C:\RacingPoint\models\glintr100.onnx".to_string()
}

fn default_similarity_threshold() -> f32 {
    0.55
}

fn default_min_face_size() -> u32 {
    80
}

fn default_min_laplacian_var() -> f64 {
    100.0
}

fn default_max_yaw_degrees() -> f64 {
    45.0
}

fn default_tracker_cooldown_secs() -> u64 {
    60
}

fn default_gallery_db_path() -> String {
    r"C:\RacingPoint\data\faces.db".to_string()
}

impl Default for RecognitionConfig {
    fn default() -> Self {
        Self {
            enabled: default_recognition_enabled(),
            model_path: default_recognition_model_path(),
            similarity_threshold: default_similarity_threshold(),
            min_face_size: default_min_face_size(),
            min_laplacian_var: default_min_laplacian_var(),
            max_yaw_degrees: default_max_yaw_degrees(),
            tracker_cooldown_secs: default_tracker_cooldown_secs(),
            gallery_db_path: default_gallery_db_path(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnrollmentConfig {
    #[serde(default = "default_duplicate_threshold")]
    pub duplicate_threshold: f32,
    #[serde(default = "default_body_limit_mb")]
    pub body_limit_mb: usize,
    #[serde(default = "default_enrollment_retention_days")]
    pub retention_days: u64,
    #[serde(default = "default_min_embeddings_complete")]
    pub min_embeddings_complete: u64,
    #[serde(default = "default_enrollment_min_face_size")]
    pub min_face_size: u32,
    #[serde(default = "default_enrollment_min_laplacian")]
    pub min_laplacian_var: f64,
    #[serde(default = "default_enrollment_max_yaw")]
    pub max_yaw_degrees: f64,
}

fn default_duplicate_threshold() -> f32 {
    0.6
}

fn default_body_limit_mb() -> usize {
    10
}

fn default_enrollment_retention_days() -> u64 {
    365
}

fn default_min_embeddings_complete() -> u64 {
    3
}

fn default_enrollment_min_face_size() -> u32 {
    120
}

fn default_enrollment_min_laplacian() -> f64 {
    150.0
}

fn default_enrollment_max_yaw() -> f64 {
    30.0
}

impl Default for EnrollmentConfig {
    fn default() -> Self {
        Self {
            duplicate_threshold: default_duplicate_threshold(),
            body_limit_mb: default_body_limit_mb(),
            retention_days: default_enrollment_retention_days(),
            min_embeddings_complete: default_min_embeddings_complete(),
            min_face_size: default_enrollment_min_face_size(),
            min_laplacian_var: default_enrollment_min_laplacian(),
            max_yaw_degrees: default_enrollment_max_yaw(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AttendanceConfig {
    #[serde(default = "default_attendance_enabled")]
    pub enabled: bool,
    #[serde(default = "default_dedup_window_secs")]
    pub dedup_window_secs: u64,
    #[serde(default = "default_present_timeout_secs")]
    pub present_timeout_secs: u64,
    #[serde(default = "default_min_shift_hours")]
    pub min_shift_hours: u64,
}

fn default_attendance_enabled() -> bool {
    true
}

fn default_dedup_window_secs() -> u64 {
    300
}

fn default_present_timeout_secs() -> u64 {
    1800
}

fn default_min_shift_hours() -> u64 {
    4
}

impl Default for AttendanceConfig {
    fn default() -> Self {
        Self {
            enabled: default_attendance_enabled(),
            dedup_window_secs: default_dedup_window_secs(),
            present_timeout_secs: default_present_timeout_secs(),
            min_shift_hours: default_min_shift_hours(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlertsConfig {
    #[serde(default = "default_alerts_enabled")]
    pub enabled: bool,
    #[serde(default = "default_unknown_rate_limit_secs")]
    pub unknown_rate_limit_secs: u64,
    #[serde(default = "default_face_crop_dir")]
    pub face_crop_dir: String,
    #[serde(default = "default_face_crop_quality")]
    pub face_crop_quality: u8,
}

fn default_alerts_enabled() -> bool {
    true
}

fn default_unknown_rate_limit_secs() -> u64 {
    300
}

fn default_face_crop_dir() -> String {
    r"C:\RacingPoint\face-crops\".to_string()
}

fn default_face_crop_quality() -> u8 {
    85
}

impl Default for AlertsConfig {
    fn default() -> Self {
        Self {
            enabled: default_alerts_enabled(),
            unknown_rate_limit_secs: default_unknown_rate_limit_secs(),
            face_crop_dir: default_face_crop_dir(),
            face_crop_quality: default_face_crop_quality(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NvrConfig {
    #[serde(default = "default_nvr_enabled")]
    pub enabled: bool,
    #[serde(default = "default_nvr_host")]
    pub host: String,
    #[serde(default = "default_nvr_port")]
    pub port: u16,
    #[serde(default = "default_nvr_username")]
    pub username: String,
    #[serde(default = "default_nvr_password")]
    pub password: String,
}

fn default_nvr_enabled() -> bool {
    false
}

fn default_nvr_host() -> String {
    "192.168.31.18".to_string()
}

fn default_nvr_port() -> u16 {
    80
}

fn default_nvr_username() -> String {
    // M11-SEC: Read from environment, no hardcoded credentials.
    // Set NVR_USERNAME in environment or [nvr].username in TOML config.
    std::env::var("NVR_USERNAME").unwrap_or_else(|_| {
        tracing::warn!("NVR_USERNAME not set — using empty default. Set in env or rc-sentry-ai.toml");
        String::new()
    })
}

fn default_nvr_password() -> String {
    // M11-SEC: Read from environment, no hardcoded credentials.
    // Set NVR_PASSWORD in environment or [nvr].password in TOML config.
    std::env::var("NVR_PASSWORD").unwrap_or_else(|_| {
        tracing::warn!("NVR_PASSWORD not set — using empty default. Set in env or rc-sentry-ai.toml");
        String::new()
    })
}

impl Default for NvrConfig {
    fn default() -> Self {
        Self {
            enabled: default_nvr_enabled(),
            host: default_nvr_host(),
            port: default_nvr_port(),
            username: default_nvr_username(),
            password: default_nvr_password(),
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

    /// Direct NVR RTSP URL (bypass go2rtc relay).
    /// Subtype 0 = main stream (4MP H.265), 1 = sub stream (D1 MJPEG/H.265).
    #[allow(dead_code)]
    pub fn nvr_rtsp_url(&self, nvr_config: &NvrConfig, subtype: u32) -> Option<String> {
        let channel = self.nvr_channel?;
        let encoded_pass = nvr_config.password.replace('@', "%40");
        Some(format!(
            "rtsp://{}:{}@{}:554/cam/realmonitor?channel={}&subtype={}",
            nvr_config.username, encoded_pass, nvr_config.host, channel, subtype
        ))
    }

    pub fn effective_display_name(&self) -> &str {
        self.display_name.as_deref().unwrap_or(&self.name)
    }
}
