use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

use super::{
    ConfirmedImportRow, RawImportRow, confirm_import_rows, detect_column, normalize_header,
    parse_csv_bytes, validate_import_row,
};

async fn test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to create test pool");

    sqlx::query("PRAGMA foreign_keys=ON")
        .execute(&pool)
        .await
        .expect("failed to enable foreign keys");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS cafe_categories (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL UNIQUE,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create cafe_categories");

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
            updated_at TEXT,
            image_path TEXT,
            is_countable BOOLEAN DEFAULT 0,
            stock_quantity INTEGER DEFAULT 0,
            low_stock_threshold INTEGER DEFAULT 0
        )",
    )
    .execute(&pool)
    .await
    .expect("failed to create cafe_items");

    // Seed one test category
    sqlx::query(
        "INSERT INTO cafe_categories (id, name, sort_order) VALUES ('cat-test', 'Test Category', 1)",
    )
    .execute(&pool)
    .await
    .expect("failed to seed test category");

    pool
}

// ─── Existing tests ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_create_and_list_items() {
    let pool = test_db().await;

    sqlx::query(
        "INSERT INTO cafe_items (id, name, category_id, selling_price_paise, cost_price_paise, is_available)
         VALUES ('item-1', 'Espresso', 'cat-test', 15000, 5000, 1)",
    )
    .execute(&pool)
    .await
    .expect("failed to insert item");

    let items = sqlx::query_as::<_, super::CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items ORDER BY name ASC",
    )
    .fetch_all(&pool)
    .await
    .expect("failed to fetch items");

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Espresso");
    assert_eq!(items[0].selling_price_paise, 15000);
    assert!(items[0].is_available);
    assert!(items[0].image_path.is_none());
    assert!(!items[0].is_countable);
    assert_eq!(items[0].stock_quantity, 0);
}

#[tokio::test]
async fn test_is_available_filter() {
    let pool = test_db().await;

    sqlx::query(
        "INSERT INTO cafe_items (id, name, category_id, selling_price_paise, cost_price_paise, is_available)
         VALUES ('item-avail', 'Available Item', 'cat-test', 10000, 3000, 1),
                ('item-unavail', 'Unavailable Item', 'cat-test', 10000, 3000, 0)",
    )
    .execute(&pool)
    .await
    .expect("failed to insert items");

    let available = sqlx::query_as::<_, super::CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items WHERE is_available = 1",
    )
    .fetch_all(&pool)
    .await
    .expect("failed to fetch available items");

    assert_eq!(available.len(), 1);
    assert_eq!(available[0].name, "Available Item");
}

#[tokio::test]
async fn test_foreign_key_enforcement() {
    let pool = test_db().await;

    let result = sqlx::query(
        "INSERT INTO cafe_items (id, name, category_id, selling_price_paise, cost_price_paise)
         VALUES ('item-fk', 'Bad Item', 'nonexistent-category', 10000, 3000)",
    )
    .execute(&pool)
    .await;

    assert!(result.is_err(), "Expected FK violation error, but insert succeeded");
}

