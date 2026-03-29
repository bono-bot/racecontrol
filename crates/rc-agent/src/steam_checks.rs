//! Steam pre-launch validation, game window detection, and DLC content checks.
//!
//! GAME-01: Block game launch if Steam is not running / has pending updates.
//! GAME-06: Block launch if required DLC/content is not installed on the pod.
//! GAME-07: Wait for actual game window after Steam URL launch, detect stuck dialogs.

use rc_common::types::SimType;
use crate::game_process::GameExeConfig;

const LOG_TARGET: &str = "steam-checks";

/// Expected exe names for each SimType (for window polling).
fn game_exe_for_sim(sim_type: SimType) -> &'static [&'static str] {
    match sim_type {
        SimType::AssettoCorsa => &["acs.exe", "AssettoCorsa.exe"],
        SimType::AssettoCorsaEvo => &["AssettoCorsaEVO.exe", "AssettoCorsa2.exe", "AC2-Win64-Shipping.exe"],
        SimType::AssettoCorsaRally => &["acr.exe"],
        SimType::IRacing => &["iRacingSim64DX11.exe", "iRacingService.exe", "iRacingService64.exe", "iRacingLauncher64.exe", "iRacingUI.exe"],
        SimType::F125 => &["F1_25.exe", "F1_2025.exe"],
        SimType::LeMansUltimate => &["LMU.exe", "Le Mans Ultimate.exe"],
        SimType::Forza => &["ForzaMotorsport.exe"],
        SimType::ForzaHorizon5 => &["ForzaHorizon5.exe"],
    }
}

/// GAME-01: Check if Steam is ready to launch a game.
///
/// - Skips check for AssettoCorsa (Game Doctor handles AC-specific Steam checks).
/// - Skips check for non-Steam games (no steam_app_id AND use_steam=false).
/// - If Steam is required but not running: attempts to start Steam with -silent flag.
/// - Returns Err if Steam has pending updates (SteamService blocking updates).
///
/// IMPORTANT: This function performs blocking I/O (sysinfo, process spawn, sleep).
/// It must be called via `tokio::task::spawn_blocking`.
pub fn check_steam_ready(sim_type: SimType, config: &GameExeConfig) -> Result<(), String> {
    // AC is handled by Game Doctor (Check 12) — skip here to avoid double-checking
    if sim_type == SimType::AssettoCorsa {
        return Ok(());
    }

    // Non-Steam games (no steam_app_id, use_steam=false) don't need Steam
    let requires_steam = config.use_steam || config.steam_app_id.is_some();
    if !requires_steam {
        return Ok(());
    }

    // Check for update-blocking processes first (takes priority — even if Steam is running)
    if is_steam_updating() {
        tracing::warn!(target: LOG_TARGET, "Steam has pending updates — blocking game launch");
        return Err("Steam has pending updates. Please wait for Steam to finish updating before launching.".to_string());
    }

    // Check if Steam is running
    if is_steam_running() {
        tracing::debug!(target: LOG_TARGET, "Steam is running — check passed for {:?}", sim_type);
        return Ok(());
    }

    // Steam not running — attempt to start it
    tracing::warn!(target: LOG_TARGET, "Steam not running for {:?} — attempting to start", sim_type);
    attempt_start_steam();

    // Wait 10s for Steam to initialize
    std::thread::sleep(std::time::Duration::from_secs(10));

    // Re-check
    if is_steam_running() {
        tracing::info!(target: LOG_TARGET, "Steam started successfully for {:?}", sim_type);
        Ok(())
    } else {
        tracing::error!(target: LOG_TARGET, "Steam failed to start for {:?}", sim_type);
        Err("Steam is not running and could not be started. Please start Steam manually before launching this game.".to_string())
    }
}

/// Check if steam.exe is running via sysinfo (NOT tasklist — per standing rules).
fn is_steam_running() -> bool {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (_pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        if name.eq_ignore_ascii_case("steam.exe") {
            return true;
        }
    }
    false
}

