//! Pre-flight session checks (Phase 97 Plan 02).
//!
//! Runs three concurrent checks before every BillingStarted session:
//! 1. HID — wheelbase connected (FfbBackend::zero_force)
//! 2. ConspitLink — process running + config valid
//! 3. Orphan game — stale game process killed before new session
//!
//! All three run concurrently via `tokio::join!` with a 5-second hard timeout.
//! ConspitLink has an auto-fix: spawn process, wait 2s, re-check.
//! Orphan game kill IS the fix — returns Pass after successful kill.
//!
//! If any check remains Fail after auto-fix: returns MaintenanceRequired.
//! If all Pass or Warn: returns Pass.

use std::sync::atomic::Ordering;

use tokio::task::spawn_blocking;
use tokio::time::{timeout, Duration};
use tracing;

use crate::app_state::AppState;
use crate::ffb_controller::FfbBackend;

// ─── Public Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

pub enum PreFlightResult {
    Pass,
    MaintenanceRequired { failures: Vec<CheckResult> },
}

// ─── Public Entry Point ───────────────────────────────────────────────────────

/// Run all pre-flight checks with a 5-second hard timeout.
///
/// Checks run concurrently. On failure, auto-fix is attempted (ConspitLink only).
/// Returns `Pass` if all checks are Pass or Warn after fixes.
/// Returns `MaintenanceRequired` if any check remains Fail after auto-fix.
pub async fn run(state: &AppState, ffb: &dyn FfbBackend) -> PreFlightResult {
    // Capture billing_active flag before the borrow of state for checks.
    // This is read-only and atomic, so safe to capture before the join.
    let billing_active = state.heartbeat_status.billing_active.load(Ordering::Relaxed);
    let game_pid = state.game_process.as_ref().and_then(|gp| gp.pid);
    let has_game_process = state.game_process.is_some();

    // 5-second hard timeout on the concurrent checks
    let join_result = timeout(
        Duration::from_secs(5),
        run_concurrent_checks(ffb, billing_active, has_game_process, game_pid),
    )
    .await;

    let mut results = match join_result {
        Ok(results) => results,
        Err(_) => {
            tracing::warn!("Pre-flight hard timeout (5s) expired");
            return PreFlightResult::MaintenanceRequired {
                failures: vec![CheckResult {
                    name: "pre_flight_timeout",
                    status: CheckStatus::Fail,
                    detail: "Pre-flight checks timed out after 5 seconds".into(),
                }],
            };
        }
    };

    // Auto-fix loop: for each Fail result, attempt fix and re-check
    for result in results.iter_mut() {
        if matches!(result.status, CheckStatus::Fail) {
            if result.name == "conspit_link" {
                tracing::warn!("ConspitLink check failed ({}), attempting auto-fix", result.detail);
                let fixed = fix_conspit().await;
                if fixed {
                    let new_result = check_conspit().await;
                    tracing::info!("ConspitLink after auto-fix: {:?}", new_result.status);
                    *result = new_result;
                } else {
                    tracing::warn!("ConspitLink auto-fix failed (process did not start)");
                }
            }
            // HID: no auto-fix available (hardware)
            // Orphan game: kill IS the fix — check_orphan_game returns Pass after kill
        }
    }

    // Collect remaining failures
    let failures: Vec<CheckResult> = results
        .into_iter()
        .filter(|r| matches!(r.status, CheckStatus::Fail))
        .collect();

    if failures.is_empty() {
        tracing::info!("Pre-flight passed");
        PreFlightResult::Pass
    } else {
        tracing::warn!("Pre-flight FAILED: {:?}", failures.iter().map(|f| &f.detail).collect::<Vec<_>>());
        PreFlightResult::MaintenanceRequired { failures }
    }
}

// ─── Concurrent Check Runner ──────────────────────────────────────────────────

async fn run_concurrent_checks(
    ffb: &dyn FfbBackend,
    billing_active: bool,
    has_game_process: bool,
    game_pid: Option<u32>,
) -> Vec<CheckResult> {
    let (hid, conspit, orphan) = tokio::join!(
        check_hid(ffb),
        check_conspit(),
        check_orphan_game(billing_active, has_game_process, game_pid),
    );
    vec![hid, conspit, orphan]
}

