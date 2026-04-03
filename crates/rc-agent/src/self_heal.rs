//! Startup self-healing -- verifies and repairs config, start script, and registry key on every boot.
//!
//! Runs synchronously before load_config(). Non-fatal: if any repair fails, logs a warning
//! and continues. Never panics.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

const LOG_TARGET: &str = "self-heal";

/// Embedded config template -- compiled into the binary from deploy/rc-agent.template.toml.
const CONFIG_TEMPLATE: &str = include_str!("../../../deploy/rc-agent.template.toml");

/// Full start-rcagent.bat content with CRLF line endings.
/// v27.0: Unified rich version — bloatware kills, power settings, deprecated binary cleanup,
/// hash-based swap, and stderr redirect. Single source of truth for all pods.
const START_SCRIPT_CONTENT: &str = "\
@echo off\r\n\
cd /d C:\\RacingPoint\r\n\
set RUST_BACKTRACE=1\r\n\
\r\n\
rem --- Enforce power settings (prevents ConspitLink flicker regression) ---\r\n\
powercfg /setactive 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c 1>nul 2>nul\r\n\
powercfg /SETACVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul\r\n\
powercfg /SETDCVALUEINDEX SCHEME_CURRENT 2a737441-1930-4402-8d77-b2bebba308a3 48e6b7a6-50f5-4782-a5d4-53bb8f07e226 0 1>nul 2>nul\r\n\
powercfg /SETACTIVE SCHEME_CURRENT 1>nul 2>nul\r\n\
\r\n\
rem --- Firewall rule ---\r\n\
netsh advfirewall firewall add rule name=\"RCAgent\" dir=in action=allow protocol=TCP localport=8090 1>nul 2>nul\r\n\
\r\n\
rem --- NTP enforcement (sync to venue server, v3.6 permanent fix) ---\r\n\
sc config w32time start= auto 1>nul 2>nul\r\n\
net start w32time 1>nul 2>nul\r\n\
w32tm /config /manualpeerlist:\"192.168.31.23\" /syncfromflags:manual /update 1>nul 2>nul\r\n\
w32tm /resync /nowait 1>nul 2>nul\r\n\
\r\n\
rem --- Kill bloatware and prevent process multiplication ---\r\n\
taskkill /F /IM Variable_dump.exe 1>nul 2>nul\r\n\
taskkill /F /IM \"Creative Cloud UI Helper.exe\" 1>nul 2>nul\r\n\
taskkill /F /IM M365Copilot.exe 1>nul 2>nul\r\n\
taskkill /F /IM Copilot.exe 1>nul 2>nul\r\n\
taskkill /F /IM ClockifyWindows.exe 1>nul 2>nul\r\n\
taskkill /F /IM OneDrive.exe 1>nul 2>nul\r\n\
taskkill /F /IM powershell.exe 1>nul 2>nul\r\n\
taskkill /F /IM ConspitLink2.0.exe 1>nul 2>nul\r\n\
taskkill /F /IM rc-agent.exe 1>nul 2>nul\r\n\
timeout /t 3 /nobreak 1>nul\r\n\
\r\n\
rem --- Clean deprecated binary naming (pre-hash era) ---\r\n\
del /Q rc-agent-old.exe 1>nul 2>nul\r\n\
del /Q rc-agent-new.exe 1>nul 2>nul\r\n\
del /Q rc-agent-swap.exe 1>nul 2>nul\r\n\
del /Q rc-sentry-old.exe 1>nul 2>nul\r\n\
del /Q rc-sentry-new.exe 1>nul 2>nul\r\n\
\r\n\
rem --- Binary swap (hash-based versioning) ---\r\n\
set \"STAGED=\"\r\n\
for /f \"delims=\" %%F in ('dir /B /O-D rc-agent-????????*.exe 2^^^>nul') do (\r\n\
    if not \"%%F\"==\"rc-agent.exe\" (\r\n\
        if not defined STAGED set \"STAGED=%%F\"\r\n\
    )\r\n\
)\r\n\
if not defined STAGED goto :start_agent\r\n\
del /Q rc-agent-prev.exe 1>nul 2>nul\r\n\
if exist rc-agent.exe ren rc-agent.exe rc-agent-prev.exe 1>nul 2>nul\r\n\
timeout /t 1 /nobreak 1>nul\r\n\
if exist rc-agent.exe del /Q rc-agent.exe 1>nul 2>nul\r\n\
ren \"%STAGED%\" rc-agent.exe 1>nul\r\n\
:start_agent\r\n\
start \"\" /D C:\\RacingPoint rc-agent.exe 2>> rc-agent-stderr.log\r\n";

