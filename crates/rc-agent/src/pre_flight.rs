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
    let (hid, conspit, orphan, http, rect) = tokio::join!(
        check_hid(ffb),
        check_conspit(),
        check_orphan_game(billing_active, has_game_process, game_pid),
        check_lock_screen_http(),
        check_window_rect(),
    );
    vec![hid, conspit, orphan, http, rect]
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

// ─── DISP-01: Lock Screen HTTP Probe ─────────────────────────────────────────

/// Public entry point: probe the lock screen HTTP server on port 18923.
///
/// Delegates to `check_lock_screen_http_on` for testability.
async fn check_lock_screen_http() -> CheckResult {
    check_lock_screen_http_on("127.0.0.1:18923").await
}

/// Implementation: TCP connect + HTTP/1.0 GET, check for 200 in response.
///
/// 2-second timeout. On connect success: sends a minimal HTTP request and
/// checks the response starts with "HTTP/1." and contains "200".
/// Returns Fail on connection error, timeout, or non-200 response.
async fn check_lock_screen_http_on(addr: &str) -> CheckResult {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let addr_owned = addr.to_string();
    let connect_result = tokio::time::timeout(
        Duration::from_secs(2),
        tokio::net::TcpStream::connect(&addr_owned),
    )
    .await;

    let mut stream = match connect_result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => {
            return CheckResult {
                name: "lock_screen_http",
                status: CheckStatus::Fail,
                detail: format!("Lock screen HTTP server not reachable on {}: {}", addr_owned, e),
            };
        }
        Err(_) => {
            return CheckResult {
                name: "lock_screen_http",
                status: CheckStatus::Fail,
                detail: format!("Lock screen HTTP server timeout (>2s) on {}", addr_owned),
            };
        }
    };

    // Send minimal HTTP GET request
    let request = format!("GET /health HTTP/1.0\r\nHost: {}\r\n\r\n", addr_owned);
    if stream.write_all(request.as_bytes()).await.is_err() {
        return CheckResult {
            name: "lock_screen_http",
            status: CheckStatus::Fail,
            detail: format!("Lock screen HTTP server write failed on {}", addr_owned),
        };
    }

    // Read up to 256 bytes of response
    let mut buf = [0u8; 256];
    let n = match stream.read(&mut buf).await {
        Ok(n) => n,
        Err(e) => {
            return CheckResult {
                name: "lock_screen_http",
                status: CheckStatus::Fail,
                detail: format!("Lock screen HTTP server read failed on {}: {}", addr_owned, e),
            };
        }
    };

    let response = String::from_utf8_lossy(&buf[..n]);
    if response.starts_with("HTTP/1.") && response.contains("200") {
        CheckResult {
            name: "lock_screen_http",
            status: CheckStatus::Pass,
            detail: format!("Lock screen HTTP server responding on {}", addr_owned),
        }
    } else {
        CheckResult {
            name: "lock_screen_http",
            status: CheckStatus::Fail,
            detail: format!("Lock screen HTTP server on {} returned non-200: {}", addr_owned,
                response.lines().next().unwrap_or("(empty)")),
        }
    }
}

// ─── DISP-02: Lock Screen Window Rect ────────────────────────────────────────

