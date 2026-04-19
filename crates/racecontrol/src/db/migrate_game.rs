//! Database migrations: game domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_game(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS game_launch_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            event_type TEXT NOT NULL,
            pid INTEGER,
            error_message TEXT,
            ai_suggestion TEXT,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // ─── Metrics: launch_events table (METRICS-01) ────────────────────────
    // Separate from legacy game_launch_events — richer schema with outcome, taxonomy, JSONL dual-write

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            car TEXT,
            track TEXT,
            session_type TEXT,
            timestamp TEXT NOT NULL,
            outcome TEXT NOT NULL,
            error_taxonomy TEXT,
            duration_to_playable_ms INTEGER,
            error_details TEXT,
            launch_args_hash TEXT,
            attempt_number INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_events_pod ON launch_events(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_events_combo ON launch_events(pod_id, sim_type, car, track)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_events_outcome ON launch_events(outcome)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_events_created ON launch_events(created_at)")
        .execute(pool)
        .await?;


    // ─── Phase 365: AI behavior samples (GLD-E-01) ──────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_behavior_samples (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assettocorsa',
            car TEXT NOT NULL,
            track TEXT NOT NULL,
            ai_level INTEGER NOT NULL,
            difficulty_tier TEXT NOT NULL,
            lap_count INTEGER NOT NULL,
            median_lap_ms INTEGER NOT NULL,
            p25_lap_ms INTEGER,
            p75_lap_ms INTEGER,
            sampled_at TEXT NOT NULL DEFAULT (datetime('now')),
            kb_batch_id TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_behavior_samples_combo ON ai_behavior_samples(car, track, difficulty_tier)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_behavior_samples_sampled ON ai_behavior_samples(sampled_at)")
        .execute(pool)
        .await?;


    // ─── Combo reliability (INTEL-01) ───────────────────────────────────────
    // Materialized rolling 30-day success rates per (pod, sim, car, track) combo.
    // Updated after every record_launch_event call via update_combo_reliability().
    // NULL car/track handled via IS NULL / OR comparison in queries.
    // No COALESCE in PRIMARY KEY (SQLite limitation) — use id row + unique index.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS combo_reliability (
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            car TEXT,
            track TEXT,
            success_rate REAL NOT NULL DEFAULT 0.0,
            avg_time_to_track_ms REAL,
            p95_time_to_track_ms REAL,
            total_launches INTEGER NOT NULL DEFAULT 0,
            common_failure_modes TEXT,
            last_updated TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_combo_rel_pk ON combo_reliability(pod_id, sim_type, COALESCE(car, ''), COALESCE(track, ''))")
        .execute(pool)
        .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_combo_rel_sim ON combo_reliability(sim_type)")
        .execute(pool)
        .await?;


    // ─── Game Intelligence (v41.0 Phase 317) ────────────────────────────────

    // pod_game_inventory: per-pod game install scan results (INV-02).
    // Upserted on each GameInventoryUpdate WS message. PRIMARY KEY (pod_id, game_id).
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_game_inventory (
            pod_id TEXT NOT NULL,
            game_id TEXT NOT NULL,
            display_name TEXT NOT NULL,
            sim_type TEXT,
            exe_path TEXT NOT NULL,
            launchable INTEGER NOT NULL DEFAULT 1,
            scan_method TEXT NOT NULL,
            steam_app_id INTEGER,
            scanned_at TEXT NOT NULL,
            server_received_at TEXT NOT NULL,
            PRIMARY KEY (pod_id, game_id)
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_pod_game_inv_game ON pod_game_inventory(game_id)",
    )
    .execute(pool)
    .await?;


    // combo_validation_flags: per-pod per-preset combo validation results (COMBO-03/04).
    // Upserted on each ComboValidationReport WS message. PRIMARY KEY (pod_id, preset_id).
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS combo_validation_flags (
            pod_id TEXT NOT NULL,
            preset_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Unknown',
            failure_reasons TEXT NOT NULL DEFAULT '[]',
            validated_at TEXT NOT NULL,
            server_received_at TEXT NOT NULL,
            PRIMARY KEY (pod_id, preset_id)
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_combo_val_preset ON combo_validation_flags(preset_id)",
    )
    .execute(pool)
    .await?;


    // launch_timeline_spans: per-launch step-level timeline spans (LAUNCH-05).
    // Stores LaunchTimeline as serialized JSON events per launch attempt.
    // launch_id is a UUID v4 generated on the agent when tracking starts.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_timeline_spans (
            launch_id   TEXT PRIMARY KEY,
            pod_id      TEXT NOT NULL,
            sim_type    TEXT NOT NULL,
            preset_id   TEXT,
            billing_session_id TEXT,
            outcome     TEXT NOT NULL,
            total_duration_ms INTEGER NOT NULL,
            started_at  TEXT NOT NULL,
            events_json TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_launch_timeline_pod ON launch_timeline_spans(pod_id)",
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_launch_timeline_created ON launch_timeline_spans(created_at)",
    )
    .execute(pool)
    .await?;


    // ─── Phase 368: Launch notes (append-only staff annotations per launch) ──
    // D-02: append-only audit trail for post-mortems; replicated via cloud_sync.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS launch_notes (
            id          TEXT PRIMARY KEY,
            launch_id   TEXT NOT NULL,
            pod_id      TEXT NOT NULL,
            staff_id    TEXT,
            staff_name  TEXT,
            body        TEXT NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_launch_notes_launch_id ON launch_notes(launch_id)",
    )
    .execute(pool)
    .await?;


    // D-11: staff_dismissed_at marks NeedsManualIntervention cards as acknowledged.
    // Idempotent: ignore error if column already exists (existing databases).
    let _ = sqlx::query(
        "ALTER TABLE launch_timeline_spans ADD COLUMN staff_dismissed_at TEXT",
    )
    .execute(pool)
    .await;

    // Intentional: ignore error if column already exists (idempotent migration pattern)

    // ─── AC LAN tables ──────────────────────────────────────────────────────

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ac_presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            config_json TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ac_sessions (
            id TEXT PRIMARY KEY,
            preset_id TEXT,
            config_json TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'starting',
            pod_ids TEXT,
            pid INTEGER,
            join_url TEXT,
            error_message TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ac_sessions_status ON ac_sessions(status)")
        .execute(pool)
        .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_events_pod ON game_launch_events(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_events_type ON game_launch_events(event_type)")
        .execute(pool)
        .await?;

    // Pattern H: lower-bound clean-exit signal — TRUE when exit_code == 0 AND
    // seconds_since_launch >= 30 AND no WerFault.exe child for the PID in last 10s.
    // Written by rc-agent at crash-event emission; consumers filter with WHERE clean_exit_heuristic = 0
    // to see real crashes. event_type stays "crashed" for all non-agent-initiated exits (no query churn).
    // See OPEN-PATTERNS.md Pattern H for full rationale.
    let _ = sqlx::query("ALTER TABLE game_launch_events ADD COLUMN clean_exit_heuristic INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE launch_events ADD COLUMN session_id TEXT")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_launch_events_session ON launch_events (session_id)")
        .execute(pool)
        .await?;


    // ─── Dynamic port allocation columns on ac_sessions ──────────────────────
    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN udp_port INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN tcp_port INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE ac_sessions ADD COLUMN http_port INTEGER")
        .execute(pool)
        .await;


    // ─── Phase 257: BILL-03 PWA game request TTL ─────────────────────────────
    // game_launch_requests: tracks customer PWA game requests with a 10-minute server-side TTL.
    // expires_at is set to now + 10 minutes on INSERT; background cleanup marks pending → expired.
    // resolved_at / resolved_by are set when staff confirms or rejects the request.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS game_launch_requests (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            resolved_at TEXT,
            resolved_by TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_game_launch_requests_status ON game_launch_requests(status, expires_at)",
    )
    .execute(pool)
    .await?;


    // ─── Phase 298: Game Preset Library ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS game_presets (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            game TEXT NOT NULL,
            car TEXT,
            track TEXT,
            session_type TEXT,
            notes TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_presets_game ON game_presets(game)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_game_presets_enabled ON game_presets(enabled)")
        .execute(pool)
        .await?;


    Ok(())
}
