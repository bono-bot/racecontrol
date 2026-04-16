//! MMA batch processing for AI behavior validation (GLD-E-02, GLD-E-03).
//!
//! Weekly OpenRouter multi-model consensus job that derives KB bands from
//! collected AI lap time samples.  Extracted from ai_behavior_batch.rs.

use std::collections::HashMap;
use std::sync::Arc;

use uuid::Uuid;

use crate::state::AppState;
use super::{compute_consensus, KbEntry, ModelBandResponse};

// ─── Constants ───────────────────────────────────────────────────────────────

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";
const MMA_MODELS: &[&str] = &[
    "anthropic/claude-3.5-sonnet",
    "openai/gpt-4o",
    "google/gemini-1.5-pro",
    "mistralai/mistral-large",
    "deepseek/deepseek-chat",
];
const MAX_TUPLES_PER_BATCH: usize = 20;
const MIN_SAMPLES_PER_TUPLE: i64 = 10;

// ─── OpenRouter key reader ───────────────────────────────────────────────────

/// Read OpenRouter API key: OPENROUTER_KEY env var first, then data/openrouter-mma-key.txt.
fn read_openrouter_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENROUTER_KEY")
        && !k.is_empty() {
            return Some(k);
        }
    let path = std::path::Path::new("data/openrouter-mma-key.txt");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

// ─── Model query ─────────────────────────────────────────────────────────────

/// Query one model for the expected lap band for a (car, track, tier, samples) tuple.
/// Returns None on HTTP error or parse failure.
async fn query_model_for_band(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    car: &str,
    track: &str,
    tier: &str,
    sample_medians: &[i64],
) -> Option<ModelBandResponse> {
    let samples_str = sample_medians
        .iter()
        .map(|ms| format!("{}ms", ms))
        .collect::<Vec<_>>()
        .join(", ");

    let prompt = format!(
        "You are analyzing Assetto Corsa AI lap times for difficulty validation.\n\
         Car: {car}\nTrack: {track}\nDifficulty tier: {tier}\n\
         Recent AI median lap times: {samples}\n\n\
         Based on these samples, what are the expected p10, p50, and p90 lap time bands \
         for {tier} AI on this car+track combination?\n\
         Reply in EXACTLY this JSON format, no other text:\n\
         {{\"p10_ms\": <integer>, \"p50_ms\": <integer>, \"p90_ms\": <integer>}}",
        car = car,
        track = track,
        tier = tier,
        samples = samples_str
    );

    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 100,
        "temperature": 0.1
    });

    let resp = client
        .post(OPENROUTER_API_URL)
        .bearer_auth(api_key)
        .header("HTTP-Referer", "https://racingpoint.in")
        .header("X-Title", "RacingPoint AI Behavior Batch")
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        tracing::warn!("OpenRouter model {} returned HTTP {}", model, resp.status());
        return None;
    }

    let json: serde_json::Value = resp.json().await.ok()?;
    let content = json["choices"][0]["message"]["content"].as_str()?;

    // Parse the JSON response
    let parsed: serde_json::Value = serde_json::from_str(content.trim()).ok()?;
    let p10 = parsed["p10_ms"].as_i64()?;
    let p50 = parsed["p50_ms"].as_i64()?;
    let p90 = parsed["p90_ms"].as_i64()?;

    // Sanity check: p10 < p50 < p90, all > 0, all < 10 minutes
    if p10 <= 0 || p50 <= p10 || p90 <= p50 || p90 > 600_000 {
        tracing::warn!(
            "Model {} returned invalid band: p10={} p50={} p90={}",
            model,
            p10,
            p50,
            p90
        );
        return None;
    }

    Some(ModelBandResponse {
        p10_ms: p10,
        p50_ms: p50,
        p90_ms: p90,
    })
}

// ─── KB writer ───────────────────────────────────────────────────────────────

/// Write a KbEntry to its TOML file. Creates the directory if needed.
pub fn write_kb_entry(entry: &KbEntry) -> std::io::Result<()> {
    let path = entry.file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, entry.to_toml_string())?;
    tracing::info!(
        car = %entry.car,
        track = %entry.track,
        batch_id = %entry.batch_id,
        "KB TOML written: {:?}",
        path
    );
    Ok(())
}

// ─── Batch cycle ─────────────────────────────────────────────────────────────

