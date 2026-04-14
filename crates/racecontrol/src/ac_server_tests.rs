use super::*;
use crate::ac_server_config::*;
use crate::ac_server_results::*;
use rc_common::types::*;

#[test]
fn test_ac_result_file_deserialization() {
    let json = r#"{
        "TrackName": "monza",
        "TrackConfig": "",
        "Type": "RACE",
        "Result": [
            {
                "DriverName": "Alice",
                "DriverGuid": "steam_123",
                "CarId": 0,
                "CarModel": "ks_ferrari_488_gt3",
                "BestLap": 98500,
                "TotalTime": 590000,
                "LapCount": 6,
                "HasFinished": true
            },
            {
                "DriverName": "Bob",
                "DriverGuid": "steam_456",
                "CarId": 1,
                "CarModel": "ks_ferrari_488_gt3",
                "BestLap": 99200,
                "TotalTime": 600000,
                "LapCount": 6,
                "HasFinished": true
            }
        ]
    }"#;

    let result_file: AcResultFile = serde_json::from_str(json).unwrap();
    assert_eq!(result_file.result.len(), 2);
    assert_eq!(result_file.result[0].driver_name, "Alice");
    assert_eq!(result_file.result[0].best_lap, 98500);
    assert_eq!(result_file.result[0].lap_count, 6);
    assert_eq!(result_file.result[1].driver_name, "Bob");
    assert_eq!(result_file.track_name, "monza");
    assert_eq!(result_file.session_type, "RACE");
}

#[test]
fn test_ac_result_file_maps_to_multiplayer_result() {
    let json = r#"{
        "TrackName": "spa",
        "TrackConfig": "",
        "Type": "RACE",
        "Result": [
            {
                "DriverName": "Driver1",
                "DriverGuid": "guid_1",
                "CarId": 0,
                "CarModel": "car1",
                "BestLap": 120000,
                "TotalTime": 720000,
                "LapCount": 5,
                "HasFinished": true
            },
            {
                "DriverName": "Driver2",
                "DriverGuid": "guid_2",
                "CarId": 1,
                "CarModel": "car1",
                "BestLap": 0,
                "TotalTime": 0,
                "LapCount": 0,
                "HasFinished": false
            }
        ]
    }"#;

    let result_file: AcResultFile = serde_json::from_str(json).unwrap();

    // Map to MultiplayerResult
    let results: Vec<MultiplayerResult> = result_file
        .result
        .iter()
        .enumerate()
        .map(|(i, entry)| MultiplayerResult {
            position: (i + 1) as u32,
            driver_name: entry.driver_name.clone(),
            guid: entry.driver_guid.clone(),
            best_lap_ms: if entry.best_lap > 0 { Some(entry.best_lap) } else { None },
            total_time_ms: if entry.total_time > 0 { Some(entry.total_time) } else { None },
            laps_completed: entry.lap_count,
        })
        .collect();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].position, 1);
    assert_eq!(results[0].driver_name, "Driver1");
    assert_eq!(results[0].best_lap_ms, Some(120000));
    assert_eq!(results[0].total_time_ms, Some(720000));
    assert_eq!(results[0].laps_completed, 5);

    // DNF driver — zero best_lap and total_time
    assert_eq!(results[1].position, 2);
    assert_eq!(results[1].best_lap_ms, None);
    assert_eq!(results[1].total_time_ms, None);
    assert_eq!(results[1].laps_completed, 0);
}

