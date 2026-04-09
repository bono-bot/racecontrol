//! Cross-boundary pod content inventory + validity types.
//!
//! Phase 361-01 (code-only session 2026-04-09). Shared by:
//! - racecontrol server `/api/v1/pods/{id}/inventory` endpoint
//! - kiosk wizard filtering (361-02)
//! - admin content drift page (361-03)
//! - rc-agent `/debug/content-dirs` probe response
//!
//! **Cross-boundary rule (CLAUDE.md Phase 62):** every TypeScript field name on
//! the kiosk/admin side MUST match a Rust field here. Do not rename without
//! grep-sweeping kiosk/src/lib/types.ts and admin/src/lib/types.ts.
//!
//! Field naming: snake_case on the wire, derived via serde default.

use serde::{Deserialize, Serialize};

/// Full inventory response for a single pod, sourced from the pod's
/// `deploy/configs/rc-agent-pod{id}.toml` on disk. See
/// `crates/racecontrol/src/api/pods.rs::load_pod_inventory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PodInventory {
    pub pod_number: u32,
    pub pod_name: String,
    pub sim: String,
    pub games: Vec<GameInventory>,
    pub ai_count_range: AiCountRange,
    /// IST timestamp string, format "YYYY-MM-DD HH:MM IST".
    pub fetched_at: String,
}

/// Per-game installed content inventory.
///
/// **Degrade-open semantics:** when `cars` is empty, the server validity gate
/// SKIPS car validation for this game (treats all cars as acceptable). Same
/// for `tracks`. This is intentional for MS Store titles (FH5) and games with
/// non-enumerable content (F1 25, iRacing). A populated `cars` array enables
/// strict validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameInventory {
    /// Canonical game key (assetto_corsa, f1_25, iracing, etc.)
    pub key: String,
    /// Human-readable name for UI display.
    pub display_name: String,
    /// True if the pod has this game configured in `[games.<key>]`.
    pub installed: bool,
    /// Installed cars on disk. Empty vec = degrade-open (no car validation).
    pub cars: Vec<String>,
    /// Installed tracks on disk. Empty vec = degrade-open (no track validation).
    pub tracks: Vec<String>,
}

/// Inclusive AI opponent count range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AiCountRange {
    pub min: u32,
    pub max: u32,
}

impl Default for AiCountRange {
    fn default() -> Self {
        // AC default grid: 0 (solo) to 23 (full grid minus player).
        Self { min: 0, max: 23 }
    }
}

/// Structured validity error returned to clients on HTTP 422 when a session
/// launch tuple (pod_id, game, car, track, ai_count) fails validation.
///
/// **IMPORTANT:** field names MUST match TS consumers byte-for-byte. Kiosk
/// (361-02) and admin (361-03) render `reason` + `suggestion` in toast
/// notifications; they discriminate on `code`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidityError {
    pub reason: String,
    pub suggestion: String,
    pub code: ValidityErrorCode,
}

/// Machine-readable validity error discriminant. Serializes to SCREAMING_SNAKE_CASE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidityErrorCode {
    GameNotInstalled,
    CarNotAvailable,
    TrackNotAvailable,
    AiCountOutOfRange,
    PodNotFound,
}

/// rc-agent `/debug/content-dirs` response payload. Separate from PodInventory
/// because this enumerates WHAT IS ON DISK right now (drift detection source),
/// whereas PodInventory reflects the committed TOML (source of truth).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentDirsResponse {
    pub games: Vec<GameDirs>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GameDirs {
    pub game_key: String,
    pub cars_dir: String,
    pub tracks_dir: String,
    pub cars_on_disk: Vec<String>,
    pub tracks_on_disk: Vec<String>,
    /// False = degrade-open (MS Store / non-enumerable title).
    pub cars_enumerable: bool,
    pub tracks_enumerable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validity_error_code_serializes_screaming_snake() {
        let code = ValidityErrorCode::CarNotAvailable;
        let json = serde_json::to_string(&code).expect("serialize");
        assert_eq!(json, "\"CAR_NOT_AVAILABLE\"");
    }

    #[test]
    fn validity_error_roundtrip() {
        let err = ValidityError {
            reason: "Car 'bogus' is not installed on Pod 1".to_string(),
            suggestion: "Pick from: ferrari_458, bmw_m3".to_string(),
            code: ValidityErrorCode::CarNotAvailable,
        };
        let json = serde_json::to_string(&err).expect("ser");
        // Field names MUST be snake_case on the wire (Phase 62 cross-boundary rule).
        assert!(json.contains("\"reason\""));
        assert!(json.contains("\"suggestion\""));
        assert!(json.contains("\"code\":\"CAR_NOT_AVAILABLE\""));
        let back: ValidityError = serde_json::from_str(&json).expect("de");
        assert_eq!(back, err);
    }

    #[test]
    fn ai_count_range_default_0_to_23() {
        let range = AiCountRange::default();
        assert_eq!(range.min, 0);
        assert_eq!(range.max, 23);
    }

    #[test]
    fn pod_inventory_snake_case_on_wire() {
        let inv = PodInventory {
            pod_number: 1,
            pod_name: "Pod 1".to_string(),
            sim: "assetto_corsa".to_string(),
            games: vec![GameInventory {
                key: "assetto_corsa".to_string(),
                display_name: "Assetto Corsa".to_string(),
                installed: true,
                cars: vec!["ferrari_458".to_string()],
                tracks: vec!["spa".to_string()],
            }],
            ai_count_range: AiCountRange::default(),
            fetched_at: "2026-04-09 22:00 IST".to_string(),
        };
        let json = serde_json::to_string(&inv).expect("ser");
        assert!(json.contains("\"pod_number\":1"));
        assert!(json.contains("\"pod_name\""));
        assert!(json.contains("\"ai_count_range\""));
        assert!(json.contains("\"fetched_at\""));
        assert!(json.contains("\"display_name\""));
    }
}
