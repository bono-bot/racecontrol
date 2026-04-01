//! Model Reputation Store — SQLite-backed persistence for MODEL_REPUTATION, DEMOTED_MODELS, PROMOTED_MODELS.
//!
//! Phase 292, Plan 01 — MREP-01/02/03: Persist reputation state across rc-agent restarts.
//!
//! Key design choices:
//! - Shares `mesh_kb.db` with knowledge_base.rs and model_eval_store.rs (no extra file on disk)
//! - Uses rusqlite (same crate as knowledge_base.rs), NOT sqlx
//! - No `.unwrap()` in production code — production paths use `?`, `.ok()`, or match
//! - Migrations are idempotent (CREATE TABLE IF NOT EXISTS, CREATE INDEX IF NOT EXISTS)
//! - `load_into_memory()` must be called before tier_engine starts so the first diagnosis
//!   uses correct reputation state.

use std::collections::HashSet;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::knowledge_base::KB_PATH;

const LOG_TARGET: &str = "model-reputation-store";

/// Reputation records are stored in the same mesh_kb.db as knowledge_base.rs.
pub const REP_DB_PATH: &str = KB_PATH;

/// One row in the model_reputation table — one record per tracked model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationRow {
    /// e.g. "deepseek/deepseek-r1-0528"
    pub model_id: String,
    /// Number of correct diagnoses from last 7-day sweep.
    pub correct_count: u32,
    /// Total diagnosis runs from last 7-day sweep.
    pub total_count: u32,
    /// "active" | "demoted" | "promoted"
    pub status: String,
    /// RFC 3339 UTC timestamp of last update.
    pub updated_at: String,
}

/// Persistent store for model reputation state.
///
/// Open with `ModelReputationStore::open(path)`. Safe to call on an existing DB.
pub struct ModelReputationStore {
    conn: Connection,
}

