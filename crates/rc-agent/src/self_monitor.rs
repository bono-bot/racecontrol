//! Self-monitoring daemon for rc-agent health.
//!
//! Runs every 5 minutes. Detects CLOSE_WAIT socket floods on :8090 (caused by
//! racecontrol's pod_monitor hammering the pod) and prolonged WebSocket disconnects.
//!
//! Two recovery paths:
//!   WS dead 10+ min → relaunch immediately (no AI needed — reconnect loop already failed)
//!   CLOSE_WAIT flood → query local Ollama for RESTART/OK decision
//! On relaunch: spawns a detached cmd.exe that waits 3s then restarts rc-agent.

use std::os::windows::process::CommandExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::ai_debugger::AiDebuggerConfig;
use crate::udp_heartbeat::HeartbeatStatus;

const CHECK_INTERVAL_SECS: u64 = 300;     // 5 minutes
const CLOSE_WAIT_THRESHOLD: usize = 20;   // flag if :8090 has 20+ stuck sockets
const WS_DEAD_SECS: u64 = 600;            // flag if disconnected for 10+ minutes
const DETACHED_PROCESS: u32 = 0x0000_0008;

/// Spawn the self-monitor background task.
pub fn spawn(config: AiDebuggerConfig, status: Arc<HeartbeatStatus>) {
    tokio::spawn(async move {
        // Give rc-agent 60s to fully start before first check
        tokio::time::sleep(Duration::from_secs(60)).await;

        let mut interval = tokio::time::interval(Duration::from_secs(CHECK_INTERVAL_SECS));
        // Track when the WS was last seen connected (local to this task)
        let mut ws_last_connected = Instant::now();

        loop {
            interval.tick().await;

            if status.ws_connected.load(Ordering::Relaxed) {
                ws_last_connected = Instant::now();
            }

            let close_wait = count_close_wait_on_8090();
            let ws_dead_secs = ws_last_connected.elapsed().as_secs();

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

            // WS dead too long — relaunch directly, no AI needed.
            // The reconnect loop already retried for 10+ minutes without success;
            // a full process restart is the right escalation regardless of Ollama.
            if ws_dead_secs >= WS_DEAD_SECS {
                tracing::warn!("[rc-bot] WebSocket dead {}s — relaunching to reestablish", ws_dead_secs);
                relaunch_self();
                continue;
            }

            // CLOSE_WAIT flood — consult Ollama for nuanced diagnosis.
            if !config.enabled {
                tracing::info!("[rc-bot] AI disabled — skipping CLOSE_WAIT analysis");
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
                    tracing::warn!("[rc-bot] Ollama unavailable: {} — skipping CLOSE_WAIT restart", e);
                }
            }
        }
    });
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

/// Query local Ollama for a short health decision. 30s timeout.
async fn query_ollama(url: &str, model: &str, prompt: &str) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct OllamaResp {
        response: String,
    }
    let resp = reqwest::Client::new()
        .post(format!("{}/api/generate", url))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "stream": false,
        }))
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .json::<OllamaResp>()
        .await?;
    Ok(resp.response)
}

/// Spawn a detached cmd.exe that waits 3s then starts a fresh rc-agent,
/// then exit the current process. The 3s gap ensures the port :8090 and
/// :18923 are freed before the new instance binds them.
fn relaunch_self() {
    let cmd = concat!(
        r#"timeout /t 3 /nobreak > nul "#,
        r#"&& start "" /D "C:\RacingPoint" "C:\RacingPoint\rc-agent.exe""#
    );
    match std::process::Command::new("cmd")
        .args(["/c", cmd])
        .creation_flags(DETACHED_PROCESS)
        .spawn()
    {
        Ok(_) => {
            tracing::info!("[rc-bot] Relaunch scheduled. Exiting current process.");
            std::process::exit(0);
        }
        Err(e) => {
            tracing::error!("[rc-bot] Failed to spawn relaunch cmd: {}", e);
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
    fn ws_dead_threshold_is_at_least_5min() {
        // We should not restart for a brief blip — 10 minutes minimum
        assert!(WS_DEAD_SECS >= 300);
    }

    #[test]
    fn restart_detected_in_response() {
        let response = "RESTART";
        assert!(response.trim().to_uppercase().contains("RESTART"));
        let response_ok = "OK";
        assert!(!response_ok.trim().to_uppercase().contains("RESTART"));
    }
}
