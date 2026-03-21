---
phase: 106-structured-log-labels
plan: 05
subsystem: rc-agent
tags: [structured-logging, tracing, targets, migration]
dependency_graph:
  requires: []
  provides: [LOG-02, LOG-03]
  affects: [rc-agent]
tech_stack:
  added: []
  patterns: [tracing structured target labels, LOG_TARGET const per module]
key_files:
  created: []
  modified:
    - crates/rc-agent/src/udp_heartbeat.rs
    - crates/rc-agent/src/failure_monitor.rs
    - crates/rc-agent/src/process_guard.rs
    - crates/rc-agent/src/firewall.rs
    - crates/rc-agent/src/debug_server.rs
    - crates/rc-agent/src/content_scanner.rs
    - crates/rc-agent/src/config.rs
    - crates/rc-agent/src/self_test.rs
    - crates/rc-agent/src/sims/assetto_corsa.rs
    - crates/rc-agent/src/sims/assetto_corsa_evo.rs
    - crates/rc-agent/src/sims/iracing.rs
    - crates/rc-agent/src/sims/f1_25.rs
    - crates/rc-agent/src/sims/lmu.rs
decisions:
  - "driving_detector.rs has 0 tracing calls — LOG_TARGET const not added (no-op)"
  - "assetto_corsa_evo.rs self.log_prefix retained in message body (differentiates EVO vs Rally at runtime)"
  - "lmu.rs and failure_monitor.rs bracket prefixes stripped in message text (redundant with target label)"
metrics:
  duration_secs: 615
  completed_date: "2026-03-21"
  tasks_completed: 2
  files_modified: 13
---

# Phase 106 Plan 05: Remaining rc-agent Files Structured Log Migration Summary

All 14 remaining rc-agent source files migrated from plain `tracing::*!()` calls to structured `target: LOG_TARGET` labels. 100% of rc-agent tracing calls now use structured labels.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Migrate utility/monitor files (9 files) | 8dbd3ba | udp_heartbeat, failure_monitor, process_guard, firewall, debug_server, content_scanner, config, self_test |
| 2 | Migrate sim module files (5 files) | 046f6a7 | assetto_corsa, assetto_corsa_evo, iracing, f1_25, lmu |

## LOG_TARGET Labels Applied

| File | Label |
|------|-------|
| udp_heartbeat.rs | `udp` |
| failure_monitor.rs | `failure-monitor` |
| process_guard.rs | `process-guard` |
| firewall.rs | `firewall` |
| debug_server.rs | `debug-server` |
| content_scanner.rs | `content-scanner` |
| config.rs | `config` |
| self_test.rs | `self-test` |
| sims/assetto_corsa.rs | `sim-ac` |
| sims/assetto_corsa_evo.rs | `sim-ac-evo` |
| sims/iracing.rs | `sim-iracing` |
| sims/f1_25.rs | `sim-f1` |
| sims/lmu.rs | `sim-lmu` |

## Call Sites Migrated

- Task 1 (utility/monitor): 36 call sites across 8 files
- Task 2 (sim modules): 32 call sites across 5 files
- driving_detector.rs: 0 call sites — const not added

## Deviations from Plan

None — plan executed exactly as written.

## Self-Check: PASSED

All 13 files modified. Commits verified:
- 8dbd3ba: feat(106-05): migrate utility/monitor files to structured target: labels
- 046f6a7: feat(106-05): migrate sim module files to structured target: labels

cargo check passes (Finished dev profile).
