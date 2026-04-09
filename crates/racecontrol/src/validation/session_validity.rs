//! Session launch validity gate.
//!
//! Pure function. No I/O, no async, no locks. Called BEFORE any DB transaction
//! or state lock acquisition in the session/launch handler. On Err, the caller
//! returns HTTP 422 with the `ValidityError` serialized to JSON.
//!
//! Degrade-open semantics: a game whose `cars` vec is empty in the PodInventory
//! skips car validation (treats all cars as acceptable). Same for `tracks`.
//! This preserves launch ability for games whose content cannot be enumerated
//! from disk (FH5, F1 25, iRacing) while keeping strict validation for AC and
//! other enumerable sims.
//!
//! Phase 361-01 (code-only session 2026-04-09).

use rc_common::inventory_types::{PodInventory, ValidityError, ValidityErrorCode};

/// Validate a (game, car, track, ai_count) tuple against a pod's inventory.
///
/// Returns `Ok(())` when the tuple is launchable, or a `ValidityError` with a
/// human-readable `reason` + `suggestion` + machine `code` discriminant.
///
/// **Order of checks** (first failure wins):
/// 1. Game installed on pod?
/// 2. Car available for that game? (skipped if the game's cars vec is empty)
/// 3. Track available for that game? (skipped if the game's tracks vec is empty)
/// 4. AI count within the pod's configured range?
///
/// Pod not found is NOT checked here — callers handle missing TOML files at
/// the `load_pod_inventory` layer and return 404 separately.
pub fn validate_session_tuple(
    game: &str,
    car: &str,
    track: &str,
    ai_count: u32,
    inventory: &PodInventory,
) -> Result<(), ValidityError> {
    let game_entry = match inventory.games.iter().find(|g| g.key == game) {
        Some(g) if g.installed => g,
        Some(_) | None => {
            let installed_keys: Vec<&str> = inventory
                .games
                .iter()
                .filter(|g| g.installed)
                .map(|g| g.key.as_str())
                .collect();
            let suggestion = if installed_keys.is_empty() {
                format!(
                    "No games are installed on Pod {}. Contact staff.",
                    inventory.pod_number
                )
            } else {
                format!(
                    "Install {} on Pod {} or pick from: {}",
                    game,
                    inventory.pod_number,
                    installed_keys.join(", ")
                )
            };
            return Err(ValidityError {
                reason: format!(
                    "Game '{}' is not installed on Pod {}",
                    game, inventory.pod_number
                ),
                suggestion,
                code: ValidityErrorCode::GameNotInstalled,
            });
        }
    };

    // Degrade-open: empty cars vec = skip car validation (FH5, F1 25, iRacing).
    if !game_entry.cars.is_empty() && !game_entry.cars.iter().any(|c| c == car) {
        let sample: Vec<&str> = game_entry
            .cars
            .iter()
            .take(5)
            .map(String::as_str)
            .collect();
        return Err(ValidityError {
            reason: format!(
                "Car '{}' is not installed for {} on Pod {}",
                car, game, inventory.pod_number
            ),
            suggestion: format!("Pick from: {} (+{} more)", sample.join(", "), game_entry.cars.len().saturating_sub(sample.len())),
            code: ValidityErrorCode::CarNotAvailable,
        });
    }

    // Degrade-open: empty tracks vec = skip track validation.
    if !game_entry.tracks.is_empty() && !game_entry.tracks.iter().any(|t| t == track) {
        let sample: Vec<&str> = game_entry
            .tracks
            .iter()
            .take(5)
            .map(String::as_str)
            .collect();
        return Err(ValidityError {
            reason: format!(
                "Track '{}' is not installed for {} on Pod {}",
                track, game, inventory.pod_number
            ),
            suggestion: format!("Pick from: {} (+{} more)", sample.join(", "), game_entry.tracks.len().saturating_sub(sample.len())),
            code: ValidityErrorCode::TrackNotAvailable,
        });
    }

    // AI count range check.
    let range = inventory.ai_count_range;
    if ai_count < range.min || ai_count > range.max {
        return Err(ValidityError {
            reason: format!(
                "AI count {} is out of range [{}, {}] for Pod {}",
                ai_count, range.min, range.max, inventory.pod_number
            ),
            suggestion: format!("Choose an AI count between {} and {}", range.min, range.max),
            code: ValidityErrorCode::AiCountOutOfRange,
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_common::inventory_types::{AiCountRange, GameInventory};

    fn test_inventory() -> PodInventory {
        PodInventory {
            pod_number: 1,
            pod_name: "Pod 1".to_string(),
            sim: "assetto_corsa".to_string(),
            games: vec![
                GameInventory {
                    key: "assetto_corsa".to_string(),
                    display_name: "Assetto Corsa".to_string(),
                    installed: true,
                    cars: vec!["ferrari_458".to_string(), "bmw_m3".to_string()],
                    tracks: vec!["spa".to_string(), "monza".to_string()],
                },
                GameInventory {
                    key: "forza_horizon_5".to_string(),
                    display_name: "Forza Horizon 5".to_string(),
                    installed: true,
                    // Intentional: FH5 content not enumerable — degrade-open.
                    cars: vec![],
                    tracks: vec![],
                },
            ],
            ai_count_range: AiCountRange { min: 0, max: 23 },
            fetched_at: "2026-04-09 22:00 IST".to_string(),
        }
    }

    #[test]
    fn test_happy_path() {
        let inv = test_inventory();
        let result = validate_session_tuple("assetto_corsa", "ferrari_458", "spa", 5, &inv);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn test_game_not_installed() {
        let inv = test_inventory();
        let err = validate_session_tuple("dirt_rally_9", "car", "track", 0, &inv).unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::GameNotInstalled);
        assert!(err.reason.contains("dirt_rally_9"));
        assert!(err.suggestion.contains("assetto_corsa"));
    }

    #[test]
    fn test_car_not_available() {
        let inv = test_inventory();
        let err = validate_session_tuple(
            "assetto_corsa",
            "nonexistent_ferrari_9999",
            "spa",
            5,
            &inv,
        )
        .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::CarNotAvailable);
        assert!(err.reason.contains("nonexistent_ferrari_9999"));
        assert!(err.suggestion.contains("ferrari_458"));
    }

    #[test]
    fn test_track_not_available() {
        let inv = test_inventory();
        let err =
            validate_session_tuple("assetto_corsa", "ferrari_458", "nurburgring_fake", 5, &inv)
                .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::TrackNotAvailable);
        assert!(err.reason.contains("nurburgring_fake"));
    }

    #[test]
    fn test_ai_count_above_max() {
        let inv = test_inventory();
        let err = validate_session_tuple("assetto_corsa", "ferrari_458", "spa", 50, &inv)
            .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::AiCountOutOfRange);
    }

    #[test]
    fn test_ai_count_zero_ok() {
        let inv = test_inventory();
        let result = validate_session_tuple("assetto_corsa", "ferrari_458", "spa", 0, &inv);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ai_count_u32_max_rejected() {
        let inv = test_inventory();
        let err =
            validate_session_tuple("assetto_corsa", "ferrari_458", "spa", u32::MAX, &inv)
                .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::AiCountOutOfRange);
    }

    #[test]
    fn test_empty_cars_degrades_open() {
        // FH5 has empty cars vec — any car string must pass.
        let inv = test_inventory();
        let result = validate_session_tuple(
            "forza_horizon_5",
            "whatever_car_id",
            "whatever_track",
            3,
            &inv,
        );
        assert!(
            result.is_ok(),
            "degrade-open should accept any car/track, got {:?}",
            result
        );
    }

    #[test]
    fn test_empty_tracks_degrades_open() {
        // FH5 also has empty tracks vec — tracks should not be validated.
        let inv = test_inventory();
        let result =
            validate_session_tuple("forza_horizon_5", "car_x", "track_y", 0, &inv);
        assert!(result.is_ok());
    }

    #[test]
    fn test_game_installed_false_rejects() {
        let mut inv = test_inventory();
        inv.games[0].installed = false;
        let err = validate_session_tuple("assetto_corsa", "ferrari_458", "spa", 5, &inv)
            .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::GameNotInstalled);
    }

    #[test]
    fn test_no_installed_games_suggestion() {
        let mut inv = test_inventory();
        for g in inv.games.iter_mut() {
            g.installed = false;
        }
        let err = validate_session_tuple("assetto_corsa", "ferrari_458", "spa", 5, &inv)
            .unwrap_err();
        assert_eq!(err.code, ValidityErrorCode::GameNotInstalled);
        assert!(err.suggestion.contains("No games are installed"));
    }
}