/// Run a single MMA batch cycle. Called weekly by spawn_ai_behavior_batch.
pub async fn run_ai_behavior_batch_cycle(state: Arc<AppState>) {
    // Feature flag check
    let enabled = {
        let flags = state.feature_flags.read().await;
        flags
            .get("phase365_mma_batch")
            .map(|f| f.enabled)
            .unwrap_or(true)
    };
    if !enabled {
        tracing::debug!("phase365_mma_batch feature flag disabled -- skipping batch");
        return;
    }

    let api_key = match read_openrouter_key() {
        Some(k) => k,
        None => {
            tracing::warn!("phase365_mma_batch: no OpenRouter key found, skipping batch");
            return;
        }
    };

    // Query distinct (car, track, difficulty_tier) tuples with >= MIN_SAMPLES rows
    // from the last 30 days, limited to MAX_TUPLES_PER_BATCH
    let tuples: Vec<(String, String, String, i64)> = sqlx::query_as(
        "SELECT car, track, difficulty_tier, COUNT(*) as sample_count \
         FROM ai_behavior_samples \
         WHERE sampled_at > datetime('now', '-30 days') \
         GROUP BY car, track, difficulty_tier \
         HAVING COUNT(*) >= ? \
         ORDER BY sample_count DESC \
         LIMIT ?",
    )
    .bind(MIN_SAMPLES_PER_TUPLE)
    .bind(MAX_TUPLES_PER_BATCH as i64)
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    if tuples.is_empty() {
        tracing::info!(
            "phase365_mma_batch: no tuples with >= {} samples, nothing to update",
            MIN_SAMPLES_PER_TUPLE
        );
        return;
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .unwrap_or_default();

    let batch_id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

    // Group tuples by (car, track) to build one TOML file per pair
    let mut kb_map: HashMap<(String, String), KbEntry> = HashMap::new();

    for (car, track, tier, sample_count) in &tuples {
        // Get sample medians for this tuple (last 30 days)
        let medians: Vec<(i64,)> = sqlx::query_as(
            "SELECT median_lap_ms FROM ai_behavior_samples \
             WHERE car = ? AND track = ? AND difficulty_tier = ? \
             AND sampled_at > datetime('now', '-30 days') \
             ORDER BY sampled_at DESC LIMIT 50",
        )
        .bind(car)
        .bind(track)
        .bind(tier)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let sample_ms: Vec<i64> = medians.iter().map(|(m,)| *m).collect();

        // Query all 5 models
        let mut responses = Vec::new();
        for model in MMA_MODELS {
            if let Some(resp) =
                query_model_for_band(&client, &api_key, model, car, track, tier, &sample_ms).await
            {
                responses.push(resp);
            }
        }

        // Consensus check
        if let Some(mut band) = compute_consensus(&responses) {
            band.samples_used = *sample_count as u32;
            let key = (car.clone(), track.clone());
            let entry = kb_map.entry(key).or_insert_with(|| KbEntry {
                car: car.clone(),
                track: track.clone(),
                bands: HashMap::new(),
                batch_id: batch_id.clone(),
                updated_at: now.clone(),
            });
            entry.bands.insert(tier.clone(), band);
            tracing::info!(
                car = %car,
                track = %track,
                tier = %tier,
                models_agreed = responses.len(),
                "MMA consensus reached for tier band"
            );
        } else {
            tracing::info!(
                car = %car,
                track = %track,
                tier = %tier,
                models_responded = responses.len(),
                "MMA: no consensus for tier band (< 3 models agreed)"
            );
        }
    }

    // Write KB TOML files
    for ((car, track), entry) in &kb_map {
        if entry.bands.is_empty() {
            continue;
        }
        if let Err(e) = write_kb_entry(entry) {
            tracing::warn!(car = %car, track = %track, "Failed to write KB TOML: {}", e);
        }
    }

    tracing::info!(
        batch_id = %batch_id,
        tuples_processed = tuples.len(),
        kb_files_written = kb_map.len(),
        "phase365 MMA batch complete"
    );
}

/// Spawn the weekly MMA batch background task. Called from main.rs.
pub async fn spawn_ai_behavior_batch(state: Arc<AppState>) {
    tracing::info!("phase365: ai-behavior-batch task started (604800s interval, 3600s initial delay)");
    // Initial delay: 1 hour -- avoid boot congestion
    tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(604800)); // 7 days
    loop {
        interval.tick().await;
        run_ai_behavior_batch_cycle(state.clone()).await;
    }
}
