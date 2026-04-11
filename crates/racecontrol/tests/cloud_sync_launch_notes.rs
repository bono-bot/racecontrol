// Phase 368 Plan 03 — Task 1: cloud_sync includes launch_notes in SYNC_TABLES
//
// Verifies that the SYNC_TABLES constant in cloud_sync.rs includes "launch_notes"
// so that Phase 301 dual-write replication carries launch notes venue↔cloud.

use racecontrol_crate::cloud_sync::SYNC_TABLES;

/// Test: SYNC_TABLES constant includes "launch_notes".
/// D-02: launch_notes replicates via Phase 301 cloud_sync infrastructure.
#[test]
fn cloud_sync_includes_launch_notes() {
    assert!(
        SYNC_TABLES.contains("launch_notes"),
        "SYNC_TABLES must include 'launch_notes' for cloud replication. \
         Current value: '{}'",
        SYNC_TABLES
    );
}

/// Test: "launch_notes" appears as a complete table name token (not substring of another name).
#[test]
fn cloud_sync_launch_notes_is_complete_token() {
    let tables: Vec<&str> = SYNC_TABLES.split(',').collect();
    assert!(
        tables.contains(&"launch_notes"),
        "SYNC_TABLES must contain 'launch_notes' as a complete comma-delimited token. \
         Tables: {:?}",
        tables
    );
}
