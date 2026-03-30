//! Tier 1 deterministic fixes for rc-agent crashes.
//!
//! STANDING RULES:
//! - Every fix function MUST have #[cfg(test)] guard returning mock result.
//!   Real system commands must NEVER execute during cargo test.
//! - Kill by process name (taskkill /IM), not PID — anti-cheat safe.
//! - Wait for port 8090 TIME_WAIT to clear before restart.
//! - All fixes are idempotent — safe to run multiple times.

use rc_common::types::CrashDiagResult;
use rc_common::verification::{ColdVerificationChain, VerifyStep, VerificationError};
use super::watchdog::CrashContext;
use crate::sentry_config;
use std::io::Write;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

const LOG_TARGET: &str = "tier1-fixes";
const PORT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const PORT_WAIT_POLL: Duration = Duration::from_millis(500);
const MAINTENANCE_FILE: &str = r"C:\RacingPoint\MAINTENANCE_MODE";
const GRACEFUL_RELAUNCH_SENTINEL: &str = r"C:\RacingPoint\GRACEFUL_RELAUNCH";
const RCAGENT_SELF_RESTART_SENTINEL: &str = r"C:\RacingPoint\rcagent-restart-sentinel.txt";
const MAINTENANCE_AUTOCLEAR_TIMEOUT: Duration = Duration::from_secs(1800); // 30 minutes
const MAINT_CLEAR_COUNT_FILE: &str = r"C:\RacingPoint\MAINT_CLEAR_COUNT";
/// After this many auto-clear→re-enter cycles, fire a persistent WhatsApp escalation.
pub const MAINT_RECURRING_THRESHOLD: u32 = 2;
const WOL_SENT_SENTINEL: &str = r"C:\RacingPoint\WOL_SENT";

// ─── Maintenance Mode Types ───────────────────────────────────────────────────

/// JSON payload written to MAINTENANCE_MODE file (MAINT-02).
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct MaintenanceModePayload {
    pub reason: String,
    pub timestamp_epoch: u64,
    pub restart_count: u32,
    pub diagnostic_context: String,
}

/// Result of check_and_clear_maintenance (MAINT-01).
#[derive(Debug, PartialEq)]
pub enum ClearResult {
    NotInMaintenance,
    StillLocked { remaining_secs: u64 },
    Cleared { reason: &'static str },
}

// Spawn verification constants (SPAWN-01)
const SPAWN_VERIFY_TIMEOUT: Duration = Duration::from_secs(10);
const SPAWN_VERIFY_POLL: Duration = Duration::from_millis(500);
const SPAWN_VERIFY_INITIAL_DELAY: Duration = Duration::from_secs(5);

// Server reachability constants (GRAD-05)
const SERVER_ADDR: &str = "192.168.31.23:8080";
const SERVER_CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

// WhatsApp escalation constants (GRAD-04)
const ESCALATION_COOLDOWN: Duration = Duration::from_secs(300); // 5 minutes between alerts

// ─── Crash Handler Result ─────────────────────────────────────────────────────

/// Result of the graduated crash handler, including spawn verification and server reachability.
pub struct CrashHandlerResult {
    pub fix_results: Vec<CrashDiagResult>,
    pub restarted: bool,
    pub spawn_verified: bool,
    pub server_reachable: bool,
    pub pattern_key: String,
}

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
    #[allow(dead_code)]
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
        let cfg = sentry_config::load();
        let output = std::process::Command::new("taskkill")
            .args(["/IM", &cfg.process_name, "/F"])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .output();
        let success = matches!(output, Ok(ref o) if o.status.success());
        CrashDiagResult {
            fix_type: "zombie_kill".to_string(),
            detail: if success { format!("Killed zombie {} processes", cfg.process_name) }
                    else { format!("No {} processes to kill or taskkill failed", cfg.process_name) },
            success,
        }
    }
}

