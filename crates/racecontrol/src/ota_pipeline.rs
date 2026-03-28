//! OTA Pipeline — state-machine-driven fleet deployment with canary-first waves,
//! SHA256 binary verification, session gating, and auto-rollback.
//!
//! v22.0 Phase 179: Deploys rc-agent and rc-sentry to 8 pods via a gated pipeline:
//! Wave 1 (canary Pod 8) → Wave 2 (Pods 1-4) → Wave 3 (Pods 5-7).
//! Each wave verifies health gates (WS connected, HTTP reachable, SHA256 match)
//! before advancing. Pods with active billing sessions are deferred, not failed.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ── Wave constants (OTA-02, OTA-06) ────────────────────────────────────────
/// Canary wave: Pod 8 always goes first.
pub const WAVE_1: &[u32] = &[8];
/// Second wave: Pods 1-4.
pub const WAVE_2: &[u32] = &[1, 2, 3, 4];
/// Third wave: remaining Pods 5-7.
pub const WAVE_3: &[u32] = &[5, 6, 7];

/// All waves in deployment order.
pub const ALL_WAVES: &[&[u32]] = &[WAVE_1, WAVE_2, WAVE_3];

// ── State file path ────────────────────────────────────────────────────────
/// Location of the pipeline state file on the server.
const DEPLOY_STATE_FILE: &str = r"C:\RacingPoint\deploy-state.json";

// ── ReleaseManifest (OTA-01, OTA-10, SYNC-05) ─────────────────────────────

/// A release manifest locks binary SHA256, config schema version, frontend
/// build_id, git commit, and timestamp as one immutable bundle.
/// No manifest = no deploy starts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseManifest {
    pub release: ReleaseInfo,
    pub binaries: BinaryHashes,
    pub compatibility: CompatibilityMatrix,
    pub deploy: DeployConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    /// ISO-8601 timestamp in IST (e.g. "2026-03-24T15:00:00+05:30")
    pub timestamp: String,
    pub git_commit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BinaryHashes {
    pub rc_agent_sha256: String,
    pub rc_sentry_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompatibilityMatrix {
    pub racecontrol_min_version: String,
    pub config_schema_version: u32,
    pub kiosk_build_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeployConfig {
    pub binary_url_base: String,
}

/// Parse a TOML string into a ReleaseManifest.
pub fn parse_manifest(toml_str: &str) -> Result<ReleaseManifest, String> {
    toml::from_str(toml_str).map_err(|e| format!("manifest parse error: {e}"))
}

// ── PipelineState (OTA-08) ─────────────────────────────────────────────────

/// State machine for the OTA deploy pipeline.
/// Persisted to deploy-state.json so the pipeline survives server restarts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PipelineState {
    Idle,
    Building,
    Staging,
    Canary,
    StagedRollout,
    HealthChecking,
    Completed,
    RollingBack,
    Paused, // HUMAN-CONFIRM gate pending operator confirmation
}

impl PipelineState {
    /// Returns true if this state represents a terminal (non-active) condition.
    pub fn is_terminal(&self) -> bool {
        matches!(self, PipelineState::Idle | PipelineState::Completed)
    }
}

// ── DeployRecord (OTA-08) ──────────────────────────────────────────────────

/// Persisted pipeline state — written atomically to deploy-state.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployRecord {
    pub state: PipelineState,
    pub manifest_version: String,
    pub started_at: String,
    pub updated_at: String,
    pub waves_completed: u8,
    pub failed_pods: Vec<String>,
    pub rollback_reason: Option<String>,
}

impl DeployRecord {
    /// Create a new deploy record for a fresh pipeline run.
    pub fn new(manifest_version: &str) -> Self {
        let now = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
        Self {
            state: PipelineState::Idle,
            manifest_version: manifest_version.to_string(),
            started_at: now.clone(),
            updated_at: now,
            waves_completed: 0,
            failed_pods: Vec::new(),
            rollback_reason: None,
        }
    }

