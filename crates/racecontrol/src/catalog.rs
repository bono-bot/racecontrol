use std::collections::HashSet;

use serde_json::{json, Value};

use rc_common::types::ContentManifest;

#[path = "catalog_data.rs"]
mod catalog_data;

pub use catalog_data::{TrackEntry, CarEntry, PresetEntry};
use catalog_data::{
    TRACK_NAME_MAP, FEATURED_TRACKS, FEATURED_CARS,
    ALL_TRACK_IDS, ALL_CAR_IDS, PRESETS,
};

/// Maximum AI cars in single-player AC (20 slots total, 1 for player)
const MAX_AI_SINGLE_PLAYER: u32 = 19;

// ─── Track Name Normalization ─────────────────────────────────────────────────

/// Normalize a raw game track name to the canonical Racing Point catalog ID.
///
/// `sim_type` must be the stored DB format: `format!("{:?}", SimType::X).to_lowercase()`.
///   e.g. "assettoCorsa", "f125", "iracing", "lemansultimate"
///
/// AC track IDs are already canonical — they pass through unchanged.
/// Unknown combinations also pass through unchanged — lap storage is never blocked.
pub fn normalize_track_name(sim_type: &str, raw_track: &str) -> String {
    let raw_lower = raw_track.to_lowercase();
    let key = (sim_type.to_string(), raw_lower.clone());
    match TRACK_NAME_MAP.get(&key) {
        Some(&canonical) => canonical.to_string(),
        None => raw_track.to_string(),
    }
}

// ─── Track/Car Lookups ───────────────────────────────────────────────────────

/// Return the minimum lap time floor for a track ID, or None if unconfigured.
///
/// Laps below this threshold will have review_required=1 set in the DB by persist_lap().
/// This is a pure static data lookup — not async.
pub fn get_min_lap_time_ms_for_track(track_id: &str) -> Option<u32> {
    FEATURED_TRACKS
        .iter()
        .find(|t| t.id == track_id)
        .and_then(|t| t.min_lap_time_ms)
}

/// Return the full list of featured track entries for passport tier grouping.
/// Used by the /customer/passport endpoint to build tiered collections.
pub fn get_featured_tracks_for_passport() -> Vec<Value> {
    FEATURED_TRACKS
        .iter()
        .enumerate()
        .map(|(i, t)| json!({
            "id": t.id,
            "name": t.name,
            "category": t.category,
            "country": t.country,
            "sort_order": i
        }))
        .collect()
}

/// Return the full list of featured car entries for passport tier grouping.
/// Used by the /customer/passport endpoint to build tiered collections.
pub fn get_featured_cars_for_passport() -> Vec<Value> {
    FEATURED_CARS
        .iter()
        .enumerate()
        .map(|(i, c)| json!({
            "id": c.id,
            "name": c.name,
            "category": c.category,
            "sort_order": i
        }))
        .collect()
}

// ─── Display Name ────────────────────────────────────────────────────────────

