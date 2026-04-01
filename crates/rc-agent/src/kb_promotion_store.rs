//! KB Promotion Store — SQLite-backed persistence for the KB promotion ladder.
//!
//! Stores `promotion_candidates` in the same `mesh_kb.db` as knowledge_base.rs.
//! Each candidate tracks its current stage (observed / shadow / canary / quorum / hardened),
//! shadow application count, and timestamps. Stage transitions survive rc-agent restarts.
//!
//! Phase 291, Plan 01: KBPP-01..04 — promotion state persistence.
//!
//! Key design choices:
//! - Shares `mesh_kb.db` with knowledge_base.rs and model_eval_store.rs (no extra file)
//! - Uses rusqlite (same crate as knowledge_base.rs), NOT sqlx
//! - No `.unwrap()` in production code — production paths use `?`, `.ok()`, or match
//! - Migrations are idempotent (CREATE TABLE IF NOT EXISTS, CREATE INDEX IF NOT EXISTS)

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::knowledge_base::KB_PATH;

const LOG_TARGET: &str = "kb-promotion-store";

/// One row in the `promotion_candidates` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionCandidate {
    /// Identifies the solution uniquely — foreign key to solutions.problem_hash
    pub problem_hash: String,
    /// Human-readable problem identifier, e.g. "game_crash"
    pub problem_key: String,
    /// Current ladder stage: observed | shadow | canary | quorum | hardened
    pub stage: String,
    /// ISO 8601 timestamp of when the current stage was entered
    pub stage_entered_at: String,
    /// Number of times this candidate has been applied in shadow mode
    pub shadow_applications: i64,
    /// ISO 8601 timestamp of initial insertion
    pub created_at: String,
}

/// Persistent store for KB promotion ladder state.
///
/// Open with `KbPromotionStore::open(path)`. Production uses `KB_PATH`; tests use `":memory:"`.
pub struct KbPromotionStore {
    conn: Connection,
}

