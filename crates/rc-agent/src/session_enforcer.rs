use std::time::Instant;

use rc_common::types::SimType;

const LOG_TARGET: &str = "session-enforcer";

/// Action returned by SessionEnforcer::tick().
#[derive(Debug, PartialEq)]
pub enum SessionAction {
    Continue,
    Warn { remaining_secs: u64 },
    Terminate,
}

/// Status returned by ProcessMonitor::check().
#[derive(Debug, PartialEq)]
pub enum ProcessStatus {
    Running,
    Exited { exit_code: Option<i32> },
}

/// Session time enforcer for open-world games (ForzaHorizon5, Forza Motorsport).
///
/// These games have no session concept — without enforcement, customers can play
/// past their paid duration. SessionEnforcer tracks elapsed time and signals
/// the caller to warn at T-60s and force-terminate at T+0.
///
/// GAME-03: Force-terminate Forza sessions at duration expiry with 1-minute warning.
pub struct SessionEnforcer {
    pub(crate) sim_type: SimType,
    pub(crate) pid: u32,
    /// Wall-clock start time.
    pub(crate) started_at: Instant,
    /// Paid duration in seconds.
    pub(crate) duration_secs: u64,
    /// Whether the T-60s warning has already been issued (one-time).
    pub(crate) warned: bool,
}

impl SessionEnforcer {
    /// Create a new enforcer for an open-world game session.
    pub fn new(sim_type: SimType, pid: u32, duration_secs: u64) -> Self {
        Self {
            sim_type,
            pid,
            started_at: Instant::now(),
            duration_secs,
            warned: false,
        }
    }

    /// Create a new enforcer with a synthetic start time (for testing).
    #[cfg(test)]
    pub(crate) fn new_with_start(
        sim_type: SimType,
        pid: u32,
        duration_secs: u64,
        started_at: Instant,
    ) -> Self {
        Self {
            sim_type,
            pid,
            started_at,
            duration_secs,
            warned: false,
        }
    }

    /// Poll the enforcer. Must be called approximately every second.
    ///
    /// Returns:
    /// - `Continue`          — no action needed
    /// - `Warn { remaining_secs }` — T-60s reached (emitted once)
    /// - `Terminate`         — duration expired, kill the process
    pub fn tick(&mut self) -> SessionAction {
        let elapsed = self.started_at.elapsed().as_secs();

        if elapsed >= self.duration_secs {
            return SessionAction::Terminate;
        }

        let remaining = self.duration_secs.saturating_sub(elapsed);

        // One-time warning at T-60s (when there are 60 or fewer seconds left)
        if !self.warned && remaining <= 60 {
            self.warned = true;
            tracing::info!(
                target: LOG_TARGET,
                sim_type = ?self.sim_type,
                pid = self.pid,
                remaining_secs = remaining,
                "Session expiry warning — {}s remaining (GAME-03)",
                remaining
            );
            return SessionAction::Warn { remaining_secs: remaining };
        }

        SessionAction::Continue
    }

    /// Force-terminate the game process via taskkill /F /PID.
    ///
    /// Uses CREATE_NO_WINDOW on Windows to prevent a console flash.
    /// This is a best-effort kill — callers should clear game state regardless.
    pub fn terminate(pid: u32) -> Result<(), String> {
        tracing::warn!(
            target: LOG_TARGET,
            pid,
            "SessionEnforcer: force-terminating game (GAME-03)"
        );
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            let status = std::process::Command::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .status()
                .map_err(|e| format!("taskkill spawn failed: {}", e))?;
            if !status.success() {
                return Err(format!("taskkill exited with: {}", status));
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let status = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status()
                .map_err(|e| format!("kill spawn failed: {}", e))?;
            if !status.success() {
                return Err(format!("kill exited with: {}", status));
            }
        }
        Ok(())
    }
}