impl ModelReputationStore {
    /// Open the reputation store at `path`.
    ///
    /// Runs idempotent migrations before returning. Safe to call on an existing DB.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Create `model_reputation` table and indices (idempotent — safe to call on upgrade).
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_reputation (
                model_id TEXT PRIMARY KEY,
                correct_count INTEGER NOT NULL DEFAULT 0,
                total_count INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'active',
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_rep_status ON model_reputation (status);",
        )?;
        tracing::debug!(target: LOG_TARGET, "model_reputation schema migration complete");
        Ok(())
    }

    /// Persist (upsert) accuracy counts for a model.
    ///
    /// Called by `run_reputation_sweep()` after aggregating 7-day eval records.
    /// Preserves existing `status` value — does not overwrite demotion/promotion state.
    pub fn save_outcome(
        &self,
        model_id: &str,
        correct_count: u32,
        total_count: u32,
    ) -> anyhow::Result<()> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO model_reputation (model_id, correct_count, total_count, status, updated_at)
             VALUES (?1, ?2, ?3, COALESCE((SELECT status FROM model_reputation WHERE model_id = ?1), 'active'), ?4)
             ON CONFLICT(model_id) DO UPDATE SET
               correct_count = excluded.correct_count,
               total_count = excluded.total_count,
               updated_at = excluded.updated_at",
            params![model_id, correct_count as i64, total_count as i64, updated_at],
        )?;
        Ok(())
    }

    /// Persist a demotion decision for a model (status = 'demoted').
    ///
    /// Called by `run_reputation_sweep()` after `mma_engine::demote_model()`.
    pub fn save_demotion(&self, model_id: &str) -> anyhow::Result<()> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO model_reputation (model_id, correct_count, total_count, status, updated_at)
             VALUES (?1, 0, 0, 'demoted', ?2)
             ON CONFLICT(model_id) DO UPDATE SET
               status = 'demoted',
               updated_at = excluded.updated_at",
            params![model_id, updated_at],
        )?;
        tracing::warn!(target: LOG_TARGET, model = model_id, "Demotion persisted to DB");
        Ok(())
    }

    /// Persist a promotion decision for a model (status = 'promoted').
    ///
    /// Called by `run_reputation_sweep()` after `mma_engine::promote_model()`.
    /// Promotion clears demotion — status is updated to 'promoted'.
    pub fn save_promotion(&self, model_id: &str) -> anyhow::Result<()> {
        let updated_at = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO model_reputation (model_id, correct_count, total_count, status, updated_at)
             VALUES (?1, 0, 0, 'promoted', ?2)
             ON CONFLICT(model_id) DO UPDATE SET
               status = 'promoted',
               updated_at = excluded.updated_at",
            params![model_id, updated_at],
        )?;
        tracing::info!(target: LOG_TARGET, model = model_id, "Promotion persisted to DB");
        Ok(())
    }

    /// Load all reputation rows from the DB.
    ///
    /// Used by `load_into_memory()` to restore MODEL_REPUTATION on boot.
    pub fn load_all_outcomes(&self) -> anyhow::Result<Vec<ReputationRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT model_id, correct_count, total_count, status, updated_at
             FROM model_reputation",
        )?;
        let rows = stmt.query_map([], Self::row_to_record)?;
        let mut out = Vec::new();
        for row in rows {
            match row {
                Ok(r) => out.push(r),
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "Failed to deserialize reputation row — skipping"
                    );
                }
            }
        }
        Ok(out)
    }

    /// Load the set of demoted model IDs from the DB.
    ///
    /// Used by `load_into_memory()` to restore DEMOTED_MODELS on boot.
    pub fn load_demotion_set(&self) -> anyhow::Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT model_id FROM model_reputation WHERE status = 'demoted'",
        )?;
        let ids = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = HashSet::new();
        for id in ids {
            match id {
                Ok(model_id) => {
                    out.insert(model_id);
                }
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "Failed to read demoted model_id — skipping"
                    );
                }
            }
        }
        Ok(out)
    }

    /// Load the set of promoted model IDs from the DB.
    ///
    /// Used by `load_into_memory()` to restore PROMOTED_MODELS on boot.
    pub fn load_promotion_set(&self) -> anyhow::Result<HashSet<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT model_id FROM model_reputation WHERE status = 'promoted'",
        )?;
        let ids = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = HashSet::new();
        for id in ids {
            match id {
                Ok(model_id) => {
                    out.insert(model_id);
                }
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "Failed to read promoted model_id — skipping"
                    );
                }
            }
        }
        Ok(out)
    }

    /// Map a rusqlite Row to a ReputationRow.
    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<ReputationRow> {
        Ok(ReputationRow {
            model_id: row.get(0)?,
            correct_count: row.get::<_, i64>(1)? as u32,
            total_count: row.get::<_, i64>(2)? as u32,
            status: row.get(3)?,
            updated_at: row.get(4)?,
        })
    }
}

