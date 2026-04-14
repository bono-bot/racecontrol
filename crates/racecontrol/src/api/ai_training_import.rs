use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::state::AppState;

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
