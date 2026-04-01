//! Fleet Knowledge Base — Central storage for mesh intelligence solutions and incidents.
//!
//! SQLite tables: fleet_solutions, fleet_experiments, fleet_incidents.
//! CRUD functions used by mesh_handler (gossip), promotion pipeline, and API endpoints.

use chrono::Utc;
use rc_common::mesh_types::*;
use sqlx::SqlitePool;

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

    tracing::info!("Mesh intelligence tables initialized");
    Ok(())
}

// ─── Solution CRUD ──────────────────────────────────────────────────────────

/// Insert a new solution into the fleet KB (from gossip announcement).
/// MMA-C3/C18: Uses INSERT OR IGNORE to prevent overwriting verified/hardened solutions.
/// Existing solutions are updated via update_confidence() instead.
pub async fn insert_solution(pool: &SqlitePool, sol: &MeshSolution) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO fleet_solutions
         (id, problem_key, problem_hash, symptoms, environment, root_cause, fix_action,
          fix_type, status, success_count, fail_count, confidence, cost_to_diagnose,
          models_used, diagnosis_tier, source_node, venue_id, created_at, updated_at,
          version, ttl_days, tags)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
    )
    .bind(&sol.id)
    .bind(&sol.problem_key)
    .bind(&sol.problem_hash)
    .bind(sol.symptoms.to_string())
    .bind(sol.environment.to_string())
    .bind(&sol.root_cause)
    .bind(sol.fix_action.to_string())
    .bind(serde_json::to_string(&sol.fix_type)?)
    .bind(serde_json::to_string(&sol.status)?)
    .bind(sol.success_count)
    .bind(sol.fail_count)
    .bind(sol.confidence)
    .bind(sol.cost_to_diagnose)
    .bind(sol.models_used.as_ref().map(|m| serde_json::to_string(m).ok()).flatten())
    .bind(serde_json::to_string(&sol.diagnosis_tier)?)
    .bind(&sol.source_node)
    .bind(&sol.venue_id)
    .bind(sol.created_at.to_rfc3339())
    .bind(sol.updated_at.to_rfc3339())
    .bind(sol.version)
    .bind(sol.ttl_days)
    .bind(sol.tags.as_ref().map(|t| serde_json::to_string(t).ok()).flatten())
    .execute(pool)
    .await?;
    Ok(())
}

/// Get a solution by ID.
pub async fn get_solution(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<MeshSolution>> {
    let row = sqlx::query_as::<_, SolutionRow>(
        "SELECT * FROM fleet_solutions WHERE id = ?1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.into()))
}

/// Get a solution by problem_hash.
pub async fn get_solution_by_hash(pool: &SqlitePool, hash: &str) -> anyhow::Result<Option<MeshSolution>> {
    let row = sqlx::query_as::<_, SolutionRow>(
        "SELECT * FROM fleet_solutions WHERE problem_hash = ?1 ORDER BY confidence DESC LIMIT 1",
    )
    .bind(hash)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.into()))
}

