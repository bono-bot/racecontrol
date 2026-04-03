//! Local Knowledge Base — SQLite-backed solution storage for Tier 2 KB lookup.
//!
//! Each node (pod or server) maintains a local DB at C:\RacingPoint\mesh_kb.db.
//! When a diagnostic event occurs, Tier 2 looks up the problem_key in this DB.
//! If a solution with confidence > 0.8 is found, the fix is applied without model calls.
//!
//! Phase 230, Plan 01: Schema + open + normalize_problem_key + fingerprint_env
//! Phase 230, Plan 02: Confidence scoring + TTL + tier_engine wiring
//!
//! MMA-trained additions (v26.1):
//!   - `diagnosis_method` column: tracks which MMA methodology found the solution
//!     (e.g., "scanner_enumeration", "reasoner_absence", "sre_stuck_state",
//!      "code_expert_session0", "security_checklist", "consensus_5model")
//!   - This enables the KB to recommend diagnostic approaches for similar future problems
//!   - Solutions with methodology data help the fleet learn not just WHAT to fix but HOW to diagnose
//!
//! Standing rules: no .unwrap() in production code, lifecycle logging for background tasks.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::diagnostic_engine::DiagnosticTrigger;

const LOG_TARGET: &str = "knowledge-base";
pub const KB_PATH: &str = r"C:\RacingPoint\mesh_kb.db";
pub const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.8;

/// A row from the solutions table, returned by lookup().
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solution {
    pub id: String,
    pub problem_key: String,
    pub problem_hash: String,
    pub symptoms: String,
    pub environment: String,
    pub root_cause: String,
    pub fix_action: String,
    pub fix_type: String,
    pub success_count: i64,
    pub fail_count: i64,
    pub confidence: f64,
    pub cost_to_diagnose: f64,
    pub models_used: Option<String>,
    pub source_node: String,
    pub created_at: String,
    pub updated_at: String,
    pub version: i64,
    pub ttl_days: i64,
    pub tags: Option<String>,
    /// MMA diagnostic methodology that found this solution.
    /// Values: "scanner_enumeration", "reasoner_absence", "sre_stuck_state",
    /// "code_expert_session0", "security_checklist", "consensus_5model",
    /// "deterministic", "fleet_gossip", or model-specific role names.
    /// Enables the fleet to learn not just WHAT to fix but HOW to diagnose.
    pub diagnosis_method: Option<String>,
    /// MMA-First Protocol: whether this is a workaround or permanent fix.
    /// Values: "workaround", "permanent", "pending_permanent", "fallback"
    #[serde(default = "default_fix_permanence")]
    pub fix_permanence: String,
    /// How many times Q1 has applied this solution (issue recurrence count).
    #[serde(default)]
    pub recurrence_count: i64,
    /// Links a workaround to its permanent replacement solution ID.
    #[serde(default)]
    pub permanent_fix_id: Option<String>,
    /// ISO 8601 timestamp of last Q1 application.
    #[serde(default)]
    pub last_recurrence: Option<String>,
    /// ISO 8601 timestamp of last Q4 permanent fix attempt.
    #[serde(default)]
    pub permanent_attempt_at: Option<String>,
}

fn default_fix_permanence() -> String { "workaround".to_string() }

/// A row from the experiments table.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Experiment {
    pub id: String,
    pub problem_key: String,
    pub hypothesis: String,
    pub test_plan: String,
    pub result: Option<String>,
    pub cost: f64,
    pub node: String,
    pub created_at: String,
}

/// A fully promoted deterministic rule — typed struct with matchers/actions/verifier/TTL.
/// KB-05: Promoted rules stored as typed Rule structs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardenedRule {
    pub problem_key: String,
    pub matchers: Vec<String>,
    pub action: String,
    pub verifier: String,
    pub ttl_secs: i64,
    pub confidence: f64,
    pub provenance: String,
}

/// Environment fingerprint captured at diagnostic time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentFingerprint {
    pub os_version: String,
    pub build_id: String,
    pub hardware_class: String,
}

/// The local knowledge base — wraps a rusqlite Connection.
pub struct KnowledgeBase {
    conn: Connection,
}

