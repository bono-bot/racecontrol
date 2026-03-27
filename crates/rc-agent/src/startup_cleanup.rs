//! Startup cleanup — removes stale files, processes, and registry entries on every boot.
//!
//! Runs after self_heal::run() and before core services start. All steps are fail-open:
//! errors are logged at WARN and collected, never abort startup.
//!
//! Two tiers:
//!   - **Every startup:** fast, idempotent tasks (stale binaries, orphan processes, Run keys)
//!   - **Periodic (every 24h):** heavier I/O tasks (crash dumps, log rotation, WER cleanup)

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const LOG_TARGET: &str = "startup-cleanup";

/// Marker file: records the last time periodic cleanup ran (Unix epoch seconds).
const PERIODIC_MARKER: &str = "cleanup-last-run.txt";

/// Interval between periodic cleanup runs.
const PERIODIC_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60); // 24 hours

// ── Known junk ──────────────────────────────────────────────────────────────

/// Deprecated binary suffixes that the hash-based swap glob doesn't catch.
const DEPRECATED_SUFFIXES: &[&str] = &["-old.exe", "-new.exe", "-swap.exe"];

/// Binary prefixes we manage (only clean deprecated suffixes for these).
const MANAGED_PREFIXES: &[&str] = &["rc-agent", "rc-sentry"];

/// Installer leftovers — explicit allowlist, NOT a glob.
const INSTALLER_JUNK: &[&str] = &[
    "OllamaSetup.exe",
    "rustdesk-setup.exe",
    "MicrosoftEdgeSetup.exe",
];

/// Known-bad HKCU Run key prefixes to remove.
const BLOATWARE_RUN_KEYS: &[&str] = &[
    "MicrosoftEdgeAutoLaunch_",
    "Discord",
    "Teams",
    "OneDrive",
];

/// Log files eligible for rotation (in exe_dir).
const ROTATABLE_LOGS: &[&str] = &[
    "process-guard.log",
    "rc-agent.log",
    "ollama-setup.log",
    "pull.log",
    "rc-bot-events.log",
    "watchdog.log",
];

/// Max log file size before rotation (10 MB).
const LOG_ROTATION_THRESHOLD: u64 = 10 * 1024 * 1024;

/// Max age for crash dumps (7 days).
const CRASH_DUMP_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

// ── Result type ─────────────────────────────────────────────────────────────

/// Structured result from a cleanup run.
#[derive(Debug)]
pub struct CleanupResult {
    pub steps_attempted: u16,
    pub steps_succeeded: u16,
    pub files_deleted: u16,
    pub bytes_reclaimed: u64,
    pub errors: Vec<String>,
}

impl CleanupResult {
    fn new() -> Self {
        Self {
            steps_attempted: 0,
            steps_succeeded: 0,
            files_deleted: 0,
            bytes_reclaimed: 0,
            errors: Vec::new(),
        }
    }

    fn record_ok(&mut self) {
        self.steps_attempted += 1;
        self.steps_succeeded += 1;
    }

    fn record_err(&mut self, step: &str, err: &str) {
        self.steps_attempted += 1;
        let msg = format!("{}: {}", step, err);
        tracing::warn!(target: LOG_TARGET, "{}", msg);
        self.errors.push(msg);
    }

