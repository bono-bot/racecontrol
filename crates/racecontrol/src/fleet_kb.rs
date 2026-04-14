//! Fleet Knowledge Base — Central storage for mesh intelligence solutions and incidents.
//!
//! SQLite tables: fleet_solutions, fleet_experiments, fleet_incidents.
//! CRUD functions used by mesh_handler (gossip), promotion pipeline, and API endpoints.

use sqlx::SqlitePool;

#[path = "fleet_kb_crud.rs"]
mod crud;

#[path = "fleet_kb_models.rs"]
mod models;

// Re-export all public items so callers continue using `fleet_kb::function_name`.
pub use crud::*;
pub use models::*;

// ─── Migration ──────────────────────────────────────────────────────────────

/// Create mesh intelligence tables. Called from db::migrate().
pub async fn migrate(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS fleet_solutions (
            id TEXT PRIMARY KEY,
            problem_key TEXT NOT NULL,
            problem_hash TEXT NOT NULL,
            symptoms TEXT NOT NULL,
            environment TEXT NOT NULL,
            root_cause TEXT NOT NULL,
            fix_action TEXT NOT NULL,
            fix_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'candidate',
            success_count INTEGER DEFAULT 1,
            fail_count INTEGER DEFAULT 0,
            confidence REAL DEFAULT 1.0,
            cost_to_diagnose REAL DEFAULT 0,
            models_used TEXT,
            diagnosis_tier TEXT NOT NULL DEFAULT 'deterministic',
            source_node TEXT NOT NULL,
            venue_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            version INTEGER DEFAULT 1,
            ttl_days INTEGER DEFAULT 90,
            tags TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_fleet_solutions_hash ON fleet_solutions(problem_hash)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_fleet_solutions_key ON fleet_solutions(problem_key)")
        .execute(pool)
        .await?;
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_fleet_solutions_status ON fleet_solutions(status)")
        .execute(pool)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS fleet_experiments (
            id TEXT PRIMARY KEY,
            problem_key TEXT NOT NULL,
            hypothesis TEXT NOT NULL,
            test_plan TEXT NOT NULL,
            result TEXT,
            cost REAL DEFAULT 0,
            node TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_fleet_experiments_key ON fleet_experiments(problem_key)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS fleet_incidents (
            id TEXT PRIMARY KEY,
            node TEXT NOT NULL,
            problem_key TEXT NOT NULL,
            severity TEXT NOT NULL DEFAULT 'medium',
            cost REAL DEFAULT 0,
            resolution TEXT,
            time_to_resolve_secs INTEGER,
            resolved_by_tier TEXT,
            detected_at TEXT NOT NULL,
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_fleet_incidents_node ON fleet_incidents(node)",
    )
    .execute(pool)
    .await?;

    // CGP + Plan Manager audit trail (v32.0)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS diagnosis_audits (
            incident_id TEXT PRIMARY KEY,
            audit_json TEXT NOT NULL,
            timestamp TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_diagnosis_audits_timestamp ON diagnosis_audits(timestamp)",
    )
    .execute(pool)
    .await?;

    // Audit Known Issues — code bugs found by ecosystem audits that MI should
    // recognize as code-level problems (not runtime fixable). When symptoms match,
    // MI short-circuits diagnosis and escalates instead of burning AI credits.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS audit_known_issues (
            id TEXT PRIMARY KEY,
            problem_key TEXT NOT NULL UNIQUE,
            severity TEXT NOT NULL,
            symptom_patterns TEXT NOT NULL,
            root_cause TEXT NOT NULL,
            fix_description TEXT NOT NULL,
            fix_status TEXT NOT NULL DEFAULT 'pending',
            fixed_in_build TEXT,
            affects_targets TEXT NOT NULL,
            audit_date TEXT NOT NULL,
            escalation_message TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_audit_known_issues_status ON audit_known_issues(fix_status)")
        .execute(pool)
        .await?;

    tracing::info!("Mesh intelligence tables initialized (with audit_known_issues)");
    Ok(())
}

// ─── Audit Known Issues ────────────────────────────────────────────────────

/// Check if a symptom string matches any known audit issue.
/// Returns the escalation message if matched, so the caller can skip Tier 1-4 diagnosis.
pub async fn check_audit_known_issues(pool: &SqlitePool, symptom: &str) -> Option<String> {
    let lower = symptom.to_lowercase();
    let rows = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT problem_key, symptom_patterns, escalation_message, fix_status FROM audit_known_issues",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (problem_key, patterns_json, escalation, fix_status) in &rows {
        // patterns_json is a JSON array of keyword strings to match
        if let Ok(patterns) = serde_json::from_str::<Vec<String>>(patterns_json) {
            let matched = patterns.iter().any(|p| lower.contains(&p.to_lowercase()));
            if matched {
                let status_note = if fix_status == "code_fixed" {
                    " [FIX AVAILABLE — verify build version]"
                } else {
                    " [NOT YET FIXED — escalate to developer]"
                };
                return Some(format!(
                    "[AUDIT KNOWN ISSUE: {}]{}\n{}",
                    problem_key, status_note, escalation
                ));
            }
        }
    }
    None
}

/// Insert or update an audit known issue.
pub async fn upsert_audit_known_issue(
    pool: &SqlitePool,
    id: &str,
    problem_key: &str,
    severity: &str,
    symptom_patterns: &[String],
    root_cause: &str,
    fix_description: &str,
    fix_status: &str,
    fixed_in_build: Option<&str>,
    affects_targets: &[String],
    audit_date: &str,
    escalation_message: &str,
) -> anyhow::Result<()> {
    let patterns_json = serde_json::to_string(symptom_patterns)?;
    let targets_json = serde_json::to_string(affects_targets)?;
    sqlx::query(
        "INSERT OR REPLACE INTO audit_known_issues
         (id, problem_key, severity, symptom_patterns, root_cause, fix_description,
          fix_status, fixed_in_build, affects_targets, audit_date, escalation_message)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
    )
    .bind(id)
    .bind(problem_key)
    .bind(severity)
    .bind(&patterns_json)
    .bind(root_cause)
    .bind(fix_description)
    .bind(fix_status)
    .bind(fixed_in_build)
    .bind(&targets_json)
    .bind(audit_date)
    .bind(escalation_message)
    .execute(pool)
    .await?;
    Ok(())
}
