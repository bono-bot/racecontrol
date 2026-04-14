use super::*;

#[test]
fn test_tier_for_level_all_tiers() {
    assert_eq!(tier_for_level(70), "rookie");
    assert_eq!(tier_for_level(79), "rookie");
    assert_eq!(tier_for_level(80), "amateur");
    assert_eq!(tier_for_level(84), "amateur");
    assert_eq!(tier_for_level(85), "semi_pro");
    assert_eq!(tier_for_level(87), "semi_pro"); // default midpoint
    assert_eq!(tier_for_level(89), "semi_pro");
    assert_eq!(tier_for_level(90), "pro");
    assert_eq!(tier_for_level(93), "pro"); // pro midpoint
    assert_eq!(tier_for_level(95), "pro");
    assert_eq!(tier_for_level(96), "alien");
    assert_eq!(tier_for_level(100), "alien");
}

#[test]
fn test_tier_for_level_fallback() {
    // Out-of-range values fall back to amateur
    assert_eq!(tier_for_level(0), "amateur");
    assert_eq!(tier_for_level(50), "amateur");
    assert_eq!(tier_for_level(69), "amateur");
}

#[test]
fn test_median_odd() {
    let s = AiLapSample {
        session_id: "s1".into(),
        pod_id: "p1".into(),
        sim_type: "ac".into(),
        car: "tatuus".into(),
        track: "magione".into(),
        ai_level: 87,
        lap_times_ms: vec![100000, 95000, 98000],
    };
    assert_eq!(s.median_lap_ms(), Some(98000));
}

#[test]
fn test_median_even() {
    let s = AiLapSample {
        session_id: "s1".into(),
        pod_id: "p1".into(),
        sim_type: "ac".into(),
        car: "tatuus".into(),
        track: "magione".into(),
        ai_level: 87,
        lap_times_ms: vec![100000, 95000, 98000, 97000],
    };
    // sorted: [95000, 97000, 98000, 100000], mid avg = (97000+98000)/2 = 97500
    assert_eq!(s.median_lap_ms(), Some(97500));
}

#[test]
fn test_median_empty() {
    let s = AiLapSample {
        session_id: "s1".into(),
        pod_id: "p1".into(),
        sim_type: "ac".into(),
        car: "tatuus".into(),
        track: "magione".into(),
        ai_level: 87,
        lap_times_ms: vec![],
    };
    assert_eq!(s.median_lap_ms(), None);
}

#[test]
fn test_ai_car_detection_via_empty_guid() {
    let human_guid = "76561198000000001";
    let ai_guid = "";
    assert!(!human_guid.is_empty(), "Human has non-empty guid");
    assert!(ai_guid.is_empty(), "AI has empty guid");
}

#[test]
fn test_consensus_3_of_5_agree() {
    // 3 models agree on ~90000ms p50, 2 outliers
    let responses = vec![
        ModelBandResponse {
            p10_ms: 85000,
            p50_ms: 90000,
            p90_ms: 97000,
        },
        ModelBandResponse {
            p10_ms: 84000,
            p50_ms: 91000,
            p90_ms: 98000,
        }, // agrees (1% off)
        ModelBandResponse {
            p10_ms: 85500,
            p50_ms: 90500,
            p90_ms: 97500,
        }, // agrees (<1%)
        ModelBandResponse {
            p10_ms: 75000,
            p50_ms: 80000,
            p90_ms: 86000,
        }, // outlier
        ModelBandResponse {
            p10_ms: 100000,
            p50_ms: 110000,
            p90_ms: 120000,
        }, // outlier
    ];
    let result = compute_consensus(&responses);
    assert!(
        result.is_some(),
        "3 agreeing models should produce consensus"
    );
    assert!(result.as_ref().map_or(false, |r| r.consensus_models >= 3));
}

