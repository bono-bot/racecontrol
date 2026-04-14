//! Database migrations: additional columns and indexes on core domain tables.
//!
//! Extracted from migrate_core.rs — contains ALTER TABLE additions, indexes,
//! and seed data that extend the base table schemas.

use sqlx::sqlite::SqlitePool;

pub(super) async fn migrate_core_columns(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Commitment ladder for pricing psychology (v14.0 Phase 94) ──────────
    let _ = sqlx::query(
        "ALTER TABLE drivers ADD COLUMN commitment_ladder TEXT DEFAULT 'trial' \
         CHECK(commitment_ladder IN ('trial', 'single', 'package', 'member'))"
    )
    .execute(pool)
    .await;


    // ─── Cloud sync tables ───────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS sync_state (
            table_name TEXT PRIMARY KEY,
            last_synced_at TEXT NOT NULL,
            last_sync_count INTEGER DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // sync_state conflict_count column (SYNC-05): track skipped writes due to LWW
    let _ = sqlx::query("ALTER TABLE sync_state ADD COLUMN conflict_count INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // Cross-domain migrations (updated_at, customer_id, employee flag) are in mod.rs

    // Add presence column to drivers (idempotent)
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN presence TEXT DEFAULT 'hidden'")
        .execute(pool)
        .await;


    // Seed default scheduler settings
    sqlx::query(
        "INSERT OR IGNORE INTO settings (key, value)
         VALUES
            ('scheduler_enabled', 'true'),
            ('scheduler_pre_wake_minutes', '15'),
            ('scheduler_pre_open_minutes', '10'),
            ('scheduler_post_close_minutes', '15')",
    )
    .execute(pool)
    .await?;


    // Add referral_code column to drivers
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN referral_code TEXT")
        .execute(pool)
        .await;


    // Nickname & leaderboard display preference
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN nickname TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN show_nickname_on_leaderboard BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;


    // Unique constraint on (name, dob) to prevent duplicate registrations
    let _ = sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_name_dob ON drivers(name, dob) WHERE registration_completed = 1")
        .execute(pool)
        .await;


    // ─── Unlimited trials flag for test/demo drivers ──────────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN unlimited_trials BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;


    // Seed test driver with unlimited trials for demos
    // Use UPSERT so existing driver always gets unlimited_trials=1 and has_used_trial reset
    let _ = sqlx::query(
        "INSERT INTO drivers (id, name, phone, has_used_trial, unlimited_trials, created_at, updated_at)
         VALUES ('driver_test_trial', 'Test Driver (Unlimited)', '0000000000', 0, 1, datetime('now'), datetime('now'))
         ON CONFLICT(id) DO UPDATE SET unlimited_trials = 1, has_used_trial = 0, updated_at = datetime('now')",
    )
    .execute(pool)
    .await;


    // DATA-01: Covering indexes for leaderboard queries
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_leaderboard ON laps(track, car, valid, lap_time_ms)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_driver_created ON laps(driver_id, created_at)")
        .execute(pool)
        .await?;


    // DATA-02: Covering index for telemetry visualization (do NOT drop idx_telemetry_lap)
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)")
        .execute(pool)
        .await?;


    // DATA-04: cloud_driver_id column on drivers for UUID mismatch resolution
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN cloud_driver_id TEXT")
        .execute(pool)
        .await;

    sqlx::query("CREATE UNIQUE INDEX IF NOT EXISTS idx_drivers_cloud_id ON drivers(cloud_driver_id)")
        .execute(pool)
        .await?;


    // DATA-05: Six new competitive tables

    // DATA-06: car_class column on laps for event auto-entry matching
    let _ = sqlx::query("ALTER TABLE laps ADD COLUMN car_class TEXT")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_laps_car_class ON laps(track, car_class)")
        .execute(pool)
        .await?;


    // LB-05: suspect column for lap validity hardening (leaderboard filtering)
    let _ = sqlx::query("ALTER TABLE laps ADD COLUMN suspect INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;


    // ─── Phase 79: PII encryption columns on drivers ──────────────────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN phone_hash TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN phone_enc TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN email_enc TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN name_enc TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_phone_hash TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN guardian_phone_enc TEXT")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_phone_hash ON drivers(phone_hash)")
        .execute(pool)
        .await?;


    // ─── LEGAL-08: Driver retention tracking columns ──────────────────────────
    // last_activity_at: updated on billing start and wallet topup — keeps active customers
    //   from being anonymized by the daily background job.
    // pii_anonymized / pii_anonymized_at: set when background job or revocation wipes PII.
    // consent_revoked / consent_revoked_at: set when guardian or driver invokes right of erasure.
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN last_activity_at TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN pii_anonymized BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN pii_anonymized_at TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN consent_revoked BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN consent_revoked_at TEXT")
        .execute(pool)
        .await;


    // Index for daily background job — queries inactive drivers by last_activity_at
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_drivers_last_activity ON drivers(last_activity_at)",
    )
    .execute(pool)
    .await?;


    // ─── Phase 260: Leaderboard integrity — lap assist evidence (UX-06, UX-07) ──
    // assist_config_hash: SHA-256 fingerprint of assist settings at time of lap.
    //   Provides immutable proof of what assists were active — leaderboard trust.
    // assist_tier: derived category — 'pro', 'semi-pro', 'amateur', 'unknown'.
    //   'pro' = TC+SC+ABS all off; 'amateur' = ideal_line on; 'semi-pro' = any other assist on.
    // billing_session_id: links lap to the billing session that paid for the track time.
    //   UX-04 integrity gate: laps without a billing_session_id are rejected at INSERT.
    //   Only laps from billed sessions appear on leaderboard — no manual entry path exists.
    // validity: lifecycle status — 'valid', 'invalid', 'unverifiable', 'suspect'.
    //   'unverifiable' is set when the telemetry adapter crashes mid-session (UX-07).
    let _ = sqlx::query("ALTER TABLE laps ADD COLUMN assist_config_hash TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query(
        "ALTER TABLE laps ADD COLUMN assist_tier TEXT NOT NULL DEFAULT 'unknown'",
    )
    .execute(pool)
    .await;

    let _ = sqlx::query("ALTER TABLE laps ADD COLUMN billing_session_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query(
        "ALTER TABLE laps ADD COLUMN validity TEXT NOT NULL DEFAULT 'valid'",
    )
    .execute(pool)
    .await;

    // Index for fast leaderboard queries filtering by assist_tier + validity
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_laps_assist_tier ON laps(track, assist_tier, validity)",
    )
    .execute(pool)
    .await?;


    // ─── Linked racers: parent account can add up to 3 guest racers ────────────
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN linked_to TEXT REFERENCES drivers(id)")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_drivers_linked_to ON drivers(linked_to)")
        .execute(pool)
        .await?;


    // Acts 1-4: Bonus tracking flags on drivers (one-time per customer)
    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN review_bonus_claimed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN follow_bonus_claimed BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE drivers ADD COLUMN registration_bonus_credited BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;


    Ok(())
}