    fn record_file_deleted(&mut self, size: u64) {
        self.files_deleted += 1;
        self.bytes_reclaimed += size;
    }
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Run all cleanup tasks. Always succeeds (fail-open).
///
/// Call after `self_heal::run()` and before starting core services.
pub fn run(exe_dir: &Path) -> CleanupResult {
    let mut r = CleanupResult::new();

    // ── Tier 1: Every startup (fast, idempotent) ────────────────────────

    // Step 1: Delete deprecated binary naming (-old, -new, -swap)
    cleanup_deprecated_binaries(exe_dir, &mut r);

    // Step 2: Delete installer leftovers (explicit allowlist)
    cleanup_installer_junk(exe_dir, &mut r);

    // Step 3: Kill orphan powershell.exe (filtered by command line)
    cleanup_orphan_powershell(&mut r);

    // Step 4: Remove bloatware HKCU Run keys
    cleanup_bloatware_run_keys(&mut r);

    // Step 5: Prune stale staged hash-binaries (keep current + prev + newest staged)
    cleanup_stale_staged_binaries(exe_dir, &mut r);

    // Step 6: Clean temp OTA artifacts (*.part, *.tmp, *.download)
    cleanup_temp_ota_artifacts(exe_dir, &mut r);

    // ── Tier 2: Periodic (every 24h) ────────────────────────────────────

    if should_run_periodic(exe_dir) {
        tracing::info!(target: LOG_TARGET, "Running periodic cleanup (24h interval)");

        // Step 7: Rotate oversized logs
        cleanup_rotate_logs(exe_dir, &mut r);

        // Step 8: Clean old crash dumps (keep newest per exe, delete >7 days)
        cleanup_crash_dumps(&mut r);

        // Step 9: Clean WER artifacts
        cleanup_wer_artifacts(&mut r);

        // Step 10: Clean stale diagnostic .bat scripts
        cleanup_stale_bat_scripts(exe_dir, &mut r);

        // Mark periodic run complete
        update_periodic_marker(exe_dir);
    }

    // ── Summary ─────────────────────────────────────────────────────────

    if r.errors.is_empty() {
        tracing::info!(
            target: LOG_TARGET,
            "cleanup: {}/{} steps ok, {} files deleted, {} bytes reclaimed",
            r.steps_succeeded, r.steps_attempted, r.files_deleted, r.bytes_reclaimed
        );
    } else {
        tracing::warn!(
            target: LOG_TARGET,
            "cleanup: {}/{} steps ok [{} errors], {} files deleted, {} bytes reclaimed",
            r.steps_succeeded, r.steps_attempted, r.errors.len(),
            r.files_deleted, r.bytes_reclaimed
        );
    }

    r
}

// ── Tier 1 Steps ────────────────────────────────────────────────────────────

/// Step 1: Delete rc-agent-old.exe, rc-agent-new.exe, rc-agent-swap.exe, etc.
/// Preserves rc-agent-prev.exe (rollback) and rc-agent-<hash>.exe (staged).
fn cleanup_deprecated_binaries(exe_dir: &Path, r: &mut CleanupResult) {
    for prefix in MANAGED_PREFIXES {
        for suffix in DEPRECATED_SUFFIXES {
            let filename = format!("{}{}", prefix, suffix);
            let path = exe_dir.join(&filename);
            if path.exists() {
                let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                match fs::remove_file(&path) {
                    Ok(()) => {
                        tracing::info!(target: LOG_TARGET, "Deleted deprecated binary: {} ({} bytes)", filename, size);
                        r.record_file_deleted(size);
                    }
                    Err(e) => {
                        r.record_err("deprecated_binaries", &format!("Failed to delete {}: {}", filename, e));
                        return;
                    }
                }
            }
        }
    }
    r.record_ok();
}

/// Step 2: Delete known installer leftovers by explicit name.
fn cleanup_installer_junk(exe_dir: &Path, r: &mut CleanupResult) {
    for name in INSTALLER_JUNK {
        let path = exe_dir.join(name);
        if path.exists() {
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            match fs::remove_file(&path) {
                Ok(()) => {
                    tracing::info!(target: LOG_TARGET, "Deleted installer: {} ({} bytes)", name, size);
                    r.record_file_deleted(size);
                }
                Err(e) => {
                    // Non-fatal — file might be locked by running installer
                    tracing::warn!(target: LOG_TARGET, "Failed to delete {}: {}", name, e);
                }
            }
        }
    }
    r.record_ok();
}

/// Step 3: Kill orphan powershell.exe processes spawned by rc-agent's relaunch_self().
///
/// Filters by command line: only kills PowerShell whose command line contains
/// "rc-agent" or "start-rcagent" (our self-monitor shells). Leaves admin/system
/// PowerShell sessions intact.
fn cleanup_orphan_powershell(r: &mut CleanupResult) {
    // Use PowerShell to query and filter by command line
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        // Get all powershell.exe PIDs whose command line references rc-agent or start-rcagent
        // Also kill those with no command line (orphaned detached shells from DETACHED_PROCESS)
        r#"Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object { $_.CommandLine -match 'rc-agent|start-rcagent|RacingPoint' -or $_.CommandLine -eq '' -or $_.CommandLine -eq $null } | ForEach-Object { try { Stop-Process -Id $_.ProcessId -Force -ErrorAction Stop; Write-Output "killed:$($_.ProcessId)" } catch { Write-Output "skip:$($_.ProcessId)" } }"#,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let killed = stdout.lines().filter(|l| l.starts_with("killed:")).count();
            if killed > 0 {
                tracing::info!(target: LOG_TARGET, "Killed {} orphan powershell processes", killed);
            }
            r.record_ok();
        }
        Err(e) => {
            r.record_err("orphan_powershell", &format!("PowerShell query failed: {}", e));
        }
    }
}

