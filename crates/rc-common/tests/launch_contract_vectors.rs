//! Golden cross-language vectors for the launch boundary (server <-> agent).
//!
//! CHARACTERIZATION test — pins the CURRENT wire shape of the launch contract; it
//! changes no behavior. It reads the single canonical fixture shared with the TS
//! structural test (packages/contract-tests/src/launch-config.contract.test.ts) so
//! both languages assert against ONE source of truth.
//!
//! Anchors the 2026-03-26 fleet-wide drift: the kiosk sent `ai_difficulty: "easy"`
//! (string) while the Rust struct had `ai_level: u8` (numeric); serde silently
//! dropped the unknown field and the AI was always Semi-Pro. These vectors turn
//! that class of drift into a CI failure on the racecontrol side; the rp-v2-apps
//! side asserts the same fixture (cross-repo, via PR).
//!
//! The fixture lives under packages/contract-tests/ (the established fixtures home,
//! alongside ws-messages.json's "TS/Rust contract" set). The cross-package
//! `include_str!` is deliberate: one fixture, two language consumers — drift on
//! either side fails here.

use rc_common::launch_contract::{GameEvent, LaunchRequest, LaunchResult};
use serde_json::Value;

const FIXTURE: &str =
    include_str!("../../../packages/contract-tests/src/fixtures/launch-config.json");

/// Pull a named vector group (e.g. "launch_request") as a JSON object map.
fn vectors(group: &str) -> serde_json::Map<String, Value> {
    let root: Value = serde_json::from_str(FIXTURE).expect("fixture is valid JSON");
    root.get(group)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_else(|| panic!("fixture missing group `{group}`"))
}

/// Deserialize each vector into `T`, re-serialize, and assert the round-trip is
/// value-identical to the golden JSON. This catches the full drift class:
///   - field rename  -> re-serialized value loses/gains a key -> assert_eq! fails
///   - type change   -> deserialize into the typed shape fails outright
///   - dropped variant/field -> serde ignores it -> round-trip lacks it -> fails
fn assert_roundtrip_exact<T>(group: &str)
where
    T: serde::de::DeserializeOwned + serde::Serialize,
{
    let group_vectors = vectors(group);
    assert!(!group_vectors.is_empty(), "group `{group}` has no vectors");
    for (name, golden) in group_vectors {
        let typed: T = serde_json::from_value(golden.clone())
            .unwrap_or_else(|e| panic!("{group}/{name}: deserialize into Rust type failed: {e}"));
        let roundtrip = serde_json::to_value(&typed)
            .unwrap_or_else(|e| panic!("{group}/{name}: re-serialize failed: {e}"));
        assert_eq!(
            golden, roundtrip,
            "{group}/{name}: wire shape drifted from the Rust type (field rename / type change / dropped field)"
        );
    }
}

#[test]
fn launch_request_vectors_match_rust_type() {
    assert_roundtrip_exact::<LaunchRequest>("launch_request");
}

#[test]
fn launch_result_vectors_match_rust_type() {
    assert_roundtrip_exact::<LaunchResult>("launch_result");
}

#[test]
fn game_event_vectors_match_rust_type() {
    assert_roundtrip_exact::<GameEvent>("game_event");
}

/// Explicit guard on the canonical drift field. `ai_level` / `ai_count` MUST be
/// numeric on the wire. If a future edit makes the fixture mirror the kiosk's old
/// `"easy"` string, this fails loudly and specifically (in addition to the
/// deserialize failure in `launch_request_vectors_match_rust_type`).
#[test]
fn ac_config_ai_fields_are_numeric_not_string() {
    let lr = vectors("launch_request");
    let ac = lr
        .get("ac_sp_staff_kiosk")
        .and_then(|v| v.get("config"))
        .and_then(|v| v.get("AssettCorsa"))
        .expect("ac_sp_staff_kiosk.config.AssettCorsa present");
    assert!(
        ac["ai_level"].is_u64(),
        "ai_level must be numeric (u8) on the wire, never a string"
    );
    assert!(
        ac["ai_count"].is_u64(),
        "ai_count must be numeric (u8) on the wire, never a string"
    );
}