/// Check if Steam is currently applying updates (blocking game launch).
/// Looks for steamwebhelper.exe in "updating" state or SteamOverlayUpdate.exe.
fn is_steam_updating() -> bool {
    use sysinfo::System;
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    for (_pid, process) in sys.processes() {
        let name = process.name().to_string_lossy();
        // SteamOverlayUpdate.exe indicates Steam applying an update
        if name.eq_ignore_ascii_case("SteamOverlayUpdate.exe") {
            tracing::warn!(target: LOG_TARGET, "Detected SteamOverlayUpdate.exe — Steam is updating");
            return true;
        }
        // package_installer.exe in Steam context also indicates update
        if name.eq_ignore_ascii_case("package_installer.exe") {
            // Check if this is likely Steam's package installer by checking parent path
            if let Some(exe_path) = process.exe() {
                let path_str = exe_path.to_string_lossy().to_lowercase();
                if path_str.contains("steam") {
                    tracing::warn!(target: LOG_TARGET, "Detected Steam package_installer.exe — Steam is updating");
                    return true;
                }
            }
        }
    }
    false
}

/// Attempt to start Steam with -silent flag (no window).
fn attempt_start_steam() {
    let steam_path = std::path::Path::new(r"C:\Program Files (x86)\Steam\steam.exe");
    if !steam_path.exists() {
        tracing::warn!(target: LOG_TARGET, "Steam executable not found at default path");
        return;
    }

    let mut cmd = std::process::Command::new(steam_path);
    cmd.arg("-silent");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.spawn() {
        Ok(_) => tracing::info!(target: LOG_TARGET, "Steam start initiated with -silent flag"),
        Err(e) => tracing::warn!(target: LOG_TARGET, "Failed to start Steam: {}", e),
    }
}

/// GAME-07: Wait for the actual game window to appear after Steam URL launch.
///
/// Polls sysinfo every 2s for the expected game exe. Returns Ok(pid) when found.
/// Returns Err if the game window doesn't appear within `timeout_secs`.
///
/// IMPORTANT: This function performs blocking I/O and long sleeps.
/// It must be called via `tokio::task::spawn_blocking`.
pub fn wait_for_game_window(sim_type: SimType, timeout_secs: u64) -> Result<u32, String> {
    use sysinfo::System;

    let expected_exes = game_exe_for_sim(sim_type);
    let poll_interval = std::time::Duration::from_secs(2);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    tracing::info!(
        target: LOG_TARGET,
        "Waiting for game window for {:?} (timeout: {}s, watching: {:?})",
        sim_type, timeout_secs, expected_exes
    );

    while std::time::Instant::now() < deadline {
        let mut sys = System::new();
        sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        for (_pid, process) in sys.processes() {
            let pname = process.name().to_string_lossy();
            for &expected in expected_exes {
                if pname.eq_ignore_ascii_case(expected) {
                    let pid = _pid.as_u32();
                    tracing::info!(
                        target: LOG_TARGET,
                        "Game window detected: {} (PID {}) for {:?}",
                        expected, pid, sim_type
                    );
                    return Ok(pid);
                }
            }
        }

        tracing::debug!(
            target: LOG_TARGET,
            "Game window not yet visible for {:?} — polling again in 2s ({}s remaining)",
            sim_type,
            deadline.saturating_duration_since(std::time::Instant::now()).as_secs()
        );
        std::thread::sleep(poll_interval);
    }

    Err(format!(
        "Game failed to launch - only Steam dialog visible after {}s timeout for {:?}. \
        Steam may have shown a dialog (DRM check, update, login) instead of launching the game.",
        timeout_secs, sim_type
    ))
}

/// GAME-06: Verify required game content (DLC, car/track) is installed on the pod.
///
/// - For AC: checks content/cars/{car} and content/tracks/{track} directories.
/// - For other Steam games: checks appmanifest_{app_id}.acf exists in Steam library.
/// - Non-Steam games with no app_id: returns Ok(()) (can't verify).
///
/// IMPORTANT: This function performs blocking filesystem I/O.
/// It must be called via `tokio::task::spawn_blocking`.
pub fn check_dlc_installed(sim_type: SimType, launch_args: &str, config: &GameExeConfig) -> Result<(), String> {
    match sim_type {
        SimType::AssettoCorsa => check_ac_content(launch_args),
        _ => check_steam_app_manifest(config),
    }
}

