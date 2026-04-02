//! Model Evaluation Store — SQLite-backed persistence for every AI diagnosis outcome.
//!
//! Every time a model call completes in the MMA pipeline, one EvalRecord is written to
//! the `model_evaluations` table in `mesh_kb.db`. This provides durable, structured data
//! for weekly rollup, reputation persistence, retrain export, and enhanced reports.
//!
//! Phase 290, Plan 01: EVAL-01 — Schema + open + insert + query.
//!
//! Key design choices:
//! - Shares `mesh_kb.db` with knowledge_base.rs (no extra file on disk)
//! - Uses rusqlite (same crate as knowledge_base.rs), NOT sqlx
//! - No `.unwrap()` in production code — production paths use `?`, `.ok()`, or match
//! - Migrations are idempotent (CREATE TABLE IF NOT EXISTS, CREATE INDEX IF NOT EXISTS)

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::knowledge_base::KB_PATH;

const LOG_TARGET: &str = "model-eval-store";
/// Evaluation records are stored in the same mesh_kb.db as knowledge_base.rs.
pub const EVAL_DB_PATH: &str = KB_PATH;

/// One row in the model_evaluations table — every field required by EVAL-01.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRecord {
    /// UUID v4 primary key.
    pub id: String,
    /// e.g. "deepseek/deepseek-r1-0528" — the model that produced this diagnosis.
    pub model_id: String,
    /// e.g. "pod_3" — the pod the event originated from.
    pub pod_id: String,
    /// DiagnosticTrigger variant name, e.g. "ProcessCrash".
    pub trigger_type: String,
    /// The model's root_cause (truncated to 500 chars).
    pub prediction: String,
    /// "fixed" | "failed_to_fix" | "not_applicable" | "escalated"
    pub actual_outcome: String,
    /// true if actual_outcome == "fixed".
    pub correct: bool,
    /// Cost charged to BudgetTracker for this MMA step (USD).
    pub cost_usd: f64,
    /// Utc::now().to_rfc3339() at insert time.
    pub created_at: String,
}

/// Persistent store for model evaluation records.
///
/// Open with `ModelEvalStore::open(path)`. The caller is responsible for choosing the
/// right path — production uses EVAL_DB_PATH, tests use ":memory:".
pub struct ModelEvalStore {
    conn: Connection,
}

