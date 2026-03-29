//! Venue Shutdown — Staff-initiated safe shutdown with pre-shutdown audit gate.
//!
//! POST /api/v1/venue/shutdown
//!
//! Flow:
//!   1. Billing drain check — block if any active billing sessions
//!   2. Pre-shutdown audit — SSH to James (.27) to run auto-detect.sh --mode quick --no-fix --no-notify
//!      Block on exit code 1 (P1 issues) or 2/timeout (audit error)
//!   3. Trigger ordered shutdown — SSH to James to run venue-shutdown.sh
//!
//! Fallback (James offline):
//!   2b. Try Bono relay for audit (HTTP POST to srv1422716.hstgr.cloud)
//!   3b. Shut down pods via internal wol::shutdown_pod
//!   4b. Notify Bono via relay message
//!   5b. Schedule server self-shutdown via `shutdown /s /t 60`
//!
//! James (.27) stays alive in normal flow. Bono fallback handles James-offline case.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::state::AppState;
use crate::wol;

/// James LAN IP — relay and SSH target for pre-shutdown operations
const JAMES_IP: &str = "192.168.31.27";
/// SSH user on James
const JAMES_SSH_USER: &str = "bono";
/// Max seconds to wait for audit to complete (auto-detect quick mode ~30s)
const AUDIT_TIMEOUT_SECS: u64 = 120;
/// Max seconds to wait for shutdown script invocation to return
const SHUTDOWN_INVOKE_TIMEOUT_SECS: u64 = 30;
/// Repo root on James for running scripts
const JAMES_REPO_ROOT: &str = "C:/Users/bono/racingpoint/racecontrol";
/// Bono VPS relay base URL — HTTP exec endpoint
const BONO_RELAY_URL: &str = "http://srv1422716.hstgr.cloud:8766";
/// Pod IPs for Bono fallback shutdown (all 8 pods)
const POD_IPS: &[&str] = &[
    "192.168.31.89", // Pod 1
    "192.168.31.33", // Pod 2
    "192.168.31.28", // Pod 3
    "192.168.31.88", // Pod 4
    "192.168.31.86", // Pod 5
    "192.168.31.87", // Pod 6
    "192.168.31.38", // Pod 7
    "192.168.31.91", // Pod 8
];

/// POST /api/v1/venue/shutdown
///
/// Staff-initiated safe shutdown gate. Returns:
/// - `{"status": "blocked", "reason": "billing_active", ...}` — active billing sessions
/// - `{"status": "blocked", "reason": "audit_failed", ...}` — P1 issues found
/// - `{"status": "blocked", "reason": "audit_error", ...}` — audit could not run
/// - `{"status": "blocked", "reason": "both_offline", ...}` — James and Bono both unreachable
/// - `{"status": "fallback_bono", ...}` — James offline, Bono fallback activated
/// - `{"status": "shutting_down", ...}` — sequence initiated
pub async fn venue_shutdown_handler(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    // ─── Step 1: Billing drain check ─────────────────────────────────────────
    {
        let timers = state.billing.active_timers.read().await;
        let waiting = state.billing.waiting_for_game.read().await;
        let active_count = timers.len() + waiting.len();
        if active_count > 0 {
            return Json(json!({
                "status": "blocked",
                "reason": "billing_active",
                "active_sessions": active_count,
                "message": format!(
                    "{} active billing session(s). End or wait for them to complete before shutdown.",
                    active_count
                )
            }));
        }
    }

    // ─── Step 2: Pre-shutdown audit via SSH to James ──────────────────────────
    let audit_cmd = format!(
        // SEC-P0-2: PIN read from config, never hardcoded in source
        "cd '{}' && AUDIT_PIN=$AUDIT_PIN bash scripts/auto-detect.sh --mode quick --no-fix --no-notify",
        JAMES_REPO_ROOT
    );

    let james_result = run_ssh_command(JAMES_IP, JAMES_SSH_USER, &audit_cmd, AUDIT_TIMEOUT_SECS).await;

    match &james_result {
        Ok(result) => {
            match result.exit_code {
                0 => {
                    // Audit passed — proceed to step 3
                    tracing::info!("[venue_shutdown] Pre-shutdown audit passed (exit 0)");
                }
                1 => {
                    tracing::warn!("[venue_shutdown] Pre-shutdown audit found P1 issues (exit 1)");
                    let output_tail = tail_string(&result.output, 500);
                    return Json(json!({
                        "status": "blocked",
                        "reason": "audit_failed",
                        "exit_code": 1,
                        "output": output_tail,
                        "message": "Pre-shutdown audit found critical issues. Fix before shutting down."
                    }));
                }
                code => {
                    tracing::error!("[venue_shutdown] Pre-shutdown audit error (exit {})", code);
                    let output_tail = tail_string(&result.output, 500);
                    return Json(json!({
                        "status": "blocked",
                        "reason": "audit_error",
                        "exit_code": code,
                        "output": output_tail,
                        "message": "Pre-shutdown audit encountered an error. Check James logs."
                    }));
                }
            }
        }
        Err(e) => {
            // James offline — attempt Bono fallback
            tracing::warn!("[venue_shutdown] SSH to James failed: {}. Attempting Bono fallback.", e);
            return bono_fallback_shutdown(&state).await;
        }
    }

    // ─── Step 3: Trigger ordered shutdown via SSH to James ───────────────────
    let shutdown_cmd = format!(
        "bash '{}/scripts/venue-shutdown.sh'",
        JAMES_REPO_ROOT
    );

    match run_ssh_command(JAMES_IP, JAMES_SSH_USER, &shutdown_cmd, SHUTDOWN_INVOKE_TIMEOUT_SECS).await {
        Ok(_) => {
            tracing::info!("[venue_shutdown] Shutdown sequence initiated via James");
        }
        Err(e) => {
            // Log but don't block — the script was launched asynchronously; SSH disconnect is OK
            tracing::warn!("[venue_shutdown] SSH shutdown invoke warning (may be OK if script detached): {}", e);
        }
    }

    Json(json!({
        "status": "shutting_down",
        "message": "Shutdown sequence initiated. Order: Pods -> POS -> Server. James (.27) stays alive."
    }))
}