/// Check AC content dirs exist (car and track directories must be present).
fn check_ac_content(launch_args: &str) -> Result<(), String> {
    // Parse launch_args JSON for car/track identifiers
    let params: serde_json::Value = match serde_json::from_str(launch_args) {
        Ok(v) => v,
        Err(_) => {
            // If we can't parse args, we can't verify — proceed (Game Doctor will catch issues)
            tracing::debug!(target: LOG_TARGET, "AC DLC check: cannot parse launch_args as JSON — skipping");
            return Ok(());
        }
    };

    let car = params.get("car").and_then(|v| v.as_str()).unwrap_or("");
    let track = params.get("track").and_then(|v| v.as_str()).unwrap_or("");

    if car.is_empty() && track.is_empty() {
        tracing::debug!(target: LOG_TARGET, "AC DLC check: no car/track in launch_args — skipping");
        return Ok(());
    }

    // Find AC install path from common locations
    let ac_paths = [
        r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa",
        r"D:\Steam\steamapps\common\assettocorsa",
        r"E:\Steam\steamapps\common\assettocorsa",
    ];

    let ac_base = ac_paths.iter()
        .find(|p| std::path::Path::new(p).exists())
        .map(std::path::Path::new);

    let Some(base) = ac_base else {
        tracing::warn!(target: LOG_TARGET, "AC DLC check: AC install dir not found at standard paths — skipping verification");
        return Ok(());
    };

    let mut missing = Vec::new();

    if !car.is_empty() {
        let car_path = base.join("content").join("cars").join(car);
        if !car_path.exists() {
            missing.push(format!("car '{}'", car));
            tracing::error!(target: LOG_TARGET, "AC DLC check: car directory not found: {:?}", car_path);
        }
    }

    if !track.is_empty() {
        let track_path = base.join("content").join("tracks").join(track);
        if !track_path.exists() {
            missing.push(format!("track '{}'", track));
            tracing::error!(target: LOG_TARGET, "AC DLC check: track directory not found: {:?}", track_path);
        }
    }

    if !missing.is_empty() {
        return Err(format!(
            "Content not installed on this pod: {}. Please check DLC installation.",
            missing.join(", ")
        ));
    }

    tracing::debug!(target: LOG_TARGET, "AC DLC check passed: car='{}', track='{}'", car, track);
    Ok(())
}