/// Result of the self-heal check-and-repair cycle.
#[derive(Debug)]
pub struct SelfHealResult {
    pub config_repaired: bool,
    pub script_repaired: bool,
    pub registry_repaired: bool,
    pub defender_repaired: bool,
    pub errors: Vec<String>,
}

/// Run all self-heal checks. Returns what was repaired (if anything).
///
/// Checks (in order):
/// 1. `exe_dir/rc-agent.toml` exists -- regenerate from template if missing.
/// 2. `exe_dir/start-rcagent.bat` exists -- recreate with CRLF if missing.
/// 3. `HKLM\...\Run\RCAgent` registry key exists -- recreate if missing.
///
/// Each repair is non-fatal: failures are collected in `errors`.
pub fn run(exe_dir: &Path) -> SelfHealResult {
    let mut result = SelfHealResult {
        config_repaired: false,
        script_repaired: false,
        registry_repaired: false,
        defender_repaired: false,
        errors: Vec::new(),
    };

    // 1. Config
    let config_path = exe_dir.join("rc-agent.toml");
    if !config_path.exists() {
        tracing::warn!(target: LOG_TARGET, "rc-agent.toml missing -- attempting repair");
        match repair_config(&config_path) {
            Ok(()) => {
                tracing::warn!(target: LOG_TARGET, "rc-agent.toml regenerated from template");
                result.config_repaired = true;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to repair config: {}", e);
                result.errors.push(format!("config: {}", e));
            }
        }
    }

    // 2. Start script — overwrite if missing OR drifted from embedded template.
    //    Single source of truth: the embedded START_SCRIPT_CONTENT is canonical.
    let script_path = exe_dir.join("start-rcagent.bat");
    let script_needs_repair = if !script_path.exists() {
        tracing::warn!(target: LOG_TARGET, "start-rcagent.bat missing");
        true
    } else {
        // Compare on-disk content with embedded template
        match fs::read(&script_path) {
            Ok(on_disk) => on_disk != START_SCRIPT_CONTENT.as_bytes(),
            Err(_) => true,
        }
    };
    if script_needs_repair {
        tracing::warn!(target: LOG_TARGET, "start-rcagent.bat needs update -- writing canonical version");
        match repair_start_script(&script_path) {
            Ok(()) => {
                tracing::warn!(target: LOG_TARGET, "start-rcagent.bat updated to canonical version");
                result.script_repaired = true;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to repair start script: {}", e);
                result.errors.push(format!("script: {}", e));
            }
        }
    }

    // 3. Registry key
    if !registry_key_exists() {
        tracing::warn!(target: LOG_TARGET, "HKLM Run key missing -- attempting repair");
        match repair_registry_key(exe_dir) {
            Ok(()) => {
                tracing::warn!(target: LOG_TARGET, "HKLM Run key recreated");
                result.registry_repaired = true;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to repair registry key: {}", e);
                result.errors.push(format!("registry: {}", e));
            }
        }
    }

    // 4. Defender exclusion for C:\RacingPoint\
    if !defender_exclusion_exists() {
        tracing::warn!(target: LOG_TARGET, "Defender exclusion for C:\\RacingPoint\\ missing -- attempting repair");
        match repair_defender_exclusion() {
            Ok(()) => {
                tracing::warn!(target: LOG_TARGET, "Defender exclusion added for C:\\RacingPoint\\");
                result.defender_repaired = true;
            }
            Err(e) => {
                tracing::error!(target: LOG_TARGET, "Failed to add Defender exclusion: {}", e);
                result.errors.push(format!("defender: {}", e));
            }
        }
    }

    result
}

/// Parse pod number from a hostname string.
///
/// Accepts patterns like "Pod-1", "POD-3", "pod8" (case-insensitive prefix "pod",
/// optional separator, then a digit 1-8).
pub fn detect_pod_number_from(hostname: &str) -> Result<u32> {
    let lower = hostname.to_lowercase();
    if !lower.starts_with("pod") {
        bail!("Hostname '{}' does not start with 'pod'", hostname);
    }

    // Skip "pod" prefix, then skip any non-digit separators (e.g., "-")
    let after_pod = &lower[3..];
    let digit_str: String = after_pod.chars().filter(|c| c.is_ascii_digit()).collect();
    if digit_str.is_empty() {
        bail!("Hostname '{}' contains no digit after 'pod'", hostname);
    }

    let num: u32 = digit_str.parse().map_err(|e| {
        anyhow::anyhow!("Hostname '{}' has invalid number '{}': {}", hostname, digit_str, e)
    })?;

    if !(1..=8).contains(&num) {
        bail!(
            "Pod number {} from hostname '{}' is out of range (must be 1-8)",
            num,
            hostname
        );
    }

    Ok(num)
}

