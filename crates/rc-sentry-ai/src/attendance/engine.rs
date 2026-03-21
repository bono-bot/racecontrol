use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::broadcast;

use crate::config::AttendanceConfig;
use crate::recognition::types::RecognitionResult;

/// Run the attendance engine, subscribing to recognition events.
///
/// Deduplicates across cameras: if the same person_id was logged within
/// `config.dedup_window_secs`, the event is skipped.
pub async fn run(
    mut rx: broadcast::Receiver<RecognitionResult>,
    db_path: String,
    config: AttendanceConfig,
) {
    // Create tables on startup
    let init_path = db_path.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || {
        let conn = rusqlite::Connection::open(&init_path)?;
        super::db::create_tables(&conn)?;
        Ok::<(), rusqlite::Error>(())
    })
    .await
    {
        tracing::error!(error = %e, "failed to initialize attendance tables");
        return;
    }

    let dedup_window = Duration::from_secs(config.dedup_window_secs);
    let mut dedup_map: HashMap<i64, Instant> = HashMap::new();
    let mut cleanup_interval = tokio::time::interval(Duration::from_secs(300));
    // Consume the first immediate tick
    cleanup_interval.tick().await;

    loop {
        tokio::select! {
            result = rx.recv() => {
                let event = match result {
                    Ok(ev) => ev,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "attendance receiver lagged, dropped events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("attendance broadcast channel closed, shutting down");
                        break;
                    }
                };

                // Cross-camera dedup check
                let now = Instant::now();
                if let Some(last) = dedup_map.get(&event.person_id) {
                    if now.duration_since(*last) < dedup_window {
                        tracing::debug!(
                            person_id = event.person_id,
                            person_name = %event.person_name,
                            camera = %event.camera,
                            "attendance dedup: skipping (within window)"
                        );
                        continue;
                    }
                }

                // Compute IST day boundary (UTC+5:30)
                let ist_offset = chrono::FixedOffset::east_opt(5 * 3600 + 1800)
                    .expect("valid IST offset");
                let ist_time = event.timestamp.with_timezone(&ist_offset);
                let day = ist_time.format("%Y-%m-%d").to_string();

                // Insert attendance record and process staff shift
                let insert_path = db_path.clone();
                let person_id = event.person_id;
                let person_name = event.person_name.clone();
                let camera = event.camera.clone();
                let confidence = event.confidence;
                let day_clone = day.clone();
                let ist_timestamp = ist_time.format("%Y-%m-%d %H:%M:%S").to_string();
                let min_shift_hours = config.min_shift_hours;

                let insert_result = tokio::task::spawn_blocking(move || {
                    let conn = rusqlite::Connection::open(&insert_path)?;
                    let row_id = super::db::insert_attendance(
                        &conn,
                        person_id,
                        &person_name,
                        &camera,
                        confidence,
                        &day_clone,
                    )?;

                    // Process staff shift tracking
                    let shift_action = super::shifts::process_staff_recognition(
                        &conn,
                        person_id,
                        &person_name,
                        &day_clone,
                        &ist_timestamp,
                        min_shift_hours,
                    )?;

                    Ok::<(i64, Option<super::db::ShiftAction>), rusqlite::Error>((row_id, shift_action))
                })
                .await;

                match insert_result {
                    Ok(Ok((row_id, shift_action))) => {
                        dedup_map.insert(event.person_id, now);
                        tracing::info!(
                            person_name = %event.person_name,
                            person_id = event.person_id,
                            camera = %event.camera,
                            day = %day,
                            row_id = row_id,
                            "attendance logged"
                        );

                        // Log staff shift actions
                        match shift_action {
                            Some(super::db::ShiftAction::ClockIn) => {
                                tracing::info!(
                                    person_name = %event.person_name,
                                    day = %day,
                                    "staff clock-in"
                                );
                            }
                            Some(super::db::ShiftAction::Update) => {
                                tracing::debug!(
                                    person_name = %event.person_name,
                                    day = %day,
                                    "staff shift updated"
                                );
                            }
                            None => {}
                        }
                    }
                    Ok(Err(e)) => {
                        tracing::error!(
                            error = %e,
                            person_id = event.person_id,
                            "failed to insert attendance record"
                        );
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "attendance spawn_blocking panicked");
                    }
                }
            }
            _ = cleanup_interval.tick() => {
                let before = dedup_map.len();
                let now = Instant::now();
                dedup_map.retain(|_, last| now.duration_since(*last) < dedup_window);
                let removed = before - dedup_map.len();
                if removed > 0 {
                    tracing::debug!(removed = removed, remaining = dedup_map.len(), "attendance dedup map cleaned");
                }
            }
        }
    }
}
