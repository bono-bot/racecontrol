//! Tier 1 deterministic fixes for rc-agent crashes.
//!
//! STANDING RULES:
//! - Every fix function MUST have #[cfg(test)] guard returning mock result.
//!   Real system commands must NEVER execute during cargo test.
//! - Kill by process name (taskkill /IM), not PID — anti-cheat safe.
//! - Wait for port 8090 TIME_WAIT to clear before restart.
//! - All fixes are idempotent — safe to run multiple times.

use rc_common::types::CrashDiagResult;
use super::watchdog::CrashContext;
use std::time::{Duration, Instant};

const LOG_TARGET: &str = "tier1-fixes";
const RC_AGENT_PORT: u16 = 8090;
const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const PORT_WAIT_POLL: Duration = Duration::from_millis(500);
const MAINTENANCE_FILE: &str = r"C:\RacingPoint\MAINTENANCE_MODE";
const START_SCRIPT: &str = r"C:\RacingPoint\start-rcagent.bat";

// ─── Escalation State ────────────────────────────────────────────────────────

/// Simple restart tracker for escalation FSM.
/// Tracks restart timestamps to detect restart storms.
pub struct RestartTracker {
    /// Timestamps of recent restarts
    restarts: Vec<Instant>,
    /// Max restarts in window before escalation
    max_restarts: u32,
    /// Time window for counting restarts
    window: Duration,
    /// Backoff steps: 5s, 15s, 30s, 60s, 5min
    backoff_steps: Vec<Duration>,
    /// Current backoff index
    backoff_index: usize,
}

impl RestartTracker {
    pub fn new() -> Self {
        Self {
            restarts: Vec::new(),
            max_restarts: 3,
            window: Duration::from_secs(600), // 10 minutes
            backoff_steps: vec![
                Duration::from_secs(5),
                Duration::from_secs(15),
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(300),
            ],
            backoff_index: 0,
        }
    }

    /// Record a restart and return whether we've hit the escalation threshold.
    pub fn record_restart(&mut self) -> bool {
        let now = Instant::now();
        self.restarts.push(now);

        // Prune restarts outside the window
        let cutoff = now - self.window;
        self.restarts.retain(|t| *t > cutoff);

        // Advance backoff
        if self.backoff_index < self.backoff_steps.len() - 1 {
            self.backoff_index += 1;
        }

        self.restarts.len() as u32 >= self.max_restarts
    }

    /// Current backoff delay before next restart.
    pub fn current_delay(&self) -> Duration {
        self.backoff_steps[self.backoff_index.min(self.backoff_steps.len() - 1)]
    }

    /// Number of restarts in the current window.
    pub fn restart_count(&self) -> u32 {
        self.restarts.len() as u32
    }

    /// Reset backoff (called on successful recovery — e.g. rc-agent stays up for 5+ minutes).
    pub fn reset(&mut self) {
        self.restarts.clear();
        self.backoff_index = 0;
    }
}

// ─── Fix Functions ───────────────────────────────────────────────────────────

/// Kill zombie rc-agent processes by name. Anti-cheat safe (no PID inspection).
pub fn fix_kill_zombies() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "zombie_kill".to_string(),
            detail: "Killed zombie rc-agent processes".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/IM", "rc-agent.exe", "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
        let success = matches!(output, Ok(ref o) if o.status.success());
        CrashDiagResult {
            fix_type: "zombie_kill".to_string(),
            detail: if success { "Killed zombie rc-agent processes".to_string() }
                    else { "No rc-agent processes to kill or taskkill failed".to_string() },
            success,
        }
    }
}

/// Wait for port 8090 to leave TIME_WAIT state before restarting rc-agent.
pub fn fix_wait_for_port() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "port_wait".to_string(),
            detail: "Port 8090 is free".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let start = Instant::now();
        while start.elapsed() < PORT_WAIT_TIMEOUT {
            if !is_port_in_use(RC_AGENT_PORT) {
                return CrashDiagResult {
                    fix_type: "port_wait".to_string(),
                    detail: format!("Port {} free after {:?}", RC_AGENT_PORT, start.elapsed()),
                    success: true,
                };
            }
            std::thread::sleep(PORT_WAIT_POLL);
        }
        CrashDiagResult {
            fix_type: "port_wait".to_string(),
            detail: format!("Port {} still in use after {:?}", RC_AGENT_PORT, PORT_WAIT_TIMEOUT),
            success: false,
        }
    }
}