    /// Update the record timestamp and state.
    pub fn transition(&mut self, new_state: PipelineState) {
        self.state = new_state;
        self.updated_at = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string();
    }
}

// ── SHA256 utilities (OTA-10) ──────────────────────────────────────────────

/// Compute SHA256 hex digest of a byte slice.
pub fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Compute SHA256 hex digest of a file using streaming reads (8KB chunks).
/// Does NOT load the entire file into memory.
pub fn compute_sha256_file(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("open failed: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let n = file.read(&mut buffer).map_err(|e| format!("read failed: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// ── deploy-state.json persistence ──────────────────────────────────────────

/// Atomically persist pipeline state to deploy-state.json (tmp file + rename).
/// If the process dies mid-write, the old file remains intact.
pub fn persist_pipeline_state(record: &DeployRecord) -> Result<(), String> {
    let json = serde_json::to_string_pretty(record)
        .map_err(|e| format!("serialize failed: {e}"))?;
    let tmp_path = format!("{DEPLOY_STATE_FILE}.tmp");
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("write tmp failed: {e}"))?;
    std::fs::rename(&tmp_path, DEPLOY_STATE_FILE)
        .map_err(|e| format!("rename failed: {e}"))?;
    Ok(())
}

/// Load pipeline state from deploy-state.json.
/// Returns None if file does not exist or contains invalid JSON.
pub fn load_pipeline_state() -> Option<DeployRecord> {
    let data = std::fs::read_to_string(DEPLOY_STATE_FILE).ok()?;
    serde_json::from_str(&data).ok()
}

/// Check if a previous pipeline was interrupted (non-terminal state on startup).
/// If found, logs a warning and marks it as interrupted.
pub fn check_interrupted_pipeline() {
    if let Some(mut record) = load_pipeline_state() {
        if !record.state.is_terminal() {
            tracing::warn!(
                state = ?record.state,
                version = %record.manifest_version,
                waves_completed = record.waves_completed,
                "Interrupted OTA pipeline detected on startup — marking as interrupted"
            );
            record.rollback_reason = Some("server_restart_interrupted".to_string());
            record.transition(PipelineState::RollingBack);
            if let Err(e) = persist_pipeline_state(&record) {
                tracing::error!("Failed to persist interrupted pipeline state: {e}");
            }
        }
    }
}

// ── Standing Rules Gate (SR-04) ─────────────────────────────────────────────

/// Exit codes from gate-check.sh
#[derive(Debug, PartialEq, Eq)]
pub enum GateResult {
    Pass,           // exit 0 -- all checks passed
    Fail(String),   // exit 1 -- gate failure, must rollback
    HumanConfirm,   // exit 2 -- HUMAN-CONFIRM items pending, must pause
}

/// Run gate-check.sh with the specified mode.
/// Returns GateResult based on exit code.
pub fn run_gate_check(mode: &str) -> GateResult {
    let repo_root = std::env::current_dir().unwrap_or_default();
    let script = repo_root.join("test").join("gate-check.sh");

    if !script.exists() {
        return GateResult::Fail(format!("gate-check.sh not found at {}", script.display()));
    }

    let output = std::process::Command::new("bash")
        .arg(&script)
        .arg(mode)
        .current_dir(&repo_root)
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            match out.status.code() {
                Some(0) => GateResult::Pass,
                Some(2) => GateResult::HumanConfirm,
                Some(code) => GateResult::Fail(format!(
                    "gate-check.sh exited with code {}\nstdout: {}\nstderr: {}",
                    code,
                    stdout.chars().take(500).collect::<String>(),
                    stderr.chars().take(500).collect::<String>()
                )),
                None => GateResult::Fail("gate-check.sh killed by signal".to_string()),
            }
        }
        Err(e) => GateResult::Fail(format!("Failed to run gate-check.sh: {}", e)),
    }
}