/// Restore MODEL_REPUTATION, DEMOTED_MODELS, and PROMOTED_MODELS from SQLite on boot.
///
/// MUST be called before `tier_engine::spawn()` so the first diagnosis round uses
/// correct reputation state. Uses `mma_engine::set_model_counts()` to bulk-restore
/// accumulated counts without calling `record_model_outcome()` 500+ times.
pub fn load_into_memory(store: &ModelReputationStore) -> anyhow::Result<()> {
    // Step 1: Load all reputation rows and restore in-memory counts
    let rows = store.load_all_outcomes()?;
    let row_count = rows.len();
    for row in &rows {
        crate::mma_engine::set_model_counts(&row.model_id, row.correct_count, row.total_count);
    }

    // Step 2: Load demotion set and restore DEMOTED_MODELS
    let demoted = store.load_demotion_set()?;
    let demote_count = demoted.len();
    for model_id in &demoted {
        crate::mma_engine::demote_model(model_id);
    }

    // Step 3: Load promotion set and restore PROMOTED_MODELS
    let promoted = store.load_promotion_set()?;
    let promote_count = promoted.len();
    for model_id in &promoted {
        crate::mma_engine::promote_model(model_id);
    }

    tracing::info!(
        target: LOG_TARGET,
        models = row_count,
        demoted = demote_count,
        promoted = promote_count,
        "Model reputation restored from SQLite"
    );
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test 1: ModelReputationStore::open(":memory:") succeeds and creates model_reputation table.
    #[test]
    fn test_open_in_memory_succeeds() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM model_reputation", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "fresh table should have 0 rows");
    }

    // Test 2: save_outcome() writes a row; load_all_outcomes() returns it with correct values.
    #[test]
    fn test_save_and_load_outcome() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        store.save_outcome("deepseek/r1", 7, 10).unwrap();

        let rows = store.load_all_outcomes().unwrap();
        assert_eq!(rows.len(), 1, "should return exactly 1 row");
        let row = &rows[0];
        assert_eq!(row.model_id, "deepseek/r1");
        assert_eq!(row.correct_count, 7);
        assert_eq!(row.total_count, 10);
        assert_eq!(row.status, "active");
    }

    // Test 3: save_demotion() persists; load_demotion_set() returns the demoted model.
    #[test]
    fn test_save_and_load_demotion() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        store.save_demotion("model_a").unwrap();

        let demoted = store.load_demotion_set().unwrap();
        assert!(demoted.contains("model_a"), "demoted set must contain model_a");
        assert_eq!(demoted.len(), 1);
    }

    // Test 4: save_promotion() persists; load_promotion_set() returns the promoted model.
    #[test]
    fn test_save_and_load_promotion() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        store.save_promotion("model_b").unwrap();

        let promoted = store.load_promotion_set().unwrap();
        assert!(promoted.contains("model_b"), "promoted set must contain model_b");
        assert_eq!(promoted.len(), 1);
    }

    // Test 5: save_demotion() then save_promotion() for same model — status becomes 'promoted'.
    #[test]
    fn test_promotion_overrides_demotion() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        store.save_demotion("model_x").unwrap();

        // Verify demoted
        let demoted = store.load_demotion_set().unwrap();
        assert!(demoted.contains("model_x"));

        // Now promote — should clear demotion
        store.save_promotion("model_x").unwrap();

        let demoted_after = store.load_demotion_set().unwrap();
        assert!(!demoted_after.contains("model_x"), "promotion must clear demotion");

        let promoted = store.load_promotion_set().unwrap();
        assert!(promoted.contains("model_x"), "model_x must now be in promoted set");

        // Verify DB status
        let status: String = store
            .conn
            .query_row(
                "SELECT status FROM model_reputation WHERE model_id = 'model_x'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "promoted");
    }

    // Test 6: load_into_memory() with 2 stored outcomes populates MODEL_REPUTATION correctly.
    #[test]
    fn test_load_into_memory_populates_reputation() {
        let store = ModelReputationStore::open(":memory:").unwrap();
        store.save_outcome("model_alpha", 8, 10).unwrap();
        store.save_outcome("model_beta", 3, 5).unwrap();

        // load_into_memory calls mma_engine::set_model_counts for each row
        let result = load_into_memory(&store);
        assert!(result.is_ok(), "load_into_memory must not return error");

        // Verify via mma_engine::get_model_accuracy (8/10 = 0.8)
        let alpha_accuracy = crate::mma_engine::get_model_accuracy("model_alpha");
        assert!(
            (alpha_accuracy - 0.8).abs() < 1e-6,
            "model_alpha accuracy must be 0.8, got {}",
            alpha_accuracy
        );

        // 3/5 = 0.6
        let beta_accuracy = crate::mma_engine::get_model_accuracy("model_beta");
        assert!(
            (beta_accuracy - 0.6).abs() < 1e-6,
            "model_beta accuracy must be 0.6, got {}",
            beta_accuracy
        );
    }
}
