use std::sync::Arc;

use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::state::AppState;

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafeCategory {
    pub id: String,
    pub name: String,
    pub sort_order: i64,
    pub created_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct CafeItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub is_available: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub image_path: Option<String>,
    pub is_countable: bool,
    pub stock_quantity: i64,
    pub low_stock_threshold: i64,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafeItemRequest {
    pub name: String,
    pub description: Option<String>,
    pub category_id: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub is_countable: Option<bool>,
    pub stock_quantity: Option<i64>,
    pub low_stock_threshold: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCafeItemRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<String>,
    pub selling_price_paise: Option<i64>,
    pub cost_price_paise: Option<i64>,
    pub is_countable: Option<bool>,
    pub stock_quantity: Option<i64>,
    pub low_stock_threshold: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCafeCategoryRequest {
    pub name: String,
    pub sort_order: Option<i64>,
}

// ─── Import Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawImportRow {
    pub row_num: usize,
    pub name: String,
    pub category: String,
    pub selling_price: String,
    pub cost_price: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct ImportRowResult {
    #[serde(flatten)]
    pub row: RawImportRow,
    pub valid: bool,
    pub errors: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmedImportRow {
    pub name: String,
    pub category: String,
    pub selling_price_paise: i64,
    pub cost_price_paise: i64,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ConfirmImportRequest {
    pub rows: Vec<ConfirmedImportRow>,
}

// ─── Import Pure Functions ────────────────────────────────────────────────────

/// Normalize a header to lowercase alphanumeric for fuzzy matching.
pub fn normalize_header(h: &str) -> String {
    h.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Map a normalized header to a known field name.
pub fn detect_column(normalized: &str) -> Option<&'static str> {
    match normalized {
        "name" | "itemname" | "item" | "productname" => Some("name"),
        "category" | "cat" | "categoryname" | "group" => Some("category"),
        "sellingprice" | "price" | "sp" | "mrp" | "rate" => Some("selling_price"),
        "costprice" | "cost" | "cp" | "purchaseprice" => Some("cost_price"),
        "description" | "desc" | "details" => Some("description"),
        _ => None,
    }
}

/// Map each header index to an optional field name.
pub fn detect_column_mapping(headers: &[String]) -> Vec<Option<&'static str>> {
    headers
        .iter()
        .map(|h| detect_column(&normalize_header(h)))
        .collect()
}

/// Validate a raw import row; returns a list of human-readable errors.
pub fn validate_import_row(row: &RawImportRow) -> Vec<String> {
    let mut errors = Vec::new();
    if row.name.trim().is_empty() {
        errors.push("name is required".to_string());
    }
    match row.selling_price.trim().parse::<f64>() {
        Ok(v) if v > 0.0 => {}
        Ok(_) => errors.push("selling_price must be > 0".to_string()),
        Err(_) => errors.push("selling_price must be a valid number".to_string()),
    }
    match row.cost_price.trim().parse::<f64>() {
        Ok(v) if v >= 0.0 => {}
        Ok(_) => errors.push("cost_price must be >= 0".to_string()),
        Err(_) => errors.push("cost_price must be a valid number".to_string()),
    }
    errors
}

/// Parse XLSX bytes into (headers, rows).
pub fn parse_xlsx_bytes(bytes: &[u8]) -> Result<(Vec<String>, Vec<RawImportRow>), String> {
    use calamine::{Data, Reader, Xlsx, open_workbook_from_rs};
    use std::io::Cursor;

    let cursor = Cursor::new(bytes);
    let mut workbook: Xlsx<_> =
        open_workbook_from_rs(cursor).map_err(|e| format!("Failed to open XLSX: {e}"))?;

    let sheet = workbook
        .worksheet_range_at(0)
        .ok_or_else(|| "No sheets in workbook".to_string())?
        .map_err(|e| format!("Failed to read sheet: {e}"))?;

    let mut rows_iter = sheet.rows();

    let raw_headers: Vec<String> = rows_iter
        .next()
        .unwrap_or_default()
        .iter()
        .map(|c| match c {
            Data::String(s) => s.clone(),
            Data::Empty => String::new(),
            other => other.to_string(),
        })
        .collect();

    let mapping = detect_column_mapping(&raw_headers);

    let mut rows = Vec::new();
    for (row_num, row) in rows_iter.enumerate() {
        let cell_str = |i: usize| {
            row.get(i)
                .map(|d| match d {
                    Data::Float(f) => {
                        // Represent prices as plain number strings (e.g. "150")
                        if f.fract() == 0.0 {
                            format!("{}", *f as i64)
                        } else {
                            format!("{f}")
                        }
                    }
                    Data::Int(n) => n.to_string(),
                    Data::String(s) => s.clone(),
                    Data::Bool(b) => b.to_string(),
                    Data::Empty => String::new(),
                    other => other.to_string(),
                })
                .unwrap_or_default()
        };

        let mut raw = RawImportRow {
            row_num: row_num + 2, // 1-indexed, skip header
            name: String::new(),
            category: String::new(),
            selling_price: String::new(),
            cost_price: String::new(),
            description: String::new(),
        };

        for (i, mapped) in mapping.iter().enumerate() {
            match *mapped {
                Some("name") => raw.name = cell_str(i),
                Some("category") => raw.category = cell_str(i),
                Some("selling_price") => raw.selling_price = cell_str(i),
                Some("cost_price") => raw.cost_price = cell_str(i),
                Some("description") => raw.description = cell_str(i),
                _ => {}
            }
        }

        rows.push(raw);
    }

    Ok((raw_headers, rows))
}

/// Parse CSV bytes into (headers, rows). Strips UTF-8 BOM from first header.
pub fn parse_csv_bytes(bytes: &[u8]) -> Result<(Vec<String>, Vec<RawImportRow>), String> {
    use csv::ReaderBuilder;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(bytes);

    let raw_headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("Failed to read CSV headers: {e}"))?
        .iter()
        .enumerate()
        .map(|(i, h)| {
            if i == 0 {
                // Strip UTF-8 BOM if present
                h.trim_start_matches('\u{feff}').to_string()
            } else {
                h.to_string()
            }
        })
        .collect();

    let mapping = detect_column_mapping(&raw_headers);

    let mut rows = Vec::new();
    for (row_num, result) in reader.records().enumerate() {
        let record = result.map_err(|e| format!("CSV parse error at row {}: {e}", row_num + 2))?;

        let cell_str = |i: usize| record.get(i).unwrap_or("").to_string();

        let mut raw = RawImportRow {
            row_num: row_num + 2,
            name: String::new(),
            category: String::new(),
            selling_price: String::new(),
            cost_price: String::new(),
            description: String::new(),
        };

        for (i, mapped) in mapping.iter().enumerate() {
            match *mapped {
                Some("name") => raw.name = cell_str(i),
                Some("category") => raw.category = cell_str(i),
                Some("selling_price") => raw.selling_price = cell_str(i),
                Some("cost_price") => raw.cost_price = cell_str(i),
                Some("description") => raw.description = cell_str(i),
                _ => {}
            }
        }

        rows.push(raw);
    }

    Ok((raw_headers, rows))
}

/// Insert confirmed rows in a single transaction. Auto-creates categories.
pub async fn confirm_import_rows(
    db: &sqlx::SqlitePool,
    rows: &[ConfirmedImportRow],
) -> Result<usize, String> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| format!("Failed to begin transaction: {e}"))?;

    let mut count = 0usize;

    for row in rows {
        // Look up or create category
        let cat_id: Option<String> = sqlx::query_scalar::<_, String>(
            "SELECT id FROM cafe_categories WHERE name = ?",
        )
        .bind(&row.category)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Category lookup error: {e}"))?;

        let cat_id = match cat_id {
            Some(id) => id,
            None => {
                let new_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO cafe_categories (id, name, sort_order) VALUES (?, ?, 0)",
                )
                .bind(&new_id)
                .bind(&row.category)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Category insert error: {e}"))?;
                new_id
            }
        };

        let item_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO cafe_items (id, name, description, category_id, selling_price_paise, cost_price_paise, is_available, is_countable, stock_quantity, low_stock_threshold)
             VALUES (?, ?, ?, ?, ?, ?, 1, 0, 0, 0)",
        )
        .bind(&item_id)
        .bind(row.name.trim())
        .bind(row.description.as_deref())
        .bind(&cat_id)
        .bind(row.selling_price_paise)
        .bind(row.cost_price_paise)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Item insert error: {e}"))?;

        count += 1;
    }

    tx.commit()
        .await
        .map_err(|e| format!("Transaction commit error: {e}"))?;

    Ok(count)
}

