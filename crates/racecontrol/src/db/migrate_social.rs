//! Database migrations: social domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_social(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Acts 1-4: Customer visits (groups sessions + cafe into one visit) ─────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS visits (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            started_at TEXT DEFAULT (datetime('now')),
            ended_at TEXT,
            status TEXT NOT NULL DEFAULT 'open',
            end_method TEXT,
            total_sessions INTEGER DEFAULT 0,
            total_spent_paise INTEGER DEFAULT 0,
            receipt_sent BOOLEAN DEFAULT 0,
            venue_id TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_visits_driver ON visits(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_visits_status ON visits(status)")
        .execute(pool)
        .await?;


    // ─── FSM-07: Split session entitlement table ──────────────────────────────
    // Each child split is an immutable record with allocated_seconds and status.
    // parent_session_id references billing_sessions(id). UNIQUE on (parent, split_number)
    // prevents duplicate splits from being created even under concurrent inserts.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS split_sessions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            parent_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            split_number INTEGER NOT NULL,
            allocated_seconds INTEGER NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(parent_session_id, split_number)
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_split_sessions_parent ON split_sessions(parent_session_id)"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_split_sessions_status ON split_sessions(parent_session_id, status)"
    )
    .execute(pool)
    .await?;


    // ─── Game launcher tables ─────────────────────────────────────────────────

    // ─── Customer sessions (PWA JWT tracking) ───────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS customer_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            token_hash TEXT NOT NULL,
            device_info TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            revoked_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    // Customer session indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_customer_sessions_driver ON customer_sessions(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_customer_sessions_token ON customer_sessions(token_hash)")
        .execute(pool)
        .await?;


    // ─── Session feedback ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_feedback (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            rating INTEGER NOT NULL CHECK(rating BETWEEN 1 AND 5),
            comment TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pod_reservations (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            pod_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK(status IN ('active','completed','expired','cancelled')),
            created_at TEXT DEFAULT (datetime('now')),
            ended_at TEXT,
            last_activity_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pod_res_driver ON pod_reservations(driver_id, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pod_res_pod ON pod_reservations(pod_id, status)")
        .execute(pool)
        .await?;


    // ─── Friends & Social ────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS friend_requests (
            id TEXT PRIMARY KEY,
            sender_id TEXT NOT NULL,
            receiver_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending',
            created_at TEXT DEFAULT (datetime('now')),
            resolved_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_sender ON friend_requests(sender_id, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friend_requests_receiver ON friend_requests(receiver_id, status)")
        .execute(pool)
        .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS friendships (
            id TEXT PRIMARY KEY,
            driver_a_id TEXT NOT NULL,
            driver_b_id TEXT NOT NULL,
            request_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            UNIQUE(driver_a_id, driver_b_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_a ON friendships(driver_a_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_friendships_b ON friendships(driver_b_id)")
        .execute(pool)
        .await?;


    // ─── Multiplayer Group Sessions ───────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_sessions (
            id TEXT PRIMARY KEY,
            host_driver_id TEXT NOT NULL,
            experience_id TEXT NOT NULL,
            pricing_tier_id TEXT NOT NULL,
            shared_pin TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'forming',
            ac_session_id TEXT,
            total_members INTEGER NOT NULL DEFAULT 1,
            validated_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            started_at TEXT,
            completed_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_sessions_host ON group_sessions(host_driver_id, status)")
        .execute(pool)
        .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS group_session_members (
            id TEXT PRIMARY KEY,
            group_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'invitee',
            status TEXT NOT NULL DEFAULT 'pending',
            pod_id TEXT,
            reservation_id TEXT,
            auth_token_id TEXT,
            billing_session_id TEXT,
            wallet_txn_id TEXT,
            invited_at TEXT DEFAULT (datetime('now')),
            accepted_at TEXT,
            validated_at TEXT,
            UNIQUE(group_session_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_session_members_driver ON group_session_members(driver_id, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_group_session_members_group ON group_session_members(group_session_id)")
        .execute(pool)
        .await?;


    // ─── AI messaging table (Bono ↔ James) ───────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS ai_messages (
            id TEXT PRIMARY KEY,
            sender TEXT NOT NULL,
            recipient TEXT NOT NULL,
            content TEXT NOT NULL,
            message_type TEXT NOT NULL DEFAULT 'text',
            metadata TEXT,
            channel TEXT NOT NULL DEFAULT 'http',
            status TEXT NOT NULL DEFAULT 'pending',
            in_reply_to TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            delivered_at TEXT,
            read_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_msg_recipient_status ON ai_messages(recipient, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_ai_msg_created ON ai_messages(created_at)")
        .execute(pool)
        .await?;


    // ─── Smart Scheduler events table ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS scheduler_events (
            id TEXT PRIMARY KEY,
            event_type TEXT NOT NULL,
            pod_id TEXT,
            details TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_scheduler_events_type ON scheduler_events(event_type)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_scheduler_events_created ON scheduler_events(created_at)")
        .execute(pool)
        .await?;


    // ─── Referral system ─────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS referrals (
            id TEXT PRIMARY KEY,
            referrer_id TEXT NOT NULL,
            referee_id TEXT,
            code TEXT NOT NULL UNIQUE,
            reward_credited INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            redeemed_at TEXT
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_referrals_code ON referrals(code)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_referrals_referrer ON referrals(referrer_id)")
        .execute(pool)
        .await?;


    // ─── Multiplayer enrichment columns (Phase 09) ─────────────────────────
    // Store track/car/ai_count on group_sessions for lobby UI enrichment
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN track TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN car TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN ai_count INTEGER")
        .execute(pool)
        .await;


    // ─── v22.0 Phase 177: Feature Flags Registry ─────────────────────────────

    // Phase 14: Link group sessions to hotlap events for F1 scoring (GRP-01)
    let _ = sqlx::query("ALTER TABLE group_sessions ADD COLUMN hotlap_event_id TEXT REFERENCES hotlap_events(id)")
        .execute(pool)
        .await;

    // ─── Remote Reservations (cloud booking) ────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS reservations (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            experience_id TEXT NOT NULL,
            pin TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending_debit'
                CHECK(status IN ('pending_debit','confirmed','redeemed','expired','cancelled','failed')),
            pod_number INTEGER,
            debit_intent_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            redeemed_at TEXT,
            cancelled_at TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_pin ON reservations(pin, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_driver ON reservations(driver_id, status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_reservations_expires ON reservations(expires_at, status)")
        .execute(pool)
        .await?;


    // ─── Phase 260 UX-08: Virtual queue for walk-in customers ───────────────
    // Customers join the queue via PWA/kiosk, see their position and ETA.
    // Staff call the next customer and mark them seated.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS virtual_queue (
            id TEXT PRIMARY KEY,
            driver_id TEXT,
            driver_name TEXT,
            phone TEXT,
            party_size INTEGER NOT NULL DEFAULT 1,
            status TEXT NOT NULL DEFAULT 'waiting'
                CHECK(status IN ('waiting','called','seated','left','expired')),
            position INTEGER,
            estimated_wait_minutes INTEGER,
            joined_at TEXT DEFAULT (datetime('now')),
            called_at TEXT,
            seated_at TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;


    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_queue_status ON virtual_queue(status, joined_at)"
    )
    .execute(pool)
    .await;


    // v31.0 Phase 270: Fleet Healer incident_log table
    crate::fleet_healer::AuditTrail::migrate(pool).await?;

    Ok(())
}
