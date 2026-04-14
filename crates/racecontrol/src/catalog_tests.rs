use super::*;
use rc_common::types::{ContentManifest, CarManifestEntry, TrackManifestEntry, TrackConfigManifest};

// ── normalize_track_name tests ──────────────────────────────────────────

#[test]
fn normalize_track_name_maps_known_tracks() {
    // F1 25 → canonical
    assert_eq!(normalize_track_name("f125", "silverstone"), "ks_silverstone");
    assert_eq!(normalize_track_name("f125", "red_bull_ring"), "ks_red_bull_ring");
    assert_eq!(normalize_track_name("f125", "monza"), "monza");

    // iRacing → canonical
    assert_eq!(normalize_track_name("iracing", "monza combined"), "monza");
    assert_eq!(normalize_track_name("iracing", "spa-francorchamps"), "spa");

    // AC passthrough (no mapping needed — already canonical)
    assert_eq!(normalize_track_name("assettoCorsa", "spa"), "spa");
    assert_eq!(normalize_track_name("assettoCorsa", "ks_silverstone"), "ks_silverstone");

    // Unknown track → passthrough unchanged
    assert_eq!(normalize_track_name("f125", "unknown_track_xyz"), "unknown_track_xyz");
    assert_eq!(normalize_track_name("iracing", "some_new_track"), "some_new_track");

    // Case-insensitive lookup for raw_track
    assert_eq!(normalize_track_name("f125", "Silverstone"), "ks_silverstone");
    assert_eq!(normalize_track_name("f125", "MONACO"), "monaco");
}

// ── Helpers ─────────────────────────────────────────────────────────────

/// Helper: build a manifest with specific car IDs and track entries
fn make_manifest(car_ids: &[&str], tracks: Vec<TrackManifestEntry>) -> ContentManifest {
    ContentManifest {
        cars: car_ids.iter().map(|id| CarManifestEntry { id: id.to_string() }).collect(),
        tracks,
    }
}

/// Helper: build a track manifest entry
fn make_track(id: &str, configs: Vec<(&str, bool, Option<u32>)>) -> TrackManifestEntry {
    TrackManifestEntry {
        id: id.to_string(),
        configs: configs.into_iter().map(|(config, has_ai, pit_count)| {
            TrackConfigManifest { config: config.to_string(), has_ai, pit_count }
        }).collect(),
    }
}

// ── get_filtered_catalog tests ───────────────────────────────────────

#[test]
fn filtered_catalog_none_manifest_returns_full_catalog() {
    let full = get_catalog();
    let filtered = get_filtered_catalog(None);
    assert_eq!(
        full["cars"]["all"].as_array().unwrap().len(),
        filtered["cars"]["all"].as_array().unwrap().len(),
        "None manifest should return full catalog"
    );
    assert_eq!(
        full["tracks"]["all"].as_array().unwrap().len(),
        filtered["tracks"]["all"].as_array().unwrap().len(),
    );
}

#[test]
fn filtered_catalog_filters_all_cars_to_manifest_only() {
    let manifest = make_manifest(
        &["bmw_z4_gt3", "ks_ferrari_488_gt3"],
        vec![],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let all_cars = result["cars"]["all"].as_array().unwrap();
    assert_eq!(all_cars.len(), 2);
    let ids: Vec<&str> = all_cars.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"bmw_z4_gt3"));
    assert!(ids.contains(&"ks_ferrari_488_gt3"));
}

#[test]
fn filtered_catalog_filters_all_tracks_to_manifest_only() {
    let manifest = make_manifest(
        &[],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let all_tracks = result["tracks"]["all"].as_array().unwrap();
    assert_eq!(all_tracks.len(), 1);
    assert_eq!(all_tracks[0]["id"].as_str().unwrap(), "spa");
}

#[test]
fn filtered_catalog_featured_cars_also_filtered() {
    // ferrari_sf25 is in FEATURED_CARS but not in manifest -> excluded
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let featured = result["cars"]["featured"].as_array().unwrap();
    // bmw_z4_gt3 IS in FEATURED_CARS, so it should appear
    let ids: Vec<&str> = featured.iter().map(|c| c["id"].as_str().unwrap()).collect();
    assert!(ids.contains(&"bmw_z4_gt3"), "bmw_z4_gt3 should be in featured");
    assert!(!ids.contains(&"ferrari_sf25"), "ferrari_sf25 not in manifest");
}

#[test]
fn filtered_catalog_track_without_ai_excludes_race_and_trackday() {
    let manifest = make_manifest(
        &[],
        vec![make_track("spa", vec![("", false, Some(24))])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let track = &result["tracks"]["all"].as_array().unwrap()[0];
    let session_types: Vec<String> = track["available_session_types"]
        .as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(session_types.iter().any(|s| s == "practice"));
    assert!(session_types.iter().any(|s| s == "hotlap"));
    assert!(!session_types.iter().any(|s| s == "race"), "no AI -> no race");
    assert!(!session_types.iter().any(|s| s == "trackday"), "no AI -> no trackday");
}

#[test]
fn filtered_catalog_track_with_ai_includes_race_and_trackday() {
    let manifest = make_manifest(
        &[],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let track = &result["tracks"]["all"].as_array().unwrap()[0];
    let session_types: Vec<String> = track["available_session_types"]
        .as_array().unwrap()
        .iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(session_types.iter().any(|s| s == "race"));
    assert!(session_types.iter().any(|s| s == "trackday"));
}

#[test]
fn filtered_catalog_track_includes_pit_count_max_across_configs() {
    let manifest = make_manifest(
        &[],
        vec![make_track("spa", vec![
            ("", true, Some(20)),
            ("gp", true, Some(30)),
        ])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let track = &result["tracks"]["all"].as_array().unwrap()[0];
    // max_ai = min(max_pit_count - 1, 19) = min(29, 19) = 19
    assert_eq!(track["max_ai"].as_u64().unwrap(), 19);
}

#[test]
fn filtered_catalog_track_pit_count_none_defaults_to_19() {
    let manifest = make_manifest(
        &[],
        vec![make_track("spa", vec![("", true, None)])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let track = &result["tracks"]["all"].as_array().unwrap()[0];
    assert_eq!(track["max_ai"].as_u64().unwrap(), 19);
}

// ── validate_launch_combo tests ──────────────────────────────────────

#[test]
fn validate_launch_combo_rejects_car_not_in_manifest() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "ferrari_sf25", "spa", "practice");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("car"));
}

#[test]
fn validate_launch_combo_rejects_track_not_in_manifest() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "bmw_z4_gt3", "monza", "practice");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("track"));
}

#[test]
fn validate_launch_combo_rejects_race_on_track_without_ai() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", false, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "bmw_z4_gt3", "spa", "race");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("AI"));
}

#[test]
fn validate_launch_combo_accepts_valid_combo() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "bmw_z4_gt3", "spa", "race");
    assert!(result.is_ok());
}

