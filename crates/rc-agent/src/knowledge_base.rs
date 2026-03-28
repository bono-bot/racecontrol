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
}

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
            ).ok(); // Ignore error if column already exists
        }

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
                created_at, updated_at, version, ttl_days, tags, diagnosis_method
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14,
                ?15, ?16, ?17, ?18, ?19, ?20
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
                    created_at, updated_at, version, ttl_days, tags, diagnosis_method
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
        DiagnosticTrigger::TaskbarVisible => "taskbar_visible".to_string(),
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
pub fn compute_problem_hash(problem_key: &str, env: &EnvironmentFingerprint) -> String {
    use sha2::{Digest, Sha256};
    let input = format!("{}|{}|{}", problem_key, env.build_id, env.hardware_class);
    let hash = Sha256::digest(input.as_bytes());
    format!("{:x}", hash)[..16].to_string()
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