// ─── Handlers ────────────────────────────────────────────────────────────────

pub async fn list_cafe_items(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let items = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items ORDER BY name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("list_cafe_items DB error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let total = items.len();
    Ok(Json(serde_json::json!({ "items": items, "total": total, "page": 1 })))
}

pub async fn create_cafe_item(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafeItemRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    // Validate inputs
    if req.name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.selling_price_paise <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    if req.cost_price_paise < 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check category exists
    let cat_exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM cafe_categories WHERE id = ?",
    )
    .bind(&req.category_id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_item category check error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if cat_exists == 0 {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "category_id not found" })),
        ));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO cafe_items (id, name, description, category_id, selling_price_paise, cost_price_paise, is_available, is_countable, stock_quantity, low_stock_threshold)
         VALUES (?, ?, ?, ?, ?, ?, 1, ?, ?, ?)",
    )
    .bind(&id)
    .bind(req.name.trim())
    .bind(req.description.as_deref())
    .bind(&req.category_id)
    .bind(req.selling_price_paise)
    .bind(req.cost_price_paise)
    .bind(req.is_countable.unwrap_or(false))
    .bind(req.stock_quantity.unwrap_or(0))
    .bind(req.low_stock_threshold.unwrap_or(0))
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_item insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((StatusCode::CREATED, Json(serde_json::json!({ "id": id }))))
}

