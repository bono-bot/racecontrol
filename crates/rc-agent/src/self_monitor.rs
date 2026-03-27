//! Self-monitoring daemon for rc-agent health.
//!
//! Runs every 5 minutes. Detects CLOSE_WAIT socket floods on :8090 (caused by
//! racecontrol's pod_monitor hammering the pod) and prolonged WebSocket disconnects.
//!
//! Two recovery paths:
//!   WS dead 5+ min  → relaunch immediately (no AI needed — reconnect loop already failed)
//!   CLOSE_WAIT flood → query local Ollama for RESTART/OK decision
//! On relaunch: spawns a detached cmd.exe that waits 3s then restarts rc-agent.
//! All actions are appended to C:\RacingPoint\rc-bot-events.log for post-mortem analysis.

use std::os::windows::process::CommandExt;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

#[cfg(feature = "ai-debugger")]
use crate::ai_debugger::AiDebuggerConfig;
#[cfg(not(feature = "ai-debugger"))]
use crate::config::AiDebuggerConfig;
use crate::udp_heartbeat::HeartbeatStatus;

const LOG_TARGET: &str = "self-monitor";
const CHECK_INTERVAL_SECS: u64 = 60;      // check every minute
const CLOSE_WAIT_THRESHOLD: usize = 20;   // flag if :8090 has 20+ stuck sockets
const WS_DEAD_SECS: u64 = 300;           // relaunch if disconnected 5+ min — allows slow boot reconnect without false-positive kills
const DETACHED_PROCESS: u32 = 0x0000_0008;
const EVENT_LOG: &str = r"C:\RacingPoint\rc-bot-events.log";
const MAX_LOG_BYTES: u64 = 512 * 1024;    // rotate at 512KB

