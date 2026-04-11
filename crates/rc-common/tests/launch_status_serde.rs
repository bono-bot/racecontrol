// Phase 368 Plan 01: DashboardEvent + AgentMessage serde roundtrip tests.
// Verifies that the new LaunchStatusChanged, LaunchNoteAdded, and LaunchStatusUpdate
// variants serialize/deserialize correctly via the existing tagged-enum protocol.
//
// DashboardEvent uses #[serde(tag = "event", content = "data", rename_all = "snake_case")]
// AgentMessage uses #[serde(tag = "type", content = "data", rename_all = "snake_case")]

use rc_common::protocol::{
    AgentMessage, CoreToAgentMessage, DashboardEvent, LaunchNoteEvent, LaunchOrigin,
    LaunchState, LaunchStatusCard,
};
use rc_common::types::SimType;

/// Test 1: DashboardEvent::LaunchStatusChanged round-trip serde.
/// JSON must have `"event":"launch_status_changed"` and `"data":{...}` envelope.
#[test]
fn dashboard_event_launch_status_serde() {
    let card = LaunchStatusCard {
        launch_id: "test-launch-001".to_string(),
        pod_id: "pod_3".to_string(),
        sim_type: "assetto_corsa".to_string(),
        state: LaunchState::LaunchStarted,
        detail: None,
        timestamp: chrono::Utc::now(),
        origin: LaunchOrigin::Customer,
        ai_tier: None,
        fix_action: None,
    };
    let event = DashboardEvent::LaunchStatusChanged(card.clone());

    let json = serde_json::to_string(&event).expect("serialize LaunchStatusChanged");
    assert!(
        json.contains(r#""event":"launch_status_changed""#),
        "Expected event tag 'launch_status_changed' in: {}",
        json
    );
    assert!(
        json.contains(r#""data":"#),
        "Expected 'data' content key in: {}",
        json
    );
    assert!(
        json.contains("test-launch-001"),
        "Expected launch_id in payload: {}",
        json
    );

    // Round-trip: deserialize back
    let parsed: DashboardEvent =
        serde_json::from_str(&json).expect("deserialize LaunchStatusChanged");
    if let DashboardEvent::LaunchStatusChanged(parsed_card) = parsed {
        assert_eq!(parsed_card.launch_id, card.launch_id);
        assert_eq!(parsed_card.pod_id, card.pod_id);
        assert_eq!(parsed_card.sim_type, card.sim_type);
        assert_eq!(parsed_card.state, card.state);
        assert_eq!(parsed_card.origin, card.origin);
    } else {
        panic!("Expected LaunchStatusChanged variant after round-trip");
    }
}

/// Test 2: DashboardEvent::LaunchNoteAdded round-trip serde.
/// JSON must have `"event":"launch_note_added"`.
#[test]
fn dashboard_event_launch_note_serde() {
    let note = LaunchNoteEvent {
        launch_id: "test-launch-002".to_string(),
        pod_id: "pod_5".to_string(),
        note_id: "note-abc-123".to_string(),
        staff_id: "staff-001".to_string(),
        staff_name: "Uday Singh".to_string(),
        body: "Pod restarted manually after driver reported freeze".to_string(),
        created_at: chrono::Utc::now(),
    };
    let event = DashboardEvent::LaunchNoteAdded(note.clone());

    let json = serde_json::to_string(&event).expect("serialize LaunchNoteAdded");
    assert!(
        json.contains(r#""event":"launch_note_added""#),
        "Expected event tag 'launch_note_added' in: {}",
        json
    );
    assert!(
        json.contains("note-abc-123"),
        "Expected note_id in payload: {}",
        json
    );

    // Round-trip
    let parsed: DashboardEvent =
        serde_json::from_str(&json).expect("deserialize LaunchNoteAdded");
    if let DashboardEvent::LaunchNoteAdded(parsed_note) = parsed {
        assert_eq!(parsed_note.launch_id, note.launch_id);
        assert_eq!(parsed_note.note_id, note.note_id);
        assert_eq!(parsed_note.body, note.body);
    } else {
        panic!("Expected LaunchNoteAdded variant after round-trip");
    }
}

/// Test 3: AgentMessage::LaunchStatusUpdate round-trip serde.
/// JSON must have `"type":"launch_status_update"` matching AgentMessage's serde tag.
#[test]
fn agent_message_launch_status_update_serde() {
    let msg = AgentMessage::LaunchStatusUpdate {
        launch_id: "test-launch-003".to_string(),
        state: LaunchState::AiAnalysisRequested,
        detail: Some("game_doctor analysis running".to_string()),
        ai_tier: Some(1),
        fix_action: None,
        origin: LaunchOrigin::Customer,
    };

    let json = serde_json::to_string(&msg).expect("serialize LaunchStatusUpdate");
    assert!(
        json.contains(r#""type":"launch_status_update""#),
        "Expected type tag 'launch_status_update' in: {}",
        json
    );
    assert!(
        json.contains("test-launch-003"),
        "Expected launch_id in payload: {}",
        json
    );
    assert!(
        json.contains("ai_analysis_requested"),
        "Expected state 'ai_analysis_requested' in: {}",
        json
    );

    // Round-trip
    let parsed: AgentMessage =
        serde_json::from_str(&json).expect("deserialize LaunchStatusUpdate");
    if let AgentMessage::LaunchStatusUpdate { launch_id, state, ai_tier, .. } = parsed {
        assert_eq!(launch_id, "test-launch-003");
        assert_eq!(state, LaunchState::AiAnalysisRequested);
        assert_eq!(ai_tier, Some(1));
    } else {
        panic!("Expected LaunchStatusUpdate variant after round-trip");
    }
}

/// Test 4: CoreToAgentMessage::LaunchGame backward compatibility.
/// A JSON payload WITHOUT `launch_id` field must deserialize to `launch_id: None`.
/// A payload WITH `launch_id` must round-trip correctly.
#[test]
fn launch_id_threaded_through_agent() {
    use rc_common::types::SimType;

    // Case A: with launch_id present
    let msg = CoreToAgentMessage::LaunchGame {
        sim_type: SimType::AssettoCorsa,
        launch_args: None,
        force_clean: false,
        duration_minutes: None,
        launch_id: Some("abc-123-def".to_string()),
    };
    let json = serde_json::to_string(&msg).expect("serialize LaunchGame with launch_id");
    assert!(
        json.contains("abc-123-def"),
        "Expected launch_id value in serialized JSON: {}",
        json
    );

    let parsed: CoreToAgentMessage =
        serde_json::from_str(&json).expect("deserialize LaunchGame with launch_id");
    if let CoreToAgentMessage::LaunchGame { launch_id, .. } = parsed {
        assert_eq!(launch_id, Some("abc-123-def".to_string()));
    } else {
        panic!("Expected LaunchGame variant");
    }

    // Case B: backward compat — JSON without launch_id field must give None.
    // CoreToAgentMessage uses tag="type", content="data" so legacy format needs both.
    let legacy_json = r#"{"type":"launch_game","data":{"sim_type":"assetto_corsa","launch_args":null,"force_clean":false,"duration_minutes":null}}"#;
    let parsed_legacy: CoreToAgentMessage =
        serde_json::from_str(legacy_json).expect("deserialize legacy LaunchGame without launch_id");
    if let CoreToAgentMessage::LaunchGame { launch_id, .. } = parsed_legacy {
        assert_eq!(
            launch_id,
            None,
            "Old LaunchGame message without launch_id field must deserialize to None"
        );
    } else {
        panic!("Expected LaunchGame variant from legacy JSON");
    }
}
