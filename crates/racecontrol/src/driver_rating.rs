//! Driver skill rating system — computes composite ratings from lap data.
//!
//! After each completed lap (via `persist_lap()`), a `RatingRequest` is sent to a
//! background worker that recomputes the driver's skill rating for that sim_type.
//!
//! Rating components:
//! - **Pace** (50%): How close the driver's best lap is to the track record.
//! - **Consistency** (30%): Coefficient of variation of the last 10 valid laps.
//! - **Experience** (20%): Log-scaled total lap count.

use serde::Serialize;
use sqlx::SqlitePool;
use tokio::sync::mpsc;

/// A request to recompute a driver's rating after a lap is persisted.
#[derive(Debug, Clone)]
pub struct RatingRequest {
    pub driver_id: String,
    pub sim_type: String,
}

/// Computed driver rating record, stored in `driver_ratings` table.
#[derive(Debug, Clone, Serialize)]
pub struct DriverRating {
    pub driver_id: String,
    pub sim_type: String,
    pub composite_rating: f64,
    pub rating_class: String,
    pub pace_score: f64,
    pub consistency_score: f64,
    pub experience_score: f64,
    pub total_laps: i64,
    pub updated_at: String,
}

/// Spawn the rating computation worker. Returns the sender half of the channel.
///
/// The worker processes `RatingRequest`s sequentially — it never blocks lap insertion
/// because the caller uses `try_send` (non-blocking).
pub fn spawn_rating_worker(db: SqlitePool, venue_id: String) -> mpsc::Sender<RatingRequest> {
    let (tx, mut rx) = mpsc::channel::<RatingRequest>(256);

    tokio::spawn(async move {
        tracing::info!("driver-rating worker started");

        while let Some(req) = rx.recv().await {
            if let Err(e) = compute_and_store_rating(&db, &req.driver_id, &req.sim_type, &venue_id).await {
                tracing::warn!(
                    driver_id = %req.driver_id,
                    sim_type = %req.sim_type,
                    "Failed to compute driver rating: {}",
                    e,
                );
            }
        }

        tracing::warn!("driver-rating worker exited (channel closed)");
    });

    tx
}

