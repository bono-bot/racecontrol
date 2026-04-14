//! Database migrations: config domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_config(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── AI suggestions table ─────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_suggestions (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT NOT NULL,
            error_context TEXT,
            suggestion TEXT NOT NULL,
            model TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'crash',
            dismissed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_suggestions_pod ON ai_suggestions(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_suggestions_created ON ai_suggestions(created_at)")
        .execute(pool)
        .await?;


    // ─── AI training pairs (Ollama learning from Claude CLI) ─────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_training_pairs (
            id TEXT PRIMARY KEY,
            query_hash TEXT NOT NULL,
            query_text TEXT NOT NULL,
            query_keywords TEXT NOT NULL,
            response_text TEXT NOT NULL,
            source TEXT NOT NULL DEFAULT 'unknown',
            model TEXT NOT NULL,
            quality_score INTEGER NOT NULL DEFAULT 1,
            use_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_training_hash ON ai_training_pairs(query_hash)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_training_keywords ON ai_training_pairs(query_keywords)")
        .execute(pool)
        .await?;


    // Phase 301: Cloud Data Sync v2 migrations
    // model_evaluations table (SYNC-02 + v35 fleet_kb): unified schema
    // James v37 fields: model_name, problem_key, actual, diagnosis_tier, updated_at, venue_id
    // Bono v35 fields: model_id, trigger_type, actual_outcome
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS model_evaluations (
            id TEXT PRIMARY KEY,
            model_id TEXT NOT NULL DEFAULT '',
            model_name TEXT NOT NULL DEFAULT '',
            pod_id TEXT,
            trigger_type TEXT NOT NULL DEFAULT '',
            problem_key TEXT,
            prediction TEXT,
            actual TEXT,
            actual_outcome TEXT NOT NULL DEFAULT '',
            correct INTEGER NOT NULL DEFAULT 0,
            cost_usd REAL DEFAULT 0,
            diagnosis_tier TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
            venue_id TEXT
        )",
    )
    .execute(pool)
    .await?;


    // ─── Debug system tables ──────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_playbooks (
            id TEXT PRIMARY KEY,
            category TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            steps TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_incidents (
            id TEXT PRIMARY KEY,
            pod_id TEXT,
            category TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            context_snapshot TEXT,
            playbook_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_status ON debug_incidents(status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_category ON debug_incidents(category)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_inc_created ON debug_incidents(created_at)")
        .execute(pool)
        .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debug_resolutions (
            id TEXT PRIMARY KEY,
            incident_id TEXT NOT NULL,
            category TEXT NOT NULL,
            resolution_text TEXT NOT NULL,
            effectiveness INTEGER NOT NULL DEFAULT 3,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_res_category ON debug_resolutions(category)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debug_res_incident ON debug_resolutions(incident_id)")
        .execute(pool)
        .await?;


    // Seed debug playbooks
    let playbooks = [
        ("pb_pod_offline", "pod_offline", "Pod Offline / Not Responding", r#"[{"step_number":1,"action":"Ping the pod IP address","expected_result":"Reply from pod IP","timeout_seconds":5},{"step_number":2,"action":"Check pod-agent on port 8090 (curl http://<ip>:8090/ping)","expected_result":"pong response","timeout_seconds":10},{"step_number":3,"action":"Check Windows Firewall (all profiles: Domain, Private, Public)","expected_result":"Firewall disabled or port 8090 allowed","timeout_seconds":30},{"step_number":4,"action":"TCP scan subnet for DHCP drift (port 8090 across 192.168.31.2-254)","expected_result":"Find pod on new IP","timeout_seconds":60},{"step_number":5,"action":"Send Wake-on-LAN magic packet","expected_result":"Pod powers on and responds within 30s","timeout_seconds":45}]"#),
        ("pb_game_crash", "game_crash", "Game Crash / Won't Launch", r#"[{"step_number":1,"action":"Check if acs.exe process is running on the pod","expected_result":"Process listed or confirmed dead","timeout_seconds":10},{"step_number":2,"action":"Verify race.ini has AUTOSPAWN=1","expected_result":"AUTOSPAWN=1 present in race.ini","timeout_seconds":15},{"step_number":3,"action":"Check CSP gui.ini for FORCE_START=1 and HIDE_MAIN_MENU=1","expected_result":"Both settings enabled","timeout_seconds":15},{"step_number":4,"action":"Check disk space on pod (must have >1GB free)","expected_result":"Sufficient disk space available","timeout_seconds":10},{"step_number":5,"action":"Kill acs.exe and relaunch AC with correct working directory","expected_result":"AC launches successfully","timeout_seconds":30}]"#),
        ("pb_billing_stuck", "billing_stuck", "Billing / Timer Stuck", r#"[{"step_number":1,"action":"Check billing_sessions table for session status","expected_result":"Session found with correct status","timeout_seconds":10},{"step_number":2,"action":"Verify WebSocket connection between agent and core","expected_result":"WebSocket connected and receiving messages","timeout_seconds":10},{"step_number":3,"action":"Check billing tick loop is running (look for BillingTick events)","expected_result":"Tick events arriving every second","timeout_seconds":15},{"step_number":4,"action":"Restart billing session via API if stuck","expected_result":"Billing resumes with correct remaining time","timeout_seconds":20}]"#),
        ("pb_screen_stuck", "screen_stuck", "Blank / Stuck Screen", r#"[{"step_number":1,"action":"Check if Edge kiosk browser process is running","expected_result":"msedge.exe process found","timeout_seconds":10},{"step_number":2,"action":"Verify lock screen server on port 18923","expected_result":"HTTP 200 from localhost:18923","timeout_seconds":10},{"step_number":3,"action":"Kill and restart lock screen browser (msedge.exe)","expected_result":"Lock screen reappears","timeout_seconds":15},{"step_number":4,"action":"Check Windows screen blanking / power settings","expected_result":"Screen never turns off","timeout_seconds":10}]"#),
        ("pb_no_steering", "no_steering_input", "No Steering / Pedal Input", r#"[{"step_number":1,"action":"Check USB wheelbase connection (VID:1209 PID:FFB0)","expected_result":"Device visible in Device Manager","timeout_seconds":15},{"step_number":2,"action":"Verify Conspit Link 2.0 is running","expected_result":"ConspitLink2.0.exe process found","timeout_seconds":10},{"step_number":3,"action":"Restart ConspitLink2.0.exe","expected_result":"Wheel display shows telemetry data","timeout_seconds":15},{"step_number":4,"action":"Check Device Manager for USB errors or disabled devices","expected_result":"No errors on HID devices","timeout_seconds":15}]"#),
        ("pb_high_idle", "high_idle_time", "High Idle Time / Not Counting", r#"[{"step_number":1,"action":"Check driving_state for the pod","expected_result":"Should be 'active' during gameplay","timeout_seconds":5},{"step_number":2,"action":"Verify UDP telemetry arriving on port 9996","expected_result":"Packets received from AC","timeout_seconds":10},{"step_number":3,"action":"Check 10-second idle threshold configuration","expected_result":"Threshold set correctly in config","timeout_seconds":5},{"step_number":4,"action":"Inspect game state — is AC actually running and in a session?","expected_result":"AC running with active driving session","timeout_seconds":10}]"#),
        ("pb_sync_failure", "sync_failure", "Cloud Sync Failure", r#"[{"step_number":1,"action":"Check cloud reachability (ping 72.60.101.58)","expected_result":"Cloud server responds","timeout_seconds":10},{"step_number":2,"action":"Verify sync_log for recent errors","expected_result":"No errors in last sync cycle","timeout_seconds":10},{"step_number":3,"action":"Check internet connectivity (ping 8.8.8.8)","expected_result":"Internet reachable","timeout_seconds":5},{"step_number":4,"action":"Restart cloud_sync module","expected_result":"Sync resumes and pushes pending changes","timeout_seconds":30}]"#),
        ("pb_kiosk_bypass", "kiosk_bypass", "Kiosk Bypass / Desktop Access", r#"[{"step_number":1,"action":"Check kiosk lockdown setting in rc-agent config","expected_result":"Kiosk mode enabled","timeout_seconds":5},{"step_number":2,"action":"Verify keyboard hook is active (blocks Alt+Tab, Ctrl+Esc)","expected_result":"System shortcuts blocked","timeout_seconds":10},{"step_number":3,"action":"Check that taskbar is hidden","expected_result":"Taskbar not visible","timeout_seconds":5},{"step_number":4,"action":"Re-enable kiosk mode and restart lock screen","expected_result":"Kiosk fully locked down","timeout_seconds":15}]"#),
    ];

    for (id, category, title, steps) in &playbooks {

        sqlx::query(
            "INSERT OR IGNORE INTO debug_playbooks (id, category, title, steps) VALUES (?, ?, ?, ?)"
        )
        .bind(id)
        .bind(category)
        .bind(title)
        .bind(steps)
        .execute(pool)
        .await?;

    }

    // ─── System Settings (PIN rotation tracking, etc.) ────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS system_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS feature_flags (
            name TEXT PRIMARY KEY,
            enabled BOOLEAN NOT NULL DEFAULT 0,
            default_value BOOLEAN NOT NULL DEFAULT 0,
            overrides TEXT NOT NULL DEFAULT '{}',
            version INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS config_push_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            pod_id TEXT NOT NULL,
            payload TEXT NOT NULL,
            seq_num INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT (datetime('now')),
            acked_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS config_audit_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            action TEXT NOT NULL,
            entity_type TEXT NOT NULL,
            entity_name TEXT NOT NULL,
            old_value TEXT,
            new_value TEXT,
            pushed_by TEXT NOT NULL,
            pods_acked TEXT NOT NULL DEFAULT '[]',
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // ─── Phase 296 PUSH-01: Per-pod AgentConfig storage ──────────────────────
    // Stores the full AgentConfig JSON for each pod so the server can push it
    // on WebSocket connect without requiring manual TOML file editing on pods.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_configs (
            pod_id TEXT PRIMARY KEY,
            config_json TEXT NOT NULL,
            config_hash TEXT NOT NULL,
            schema_version INTEGER NOT NULL DEFAULT 1,
            last_modified TEXT DEFAULT (datetime('now')),
            updated_by TEXT NOT NULL DEFAULT 'system'
        )",
    )
    .execute(pool)
    .await?;


    // ─── Phase 12: Data Foundation ───────────────────────────────────────────

    // Phase 363 kill switch — default enabled; admin can toggle to false to bypass all audit paths
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('phase363_session_audit', 1, 1, '{}')",
    )
    .execute(pool)
    .await;


    // Phase 364 kill switch — default enabled; gates lap consistency checker + quality gap detectors
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('phase364_quality_monitor', 1, 1, '{}')",
    )
    .execute(pool)
    .await;


    // Phase 365 kill switch — MMA batch (weekly KB generation)
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('phase365_mma_batch', 1, 1, '{}')",
    )
    .execute(pool)
    .await;


    // Phase 365 kill switch — anomaly detection at session end
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('phase365_anomaly_detection', 1, 1, '{}')",
    )
    .execute(pool)
    .await;


    // Phase 368 kill switch — kiosk launch cards (default disabled for shadow deploy)
    // D-13 (P2-06): flag lives in the DB feature_flags table, NOT racecontrol.toml.
    // Kiosk reads via GET /api/v1/flags + Phase 177+ spawn_periodic_refetch (5-min interval).
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO feature_flags (name, enabled, default_value, overrides)
         VALUES ('kiosk_launch_cards_enabled', 0, 0, '{}')",
    )
    .execute(pool)
    .await;


    tracing::info!("Phase 363+364+365 schema migrated");

    // v22.0 Phase 177-02: Add seq_num column to config_audit_log
    let _ = sqlx::query("ALTER TABLE config_audit_log ADD COLUMN seq_num INTEGER")
        .execute(pool)
        .await;

    Ok(())
}