/// Check that the Edge/Chromium lock screen window covers >= 90% of the screen.
///
/// Uses FindWindowA("Chrome_WidgetWin_1") + GetWindowRect via spawn_blocking.
/// Returns Warn (not Fail) if the window is not found — it may not be launched yet.
/// Returns Fail only if the window is found but does not cover enough of the screen.
#[cfg(windows)]
async fn check_window_rect() -> CheckResult {
    let result = spawn_blocking(|| {
        unsafe extern "system" {
            fn GetSystemMetrics(nIndex: i32) -> i32;
            fn FindWindowA(lpClassName: *const u8, lpWindowName: *const u8) -> isize;
            fn GetWindowRect(hWnd: isize, lpRect: *mut [i32; 4]) -> i32;
        }

        // Get primary screen dimensions
        let screen_w = unsafe { GetSystemMetrics(0) }; // SM_CXSCREEN
        let screen_h = unsafe { GetSystemMetrics(1) }; // SM_CYSCREEN

        // Find the Edge/Chromium window by class name
        let class_name = b"Chrome_WidgetWin_1\0";
        let hwnd = unsafe { FindWindowA(class_name.as_ptr(), std::ptr::null()) };

        if hwnd == 0 {
            return CheckResult {
                name: "lock_screen_window_rect",
                status: CheckStatus::Warn,
                detail: "Lock screen Edge window not found (may not be launched yet)".into(),
            };
        }

        // Get the window rectangle
        let mut rect = [0i32; 4]; // left, top, right, bottom
        let ok = unsafe { GetWindowRect(hwnd, &mut rect as *mut [i32; 4]) };

        if ok == 0 {
            return CheckResult {
                name: "lock_screen_window_rect",
                status: CheckStatus::Warn,
                detail: "GetWindowRect failed — window may have closed".into(),
            };
        }

        let win_w = rect[2] - rect[0]; // right - left
        let win_h = rect[3] - rect[1]; // bottom - top

        // Check if window covers at least 90% of screen dimensions
        let w_ok = screen_w > 0 && win_w as f32 >= screen_w as f32 * 0.90;
        let h_ok = screen_h > 0 && win_h as f32 >= screen_h as f32 * 0.90;

        if w_ok && h_ok {
            CheckResult {
                name: "lock_screen_window_rect",
                status: CheckStatus::Pass,
                detail: format!(
                    "Lock screen window covers full screen ({}x{} of {}x{})",
                    win_w, win_h, screen_w, screen_h
                ),
            }
        } else {
            CheckResult {
                name: "lock_screen_window_rect",
                status: CheckStatus::Fail,
                detail: format!(
                    "Lock screen window too small: {}x{} vs screen {}x{} (< 90%)",
                    win_w, win_h, screen_w, screen_h
                ),
            }
        }
    })
    .await
    .unwrap_or_else(|e| CheckResult {
        name: "lock_screen_window_rect",
        status: CheckStatus::Warn,
        detail: format!("spawn_blocking panicked in window rect check: {}", e),
    });

    result
}

#[cfg(not(windows))]
async fn check_window_rect() -> CheckResult {
    CheckResult {
        name: "lock_screen_window_rect",
        status: CheckStatus::Pass,
        detail: "Window rect check skipped (non-Windows)".into(),
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

    // ─── DISP-01 tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_lock_screen_http_fail() {
        // No server bound on a high ephemeral port — connection should be refused
        // We probe a port that should definitely not have a server running.
        // Use a port in the ephemeral range that is extremely unlikely to be in use.
        let result = check_lock_screen_http_on("127.0.0.1:19999").await;
        assert!(matches!(result.status, CheckStatus::Fail),
            "Expected Fail when no server is listening, got: {:?} ({})", result.status, result.detail);
        assert_eq!(result.name, "lock_screen_http");
    }

    #[tokio::test]
    async fn test_lock_screen_http_pass() {
        use tokio::net::TcpListener;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        // Bind an ephemeral listener that responds with HTTP 200
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let addr_str = addr.to_string();

        // Spawn a simple HTTP server that responds with 200
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 256];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(b"HTTP/1.0 200 OK\r\nContent-Length: 2\r\n\r\nOK").await;
            }
        });

        let result = check_lock_screen_http_on(&addr_str).await;
        assert!(matches!(result.status, CheckStatus::Pass),
            "Expected Pass when server is listening and responds 200, got: {:?} ({})", result.status, result.detail);
        assert_eq!(result.name, "lock_screen_http");
    }

    // ─── DISP-02 tests ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_window_rect_non_windows() {
        // On non-Windows, check_window_rect always returns Pass
        let result = check_window_rect().await;
        #[cfg(not(windows))]
        assert!(matches!(result.status, CheckStatus::Pass),
            "Expected Pass on non-Windows, got: {:?}", result.status);
        // On Windows the result depends on environment — just verify name and no panic
        assert_eq!(result.name, "lock_screen_window_rect");
    }

    // ─── Concurrent runner test ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_concurrent_checks_returns_five() {
        let mut mock = MockHidBackend::new();
        mock.expect_zero_force().returning(|| Ok(true));
        let results = run_concurrent_checks(&mock, false, false, None).await;
        assert_eq!(results.len(), 5,
            "run_concurrent_checks must return exactly 5 results (was 3, now 5 with DISP-01 + DISP-02)");
    }
}
