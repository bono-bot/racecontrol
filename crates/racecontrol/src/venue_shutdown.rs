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
//! James (.27) stays alive. The shutdown script shuts down: pods -> POS -> server.

use axum::Json;
use axum::extract::State;
use serde_json::{Value, json};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::state::AppState;

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

/// POST /api/v1/venue/shutdown
///
/// Staff-initiated safe shutdown gate. Returns:
/// - `{"status": "blocked", "reason": "billing_active", ...}` — active billing sessions
/// - `{"status": "blocked", "reason": "audit_failed", ...}` — P1 issues found
/// - `{"status": "blocked", "reason": "audit_error", ...}` — audit could not run
/// - `{"status": "blocked", "reason": "james_offline", ...}` — SSH unreachable
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
        "cd '{}' && AUDIT_PIN=261121 bash scripts/auto-detect.sh --mode quick --no-fix --no-notify",
        JAMES_REPO_ROOT
    );

    match run_ssh_command(JAMES_IP, JAMES_SSH_USER, &audit_cmd, AUDIT_TIMEOUT_SECS).await {
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
            tracing::error!("[venue_shutdown] SSH to James failed: {}", e);
            return Json(json!({
                "status": "blocked",
                "reason": "james_offline",
                "message": format!(
                    "James is offline or SSH failed: {}. Try manual shutdown procedure.",
                    e
                )
            }));
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