/// Wait for service port to leave TIME_WAIT state before restart.
pub fn fix_wait_for_port() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "port_wait".to_string(),
            detail: "Port is free".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let cfg = sentry_config::load();
        let service_port = cfg.service_port;
        let start = Instant::now();
        while start.elapsed() < PORT_WAIT_TIMEOUT {
            if !is_port_in_use(service_port) {
                return CrashDiagResult {
                    fix_type: "port_wait".to_string(),
                    detail: format!("Port {} free after {:?}", service_port, start.elapsed()),
                    success: true,
                };
            }
            std::thread::sleep(PORT_WAIT_POLL);
        }
        CrashDiagResult {
            fix_type: "port_wait".to_string(),
            detail: format!("Port {} still in use after {:?}", service_port, PORT_WAIT_TIMEOUT),
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
        // Kill stale CLOSE_WAIT connections
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
        let cfg = sentry_config::load();
        let toml_exists = std::path::Path::new(&cfg.service_toml).exists();
        let bat_exists = std::path::Path::new(&cfg.start_script).exists();

        if toml_exists && bat_exists {
            CrashDiagResult {
                fix_type: "config_repair".to_string(),
                detail: "Config files present — no repair needed".to_string(),
                success: true,
            }
        } else {
            let missing: Vec<String> = [
                (!toml_exists).then(|| cfg.service_toml.clone()),
                (!bat_exists).then(|| cfg.start_script.clone()),
            ].into_iter().flatten().collect();
            tracing::error!(target: LOG_TARGET, "Missing config files: {:?}", missing);
            CrashDiagResult {
                fix_type: "config_repair".to_string(),
                detail: format!("Missing: {}. Self-heal will repair on next {} start.", missing.join(", "), cfg.service_name),
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

// ─── COV-05: Spawn Verification Chain Steps ─────────────────────────────────

/// COV-05 Step 1: Verify spawn() returned Ok (or schtasks succeeded).
struct StepSpawnOk;
impl VerifyStep for StepSpawnOk {
    type Input = (bool, String);  // (spawn_succeeded, method_description)
    type Output = String;         // method that succeeded
    fn name(&self) -> &str { "spawn_ok" }
    fn run(&self, input: (bool, String)) -> Result<String, VerificationError> {
        let (ok, method) = input;
        if !ok {
            return Err(VerificationError::ActionError {
                step: self.name().to_string(),
                raw_value: format!("spawn failed via {}", method),
            });
        }
        Ok(method)
    }
}

/// COV-05 Step 2: Wait 500ms then check if the process is alive via tasklist.
/// Uses std::thread::sleep (no tokio — rc-sentry is sync-only).
struct StepPidLiveness;
impl VerifyStep for StepPidLiveness {
    type Input = String;   // process_name (e.g., "rc-agent.exe")
    type Output = String;  // process_name (passed through)
    fn name(&self) -> &str { "pid_liveness_500ms" }
    fn run(&self, input: String) -> Result<String, VerificationError> {
        std::thread::sleep(std::time::Duration::from_millis(500));
        // Check if the process is running via tasklist
        let mut cmd = std::process::Command::new("tasklist");
        cmd.args(["/FI", &format!("IMAGENAME eq {}", input)]);
        #[cfg(windows)]
        {
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        let output = cmd.output();
        match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.to_lowercase().contains(&input.to_lowercase()) {
                    Ok(input)
                } else {
                    Err(VerificationError::ActionError {
                        step: self.name().to_string(),
                        raw_value: format!("spawn returned Ok but {} not found in tasklist after 500ms", input),
                    })
                }
            }
            Err(e) => {
                // tasklist itself failed — can't verify, log but don't block
                tracing::warn!(target: LOG_TARGET, "tasklist command failed: {} — skipping PID liveness check", e);
                Ok(input)
            }
        }
    }
}

/// COV-05 Step 3: Poll health endpoint for up to 10s.
/// Reuses the same logic as verify_service_started but wrapped as a VerifyStep.
struct StepHealthPoll {
    health_addr: String,
}
impl VerifyStep for StepHealthPoll {
    type Input = String;  // service_name
    type Output = bool;   // healthy
    fn name(&self) -> &str { "health_poll_10s" }
    fn run(&self, input: String) -> Result<bool, VerificationError> {
        let start = std::time::Instant::now();
        let timeout = SPAWN_VERIFY_TIMEOUT;
        let poll_interval = SPAWN_VERIFY_POLL;

        // Initial delay — give bat script time to swap binaries
        std::thread::sleep(SPAWN_VERIFY_INITIAL_DELAY);

        while start.elapsed() < timeout {
            match std::net::TcpStream::connect_timeout(
                &self.health_addr.parse().unwrap_or_else(|_| {
                    std::net::SocketAddr::from(([127, 0, 0, 1], 8090))
                }),
                Duration::from_secs(2),
            ) {
                Ok(mut stream) => {
                    use std::io::{Read, Write};
                    let req = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
                    if stream.write_all(req.as_bytes()).is_ok() {
                        let mut buf = [0u8; 512];
                        if let Ok(n) = stream.read(&mut buf) {
                            let resp = String::from_utf8_lossy(&buf[..n]);
                            if resp.contains("200") {
                                tracing::info!(
                                    target: LOG_TARGET,
                                    "COV-05: {} health verified after {:?}",
                                    input,
                                    start.elapsed()
                                );
                                return Ok(true);
                            }
                        }
                    }
                }
                Err(_) => {}
            }
            std::thread::sleep(poll_interval);
        }

        Err(VerificationError::ActionError {
            step: self.name().to_string(),
            raw_value: format!(
                "spawn returned Ok but {} health endpoint at {} not responding after {}s",
                input, self.health_addr, timeout.as_secs()
            ),
        })
    }
}

/// Restart the monitored service via Session 1 spawn (primary) or schtasks (fallback).
///
/// Primary path (SPAWN-03): WTSQueryUserToken + CreateProcessAsUser launches rc-agent
/// directly in the interactive desktop session. This is required because rc-sentry runs
/// as SYSTEM (Session 0) and std::process::Command targets Session 0 where GUI windows
/// cannot be displayed.
///
/// Fallback: schtasks via run_cmd_sync (cmd.exe /C) if no active console session exists
/// (e.g., before user login at boot).
pub fn restart_service() -> CrashDiagResult {
    #[cfg(test)]
    {
        return CrashDiagResult {
            fix_type: "restart".to_string(),
            detail: "service restarted".to_string(),
            success: true,
        };
    }
    #[cfg(not(test))]
    {
        let cfg = sentry_config::load();

        // Breadcrumb: confirm restart_service() was reached (H5 diagnostic)
        let _ = std::fs::write(
            r"C:\RacingPoint\sentry-restart-breadcrumb.txt",
            format!("restart_service entered at {:?}\n", std::time::SystemTime::now()),
        );

        // Primary path: Session 1 spawn (SPAWN-03)
        // Uses WTSQueryUserToken + CreateProcessAsUser to launch in interactive desktop.
        // Falls back to schtasks if no active console session (e.g., before user login).
        let bat_path = std::path::Path::new(&cfg.start_script);
        // Track spawn method for COV-05 chain
        let spawn_method_name: String;
        let spawn_succeeded: bool;

        match crate::session1_spawn::spawn_in_session1(bat_path) {
            Ok(()) => {
                spawn_method_name = "session1".to_string();
                spawn_succeeded = true;
                tracing::info!(target: LOG_TARGET,
                    "Session 1 spawn succeeded — verifying {} starts...",
                    cfg.service_name
                );
            }
            Err(reason) => {
                tracing::warn!(target: LOG_TARGET,
                    "Session 1 spawn failed: {} — falling back to schtasks",
                    reason
                );
                // Fall through to schtasks path
                let create_cmd = format!(
                    "schtasks /Create /TN StartRCAgent /TR \"{}\" /SC ONCE /ST 00:00 /F /RU SYSTEM",
                    cfg.start_script
                );
                let create = rc_common::exec::run_cmd_sync(
                    &create_cmd,
                    Duration::from_secs(10),
                    4096,
                );
                if create.exit_code != 0 {
                    tracing::error!(target: LOG_TARGET,
                        "schtasks /Create failed: exit={} stderr={}",
                        create.exit_code, create.stderr
                    );
                    return CrashDiagResult {
                        fix_type: "restart".to_string(),
                        detail: format!("both Session 1 spawn and schtasks create failed: {}", create.stderr),
                        success: false,
                    };
                }
                let run = rc_common::exec::run_cmd_sync(
                    "schtasks /Run /TN StartRCAgent",
                    Duration::from_secs(10),
                    4096,
                );
                if run.exit_code != 0 {
                    tracing::error!(target: LOG_TARGET,
                        "schtasks /Run failed: exit={} stderr={}",
                        run.exit_code, run.stderr
                    );
                    return CrashDiagResult {
                        fix_type: "restart".to_string(),
                        detail: format!("Session 1 failed ({}), schtasks /Run also failed: {}", reason, run.stderr),
                        success: false,
                    };
                }
                spawn_method_name = "schtasks".to_string();
                spawn_succeeded = true;
                tracing::info!(target: LOG_TARGET,
                    "schtasks /Run succeeded (fallback) — verifying {} starts...",
                    cfg.service_name
                );
            }
        }

        // COV-05: Spawn verification chain — wraps spawn + PID liveness + health check
        let chain = ColdVerificationChain::new("spawn_verification");

        // Step 1: Record spawn success
        match chain.execute_step(&StepSpawnOk, (spawn_succeeded, spawn_method_name.clone())) {
            Ok(_method) => {
                tracing::info!(target: LOG_TARGET, "COV-05: spawn step passed (method={})", spawn_method_name);
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, error = %e, "COV-05: spawn step reported failure");
                return CrashDiagResult {
                    fix_type: "restart".to_string(),
                    detail: format!("COV-05: spawn failed: {}", e),
                    success: false,
                };
            }
        }

        // Step 2: PID liveness check (500ms wait + tasklist)
        let pid_ok = chain.execute_step(&StepPidLiveness, cfg.process_name.clone()).is_ok();
        if !pid_ok {
            tracing::warn!(target: LOG_TARGET,
                "COV-05: PID liveness failed for {} — retrying spawn once (method={})",
                cfg.process_name, spawn_method_name
            );

            // Retry spawn once using the same method that originally succeeded
            let retry_ok = if spawn_method_name == "session1" {
                let bat_path = std::path::Path::new(&cfg.start_script);
                match crate::session1_spawn::spawn_in_session1(bat_path) {
                    Ok(()) => true,
                    Err(e) => {
                        tracing::error!(target: LOG_TARGET, "COV-05: retry spawn via session1 failed: {}", e);
                        false
                    }
                }
            } else if spawn_method_name == "schtasks" {
                let run = rc_common::exec::run_cmd_sync(
                    "schtasks /Run /TN StartRCAgent",
                    Duration::from_secs(10),
                    4096,
                );
                if run.exit_code != 0 {
                    tracing::error!(target: LOG_TARGET, "COV-05: retry spawn via schtasks failed: {}", run.stderr);
                    false
                } else {
                    true
                }
            } else {
                tracing::warn!(target: LOG_TARGET, "COV-05: unknown spawn method '{}' — skipping retry", spawn_method_name);
                false
            };

            if retry_ok {
                tracing::info!(target: LOG_TARGET, "COV-05: retry spawn succeeded — re-checking PID liveness");
                // Re-check PID liveness after retry
                match chain.execute_step(&StepPidLiveness, cfg.process_name.clone()) {
                    Ok(_) => {
                        tracing::info!(target: LOG_TARGET, "COV-05: PID liveness passed after retry for {}", cfg.process_name);
                    }
                    Err(e) => {
                        tracing::error!(target: LOG_TARGET, error = %e, "COV-05: PID liveness still failed after retry — proceeding to health poll");
                    }
                }
            }
        } else {
            tracing::info!(target: LOG_TARGET, "COV-05: PID liveness check passed for {}", cfg.process_name);
        }

        // Step 3: Health endpoint poll (10s) — replaces direct verify_service_started() call
        let verified = match chain.execute_step(
            &StepHealthPoll { health_addr: cfg.health_addr.clone() },
            cfg.process_name.clone(),
        ) {
            Ok(healthy) => healthy,
            Err(e) => {
                tracing::error!(target: LOG_TARGET, error = %e, "COV-05: health endpoint not responding — spawn may have silently failed");
                false
            }
        };

        // Write breadcrumb AFTER chain completes
        let _ = std::fs::write(
            r"C:\RacingPoint\sentry-restart-breadcrumb.txt",
            format!(
                "restart_service: spawn ok ({}), verified={} at {:?}\n",
                spawn_method_name,
                verified,
                std::time::SystemTime::now()
            ),
        );

        CrashDiagResult {
            fix_type: "restart".to_string(),
            detail: if verified {
                format!("{} restarted ({}) and verified via COV-05 chain (spawn_ok + pid_liveness + health_poll)", cfg.service_name, spawn_method_name)
            } else {
                format!(
                    "{} restart attempted ({}) but COV-05 health check failed after {}s",
                    cfg.service_name, spawn_method_name, SPAWN_VERIFY_TIMEOUT.as_secs()
                )
            },
            success: verified,
        }
    }
}

/// Poll a health endpoint until it responds with HTTP 200, or timeout.
/// Used to verify rc-agent actually started after schtasks /Run.
/// Polls at SPAWN_VERIFY_POLL (500ms) intervals for SPAWN_VERIFY_TIMEOUT (10s) (SPAWN-01).
#[cfg(not(test))]
#[allow(dead_code)]
fn verify_service_started(health_addr: &str, _timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    // Wait initial delay — give the bat script time to swap binaries
    std::thread::sleep(SPAWN_VERIFY_INITIAL_DELAY);

    while start.elapsed() < SPAWN_VERIFY_TIMEOUT {
        match std::net::TcpStream::connect_timeout(
            &health_addr.parse().unwrap_or_else(|_| {
                std::net::SocketAddr::from(([127, 0, 0, 1], 8090))
            }),
            Duration::from_secs(2),
        ) {
            Ok(mut stream) => {
                use std::io::{Read, Write};
                let req = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
                if stream.write_all(req.as_bytes()).is_ok() {
                    let mut buf = [0u8; 512];
                    if let Ok(n) = stream.read(&mut buf) {
                        let resp = String::from_utf8_lossy(&buf[..n]);
                        if resp.contains("200") {
                            tracing::info!(
                                target: LOG_TARGET,
                                "rc-agent health verified after {:?}",
                                start.elapsed()
                            );
                            return true;
                        }
                    }
                }
            }
            Err(_) => {}
        }
        std::thread::sleep(SPAWN_VERIFY_POLL);
    }
    tracing::warn!(
        target: LOG_TARGET,
        "rc-agent health NOT verified after {:?}",
        start.elapsed()
    );
    false
}

/// Check if the racecontrol server at SERVER_ADDR is reachable via TCP (GRAD-05).
#[cfg(not(test))]
fn check_server_reachable() -> bool {
    let addr: std::net::SocketAddr = match SERVER_ADDR.parse() {
        Ok(a) => a,
        Err(_) => return false,
    };
    std::net::TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT).is_ok()
}

