//! MI Knowledge Base for rc-sentry -- SQLite-backed solution storage.
//!
//! Mirrors the KnowledgeBase from rc-agent so that rc-sentry can perform
//! Tier 2 KB lookups independently (e.g., during rc-agent downtime).
//!
//! Key design:
//!   - rusqlite::Connection is !Send -- each caller opens its own connection
//!   - Same DB path as rc-agent: C:\RacingPoint\mesh_kb.db
//!   - Uses rc_common::diagnostic_types::SolutionRecord as the solution type
//!   - Pure std -- no tokio, no async
//!
//! Phase 322, Plan 02 (MIG-03)

use rusqlite::{params, Connection};
use rc_common::diagnostic_types::{DiagnosticTrigger, SolutionRecord};
use serde::{Deserialize, Serialize};

const LOG_TARGET: &str = "mi-knowledge-base";
pub const KB_PATH: &str = r"C:\RacingPoint\mesh_kb.db";
pub const HIGH_CONFIDENCE_THRESHOLD: f64 = 0.8;

/// A lightweight error type wrapping rusqlite and generic errors.
type KbResult<T> = Result<T, Box<dyn std::error::Error>>;

/// A hardened rule entry from the hardened_rules table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardenedRule {
    pub id: String,
    pub matchers: Vec<String>,
    pub action: String,
    pub confidence: f64,
    pub provenance: String,
}

/// The local knowledge base -- wraps a rusqlite Connection.
pub struct KnowledgeBase {
    conn: Connection,
}

impl KnowledgeBase {
    /// Open (or create) the local KB at the given path.
    /// Runs migrations -- idempotent via CREATE TABLE IF NOT EXISTS.
    pub fn open(path: &str) -> KbResult<Self> {
        tracing::info!(target: LOG_TARGET, path = path, "Opening MI knowledge base");
        let conn = Connection::open(path)?;
        let kb = Self { conn };
        kb.run_migrations()?;
        tracing::info!(target: LOG_TARGET, "MI knowledge base ready");
        Ok(kb)
    }

    /// Run schema migrations. Idempotent -- safe to call on existing DB.
    fn run_migrations(&self) -> KbResult<()> {
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

            CREATE INDEX IF NOT EXISTS idx_solutions_hash ON solutions(problem_hash);
            CREATE INDEX IF NOT EXISTS idx_solutions_key ON solutions(problem_key);

            CREATE TABLE IF NOT EXISTS hardened_rules (
                id TEXT PRIMARY KEY,
                matchers TEXT NOT NULL,
                action TEXT NOT NULL,
                confidence REAL DEFAULT 1.0,
                provenance TEXT DEFAULT 'manual'
            );
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

        // MMA-First Protocol columns
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

        // Promotion status column
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

        tracing::debug!(target: LOG_TARGET, "Migrations complete");
        Ok(())
    }

    /// Look up a solution by problem_hash.
    /// Returns Some(solution) only if confidence >= HIGH_CONFIDENCE_THRESHOLD (0.8).
    pub fn lookup(&self, problem_hash: &str) -> KbResult<Option<SolutionRecord>> {
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
            Ok(SolutionRecord {
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
            Err(e) => Err(Box::new(e)),
        }
    }

    /// Store a solution in the local KB.
    /// Uses INSERT OR REPLACE -- if the same id exists, it is overwritten.
    pub fn store_solution(&self, solution: &SolutionRecord) -> KbResult<()> {
        if solution.problem_key == "periodic" {
            tracing::debug!(target: LOG_TARGET, "Skipping KB store for periodic problem_key");
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
            "Solution stored in MI KB"
        );
        Ok(())
    }

    /// Get all hardened rules from the hardened_rules table.
    pub fn get_hardened_rules(&self) -> KbResult<Vec<HardenedRule>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, matchers, action, confidence, provenance FROM hardened_rules"
        )?;