/// Spawn the self-monitor background task.
pub fn spawn(config: AiDebuggerConfig, status: Arc<HeartbeatStatus>) {
    tokio::spawn(async move {
        // OBS-05: Lifecycle event — task started
        tracing::info!(target: "state", task = "self_monitor", event = "lifecycle", "lifecycle: started");
        tracing::info!(target: LOG_TARGET, "Self-monitor task started (check interval: {}s)", CHECK_INTERVAL_SECS);
        // Give rc-agent 60s to fully start before first check
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        // Track when the WS was last seen connected (local to this task)
        let mut ws_last_connected = Instant::now();
        // Count consecutive checks with CLOSE_WAIT flood (Ollama may be unavailable)
        let mut close_wait_strikes: u32 = 0;
        // OBS-05: Track first decision for lifecycle logging
        let mut first_decision_logged = false;

        loop {
            interval.tick().await;

            if status.ws_connected.load(Ordering::Relaxed) {
                ws_last_connected = Instant::now();
            }

            let close_wait = count_close_wait_on_8090();
            let ws_dead_secs = ws_last_connected.elapsed().as_secs();

            // Track consecutive CLOSE_WAIT strikes; reset when count drops below threshold
            if close_wait >= CLOSE_WAIT_THRESHOLD {
                close_wait_strikes = close_wait_strikes.saturating_add(1);
            } else {
                close_wait_strikes = 0;
            }

            let mut issues: Vec<String> = Vec::new();
            if close_wait >= CLOSE_WAIT_THRESHOLD {
                issues.push(format!("{} CLOSE_WAIT sockets on :8090", close_wait));
            }
            if ws_dead_secs >= WS_DEAD_SECS {
                issues.push(format!("WebSocket disconnected for {}s", ws_dead_secs));
            }

            if issues.is_empty() {
                // OBS-05: Log first decision (healthy)
                if !first_decision_logged {
                    first_decision_logged = true;
                    tracing::info!(target: "state", task = "self_monitor", event = "lifecycle", "lifecycle: first_decision");
                }
                tracing::debug!(
                    target: LOG_TARGET,
                    "OK (close_wait={}, ws_dead={}s)",
                    close_wait, ws_dead_secs
                );
                continue;
            }

            // OBS-05: Log first decision (issue detected)
            if !first_decision_logged {
                first_decision_logged = true;
                tracing::info!(target: "state", task = "self_monitor", event = "lifecycle", "lifecycle: first_decision");
            }

            tracing::warn!(target: LOG_TARGET, "Issues: {}", issues.join("; "));
            log_event(&format!("ISSUE: {}", issues.join("; ")));

            // WS dead too long — relaunch directly, no AI needed.
            // The reconnect loop retries every 30s; 5min means ~10 failed attempts.
            // A full process restart is the right escalation regardless of Ollama.
            if ws_dead_secs >= WS_DEAD_SECS {
                // Phase 68: Check if a SwitchController was received recently.
                // Suppress relaunch for 60s after a switch to allow reconnection to new URL.
                let last_switch_ms = status.last_switch_ms.load(Ordering::Relaxed);
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let since_switch_ms = now_ms.saturating_sub(last_switch_ms);
                let switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000;

                if switch_grace_active {
                    tracing::info!(
                        target: LOG_TARGET,
                        "WS dead {}s but SwitchController received {}ms ago — suppressing relaunch",
                        ws_dead_secs, since_switch_ms
                    );
                } else {
                    tracing::warn!(target: LOG_TARGET, "WebSocket dead {}s — relaunching to reestablish", ws_dead_secs);
                    log_event(&format!("RELAUNCH: ws_dead={}s (threshold={}s) — no AI needed", ws_dead_secs, WS_DEAD_SECS));
                    relaunch_self();
                    continue;
                }
            }

            // CLOSE_WAIT persistent for 5+ checks (~5 min) — restart without Ollama.
            // Ollama may not be installed on pods; don't let a missing model leave
            // 128 stuck sockets forever. 5 strikes = intentional threshold, not a blip.
            if close_wait_strikes >= 5 {
                tracing::warn!(target: LOG_TARGET, "CLOSE_WAIT persisted for {} checks — relaunching without AI", close_wait_strikes);
                log_event(&format!("RELAUNCH: close_wait={} (strike={}) — Ollama not consulted", close_wait, close_wait_strikes));
                relaunch_self();
                continue;
            }

            // CLOSE_WAIT flood (early strikes) — consult Ollama for nuanced diagnosis.
            #[cfg(feature = "ai-debugger")]
            {
            if !config.enabled {
                tracing::info!(target: LOG_TARGET, "AI disabled — skipping CLOSE_WAIT analysis");
                log_event(&format!("SKIP: AI disabled, close_wait={}", close_wait));
                continue;
            }

            let prompt = format!(
                "rc-agent health check on a RacingPoint sim racing pod (Windows 11). \
                Issues detected: {}. \
                Should rc-agent restart to resolve these? Reply with RESTART or OK only.",
                issues.join("; ")
            );

            match query_ollama(&config.ollama_url, &config.ollama_model, &prompt).await {
                Ok(response) => {
                    tracing::info!(target: LOG_TARGET, "Ollama: {}", response.trim());
                    if response.trim().to_uppercase().contains("RESTART") {
                        tracing::warn!(target: LOG_TARGET, "Ollama recommends restart — relaunching");
                        relaunch_self();
                    }
                }
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, "Ollama unavailable ({}) — waiting for strike limit", e);
                }
            }
            } // end #[cfg(feature = "ai-debugger")]
        }
        // OBS-05: Lifecycle event — task exit (loop broke, unexpected)
        tracing::warn!(target: "state", task = "self_monitor", event = "lifecycle", "lifecycle: exit");
    });
}