/// Compute the rating for a single driver+sim_type and upsert into the DB.
async fn compute_and_store_rating(
    db: &SqlitePool,
    driver_id: &str,
    sim_type: &str,
    venue_id: &str,
) -> Result<(), sqlx::Error> {
    // --- 1. Total valid laps for this driver+sim_type ---
    let total_laps: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM laps WHERE driver_id = ? AND sim_type = ? AND valid = 1 AND (suspect IS NULL OR suspect = 0)",
    )
    .bind(driver_id)
    .bind(sim_type)
    .fetch_one(db)
    .await?
    .0;

    // --- 2. Experience score (log-scaled, never divides by zero) ---
    let experience_score = (100.0 * (total_laps as f64 + 1.0).log10() / 1001.0_f64.log10())
        .clamp(0.0, 100.0);

    // --- 3. Pace score ---
    // Best lap for this driver across all tracks for this sim_type
    let best_lap_ms: Option<i64> = sqlx::query_as::<_, (i64,)>(
        "SELECT MIN(lap_time_ms) FROM laps WHERE driver_id = ? AND sim_type = ? AND valid = 1 AND (suspect IS NULL OR suspect = 0)",
    )
    .bind(driver_id)
    .bind(sim_type)
    .fetch_optional(db)
    .await?
    .map(|r| r.0);

    // Track record for the same track as the driver's best lap
    let track_record_ms: Option<i64> = if best_lap_ms.is_some() {
        // Get the track where the driver's best lap was set
        let best_track: Option<String> = sqlx::query_as::<_, (String,)>(
            "SELECT track FROM laps WHERE driver_id = ? AND sim_type = ? AND valid = 1 AND (suspect IS NULL OR suspect = 0) ORDER BY lap_time_ms ASC LIMIT 1",
        )
        .bind(driver_id)
        .bind(sim_type)
        .fetch_optional(db)
        .await?
        .map(|r| r.0);

        if let Some(ref track) = best_track {
            // Get the track record for that specific track+sim_type (any car)
            sqlx::query_as::<_, (i64,)>(
                "SELECT MIN(best_lap_ms) FROM track_records WHERE track = ? AND sim_type = ?",
            )
            .bind(track)
            .bind(sim_type)
            .fetch_optional(db)
            .await?
            .map(|r| r.0)
        } else {
            None
        }
    } else {
        None
    };

    let pace_score = match (best_lap_ms, track_record_ms) {
        (Some(best), Some(record)) if record > 0 => {
            let diff = (best - record).max(0) as f64;
            (100.0 * (1.0 - diff / record as f64)).clamp(0.0, 100.0)
        }
        _ => 50.0, // No track record exists: neutral
    };

    // --- 4. Consistency score (std_dev / mean of last 10 valid laps) ---
    let recent_laps: Vec<(i64,)> = sqlx::query_as::<_, (i64,)>(
        "SELECT lap_time_ms FROM laps WHERE driver_id = ? AND sim_type = ? AND valid = 1 AND (suspect IS NULL OR suspect = 0) ORDER BY created_at DESC LIMIT 10",
    )
    .bind(driver_id)
    .bind(sim_type)
    .fetch_all(db)
    .await?;

    let consistency_score = if recent_laps.len() < 3 {
        50.0 // Neutral if fewer than 3 laps
    } else {
        let times: Vec<f64> = recent_laps.iter().map(|r| r.0 as f64).collect();
        let n = times.len() as f64;
        let mean = times.iter().sum::<f64>() / n;
        if mean == 0.0 {
            0.0
        } else {
            let variance = times.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n;
            let std_dev = variance.sqrt();
            (100.0 * (1.0 - std_dev / mean)).clamp(0.0, 100.0)
        }
    };

    // --- 5. Composite rating ---
    let composite_rating = 0.5 * pace_score + 0.3 * consistency_score + 0.2 * experience_score;

    // --- 6. Rating class ---
    let rating_class = if total_laps < 3 {
        "Unrated"
    } else if composite_rating <= 30.0 {
        "Rookie"
    } else if composite_rating <= 50.0 {
        "Amateur"
    } else if composite_rating <= 70.0 {
        "Club"
    } else if composite_rating <= 85.0 {
        "Pro"
    } else {
        "Elite"
    };

    // --- 7. Upsert into driver_ratings ---
    sqlx::query(
        "INSERT INTO driver_ratings (driver_id, sim_type, composite_rating, rating_class, pace_score, consistency_score, experience_score, total_laps, updated_at, venue_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)
         ON CONFLICT(driver_id, sim_type) DO UPDATE SET
            composite_rating = excluded.composite_rating,
            rating_class = excluded.rating_class,
            pace_score = excluded.pace_score,
            consistency_score = excluded.consistency_score,
            experience_score = excluded.experience_score,
            total_laps = excluded.total_laps,
            updated_at = excluded.updated_at",
    )
    .bind(driver_id)
    .bind(sim_type)
    .bind(composite_rating)
    .bind(rating_class)
    .bind(pace_score)
    .bind(consistency_score)
    .bind(experience_score)
    .bind(total_laps)
    .bind(venue_id)
    .execute(db)
    .await?;

    tracing::debug!(
        driver_id = %driver_id,
        sim_type = %sim_type,
        composite = composite_rating,
        class = %rating_class,
        "Driver rating updated",
    );

    Ok(())
}

