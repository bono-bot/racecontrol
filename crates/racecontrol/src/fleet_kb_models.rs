//! Fleet KB — Model Evaluation + Reputation stores (EVAL-03, MREP-04).
//!
//! Server-side storage for model evaluation records and reputation data
//! pushed from rc-agent pods.

use sqlx::SqlitePool;

// ─── Model Evaluation Store (EVAL-03) ─────────────────────────────────────────

/// Create model_evaluations table on server. Called from db::migrate().
pub async fn migrate_eval_store(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
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
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_svc_eval_model_id ON model_evaluations (model_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_svc_eval_created_at ON model_evaluations (created_at)",
    )
    .execute(pool)
    .await?;

    tracing::info!("Model evaluation store table initialized (EVAL-03)");
    Ok(())
}

/// Insert one evaluation record from an rc-agent push. Uses INSERT OR IGNORE to be idempotent.
pub async fn insert_eval_record(
    pool: &SqlitePool,
    rec: &rc_common::protocol::EvalRecordPayload,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR IGNORE INTO model_evaluations \
         (id, model_id, pod_id, trigger_type, prediction, actual_outcome, correct, cost_usd, created_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&rec.id)
    .bind(&rec.model_id)
    .bind(&rec.pod_id)
    .bind(&rec.trigger_type)
    .bind(&rec.prediction)
    .bind(&rec.actual_outcome)
    .bind(rec.correct as i64)
    .bind(rec.cost_usd)
    .bind(&rec.created_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query evaluation records with optional filters. Used by GET /api/v1/models/evaluations.
pub async fn query_eval_records(
    pool: &SqlitePool,
    model_id: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<rc_common::protocol::EvalRecordPayload>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT id, model_id, pod_id, trigger_type, prediction, actual_outcome, correct, cost_usd, created_at \
         FROM model_evaluations WHERE 1=1",
    );
    if let Some(m) = model_id {
        qb.push(" AND model_id = ").push_bind(m);
    }
    if let Some(f) = from {
        qb.push(" AND created_at >= ").push_bind(f);
    }
    if let Some(t) = to {
        qb.push(" AND created_at <= ").push_bind(t);
    }
    qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit);

    let rows = qb.build().fetch_all(pool).await?;
    let records = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            rc_common::protocol::EvalRecordPayload {
                id: row.get("id"),
                model_id: row.get("model_id"),
                pod_id: row.get("pod_id"),
                trigger_type: row.get("trigger_type"),
                prediction: row.get("prediction"),
                actual_outcome: row.get("actual_outcome"),
                correct: row.get::<i64, _>("correct") != 0,
                cost_usd: row.get("cost_usd"),
                created_at: row.get("created_at"),
            }
        })
        .collect();
    Ok(records)
}

// ─── Model Reputation Store (MREP-04) ─────────────────────────────────────────

/// Create server-side model_reputation table. Called from db::migrate().
pub async fn migrate_reputation_store(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_reputation (
            model_id TEXT PRIMARY KEY,
            correct_count INTEGER NOT NULL DEFAULT 0,
            total_count INTEGER NOT NULL DEFAULT 0,
            accuracy REAL NOT NULL DEFAULT 0.0,
            status TEXT NOT NULL DEFAULT 'active',
            cost_per_correct_usd REAL NOT NULL DEFAULT 0.0,
            pod_id TEXT NOT NULL DEFAULT '',
            updated_at TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_rep_status ON model_reputation (status)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_rep_accuracy ON model_reputation (accuracy)",
    )
    .execute(pool)
    .await?;

    tracing::info!("Model reputation store table initialized (MREP-04)");
    Ok(())
}

/// Upsert one reputation row from a ModelReputationSync push (idempotent via ON CONFLICT DO UPDATE).
pub async fn upsert_reputation(
    pool: &SqlitePool,
    row: &rc_common::protocol::ReputationPayload,
    pod_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO model_reputation \
         (model_id, correct_count, total_count, accuracy, status, cost_per_correct_usd, pod_id, updated_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(model_id) DO UPDATE SET \
           correct_count = excluded.correct_count, \
           total_count = excluded.total_count, \
           accuracy = excluded.accuracy, \
           status = excluded.status, \
           cost_per_correct_usd = excluded.cost_per_correct_usd, \
           pod_id = excluded.pod_id, \
           updated_at = excluded.updated_at",
    )
    .bind(&row.model_id)
    .bind(row.correct_count as i64)
    .bind(row.total_count as i64)
    .bind(row.accuracy)
    .bind(&row.status)
    .bind(row.cost_per_correct_usd)
    .bind(pod_id)
    .bind(&row.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all reputation rows with optional status filter.
/// Used by GET /api/v1/models/reputation. Returns rows sorted by accuracy DESC.
pub async fn query_reputation(
    pool: &SqlitePool,
    status_filter: Option<&str>,
) -> anyhow::Result<Vec<rc_common::protocol::ReputationPayload>> {
    let mut qb = sqlx::QueryBuilder::new(
        "SELECT model_id, correct_count, total_count, accuracy, status, cost_per_correct_usd, updated_at \
         FROM model_reputation WHERE 1=1",
    );
    if let Some(s) = status_filter {
        qb.push(" AND status = ").push_bind(s);
    }
    qb.push(" ORDER BY accuracy DESC");

    let rows = qb.build().fetch_all(pool).await?;
    let records = rows
        .iter()
        .map(|row| {
            use sqlx::Row;
            rc_common::protocol::ReputationPayload {
                model_id: row.get("model_id"),
                correct_count: row.get::<i64, _>("correct_count") as u32,
                total_count: row.get::<i64, _>("total_count") as u32,
                accuracy: row.get("accuracy"),
                status: row.get("status"),
                cost_per_correct_usd: row.get("cost_per_correct_usd"),
                updated_at: row.get("updated_at"),
            }
        })
        .collect();
    Ok(records)
}