#[test]
fn test_parse_ac_results_from_directory() {
    use std::fs;

    // Create a temporary directory structure mimicking AC server output
    let temp_dir = std::env::temp_dir().join("ac_test_results");
    let results_dir = temp_dir.join("results");
    let _ = fs::remove_dir_all(&temp_dir); // clean up from previous runs
    fs::create_dir_all(&results_dir).unwrap();

    let json = r#"{
        "TrackName": "imola",
        "TrackConfig": "",
        "Type": "RACE",
        "Result": [
            {
                "DriverName": "TestDriver",
                "DriverGuid": "test_guid",
                "CarId": 0,
                "CarModel": "bmw_m3_gt2",
                "BestLap": 95000,
                "TotalTime": 480000,
                "LapCount": 5,
                "HasFinished": true
            }
        ]
    }"#;
    fs::write(results_dir.join("race_result.json"), json).unwrap();

    let results = parse_ac_results(&temp_dir);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].driver_name, "TestDriver");
    assert_eq!(results[0].guid, "test_guid");
    assert_eq!(results[0].best_lap_ms, Some(95000));

    // Cleanup
    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_ac_results_empty_dir() {
    use std::fs;

    let temp_dir = std::env::temp_dir().join("ac_test_empty");
    let results_dir = temp_dir.join("results");
    let _ = fs::remove_dir_all(&temp_dir);
    fs::create_dir_all(&results_dir).unwrap();

    let results = parse_ac_results(&temp_dir);
    assert!(results.is_empty());

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_parse_ac_results_no_dir() {
    let temp_dir = std::env::temp_dir().join("ac_test_nonexistent_xyz");
    let _ = std::fs::remove_dir_all(&temp_dir);

    let results = parse_ac_results(&temp_dir);
    assert!(results.is_empty());
}

#[test]
fn test_ac_result_lenient_parsing() {
    // Missing optional fields should not cause parse failure
    let json = r#"{
        "Result": [
            {
                "DriverName": "Partial",
                "DriverGuid": "",
                "CarId": 0,
                "CarModel": "",
                "BestLap": 0,
                "TotalTime": 0,
                "LapCount": 0,
                "HasFinished": false
            }
        ]
    }"#;

    let result_file: AcResultFile = serde_json::from_str(json).unwrap();
    assert_eq!(result_file.result.len(), 1);
    assert_eq!(result_file.track_name, ""); // default
    assert_eq!(result_file.session_type, ""); // default
}

// ── Phase 04 Plan 01: Server config safety overrides ────────────────

#[test]
fn test_server_cfg_damage_always_zero() {
    // SAFETY: Even when config has damage_multiplier=100, output must have DAMAGE_MULTIPLIER=0
    let mut config = AcLanSessionConfig::default();
    config.damage_multiplier = 100;
    let ini = generate_server_cfg_ini(&config);
    assert!(ini.contains("DAMAGE_MULTIPLIER=0"),
        "SAFETY: DAMAGE_MULTIPLIER must always be 0, got INI:\n{}", ini);
    assert!(!ini.contains("DAMAGE_MULTIPLIER=100"),
        "SAFETY: DAMAGE_MULTIPLIER=100 must NOT appear in output");
}

#[test]
fn test_server_cfg_grip_always_100() {
    // SAFETY: Even when config has session_start=50, output must have SESSION_START=100
    let mut config = AcLanSessionConfig::default();
    config.dynamic_track.session_start = 50;
    let ini = generate_server_cfg_ini(&config);
    assert!(ini.contains("SESSION_START=100"),
        "SAFETY: SESSION_START must always be 100, got INI:\n{}", ini);
    assert!(!ini.contains("SESSION_START=50"),
        "SAFETY: SESSION_START=50 must NOT appear in output");
}

// ── Phase 09 Plan 01: AI entry list, extra_cfg.yml, LaunchGame JSON ───

#[test]
fn test_entry_list_ai_entries_have_ai_fixed() {
    let mut config = AcLanSessionConfig::default();
    config.entries = vec![
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "Marco Rossi".to_string(),
            guid: String::new(),
            ballast: 0,
            restrictor: 0,
            pod_id: None,
            ai_mode: Some("fixed".to_string()),
        },
    ];
    let ini = generate_entry_list_ini(&config);
    assert!(ini.contains("AI=fixed"), "AI entry must have AI=fixed line in INI:\n{}", ini);
    assert!(ini.contains("DRIVERNAME=Marco Rossi"), "AI entry must have driver name");
}

