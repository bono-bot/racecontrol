//! Database migrations: staff domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_staff(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Staff gamification opt-in (v14.0 Phase 95) ────────────────────────
    let _ = sqlx::query("ALTER TABLE staff_members ADD COLUMN gamification_opt_in INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // Staff kudos table (v14.0 Phase 95)
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_kudos (
            id TEXT PRIMARY KEY,
            sender_id TEXT NOT NULL REFERENCES staff_members(id),
            receiver_id TEXT NOT NULL REFERENCES staff_members(id),
            message TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT 'teamwork'
                CHECK(category IN ('teamwork', 'service', 'initiative')),
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await;


    // Staff earned badges (v14.0 Phase 95)
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_earned_badges (
            id TEXT PRIMARY KEY,
            staff_id TEXT NOT NULL REFERENCES staff_members(id),
            badge_id TEXT NOT NULL REFERENCES staff_badges(id),
            earned_at TEXT DEFAULT (datetime('now')),
            UNIQUE(staff_id, badge_id)
        )"
    )
    .execute(pool)
    .await;


    // ─── HR & Hiring Psychology (v14.0 Phase 96) ─────────────────────────────

    // Hiring SJT scenarios
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS hiring_sjts (
            id TEXT PRIMARY KEY,
            scenario_text TEXT NOT NULL,
            options_json TEXT NOT NULL,
            scoring_json TEXT NOT NULL,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await;


    // Job preview content for hiring bot
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS job_preview (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            content TEXT NOT NULL,
            media_url TEXT,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await;


    // Seed 3 SJT scenarios
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO hiring_sjts (id, scenario_text, options_json, scoring_json) VALUES
         ('sjt_01', 'A customer is upset because their session ended 5 minutes early due to a technical glitch during peak hours. They are demanding a full refund and becoming increasingly loud. What do you do?',
          '[\"Immediately offer a full refund to calm them down\",\"Apologize, explain the issue, and offer a complimentary 15-minute extension on their next visit\",\"Tell them to wait while you get the manager\",\"Explain that technical issues happen and offer no compensation\"]',
          '[2,4,3,1]'),
         ('sjt_02', 'You notice a child (around 8 years old) wandering near the sim pods unsupervised. The venue is busy and you cannot immediately identify the parent. What do you do?',
          '[\"Ignore it — not your responsibility\",\"Ask the child where their parent is and escort them to the waiting area while locating the guardian\",\"Tell the child to go sit down\",\"Wait to see if a parent shows up\"]',
          '[1,4,2,1]'),
         ('sjt_03', 'Mid-session, a customer reports their steering wheel has stopped responding. Their billing timer is running. What do you do?',
          '[\"Tell them to restart the game\",\"Immediately pause their billing, apologize, and either fix the issue or move them to another pod\",\"Ask them to finish on the broken pod and offer a discount later\",\"Say you will look into it after the current rush\"]',
          '[2,4,1,1]')"
    )
    .execute(pool)
    .await;


    // Seed 3 job preview items
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO job_preview (id, title, content, sort_order) VALUES
         ('jp_01', 'A Typical Day', 'You will greet customers, set up racing simulators, manage billing sessions, and ensure every driver has an amazing experience. Expect busy weekend rushes and quieter weekday mornings.', 1),
         ('jp_02', 'What We Look For', 'Enthusiasm for motorsport (not required but a plus!), great people skills, ability to stay calm under pressure, and comfort with technology. No prior sim racing experience needed — we will train you.', 2),
         ('jp_03', 'Perks & Culture', 'Free sim racing during off-hours, team challenges and kudos system, flexible scheduling, and a passionate team that loves what they do. We are a small team — your contributions matter.', 3)"
    )
    .execute(pool)
    .await;


    // ─── Staff members ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_members (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            phone TEXT NOT NULL UNIQUE,
            pin TEXT NOT NULL UNIQUE,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            last_login_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    // Add updated_at column for sync incremental change detection
    let _ = sqlx::query("ALTER TABLE staff_members ADD COLUMN updated_at TEXT DEFAULT (datetime('now'))")
        .execute(pool).await;

    // Backfill: set updated_at = created_at for existing rows
    let _ = sqlx::query("UPDATE staff_members SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool).await;


    // Add role column for RBAC (matches bot-side Phase 3 migration)
    let _ = sqlx::query("ALTER TABLE staff_members ADD COLUMN role TEXT DEFAULT 'staff'")
        .execute(pool).await;


    // Action queue — cloud queues actions for venue to pick up
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS action_queue (
            id TEXT PRIMARY KEY,
            action_type TEXT NOT NULL,
            payload TEXT NOT NULL DEFAULT '{}',
            status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending','processing','completed','failed')),
            error TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            processed_at TEXT,
            acked_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    // ─── M7-SEC: Persistent admin lockout state ────────────────────────────
    // Previously: in-memory only, reset on server restart.
    // Now: stored in DB so lockout survives restarts.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS admin_lockout (
            ip TEXT PRIMARY KEY,
            failed_attempts INTEGER NOT NULL DEFAULT 0,
            locked_until TEXT,
            last_attempt TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;


    // ─── Phase 348: Staff login attempt audit trail + per-staff-id lockout ──────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_login_attempts (
            id TEXT PRIMARY KEY,
            source_ip TEXT NOT NULL,
            staff_id TEXT,
            success INTEGER NOT NULL DEFAULT 0,
            attempted_at TEXT NOT NULL DEFAULT (datetime('now'))
        )"
    ).execute(pool).await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_staff_login_attempts_staff_id ON staff_login_attempts(staff_id, attempted_at)")
        .execute(pool).await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_staff_login_attempts_ip ON staff_login_attempts(source_ip, attempted_at)")
        .execute(pool).await?;


    // ─── Phase 260: Notification Outbox (UX-01, UX-02) ─────────────────────
    // Durable outbox for WhatsApp/SMS/screen notifications with retry, exponential
    // backoff, and OTP fallback chain. Worker polls every 10s.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS notification_outbox (
            id TEXT PRIMARY KEY,
            recipient TEXT NOT NULL,
            channel TEXT NOT NULL CHECK(channel IN ('whatsapp','sms','screen')),
            payload TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK(status IN ('pending','sent','delivered','failed','exhausted')),
            retry_count INTEGER NOT NULL DEFAULT 0,
            max_retries INTEGER NOT NULL DEFAULT 3,
            next_retry_at TEXT DEFAULT (datetime('now')),
            fallback_channel TEXT,
            fallback_token TEXT,
            context_type TEXT,
            context_id TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;


    let _ = sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_notif_outbox_pending
         ON notification_outbox(status, next_retry_at)"
    )
    .execute(pool)
    .await;

    // ─── Phase 391: Digital Staff Operations — Checklists ────────────────────

    // Checklist templates (morning opening, evening closing, etc.)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_checklists (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            checklist_type TEXT NOT NULL CHECK(checklist_type IN ('opening', 'closing', 'maintenance', 'custom')),
            items_json TEXT NOT NULL DEFAULT '[]',
            is_active BOOLEAN DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await?;

    // Checklist completion records
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_checklist_completions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            checklist_id TEXT NOT NULL REFERENCES staff_checklists(id),
            staff_id TEXT NOT NULL,
            staff_name TEXT,
            completed_items_json TEXT NOT NULL DEFAULT '[]',
            total_items INTEGER NOT NULL DEFAULT 0,
            completed_count INTEGER NOT NULL DEFAULT 0,
            notes TEXT,
            started_at TEXT DEFAULT (datetime('now')),
            completed_at TEXT,
            date TEXT NOT NULL DEFAULT (date('now'))
        )"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_checklist_completions_date
         ON staff_checklist_completions(date, checklist_id)"
    )
    .execute(pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_checklist_completions_staff
         ON staff_checklist_completions(staff_id, date)"
    )
    .execute(pool)
    .await?;

    // Seed default checklists
    sqlx::query(
        "INSERT OR IGNORE INTO staff_checklists (id, name, checklist_type, items_json) VALUES
         ('chk_opening', 'Morning Opening', 'opening', '[
            {\"id\": \"o1\", \"label\": \"Turn on all 8 pods and monitors\", \"category\": \"hardware\"},
            {\"id\": \"o2\", \"label\": \"Check AC and lighting in venue\", \"category\": \"venue\"},
            {\"id\": \"o3\", \"label\": \"Verify all pods show RaceControl lock screen\", \"category\": \"software\"},
            {\"id\": \"o4\", \"label\": \"Check wheelbase and pedal connections on each pod\", \"category\": \"hardware\"},
            {\"id\": \"o5\", \"label\": \"Wipe down steering wheels and seats\", \"category\": \"cleaning\"},
            {\"id\": \"o6\", \"label\": \"Check cafe stock and coffee machine\", \"category\": \"cafe\"},
            {\"id\": \"o7\", \"label\": \"Open POS dashboard and verify connection\", \"category\": \"software\"},
            {\"id\": \"o8\", \"label\": \"Check printer paper and ink\", \"category\": \"supplies\"}
         ]'),
         ('chk_closing', 'Evening Closing', 'closing', '[
            {\"id\": \"c1\", \"label\": \"End all active billing sessions\", \"category\": \"billing\"},
            {\"id\": \"c2\", \"label\": \"Shut down all 8 pods\", \"category\": \"hardware\"},
            {\"id\": \"c3\", \"label\": \"Close cafe register and count cash\", \"category\": \"cafe\"},
            {\"id\": \"c4\", \"label\": \"Wipe down all steering wheels and seats\", \"category\": \"cleaning\"},
            {\"id\": \"c5\", \"label\": \"Turn off AC, fans, and unnecessary lights\", \"category\": \"venue\"},
            {\"id\": \"c6\", \"label\": \"Lock up venue and set alarm\", \"category\": \"security\"},
            {\"id\": \"c7\", \"label\": \"Report any hardware issues noticed today\", \"category\": \"maintenance\"}
         ]')"
    )
    .execute(pool)
    .await?;

    Ok(())
}