/// Step 4: Remove known bloatware HKCU Run keys (Edge AutoLaunch, Discord, Teams, OneDrive).
fn cleanup_bloatware_run_keys(r: &mut CleanupResult) {
    let run_key_path = r"HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Run";

    // First, enumerate all values in the Run key
    let mut cmd = Command::new("reg");
    cmd.args(["query", run_key_path]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            r.record_err("bloatware_run_keys", &format!("reg query failed: {}", e));
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut removed = 0u32;

    for line in stdout.lines() {
        let trimmed = line.trim();
        // Registry output format: "    ValueName    REG_SZ    Data"
        // Extract the value name (first whitespace-delimited token after leading spaces)
        let parts: Vec<&str> = trimmed.splitn(3, "    ").collect();
        if parts.len() < 2 {
            continue;
        }
        let value_name = parts[0].trim();

        // Check against our denylist (prefix match)
        let is_bloatware = BLOATWARE_RUN_KEYS.iter().any(|prefix| value_name.starts_with(prefix));
        if !is_bloatware {
            continue;
        }

        // Delete this Run key value
        let mut del_cmd = Command::new("reg");
        del_cmd.args(["delete", run_key_path, "/v", value_name, "/f"]);
        #[cfg(windows)]
        del_cmd.creation_flags(CREATE_NO_WINDOW);

        match del_cmd.output() {
            Ok(o) if o.status.success() => {
                tracing::info!(target: LOG_TARGET, "Removed bloatware Run key: {}", value_name);
                removed += 1;
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr);
                tracing::warn!(target: LOG_TARGET, "Failed to remove Run key {}: {}", value_name, stderr.trim());
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "reg delete failed for {}: {}", value_name, e);
            }
        }
    }

    if removed > 0 {
        tracing::info!(target: LOG_TARGET, "Removed {} bloatware Run keys", removed);
    }
    r.record_ok();
}

