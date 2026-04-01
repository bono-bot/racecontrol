//! Model Evaluation Rollup — EVAL-02.
//!
//! Reads all evaluation records from the past 7 days via `ModelEvalStore::query_all()`,
//! computes per-model accuracy and cost-per-correct-diagnosis, and writes one
//! `EvalRollup` row per model to the `model_eval_rollups` table in `mesh_kb.db`.
//!
//! A weekly cron fires every Sunday at midnight IST (same pattern as weekly_report.rs).
//!
//! Phase 290, Plan 02: EVAL-02 — Rollup schema, computation, and weekly cron.
//!
//! Key design choices:
//! - `model_eval_rollups` table lives in the same `mesh_kb.db` as `model_evaluations`
//! - Pure `compute_rollup()` function — no IO, fully unit-testable
//! - Never holds Mutex across `.await` — guard acquired and dropped in a tight `{ }` block
//! - Zero divide-by-zero risk — cost_per_correct_usd = 0.0 when correct_runs = 0

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::model_eval_store::{EvalRecord, ModelEvalStore, EVAL_DB_PATH};

const LOG_TARGET: &str = "eval-rollup";

// ─── EvalRollup struct ────────────────────────────────────────────────────────

/// One row in `model_eval_rollups` — one row per (model_id, weekly window).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRollup {
    /// UUID v4 primary key.
    pub id: String,
    /// e.g. "deepseek/deepseek-r1-0528"
    pub model_id: String,
    /// ISO 8601 UTC — earliest created_at in the group (or period window start).
    pub period_start: String,
    /// ISO 8601 UTC — latest created_at in the group (or period window end).
    pub period_end: String,
    /// Total diagnosis runs in this period.
    pub total_runs: i64,
    /// Number of runs where correct == true.
    pub correct_runs: i64,
    /// correct_runs / total_runs; 0.0 if total_runs == 0.
    pub accuracy: f64,
    /// total_cost / total_runs; 0.0 if total_runs == 0.
    pub avg_cost_usd: f64,
    /// total_cost / correct_runs; 0.0 if correct_runs == 0 (avoid divide-by-zero).
    pub cost_per_correct_usd: f64,
    /// Utc::now().to_rfc3339() at insert time.
    pub created_at: String,
}

// ─── ModelEvalRollupStore struct ──────────────────────────────────────────────

/// Write-only store for evaluation rollup rows.
///
/// Opens the same `mesh_kb.db` as `ModelEvalStore`. Call `open(path)` with
/// `EVAL_DB_PATH` in production and `":memory:"` in tests.
pub struct ModelEvalRollupStore {
    conn: Connection,
}

impl ModelEvalRollupStore {
    /// Open the rollup store at `path` and run idempotent migrations.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.run_migrations()?;
        Ok(store)
    }

    /// Create `model_eval_rollups` table and indices (idempotent — safe on upgrade).
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS model_eval_rollups (
                id TEXT PRIMARY KEY,
                model_id TEXT NOT NULL,
                period_start TEXT NOT NULL,
                period_end TEXT NOT NULL,
                total_runs INTEGER NOT NULL DEFAULT 0,
                correct_runs INTEGER NOT NULL DEFAULT 0,
                accuracy REAL NOT NULL DEFAULT 0.0,
                avg_cost_usd REAL NOT NULL DEFAULT 0.0,
                cost_per_correct_usd REAL NOT NULL DEFAULT 0.0,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_rollup_model_id ON model_eval_rollups (model_id);
            CREATE INDEX IF NOT EXISTS idx_rollup_period_end ON model_eval_rollups (period_end);",
        )?;
        tracing::debug!(target: LOG_TARGET, "model_eval_rollups schema migration complete");
        Ok(())
    }

    /// Insert a single rollup row. Called once per model after compute_rollup().
    pub fn insert_rollup(&self, rollup: &EvalRollup) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO model_eval_rollups \
             (id, model_id, period_start, period_end, total_runs, correct_runs, \
              accuracy, avg_cost_usd, cost_per_correct_usd, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                rollup.id,
                rollup.model_id,
                rollup.period_start,
                rollup.period_end,
                rollup.total_runs,
                rollup.correct_runs,
                rollup.accuracy,
                rollup.avg_cost_usd,
                rollup.cost_per_correct_usd,
                rollup.created_at,
            ],
        )?;
        Ok(())
    }
}

// ─── compute_rollup — pure computation, no IO ─────────────────────────────────

