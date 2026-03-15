//! Racing Point Pod Installer — Robust Rust binary
//!
//! Launched by install.bat (Defender shield wrapper).
//! Handles all 14 installation steps with proper error handling.
//!
//! Design:
//!   - install.bat adds Defender exclusions (can't be quarantined — it's a .bat)
//!   - This binary does everything else with Result<> error handling
//!   - Critical steps abort on failure, non-critical steps warn and continue
//!   - Zero dependencies — just std

#[cfg(not(windows))]
compile_error!("rc-installer is Windows-only");

use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

// FFI for mutex probing (zombie process detection)
#[cfg(windows)]
unsafe extern "system" {
    fn OpenMutexA(dwDesiredAccess: u32, bInheritHandle: i32, lpName: *const u8) -> *mut core::ffi::c_void;
    fn CloseHandle(hObject: *mut core::ffi::c_void) -> i32;
}
const SYNCHRONIZE: u32 = 0x00100000;

// Windows process creation flags
const CREATE_NO_WINDOW: u32 = 0x08000000;
const DETACHED_PROCESS: u32 = 0x00000008;

// Installation constants
const DEST_DIR: &str = r"C:\RacingPoint";
const CORE_URL: &str = "ws://192.168.31.23:8080/ws/agent";
const TOTAL_STEPS: u8 = 14;
const MIN_BINARY_SIZE: u64 = 1_000_000; // 1MB — anything less is truncated

// ANSI color codes (Windows 10+ with virtual terminal processing)
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

// ═══════════════════════════════════════════════════════════════
//  Entry point
// ═══════════════════════════════════════════════════════════════

fn main() {
    enable_ansi_colors();

    println!();
    println!("  ========================================");
    println!("  {}  Racing Point Pod Installer{}", BOLD, RESET);
    println!("  ========================================");
    println!();

    // Must be admin (install.bat handles elevation, but check anyway)
    if !is_admin() {
        fail("This installer requires Administrator privileges.");
        fail("Run install.bat (it auto-elevates) instead.");
        wait_and_exit(1);
    }

    // Get pod number from CLI arg or interactive prompt
    let pod = match get_pod_number() {
        Ok(p) => p,
        Err(e) => {
            fail(&e);
            wait_and_exit(1);
        }
    };

    // Source = directory where this exe lives (pendrive)
    let src = match get_source_dir() {
        Ok(s) => s,
        Err(e) => {
            fail(&e);
            wait_and_exit(1);
        }
    };

    let dest = PathBuf::from(DEST_DIR);

    println!("  Pod {} | Source: {}", pod, src.display());
    println!("  Destination: {}", dest.display());
    println!();
    println!("  Press Enter to start, or close window to cancel.");
    let _ = io::stdin().read_line(&mut String::new());

    println!();
    println!("  ========================================");
    println!("  {}  Installing Pod {}{}", BOLD, pod, RESET);
    println!("  ========================================");

    let exit_code = run_installation(pod, &src, &dest);
    std::process::exit(exit_code);
}

// ═══════════════════════════════════════════════════════════════
//  Orchestrator — runs all 14 steps + health check
// ═══════════════════════════════════════════════════════════════