impl KbPromotionStore {
    /// Open the promotion store at `path`.
    ///
    /// Runs idempotent migrations before returning. Safe to call on an existing DB.
    /// On success, logs the store path so operators can confirm the DB location.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        tracing::info!(target: LOG_TARGET, path = path, "Opening KB promotion store");
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        tracing::info!(target: LOG_TARGET, "KB promotion store migration complete");
        Ok(store)
    }

    /// Create `promotion_candidates` table and indices (idempotent — safe to call on upgrade).
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS promotion_candidates (
                problem_hash     TEXT PRIMARY KEY,
                problem_key      TEXT NOT NULL,
                stage            TEXT NOT NULL DEFAULT 'observed',
                stage_entered_at TEXT NOT NULL,
                shadow_applications INTEGER NOT NULL DEFAULT 0,
                created_at       TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_promo_stage ON promotion_candidates (stage);",
        )?;
        tracing::debug!(target: LOG_TARGET, "promotion_candidates schema migration complete");
        Ok(())
    }

    /// Insert a new candidate or update an existing one (upsert by problem_hash).
    ///
    /// On conflict, updates stage, stage_entered_at, and shadow_applications from the
    /// provided candidate. `created_at` is only set on first insert (preserved on update).
    pub fn upsert_candidate(&self, candidate: &PromotionCandidate) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO promotion_candidates
                (problem_hash, problem_key, stage, stage_entered_at, shadow_applications, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(problem_hash) DO UPDATE SET
                stage            = excluded.stage,
                stage_entered_at = excluded.stage_entered_at,
                shadow_applications = excluded.shadow_applications",
            params![
                candidate.problem_hash,
                candidate.problem_key,
                candidate.stage,
                candidate.stage_entered_at,
                candidate.shadow_applications,
                candidate.created_at,
            ],
        )?;
        Ok(())
    }

    /// Load all candidates at a given stage.
    ///
    /// Returns empty Vec (not an error) when no rows match the stage filter.
    pub fn candidates_at_stage(&self, stage: &str) -> anyhow::Result<Vec<PromotionCandidate>> {
        let mut stmt = self.conn.prepare(
            "SELECT problem_hash, problem_key, stage, stage_entered_at, shadow_applications, created_at
             FROM promotion_candidates WHERE stage = ?1",
        )?;
        let rows = stmt.query_map(params![stage], Self::row_to_candidate)?;

        let mut out = Vec::new();
        for row in rows {
            match row {
                Ok(c) => out.push(c),
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to deserialize promotion_candidate row — skipping");
                }
            }
        }
        Ok(out)
    }

    /// Load ALL candidates regardless of stage (used at startup to restore ladder state).
    ///
    /// Returns empty Vec (not an error) when no rows exist.
    pub fn all_candidates(&self) -> anyhow::Result<Vec<PromotionCandidate>> {
        let mut stmt = self.conn.prepare(
            "SELECT problem_hash, problem_key, stage, stage_entered_at, shadow_applications, created_at
             FROM promotion_candidates",
        )?;
        let rows = stmt.query_map([], Self::row_to_candidate)?;

        let mut out = Vec::new();
        for row in rows {
            match row {
                Ok(c) => out.push(c),
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to deserialize promotion_candidate row — skipping");
                }
            }
        }
        Ok(out)
    }

    /// Advance or set the stage for a candidate. Sets `stage_entered_at` to datetime('now').
    ///
    /// No-ops gracefully (0 rows updated) if problem_hash does not exist.
    pub fn update_stage(&self, problem_hash: &str, new_stage: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE promotion_candidates
             SET stage = ?1, stage_entered_at = datetime('now')
             WHERE problem_hash = ?2",
            params![new_stage, problem_hash],
        )?;
        Ok(())
    }

    /// Increment the `shadow_applications` counter by 1 for the given problem_hash.
    ///
    /// No-ops gracefully (0 rows updated) if problem_hash does not exist.
    pub fn record_shadow_application(&self, problem_hash: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE promotion_candidates
             SET shadow_applications = shadow_applications + 1
             WHERE problem_hash = ?1",
            params![problem_hash],
        )?;
        Ok(())
    }

    /// Get the current `shadow_applications` count for a given problem_hash.
    ///
    /// Returns 0 (not an error) if the problem_hash does not exist.
    pub fn shadow_application_count(&self, problem_hash: &str) -> anyhow::Result<i64> {
        let count: i64 = self
            .conn
            .query_row(
                "SELECT shadow_applications FROM promotion_candidates WHERE problem_hash = ?1",
                params![problem_hash],
                |row| row.get(0),
            )
            .unwrap_or(0);
        Ok(count)
    }

    /// Map a rusqlite Row to a PromotionCandidate.
    fn row_to_candidate(row: &rusqlite::Row) -> rusqlite::Result<PromotionCandidate> {
        Ok(PromotionCandidate {
            problem_hash: row.get(0)?,
            problem_key: row.get(1)?,
            stage: row.get(2)?,
            stage_entered_at: row.get(3)?,
            shadow_applications: row.get(4)?,
            created_at: row.get(5)?,
        })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_candidate(hash: &str, key: &str, stage: &str) -> PromotionCandidate {
        PromotionCandidate {
            problem_hash: hash.to_string(),
            problem_key: key.to_string(),
            stage: stage.to_string(),
            stage_entered_at: Utc::now().to_rfc3339(),
            shadow_applications: 0,
            created_at: Utc::now().to_rfc3339(),
        }
    }

    // Test 1: open(":memory:") succeeds and creates promotion_candidates table
    #[test]
    fn test_open_in_memory_succeeds() {
        let store = KbPromotionStore::open(":memory:");
        assert!(store.is_ok(), "open(':memory:') must succeed");

        let store = store.unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM promotion_candidates", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "fresh table should have 0 rows");
    }

    // Test 2: upsert_candidate inserts a new candidate; re-calling with same hash updates stage
    #[test]
    fn test_upsert_inserts_and_updates() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        // Insert
        let c = make_candidate("abc123", "game_crash", "shadow");
        store.upsert_candidate(&c).unwrap();

        let rows = store.candidates_at_stage("shadow").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stage, "shadow");

        // Update (upsert again with different stage)
        let c2 = PromotionCandidate {
            stage: "canary".to_string(),
            ..make_candidate("abc123", "game_crash", "canary")
        };
        store.upsert_candidate(&c2).unwrap();

        let rows_shadow = store.candidates_at_stage("shadow").unwrap();
        assert_eq!(rows_shadow.len(), 0, "no longer in shadow after upsert with canary");

        let rows_canary = store.candidates_at_stage("canary").unwrap();
        assert_eq!(rows_canary.len(), 1);
        assert_eq!(rows_canary[0].stage, "canary");
    }

    // Test 3: candidates_at_stage returns only entries matching the stage filter
    #[test]
    fn test_candidates_at_stage_filter() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        store.upsert_candidate(&make_candidate("h1", "crash", "shadow")).unwrap();
        store.upsert_candidate(&make_candidate("h2", "freeze", "canary")).unwrap();
        store.upsert_candidate(&make_candidate("h3", "netfail", "shadow")).unwrap();

        let shadows = store.candidates_at_stage("shadow").unwrap();
        assert_eq!(shadows.len(), 2, "exactly 2 shadow candidates");

        let canaries = store.candidates_at_stage("canary").unwrap();
        assert_eq!(canaries.len(), 1, "exactly 1 canary candidate");

        let hardened = store.candidates_at_stage("hardened").unwrap();
        assert_eq!(hardened.len(), 0, "no hardened candidates");
    }

    // Test 4: update_stage changes stage and sets stage_entered_at
    #[test]
    fn test_update_stage_changes_stage() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        store.upsert_candidate(&make_candidate("hash_x", "test_key", "shadow")).unwrap();
        store.update_stage("hash_x", "canary").unwrap();

        let rows = store.candidates_at_stage("canary").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].stage, "canary");

        let old_shadows = store.candidates_at_stage("shadow").unwrap();
        assert_eq!(old_shadows.len(), 0);
    }

    // Test 5: After upsert + update_stage, data persists correctly in the same store instance
    // (simulating restart — data survives within the same in-memory connection per SQLite semantics)
    #[test]
    fn test_data_survives_across_store_usage() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        let c = make_candidate("persist_hash", "persist_key", "shadow");
        store.upsert_candidate(&c).unwrap();
        store.update_stage("persist_hash", "canary").unwrap();

        // Confirm stage is now canary in the same store instance
        let rows = store.candidates_at_stage("canary").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].problem_hash, "persist_hash");
        assert_eq!(rows[0].stage, "canary");

        // Confirm all_candidates also reflects the updated stage
        let all = store.all_candidates().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].stage, "canary", "all_candidates must reflect canary stage");
    }

    // Test 6: shadow_application_count returns sum of shadow_applications for problem_hash
    #[test]
    fn test_shadow_application_count() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        store.upsert_candidate(&make_candidate("count_hash", "count_key", "shadow")).unwrap();

        // Initial count should be 0
        let count = store.shadow_application_count("count_hash").unwrap();
        assert_eq!(count, 0, "initial count must be 0");

        // Record 3 applications
        store.record_shadow_application("count_hash").unwrap();
        store.record_shadow_application("count_hash").unwrap();
        store.record_shadow_application("count_hash").unwrap();

        let count = store.shadow_application_count("count_hash").unwrap();
        assert_eq!(count, 3, "count must be 3 after 3 applications");
    }

    // Test 7: record_shadow_application increments by 1
    #[test]
    fn test_record_shadow_application_increments() {
        let store = KbPromotionStore::open(":memory:").unwrap();

        store.upsert_candidate(&make_candidate("inc_hash", "inc_key", "shadow")).unwrap();

        let before = store.shadow_application_count("inc_hash").unwrap();
        store.record_shadow_application("inc_hash").unwrap();
        let after = store.shadow_application_count("inc_hash").unwrap();

        assert_eq!(after - before, 1, "record_shadow_application must increment by exactly 1");
    }
}