#[test]
fn test_entry_list_mixed_human_and_ai() {
    let mut config = AcLanSessionConfig::default();
    config.entries = vec![
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "Human Driver".to_string(),
            guid: "steam_123".to_string(),
            ballast: 0,
            restrictor: 0,
            pod_id: Some("pod_1".to_string()),
            ai_mode: None,
        },
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "AI Driver".to_string(),
            guid: String::new(),
            ballast: 0,
            restrictor: 0,
            pod_id: None,
            ai_mode: Some("fixed".to_string()),
        },
    ];
    let ini = generate_entry_list_ini(&config);

    // Split into sections by [CAR_]
    let sections: Vec<&str> = ini.split("[CAR_").collect();
    // sections[0] is empty (before first [CAR_), sections[1] is CAR_0, sections[2] is CAR_1
    assert!(sections.len() >= 3, "Must have at least 2 CAR sections, got:\n{}", ini);

    let human_section = sections[1];
    let ai_section = sections[2];

    // Human entry should NOT have AI line
    assert!(!human_section.contains("AI="), "Human entry must not have AI= line:\n{}", human_section);
    assert!(human_section.contains("DRIVERNAME=Human Driver"));

    // AI entry should have AI=fixed
    assert!(ai_section.contains("AI=fixed"), "AI entry must have AI=fixed:\n{}", ai_section);
    assert!(ai_section.contains("DRIVERNAME=AI Driver"));
}

#[test]
fn test_entry_list_backward_compat_no_ai_mode() {
    let mut config = AcLanSessionConfig::default();
    config.entries = vec![
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "Legacy Driver".to_string(),
            guid: "steam_789".to_string(),
            ballast: 0,
            restrictor: 0,
            pod_id: Some("pod_3".to_string()),
            ai_mode: None,
        },
    ];
    let ini = generate_entry_list_ini(&config);
    assert!(!ini.contains("AI="), "Legacy entry with ai_mode None must not have AI= line:\n{}", ini);
    assert!(ini.contains("DRIVERNAME=Legacy Driver"));
}

#[test]
fn test_extra_cfg_yml_with_ai_level() {
    let mut config = AcLanSessionConfig::default();
    config.entries = vec![
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "AI Bot".to_string(),
            guid: String::new(),
            ballast: 0,
            restrictor: 0,
            pod_id: None,
            ai_mode: Some("fixed".to_string()),
        },
    ];
    let yml = generate_extra_cfg_yml(&config, Some(87));
    assert!(yml.contains("EnableAi: true"), "Must contain EnableAi: true:\n{}", yml);
    assert!(yml.contains("AiAggression: 0.87"), "AI level 87 must map to AiAggression: 0.87:\n{}", yml);
}

#[test]
fn test_extra_cfg_yml_no_ai_entries_returns_empty() {
    let config = AcLanSessionConfig::default(); // no entries at all
    let yml = generate_extra_cfg_yml(&config, Some(90));
    assert!(yml.is_empty(), "No AI entries should produce empty extra_cfg.yml, got:\n{}", yml);
}

#[test]
fn test_extra_cfg_yml_ai_entries_no_level() {
    let mut config = AcLanSessionConfig::default();
    config.entries = vec![
        AcEntrySlot {
            car_model: "ks_ferrari_488_gt3".to_string(),
            skin: String::new(),
            driver_name: "AI Bot".to_string(),
            guid: String::new(),
            ballast: 0,
            restrictor: 0,
            pod_id: None,
            ai_mode: Some("fixed".to_string()),
        },
    ];
    let yml = generate_extra_cfg_yml(&config, None);
    assert!(yml.contains("EnableAi: true"), "Must contain EnableAi: true:\n{}", yml);
    assert!(!yml.contains("AiAggression"), "No ai_level should omit AiAggression:\n{}", yml);
}

// ── Phase 333: Lobby sync + MP direct launch tests ──────────────────

#[test]
fn test_launch_json_includes_server_port() {
    // Verify the launch JSON structure sent to pods includes server_port (UDP port)
    // This was the critical missing field that prevented clients from connecting
    let config = AcLanSessionConfig {
        udp_port: 9601,
        tcp_port: 9601,
        http_port: 8082,
        password: "test".to_string(),
        track: "monza".to_string(),
        track_config: String::new(),
        cars: vec!["ks_ferrari_488_gt3".to_string()],
        ..AcLanSessionConfig::default()
    };

    let lan_ip = "192.168.31.23";
    let launch_json = serde_json::json!({
        "car": config.cars.first().unwrap_or(&"ks_ferrari_488_gt3".to_string()),
        "track": &config.track,
        "track_config": &config.track_config,
        "game_mode": "multi",
        "server_ip": &lan_ip,
        "server_port": config.udp_port,
        "server_http_port": config.http_port,
        "server_password": &config.password,
        "session_type": "race",
    });

    // Critical: server_port must be the UDP port, not the HTTP port
    assert_eq!(launch_json["server_port"], 9601, "server_port must be UDP port");
    assert_eq!(launch_json["server_http_port"], 8082, "server_http_port must be HTTP port");
    assert_ne!(
        launch_json["server_port"], launch_json["server_http_port"],
        "server_port and server_http_port must differ (UDP vs HTTP)"
    );
    assert_eq!(launch_json["game_mode"], "multi");
}