/// Derive a human-readable name from a folder ID
pub fn id_to_display_name(id: &str) -> String {
    // Strip common prefixes
    let stripped = id
        .strip_prefix("ks_")
        .or_else(|| id.strip_prefix("ddm_"))
        .or_else(|| id.strip_prefix("gp_2025_"))
        .or_else(|| id.strip_prefix("rss_"))
        .or_else(|| id.strip_prefix("nohesi_"))
        .or_else(|| id.strip_prefix("art_"))
        .or_else(|| id.strip_prefix("srp_"))
        .or_else(|| id.strip_prefix("j8_"))
        .or_else(|| id.strip_prefix("wm_"))
        .unwrap_or(id);

    stripped
        .replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            // Keep ALL-CAPS words (e.g. "GT3", "BMW", "LMS")
            if word.len() <= 4 && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()) {
                word.to_string()
            } else {
                // Title case
                let mut chars = word.chars();
                match chars.next() {
                    Some(c) => format!("{}{}", c.to_uppercase(), chars.as_str()),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ─── Full Catalog ────────────────────────────────────────────────────────────

/// Build the full catalog JSON
pub fn get_catalog() -> Value {
    let featured_tracks: Vec<Value> = FEATURED_TRACKS
        .iter()
        .map(|t| json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country }))
        .collect();

    let all_tracks: Vec<Value> = ALL_TRACK_IDS
        .iter()
        .map(|id| {
            // Use featured name if available, otherwise derive
            let featured = FEATURED_TRACKS.iter().find(|t| t.id == *id);
            match featured {
                Some(t) => json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other", "country": "" }),
            }
        })
        .collect();

    let featured_cars: Vec<Value> = FEATURED_CARS
        .iter()
        .map(|c| json!({ "id": c.id, "name": c.name, "category": c.category }))
        .collect();

    let all_cars: Vec<Value> = ALL_CAR_IDS
        .iter()
        .map(|id| {
            let featured = FEATURED_CARS.iter().find(|c| c.id == *id);
            match featured {
                Some(c) => json!({ "id": c.id, "name": c.name, "category": c.category }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other" }),
            }
        })
        .collect();

    let all_presets: Vec<&catalog_data::PresetEntry> = PRESETS.iter().collect();

    json!({
        "tracks": {
            "featured": featured_tracks,
            "all": all_tracks,
        },
        "cars": {
            "featured": featured_cars,
            "all": all_cars,
        },
        "categories": {
            "tracks": ["F1 Circuits", "Real Circuits", "Indian Circuits", "Street / Touge", "Other"],
            "cars": ["F1 2025", "GT3", "Supercars", "Porsche", "JDM", "Classics", "Other"],
        },
        "presets": presets_to_json(&all_presets),
    })
}

// ─── Filtered Catalog ────────────────────────────────────────────────────────

/// Return the AC catalog filtered to only content present in the pod's manifest.
///
/// - If `manifest` is `None`, returns the full static catalog (fallback mode).
/// - If `manifest` is `Some`, filters cars/tracks to only IDs in the manifest
///   and enriches track entries with `available_session_types`, `max_ai`, and `configs`.
pub fn get_filtered_catalog(manifest: Option<&ContentManifest>) -> Value {
    let manifest = match manifest {
        Some(m) => m,
        None => return get_catalog(),
    };

    let car_ids: HashSet<&str> = manifest.cars.iter().map(|c| c.id.as_str()).collect();
    let track_ids: HashSet<&str> = manifest.tracks.iter().map(|t| t.id.as_str()).collect();

    // Filter featured cars
    let featured_cars: Vec<Value> = FEATURED_CARS
        .iter()
        .filter(|c| car_ids.contains(c.id))
        .map(|c| json!({ "id": c.id, "name": c.name, "category": c.category }))
        .collect();

    // Filter all cars
    let all_cars: Vec<Value> = ALL_CAR_IDS
        .iter()
        .filter(|id| car_ids.contains(**id))
        .map(|id| {
            let featured = FEATURED_CARS.iter().find(|c| c.id == *id);
            match featured {
                Some(c) => json!({ "id": c.id, "name": c.name, "category": c.category }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other" }),
            }
        })
        .collect();

    // Filter featured tracks (enriched)
    let featured_tracks: Vec<Value> = FEATURED_TRACKS
        .iter()
        .filter(|t| track_ids.contains(t.id))
        .map(|t| {
            let mut entry = json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country });
            enrich_track_entry(&mut entry, t.id, manifest);
            entry
        })
        .collect();

    // Filter all tracks (enriched)
    let all_tracks: Vec<Value> = ALL_TRACK_IDS
        .iter()
        .filter(|id| track_ids.contains(**id))
        .map(|id| {
            let featured = FEATURED_TRACKS.iter().find(|t| t.id == *id);
            let mut entry = match featured {
                Some(t) => json!({ "id": t.id, "name": t.name, "category": t.category, "country": t.country }),
                None => json!({ "id": id, "name": id_to_display_name(id), "category": "Other", "country": "" }),
            };
            enrich_track_entry(&mut entry, id, manifest);
            entry
        })
        .collect();

    // Filter presets: car + track must be in manifest; race/trackday require AI lines
    let filtered_presets: Vec<&catalog_data::PresetEntry> = PRESETS.iter().filter(|p| {
        if !car_ids.contains(p.car_id) || !track_ids.contains(p.track_id) {
            return false;
        }
        // For race/trackday, require AI lines on the track
        if matches!(p.session_type, "race" | "trackday") {
            let has_ai = manifest.tracks.iter()
                .find(|t| t.id == p.track_id)
                .map(|t| t.configs.iter().any(|c| c.has_ai))
                .unwrap_or(false);
            if !has_ai {
                return false;
            }
        }
        true
    }).collect();

    json!({
        "tracks": {
            "featured": featured_tracks,
            "all": all_tracks,
        },
        "cars": {
            "featured": featured_cars,
            "all": all_cars,
        },
        "categories": {
            "tracks": ["F1 Circuits", "Real Circuits", "Indian Circuits", "Street / Touge", "Other"],
            "cars": ["F1 2025", "GT3", "Supercars", "Porsche", "JDM", "Classics", "Other"],
        },
        "presets": presets_to_json(&filtered_presets),
    })
}