impl KnowledgeBase {
    /// Open (or create) the local KB at the given path.
    /// Runs migrations — idempotent via CREATE TABLE IF NOT EXISTS.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        tracing::info!(target: LOG_TARGET, path = path, "Opening local knowledge base");
        let conn = Connection::open(path)?;
        let kb = Self { conn };
        kb.run_migrations()?;
        tracing::info!(target: LOG_TARGET, "Knowledge base ready");
        Ok(kb)
    }

    /// Run schema migrations. Idempotent — safe to call on existing DB.
    fn run_migrations(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS solutions (
                id TEXT PRIMARY KEY,
                problem_key TEXT NOT NULL,
                problem_hash TEXT NOT NULL,
                symptoms TEXT NOT NULL,
                environment TEXT NOT NULL,
                root_cause TEXT NOT NULL,
                fix_action TEXT NOT NULL,
                fix_type TEXT NOT NULL,
                success_count INTEGER DEFAULT 1,
                fail_count INTEGER DEFAULT 0,
                confidence REAL DEFAULT 1.0,
                cost_to_diagnose REAL DEFAULT 0,
                models_used TEXT,
                source_node TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                version INTEGER DEFAULT 1,
                ttl_days INTEGER DEFAULT 90,
                tags TEXT
            );

            CREATE TABLE IF NOT EXISTS experiments (
                id TEXT PRIMARY KEY,
                problem_key TEXT NOT NULL,
                hypothesis TEXT NOT NULL,
                test_plan TEXT NOT NULL,
                result TEXT,
                cost REAL DEFAULT 0,
                node TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_solutions_hash ON solutions(problem_hash);
            CREATE INDEX IF NOT EXISTS idx_solutions_key ON solutions(problem_key);
            CREATE INDEX IF NOT EXISTS idx_experiments_key ON experiments(problem_key);

            -- MMA v26.1: Track diagnostic methodology that found each solution
            -- ALTER TABLE is a no-op if column already exists (SQLite returns error, we ignore)
        ",
        )?;

        // Add diagnosis_method column if missing (idempotent migration)
        let has_column: bool = self.conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('solutions') WHERE name='diagnosis_method'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) > 0;

        if !has_column {
            self.conn.execute_batch(
                "ALTER TABLE solutions ADD COLUMN diagnosis_method TEXT;"
            ).ok();
        }

        // MMA-First Protocol (v31.0): 5 new columns for Q1-Q4 protocol
        let mma_columns = [
            ("fix_permanence", "TEXT DEFAULT 'workaround'"),
            ("recurrence_count", "INTEGER DEFAULT 0"),
            ("permanent_fix_id", "TEXT"),
            ("last_recurrence", "TEXT"),
            ("permanent_attempt_at", "TEXT"),
        ];
        for (col_name, col_type) in &mma_columns {
            let exists: bool = self.conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM pragma_table_info('solutions') WHERE name='{}'", col_name),
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0) > 0;
            if !exists {
                self.conn.execute_batch(
                    &format!("ALTER TABLE solutions ADD COLUMN {} {};", col_name, col_type)
                ).ok();
            }
        }

        // Phase 278: promotion_status column for KB hardening ladder
        let has_promotion_status: bool = self.conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('solutions') WHERE name='promotion_status'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) > 0;
        if !has_promotion_status {
            self.conn.execute_batch(
                "ALTER TABLE solutions ADD COLUMN promotion_status TEXT DEFAULT 'observed';"
            ).ok();
        }

        // Phase 278: promoted_at timestamp for tracking time-in-status
        let has_promoted_at: bool = self.conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('solutions') WHERE name='promoted_at'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .unwrap_or(0) > 0;
        if !has_promoted_at {
            self.conn.execute_batch(
                "ALTER TABLE solutions ADD COLUMN promoted_at TEXT;"
            ).ok();
        }

        // Phase 278: hardened_rules table for fully promoted deterministic rules
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hardened_rules (
                problem_key TEXT PRIMARY KEY,
                matchers TEXT NOT NULL,
                action TEXT NOT NULL,
                verifier TEXT NOT NULL,
                ttl_secs INTEGER NOT NULL,
                confidence REAL NOT NULL,
                provenance TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_hardened_rules_key ON hardened_rules(problem_key);"
        )?;

        // Phase 278: solution_nodes table for tracking distinct nodes per solution
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS solution_nodes (
                problem_hash TEXT NOT NULL,
                node_id TEXT NOT NULL,
                success_count INTEGER DEFAULT 0,
                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (problem_hash, node_id)
            );"
        )?;

        // Index on stable_hash for two-tier lookup (added if not exists)
        self.conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_solutions_stable_hash ON solutions(problem_key);"
        ).ok();

        // KB hit rate metrics table — tracks Q1 hit/miss for quality assessment
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS kb_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                problem_key TEXT NOT NULL,
                result TEXT NOT NULL,
                tier INTEGER NOT NULL,
                timestamp TEXT NOT NULL DEFAULT (datetime('now'))
            );
            CREATE INDEX IF NOT EXISTS idx_kb_metrics_ts ON kb_metrics(timestamp);"
        )?;

        // CGP + Plan Manager tables (v32.0)
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS diagnosis_plans (
                plan_id TEXT PRIMARY KEY,
                incident_id TEXT NOT NULL,
                problem_key TEXT NOT NULL,
                steps_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                completed_at TEXT,
                tier TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_plans_incident ON diagnosis_plans(incident_id);

            CREATE TABLE IF NOT EXISTS diagnosis_audits (
                incident_id TEXT PRIMARY KEY,
                audit_json TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_audits_timestamp ON diagnosis_audits(timestamp);"
        )?;

        tracing::debug!(target: LOG_TARGET, "Migrations complete");
        Ok(())
    }

    /// Return the underlying connection for direct queries.
    #[allow(dead_code)]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Store a solution in the local KB.
    /// Uses INSERT OR REPLACE — if the same id exists, it is overwritten.
    /// Rejects "periodic" problem_key — routine health checks don't produce actionable solutions.
    pub fn store_solution(&self, solution: &Solution) -> anyhow::Result<()> {
        if solution.problem_key == "periodic" {
            tracing::debug!(target: LOG_TARGET, "Skipping KB store for periodic problem_key — no actionable solution");
            return Ok(());
        }
        self.conn.execute(
            "INSERT OR REPLACE INTO solutions (
                id, problem_key, problem_hash, symptoms, environment,
                root_cause, fix_action, fix_type, success_count, fail_count,
                confidence, cost_to_diagnose, models_used, source_node,
                created_at, updated_at, version, ttl_days, tags, diagnosis_method,
                fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20,
                ?21, ?22, ?23, ?24, ?25
            )",
            params![
                solution.id,
                solution.problem_key,
                solution.problem_hash,
                solution.symptoms,
                solution.environment,
                solution.root_cause,
                solution.fix_action,
                solution.fix_type,
                solution.success_count,
                solution.fail_count,
                solution.confidence,
                solution.cost_to_diagnose,
                solution.models_used,
                solution.source_node,
                solution.created_at,
                solution.updated_at,
                solution.version,
                solution.ttl_days,
                solution.tags,
                solution.diagnosis_method,
                solution.fix_permanence,
                solution.recurrence_count,
                solution.permanent_fix_id,
                solution.last_recurrence,
                solution.permanent_attempt_at,
            ],
        )?;
        tracing::debug!(
            target: LOG_TARGET,
            id = %solution.id,
            problem_key = %solution.problem_key,
            confidence = solution.confidence,
            "Solution stored in local KB"
        );
        Ok(())
    }

    /// Look up a solution by problem_hash.
    /// Returns Some(solution) only if confidence >= HIGH_CONFIDENCE_THRESHOLD (0.8).
    /// Returns None if no match or confidence too low.
    pub fn lookup(&self, problem_hash: &str) -> anyhow::Result<Option<Solution>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, problem_key, problem_hash, symptoms, environment,
                    root_cause, fix_action, fix_type, success_count, fail_count,
                    confidence, cost_to_diagnose, models_used, source_node,
                    created_at, updated_at, version, ttl_days, tags, diagnosis_method,
                    fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
             FROM solutions
             WHERE problem_hash = ?1
               AND confidence >= ?2
             ORDER BY confidence DESC
             LIMIT 1",
        )?;

        let result = stmt.query_row(params![problem_hash, HIGH_CONFIDENCE_THRESHOLD], |row| {
            Ok(Solution {
                id: row.get(0)?,
                problem_key: row.get(1)?,
                problem_hash: row.get(2)?,
                symptoms: row.get(3)?,
                environment: row.get(4)?,
                root_cause: row.get(5)?,
                fix_action: row.get(6)?,
                fix_type: row.get(7)?,
                success_count: row.get(8)?,
                fail_count: row.get(9)?,
                confidence: row.get(10)?,
                cost_to_diagnose: row.get(11)?,
                models_used: row.get(12)?,
                source_node: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                version: row.get(16)?,
                ttl_days: row.get(17)?,
                tags: row.get(18)?,
                diagnosis_method: row.get(19)?,
                fix_permanence: row.get::<_, Option<String>>(20)?.unwrap_or_else(|| "workaround".to_string()),
                recurrence_count: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
                permanent_fix_id: row.get(22)?,
                last_recurrence: row.get(23)?,
                permanent_attempt_at: row.get(24)?,
            })
        });

        match result {
            Ok(solution) => {
                tracing::debug!(
                    target: LOG_TARGET,
                    problem_hash = problem_hash,
                    confidence = solution.confidence,
                    "KB hit: solution found"
                );
                Ok(Some(solution))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                tracing::debug!(target: LOG_TARGET, problem_hash = problem_hash, "KB miss: no solution found");
                Ok(None)
            }
            Err(e) => Err(anyhow::anyhow!("KB lookup error: {}", e)),
        }
    }

    /// Record an experiment in the experiments table.
    /// Uses INSERT OR IGNORE — experiments are append-only, never overwrite.
    #[allow(dead_code)]
    pub fn record_experiment(&self, exp: &Experiment) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO experiments (
                id, problem_key, hypothesis, test_plan, result, cost, node, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                exp.id,
                exp.problem_key,
                exp.hypothesis,
                exp.test_plan,
                exp.result,
                exp.cost,
                exp.node,
                exp.created_at,
            ],
        )?;
        tracing::debug!(
            target: LOG_TARGET,
            id = %exp.id,
            problem_key = %exp.problem_key,
            "Experiment recorded"
        );
        Ok(())
    }

    /// Find an open experiment (result IS NULL) for a given problem_key.
    /// Used by Phase 231 to avoid starting duplicate diagnosis work.
    #[allow(dead_code)]
    pub fn get_open_experiment(&self, problem_key: &str) -> anyhow::Result<Option<Experiment>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, problem_key, hypothesis, test_plan, result, cost, node, created_at
             FROM experiments
             WHERE problem_key = ?1
               AND result IS NULL
             ORDER BY created_at DESC
             LIMIT 1",
        )?;

        let result = stmt.query_row(params![problem_key], |row| {
            Ok(Experiment {
                id: row.get(0)?,
                problem_key: row.get(1)?,
                hypothesis: row.get(2)?,
                test_plan: row.get(3)?,
                result: row.get(4)?,
                cost: row.get(5)?,
                node: row.get(6)?,
                created_at: row.get(7)?,
            })
        });

        match result {
            Ok(exp) => Ok(Some(exp)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Experiment lookup error: {}", e)),
        }
    }

    /// Update solution outcome — increment success or fail count, recalculate confidence.
    /// KB-05: confidence = success / (success + fail), auto-demotion on failure.
    pub fn record_outcome(&self, solution_id: &str, success: bool) -> anyhow::Result<()> {
        let field = if success {
            "success_count"
        } else {
            "fail_count"
        };
        self.conn.execute(
            &format!(
                "UPDATE solutions SET {field} = {field} + 1,
                 confidence = CAST(success_count + CASE WHEN '{field}' = 'success_count' THEN 1 ELSE 0 END AS REAL)
                            / CAST(success_count + fail_count + 1 AS REAL),
                 updated_at = datetime('now')
                 WHERE id = ?1"
            ),
            params![solution_id],
        )?;
        tracing::info!(
            target: LOG_TARGET,
            id = solution_id,
            success = success,
            "Solution outcome recorded — confidence recalculated"
        );
        Ok(())
    }

    /// Record a KB hit or miss for metrics tracking.
    ///
    /// Called by `mma_decision()` in tier_engine to track Q1 lookup quality.
    /// `result` should be "hit" or "miss". `tier` is the tier that handled it.
    pub fn record_kb_metric(&self, problem_key: &str, result: &str, tier: u8) {
        if let Err(e) = self.conn.execute(
            "INSERT INTO kb_metrics (problem_key, result, tier, timestamp) VALUES (?1, ?2, ?3, datetime('now'))",
            params![problem_key, result, tier as i64],
        ) {
            tracing::debug!(target: LOG_TARGET, error = %e, "Failed to record KB metric");
        }
    }

    /// Get KB hit rate for the last 24 hours.
    ///
    /// Returns (hits, misses, hit_rate).
    #[allow(dead_code)]
    pub fn kb_hit_rate_24h(&self) -> anyhow::Result<(u64, u64, f64)> {
        let hits: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM kb_metrics WHERE result = 'hit' AND timestamp > datetime('now', '-24 hours')",
            [],
            |r| r.get(0),
        )?;
        let misses: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM kb_metrics WHERE result = 'miss' AND timestamp > datetime('now', '-24 hours')",
            [],
            |r| r.get(0),
        )?;
        let total = hits + misses;
        let rate = if total > 0 { hits as f64 / total as f64 } else { 0.0 };
        Ok((hits as u64, misses as u64, rate))
    }

    /// Archive solutions unused for > ttl_days.
    /// KB-06: TTL expiration — 90 days unused → auto-archive.
    /// Returns the count of archived solutions.
    pub fn archive_expired_solutions(&self) -> anyhow::Result<usize> {
        let count = self.conn.execute(
            "DELETE FROM solutions WHERE
             julianday('now') - julianday(updated_at) > ttl_days",
            [],
        )?;
        if count > 0 {
            tracing::info!(target: LOG_TARGET, count = count, "Archived expired solutions (TTL exceeded)");
        }
        Ok(count)
    }

    /// Count total solutions in the KB.
    pub fn solution_count(&self) -> anyhow::Result<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM solutions", [], |r| r.get(0))?;
        Ok(count)
    }

    /// Convenience wrapper for recording a game launch fix (GAME-04).
    /// Delegates to `record_resolution` with game-specific defaults.
    pub fn record_game_fix(&self, cause: &str, fix: &str, node_id: &str) -> anyhow::Result<()> {
        let problem_key = "game_launch_fail";
        let problem_hash = format!("game_launch:{}", cause);
        self.record_resolution(
            problem_key,
            &problem_hash,
            "game launch failure",
            fix,
            "deterministic",
            1,
            "verified_pass",
            node_id,
            Some("game_doctor_retry"),
        )
    }

    /// Compute a simple hash of the KB state for drift detection (GAP-10 mesh heartbeat).
    /// Uses count + max updated_at as a cheap fingerprint.
    pub fn kb_hash(&self) -> anyhow::Result<String> {
        let count = self.solution_count().unwrap_or(0);
        let max_updated: String = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(updated_at), '') FROM solutions",
                [],
                |r| r.get(0),
            )
            .unwrap_or_default();
        Ok(format!("{}:{}", count, max_updated))
    }

    // ─── MMA-First Protocol: Q1-Q4 Helper Methods ─────────────────────────────

    /// Q1: Two-tier lookup — try exact hash first, then stable hash.
    /// Returns the best matching solution, preferring permanent fixes over workarounds.
    /// If a workaround has a permanent_fix_id, follows the link and returns the permanent fix.
    pub fn lookup_two_tier(&self, exact_hash: &str, stable_hash: &str) -> anyhow::Result<Option<Solution>> {
        // First: try exact hash (version-specific)
        if let Some(sol) = self.lookup(exact_hash)? {
            // If this workaround has been replaced by a permanent fix, return that instead
            if let Some(ref perm_id) = sol.permanent_fix_id {
                if let Ok(Some(perm)) = self.lookup_by_id(perm_id) {
                    return Ok(Some(perm));
                }
            }
            return Ok(Some(sol));
        }
        // Second: try stable hash (cross-version)
        if let Some(sol) = self.lookup(stable_hash)? {
            if let Some(ref perm_id) = sol.permanent_fix_id {
                if let Ok(Some(perm)) = self.lookup_by_id(perm_id) {
                    return Ok(Some(perm));
                }
            }
            return Ok(Some(sol));
        }
        Ok(None)
    }

    /// Lookup a solution by its ID (not hash). Used to follow permanent_fix_id links.
    fn lookup_by_id(&self, id: &str) -> anyhow::Result<Option<Solution>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, problem_key, problem_hash, symptoms, environment,
                    root_cause, fix_action, fix_type, success_count, fail_count,
                    confidence, cost_to_diagnose, models_used, source_node,
                    created_at, updated_at, version, ttl_days, tags, diagnosis_method,
                    fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
             FROM solutions WHERE id = ?1 LIMIT 1",
        )?;
        let result = stmt.query_row(params![id], |row| {
            Ok(Solution {
                id: row.get(0)?,
                problem_key: row.get(1)?,
                problem_hash: row.get(2)?,
                symptoms: row.get(3)?,
                environment: row.get(4)?,
                root_cause: row.get(5)?,
                fix_action: row.get(6)?,
                fix_type: row.get(7)?,
                success_count: row.get(8)?,
                fail_count: row.get(9)?,
                confidence: row.get(10)?,
                cost_to_diagnose: row.get(11)?,
                models_used: row.get(12)?,
                source_node: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                version: row.get(16)?,
                ttl_days: row.get(17)?,
                tags: row.get(18)?,
                diagnosis_method: row.get(19)?,
                fix_permanence: row.get::<_, Option<String>>(20)?.unwrap_or_else(|| "workaround".to_string()),
                recurrence_count: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
                permanent_fix_id: row.get(22)?,
                last_recurrence: row.get(23)?,
                permanent_attempt_at: row.get(24)?,
            })
        });
        match result {
            Ok(sol) => Ok(Some(sol)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("KB lookup_by_id error: {}", e)),
        }
    }

    /// Q1: Increment recurrence_count and update last_recurrence timestamp.
    /// Called every time Q1 applies a workaround from the KB.
    pub fn increment_recurrence(&self, solution_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE solutions SET recurrence_count = recurrence_count + 1,
             last_recurrence = datetime('now'),
             updated_at = datetime('now')
             WHERE id = ?1",
            params![solution_id],
        )?;
        tracing::debug!(target: LOG_TARGET, id = solution_id, "Recurrence count incremented");
        Ok(())
    }

    // ─── Plan 273-03: Universal Solution Recording ─────────────────────────────

    /// Record a resolution in the KB — simpler API for the tier engine main loop.
    ///
    /// If a solution with the same `problem_hash` already exists:
    ///   - increments `success_count` or `fail_count` based on `verification_result`
    ///   - recalculates `confidence` = success / (success + fail)
    ///   - updates `updated_at`
    /// If no solution exists:
    ///   - inserts a new row with initial confidence based on verification
    ///
    /// Called after EVERY TierResult::Fixed and TierResult::FailedToFix.
    pub fn record_resolution(
        &self,
        problem_key: &str,
        problem_hash: &str,
        symptoms: &str,
        fix_action: &str,
        fix_type: &str,       // "deterministic", "kb_cached", "model_suggested"
        tier: u8,
        verification_result: &str,  // "verified_pass", "verified_fail", "not_verified"
        node_id: &str,
        diagnosis_method: Option<&str>,
    ) -> anyhow::Result<()> {
        // Skip periodic — routine health checks don't produce actionable solutions
        if problem_key == "periodic" {
            tracing::debug!(target: LOG_TARGET, "Skipping record_resolution for periodic problem_key");
            return Ok(());
        }

        // Check if solution with this hash already exists
        let existing_id: Option<String> = self.conn
            .query_row(
                "SELECT id FROM solutions WHERE problem_hash = ?1 LIMIT 1",
                params![problem_hash],
                |row| row.get(0),
            )
            .ok();

        if let Some(ref id) = existing_id {
            // Update existing: bump counts and recalculate confidence
            let is_success = verification_result == "verified_pass";
            self.record_outcome(id, is_success)?;
            tracing::info!(
                target: LOG_TARGET,
                tier = tier,
                action = fix_action,
                hash = problem_hash,
                verification = verification_result,
                "KB recorded (update): tier={} action={} hash={} verification={}",
                tier, fix_action, problem_hash, verification_result
            );
        } else {
            // Insert new solution
            let confidence = if verification_result == "verified_pass" { 1.0 } else { 0.5 };
            let now = chrono::Utc::now().to_rfc3339();
            let solution = Solution {
                id: uuid::Uuid::new_v4().to_string(),
                problem_key: problem_key.to_string(),
                problem_hash: problem_hash.to_string(),
                symptoms: symptoms.to_string(),
                environment: "{}".to_string(),
                root_cause: fix_action.to_string(),
                fix_action: fix_action.to_string(),
                fix_type: fix_type.to_string(),
                success_count: if verification_result == "verified_pass" { 1 } else { 0 },
                fail_count: if verification_result == "verified_fail" { 1 } else { 0 },
                confidence,
                cost_to_diagnose: 0.0,
                models_used: None,
                source_node: node_id.to_string(),
                created_at: now.clone(),
                updated_at: now,
                version: 1,
                ttl_days: 90,
                tags: Some(format!("[\"{}\",\"tier_{}\"]", problem_key, tier)),
                diagnosis_method: diagnosis_method.map(|s| s.to_string()),
                fix_permanence: if tier <= 2 { "workaround".to_string() } else { "permanent".to_string() },
                recurrence_count: 0,
                permanent_fix_id: None,
                last_recurrence: None,
                permanent_attempt_at: None,
            };
            self.store_solution(&solution)?;
            tracing::info!(
                target: LOG_TARGET,
                tier = tier,
                action = fix_action,
                hash = problem_hash,
                verification = verification_result,
                "KB recorded (new): tier={} action={} hash={} verification={}",
                tier, fix_action, problem_hash, verification_result
            );
        }
        Ok(())
    }

    /// Lookup a solution by problem_hash (alias for `lookup()` for Plan 273-03 API clarity).
    /// Returns the highest-confidence solution filtered to confidence >= HIGH_CONFIDENCE_THRESHOLD.
    pub fn lookup_by_hash(&self, problem_hash: &str) -> anyhow::Result<Option<Solution>> {
        self.lookup(problem_hash)
    }

    /// Q4: Check whether this solution should trigger a permanent fix search.
    /// Returns true when: fix_permanence is "workaround" AND recurrence_count >= 3
    /// AND no permanent fix attempt in the last 7 days.
    pub fn should_trigger_q4(&self, solution: &Solution) -> bool {
        if solution.fix_permanence != "workaround" {
            return false;
        }
        if solution.recurrence_count < 3 {
            return false;
        }
        // Check cooldown: no attempt in last 7 days
        if let Some(ref attempt_at) = solution.permanent_attempt_at {
            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(attempt_at) {
                let days_since = chrono::Utc::now()
                    .signed_duration_since(dt.with_timezone(&chrono::Utc))
                    .num_days();
                if days_since < 7 {
                    return false;
                }
            }
        }
        true
    }

    /// Q4: Mark that a permanent fix attempt has been made for this solution.
    pub fn mark_permanent_attempt(&self, solution_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE solutions SET permanent_attempt_at = datetime('now'),
             updated_at = datetime('now')
             WHERE id = ?1",
            params![solution_id],
        )?;
        tracing::info!(target: LOG_TARGET, id = solution_id, "Q4 permanent fix attempt marked");
        Ok(())
    }

    /// Q4: Link a workaround to its permanent replacement and demote the workaround.
    pub fn link_permanent_fix(&self, workaround_id: &str, permanent_id: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE solutions SET permanent_fix_id = ?2,
             fix_permanence = 'fallback',
             updated_at = datetime('now')
             WHERE id = ?1",
            params![workaround_id, permanent_id],
        )?;
        tracing::info!(
            target: LOG_TARGET,
            workaround = workaround_id,
            permanent = permanent_id,
            "Workaround linked to permanent fix — demoted to fallback"
        );
        Ok(())
    }

    // ─── CGP + Plan Manager Support (v32.0) ──────────────────────────────────

    /// Look up multiple solutions by problem_key (for G5 competing hypotheses).
    /// Returns up to `limit` solutions ordered by confidence desc.
    pub fn lookup_all(&self, problem_key: &str, limit: usize) -> anyhow::Result<Vec<Solution>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, problem_key, problem_hash, symptoms, environment, root_cause,
                    fix_action, fix_type, success_count, fail_count, confidence,
                    cost_to_diagnose, models_used, source_node, created_at, updated_at,
                    version, ttl_days, tags, diagnosis_method, fix_permanence,
                    recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
             FROM solutions WHERE problem_key = ?1
             ORDER BY confidence DESC LIMIT ?2"
        )?;

        let rows = stmt.query_map(rusqlite::params![problem_key, limit as i64], |row| {
            Ok(Solution {
                id: row.get(0)?,
                problem_key: row.get(1)?,
                problem_hash: row.get(2)?,
                symptoms: row.get(3)?,
                environment: row.get(4)?,
                root_cause: row.get(5)?,
                fix_action: row.get(6)?,
                fix_type: row.get(7)?,
                success_count: row.get(8)?,
                fail_count: row.get(9)?,
                confidence: row.get(10)?,
                cost_to_diagnose: row.get(11)?,
                models_used: row.get(12)?,
                source_node: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                version: row.get(16)?,
                ttl_days: row.get(17)?,
                tags: row.get(18)?,
                diagnosis_method: row.get(19)?,
                fix_permanence: row.get::<_, Option<String>>(20)?.unwrap_or_else(|| "workaround".to_string()),
                recurrence_count: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
                permanent_fix_id: row.get(22)?,
                last_recurrence: row.get(23)?,
                permanent_attempt_at: row.get(24)?,
            })
        })?;

        let mut solutions = Vec::new();
        for row in rows {
            if let Ok(sol) = row {
                solutions.push(sol);
            }
        }
        Ok(solutions)
    }

    /// MMA-F2: Typed method for saving diagnosis plans (no raw SQL exposure).
    pub fn save_diagnosis_plan(
        &self,
        plan_id: &str,
        incident_id: &str,
        problem_key: &str,
        steps_json: &str,
        created_at: &str,
        completed_at: Option<&str>,
        tier: &str,
    ) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO diagnosis_plans (plan_id, incident_id, problem_key, steps_json, created_at, completed_at, tier) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![plan_id, incident_id, problem_key, steps_json, created_at, completed_at, tier],
        )?;
        Ok(())
    }

    /// MMA-F2: Typed method for saving diagnosis audits (no raw SQL exposure).
    pub fn save_diagnosis_audit(&self, incident_id: &str, audit_json: &str, timestamp: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO diagnosis_audits (incident_id, audit_json, timestamp) VALUES (?1, ?2, ?3)",
            params![incident_id, audit_json, timestamp],
        )?;
        Ok(())
    }
}