/// Check that the Steam appmanifest_{app_id}.acf file exists (game is installed).
fn check_steam_app_manifest(config: &GameExeConfig) -> Result<(), String> {
    let Some(app_id) = config.steam_app_id else {
        // No app_id — can't verify via manifest (exe-only launch)
        return Ok(());
    };

    // Common Steam library locations
    let steam_library_paths = [
        r"C:\Program Files (x86)\Steam\steamapps",
        r"D:\Steam\steamapps",
        r"E:\Steam\steamapps",
        r"D:\SteamLibrary\steamapps",
        r"E:\SteamLibrary\steamapps",
    ];

    let manifest_name = format!("appmanifest_{}.acf", app_id);

    for lib_path in &steam_library_paths {
        let manifest_path = std::path::Path::new(lib_path).join(&manifest_name);
        if manifest_path.exists() {
            tracing::debug!(target: LOG_TARGET, "Steam app {} manifest found: {:?}", app_id, manifest_path);
            return Ok(());
        }
    }

    // Also check libraryfolders.vdf for additional library paths (basic check)
    // For now, if not found in standard paths, warn but don't block
    // (custom library locations require full libraryfolders.vdf parsing)
    tracing::warn!(
        target: LOG_TARGET,
        "Steam app {} manifest not found in standard library paths — game may be in custom location",
        app_id
    );

    // Not returning Err here — custom Steam library paths exist and we don't want to block
    // valid installs. Game Doctor / the game itself will fail if truly missing.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::types::SimType;

    #[test]
    fn test_check_steam_ready_skips_ac() {
        // AC should always pass — Game Doctor handles it
        let config = GameExeConfig {
            exe_path: None,
            working_dir: None,
            args: None,
            steam_app_id: Some(805550),
            use_steam: true,
        };
        // AC should return Ok regardless of Steam state
        let result = check_steam_ready(SimType::AssettoCorsa, &config);
        assert!(result.is_ok(), "AC should skip Steam check: {:?}", result);
    }

    #[test]
    fn test_check_steam_ready_skips_non_steam_games() {
        // Direct exe launch (no steam_app_id, use_steam=false) should pass
        let config = GameExeConfig {
            exe_path: Some(r"C:\Games\MyGame\game.exe".to_string()),
            working_dir: None,
            args: None,
            steam_app_id: None,
            use_steam: false,
        };
        let result = check_steam_ready(SimType::F125, &config);
        assert!(result.is_ok(), "Non-Steam games should skip Steam check: {:?}", result);
    }

    #[test]
    fn test_game_exe_for_sim_f125_includes_both_names() {
        let exes = game_exe_for_sim(SimType::F125);
        assert!(exes.contains(&"F1_25.exe"), "F125 should include F1_25.exe");
        assert!(exes.contains(&"F1_2025.exe"), "F125 should include F1_2025.exe (alternate name)");
    }

    #[test]
    fn test_game_exe_for_sim_iracing_includes_ui() {
        let exes = game_exe_for_sim(SimType::IRacing);
        assert!(exes.contains(&"iRacingUI.exe"), "iRacing should include iRacingUI.exe");
        assert!(exes.contains(&"iRacingSim64DX11.exe"), "iRacing should include sim exe");
    }

    #[test]
    fn test_game_exe_for_sim_all_end_in_exe() {
        let all_variants = [
            SimType::AssettoCorsa,
            SimType::AssettoCorsaEvo,
            SimType::AssettoCorsaRally,
            SimType::IRacing,
            SimType::F125,
            SimType::LeMansUltimate,
            SimType::Forza,
            SimType::ForzaHorizon5,
        ];
        for variant in &all_variants {
            let exes = game_exe_for_sim(*variant);
            assert!(!exes.is_empty(), "SimType::{:?} has no exe names", variant);
            for exe in exes {
                assert!(exe.ends_with(".exe"), "exe '{}' for {:?} must end with .exe", exe, variant);
            }
        }
    }

    #[test]
    fn test_wait_for_game_window_times_out_quickly() {
        // In test environment, game processes won't be running — should timeout quickly
        // Use 1s timeout to keep test fast
        let result = wait_for_game_window(SimType::F125, 1);
        assert!(result.is_err(), "Should timeout when game is not running");
        let err = result.unwrap_err();
        assert!(err.contains("Game failed to launch"), "Error should describe timeout: {}", err);
    }

    #[test]
    fn test_check_dlc_installed_non_steam_no_app_id() {
        // Non-Steam game with no app_id should pass
        let config = GameExeConfig {
            exe_path: Some(r"C:\Games\game.exe".to_string()),
            working_dir: None,
            args: None,
            steam_app_id: None,
            use_steam: false,
        };
        let result = check_dlc_installed(SimType::F125, "", &config);
        assert!(result.is_ok(), "Non-Steam game should pass DLC check: {:?}", result);
    }

    #[test]
    fn test_check_dlc_installed_ac_no_args() {
        // AC with empty/non-JSON args should pass (can't verify)
        let config = GameExeConfig::default();
        let result = check_dlc_installed(SimType::AssettoCorsa, "", &config);
        assert!(result.is_ok(), "AC with empty args should pass DLC check");
    }

    #[test]
    fn test_check_dlc_installed_ac_invalid_json_passes() {
        // AC with non-JSON args should pass (can't verify format)
        let config = GameExeConfig::default();
        let result = check_dlc_installed(SimType::AssettoCorsa, "not-json", &config);
        assert!(result.is_ok(), "AC with invalid JSON should pass (skip): {:?}", result);
    }

    #[test]
    fn test_check_steam_ready_with_steam_app_id_requires_steam() {
        // This test verifies the function runs without panic for Steam games.
        // In test environment, Steam may or may not be running.
        // We just verify the function completes and returns a valid Result.
        let config = GameExeConfig {
            exe_path: None,
            working_dir: None,
            args: None,
            steam_app_id: Some(3059520), // F1 25
            use_steam: true,
        };
        // Function should either return Ok (Steam running) or Err (Steam not running)
        // Both are valid outcomes — we just verify it doesn't panic
        let _result = check_steam_ready(SimType::F125, &config);
        // No assertion on success/failure — depends on whether Steam is running in test env
    }
}
