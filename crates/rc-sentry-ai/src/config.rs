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

#[derive(Debug, Clone, Deserialize)]
pub struct DetectionConfig {
    #[serde(default = "default_detection_enabled")]
    pub enabled: bool,
    #[serde(default = "default_model_path")]
    pub model_path: String,
    #[serde(default = "default_confidence")]
    pub confidence_threshold: f32,
    #[serde(default = "default_nms_threshold")]
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
    0.45
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
