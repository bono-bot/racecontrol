//! Session 1 spawn — launch processes in the active interactive desktop session.
//!
//! Two modes:
//! - **Session 0 (service/SYSTEM):** Uses WTSQueryUserToken + CreateProcessAsUser
//!   to bridge the Session 0/1 boundary. Requires SE_TCB_NAME privilege.
//! - **Session 1 (interactive/user):** Uses std::process::Command directly since
//!   we're already in the interactive session. No special privileges needed.
//!
//! Auto-detects which mode to use via ProcessIdToSessionId on the current process.
//! MMA fix (5/5 consensus): WTSQueryUserToken silently fails from user context
//! (error 1314 = ERROR_PRIVILEGE_NOT_HELD), leaving rc-agent permanently down.
//!
//! Pure std — no anyhow, no tokio. Error handling via Result<(), String>.

use std::path::Path;

const LOG_TARGET: &str = "session1-spawn";

/// Get the session ID of the current process.
#[cfg(windows)]
fn current_session_id() -> u32 {
    use winapi::um::processthreadsapi::{GetCurrentProcessId, ProcessIdToSessionId};
    unsafe {
        let pid = GetCurrentProcessId();
        let mut session_id: u32 = 0;
        if ProcessIdToSessionId(pid, &mut session_id) != 0 {
            session_id
        } else {
            0xFFFF_FFFF // unknown
        }
    }
}

/// Spawn a bat script in the active interactive Session 1.
///
/// Auto-detects context:
/// - If caller is in Session 0 (SYSTEM service): uses WTS token bridge
/// - If caller is in Session 1 (interactive user): uses direct Command spawn
///
/// Returns Err(reason) if no active console session exists (e.g., before user login at boot).
/// The caller should fall back to schtasks in that case.
#[cfg(windows)]
pub fn spawn_in_session1(bat_path: &Path) -> Result<(), String> {
    let my_session = current_session_id();
    tracing::info!(target: LOG_TARGET, "Current process session: {}", my_session);

    // If we're already in an interactive session (not Session 0), spawn directly.
    // WTSQueryUserToken requires SYSTEM/SE_TCB_NAME — it will fail from user context.
    if my_session != 0 && my_session != 0xFFFF_FFFF {
        tracing::info!(target: LOG_TARGET,
            "Already in Session {} — using direct Command spawn (no WTS needed)",
            my_session
        );
        return spawn_direct(bat_path);
    }

    spawn_via_wts(bat_path)
}

/// Direct spawn — used when rc-sentry is already in Session 1 (interactive).
/// Child process inherits the caller's session, desktop, and environment.
#[cfg(windows)]
fn spawn_direct(bat_path: &Path) -> Result<(), String> {
    let work_dir = bat_path.parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\RacingPoint"));

    match std::process::Command::new("cmd.exe")
        .args(["/c", &bat_path.to_string_lossy()])
        .current_dir(&work_dir)
        .spawn()
    {
        Ok(child) => {
            tracing::info!(target: LOG_TARGET,
                "Direct spawn succeeded — PID {} (bat: {})",
                child.id(), bat_path.display()
            );
            Ok(())
        }
        Err(e) => Err(format!("Direct spawn failed: {}", e)),
    }
}

/// WTS token bridge — used when rc-sentry is in Session 0 (SYSTEM service).
/// Requires SE_TCB_NAME privilege (only SYSTEM has this).
#[cfg(windows)]
fn spawn_via_wts(bat_path: &Path) -> Result<(), String> {
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
            return Err(format!("WTSQueryUserToken failed: error code {} (1314=needs SYSTEM privileges)", err));
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