/// Detect pod number from the COMPUTERNAME environment variable.
fn detect_pod_number() -> Result<u32> {
    let hostname = std::env::var("COMPUTERNAME")
        .map_err(|_| anyhow::anyhow!("COMPUTERNAME environment variable not set"))?;
    detect_pod_number_from(&hostname)
}

/// Regenerate rc-agent.toml from the embedded template.
fn repair_config(config_path: &Path) -> Result<()> {
    let pod_num = detect_pod_number()?;
    let pod_name = format!("Pod {}", pod_num);

    let content = CONFIG_TEMPLATE
        .replace("{pod_number}", &pod_num.to_string())
        .replace("{pod_name}", &pod_name);

    // Validate the generated TOML parses correctly before writing
    content.parse::<toml::Value>().map_err(|e| {
        anyhow::anyhow!("Generated config is invalid TOML: {}", e)
    })?;

    fs::write(config_path, &content)?;
    Ok(())
}

/// Regenerate rc-agent.toml from template using a specific pod number (for testing).
#[cfg(test)]
fn repair_config_for_pod(config_path: &Path, pod_num: u32) -> Result<()> {
    let pod_name = format!("Pod {}", pod_num);
    let content = CONFIG_TEMPLATE
        .replace("{pod_number}", &pod_num.to_string())
        .replace("{pod_name}", &pod_name);

    content.parse::<toml::Value>().map_err(|e| {
        anyhow::anyhow!("Generated config is invalid TOML: {}", e)
    })?;

    fs::write(config_path, &content)?;
    Ok(())
}

/// Write start-rcagent.bat with CRLF line endings.
fn repair_start_script(script_path: &Path) -> Result<()> {
    fs::write(script_path, START_SCRIPT_CONTENT)?;
    Ok(())
}