/// Run pre-deploy gate. Returns the state the pipeline should transition to.
/// On Pass: proceed to Canary
/// On Fail: RollingBack
/// On HumanConfirm: Paused
pub fn run_pre_deploy_gate() -> PipelineState {
    match run_gate_check("--pre-deploy") {
        GateResult::Pass => PipelineState::Canary,
        GateResult::Fail(reason) => {
            tracing::error!("Pre-deploy gate FAILED: {}", reason);
            PipelineState::RollingBack
        }
        GateResult::HumanConfirm => {
            tracing::warn!("Pre-deploy gate requires HUMAN-CONFIRM -- pipeline paused");
            PipelineState::Paused
        }
    }
}

/// Run post-wave gate. Returns the state the pipeline should transition to.
pub fn run_post_wave_gate(wave: u32) -> PipelineState {
    match run_gate_check(&format!("--post-wave {}", wave)) {
        GateResult::Pass => PipelineState::StagedRollout,
        GateResult::Fail(reason) => {
            tracing::error!("Post-wave {} gate FAILED: {}", wave, reason);
            PipelineState::RollingBack
        }
        GateResult::HumanConfirm => {
            tracing::warn!("Post-wave {} gate requires HUMAN-CONFIRM -- pipeline paused", wave);
            PipelineState::Paused
        }
    }
}

/// Resume pipeline from Paused state after operator confirmation.
/// Only valid when current state is Paused.
pub fn resume_from_pause(record: &mut DeployRecord) -> Result<(), String> {
    if record.state != PipelineState::Paused {
        return Err(format!(
            "Cannot resume: pipeline is in {:?}, not Paused",
            record.state
        ));
    }
    // Re-run the gate check to confirm operator has resolved all items
    match run_gate_check("--pre-deploy") {
        GateResult::Pass => {
            record.transition(PipelineState::Canary);
            Ok(())
        }
        GateResult::Fail(reason) => {
            record.transition(PipelineState::RollingBack);
            Err(format!("Gate still failing after resume: {}", reason))
        }
        GateResult::HumanConfirm => {
            Err("HUMAN-CONFIRM items still pending".to_string())
        }
    }
}

// ── Health Gate (OTA-02, OTA-10) ───────────────────────────────────────────

/// Error spike threshold — pods with more than this many violations fail the health gate.
const ERROR_SPIKE_THRESHOLD: u32 = 100;

/// Pipeline errors with structured context for logging and rollback decisions.
#[derive(Debug)]
pub enum PipelineError {
    ManifestInvalid(String),
    HealthGateFailed { wave: u8, failures: Vec<HealthFailure> },
    SessionTimeout { pod_id: String },
    RollbackTriggered { wave: u8, reason: String },
    PodNotFound(String),
    PersistFailed(String),
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineError::ManifestInvalid(e) => write!(f, "manifest invalid: {e}"),
            PipelineError::HealthGateFailed { wave, failures } => {
                write!(f, "health gate failed on wave {wave}: ")?;
                for (i, fail) in failures.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}={}", fail.pod_id, fail.reason)?;
                }
                Ok(())
            }
            PipelineError::SessionTimeout { pod_id } => write!(f, "session timeout on {pod_id}"),
            PipelineError::RollbackTriggered { wave, reason } => write!(f, "rollback on wave {wave}: {reason}"),
            PipelineError::PodNotFound(id) => write!(f, "pod not found: {id}"),
            PipelineError::PersistFailed(e) => write!(f, "persist failed: {e}"),
        }
    }
}

/// A single health check failure for a specific pod.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthFailure {
    pub pod_id: String,
    pub reason: String,
}

