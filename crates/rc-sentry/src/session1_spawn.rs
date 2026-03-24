//! Session 1 spawn — launch processes in the active interactive desktop session.
//!
//! rc-sentry runs as SYSTEM (Session 0). GUI processes like rc-agent MUST run in
//! Session 1 where the interactive desktop exists. std::process::Command from SYSTEM
//! always targets Session 0 — processes start but have no visible desktop.
//!
//! Uses WTSGetActiveConsoleSessionId + WTSQueryUserToken + CreateProcessAsUser
//! to bridge the Session 0/Session 1 boundary.
//!
//! Pure std — no anyhow, no tokio. Error handling via Result<(), String>.

use std::path::Path;

const LOG_TARGET: &str = "session1-spawn";

/// Spawn a bat script in the active interactive Session 1 from a SYSTEM service context.
///
/// Returns Err(reason) if no active console session exists (e.g., before user login at boot).
/// The caller should fall back to schtasks in that case.
#[cfg(windows)]
pub fn spawn_in_session1(bat_path: &Path) -> Result<(), String> {
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
            return Err("No active console session — deferring to schtasks fallback".to_string());
        }
        tracing::info!(target: LOG_TARGET, "Active console session: {}", session_id);

        // 2. Get user token for that session
        let mut user_token = ptr::null_mut();
        if WTSQueryUserToken(session_id, &mut user_token) == 0 {
            let err = GetLastError();
            return Err(format!("WTSQueryUserToken failed: error code {}", err));
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
            return Err(format!("DuplicateTokenEx failed: error code {}", err));
        }

        // 4. Create environment block for the user
        let mut env_block: *mut c_void = ptr::null_mut();
        let env_result = CreateEnvironmentBlock(&mut env_block, dup_token, 0);
        if env_result == 0 {
            tracing::warn!(target: LOG_TARGET, "CreateEnvironmentBlock failed, proceeding without user environment");
            env_block = ptr::null_mut();
        }

        // 5. Build command line: cmd.exe /c "C:\RacingPoint\start-rcagent.bat"
        let cmd_str = format!("cmd.exe /c \"{}\"", bat_path.display());
        let mut cmd_wide: Vec<u16> = cmd_str.encode_utf16().chain(std::iter::once(0)).collect();

        // Desktop string for interactive session
        let mut desktop: Vec<u16> = "winsta0\\default\0".encode_utf16().collect();

        // 6. Set up STARTUPINFO
        let mut si: STARTUPINFOW = std::mem::zeroed();
        si.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        si.lpDesktop = desktop.as_mut_ptr();

        let mut pi: PROCESS_INFORMATION = std::mem::zeroed();

        // 7. Get working directory from bat_path parent
        let work_dir = bat_path.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| r"C:\RacingPoint".to_string());
        let mut work_dir_wide: Vec<u16> = work_dir.encode_utf16().chain(std::iter::once(0)).collect();

        // 8. CreateProcessAsUser — CREATE_UNICODE_ENVIRONMENT | CREATE_NEW_CONSOLE
        let create_flags = winapi::um::winbase::CREATE_UNICODE_ENVIRONMENT
            | winapi::um::winbase::CREATE_NEW_CONSOLE;

        let success = CreateProcessAsUserW(
            dup_token,
            ptr::null(),
            cmd_wide.as_mut_ptr(),
            ptr::null_mut(),
            ptr::null_mut(),
            0, // don't inherit handles
            create_flags,
            env_block,
            work_dir_wide.as_mut_ptr(),
            &mut si,
            &mut pi,
        );

        // Cleanup
        if !env_block.is_null() {
            DestroyEnvironmentBlock(env_block);
        }
        CloseHandle(dup_token);

        if success == 0 {
            let err = GetLastError();
            return Err(format!("CreateProcessAsUserW failed: error code {}", err));
        }

        tracing::info!(target: LOG_TARGET,
            "Session 1 spawn succeeded — PID {} (bat: {})",
            pi.dwProcessId, bat_path.display()
        );

        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);

        Ok(())
    }
}

/// Non-Windows stub — always returns Err.
#[cfg(not(windows))]
pub fn spawn_in_session1(_bat_path: &Path) -> Result<(), String> {
    Err("Session 1 spawn only supported on Windows".to_string())
}