/// Generic crash detector for non-AC games (GAME-08).
///
/// AC uses shared memory (AcStatus::Off) for crash detection. All other sims
/// (F1 25, iRacing, LMU, Forza, FH5, AC EVO, AC Rally) need process-exit monitoring.
///
/// ProcessMonitor polls `is_process_alive(pid)` and reports Exited when the
/// process disappears outside of a controlled StopGame flow.
pub struct ProcessMonitor {
    pub(crate) pid: u32,
    pub(crate) sim_type: SimType,
}

impl ProcessMonitor {
    /// Create a new process monitor for a launched non-AC game.
    pub fn new(pid: u32, sim_type: SimType) -> Self {
        Self { pid, sim_type }
    }

    /// Check whether the monitored process is still alive.
    ///
    /// Returns `Running` or `Exited { exit_code }`.
    /// Exit code extraction uses Windows `GetExitCodeProcess` when available.
    pub fn check(&self) -> ProcessStatus {
        if is_process_alive(self.pid) {
            ProcessStatus::Running
        } else {
            let exit_code = get_exit_code(self.pid);
            ProcessStatus::Exited { exit_code }
        }
    }
}

/// Platform-specific process alive check.
/// Re-uses the same logic as game_process::is_process_alive but exposed here
/// so session_enforcer is self-contained and testable without the game_process module.
#[cfg(target_os = "windows")]
fn is_process_alive(pid: u32) -> bool {
    unsafe {
        let handle = winapi::um::processthreadsapi::OpenProcess(
            winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let result = winapi::um::processthreadsapi::GetExitCodeProcess(
            handle,
            &mut exit_code as *mut u32,
        );
        winapi::um::handleapi::CloseHandle(handle);
        result != 0 && exit_code == 259 // STILL_ACTIVE
    }
}

#[cfg(not(target_os = "windows"))]
fn is_process_alive(pid: u32) -> bool {
    std::path::Path::new(&format!("/proc/{}", pid)).exists()
}

/// Extract the exit code of a process that has already exited.
/// On Windows, uses GetExitCodeProcess. On Linux, not easily available
/// for non-child processes — returns None.
#[cfg(target_os = "windows")]
fn get_exit_code(pid: u32) -> Option<i32> {
    unsafe {
        let handle = winapi::um::processthreadsapi::OpenProcess(
            winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        );
        if handle.is_null() {
            return None;
        }
        let mut exit_code: u32 = 0;
        let result = winapi::um::processthreadsapi::GetExitCodeProcess(
            handle,
            &mut exit_code as *mut u32,
        );
        winapi::um::handleapi::CloseHandle(handle);
        if result != 0 && exit_code != 259 {
            // 259 = STILL_ACTIVE — if we see something else after is_process_alive=false, it's a real exit code
            Some(exit_code as i32)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn get_exit_code(_pid: u32) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── SessionEnforcer tick() timing tests ──────────────────────────────────

    /// At T=0s (just started, 120s duration), tick returns Continue.
    #[test]
    fn test_tick_continue_at_start() {
        let mut enforcer = SessionEnforcer::new(SimType::ForzaHorizon5, 1234, 120);
        assert_eq!(enforcer.tick(), SessionAction::Continue);
    }

    /// At T=59s (61s remaining), 120s duration — still Continue.
    #[test]
    fn test_tick_continue_before_warning_window() {
        let past = Instant::now() - Duration::from_secs(59);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        assert_eq!(enforcer.tick(), SessionAction::Continue);
    }

    /// At T=60s (60s remaining), 120s duration — first tick returns Warn.
    #[test]
    fn test_tick_warn_at_t_minus_60() {
        let past = Instant::now() - Duration::from_secs(60);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        let action = enforcer.tick();
        assert!(
            matches!(action, SessionAction::Warn { remaining_secs: r } if r <= 60),
            "Expected Warn at T-60, got {:?}",
            action
        );
    }

    /// Warning is only emitted ONCE — second tick at same time returns Continue.
    #[test]
    fn test_tick_warn_is_one_time() {
        let past = Instant::now() - Duration::from_secs(60);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        let first = enforcer.tick();
        assert!(matches!(first, SessionAction::Warn { .. }), "First tick should warn");
        let second = enforcer.tick();
        assert_eq!(second, SessionAction::Continue, "Second tick should Continue after warning");
    }

    /// At T=120s (0s remaining), tick returns Terminate.
    #[test]
    fn test_tick_terminate_at_expiry() {
        let past = Instant::now() - Duration::from_secs(120);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        assert_eq!(enforcer.tick(), SessionAction::Terminate);
    }

    /// Terminate is returned on every tick after expiry (not just once).
    #[test]
    fn test_tick_terminate_repeatedly() {
        let past = Instant::now() - Duration::from_secs(200);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        assert_eq!(enforcer.tick(), SessionAction::Terminate);
        assert_eq!(enforcer.tick(), SessionAction::Terminate);
    }

    /// Forza Motorsport also gets session enforcement.
    #[test]
    fn test_tick_terminate_forza_motorsport() {
        let past = Instant::now() - Duration::from_secs(3600);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::Forza, 9999, 3600, past);
        assert_eq!(enforcer.tick(), SessionAction::Terminate);
    }

    /// Warn uses the actual remaining_secs value (not hardcoded 60).
    #[test]
    fn test_warn_remaining_secs_accurate() {
        // At T=70s, 120s duration → 50s remaining → Warn(50)
        let past = Instant::now() - Duration::from_secs(70);
        let mut enforcer = SessionEnforcer::new_with_start(SimType::ForzaHorizon5, 1234, 120, past);
        let action = enforcer.tick();
        match action {
            SessionAction::Warn { remaining_secs } => {
                // Allow ±2s for Instant imprecision during test execution
                assert!(
                    remaining_secs <= 52 && remaining_secs >= 48,
                    "Expected ~50s remaining, got {}",
                    remaining_secs
                );
            }
            other => panic!("Expected Warn, got {:?}", other),
        }
    }

    // ── ProcessMonitor check() tests ─────────────────────────────────────────

    /// ProcessMonitor::check() returns Exited for a PID that doesn't exist.
    #[test]
    fn test_process_monitor_nonexistent_pid() {
        // PID u32::MAX is extremely unlikely to be a real process.
        let monitor = ProcessMonitor::new(u32::MAX, SimType::ForzaHorizon5);
        let status = monitor.check();
        assert!(
            matches!(status, ProcessStatus::Exited { .. }),
            "PID u32::MAX should not be alive, got {:?}",
            status
        );
    }

    /// ProcessMonitor::check() returns Running for the current process.
    #[test]
    fn test_process_monitor_current_process_is_running() {
        let my_pid = std::process::id();
        let monitor = ProcessMonitor::new(my_pid, SimType::ForzaHorizon5);
        assert_eq!(monitor.check(), ProcessStatus::Running);
    }

    /// ProcessMonitor is constructed with correct sim_type.
    #[test]
    fn test_process_monitor_stores_sim_type() {
        let monitor = ProcessMonitor::new(1234, SimType::F125);
        assert_eq!(monitor.sim_type, SimType::F125);
    }

    // ── Enum structural tests ─────────────────────────────────────────────────

    /// SessionAction variants are distinct and comparable.
    #[test]
    fn test_session_action_equality() {
        assert_eq!(SessionAction::Continue, SessionAction::Continue);
        assert_eq!(SessionAction::Terminate, SessionAction::Terminate);
        assert_eq!(
            SessionAction::Warn { remaining_secs: 30 },
            SessionAction::Warn { remaining_secs: 30 }
        );
        assert_ne!(
            SessionAction::Warn { remaining_secs: 30 },
            SessionAction::Warn { remaining_secs: 45 }
        );
    }

    /// ProcessStatus variants are distinct and comparable.
    #[test]
    fn test_process_status_equality() {
        assert_eq!(ProcessStatus::Running, ProcessStatus::Running);
        assert_eq!(
            ProcessStatus::Exited { exit_code: Some(0) },
            ProcessStatus::Exited { exit_code: Some(0) }
        );
        assert_ne!(
            ProcessStatus::Running,
            ProcessStatus::Exited { exit_code: None }
        );
    }
}
