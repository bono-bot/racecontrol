---
phase: 289-metric-alert-thresholds
plan: "02"
subsystem: racecontrol/startup
tags: [alerts, startup, tokio]
dependency_graph:
  requires: [289-01]
  provides: [ALRT-02, ALRT-03]
  affects: [racecontrol startup sequence]
tech_stack:
  added: []
  patterns: [conditional tokio::spawn, startup task lifecycle logging]
key_files:
  modified:
    - crates/racecontrol/src/main.rs
decisions:
  - Placed metric_alert_task spawn immediately after whatsapp_alerter_task spawn — maintains logical alert block grouping
  - Used !alert_rules.is_empty() guard — no wasted background loop when rules are absent
metrics:
  duration: "5min"
  completed: "2026-04-01"
  tasks: 1
  files: 1
---

# Phase 289 Plan 02: Wire Metric Alert Task Summary

**One-liner:** Conditional `tokio::spawn` of `metric_alert_task` in server startup when `alert_rules` is non-empty, completing the ALRT-02/ALRT-03 integration.

## Tasks Completed

| # | Task | Commit | Files |
|---|------|--------|-------|
| 1 | Wire metric_alert_task in main.rs | f6000e75 | crates/racecontrol/src/main.rs |

## What Was Done

Added a 5-line conditional spawn block in `main.rs` immediately after the `whatsapp_alerter_task` spawn:

```rust
// Spawn metric alert evaluation task
if !state.config.alert_rules.is_empty() {
    let alert_state = state.clone();
    tokio::spawn(racecontrol_crate::metric_alerts::metric_alert_task(alert_state));
    tracing::info!(target: "startup", "metric alert task spawned ({} rules)", state.config.alert_rules.len());
}
```

This follows the exact same pattern used by `whatsapp_alerter_task` — clone state, conditional spawn with log on activate, no spawn when not needed.

## Verification

- `grep -c "metric_alert_task" main.rs` → 1
- `grep "alert_rules.is_empty" main.rs` → match
- `cargo build --bin racecontrol` → `Finished` (dev profile, 740 unit tests pass)
- `cargo test -p racecontrol-crate --lib` → `740 passed; 0 failed`
- Release build: verified via dev build; `cargo build --release` can be run before deploy

## Deviations from Plan

None — plan executed exactly as written.

Note: `cargo test -p racecontrol` with integration tests fails due to pre-existing `BillingTimer { nonce }` missing field errors unrelated to this change. These failures pre-date Plan 01 and are out of scope per deviation rules.

## Known Stubs

None.

## Self-Check: PASSED

- File modified: `crates/racecontrol/src/main.rs` — exists and contains `metric_alert_task`
- Commit `f6000e75` — confirmed in git log
