//! Filesystem scanner for Assetto Corsa content (cars and tracks).
//!
//! Produces a [`ContentManifest`] describing all installed cars and tracks on a pod.
//! Called at startup and on WebSocket reconnect to racecontrol.

use std::path::Path;

use rc_common::types::{CarManifestEntry, ContentManifest, TrackConfigManifest, TrackManifestEntry};

const LOG_TARGET: &str = "content-scanner";

/// Default AC content path on pods.
const AC_CONTENT_PATH: &str =
    r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa\content";

/// Scan the default AC installation and return a content manifest.
pub fn scan_ac_content() -> ContentManifest {
    scan_ac_content_at(Path::new(AC_CONTENT_PATH))
}

/// Scan an arbitrary content directory (for testing).
pub fn scan_ac_content_at(content_path: &Path) -> ContentManifest {
    let cars = scan_cars(&content_path.join("cars"));
    let tracks = scan_tracks(&content_path.join("tracks"));
    tracing::info!(
        target: LOG_TARGET,
        "Content scan complete: {} cars, {} tracks",
        cars.len(),
        tracks.len()
    );
    ContentManifest { cars, tracks }
}

/// Enumerate car folders under `content/cars/`.
fn scan_cars(cars_dir: &Path) -> Vec<CarManifestEntry> {
    let Ok(entries) = std::fs::read_dir(cars_dir) else {
        tracing::warn!(target: LOG_TARGET, "Cannot read cars directory: {:?}", cars_dir);
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden folders
            if name.starts_with('.') {
                return None;
            }
            Some(CarManifestEntry { id: name })
        })
        .collect()
}

/// Enumerate track folders and detect configs for each.
fn scan_tracks(tracks_dir: &Path) -> Vec<TrackManifestEntry> {
    let Ok(entries) = std::fs::read_dir(tracks_dir) else {
        tracing::warn!(target: LOG_TARGET, "Cannot read tracks directory: {:?}", tracks_dir);
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                return None;
            }
            let track_dir = entry.path();
            let configs = detect_track_configs(&track_dir);
            Some(TrackManifestEntry {
                id: name,
                configs,
            })
        })
        .collect()
}

/// Non-config subfolder names that should be ignored when detecting track configs.
const NON_CONFIG_DIRS: &[&str] = &["skins", "sfx", "extension", "ui", "data", "ai"];

/// Detect track config layouts within a track folder.
///
/// A valid config subfolder must contain at least one of: `data/`, `ai/`, or `models.ini`.
/// If no valid config subfolders are found, check for root-level `data/` to create
/// a default config with `config=""`.
fn detect_track_configs(track_dir: &Path) -> Vec<TrackConfigManifest> {
    let mut configs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(track_dir) {
        for entry in entries.flatten() {
            let metadata = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !metadata.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden folders and known non-config directories
            if name.starts_with('.') {
                continue;
            }
            if NON_CONFIG_DIRS.contains(&name.as_str()) {
                continue;
            }
            // A config subfolder must contain data/ or ai/ or models.ini
            let sub_path = entry.path();
            let has_data = sub_path.join("data").is_dir();
            let has_ai_dir = sub_path.join("ai").is_dir();
            let has_models = sub_path.join("models.ini").is_file();
            if !has_data && !has_ai_dir && !has_models {
                continue;
            }
            // Valid config subfolder
            let has_ai = check_has_ai(track_dir, &name);
            let pit_count = parse_pit_count(track_dir, &name);
            configs.push(TrackConfigManifest {
                config: name,
                has_ai,
                pit_count,
            });
        }
    }

    // If no valid config subfolders found, check for default layout (root-level data/)
    if configs.is_empty() && track_dir.join("data").is_dir() {
        let has_ai = check_has_ai(track_dir, "");
        let pit_count = parse_pit_count(track_dir, "");
        configs.push(TrackConfigManifest {
            config: String::new(),
            has_ai,
            pit_count,
        });
    }

    configs
}

