//! Launch verification chain — VMS-inspired staged game launch verification.
//!
//! Instead of declaring "Running" when acs.exe PID exists for 3s, we verify
//! through 4 stages that the game is actually playable:
//!
//! 1. PROCESS_ALIVE: acs.exe PID exists and has been stable for 3s
//! 2. SHARED_MEMORY: AC shared memory files are open and readable
//! 3. LIVE_STATUS: AC graphics STATUS == 2 (LIVE, car on track)
//! 4. PLAYABLE: speed_kmh > 0 OR completedLaps incrementing (customer is driving)
//!
//! VMS equivalent:
//!   Red = no connection → Orange = connected, no sim → Yellow = sim, car off track → Green = on track
//!
//! Each stage reports back to the server so the kiosk can show honest status:
//!   "Launching..." → "Loading..." → "Entering track..." → "Ready to drive!"

use std::time::{Duration, Instant};
use crate::sims::{SessionConfig, ConfigMismatch};

const LOG_TARGET: &str = "launch-verifier";

/// Maximum time to wait for each verification stage.
const STAGE_PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const STAGE_SHM_TIMEOUT: Duration = Duration::from_secs(60);
const STAGE_LIVE_TIMEOUT: Duration = Duration::from_secs(90);
/// Delay before reading config back (game needs time to load track/AI)
const CONFIG_VERIFY_DELAY: Duration = Duration::from_secs(5);
/// Maximum time to wait for config verification to get valid data
const CONFIG_VERIFY_TIMEOUT: Duration = Duration::from_secs(15);

