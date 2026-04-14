//! Database migrations: marketing domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_marketing(pool: &SqlitePool) -> anyhow::Result<()> {
    // Cialdini campaign templates
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS campaign_templates (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            cialdini_principle TEXT NOT NULL,
            message_template TEXT NOT NULL,
            target_segment TEXT DEFAULT 'all',
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await;


    // Nudge templates (loss-framed review nudges, peak-end timing)
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS nudge_templates (
            id TEXT PRIMARY KEY,
            template_type TEXT NOT NULL CHECK(template_type IN ('review', 'winback', 'milestone')),
            copy_text TEXT NOT NULL,
            timing_rules_json TEXT DEFAULT '{}',
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )"
    )
    .execute(pool)
    .await;


    // Seed 3 Cialdini campaign templates
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO campaign_templates (id, name, cialdini_principle, message_template, target_segment) VALUES
         ('camp_social_proof', 'Weekly Social Proof', 'social_proof',
          'Hey {name}! {drivers_this_week} drivers raced at RacingPoint this week. Join them this weekend and see if you can beat the track record!',
          'all'),
         ('camp_scarcity', 'Weekend Scarcity', 'scarcity',
          'Hi {name}, heads up — only {available_slots} slots left for this Saturday! Book now before they fill up. Walk-ins welcome but slots go fast.',
          'active'),
         ('camp_commitment', 'Commitment Ladder', 'commitment',
          'Hey {name}, you have completed {session_count} sessions! You are just {remaining} away from unlocking member benefits. Your next session could be the one!',
          'returning')"
    )
    .execute(pool)
    .await;


    // Seed loss-framed nudge templates
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO nudge_templates (id, template_type, copy_text, timing_rules_json) VALUES
         ('nudge_review_loss', 'review', 'Your RacingPoint experience is fresh — share it now before the details fade! Drivers who review within 24 hours help us improve the most.', '{\"delay_hours\":2,\"window_hours\":24}'),
         ('nudge_winback', 'winback', 'We miss you at RacingPoint! It has been {days_since} days since your last session. Your personal best of {pb_time} is still on the board — come defend it before someone beats it!', '{\"trigger_days\":14}'),
         ('nudge_milestone', 'milestone', 'You just hit {milestone_name}! Do not lose momentum — your next session keeps the streak alive. Come back this week and keep climbing.', '{\"trigger\":\"post_milestone\"}')"
    )
    .execute(pool)
    .await;


    // ─── Coupons & Promo Codes ───────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS coupons (
            id TEXT PRIMARY KEY,
            code TEXT NOT NULL UNIQUE,
            coupon_type TEXT NOT NULL DEFAULT 'flat' CHECK(coupon_type IN ('percent', 'flat', 'free_minutes')),
            value INTEGER NOT NULL,
            max_uses INTEGER,
            used_count INTEGER DEFAULT 0,
            valid_from TEXT,
            valid_until TEXT,
            min_spend_paise INTEGER DEFAULT 0,
            first_session_only INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_coupons_code ON coupons(code)")
        .execute(pool)
        .await?;


    // ─── Coupon redemptions ──────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS coupon_redemptions (
            id TEXT PRIMARY KEY,
            coupon_id TEXT NOT NULL REFERENCES coupons(id),
            driver_id TEXT NOT NULL,
            billing_session_id TEXT,
            discount_paise INTEGER NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_coupon_redemptions_driver ON coupon_redemptions(driver_id)")
        .execute(pool)
        .await?;


    // ─── FATM-08: Coupon lifecycle FSM columns (ALTER migration) ────────────
    // Add coupon_status, reserved_at, reserved_for_session if not present.
    // Wrapped in existence checks so re-running init is idempotent.
    let has_coupon_status: bool = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM pragma_table_info('coupons') WHERE name = 'coupon_status'",
    )
    .fetch_one(pool)
    .await
    .map(|r| r.0 > 0)
    .unwrap_or(false);
    if !has_coupon_status {
        let _ = sqlx::query(
            "ALTER TABLE coupons ADD COLUMN coupon_status TEXT NOT NULL DEFAULT 'available'",
        )
        .execute(pool)
        .await;

    }

    let has_reserved_at: bool = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM pragma_table_info('coupons') WHERE name = 'reserved_at'",
    )
    .fetch_one(pool)
    .await
    .map(|r| r.0 > 0)
    .unwrap_or(false);
    if !has_reserved_at {
        let _ = sqlx::query("ALTER TABLE coupons ADD COLUMN reserved_at TEXT")
            .execute(pool)
            .await;

    }

    let has_reserved_for_session: bool = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM pragma_table_info('coupons') WHERE name = 'reserved_for_session'",
    )
    .fetch_one(pool)
    .await
    .map(|r| r.0 > 0)
    .unwrap_or(false);
    if !has_reserved_for_session {
        let _ = sqlx::query("ALTER TABLE coupons ADD COLUMN reserved_for_session TEXT")
            .execute(pool)
            .await;

    }

    // ─── Dynamic Pricing Rules ───────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pricing_rules (
            id TEXT PRIMARY KEY,
            rule_name TEXT NOT NULL,
            rule_type TEXT NOT NULL CHECK(rule_type IN ('peak', 'off_peak', 'group', 'custom')),
            day_of_week TEXT,
            hour_start INTEGER,
            hour_end INTEGER,
            multiplier REAL DEFAULT 1.0,
            flat_adjustment_paise INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed default pricing rules
    sqlx::query(
        "INSERT OR IGNORE INTO pricing_rules (id, rule_name, rule_type, day_of_week, hour_start, hour_end, multiplier)
         VALUES
            ('rule_weekday_offpeak', 'Weekday Off-Peak', 'off_peak', '1,2,3,4,5', 10, 15, 0.78),
            ('rule_weekend_peak', 'Weekend Peak', 'peak', '0,6', 0, 24, 1.22),
            ('rule_group_4plus', 'Group 4+', 'group', NULL, NULL, NULL, 0.89)"
    )
    .execute(pool)
    .await?;


    // ─── Packages (occasion-based) ───────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS packages (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            num_rigs INTEGER NOT NULL,
            duration_minutes INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            includes_cafe INTEGER DEFAULT 0,
            cafe_budget_paise INTEGER DEFAULT 0,
            day_restriction TEXT,
            hour_start INTEGER,
            hour_end INTEGER,
            is_active INTEGER DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Migration: ensure hour_start/hour_end columns exist (older DBs may have hour_restriction instead)
    let _ = sqlx::query("ALTER TABLE packages ADD COLUMN hour_start INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE packages ADD COLUMN hour_end INTEGER")
        .execute(pool)
        .await;


    // Seed default packages
    sqlx::query(
        "INSERT OR IGNORE INTO packages (id, name, description, num_rigs, duration_minutes, price_paise, includes_cafe, cafe_budget_paise, sort_order)
         VALUES
            ('pkg_date', 'Date Night', '2 rigs + 2 drinks from cafe', 2, 60, 180000, 1, 20000, 1),
            ('pkg_squad', 'Squad (4 Friends)', '4 rigs, group discount applied', 4, 60, 320000, 0, 0, 2),
            ('pkg_birthday', 'Birthday Bash', '6 rigs + cake + drinks for 2 hours', 6, 120, 800000, 1, 100000, 3),
            ('pkg_corporate', 'Corporate Team Building', '8 rigs + tournament + lunch for 2 hours', 8, 120, 1500000, 1, 200000, 4),
            ('pkg_student', 'Student Special', '1 rig, weekday 10am-3pm only', 1, 60, 60000, 0, 0, 5)"
    )
    .execute(pool)
    .await?;


    // ─── Membership tiers & subscriptions ────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS membership_tiers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            hours_included INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            perks TEXT,
            is_active INTEGER DEFAULT 1,
            sort_order INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "INSERT OR IGNORE INTO membership_tiers (id, name, hours_included, price_paise, perks, sort_order)
         VALUES
            ('mem_rookie', 'Rookie', 4, 300000, '{\"priority_booking\":true}', 1),
            ('mem_pro', 'Pro', 8, 500000, '{\"priority_booking\":true,\"league_entry\":true,\"telemetry_coaching\":true}', 2),
            ('mem_champion', 'Champion', 0, 800000, '{\"unlimited_offpeak\":true,\"all_leagues\":true,\"merch\":true}', 3)"
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS memberships (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            tier_id TEXT NOT NULL REFERENCES membership_tiers(id),
            hours_used_minutes INTEGER DEFAULT 0,
            price_paise INTEGER NOT NULL,
            started_at TEXT NOT NULL DEFAULT (datetime('now')),
            expires_at TEXT NOT NULL,
            auto_renew INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active' CHECK(status IN ('active', 'expired', 'cancelled')),
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_memberships_driver ON memberships(driver_id, status)")
        .execute(pool)
        .await?;


    // ─── Session highlights (clip URLs) ──────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS session_highlights (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL,
            clip_type TEXT DEFAULT 'best_lap',
            file_path TEXT,
            cloud_url TEXT,
            duration_secs INTEGER,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_highlights_session ON session_highlights(billing_session_id)")
        .execute(pool)
        .await?;


    // ─── Time trials (weekly challenges) ─────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS time_trials (
            id TEXT PRIMARY KEY,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            week_start TEXT NOT NULL,
            week_end TEXT NOT NULL,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // ─── Google review tracking ──────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS review_nudges (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL,
            billing_session_id TEXT NOT NULL,
            sent_at TEXT DEFAULT (datetime('now')),
            review_credited INTEGER DEFAULT 0
        )",
    )
    .execute(pool)
    .await?;


    // ─── Tournaments ─────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournaments (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            format TEXT NOT NULL DEFAULT 'time_attack' CHECK(format IN ('time_attack', 'bracket', 'round_robin')),
            max_participants INTEGER DEFAULT 16,
            entry_fee_paise INTEGER DEFAULT 0,
            prize_pool_paise INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'upcoming' CHECK(status IN ('upcoming', 'registration', 'in_progress', 'completed', 'cancelled')),
            registration_start TEXT,
            registration_end TEXT,
            event_date TEXT,
            rules TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournament_registrations (
            id TEXT PRIMARY KEY,
            tournament_id TEXT NOT NULL REFERENCES tournaments(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            seed INTEGER,
            status TEXT DEFAULT 'registered' CHECK(status IN ('registered', 'checked_in', 'eliminated', 'winner')),
            best_time_ms INTEGER,
            created_at TEXT DEFAULT (datetime('now')),
            UNIQUE(tournament_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_tourney_reg ON tournament_registrations(tournament_id, driver_id)")
        .execute(pool)
        .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS tournament_matches (
            id TEXT PRIMARY KEY,
            tournament_id TEXT NOT NULL REFERENCES tournaments(id),
            round INTEGER NOT NULL,
            match_number INTEGER NOT NULL,
            driver_a TEXT REFERENCES drivers(id),
            driver_b TEXT REFERENCES drivers(id),
            time_a_ms INTEGER,
            time_b_ms INTEGER,
            winner_id TEXT REFERENCES drivers(id),
            status TEXT DEFAULT 'pending' CHECK(status IN ('pending', 'in_progress', 'completed')),
            completed_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    // ─── Bonus tiers table (configurable topup bonus percentages) ────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bonus_tiers (
            id TEXT PRIMARY KEY,
            min_amount_paise INTEGER NOT NULL,
            bonus_percent INTEGER NOT NULL,
            is_active INTEGER NOT NULL DEFAULT 1,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed default bonus tiers
    sqlx::query(
        "INSERT OR IGNORE INTO bonus_tiers (id, min_amount_paise, bonus_percent, sort_order)
         VALUES ('bt_2000', 200000, 10, 1), ('bt_4000', 400000, 20, 2)"
    )
    .execute(pool)
    .await?;


    // ─── Multiplayer Results ──────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS multiplayer_results (
            id TEXT PRIMARY KEY,
            group_session_id TEXT NOT NULL,
            ac_session_id TEXT,
            driver_id TEXT NOT NULL,
            position INTEGER NOT NULL,
            best_lap_ms INTEGER,
            total_time_ms INTEGER,
            laps_completed INTEGER DEFAULT 0,
            dnf INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_multiplayer_results_group ON multiplayer_results(group_session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_multiplayer_results_driver ON multiplayer_results(driver_id)")
        .execute(pool)
        .await?;


    // Table 5: nudge_queue (notification priority queue with channel routing)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS nudge_queue (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            channel TEXT NOT NULL CHECK(channel IN ('whatsapp', 'discord', 'pwa')),
            priority INTEGER NOT NULL DEFAULT 5,
            template TEXT NOT NULL,
            payload_json TEXT DEFAULT '{}',
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK(status IN ('pending', 'sent', 'failed', 'expired', 'throttled')),
            scheduled_at TEXT DEFAULT (datetime('now')),
            expires_at TEXT,
            sent_at TEXT,
            error_text TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_nudge_queue_status ON nudge_queue(status, priority, scheduled_at)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_nudge_queue_driver ON nudge_queue(driver_id, channel, sent_at)",
    )
    .execute(pool)
    .await?;


    Ok(())
}