#[test]
fn test_consensus_2_of_5_no_consensus() {
    // All 5 models disagree significantly
    let responses = vec![
        ModelBandResponse {
            p10_ms: 80000,
            p50_ms: 85000,
            p90_ms: 90000,
        },
        ModelBandResponse {
            p10_ms: 90000,
            p50_ms: 95000,
            p90_ms: 100000,
        },
        ModelBandResponse {
            p10_ms: 100000,
            p50_ms: 105000,
            p90_ms: 110000,
        },
        ModelBandResponse {
            p10_ms: 70000,
            p50_ms: 75000,
            p90_ms: 80000,
        },
        ModelBandResponse {
            p10_ms: 110000,
            p50_ms: 115000,
            p90_ms: 120000,
        },
    ];
    let result = compute_consensus(&responses);
    assert!(
        result.is_none(),
        "5 disagreeing models should produce no consensus"
    );
}

#[test]
fn test_slugify() {
    assert_eq!(slugify("Assetto Corsa"), "assetto-corsa");
    assert_eq!(slugify("tatuusFA1"), "tatuusfa1");
    assert_eq!(slugify("magione"), "magione");
    assert_eq!(slugify("Red Bull Ring GP"), "red-bull-ring-gp");
}

#[test]
fn test_toml_output_format() {
    let mut bands = HashMap::new();
    bands.insert(
        "semi_pro".to_string(),
        TierBand {
            p10_ms: 85000,
            p50_ms: 90000,
            p90_ms: 97000,
            consensus_models: 3,
            samples_used: 42,
        },
    );
    let entry = KbEntry {
        car: "tatuus".into(),
        track: "magione".into(),
        bands,
        batch_id: "test-batch".into(),
        updated_at: "2026-04-10T12:00:00Z".into(),
    };
    let toml = entry.to_toml_string();
    assert!(
        toml.contains("[semi_pro]"),
        "TOML must contain [semi_pro] section"
    );
    assert!(toml.contains("p10_ms = 85000"), "Must contain p10_ms");
    assert!(toml.contains("p90_ms = 97000"), "Must contain p90_ms");
    assert!(
        toml.contains("consensus_models = 3"),
        "Must contain consensus count"
    );
}

#[test]
fn test_anomaly_too_slow() {
    let mut bands = HashMap::new();
    bands.insert(
        "semi_pro".to_string(),
        TierBand {
            p10_ms: 85000,
            p50_ms: 90000,
            p90_ms: 97000,
            consensus_models: 3,
            samples_used: 10,
        },
    );
    let kb = KbEntry {
        car: "tatuus".into(),
        track: "magione".into(),
        bands,
        batch_id: "b1".into(),
        updated_at: "now".into(),
    };
    let (dir, band) = check_anomaly(&kb, "semi_pro", 110000);
    assert_eq!(dir, AnomalyDirection::TooSlow);
    assert!(band.is_some());
}

#[test]
fn test_anomaly_too_fast() {
    let mut bands = HashMap::new();
    bands.insert(
        "semi_pro".to_string(),
        TierBand {
            p10_ms: 85000,
            p50_ms: 90000,
            p90_ms: 97000,
            consensus_models: 3,
            samples_used: 10,
        },
    );
    let kb = KbEntry {
        car: "tatuus".into(),
        track: "magione".into(),
        bands,
        batch_id: "b1".into(),
        updated_at: "now".into(),
    };
    let (dir, _) = check_anomaly(&kb, "semi_pro", 70000);
    assert_eq!(dir, AnomalyDirection::TooFast);
}

#[test]
fn test_no_anomaly_within_band() {
    let mut bands = HashMap::new();
    bands.insert(
        "semi_pro".to_string(),
        TierBand {
            p10_ms: 85000,
            p50_ms: 90000,
            p90_ms: 97000,
            consensus_models: 3,
            samples_used: 10,
        },
    );
    let kb = KbEntry {
        car: "tatuus".into(),
        track: "magione".into(),
        bands,
        batch_id: "b1".into(),
        updated_at: "now".into(),
    };
    let (dir, _) = check_anomaly(&kb, "semi_pro", 90000);
    assert_eq!(dir, AnomalyDirection::None);
}

#[test]
fn test_no_kb_no_anomaly() {
    let kb = KbEntry {
        car: "tatuus".into(),
        track: "magione".into(),
        bands: HashMap::new(), // empty -- no KB for this tier
        batch_id: "b1".into(),
        updated_at: "now".into(),
    };
    let (dir, band) = check_anomaly(&kb, "alien", 90000);
    assert_eq!(dir, AnomalyDirection::None, "No KB entry = no anomaly");
    assert!(band.is_none());
}
