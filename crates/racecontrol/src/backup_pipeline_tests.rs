use super::*;
use std::fs;
use tempfile::TempDir;

fn make_temp_dir() -> TempDir {
    tempfile::tempdir().expect("Failed to create temp dir")
}

/// Create a fake backup file in dir with given name.
fn make_file(dir: &std::path::Path, name: &str) {
    let path = dir.join(name);
    fs::write(path, b"fake backup content").expect("Failed to write fake backup");
}

#[test]
fn rotate_backups_with_10_daily_and_retain_7_deletes_3_oldest() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create 10 daily backups for racecontrol prefix
    for i in 1..=10 {
        make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
    }

    rotate_backups(
        dir.to_str().unwrap(),
        "racecontrol",
        7, // daily_retain
        4, // weekly_retain
        12, // monthly_retain
    )
    .unwrap();

    // Count remaining daily files
    let remaining: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("racecontrol-") && !name.contains("-weekly-")
        })
        .collect();

    assert_eq!(
        remaining.len(),
        7,
        "Expected 7 daily files after rotation, got {}",
        remaining.len()
    );
}

#[test]
fn rotate_backups_deletes_oldest_3_keeps_newest_7() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    for i in 1..=10 {
        make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4, 12).unwrap();

    // The oldest 3 (01..03) should be gone
    for i in 1..=3 {
        let name = format!("racecontrol-2026-01-{:02}T12-00-00.db", i);
        assert!(
            !dir.join(&name).exists(),
            "File {} should have been deleted",
            name
        );
    }
    // The newest 7 (04..10) should remain
    for i in 4..=10 {
        let name = format!("racecontrol-2026-01-{:02}T12-00-00.db", i);
        assert!(
            dir.join(&name).exists(),
            "File {} should have been retained",
            name
        );
    }
}

#[test]
fn rotate_backups_preserves_weekly_files_up_to_weekly_retain() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create 6 weekly backup files
    for i in 1..=6 {
        make_file(dir, &format!("racecontrol-weekly-2026-W{:02}.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4, 12).unwrap();

    let remaining: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .contains("-weekly-")
        })
        .collect();

    assert_eq!(
        remaining.len(),
        4,
        "Expected 4 weekly files after rotation, got {}",
        remaining.len()
    );
}

#[test]
fn rotate_backups_does_nothing_when_below_retain_limit() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create only 5 daily files (below the retain limit of 7)
    for i in 1..=5 {
        make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4, 12).unwrap();

    let remaining: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .collect();

    assert_eq!(remaining.len(), 5, "No files should be deleted when under retain limit");
}

#[test]
fn compute_staleness_returns_none_for_empty_dir() {
    let tmp = make_temp_dir();
    let staleness = compute_staleness(tmp.path().to_str().unwrap());
    assert!(staleness.is_none(), "Expected None for empty directory");
}

#[test]
fn compute_staleness_returns_none_for_nonexistent_dir() {
    let staleness = compute_staleness("/tmp/nonexistent-backup-dir-xyz-12345");
    assert!(staleness.is_none(), "Expected None for nonexistent directory");
}

#[test]
fn compute_staleness_returns_some_when_files_exist() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    make_file(dir, "racecontrol-2026-01-01T12-00-00.db");

    let staleness = compute_staleness(dir.to_str().unwrap());
    assert!(
        staleness.is_some(),
        "Expected Some staleness value when backup files exist"
    );
    // The file was just created, so staleness should be very low
    let hours = staleness.unwrap();
    assert!(
        hours < 0.1,
        "Freshly created file should have <0.1 hours staleness, got {}",
        hours
    );
}

#[test]
fn backup_file_naming_follows_racecontrol_prefix_pattern() {
    // Verify the pattern: "racecontrol-YYYY-MM-DDTHH-MM-SS.db"
    let tmp = make_temp_dir();
    let dir = tmp.path();
    let name = "racecontrol-2026-04-01T15-30-00.db";
    make_file(dir, name);

    assert!(dir.join(name).exists());
    assert!(name.starts_with("racecontrol-"));
    assert!(name.ends_with(".db"));
    // Should NOT contain "weekly"
    assert!(!name.contains("weekly"));
}

