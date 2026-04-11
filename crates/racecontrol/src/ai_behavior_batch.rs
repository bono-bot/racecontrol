//! # ai_behavior_batch.rs -- Phase 365 AI Behavior Validation via MMA
//!
//! GLD-E-01: AI lap time collector -- after session end, median AI lap times recorded to
//!           `ai_behavior_samples` keyed by (car, track, ai_level, difficulty_tier).
//! GLD-E-02: Weekly MMA batch job -- uses OpenRouter 5-model consensus to derive KB bands.
//! GLD-E-03: KB TOML files in `.planning/kb/ai-behavior/{car}-{track}.toml`.
//! GLD-E-04: Anomaly detector -- fires AiBehaviorAnomaly DashboardEvent on deviation.
//!
//! ## Design decisions
//! - D-01: ai_behavior_samples is a new table separate from laps (no is_ai column on laps).
//! - D-02: AI cars identified in AcResultEntry by driver_guid.is_empty().
//! - D-05: MMA = analytics batch (OpenRouter), NOT the Unified MMA Protocol Q1-Q4 incident gate.
//! - D-06: Weekly batch follows spawn_data_retention_job pattern (tokio::time::interval 604800s).
//! - D-17: ai_behavior_samples NOT synced to cloud (venue-specific data).

use std::collections::HashMap;
use std::sync::Arc;

use sqlx::SqlitePool;
use uuid::Uuid;

use crate::state::AppState;

// ─── Difficulty tier mapping (mirrors rc-agent DifficultyTier) ─────────────────

/// Map ai_level (0-100) to difficulty tier string.
/// Matches rc-agent/src/ac_launcher.rs DifficultyTier ranges exactly.
pub fn tier_for_level(ai_level: u32) -> &'static str {
    match ai_level {
        70..=79 => "rookie",
        80..=84 => "amateur",
        85..=89 => "semi_pro",
        90..=95 => "pro",
        96..=100 => "alien",
        _ => "amateur", // fallback for out-of-range (87 default = semi_pro midpoint)
    }
}

// ─── AI sample data ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AiLapSample {
    pub session_id: String,
    pub pod_id: String,
    pub sim_type: String,
    pub car: String,
    pub track: String,
    pub ai_level: u32,
    pub lap_times_ms: Vec<i64>, // all valid AI lap times for this session
}

impl AiLapSample {
    /// Compute median lap time. Returns None if no laps.
    pub fn median_lap_ms(&self) -> Option<i64> {
        if self.lap_times_ms.is_empty() {
            return None;
        }
        let mut sorted = self.lap_times_ms.clone();
        sorted.sort_unstable();
        let mid = sorted.len() / 2;
        if sorted.len() % 2 == 0 {
            Some((sorted[mid - 1] + sorted[mid]) / 2)
        } else {
            Some(sorted[mid])
        }
    }

    /// Compute p25 percentile. Returns None if fewer than 4 laps.
    pub fn p25_lap_ms(&self) -> Option<i64> {
        if self.lap_times_ms.len() < 4 {
            return None;
        }
        let mut sorted = self.lap_times_ms.clone();
        sorted.sort_unstable();
        Some(sorted[sorted.len() / 4])
    }

    /// Compute p75 percentile. Returns None if fewer than 4 laps.
    pub fn p75_lap_ms(&self) -> Option<i64> {
        if self.lap_times_ms.len() < 4 {
            return None;
        }
        let mut sorted = self.lap_times_ms.clone();
        sorted.sort_unstable();
        Some(sorted[sorted.len() * 3 / 4])
    }
}

// ─── Collector (GLD-E-01) ─────────────────────────────────────────────────────