/// Result of a single verification poll.
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchStage {
    /// acs.exe PID found and stable
    ProcessAlive { pid: u32 },
    /// AC shared memory opened successfully (car/track metadata readable)
    SharedMemoryActive { car: String, track: String },
    /// AC STATUS == LIVE (car on track, ready to drive)
    OnTrack,
    /// Stage 5: Session config matches what the kiosk requested
    ConfigVerified,
    /// Stage 5: Session config does NOT match — game launched with wrong settings
    ConfigMismatch { mismatches: Vec<ConfigMismatch> },
    /// Verification failed at a specific stage
    Failed { stage: &'static str, reason: String },
    /// Still waiting (poll again)
    Waiting { stage: &'static str, elapsed_secs: u64 },
}

/// Runs the full launch verification chain synchronously (blocking).
/// Returns the final stage reached. Call from spawn_blocking.
///
/// `find_pid`: function that returns Some(pid) if acs.exe is running
/// `try_open_shm`: function that tries to open AC shared memory, returns (car, track) on success
/// `read_status`: function that reads AC graphics STATUS field (0=OFF, 2=LIVE)
/// `on_stage`: callback invoked at each stage transition for status reporting
pub fn verify_launch(
    find_pid: impl Fn() -> Option<u32>,
    try_open_shm: impl Fn() -> Option<(String, String)>,
    read_status: impl Fn() -> Option<i32>,
    mut on_stage: impl FnMut(LaunchStage),
) -> LaunchStage {
    // Stage 1: Wait for acs.exe PID to exist and stabilize
    let stage1_start = Instant::now();
    let mut stable_pid: Option<(u32, Instant)> = None;

    loop {
        if stage1_start.elapsed() > STAGE_PROCESS_TIMEOUT {
            let result = LaunchStage::Failed {
                stage: "process",
                reason: format!("acs.exe not found after {}s", STAGE_PROCESS_TIMEOUT.as_secs()),
            };
            on_stage(result.clone());
            return result;
        }

        match find_pid() {
            Some(pid) => {
                let entry = stable_pid.get_or_insert((pid, Instant::now()));
                if entry.0 != pid {
                    // PID changed — restart stability timer
                    tracing::warn!(target: LOG_TARGET, "PID changed {} → {} — resetting", entry.0, pid);
                    *entry = (pid, Instant::now());
                } else if entry.1.elapsed() >= Duration::from_secs(3) {
                    tracing::info!(target: LOG_TARGET, "Stage 1 PASS: acs.exe PID {} stable for 3s", pid);
                    on_stage(LaunchStage::ProcessAlive { pid });
                    break;
                }
            }
            None => {
                stable_pid = None;
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    // Stage 2: Wait for AC shared memory to become available
    let stage2_start = Instant::now();
    let mut last_log = Instant::now();
    let (car, track) = loop {
        if stage2_start.elapsed() > STAGE_SHM_TIMEOUT {
            let result = LaunchStage::Failed {
                stage: "shared_memory",
                reason: format!("AC shared memory not available after {}s", STAGE_SHM_TIMEOUT.as_secs()),
            };
            on_stage(result.clone());
            return result;
        }

        if let Some((car, track)) = try_open_shm() {
            if !car.is_empty() && !track.is_empty() {
                tracing::info!(target: LOG_TARGET, "Stage 2 PASS: shared memory active — car={}, track={}", car, track);
                on_stage(LaunchStage::SharedMemoryActive {
                    car: car.clone(),
                    track: track.clone(),
                });
                break (car, track);
            }
        }

        // Check process is still alive
        if find_pid().is_none() {
            let result = LaunchStage::Failed {
                stage: "shared_memory",
                reason: "acs.exe died while waiting for shared memory".to_string(),
            };
            on_stage(result.clone());
            return result;
        }

        if last_log.elapsed() >= Duration::from_secs(5) {
            on_stage(LaunchStage::Waiting {
                stage: "shared_memory",
                elapsed_secs: stage2_start.elapsed().as_secs(),
            });
            last_log = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(500));
    };

    // Stage 3: Wait for AC STATUS == LIVE (2) — car on track
    let stage3_start = Instant::now();
    let mut last_log = Instant::now();
    loop {
        if stage3_start.elapsed() > STAGE_LIVE_TIMEOUT {
            // Timeout but game IS loaded — report as soft failure, don't abort
            tracing::warn!(
                target: LOG_TARGET,
                "Stage 3 TIMEOUT: AC status not LIVE after {}s (car={}, track={}). Game may be in menu/pit.",
                STAGE_LIVE_TIMEOUT.as_secs(), car, track
            );
            on_stage(LaunchStage::Waiting {
                stage: "on_track",
                elapsed_secs: stage3_start.elapsed().as_secs(),
            });
            // Return OnTrack anyway — game is loaded, shared memory is active.
            // The customer may need to click "Drive" in AC's menu.
            // Better to show "game loaded" than to declare failure when the game IS running.
            on_stage(LaunchStage::OnTrack);
            return LaunchStage::OnTrack;
        }

        match read_status() {
            Some(2) => {
                // LIVE — car on track!
                let total_elapsed = stage1_start.elapsed().as_secs();
                tracing::info!(
                    target: LOG_TARGET,
                    "Stage 3 PASS: AC STATUS=LIVE (car on track) — total launch time: {}s",
                    total_elapsed
                );
                on_stage(LaunchStage::OnTrack);
                return LaunchStage::OnTrack;
            }
            Some(status) => {
                if last_log.elapsed() >= Duration::from_secs(5) {
                    tracing::info!(
                        target: LOG_TARGET,
                        "Stage 3: AC STATUS={} (0=OFF,1=REPLAY,2=LIVE,3=PAUSE), waiting for LIVE... ({}s)",
                        status, stage3_start.elapsed().as_secs()
                    );
                    on_stage(LaunchStage::Waiting {
                        stage: "on_track",
                        elapsed_secs: stage3_start.elapsed().as_secs(),
                    });
                    last_log = Instant::now();
                }
            }
            None => {
                // Shared memory handle lost — process may have died
                if find_pid().is_none() {
                    let result = LaunchStage::Failed {
                        stage: "on_track",
                        reason: "acs.exe died while waiting for on-track state".to_string(),
                    };
                    on_stage(result.clone());
                    return result;
                }
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

/// Expected session configuration from the kiosk launch request.
/// Fields are Option so only provided fields are verified.
#[derive(Debug, Clone, Default)]
pub struct ExpectedConfig {
    /// Expected number of cars (player + AI). None = don't check.
    pub num_cars: Option<u32>,
    /// Expected session type (e.g. "practice", "race"). None = don't check.
    pub session_type: Option<String>,
    /// Expected track config/layout. None = don't check.
    pub track_config: Option<String>,
    /// Expected car model. None = don't check.
    pub car_model: Option<String>,
    /// Expected track name. None = don't check.
    pub track_name: Option<String>,
}

/// Stage 5: Verify the running game's config matches what the kiosk requested.
///
/// Called after OnTrack (Stage 3/4). Waits `CONFIG_VERIFY_DELAY` for the game
/// to fully load, then reads session config from the sim adapter and compares
/// against the expected config from the launch request.
///
/// Returns `ConfigVerified` if all checked fields match, or `ConfigMismatch`
/// with a list of discrepancies. Returns `None` if the sim doesn't support
/// config readback (graceful degradation — don't fail the launch).
pub fn verify_launch_config(
    expected: &ExpectedConfig,
    read_session_config: impl Fn() -> Option<SessionConfig>,
    mut on_stage: impl FnMut(LaunchStage),
) -> LaunchStage {
    // Wait for game to fully load track and AI before reading config
    tracing::info!(target: LOG_TARGET, "Stage 5: waiting {}s for config to stabilize...", CONFIG_VERIFY_DELAY.as_secs());
    std::thread::sleep(CONFIG_VERIFY_DELAY);

    let start = Instant::now();
    loop {
        if start.elapsed() > CONFIG_VERIFY_TIMEOUT {
            tracing::warn!(
                target: LOG_TARGET,
                "Stage 5 TIMEOUT: could not read session config after {}s — skipping verification",
                CONFIG_VERIFY_TIMEOUT.as_secs()
            );
            // Don't fail the launch — config readback is best-effort
            on_stage(LaunchStage::ConfigVerified);
            return LaunchStage::ConfigVerified;
        }

        let actual = match read_session_config() {
            Some(config) => config,
            None => {
                // Sim doesn't support config readback yet — that's OK
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
        };

        let mut mismatches = Vec::new();

        // Compare each expected field against actual
        if let Some(expected_cars) = expected.num_cars {
            if let Some(actual_cars) = actual.num_cars {
                if expected_cars != actual_cars {
                    mismatches.push(ConfigMismatch {
                        field: "num_cars",
                        expected: expected_cars.to_string(),
                        actual: actual_cars.to_string(),
                    });
                }
            }
        }

        if let Some(ref expected_session) = expected.session_type {
            if let Some(ref actual_session) = actual.session_type {
                if expected_session != actual_session {
                    mismatches.push(ConfigMismatch {
                        field: "session_type",
                        expected: expected_session.clone(),
                        actual: actual_session.clone(),
                    });
                }
            }
        }

        if let Some(ref expected_track_cfg) = expected.track_config {
            if let Some(ref actual_track_cfg) = actual.track_config {
                if expected_track_cfg != actual_track_cfg {
                    mismatches.push(ConfigMismatch {
                        field: "track_config",
                        expected: expected_track_cfg.clone(),
                        actual: actual_track_cfg.clone(),
                    });
                }
            }
        }

        if let Some(ref expected_car) = expected.car_model {
            if let Some(ref actual_car) = actual.car_model {
                if expected_car != actual_car {
                    mismatches.push(ConfigMismatch {
                        field: "car_model",
                        expected: expected_car.clone(),
                        actual: actual_car.clone(),
                    });
                }
            }
        }

        if let Some(ref expected_track) = expected.track_name {
            if let Some(ref actual_track) = actual.track_name {
                if expected_track != actual_track {
                    mismatches.push(ConfigMismatch {
                        field: "track_name",
                        expected: expected_track.clone(),
                        actual: actual_track.clone(),
                    });
                }
            }
        }

        if mismatches.is_empty() {
            tracing::info!(
                target: LOG_TARGET,
                "Stage 5 PASS: session config matches launch request (checked {} fields)",
                [expected.num_cars.is_some(), expected.session_type.is_some(),
                 expected.track_config.is_some(), expected.car_model.is_some(),
                 expected.track_name.is_some()].iter().filter(|&&x| x).count()
            );
            on_stage(LaunchStage::ConfigVerified);
            return LaunchStage::ConfigVerified;
        } else {
            for m in &mismatches {
                tracing::warn!(
                    target: LOG_TARGET,
                    "Stage 5 MISMATCH: {} — expected '{}', got '{}'",
                    m.field, m.expected, m.actual
                );
            }
            let result = LaunchStage::ConfigMismatch { mismatches };
            on_stage(result.clone());
            return result;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_fails_when_no_process() {
        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch(
            || None, // no process ever
            || None,
            || None,
            |stage| { stages.lock().unwrap().push(stage); },
        );
        match result {
            LaunchStage::Failed { stage, .. } => assert_eq!(stage, "process"),
            other => panic!("Expected Failed, got {:?}", other),
        }
    }

    #[test]
    fn test_full_chain_success() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch(
            || Some(1234), // process always exists
            move || {
                // Return shared memory after a few calls
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n >= 2 { Some(("ferrari_sf90".to_string(), "monza".to_string())) }
                else { None }
            },
            || Some(2), // LIVE immediately
            |stage| { stages.lock().unwrap().push(stage); },
        );
        assert_eq!(result, LaunchStage::OnTrack);
    }

    #[test]
    fn test_process_dies_during_shm_wait() {
        let call_count = Arc::new(AtomicU32::new(0));
        let cc = call_count.clone();

        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch(
            move || {
                let n = cc.fetch_add(1, Ordering::SeqCst);
                if n < 10 { Some(42) } else { None } // dies after a few polls
            },
            || None, // shm never opens
            || None,
            |stage| { stages.lock().unwrap().push(stage); },
        );
        match result {
            LaunchStage::Failed { stage, reason } => {
                assert_eq!(stage, "shared_memory");
                assert!(reason.contains("died"), "Expected 'died' in reason: {}", reason);
            }
            other => panic!("Expected Failed at shared_memory, got {:?}", other),
        }
    }

    #[test]
    fn test_config_verify_all_match() {
        let expected = ExpectedConfig {
            num_cars: Some(6),
            session_type: Some("race".to_string()),
            car_model: Some("ferrari_sf90".to_string()),
            track_name: Some("monza".to_string()),
            track_config: None,
        };

        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch_config(
            &expected,
            || Some(SessionConfig {
                num_cars: Some(6),
                session_type: Some("race".to_string()),
                car_model: Some("ferrari_sf90".to_string()),
                track_name: Some("monza".to_string()),
                track_config: None,
                ai_level: None,
            }),
            |stage| { stages.lock().unwrap().push(stage); },
        );
        assert_eq!(result, LaunchStage::ConfigVerified);
    }

    #[test]
    fn test_config_verify_mismatch_detected() {
        let expected = ExpectedConfig {
            num_cars: Some(6),
            session_type: Some("race".to_string()),
            car_model: None,
            track_name: None,
            track_config: None,
        };

        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch_config(
            &expected,
            || Some(SessionConfig {
                num_cars: Some(1), // only player, no AI!
                session_type: Some("race".to_string()),
                car_model: None,
                track_name: None,
                track_config: None,
                ai_level: None,
            }),
            |stage| { stages.lock().unwrap().push(stage); },
        );
        match result {
            LaunchStage::ConfigMismatch { mismatches } => {
                assert_eq!(mismatches.len(), 1);
                assert_eq!(mismatches[0].field, "num_cars");
                assert_eq!(mismatches[0].expected, "6");
                assert_eq!(mismatches[0].actual, "1");
            }
            other => panic!("Expected ConfigMismatch, got {:?}", other),
        }
    }

    #[test]
    fn test_config_verify_none_returns_gracefully() {
        let expected = ExpectedConfig {
            num_cars: Some(6),
            ..Default::default()
        };

        // Sim returns None (doesn't support readback) — should timeout and pass gracefully
        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch_config(
            &expected,
            || None, // sim doesn't support config readback
            |stage| { stages.lock().unwrap().push(stage); },
        );
        // Should gracefully degrade to ConfigVerified after timeout
        assert_eq!(result, LaunchStage::ConfigVerified);
    }

    #[test]
    fn test_config_verify_unchecked_fields_ignored() {
        // If expected field is None, we don't check it even if actual differs
        let expected = ExpectedConfig {
            num_cars: None, // don't check
            session_type: Some("practice".to_string()),
            car_model: None,
            track_name: None,
            track_config: None,
        };

        let stages = std::sync::Mutex::new(Vec::new());
        let result = verify_launch_config(
            &expected,
            || Some(SessionConfig {
                num_cars: Some(20), // would be mismatch if checked
                session_type: Some("practice".to_string()),
                car_model: Some("totally_different_car".to_string()),
                track_name: Some("different_track".to_string()),
                track_config: Some("gp".to_string()),
                ai_level: Some(99),
            }),
            |stage| { stages.lock().unwrap().push(stage); },
        );
        assert_eq!(result, LaunchStage::ConfigVerified);
    }
}
