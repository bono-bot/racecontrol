use super::*;

#[test]
fn test_weekend_phase_display() {
    assert_eq!(WeekendPhase::Practice.to_string(), "practice");
    assert_eq!(WeekendPhase::Qualifying.to_string(), "qualifying");
    assert_eq!(WeekendPhase::Race.to_string(), "race");
    assert_eq!(WeekendPhase::Finished.to_string(), "finished");
}

#[test]
fn test_weekend_phase_serde_roundtrip() {
    let phase = WeekendPhase::Qualifying;
    let json = serde_json::to_string(&phase).unwrap();
    assert_eq!(json, "\"qualifying\"");
    let parsed: WeekendPhase = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, phase);
}

#[test]
fn test_create_weekend_request_defaults() {
    let json = r#"{"pod_ids": ["pod_1"], "track": "monza", "car_class": "ks_ferrari_488_gt3"}"#;
    let req: CreateWeekendRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.practice_minutes, 10);
    assert_eq!(req.quali_minutes, 10);
    assert_eq!(req.race_laps, 10);
}

#[test]
fn test_create_weekend_request_custom() {
    let json = r#"{"pod_ids": ["pod_1", "pod_2"], "track": "spa", "car_class": "ks_porsche_911_gt3_r", "practice_minutes": 15, "quali_minutes": 12, "race_laps": 20}"#;
    let req: CreateWeekendRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.practice_minutes, 15);
    assert_eq!(req.quali_minutes, 12);
    assert_eq!(req.race_laps, 20);
    assert_eq!(req.pod_ids.len(), 2);
}