/// Normalize a DiagnosticTrigger into a stable, canonical problem key.
///
/// The key is stripped of variable data (timestamps, PIDs, ephemeral counts)
/// so that the same problem class always maps to the same key regardless of
/// when or how severely it occurred.
/// Sanitize a dynamic value for use in KB problem keys.
/// Replaces colons (IPv6), slashes (URLs), and control characters with underscores.
/// Truncates to 64 chars to prevent key bloat from long endpoints.
fn sanitize_kb_key_component(value: &str) -> String {
    value.chars()
        .take(64)
        .map(|c| match c {
            ':' | '/' | '\\' | '?' | '#' | '&' | '=' | ' ' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

pub fn normalize_problem_key(trigger: &DiagnosticTrigger) -> String {
    match trigger {
        DiagnosticTrigger::Periodic => "periodic".to_string(),
        DiagnosticTrigger::HealthCheckFail => "health_check_fail".to_string(),
        DiagnosticTrigger::GameLaunchFail => "game_launch_fail".to_string(),
        DiagnosticTrigger::ErrorSpike { .. } => "error_spike".to_string(),
        DiagnosticTrigger::WsDisconnect { .. } => "ws_disconnect".to_string(),
        DiagnosticTrigger::ViolationSpike { .. } => "violation_spike".to_string(),
        DiagnosticTrigger::DisplayMismatch { .. } => "display_mismatch".to_string(),
        DiagnosticTrigger::ProcessCrash { process_name } => {
            format!("process_crash:{}", process_name.to_lowercase())
        }
        DiagnosticTrigger::SentinelUnexpected { file_name } => {
            format!("sentinel_unexpected:{}", file_name.to_uppercase())
        }
        DiagnosticTrigger::PreFlightFailed { check_name, .. } => {
            format!("preflight_failed:{}", check_name.to_lowercase())
        }
        DiagnosticTrigger::PosKioskDown { .. } => "pos_kiosk_down".to_string(),
        DiagnosticTrigger::PosNetworkDown { server_ip, .. } => {
            // MMA P2 fix: sanitize server_ip — IPv6 colons break key delimiter parsing
            format!("pos_network_down:{}", sanitize_kb_key_component(server_ip))
        }
        DiagnosticTrigger::PosBillingApiError { endpoint, .. } => {
            // MMA P2 fix: sanitize endpoint — may contain colons, slashes, query params
            format!("pos_billing_api_error:{}", sanitize_kb_key_component(endpoint))
        }
        DiagnosticTrigger::PosWifiDegraded { rssi_dbm, .. } => format!("pos_wifi_degraded:{rssi_dbm}"),
        DiagnosticTrigger::PosKioskEscaped { foreground_process } => {
            format!("pos_kiosk_escaped:{}", sanitize_kb_key_component(foreground_process))
        }
        DiagnosticTrigger::TaskbarVisible => "taskbar_visible".to_string(),
        // MMA-First Protocol triggers (v31.0)
        DiagnosticTrigger::GameMidSessionCrash { .. } => "game_mid_session_crash".to_string(),
        DiagnosticTrigger::PostSessionAnalysis { .. } => "post_session_analysis".to_string(),
        DiagnosticTrigger::PreShiftAudit => "pre_shift_audit".to_string(),
        DiagnosticTrigger::DeployVerification { new_build_id } => {
            format!("deploy_verification:{}", new_build_id)
        }
        // Phase 318 (LAUNCH-01)
        DiagnosticTrigger::GameLaunchTimeout { .. } => "game_launch_timeout".to_string(),
        DiagnosticTrigger::WsInstability { reconnects_5m, .. } => format!("ws_instability:{reconnects_5m}"),
    }
}

/// Capture the current node's environment fingerprint.
pub fn fingerprint_env(build_id: &str) -> EnvironmentFingerprint {
    let os_version = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Windows_NT".to_string());

    EnvironmentFingerprint {
        os_version,
        build_id: build_id.to_string(),
        hardware_class: "pod".to_string(),
    }
}

/// Compute a problem_hash from the problem_key + environment fingerprint.
/// Used as the lookup key in the solutions table.
/// This is the "exact" hash — includes build_id, so it changes on every deploy.
pub fn compute_problem_hash(problem_key: &str, env: &EnvironmentFingerprint) -> String {
    compute_exact_hash(problem_key, env)
}

/// Exact hash: problem_key + build_id + hardware_class.
/// Changes on every deploy — use for version-specific issues.
pub fn compute_exact_hash(problem_key: &str, env: &EnvironmentFingerprint) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}|{}|{}", problem_key, env.build_id, env.hardware_class);
    let hash = Sha256::digest(input.as_bytes());
    format!("{:x}", hash)[..16].to_string()
}

/// Stable hash: problem_key + hardware_class only (no build_id).
/// Survives across deploys — use for issues that are version-independent
/// (corrupted track data, GPU thermal, orphan processes, etc).
pub fn compute_stable_hash(problem_key: &str, env: &EnvironmentFingerprint) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}|{}", problem_key, env.hardware_class);
    let hash = Sha256::digest(input.as_bytes());
    format!("s_{:x}", hash)[..16].to_string()
}