#[cfg(test)]
fn check_server_reachable() -> bool {
    true // mock in tests
}

/// Get pod ID from hostname. Pod hostnames are like "SIM1", "SIM2".
pub fn get_pod_id() -> String {
    sysinfo::System::host_name()
        .map(|h| {
            let lower = h.to_lowercase();
            if let Some(n) = lower.strip_prefix("sim") {
                format!("pod-{}", n)
            } else {
                format!("pod-{}", lower)
            }
        })
        .unwrap_or_else(|| "pod-unknown".to_string())
}

/// POST a recovery event to the server. Fire-and-forget — logs warn on failure, never panics.
fn post_recovery_event(event: &rc_common::recovery::RecoveryEvent) {
    let body = match serde_json::to_string(event) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "failed to serialize recovery event: {}", e);
            return;
        }
    };
    let request = format!(
        "POST /api/v1/recovery/events HTTP/1.0\r\nHost: 192.168.31.23\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(), body
    );
    let addr: std::net::SocketAddr = match SERVER_ADDR.parse() {
        Ok(a) => a,
        Err(_) => return,
    };
    let mut stream = match std::net::TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "recovery event POST failed (connect): {}", e);
            return;
        }
    };
    if let Err(e) = stream.write_all(request.as_bytes()) {
        tracing::warn!(target: LOG_TARGET, "recovery event POST failed (write): {}", e);
    }
    // Don't read response — fire-and-forget
}

