//! Pod inventory HTTP handler + on-disk TOML loader.
//!
//! Phase 361-01 (code-only session 2026-04-09).
//!
//! Reads `{config_dir}/rc-agent-pod{id}.toml` from disk, parses the
//! `[content.<game_key>]` sections, and returns a `PodInventory` payload.
//! Sourced from the committed TOML (ground truth), NOT from rc-agent live
//! disk. Drift detection is a separate `/debug/content-dirs` probe on rc-agent.
//!
//! Registered in `staff_routes` at `GET /api/v1/pods/{id}/inventory` — staff
//! JWT required per CLAUDE.md "Pod HTTP endpoints default to protected"
//! (inventory reveals fleet content posture, same info-disclosure class as
//! `/events/recent`).

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rc_common::inventory_types::{
    AiCountRange, GameInventory, PodInventory,
};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;

use crate::state::AppState;

/// Display-name lookup for known game keys. Unknown keys fall back to key.
fn game_display_name(key: &str) -> String {
    match key {
        "assetto_corsa" => "Assetto Corsa",
        "assetto_corsa_evo" => "Assetto Corsa EVO",
        "assetto_corsa_rally" => "Assetto Corsa Rally",
        "iracing" => "iRacing",
        "le_mans_ultimate" | "lemu" => "Le Mans Ultimate",
        "f1_25" => "F1 25",
        "forza" => "Forza Motorsport",
        "forza_horizon_5" => "Forza Horizon 5",
        other => return other.to_string(),
    }
    .to_string()
}

/// Partial pod TOML schema — only the fields this loader needs. Tolerant of
/// extra keys (serde default + deny nothing).
#[derive(Debug, Deserialize)]
struct PodTomlDoc {
    #[serde(default)]
    pod: PodSection,
    #[serde(default)]
    games: BTreeMap<String, toml::Value>,
    #[serde(default)]
    content: BTreeMap<String, ContentSection>,
}

#[derive(Debug, Default, Deserialize)]
struct PodSection {
    #[serde(default)]
    number: Option<u32>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    sim: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ContentSection {
    #[serde(default)]
    cars: Vec<String>,
    #[serde(default)]
    tracks: Vec<String>,
}

/// Compute an IST timestamp string. Git Bash TZ is unreliable on Windows per
/// CLAUDE.md — compute UTC + 5:30 manually.
fn ist_now_string() -> String {
    let now = chrono::Utc::now() + chrono::Duration::hours(5) + chrono::Duration::minutes(30);
    now.format("%Y-%m-%d %H:%M IST").to_string()
}

/// Read and parse `rc-agent-pod{pod_id}.toml` from `config_dir`, returning
/// the resulting `PodInventory` or an axum-compatible error tuple.
///
/// **Degrade-open behavior:** if a `[games.<key>]` section exists but the
/// corresponding `[content.<key>]` section is missing or empty, the returned
/// `GameInventory.cars` / `.tracks` vecs are empty — the validity gate treats
/// this as "skip car/track checks for this game" (see
/// `validation::session_validity` docs).
///
/// TODO(361-01 deferred runtime): Task 2.5 populates `[content.*]` sections
/// on all 8 pod TOMLs via SSH enumeration. Until that ships, every game on
/// every pod will degrade-open. The code path is complete; only the data is
/// missing. See DEFERRED-361-01.md.
pub fn load_pod_inventory(
    pod_id: u32,
    config_dir: &StdPath,
) -> Result<PodInventory, (StatusCode, String)> {
    let toml_path: PathBuf = config_dir.join(format!("rc-agent-pod{}.toml", pod_id));

    let raw = match std::fs::read_to_string(&toml_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err((
                StatusCode::NOT_FOUND,
                format!(
                    "Pod {} TOML not found at {}",
                    pod_id,
                    toml_path.display()
                ),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("read {}: {}", toml_path.display(), e),
            ));
        }
    };