/// Check if a track config has AI line data (ai/ folder with at least one file).
///
/// For default config (config=""): check `{track}/ai/`
/// For named config: check `{track}/{config}/ai/`
fn check_has_ai(track_dir: &Path, config: &str) -> bool {
    let ai_dir = if config.is_empty() {
        track_dir.join("ai")
    } else {
        track_dir.join(config).join("ai")
    };

    let Ok(entries) = std::fs::read_dir(&ai_dir) else {
        return false;
    };

    // Must contain at least one file (not just empty directory)
    entries
        .flatten()
        .any(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
}

/// Parse pit stall count from ui_track.json.
///
/// For default config: `{track}/ui/ui_track.json`
/// For named config: `{track}/ui/{config}/ui_track.json`
///
/// The `pitboxes` field is a STRING in AC's JSON format: `"pitboxes": "15"`.
fn parse_pit_count(track_dir: &Path, config: &str) -> Option<u32> {
    let ui_json_path = if config.is_empty() {
        track_dir.join("ui").join("ui_track.json")
    } else {
        track_dir.join("ui").join(config).join("ui_track.json")
    };

    let content = std::fs::read_to_string(&ui_json_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;

    // pitboxes is a STRING field in AC's ui_track.json: "pitboxes": "15"
    json.get("pitboxes")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u32>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a car folder
    fn create_car(base: &Path, name: &str) {
        fs::create_dir_all(base.join("cars").join(name)).unwrap();
    }

    /// Helper: create a default-layout track with optional AI and ui_track.json
    fn create_default_track(base: &Path, name: &str, ai_files: &[&str], pitboxes: Option<&str>) {
        let track = base.join("tracks").join(name);
        fs::create_dir_all(track.join("data")).unwrap();
        if !ai_files.is_empty() {
            let ai = track.join("ai");
            fs::create_dir_all(&ai).unwrap();
            for file in ai_files {
                fs::write(ai.join(file), "dummy").unwrap();
            }
        }
        if let Some(pb) = pitboxes {
            let ui = track.join("ui");
            fs::create_dir_all(&ui).unwrap();
            let json = format!(r#"{{"pitboxes": "{}"}}"#, pb);
            fs::write(ui.join("ui_track.json"), json).unwrap();
        }
    }

    /// Helper: create a named config for a multi-config track
    fn create_config(
        base: &Path,
        track: &str,
        config: &str,
        ai_files: &[&str],
        pitboxes: Option<&str>,
    ) {
        let track_dir = base.join("tracks").join(track);
        let config_dir = track_dir.join(config);
        // Config subfolder needs data/ to be detected
        fs::create_dir_all(config_dir.join("data")).unwrap();
        if !ai_files.is_empty() {
            let ai = config_dir.join("ai");
            fs::create_dir_all(&ai).unwrap();
            for file in ai_files {
                fs::write(ai.join(file), "dummy").unwrap();
            }
        }
        if let Some(pb) = pitboxes {
            let ui = track_dir.join("ui").join(config);
            fs::create_dir_all(&ui).unwrap();
            let json = format!(r#"{{"pitboxes": "{}"}}"#, pb);
            fs::write(ui.join("ui_track.json"), json).unwrap();
        }
    }

    // ── Car scanning tests ──────────────────────────────────────────────

    #[test]
    fn test_content_scanner_scan_cars_returns_correct_ids() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_car(base, "ks_ferrari_488_gt3");
        create_car(base, "ks_porsche_911_gt3_r");
        create_car(base, "bmw_z4_gt3");

        let cars = scan_cars(&base.join("cars"));
        assert_eq!(cars.len(), 3);
        let ids: Vec<&str> = cars.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"ks_ferrari_488_gt3"));
        assert!(ids.contains(&"ks_porsche_911_gt3_r"));
        assert!(ids.contains(&"bmw_z4_gt3"));
    }

    #[test]
    fn test_content_scanner_scan_cars_skips_non_dirs_and_hidden() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        let cars_dir = base.join("cars");
        fs::create_dir_all(&cars_dir).unwrap();
        // Real car folder
        fs::create_dir_all(cars_dir.join("ks_audi_r8_lms")).unwrap();
        // File (not directory) -- should be skipped
        fs::write(cars_dir.join("readme.txt"), "ignored").unwrap();
        // Hidden folder -- should be skipped
        fs::create_dir_all(cars_dir.join(".hidden_car")).unwrap();

        let cars = scan_cars(&cars_dir);
        assert_eq!(cars.len(), 1);
        assert_eq!(cars[0].id, "ks_audi_r8_lms");
    }

    #[test]
    fn test_content_scanner_scan_cars_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cars = scan_cars(&tmp.path().join("nonexistent"));
        assert!(cars.is_empty());
    }

    // ── Track scanning tests ────────────────────────────────────────────

    #[test]
    fn test_content_scanner_scan_default_layout_track() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        create_default_track(base, "magione", &["fast_lane.ai"], Some("15"));

        let tracks = scan_tracks(&base.join("tracks"));
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "magione");
        assert_eq!(tracks[0].configs.len(), 1);
        assert_eq!(tracks[0].configs[0].config, "");
        assert!(tracks[0].configs[0].has_ai);
        assert_eq!(tracks[0].configs[0].pit_count, Some(15));
    }

    #[test]
    fn test_content_scanner_scan_multi_config_track() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        let track_dir = base.join("tracks").join("spa");
        fs::create_dir_all(&track_dir).unwrap();
        create_config(base, "spa", "gp", &["fast_lane.ai"], Some("40"));
        create_config(base, "spa", "drift", &[], None);

        let tracks = scan_tracks(&base.join("tracks"));
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].id, "spa");
        assert_eq!(tracks[0].configs.len(), 2);

        let gp = tracks[0].configs.iter().find(|c| c.config == "gp").unwrap();
        assert!(gp.has_ai);
        assert_eq!(gp.pit_count, Some(40));

        let drift = tracks[0]
            .configs
            .iter()
            .find(|c| c.config == "drift")
            .unwrap();
        assert!(!drift.has_ai);
        assert_eq!(drift.pit_count, None);
    }

    // ── AI detection tests ──────────────────────────────────────────────

    #[test]
    fn test_content_scanner_has_ai_true_when_files_present() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("monza");
        fs::create_dir_all(track.join("ai")).unwrap();
        fs::write(track.join("ai").join("fast_lane.ai"), "data").unwrap();

        assert!(check_has_ai(&track, ""));
    }

    #[test]
    fn test_content_scanner_has_ai_false_when_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("mod_track");
        fs::create_dir_all(track.join("ai")).unwrap();
        // Empty ai/ folder -- no files

        assert!(!check_has_ai(&track, ""));
    }

    #[test]
    fn test_content_scanner_has_ai_false_when_missing() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("noai");
        fs::create_dir_all(&track).unwrap();
        // No ai/ folder at all

        assert!(!check_has_ai(&track, ""));
    }

    #[test]
    fn test_content_scanner_has_ai_named_config() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("nurb");
        let config_ai = track.join("gp").join("ai");
        fs::create_dir_all(&config_ai).unwrap();
        fs::write(config_ai.join("fast_lane.ai"), "data").unwrap();

        assert!(check_has_ai(&track, "gp"));
    }

    // ── Pit count parsing tests ─────────────────────────────────────────

    #[test]
    fn test_content_scanner_pit_count_from_string() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("monza");
        let ui = track.join("ui");
        fs::create_dir_all(&ui).unwrap();
        fs::write(
            ui.join("ui_track.json"),
            r#"{"pitboxes": "15", "name": "Monza"}"#,
        )
        .unwrap();

        assert_eq!(parse_pit_count(&track, ""), Some(15));
    }

    #[test]
    fn test_content_scanner_pit_count_none_when_missing_file() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("noui");
        fs::create_dir_all(&track).unwrap();

        assert_eq!(parse_pit_count(&track, ""), None);
    }

    #[test]
    fn test_content_scanner_pit_count_none_when_no_pitboxes_field() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("nopits");
        let ui = track.join("ui");
        fs::create_dir_all(&ui).unwrap();
        fs::write(ui.join("ui_track.json"), r#"{"name": "No Pits Track"}"#).unwrap();

        assert_eq!(parse_pit_count(&track, ""), None);
    }

    #[test]
    fn test_content_scanner_pit_count_named_config_path() {
        let tmp = TempDir::new().unwrap();
        let track = tmp.path().join("tracks").join("spa");
        let ui = track.join("ui").join("gp");
        fs::create_dir_all(&ui).unwrap();
        fs::write(
            ui.join("ui_track.json"),
            r#"{"pitboxes": "40", "name": "Spa GP"}"#,
        )
        .unwrap();

        assert_eq!(parse_pit_count(&track, "gp"), Some(40));
    }

    // ── Non-config subfolder filtering ──────────────────────────────────

    #[test]
    fn test_content_scanner_skips_non_config_subfolders() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        let track = base.join("tracks").join("imola");
        // Create default-layout track with data/ at root
        fs::create_dir_all(track.join("data")).unwrap();
        // Create non-config subfolders that should be ignored
        fs::create_dir_all(track.join("skins")).unwrap();
        fs::create_dir_all(track.join("sfx")).unwrap();
        fs::create_dir_all(track.join("extension")).unwrap();

        let tracks = scan_tracks(&base.join("tracks"));
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].configs.len(), 1);
        assert_eq!(tracks[0].configs[0].config, "");
    }

    // ── Integration: scan_ac_content_at ─────────────────────────────────

    #[test]
    fn test_content_scanner_scan_ac_content_at_combines() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Cars
        create_car(base, "ks_ferrari_488_gt3");
        create_car(base, "bmw_z4_gt3");

        // Default-layout track
        create_default_track(base, "monza", &["fast_lane.ai"], Some("29"));

        // Multi-config track
        let spa = base.join("tracks").join("spa");
        fs::create_dir_all(&spa).unwrap();
        create_config(base, "spa", "gp", &["fast_lane.ai"], Some("40"));
        create_config(base, "spa", "drift", &[], None);

        let manifest = scan_ac_content_at(base);
        assert_eq!(manifest.cars.len(), 2);
        assert_eq!(manifest.tracks.len(), 2);

        let car_ids: Vec<&str> = manifest.cars.iter().map(|c| c.id.as_str()).collect();
        assert!(car_ids.contains(&"ks_ferrari_488_gt3"));
        assert!(car_ids.contains(&"bmw_z4_gt3"));

        let track_ids: Vec<&str> = manifest.tracks.iter().map(|t| t.id.as_str()).collect();
        assert!(track_ids.contains(&"monza"));
        assert!(track_ids.contains(&"spa"));
    }
}
