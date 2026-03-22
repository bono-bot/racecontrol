---
plan: 171-01
phase: 171
title: Bug Fixes — Auto-Seed, Process Guard, Bat Confirmations
status: partial
started: 2026-03-23
completed: 2026-03-23
---

## What Was Built

1. **BUG-01 — Auto-seed pods on startup** (`crates/racecontrol/src/main.rs`): Added `seed_pods_on_startup()` function that checks if pods table is empty and inserts all 8 pod records with correct IPs and MAC addresses. Called after `AppState::new()` in `main()`.

2. **BUG-03 — Process guard report_only** (`racecontrol.toml`, `deploy-staging/racecontrol.toml`): Added `[process_guard]` section with `enabled = true`, `violation_action = "report_only"`, 60s poll interval, and baseline allowlist of 16 known-good processes. Variable_dump.exe deliberately excluded.

3. **BUG-02 + BUG-04** — Confirmed bat fixes already present in `deploy-staging/start-rcagent.bat` (line 5: Variable_dump kill, line 12: orphan PowerShell kill). No code changes needed.

## Deferred

- Live deployment to server and pods (infrastructure offline)
- All 4 bug verification checks (require live environment)

## Commits

| Hash | Description |
|------|-------------|
| 5f5bbd50 | feat(171-01): auto-seed 8 pods on server startup (BUG-01) |
| 9c431952 | feat(171-01): enable process guard report_only mode in config (BUG-03) |

## Self-Check: PARTIAL

Code changes complete. Live verification deferred.