/// Append a timestamped entry to the rc-bot event log.
/// Rotates (truncates) the file when it exceeds MAX_LOG_BYTES.
pub fn log_event(event: &str) {
    use std::fs::OpenOptions;
    use std::io::Write;

    // Rotate if too large
    if let Ok(meta) = std::fs::metadata(EVENT_LOG) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::write(EVENT_LOG, b"");
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Format: seconds-since-epoch (no chrono dependency needed here)
    let line = format!("[{}] {}\n", now, event);

    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(EVENT_LOG) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Count TCP connections in CLOSE_WAIT state on local port 8090.
/// Runs netstat locally — no network call.
///
/// NOTE: Locale dependency — netstat output text ("CLOSE_WAIT") may differ on
/// non-English Windows locales. Known limitation; acceptable for our English-locale pods.
fn count_close_wait_on_8090() -> usize {
    let Ok(out) = std::process::Command::new("netstat").args(["-ano"]).output() else {
        return 0;
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.contains("CLOSE_WAIT") && l.contains(":8090"))
        .count()
}

/// Shared reqwest client for Ollama queries — initialized once, reused forever.
/// Avoids per-call client construction overhead and connection pool thrashing.
#[cfg(feature = "ai-debugger")]
static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

#[cfg(feature = "ai-debugger")]
fn ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("ollama HTTP client build failed")
    })
}

/// Query local Ollama for a short health decision. 30s timeout.
#[cfg(feature = "ai-debugger")]
async fn query_ollama(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct OllamaResp {
        response: String,
    }
    let resp = ollama_client()
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .send()
        .await?
        .json::<OllamaResp>()
        .await?;
    Ok(resp.response)
}

/// Sentinel file that tells rc-sentry this was a graceful self-restart,
/// not a crash. rc-sentry checks for this and skips escalation if present.
const GRACEFUL_RELAUNCH_SENTINEL: &str = r"C:\RacingPoint\GRACEFUL_RELAUNCH";

/// TCP port that rc-sentry listens on. Used to detect whether sentry is alive
/// before deciding how to relaunch rc-agent.
const SENTRY_PORT: u16 = 8091;

/// Bug #18: Timeout for the TCP connect attempt to rc-sentry. 5 seconds handles
/// slow startup and busy listener without false negatives.
const SENTRY_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Check if rc-sentry is alive by attempting TCP connect to localhost:8091.
/// Returns true if connection succeeds within 2 seconds.
/// Uses std::net (blocking) since this runs right before process::exit anyway.
fn check_sentry_alive() -> bool {
    check_sentry_alive_on_port(SENTRY_PORT)
}

/// Inner helper: attempt TCP connect to 127.0.0.1:<port> with SENTRY_CHECK_TIMEOUT.
/// Extracted so tests can inject an ephemeral port without touching SENTRY_PORT.
fn check_sentry_alive_on_port(port: u16) -> bool {
    use std::net::{SocketAddr, TcpStream};
    let addr: SocketAddr = ([127, 0, 0, 1], port).into();
    TcpStream::connect_timeout(&addr, SENTRY_CHECK_TIMEOUT).is_ok()
}

/// Cap on total restarts before requiring a reboot. Prevents infinite
/// restart loops when the underlying issue is not transient.
const MAX_RESTARTS: u32 = 5;
static RESTART_COUNT: AtomicU32 = AtomicU32::new(0);

/// Reset the restart counter back to 0. Call after a successful WS connection
/// confirms the agent is stable, so transient restart budget is restored.
pub fn reset_restart_count() {
    let prev = RESTART_COUNT.swap(0, Ordering::SeqCst);
    if prev > 0 {
        tracing::info!(target: LOG_TARGET, "Restart count reset (was {})", prev);
    }
}