/// Aggregate evaluation records into per-model rollups.
///
/// Algorithm:
/// 1. Group records by model_id.
/// 2. For each group, compute accuracy, avg_cost_usd, cost_per_correct_usd.
/// 3. period_start = earliest created_at in group; period_end = latest.
/// 4. Groups with 0 records are skipped entirely (never an error row).
///
/// Returns one `EvalRollup` per model that had at least one record.
pub fn compute_rollup(records: &[EvalRecord]) -> Vec<EvalRollup> {
    // Group records by model_id.
    let mut groups: HashMap<String, Vec<&EvalRecord>> = HashMap::new();
    for rec in records {
        groups.entry(rec.model_id.clone()).or_default().push(rec);
    }

    let mut rollups = Vec::with_capacity(groups.len());

    for (model_id, recs) in groups {
        let total_runs = recs.len() as i64;
        if total_runs == 0 {
            // Defensive: should never happen because HashMap entry requires at least one record.
            continue;
        }

        let correct_runs = recs.iter().filter(|r| r.correct).count() as i64;
        let total_cost: f64 = recs.iter().map(|r| r.cost_usd).sum();

        let accuracy = correct_runs as f64 / total_runs as f64;
        let avg_cost_usd = total_cost / total_runs as f64;
        let cost_per_correct_usd = if correct_runs > 0 {
            total_cost / correct_runs as f64
        } else {
            0.0 // avoid divide-by-zero when no correct diagnoses
        };

        // period_start = earliest created_at; period_end = latest.
        let period_start = recs
            .iter()
            .map(|r| r.created_at.as_str())
            .min()
            .unwrap_or("")
            .to_string();
        let period_end = recs
            .iter()
            .map(|r| r.created_at.as_str())
            .max()
            .unwrap_or("")
            .to_string();

        rollups.push(EvalRollup {
            id: uuid::Uuid::new_v4().to_string(),
            model_id,
            period_start,
            period_end,
            total_runs,
            correct_runs,
            accuracy,
            avg_cost_usd,
            cost_per_correct_usd,
            created_at: Utc::now().to_rfc3339(),
        });
    }

    rollups
}

// ─── Weekly cron ──────────────────────────────────────────────────────────────

/// Spawn the weekly eval rollup cron.
///
/// Sleeps until next Sunday midnight IST (reusing `weekly_report::seconds_until_next_sunday_midnight_ist`),
/// then reads the last 7 days of eval records and writes one rollup row per model.
///
/// The `eval_store` Arc<Mutex> is never held across `.await` — the guard is acquired and
/// dropped in a tight `{ }` block before any async work begins.
pub fn spawn(eval_store: Arc<Mutex<ModelEvalStore>>) {
    tokio::spawn(async move {
        tracing::info!(
            target: "state",
            task = "eval_rollup",
            event = "lifecycle",
            "lifecycle: started"
        );
        loop {
            let secs = crate::weekly_report::seconds_until_next_sunday_midnight_ist();
            let jitter_secs: u64 = rand::random::<u64>() % 300; // 0-5 min jitter
            tracing::info!(
                target: LOG_TARGET,
                secs_until_rollup = secs,
                "Sleeping until next Sunday midnight IST for eval rollup"
            );
            tokio::time::sleep(std::time::Duration::from_secs(secs + jitter_secs)).await;
            run_weekly_rollup(&eval_store).await;
        }
    });
}

