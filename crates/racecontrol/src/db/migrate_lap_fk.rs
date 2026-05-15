//! §S-146 lap-FK-gap fix migration (2026-05-14).
//!
//! Drops the FK constraint on `laps.session_id REFERENCES sessions(id)`.
//!
//! The column holds `billing_session_id` at runtime (per
//! `363-RESEARCH.md` Open Question 1 and the schema comment at
//! `migrate_policy.rs:141-143`), but the FK declaration points at the WRONG
//! parent table — `sessions` is barely populated, `billing_sessions` holds the
//! values that arrive. SQLite error 787 fires silently on every INSERT,
//! producing 0 recorded laps over the live Pod 6 / Vishal / Monza event
//! 2026-05-14 14:23–14:30 IST.
//!
//! MMA Step 2 5/5 unanimous Candidate A (confidence 0.948): drop FK, keep the
//! column as annotation only. Audit trail:
//! - `.planning/audits/RCA-lap-fk-gap-vms-grounded-20260514.md`
//! - `.planning/audits/RCA-lap-fk-gap-MMA-DIAGNOSE-20260514.md`
//! - `.planning/audits/RCA-lap-fk-gap-MMA-PLAN-20260514.md`
//!
//! Runs LAST in the migration chain so all upstream ALTER TABLE column
//! additions (`car_class`, `suspect`, `assist_config_hash`, `assist_tier`,
//! `billing_session_id`, `validity`, `venue_id`, `review_required`,
//! `session_type`) are present before the table swap.
//!
//! Sibling FK preservation: `personal_bests.lap_id`, `track_records.lap_id`,
//! and `telemetry_samples.lap_id` all REFERENCE `laps(id)`. Per Captain
//! NF-james-4 standing rule (SQLite ALTER TABLE RENAME rewrites sibling FK
//! references when `legacy_alter_table=0`), we explicitly set
//! `legacy_alter_table=1` during the swap so sibling FK clauses are NOT
//! rewritten to point at a transient `laps_new` name.

use sqlx::sqlite::SqlitePool;
use sqlx::{Acquire, Row};

pub(crate) async fn migrate_lap_fk(pool: &SqlitePool) -> anyhow::Result<()> {
    let existing_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='laps'",
    )
    .fetch_optional(pool)
    .await?;

    let Some(sql) = existing_sql else {
        return Ok(());
    };

    if !sql.contains("REFERENCES sessions(id)") {
        return Ok(());
    }

    tracing::warn!(
        "§S-146 lap-FK-gap: detected laps.session_id FK to sessions(id) — \
         rewriting table to drop FK (MMA Step 2 Candidate A, 5/5 unanimous)"
    );

    // Dedicated connection so PRAGMA state stays scoped to this migration.
    let mut conn = pool.acquire().await?;

    sqlx::query("PRAGMA foreign_keys=OFF")
        .execute(&mut *conn)
        .await?;
    sqlx::query("PRAGMA legacy_alter_table=1")
        .execute(&mut *conn)
        .await?;

    // Enumerate the live column set so we copy whatever schema the DB
    // accumulated (base + every ALTER TABLE ADD COLUMN above this point).
    let col_rows = sqlx::query("PRAGMA table_info('laps')")
        .fetch_all(&mut *conn)
        .await?;
    let col_names: Vec<String> = col_rows
        .iter()
        .map(|r| r.get::<String, _>("name"))
        .collect();

    if col_names.is_empty() {
        anyhow::bail!("§S-146: PRAGMA table_info returned no columns for laps");
    }

    // Build the new CREATE statement by substituting just the FK clause.
    // sqlite_master.sql stores the literal text from the original CREATE.
    let new_create = sql
        .replacen(
            "session_id TEXT REFERENCES sessions(id)",
            "session_id TEXT",
            1,
        )
        .replacen("CREATE TABLE laps", "CREATE TABLE laps_new", 1)
        .replacen("CREATE TABLE \"laps\"", "CREATE TABLE \"laps_new\"", 1);

    if !new_create.contains("laps_new") {
        anyhow::bail!(
            "§S-146: could not rewrite CREATE statement (rename substitution missed); \
             aborting to preserve original table"
        );
    }
    if new_create.contains("REFERENCES sessions(id)") {
        anyhow::bail!(
            "§S-146: FK clause still present after substitution; aborting"
        );
    }

    let col_list = col_names.join(", ");
    let copy_sql = format!(
        "INSERT INTO laps_new ({0}) SELECT {0} FROM laps",
        col_list
    );

    let mut tx = conn.begin().await?;
    sqlx::query(&new_create).execute(&mut *tx).await?;
    sqlx::query(&copy_sql).execute(&mut *tx).await?;
    sqlx::query("DROP TABLE laps").execute(&mut *tx).await?;
    sqlx::query("ALTER TABLE laps_new RENAME TO laps")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Recreate every index on laps that earlier migrations created.
    // Each `CREATE INDEX IF NOT EXISTS` is idempotent. Mirrors:
    //   migrate_core.rs:299/303/307 + migrate_core_columns.rs:98/102/130/237.
    for index_sql in [
        "CREATE INDEX IF NOT EXISTS idx_laps_session ON laps(session_id)",
        "CREATE INDEX IF NOT EXISTS idx_laps_driver ON laps(driver_id)",
        "CREATE INDEX IF NOT EXISTS idx_laps_track_car ON laps(track, car)",
        "CREATE INDEX IF NOT EXISTS idx_laps_leaderboard ON laps(track, car, valid, lap_time_ms)",
        "CREATE INDEX IF NOT EXISTS idx_laps_driver_created ON laps(driver_id, created_at)",
        "CREATE INDEX IF NOT EXISTS idx_laps_car_class ON laps(track, car_class)",
        "CREATE INDEX IF NOT EXISTS idx_laps_assist_tier ON laps(track, assist_tier, validity)",
    ] {
        let _ = sqlx::query(index_sql).execute(&mut *conn).await;
    }

    sqlx::query("PRAGMA legacy_alter_table=0")
        .execute(&mut *conn)
        .await?;
    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&mut *conn)
        .await?;

    // Verify the FK is gone before declaring success.
    let after_sql: Option<String> = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master WHERE type='table' AND name='laps'",
    )
    .fetch_optional(&mut *conn)
    .await?;
    match after_sql {
        Some(s) if s.contains("REFERENCES sessions(id)") => {
            anyhow::bail!(
                "§S-146: laps.session_id FK still present after rewrite — schema unchanged"
            );
        }
        Some(_) => {
            tracing::warn!(
                "§S-146: laps FK drop complete; sibling FKs preserved via legacy_alter_table=1"
            );
            Ok(())
        }
        None => anyhow::bail!("§S-146: laps table missing after rewrite"),
    }
}