#[test]
fn validate_launch_combo_none_manifest_accepts_anything() {
    let result = validate_launch_combo(None, "any_car", "any_track", "race");
    assert!(result.is_ok(), "No manifest = fallback mode, accept anything");
}

// ── Preset tests ────────────────────────────────────────────────────

#[test]
fn catalog_includes_presets() {
    let catalog = get_catalog();
    let presets = catalog["presets"].as_array().expect("presets array must exist");
    assert!(
        presets.len() >= 13 && presets.len() <= 15,
        "Expected 13-15 presets, got {}",
        presets.len()
    );
}

#[test]
fn preset_car_ids_valid() {
    for preset in catalog_data::PRESETS.iter() {
        assert!(
            catalog_data::ALL_CAR_IDS.contains(&preset.car_id),
            "Preset '{}' has invalid car_id '{}'",
            preset.id,
            preset.car_id
        );
    }
}

#[test]
fn preset_track_ids_valid() {
    for preset in catalog_data::PRESETS.iter() {
        assert!(
            catalog_data::ALL_TRACK_IDS.contains(&preset.track_id),
            "Preset '{}' has invalid track_id '{}'",
            preset.id,
            preset.track_id
        );
    }
}

#[test]
fn presets_featured_flag() {
    let featured_count = catalog_data::PRESETS.iter().filter(|p| p.featured).count();
    assert!(
        featured_count >= 3 && featured_count <= 4,
        "Expected 3-4 featured presets, got {}",
        featured_count
    );
}

#[test]
fn filtered_catalog_filters_presets() {
    // Manifest with only one car and one track that matches a preset
    let manifest = make_manifest(
        &["ks_ferrari_488_gt3"],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let presets = result["presets"].as_array().expect("presets array must exist");
    // Only gt3-spa-race should match (car=ks_ferrari_488_gt3, track=spa)
    assert_eq!(presets.len(), 1, "Only one preset should match manifest");
    assert_eq!(presets[0]["id"].as_str().unwrap(), "gt3-spa-race");
}

#[test]
fn preset_race_filtered_no_ai() {
    // Manifest has the car and track for gt3-spa-race, but track has no AI
    let manifest = make_manifest(
        &["ks_ferrari_488_gt3"],
        vec![make_track("spa", vec![("", false, Some(24))])],
    );
    let result = get_filtered_catalog(Some(&manifest));
    let presets = result["presets"].as_array().expect("presets array must exist");
    // gt3-spa-race is session_type=race, so it should be excluded when has_ai=false
    assert!(
        presets.is_empty(),
        "Race preset should be excluded when track has no AI, got {} presets",
        presets.len()
    );
}

// ── build_custom_launch_args session_type tests ─────────────────────

#[test]
fn test_build_custom_launch_args_includes_session_type() {
    for session in &["practice", "hotlap", "race", "trackday", "race_weekend"] {
        let result = build_custom_launch_args(
            "bmw_z4_gt3", "spa", "Driver", "easy", "auto", "medium", session,
        );
        assert_eq!(
            result["session_type"].as_str().unwrap(),
            *session,
            "session_type should be '{}' in output JSON",
            session
        );
    }
}

// ── validate_launch_combo race_weekend tests ────────────────────────

#[test]
fn validate_launch_combo_rejects_race_weekend_without_ai() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", false, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "bmw_z4_gt3", "spa", "race_weekend");
    assert!(result.is_err(), "race_weekend should be rejected on track without AI");
    assert!(result.unwrap_err().contains("AI"), "error should mention AI");
}

#[test]
fn validate_launch_combo_accepts_race_weekend_with_ai() {
    let manifest = make_manifest(
        &["bmw_z4_gt3"],
        vec![make_track("spa", vec![("", true, Some(24))])],
    );
    let result = validate_launch_combo(Some(&manifest), "bmw_z4_gt3", "spa", "race_weekend");
    assert!(result.is_ok(), "race_weekend should be accepted on track with AI");
}