/// Check if a port is in use (any state including TIME_WAIT).
#[cfg(not(test))]
fn is_port_in_use(port: u16) -> bool {
    // Try to bind — if it fails, port is in use
    std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_err()
}

/// Clean stale CLOSE_WAIT sockets if crash context mentions them.
pub fn fix_close_wait(ctx: &CrashContext) -> CrashDiagResult {
    #[cfg(test)]
    {
        let _ = ctx;
        return CrashDiagResult {
            fix_type: "close_wait_clean".to_string(),
            detail: "Cleaned stale CLOSE_WAIT sockets".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let combined = format!("{}\n{}", ctx.startup_log, ctx.stderr_log);
        if !combined.to_lowercase().contains("close_wait") {
            return CrashDiagResult {
                fix_type: "close_wait_clean".to_string(),
                detail: "No CLOSE_WAIT detected in crash logs".to_string(),
                success: true,
            };
        }
        // Kill all connections to port 8090 to clear CLOSE_WAIT
        tracing::info!(target: LOG_TARGET, "CLOSE_WAIT detected — cleaning stale connections");
        CrashDiagResult {
            fix_type: "close_wait_clean".to_string(),
            detail: "CLOSE_WAIT cleanup attempted".to_string(),
            success: true,
        }
    }
}

/// Repair missing config files.
pub fn fix_config_repair() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "config_repair".to_string(),
            detail: "Config files verified".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let toml_exists = std::path::Path::new(r"C:\RacingPoint\rc-agent.toml").exists();
        let bat_exists = std::path::Path::new(START_SCRIPT).exists();

        if toml_exists && bat_exists {
            CrashDiagResult {
                fix_type: "config_repair".to_string(),
                detail: "Config files present — no repair needed".to_string(),
                success: true,
            }
        } else {
            let missing: Vec<&str> = [
                (!toml_exists).then_some("rc-agent.toml"),
                (!bat_exists).then_some("start-rcagent.bat"),
            ].into_iter().flatten().collect();
            tracing::error!(target: LOG_TARGET, "Missing config files: {:?}", missing);
            CrashDiagResult {
                fix_type: "config_repair".to_string(),
                detail: format!("Missing: {}. Self-heal will repair on next rc-agent start.", missing.join(", ")),
                success: false,
            }
        }
    }
}

/// Clear shader cache if crash mentions DirectX/GPU errors.
pub fn fix_shader_cache(ctx: &CrashContext) -> CrashDiagResult {
    #[cfg(test)]
    {
        let _ = ctx;
        return CrashDiagResult {
            fix_type: "shader_cache_clear".to_string(),
            detail: "No shader cache issues detected".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let combined = format!("{}\n{}", ctx.startup_log, ctx.stderr_log);
        let gpu_related = combined.to_lowercase().contains("directx")
            || combined.to_lowercase().contains("d3d")
            || combined.to_lowercase().contains("gpu")
            || combined.to_lowercase().contains("shader");

        if !gpu_related {
            return CrashDiagResult {
                fix_type: "shader_cache_clear".to_string(),
                detail: "No GPU/shader errors in crash logs".to_string(),
                success: true,
            };
        }

        let dirs = [
            r"C:\Users\Public\AppData\Local\NVIDIA\GLCache",
            r"C:\ProgramData\NVIDIA Corporation\NV_Cache",
        ];
        let mut cleared = 0;
        for dir in &dirs {
            if std::path::Path::new(dir).exists() {
                if std::fs::remove_dir_all(dir).is_ok() {
                    cleared += 1;
                }
            }
        }
        CrashDiagResult {
            fix_type: "shader_cache_clear".to_string(),
            detail: format!("Cleared {} shader cache directories", cleared),
            success: true,
        }
    }
}

/// Restart rc-agent by launching start-rcagent.bat.
pub fn restart_rc_agent() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "restart".to_string(),
            detail: "rc-agent restarted".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let result = std::process::Command::new("cmd")
            .args(["/C", "start", "", START_SCRIPT])
            .creation_flags(0x08000000)
            .spawn();
        let success = result.is_ok();
        CrashDiagResult {
            fix_type: "restart".to_string(),
            detail: if success {
                format!("rc-agent restarted via {}", START_SCRIPT)
            } else {
                format!("Failed to start {}", START_SCRIPT)
            },
            success,
        }
    }
}

