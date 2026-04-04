//! Graduated restart via Tailscale SSH (EG-03, EG-07).
//!
//! Restart levels:
//!   1. Soft: `schtasks /Run /TN StartRCTemp` — asks Windows to start the server
//!   2. Hard: `taskkill /F /IM racecontrol.exe` + `schtasks /Run /TN StartRCTemp`
//!   3. Report-only: both failed, escalate to WhatsApp (handled by caller)

use crate::config::GuardianConfig;
use tracing::{info, warn, error};

/// Run an SSH command on the server via Tailscale.
///
/// Uses `ssh` with strict options to prevent interactive prompts.
/// All commands are hardcoded strings — no user input is interpolated.
async fn ssh_exec(config: &GuardianConfig, command: &str) -> Result<String, String> {
    info!(command, tailscale_ip = %config.tailscale_ip, "Executing SSH command");

    let user_host = format!("{}@{}", config.ssh_user, config.tailscale_ip);

    let result = tokio::process::Command::new("ssh")
        .args([
            "-o", "StrictHostKeyChecking=accept-new",
            "-o", "ConnectTimeout=15",
            "-o", "BatchMode=yes",
            &user_host,
            command,
        ])
        .output()
        .await;

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if output.status.success() {
                info!(
                    exit_code = output.status.code().unwrap_or(-1),
                    stdout_len = stdout.len(),
                    "SSH command succeeded"
                );
                Ok(stdout)
            } else {
                let code = output.status.code().unwrap_or(-1);
                warn!(
                    exit_code = code,
                    stderr = %stderr,
                    "SSH command failed"
                );
                Err(format!("SSH exit code {}: {}", code, stderr))
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to spawn SSH process");
            Err(format!("SSH spawn failed: {e}"))
        }
    }
}

/// EG-07 Step 1: Soft restart via schtasks.
///
/// Uses `schtasks /Run /TN StartRCTemp` which calls `start-racecontrol.bat`,
/// which kills orphan watchdogs and starts the server via StartRCDirect.
pub async fn soft_restart(config: &GuardianConfig) -> bool {
    info!("Attempting SOFT restart via schtasks");

    match ssh_exec(config, "schtasks /Run /TN StartRCTemp").await {
        Ok(output) => {
            // schtasks prints "SUCCESS: Attempted to run..." on success
            let success = output.contains("SUCCESS") || output.contains("success");
            if success {
                info!("Soft restart command accepted by scheduler");
            } else {
                warn!(output = %output, "schtasks returned but without SUCCESS marker");
            }
            // Return actual success status — false lets caller fall through to hard restart immediately
            success
        }
        Err(e) => {
            error!(error = %e, "Soft restart SSH command failed");
            false
        }
    }
}

/// EG-07 Step 2: Hard restart — kill process then start.
///
/// First kills racecontrol.exe forcefully, waits 5s, then starts via schtasks.
/// Also kills orphan PowerShell watchdog processes to prevent interference.
pub async fn hard_restart(config: &GuardianConfig) -> bool {
    info!("Attempting HARD restart (taskkill + schtasks)");

    // Kill the racecontrol process first
    match ssh_exec(config, "taskkill /F /IM racecontrol.exe").await {
        Ok(_) => {
            info!("taskkill command sent — waiting 5s before starting");
        }
        Err(e) => {
            // taskkill might fail if process is already dead — that's OK
            warn!(error = %e, "taskkill failed (process may already be dead) — proceeding with start");
        }
    }

    // Wait for process to die
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    // Also kill any orphan watchdog PowerShell processes
    match ssh_exec(config, "taskkill /F /IM powershell.exe").await {
        Ok(_) => {
            info!("Killed orphan PowerShell watchdog processes");
        }
        Err(e) => {
            // May fail if no PowerShell running — that's fine
            warn!(error = %e, "PowerShell kill failed (may not be running)");
        }
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Now start via schtasks
    match ssh_exec(config, "schtasks /Run /TN StartRCTemp").await {
        Ok(output) => {
            let success = output.contains("SUCCESS") || output.contains("success");
            if success {
                info!("Hard restart: schtasks start command accepted");
            } else {
                warn!(output = %output, "Hard restart: schtasks returned without SUCCESS marker");
            }
            true
        }
        Err(e) => {
            error!(error = %e, "Hard restart: schtasks start failed");

            // Fallback: try StartRCDirect
            info!("Trying fallback: schtasks /Run /TN StartRCDirect");
            match ssh_exec(config, "schtasks /Run /TN StartRCDirect").await {
                Ok(_) => {
                    info!("Fallback StartRCDirect accepted");
                    true
                }
                Err(e2) => {
                    error!(error = %e2, "Both StartRCTemp and StartRCDirect failed");
                    false
                }
            }
        }
    }
}