#[tokio::test]
async fn test_category_unique_constraint() {
    let pool = test_db().await;

    // First insert (should succeed — 'Test Category' already seeded, use new name)
    sqlx::query(
        "INSERT INTO cafe_categories (id, name, sort_order) VALUES ('cat-dup', 'Duplicate Cat', 5)",
    )
    .execute(&pool)
    .await
    .expect("first insert should succeed");

    // Second insert with same name should fail (UNIQUE constraint)
    let result = sqlx::query(
        "INSERT INTO cafe_categories (id, name, sort_order) VALUES ('cat-dup2', 'Duplicate Cat', 6)",
    )
    .execute(&pool)
    .await;

    assert!(result.is_err(), "Expected UNIQUE constraint violation");

    // Assert only one row with that name exists
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM cafe_categories WHERE name = 'Duplicate Cat'",
    )
    .fetch_one(&pool)
    .await
    .expect("count query failed");

    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_toggle_availability() {
    let pool = test_db().await;

    sqlx::query(
        "INSERT INTO cafe_items (id, name, category_id, selling_price_paise, cost_price_paise, is_available)
         VALUES ('item-toggle', 'Toggle Item', 'cat-test', 10000, 3000, 1)",
    )
    .execute(&pool)
    .await
    .expect("failed to insert item");

    // Toggle once — should become false
    sqlx::query(
        "UPDATE cafe_items SET is_available = NOT is_available, updated_at = datetime('now') WHERE id = 'item-toggle'",
    )
    .execute(&pool)
    .await
    .expect("failed to toggle");

    let avail: bool = sqlx::query_scalar::<_, bool>(
        "SELECT is_available FROM cafe_items WHERE id = 'item-toggle'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to fetch");

    assert!(!avail);

    // Toggle again — should become true
    sqlx::query(
        "UPDATE cafe_items SET is_available = NOT is_available, updated_at = datetime('now') WHERE id = 'item-toggle'",
    )
    .execute(&pool)
    .await
    .expect("failed to toggle again");

    let avail2: bool = sqlx::query_scalar::<_, bool>(
        "SELECT is_available FROM cafe_items WHERE id = 'item-toggle'",
    )
    .fetch_one(&pool)
    .await
    .expect("failed to fetch again");

    assert!(avail2);
}

// ─── New import / image_path tests ───────────────────────────────────────

#[test]
fn test_normalize_header() {
    assert_eq!(normalize_header("Selling Price"), "sellingprice");
    assert_eq!(normalize_header("Item Name"), "itemname");
    assert_eq!(normalize_header("COST_PRICE"), "costprice");
    assert_eq!(normalize_header("  desc  "), "desc");
}

#[test]
fn test_detect_column() {
    assert_eq!(detect_column("sellingprice"), Some("selling_price"));
    assert_eq!(detect_column("name"), Some("name"));
    assert_eq!(detect_column("xyz"), None);
    assert_eq!(detect_column("costprice"), Some("cost_price"));
    assert_eq!(detect_column("category"), Some("category"));
    assert_eq!(detect_column("description"), Some("description"));
}

#[test]
fn test_validate_import_row_valid() {
    let row = RawImportRow {
        row_num: 2,
        name: "Espresso".to_string(),
        category: "Beverages".to_string(),
        selling_price: "150".to_string(),
        cost_price: "40".to_string(),
        description: String::new(),
    };
    let errors = validate_import_row(&row);
    assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
}

#[test]
fn test_validate_import_row_empty_name() {
    let row = RawImportRow {
        row_num: 2,
        name: String::new(),
        category: "Beverages".to_string(),
        selling_price: "150".to_string(),
        cost_price: "40".to_string(),
        description: String::new(),
    };
    let errors = validate_import_row(&row);
    assert!(!errors.is_empty());
    assert!(
        errors.iter().any(|e| e.contains("name")),
        "Expected error mentioning 'name', got: {:?}",
        errors
    );
}

#[test]
fn test_validate_import_row_zero_price() {
    let row = RawImportRow {
        row_num: 2,
        name: "Espresso".to_string(),
        category: "Beverages".to_string(),
        selling_price: "0".to_string(),
        cost_price: "0".to_string(),
        description: String::new(),
    };
    let errors = validate_import_row(&row);
    assert!(!errors.is_empty());
    assert!(
        errors.iter().any(|e| e.contains("selling_price")),
        "Expected error mentioning 'selling_price', got: {:?}",
        errors
    );
}

