//! Event scoring, championship standings, and hotlap event auto-entry.
//!
//! Extracted from `lap_tracker.rs` for module size compliance (<500 lines).
//! Contains: auto_enter_event, recalculate_event_positions, f1_points_for_position,
//! score_group_event, compute_championship_standings, assign_championship_positions.

// ─── F1 Scoring constants ─────────────────────────────────────────────────────

/// F1 2010 points system: P1=25, P2=18, ..., P10=1, P11+=0.
const F1_2010_POINTS: [i64; 10] = [25, 18, 15, 12, 10, 8, 6, 4, 2, 1];

/// Return F1 points for a given finishing position.
/// DNF drivers, positions outside 1-10, and positions < 1 all receive 0 points.
pub fn f1_points_for_position(position: i64, dnf: bool) -> i64 {
    if dnf || !(1..=10).contains(&position) {
        return 0;
    }
    F1_2010_POINTS[(position - 1) as usize]
}

/// Auto-enter a valid lap into matching active hotlap events.
/// Called from persist_lap() after lap INSERT, only if valid && suspect==0.
/// Made pub so integration tests can call it directly with a pool.
pub async fn auto_enter_event(
    pool: &sqlx::SqlitePool,
    lap_id: Option<&str>,
    driver_id: &str,
    track: &str,
    car_class: &str,
    sim_type_str: &str,
    lap_time_ms: u32,
    sector1_ms: Option<u32>,
    sector2_ms: Option<u32>,
    sector3_ms: Option<u32>,
    venue_id: &str,
) {
    // Query matching active/upcoming events for this track+car_class+sim_type in the current date range
    let events = sqlx::query_as::<_, (String, Option<i64>)>(
        "SELECT id, reference_time_ms FROM hotlap_events
         WHERE track = ? AND car_class = ? AND sim_type = ?
           AND status IN ('active', 'upcoming')
           AND (starts_at IS NULL OR datetime(starts_at) <= datetime('now'))
           AND (ends_at IS NULL OR datetime(ends_at) >= datetime('now'))",
    )
    .bind(track)
    .bind(car_class)
    .bind(sim_type_str)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    for (event_id, reference_time_ms) in events {
        // Check if driver already has a faster or equal entry for this event
        let existing: Option<(i64,)> = sqlx::query_as(
            "SELECT lap_time_ms FROM hotlap_event_entries WHERE event_id = ? AND driver_id = ?",
        )
        .bind(&event_id)
        .bind(driver_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        if let Some((existing_ms,)) = existing
            && existing_ms <= lap_time_ms as i64 {
                // Existing entry is faster or equal -- skip
                continue;
            }

        // Compute badge from reference_time_ms
        let badge: Option<&str> = match reference_time_ms {
            None => None,
            Some(ref_ms) => {
                let ratio = lap_time_ms as f64 / ref_ms as f64;
                if ratio <= 1.02 {
                    Some("gold")
                } else if ratio <= 1.05 {
                    Some("silver")
                } else if ratio <= 1.08 {
                    Some("bronze")
                } else {
                    Some("none")
                }
            }
        };

        // Generate a new UUID for the entry
        let entry_id = uuid::Uuid::new_v4().to_string();

        // UPSERT the entry (ON CONFLICT updates if this lap is faster)
        let upsert_result = sqlx::query(
            "INSERT INTO hotlap_event_entries
                (id, event_id, driver_id, lap_id, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, badge, result_status, entered_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'finished', datetime('now'), ?)
             ON CONFLICT(event_id, driver_id) DO UPDATE SET
                lap_id = excluded.lap_id,
                lap_time_ms = excluded.lap_time_ms,
                sector1_ms = excluded.sector1_ms,
                sector2_ms = excluded.sector2_ms,
                sector3_ms = excluded.sector3_ms,
                badge = excluded.badge,
                result_status = 'finished',
                entered_at = excluded.entered_at",
        )
        .bind(&entry_id)
        .bind(&event_id)
        .bind(driver_id)
        .bind(lap_id)
        .bind(lap_time_ms as i64)
        .bind(sector1_ms.map(|v| v as i64))
        .bind(sector2_ms.map(|v| v as i64))
        .bind(sector3_ms.map(|v| v as i64))
        .bind(badge)
        .bind(venue_id)
        .execute(pool)
        .await;

        match upsert_result {
            Ok(_) => {
                tracing::info!(
                    "[events] Driver {} entered event {} with {}ms (badge: {:?})",
                    driver_id, event_id, lap_time_ms, badge
                );
                // Recalculate positions and gaps for all entries in this event
                recalculate_event_positions(pool, &event_id).await;
            }
            Err(e) => {
                tracing::error!(
                    "[events] Failed to upsert event entry for driver {} event {}: {}",
                    driver_id, event_id, e
                );
            }
        }
    }
}

/// Recalculate position, gap_to_leader_ms, and within_107_percent for all entries in an event.
/// Uses a two-step approach since SQLite UPDATE doesn't support window functions directly.
/// Made pub so integration tests can call it directly.
pub async fn recalculate_event_positions(pool: &sqlx::SqlitePool, event_id: &str) {
    // Step 1: Fetch all finished entries ordered by lap time (fastest first)
    let entries: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, lap_time_ms FROM hotlap_event_entries
         WHERE event_id = ? AND result_status = 'finished'
         ORDER BY lap_time_ms ASC",
    )
    .bind(event_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    if entries.is_empty() {
        return;
    }

    let leader_ms = entries[0].1;

    // Step 2: Update each entry's position, gap, and 107% flag
    for (position, (entry_id, lap_ms)) in entries.iter().enumerate() {
        let pos = (position + 1) as i64;
        let gap = lap_ms - leader_ms;
        // Integer math: lap_ms * 100 <= leader_ms * 107
        let within_107: i64 = if lap_ms * 100 <= leader_ms * 107 { 1 } else { 0 };

        let _ = sqlx::query(
            "UPDATE hotlap_event_entries
             SET position = ?, gap_to_leader_ms = ?, within_107_percent = ?
             WHERE id = ?",
        )
        .bind(pos)
        .bind(gap)
        .bind(within_107)
        .bind(entry_id)
        .execute(pool)
        .await;
    }

    tracing::debug!(
        "[events] Recalculated positions for event {} ({} entries, leader: {}ms)",
        event_id, entries.len(), leader_ms
    );
}

/// Score a completed group session linked to a hotlap event.
///
/// Reads multiplayer_results for the group session, assigns F1 2010 points
/// based on finishing position (DNF = 0 points), and upserts entries into
/// hotlap_event_entries. The leader's gap_to_leader_ms = 0; others are
/// computed as (their best_lap_ms - leader best_lap_ms).
///
/// Made pub so integration tests can call it directly.
pub async fn score_group_event(
    pool: &sqlx::SqlitePool,
    group_session_id: &str,
    hotlap_event_id: &str,
    venue_id: &str,
) -> Result<(), String> {
    // Fetch all results ordered by position
    let results: Vec<(String, i64, Option<i64>, i64)> = sqlx::query_as(
        "SELECT driver_id, position, best_lap_ms, dnf
         FROM multiplayer_results
         WHERE group_session_id = ?
         ORDER BY position ASC",
    )
    .bind(group_session_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch multiplayer results: {e}"))?;

    if results.is_empty() {
        return Ok(());
    }

    // Find leader lap time (minimum best_lap_ms among non-DNF drivers)
    let leader_ms: Option<i64> = results
        .iter()
        .filter(|(_, _, _, dnf)| *dnf == 0)
        .filter_map(|(_, _, best_lap_ms, _)| *best_lap_ms)
        .reduce(i64::min);

    for (driver_id, position, best_lap_ms, dnf) in &results {
        let is_dnf = *dnf == 1;
        let points = f1_points_for_position(*position, is_dnf);
        let result_status = if is_dnf { "dnf" } else { "finished" };

        let gap_to_leader_ms: Option<i64> = if is_dnf {
            None
        } else {
            match (best_lap_ms, leader_ms) {
                (Some(ms), Some(leader)) => Some(ms - leader),
                _ => None,
            }
        };

        let within_107: i64 = match (is_dnf, best_lap_ms, leader_ms) {
            (false, Some(ms), Some(leader)) => {
                if ms * 100 <= leader * 107 { 1 } else { 0 }
            }
            _ => 0,
        };

        let entry_id = uuid::Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO hotlap_event_entries
                (id, event_id, driver_id, lap_time_ms, position, points,
                 gap_to_leader_ms, within_107_percent, result_status, entered_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(event_id, driver_id) DO UPDATE SET
                lap_time_ms = excluded.lap_time_ms,
                position = excluded.position,
                points = excluded.points,
                gap_to_leader_ms = excluded.gap_to_leader_ms,
                within_107_percent = excluded.within_107_percent,
                result_status = excluded.result_status",
        )
        .bind(&entry_id)
        .bind(hotlap_event_id)
        .bind(driver_id)
        .bind(*best_lap_ms)
        .bind(*position)
        .bind(points)
        .bind(gap_to_leader_ms)
        .bind(within_107)
        .bind(result_status)
        .bind(venue_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to upsert hotlap_event_entry for driver {driver_id}: {e}"))?;
    }

    tracing::info!(
        "[events] Scored group session {} into event {} ({} results)",
        group_session_id, hotlap_event_id, results.len()
    );

    Ok(())
}

/// Compute championship standings from hotlap_event_entries and persist to championship_standings.
///
/// Aggregates points, wins, P2 count, P3 count across all rounds in the championship,
/// upserts into championship_standings, then calls assign_championship_positions to
/// apply the F1 tiebreaker ordering (points DESC, wins DESC, p2_count DESC, p3_count DESC).
///
/// Made pub so integration tests can call it directly.
pub async fn compute_championship_standings(
    pool: &sqlx::SqlitePool,
    championship_id: &str,
    venue_id: &str,
) -> Result<(), String> {
    // Aggregate points and counts from hotlap_event_entries across all rounds
    let rows: Vec<(String, i64, i64, i64, i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT hee.driver_id,
                COALESCE(SUM(hee.points), 0) as total_points,
                COUNT(DISTINCT cr.event_id) as rounds_entered,
                SUM(CASE WHEN hee.position = 1 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as wins,
                SUM(CASE WHEN hee.position = 2 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p2_count,
                SUM(CASE WHEN hee.position = 3 AND hee.result_status = 'finished' THEN 1 ELSE 0 END) as p3_count,
                MIN(hee.position) as best_result
         FROM hotlap_event_entries hee
         INNER JOIN championship_rounds cr ON cr.event_id = hee.event_id
         WHERE cr.championship_id = ?
           AND hee.result_status IN ('finished', 'dnf', 'dns')
         GROUP BY hee.driver_id",
    )
    .bind(championship_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to aggregate championship standings: {e}"))?;

    for (driver_id, total_points, rounds_entered, wins, p2_count, p3_count, best_result) in &rows {
        sqlx::query(
            "INSERT INTO championship_standings
                (championship_id, driver_id, total_points, rounds_entered, wins, p2_count, p3_count, best_result, updated_at, venue_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), ?)
             ON CONFLICT(championship_id, driver_id) DO UPDATE SET
                total_points = excluded.total_points,
                rounds_entered = excluded.rounds_entered,
                wins = excluded.wins,
                p2_count = excluded.p2_count,
                p3_count = excluded.p3_count,
                best_result = excluded.best_result,
                updated_at = excluded.updated_at",
        )
        .bind(championship_id)
        .bind(driver_id)
        .bind(total_points)
        .bind(rounds_entered)
        .bind(wins)
        .bind(p2_count)
        .bind(p3_count)
        .bind(best_result)
        .bind(venue_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Failed to upsert standing for driver {driver_id}: {e}"))?;
    }

    // Now assign positions based on F1 tiebreaker order
    assign_championship_positions(pool, championship_id).await?;

    tracing::info!(
        "[championship] Computed standings for championship {} ({} drivers)",
        championship_id, rows.len()
    );

    Ok(())
}

/// Assign position column to all championship_standings rows for a championship.
///
/// Reads all standing rows, sorts by (total_points DESC, wins DESC, p2_count DESC, p3_count DESC),
/// then updates the position column (1-indexed). This is the F1 tiebreaker rule.
///
/// Made pub so integration tests can call it directly without seeding hotlap_event_entries.
pub async fn assign_championship_positions(
    pool: &sqlx::SqlitePool,
    championship_id: &str,
) -> Result<(), String> {
    // Fetch all standings rows sorted by tiebreaker
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT driver_id
         FROM championship_standings
         WHERE championship_id = ?
         ORDER BY total_points DESC, wins DESC, p2_count DESC, p3_count DESC",
    )
    .bind(championship_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch championship standings for position assignment: {e}"))?;

    // Update position for each driver
    for (position, (driver_id,)) in rows.iter().enumerate() {
        let pos = (position + 1) as i64;
        sqlx::query(
            "UPDATE championship_standings SET position = ? WHERE championship_id = ? AND driver_id = ?",
        )
        .bind(pos)
        .bind(championship_id)
        .bind(driver_id)
        .execute(pool)
        .await
        .map_err(|e| {
            format!(
                "Failed to update position for driver {driver_id} in championship {championship_id}: {e}"
            )
        })?;
    }

    tracing::debug!(
        "[championship] Assigned positions for {} in championship {}",
        rows.len(), championship_id
    );

    Ok(())
}