/// Verify a pod's health after deploy. Uses binary SHA256 (OTA-10) for identity,
/// not git commit hash. Pure function — testable without AppState.
///
/// - `binary_sha256`: from pod's /health endpoint (computed once at startup)
/// - `expected_sha256`: from manifest.binaries.rc_agent_sha256
pub fn health_check_pod(
    _pod_id: &str,
    ws_connected: bool,
    http_reachable: bool,
    binary_sha256: Option<&str>,
    expected_sha256: &str,
    violation_count_24h: u32,
) -> Result<(), String> {
    if !ws_connected {
        return Err("ws_disconnected".to_string());
    }
    if !http_reachable {
        return Err("http_unreachable".to_string());
    }
    match binary_sha256 {
        Some(sha) if sha == expected_sha256 => {}
        Some(_sha) => return Err("sha256_mismatch".to_string()),
        None => return Err("sha256_missing".to_string()),
    }
    if violation_count_24h > ERROR_SPIKE_THRESHOLD {
        return Err(format!(
            "error_spike: {} violations (threshold {})",
            violation_count_24h, ERROR_SPIKE_THRESHOLD
        ));
    }
    Ok(())
}

// ── OTA Sentinel + Kill Switch (OTA-09) ────────────────────────────────────

/// Sentinel file path on each pod — prevents recovery systems from fighting the OTA.
const OTA_SENTINEL_PATH: &str = r"C:\RacingPoint\ota-in-progress.flag";
const OTA_SENTINEL_CONTENT: &str = "ota_pipeline_in_progress\n";
/// rc-sentry reads this file at each watchdog tick to decide whether to restart rc-agent.
const SENTRY_FLAGS_PATH: &str = r"C:\RacingPoint\sentry-flags.json";

/// Session wait timeout — how long to wait for active billing sessions to end.
#[allow(dead_code)]
const SESSION_WAIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30 * 60);

/// Write OTA sentinel to each pod via rc-agent /write endpoint.
/// Called at pipeline start to prevent WoL and watchdog from fighting the deploy.
pub async fn set_ota_sentinel(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)], // (pod_id, ip)
) {
    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/write");
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({
                "path": OTA_SENTINEL_PATH,
                "content": OTA_SENTINEL_CONTENT,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("OTA sentinel set on {pod_id}"),
            Err(e) => tracing::warn!("Failed to set OTA sentinel on {pod_id}: {e}"),
        }
    }
}

/// Remove OTA sentinel from each pod via rc-agent /exec endpoint.
/// Called at pipeline end (success or failure).
pub async fn clear_ota_sentinel(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)],
) {
    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/exec");
        let cmd = format!(r#"del /Q "{OTA_SENTINEL_PATH}""#);
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({ "cmd": cmd, "timeout_ms": 5000 }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("OTA sentinel cleared on {pod_id}"),
            Err(e) => tracing::warn!("Failed to clear OTA sentinel on {pod_id}: {e}"),
        }
    }
}