/// Step 5: Prune stale staged hash-binaries.
///
/// Keeps: rc-agent.exe (current), rc-agent-prev.exe (rollback), and ONE newest
/// rc-agent-<hash>.exe (pending OTA). Deletes any additional staged files.
fn cleanup_stale_staged_binaries(exe_dir: &Path, r: &mut CleanupResult) {
    // Find all rc-agent-????????*.exe files (hash pattern: 8+ hex chars)
    let entries: Vec<(PathBuf, u64, SystemTime)> = match fs::read_dir(exe_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                // Match: rc-agent-<8+hexchars>.exe but NOT rc-agent.exe or rc-agent-prev.exe
                if name.starts_with("rc-agent-")
                    && name.ends_with(".exe")
                    && name != "rc-agent-prev.exe"
                {
                    // Verify it has hex chars (not -old, -new, -swap which are handled in step 1)
                    let middle = &name["rc-agent-".len()..name.len() - 4]; // strip prefix and .exe
                    if middle.len() >= 8 && middle.chars().all(|c| c.is_ascii_hexdigit()) {
                        let meta = e.metadata().ok()?;
                        let modified = meta.modified().ok()?;
                        Some((e.path(), meta.len(), modified))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect(),
        Err(e) => {
            r.record_err("stale_staged", &format!("read_dir failed: {}", e));
            return;
        }
    };

    if entries.len() <= 1 {
        // 0 or 1 staged binary — nothing to prune
        r.record_ok();
        return;
    }

    // Sort by modified time descending — keep the newest one
    let mut sorted = entries;
    sorted.sort_by(|a, b| b.2.cmp(&a.2));

    // Delete all except the newest
    for (path, size, _) in sorted.iter().skip(1) {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        match fs::remove_file(path) {
            Ok(()) => {
                tracing::info!(target: LOG_TARGET, "Pruned stale staged binary: {} ({} bytes)", name, size);
                r.record_file_deleted(*size);
            }
            Err(e) => {
                tracing::warn!(target: LOG_TARGET, "Failed to prune {}: {}", name, e);
            }
        }
    }
    r.record_ok();
}

/// Step 6: Delete temp OTA artifacts (*.part, *.tmp, *.download).
fn cleanup_temp_ota_artifacts(exe_dir: &Path, r: &mut CleanupResult) {
    let extensions = &[".part", ".tmp", ".download"];
    if let Ok(entries) = fs::read_dir(exe_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if extensions.iter().any(|ext| name.ends_with(ext)) {
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                if let Ok(()) = fs::remove_file(entry.path()) {
                    tracing::info!(target: LOG_TARGET, "Deleted OTA temp: {} ({} bytes)", name, size);
                    r.record_file_deleted(size);
                }
            }
        }
    }
    r.record_ok();
}

// ── Tier 2 Steps (Periodic) ────────────────────────────────────────────────

/// Step 7: Rotate logs that exceed threshold. Keeps .1 backup, deletes .2+.
fn cleanup_rotate_logs(exe_dir: &Path, r: &mut CleanupResult) {
    for log_name in ROTATABLE_LOGS {
        let log_path = exe_dir.join(log_name);
        if !log_path.exists() {
            continue;
        }

        let size = match fs::metadata(&log_path) {
            Ok(m) => m.len(),
            Err(_) => continue,
        };

        if size <= LOG_ROTATION_THRESHOLD {
            continue;
        }

        // Rotate: delete .2, rename .1 -> .2, rename current -> .1
        let backup1 = exe_dir.join(format!("{}.1", log_name));
        let backup2 = exe_dir.join(format!("{}.2", log_name));

        let _ = fs::remove_file(&backup2);
        if backup1.exists() {
            let _ = fs::rename(&backup1, &backup2);
        }
        match fs::rename(&log_path, &backup1) {
            Ok(()) => {
                tracing::info!(target: LOG_TARGET, "Rotated {} ({} bytes)", log_name, size);
            }
            Err(e) => {
                // Log file might be locked by another process; non-fatal
                tracing::warn!(target: LOG_TARGET, "Failed to rotate {}: {}", log_name, e);
            }
        }
    }
    r.record_ok();
}

/// Step 8: Clean crash dumps older than 7 days, keeping newest per executable.
fn cleanup_crash_dumps(r: &mut CleanupResult) {
    let crash_dir = match std::env::var("LOCALAPPDATA") {
        Ok(base) => PathBuf::from(base).join("CrashDumps"),
        Err(_) => {
            // Fallback to common path
            PathBuf::from(r"C:\Users\User\AppData\Local\CrashDumps")
        }
    };

    if !crash_dir.exists() {
        r.record_ok();
        return;
    }

    let entries: Vec<_> = match fs::read_dir(&crash_dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(e) => {
            r.record_err("crash_dumps", &format!("read_dir failed: {}", e));
            return;
        }
    };

    let now = SystemTime::now();

    // Group dumps by executable name prefix (e.g., "Variable_dump.exe" -> all its dumps)
    // Keep the newest dump per exe, delete the rest if older than threshold
    let mut newest_per_exe: std::collections::HashMap<String, (PathBuf, SystemTime)> =
        std::collections::HashMap::new();
    let mut all_dumps: Vec<(PathBuf, String, u64, SystemTime)> = Vec::new();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".dmp") {
            continue;
        }

        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let modified = match meta.modified() {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Extract exe name: "Variable_dump.exe.15796.dmp" -> "Variable_dump.exe"
        // Also handles "Variable_dump.exe(1).15796.dmp"
        let exe_name = name
            .split('.')
            .take_while(|part| !part.chars().all(|c| c.is_ascii_digit()))
            .collect::<Vec<_>>()
            .join(".")
            + ".exe";

        // Track newest per exe
        let is_newest = match newest_per_exe.get(&exe_name) {
            Some((_, existing_time)) => modified > *existing_time,
            None => true,
        };
        if is_newest {
            newest_per_exe.insert(exe_name.clone(), (entry.path(), modified));
        }

        all_dumps.push((entry.path(), exe_name, meta.len(), modified));
    }

    // Delete dumps: older than threshold AND not the newest for their exe
    for (path, exe_name, size, modified) in &all_dumps {
        let age = now.duration_since(*modified).unwrap_or_default();
        if age <= CRASH_DUMP_MAX_AGE {
            continue; // Too fresh to delete
        }

        // Keep the newest dump per exe regardless of age
        if let Some((newest_path, _)) = newest_per_exe.get(exe_name) {
            if newest_path == path {
                continue; // This is the newest — keep it
            }
        }

        if let Ok(()) = fs::remove_file(path) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            tracing::info!(target: LOG_TARGET, "Deleted old crash dump: {} ({} bytes)", name, size);
            r.record_file_deleted(*size);
        }
    }
    r.record_ok();
}