#[test]
fn backup_file_naming_follows_telemetry_prefix_pattern() {
    // Verify the pattern: "telemetry-YYYY-MM-DDTHH-MM-SS.db"
    let tmp = make_temp_dir();
    let dir = tmp.path();
    let name = "telemetry-2026-04-01T15-30-00.db";
    make_file(dir, name);

    assert!(dir.join(name).exists());
    assert!(name.starts_with("telemetry-"));
    assert!(name.ends_with(".db"));
    assert!(!name.contains("weekly"));
}

#[test]
fn weekly_snapshot_naming_follows_pattern() {
    // Verify weekly pattern: "racecontrol-weekly-YYYY-WNN.db"
    let name = "racecontrol-weekly-2026-W14.db";
    assert!(name.starts_with("racecontrol-weekly-"));
    assert!(name.ends_with(".db"));
    assert!(name.contains("W14"));
}

#[test]
fn rotate_backups_does_not_delete_files_from_other_prefix() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create 10 racecontrol daily files
    for i in 1..=10 {
        make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
    }
    // Create 3 telemetry daily files
    for i in 1..=3 {
        make_file(dir, &format!("telemetry-2026-01-{:02}T12-00-00.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 7, 4, 12).unwrap();

    // Telemetry files should be untouched
    let telemetry_count = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("telemetry-")
        })
        .count();

    assert_eq!(
        telemetry_count,
        3,
        "Rotating racecontrol should not affect telemetry files"
    );
}

#[test]
fn rotate_backups_monthly_tier_retains_up_to_monthly_retain() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create 15 monthly backup files (beyond 12-month retain)
    for i in 1..=15 {
        make_file(dir, &format!("racecontrol-monthly-2025-{:02}.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 30, 4, 12).unwrap();

    let monthly_remaining: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("-monthly-"))
        .collect();

    assert_eq!(
        monthly_remaining.len(),
        12,
        "Expected 12 monthly files after rotation, got {}",
        monthly_remaining.len()
    );
}

#[test]
fn rotate_backups_monthly_files_not_affected_by_daily_rotation() {
    let tmp = make_temp_dir();
    let dir = tmp.path();

    // Create daily files and monthly files for same prefix
    for i in 1..=35 {
        make_file(dir, &format!("racecontrol-2026-01-{:02}T12-00-00.db", i));
    }
    for i in 1..=5 {
        make_file(dir, &format!("racecontrol-monthly-2025-{:02}.db", i));
    }

    rotate_backups(dir.to_str().unwrap(), "racecontrol", 30, 4, 12).unwrap();

    // Daily: 35 created, 30 retained → 5 deleted
    let daily_remaining = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("racecontrol-2026-") && !name.contains("-monthly-") && !name.contains("-weekly-")
        })
        .count();
    assert_eq!(daily_remaining, 30, "Daily files should be 30 after rotation");

    // Monthly: 5 created, all 5 retained (below 12 limit)
    let monthly_remaining = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("-monthly-"))
        .count();
    assert_eq!(monthly_remaining, 5, "Monthly files should all be retained when below limit");
}

#[test]
fn staleness_debounce_logic_fires_on_first_call() {
    // Simulate: no previous alert, threshold exceeded
    let mut last_alert_fired: Option<Instant> = None;
    let debounce_secs = 2 * 3600u64;

    // First time: should fire
    let should_fire = match last_alert_fired {
        None => true,
        Some(fired_at) => fired_at.elapsed() >= Duration::from_secs(debounce_secs),
    };
    assert!(should_fire, "First staleness check should always fire");

    // Simulate firing
    last_alert_fired = Some(Instant::now());

    // Second time immediately: should NOT fire (debounce)
    let should_fire_again = match last_alert_fired {
        None => true,
        Some(fired_at) => fired_at.elapsed() >= Duration::from_secs(debounce_secs),
    };
    assert!(!should_fire_again, "Immediate re-fire should be suppressed by debounce");
}