/// Backfill ratings for all drivers with 3+ laps.
/// Called once at startup if driver_ratings is empty but laps exist.
/// Processes in batches of 100 with 100ms sleep between.
pub async fn backfill_ratings(db: SqlitePool, venue_id: String) {
    // Check if ratings table is empty
    let count: i64 = match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM driver_ratings",
    )
    .fetch_one(&db)
    .await
    {
        Ok(r) => r.0,
        Err(e) => {
            tracing::warn!("Failed to check driver_ratings count for backfill: {}", e);
            return;
        }
    };

    if count > 0 {
        tracing::debug!("driver_ratings already populated ({} rows), skipping backfill", count);
        return;
    }

    // Check if laps exist
    let lap_count: i64 = match sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM laps WHERE valid = 1 AND (suspect IS NULL OR suspect = 0)",
    )
    .fetch_one(&db)
    .await
    {
        Ok(r) => r.0,
        Err(e) => {
            tracing::warn!("Failed to check laps count for backfill: {}", e);
            return;
        }
    };

    if lap_count == 0 {
        tracing::debug!("No laps in DB, skipping rating backfill");
        return;
    }

    // Get all driver+sim_type pairs with 3+ valid laps
    let pairs: Vec<(String, String)> = match sqlx::query_as::<_, (String, String)>(
        "SELECT driver_id, sim_type FROM laps WHERE valid = 1 AND (suspect IS NULL OR suspect = 0) GROUP BY driver_id, sim_type HAVING COUNT(*) >= 3",
    )
    .fetch_all(&db)
    .await
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to query driver+sim_type pairs for backfill: {}", e);
            return;
        }
    };

    tracing::info!("Backfilling driver ratings for {} driver+sim_type pairs", pairs.len());

    for (batch_idx, chunk) in pairs.chunks(100).enumerate() {
        for (driver_id, sim_type) in chunk {
            if let Err(e) = compute_and_store_rating(&db, driver_id, sim_type, &venue_id).await {
                tracing::warn!(
                    driver_id = %driver_id,
                    sim_type = %sim_type,
                    "Backfill rating failed: {}",
                    e,
                );
            }
        }
        if batch_idx + 1 < pairs.chunks(100).count() {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    tracing::info!("Driver rating backfill complete ({} pairs processed)", pairs.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rating_class_boundaries() {
        // Unrated is handled by total_laps < 3 check, not composite score
        assert_eq!(classify(0.0), "Rookie");
        assert_eq!(classify(30.0), "Rookie");
        assert_eq!(classify(30.1), "Amateur");
        assert_eq!(classify(50.0), "Amateur");
        assert_eq!(classify(50.1), "Club");
        assert_eq!(classify(70.0), "Club");
        assert_eq!(classify(70.1), "Pro");
        assert_eq!(classify(85.0), "Pro");
        assert_eq!(classify(85.1), "Elite");
        assert_eq!(classify(100.0), "Elite");
    }

    fn classify(composite: f64) -> &'static str {
        if composite <= 30.0 {
            "Rookie"
        } else if composite <= 50.0 {
            "Amateur"
        } else if composite <= 70.0 {
            "Club"
        } else if composite <= 85.0 {
            "Pro"
        } else {
            "Elite"
        }
    }

    #[test]
    fn experience_score_edge_cases() {
        // 0 laps: (0+1).log10() / 1001.0.log10() = 0 / 3.0004 ≈ 0
        let score_0 = 100.0 * (0.0_f64 + 1.0).log10() / 1001.0_f64.log10();
        assert!((score_0 - 0.0).abs() < 0.01);

        // 1000 laps: (1000+1).log10() / 1001.0.log10() ≈ 1.0
        let score_1000 = 100.0 * (1000.0_f64 + 1.0).log10() / 1001.0_f64.log10();
        assert!((score_1000 - 100.0).abs() < 0.1);

        // 10 laps: mid-range
        let score_10 = 100.0 * (10.0_f64 + 1.0).log10() / 1001.0_f64.log10();
        assert!(score_10 > 30.0 && score_10 < 40.0);
    }

    #[test]
    fn pace_score_at_record() {
        // If driver's best equals track record, pace = 100
        let record = 60000_i64;
        let best = 60000_i64;
        let diff = (best - record).max(0) as f64;
        let pace = 100.0 * (1.0 - diff / record as f64);
        assert!((pace - 100.0).abs() < 0.01);
    }

    #[test]
    fn pace_score_clamped() {
        // If driver's best is way slower, pace is clamped to 0
        let record = 60000_i64;
        let best = 200000_i64;
        let diff = (best - record).max(0) as f64;
        let pace = (100.0 * (1.0 - diff / record as f64)).clamp(0.0, 100.0);
        assert!((pace - 0.0).abs() < 0.01);
    }

    #[test]
    fn consistency_score_perfect() {
        // All same times → std_dev = 0 → consistency = 100
        let times = vec![60000.0, 60000.0, 60000.0, 60000.0, 60000.0];
        let n = times.len() as f64;
        let mean = times.iter().sum::<f64>() / n;
        let variance = times.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n;
        let std_dev = variance.sqrt();
        let score = 100.0 * (1.0 - std_dev / mean);
        assert!((score - 100.0).abs() < 0.01);
    }
}