/// Step 9: Clean Windows Error Reporting artifacts.
fn cleanup_wer_artifacts(r: &mut CleanupResult) {
    let wer_dir = match std::env::var("LOCALAPPDATA") {
        Ok(base) => PathBuf::from(base).join(r"Microsoft\Windows\WER\ReportQueue"),
        Err(_) => PathBuf::from(r"C:\Users\User\AppData\Local\Microsoft\Windows\WER\ReportQueue"),
    };

    if !wer_dir.exists() {
        r.record_ok();
        return;
    }

    let now = SystemTime::now();

    if let Ok(entries) = fs::read_dir(&wer_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Only delete directories (WER stores reports as folders)
            if !meta.is_dir() {
                continue;
            }

            let modified = match meta.modified() {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Delete reports older than 7 days
            let age = now.duration_since(modified).unwrap_or_default();
            if age > CRASH_DUMP_MAX_AGE {
                if let Ok(()) = fs::remove_dir_all(entry.path()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    tracing::info!(target: LOG_TARGET, "Deleted old WER report: {}", name);
                    r.record_file_deleted(0); // Folder size unknown without recursion
                }
            }
        }
    }
    r.record_ok();
}

/// Step 10: Clean stale diagnostic .bat scripts (one-off debug/check scripts).
///
/// Only deletes scripts matching known diagnostic patterns. Never touches
/// essential scripts (start-rcagent.bat, start-rcsentry.bat, etc.).
fn cleanup_stale_bat_scripts(exe_dir: &Path, r: &mut CleanupResult) {
    /// Scripts that MUST be preserved (essential operations).
    const KEEP_SCRIPTS: &[&str] = &[
        "start-rcagent.bat",
        "start-rcsentry.bat",
        "deploy-update.bat",
        "rcagent-watchdog.bat",
        "edge-harden.bat",
        "swap-agent.bat",
        "swap-sentry.bat",
        "cleanup-mdm.bat",
    ];

    /// Prefixes that indicate one-off diagnostic scripts safe to remove.
    const DIAGNOSTIC_PREFIXES: &[&str] = &[
        "check-",
        "find-",
        "ts-",
        "scan-",
        "setup-",
        "install-",
        "pull-",
        "add-",
        "debug-",
        "diag-",
        "test-",
    ];

    if let Ok(entries) = fs::read_dir(exe_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();

            // Only .bat files
            if !name.ends_with(".bat") {
                continue;
            }

            // Never touch essential scripts
            if KEEP_SCRIPTS.iter().any(|k| name.eq_ignore_ascii_case(k)) {
                continue;
            }

            // Only remove if it matches a diagnostic prefix
            let is_diagnostic = DIAGNOSTIC_PREFIXES
                .iter()
                .any(|prefix| name.to_lowercase().starts_with(prefix));

            if !is_diagnostic {
                continue;
            }

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            if let Ok(()) = fs::remove_file(entry.path()) {
                tracing::info!(target: LOG_TARGET, "Deleted stale script: {} ({} bytes)", name, size);
                r.record_file_deleted(size);
            }
        }
    }
    r.record_ok();
}

// ── Periodic timer ──────────────────────────────────────────────────────────

/// Check if periodic cleanup should run (24h since last run).
fn should_run_periodic(exe_dir: &Path) -> bool {
    let marker_path = exe_dir.join(PERIODIC_MARKER);

    match fs::read_to_string(&marker_path) {
        Ok(content) => {
            let last_run_secs: u64 = content.trim().parse().unwrap_or(0);
            let now_secs = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            now_secs.saturating_sub(last_run_secs) >= PERIODIC_INTERVAL.as_secs()
        }
        Err(_) => true, // No marker = first run, do periodic cleanup
    }
}

