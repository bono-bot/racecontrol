// Structural audit: every `INSERT INTO billing_events` in production source (not test
// fixtures) must explicitly include the `venue_id` column. Regression test for P0-5:
// `handle_offline_auto_end` in billing_timer_expiry_timeout.rs omitted the column, which
// caused `offline_auto_ended` events to silently fall back to the DB default venue
// (`racingpoint-hyd-001`), breaking cloud-sync replication for any non-default venue.

use std::fs;
use std::path::{Path, PathBuf};

fn crate_src_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn walk_rs_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_rs_files(&path, acc);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            // Skip files named `*_tests.rs` — these are unit-test fixtures that
            // live under `src/` (routes_tests.rs, cloud_sync_tests.rs, etc.) and
            // seed fake data via raw INSERT without venue_id on purpose.
            let is_test_fixture = path
                .file_name()
                .and_then(|s| s.to_str())
                .is_some_and(|name| name.ends_with("_tests.rs"));
            if !is_test_fixture {
                acc.push(path);
            }
        }
    }
}

#[test]
fn every_production_insert_into_billing_events_includes_venue_id() {
    let mut files = Vec::new();
    walk_rs_files(&crate_src_root(), &mut files);

    let mut violations: Vec<String> = Vec::new();

    for path in &files {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if !content.contains("INSERT INTO billing_events") {
            continue;
        }
        let lines: Vec<&str> = content.lines().collect();
        for (idx, line) in lines.iter().enumerate() {
            if !line.contains("INSERT INTO billing_events") {
                continue;
            }
            // Aggregate up to 3 lines to cover multi-line SQL like
            // `"INSERT INTO billing_events (...)\n VALUES (...)"`.
            let end = (idx + 3).min(lines.len());
            let block: String = lines[idx..end].join(" ");
            if !block.contains("venue_id") {
                violations.push(format!("{}:{}", path.display(), idx + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "production INSERT INTO billing_events missing venue_id column at:\n  {}",
        violations.join("\n  ")
    );
}
