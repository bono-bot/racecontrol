//! Process guard server module.
//!
//! Provides:
//! - `merge_for_machine()`: merges global whitelist + per-machine overrides into MachineWhitelist
//! - `get_whitelist_handler`: GET /api/v1/guard/whitelist/{machine_id}
//!
//! Phase 102 scope: config loading + fetch endpoint only.
//! Phase 103 adds rc-agent guard module.
//! Phase 104 adds violation storage and kiosk notifications.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use std::collections::HashSet;
use std::sync::Arc;

use crate::config::ProcessGuardConfig;
use crate::state::AppState;
use rc_common::types::MachineWhitelist;

/// Returns the machine_type string for a given machine_id.
/// Returns None for unknown / invalid IDs.
///
/// Valid machine IDs: "pod-1" through "pod-8", "james", "server"
pub fn machine_type_for_id(machine_id: &str) -> Option<&'static str> {
    match machine_id {
        "james" => Some("james"),
        "server" => Some("server"),
        id if id.starts_with("pod-") => {
            let num_str = &id[4..]; // everything after "pod-"
            let n: u32 = num_str.parse().ok()?;
            if n >= 1 && n <= 8 {
                Some("pod")
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Merge the global whitelist with per-machine overrides for a specific machine.
///
/// Returns None if machine_id is not a valid Racing Point machine identifier.
///
/// Merge algorithm:
///   1. Include global allowed entries matching machine_type ("all" or exact type match)
///   2. Apply per-machine deny_processes (remove from set)
///   3. Apply per-machine allow_extra_* (add to sets)
///   4. Build and return MachineWhitelist
pub fn merge_for_machine(config: &ProcessGuardConfig, machine_id: &str) -> Option<MachineWhitelist> {
    let machine_type = machine_type_for_id(machine_id)?;

    // Step 1: collect global allowed entries for this machine type
    let mut processes: HashSet<String> = config
        .allowed
        .iter()
        .filter(|entry| {
            entry.machines.is_empty()
                || entry.machines.iter().any(|m| m == "all" || m == machine_type)
        })
        .map(|entry| entry.name.to_lowercase())
        .collect();

    let mut ports: HashSet<u16> = HashSet::new();
    let mut autostart_keys: HashSet<String> = HashSet::new();

    // Step 2 & 3: apply per-machine overrides
    if let Some(override_cfg) = config.overrides.get(machine_type) {
        // Remove denied processes (case-insensitive)
        for deny in &override_cfg.deny_processes {
            processes.remove(&deny.to_lowercase());
        }
        // Add extra allowed processes
        for extra in &override_cfg.allow_extra_processes {
            processes.insert(extra.to_lowercase());
        }
        // Add extra ports
        for &port in &override_cfg.allow_extra_ports {
            ports.insert(port);
        }
        // Add extra autostart keys
        for key in &override_cfg.allow_extra_autostart {
            autostart_keys.insert(key.clone());
        }
    }

    let mut processes_vec: Vec<String> = processes.into_iter().collect();
    processes_vec.sort();
    let mut ports_vec: Vec<u16> = ports.into_iter().collect();
    ports_vec.sort();
    let mut autostart_vec: Vec<String> = autostart_keys.into_iter().collect();
    autostart_vec.sort();

    Some(MachineWhitelist {
        machine_id: machine_id.to_string(),
        processes: processes_vec,
        ports: ports_vec,
        autostart_keys: autostart_vec,
        violation_action: config.violation_action.clone(),
        warn_before_kill: config.warn_before_kill,
    })
}

/// GET /api/v1/guard/whitelist/{machine_id}
///
/// Returns the merged MachineWhitelist for the specified machine.
/// No auth — internal LAN only (consistent with /api/v1/fleet/health).
///
/// machine_id must be one of: "pod-1" through "pod-8", "james", "server"
pub async fn get_whitelist_handler(
    State(state): State<Arc<AppState>>,
    Path(machine_id): Path<String>,
) -> impl IntoResponse {
    match merge_for_machine(&state.config.process_guard, &machine_id) {
        Some(whitelist) => (StatusCode::OK, Json(whitelist)).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": format!(
                    "Unknown machine_id: {}. Valid: pod-1..pod-8, james, server",
                    machine_id
                )
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AllowedProcess, ProcessGuardConfig, ProcessGuardOverride};
    use std::collections::HashMap;

    fn make_test_config() -> ProcessGuardConfig {
        ProcessGuardConfig {
            enabled: true,
            poll_interval_secs: 60,
            violation_action: "report_only".to_string(),
            warn_before_kill: true,
            allowed: vec![
                AllowedProcess {
                    name: "svchost.exe".to_string(),
                    category: "system".to_string(),
                    machines: vec!["all".to_string()],
                },
                AllowedProcess {
                    name: "rc-agent.exe".to_string(),
                    category: "racecontrol".to_string(),
                    machines: vec!["pod".to_string()],
                },
                AllowedProcess {
                    name: "ollama.exe".to_string(),
                    category: "ollama".to_string(),
                    machines: vec!["pod".to_string()],
                },
                AllowedProcess {
                    name: "racecontrol.exe".to_string(),
                    category: "racecontrol".to_string(),
                    machines: vec!["server".to_string()],
                },
                AllowedProcess {
                    name: "steam.exe".to_string(),
                    category: "game".to_string(),
                    machines: vec!["all".to_string()],
                },
            ],
            overrides: {
                let mut m = HashMap::new();
                m.insert(
                    "pod".to_string(),
                    ProcessGuardOverride {
                        allow_extra_processes: vec![],
                        allow_extra_ports: vec![8090],
                        allow_extra_autostart: vec!["RCAgent".to_string()],
                        deny_processes: vec!["steam.exe".to_string()],
                    },
                );
                m.insert(
                    "james".to_string(),
                    ProcessGuardOverride {
                        allow_extra_processes: vec!["ollama.exe".to_string(), "Code.exe".to_string()],
                        allow_extra_ports: vec![11434],
                        allow_extra_autostart: vec![],
                        deny_processes: vec!["rc-agent.exe".to_string()],
                    },
                );
                m
            },
        }
    }

    // Test 1: pod machine includes entries with machines=["all"] and machines=["pod"]
    #[test]
    fn test_merge_pod_includes_all_and_pod_entries() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        // svchost.exe has machines=["all"] — must be present
        assert!(
            result.processes.contains(&"svchost.exe".to_string()),
            "svchost.exe (all) must be in pod result"
        );
        // rc-agent.exe has machines=["pod"] — must be present before deny filter
        // (no pod deny for rc-agent, so it stays)
        assert!(
            result.processes.contains(&"rc-agent.exe".to_string()),
            "rc-agent.exe (pod) must be in pod result"
        );
    }

    // Test 2: pod machine does NOT include entries with machines=["james"] or machines=["server"]
    #[test]
    fn test_merge_pod_excludes_james_and_server_entries() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        // racecontrol.exe has machines=["server"] — must NOT be in pod result
        assert!(
            !result.processes.contains(&"racecontrol.exe".to_string()),
            "racecontrol.exe (server-only) must NOT be in pod result"
        );
    }

    // Test 3: james machine includes entries with machines=["all"] and machines=["james"]
    // but NOT machines=["pod"] or machines=["server"]
    #[test]
    fn test_merge_james_includes_all_and_james_entries_excludes_pod_server() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "james").expect("james should be valid");
        // svchost.exe has machines=["all"] — must be present
        assert!(
            result.processes.contains(&"svchost.exe".to_string()),
            "svchost.exe (all) must be in james result"
        );
        // ollama.exe and Code.exe are in james allow_extra_processes — must be present
        assert!(
            result.processes.contains(&"ollama.exe".to_string()),
            "ollama.exe (james extra) must be in james result"
        );
        assert!(
            result.processes.contains(&"code.exe".to_string()),
            "code.exe (james extra) must be in james result"
        );
        // racecontrol.exe has machines=["server"] — must NOT be in james result
        assert!(
            !result.processes.contains(&"racecontrol.exe".to_string()),
            "racecontrol.exe (server-only) must NOT be in james result"
        );
        // ollama.exe in global has machines=["pod"] — would NOT be included via global,
        // but james override adds it explicitly
    }

    // Test 4: pod result has deny_processes from overrides["pod"] removed
    // (steam.exe is in global allowed=["all"] but denied for pods)
    #[test]
    fn test_merge_pod_deny_processes_removes_steam() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        assert!(
            !result.processes.contains(&"steam.exe".to_string()),
            "steam.exe must be absent from pod result (in deny_processes)"
        );
    }

    // Test 5: james result has allow_extra_processes from overrides["james"] present
    // (ollama.exe in james override)
    #[test]
    fn test_merge_james_allow_extra_processes_present() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "james").expect("james should be valid");
        assert!(
            result.processes.contains(&"ollama.exe".to_string()),
            "ollama.exe must be in james result (james allow_extra_processes)"
        );
        // rc-agent.exe is not in global for james, so deny_processes has no effect
        // (it was machines=["pod"], not james)
        assert!(
            !result.processes.contains(&"rc-agent.exe".to_string()),
            "rc-agent.exe must NOT be in james result (pod-only binary)"
        );
    }

    // Test 6: unknown machine_id returns None
    #[test]
    fn test_merge_unknown_machine_returns_none() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "unknown-machine");
        assert!(result.is_none(), "unknown-machine should return None");
    }

    // Test 7: result.violation_action matches config.process_guard.violation_action
    #[test]
    fn test_merge_violation_action_matches_config() {
        let config = make_test_config();
        let result = merge_for_machine(&config, "pod-3").expect("pod-3 should be valid");
        assert_eq!(
            result.violation_action, "report_only",
            "violation_action must match config value"
        );
        assert!(result.warn_before_kill, "warn_before_kill must match config value");
    }

    // Test 8: machine_type_for_id correctness
    #[test]
    fn test_machine_type_for_id() {
        assert_eq!(machine_type_for_id("pod-7"), Some("pod"));
        assert_eq!(machine_type_for_id("pod-1"), Some("pod"));
        assert_eq!(machine_type_for_id("pod-8"), Some("pod"));
        assert_eq!(machine_type_for_id("james"), Some("james"));
        assert_eq!(machine_type_for_id("server"), Some("server"));
        // Out-of-range pod number
        assert_eq!(machine_type_for_id("pod-99"), None);
        assert_eq!(machine_type_for_id("pod-0"), None);
        // Completely unknown
        assert_eq!(machine_type_for_id("unknown"), None);
        assert_eq!(machine_type_for_id(""), None);
    }
}
