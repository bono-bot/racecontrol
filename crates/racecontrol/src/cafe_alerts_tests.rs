use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    use super::{check_low_stock_alerts, reset_alert_cooldown};
    use crate::config::Config;

    /// Create an in-memory SQLite database with the cafe_items schema needed for tests.
    async fn test_db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("failed to create test pool");

        sqlx::query(
            "CREATE TABLE cafe_items (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                is_countable BOOLEAN NOT NULL DEFAULT 0,
                stock_quantity INTEGER NOT NULL DEFAULT 0,
                low_stock_threshold INTEGER NOT NULL DEFAULT 0,
                last_stock_alert_at TEXT
            )",
        )
        .execute(&pool)
        .await
        .expect("failed to create cafe_items");

        pool
    }

    /// Insert a test item into the DB.
    async fn insert_item(
        pool: &SqlitePool,
        id: &str,
        name: &str,
        is_countable: bool,
        stock: i64,
        threshold: i64,
        last_alert_at: Option<&str>,
    ) {
        sqlx::query(
            "INSERT INTO cafe_items (id, name, is_countable, stock_quantity, low_stock_threshold, last_stock_alert_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(name)
        .bind(is_countable)
        .bind(stock)
        .bind(threshold)
        .bind(last_alert_at)
        .execute(pool)
        .await
        .expect("failed to insert item");
    }

    /// Read last_stock_alert_at for an item from DB.
    async fn get_last_alert_at(pool: &SqlitePool, id: &str) -> Option<String> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT last_stock_alert_at FROM cafe_items WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .expect("query failed")
        .flatten()
    }

    fn test_config() -> Config {
        let toml_str = r#"
[venue]
name = "Test Venue"
[server]
[database]
"#;
        toml::from_str(toml_str).expect("failed to parse test config")
    }

    #[tokio::test]
    async fn skips_uncountable_items() {
        let pool = test_db().await;
        insert_item(&pool, "item-1", "Coffee", false, 0, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-1").await;

        // last_stock_alert_at must remain NULL — no alert fired
        let last_at = get_last_alert_at(&pool, "item-1").await;
        assert!(
            last_at.is_none(),
            "expected last_stock_alert_at=NULL for uncountable item, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn skips_when_stock_above_threshold() {
        let pool = test_db().await;
        insert_item(&pool, "item-2", "Cola", true, 10, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-2").await;

        let last_at = get_last_alert_at(&pool, "item-2").await;
        assert!(
            last_at.is_none(),
            "expected no alert when stock > threshold, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn skips_when_threshold_zero() {
        let pool = test_db().await;
        insert_item(&pool, "item-3", "Water", true, 0, 0, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-3").await;

        let last_at = get_last_alert_at(&pool, "item-3").await;
        assert!(
            last_at.is_none(),
            "expected no alert when threshold=0, got {:?}",
            last_at
        );
    }

    #[tokio::test]
    async fn sets_alert_timestamp_on_breach() {
        let pool = test_db().await;
        // stock=3 <= threshold=5, is_countable=true, no previous alert
        insert_item(&pool, "item-4", "Energy Drink", true, 3, 5, None).await;

        check_low_stock_alerts(&pool, &test_config(), "item-4").await;

        let last_at = get_last_alert_at(&pool, "item-4").await;
        assert!(
            last_at.is_some(),
            "expected last_stock_alert_at to be set on breach, got None"
        );
    }

    #[tokio::test]
    async fn suppresses_within_cooldown() {
        let pool = test_db().await;
        // Set last_stock_alert_at to NOW — cooldown active
        insert_item(&pool, "item-5", "Chips", true, 2, 5, Some("2099-01-01 00:00:00")).await;

        // Override with actual current time to ensure cooldown is active
        sqlx::query(
            "UPDATE cafe_items SET last_stock_alert_at = datetime('now') WHERE id = 'item-5'",
        )
        .execute(&pool)
        .await
        .expect("update failed");

        let before = get_last_alert_at(&pool, "item-5").await;

        check_low_stock_alerts(&pool, &test_config(), "item-5").await;

        let after = get_last_alert_at(&pool, "item-5").await;
        // The timestamp should not have changed since cooldown was active
        assert_eq!(
            before, after,
            "expected alert to be suppressed within cooldown window"
        );
    }

    #[tokio::test]
    async fn fires_again_after_cooldown_expired() {
        let pool = test_db().await;
        // Set last_stock_alert_at to 5 hours ago — cooldown expired
        insert_item(&pool, "item-6", "Juice", true, 1, 5, None).await;
        sqlx::query(
            "UPDATE cafe_items SET last_stock_alert_at = datetime('now', '-5 hours') WHERE id = 'item-6'",
        )
        .execute(&pool)
        .await
        .expect("update failed");

        let before = get_last_alert_at(&pool, "item-6").await;

        check_low_stock_alerts(&pool, &test_config(), "item-6").await;

        let after = get_last_alert_at(&pool, "item-6").await;
        assert_ne!(
            before, after,
            "expected alert to fire after cooldown expired (timestamp should update)"
        );
        assert!(after.is_some(), "expected new timestamp to be set");
    }

    #[tokio::test]
    async fn reset_cooldown_clears_timestamp() {
        let pool = test_db().await;
        insert_item(&pool, "item-7", "Sandwich", true, 0, 5, Some("2026-03-22 10:00:00")).await;

        // Verify it is set
        let before = get_last_alert_at(&pool, "item-7").await;
        assert!(before.is_some(), "precondition: last_stock_alert_at should be set");

        reset_alert_cooldown(&pool, "item-7").await;

        let after = get_last_alert_at(&pool, "item-7").await;
        assert!(
            after.is_none(),
            "expected last_stock_alert_at=NULL after reset, got {:?}",
            after
        );
    }

    #[tokio::test]
    async fn list_low_stock_returns_only_breached() {
        // This test is for the query logic only — we test via direct DB query
        // since list_low_stock_items requires axum State which is unavailable in unit tests.
        let pool = test_db().await;

        // Item A: countable, breached (stock <= threshold)
        insert_item(&pool, "item-a", "Bread", true, 2, 5, None).await;
        // Item B: countable, OK (stock > threshold)
        insert_item(&pool, "item-b", "Butter", true, 10, 5, None).await;
        // Item C: uncountable (should be excluded)
        insert_item(&pool, "item-c", "Service", false, 0, 0, None).await;
        // Item D: countable, threshold=0 (should be excluded)
        insert_item(&pool, "item-d", "Gift Card", true, 0, 0, None).await;

        let items: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT id, name, stock_quantity, low_stock_threshold
             FROM cafe_items
             WHERE is_countable = 1
               AND low_stock_threshold > 0
               AND stock_quantity <= low_stock_threshold
             ORDER BY name ASC",
        )
        .fetch_all(&pool)
        .await
        .expect("query failed");

        assert_eq!(items.len(), 1, "expected exactly 1 breached item, got {:?}", items);
        assert_eq!(items[0].0, "item-a", "expected item-a to be in low-stock list");
    }
