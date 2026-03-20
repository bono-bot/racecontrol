//! Shared command execution primitives.
//!
//! `run_cmd_sync` — stdlib-only, uses `wait-timeout` crate for cross-platform
//! child-process timeout without pulling in tokio. Safe for use in rc-sentry.
//!
//! `run_cmd_async` — tokio-backed, only compiled when the `tokio` feature is
//! enabled. Intended for rc-agent which already depends on tokio.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Result returned by both sync and async exec helpers.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    /// Process exit code. -1 when the process could not be spawned or waited on.
    pub exit_code: i32,
    /// True when the process was killed because it exceeded the timeout.
    pub timed_out: bool,
    /// True when combined stdout+stderr exceeded `max_output` bytes and was truncated.
    pub truncated: bool,
}

/// Execute `cmd` via `cmd.exe /C` synchronously, with a hard timeout and output cap.
///
/// - Uses `wait-timeout` (stdlib-only) — no tokio dependency.
/// - On Windows, spawns with `CREATE_NO_WINDOW` so no console flickers on pods.
/// - Kills and reaps the child on timeout before returning.
pub fn run_cmd_sync(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult {
    let mut c = Command::new("cmd.exe");
    c.args(["/C", cmd]);
    c.stdout(Stdio::piped());
    c.stderr(Stdio::piped());

    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);

    let mut child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("spawn error: {e}"),
                exit_code: -1,
                timed_out: false,
                truncated: false,
            };
        }
    };

    match child.wait_timeout(timeout) {
        Err(e) => ExecResult {
            stdout: String::new(),
            stderr: format!("wait error: {e}"),
            exit_code: -1,
            timed_out: false,
            truncated: false,
        },

        // Timeout branch — process still running.
        Ok(None) => {
            let _ = child.kill();
            // Reap the child to avoid a zombie.
            let _ = child.wait();

            // Read whatever bytes accumulated in the pipes.
            let mut out_bytes = Vec::new();
            let mut err_bytes = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_end(&mut out_bytes);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_end(&mut err_bytes);
            }

            let (stdout, stderr, truncated) = truncate_output(out_bytes, err_bytes, max_output);
            ExecResult {
                stdout,
                stderr,
                exit_code: -1,
                timed_out: true,
                truncated,
            }
        }

        // Normal exit branch.
        Ok(Some(status)) => {
            let exit_code = status.code().unwrap_or(-1);

            let mut out_bytes = Vec::new();
            let mut err_bytes = Vec::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_end(&mut out_bytes);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_end(&mut err_bytes);
            }

            let (stdout, stderr, truncated) = truncate_output(out_bytes, err_bytes, max_output);
            ExecResult {
                stdout,
                stderr,
                exit_code,
                timed_out: false,
                truncated,
            }
        }
    }
}

/// Truncate raw byte buffers to `max` combined bytes before converting to UTF-8.
///
/// Stdout gets priority: it may use up to `max` bytes. Stderr gets the remainder.
/// Truncation is done on the Vec<u8> (never on a String) to avoid UTF-8 boundary
/// panics.
fn truncate_output(mut out: Vec<u8>, mut err: Vec<u8>, max: usize) -> (String, String, bool) {
    let total = out.len() + err.len();
    let truncated = total > max;

    if truncated {
        if out.len() > max {
            out.truncate(max);
            err.truncate(0);
        } else {
            let err_budget = max.saturating_sub(out.len());
            err.truncate(err_budget);
        }
    }

    (
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
        truncated,
    )
}

/// Execute `cmd` via `cmd.exe /C` asynchronously, with a hard timeout and output cap.
///
/// Only compiled when the `tokio` feature is enabled. rc-sentry must NOT enable
/// this feature — `cargo tree -p rc-sentry` must show zero tokio references.
#[cfg(feature = "tokio")]
pub async fn run_cmd_async(cmd: &str, timeout: Duration, max_output: usize) -> ExecResult {
    use tokio::process::Command as TokioCommand;
    use tokio::time::timeout as tokio_timeout;

    let mut c = TokioCommand::new("cmd.exe");
    c.args(["/C", cmd]);
    c.stdout(Stdio::piped());
    c.stderr(Stdio::piped());
    c.kill_on_drop(true);

    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW);

    let child = match c.spawn() {
        Ok(ch) => ch,
        Err(e) => {
            return ExecResult {
                stdout: String::new(),
                stderr: format!("spawn error: {e}"),
                exit_code: -1,
                timed_out: false,
                truncated: false,
            };
        }
    };

    match tokio_timeout(timeout, child.wait_with_output()).await {
        Err(_elapsed) => {
            // kill_on_drop handles cleanup.
            ExecResult {
                stdout: String::new(),
                stderr: String::new(),
                exit_code: -1,
                timed_out: true,
                truncated: false,
            }
        }
        Ok(Err(e)) => ExecResult {
            stdout: String::new(),
            stderr: format!("wait error: {e}"),
            exit_code: -1,
            timed_out: false,
            truncated: false,
        },
        Ok(Ok(output)) => {
            let exit_code = output.status.code().unwrap_or(-1);
            let (stdout, stderr, truncated) =
                truncate_output(output.stdout, output.stderr, max_output);
            ExecResult {
                stdout,
                stderr,
                exit_code,
                timed_out: false,
                truncated,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_run_cmd_sync_basic() {
        let result = run_cmd_sync("echo hello", Duration::from_secs(10), 64 * 1024);
        assert!(
            result.stdout.contains("hello"),
            "expected 'hello' in stdout, got: {:?}",
            result.stdout
        );
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
    }

    #[test]
    fn test_run_cmd_sync_timeout() {
        // ping -n 10 adds ~9 seconds of delay; timeout is 1s so it must be killed.
        let result = run_cmd_sync("ping -n 10 127.0.0.1", Duration::from_secs(1), 64 * 1024);
        assert!(result.timed_out, "expected timed_out=true, got: {result:?}");
    }

    #[test]
    fn test_run_cmd_sync_bad_command() {
        let result = run_cmd_sync(
            "nonexistent_command_xyz_abc",
            Duration::from_secs(10),
            64 * 1024,
        );
        // Either exit_code != 0 or stderr has content.
        assert!(
            result.exit_code != 0 || !result.stderr.is_empty(),
            "expected non-zero exit or stderr for unknown command, got: {result:?}"
        );
    }

    #[test]
    fn test_output_truncation() {
        // echo produces very short output; max_output=5 means it should be truncated.
        // Use a command that produces enough chars.
        let result = run_cmd_sync("echo 1234567890", Duration::from_secs(10), 5);
        assert!(result.truncated, "expected truncated=true");
        // Combined output must not exceed max.
        assert!(
            result.stdout.len() + result.stderr.len() <= 5,
            "output exceeded max: stdout={} stderr={}",
            result.stdout.len(),
            result.stderr.len()
        );
    }

    #[test]
    fn test_truncate_output_fn() {
        // 6 bytes stdout + 6 bytes stderr = 12 total; max = 8.
        let out = b"abcdef".to_vec();
        let err = b"ghijkl".to_vec();
        let (stdout, stderr, truncated) = truncate_output(out, err, 8);
        assert!(truncated);
        assert!(stdout.len() + stderr.len() <= 8);
    }
}