/// Enrich a track JSON entry with session type availability, max_ai, and configs from manifest.
fn enrich_track_entry(entry: &mut Value, track_id: &str, manifest: &ContentManifest) {
    let track_manifest = manifest.tracks.iter().find(|t| t.id == track_id);
    let has_ai = track_manifest
        .map(|t| t.configs.iter().any(|c| c.has_ai))
        .unwrap_or(false);

    let mut session_types = vec!["practice", "hotlap"];
    if has_ai {
        session_types.push("race");
        session_types.push("trackday");
        session_types.push("race_weekend");
    }

    // max_ai = min(max_pit_count - 1, MAX_AI_SINGLE_PLAYER)
    // Default to MAX_AI_SINGLE_PLAYER if all configs have pit_count=None
    let max_pit_count = track_manifest
        .and_then(|t| {
            t.configs.iter()
                .filter_map(|c| c.pit_count)
                .max()
        });

    let max_ai = match max_pit_count {
        Some(pits) => std::cmp::min(pits.saturating_sub(1), MAX_AI_SINGLE_PLAYER),
        None => MAX_AI_SINGLE_PLAYER,
    };

    let configs: Vec<String> = track_manifest
        .map(|t| t.configs.iter().map(|c| c.config.clone()).collect())
        .unwrap_or_default();

    entry["available_session_types"] = json!(session_types);
    entry["max_ai"] = json!(max_ai);
    entry["configs"] = json!(configs);
}

// ─── Launch Validation ───────────────────────────────────────────────────────

/// Validate that a car/track/session_type combo is installed on the pod.
///
/// Returns `Ok(())` if valid, `Err(reason)` if the combo should be rejected.
/// When `manifest` is `None` (no cached manifest), allows anything (fallback mode).
pub fn validate_launch_combo(
    manifest: Option<&ContentManifest>,
    car_id: &str,
    track_id: &str,
    session_type: &str,
) -> Result<(), String> {
    let manifest = match manifest {
        Some(m) => m,
        None => return Ok(()), // Fallback mode: no manifest cached, allow anything
    };

    // Validate car exists (skip if empty -- kiosk bookings may send empty car for non-AC games)
    if !car_id.is_empty() && !manifest.cars.iter().any(|c| c.id == car_id) {
        return Err(format!("car '{}' not installed on pod", car_id));
    }

    // Validate track exists (skip if empty)
    if !track_id.is_empty() {
        let track = manifest.tracks.iter().find(|t| t.id == track_id);
        match track {
            None => return Err(format!("track '{}' not installed on pod", track_id)),
            Some(t) => {
                // For race/trackday, require AI lines
                if matches!(session_type, "race" | "trackday" | "race_weekend")
                    && !t.configs.iter().any(|c| c.has_ai) {
                        return Err(format!(
                            "track '{}' has no AI lines — cannot use session type '{}'",
                            track_id, session_type
                        ));
                    }
            }
        }
    }

    Ok(())
}

// ─── Preset Helpers ──────────────────────────────────────────────────────────

fn preset_car_name(car_id: &str) -> &str {
    FEATURED_CARS.iter().find(|c| c.id == car_id).map(|c| c.name).unwrap_or(car_id)
}

fn preset_track_name(track_id: &str) -> &str {
    FEATURED_TRACKS.iter().find(|t| t.id == track_id).map(|t| t.name).unwrap_or(track_id)
}

fn presets_to_json(presets: &[&catalog_data::PresetEntry]) -> Vec<Value> {
    presets.iter().map(|p| json!({
        "id": p.id,
        "name": p.name,
        "tagline": p.tagline,
        "car_id": p.car_id,
        "car_name": preset_car_name(p.car_id),
        "track_id": p.track_id,
        "track_name": preset_track_name(p.track_id),
        "session_type": p.session_type,
        "difficulty": p.difficulty,
        "category": p.category,
        "duration_hint": p.duration_hint,
        "featured": p.featured,
    })).collect()
}

// ─── Difficulty Presets ──────────────────────────────────────────────────────

/// Build launch_args JSON with difficulty/transmission/conditions encoded
pub fn build_custom_launch_args(
    car: &str,
    track: &str,
    driver: &str,
    difficulty: &str,
    transmission: &str,
    ffb: &str,
    session_type: &str,
) -> Value {
    let (abs, tc, stability, autoclutch, ideal_line) = match difficulty {
        "easy" => (1, 1, 1, 1, 1),
        "medium" => (1, 1, 0, 1, 0),
        "hard" => (0, 0, 0, 0, 0),
        _ => (1, 1, 1, 1, 1), // default to easy
    };

    json!({
        "car": car,
        "track": track,
        "driver": driver,
        "difficulty": difficulty,
        "transmission": transmission,
        "ffb": ffb,
        "session_type": session_type,
        "aids": {
            "abs": abs,
            "tc": tc,
            "stability": stability,
            "autoclutch": autoclutch,
            "ideal_line": ideal_line,
        },
        "conditions": {
            "damage": 0,
            "dynamic_track": {
                "session_start": 100,
                "randomness": 0,
                "session_transfer": 100,
                "lap_gain": 0,
            },
            "sun_angle": 16,
            "weather": "clear",
        }
    })
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
