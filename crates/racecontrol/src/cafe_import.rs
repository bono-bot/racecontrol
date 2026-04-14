//! Cafe import parsing — XLSX/CSV column detection, validation, and confirmation.
//!
//! Extracted from cafe.rs (Phase 385, v49.0 Architecture Completion).
//! Pure functions for import preview + DB confirmation handler.

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use axum::Json;
use axum::http::StatusCode;
use axum::extract::{Multipart, Path, State};
use uuid::Uuid;
use crate::state::AppState;

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
