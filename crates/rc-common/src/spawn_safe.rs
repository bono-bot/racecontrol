//! Safe process spawning helpers for FreeConsole() environments.
//!
//! After `FreeConsole()` at startup, inherited stdio handles are INVALID.
//! Any `Command::new()` that doesn't set `Stdio::null()` risks
//! `ERROR_INVALID_HANDLE (os error 6)`. These helpers ensure all child
//! process spawns are safe.
//!
//! Two variants:
//! - `spawn_safe()` — all stdio null, for fire-and-forget (taskkill, reg, etc.)
//! - `spawn_safe_capture()` — stdin null, stdout/stderr default, for `.output()` calls

use std::process::{Command, Stdio};

/// Create a Command pre-configured for the FreeConsole() environment:
/// - `Stdio::null()` on stdin/stdout/stderr (prevents os error 6)
/// - `CREATE_NO_WINDOW` on Windows (prevents console flash)
///
/// Use for ALL background utilities (taskkill, reg, powershell, netstat, etc.)
/// Do NOT use for processes that need visible windows (Edge, games, ConspitLink).
pub fn spawn_safe(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null())
       .stdout(Stdio::null())
       .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Like `spawn_safe()` but leaves stdout/stderr as default for `.output()` capture.
/// Still sets stdin to null and `CREATE_NO_WINDOW`.
///
/// Use when you need to read the child's output (tasklist, netstat, nvidia-smi, etc.)
pub fn spawn_safe_capture(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null());
    // stdout and stderr left as default — .output() sets up pipes automatically
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_safe_creates_command() {
        let cmd = spawn_safe("echo");
        // Can't inspect Stdio fields, but verify it doesn't panic
        assert_eq!(cmd.get_program(), "echo");
    }

    #[test]
    fn spawn_safe_capture_creates_command() {
        let cmd = spawn_safe_capture("echo");
        assert_eq!(cmd.get_program(), "echo");
    }

    #[test]
    fn spawn_safe_output_works() {
        // spawn_safe with all null stdio should be able to run a simple command
        let result = spawn_safe("echo").arg("test").status();
        assert!(result.is_ok(), "spawn_safe echo should succeed: {:?}", result);
    }

    #[test]
    fn spawn_safe_capture_output_works() {
        // spawn_safe_capture should be able to capture output
        let result = spawn_safe_capture("echo").arg("test").output();
        assert!(result.is_ok(), "spawn_safe_capture echo should succeed: {:?}", result);
        if let Ok(output) = result {
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(stdout.contains("test"), "stdout should contain 'test': {}", stdout);
        }
    }
}
