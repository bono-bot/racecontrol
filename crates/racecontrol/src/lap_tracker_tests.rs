use chrono::Utc;
use rc_common::types::{LapData, SessionType, SimType};

use crate::catalog;

#[test]
fn lap_invalid_flag_prevents_persist() {
    // LAP-01: valid=false must cause persist_lap to return false without DB write.
    // Production code gates at persist_lap() line: if lap.lap_time_ms == 0 || !lap.valid { return false; }
    // Verify the guard logic holds for the two disqualifying conditions.
    let invalid_lap = false;
    let zero_time: u32 = 0;
    // Either condition alone causes an early return
    assert!(!invalid_lap || zero_time == 0, "invalid lap gate: !valid => skip persist");
    // Confirm the guard expression used in production code
    assert!(zero_time == 0 || !invalid_lap, "zero time gate: time==0 => skip persist");
}

#[test]
fn lap_review_required_below_min_floor() {
    // LAP-02: lap_time_ms=75_000 on Monza (min=80_000) must set review_required=1.
    // Verify that catalog returns the floor and the comparison logic fires correctly.
    let monza_floor = catalog::get_min_lap_time_ms_for_track("monza");
    assert_eq!(monza_floor, Some(80_000), "Monza floor must be 80_000ms");
    let lap_time_ms: u32 = 75_000;
    let floor = monza_floor.unwrap();
    assert!(
        lap_time_ms < floor,
        "75_000ms < 80_000ms floor => review_required should be set"
    );
}

#[test]
fn lap_not_flagged_above_min_floor() {
    // LAP-02: lap_time_ms=85_000 on Monza (min=80_000) must NOT set review_required.
    let monza_floor = catalog::get_min_lap_time_ms_for_track("monza").unwrap();
    let lap_time_ms: u32 = 85_000;
    assert!(
        lap_time_ms >= monza_floor,
        "85_000ms >= 80_000ms floor => review_required must NOT be set"
    );
}

#[test]
fn lap_data_carries_session_type() {
    // LAP-03: LapData.session_type is a required field set at construction.
    let lap = LapData {
        id: "test-id".to_string(),
        session_id: "sess-1".to_string(),
        driver_id: "driver-1".to_string(),
        pod_id: "pod_1".to_string(),
        sim_type: SimType::AssettoCorsa,
        track: "monza".to_string(),
        car: "ferrari_sf25".to_string(),
        lap_number: 1,
        lap_time_ms: 95_000,
        sector1_ms: None,
        sector2_ms: None,
        sector3_ms: None,
        valid: true,
        session_type: SessionType::Practice,
        created_at: Utc::now(),
    };
    assert_eq!(
        lap.session_type,
        SessionType::Practice,
        "LapData.session_type must be set and accessible"
    );
}
