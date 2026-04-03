//! Budget Tracker Store — SQLite-backed persistence for BudgetTracker state.
//!
//! Prevents budget overrun after rc-agent restarts. Without persistence, a pod that
//! spent $8 today can restart and spend another $8 — exceeding the $10 daily limit.
//!
//! Design choices (same as model_reputation_store.rs):
//! - Shares `mesh_kb.db` with knowledge_base.rs, model_reputation_store.rs, model_eval_store.rs
//! - Uses rusqlite (same crate as knowledge_base.rs), NOT sqlx
//! - No `.unwrap()` in production code — all paths use `?`, `.ok()`, or match
//! - Migrations are idempotent (CREATE TABLE IF NOT EXISTS)
//! - `load()` must be called before `tier_engine::spawn()` so the first diagnosis round
//!   uses correct budget state.

use chrono::{FixedOffset, Utc};
use rusqlite::{params, Connection};

use crate::knowledge_base::KB_PATH;

const LOG_TARGET: &str = "budget-tracker-store";

/// Budget state is stored in the same mesh_kb.db as knowledge_base.rs.
pub const BUDGET_DB_PATH: &str = KB_PATH;

/// Persistent store for budget tracker state.
///
/// Open with `BudgetTrackerStore::open(path)`. Safe to call on an existing DB.
pub struct BudgetTrackerStore {
    conn: Connection,
}

impl BudgetTrackerStore {
    /// Open the budget store at `path`.
    ///
    /// Runs idempotent migrations before returning. Safe to call on an existing DB.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Create `budget_state` table (idempotent — safe to call on upgrade).
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS budget_state (
                id TEXT PRIMARY KEY DEFAULT 'default',
                spent_today REAL NOT NULL DEFAULT 0.0,
                monthly_spent REAL NOT NULL DEFAULT 0.0,
                model_calls_today INTEGER NOT NULL DEFAULT 0,
                budget_date TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )?;
        tracing::debug!(target: LOG_TARGET, "budget_state schema migration complete");
        Ok(())
    }

    /// Persist current budget state to SQLite.
    ///
    /// Called by `BudgetTracker::record_spend()` after every cost record — ensures
    /// persistence is immediate, not batched.
    pub fn save(
        &self,
        spent_today: f64,
        monthly_spent: f64,
        model_calls_today: u32,
        budget_date: &str,
    ) -> anyhow::Result<()> {
        let updated_at = ist_now_rfc3339();
        self.conn.execute(
            "INSERT INTO budget_state (id, spent_today, monthly_spent, model_calls_today, budget_date, updated_at)
             VALUES ('default', ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
               spent_today = excluded.spent_today,
               monthly_spent = excluded.monthly_spent,
               model_calls_today = excluded.model_calls_today,
               budget_date = excluded.budget_date,
               updated_at = excluded.updated_at",
            params![spent_today, monthly_spent, model_calls_today as i64, budget_date, updated_at],
        )?;
        Ok(())
    }

    /// Load persisted budget state from SQLite.
    ///
    /// Returns `None` if no state has been persisted yet (first boot).
    pub fn load(&self) -> anyhow::Result<Option<BudgetStateRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT spent_today, monthly_spent, model_calls_today, budget_date, updated_at
             FROM budget_state WHERE id = 'default'",
        )?;
        let mut rows = stmt.query_map([], |row| {
            Ok(BudgetStateRow {
                spent_today: row.get(0)?,
                monthly_spent: row.get(1)?,
                model_calls_today: row.get::<_, i64>(2)? as u32,
                budget_date: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;

        match rows.next() {
            Some(Ok(row)) => Ok(Some(row)),
            Some(Err(e)) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    error = %e,
                    "Failed to deserialize budget_state row"
                );
                Ok(None)
            }
            None => Ok(None),
        }
    }
}

/// One row from the budget_state table.
#[derive(Debug, Clone)]
pub struct BudgetStateRow {
    pub spent_today: f64,
    pub monthly_spent: f64,
    pub model_calls_today: u32,
    pub budget_date: String,
    pub updated_at: String,
}

/// IST timestamp in RFC 3339 format.
fn ist_now_rfc3339() -> String {
    let ist = FixedOffset::east_opt(5 * 3600 + 30 * 60)
        .unwrap_or_else(|| FixedOffset::east_opt(0).expect("UTC offset 0 is always valid"));
    Utc::now().with_timezone(&ist).to_rfc3339()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_in_memory_succeeds() {
        let store = BudgetTrackerStore::open(":memory:").unwrap();
        let count: i64 = store
            .conn
            .query_row("SELECT COUNT(*) FROM budget_state", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0, "fresh table should have 0 rows");
    }

    #[test]
    fn test_save_and_load() {
        let store = BudgetTrackerStore::open(":memory:").unwrap();
        store.save(5.50, 42.0, 7, "2026-04-03").unwrap();

        let row = store.load().unwrap().expect("should return a row");
        assert!((row.spent_today - 5.50).abs() < 0.001);
        assert!((row.monthly_spent - 42.0).abs() < 0.001);
        assert_eq!(row.model_calls_today, 7);
        assert_eq!(row.budget_date, "2026-04-03");
    }

    #[test]
    fn test_save_overwrites_previous() {
        let store = BudgetTrackerStore::open(":memory:").unwrap();
        store.save(1.0, 10.0, 2, "2026-04-03").unwrap();
        store.save(3.0, 10.0, 5, "2026-04-03").unwrap();

        let row = store.load().unwrap().expect("should return a row");
        assert!((row.spent_today - 3.0).abs() < 0.001, "should have latest value");
        assert_eq!(row.model_calls_today, 5);
    }

    #[test]
    fn test_load_empty_returns_none() {
        let store = BudgetTrackerStore::open(":memory:").unwrap();
        let row = store.load().unwrap();
        assert!(row.is_none(), "should return None on empty table");
    }

    #[test]
    fn test_persistence_survives_reopen() {
        // Use a temp file to test persistence across open/close cycles
        let dir = std::env::temp_dir();
        let path = dir.join("budget_tracker_test.db");
        let path_str = path.to_str().unwrap();

        // Clean up from any previous test run
        let _ = std::fs::remove_file(&path);

        {
            let store = BudgetTrackerStore::open(path_str).unwrap();
            store.save(5.0, 20.0, 3, "2026-04-03").unwrap();
        }
        // store dropped, connection closed

        {
            let store2 = BudgetTrackerStore::open(path_str).unwrap();
            let row = store2.load().unwrap().expect("should persist across reopen");
            assert!((row.spent_today - 5.0).abs() < 0.001);
            assert_eq!(row.model_calls_today, 3);
        }

        // Cleanup
        let _ = std::fs::remove_file(&path);
    }
}