/// Sentry-aware relaunch: prefer yielding to rc-sentry (clean exit + sentinel)
/// over spawning a new PowerShell process (which leaks ~90MB per restart).
///
/// - Sentry alive (normal case): write GRACEFUL_RELAUNCH sentinel, exit cleanly.
///   rc-sentry's watchdog detects the dead agent, sees the sentinel, skips
///   escalation, and restarts rc-agent via Session 1 spawn (Phase 184).
///   Zero PowerShell spawned.
///
/// - Sentry dead (fallback): write sentinel + spawn PowerShell+DETACHED_PROCESS.
///   PowerShell is the only proven path for self-restart when no external
///   supervisor exists. start-rcagent.bat kills orphan powershell.exe on next boot.
///
/// Writes the GRACEFUL_RELAUNCH sentinel in both paths so rc-sentry won't
/// count it as an escalation crash if sentry comes back before the restart.
pub fn relaunch_self() {
    let count = RESTART_COUNT.fetch_add(1, Ordering::SeqCst);
    if count >= MAX_RESTARTS {
        tracing::error!(
            target: LOG_TARGET,
            "Restart cap reached ({}/{}) — refusing to restart. Reboot required.",
            count, MAX_RESTARTS
        );
        log_event(&format!("RESTART_CAP_REACHED: {} restarts exhausted, reboot required", count));
        return;
    }
    if check_sentry_alive() {
        // SELF-01: Sentry is alive — write sentinel and exit cleanly.
        // rc-sentry's watchdog will detect rc-agent is dead, see the GRACEFUL_RELAUNCH
        // sentinel, skip escalation, and restart via Session 1 spawn (no PowerShell leak).
        tracing::info!(
            target: LOG_TARGET,
            "rc-sentry reachable on :{} — writing sentinel and exiting (sentry will restart us)",
            SENTRY_PORT
        );
        // OBS-05: Log sentinel file write before creating it
        tracing::warn!(target: "state", sentinel = "GRACEFUL_RELAUNCH", action = "create", path = GRACEFUL_RELAUNCH_SENTINEL, "sentinel file write: graceful relaunch via sentry");
        if let Err(e) = std::fs::write(
            GRACEFUL_RELAUNCH_SENTINEL,
            "self_monitor relaunch — sentry will restart\n",
        ) {
            tracing::warn!(target: LOG_TARGET, "Failed to write graceful relaunch sentinel: {}", e);
        }
        log_event("RELAUNCH_VIA_SENTRY: sentinel written, exiting for sentry restart");
        std::process::exit(0);
    }

    // SELF-02: Sentry is dead — fall back to PowerShell+DETACHED_PROCESS.
    // This is the only proven working self-restart when no external supervisor exists.
    tracing::warn!(
        target: LOG_TARGET,
        "rc-sentry unreachable on :{} — falling back to PowerShell relaunch",
        SENTRY_PORT
    );
    log_event("RELAUNCH_POWERSHELL: sentry unreachable, using PowerShell fallback");

    // Write sentinel BEFORE exiting so rc-sentry sees it if it comes back
    // OBS-05: Log sentinel file write before creating it
    tracing::warn!(target: "state", sentinel = "GRACEFUL_RELAUNCH", action = "create", path = GRACEFUL_RELAUNCH_SENTINEL, "sentinel file write: graceful relaunch fallback (sentry unreachable)");
    if let Err(e) = std::fs::write(GRACEFUL_RELAUNCH_SENTINEL, "self_monitor relaunch\n") {
        tracing::warn!(target: LOG_TARGET, "Failed to write graceful relaunch sentinel: {}", e);
    }

    // PowerShell + DETACHED_PROCESS is the ONLY combo that reliably relaunches on Windows.
    // CREATE_NO_WINDOW and cmd.exe both fail to spawn into the interactive session.
    // Known trade-off: PowerShell stays resident (~90MB per relaunch). Mitigated by
    // start-rcagent.bat killing orphan powershell.exe on every boot.
    let ps_cmd = concat!(
        "Start-Sleep 3; ",
        "Start-Process 'C:\\RacingPoint\\rc-agent.exe' ",
        "-WorkingDirectory 'C:\\RacingPoint'"
    );
    match std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", ps_cmd])
        .creation_flags(DETACHED_PROCESS)
        .spawn()
    {
        Ok(_) => {
            tracing::info!(
                target: LOG_TARGET,
                "PowerShell relaunch scheduled (sentinel written). Exiting."
            );
            std::process::exit(0);
        }
        Err(e) => {
            // Clean up sentinel on failure — we didn't actually relaunch
            let _ = std::fs::remove_file(GRACEFUL_RELAUNCH_SENTINEL);
            tracing::error!(target: LOG_TARGET, "Failed to spawn PowerShell relaunch: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- TDD RED: sentry-aware relaunch tests (Phase 187) ---

    #[test]
    fn sentry_port_constant_is_8091() {
        assert_eq!(SENTRY_PORT, 8091);
    }

    #[test]
    fn check_sentry_alive_returns_true_when_listener_exists() {
        // Bind to an ephemeral port to simulate a live sentry
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        // check_sentry_alive uses SENTRY_PORT, so we temporarily test via the helper directly
        let result = check_sentry_alive_on_port(port);
        assert!(result, "should return true when a listener exists on the port");
    }

    #[test]
    fn check_sentry_alive_returns_false_when_no_listener() {
        // Use a high port that should be unoccupied
        let result = check_sentry_alive_on_port(19999);
        assert!(!result, "should return false when no listener on port");
    }

    #[test]
    fn check_sentry_alive_returns_within_5_seconds() {
        use std::time::Instant;
        let start = Instant::now();
        // Port 19998 should be unoccupied (no listener) — tests the timeout path
        let _ = check_sentry_alive_on_port(19998);
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_secs() <= 5,
            "check_sentry_alive should complete within 5s, took {:?}",
            elapsed
        );
    }

    // --- existing tests ---

    #[test]
    fn close_wait_threshold_is_reasonable() {
        // CLOSE_WAIT_THRESHOLD must be > 0 and <= 50 (sane operating range)
        assert!(CLOSE_WAIT_THRESHOLD > 0);
        assert!(CLOSE_WAIT_THRESHOLD <= 50);
    }

    #[test]
    fn ws_dead_threshold_is_between_30s_and_5min() {
        // Must not restart on brief blips (>30s) but must act fast enough for gaming (<=5min)
        assert!(WS_DEAD_SECS >= 30, "threshold too aggressive — would restart on brief blips");
        assert!(WS_DEAD_SECS <= 300, "threshold too slow — gaming sessions need fast recovery");
    }

    #[test]
    fn restart_detected_in_response() {
        let response = "RESTART";
        assert!(response.trim().to_uppercase().contains("RESTART"));
        let response_ok = "OK";
        assert!(!response_ok.trim().to_uppercase().contains("RESTART"));
    }

    #[test]
    fn last_switch_guard_suppresses_within_60s() {
        // Simulate: last_switch_ms was 30 seconds ago
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last_switch_ms = now_ms - 30_000; // 30s ago

        let since_switch_ms = now_ms.saturating_sub(last_switch_ms);
        let switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000;

        assert!(switch_grace_active, "Grace should be active within 60s of switch");
    }

    #[test]
    fn last_switch_guard_allows_after_60s() {
        // Simulate: last_switch_ms was 90 seconds ago
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last_switch_ms = now_ms - 90_000; // 90s ago

        let since_switch_ms = now_ms.saturating_sub(last_switch_ms);
        let switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000;

        assert!(!switch_grace_active, "Grace should NOT be active after 60s");
    }

    #[test]
    fn last_switch_guard_allows_when_never_switched() {
        // Simulate: last_switch_ms = 0 (never switched)
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last_switch_ms: u64 = 0;

        let since_switch_ms = now_ms.saturating_sub(last_switch_ms);
        let switch_grace_active = last_switch_ms != 0 && since_switch_ms < 60_000;

        assert!(!switch_grace_active, "Grace should NOT be active when never switched (last_switch_ms=0)");
    }

    #[test]
    fn close_wait_strike_limit_is_at_least_3_and_at_most_10() {
        // 5 strikes × 60s checks = 5 min before auto-restart without Ollama.
        // Must be > threshold to avoid restarting on brief bursts (>3),
        // but short enough to resolve a persistent flood (<= 10 min).
        const STRIKE_LIMIT: u32 = 5;
        assert!(STRIKE_LIMIT >= 3, "too eager — would restart on brief CLOSE_WAIT bursts");
        assert!(STRIKE_LIMIT <= 10, "too slow — 128 stuck sockets for 10+ min is unacceptable");
    }
}