// ─── Phase 278: KB Hardening Promotion Methods ─────────────────────────────

impl KnowledgeBase {
    /// Get all solutions with a given promotion_status.
    pub fn get_promotion_candidates(&self, status: &str) -> anyhow::Result<Vec<Solution>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, problem_key, problem_hash, symptoms, environment,
                    root_cause, fix_action, fix_type, success_count, fail_count,
                    confidence, cost_to_diagnose, models_used, source_node,
                    created_at, updated_at, version, ttl_days, tags, diagnosis_method,
                    fix_permanence, recurrence_count, permanent_fix_id, last_recurrence, permanent_attempt_at
             FROM solutions
             WHERE promotion_status = ?1"
        )?;

        let rows = stmt.query_map(params![status], |row| {
            Ok(Solution {
                id: row.get(0)?,
                problem_key: row.get(1)?,
                problem_hash: row.get(2)?,
                symptoms: row.get(3)?,
                environment: row.get(4)?,
                root_cause: row.get(5)?,
                fix_action: row.get(6)?,
                fix_type: row.get(7)?,
                success_count: row.get(8)?,
                fail_count: row.get(9)?,
                confidence: row.get(10)?,
                cost_to_diagnose: row.get(11)?,
                models_used: row.get(12)?,
                source_node: row.get(13)?,
                created_at: row.get(14)?,
                updated_at: row.get(15)?,
                version: row.get(16)?,
                ttl_days: row.get(17)?,
                tags: row.get(18)?,
                diagnosis_method: row.get(19)?,
                fix_permanence: row.get::<_, Option<String>>(20)?.unwrap_or_else(|| "workaround".to_string()),
                recurrence_count: row.get::<_, Option<i64>>(21)?.unwrap_or(0),
                permanent_fix_id: row.get(22)?,
                last_recurrence: row.get(23)?,
                permanent_attempt_at: row.get(24)?,
            })
        })?;

        let mut solutions = Vec::new();
        for row in rows {
            match row {
                Ok(sol) => solutions.push(sol),
                Err(e) => tracing::warn!(target: LOG_TARGET, error = %e, "Failed to read promotion candidate"),
            }
        }
        Ok(solutions)
    }

    /// Promote a solution to a new status in the ladder.
    pub fn promote_solution(&self, problem_hash: &str, new_status: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE solutions SET promotion_status = ?1, promoted_at = datetime('now'), updated_at = datetime('now')
             WHERE problem_hash = ?2",
            params![new_status, problem_hash],
        )?;
        tracing::info!(
            target: LOG_TARGET,
            problem_hash = problem_hash,
            new_status = new_status,
            "Solution promotion_status updated"
        );
        Ok(())
    }

    /// Get the current promotion_status for a solution.
    pub fn get_promotion_status(&self, problem_hash: &str) -> Option<String> {
        self.conn
            .query_row(
                "SELECT COALESCE(promotion_status, 'observed') FROM solutions WHERE problem_hash = ?1 LIMIT 1",
                params![problem_hash],
                |r| r.get(0),
            )
            .ok()
    }

    /// Count distinct source nodes that have successfully applied this solution.
    pub fn count_distinct_nodes(&self, problem_hash: &str) -> anyhow::Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT node_id) FROM solution_nodes WHERE problem_hash = ?1 AND success_count > 0",
            params![problem_hash],
            |r| r.get(0),
        )?;
        Ok(count as usize)
    }

    /// Record a node's success/failure for a solution (for quorum tracking).
    pub fn record_node_outcome(&self, problem_hash: &str, node_id: &str, success: bool) -> anyhow::Result<()> {
        if success {
            self.conn.execute(
                "INSERT INTO solution_nodes (problem_hash, node_id, success_count, last_seen)
                 VALUES (?1, ?2, 1, datetime('now'))
                 ON CONFLICT(problem_hash, node_id) DO UPDATE SET
                    success_count = success_count + 1,
                    last_seen = datetime('now')",
                params![problem_hash, node_id],
            )?;
        } else {
            self.conn.execute(
                "INSERT INTO solution_nodes (problem_hash, node_id, success_count, last_seen)
                 VALUES (?1, ?2, 0, datetime('now'))
                 ON CONFLICT(problem_hash, node_id) DO UPDATE SET
                    last_seen = datetime('now')",
                params![problem_hash, node_id],
            )?;
        }
        Ok(())
    }

    /// Check if a canary pod (pod_8) has successfully applied this solution.
    pub fn has_canary_pod_success(&self, problem_hash: &str) -> anyhow::Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM solution_nodes
             WHERE problem_hash = ?1
               AND (node_id LIKE '%pod_8%' OR node_id LIKE '%pod-8%')
               AND success_count > 0",
            params![problem_hash],
            |r| r.get(0),
        )?;
        Ok(count > 0)
    }

    /// Calculate days since the solution was last promoted (for shadow duration check).
    pub fn days_since_promotion(&self, problem_hash: &str) -> Option<i64> {
        self.conn.query_row(
            "SELECT CAST(julianday('now') - julianday(COALESCE(promoted_at, created_at)) AS INTEGER)
             FROM solutions WHERE problem_hash = ?1 LIMIT 1",
            params![problem_hash],
            |r| r.get(0),
        ).ok()
    }

    /// Store a fully promoted hardened rule.
    pub fn store_hardened_rule(&self, rule: &HardenedRule) -> anyhow::Result<()> {
        let matchers_json = serde_json::to_string(&rule.matchers).unwrap_or_else(|_| "[]".to_string());
        self.conn.execute(
            "INSERT OR REPLACE INTO hardened_rules (
                problem_key, matchers, action, verifier, ttl_secs, confidence, provenance, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            params![
                rule.problem_key,
                matchers_json,
                rule.action,
                rule.verifier,
                rule.ttl_secs,
                rule.confidence,
                rule.provenance,
            ],
        )?;
        tracing::info!(
            target: LOG_TARGET,
            problem_key = %rule.problem_key,
            confidence = rule.confidence,
            "Hardened rule stored"
        );
        Ok(())
    }

    /// Retrieve all hardened rules for Tier 1 deterministic checks.
    pub fn get_hardened_rules(&self) -> anyhow::Result<Vec<HardenedRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT problem_key, matchers, action, verifier, ttl_secs, confidence, provenance
             FROM hardened_rules"
        )?;

        let rows = stmt.query_map([], |row| {
            let matchers_str: String = row.get(1)?;
            let matchers: Vec<String> = serde_json::from_str(&matchers_str).unwrap_or_default();
            Ok(HardenedRule {
                problem_key: row.get(0)?,
                matchers,
                action: row.get(2)?,
                verifier: row.get(3)?,
                ttl_secs: row.get(4)?,
                confidence: row.get(5)?,
                provenance: row.get(6)?,
            })
        })?;

        let mut rules = Vec::new();
        for row in rows {
            match row {
                Ok(rule) => rules.push(rule),
                Err(e) => tracing::warn!(target: LOG_TARGET, error = %e, "Failed to read hardened rule"),
            }
        }
        Ok(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_kb() -> KnowledgeBase {
        let conn = rusqlite::Connection::open_in_memory().expect("in-memory DB");
        let kb = KnowledgeBase { conn };
        kb.run_migrations().expect("migrations");
        kb
    }

    fn make_solution(id: &str, problem_hash: &str, confidence: f64) -> Solution {
        Solution {
            id: id.to_string(),
            problem_key: "test_key".to_string(),
            problem_hash: problem_hash.to_string(),
            symptoms: "{}".to_string(),
            environment: "{}".to_string(),
            root_cause: "test root cause".to_string(),
            fix_action: r#"{"action":"restart"}"#.to_string(),
            fix_type: "restart".to_string(),
            success_count: 3,
            fail_count: 0,
            confidence,
            cost_to_diagnose: 0.0,
            models_used: None,
            source_node: "pod_1".to_string(),
            created_at: "2026-03-27T00:00:00+05:30".to_string(),
            updated_at: "2026-03-27T00:00:00+05:30".to_string(),
            version: 1,
            ttl_days: 90,
            tags: None,
            diagnosis_method: None,
            fix_permanence: "workaround".to_string(),
            recurrence_count: 0,
            permanent_fix_id: None,
            last_recurrence: None,
            permanent_attempt_at: None,
        }
    }

    #[test]
    fn test_open_creates_tables() {
        let kb = open_test_kb();
        let count: i64 = kb
            .conn()
            .query_row("SELECT COUNT(*) FROM solutions", [], |r| r.get(0))
            .expect("solutions table exists");
        assert_eq!(count, 0);
    }

    #[test]
    fn test_store_and_lookup_hit() {
        let kb = open_test_kb();
        let sol = make_solution("id1", "hash_abc", 0.9);
        kb.store_solution(&sol).expect("store");
        let result = kb.lookup("hash_abc").expect("lookup");
        assert!(result.is_some());
        assert_eq!(result.as_ref().expect("some").root_cause, "test root cause");
    }

    #[test]
    fn test_lookup_miss_no_row() {
        let kb = open_test_kb();
        let result = kb.lookup("nonexistent_hash").expect("lookup");
        assert!(result.is_none(), "expected None for unknown hash");
    }

    #[test]
    fn test_lookup_miss_low_confidence() {
        let kb = open_test_kb();
        let sol = make_solution("id2", "hash_lowconf", 0.5);
        kb.store_solution(&sol).expect("store");
        let result = kb.lookup("hash_lowconf").expect("lookup");
        assert!(result.is_none(), "expected None when confidence < 0.8");
    }

    #[test]
    fn test_normalize_problem_key_stable() {
        let t1 = DiagnosticTrigger::WsDisconnect {
            disconnected_secs: 30,
        };
        let t2 = DiagnosticTrigger::WsDisconnect {
            disconnected_secs: 9999,
        };
        assert_eq!(
            normalize_problem_key(&t1),
            normalize_problem_key(&t2),
            "WsDisconnect key must not include variable secs"
        );

        let t3 = DiagnosticTrigger::ProcessCrash {
            process_name: "acs.exe".to_string(),
        };
        let t4 = DiagnosticTrigger::ProcessCrash {
            process_name: "rc-sentry.exe".to_string(),
        };
        assert_ne!(
            normalize_problem_key(&t3),
            normalize_problem_key(&t4),
            "Different process names must produce different keys"
        );

        let t5 = DiagnosticTrigger::ProcessCrash {
            process_name: "ACS.EXE".to_string(),
        };
        assert_eq!(
            normalize_problem_key(&t3),
            normalize_problem_key(&t5),
            "ProcessCrash key must be case-insensitive"
        );
    }

    #[test]
    fn test_normalize_all_trigger_variants() {
        let triggers = vec![
            DiagnosticTrigger::Periodic,
            DiagnosticTrigger::HealthCheckFail,
            DiagnosticTrigger::GameLaunchFail,
            DiagnosticTrigger::ErrorSpike { errors_per_min: 10 },
            DiagnosticTrigger::WsDisconnect {
                disconnected_secs: 60,
            },
            DiagnosticTrigger::ViolationSpike { delta: 55 },
            DiagnosticTrigger::DisplayMismatch {
                expected_edge_count: 1,
                actual_edge_count: 0,
            },
            DiagnosticTrigger::ProcessCrash {
                process_name: "test.exe".to_string(),
            },
            DiagnosticTrigger::SentinelUnexpected {
                file_name: "FORCE_CLEAN".to_string(),
            },
        ];
        for trigger in &triggers {
            let key = normalize_problem_key(trigger);
            assert!(!key.is_empty(), "Key must not be empty for {:?}", trigger);
            assert!(
                !key.contains(' '),
                "Key must not contain spaces for {:?}",
                trigger
            );
        }
    }

    #[test]
    fn test_record_experiment_and_get_open() {
        let kb = open_test_kb();
        let exp = Experiment {
            id: "exp1".to_string(),
            problem_key: "process_crash:acs.exe".to_string(),
            hypothesis: "Stale sentinel blocking launch".to_string(),
            test_plan: "Check for FORCE_CLEAN".to_string(),
            result: None,
            cost: 0.0,
            node: "pod_3".to_string(),
            created_at: "2026-03-27T00:00:00+05:30".to_string(),
        };
        kb.record_experiment(&exp).expect("record");
        let found = kb
            .get_open_experiment("process_crash:acs.exe")
            .expect("get");
        assert!(found.is_some());
        assert_eq!(found.as_ref().expect("some").id, "exp1");
    }

    #[test]
    fn test_record_experiment_idempotent() {
        let kb = open_test_kb();
        let exp = Experiment {
            id: "exp2".to_string(),
            problem_key: "health_check_fail".to_string(),
            hypothesis: "Port 8090 blocked".to_string(),
            test_plan: "netstat check".to_string(),
            result: None,
            cost: 0.0,
            node: "pod_5".to_string(),
            created_at: "2026-03-27T00:00:00+05:30".to_string(),
        };
        kb.record_experiment(&exp).expect("first record");
        kb.record_experiment(&exp)
            .expect("second record must not error");
        let count: i64 = kb
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM experiments WHERE id='exp2'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(count, 1, "INSERT OR IGNORE must prevent duplicates");
    }

    #[test]
    fn test_compute_problem_hash() {
        let env = EnvironmentFingerprint {
            os_version: "Windows 11".to_string(),
            build_id: "abc123".to_string(),
            hardware_class: "pod".to_string(),
        };
        let h1 = compute_problem_hash("ws_disconnect", &env);
        let h2 = compute_problem_hash("ws_disconnect", &env);
        assert_eq!(h1, h2, "Same inputs must produce same hash");
        assert_eq!(h1.len(), 16, "Hash should be 16 hex chars");

        let env2 = EnvironmentFingerprint {
            os_version: "Windows 11".to_string(),
            build_id: "def456".to_string(),
            hardware_class: "pod".to_string(),
        };
        let h3 = compute_problem_hash("ws_disconnect", &env2);
        assert_ne!(h1, h3, "Different build_id must produce different hash");
    }

    #[test]
    fn test_solution_count() {
        let kb = open_test_kb();
        assert_eq!(kb.solution_count().expect("count"), 0);
        kb.store_solution(&make_solution("s1", "h1", 0.9))
            .expect("store");
        assert_eq!(kb.solution_count().expect("count"), 1);
    }
}