// ─── Individual Check Functions ───────────────────────────────────────────────

/// HID check: verify the wheelbase is connected.
///
/// Uses FfbBackend::zero_force() — a quick HID write, no spawn_blocking needed.
/// Ok(true) = device found (Pass)
/// Ok(false) = device not found (Fail)
/// Err(e) = HID write failed (Fail)
async fn check_hid(ffb: &dyn FfbBackend) -> CheckResult {
    match ffb.zero_force() {
        Ok(true) => CheckResult {
            name: "hid",
            status: CheckStatus::Pass,
            detail: "Wheelbase HID connected".into(),
        },
        Ok(false) => CheckResult {
            name: "hid",
            status: CheckStatus::Fail,
            detail: "Wheelbase HID not detected (VID:0x1209 PID:0xFFB0)".into(),
        },
        Err(e) => CheckResult {
            name: "hid",
            status: CheckStatus::Fail,
            detail: format!("Wheelbase HID error: {}", e),
        },
    }
}

/// ConspitLink check: verify ConspitLink.exe is running and its config is valid.
///
/// Uses spawn_blocking because sysinfo::refresh_processes blocks 100-300ms on Windows.
/// Two-stage: (1) process running? (2) config.json present and valid?
async fn check_conspit() -> CheckResult {
    let result = spawn_blocking(|| {
        use sysinfo::{ProcessesToUpdate, System};

        let mut sys = System::new();
        sys.refresh_processes(ProcessesToUpdate::All, true);
        let found = sys.processes().values().any(|p| {
            p.name().to_string_lossy().eq_ignore_ascii_case("ConspitLink.exe")
        });

        if !found {
            return CheckResult {
                name: "conspit_link",
                status: CheckStatus::Fail,
                detail: "ConspitLink.exe not running".into(),
            };
        }

        // Stage 2: check config file
        let config_path = std::path::Path::new(r"C:\ConspitLink\config.json");
        if !config_path.exists() {
            return CheckResult {
                name: "conspit_link",
                status: CheckStatus::Warn,
                detail: "ConspitLink running but config.json missing".into(),
            };
        }

        match std::fs::read_to_string(config_path) {
            Ok(content) => {
                if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
                    CheckResult {
                        name: "conspit_link",
                        status: CheckStatus::Pass,
                        detail: "ConspitLink running, config valid".into(),
                    }
                } else {
                    CheckResult {
                        name: "conspit_link",
                        status: CheckStatus::Warn,
                        detail: "ConspitLink running but config.json is invalid JSON".into(),
                    }
                }
            }
            Err(e) => CheckResult {
                name: "conspit_link",
                status: CheckStatus::Warn,
                detail: format!("ConspitLink running but config.json unreadable: {}", e),
            },
        }
    })
    .await
    .unwrap_or_else(|e| CheckResult {
        name: "conspit_link",
        status: CheckStatus::Fail,
        detail: format!("spawn_blocking panicked: {}", e),
    });

    result
}

/// Orphan game check: kill stale game process before new session.
///
/// Only runs kill if: game_process exists AND billing is NOT active.
/// Kill IS the fix — returns Pass after successful kill.
/// Uses PID from state.game_process (never name-based kill).
async fn check_orphan_game(billing_active: bool, has_game_process: bool, game_pid: Option<u32>) -> CheckResult {
    if !has_game_process || billing_active {
        return CheckResult {
            name: "orphan_game",
            status: CheckStatus::Pass,
            detail: "No orphaned game process".into(),
        };
    }

    match game_pid {
        Some(pid) => {
            // PID-targeted kill — never name-based
            let kill_result = spawn_blocking(move || {
                std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &pid.to_string()])
                    .output()
            })
            .await;

            match kill_result {
                Ok(Ok(output)) if output.status.success() => {
                    tracing::info!("Orphaned game process (PID {}) killed successfully", pid);
                    CheckResult {
                        name: "orphan_game",
                        status: CheckStatus::Pass,
                        detail: format!("Orphaned game process (PID {}) killed", pid),
                    }
                }
                Ok(Ok(output)) => {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    tracing::warn!("taskkill PID {} failed: {}", pid, stderr);
                    CheckResult {
                        name: "orphan_game",
                        status: CheckStatus::Fail,
                        detail: format!("Failed to kill orphaned game process (PID {}): {}", pid, stderr),
                    }
                }
                Ok(Err(e)) => CheckResult {
                    name: "orphan_game",
                    status: CheckStatus::Fail,
                    detail: format!("taskkill spawn failed for PID {}: {}", pid, e),
                },
                Err(e) => CheckResult {
                    name: "orphan_game",
                    status: CheckStatus::Fail,
                    detail: format!("spawn_blocking panicked during taskkill: {}", e),
                },
            }
        }
        None => CheckResult {
            name: "orphan_game",
            status: CheckStatus::Warn,
            detail: "Orphaned game_process record but no PID — clearing state".into(),
        },
    }
}