pub async fn update_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateCafeItemRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // If category_id provided, verify it exists
    if let Some(ref cat_id) = req.category_id {
        let cat_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM cafe_categories WHERE id = ?",
        )
        .bind(cat_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| {
            tracing::warn!("update_cafe_item category check error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if cat_exists == 0 {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Build dynamic SET clause
    let mut set_clauses: Vec<String> = Vec::new();
    set_clauses.push("updated_at = datetime('now')".to_string());

    if req.name.is_some() {
        set_clauses.push("name = ?".to_string());
    }
    if req.description.is_some() {
        set_clauses.push("description = ?".to_string());
    }
    if req.category_id.is_some() {
        set_clauses.push("category_id = ?".to_string());
    }
    if req.selling_price_paise.is_some() {
        set_clauses.push("selling_price_paise = ?".to_string());
    }
    if req.cost_price_paise.is_some() {
        set_clauses.push("cost_price_paise = ?".to_string());
    }
    if req.is_countable.is_some() {
        set_clauses.push("is_countable = ?".to_string());
    }
    if req.stock_quantity.is_some() {
        set_clauses.push("stock_quantity = ?".to_string());
    }
    if req.low_stock_threshold.is_some() {
        set_clauses.push("low_stock_threshold = ?".to_string());
    }

    let query_str = format!(
        "UPDATE cafe_items SET {} WHERE id = ?",
        set_clauses.join(", ")
    );

    let mut q = sqlx::query(&query_str);
    if let Some(ref name) = req.name {
        q = q.bind(name);
    }
    if let Some(ref desc) = req.description {
        q = q.bind(desc);
    }
    if let Some(ref cat_id) = req.category_id {
        q = q.bind(cat_id);
    }
    if let Some(price) = req.selling_price_paise {
        q = q.bind(price);
    }
    if let Some(cost) = req.cost_price_paise {
        q = q.bind(cost);
    }
    if let Some(countable) = req.is_countable {
        q = q.bind(countable);
    }
    if let Some(qty) = req.stock_quantity {
        q = q.bind(qty);
    }
    if let Some(threshold) = req.low_stock_threshold {
        q = q.bind(threshold);
    }
    q = q.bind(&id);

    q.execute(&state.db).await.map_err(|e| {
        tracing::warn!("update_cafe_item execute error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Return updated item
    let item = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("update_cafe_item fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item {
        Some(i) => Ok(Json(serde_json::json!(i))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn delete_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query("DELETE FROM cafe_items WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await
        .map_err(|e| {
            tracing::warn!("delete_cafe_item error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn toggle_cafe_item_availability(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let result = sqlx::query(
        "UPDATE cafe_items SET is_available = NOT is_available, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("toggle_cafe_item_availability error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    // Fetch new value
    let new_available = sqlx::query_scalar::<_, bool>(
        "SELECT is_available FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("toggle_cafe_item_availability fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "id": id, "is_available": new_available })))
}

pub async fn list_cafe_categories(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let categories = sqlx::query_as::<_, CafeCategory>(
        "SELECT id, name, sort_order, created_at FROM cafe_categories ORDER BY sort_order ASC, name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("list_cafe_categories error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "categories": categories })))
}

pub async fn create_cafe_category(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCafeCategoryRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let id = Uuid::new_v4().to_string();
    let sort_order = req.sort_order.unwrap_or(0);

    sqlx::query(
        "INSERT OR IGNORE INTO cafe_categories (id, name, sort_order) VALUES (?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.name)
    .bind(sort_order)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_category insert error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // SELECT by name to return (handles both new and existing)
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM cafe_categories WHERE name = ?",
    )
    .bind(&req.name)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("create_cafe_category fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "id": row.0, "name": row.1 })))
}

pub async fn public_menu(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Custom row type for JOIN result
    #[derive(Debug, Serialize, sqlx::FromRow)]
    struct MenuItem {
        id: String,
        name: String,
        description: Option<String>,
        category_id: String,
        category_name: String,
        selling_price_paise: i64,
        cost_price_paise: i64,
        is_available: bool,
        created_at: Option<String>,
        updated_at: Option<String>,
        image_path: Option<String>,
        is_countable: bool,
        stock_quantity: i64,
    }

    let items = sqlx::query_as::<_, MenuItem>(
        "SELECT ci.id, ci.name, ci.description, ci.category_id,
                cc.name AS category_name,
                ci.selling_price_paise, ci.cost_price_paise,
                ci.is_available, ci.created_at, ci.updated_at,
                ci.image_path, ci.is_countable, ci.stock_quantity
         FROM cafe_items ci
         JOIN cafe_categories cc ON ci.category_id = cc.id
         WHERE ci.is_available = 1
         ORDER BY cc.sort_order ASC, ci.name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("public_menu error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let items_json: Vec<serde_json::Value> = items
        .iter()
        .map(|item| {
            let mut val = serde_json::to_value(item).unwrap_or_default();
            if let Some(obj) = val.as_object_mut() {
                let out_of_stock = item.is_countable && item.stock_quantity <= 0;
                obj.insert(
                    "out_of_stock".to_string(),
                    serde_json::Value::Bool(out_of_stock),
                );
            }
            val
        })
        .collect();

    let total = items_json.len();
    Ok(Json(serde_json::json!({ "items": items_json, "total": total, "page": 1 })))
}

// ─── Import Handlers ──────────────────────────────────────────────────────────

pub async fn import_preview(
    State(_state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("import_preview multipart error: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        if field.name() == Some("file") {
            filename = field
                .file_name()
                .unwrap_or("upload.csv")
                .to_lowercase();
            file_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| {
                        tracing::warn!("import_preview read bytes error: {}", e);
                        StatusCode::BAD_REQUEST
                    })?
                    .to_vec(),
            );
        }
    }

    let bytes = file_bytes.ok_or(StatusCode::BAD_REQUEST)?;

    let (raw_headers, rows) = if filename.ends_with(".xlsx") || filename.ends_with(".xls") {
        parse_xlsx_bytes(&bytes).map_err(|e| {
            tracing::warn!("import_preview XLSX parse error: {}", e);
            StatusCode::BAD_REQUEST
        })?
    } else if filename.ends_with(".csv") {
        parse_csv_bytes(&bytes).map_err(|e| {
            tracing::warn!("import_preview CSV parse error: {}", e);
            StatusCode::BAD_REQUEST
        })?
    } else {
        tracing::warn!("import_preview: unsupported file type '{}'", filename);
        return Err(StatusCode::BAD_REQUEST);
    };

    let mapping = detect_column_mapping(&raw_headers);
    let columns: Vec<serde_json::Value> = raw_headers
        .iter()
        .zip(mapping.iter())
        .enumerate()
        .map(|(i, (header, mapped))| {
            serde_json::json!({
                "index": i,
                "header": header,
                "mapped_to": mapped
            })
        })
        .collect();

    let row_results: Vec<ImportRowResult> = rows
        .into_iter()
        .map(|row| {
            let errors = validate_import_row(&row);
            let valid = errors.is_empty();
            ImportRowResult { row, valid, errors }
        })
        .collect();

    let total_rows = row_results.len();
    let valid_rows = row_results.iter().filter(|r| r.valid).count();
    let invalid_rows = total_rows - valid_rows;

    Ok(Json(serde_json::json!({
        "columns": columns,
        "rows": row_results,
        "total_rows": total_rows,
        "valid_rows": valid_rows,
        "invalid_rows": invalid_rows
    })))
}

pub async fn confirm_import(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConfirmImportRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let count = confirm_import_rows(&state.db, &req.rows).await.map_err(|e| {
        tracing::warn!("confirm_import error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(serde_json::json!({ "imported": count })))
}

// ─── Inventory Handlers ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RestockRequest {
    pub quantity: i64,
}

pub async fn restock_cafe_item(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<RestockRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if req.quantity <= 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Check item exists and is_countable
    let item_check = sqlx::query_as::<_, (String, bool)>(
        "SELECT id, is_countable FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("restock_cafe_item fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item_check {
        None => return Err(StatusCode::NOT_FOUND),
        Some((_, false)) => {
            return Ok(Json(serde_json::json!({ "error": "item is not countable" })));
        }
        Some(_) => {}
    }

    sqlx::query(
        "UPDATE cafe_items SET stock_quantity = stock_quantity + ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(req.quantity)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("restock_cafe_item update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Check new stock level and fire or reset low-stock alerts accordingly
    let new_stock_row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT stock_quantity, low_stock_threshold FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some((new_stock, threshold)) = new_stock_row {
        if new_stock > threshold {
            // Restocked above threshold: reset cooldown so next breach re-alerts
            crate::cafe_alerts::reset_alert_cooldown(&state.db, &id).await;
        } else {
            // Still at or below threshold: check and possibly fire alert
            crate::cafe_alerts::check_low_stock_alerts(&state.db, &state.config, &id).await;
        }
    }

    // Return updated item
    let item = sqlx::query_as::<_, CafeItem>(
        "SELECT id, name, description, category_id, selling_price_paise, cost_price_paise,
                is_available, created_at, updated_at, image_path,
                is_countable, stock_quantity, low_stock_threshold
         FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("restock_cafe_item fetch updated error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match item {
        Some(i) => Ok(Json(serde_json::json!(i))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

pub async fn upload_item_image(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Verify item exists and get old image_path
    let old_image: Option<String> = sqlx::query_scalar::<_, Option<String>>(
        "SELECT image_path FROM cafe_items WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("upload_item_image fetch error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or(StatusCode::NOT_FOUND)?;

    let mut image_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("upload_item_image multipart error: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        if field.name() == Some("file") {
            image_bytes = Some(
                field
                    .bytes()
                    .await
                    .map_err(|e| {
                        tracing::warn!("upload_item_image read bytes error: {}", e);
                        StatusCode::BAD_REQUEST
                    })?
                    .to_vec(),
            );
        }
    }

    let bytes = image_bytes.ok_or(StatusCode::BAD_REQUEST)?;

    // Decode and conditionally resize image
    let img = image::load_from_memory(&bytes).map_err(|e| {
        tracing::warn!("upload_item_image decode error: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    let resized = if img.width() > 800 {
        img.resize(800, u32::MAX, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let mut cursor = std::io::Cursor::new(Vec::new());
    resized
        .write_to(&mut cursor, image::ImageFormat::Jpeg)
        .map_err(|e| {
            tracing::warn!("upload_item_image encode error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let jpeg_bytes = cursor.into_inner();

    // Save to disk
    let filename = format!("{}.jpg", Uuid::new_v4());
    tokio::fs::create_dir_all("./data/cafe-images")
        .await
        .map_err(|e| {
            tracing::warn!("upload_item_image mkdir error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tokio::fs::write(format!("./data/cafe-images/{}", filename), &jpeg_bytes)
        .await
        .map_err(|e| {
            tracing::warn!("upload_item_image write error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Delete old image if present (ignore errors)
    if let Some(old_path) = old_image {
        if !old_path.is_empty() {
            let _ = tokio::fs::remove_file(format!("./data/cafe-images/{}", old_path)).await;
        }
    }

    // Update DB
    sqlx::query(
        "UPDATE cafe_items SET image_path = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(&filename)
    .bind(&id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("upload_item_image db update error: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(
        serde_json::json!({ "image_url": format!("/static/cafe-images/{}", filename) }),
    ))
}

// ─── Order Types ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PlaceOrderRequest {
    pub driver_id: String,
    pub items: Vec<OrderItemRequest>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct OrderItemRequest {
    pub item_id: String,
    pub quantity: i64,
}

#[derive(Debug, Serialize)]
pub struct PlaceOrderResponse {
    pub order_id: String,
    pub receipt_number: String,
    pub wallet_txn_id: String,
    pub total_paise: i64,
    pub discount_paise: i64,
    pub applied_promo_id: Option<String>,
    pub applied_promo_name: Option<String>,
    pub new_balance_paise: i64,
    pub items: Vec<OrderItemDetail>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OrderItemDetail {
    pub item_id: String,
    pub name: String,
    pub quantity: i64,
    pub unit_price_paise: i64,
    pub line_total_paise: i64,
}

/// Internal verified item during order processing (held between transaction and wallet debit).
#[derive(Clone)]
struct VerifiedOrderItem {
    item_id: String,
    name: String,
    quantity: i64,
    unit_price_paise: i64,
    is_countable: bool,
}

// ─── Order Handlers ───────────────────────────────────────────────────────────

/// Core order logic shared between staff and customer routes.
pub async fn place_cafe_order_inner(
    state: &Arc<AppState>,
    req: PlaceOrderRequest,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // ── Validation (before transaction) ──────────────────────────────────────
    if req.items.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "items must not be empty" })),
        ));
    }
    if req.driver_id.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "driver_id must not be empty" })),
        ));
    }
    for item in &req.items {
        if item.quantity < 1 {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "each item quantity must be >= 1" })),
            ));
        }
    }

    // ── Step A: Acquire raw connection and BEGIN IMMEDIATE ────────────────────
    let mut conn = state.db.acquire().await.map_err(|e| {
        tracing::warn!("place_cafe_order: failed to acquire connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Database unavailable" })),
        )
    })?;

    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: BEGIN IMMEDIATE failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Could not acquire write lock" })),
            )
        })?;

    // ── Step B: Validate all items exist, are available, check stock ──────────
    let mut verified_items: Vec<VerifiedOrderItem> = Vec::new();
    for req_item in &req.items {
        let row: Option<(String, String, i64, bool, i64, bool)> = sqlx::query_as(
            "SELECT id, name, selling_price_paise, is_countable, stock_quantity, is_available
             FROM cafe_items WHERE id = ?",
        )
        .bind(&req_item.item_id)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: item lookup error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Database error during item lookup" })),
            )
        })?;

        match row {
            None => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Item not found or unavailable: {}", req_item.item_id)
                    })),
                ));
            }
            Some((id, name, price, is_countable, stock_qty, is_available)) => {
                if !is_available {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": format!("Item not found or unavailable: {}", name)
                        })),
                    ));
                }
                if is_countable && stock_qty < req_item.quantity {
                    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": format!(
                                "Out of stock: {} (available: {}, requested: {})",
                                name, stock_qty, req_item.quantity
                            )
                        })),
                    ));
                }
                verified_items.push(VerifiedOrderItem {
                    item_id: id,
                    name,
                    quantity: req_item.quantity,
                    unit_price_paise: price,
                    is_countable,
                });
            }
        }
    }

    // ── Step C: Calculate total and build OrderItemDetail list ────────────────
    let mut total_paise: i64 = 0;
    let mut order_item_details: Vec<OrderItemDetail> = Vec::new();
    for item in &verified_items {
        let line_total = item.unit_price_paise * item.quantity;
        total_paise += line_total;
        order_item_details.push(OrderItemDetail {
            item_id: item.item_id.clone(),
            name: item.name.clone(),
            quantity: item.quantity,
            unit_price_paise: item.unit_price_paise,
            line_total_paise: line_total,
        });
    }

    // ── Step C2: Evaluate promos and apply best discount ─────────────────────
    // Fetch currently active promos from DB (outside transaction — read-only, non-blocking)
    let active_promos: Vec<crate::cafe_promos::ActivePromo> = {
        let now_ist = {
            let now_utc = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let ist_secs = now_utc + 19800;
            let h = (ist_secs / 3600) % 24;
            let m = (ist_secs % 3600) / 60;
            format!("{:02}:{:02}", h, m)
        };
        sqlx::query_as::<_, crate::cafe_promos::CafePromo>(
            "SELECT id, name, promo_type, config, is_active, start_time, end_time, stacking_group, created_at, updated_at
             FROM cafe_promos WHERE is_active = 1",
        )
        .fetch_all(&state.db)
        .await
        .unwrap_or_default() // promo fetch failure must NOT block the order
        .into_iter()
        .filter_map(|p| {
            if let (Some(start), Some(end)) = (&p.start_time, &p.end_time) {
                if !(now_ist.as_str() >= start.as_str() && now_ist.as_str() < end.as_str()) {
                    return None;
                }
            }
            let config = serde_json::from_str(&p.config).unwrap_or_default();
            Some(crate::cafe_promos::ActivePromo {
                id: p.id,
                name: p.name,
                promo_type: p.promo_type,
                config,
                stacking_group: p.stacking_group,
                time_label: None,
            })
        })
        .collect()
    };

    let cart_items: Vec<(String, i64)> = verified_items
        .iter()
        .map(|v| (v.item_id.clone(), v.quantity))
        .collect();

    let promo_result =
        crate::cafe_promos::evaluate_promos(&cart_items, &active_promos, total_paise);
    let discount_paise = promo_result.discount_paise.min(total_paise); // discount cannot exceed total
    let final_total_paise = total_paise - discount_paise;

    // ── Step D: Decrement stock for countable items (with race check) ─────────
    for item in &verified_items {
        if item.is_countable {
            let result = sqlx::query(
                "UPDATE cafe_items SET stock_quantity = stock_quantity - ?, updated_at = datetime('now')
                 WHERE id = ? AND is_countable = 1 AND stock_quantity >= ?",
            )
            .bind(item.quantity)
            .bind(&item.item_id)
            .bind(item.quantity)
            .execute(&mut *conn)
            .await
            .map_err(|e| {
                tracing::warn!("place_cafe_order: stock decrement error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "Stock update failed" })),
                )
            })?;

            if result.rows_affected() == 0 {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err((
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "error": "Stock changed during order, please retry"
                    })),
                ));
            }
        }
    }

    // ── Step E: Generate receipt number ──────────────────────────────────────
    let today_prefix = chrono::Utc::now().format("%Y%m%d").to_string();
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM cafe_orders WHERE receipt_number LIKE ?",
    )
    .bind(format!("RP-{}-%%", today_prefix))
    .fetch_one(&mut *conn)
    .await
    .map_err(|e| {
        tracing::warn!("place_cafe_order: receipt count error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Receipt generation failed" })),
        )
    })?;

    let receipt_number = format!("RP-{}-{:04}", today_prefix, count + 1);

    // ── Step F: Generate order_id ─────────────────────────────────────────────
    let order_id = Uuid::new_v4().to_string();

    // ── Step G: COMMIT transaction (stock decremented, receipt reserved) ──────
    sqlx::query("COMMIT")
        .execute(&mut *conn)
        .await
        .map_err(|e| {
            tracing::warn!("place_cafe_order: COMMIT failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Transaction commit failed" })),
            )
        })?;

    // Drop raw connection — pool is available again
    drop(conn);

    // ── Step H: Wallet debit (outside raw transaction, uses pool internally) ──
    let order_id_for_log = order_id.clone();
    let debit_result = crate::wallet::debit(
        state,
        &req.driver_id,
        final_total_paise,
        "cafe_order",
        Some(&order_id),
        Some(&format!("Cafe order {}", receipt_number)),
    )
    .await;

    let (new_balance, wallet_txn_id) = match debit_result {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("place_cafe_order: wallet debit failed for order {}: {}", order_id_for_log, e);
            // Compensating stock rollback (best-effort)
            let state_clone = state.clone();
            let items_clone = verified_items.clone();
            let oid = order_id_for_log.clone();
            tokio::spawn(async move {
                for item in &items_clone {
                    if item.is_countable {
                        let _ = sqlx::query(
                            "UPDATE cafe_items SET stock_quantity = stock_quantity + ? WHERE id = ?",
                        )
                        .bind(item.quantity)
                        .bind(&item.item_id)
                        .execute(&state_clone.db)
                        .await;
                    }
                }
                tracing::warn!("Rolled back stock for failed wallet debit on order {}", oid);
            });
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e })),
            ));
        }
    };

    // ── Step I: Insert order record ───────────────────────────────────────────
    let items_json = serde_json::to_string(&order_item_details).unwrap_or_else(|_| "[]".to_string());
    sqlx::query(
        "INSERT INTO cafe_orders (id, receipt_number, driver_id, items, total_paise, discount_paise, applied_promo_id, wallet_txn_id, status)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'confirmed')",
    )
    .bind(&order_id)
    .bind(&receipt_number)
    .bind(&req.driver_id)
    .bind(&items_json)
    .bind(final_total_paise)
    .bind(discount_paise)
    .bind(&promo_result.applied_promo_id)
    .bind(&wallet_txn_id)
    .execute(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!("place_cafe_order: order insert error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to record order" })),
        )
    })?;

    // ── Step J: Fire low-stock alerts (non-blocking) ──────────────────────────
    for item in &verified_items {
        if item.is_countable {
            crate::cafe_alerts::check_low_stock_alerts(&state.db, &state.config, &item.item_id).await;
        }
    }

    // ── Step L: Send WhatsApp receipt (fire-and-forget) ───────────────────────
    {
        let state_l = state.clone();
        let driver_id = req.driver_id.clone();
        let receipt_number_l = receipt_number.clone();
        let items_for_wa = order_item_details.clone();
        let total = final_total_paise;
        let balance = new_balance;
        tokio::spawn(async move {
            send_order_receipt_whatsapp(&state_l, &driver_id, &receipt_number_l, &items_for_wa, total, balance).await;
        });
    }

    // ── Step M: Print thermal receipt (fire-and-forget) ──────────────────────
    {
        let state_m = state.clone();
        let receipt_number_m = receipt_number.clone();
        let items_for_print = order_item_details.clone();
        let total = final_total_paise;
        // Fetch customer name best-effort — empty string is acceptable
        let customer_name = sqlx::query_scalar::<_, String>("SELECT COALESCE(name, '') FROM drivers WHERE id = ?")
            .bind(&req.driver_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten()
            .unwrap_or_default();
        tokio::spawn(async move {
            print_thermal_receipt(&state_m, &receipt_number_m, &items_for_print, total, &customer_name).await;
        });
    }

    // ── Step K: Return response ───────────────────────────────────────────────
    tracing::info!(
        "Cafe order placed: {} receipt={} driver={} gross={}p discount={}p final={}p promo={:?}",
        order_id,
        receipt_number,
        req.driver_id,
        total_paise,
        discount_paise,
        final_total_paise,
        promo_result.applied_promo_id
    );

    Ok(Json(serde_json::to_value(PlaceOrderResponse {
        order_id,
        receipt_number,
        wallet_txn_id,
        total_paise: final_total_paise,
        discount_paise,
        applied_promo_id: promo_result.applied_promo_id,
        applied_promo_name: promo_result.promo_name,
        new_balance_paise: new_balance,
        items: order_item_details,
    })
    .unwrap_or_default()))
}