impl ModelEvalStore {
    /// Open the evaluation store at `path`.
    ///
    /// Runs idempotent migrations before returning. Safe to call on an existing DB.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Create `model_evaluations` table and indices (idempotent — safe to call on upgrade).
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_evaluations (
                id TEXT PRIMARY KEY,
                model_id TEXT NOT NULL,
                pod_id TEXT NOT NULL,
                trigger_type TEXT NOT NULL,
                prediction TEXT NOT NULL,
                actual_outcome TEXT NOT NULL,
                correct INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_eval_model_id ON model_evaluations (model_id);
            CREATE INDEX IF NOT EXISTS idx_eval_created_at ON model_evaluations (created_at);",
        )?;
        tracing::debug!(target: LOG_TARGET, "model_evaluations schema migration complete");
        Ok(())
    }

    /// Persist a single evaluation record.
    ///
    /// Called in tier_engine after every Step 4 VERIFY (EVAL-01 requirement).
    pub fn insert(&self, record: &EvalRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO model_evaluations \
             (id, model_id, pod_id, trigger_type, prediction, actual_outcome, correct, cost_usd, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                record.id,
                record.model_id,
                record.pod_id,
                record.trigger_type,
                record.prediction,
                record.actual_outcome,
                record.correct as i64,
                record.cost_usd,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    /// Query all evaluation records for a given model, optionally filtered by time range.
    ///
    /// `from` and `to` are ISO 8601 strings (RFC 3339). Rows returned most-recent-first.
    /// Returns empty Vec (not an error) when no rows match.
    pub fn query_by_model(
        &self,
        model_id: &str,
        from: Option<&str>,
        to: Option<&str>,
    ) -> anyhow::Result<Vec<EvalRecord>> {
        let mut sql = String::from(
            "SELECT id, model_id, pod_id, trigger_type, prediction, actual_outcome, \
             correct, cost_usd, created_at \
             FROM model_evaluations WHERE model_id = ?1",
        );
        let mut positional = 1i32;

        if from.is_some() {
            positional += 1;
            sql.push_str(&format!(" AND created_at >= ?{}", positional));
        }
        if to.is_some() {
            positional += 1;
            sql.push_str(&format!(" AND created_at <= ?{}", positional));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 10000");

        let mut stmt = self.conn.prepare(&sql)?;

        // Build the params list dynamically — rusqlite requires slice of dyn ToSql.
        // We always have model_id at ?1; from/to are appended if present.
        let records = match (from, to) {
            (None, None) => stmt.query_map(params![model_id], Self::row_to_record)?,
            (Some(f), None) => stmt.query_map(params![model_id, f], Self::row_to_record)?,
            (None, Some(t)) => stmt.query_map(params![model_id, t], Self::row_to_record)?,
            (Some(f), Some(t)) => {
                stmt.query_map(params![model_id, f, t], Self::row_to_record)?
            }
        };

        let mut out = Vec::new();
        for row in records {
            match row {
                Ok(r) => out.push(r),
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to deserialize eval row — skipping");
                }
            }
        }
        Ok(out)
    }

    /// Query all evaluation records, optionally filtered by time range.
    ///
    /// Used by weekly rollup cron. Returns empty Vec (not an error) when no rows match.
    pub fn query_all(
        &self,
        from: Option<&str>,
        to: Option<&str>,
    ) -> anyhow::Result<Vec<EvalRecord>> {
        let mut sql = String::from(
            "SELECT id, model_id, pod_id, trigger_type, prediction, actual_outcome, \
             correct, cost_usd, created_at \
             FROM model_evaluations WHERE 1=1",
        );
        let mut positional = 0i32;

        if from.is_some() {
            positional += 1;
            sql.push_str(&format!(" AND created_at >= ?{}", positional));
        }
        if to.is_some() {
            positional += 1;
            sql.push_str(&format!(" AND created_at <= ?{}", positional));
        }
        sql.push_str(" ORDER BY created_at DESC LIMIT 10000");

        let mut stmt = self.conn.prepare(&sql)?;

        let records = match (from, to) {
            (None, None) => stmt.query_map([], Self::row_to_record)?,
            (Some(f), None) => stmt.query_map(params![f], Self::row_to_record)?,
            (None, Some(t)) => stmt.query_map(params![t], Self::row_to_record)?,
            (Some(f), Some(t)) => stmt.query_map(params![f, t], Self::row_to_record)?,
        };

        let mut out = Vec::new();
        for row in records {
            match row {
                Ok(r) => out.push(r),
                Err(e) => {
                    tracing::warn!(target: LOG_TARGET, error = %e, "Failed to deserialize eval row — skipping");
                }
            }
        }
        Ok(out)
    }

    /// Map a rusqlite Row to an EvalRecord.
    fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<EvalRecord> {
        Ok(EvalRecord {
            id: row.get(0)?,
            model_id: row.get(1)?,
            pod_id: row.get(2)?,
            trigger_type: row.get(3)?,
            prediction: row.get(4)?,
            actual_outcome: row.get(5)?,
            correct: row.get::<_, i64>(6)? != 0,
            cost_usd: row.get(7)?,
            created_at: row.get(8)?,
        })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(model_id: &str, correct: bool) -> EvalRecord {
        EvalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            pod_id: "pod_3".to_string(),
            trigger_type: "ProcessCrash".to_string(),
            prediction: "orphan werfault process holding HID device".to_string(),
            actual_outcome: if correct { "fixed" } else { "failed_to_fix" }.to_string(),
            correct,
            cost_usd: 0.10,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    // Test 1: ModelEvalStore::open(":memory:") returns Ok and migrates schema successfully.
    #[test]
    fn test_open_in_memory_succeeds() {
        let store = ModelEvalStore::open(":memory:");
        assert!(store.is_ok(), "open(':memory:') must succeed");

        // Verify table exists by doing a count query
        let store = store.unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM model_evaluations",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "fresh table should have 0 rows");
    }

    // Test 2: insert() writes a row; query_by_model() returns it with all fields intact.
    #[test]
    fn test_insert_and_query_by_model() {
        let store = ModelEvalStore::open(":memory:").unwrap();
        let rec = sample_record("deepseek/deepseek-r1-0528", true);
        let orig_id = rec.id.clone();
        let orig_model = rec.model_id.clone();

        store.insert(&rec).unwrap();

        let rows = store.query_by_model("deepseek/deepseek-r1-0528", None, None).unwrap();
        assert_eq!(rows.len(), 1, "should return exactly 1 row");

        let r = &rows[0];
        assert_eq!(r.id, orig_id);
        assert_eq!(r.model_id, orig_model);
        assert_eq!(r.pod_id, "pod_3");
        assert_eq!(r.trigger_type, "ProcessCrash");
        assert_eq!(r.actual_outcome, "fixed");
        assert!(r.correct);
        assert!((r.cost_usd - 0.10).abs() < 1e-9);
    }

    // Test 3: query_range() (via query_by_model with from/to) filters correctly.
    #[test]
    fn test_query_range_filters() {
        let store = ModelEvalStore::open(":memory:").unwrap();

        // Insert a record at a fixed past timestamp
        let mut old_rec = sample_record("qwen3/235b", false);
        old_rec.created_at = "2026-01-01T00:00:00Z".to_string();
        store.insert(&old_rec).unwrap();

        // Insert a record at a more recent timestamp
        let mut new_rec = sample_record("qwen3/235b", true);
        new_rec.id = uuid::Uuid::new_v4().to_string();
        new_rec.created_at = "2026-03-15T12:00:00Z".to_string();
        store.insert(&new_rec).unwrap();

        // Query only records from Feb onwards — should only get the March record
        let rows = store
            .query_by_model("qwen3/235b", Some("2026-02-01T00:00:00Z"), None)
            .unwrap();
        assert_eq!(rows.len(), 1, "range filter should exclude old record");
        assert_eq!(rows[0].created_at, "2026-03-15T12:00:00Z");

        // Query with upper bound before March — should only get the January record
        let rows = store
            .query_by_model("qwen3/235b", None, Some("2026-01-31T23:59:59Z"))
            .unwrap();
        assert_eq!(rows.len(), 1, "upper bound filter should exclude new record");
        assert_eq!(rows[0].created_at, "2026-01-01T00:00:00Z");
    }

    // Test 4: insert() with correct=true stores 1 for correct; correct=false stores 0.
    #[test]
    fn test_correct_bool_stored_correctly() {
        let store = ModelEvalStore::open(":memory:").unwrap();

        let true_rec = sample_record("model_a", true);
        let mut false_rec = sample_record("model_a", false);
        false_rec.id = uuid::Uuid::new_v4().to_string();
        false_rec.created_at = "2026-01-01T00:00:00Z".to_string(); // ensure deterministic order

        store.insert(&true_rec).unwrap();
        store.insert(&false_rec).unwrap();

        let rows = store.query_by_model("model_a", None, None).unwrap();
        assert_eq!(rows.len(), 2);

        // Find by outcome to avoid order dependency
        let correct_row = rows.iter().find(|r| r.actual_outcome == "fixed").unwrap();
        let incorrect_row = rows.iter().find(|r| r.actual_outcome == "failed_to_fix").unwrap();

        assert!(correct_row.correct, "correct=true must round-trip as true");
        assert!(!incorrect_row.correct, "correct=false must round-trip as false");

        // Verify raw INTEGER values in DB
        let correct_int: i64 = store
            .conn
            .query_row(
                "SELECT correct FROM model_evaluations WHERE id = ?1",
                params![correct_row.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(correct_int, 1, "correct=true must be stored as INTEGER 1");

        let incorrect_int: i64 = store
            .conn
            .query_row(
                "SELECT correct FROM model_evaluations WHERE id = ?1",
                params![incorrect_row.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(incorrect_int, 0, "correct=false must be stored as INTEGER 0");
    }

    // Test 5: query_by_model() with no matching rows returns empty Vec (not an error).
    #[test]
    fn test_query_no_matching_rows_returns_empty() {
        let store = ModelEvalStore::open(":memory:").unwrap();

        // No rows inserted for this model
        let rows = store
            .query_by_model("nonexistent/model", None, None)
            .unwrap();
        assert!(rows.is_empty(), "no matching rows must return empty Vec, not error");
    }
}