// ─── Auto-Fix Functions ───────────────────────────────────────────────────────

/// Attempt to restart ConspitLink.exe.
///
/// Spawns the process with CREATE_NO_WINDOW, waits 2 seconds, then re-scans.
/// Returns true if ConspitLink.exe is found in the process list after the wait.
/// Wraps everything in a 3-second timeout.
async fn fix_conspit() -> bool {
    let fix_result = timeout(Duration::from_secs(3), async {
        spawn_blocking(|| {
            use sysinfo::{ProcessesToUpdate, System};

            let mut cmd = std::process::Command::new(r"C:\ConspitLink\ConspitLink.exe");
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            // Attempt to spawn — ignore result (process may already be starting)
            let _ = cmd.spawn();

            // Wait for process to start
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Re-scan process list
            let mut sys = System::new();
            sys.refresh_processes(ProcessesToUpdate::All, true);
            sys.processes().values().any(|p| {
                p.name().to_string_lossy().eq_ignore_ascii_case("ConspitLink.exe")
            })
        })
        .await
        .unwrap_or(false)
    })
    .await;

    fix_result.unwrap_or(false)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;

    mock! {
        pub HidBackend {}
        impl FfbBackend for HidBackend {
            fn zero_force(&self) -> Result<bool, String>;
            fn zero_force_with_retry(&self, attempts: u8, delay_ms: u64) -> bool;
            fn set_gain(&self, percent: u8) -> Result<bool, String>;
            fn fxm_reset(&self) -> Result<bool, String>;
            fn set_idle_spring(&self, value: i64) -> Result<bool, String>;
        }
    }

    #[tokio::test]
    async fn test_hid_pass() {
        let mut mock = MockHidBackend::new();
        mock.expect_zero_force().returning(|| Ok(true));
        let result = check_hid(&mock).await;
        assert!(matches!(result.status, CheckStatus::Pass));
        assert_eq!(result.detail, "Wheelbase HID connected");
    }

    #[tokio::test]
    async fn test_hid_fail_not_found() {
        let mut mock = MockHidBackend::new();
        mock.expect_zero_force().returning(|| Ok(false));
        let result = check_hid(&mock).await;
        assert!(matches!(result.status, CheckStatus::Fail));
        assert!(result.detail.contains("VID:0x1209 PID:0xFFB0"));
    }

    #[tokio::test]
    async fn test_hid_fail_error() {
        let mut mock = MockHidBackend::new();
        mock.expect_zero_force().returning(|| Err("USB error".to_string()));
        let result = check_hid(&mock).await;
        assert!(matches!(result.status, CheckStatus::Fail));
        assert!(result.detail.contains("USB error"));
    }

    #[tokio::test]
    async fn test_orphan_game_no_process() {
        // No game process, no billing — should pass
        let result = check_orphan_game(false, false, None).await;
        assert!(matches!(result.status, CheckStatus::Pass));
    }

    #[tokio::test]
    async fn test_orphan_game_billing_active() {
        // Game running but billing active — should pass (do not kill during active session)
        let result = check_orphan_game(true, true, Some(1234)).await;
        assert!(matches!(result.status, CheckStatus::Pass));
    }

    #[tokio::test]
    async fn test_orphan_game_no_pid() {
        // Game process record exists but no PID — should warn
        let result = check_orphan_game(false, true, None).await;
        assert!(matches!(result.status, CheckStatus::Warn));
        assert!(result.detail.contains("no PID"));
    }
}