fn run_installation(pod: u8, src: &Path, dest: &Path) -> i32 {
    // ── Step 1: Verify source files (CRITICAL) ───────────────
    step(1, "Checking source files on pendrive");
    if let Err(e) = verify_sources(pod, src) {
        fail(&e);
        return 1;
    }

    // ── Step 2: Verify Defender exclusion (IMPORTANT) ────────
    // install.bat should have already done this, but verify
    step(2, "Verifying Defender exclusions");
    if let Err(e) = verify_defender_exclusion(dest) {
        warn(&format!("{}", e));
        warn("Batch wrapper should have handled this — proceeding");
    }

    // ── Step 3: Kill existing processes (CRITICAL) ───────────
    // Must confirm dead — old rc-agent holds a single-instance
    // mutex that makes new instances exit(0) silently
    step(3, "Killing existing processes");
    if let Err(e) = kill_processes() {
        fail(&e);
        return 1;
    }

    // ── Step 4: Prepare destination ──────────────────────────
    step(4, &format!("Preparing {}", dest.display()));
    if let Err(e) = prepare_destination(dest) {
        warn(&format!("Cleanup issue: {}", e));
        warn("Continuing — files will be overwritten");
    }

    // ── Step 5: Copy files (CRITICAL) ────────────────────────
    step(5, "Copying files from pendrive");
    if let Err(e) = copy_files(pod, src, dest) {
        fail(&e);
        return 1;
    }

    // ── Step 6: Unblock files ────────────────────────────────
    // Remove Zone.Identifier ADS (SmartScreen mark-of-the-web)
    step(6, "Unblocking files (SmartScreen)");
    if let Err(e) = unblock_files(dest) {
        warn(&format!("Unblock issue: {}", e));
    }

    // ── Step 7: Quarantine check (CRITICAL) ──────────────────
    // Wait for Defender real-time scan, verify binary survived
    step(7, "Anti-quarantine check (waiting 5 seconds)");
    if let Err(e) = quarantine_check(src, dest) {
        fail(&e);
        return 1;
    }

    // ── Step 8: Verify files + binary size (CRITICAL) ────────
    step(8, "Verifying files at destination");
    let bin_size = match verify_files(dest) {
        Ok(size) => {
            ok(&format!("4/4 files verified, binary: {} bytes", size));
            size
        }
        Err(e) => {
            fail(&e);
            return 1;
        }
    };

    // ── Step 9: Verify config (CRITICAL) ─────────────────────
    step(9, "Verifying config");
    match verify_config(pod, dest) {
        Ok(()) => ok(&format!("Config OK: Pod {}, core URL present", pod)),
        Err(e) => {
            fail(&e);
            return 1;
        }
    }

    // ── Step 10: Harden Edge ─────────────────────────────────
    step(10, "Hardening Edge");
    match harden_edge() {
        Ok(()) => ok("Edge hardened"),
        Err(e) => warn(&format!("Edge hardening issue: {}", e)),
    }

    // ── Step 11: Registry keys (IMPORTANT) ───────────────────
    step(11, "Setting registry keys");
    match set_registry_keys(dest) {
        Ok(()) => ok("RCAgent Run key set, PodAgent key removed"),
        Err(e) => {
            warn(&format!("Registry issue: {}", e));
            warn("Auto-start may not work until manual setup");
        }
    }

    // ── Step 12: WinRM remote management (IMPORTANT) ─────────
    step(12, "Setting up WinRM remote management");
    match setup_winrm() {
        Ok(()) => {}
        Err(e) => {
            warn(&format!("WinRM issue: {}", e));
            warn("Remote management may not work");
        }
    }

    // ── Step 13: Firewall rules (IMPORTANT) ──────────────────
    step(13, "Setting firewall rules");
    match setup_firewall() {
        Ok(()) => ok("Ports 5985 (WinRM) and 8090 (rc-agent) open"),
        Err(e) => warn(&format!("Firewall issue: {}", e)),
    }

    // ── Step 14: Start rc-agent ──────────────────────────────
    step(14, "Starting rc-agent");
    if let Err(e) = start_rc_agent(src, dest) {
        warn(&format!("{}", e));
    }

    // ── Health Check ─────────────────────────────────────────
    println!();
    println!("  ========================================");
    println!("  {}  Post-Install Health Check{}", BOLD, RESET);
    println!("  ========================================");
    println!();

    let problems = health_check(dest);

    println!();
    if problems > 0 {
        println!("  ========================================");
        println!(
            "  {}{}  WARNING: {} critical issue(s)!{}",
            BOLD, RED, problems, RESET
        );
        println!("  Check errors above before leaving.");
        println!("  WinRM may still work for remote debug.");
        println!("  ========================================");
        1
    } else {
        println!("  ========================================");
        println!(
            "  {}{}  Pod {} INSTALLED SUCCESSFULLY{}",
            BOLD, GREEN, pod, RESET
        );
        println!("  Binary: rc-agent.exe ({} bytes)", bin_size);
        println!("  Services: rc-agent + WinRM");
        println!("  Reboot pod for Session 1 GUI.");
        println!("  ========================================");
        0
    }
}

