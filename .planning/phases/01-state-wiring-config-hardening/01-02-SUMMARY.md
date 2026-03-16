---
phase: 01-state-wiring-config-hardening
plan: "02"
subsystem: pod-agent, deploy-tooling
tags: [pod-agent, deploy, http-status, exec, config]
dependency_graph:
  requires: []
  provides: [honest-exec-http-codes, deploy-pod-script, rc-agent-config-template]
  affects: [racecontrol/pod_monitor, deploy-staging/workflow]
tech_stack:
  added: [serial_test, http-body-util, tower (dev-deps)]
  patterns: [TDD-red-green, argparse-cli, template-substitution]
key_files:
  created:
    - deploy/deploy_pod.py
    - deploy/rc-agent.template.toml
  modified:
    - ../pod-agent/src/main.rs
    - ../pod-agent/Cargo.toml
decisions:
  - "Used serial_test to prevent global semaphore contention in unit tests — cleaner than artificially inflating MAX_CONCURRENT_EXECS or adding sleep delays"
  - "Committed deploy scripts under racecontrol/deploy/ since deploy-staging/ is not a git repo — preserves git history for operational scripts"
  - "LAN bind falls back to 0.0.0.0 with a warning log rather than panicking — pods without 192.168. IP can still function"
metrics:
  duration: "8m 30s"
  completed_date: "2026-03-12"
  tasks_completed: 2
  files_changed: 4
---

# Phase 1 Plan 2: pod-agent Exec Fix and Deploy Helper Summary

**One-liner:** Fixed pod-agent /exec to return HTTP 500 for failures (not 200 for everything), added serial-safe unit tests, LAN-only bind, and a template-based Python deploy script with explicit config delete before write.

## What Was Built

### Task 1: pod-agent /exec honest HTTP status codes

Changed `../pod-agent/src/main.rs`:

- Added `success: bool` field to `ExecResponse` struct
- `/exec` now returns HTTP 200 only when the command exits with code 0
- Returns HTTP 500 for: non-zero exit code, spawn failure (bad executable), timeout
- Returns HTTP 429 for semaphore exhaustion (unchanged behavior, added `success: false`)
- LAN-only bind: detects 192.168.x.x via `local_ip()` and binds there; falls back to 0.0.0.0 with warning log

Added 5 unit tests (`#[cfg(test)] mod tests`):
- `test_exec_success_echo` — HTTP 200, success=true for `echo hello`
- `test_exec_nonzero_exit_returns_500` — HTTP 500 for `cmd /C exit 1`
- `test_exec_invalid_command_returns_500` — HTTP 500 for nonexistent binary
- `test_exec_timeout_returns_500` — HTTP 500, exit_code=124 for ping with 500ms timeout
- `test_exec_response_always_has_success_field` — presence check

Added dev-deps: `tower`, `http-body-util`, `serial_test` (prevents semaphore contention in parallel test runs).

**Why this matters:** `racecontrol/pod_monitor.rs` checks `resp.status().is_success()` to decide if a restart command succeeded. Previously all responses were HTTP 200, so pod_monitor thought failed restarts succeeded and stopped retrying.

### Task 2: Deploy helper script

Created `deploy/deploy_pod.py`:
- Argparse CLI with `--config-only`, `--binary-url`, and `all` pod target
- 5-step deploy flow: kill, delete config, write config, download binary, start
- Uses pod-agent `/write` for config push (not shell redirect — avoids escaping issues)
- Explicit `del /Q C:\RacingPoint\rc-agent.toml` before `/write` — defense in depth
- `generate_config(pod_number)` renders template with `{pod_number}` and `{pod_name}` substitution
- Full 8-pod IP map hardcoded

Created `deploy/rc-agent.template.toml`:
- `{pod_number}` and `{pod_name}` as the only per-pod variables
- Shared: `server_url` points to `.23`, `ollama_url` points to `.27`
- All 5 games configured (AC, F1 25, LMU, Forza, iRacing)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical Functionality] Added serial_test to prevent semaphore contention**
- **Found during:** Task 1 — TDD GREEN phase, first test run
- **Issue:** Global `EXEC_SEMAPHORE` has 4 permits; 5 parallel tests exhausted all permits causing `test_exec_success_echo` to receive HTTP 429 instead of HTTP 200
- **Fix:** Added `serial_test = "3"` dev-dependency; annotated all 5 tests with `#[serial]` to ensure sequential execution
- **Files modified:** `../pod-agent/Cargo.toml`, `../pod-agent/src/main.rs`
- **Commit:** 340e0e0 (part of Task 1 commit)

**2. [Rule 3 - Blocking] Committed deploy scripts to racecontrol/deploy/ instead of deploy-staging/**
- **Found during:** Task 2 commit
- **Issue:** `deploy-staging/` at `C:/Users/bono/racingpoint/deploy-staging/` is not a git repository — cannot commit there
- **Fix:** Copied `deploy_pod.py` and `rc-agent.template.toml` to `racecontrol/deploy/` for git tracking. Operational copies remain in `deploy-staging/` for day-to-day use.
- **Files modified:** `deploy/deploy_pod.py`, `deploy/rc-agent.template.toml` (new)
- **Commit:** 0bf72e9

## Success Criteria Verification

- [x] `cargo test` in pod-agent passes with 5 new tests proving HTTP 500 for failures
- [x] `ExecResponse` struct has `success: bool` field
- [x] `/exec` returns 200 only for zero-exit commands, 500 for everything else
- [x] pod-agent binds to LAN IP (192.168.31.x) with 0.0.0.0 fallback
- [x] `deploy_pod.py` imports and shows `--help` without errors
- [x] `rc-agent.template.toml` has `{pod_number}` as the only per-pod variable (plus `{pod_name}`)
- [x] `deploy_pod.py` includes explicit config delete step before write

## Commits

| Hash | Repo | Description |
|------|------|-------------|
| 340e0e0 | pod-agent | fix(01-02): honest HTTP status codes in /exec + LAN bind + tests |
| 0bf72e9 | racecontrol | feat(01-02): deploy helper script with config cleanup for remote deploys |

## Self-Check: PASSED

Files verified:
- `C:/Users/bono/racingpoint/pod-agent/src/main.rs` — exists, contains `success: bool`
- `C:/Users/bono/racingpoint/racecontrol/deploy/deploy_pod.py` — exists, --help works
- `C:/Users/bono/racingpoint/racecontrol/deploy/rc-agent.template.toml` — exists, has `{pod_number}`
- `C:/Users/bono/racingpoint/deploy-staging/deploy_pod.py` — exists (operational copy)
- `C:/Users/bono/racingpoint/deploy-staging/rc-agent.template.toml` — exists (operational copy)

Commits verified:
- 340e0e0 — pod-agent main branch
- 0bf72e9 — racecontrol main branch (pushed to github.com:bono-bot/racecontrol.git)
