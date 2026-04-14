use super::*;

// ── validate_binary_size tests ──────────────────────────────────────────

#[test]
fn validate_binary_size_at_threshold_ok() {
    assert!(validate_binary_size(5_000_000).is_ok());
}

#[test]
fn validate_binary_size_above_threshold_ok() {
    assert!(validate_binary_size(5_000_001).is_ok());
    assert!(validate_binary_size(15_000_000).is_ok());
}

#[test]
fn validate_binary_size_below_threshold_err() {
    assert!(validate_binary_size(4_999_999).is_err());
}

#[test]
fn validate_binary_size_zero_err() {
    let result = validate_binary_size(0);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("too small"));
}

#[test]
fn validate_binary_size_small_file_err() {
    // HTML error page saved as .exe
    assert!(validate_binary_size(1024).is_err());
}

// ── parse_file_size_from_dir tests ──────────────────────────────────────

#[test]
fn parse_file_size_normal_dir_output() {
    let output = " Volume in drive C is Windows\n Directory of C:\\RacingPoint\n\n03/13/2026  10:30 AM        15,234,567 rc-agent.exe\n               1 File(s)     15,234,567 bytes\n";
    assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), Some(15234567));
}

#[test]
fn parse_file_size_file_not_found() {
    let output = "File Not Found\n";
    assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), None);
}

#[test]
fn parse_file_size_empty() {
    assert_eq!(parse_file_size_from_dir("", "rc-agent.exe"), None);
}

#[test]
fn parse_file_size_no_commas() {
    let output = "03/13/2026  10:30 AM        15234567 rc-agent.exe\n";
    assert_eq!(parse_file_size_from_dir(output, "rc-agent.exe"), Some(15234567));
}

// ── deploy_step_label tests ─────────────────────────────────────────────

#[test]
fn deploy_step_label_killing() {
    let label = deploy_step_label(&DeployState::Killing);
    assert_eq!(label, "Killing old rc-agent process");
}

#[test]
fn deploy_step_label_failed() {
    let label = deploy_step_label(&DeployState::Failed {
        reason: "binary too small".to_string(),
    });
    assert_eq!(label, "Deploy failed: binary too small");
}

#[test]
fn deploy_step_label_downloading() {
    let label = deploy_step_label(&DeployState::Downloading { progress_pct: 75 });
    assert_eq!(label, "Downloading new binary (75%)");
}

#[test]
fn deploy_step_label_complete() {
    let label = deploy_step_label(&DeployState::Complete);
    assert_eq!(label, "Deploy completed successfully");
}

// ── RollingBack + script constant tests (Phase 20-deploy-resilience Plan 01) ──

#[test]
fn deploy_step_label_rolling_back() {
    let label = deploy_step_label(&DeployState::RollingBack);
    assert_eq!(label, "Rolling back to previous binary");
}

#[test]
fn swap_script_preserves_prev() {
    assert!(
        SWAP_SCRIPT_CONTENT.contains("rc-agent-prev.exe"),
        "SWAP_SCRIPT_CONTENT must reference rc-agent-prev.exe"
    );
}

#[test]
fn swap_script_crlf() {
    assert!(
        SWAP_SCRIPT_CONTENT.contains("\r\n"),
        "SWAP_SCRIPT_CONTENT must use CRLF line endings for Windows batch files"
    );
}

#[test]
fn swap_script_has_av_retry() {
    assert!(
        SWAP_SCRIPT_CONTENT.contains(":RETRY"),
        "SWAP_SCRIPT_CONTENT must have a :RETRY label for AV retry loop"
    );
    assert!(
        SWAP_SCRIPT_CONTENT.contains("LSS 5"),
        "SWAP_SCRIPT_CONTENT must limit AV retries with LSS 5"
    );
}

#[test]
fn swap_script_preserves_move() {
    assert!(
        SWAP_SCRIPT_CONTENT.contains("move /Y rc-agent.exe rc-agent-prev.exe"),
        "SWAP_SCRIPT_CONTENT must move current binary to rc-agent-prev.exe before swap"
    );
}