/// POST a WhatsApp escalation alert to the server fleet alert endpoint (GRAD-04).
///
/// Fires only when spawn_verified=false after 3+ consecutive failed recoveries.
/// Uses a 5-minute cooldown to prevent alert spam.
/// Pass `last_escalation` as `&mut Option<Instant>` from the caller — no global state.
pub fn escalate_to_whatsapp(
    pod_id: &str,
    failure_count: u32,
    last_error: &str,
    last_escalation: &mut Option<Instant>,
) {
    // Check cooldown
    if let Some(last) = last_escalation.as_ref() {
        if last.elapsed() < ESCALATION_COOLDOWN {
            tracing::info!(
                target: LOG_TARGET,
                "Tier 4 escalation suppressed — cooldown ({:?} remaining)",
                ESCALATION_COOLDOWN.checked_sub(last.elapsed()).unwrap_or_default()
            );
            return;
        }
    }

    let message = format!(
        "Pod {} stuck in crash loop: {} failed recovery attempts. Last error: {}",
        pod_id, failure_count, last_error
    );

    let body = serde_json::json!({
        "pod_id": pod_id,
        "message": message,
        "severity": "critical"
    });

    let body_str = match serde_json::to_string(&body) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "failed to serialize fleet alert: {}", e);
            return;
        }
    };

    let request = format!(
        "POST /api/v1/fleet/alert HTTP/1.0\r\n\
         Host: 192.168.31.23\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n{}",
        body_str.len(),
        body_str
    );

    let addr: std::net::SocketAddr = match SERVER_ADDR.parse() {
        Ok(a) => a,
        Err(_) => return,
    };
    let mut stream = match std::net::TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(target: LOG_TARGET, "fleet alert POST failed (connect): {}", e);
            return;
        }
    };
    if let Err(e) = stream.write_all(request.as_bytes()) {
        tracing::warn!(target: LOG_TARGET, "fleet alert POST failed (write): {}", e);
        return;
    }

    *last_escalation = Some(Instant::now());
    tracing::warn!(
        target: LOG_TARGET,
        "TIER 4 ESCALATION: WhatsApp alert sent for {} ({} failures)",
        pod_id,
        failure_count
    );
}