        let rules = stmt.query_map([], |row| {
            let matchers_json: String = row.get(1)?;
            let matchers: Vec<String> = serde_json::from_str(&matchers_json)
                .unwrap_or_default();
            Ok(HardenedRule {
                id: row.get(0)?,
                matchers,
                action: row.get(2)?,
                confidence: row.get(3)?,
                provenance: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for rule in rules {
            if let Ok(r) = rule {
                result.push(r);
            }
        }
        Ok(result)
    }

    /// Count total solutions in the KB.
    pub fn solution_count(&self) -> KbResult<i64> {
        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM solutions", [], |r| r.get(0))?;
        Ok(count)
    }
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

/// Normalize a DiagnosticTrigger into a stable problem_key string.
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
            // Sanitize IPv6 colons -- they break key delimiter parsing
            let sanitized = server_ip.replace(':', "_");
            format!("pos_network_down:{}", sanitized)
        }
        DiagnosticTrigger::WsInstability { .. } => "ws_instability".to_string(),
        DiagnosticTrigger::PosBillingApiError { endpoint, .. } => {
            format!("pos_billing_api_error:{}", endpoint.to_lowercase())
        }
        DiagnosticTrigger::PosWifiDegraded { .. } => "pos_wifi_degraded".to_string(),
        DiagnosticTrigger::PosKioskEscaped { .. } => "pos_kiosk_escaped".to_string(),
        DiagnosticTrigger::TaskbarVisible => "taskbar_visible".to_string(),
        DiagnosticTrigger::GameMidSessionCrash { .. } => "game_mid_session_crash".to_string(),
        DiagnosticTrigger::PostSessionAnalysis { .. } => "post_session_analysis".to_string(),
        DiagnosticTrigger::PreShiftAudit => "pre_shift_audit".to_string(),
        DiagnosticTrigger::DeployVerification { .. } => "deploy_verification".to_string(),
        DiagnosticTrigger::GameLaunchTimeout { .. } => "game_launch_timeout".to_string(),
    }
}

/// Compute a problem hash from problem_key and environment info.
/// Uses SHA-256 truncated to 16 hex chars (same as rc-agent).
pub fn compute_problem_hash(problem_key: &str, build_id: &str, hardware_class: &str) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}|{}|{}", problem_key, build_id, hardware_class);
    let hash = Sha256::digest(input.as_bytes());
    format!("{:x}", hash)[..16].to_string()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_solution(id: &str, key: &str, hash: &str, confidence: f64) -> SolutionRecord {
        SolutionRecord {
            id: id.to_string(),
            problem_key: key.to_string(),
            problem_hash: hash.to_string(),
            symptoms: "test symptom".to_string(),
            environment: "test".to_string(),
            root_cause: "test cause".to_string(),
            fix_action: "restart".to_string(),
            fix_type: "deterministic".to_string(),
            success_count: 1,
            fail_count: 0,
            confidence,
            cost_to_diagnose: 0.0,
            models_used: None,
            source_node: "test-node".to_string(),
            created_at: "2026-04-06T00:00:00Z".to_string(),
            updated_at: "2026-04-06T00:00:00Z".to_string(),
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
    fn test_open_creates_db() {
        let dir = std::env::temp_dir().join("rc_sentry_kb_test_open");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_open.db");
        let path_str = path.to_str().unwrap();

        let kb = KnowledgeBase::open(path_str).unwrap();
        // DB should exist and have the solutions table
        let count = kb.solution_count().unwrap();
        assert_eq!(count, 0);
        assert!(path.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_lookup_missing_returns_none() {
        let dir = std::env::temp_dir().join("rc_sentry_kb_test_miss");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_miss.db");
        let path_str = path.to_str().unwrap();

        let kb = KnowledgeBase::open(path_str).unwrap();
        let result = kb.lookup("nonexistent_hash").unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_store_and_retrieve_solution() {
        let dir = std::env::temp_dir().join("rc_sentry_kb_test_store");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_store.db");
        let path_str = path.to_str().unwrap();

        let kb = KnowledgeBase::open(path_str).unwrap();
        let solution = make_test_solution("sol-1", "health_check_fail", "hash123", 0.95);

        kb.store_solution(&solution).unwrap();
        let retrieved = kb.lookup("hash123").unwrap();
        assert!(retrieved.is_some());

        let r = retrieved.unwrap();
        assert_eq!(r.id, "sol-1");
        assert_eq!(r.problem_key, "health_check_fail");
        assert_eq!(r.confidence, 0.95);
        assert_eq!(r.fix_action, "restart");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_solution_count() {
        let dir = std::env::temp_dir().join("rc_sentry_kb_test_count");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_count.db");
        let path_str = path.to_str().unwrap();

        let kb = KnowledgeBase::open(path_str).unwrap();
        assert_eq!(kb.solution_count().unwrap(), 0);

        kb.store_solution(&make_test_solution("sol-a", "key-a", "hash-a", 0.9)).unwrap();
        assert_eq!(kb.solution_count().unwrap(), 1);

        kb.store_solution(&make_test_solution("sol-b", "key-b", "hash-b", 0.85)).unwrap();
        assert_eq!(kb.solution_count().unwrap(), 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_lookup_below_threshold_returns_none() {
        let dir = std::env::temp_dir().join("rc_sentry_kb_test_threshold");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_threshold.db");
        let path_str = path.to_str().unwrap();

        let kb = KnowledgeBase::open(path_str).unwrap();
        // Store with confidence below threshold
        let solution = make_test_solution("sol-low", "key-low", "hash-low", 0.5);
        kb.store_solution(&solution).unwrap();

        // Should not be returned (below 0.8 threshold)
        let result = kb.lookup("hash-low").unwrap();
        assert!(result.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
