//! Database migrations: gamification domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_gamification(pool: &SqlitePool) -> anyhow::Result<()> {
    // 1. hotlap_events
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS hotlap_events (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            car_class TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
            status TEXT NOT NULL DEFAULT 'upcoming'
                CHECK(status IN ('upcoming', 'active', 'scoring', 'completed', 'cancelled')),
            starts_at TEXT,
            ends_at TEXT,
            rule_107_percent INTEGER DEFAULT 1,
            reference_time_ms INTEGER,
            max_valid_laps INTEGER,
            championship_id TEXT REFERENCES championships(id),
            created_by TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // 2. hotlap_event_entries
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS hotlap_event_entries (
            id TEXT PRIMARY KEY,
            event_id TEXT NOT NULL REFERENCES hotlap_events(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            lap_id TEXT REFERENCES laps(id),
            lap_time_ms INTEGER,
            sector1_ms INTEGER,
            sector2_ms INTEGER,
            sector3_ms INTEGER,
            position INTEGER,
            points INTEGER DEFAULT 0,
            badge TEXT,
            gap_to_leader_ms INTEGER,
            within_107_percent INTEGER DEFAULT 1,
            result_status TEXT DEFAULT 'pending'
                CHECK(result_status IN ('pending', 'finished', 'dns', 'dnf')),
            entered_at TEXT DEFAULT (datetime('now')),
            UNIQUE(event_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;


    // 3. championships
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championships (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            season TEXT,
            car_class TEXT NOT NULL,
            sim_type TEXT NOT NULL DEFAULT 'assetto_corsa',
            status TEXT NOT NULL DEFAULT 'upcoming'
                CHECK(status IN ('upcoming', 'active', 'completed')),
            scoring_system TEXT NOT NULL DEFAULT 'f1_2010',
            total_rounds INTEGER DEFAULT 0,
            completed_rounds INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // 4. championship_rounds
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championship_rounds (
            championship_id TEXT NOT NULL REFERENCES championships(id),
            event_id TEXT NOT NULL REFERENCES hotlap_events(id),
            round_number INTEGER NOT NULL,
            PRIMARY KEY (championship_id, event_id)
        )",
    )
    .execute(pool)
    .await?;


    // 5. championship_standings
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS championship_standings (
            championship_id TEXT NOT NULL REFERENCES championships(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            position INTEGER,
            total_points INTEGER DEFAULT 0,
            rounds_entered INTEGER DEFAULT 0,
            best_result INTEGER,
            wins INTEGER DEFAULT 0,
            podiums INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now')),
            PRIMARY KEY (championship_id, driver_id)
        )",
    )
    .execute(pool)
    .await?;


    // 6. driver_ratings (Phase 253: skill rating system)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driver_ratings (
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            sim_type TEXT NOT NULL DEFAULT 'assettocorsa',
            composite_rating REAL NOT NULL DEFAULT 0.0,
            rating_class TEXT NOT NULL DEFAULT 'Unrated',
            pace_score REAL NOT NULL DEFAULT 0.0,
            consistency_score REAL NOT NULL DEFAULT 0.0,
            experience_score REAL NOT NULL DEFAULT 0.0,
            total_laps INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now')),
            PRIMARY KEY (driver_id, sim_type)
        )",
    )
    .execute(pool)
    .await?;

    // Idempotent ALTER for existing DBs that have old schema (single PK on driver_id)
    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN sim_type TEXT NOT NULL DEFAULT 'assettocorsa'").execute(pool).await;

    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN composite_rating REAL NOT NULL DEFAULT 0.0").execute(pool).await;

    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN pace_score REAL NOT NULL DEFAULT 0.0").execute(pool).await;

    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN consistency_score REAL NOT NULL DEFAULT 0.0").execute(pool).await;

    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN experience_score REAL NOT NULL DEFAULT 0.0").execute(pool).await;

    let _ = sqlx::query("ALTER TABLE driver_ratings ADD COLUMN total_laps INTEGER NOT NULL DEFAULT 0").execute(pool).await;


    // Indexes for new competitive tables
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_events_status ON hotlap_events(status, track)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_events_updated ON hotlap_events(updated_at)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_entries_event ON hotlap_event_entries(event_id, position)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_hotlap_entries_driver ON hotlap_event_entries(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_championships_updated ON championships(updated_at)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_champ_rounds_champ ON championship_rounds(championship_id, round_number)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_champ_standings_champ ON championship_standings(championship_id, position)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_driver_ratings_class ON driver_ratings(rating_class, composite_rating)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_driver_ratings_driver ON driver_ratings(driver_id)")
        .execute(pool)
        .await?;


    // Phase 14: Championship tiebreaker counts (CHP-04)
    let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p2_count INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE championship_standings ADD COLUMN p3_count INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // Table 1: achievements (badge/achievement definitions with JSON criteria)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS achievements (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            category TEXT NOT NULL DEFAULT 'general'
                CHECK(category IN ('milestone', 'skill', 'dedication', 'social', 'special')),
            criteria_json TEXT NOT NULL,
            badge_icon TEXT,
            reward_credits_paise INTEGER DEFAULT 0,
            sort_order INTEGER DEFAULT 0,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed initial badge definitions (psychology foundation)
    sqlx::query(
        "INSERT OR IGNORE INTO achievements (id, name, description, category, criteria_json, badge_icon, reward_credits_paise, sort_order) VALUES
         ('badge_first_lap', 'First Lap', 'Completed your very first lap at RacingPoint', 'milestone', '{\"type\":\"first_lap\",\"operator\":\">=\",\"value\":1}', 'flag', 0, 1),
         ('badge_10_tracks', 'Explorer', 'Driven on 10 different tracks', 'milestone', '{\"type\":\"unique_tracks\",\"operator\":\">=\",\"value\":10}', 'map', 0, 2),
         ('badge_100_laps', 'Century', 'Completed 100 laps at RacingPoint', 'dedication', '{\"type\":\"total_laps\",\"operator\":\">=\",\"value\":100}', 'trophy', 0, 3),
         ('badge_10_cars', 'Collector', 'Driven 10 different cars', 'milestone', '{\"type\":\"unique_cars\",\"operator\":\">=\",\"value\":10}', 'car', 0, 4),
         ('badge_streak_4', 'Regular', 'Maintained a 4-week visit streak', 'dedication', '{\"type\":\"streak_weeks\",\"operator\":\">=\",\"value\":4}', 'fire', 0, 5)"
    )
    .execute(pool)
    .await?;


    // Table 2: driver_achievements (which drivers earned which badges)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driver_achievements (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            achievement_id TEXT NOT NULL REFERENCES achievements(id),
            earned_at TEXT DEFAULT (datetime('now')),
            notified INTEGER DEFAULT 0,
            UNIQUE(driver_id, achievement_id)
        )",
    )
    .execute(pool)
    .await?;


    // Table 3: streaks (weekly visit streak tracking per driver)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS streaks (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL UNIQUE REFERENCES drivers(id),
            current_streak INTEGER NOT NULL DEFAULT 0,
            longest_streak INTEGER NOT NULL DEFAULT 0,
            last_visit_date TEXT,
            grace_expires_date TEXT,
            streak_started_at TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Table 4: driving_passport (track/car completion progress per driver)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS driving_passport (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            track TEXT NOT NULL,
            car TEXT NOT NULL,
            first_driven_at TEXT DEFAULT (datetime('now')),
            best_lap_ms INTEGER,
            lap_count INTEGER DEFAULT 1,
            UNIQUE(driver_id, track, car)
        )",
    )
    .execute(pool)
    .await?;


    // Table 6: staff_badges (staff skill badges)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_badges (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            criteria_json TEXT NOT NULL,
            badge_icon TEXT,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Table 7: staff_challenges (team challenges with collective goals)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS staff_challenges (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            goal_type TEXT NOT NULL,
            goal_target INTEGER NOT NULL,
            reward_description TEXT,
            start_date TEXT NOT NULL,
            end_date TEXT NOT NULL,
            current_progress INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK(status IN ('active', 'completed', 'expired')),
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed staff badges (v14.0 Phase 95)
    let _ = sqlx::query(
        "INSERT OR IGNORE INTO staff_badges (id, name, description, criteria_json, badge_icon) VALUES
         ('sbadge_first_shift', 'First Shift', 'Hosted your first racing session', '{\"type\":\"sessions_hosted\",\"operator\":\">=\",\"value\":1}', 'play'),
         ('sbadge_event_host', 'Event Host', 'Created and ran a hotlap event', '{\"type\":\"events_created\",\"operator\":\">=\",\"value\":1}', 'calendar'),
         ('sbadge_streak_4w', 'Streak 4 Weeks', 'Worked 4 consecutive weeks', '{\"type\":\"work_streak_weeks\",\"operator\":\">=\",\"value\":4}', 'flame'),
         ('sbadge_pod_master', 'Pod Master', 'Hosted 100 racing sessions', '{\"type\":\"sessions_hosted\",\"operator\":\">=\",\"value\":100}', 'crown'),
         ('sbadge_team_player', 'Team Player', 'Received 10 kudos from colleagues', '{\"type\":\"kudos_received\",\"operator\":\">=\",\"value\":10}', 'heart')"
    )
    .execute(pool)
    .await;


    // Psychology Foundation indexes
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_driver_achievements_driver ON driver_achievements(driver_id)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_driver_achievements_achievement ON driver_achievements(achievement_id)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_streaks_driver ON streaks(driver_id)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_driving_passport_driver ON driving_passport(driver_id)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_driving_passport_track ON driving_passport(track, car)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_staff_challenges_status ON staff_challenges(status)",
    )
    .execute(pool)
    .await?;


    // Phase 92: variable_reward_log — audit trail for RET-06 monthly cap enforcement
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS variable_reward_log (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            trigger TEXT NOT NULL CHECK(trigger IN ('pb', 'milestone')),
            month TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_variable_reward_driver_month
            ON variable_reward_log(driver_id, month)",
    )
    .execute(pool)
    .await?;

    Ok(())
}