/// List solutions with optional status filter and pagination.
pub async fn list_solutions(
    pool: &SqlitePool,
    status: Option<&str>,
    limit: u32,
    offset: u32,
) -> anyhow::Result<Vec<MeshSolution>> {
    let rows = if let Some(s) = status {
        sqlx::query_as::<_, SolutionRow>(
            "SELECT * FROM fleet_solutions WHERE status = ?1 ORDER BY updated_at DESC LIMIT ?2 OFFSET ?3",
        )
        .bind(s)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, SolutionRow>(
            "SELECT * FROM fleet_solutions ORDER BY updated_at DESC LIMIT ?1 OFFSET ?2",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?
    };
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Update solution status (promote/demote/retire).
pub async fn update_status(
    pool: &SqlitePool,
    id: &str,
    new_status: SolutionStatus,
) -> anyhow::Result<bool> {
    let now = Utc::now().to_rfc3339();
    let status_str = serde_json::to_string(&new_status)?;
    // Strip quotes from serde output: "\"candidate\"" -> "candidate"
    let status_str = status_str.trim_matches('"');
    let result = sqlx::query(
        "UPDATE fleet_solutions SET status = ?1, updated_at = ?2, version = version + 1 WHERE id = ?3",
    )
    .bind(status_str)
    .bind(&now)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Update confidence after a success or failure report.
pub async fn update_confidence(
    pool: &SqlitePool,
    id: &str,
    success: bool,
) -> anyhow::Result<()> {
    let now = Utc::now().to_rfc3339();
    if success {
        sqlx::query(
            "UPDATE fleet_solutions SET
                success_count = success_count + 1,
                confidence = CAST(success_count + 1 AS REAL) / CAST(success_count + 1 + fail_count AS REAL),
                updated_at = ?1, version = version + 1
             WHERE id = ?2",
        )
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    } else {
        sqlx::query(
            "UPDATE fleet_solutions SET
                fail_count = fail_count + 1,
                confidence = CAST(success_count AS REAL) / CAST(success_count + fail_count + 1 AS REAL),
                updated_at = ?1, version = version + 1
             WHERE id = ?2",
        )
        .bind(&now)
        .bind(id)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// Search solutions by keyword matching against symptoms, root_cause, and problem_key.
/// Returns up to `limit` results ordered by confidence descending.
/// Keywords are split on whitespace; a solution matches if it contains ALL keywords
/// (case-insensitive) across symptoms + root_cause + problem_key fields.
pub async fn search_solutions(
    pool: &SqlitePool,
    query: &str,
    limit: u32,
) -> anyhow::Result<Vec<MeshSolution>> {
    let keywords: Vec<&str> = query.split_whitespace()
        .filter(|w| w.len() >= 3)
        .collect();
    if keywords.is_empty() {
        return Ok(vec![]);
    }

    // Build WHERE clause: each keyword must appear in symptoms OR root_cause OR problem_key
    let mut conditions = Vec::new();
    let mut binds = Vec::new();
    for kw in &keywords {
        let pattern = format!("%{}%", kw.to_lowercase());
        let idx = binds.len();
        conditions.push(format!(
            "(LOWER(symptoms) LIKE ?{} OR LOWER(root_cause) LIKE ?{} OR LOWER(problem_key) LIKE ?{})",
            idx + 1, idx + 2, idx + 3
        ));
        binds.push(pattern.clone());
        binds.push(pattern.clone());
        binds.push(pattern);
    }

    let sql = format!(
        "SELECT * FROM fleet_solutions WHERE status != 'retired' AND {} ORDER BY confidence DESC, success_count DESC LIMIT {}",
        conditions.join(" AND "),
        limit
    );

    let mut q = sqlx::query_as::<_, SolutionRow>(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(pool).await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Get all candidate solutions eligible for promotion check.
pub async fn get_candidates(pool: &SqlitePool) -> anyhow::Result<Vec<MeshSolution>> {
    let rows = sqlx::query_as::<_, SolutionRow>(
        "SELECT * FROM fleet_solutions WHERE status = 'candidate' ORDER BY success_count DESC",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Count unique source nodes for a given problem_key.
pub async fn count_unique_nodes(pool: &SqlitePool, problem_key: &str) -> anyhow::Result<u32> {
    let row: (i32,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT source_node) FROM fleet_solutions WHERE problem_key = ?1 AND status != 'retired'",
    )
    .bind(problem_key)
    .fetch_one(pool)
    .await?;
    Ok(row.0 as u32)
}

// ─── Experiment CRUD ────────────────────────────────────────────────────────

pub async fn insert_experiment(pool: &SqlitePool, exp: &MeshExperiment) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO fleet_experiments (id, problem_key, hypothesis, test_plan, result, cost, node, created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
    )
    .bind(&exp.id)
    .bind(&exp.problem_key)
    .bind(&exp.hypothesis)
    .bind(&exp.test_plan)
    .bind(exp.result.map(|r| serde_json::to_string(&r).ok()).flatten())
    .bind(exp.cost)
    .bind(&exp.node)
    .bind(exp.created_at.to_rfc3339())
    .execute(pool)
    .await?;
    Ok(())
}

// ─── Incident CRUD ──────────────────────────────────────────────────────────

pub async fn insert_incident(pool: &SqlitePool, inc: &MeshIncident) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO fleet_incidents (id, node, problem_key, severity, cost, resolution, time_to_resolve_secs, resolved_by_tier, detected_at, resolved_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
    )
    .bind(&inc.id)
    .bind(&inc.node)
    .bind(&inc.problem_key)
    .bind(serde_json::to_string(&inc.severity).ok().map(|s| s.trim_matches('"').to_string()))
    .bind(inc.cost)
    .bind(&inc.resolution)
    .bind(inc.time_to_resolve_secs.map(|t| t as i64))
    .bind(inc.resolved_by_tier.map(|t| serde_json::to_string(&t).ok()).flatten().map(|s| s.trim_matches('"').to_string()))
    .bind(inc.detected_at.to_rfc3339())
    .bind(inc.resolved_at.map(|t| t.to_rfc3339()))
    .execute(pool)
    .await?;
    Ok(())
}

/// List recent incidents with pagination.
pub async fn list_incidents(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
) -> anyhow::Result<Vec<MeshIncident>> {
    let rows = sqlx::query_as::<_, IncidentRow>(
        "SELECT * FROM fleet_incidents ORDER BY detected_at DESC LIMIT ?1 OFFSET ?2",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.into()).collect())
}

/// Count incidents by problem_key in a recent time window (for systemic detection).
pub async fn count_recent_incidents(
    pool: &SqlitePool,
    problem_key: &str,
    window_minutes: u32,
) -> anyhow::Result<Vec<String>> {
    let cutoff = (Utc::now() - chrono::Duration::minutes(window_minutes as i64)).to_rfc3339();
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT node FROM fleet_incidents WHERE problem_key = ?1 AND detected_at > ?2",
    )
    .bind(problem_key)
    .bind(&cutoff)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Get fleet solution count by status (for dashboard).
pub async fn solution_counts(pool: &SqlitePool) -> anyhow::Result<Vec<(String, i64)>> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT status, COUNT(*) FROM fleet_solutions GROUP BY status",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Expire solutions that haven't been updated within their TTL.
pub async fn expire_stale_solutions(pool: &SqlitePool) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE fleet_solutions SET status = 'retired', updated_at = ?1
         WHERE status NOT IN ('retired', 'demoted')
           AND julianday('now') - julianday(updated_at) > ttl_days",
    )
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

// ─── SQLite Row Mapping ─────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct SolutionRow {
    id: String,
    problem_key: String,
    problem_hash: String,
    symptoms: String,
    environment: String,
    root_cause: String,
    fix_action: String,
    fix_type: String,
    status: String,
    success_count: i32,
    fail_count: i32,
    confidence: f64,
    cost_to_diagnose: f64,
    models_used: Option<String>,
    diagnosis_tier: String,
    source_node: String,
    venue_id: Option<String>,
    created_at: String,
    updated_at: String,
    version: i32,
    ttl_days: i32,
    tags: Option<String>,
}

impl From<SolutionRow> for MeshSolution {
    fn from(r: SolutionRow) -> Self {
        Self {
            id: r.id,
            problem_key: r.problem_key,
            problem_hash: r.problem_hash,
            symptoms: serde_json::from_str(&r.symptoms).unwrap_or_default(),
            environment: serde_json::from_str(&r.environment).unwrap_or_default(),
            root_cause: r.root_cause,
            fix_action: serde_json::from_str(&r.fix_action).unwrap_or_default(),
            fix_type: serde_json::from_str(&format!("\"{}\"", r.fix_type)).unwrap_or(FixType::Deterministic),
            status: serde_json::from_str(&format!("\"{}\"", r.status)).unwrap_or(SolutionStatus::Candidate),
            success_count: r.success_count as u32,
            fail_count: r.fail_count as u32,
            confidence: r.confidence,
            cost_to_diagnose: r.cost_to_diagnose,
            models_used: r.models_used.and_then(|m| serde_json::from_str(&m).ok()),
            diagnosis_tier: serde_json::from_str(&format!("\"{}\"", r.diagnosis_tier)).unwrap_or(DiagnosisTier::Deterministic),
            source_node: r.source_node,
            venue_id: r.venue_id,
            created_at: chrono::DateTime::parse_from_rfc3339(&r.created_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            updated_at: chrono::DateTime::parse_from_rfc3339(&r.updated_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            version: r.version as u32,
            ttl_days: r.ttl_days as u32,
            tags: r.tags.and_then(|t| serde_json::from_str(&t).ok()),
        }
    }
}

#[derive(sqlx::FromRow)]
struct IncidentRow {
    id: String,
    node: String,
    problem_key: String,
    severity: String,
    cost: f64,
    resolution: Option<String>,
    time_to_resolve_secs: Option<i64>,
    resolved_by_tier: Option<String>,
    detected_at: String,
    resolved_at: Option<String>,
}

impl From<IncidentRow> for MeshIncident {
    fn from(r: IncidentRow) -> Self {
        Self {
            id: r.id,
            node: r.node,
            problem_key: r.problem_key,
            severity: serde_json::from_str(&format!("\"{}\"", r.severity)).unwrap_or(IncidentSeverity::Medium),
            cost: r.cost,
            resolution: r.resolution,
            time_to_resolve_secs: r.time_to_resolve_secs.map(|t| t as u64),
            resolved_by_tier: r.resolved_by_tier.and_then(|t| serde_json::from_str(&format!("\"{}\"", t)).ok()),
            detected_at: chrono::DateTime::parse_from_rfc3339(&r.detected_at)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            resolved_at: r.resolved_at.and_then(|t| chrono::DateTime::parse_from_rfc3339(&t).ok().map(|d| d.with_timezone(&Utc))),
        }
    }
}