/// Called at session end (from collect_results in ac_server.rs).
/// Inserts one row per (car, track, ai_level) tuple where AI lap_count >= 3.
///
/// `results` is the Vec<MultiplayerResult> from parse_ac_results().
/// AI cars are identified by `guid.is_empty()`.
/// `ai_level` is extracted from sessions.config_json for this session.
pub async fn collect_ai_behavior_samples(
    db: &SqlitePool,
    session_id: &str,
    pod_id: &str,
    car: &str,
    track: &str,
    ai_level: u32,
    results: &[crate::ac_server::MultiplayerResult],
    flags: &HashMap<String, crate::flags::FeatureFlagRow>,
) {
    // Feature flag kill-switch
    if let Some(flag) = flags.get("phase365_mma_batch") {
        if !flag.enabled {
            return;
        }
    }

    // Collect AI lap times: AI cars have empty guid AND best_lap > 0 AND laps_completed >= 3
    let ai_laps: Vec<i64> = results
        .iter()
        .filter(|e| e.guid.is_empty() && e.best_lap_ms.unwrap_or(0) > 0 && e.laps_completed >= 3)
        .filter_map(|e| e.best_lap_ms)
        .collect();

    if ai_laps.is_empty() {
        tracing::debug!(
            session_id = session_id,
            "ai_behavior_collector: no AI cars with >=3 laps, skipping"
        );
        return;
    }

    let sample = AiLapSample {
        session_id: session_id.to_string(),
        pod_id: pod_id.to_string(),
        sim_type: "assettocorsa".to_string(),
        car: car.to_string(),
        track: track.to_string(),
        ai_level,
        lap_times_ms: ai_laps,
    };

    let median = match sample.median_lap_ms() {
        Some(m) => m,
        None => return,
    };

    let tier = tier_for_level(ai_level);
    let id = Uuid::new_v4().to_string();

    let result = sqlx::query(
        "INSERT INTO ai_behavior_samples \
         (id, session_id, pod_id, sim_type, car, track, ai_level, difficulty_tier, \
          lap_count, median_lap_ms, p25_lap_ms, p75_lap_ms) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&sample.session_id)
    .bind(&sample.pod_id)
    .bind(&sample.sim_type)
    .bind(&sample.car)
    .bind(&sample.track)
    .bind(sample.ai_level as i64)
    .bind(tier)
    .bind(sample.lap_times_ms.len() as i64)
    .bind(median)
    .bind(sample.p25_lap_ms())
    .bind(sample.p75_lap_ms())
    .execute(db)
    .await;

    match result {
        Ok(_) => tracing::info!(
            session_id = session_id,
            car = car,
            track = track,
            ai_level = ai_level,
            tier = tier,
            median_ms = median,
            "ai_behavior_samples: row inserted"
        ),
        Err(e) => tracing::warn!(
            session_id = session_id,
            "ai_behavior_samples: insert failed: {}",
            e
        ),
    }
}

// ─── KB Band types ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TierBand {
    pub p10_ms: i64,
    pub p50_ms: i64,
    pub p90_ms: i64,
    pub consensus_models: u32,
    pub samples_used: u32,
}

#[derive(Debug, Clone)]
pub struct KbEntry {
    pub car: String,
    pub track: String,
    pub bands: HashMap<String, TierBand>, // tier_name -> band
    pub batch_id: String,
    pub updated_at: String,
}

impl KbEntry {
    /// Serialize to TOML string.
    pub fn to_toml_string(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# Auto-generated by Phase 365 MMA batch. Do not edit manually.\n\
             # Last updated: {}  batch_id = \"{}\"\n\n",
            self.updated_at, self.batch_id
        ));
        // Sort tiers for deterministic output
        let mut tiers: Vec<(&String, &TierBand)> = self.bands.iter().collect();
        tiers.sort_by_key(|(k, _)| k.as_str());
        for (tier, band) in tiers {
            out.push_str(&format!(
                "[{}]\np10_ms = {}\np50_ms = {}\np90_ms = {}\nconsensus_models = {}\nsamples_used = {}\n\n",
                tier, band.p10_ms, band.p50_ms, band.p90_ms,
                band.consensus_models, band.samples_used
            ));
        }
        out
    }

    /// Derive the KB file path for this entry.
    /// car and track are slugified: lowercase, spaces -> dashes, non-alphanum stripped.
    pub fn file_path(&self) -> std::path::PathBuf {
        let car_slug = slugify(&self.car);
        let track_slug = slugify(&self.track);
        std::path::PathBuf::from(format!(
            ".planning/kb/ai-behavior/{}-{}.toml",
            car_slug, track_slug
        ))
    }
}