#[test]
fn test_parse_csv_bytes() {
    // CSV with BOM prefix and standard headers
    let csv_with_bom =
        "\u{feff}Item Name,Category,Selling Price,Cost Price,Description\nEspresso,Beverages,150,40,Strong coffee\nLatte,Beverages,180,50,Milk coffee\n";
    let bytes = csv_with_bom.as_bytes();

    let (headers, rows) = parse_csv_bytes(bytes).expect("CSV parse failed");

    // BOM should be stripped from first header
    assert_eq!(headers[0], "Item Name", "BOM not stripped from first header");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].name, "Espresso");
    assert_eq!(rows[0].category, "Beverages");
    assert_eq!(rows[0].selling_price, "150");
    assert_eq!(rows[0].cost_price, "40");
    assert_eq!(rows[0].description, "Strong coffee");
    assert_eq!(rows[1].name, "Latte");
}

#[test]
fn test_parse_csv_bytes_no_bom() {
    let csv = "name,category,sellingprice,costprice\nCappuccino,Coffee,120,35\n";
    let bytes = csv.as_bytes();

    let (headers, rows) = parse_csv_bytes(bytes).expect("CSV parse failed");
    assert_eq!(headers[0], "name");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "Cappuccino");
    assert_eq!(rows[0].selling_price, "120");
}

// XLSX parsing: manual test note
// parse_xlsx_bytes is tested indirectly via the handler. Direct unit testing requires
// a valid .xlsx byte array which is non-trivial to construct inline without rust_xlsxwriter.
// Integration testing via the import_preview endpoint covers the XLSX path in practice.

#[tokio::test]
async fn test_image_path_column() {
    let pool = test_db().await;

    // Insert item with image_path
    sqlx::query(
        "INSERT INTO cafe_items (id, name, category_id, selling_price_paise, cost_price_paise, is_available, image_path)
         VALUES ('item-img', 'Item With Image', 'cat-test', 10000, 3000, 1, 'test-image.jpg')",
    )
    .execute(&pool)
    .await
    .expect("INSERT with image_path failed");

    // SELECT including image_path and inventory columns
    let item = sqlx::query_as::<_, super::CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items WHERE id = 'item-img'",
    )
    .fetch_one(&pool)
    .await
    .expect("SELECT with image_path failed");

    assert_eq!(item.image_path, Some("test-image.jpg".to_string()));
    assert!(!item.is_countable);
    assert_eq!(item.stock_quantity, 0);
}

#[tokio::test]
async fn test_import_confirm_transaction() {
    let pool = test_db().await;

    let rows = vec![
        ConfirmedImportRow {
            name: "Espresso".to_string(),
            category: "Test Category".to_string(),
            selling_price_paise: 15000,
            cost_price_paise: 5000,
            description: None,
        },
        ConfirmedImportRow {
            name: "Latte".to_string(),
            category: "Test Category".to_string(),
            selling_price_paise: 18000,
            cost_price_paise: 6000,
            description: Some("Milk coffee".to_string()),
        },
        ConfirmedImportRow {
            name: "Cappuccino".to_string(),
            category: "Test Category".to_string(),
            selling_price_paise: 16000,
            cost_price_paise: 5500,
            description: None,
        },
    ];

    let count = confirm_import_rows(&pool, &rows)
        .await
        .expect("confirm_import_rows failed");

    assert_eq!(count, 3);

    let db_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM cafe_items")
        .fetch_one(&pool)
        .await
        .expect("count query failed");

    assert_eq!(db_count, 3);
}

#[tokio::test]
async fn test_import_creates_categories() {
    let pool = test_db().await;

    // Use a category that doesn't exist yet
    let rows = vec![ConfirmedImportRow {
        name: "Sandwich".to_string(),
        category: "NewCat".to_string(),
        selling_price_paise: 12000,
        cost_price_paise: 5000,
        description: None,
    }];

    let count = confirm_import_rows(&pool, &rows)
        .await
        .expect("confirm_import_rows failed");

    assert_eq!(count, 1);

    // Verify the category was auto-created
    let cat_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM cafe_categories WHERE name = 'NewCat'",
    )
    .fetch_one(&pool)
    .await
    .expect("category count query failed");

    assert_eq!(cat_count, 1, "Category 'NewCat' should have been auto-created");
}