/// Staff endpoint — driver_id is provided in request body.
pub async fn place_cafe_order(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PlaceOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    place_cafe_order_inner(&state, req).await
}

/// Customer endpoint — driver_id is extracted from Authorization JWT (prevents spoofing).
pub async fn place_cafe_order_customer(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Json(mut req): Json<PlaceOrderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract driver_id from JWT — ignore any driver_id in body
    let driver_id = crate::auth::verify_jwt(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or(""),
        &state.config.auth.jwt_secret,
    )
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e })),
        )
    })?;

    req.driver_id = driver_id;
    place_cafe_order_inner(&state, req).await
}

// ─── Post-Order Side Effects ──────────────────────────────────────────────────

/// Send a WhatsApp order confirmation receipt to the customer's phone.
/// Fire-and-forget: all errors are logged as warnings, never propagated.
async fn send_order_receipt_whatsapp(
    state: &Arc<AppState>,
    driver_id: &str,
    receipt_number: &str,
    items: &[OrderItemDetail],
    total_paise: i64,
    new_balance_paise: i64,
) {
    let config = &state.config;
    let db = &state.db;

    if !config.alerting.enabled {
        tracing::debug!(target: "cafe", "WA alerting disabled, skipping receipt for driver {}", driver_id);
        return;
    }

    // Fetch driver phone
    let phone_opt: Option<Option<String>> = sqlx::query_scalar("SELECT phone FROM drivers WHERE id = ?")
        .bind(driver_id)
        .fetch_optional(db)
        .await
        .ok();

    let phone = match phone_opt.flatten() {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            tracing::warn!(target: "cafe", "No phone for driver {}, skipping WA receipt", driver_id);
            return;
        }
    };

    let (evo_url, evo_key, evo_instance) = match (
        &config.auth.evolution_url,
        &config.auth.evolution_api_key,
        &config.auth.evolution_instance,
    ) {
        (Some(url), Some(key), Some(inst)) => (url, key, inst),
        _ => {
            tracing::warn!(target: "cafe", "Evolution API not configured, skipping WA receipt for {}", receipt_number);
            return;
        }
    };

    let ist = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string();

    let mut items_text = String::new();
    for item in items {
        items_text.push_str(&format!(
            "  {} x{}  Rs.{}\n",
            item.name,
            item.quantity,
            item.line_total_paise / 100
        ));
    }

    let message = format!(
        "[Racing Point Cafe] Order Confirmed!\nReceipt: {}\n{}\n\n{}
Total: Rs.{}\nBalance: Rs.{}\n\nThank you! Your order is being prepared.",
        receipt_number,
        ist,
        items_text,
        total_paise / 100,
        new_balance_paise / 100
    );

    let url = format!("{}/message/sendText/{}", evo_url, evo_instance);
    let body = serde_json::json!({ "number": phone, "text": message });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(target: "cafe", "Failed to build HTTP client for WA receipt: {}", e);
            return;
        }
    };

    match client
        .post(&url)
        .header("apikey", evo_key.as_str())
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!(target: "cafe", "WA receipt sent for order {} to driver {}", receipt_number, driver_id);
        }
        Ok(resp) => {
            tracing::warn!(target: "cafe", "Evolution API returned {} for WA receipt {}", resp.status(), receipt_number);
        }
        Err(e) => {
            tracing::warn!(target: "cafe", "WA receipt send failed for {}: {}", receipt_number, e);
        }
    }
}