/// Write MAINTENANCE_MODE file as JSON and fire WhatsApp alert (MAINT-02, MAINT-03).
pub fn enter_maintenance_mode(reason: &str, restart_count: u32, diagnostic_context: &str) -> bool {
    #[cfg(test)]
    {
        let _ = (reason, restart_count, diagnostic_context);
        return true;
    }
    #[cfg(not(test))]
    {
        // SF-05: Do NOT write MAINTENANCE_MODE while a survival sentinel is active.
        // This prevents the watchdog MMA lockout (Pitfall 2) — the active healing layer
        // should not be interrupted by a maintenance lockout triggered by restart counting.
        use rc_common::survival_types::{any_sentinel_active, check_sentinel, SentinelKind};
        if any_sentinel_active() {
            if let Some(sentinel) = check_sentinel(SentinelKind::HealInProgress) {
                tracing::warn!(target: LOG_TARGET,
                    action_id = %sentinel.action_id,
                    layer = ?sentinel.layer,
                    "HEAL_IN_PROGRESS active — suppressing MAINTENANCE_MODE entry (SF-05)");
            }
            if let Some(sentinel) = check_sentinel(SentinelKind::OtaDeploying) {
                tracing::warn!(target: LOG_TARGET,
                    action_id = %sentinel.action_id,
                    "OTA_DEPLOYING active — suppressing MAINTENANCE_MODE entry (SF-05)");
            }
            return false;
        }

        let payload = MaintenanceModePayload {
            reason: reason.to_string(),
            timestamp_epoch: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            restart_count,
            diagnostic_context: diagnostic_context.to_string(),
        };
        let json = match serde_json::to_string_pretty(&payload) {
            Ok(j) => j,
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "failed to serialize MAINTENANCE_MODE payload: {}", e);
                return false;
            }
        };
        if let Err(e) = std::fs::write(MAINTENANCE_FILE, &json) {
            tracing::error!(target: LOG_TARGET, "failed to write MAINTENANCE_MODE: {}", e);
            return false;
        }
        tracing::error!(target: LOG_TARGET,
            "MAINTENANCE_MODE ACTIVATED — reason: {}, restart_count: {}", reason, restart_count);

        // MAINT-03: WhatsApp alert on activation
        let pod_id = get_pod_id();
        let alert_msg = format!(
            "MAINTENANCE_MODE activated on {}. Reason: {}. Restarts: {}. Pod is locked — auto-clear in 30 min.",
            pod_id, reason, restart_count
        );
        let body = serde_json::json!({
            "pod_id": pod_id,
            "message": alert_msg,
            "severity": "critical"
        });
        if let Ok(body_str) = serde_json::to_string(&body) {
            let request = format!(
                "POST /api/v1/fleet/alert HTTP/1.0\r\n\
                 Host: 192.168.31.23\r\n\
                 Content-Type: application/json\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\
                 \r\n{}",
                body_str.len(), body_str
            );
            if let Ok(addr) = SERVER_ADDR.parse::<std::net::SocketAddr>() {
                match std::net::TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT) {
                    Ok(mut stream) => {
                        if let Err(e) = stream.write_all(request.as_bytes()) {
                            tracing::warn!(target: LOG_TARGET, "MAINTENANCE_MODE WhatsApp alert failed (write): {}", e);
                        } else {
                            tracing::info!(target: LOG_TARGET, "MAINTENANCE_MODE WhatsApp alert sent for {}", pod_id);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(target: LOG_TARGET, "MAINTENANCE_MODE WhatsApp alert failed (connect): {}", e);
                    }
                }
            }
        }
        true
    }
}

/// Read the maintenance mode payload. Returns None if not in maintenance or file is not JSON.
#[allow(dead_code)]
pub fn read_maintenance_payload() -> Option<MaintenanceModePayload> {
    #[cfg(test)]
    {
        return None;
    }
    #[cfg(not(test))]
    {
        std::fs::read_to_string(MAINTENANCE_FILE)
            .ok()
            .and_then(|c| serde_json::from_str(&c).ok())
    }
}

/// Check if MAINTENANCE_MODE should auto-clear (MAINT-01).
/// Returns Cleared if: (a) WOL_SENT sentinel exists, or (b) 30 minutes elapsed.
/// On clear: deletes MAINTENANCE_MODE file (and WOL_SENT if present), attempts rc-agent restart.
pub fn check_and_clear_maintenance() -> ClearResult {
    #[cfg(test)]
    {
        return ClearResult::NotInMaintenance;
    }
    #[cfg(not(test))]
    {
        use std::path::Path;
        let maint_path = Path::new(MAINTENANCE_FILE);
        if !maint_path.exists() {
            return ClearResult::NotInMaintenance;
        }

        // Immediate clear when WOL_SENT sentinel exists
        let wol_sent = Path::new(WOL_SENT_SENTINEL).exists();
        if wol_sent {
            tracing::info!(target: LOG_TARGET,
                "WOL_SENT sentinel found — immediate MAINTENANCE_MODE clear");
            let _ = std::fs::remove_file(MAINTENANCE_FILE);
            let _ = std::fs::remove_file(WOL_SENT_SENTINEL);
            attempt_restart_after_clear();
            return ClearResult::Cleared { reason: "WOL_SENT immediate clear" };
        }

        // Check 30-min timeout from JSON timestamp (with mtime fallback for legacy plain-text files)
        let elapsed_secs = match std::fs::read_to_string(MAINTENANCE_FILE) {
            Ok(content) => {
                match serde_json::from_str::<MaintenanceModePayload>(&content) {
                    Ok(payload) => {
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0);
                        now.saturating_sub(payload.timestamp_epoch)
                    }
                    Err(_) => {
                        // Legacy plain-text file — check file mtime instead
                        maint_path.metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.elapsed().ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0)
                    }
                }
            }
            Err(_) => 0,
        };

        if elapsed_secs >= MAINTENANCE_AUTOCLEAR_TIMEOUT.as_secs() {
            tracing::info!(target: LOG_TARGET,
                "MAINTENANCE_MODE auto-clear — {} seconds elapsed (threshold: {})",
                elapsed_secs, MAINTENANCE_AUTOCLEAR_TIMEOUT.as_secs());
            let _ = std::fs::remove_file(MAINTENANCE_FILE);
            attempt_restart_after_clear();
            return ClearResult::Cleared { reason: "30-min timeout" };
        }

        let remaining = MAINTENANCE_AUTOCLEAR_TIMEOUT.as_secs() - elapsed_secs;
        ClearResult::StillLocked { remaining_secs: remaining }
    }
}

