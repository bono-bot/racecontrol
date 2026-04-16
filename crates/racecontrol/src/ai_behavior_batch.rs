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
        if sorted.len().is_multiple_of(2) {
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
    if let Some(flag) = flags.get("phase365_mma_batch")
        && !flag.enabled {
            return;
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

// ─── MMA batch submodule (GLD-E-02, GLD-E-03) ───────────────────────────────

#[path = "ai_behavior_batch_mma.rs"]
mod ai_behavior_batch_mma;
pub use ai_behavior_batch_mma::{
    run_ai_behavior_batch_cycle, spawn_ai_behavior_batch, write_kb_entry,
};

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "ai_behavior_batch_tests.rs"]
mod tests;