// ═══════════════════════════════════════════════════════════════
//  Step implementations
// ═══════════════════════════════════════════════════════════════

fn verify_sources(pod: u8, src: &Path) -> Result<(), String> {
    let files: Vec<String> = vec![
        "rc-agent.exe".into(),
        format!("rc-agent-pod{}.toml", pod),
        "start-rcagent.bat".into(),
        "edge-harden.bat".into(),
    ];

    let missing: Vec<&String> = files.iter().filter(|f| !src.join(f).exists()).collect();

    if !missing.is_empty() {
        return Err(format!(
            "Missing on pendrive: {}",
            missing
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    ok(&format!("All {} source files found", files.len()));
    Ok(())
}

fn verify_defender_exclusion(dest: &Path) -> Result<(), String> {
    let dest_str = dest.to_string_lossy();

    // Check if exclusion already exists
    let output = run_ps(&format!(
        "if ((Get-MpPreference).ExclusionPath -contains '{}') {{ exit 0 }} else {{ exit 1 }}",
        dest_str
    ))?;

    if output.status.success() {
        ok(&format!("Defender exclusion active for {}", dest_str));
        return Ok(());
    }

    // Try to add it ourselves (belt and suspenders — batch wrapper should have done this)
    info("Exclusion not found — adding now...");
    let _ = run_ps(&format!(
        "Add-MpPreference -ExclusionPath '{}' -ErrorAction SilentlyContinue",
        dest_str
    ));
    let _ = run_ps(
        &format!("Add-MpPreference -ExclusionProcess '{}\\rc-agent.exe' -ErrorAction SilentlyContinue", dest_str),
    );

    // Wait for WdFilter.sys minifilter to recognize new exclusions
    info("Waiting 2s for exclusion propagation...");
    thread::sleep(Duration::from_secs(2));

    // Verify
    let check = run_ps(&format!(
        "if ((Get-MpPreference).ExclusionPath -contains '{}') {{ exit 0 }} else {{ exit 1 }}",
        dest_str
    ))?;

    if check.status.success() {
        ok(&format!("Defender exclusion added for {}", dest_str));
        Ok(())
    } else {
        Err("Could not verify or add Defender exclusion".into())
    }
}

fn kill_processes() -> Result<(), String> {
    // Kill all relevant processes
    for proc in &[
        "rc-agent.exe",
        "pod-agent.exe",
        "msedge.exe",
        "msedgewebview2.exe",
    ] {
        let _ = run("taskkill", &["/F", "/IM", proc]);
    }

    // Verify rc-agent is truly dead (critical for mutex release).
    // taskkill /F calls TerminateProcess which is ASYNCHRONOUS — it can return
    // success while the kernel is still closing handles (including the mutex).
    // Check every second for up to 15 seconds, re-kill every 3s.
    for elapsed in 1..=15 {
        thread::sleep(Duration::from_secs(1));

        if !is_process_running("rc-agent.exe") {
            // Process gone from tasklist, but kernel may still be closing handles.
            // Add safety margin for mutex handle cleanup.
            thread::sleep(Duration::from_millis(500));
            ok(&format!("All processes confirmed dead ({}s)", elapsed));
            return Ok(());
        }

        if elapsed % 3 == 0 {
            info(&format!(
                "rc-agent still alive ({}s), retrying kill...",
                elapsed
            ));
            let _ = run("taskkill", &["/F", "/IM", "rc-agent.exe"]);
        }
    }

    // Last resort: process may be "zombie" in tasklist (held by antivirus handle)
    // but mutex is already released. Check if mutex is actually free.
    if !is_mutex_held("Global\\RacingPoint_RCAgent_SingleInstance") {
        warn("rc-agent still in tasklist but mutex is released (zombie process)");
        ok("Safe to proceed — mutex is free");
        return Ok(());
    }

    Err("rc-agent.exe won't die after 15 seconds. Reboot the pod and try again.".into())
}

fn prepare_destination(dest: &Path) -> Result<(), String> {
    if !dest.exists() {
        fs::create_dir_all(dest)
            .map_err(|e| format!("Cannot create {}: {}", dest.display(), e))?;
    }

    // Clean all known files (best effort)
    let stale = [
        "rc-agent.exe",
        "pod-agent.exe",
        "rc-agent-new.exe",
        "rc-agent-prev.exe",
        "do-deploy.bat",
        "do-swap.bat",
        "do-rollback.bat",
        "cleanup.bat",
        "deploy-error.log",
        "rc-agent.log",
        "rc-agent-stderr.txt",
        "rc-agent.toml",
        "start-rcagent.bat",
        "start-podagent.bat",
        "edge-harden.bat",
        "watchdog-rcagent.bat",
    ];

    for f in &stale {
        let path = dest.join(f);
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
    }

    ok("Cleaned");
    Ok(())
}

fn copy_files(pod: u8, src: &Path, dest: &Path) -> Result<(), String> {
    // Use std::fs::copy — proper error handling, no shell quoting issues
    let copies: Vec<(PathBuf, PathBuf)> = vec![
        (src.join("rc-agent.exe"), dest.join("rc-agent.exe")),
        (
            src.join(format!("rc-agent-pod{}.toml", pod)),
            dest.join("rc-agent.toml"),
        ),
        (
            src.join("start-rcagent.bat"),
            dest.join("start-rcagent.bat"),
        ),
        (src.join("edge-harden.bat"), dest.join("edge-harden.bat")),
    ];

    for (from, to) in &copies {
        fs::copy(from, to).map_err(|e| {
            format!(
                "Copy failed: {} -> {}: {}",
                from.file_name().unwrap_or_default().to_string_lossy(),
                to.file_name().unwrap_or_default().to_string_lossy(),
                e
            )
        })?;
    }

    ok("4 files copied");
    Ok(())
}

fn unblock_files(dest: &Path) -> Result<(), String> {
    // Remove Zone.Identifier ADS that SmartScreen adds to files from USB
    run_ps(&format!(
        "Get-ChildItem '{}' | Unblock-File -ErrorAction SilentlyContinue",
        dest.to_string_lossy()
    ))?;
    ok("Files unblocked");
    Ok(())
}

fn quarantine_check(src: &Path, dest: &Path) -> Result<(), String> {
    // Give Defender 5 seconds to scan and potentially quarantine
    thread::sleep(Duration::from_secs(5));

    let binary = dest.join("rc-agent.exe");

    if binary.exists() {
        let size = binary
            .metadata()
            .map_err(|e| format!("Cannot read binary metadata: {}", e))?
            .len();
        ok(&format!(
            "rc-agent.exe on disk ({} bytes) — not quarantined",
            size
        ));
        return Ok(());
    }

    // Binary was quarantined! Attempt recovery.
    warn("DEFENDER QUARANTINED rc-agent.exe!");
    info("Attempting recovery: re-adding exclusion and re-copying...");

    let dest_str = dest.to_string_lossy();
    let _ = run_ps(&format!(
        "Add-MpPreference -ExclusionPath '{}' -ErrorAction SilentlyContinue",
        dest_str
    ));
    let _ = run_ps(
        &format!("Add-MpPreference -ExclusionProcess '{}\\rc-agent.exe' -ErrorAction SilentlyContinue", dest_str),
    );

    thread::sleep(Duration::from_secs(2));

    // Re-copy
    fs::copy(src.join("rc-agent.exe"), &binary)
        .map_err(|e| format!("Re-copy failed: {}", e))?;

    let _ = run_ps(&format!(
        "Unblock-File '{}' -ErrorAction SilentlyContinue",
        binary.to_string_lossy()
    ));

    // Wait and verify
    thread::sleep(Duration::from_secs(5));

    if !binary.exists() {
        return Err(
            "rc-agent.exe keeps getting quarantined!\n\
             \n\
             MANUAL FIX:\n\
               1. Open Windows Security\n\
               2. Virus & threat protection > Protection history\n\
               3. Find rc-agent.exe > Actions > Allow on device\n\
               4. Run this installer again"
                .into(),
        );
    }

    ok("Recovery successful — binary restored after re-copy");
    Ok(())
}

fn verify_files(dest: &Path) -> Result<u64, String> {
    let required = [
        "rc-agent.exe",
        "rc-agent.toml",
        "start-rcagent.bat",
        "edge-harden.bat",
    ];

    let missing: Vec<&&str> = required.iter().filter(|f| !dest.join(f).exists()).collect();
    if !missing.is_empty() {
        return Err(format!(
            "Missing after copy: {}",
            missing
                .iter()
                .map(|s| **s)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    let size = dest
        .join("rc-agent.exe")
        .metadata()
        .map_err(|e| format!("Cannot read binary: {}", e))?
        .len();

    if size < MIN_BINARY_SIZE {
        return Err(format!(
            "rc-agent.exe is {} bytes — likely truncated (expected > {})",
            size, MIN_BINARY_SIZE
        ));
    }

    Ok(size)
}

fn verify_config(pod: u8, dest: &Path) -> Result<(), String> {
    let content = fs::read_to_string(dest.join("rc-agent.toml"))
        .map_err(|e| format!("Cannot read config: {}", e))?;

    let pod_str = format!("number = {}", pod);
    if !content.contains(&pod_str) {
        return Err(format!(
            "Config missing '{}'\nContents:\n{}",
            pod_str, content
        ));
    }

    if !content.contains(CORE_URL) {
        return Err(format!(
            "Config missing core URL '{}'\nContents:\n{}",
            CORE_URL, content
        ));
    }

    Ok(())
}

fn harden_edge() -> Result<(), String> {
    // Stop and disable Edge update services (direct — no bat file needed)
    for svc in &[
        "EdgeUpdate",
        "edgeupdate",
        "MicrosoftEdgeElevationService",
    ] {
        let _ = run("sc", &["stop", svc]);
        let _ = run("sc", &["config", svc, "start=", "disabled"]);
    }

    // Disable startup boost and background mode via policy
    let _ = run("reg", &[
        "add",
        r"HKLM\SOFTWARE\Policies\Microsoft\Edge",
        "/v", "StartupBoostEnabled",
        "/t", "REG_DWORD",
        "/d", "0",
        "/f",
    ]);
    let _ = run("reg", &[
        "add",
        r"HKLM\SOFTWARE\Policies\Microsoft\Edge",
        "/v", "BackgroundModeEnabled",
        "/t", "REG_DWORD",
        "/d", "0",
        "/f",
    ]);

    Ok(())
}

fn set_registry_keys(dest: &Path) -> Result<(), String> {
    let start_bat = dest.join("start-rcagent.bat");
    let start_bat_str = start_bat.to_string_lossy();

    // Set auto-start Run key
    let output = run("reg", &[
        "add",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v", "RCAgent",
        "/d", &start_bat_str,
        "/f",
    ])?;

    if !output.status.success() {
        return Err("Failed to set RCAgent Run key".into());
    }

    // Remove legacy PodAgent key (best effort)
    let _ = run("reg", &[
        "delete",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v", "PodAgent",
        "/f",
    ]);

    Ok(())
}

fn setup_winrm() -> Result<(), String> {
    // Set network to Private (required for WinRM)
    let _ = run_ps(
        "Get-NetConnectionProfile | Set-NetConnectionProfile -NetworkCategory Private",
    );

    // Get current username (pods use "SIM 1", "SIM 2", etc.)
    let username =
        env::var("USERNAME").map_err(|_| "Cannot determine current username".to_string())?;

    // Set password (WinRM requires non-blank password).
    // Note: pod usernames like "SIM 1" have spaces — net user handles this
    // because Rust passes each arg separately (no shell quoting issues).
    let pw_result = run("net", &["user", &username, "racing", "/y"]);
    match &pw_result {
        Ok(o) if o.status.success() => {
            info(&format!("Password set for '{}'", username));
        }
        _ => {
            // Fallback: try PowerShell Set-LocalUser (works with spaces in usernames)
            warn("net user failed, trying PowerShell fallback...");
            let ps_pw = format!(
                "Set-LocalUser -Name '{}' -Password (ConvertTo-SecureString 'racing' -AsPlainText -Force)",
                username
            );
            let _ = run_ps(&ps_pw);
        }
    }

    // Configure auto-login (keeps boot seamless after password change)
    let winlogon = r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion\Winlogon";
    let _ = run("reg", &[
        "add", winlogon,
        "/v", "AutoAdminLogon",
        "/t", "REG_SZ",
        "/d", "1",
        "/f",
    ]);
    let _ = run("reg", &[
        "add", winlogon,
        "/v", "DefaultUserName",
        "/t", "REG_SZ",
        "/d", &username,
        "/f",
    ]);
    let _ = run("reg", &[
        "add", winlogon,
        "/v", "DefaultPassword",
        "/t", "REG_SZ",
        "/d", "racing",
        "/f",
    ]);

    // Enable WinRM via PowerShell (avoids cmd.exe quoting nightmares with winrm.cmd)
    let _ = run_ps("Enable-PSRemoting -Force -SkipNetworkProfileCheck");
    let _ = run_ps(r"Set-Item WSMan:\localhost\Service\AllowUnencrypted -Value $true -Force");
    let _ = run_ps(r"Set-Item WSMan:\localhost\Service\Auth\Basic -Value $true -Force");

    // Ensure auto-start
    let _ = run("sc", &["config", "WinRM", "start=", "auto"]);
    let _ = run("net", &["start", "WinRM"]);

    ok(&format!(
        "Password set for '{}', auto-login configured, WinRM enabled",
        username
    ));
    Ok(())
}

fn setup_firewall() -> Result<(), String> {
    let _ = run("netsh", &[
        "advfirewall", "firewall", "add", "rule",
        "name=WinRM-HTTP",
        "dir=in", "action=allow", "protocol=TCP", "localport=5985",
    ]);
    let _ = run("netsh", &[
        "advfirewall", "firewall", "add", "rule",
        "name=RCAgent",
        "dir=in", "action=allow", "protocol=TCP", "localport=8090",
    ]);
    Ok(())
}

fn start_rc_agent(src: &Path, dest: &Path) -> Result<(), String> {
    let binary = dest.join("rc-agent.exe");

    // Final sanity: binary must exist right before start
    if !binary.exists() {
        return Err("rc-agent.exe disappeared right before start!".into());
    }

    // Clean logs for fresh diagnostic output
    let _ = fs::remove_file(dest.join("rc-agent.log"));
    let _ = fs::remove_file(dest.join("rc-agent-stderr.txt"));

    // ── Attempt 1: Direct launch ─────────────────────────────
    info("Starting rc-agent (attempt 1/2)...");
    Command::new(&binary)
        .current_dir(dest)
        .creation_flags(DETACHED_PROCESS)
        .spawn()
        .map_err(|e| format!("Cannot spawn rc-agent: {}", e))?;

    info("Waiting 10 seconds for startup...");
    thread::sleep(Duration::from_secs(10));

    if is_process_running("rc-agent.exe") {
        ok("rc-agent is running");
        return Ok(());
    }

    // ── First attempt failed — diagnose ──────────────────────
    warn("rc-agent NOT running after first attempt");

    // Was binary quarantined between start and check?
    if !binary.exists() {
        warn("Binary quarantined between start and check!");
        info("Re-copying from pendrive...");
        let _ = fs::copy(src.join("rc-agent.exe"), &binary);
        let _ = run_ps(&format!(
            "Unblock-File '{}' -ErrorAction SilentlyContinue",
            binary.to_string_lossy()
        ));
        thread::sleep(Duration::from_secs(3));
    }

    // Dump any log output from first attempt
    dump_diagnostics(dest, "first attempt");

    // ── Attempt 2: Via cmd wrapper to capture stderr ─────────
    let _ = fs::remove_file(dest.join("rc-agent.log"));
    info("Starting rc-agent (attempt 2/2) with stderr capture...");

    let _ = Command::new("cmd")
        .args(["/C", "rc-agent.exe 2>rc-agent-stderr.txt"])
        .current_dir(dest)
        .creation_flags(DETACHED_PROCESS)
        .spawn();

    info("Waiting 10 seconds...");
    thread::sleep(Duration::from_secs(10));

    if is_process_running("rc-agent.exe") {
        ok("rc-agent started on second attempt");
        return Ok(());
    }

    // ── Both attempts failed ─────────────────────────────────
    fail("rc-agent FAILED TO START on both attempts");
    dump_diagnostics(dest, "second attempt");

    if !binary.exists() {
        fail("Binary no longer on disk — Defender quarantined it");
    }

    println!();
    info("Common causes:");
    info("  1. Defender quarantined the binary (check Protection history)");
    info("  2. Config file invalid (check rc-agent.toml)");
    info("  3. Port 8090 in use by another process");
    info("  4. Missing DLL (check stderr above)");

    Err("rc-agent failed to start after 2 attempts".into())
}

fn health_check(dest: &Path) -> u32 {
    let mut problems: u32 = 0;

    // 1. rc-agent process running
    if is_process_running("rc-agent.exe") {
        ok("rc-agent.exe is running");
    } else {
        fail("rc-agent.exe is NOT running");
        problems += 1;
    }

    // 2. Binary on disk (not quarantined)
    if dest.join("rc-agent.exe").exists() {
        ok("rc-agent.exe binary present on disk");
    } else {
        fail("rc-agent.exe binary MISSING (quarantined?)");
        problems += 1;
    }

    // 3. Config present
    if dest.join("rc-agent.toml").exists() {
        ok("rc-agent.toml present");
    } else {
        fail("rc-agent.toml missing");
        problems += 1;
    }

    // 4. Defender exclusion
    let defender_ok = run_ps(&format!(
        "if ((Get-MpPreference).ExclusionPath -contains '{}') {{ exit 0 }} else {{ exit 1 }}",
        dest.to_string_lossy()
    ))
    .map_or(false, |o| o.status.success());

    if defender_ok {
        ok("Defender exclusion active");
    } else {
        warn("Defender exclusion not confirmed");
    }

    // 5. WinRM service
    let winrm_ok = run("sc", &["query", "WinRM"])
        .map_or(false, |o| {
            String::from_utf8_lossy(&o.stdout).contains("RUNNING")
        });

    if winrm_ok {
        ok("WinRM running on port 5985");
    } else {
        warn("WinRM not running — should start on reboot");
    }

    // 6. Port 8090 (rc-agent HTTP server)
    let port_ok = run("curl", &["-s", "-m", "5", "http://127.0.0.1:8090/ping"])
        .map_or(false, |o| {
            String::from_utf8_lossy(&o.stdout).contains("pong")
        });

    if port_ok {
        ok("Port 8090 responding — remote ops active");
    } else {
        warn("Port 8090 not responding (may need more time or reboot)");
    }

    // 7. Run key
    let run_key_ok = run("reg", &[
        "query",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v", "RCAgent",
    ])
    .map_or(false, |o| o.status.success());

    if run_key_ok {
        ok("RCAgent Run key set (auto-start on boot)");
    } else {
        fail("RCAgent Run key missing");
        problems += 1;
    }

    problems
}

// ═══════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════

/// Run a command with hidden window, capture output.
fn run(program: &str, args: &[&str]) -> Result<Output, String> {
    Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to run '{}': {}", program, e))
}

/// Run a PowerShell command with hidden window, non-interactive.
fn run_ps(script: &str) -> Result<Output, String> {
    run("powershell", &["-NoProfile", "-NonInteractive", "-Command", script])
}

/// Check if a process is running by exact image name.
/// Uses line-based matching to avoid false positives (e.g., "rc-agent-new.exe").
fn is_process_running(name: &str) -> bool {
    run("tasklist", &["/FI", &format!("IMAGENAME eq {}", name), "/NH"])
        .map_or(false, |o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let name_lower = name.to_lowercase();
            stdout.lines().any(|line| {
                let lower = line.to_lowercase();
                // tasklist output: "rc-agent.exe    1234 Console  1  12,345 K"
                // Match exact image name at start of line (before whitespace)
                lower.split_whitespace().next().map_or(false, |first| first == name_lower)
            })
        })
}

/// Check if a named mutex exists (is held by some process).
/// Returns false if the mutex doesn't exist (safe to proceed).
/// Used as zombie-process fallback: tasklist shows process but mutex is gone.
fn is_mutex_held(name: &str) -> bool {
    let mut name_bytes = name.as_bytes().to_vec();
    name_bytes.push(0); // null-terminate
    let handle = unsafe { OpenMutexA(SYNCHRONIZE, 0, name_bytes.as_ptr()) };
    if handle.is_null() {
        // ERROR_FILE_NOT_FOUND (2) = mutex doesn't exist = safe
        false
    } else {
        unsafe { CloseHandle(handle) };
        true
    }
}

/// Dump rc-agent.log and rc-agent-stderr.txt if they exist.
fn dump_diagnostics(dest: &Path, label: &str) {
    let log = dest.join("rc-agent.log");
    if log.exists() {
        if let Ok(content) = fs::read_to_string(&log) {
            if !content.trim().is_empty() {
                info(&format!("-- rc-agent.log ({}) --", label));
                println!("{}", content.trim());
                info("------------------------------------");
            }
        }
    } else {
        info("No log file — crash happened before logger init");
        info("Likely: mutex held, config error, or Defender");
    }

    let stderr = dest.join("rc-agent-stderr.txt");
    if stderr.exists() {
        if let Ok(content) = fs::read_to_string(&stderr) {
            if !content.trim().is_empty() {
                info(&format!("-- stderr ({}) --", label));
                println!("{}", content.trim());
                info("-----------------------");
            }
        }
    }
}

/// Get pod number from CLI argument or interactive prompt.
fn get_pod_number() -> Result<u8, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let pod: u8 = args[1]
            .trim()
            .parse()
            .map_err(|_| format!("'{}' is not a valid number", args[1]))?;
        if !(1..=8).contains(&pod) {
            return Err(format!("Pod number must be 1-8, got: {}", pod));
        }
        return Ok(pod);
    }

    // Interactive prompt
    print!("  Enter pod number (1-8): ");
    io::stdout().flush().map_err(|e| e.to_string())?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|e| e.to_string())?;

    let pod: u8 = input
        .trim()
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", input.trim()))?;
    if !(1..=8).contains(&pod) {
        return Err(format!("Pod number must be 1-8, got: {}", pod));
    }

    Ok(pod)
}

/// Get the directory where this exe lives (= pendrive).
fn get_source_dir() -> Result<PathBuf, String> {
    let exe = env::current_exe().map_err(|e| format!("Cannot find installer location: {}", e))?;
    exe.parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "Cannot determine parent directory".into())
}

/// Enable ANSI escape codes on Windows console.
fn enable_ansi_colors() {
    #[cfg(windows)]
    {
        const STD_OUTPUT_HANDLE: u32 = 0xFFFF_FFF5; // (DWORD)-11
        const ENABLE_VIRTUAL_TERMINAL_PROCESSING: u32 = 0x0004;

        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> *mut core::ffi::c_void;
            fn GetConsoleMode(
                hConsoleHandle: *mut core::ffi::c_void,
                lpMode: *mut u32,
            ) -> i32;
            fn SetConsoleMode(hConsoleHandle: *mut core::ffi::c_void, dwMode: u32) -> i32;
        }

        unsafe {
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle.is_null() {
                return;
            }
            let mut mode: u32 = 0;
            if GetConsoleMode(handle, &mut mode) != 0 {
                let _ = SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
            }
        }
    }
}

/// Check if running as admin (same method as install.bat).
fn is_admin() -> bool {
    let sys_drive = env::var("SYSTEMDRIVE").unwrap_or_else(|_| "C:".into());
    run("fsutil", &["dirty", "query", &sys_drive]).map_or(false, |o| o.status.success())
}

fn wait_and_exit(code: i32) -> ! {
    println!();
    println!("  Press Enter to close...");
    let _ = io::stdin().read_line(&mut String::new());
    std::process::exit(code)
}

// ═══════════════════════════════════════════════════════════════
//  Display helpers
// ═══════════════════════════════════════════════════════════════

fn step(num: u8, description: &str) {
    println!();
    println!(
        "{}[{}/{}]{} {}...",
        BOLD, num, TOTAL_STEPS, RESET, description
    );
}

fn ok(msg: &str) {
    println!("   {}[OK]{}   {}", GREEN, RESET, msg);
}

fn warn(msg: &str) {
    println!("   {}[WARN]{} {}", YELLOW, RESET, msg);
}

fn fail(msg: &str) {
    println!("   {}[FAIL]{} {}", RED, RESET, msg);
}

fn info(msg: &str) {
    println!("   {}", msg);
}