/// Slugify a string: lowercase, spaces/underscores -> dashes, strip non-alphanum-dash.
pub fn slugify(s: &str) -> String {
    s.to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ─── MMA consensus (GLD-E-02) ─────────────────────────────────────────────────

/// Response from a single model for one (car, track, tier) query.
#[derive(Debug, Clone)]
pub struct ModelBandResponse {
    pub p10_ms: i64,
    pub p50_ms: i64,
    pub p90_ms: i64,
}

/// Check if two band responses agree within 5% tolerance on p50.
pub fn bands_agree(a: &ModelBandResponse, b: &ModelBandResponse) -> bool {
    let diff = (a.p50_ms - b.p50_ms).abs();
    let threshold = (a.p50_ms.max(b.p50_ms) as f64 * 0.05) as i64;
    diff <= threshold
}

/// Compute consensus from a list of model responses.
/// Returns Some(consensus band) if >= 3 models agree within 5% on p50.
/// Returns None if fewer than 3 models agree.
pub fn compute_consensus(responses: &[ModelBandResponse]) -> Option<TierBand> {
    if responses.len() < 3 {
        return None;
    }

    // Find the largest group where all members agree with each other within 5%
    let n = responses.len();
    let mut best_group: Vec<usize> = Vec::new();

    for i in 0..n {
        let group: Vec<usize> = (0..n)
            .filter(|&j| bands_agree(&responses[i], &responses[j]))
            .collect();
        if group.len() > best_group.len() {
            best_group = group;
        }
    }

    if best_group.len() < 3 {
        return None;
    }

    // Average the agreeing responses
    let count = best_group.len() as i64;
    let p10 = best_group.iter().map(|&i| responses[i].p10_ms).sum::<i64>() / count;
    let p50 = best_group.iter().map(|&i| responses[i].p50_ms).sum::<i64>() / count;
    let p90 = best_group.iter().map(|&i| responses[i].p90_ms).sum::<i64>() / count;

    Some(TierBand {
        p10_ms: p10,
        p50_ms: p50,
        p90_ms: p90,
        consensus_models: best_group.len() as u32,
        samples_used: 0, // filled by caller
    })
}

// ─── KB reader (GLD-E-04) ─────────────────────────────────────────────────────

/// Read KB TOML file for a (car, track) pair. Returns None if file doesn't exist.
pub fn read_kb_entry(car: &str, track: &str) -> Option<KbEntry> {
    let car_slug = slugify(car);
    let track_slug = slugify(track);
    let path = std::path::PathBuf::from(format!(
        ".planning/kb/ai-behavior/{}-{}.toml",
        car_slug, track_slug
    ));

    let content = std::fs::read_to_string(&path).ok()?;

    // Parse TOML
    let parsed: toml::Value = content.parse().ok()?;
    let table = parsed.as_table()?;

    let mut bands = HashMap::new();
    for (tier, val) in table {
        if let Some(tier_table) = val.as_table() {
            let p10 = tier_table.get("p10_ms").and_then(|v| v.as_integer())?;
            let p50 = tier_table.get("p50_ms").and_then(|v| v.as_integer())?;
            let p90 = tier_table.get("p90_ms").and_then(|v| v.as_integer())?;
            let consensus = tier_table
                .get("consensus_models")
                .and_then(|v| v.as_integer())
                .unwrap_or(3) as u32;
            let samples = tier_table
                .get("samples_used")
                .and_then(|v| v.as_integer())
                .unwrap_or(0) as u32;
            bands.insert(
                tier.clone(),
                TierBand {
                    p10_ms: p10,
                    p50_ms: p50,
                    p90_ms: p90,
                    consensus_models: consensus,
                    samples_used: samples,
                },
            );
        }
    }

    Some(KbEntry {
        car: car.to_string(),
        track: track.to_string(),
        bands,
        batch_id: String::new(),
        updated_at: String::new(),
    })
}

// ─── Anomaly detection (GLD-E-04) ─────────────────────────────────────────────

/// Anomaly check direction.
#[derive(Debug, PartialEq)]
pub enum AnomalyDirection {
    TooSlow,
    TooFast,
    None,
}

/// Check if median_ms falls outside the p10-p90 band for the given tier.
/// Returns AnomalyDirection::None if within band or if KB has no entry for this tier.
pub fn check_anomaly(kb: &KbEntry, tier: &str, median_ms: i64) -> (AnomalyDirection, Option<TierBand>) {
    let band = match kb.bands.get(tier) {
        Some(b) => b.clone(),
        None => return (AnomalyDirection::None, None),
    };

    if median_ms < band.p10_ms {
        (AnomalyDirection::TooFast, Some(band))
    } else if median_ms > band.p90_ms {
        (AnomalyDirection::TooSlow, Some(band))
    } else {
        (AnomalyDirection::None, Some(band))
    }
}

/// Called at session end alongside collect_ai_behavior_samples.
/// Reads KB, checks anomaly, broadcasts DashboardEvent::AiBehaviorAnomaly if deviation found.
pub async fn check_and_broadcast_anomaly(
    state: &Arc<AppState>,
    session_id: &str,
    pod_id: &str,
    car: &str,
    track: &str,
    ai_level: u32,
    median_lap_ms: i64,
    observed_lap_count: u32,
) {
    // Feature flag check
    let enabled = {
        let flags = state.feature_flags.read().await;
        flags
            .get("phase365_anomaly_detection")
            .map(|f| f.enabled)
            .unwrap_or(true)
    };
    if !enabled {
        tracing::debug!("phase365_anomaly_detection disabled -- skipping anomaly check");
        return;
    }

    // Read KB
    let kb = match read_kb_entry(car, track) {
        Some(k) => k,
        None => {
            tracing::debug!(
                car = car,
                track = track,
                "AiBehaviorAnomaly: no KB file for this car+track, skipping check"
            );
            return;
        }
    };

    let tier = tier_for_level(ai_level);
    let (direction, band) = check_anomaly(&kb, tier, median_lap_ms);

    if direction == AnomalyDirection::None {
        tracing::debug!(
            car = car,
            track = track,
            tier = tier,
            median_ms = median_lap_ms,
            "AI behavior within expected band"
        );
        return;
    }

    let band = match band {
        Some(b) => b,
        None => return,
    };

    let direction_str = match direction {
        AnomalyDirection::TooSlow => "too_slow",
        AnomalyDirection::TooFast => "too_fast",
        AnomalyDirection::None => return,
    };

    tracing::warn!(
        pod_id = pod_id,
        session_id = session_id,
        car = car,
        track = track,
        tier = tier,
        median_ms = median_lap_ms,
        expected_p10 = band.p10_ms,
        expected_p90 = band.p90_ms,
        direction = direction_str,
        "AiBehaviorAnomaly detected"
    );

    let _ = state.dashboard_tx.send(
        rc_common::protocol::DashboardEvent::AiBehaviorAnomaly {
            pod_id: pod_id.to_string(),
            session_id: session_id.to_string(),
            car: car.to_string(),
            track: track.to_string(),
            difficulty_tier: tier.to_string(),
            expected_p10_ms: band.p10_ms,
            expected_p90_ms: band.p90_ms,
            observed_median_ms: median_lap_ms,
            observed_lap_count,
            direction: direction_str.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        },
    );
}

// ─── OpenRouter MMA batch (GLD-E-02, GLD-E-03) ───────────────────────────────

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

/// Read OpenRouter API key: OPENROUTER_KEY env var first, then data/openrouter-mma-key.txt.
fn read_openrouter_key() -> Option<String> {
    if let Ok(k) = std::env::var("OPENROUTER_KEY") {
        if !k.is_empty() {
            return Some(k);
        }
    }
    let path = std::path::Path::new("data/openrouter-mma-key.txt");
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

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

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_for_level_all_tiers() {
        assert_eq!(tier_for_level(70), "rookie");
        assert_eq!(tier_for_level(79), "rookie");
        assert_eq!(tier_for_level(80), "amateur");
        assert_eq!(tier_for_level(84), "amateur");
        assert_eq!(tier_for_level(85), "semi_pro");
        assert_eq!(tier_for_level(87), "semi_pro"); // default midpoint
        assert_eq!(tier_for_level(89), "semi_pro");
        assert_eq!(tier_for_level(90), "pro");
        assert_eq!(tier_for_level(93), "pro"); // pro midpoint
        assert_eq!(tier_for_level(95), "pro");
        assert_eq!(tier_for_level(96), "alien");
        assert_eq!(tier_for_level(100), "alien");
    }

    #[test]
    fn test_tier_for_level_fallback() {
        // Out-of-range values fall back to amateur
        assert_eq!(tier_for_level(0), "amateur");
        assert_eq!(tier_for_level(50), "amateur");
        assert_eq!(tier_for_level(69), "amateur");
    }

    #[test]
    fn test_median_odd() {
        let s = AiLapSample {
            session_id: "s1".into(),
            pod_id: "p1".into(),
            sim_type: "ac".into(),
            car: "tatuus".into(),
            track: "magione".into(),
            ai_level: 87,
            lap_times_ms: vec![100000, 95000, 98000],
        };
        assert_eq!(s.median_lap_ms(), Some(98000));
    }

    #[test]
    fn test_median_even() {
        let s = AiLapSample {
            session_id: "s1".into(),
            pod_id: "p1".into(),
            sim_type: "ac".into(),
            car: "tatuus".into(),
            track: "magione".into(),
            ai_level: 87,
            lap_times_ms: vec![100000, 95000, 98000, 97000],
        };
        // sorted: [95000, 97000, 98000, 100000], mid avg = (97000+98000)/2 = 97500
        assert_eq!(s.median_lap_ms(), Some(97500));
    }

    #[test]
    fn test_median_empty() {
        let s = AiLapSample {
            session_id: "s1".into(),
            pod_id: "p1".into(),
            sim_type: "ac".into(),
            car: "tatuus".into(),
            track: "magione".into(),
            ai_level: 87,
            lap_times_ms: vec![],
        };
        assert_eq!(s.median_lap_ms(), None);
    }

    #[test]
    fn test_ai_car_detection_via_empty_guid() {
        let human_guid = "76561198000000001";
        let ai_guid = "";
        assert!(!human_guid.is_empty(), "Human has non-empty guid");
        assert!(ai_guid.is_empty(), "AI has empty guid");
    }

    #[test]
    fn test_consensus_3_of_5_agree() {
        // 3 models agree on ~90000ms p50, 2 outliers
        let responses = vec![
            ModelBandResponse {
                p10_ms: 85000,
                p50_ms: 90000,
                p90_ms: 97000,
            },
            ModelBandResponse {
                p10_ms: 84000,
                p50_ms: 91000,
                p90_ms: 98000,
            }, // agrees (1% off)
            ModelBandResponse {
                p10_ms: 85500,
                p50_ms: 90500,
                p90_ms: 97500,
            }, // agrees (<1%)
            ModelBandResponse {
                p10_ms: 75000,
                p50_ms: 80000,
                p90_ms: 86000,
            }, // outlier
            ModelBandResponse {
                p10_ms: 100000,
                p50_ms: 110000,
                p90_ms: 120000,
            }, // outlier
        ];
        let result = compute_consensus(&responses);
        assert!(
            result.is_some(),
            "3 agreeing models should produce consensus"
        );
        assert!(result.as_ref().map_or(false, |r| r.consensus_models >= 3));
    }

    #[test]
    fn test_consensus_2_of_5_no_consensus() {
        // All 5 models disagree significantly
        let responses = vec![
            ModelBandResponse {
                p10_ms: 80000,
                p50_ms: 85000,
                p90_ms: 90000,
            },
            ModelBandResponse {
                p10_ms: 90000,
                p50_ms: 95000,
                p90_ms: 100000,
            },
            ModelBandResponse {
                p10_ms: 100000,
                p50_ms: 105000,
                p90_ms: 110000,
            },
            ModelBandResponse {
                p10_ms: 70000,
                p50_ms: 75000,
                p90_ms: 80000,
            },
            ModelBandResponse {
                p10_ms: 110000,
                p50_ms: 115000,
                p90_ms: 120000,
            },
        ];
        let result = compute_consensus(&responses);
        assert!(
            result.is_none(),
            "5 disagreeing models should produce no consensus"
        );
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Assetto Corsa"), "assetto-corsa");
        assert_eq!(slugify("tatuusFA1"), "tatuusfa1");
        assert_eq!(slugify("magione"), "magione");
        assert_eq!(slugify("Red Bull Ring GP"), "red-bull-ring-gp");
    }

    #[test]
    fn test_toml_output_format() {
        let mut bands = HashMap::new();
        bands.insert(
            "semi_pro".to_string(),
            TierBand {
                p10_ms: 85000,
                p50_ms: 90000,
                p90_ms: 97000,
                consensus_models: 3,
                samples_used: 42,
            },
        );
        let entry = KbEntry {
            car: "tatuus".into(),
            track: "magione".into(),
            bands,
            batch_id: "test-batch".into(),
            updated_at: "2026-04-10T12:00:00Z".into(),
        };
        let toml = entry.to_toml_string();
        assert!(
            toml.contains("[semi_pro]"),
            "TOML must contain [semi_pro] section"
        );
        assert!(toml.contains("p10_ms = 85000"), "Must contain p10_ms");
        assert!(toml.contains("p90_ms = 97000"), "Must contain p90_ms");
        assert!(
            toml.contains("consensus_models = 3"),
            "Must contain consensus count"
        );
    }

    #[test]
    fn test_anomaly_too_slow() {
        let mut bands = HashMap::new();
        bands.insert(
            "semi_pro".to_string(),
            TierBand {
                p10_ms: 85000,
                p50_ms: 90000,
                p90_ms: 97000,
                consensus_models: 3,
                samples_used: 10,
            },
        );
        let kb = KbEntry {
            car: "tatuus".into(),
            track: "magione".into(),
            bands,
            batch_id: "b1".into(),
            updated_at: "now".into(),
        };
        let (dir, band) = check_anomaly(&kb, "semi_pro", 110000);
        assert_eq!(dir, AnomalyDirection::TooSlow);
        assert!(band.is_some());
    }

    #[test]
    fn test_anomaly_too_fast() {
        let mut bands = HashMap::new();
        bands.insert(
            "semi_pro".to_string(),
            TierBand {
                p10_ms: 85000,
                p50_ms: 90000,
                p90_ms: 97000,
                consensus_models: 3,
                samples_used: 10,
            },
        );
        let kb = KbEntry {
            car: "tatuus".into(),
            track: "magione".into(),
            bands,
            batch_id: "b1".into(),
            updated_at: "now".into(),
        };
        let (dir, _) = check_anomaly(&kb, "semi_pro", 70000);
        assert_eq!(dir, AnomalyDirection::TooFast);
    }

    #[test]
    fn test_no_anomaly_within_band() {
        let mut bands = HashMap::new();
        bands.insert(
            "semi_pro".to_string(),
            TierBand {
                p10_ms: 85000,
                p50_ms: 90000,
                p90_ms: 97000,
                consensus_models: 3,
                samples_used: 10,
            },
        );
        let kb = KbEntry {
            car: "tatuus".into(),
            track: "magione".into(),
            bands,
            batch_id: "b1".into(),
            updated_at: "now".into(),
        };
        let (dir, _) = check_anomaly(&kb, "semi_pro", 90000);
        assert_eq!(dir, AnomalyDirection::None);
    }

    #[test]
    fn test_no_kb_no_anomaly() {
        let kb = KbEntry {
            car: "tatuus".into(),
            track: "magione".into(),
            bands: HashMap::new(), // empty -- no KB for this tier
            batch_id: "b1".into(),
            updated_at: "now".into(),
        };
        let (dir, band) = check_anomaly(&kb, "alien", 90000);
        assert_eq!(dir, AnomalyDirection::None, "No KB entry = no anomaly");
        assert!(band.is_none());
    }
}
