---
phase: 54-structured-logging-error-rate-alerting
plan: "01"
subsystem: racecontrol
tags: [logging, tracing, json, monitoring]
dependency_graph:
  requires: []
  provides: [MON-01]
  affects: [racecontrol-crate]
tech_stack:
  added: ["tracing-subscriber json feature"]
  patterns: ["daily-rotating JSONL file layer", "dual-layer tracing (text stdout + JSON file)", "startup log cleanup"]
key_files:
  created: []
  modified:
    - Cargo.toml
    - crates/racecontrol/src/main.rs
decisions:
  - "File layer uses .json() for structured JSONL; stdout layer stays plain text — no JSON on stdout per requirement"
  - "RollingFileAppender::builder() used over rolling::daily() to produce racecontrol-YYYY-MM-DD.jsonl naming"
  - "cleanup_old_logs() added as standalone fn before main() — deletes .jsonl and .log files older than 30 days on every startup"
  - "with_target(true) on both layers — module path aids incident investigation"
metrics:
  duration: 8
  completed_date: "2026-03-20"
  tasks_completed: 1
  files_modified: 2
---

# Phase 54 Plan 01: Structured JSON Logging Summary

**One-liner:** JSON file logging layer with daily rotation (racecontrol-YYYY-MM-DD.jsonl) and 30-day startup cleanup via tracing-subscriber json feature.

## What Was Built

racecontrol now emits structured JSON log entries to a daily-rotating file while keeping stdout as human-readable text. The file layer uses `.json()` for JSONL output — one JSON object per line — enabling `jq`-based incident investigation. Old logs (`.jsonl` and `.log` files older than 30 days) are cleaned on every startup.

## Tasks Completed

| Task | Description | Commit | Files |
|------|-------------|--------|-------|
| 1 | Enable JSON feature + switch racecontrol file layer to JSON with daily rotation | 451b4c6 | Cargo.toml, crates/racecontrol/src/main.rs |

## Decisions Made

1. **File layer JSON only** — `.json()` applied exclusively to the file layer. Stdout layer remains plain text (human-readable during live ops).
2. **RollingFileAppender::builder()** — used instead of `rolling::daily()` to control filename prefix ("racecontrol-") and suffix ("jsonl"), producing `racecontrol-YYYY-MM-DD.jsonl`.
3. **cleanup_old_logs() as standalone fn** — defined before `main()`, deletes both `.jsonl` and `.log` files (covers old format during migration) older than 30 days.
4. **with_target(true) on both layers** — module path included in every log entry for easier filtering with `jq`.

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

- Cargo.toml modified: `grep '"json"' Cargo.toml` → PASS
- main.rs has `.json()`: `grep '\.json()' crates/racecontrol/src/main.rs` → PASS
- main.rs has `cleanup_old_logs`: `grep 'cleanup_old_logs' crates/racecontrol/src/main.rs` → PASS
- main.rs has `filename_suffix.*jsonl`: PASS
- `cargo check -p racecontrol-crate` → Finished with 0 errors (1 pre-existing unused import warning, out of scope)
- Commit 451b4c6 exists: confirmed