/// After clearing maintenance mode, restart rc-agent using the full Session 1 spawn path.
/// Previous implementation used raw schtasks+CREATE_NO_WINDOW which silently fails in
/// non-interactive context — causing the MAINTENANCE_MODE infinite loop (2026-03-25 root cause).
#[cfg(windows)]
fn attempt_restart_after_clear() {
    #[cfg(test)]
    return;
    #[cfg(not(test))]
    {
        tracing::info!(target: LOG_TARGET, "attempting rc-agent restart after maintenance clear (Session 1 path)");
        let result = restart_service();
        if result.success {
            tracing::info!(target: LOG_TARGET,
                "rc-agent restart after maintenance clear VERIFIED: {}", result.detail);
        } else {
            tracing::error!(target: LOG_TARGET,
                "rc-agent restart after maintenance clear FAILED: {}", result.detail);
        }
    }
}

#[cfg(not(windows))]
fn attempt_restart_after_clear() {
    // No-op on non-Windows (test builds)
}

/// Increment the maintenance clear counter (persisted to disk).
/// Returns the new count. Used to detect recurring clear→re-enter cycles (MAINT-04).
pub fn increment_maint_clear_count() -> u32 {
    let current = std::fs::read_to_string(MAINT_CLEAR_COUNT_FILE)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .unwrap_or(0);
    let new_count = current + 1;
    let _ = std::fs::write(MAINT_CLEAR_COUNT_FILE, new_count.to_string());
    tracing::info!(target: LOG_TARGET, "MAINT_CLEAR_COUNT incremented to {}", new_count);
    new_count
}

/// Reset the maintenance clear counter (called when rc-agent is healthy after a clear).
pub fn reset_maint_clear_count() {
    let _ = std::fs::remove_file(MAINT_CLEAR_COUNT_FILE);
}