/// Set/clear kill_watchdog_restart flag on all connected pods via rc-agent /write.
/// When active=true, rc-sentry's watchdog skips rc-agent restart attempts
/// during the deploy window.
pub async fn set_kill_switch(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)],
    active: bool,
) {
    let flags_json = serde_json::json!({
        "kill_switches": {
            "kill_watchdog_restart": active
        }
    })
    .to_string();

    for (pod_id, ip) in pod_ips {
        let url = format!("http://{ip}:8090/write");
        let result = http_client
            .post(&url)
            .json(&serde_json::json!({
                "path": SENTRY_FLAGS_PATH,
                "content": flags_json,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        match result {
            Ok(_) => tracing::debug!("kill_watchdog_restart={active} written to {pod_id}"),
            Err(e) => tracing::warn!("Failed to write sentry-flags.json to {pod_id}: {e}"),
        }
    }
}

// ── Rollback (OTA-04, OTA-07) ──────────────────────────────────────────────

/// Rollback a wave of pods to rc-agent-prev.exe.
///
/// CRITICAL: Executes rollback via rc-sentry :8091/exec, NOT rc-agent :8090/exec.
/// The rollback bat runs `taskkill /F /IM rc-agent.exe` — executing via rc-agent
/// would kill the process serving the exec endpoint. rc-sentry is a separate binary
/// that survives the kill. (Standing rule: "NEVER use taskkill /F /IM rc-agent.exe
/// followed by start in the same exec chain [via rc-agent].")
pub async fn rollback_wave(
    http_client: &reqwest::Client,
    pod_ips: &[(String, String)], // (pod_id, ip)
    sentry_service_key: Option<&str>,
) {
    for (pod_id, ip) in pod_ips {
        tracing::warn!("OTA: Rolling back {pod_id}");

        // Step 1: Write do-rollback.bat via rc-agent /write (agent still alive)
        let write_url = format!("http://{ip}:8090/write");
        let write_result = http_client
            .post(&write_url)
            .json(&serde_json::json!({
                "path": r"C:\RacingPoint\do-rollback.bat",
                "content": crate::deploy::ROLLBACK_SCRIPT_CONTENT,
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        if write_result.is_err() {
            tracing::error!("OTA: Failed to write rollback script to {pod_id}");
            continue;
        }

        // Step 2: Execute rollback via rc-SENTRY :8091/exec (NOT rc-agent :8090)
        let exec_url = format!("http://{ip}:8091/exec");
        let mut req = http_client
            .post(&exec_url);
        if let Some(key) = sentry_service_key {
            req = req.header("X-Service-Key", key);
        }
        let _ = req
            .json(&serde_json::json!({
                "cmd": r#"start /min cmd /c C:\RacingPoint\do-rollback.bat"#,
                "timeout_ms": 5000,
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        tracing::info!("OTA: Rollback triggered for {pod_id} via rc-sentry :8091");
    }

    // Wait for rollback to complete (same delay pattern as deploy.rs)
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
}

/// Check if a pod has an active billing session that should defer its deploy.
pub fn has_active_billing_session(billing_session_id: &Option<String>) -> bool {
    billing_session_id.is_some()
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_MANIFEST_TOML: &str = r#"
[release]
version = "0c0c8134"
timestamp = "2026-03-24T15:00:00+05:30"
git_commit = "0c0c8134"

[binaries]
rc_agent_sha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
rc_sentry_sha256 = "a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2c3d4e5f6a1b2"

[compatibility]
racecontrol_min_version = "0c0c8134"
config_schema_version = 3
kiosk_build_id = "0c0c8134"

[deploy]
binary_url_base = "http://192.168.31.27:9998"
"#;

    #[test]
    fn manifest_round_trip() {
        let manifest = parse_manifest(VALID_MANIFEST_TOML).unwrap();
        assert_eq!(manifest.release.version, "0c0c8134");
        assert_eq!(manifest.binaries.rc_agent_sha256, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(manifest.compatibility.config_schema_version, 3);
        assert_eq!(manifest.deploy.binary_url_base, "http://192.168.31.27:9998");

        // Round-trip: serialize back to TOML and parse again
        let toml_str = toml::to_string_pretty(&manifest).unwrap();
        let reparsed = parse_manifest(&toml_str).unwrap();
        assert_eq!(manifest, reparsed);
    }

    #[test]
    fn manifest_rejects_missing_release() {
        let bad = "[binaries]\nrc_agent_sha256 = \"abc\"\nrc_sentry_sha256 = \"def\"\n";
        let result = parse_manifest(bad);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("manifest parse error"));
    }

    #[test]
    fn manifest_rejects_missing_sha256() {
        let bad = r#"
[release]
version = "test"
timestamp = "2026-01-01T00:00:00+05:30"
git_commit = "abc123"

[binaries]
rc_sentry_sha256 = "def456"

[compatibility]
racecontrol_min_version = "test"
config_schema_version = 1
kiosk_build_id = "test"

[deploy]
binary_url_base = "http://localhost:9998"
"#;
        let result = parse_manifest(bad);
        assert!(result.is_err(), "Should reject missing rc_agent_sha256");
    }

    #[test]
    fn manifest_compatibility_fields_present() {
        let manifest = parse_manifest(VALID_MANIFEST_TOML).unwrap();
        assert_eq!(manifest.compatibility.racecontrol_min_version, "0c0c8134");
        assert_eq!(manifest.compatibility.config_schema_version, 3);
        assert_eq!(manifest.compatibility.kiosk_build_id, "0c0c8134");
    }

    #[test]
    fn pipeline_state_serde_round_trip() {
        let states = vec![
            PipelineState::Idle,
            PipelineState::Building,
            PipelineState::Staging,
            PipelineState::Canary,
            PipelineState::StagedRollout,
            PipelineState::HealthChecking,
            PipelineState::Completed,
            PipelineState::RollingBack,
            PipelineState::Paused,
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let reparsed: PipelineState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, reparsed, "Round-trip failed for {state:?}");
        }
    }

    #[test]
    fn pipeline_state_snake_case_format() {
        assert_eq!(
            serde_json::to_string(&PipelineState::StagedRollout).unwrap(),
            "\"staged_rollout\""
        );
        assert_eq!(
            serde_json::to_string(&PipelineState::HealthChecking).unwrap(),
            "\"health_checking\""
        );
        assert_eq!(
            serde_json::to_string(&PipelineState::RollingBack).unwrap(),
            "\"rolling_back\""
        );
    }

    #[test]
    fn pipeline_state_terminal_check() {
        assert!(PipelineState::Idle.is_terminal());
        assert!(PipelineState::Completed.is_terminal());
        assert!(!PipelineState::Canary.is_terminal());
        assert!(!PipelineState::RollingBack.is_terminal());
        assert!(!PipelineState::StagedRollout.is_terminal());
    }

    #[test]
    fn deploy_record_serializes_with_all_fields() {
        let record = DeployRecord {
            state: PipelineState::RollingBack,
            manifest_version: "abc123".to_string(),
            started_at: "2026-03-24T15:00:00+05:30".to_string(),
            updated_at: "2026-03-24T15:05:00+05:30".to_string(),
            waves_completed: 1,
            failed_pods: vec!["pod-8".to_string()],
            rollback_reason: Some("health gate failed: SHA256 mismatch".to_string()),
        };
        let json = serde_json::to_string_pretty(&record).unwrap();
        assert!(json.contains("rolling_back"));
        assert!(json.contains("abc123"));
        assert!(json.contains("health gate failed"));
        assert!(json.contains("pod-8"));

        // Deserialize back
        let reparsed: DeployRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.state, PipelineState::RollingBack);
        assert_eq!(reparsed.rollback_reason.as_deref(), Some("health gate failed: SHA256 mismatch"));
    }

    #[test]
    fn deploy_record_optional_rollback_reason() {
        let record = DeployRecord {
            state: PipelineState::Completed,
            manifest_version: "v1".to_string(),
            started_at: "t0".to_string(),
            updated_at: "t1".to_string(),
            waves_completed: 3,
            failed_pods: Vec::new(),
            rollback_reason: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        let reparsed: DeployRecord = serde_json::from_str(&json).unwrap();
        assert!(reparsed.rollback_reason.is_none());
    }

    #[test]
    fn sha256_known_input() {
        // SHA256("hello world") = b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9
        let hash = compute_sha256(b"hello world");
        assert_eq!(hash, "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    #[test]
    fn sha256_empty_input() {
        // SHA256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
        let hash = compute_sha256(b"");
        assert_eq!(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    }

    #[test]
    fn sha256_file_matches_in_memory() {
        let data = b"Racing Point eSports OTA Pipeline test data";
        let expected = compute_sha256(data);

        let tmp = std::env::temp_dir().join("ota_sha256_test.bin");
        std::fs::write(&tmp, data).unwrap();
        let file_hash = compute_sha256_file(&tmp).unwrap();
        std::fs::remove_file(&tmp).ok();

        assert_eq!(file_hash, expected);
    }

    #[test]
    fn sha256_file_not_found() {
        let result = compute_sha256_file(std::path::Path::new(r"C:\nonexistent\fake.bin"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("open failed"));
    }

    #[test]
    fn wave_constants_correct() {
        assert_eq!(WAVE_1, &[8]);
        assert_eq!(WAVE_2, &[1, 2, 3, 4]);
        assert_eq!(WAVE_3, &[5, 6, 7]);
        assert_eq!(ALL_WAVES.len(), 3);
    }

    #[test]
    fn persistence_round_trip() {
        // Use a temp file for testing instead of the real path
        let tmp = std::env::temp_dir().join("test-deploy-state.json");
        let record = DeployRecord {
            state: PipelineState::Canary,
            manifest_version: "test-v1".to_string(),
            started_at: "2026-03-24T15:00:00+05:30".to_string(),
            updated_at: "2026-03-24T15:01:00+05:30".to_string(),
            waves_completed: 1,
            failed_pods: Vec::new(),
            rollback_reason: None,
        };
        let json = serde_json::to_string_pretty(&record).unwrap();
        let tmp_write = format!("{}.tmp", tmp.display());
        std::fs::write(&tmp_write, &json).unwrap();
        std::fs::rename(&tmp_write, &tmp).unwrap();

        let loaded: DeployRecord =
            serde_json::from_str(&std::fs::read_to_string(&tmp).unwrap()).unwrap();
        assert_eq!(loaded.state, PipelineState::Canary);
        assert_eq!(loaded.manifest_version, "test-v1");
        assert_eq!(loaded.waves_completed, 1);
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn load_returns_none_for_missing_file() {
        // The actual DEPLOY_STATE_FILE may or may not exist — test the pattern
        let missing = std::path::Path::new(r"C:\nonexistent\deploy-state.json");
        let data = std::fs::read_to_string(missing).ok();
        assert!(data.is_none());
    }

    #[test]
    fn load_returns_none_for_corrupted_json() {
        let tmp = std::env::temp_dir().join("test-corrupted-deploy-state.json");
        std::fs::write(&tmp, "not valid json {{{{").unwrap();
        let result: Option<DeployRecord> =
            serde_json::from_str(&std::fs::read_to_string(&tmp).unwrap_or_default()).ok();
        assert!(result.is_none());
        std::fs::remove_file(&tmp).ok();
    }

    #[test]
    fn deploy_record_transition_updates_state() {
        let mut record = DeployRecord {
            state: PipelineState::Idle,
            manifest_version: "v1".to_string(),
            started_at: "t0".to_string(),
            updated_at: "t0".to_string(),
            waves_completed: 0,
            failed_pods: Vec::new(),
            rollback_reason: None,
        };
        record.transition(PipelineState::Canary);
        assert_eq!(record.state, PipelineState::Canary);
        assert_ne!(record.updated_at, "t0"); // timestamp was updated
    }

    // ── Health Gate Tests (Plan 02) ────────────────────────────────────────

    const EXPECTED_SHA: &str = "abc123def456";

    #[test]
    fn health_check_passes_for_healthy_pod() {
        let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 5);
        assert!(result.is_ok());
    }

    #[test]
    fn health_check_fails_ws_disconnected() {
        let result = health_check_pod("pod_8", false, true, Some(EXPECTED_SHA), EXPECTED_SHA, 0);
        assert_eq!(result.unwrap_err(), "ws_disconnected");
    }

    #[test]
    fn health_check_fails_http_unreachable() {
        let result = health_check_pod("pod_8", true, false, Some(EXPECTED_SHA), EXPECTED_SHA, 0);
        assert_eq!(result.unwrap_err(), "http_unreachable");
    }

    #[test]
    fn health_check_fails_sha256_mismatch() {
        let result = health_check_pod("pod_8", true, true, Some("wrong_hash"), EXPECTED_SHA, 0);
        assert_eq!(result.unwrap_err(), "sha256_mismatch");
    }

    #[test]
    fn health_check_fails_sha256_missing() {
        let result = health_check_pod("pod_8", true, true, None, EXPECTED_SHA, 0);
        assert_eq!(result.unwrap_err(), "sha256_missing");
    }

    #[test]
    fn health_check_fails_error_spike() {
        let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 101);
        let err = result.unwrap_err();
        assert!(err.contains("error_spike"));
        assert!(err.contains("101"));
    }

    #[test]
    fn health_check_passes_at_threshold() {
        // Exactly at threshold should pass (> not >=)
        let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 100);
        assert!(result.is_ok());
    }

    // ── Sentinel + Kill Switch Tests (Plan 03) ────────────────────────────

    #[test]
    fn ota_sentinel_path_is_correct() {
        assert_eq!(OTA_SENTINEL_PATH, r"C:\RacingPoint\ota-in-progress.flag");
    }

    #[test]
    fn sentry_flags_path_is_correct() {
        assert_eq!(SENTRY_FLAGS_PATH, r"C:\RacingPoint\sentry-flags.json");
    }

    #[test]
    fn has_active_billing_true_when_session() {
        assert!(has_active_billing_session(&Some("sess-123".to_string())));
    }

    #[test]
    fn has_active_billing_false_when_none() {
        assert!(!has_active_billing_session(&None));
    }

    #[test]
    fn pipeline_error_display() {
        let err = PipelineError::HealthGateFailed {
            wave: 1,
            failures: vec![
                HealthFailure { pod_id: "pod_8".to_string(), reason: "ws_disconnected".to_string() },
            ],
        };
        let msg = format!("{err}");
        assert!(msg.contains("wave 1"));
        assert!(msg.contains("pod_8"));
        assert!(msg.contains("ws_disconnected"));

        let err2 = PipelineError::SessionTimeout { pod_id: "pod_3".to_string() };
        assert!(format!("{err2}").contains("pod_3"));

        let err3 = PipelineError::PersistFailed("disk full".to_string());
        assert!(format!("{err3}").contains("disk full"));
    }

    #[test]
    fn health_failure_serializes() {
        let failure = HealthFailure {
            pod_id: "pod_8".to_string(),
            reason: "sha256_mismatch".to_string(),
        };
        let json = serde_json::to_string(&failure).unwrap();
        assert!(json.contains("pod_8"));
        assert!(json.contains("sha256_mismatch"));
    }

    // ── Paused State + Gate Integration Tests (Plan 03, SR-04) ──────────

    #[test]
    fn paused_state_serialization() {
        let json = serde_json::to_string(&PipelineState::Paused).unwrap();
        assert_eq!(json, "\"paused\"");
        let reparsed: PipelineState = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed, PipelineState::Paused);
    }

    #[test]
    fn paused_is_not_terminal() {
        assert!(!PipelineState::Paused.is_terminal());
    }

    #[test]
    fn gate_result_debug_format() {
        let pass = GateResult::Pass;
        let fail = GateResult::Fail("test failure".to_string());
        let confirm = GateResult::HumanConfirm;
        assert_eq!(format!("{pass:?}"), "Pass");
        assert!(format!("{fail:?}").contains("test failure"));
        assert_eq!(format!("{confirm:?}"), "HumanConfirm");
    }

    #[test]
    fn paused_deploy_record_serializes() {
        let record = DeployRecord {
            state: PipelineState::Paused,
            manifest_version: "gate-test".to_string(),
            started_at: "t0".to_string(),
            updated_at: "t1".to_string(),
            waves_completed: 0,
            failed_pods: Vec::new(),
            rollback_reason: None,
        };
        let json = serde_json::to_string_pretty(&record).unwrap();
        assert!(json.contains("\"paused\""));
        let reparsed: DeployRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(reparsed.state, PipelineState::Paused);
    }
}
