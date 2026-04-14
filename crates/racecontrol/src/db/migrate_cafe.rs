//! Database migrations: cafe domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_cafe(pool: &SqlitePool) -> anyhow::Result<()> {
    // ─── Cafe Menu ──────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cafe_categories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cafe_items (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            description TEXT,
            category_id TEXT NOT NULL REFERENCES cafe_categories(id),
            selling_price_paise INTEGER NOT NULL,
            cost_price_paise INTEGER NOT NULL,
            is_available BOOLEAN DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_items_category ON cafe_items(category_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_items_available ON cafe_items(is_available)")
        .execute(pool)
        .await?;


    // Idempotent migration: add image_path column (ignore error if already exists)
    let _ = sqlx::query("ALTER TABLE cafe_items ADD COLUMN image_path TEXT")
        .execute(pool)
        .await;

    // Intentionally ignore error — column already exists on second run

    // Idempotent migrations: inventory tracking columns
    let _ = sqlx::query("ALTER TABLE cafe_items ADD COLUMN is_countable BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE cafe_items ADD COLUMN stock_quantity INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE cafe_items ADD COLUMN low_stock_threshold INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    // Add last_stock_alert_at to cafe_items (idempotent — ignore error if column exists)
    let _ = sqlx::query("ALTER TABLE cafe_items ADD COLUMN last_stock_alert_at TEXT")
        .execute(pool)
        .await;


    // ─── Cafe Orders ──────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cafe_orders (
            id TEXT PRIMARY KEY,
            receipt_number TEXT NOT NULL UNIQUE,
            driver_id TEXT NOT NULL,
            items TEXT NOT NULL,
            total_paise INTEGER NOT NULL,
            wallet_txn_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'confirmed',
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_orders_driver ON cafe_orders(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_orders_receipt ON cafe_orders(receipt_number)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_orders_created ON cafe_orders(created_at)")
        .execute(pool)
        .await?;


    // Phase 157: promo columns on cafe_orders
    let _ = sqlx::query(
        "ALTER TABLE cafe_orders ADD COLUMN applied_promo_id TEXT"
    )
    .execute(pool)
    .await;


    let _ = sqlx::query(
        "ALTER TABLE cafe_orders ADD COLUMN discount_paise INTEGER NOT NULL DEFAULT 0"
    )
    .execute(pool)
    .await;


    // cafe_promos table (Phase 156: promotions engine)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cafe_promos (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            promo_type TEXT NOT NULL CHECK(promo_type IN ('combo', 'happy_hour', 'gaming_bundle')),
            config TEXT NOT NULL DEFAULT '{}',
            is_active BOOLEAN NOT NULL DEFAULT 0,
            start_time TEXT,
            end_time TEXT,
            stacking_group TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_cafe_promos_active ON cafe_promos(is_active)")
        .execute(pool)
        .await?;


    // Seed default categories (idempotent)
    for (name, order) in [("Beverages", 1), ("Snacks", 2), ("Meals", 3)] {
        let id = uuid::Uuid::new_v4().to_string();

        sqlx::query("INSERT OR IGNORE INTO cafe_categories (id, name, sort_order) VALUES (?, ?, ?)")
            .bind(&id)
            .bind(name)
            .bind(order)
            .execute(pool)
            .await?;

    }

    Ok(())
}