/// Print a thermal receipt via a Node.js script (fire-and-forget).
/// Skipped silently if print_script_path is not configured.
async fn print_thermal_receipt(
    state: &Arc<AppState>,
    receipt_number: &str,
    items: &[OrderItemDetail],
    total_paise: i64,
    customer_name: &str,
) {
    let config = &state.config;
    let script_path = match &config.cafe.print_script_path {
        Some(p) => p.clone(),
        None => {
            tracing::debug!(target: "cafe", "Thermal print skipped: print_script_path not configured");
            return;
        }
    };

    let ist = chrono::Utc::now()
        .with_timezone(&chrono_tz::Asia::Kolkata)
        .format("%d %b %Y %H:%M IST")
        .to_string();

    let mut items_text = String::new();
    for item in items {
        items_text.push_str(&format!(
            "{}\n  {} x Rs.{} = Rs.{}\n",
            item.name,
            item.quantity,
            item.unit_price_paise / 100,
            item.line_total_paise / 100
        ));
    }

    let receipt_text = format!(
        "================================\n    RACING POINT CAFE\n================================\nReceipt: {}\n{}\nCustomer: {}\n--------------------------------\n{}--------------------------------\nTOTAL: Rs.{}\n================================\n     Thank you!\n================================",
        receipt_number,
        ist,
        customer_name,
        items_text,
        total_paise / 100
    );

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::process::Command::new("node")
            .arg(&script_path)
            .arg(&receipt_text)
            .kill_on_drop(true)
            .output(),
    )
    .await
    {
        Ok(Ok(output)) => {
            if output.status.success() {
                tracing::info!(target: "cafe", "Thermal receipt printed for {}", receipt_number);
            } else {
                tracing::warn!(
                    target: "cafe",
                    "Print script exited with non-zero status for {}: {}",
                    receipt_number,
                    output.status
                );
            }
        }
        Ok(Err(e)) => {
            tracing::warn!(target: "cafe", "Print script failed to launch for {}: {}", receipt_number, e);
        }
        Err(_) => {
            tracing::warn!(target: "cafe", "Thermal print timed out for {}", receipt_number);
        }
    }
}