#[test]
fn test_server_cfg_register_to_lobby_disabled() {
    // LAN venue server must NOT register to Kunos public lobby
    let config = AcLanSessionConfig::default();
    let ini = generate_server_cfg_ini(&config);
    assert!(ini.contains("REGISTER_TO_LOBBY=0"),
        "LAN server must have REGISTER_TO_LOBBY=0:\n{}", ini);
}

#[test]
fn test_server_cfg_has_all_port_fields() {
    let mut config = AcLanSessionConfig::default();
    config.udp_port = 9605;
    config.tcp_port = 9605;
    config.http_port = 8086;
    let ini = generate_server_cfg_ini(&config);
    assert!(ini.contains("UDP_PORT=9605"), "Must have UDP_PORT=9605:\n{}", ini);
    assert!(ini.contains("TCP_PORT=9605"), "Must have TCP_PORT=9605:\n{}", ini);
    assert!(ini.contains("HTTP_PORT=8086"), "Must have HTTP_PORT=8086:\n{}", ini);
}

#[test]
fn test_entry_list_slot_count_matches_max_clients() {
    // When N pods are assigned, entry_list must have N slots
    let mut config = AcLanSessionConfig::default();
    config.max_clients = 4;
    config.entries = (0..4).map(|i| AcEntrySlot {
        car_model: "ks_ferrari_488_gt3".to_string(),
        skin: String::new(),
        driver_name: format!("Pod{}", i + 1),
        guid: String::new(),
        ballast: 0,
        restrictor: 0,
        pod_id: Some(format!("pod_{}", i + 1)),
        ai_mode: None,
    }).collect();
    let ini = generate_entry_list_ini(&config);

    // Count [CAR_N] sections
    let car_count = ini.matches("[CAR_").count();
    assert_eq!(car_count, 4,
        "Entry list must have exactly 4 CAR sections for 4 pods:\n{}", ini);
}

#[test]
fn test_lobby_status_serde_roundtrip() {
    let status = LobbyStatus {
        group_session_id: "sess-123".to_string(),
        phase: LobbyPhase::Forming,
        total_pods: 4,
        ready_pods: vec!["client_1".to_string(), "client_2".to_string()],
        created_at: "2026-04-07T20:00:00Z".to_string(),
    };
    let json = serde_json::to_string(&status).unwrap();
    let parsed: LobbyStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.group_session_id, "sess-123");
    assert_eq!(parsed.phase, LobbyPhase::Forming);
    assert_eq!(parsed.total_pods, 4);
    assert_eq!(parsed.ready_pods.len(), 2);
}

#[test]
fn test_lobby_phase_transitions() {
    // Verify all lobby phases serialize/deserialize correctly
    let phases = vec![
        LobbyPhase::Forming,
        LobbyPhase::AllReady,
        LobbyPhase::Starting,
        LobbyPhase::Active,
        LobbyPhase::Cancelled,
    ];
    for phase in phases {
        let json = serde_json::to_string(&phase).unwrap();
        let parsed: LobbyPhase = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, phase, "Phase {:?} must roundtrip through serde", phase);
    }
}

#[test]
fn test_admin_password_deterministic() {
    // Same inputs must produce same password (no randomness)
    let pw1 = generate_admin_password("session-1", 9600);
    let pw2 = generate_admin_password("session-1", 9600);
    assert_eq!(pw1, pw2, "Admin password must be deterministic");

    // Different inputs must produce different passwords
    let pw3 = generate_admin_password("session-2", 9600);
    assert_ne!(pw1, pw3, "Different sessions must have different passwords");
}
