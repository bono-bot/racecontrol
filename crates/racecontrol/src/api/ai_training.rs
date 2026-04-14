//! AI Training Management handlers — extracted from ai_routes.rs for <500 line compliance.

#![allow(unused_imports)]
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

/// GET /ai/training/stats — training pair counts, avg quality, top keywords.
pub(crate) async fn ai_training_stats(
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let db = &state.db;

    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ai_training_pairs")
        .fetch_one(db).await.unwrap_or(0);

    let avg_quality: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(quality_score), 0.0) FROM ai_training_pairs"
    ).fetch_one(db).await.unwrap_or(0.0);

    let by_source = sqlx::query_as::<_, (String, i64)>(
        "SELECT source, COUNT(*) as cnt FROM ai_training_pairs GROUP BY source ORDER BY cnt DESC"
    ).fetch_all(db).await.unwrap_or_default();

    let top_used = sqlx::query_as::<_, (String, i64)>(
        "SELECT query_text, use_count FROM ai_training_pairs ORDER BY use_count DESC LIMIT 10"
    ).fetch_all(db).await.unwrap_or_default();

    Json(json!({
        "total": total,
        "avg_quality_score": (avg_quality * 100.0).round() / 100.0,
        "by_source": by_source.iter().map(|(s, c)| json!({"source": s, "count": c})).collect::<Vec<_>>(),
        "top_used": top_used.iter().map(|(q, u)| json!({"query": q, "use_count": u})).collect::<Vec<_>>(),
    }))
}

/// GET /ai/training/pairs — paginated list for review.
pub(crate) async fn ai_training_pairs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit: i64 = params.get("limit").and_then(|v| v.parse().ok()).unwrap_or(20);
    let offset: i64 = params.get("offset").and_then(|v| v.parse().ok()).unwrap_or(0);
    let source_filter = params.get("source");

    let (pairs, total) = if let Some(src) = source_filter {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs WHERE source = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(src).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs WHERE source = ?"
        ).bind(src).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    } else {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, i64, i64, String)>(
            "SELECT id, query_text, response_text, source, model, quality_score, use_count, created_at \
             FROM ai_training_pairs ORDER BY created_at DESC LIMIT ? OFFSET ?",
        ).bind(limit).bind(offset).fetch_all(&state.db).await.unwrap_or_default();

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM ai_training_pairs"
        ).fetch_one(&state.db).await.unwrap_or(0);

        (rows, total)
    };

    Json(json!({
        "total": total,
        "limit": limit,
        "offset": offset,
        "pairs": pairs.iter().map(|(id, q, r, src, model, quality, use_count, created)| json!({
            "id": id,
            "query": q,
            "response": r,
            "source": src,
            "model": model,
            "quality_score": quality,
            "use_count": use_count,
            "created_at": created,
        })).collect::<Vec<_>>(),
    }))
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TrainingImportItem {
    query: String,
    response: String,
    #[serde(default = "default_source")]
    source: String,
    #[serde(default = "default_quality")]
    quality_score: i64,
}
pub(crate) fn default_source() -> String { "import".to_string() }
pub(crate) fn default_quality() -> i64 { 1 }

/// POST /ai/training/import — bulk import training pairs.
pub(crate) async fn ai_training_import(
    State(state): State<Arc<AppState>>,
    Json(pairs): Json<Vec<TrainingImportItem>>,
) -> Json<Value> {
    let mut inserted = 0u32;
    let mut skipped = 0u32;

    for item in &pairs {
        // Reuse the same log_training_pair logic but with quality_score support
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        item.query.hash(&mut hasher);
        let qhash = format!("{:x}", hasher.finish());

        let keywords = crate::ai::extract_keywords_pub(&item.query);
        let id = uuid::Uuid::new_v4().to_string();

        let result = sqlx::query(
            "INSERT INTO ai_training_pairs \
             (id, query_hash, query_text, query_keywords, response_text, source, model, quality_score) \
             SELECT ?, ?, ?, ?, ?, ?, 'import', ? \
             WHERE NOT EXISTS (SELECT 1 FROM ai_training_pairs WHERE query_hash = ?)",
        )
        .bind(&id)
        .bind(&qhash)
        .bind(&item.query)
        .bind(&keywords)
        .bind(&item.response)
        .bind(&item.source)
        .bind(item.quality_score)
        .bind(&qhash)
        .execute(&state.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => inserted += 1,
            _ => skipped += 1,
        }
    }

    Json(json!({
        "imported": inserted,
        "skipped": skipped,
        "total_submitted": pairs.len(),
    }))
}