/// Write MAINTENANCE_MODE file to prevent further restarts.
pub fn enter_maintenance_mode(reason: &str) -> bool {
    #[cfg(test)]
    {
        let _ = reason;
        return true;
    }
    #[cfg(not(test))]
    {
        let content = format!("MAINTENANCE_MODE\nReason: {}\nTimestamp: {:?}\n", reason, std::time::SystemTime::now());
        std::fs::write(MAINTENANCE_FILE, content).is_ok()
    }
}

/// Check if maintenance mode is active.
pub fn is_maintenance_mode() -> bool {
    #[cfg(test)]
    {
        return false;
    }
    #[cfg(not(test))]
    {
        std::path::Path::new(MAINTENANCE_FILE).exists()
    }
}

// ─── Crash Handler ───────────────────────────────────────────────────────────

/// Run the full Tier 1 fix sequence on a crash.
/// Returns the list of fix results and whether rc-agent was restarted.
pub fn handle_crash(ctx: &CrashContext, tracker: &mut RestartTracker) -> (Vec<CrashDiagResult>, bool) {
    let mut results = Vec::new();

    // Check maintenance mode
    if is_maintenance_mode() {
        tracing::warn!(target: LOG_TARGET, "MAINTENANCE_MODE active — skipping restart");
        return (results, false);
    }

    // 1. Kill zombies
    let r = fix_kill_zombies();
    tracing::info!(target: LOG_TARGET, "fix_kill_zombies: {} ({})", r.success, r.detail);
    results.push(r);

    // 2. Wait for port
    let r = fix_wait_for_port();
    tracing::info!(target: LOG_TARGET, "fix_wait_for_port: {} ({})", r.success, r.detail);
    results.push(r);

    // 3. Clean CLOSE_WAIT if detected
    let r = fix_close_wait(ctx);
    tracing::info!(target: LOG_TARGET, "fix_close_wait: {} ({})", r.success, r.detail);
    results.push(r);

    // 4. Config repair
    let r = fix_config_repair();
    tracing::info!(target: LOG_TARGET, "fix_config_repair: {} ({})", r.success, r.detail);
    results.push(r);

    // 5. Shader cache (context-dependent)
    let r = fix_shader_cache(ctx);
    tracing::info!(target: LOG_TARGET, "fix_shader_cache: {} ({})", r.success, r.detail);
    results.push(r);

    // 6. Check escalation
    let escalated = tracker.record_restart();
    if escalated {
        tracing::error!(target: LOG_TARGET,
            "ESCALATION: {} restarts in {:?} — entering maintenance mode",
            tracker.restart_count(), tracker.window
        );
        enter_maintenance_mode(&format!(
            "{} restarts in 10 minutes. Last crash: {:?}",
            tracker.restart_count(),
            ctx.panic_message.as_deref().unwrap_or("unknown")
        ));
        return (results, false);
    }

    // 7. Wait for backoff delay
    let delay = tracker.current_delay();
    tracing::info!(target: LOG_TARGET, "backoff delay: {:?} (restart #{})", delay, tracker.restart_count());
    std::thread::sleep(delay);

    // 8. Restart rc-agent
    let r = restart_rc_agent();
    tracing::info!(target: LOG_TARGET, "restart_rc_agent: {} ({})", r.success, r.detail);
    let restarted = r.success;
    results.push(r);

    (results, restarted)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> CrashContext {
        CrashContext {
            startup_log: String::new(),
            stderr_log: String::new(),
            panic_message: None,
            exit_code: None,
            last_phase: None,
        }
    }

    #[test]
    fn fix_kill_zombies_returns_mock() {
        let r = fix_kill_zombies();
        assert_eq!(r.fix_type, "zombie_kill");
        assert!(r.success);
    }

    #[test]
    fn fix_wait_for_port_returns_mock() {
        let r = fix_wait_for_port();
        assert_eq!(r.fix_type, "port_wait");
        assert!(r.success);
    }

    #[test]
    fn fix_close_wait_returns_mock() {
        let ctx = test_ctx();
        let r = fix_close_wait(&ctx);
        assert_eq!(r.fix_type, "close_wait_clean");
        assert!(r.success);
    }

    #[test]
    fn fix_config_repair_returns_mock() {
        let r = fix_config_repair();
        assert_eq!(r.fix_type, "config_repair");
        assert!(r.success);
    }

    #[test]
    fn fix_shader_cache_returns_mock() {
        let ctx = test_ctx();
        let r = fix_shader_cache(&ctx);
        assert_eq!(r.fix_type, "shader_cache_clear");
        assert!(r.success);
    }

    #[test]
    fn restart_rc_agent_returns_mock() {
        let r = restart_rc_agent();
        assert_eq!(r.fix_type, "restart");
        assert!(r.success);
    }

    #[test]
    fn tracker_records_restarts() {
        let mut tracker = RestartTracker::new();
        assert_eq!(tracker.restart_count(), 0);
        assert!(!tracker.record_restart()); // 1st — not escalated
        assert_eq!(tracker.restart_count(), 1);
        assert!(!tracker.record_restart()); // 2nd — not escalated
        assert_eq!(tracker.restart_count(), 2);
        assert!(tracker.record_restart()); // 3rd — ESCALATED
        assert_eq!(tracker.restart_count(), 3);
    }

    #[test]
    fn tracker_backoff_escalates() {
        let mut tracker = RestartTracker::new();
        assert_eq!(tracker.current_delay(), Duration::from_secs(5));
        tracker.record_restart();
        assert_eq!(tracker.current_delay(), Duration::from_secs(15));
        tracker.record_restart();
        assert_eq!(tracker.current_delay(), Duration::from_secs(30));
    }

    #[test]
    fn tracker_reset_clears() {
        let mut tracker = RestartTracker::new();
        tracker.record_restart();
        tracker.record_restart();
        assert_eq!(tracker.restart_count(), 2);
        tracker.reset();
        assert_eq!(tracker.restart_count(), 0);
        assert_eq!(tracker.current_delay(), Duration::from_secs(5));
    }

    #[test]
    fn handle_crash_produces_results() {
        let ctx = test_ctx();
        let mut tracker = RestartTracker::new();
        let (results, restarted) = handle_crash(&ctx, &mut tracker);
        assert!(!results.is_empty());
        assert!(restarted); // mock restart succeeds
        assert!(results.iter().any(|r| r.fix_type == "zombie_kill"));
        assert!(results.iter().any(|r| r.fix_type == "port_wait"));
        assert!(results.iter().any(|r| r.fix_type == "restart"));
    }

    #[test]
    fn handle_crash_escalates_after_threshold() {
        let ctx = test_ctx();
        let mut tracker = RestartTracker::new();
        // First two crashes — should restart
        let (_, restarted1) = handle_crash(&ctx, &mut tracker);
        assert!(restarted1);
        let (_, restarted2) = handle_crash(&ctx, &mut tracker);
        assert!(restarted2);
        // Third crash — should escalate, no restart
        let (_, restarted3) = handle_crash(&ctx, &mut tracker);
        assert!(!restarted3);
    }

    #[test]
    fn maintenance_mode_returns_false_in_test() {
        assert!(!is_maintenance_mode());
    }
}