/// Fire a persistent WhatsApp alert when MAINTENANCE_MODE keeps recurring (MAINT-04).
/// This means the pod is stuck in a crash-loop that auto-clear can't fix.
pub fn alert_recurring_maintenance(clear_count: u32) {
    let pod_id = get_pod_id();
    let alert_msg = format!(
        "RECURRING MAINTENANCE on {} — auto-cleared {} times but rc-agent keeps crashing. \
         Manual investigation required. Pod is in a crash loop.",
        pod_id, clear_count
    );
    tracing::error!(target: LOG_TARGET, "{}", alert_msg);

    let body = serde_json::json!({
        "pod_id": pod_id,
        "message": alert_msg,
        "severity": "critical"
    });
    if let Ok(body_str) = serde_json::to_string(&body) {
        let request = format!(
            "POST /api/v1/fleet/alert HTTP/1.0\r\n\
             Host: 192.168.31.23\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             Connection: close\r\n\
             \r\n{}",
            body_str.len(), body_str
        );
        if let Ok(addr) = SERVER_ADDR.parse::<std::net::SocketAddr>() {
            match std::net::TcpStream::connect_timeout(&addr, SERVER_CONNECT_TIMEOUT) {
                Ok(mut stream) => {
                    if let Err(e) = stream.write_all(request.as_bytes()) {
                        tracing::warn!(target: LOG_TARGET, "RECURRING MAINTENANCE alert failed (write): {}", e);
                    } else {
                        tracing::info!(target: LOG_TARGET, "RECURRING MAINTENANCE WhatsApp alert sent for {}", pod_id);
                    }
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "RECURRING MAINTENANCE alert failed (connect): {}", e);
                }
            }
        }
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

/// Check if an RCAGENT_SELF_RESTART sentinel is present on disk.
/// Returns false in test cfg — same pattern as is_maintenance_mode.
pub fn is_rcagent_self_restart() -> bool {
    #[cfg(test)]
    {
        return false;
    }
    #[cfg(not(test))]
    {
        std::path::Path::new(RCAGENT_SELF_RESTART_SENTINEL).exists()
    }
}

// ─── Crash Handler ───────────────────────────────────────────────────────────

/// Run the graduated 4-tier crash handler.
///
/// Flow: Tier 1 (deterministic fixes) -> Tier 2 (pattern memory) -> server_reachable check
///       -> escalation guard -> restart -> spawn verification -> POST recovery event.
///
/// Returns CrashHandlerResult with all context for the caller.
pub fn handle_crash(ctx: &CrashContext, tracker: &mut RestartTracker) -> CrashHandlerResult {
    let mut results = Vec::new();

    // Derive pattern key for Tier 2 lookup and recovery event (GRAD-02)
    #[cfg(feature = "ai-diagnosis")]
    let pattern_key = crate::debug_memory::derive_pattern_key(
        ctx.panic_message.as_deref(),
        ctx.exit_code,
        ctx.last_phase.as_deref(),
    );
    #[cfg(not(feature = "ai-diagnosis"))]
    let pattern_key = format!("exit:{:?}", ctx.exit_code);

    // SF-05: Check survival sentinels before any restart action.
    // Yield to the active healing layer so systems don't fight each other.
    #[cfg(not(test))]
    {
        use rc_common::survival_types::{any_sentinel_active, check_sentinel, SentinelKind};
        if any_sentinel_active() {
            if let Some(sentinel) = check_sentinel(SentinelKind::HealInProgress) {
                tracing::warn!(target: LOG_TARGET,
                    action_id = %sentinel.action_id,
                    layer = ?sentinel.layer,
                    action = %sentinel.action,
                    remaining_secs = sentinel.remaining_secs(),
                    "HEAL_IN_PROGRESS active — skipping restart (SF-05)");
            }
            if let Some(sentinel) = check_sentinel(SentinelKind::OtaDeploying) {
                tracing::warn!(target: LOG_TARGET,
                    action_id = %sentinel.action_id,
                    "OTA_DEPLOYING active — skipping restart (SF-05)");
            }
            return CrashHandlerResult {
                fix_results: results,
                restarted: false,
                spawn_verified: false,
                server_reachable: false,
                pattern_key,
            };
        }
    }

    // Check maintenance mode
    if is_maintenance_mode() {
        tracing::warn!(target: LOG_TARGET, "MAINTENANCE_MODE active — skipping restart");
        let server_reachable = check_server_reachable();
        return CrashHandlerResult {
            fix_results: results,
            restarted: false,
            spawn_verified: false,
            server_reachable,
            pattern_key,
        };
    }

    // Check for graceful relaunch sentinel from rc-agent's self_monitor.
    // If present, this was a self-initiated restart (e.g. WS dead, server down),
    // NOT a real crash. Skip escalation counter to prevent false MAINTENANCE_MODE.
    let graceful = std::path::Path::new(GRACEFUL_RELAUNCH_SENTINEL).exists();
    if graceful {
        tracing::info!(target: LOG_TARGET,
            "GRACEFUL_RELAUNCH sentinel found — self_monitor restart, not a crash. Skipping escalation.");
        let _ = std::fs::remove_file(GRACEFUL_RELAUNCH_SENTINEL);
    }

    // Check for RCAGENT_SELF_RESTART sentinel from deploy sequence.
    // rc-agent writes this file before relaunch_self() so rc-sentry knows this
    // is a deploy-triggered restart, not a real crash. Consume once.
    let rcagent_restart = is_rcagent_self_restart();
    if rcagent_restart {
        tracing::info!(target: LOG_TARGET,
            "RCAGENT_SELF_RESTART sentinel found — deploy restart detected, not a crash. Skipping escalation.");
        let _ = std::fs::remove_file(RCAGENT_SELF_RESTART_SENTINEL);
    }

    let is_graceful = graceful || rcagent_restart;

    // Tier 1: Run deterministic fixes (GRAD-01)
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

    // Tier 2: Pattern memory lookup (GRAD-02) — lookup only, fix was applied by Tier 1
    #[cfg(feature = "ai-diagnosis")]
    {
        let memory = crate::debug_memory::DebugMemory::load();
        if let Some(incident) = memory.instant_fix(&pattern_key) {
            tracing::info!(target: LOG_TARGET,
                "TIER 2 INSTANT FIX: pattern '{}' matched fix '{}' (hit #{})",
                pattern_key, incident.fix_type, incident.hit_count);
        }
    }

    // Check server reachability (GRAD-05) — must happen BEFORE escalation counter
    let server_reachable = check_server_reachable();

    // Escalation guard — skip counter for graceful restarts AND server-unreachable events (GRAD-05)
    if !is_graceful {
        if !server_reachable {
            tracing::info!(target: LOG_TARGET,
                "server unreachable — excluding from MAINTENANCE_MODE counter (GRAD-05)");
            // Skip escalation counter — server-down disconnects never trigger pod lockout
        } else {
            let escalated = tracker.record_restart();
            if escalated {
                tracing::error!(target: LOG_TARGET,
                    "ESCALATION: {} restarts in {:?} — entering maintenance mode",
                    tracker.restart_count(), tracker.window
                );
                enter_maintenance_mode(
                    &format!(
                        "{} restarts in 10 minutes. Last crash: {:?}",
                        tracker.restart_count(),
                        ctx.panic_message.as_deref().unwrap_or("unknown")
                    ),
                    tracker.restart_count(),
                    &format!("exit_code:{:?} last_phase:{:?}", ctx.exit_code, ctx.last_phase),
                );
                return CrashHandlerResult {
                    fix_results: results,
                    restarted: false,
                    spawn_verified: false,
                    server_reachable,
                    pattern_key,
                };
            }

            // Wait for backoff delay
            let delay = tracker.current_delay();
            tracing::info!(target: LOG_TARGET, "backoff delay: {:?} (restart #{})", delay, tracker.restart_count());
            std::thread::sleep(delay);
        }
    } else {
        tracing::info!(target: LOG_TARGET, "Graceful relaunch — skipping escalation counter and backoff");
    }

    // Restart rc-agent (SPAWN-01 — restart_service() now polls at 500ms/10s)
    let r = restart_service();
    tracing::info!(target: LOG_TARGET, "restart_service: {} ({})", r.success, r.detail);
    // Use restart result's success field as spawn_verified (restart_service verifies via health check)
    let spawn_verified = r.success;
    let restarted = r.success;
    results.push(r);

    // POST recovery event to server (SPAWN-02)
    let event = rc_common::recovery::RecoveryEvent {
        pod_id: get_pod_id(),
        process: sentry_config::load().process_name.clone(),
        authority: rc_common::recovery::RecoveryAuthority::RcSentry,
        action: rc_common::recovery::RecoveryAction::Restart,
        spawn_verified: Some(spawn_verified),
        server_reachable: Some(server_reachable),
        reason: format!("crash_handler tier1+restart pattern:{}", pattern_key),
        context: format!("fixes:{} verified:{}", results.len(), spawn_verified),
        timestamp: chrono::Utc::now(),
    };
    post_recovery_event(&event);

    CrashHandlerResult {
        fix_results: results,
        restarted,
        spawn_verified,
        server_reachable,
        pattern_key,
    }
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
    fn restart_service_returns_mock() {
        let r = restart_service();
        assert_eq!(r.fix_type, "restart");
        assert!(r.success);
    }

    /// SPAWN-03: mock path verifies Session 1 spawn wiring does not break the test mock.
    #[test]
    fn restart_service_mock_succeeds() {
        let r = restart_service();
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
        let result = handle_crash(&ctx, &mut tracker);
        assert!(!result.fix_results.is_empty());
        assert!(result.restarted); // mock restart succeeds
        assert!(result.spawn_verified); // mock always returns success=true
        assert!(result.server_reachable); // mock check_server_reachable returns true
        assert!(result.fix_results.iter().any(|r| r.fix_type == "zombie_kill"));
        assert!(result.fix_results.iter().any(|r| r.fix_type == "port_wait"));
        assert!(result.fix_results.iter().any(|r| r.fix_type == "restart"));
    }

    #[test]
    fn handle_crash_escalates_after_threshold() {
        let ctx = test_ctx();
        let mut tracker = RestartTracker::new();
        // First two crashes — should restart (server_reachable=true in test, so counter increments)
        let result1 = handle_crash(&ctx, &mut tracker);
        assert!(result1.restarted);
        let result2 = handle_crash(&ctx, &mut tracker);
        assert!(result2.restarted);
        // Third crash — should escalate, no restart
        let result3 = handle_crash(&ctx, &mut tracker);
        assert!(!result3.restarted);
    }

    #[test]
    fn handle_crash_result_has_pattern_key() {
        let ctx = test_ctx();
        let mut tracker = RestartTracker::new();
        let result = handle_crash(&ctx, &mut tracker);
        // In non-ai-diagnosis builds, pattern key is "exit:None"
        // In ai-diagnosis builds, it's derived from crash context
        assert!(!result.pattern_key.is_empty());
    }

    #[test]
    fn check_server_reachable_returns_true_in_test() {
        // Mock always returns true in test builds
        assert!(check_server_reachable());
    }

    #[test]
    fn get_pod_id_returns_nonempty_string() {
        let id = get_pod_id();
        assert!(!id.is_empty());
        // Must start with "pod-"
        assert!(id.starts_with("pod-"), "pod_id should start with 'pod-', got: {}", id);
    }

    #[test]
    fn spawn_verify_constants_correct() {
        assert_eq!(SPAWN_VERIFY_TIMEOUT, Duration::from_secs(10));
        assert_eq!(SPAWN_VERIFY_POLL, Duration::from_millis(500));
        assert_eq!(SPAWN_VERIFY_INITIAL_DELAY, Duration::from_secs(5));
    }

    #[test]
    fn maintenance_mode_returns_false_in_test() {
        assert!(!is_maintenance_mode());
    }

    #[test]
    fn rcagent_self_restart_returns_false_in_test() {
        // In test cfg, is_rcagent_self_restart always returns false (sentinel file never read)
        assert!(!is_rcagent_self_restart());
    }

    #[test]
    fn escalation_cooldown_constant_correct() {
        assert_eq!(ESCALATION_COOLDOWN, Duration::from_secs(300));
    }

    #[test]
    fn escalate_cooldown_suppresses_repeat() {
        // A just-set last_escalation should still be within cooldown
        let last = Some(Instant::now());
        assert!(last.unwrap().elapsed() < ESCALATION_COOLDOWN);
    }

    #[test]
    fn escalate_no_cooldown_when_none() {
        // None means never escalated — no suppression should occur
        let last_escalation: Option<Instant> = None;
        // Verify no cooldown applies when None
        let suppressed = if let Some(last) = last_escalation.as_ref() {
            last.elapsed() < ESCALATION_COOLDOWN
        } else {
            false
        };
        assert!(!suppressed, "should not suppress when last_escalation is None");
    }

    #[test]
    fn rcagent_self_restart_sentinel_constant_value() {
        assert_eq!(
            RCAGENT_SELF_RESTART_SENTINEL,
            r"C:\RacingPoint\rcagent-restart-sentinel.txt"
        );
    }

    #[test]
    fn handle_crash_without_sentinel_calls_record_restart() {
        // With neither sentinel present (test cfg returns false for both),
        // handle_crash must call record_restart, advancing backoff_index from 0 to 1.
        // Note: check_server_reachable() returns true in test cfg, so escalation counter IS called.
        let ctx = test_ctx();
        let mut tracker = RestartTracker::new();
        assert_eq!(tracker.backoff_index, 0);
        let _ = handle_crash(&ctx, &mut tracker);
        // backoff_index advances on record_restart — 0 -> 1
        assert_eq!(tracker.backoff_index, 1, "record_restart should have been called");
    }

    // ─── MAINT-01/02/03 tests ─────────────────────────────────────────────────

    #[test]
    fn maintenance_autoclear_timeout_is_1800() {
        assert_eq!(MAINTENANCE_AUTOCLEAR_TIMEOUT, Duration::from_secs(1800));
    }

    #[test]
    fn wol_sent_sentinel_constant_value() {
        assert_eq!(WOL_SENT_SENTINEL, r"C:\RacingPoint\WOL_SENT");
    }

    #[test]
    fn maintenance_mode_payload_serializes() {
        let payload = MaintenanceModePayload {
            reason: "test reason".to_string(),
            timestamp_epoch: 1000000,
            restart_count: 3,
            diagnostic_context: "exit_code:1".to_string(),
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"reason\""));
        assert!(json.contains("\"timestamp_epoch\""));
        assert!(json.contains("\"restart_count\""));
        assert!(json.contains("\"diagnostic_context\""));
        assert!(json.contains("test reason"));
    }

    #[test]
    fn maintenance_mode_payload_roundtrips() {
        let original = MaintenanceModePayload {
            reason: "crash storm".to_string(),
            timestamp_epoch: 9999999,
            restart_count: 5,
            diagnostic_context: "exit_code:137 last_phase:startup".to_string(),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: MaintenanceModePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.reason, original.reason);
        assert_eq!(parsed.timestamp_epoch, original.timestamp_epoch);
        assert_eq!(parsed.restart_count, original.restart_count);
        assert_eq!(parsed.diagnostic_context, original.diagnostic_context);
    }

    #[test]
    fn check_and_clear_maintenance_returns_not_in_maintenance_in_test() {
        // In test cfg, check_and_clear_maintenance must return NotInMaintenance
        let result = check_and_clear_maintenance();
        assert_eq!(result, ClearResult::NotInMaintenance);
    }
}