/// GET /customer/cafe/orders/history
/// Returns the authenticated customer's cafe order history as JSON.
pub async fn list_customer_orders(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let driver_id = crate::auth::verify_jwt(
        headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or(""),
        &state.config.auth.jwt_secret,
    )
    .map_err(|e| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": e })),
        )
    })?;

    let rows: Vec<(String, String, String, i64, String, String)> = sqlx::query_as(
        "SELECT id, receipt_number, items, total_paise, status, created_at
         FROM cafe_orders
         WHERE driver_id = ?
         ORDER BY created_at DESC",
    )
    .bind(&driver_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| {
        tracing::warn!(target: "cafe", "list_customer_orders DB error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Failed to fetch orders" })),
        )
    })?;

    let orders: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|(id, receipt_number, items_json, total_paise, status, created_at)| {
            let items: Vec<OrderItemDetail> = serde_json::from_str(&items_json)
                .unwrap_or_else(|e| {
                    tracing::warn!(target: "cafe", "Failed to parse items for order {}: {}", id, e);
                    Vec::new()
                });
            serde_json::json!({
                "id": id,
                "receipt_number": receipt_number,
                "items": items,
                "total_paise": total_paise,
                "status": status,
                "created_at": created_at,
            })
        })
        .collect();

    Ok(Json(serde_json::json!({ "orders": orders })))
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
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
}