/// Execute one weekly rollup cycle.
///
/// 1. Compute 7-day window.
/// 2. Lock eval_store, call query_all(), drop lock immediately.
/// 3. compute_rollup() — pure, no IO.
/// 4. Open ModelEvalRollupStore via spawn_blocking (rusqlite conn creation is sync).
/// 5. Insert each rollup row.
/// 6. Log summary.
async fn run_weekly_rollup(eval_store: &Arc<Mutex<ModelEvalStore>>) {
    let to = Utc::now().to_rfc3339();
    let from = (Utc::now() - chrono::Duration::days(7)).to_rfc3339();

    // Acquire lock, query records, drop lock before any async work.
    let records = {
        match eval_store.lock() {
            Ok(guard) => match guard.query_all(Some(&from), Some(&to)) {
                Ok(recs) => recs,
                Err(e) => {
                    tracing::warn!(
                        target: LOG_TARGET,
                        error = %e,
                        "Failed to query eval records for rollup — skipping cycle"
                    );
                    return;
                }
            },
            Err(e) => {
                tracing::warn!(
                    target: LOG_TARGET,
                    error = %e,
                    "Eval store Mutex poisoned — skipping rollup cycle"
                );
                return;
            }
        }
    }; // guard dropped here

    let rollups = compute_rollup(&records);
    let model_count = rollups.len();

    if model_count == 0 {
        tracing::info!(
            target: LOG_TARGET,
            "EVAL-02: no eval records in past 7 days — rollup skipped"
        );
        return;
    }

    // Open rollup store in spawn_blocking (rusqlite Connection creation is sync).
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let rollup_store = ModelEvalRollupStore::open(EVAL_DB_PATH)?;
        for rollup in &rollups {
            rollup_store.insert_rollup(rollup)?;
        }
        Ok(())
    })
    .await;

    match result {
        Ok(Ok(())) => {
            tracing::info!(
                target: LOG_TARGET,
                model_count,
                "EVAL-02: weekly rollup complete"
            );
        }
        Ok(Err(e)) => {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "EVAL-02: failed to insert rollup rows"
            );
        }
        Err(e) => {
            tracing::warn!(
                target: LOG_TARGET,
                error = %e,
                "EVAL-02: spawn_blocking for rollup store panicked"
            );
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_eval_record(model_id: &str, correct: bool, cost_usd: f64) -> EvalRecord {
        EvalRecord {
            id: uuid::Uuid::new_v4().to_string(),
            model_id: model_id.to_string(),
            pod_id: "pod_1".to_string(),
            trigger_type: "ProcessCrash".to_string(),
            prediction: "orphan werfault process".to_string(),
            actual_outcome: if correct { "fixed" } else { "failed_to_fix" }.to_string(),
            correct,
            cost_usd,
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    // Test 1: ModelEvalRollupStore::open(":memory:") returns Ok and migrates schema.
    #[test]
    fn test_rollup_store_open_migrates_schema() {
        let store = ModelEvalRollupStore::open(":memory:");
        assert!(store.is_ok(), "open(':memory:') must succeed");

        let store = store.unwrap();
        let count: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM model_eval_rollups",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "fresh table should have 0 rows");
    }

    // Test 2: compute_rollup() with 10 records for model A (7 correct, 3 incorrect).
    #[test]
    fn test_compute_rollup_accuracy() {
        let mut records: Vec<EvalRecord> = Vec::new();
        for _ in 0..7 {
            records.push(make_eval_record("model_a", true, 0.10));
        }
        for _ in 0..3 {
            records.push(make_eval_record("model_a", false, 0.10));
        }

        let rollups = compute_rollup(&records);
        assert_eq!(rollups.len(), 1, "should return exactly 1 rollup for model_a");

        let r = &rollups[0];
        assert_eq!(r.model_id, "model_a");
        assert_eq!(r.total_runs, 10);
        assert_eq!(r.correct_runs, 7);
        assert!((r.accuracy - 0.7).abs() < 1e-9, "accuracy should be 0.7, got {}", r.accuracy);
    }

    // Test 3: compute_rollup() with records for two models returns two EvalRollup rows.
    #[test]
    fn test_compute_rollup_two_models() {
        let mut records: Vec<EvalRecord> = Vec::new();
        // Model A: 3 records
        for _ in 0..3 {
            records.push(make_eval_record("model_a", true, 0.10));
        }
        // Model B: 2 records
        for _ in 0..2 {
            records.push(make_eval_record("model_b", false, 0.05));
        }

        let rollups = compute_rollup(&records);
        assert_eq!(rollups.len(), 2, "should return one rollup per model");

        let model_ids: Vec<&str> = rollups.iter().map(|r| r.model_id.as_str()).collect();
        assert!(model_ids.contains(&"model_a"), "model_a must be present");
        assert!(model_ids.contains(&"model_b"), "model_b must be present");

        let a = rollups.iter().find(|r| r.model_id == "model_a").unwrap();
        assert_eq!(a.total_runs, 3);
        assert_eq!(a.correct_runs, 3);

        let b = rollups.iter().find(|r| r.model_id == "model_b").unwrap();
        assert_eq!(b.total_runs, 2);
        assert_eq!(b.correct_runs, 0);
    }

    // Test 4: compute_rollup() with zero records returns empty output.
    #[test]
    fn test_compute_rollup_empty_input() {
        let records: Vec<EvalRecord> = Vec::new();
        let rollups = compute_rollup(&records);
        assert!(rollups.is_empty(), "empty input must produce empty output");
    }

    // Test 5: cost_per_correct_usd is computed correctly; if correct_runs=0, value is 0.0.
    #[test]
    fn test_cost_per_correct_divide_by_zero_safe() {
        // All incorrect — correct_runs = 0
        let mut records: Vec<EvalRecord> = Vec::new();
        for _ in 0..5 {
            records.push(make_eval_record("model_zero", false, 0.20));
        }

        let rollups = compute_rollup(&records);
        assert_eq!(rollups.len(), 1);
        let r = &rollups[0];
        assert_eq!(r.correct_runs, 0);
        assert_eq!(
            r.cost_per_correct_usd, 0.0,
            "cost_per_correct_usd must be 0.0 when correct_runs=0, not NaN or Inf"
        );
        assert!(
            r.cost_per_correct_usd.is_finite(),
            "cost_per_correct_usd must be finite"
        );

        // All correct — verify actual cost_per_correct
        let mut records2: Vec<EvalRecord> = Vec::new();
        for _ in 0..4 {
            records2.push(make_eval_record("model_all_correct", true, 0.25));
        }
        let rollups2 = compute_rollup(&records2);
        let r2 = &rollups2[0];
        assert_eq!(r2.correct_runs, 4);
        // total_cost = 1.0, correct_runs = 4 → cost_per_correct = 0.25
        assert!(
            (r2.cost_per_correct_usd - 0.25).abs() < 1e-9,
            "cost_per_correct_usd should be 0.25, got {}",
            r2.cost_per_correct_usd
        );
    }
}
