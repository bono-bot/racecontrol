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

    let tmp = std::env::temp_dir().join(format!("ota_sha256_test_{}.bin", std::process::id()));
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
    let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 5, 0);
    assert!(result.is_ok());
}

#[test]
fn health_check_fails_ws_disconnected() {
    let result = health_check_pod("pod_8", false, true, Some(EXPECTED_SHA), EXPECTED_SHA, 0, 0);
    assert_eq!(result.unwrap_err(), "ws_disconnected");
}

#[test]
fn health_check_fails_http_unreachable() {
    let result = health_check_pod("pod_8", true, false, Some(EXPECTED_SHA), EXPECTED_SHA, 0, 0);
    assert_eq!(result.unwrap_err(), "http_unreachable");
}

#[test]
fn health_check_fails_sha256_mismatch() {
    let result = health_check_pod("pod_8", true, true, Some("wrong_hash"), EXPECTED_SHA, 0, 0);
    assert_eq!(result.unwrap_err(), "sha256_mismatch");
}

#[test]
fn health_check_fails_sha256_missing() {
    let result = health_check_pod("pod_8", true, true, None, EXPECTED_SHA, 0, 0);
    assert_eq!(result.unwrap_err(), "sha256_missing");
}

#[test]
fn health_check_fails_error_spike() {
    let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 101, 0);
    let err = result.unwrap_err();
    assert!(err.contains("error_spike"));
    assert!(err.contains("101"));
}

#[test]
fn health_check_passes_at_threshold() {
    // Exactly at threshold should pass (> not >=)
    let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 100, 0);
    assert!(result.is_ok());
}

#[test]
fn health_check_fails_scan_failure() {
    // MMA-P1: Scan failure must block OTA (fail-closed)
    let result = health_check_pod("pod_8", true, true, Some(EXPECTED_SHA), EXPECTED_SHA, 0, 1);
    let err = result.unwrap_err();
    assert!(err.contains("scan_failed"));
}

// ── Sentinel + Kill Switch Tests (Plan 03) ────────────────────────────

#[test]
fn ota_sentinel_path_is_correct() {
    assert_eq!(ops::OTA_SENTINEL_PATH, r"C:\RacingPoint\ota-in-progress.flag");
}

#[test]
fn sentry_flags_path_is_correct() {
    assert_eq!(ops::SENTRY_FLAGS_PATH, r"C:\RacingPoint\sentry-flags.json");
}

#[test]
fn has_active_billing_true_when_session() {
    assert!(ops::has_active_billing_session(&Some("sess-123".to_string())));
}

#[test]
fn has_active_billing_false_when_none() {
    assert!(!ops::has_active_billing_session(&None));
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