#[test]
fn rollback_script_contains_prev() {
    assert!(
        ROLLBACK_SCRIPT_CONTENT.contains("rc-agent-prev.exe"),
        "ROLLBACK_SCRIPT_CONTENT must reference rc-agent-prev.exe"
    );
}

#[test]
fn rollback_script_crlf() {
    assert!(
        ROLLBACK_SCRIPT_CONTENT.contains("\r\n"),
        "ROLLBACK_SCRIPT_CONTENT must use CRLF line endings for Windows batch files"
    );
}

#[test]
fn rollback_script_restores_prev() {
    assert!(
        ROLLBACK_SCRIPT_CONTENT.contains("move /Y rc-agent-prev.exe rc-agent.exe"),
        "ROLLBACK_SCRIPT_CONTENT must restore rc-agent-prev.exe to rc-agent.exe"
    );
}

#[test]
fn rollback_verify_delays_shorter() {
    assert_eq!(
        ROLLBACK_VERIFY_DELAYS.len(),
        3,
        "ROLLBACK_VERIFY_DELAYS must have 3 entries (shorter than deploy's 4)"
    );
    let sum: u64 = ROLLBACK_VERIFY_DELAYS.iter().sum();
    assert_eq!(sum, 50, "ROLLBACK_VERIFY_DELAYS sum must be 50s (5+15+30)");
}

// ── generate_pod_config tests ───────────────────────────────────────────

#[test]
fn generate_pod_config_contains_correct_pod_number() {
    let config = generate_pod_config(3);
    assert!(config.contains("number = 3"));
    assert!(config.contains("\"Pod 03\""));
}

#[test]
fn generate_pod_config_contains_core_url() {
    let config = generate_pod_config(1);
    assert!(config.contains("ws://192.168.31.23:8080/ws/agent"));
}

#[test]
fn generate_pod_config_contains_games() {
    let config = generate_pod_config(8);
    assert!(config.contains("[games.assetto_corsa]"));
    assert!(config.contains("[games.f1_25]"));
    assert!(config.contains("[ai_debugger]"));
}

// ── is_deploy_window_locked tests (DEPLOY-03) ───────────────────────────
// Note: We cannot mock Utc::now() directly, so we test the logic by verifying
// the function signature and that it returns the correct error message format.

#[test]
fn deploy_window_locked_force_true_always_passes() {
    // When force=true the function must always return Ok regardless of time
    // (the weekday/hour check is bypassed by force=true only if locked)
    // We can call it and verify it doesn't panic or return a non-force error
    let result = is_deploy_window_locked(true, "superadmin");
    // force=true must always return Ok (worst case: logs WARN during peak hours)
    assert!(result.is_ok(), "force=true must always return Ok, got: {:?}", result);
}

#[test]
fn deploy_window_error_message_contains_expected_text() {
    // The error message (if triggered) must contain venue-open text.
    // Since we can't mock venue_state, we test the function doesn't panic
    // and returns correct error shape when venue is open.
    let result = is_deploy_window_locked(false, "superadmin");
    match result {
        Ok(()) => {
            // Venue is closed — test passes (deploy allowed)
        }
        Err(msg) => {
            // Venue is open — verify error message contains expected text
            assert!(
                msg.contains("Deploy blocked") && msg.contains("venue is open"),
                "DEPLOY-03 error must mention 'Deploy blocked' and 'venue is open', got: {}",
                msg
            );
            assert!(
                msg.contains("force=true"),
                "DEPLOY-03 error must hint at force=true override, got: {}",
                msg
            );
        }
    }
}

#[test]
fn is_deploy_window_locked_fn_signature() {
    // Verify the function compiles and can be called — name must be exactly
    // `is_deploy_window_locked` per DEPLOY-03 acceptance criteria
    let _: fn(bool, &str) -> Result<(), String> = is_deploy_window_locked;
}
