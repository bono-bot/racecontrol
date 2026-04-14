//! Database migrations: ops domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_ops(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Pod Activity Log (unified event stream) ─────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_activity_log (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            pod_number INTEGER DEFAULT 0,
            timestamp TEXT DEFAULT (datetime('now')),
            category TEXT NOT NULL,
            action TEXT NOT NULL,
            details TEXT DEFAULT '',
            source TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_pod ON pod_activity_log (pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_ts ON pod_activity_log (timestamp)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_cat ON pod_activity_log (category)")
        .execute(pool)
        .await?;


    // ─── Phase 307: Hash chain columns for append-only audit integrity ────────
    // entry_hash:    SHA-256 of (id|timestamp|category|action|details|source|previous_hash)
    // previous_hash: hash of the preceding entry, or "GENESIS" for first hashed entry
    // Pre-migration rows retain NULL - they are outside the chain and not verified.
    let _ = sqlx::query("ALTER TABLE pod_activity_log ADD COLUMN entry_hash TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE pod_activity_log ADD COLUMN previous_hash TEXT")
        .execute(pool)
        .await;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_hash ON pod_activity_log (entry_hash)")
        .execute(pool)
        .await?;


    // ─── Phase 310: Session trace ID for end-to-end customer journey tracing ───
    let _ = sqlx::query("ALTER TABLE pod_activity_log ADD COLUMN session_id TEXT")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_activity_session ON pod_activity_log (session_id)")
        .execute(pool)
        .await?;

    // ─── Phase 56: WhatsApp Alerting + Weekly Report ───────────────────────────
    // Uptime sampling for weekly report
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_uptime_samples (
            pod_id TEXT NOT NULL,
            sampled_at TEXT NOT NULL,
            ws_connected INTEGER NOT NULL,
            PRIMARY KEY (pod_id, sampled_at)
        )"
    ).execute(pool).await?;


    // Alert incident log for weekly report
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS alert_incidents (
            id TEXT PRIMARY KEY,
            alert_type TEXT NOT NULL,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            resolved_at TEXT,
            pod_count INTEGER,
            description TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_alert_incidents_type ON alert_incidents(alert_type)")
        .execute(pool).await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_alert_incidents_started ON alert_incidents(started_at)")
        .execute(pool).await?;


    // Phase 352: Add subsystem-level columns to alert_incidents (per OPS-05/D4)
    for col in &[
        "ALTER TABLE alert_incidents ADD COLUMN subsystem TEXT",
        "ALTER TABLE alert_incidents ADD COLUMN severity TEXT DEFAULT 'info'",
        "ALTER TABLE alert_incidents ADD COLUMN correlation_id TEXT",
    ] {

        if let Err(e) = sqlx::query(col).execute(pool).await {
            let err_str = e.to_string();
            if !err_str.contains("duplicate column") {
                tracing::warn!(target: "db", "alert_incidents migration: {}", err_str);
            }
        }
    }
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_alert_incidents_subsystem ON alert_incidents(subsystem)")
        .execute(pool).await?;


    // --- Psychology Foundation (Phase 1) ---

    // ─── Deploy Audit Log (Phase 177) ────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS deploy_logs (
            id TEXT PRIMARY KEY,
            app TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            deployer TEXT NOT NULL DEFAULT 'james',
            result TEXT NOT NULL,
            pages_before INTEGER,
            pages_after INTEGER,
            pages_missing TEXT,
            duration_secs INTEGER,
            error TEXT,
            build_hash TEXT
        )",
    )
    .execute(pool)
    .await?;


    // ─── App Health Log (Phase 179) ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS app_health_log (
            id TEXT PRIMARY KEY,
            app TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            status TEXT NOT NULL,
            pages_expected INTEGER,
            pages_available INTEGER,
            response_ms INTEGER,
            error TEXT
        )",
    )
    .execute(pool)
    .await?;


    // v38.0 Phase 1.5: Add semantic_status column to app_health_log
    let _ = sqlx::query("ALTER TABLE app_health_log ADD COLUMN semantic_status TEXT")
        .execute(pool)
        .await;


    // v38.0 Phase 3: App restart log table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS app_restart_log (
            id TEXT PRIMARY KEY,
            app TEXT NOT NULL,
            trigger TEXT NOT NULL,
            outcome TEXT NOT NULL,
            pm2_stdout TEXT,
            timestamp TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    // v38.0 Phase 5: Synthetic transaction probes table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS synthetic_probes (
            id TEXT PRIMARY KEY,
            probe_name TEXT NOT NULL,
            passed INTEGER NOT NULL,
            response_ms INTEGER,
            error TEXT,
            timestamp TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    // fleet_kb migrations (Meshed Intelligence, Eval Store, Reputation Store) are called from mod.rs

    // ─── RESIL-06: Pod crash event tracking ────────────────────────────────
    // Records each game crash per pod with timestamp, enabling >3/hour detection.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_crash_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            crash_type TEXT NOT NULL DEFAULT 'game_crash',
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;


    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_crash_events_pod_time ON pod_crash_events(pod_id, created_at)"
    )
    .execute(pool)
    .await;


    // Phase 302: system_events table for structured system-wide event archive.
    // NOTE: NOT the same as `events` (hotlap/competition) or `scheduler_events` (pod wake/sleep).
    // This is the cross-subsystem operational log: billing, deploy, alert, pod recovery, etc.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS system_events (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            source TEXT NOT NULL,
            pod TEXT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            payload TEXT NOT NULL DEFAULT '{}',
            venue_id TEXT NOT NULL DEFAULT 'racingpoint-hyd-001'
        )"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_system_events_type ON system_events(event_type)"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_system_events_pod ON system_events(pod)"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_system_events_ts ON system_events(timestamp)"
    )
    .execute(pool)
    .await?;


    // Cross-domain venue_id migration is in mod.rs

    // GLD-F-03: Content drift events audit log
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS content_drift_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            detected_at TEXT NOT NULL,
            game_key TEXT NOT NULL,
            delta_type TEXT NOT NULL,
            item TEXT NOT NULL,
            resolved_at TEXT,
            resolution_note TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_content_drift_pod ON content_drift_events(pod_id)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_content_drift_detected ON content_drift_events(detected_at)",
    )
    .execute(pool)
    .await?;


    tracing::info!("Phase 366 Fleet Intelligence schema migrated");

    Ok(())
}
