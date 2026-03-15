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

/// Embedded config template -- compiled into the binary from deploy/rc-agent.template.toml.
const CONFIG_TEMPLATE: &str = include_str!("../../../deploy/rc-agent.template.toml");

/// Full start-rcagent.bat content with CRLF line endings.
const START_SCRIPT_CONTENT: &str = "@echo off\r\n\
    cd /d C:\\RacingPoint\r\n\
    netsh advfirewall firewall add rule name=\"RCAgent\" dir=in action=allow protocol=TCP localport=8090 1>nul 2>nul\r\n\
    taskkill /F /IM rc-agent.exe 1>nul 2>nul\r\n\
    timeout /t 3 /nobreak 1>nul\r\n\
    if exist rc-agent-new.exe (\r\n\
        del /Q rc-agent.exe 1>nul 2>nul\r\n\
        timeout /t 1 /nobreak 1>nul\r\n\
        if exist rc-agent.exe del /Q rc-agent.exe 1>nul 2>nul\r\n\
        move rc-agent-new.exe rc-agent.exe 1>nul\r\n\
    )\r\n\
    start \"\" /D C:\\RacingPoint rc-agent.exe\r\n";

/// Result of the self-heal check-and-repair cycle.
#[derive(Debug)]
pub struct SelfHealResult {
    pub config_repaired: bool,
    pub script_repaired: bool,
    pub registry_repaired: bool,
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
        errors: Vec::new(),
    };

    // 1. Config
    let config_path = exe_dir.join("rc-agent.toml");
    if !config_path.exists() {
        tracing::warn!("[self-heal] rc-agent.toml missing -- attempting repair");
        match repair_config(&config_path) {
            Ok(()) => {
                tracing::warn!("[self-heal] rc-agent.toml regenerated from template");
                result.config_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair config: {}", e);
                result.errors.push(format!("config: {}", e));
            }
        }
    }

    // 2. Start script
    let script_path = exe_dir.join("start-rcagent.bat");
    if !script_path.exists() {
        tracing::warn!("[self-heal] start-rcagent.bat missing -- attempting repair");
        match repair_start_script(&script_path) {
            Ok(()) => {
                tracing::warn!("[self-heal] start-rcagent.bat recreated");
                result.script_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair start script: {}", e);
                result.errors.push(format!("script: {}", e));
            }
        }
    }

    // 3. Registry key
    if !registry_key_exists() {
        tracing::warn!("[self-heal] HKLM Run key missing -- attempting repair");
        match repair_registry_key(exe_dir) {
            Ok(()) => {
                tracing::warn!("[self-heal] HKLM Run key recreated");
                result.registry_repaired = true;
            }
            Err(e) => {
                tracing::error!("[self-heal] Failed to repair registry key: {}", e);
                result.errors.push(format!("registry: {}", e));
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
            errors: Vec::new(),
        };
        assert!(!result.config_repaired);
        assert!(!result.script_repaired);
        assert!(!result.registry_repaired);
        assert!(result.errors.is_empty());
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
    fn test_no_repair_when_exists() {
        let dir = tempdir().unwrap();
        // Create both files so they already exist
        fs::write(dir.path().join("rc-agent.toml"), "[pod]\nnumber = 1").unwrap();
        fs::write(dir.path().join("start-rcagent.bat"), "@echo off").unwrap();

        let result = run(dir.path());
        assert!(!result.config_repaired, "Should not repair existing config");
        assert!(!result.script_repaired, "Should not repair existing script");
        // registry_repaired depends on the actual system state, so we don't assert it here
    }
}