/// Update the periodic cleanup marker with current timestamp.
fn update_periodic_marker(exe_dir: &Path) {
    let marker_path = exe_dir.join(PERIODIC_MARKER);
    let now_secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = fs::write(marker_path, now_secs.to_string());
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cleanup_deprecated_binaries() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Create deprecated files
        fs::write(dir_path.join("rc-agent-old.exe"), b"old").unwrap();
        fs::write(dir_path.join("rc-agent-new.exe"), b"new").unwrap();
        fs::write(dir_path.join("rc-sentry-old.exe"), b"old").unwrap();

        // Create files that should NOT be deleted
        fs::write(dir_path.join("rc-agent.exe"), b"current").unwrap();
        fs::write(dir_path.join("rc-agent-prev.exe"), b"prev").unwrap();
        fs::write(dir_path.join("rc-agent-abc12345.exe"), b"staged").unwrap();

        let mut r = CleanupResult::new();
        cleanup_deprecated_binaries(dir_path, &mut r);

        assert!(!dir_path.join("rc-agent-old.exe").exists());
        assert!(!dir_path.join("rc-agent-new.exe").exists());
        assert!(!dir_path.join("rc-sentry-old.exe").exists());

        // These must survive
        assert!(dir_path.join("rc-agent.exe").exists());
        assert!(dir_path.join("rc-agent-prev.exe").exists());
        assert!(dir_path.join("rc-agent-abc12345.exe").exists());
    }

    #[test]
    fn test_cleanup_installer_junk() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::write(dir_path.join("OllamaSetup.exe"), b"big installer").unwrap();
        fs::write(dir_path.join("rustdesk-setup.exe"), b"installer").unwrap();
        fs::write(dir_path.join("rc-agent.exe"), b"keep me").unwrap();

        let mut r = CleanupResult::new();
        cleanup_installer_junk(dir_path, &mut r);

        assert!(!dir_path.join("OllamaSetup.exe").exists());
        assert!(!dir_path.join("rustdesk-setup.exe").exists());
        assert!(dir_path.join("rc-agent.exe").exists());
    }

    #[test]
    fn test_cleanup_stale_staged_binaries() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // Create multiple staged binaries
        fs::write(dir_path.join("rc-agent-aaaaaaaa.exe"), b"oldest").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(dir_path.join("rc-agent-bbbbbbbb.exe"), b"middle").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        fs::write(dir_path.join("rc-agent-cccccccc.exe"), b"newest").unwrap();

        // These should NOT be touched
        fs::write(dir_path.join("rc-agent.exe"), b"current").unwrap();
        fs::write(dir_path.join("rc-agent-prev.exe"), b"rollback").unwrap();

        let mut r = CleanupResult::new();
        cleanup_stale_staged_binaries(dir_path, &mut r);

        // Newest staged should survive
        assert!(dir_path.join("rc-agent-cccccccc.exe").exists());
        // Older staged should be deleted
        assert!(!dir_path.join("rc-agent-aaaaaaaa.exe").exists());
        assert!(!dir_path.join("rc-agent-bbbbbbbb.exe").exists());
        // Current and rollback untouched
        assert!(dir_path.join("rc-agent.exe").exists());
        assert!(dir_path.join("rc-agent-prev.exe").exists());
    }

    #[test]
    fn test_periodic_marker() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        // No marker = should run
        assert!(should_run_periodic(dir_path));

        // After update, should NOT run (just ran)
        update_periodic_marker(dir_path);
        assert!(!should_run_periodic(dir_path));

        // Write an old timestamp (>24h ago)
        let old_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 90_000; // 25 hours ago
        fs::write(dir_path.join(PERIODIC_MARKER), old_ts.to_string()).unwrap();
        assert!(should_run_periodic(dir_path));
    }

    #[test]
    fn test_cleanup_temp_ota_artifacts() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::write(dir_path.join("rc-agent.exe.part"), b"partial").unwrap();
        fs::write(dir_path.join("download.tmp"), b"temp").unwrap();
        fs::write(dir_path.join("binary.download"), b"dl").unwrap();
        fs::write(dir_path.join("rc-agent.exe"), b"keep").unwrap();

        let mut r = CleanupResult::new();
        cleanup_temp_ota_artifacts(dir_path, &mut r);

        assert!(!dir_path.join("rc-agent.exe.part").exists());
        assert!(!dir_path.join("download.tmp").exists());
        assert!(!dir_path.join("binary.download").exists());
        assert!(dir_path.join("rc-agent.exe").exists());
    }
}