    let doc: PodTomlDoc = toml::from_str(&raw).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse {}: {}", toml_path.display(), e),
        )
    })?;

    let pod_number = doc.pod.number.unwrap_or(pod_id);
    let pod_name = doc
        .pod
        .name
        .unwrap_or_else(|| format!("Pod {}", pod_number));
    let sim = doc.pod.sim.unwrap_or_else(|| "assetto_corsa".to_string());

    // Collect installed games from [games.*] keys. Preserves deterministic
    // ordering (BTreeMap is sorted by key).
    let games: Vec<GameInventory> = doc
        .games
        .keys()
        .map(|key| {
            let content = doc.content.get(key);
            let (cars, tracks) = match content {
                Some(c) => (c.cars.clone(), c.tracks.clone()),
                None => (Vec::new(), Vec::new()),
            };
            GameInventory {
                key: key.clone(),
                display_name: game_display_name(key),
                installed: true,
                cars,
                tracks,
            }
        })
        .collect();

    Ok(PodInventory {
        pod_number,
        pod_name,
        sim,
        games,
        ai_count_range: AiCountRange::default(),
        fetched_at: ist_now_string(),
    })
}

/// axum handler for `GET /api/v1/pods/{id}/inventory`.
///
/// **Auth:** registered in `staff_routes` — staff JWT required. Do NOT move
/// to public_routes or customer_routes. Inventory reveals fleet content
/// posture (information disclosure class, see CLAUDE.md Security section).
pub async fn pod_inventory_handler(
    State(state): State<Arc<AppState>>,
    Path(id): Path<u32>,
) -> Result<Json<PodInventory>, (StatusCode, String)> {
    let config_dir = state.config.server.config_dir_path();
    let inv = load_pod_inventory(id, &config_dir)?;
    Ok(Json(inv))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_toml(dir: &StdPath, pod_id: u32, contents: &str) {
        let path = dir.join(format!("rc-agent-pod{}.toml", pod_id));
        let mut f = std::fs::File::create(&path).expect("create toml");
        f.write_all(contents.as_bytes()).expect("write toml");
    }

    #[test]
    fn loads_pod_with_games_and_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(
            dir.path(),
            1,
            r#"
[pod]
number = 1
name = "Pod 1"
sim = "assetto_corsa"

[games.assetto_corsa]
exe_path = "C:/fake/acs.exe"
steam_app_id = 244210

[games.f1_25]
exe_path = "C:/fake/f1.exe"

[content.assetto_corsa]
cars = ["ferrari_458", "bmw_m3"]
tracks = ["spa", "monza"]

[content.f1_25]
cars = []
tracks = []
"#,
        );

        let inv = load_pod_inventory(1, dir.path()).expect("load ok");
        assert_eq!(inv.pod_number, 1);
        assert_eq!(inv.pod_name, "Pod 1");
        assert_eq!(inv.games.len(), 2);

        let ac = inv
            .games
            .iter()
            .find(|g| g.key == "assetto_corsa")
            .expect("ac present");
        assert!(ac.installed);
        assert_eq!(ac.cars, vec!["ferrari_458", "bmw_m3"]);
        assert_eq!(ac.tracks, vec!["spa", "monza"]);
        assert_eq!(ac.display_name, "Assetto Corsa");

        let f1 = inv.games.iter().find(|g| g.key == "f1_25").expect("f1");
        assert!(f1.cars.is_empty(), "f1_25 degrade-open cars");
        assert!(f1.tracks.is_empty(), "f1_25 degrade-open tracks");
    }

    #[test]
    fn missing_content_section_degrades_open() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(
            dir.path(),
            2,
            r#"
[pod]
number = 2
name = "Pod 2"
sim = "assetto_corsa"

[games.assetto_corsa]
exe_path = "C:/fake/acs.exe"
"#,
        );
        let inv = load_pod_inventory(2, dir.path()).expect("load");
        assert_eq!(inv.games.len(), 1);
        assert!(inv.games[0].cars.is_empty());
        assert!(inv.games[0].tracks.is_empty());
    }

    #[test]
    fn missing_file_returns_404() {
        let dir = tempfile::tempdir().expect("tempdir");
        let err = load_pod_inventory(999, dir.path()).unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[test]
    fn parse_error_returns_500() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_toml(dir.path(), 3, "this is = not [valid toml [[[");
        let err = load_pod_inventory(3, dir.path()).unwrap_err();
        assert_eq!(err.0, StatusCode::INTERNAL_SERVER_ERROR);
    }
}
