use anyhow;
use std::path::Path;
use tracing;

/// Spawn start-rcagent.bat in the active interactive Session 1 from a SYSTEM service context.
///
/// Uses WTSGetActiveConsoleSessionId + WTSQueryUserToken + CreateProcessAsUser
/// to bridge the Session 0/Session 1 boundary. std::process::Command from SYSTEM
/// always targets Session 0 and cannot show a GUI.
///
/// Returns Err if no active console session exists (normal at boot before user login).
#[cfg(windows)]
pub fn spawn_in_session1(exe_dir: &Path) -> anyhow::Result<()> {
    use std::ptr;
    use winapi::ctypes::c_void;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{CreateProcessAsUserW, PROCESS_INFORMATION, STARTUPINFOW};
    use winapi::um::securitybaseapi::DuplicateTokenEx;
    use winapi::um::userenv::{CreateEnvironmentBlock, DestroyEnvironmentBlock};
    use winapi::um::winbase::WTSGetActiveConsoleSessionId;
    use winapi::um::winnt::{SecurityImpersonation, TokenPrimary, TOKEN_ALL_ACCESS};
    use winapi::um::wtsapi32::WTSQueryUserToken;

    unsafe {
        // 1. Get active console session ID
        let session_id = WTSGetActiveConsoleSessionId();
        if session_id == 0xFFFF_FFFF {
            anyhow::bail!("No active console session — deferring restart");
        }
        tracing::info!("Active console session: {}", session_id);

        // 2. Get user token for that session
        let mut user_token = ptr::null_mut();
        if WTSQueryUserToken(session_id, &mut user_token) == 0 {
            let err = GetLastError();
            anyhow::bail!("WTSQueryUserToken failed: error code {}", err);
        }

        // 3. Duplicate token as a primary token
        let mut dup_token = ptr::null_mut();
        let dup_result = DuplicateTokenEx(
            user_token,
            TOKEN_ALL_ACCESS,
            ptr::null_mut(),
            SecurityImpersonation,
            TokenPrimary,
            &mut dup_token,
        );
        CloseHandle(user_token);

        if dup_result == 0 {
            let err = GetLastError();
            anyhow::bail!("DuplicateTokenEx failed: error code {}", err);
        }

        // 4. Create environment block for the user
        let mut env_block: *mut c_void = ptr::null_mut();
        let env_result = CreateEnvironmentBlock(&mut env_block, dup_token, 0);
        if env_result == 0 {
            tracing::warn!("CreateEnvironmentBlock failed, proceeding without user environment");
            env_block = ptr::null_mut();
        }

        // 5. Build command line: cmd.exe /c "C:\RacingPoint\start-rcagent.bat"
        let bat_path = exe_dir.join("start-rcagent.bat");
        let cmd_str = format!("cmd.exe /c \"{}\"", bat_path.display());
        let mut cmd_wide: Vec<u16> = cmd_str.encode_utf16().chain(std::iter::once(0)).collect();

        // Desktop string for interactive session
        let mut desktop: Vec<u16> = "winsta0\\default\0"
            .encode_utf16()
            .collect();

        // 6. Set up STARTUPINFO
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

        // CREATE_UNICODE_ENVIRONMENT (0x00000400) | CREATE_NO_WINDOW (0x08000000)
        let creation_flags: u32 = 0x00000400 | 0x08000000;

        let create_result = CreateProcessAsUserW(
            dup_token,
            ptr::null(),
            cmd_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            0, // bInheritHandles = FALSE
            creation_flags,
            env_block as *mut c_void,
            ptr::null(),
            &mut si,
            &mut pi,
        );

        // 7. Clean up all handles
        if !env_block.is_null() {
            DestroyEnvironmentBlock(env_block);
        }
        CloseHandle(dup_token);

        if create_result == 0 {
            let err = GetLastError();
            anyhow::bail!("CreateProcessAsUserW failed: error code {}", err);
        }

        // Close process and thread handles (we don't need to track the child)
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);

        tracing::info!(
            "Spawned start-rcagent.bat in session {} (PID {})",
            session_id,
            pi.dwProcessId
        );
    }

    Ok(())
}

/// Non-Windows stub: Session 1 spawn is not supported.
#[cfg(not(windows))]
pub fn spawn_in_session1(_exe_dir: &Path) -> anyhow::Result<()> {
    anyhow::bail!("Session 1 spawn not supported on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // On Windows, we test that the function handles gracefully when not running as SYSTEM.
    // WTSQueryUserToken requires SE_TCB_NAME privilege which only LocalSystem has.
    // The function should return an error, not panic.
    #[cfg(windows)]
    #[test]
    fn test_spawn_in_session1_returns_error_in_test_context() {
        let result = spawn_in_session1(&PathBuf::from("C:\\RacingPoint"));
        // We expect an error because we don't have SYSTEM privileges
        assert!(result.is_err(), "Expected error when not running as SYSTEM");
    }

    // On non-Windows, spawn_in_session1 always returns Err.
    #[cfg(not(windows))]
    #[test]
    fn test_spawn_in_session1_not_supported_on_non_windows() {
        let result = spawn_in_session1(&PathBuf::from("C:\\RacingPoint"));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("not supported"),
            "Expected 'not supported' in error: {}",
            err_msg
        );
    }
}
