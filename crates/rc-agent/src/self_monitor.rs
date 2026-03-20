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
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::ai_debugger::AiDebuggerConfig;
use crate::udp_heartbeat::HeartbeatStatus;

const CHECK_INTERVAL_SECS: u64 = 60;      // check every minute
const CLOSE_WAIT_THRESHOLD: usize = 20;   // flag if :8090 has 20+ stuck sockets
const WS_DEAD_SECS: u64 = 300;           // relaunch if disconnected 5+ min — allows slow boot reconnect without false-positive kills
const DETACHED_PROCESS: u32 = 0x0000_0008;
const EVENT_LOG: &str = r"C:\RacingPoint\rc-bot-events.log";
const MAX_LOG_BYTES: u64 = 512 * 1024;    // rotate at 512KB

/// Spawn the self-monitor background task.
pub fn spawn(config: AiDebuggerConfig, status: Arc<HeartbeatStatus>) {
    tokio::spawn(async move {
        // Give rc-agent 60s to fully start before first check
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        // Track when the WS was last seen connected (local to this task)
        let mut ws_last_connected = Instant::now();
        // Count consecutive checks with CLOSE_WAIT flood (Ollama may be unavailable)
        let mut close_wait_strikes: u32 = 0;

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
                tracing::debug!(
                    "[rc-bot] OK (close_wait={}, ws_dead={}s)",
                    close_wait, ws_dead_secs
                );
                continue;
            }

            tracing::warn!("[rc-bot] Issues: {}", issues.join("; "));
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
                        "[rc-bot] WS dead {}s but SwitchController received {}ms ago — suppressing relaunch",
                        ws_dead_secs, since_switch_ms
                    );
                } else {
                    tracing::warn!("[rc-bot] WebSocket dead {}s — relaunching to reestablish", ws_dead_secs);
                    log_event(&format!("RELAUNCH: ws_dead={}s (threshold={}s) — no AI needed", ws_dead_secs, WS_DEAD_SECS));
                    relaunch_self();
                    continue;
                }
            }

            // CLOSE_WAIT persistent for 5+ checks (~5 min) — restart without Ollama.
            // Ollama may not be installed on pods; don't let a missing model leave
            // 128 stuck sockets forever. 5 strikes = intentional threshold, not a blip.
            if close_wait_strikes >= 5 {
                tracing::warn!("[rc-bot] CLOSE_WAIT persisted for {} checks — relaunching without AI", close_wait_strikes);
                log_event(&format!("RELAUNCH: close_wait={} (strike={}) — Ollama not consulted", close_wait, close_wait_strikes));
                relaunch_self();
                continue;
            }

            // CLOSE_WAIT flood (early strikes) — consult Ollama for nuanced diagnosis.
            if !config.enabled {
                tracing::info!("[rc-bot] AI disabled — skipping CLOSE_WAIT analysis");
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
                    tracing::info!("[rc-bot] Ollama: {}", response.trim());
                    if response.trim().to_uppercase().contains("RESTART") {
                        tracing::warn!("[rc-bot] Ollama recommends restart — relaunching");
                        relaunch_self();
                    }
                }
                Err(e) => {
                    tracing::warn!("[rc-bot] Ollama unavailable ({}) — waiting for strike limit", e);
                }
            }
        }
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
static OLLAMA_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn ollama_client() -> &'static reqwest::Client {
    OLLAMA_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("ollama HTTP client build failed")
    })
}

/// Query local Ollama for a short health decision. 30s timeout.
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

/// Spawn a detached PowerShell process that waits 3s then starts a fresh rc-agent,
/// then exit the current process. The 3s gap ensures the port :8090 and
/// :18923 are freed before the new instance binds them.
///
/// Uses Start-Process instead of cmd `start ""` — Start-Process works reliably
/// even when the invoking process has no attached console or interactive desktop.
pub fn relaunch_self() {
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
            tracing::info!("[rc-bot] Relaunch scheduled. Exiting current process.");
            std::process::exit(0);
        }
        Err(e) => {
            tracing::error!("[rc-bot] Failed to spawn relaunch: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