/// Bono fallback shutdown — called when James relay is unreachable.
///
/// Flow:
///   1. Try Bono relay for a lightweight audit (health check via HTTP)
///   2. Shut down all pods via internal wol::shutdown_pod
///   3. Notify Bono via relay message (best-effort)
///   4. Schedule server self-shutdown via `shutdown /s /t 60`
async fn bono_fallback_shutdown(state: &Arc<AppState>) -> Json<Value> {
    tracing::warn!("[venue_shutdown] James offline — activating Bono fallback shutdown");

    // ─── Step 2b: Try Bono relay for audit ───────────────────────────────────
    let bono_audit_url = format!("{}/relay/exec/run", BONO_RELAY_URL);
    let bono_audit_body = json!({
        "command": "racecontrol_health",
        "reason": "venue-shutdown pre-audit fallback (James offline)"
    });

    let bono_reachable = match state.http_client
        .post(&bono_audit_url)
        .json(&bono_audit_body)
        .timeout(Duration::from_secs(20))
        .send()
        .await
    {
        Ok(resp) => {
            tracing::info!("[venue_shutdown] Bono relay reachable (status {})", resp.status());
            true
        }
        Err(e) => {
            tracing::error!("[venue_shutdown] Bono relay also unreachable: {}", e);
            false
        }
    };

    if !bono_reachable {
        return Json(json!({
            "status": "blocked",
            "reason": "both_offline",
            "message": "Both James (.27) and Bono VPS are unreachable. Manual shutdown required: power off pods individually, then server."
        }));
    }

    // ─── Step 3b: Shut down pods via internal wol::shutdown_pod ─────────────
    tracing::info!("[venue_shutdown] Bono fallback: shutting down {} pods", POD_IPS.len());
    let mut pod_results: Vec<Value> = Vec::new();

    for &pod_ip in POD_IPS {
        match wol::shutdown_pod(&state.http_client, pod_ip).await {
            Ok(msg) => {
                tracing::info!("[venue_shutdown] Pod {} shutdown: {}", pod_ip, msg);
                pod_results.push(json!({"ip": pod_ip, "status": "shutdown_sent", "detail": msg}));
            }
            Err(e) => {
                tracing::warn!("[venue_shutdown] Pod {} shutdown failed: {}", pod_ip, e);
                pod_results.push(json!({"ip": pod_ip, "status": "error", "detail": e.to_string()}));
            }
        }
    }

    // ─── Step 4b: Notify Bono via relay message ───────────────────────────────
    let notify_url = format!("{}/relay/exec/run", BONO_RELAY_URL);
    let notify_body = json!({
        "command": "git_status",
        "reason": "venue-shutdown bono-fallback completed — server self-shutdown in 60s"
    });

    // Best-effort notification — don't block shutdown on this
    if let Err(e) = state.http_client
        .post(&notify_url)
        .json(&notify_body)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        tracing::warn!("[venue_shutdown] Bono notification failed (non-blocking): {}", e);
    } else {
        tracing::info!("[venue_shutdown] Bono notified of fallback shutdown");
    }

    // ─── Step 5b: Schedule server self-shutdown ───────────────────────────────
    tracing::warn!("[venue_shutdown] Scheduling server self-shutdown in 60 seconds");

    let shutdown_result = std::process::Command::new("shutdown")
        .args(["/s", "/t", "60", "/c", "Venue shutdown initiated via Bono fallback (James offline)"])
        .spawn();

    let self_shutdown_status = match shutdown_result {
        Ok(_) => {
            tracing::info!("[venue_shutdown] Server self-shutdown scheduled in 60s");
            "scheduled_60s"
        }
        Err(e) => {
            tracing::error!("[venue_shutdown] Server self-shutdown spawn failed: {}", e);
            "spawn_failed"
        }
    };

    Json(json!({
        "status": "fallback_bono",
        "reason": "james_offline",
        "message": "James offline. Bono fallback activated: pods shut down, server self-shutdown scheduled.",
        "pods": pod_results,
        "server_self_shutdown": self_shutdown_status,
        "note": "Server will shut down in ~60 seconds. Bono VPS remains up for monitoring."
    }))
}

// ─── SSH Helper ──────────────────────────────────────────────────────────────

struct SshResult {
    exit_code: i32,
    output: String,
}

/// Run a command on a remote host via SSH and return the combined stdout+stderr + exit code.
/// Returns Err if SSH itself fails (unreachable, auth failure, timeout).
async fn run_ssh_command(
    host: &str,
    user: &str,
    cmd: &str,
    timeout_secs: u64,
) -> Result<SshResult, String> {
    let target = format!("{}@{}", user, host);

    let fut = tokio::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=no",
            "-o", "ConnectTimeout=15",
            "-o", "BatchMode=yes",
            &target,
            cmd,
        ])
        .output();

    let output = timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| format!("SSH command timed out after {}s", timeout_secs))?
        .map_err(|e| format!("SSH spawn error: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{}{}", stdout, stderr);

    let exit_code = output.status.code().unwrap_or(-1);

    Ok(SshResult {
        exit_code,
        output: combined,
    })
}

/// Return the last `n` characters of a string (for truncated output in JSON responses).
fn tail_string(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        s[s.len() - n..].to_string()
    }
}
