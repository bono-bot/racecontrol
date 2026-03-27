//! Bono alert via comms-link send-message.js.
//! Spawns node with COMMS_PSK + COMMS_URL set in environment.
//! Fire-and-forget — waits for node to exit (~1-2s for WS message).

const NODE_EXE: &str = r"C:\Program Files\nodejs\node.exe";
const SEND_MSG_JS: &str = r"C:\Users\bono\racingpoint\comms-link\send-message.js";
const COMMS_DIR: &str = r"C:\Users\bono\racingpoint\comms-link";
// Env var names for COMMS_PSK and COMMS_URL (must be set in environment)
// COMMS_PSK — pre-shared key for comms-link WS authentication
// COMMS_URL — WebSocket URL for comms-link server (e.g. ws://srv1422716.hstgr.cloud:8765)

/// Alert Bono via comms-link WebSocket. Returns Ok(()) even if node.exe is absent.
pub fn alert_bono(message: &str) -> std::io::Result<()> {
    alert_bono_with_exe(NODE_EXE, message)
}

/// Inner function for testability — accepts custom node executable path.
pub fn alert_bono_with_exe(node_exe: &str, message: &str) -> std::io::Result<()> {
    let comms_psk = match std::env::var("COMMS_PSK") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            tracing::error!("bono_alert: COMMS_PSK env var is unset or empty — cannot send alert");
            return Ok(());
        }
    };
    let comms_url = match std::env::var("COMMS_URL") {
        Ok(v) if !v.is_empty() => v,
        _ => {
            tracing::error!("bono_alert: COMMS_URL env var is unset or empty — cannot send alert");
            return Ok(());
        }
    };

    let mut cmd = std::process::Command::new(node_exe);
    cmd.arg(SEND_MSG_JS)
        .arg(message)
        .current_dir(COMMS_DIR)
        .env("COMMS_PSK", &comms_psk)
        .env("COMMS_URL", &comms_url);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.spawn() {
        Ok(mut child) => {
            // Wait for node to exit with 5-second timeout
            let timeout = std::time::Duration::from_secs(5);
            let start = std::time::Instant::now();
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => {
                        if start.elapsed() >= timeout {
                            tracing::warn!("bono_alert: node process exceeded 5s timeout, killing");
                            let _ = child.kill();
                            let _ = child.wait();
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    Err(e) => {
                        tracing::warn!("bono_alert: error waiting for node: {}", e);
                        break;
                    }
                }
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!("bono_alert: failed to spawn node: {}", e);
            Ok(()) // Degraded alert — do not panic
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_bono_missing_node_returns_ok() {
        // Should not panic or return Err even when node.exe doesn't exist
        let result = alert_bono_with_exe(r"C:\nonexistent\node.exe", "test watchdog alert");
        assert!(
            result.is_ok(),
            "alert_bono_with_exe must return Ok(()) on missing exe"
        );
    }

    #[test]
    fn test_alert_bono_empty_message_returns_ok() {
        // Empty message with missing exe — still must return Ok(())
        let result = alert_bono_with_exe(r"C:\nonexistent\node.exe", "");
        assert!(result.is_ok(), "alert_bono_with_exe must handle empty message");
    }
}
