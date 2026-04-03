//! Phase 317: Server-side persistence for pod game inventory and combo validation results.
//!
//! Provides:
//! - `handle_game_inventory_update`: WS handler — persists GameInventory to pod_game_inventory table
//! - `handle_combo_validation_report`: WS handler — persists ComboValidationResult to combo_validation_flags,
//!   triggers auto-disable + WhatsApp alert when a preset is invalid on ALL pods
//! - `upsert_pod_game_inventory`: DB upsert helper
//! - `upsert_combo_validation_flags`: DB upsert helper
//! - `compute_fleet_validity`: aggregates combo_validation_flags → "valid"/"partial"/"invalid"/"unknown"
//! - `auto_disable_invalid_presets`: disables a preset in game_presets when fleet_validity is "invalid"

use sqlx::SqlitePool;

use crate::config::Config;
use crate::state::AppState;
use rc_common::types::{ComboAvailabilityStatus, ComboValidationResult, GameInventory};

// ─── DB helpers ──────────────────────────────────────────────────────────────

/// Upsert all games from a pod's GameInventory into pod_game_inventory.
///
/// Uses INSERT OR REPLACE so each (pod_id, game_id) row is always current.
/// server_received_at is set by SQLite to the current UTC datetime.
pub async fn upsert_pod_game_inventory(
    db: &SqlitePool,
    inventory: &GameInventory,
) -> Result<(), sqlx::Error> {
    let pod_id = &inventory.pod_id;
    for game in &inventory.games {
        // Serialize SimType as debug string (e.g. "AssettoCorsa"), None → NULL
        let sim_type_str: Option<String> = game.sim_type.as_ref().map(|s| format!("{:?}", s));
        let steam_app_id: Option<i64> = game.steam_app_id.map(|id| id as i64);

        sqlx::query(
            "INSERT OR REPLACE INTO pod_game_inventory
                (pod_id, game_id, display_name, sim_type, exe_path, launchable,
                 scan_method, steam_app_id, scanned_at, server_received_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(pod_id)
        .bind(&game.game_id)
        .bind(&game.display_name)
        .bind(&sim_type_str)
        .bind(&game.exe_path)
        .bind(game.launchable as i64)
        .bind(&game.scan_method)
        .bind(steam_app_id)
        .bind(&game.scanned_at)
        .execute(db)
        .await?;
    }
    tracing::info!(
        target: "fleet-inventory",
        "Upserted {} game inventory rows for pod {}",
        inventory.games.len(),
        pod_id
    );
    Ok(())
}

/// Upsert combo validation results for a pod into combo_validation_flags.
///
/// Uses INSERT OR REPLACE so each (pod_id, preset_id) row reflects the most recent validation.
/// failure_reasons is stored as a JSON array string.
pub async fn upsert_combo_validation_flags(
    db: &SqlitePool,
    pod_id: &str,
    results: &[ComboValidationResult],
) -> Result<(), sqlx::Error> {
    for result in results {
        let status_str = format!("{:?}", result.status);
        let failure_json = serde_json::to_string(&result.failure_reasons)
            .unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            "INSERT OR REPLACE INTO combo_validation_flags
                (pod_id, preset_id, status, failure_reasons, validated_at, server_received_at)
             VALUES (?, ?, ?, ?, ?, datetime('now'))",
        )
        .bind(pod_id)
        .bind(&result.preset_id)
        .bind(&status_str)
        .bind(&failure_json)
        .bind(&result.validated_at)
        .execute(db)
        .await?;
    }
    tracing::info!(
        target: "fleet-inventory",
        "Upserted {} combo validation flags for pod {}",
        results.len(),
        pod_id
    );
    Ok(())
}

/// Compute fleet-wide validity for a preset based on combo_validation_flags.
///
/// - "valid"   — all pods that have reported show Available
/// - "partial" — some pods Available, some not
/// - "invalid" — no pods show Available (but at least one has reported)
/// - "unknown" — no validation data yet
pub async fn compute_fleet_validity(
    db: &SqlitePool,
    preset_id: &str,
) -> Result<String, sqlx::Error> {
    let row = sqlx::query(
        "SELECT
            COUNT(*) as total,
            SUM(CASE WHEN status = 'Available' THEN 1 ELSE 0 END) as valid_count
         FROM combo_validation_flags
         WHERE preset_id = ?",
    )
    .bind(preset_id)
    .fetch_one(db)
    .await?;

    use sqlx::Row;
    let total: i64 = row.try_get(0).unwrap_or(0);
    let valid_count: i64 = row.try_get(1).unwrap_or(0);

    if total == 0 {
        Ok("unknown".to_string())
    } else if valid_count == total {
        Ok("valid".to_string())
    } else if valid_count == 0 {
        Ok("invalid".to_string())
    } else {
        Ok("partial".to_string())
    }
}

/// Auto-disable a preset in game_presets when it is invalid on ALL pods.
///
/// Returns Ok(true) if the preset was newly disabled (caller should send WhatsApp alert).
/// Returns Ok(false) if already disabled or not fully invalid.
pub async fn auto_disable_invalid_presets(
    db: &SqlitePool,
    _config: &Config,
    preset_id: &str,
    preset_name: &str,
) -> Result<bool, sqlx::Error> {
    let validity = compute_fleet_validity(db, preset_id).await?;

    if validity != "invalid" {
        return Ok(false);
    }

    // Set enabled = 0 only if currently enabled (avoids duplicate alerts on repeated reports)
    let result = sqlx::query(
        "UPDATE game_presets SET enabled = 0 WHERE id = ? AND enabled = 1",
    )
    .bind(preset_id)
    .execute(db)
    .await?;

    if result.rows_affected() == 0 {
        // Already disabled — no alert needed
        return Ok(false);
    }

    // Fetch the first failure reason for the alert message
    let first_reason: Option<String> = sqlx::query_scalar(
        "SELECT failure_reasons FROM combo_validation_flags
         WHERE preset_id = ? AND status != 'Available'
         LIMIT 1",
    )
    .bind(preset_id)
    .fetch_optional(db)
    .await
    .unwrap_or(None);

    let reason_display = first_reason
        .as_deref()
        .unwrap_or("unknown reason");

    tracing::error!(
        target: "fleet-inventory",
        "AUTO-DISABLED preset '{}' ({}): invalid on all pods. First failure: {}",
        preset_name, preset_id, reason_display
    );

    Ok(true)
}

// ─── WS event handlers ────────────────────────────────────────────────────────

/// Handle GameInventoryUpdate from a pod — persist to pod_game_inventory.
/// Fire-and-forget: errors are logged, not returned.
pub async fn handle_game_inventory_update(state: &AppState, inventory: GameInventory) {
    if let Err(e) = upsert_pod_game_inventory(&state.db, &inventory).await {
        tracing::warn!(
            target: "fleet-inventory",
            "Failed to upsert game inventory for pod {}: {}",
            inventory.pod_id, e
        );
    }
}

/// Handle ComboValidationReport from a pod — persist to combo_validation_flags
/// and auto-disable presets that are invalid on ALL pods.
/// Fire-and-forget: errors are logged, not returned.
pub async fn handle_combo_validation_report(
    state: &AppState,
    pod_id: String,
    results: Vec<ComboValidationResult>,
) {
    if let Err(e) = upsert_combo_validation_flags(&state.db, &pod_id, &results).await {
        tracing::warn!(
            target: "fleet-inventory",
            "Failed to upsert combo validation flags for pod {}: {}",
            pod_id, e
        );
        return;
    }

    // De-duplicate by preset_id to avoid redundant auto-disable checks
    let mut seen_presets = std::collections::HashSet::new();
    for result in &results {
        if !seen_presets.insert(result.preset_id.clone()) {
            continue;
        }
        let preset_name = result.preset_name.clone();
        let preset_id = result.preset_id.clone();

        match auto_disable_invalid_presets(
            &state.db,
            &state.config,
            &preset_id,
            &preset_name,
        )
        .await
        {
            Ok(true) => {
                // Preset was newly disabled — send WhatsApp alert
                // NEVER hold a lock across .await — clone config before spawning
                let config = state.config.clone();
                let msg = format!(
                    "AUTO-DISABLED preset '{}': invalid on all pods. Check AC filesystem on pods — car/track/AI files may be missing.",
                    preset_name
                );
                tokio::spawn(async move {
                    crate::whatsapp_alerter::send_whatsapp(&config, &msg).await;
                });
            }
            Ok(false) => {
                // Not invalid on all pods or already disabled — nothing to do
            }
            Err(e) => {
                tracing::warn!(
                    target: "fleet-inventory",
                    "Failed to check/disable preset '{}' ({}): {}",
                    preset_name, preset_id, e
                );
            }
        }
    }
}

// ─── Suppress unused import warning ──────────────────────────────────────────
// ComboAvailabilityStatus is imported for readability context; used via format!("{:?}")
#[allow(dead_code)]
fn _uses_combo_status(_s: ComboAvailabilityStatus) {}