/// Check if the HKLM Run key for RCAgent exists.
fn registry_key_exists() -> bool {
    let mut cmd = Command::new("reg");
    cmd.args([
        "query",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "RCAgent",
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Create the HKLM Run key pointing to start-rcagent.bat in exe_dir.
fn repair_registry_key(exe_dir: &Path) -> Result<()> {
    let bat_path = exe_dir.join("start-rcagent.bat");
    let data = bat_path.to_string_lossy().to_string();

    let mut cmd = Command::new("reg");
    cmd.args([
        "add",
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Run",
        "/v",
        "RCAgent",
        "/d",
        &data,
        "/f",
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| {
        anyhow::anyhow!("Failed to spawn reg.exe: {}", e)
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("reg add failed: {}", stderr.trim());
    }

    Ok(())
}

/// Check if C:\RacingPoint\ is in Windows Defender ExclusionPath.
/// Returns false if PowerShell is unavailable or the check fails (non-fatal).
fn defender_exclusion_exists() -> bool {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        r#"(Get-MpPreference).ExclusionPath -contains 'C:\RacingPoint'"#,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            stdout.trim() == "True"
        }
        Err(_) => false,
    }
}

/// Add C:\RacingPoint\ to Windows Defender exclusion paths.
/// Requires admin privileges (rc-agent runs as admin user on pod PCs).
fn repair_defender_exclusion() -> Result<()> {
    let mut cmd = Command::new("powershell");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        r#"Add-MpPreference -ExclusionPath 'C:\RacingPoint'"#,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd.output().map_err(|e| anyhow::anyhow!("powershell failed: {}", e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Add-MpPreference failed: {}", stderr.trim());
    }
    Ok(())
}

/// Compute a deterministic hash of the config file contents.
///
/// Returns hex string of a u64 hash, or "unknown" if the file cannot be read.
pub fn config_hash(config_path: &Path) -> String {
    match fs::read(config_path) {
        Ok(bytes) => {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            bytes.hash(&mut hasher);
            format!("{:016x}", hasher.finish())
        }
        Err(_) => "unknown".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_pod_number_valid() {
        assert_eq!(detect_pod_number_from("Pod-1").unwrap(), 1);
        assert_eq!(detect_pod_number_from("Pod-8").unwrap(), 8);
        assert_eq!(detect_pod_number_from("POD-3").unwrap(), 3);
        assert_eq!(detect_pod_number_from("pod1").unwrap(), 1);
    }

    #[test]
    fn test_detect_pod_number_invalid() {
        assert!(detect_pod_number_from("Pod-0").is_err());
        assert!(detect_pod_number_from("Pod-9").is_err());
        assert!(detect_pod_number_from("Pod-10").is_err());
        assert!(detect_pod_number_from("DESKTOP-ABC").is_err());
        assert!(detect_pod_number_from("").is_err());
    }

    #[test]
    fn test_repair_config_generates_valid_toml() {
        let dir = tempdir().unwrap();
        let config_path = dir.path().join("rc-agent.toml");
        repair_config_for_pod(&config_path, 3).unwrap();

        let content = fs::read_to_string(&config_path).unwrap();
        // Must parse as valid TOML
        let parsed: toml::Value = content.parse().unwrap();
        // Check pod section
        let pod = parsed.get("pod").expect("missing [pod] section");
        assert_eq!(pod.get("number").unwrap().as_integer().unwrap(), 3);
        assert_eq!(pod.get("name").unwrap().as_str().unwrap(), "Pod 3");
        // Check core section
        let core = parsed.get("core").expect("missing [core] section");
        assert!(core.get("url").unwrap().as_str().unwrap().contains("ws://"));
    }

    #[test]
    fn test_repair_start_script_crlf() {
        let dir = tempdir().unwrap();
        let script_path = dir.path().join("start-rcagent.bat");
        repair_start_script(&script_path).unwrap();

        let content = fs::read_to_string(&script_path).unwrap();
        assert!(content.contains("\r\n"), "Script must have CRLF line endings");
        assert!(content.contains("@echo off"), "Script must contain @echo off");
        assert!(content.contains("start"), "Script must contain start command");
        assert!(
            content.contains("cd /d C:\\RacingPoint"),
            "Script must cd to C:\\RacingPoint"
        );
    }

    #[test]
    fn test_self_heal_result_default() {
        let result = SelfHealResult {
            config_repaired: false,
            script_repaired: false,
            registry_repaired: false,
            defender_repaired: false,
            errors: Vec::new(),
        };
        assert!(!result.config_repaired);
        assert!(!result.script_repaired);
        assert!(!result.registry_repaired);
        assert!(!result.defender_repaired);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_defender_repaired_field_exists() {
        // Compile-time check: SelfHealResult must have defender_repaired field
        let result = SelfHealResult {
            config_repaired: false,
            script_repaired: false,
            registry_repaired: false,
            defender_repaired: false,
            errors: Vec::new(),
        };
        // run() must initialize defender_repaired to false
        assert!(!result.defender_repaired);
    }

    #[test]
    fn test_config_hash_deterministic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.toml");
        fs::write(&path, "hello world").unwrap();

        let hash1 = config_hash(&path);
        let hash2 = config_hash(&path);
        assert_eq!(hash1, hash2, "Same file must produce same hash");

        // Different content produces different hash
        let path2 = dir.path().join("test2.toml");
        fs::write(&path2, "different content").unwrap();
        let hash3 = config_hash(&path2);
        assert_ne!(hash1, hash3, "Different files must produce different hashes");
    }

    #[test]
    fn test_config_hash_missing_file() {
        let hash = config_hash(Path::new("/nonexistent/file.toml"));
        assert_eq!(hash, "unknown");
    }

    #[test]
    fn test_no_repair_when_exists_and_canonical() {
        let dir = tempdir().unwrap();
        // Create config (any valid content — self_heal only checks existence)
        fs::write(dir.path().join("rc-agent.toml"), "[pod]\nnumber = 1").unwrap();
        // Create script with the EXACT canonical content (drift detection must see no diff)
        fs::write(dir.path().join("start-rcagent.bat"), START_SCRIPT_CONTENT).unwrap();

        let result = run(dir.path());
        assert!(!result.config_repaired, "Should not repair existing config");
        assert!(!result.script_repaired, "Should not repair canonical script");
        // registry_repaired depends on the actual system state, so we don't assert it here
    }

    #[test]
    fn test_repairs_drifted_script() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("rc-agent.toml"), "[pod]\nnumber = 1").unwrap();
        // Write a DRIFTED script (different from canonical)
        fs::write(dir.path().join("start-rcagent.bat"), "@echo off\r\nold version\r\n").unwrap();

        let result = run(dir.path());
        assert!(!result.config_repaired, "Should not repair existing config");
        assert!(result.script_repaired, "Should repair drifted script");

        // Verify the script was overwritten with canonical content
        let content = fs::read(dir.path().join("start-rcagent.bat")).unwrap();
        assert_eq!(content, START_SCRIPT_CONTENT.as_bytes());
    }
}
