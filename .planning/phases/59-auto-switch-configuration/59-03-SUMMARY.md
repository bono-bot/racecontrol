---
phase: 59-auto-switch-configuration
plan: "03"
subsystem: rc-agent / ffb_controller
tags: [gap-closure, conspit-link, ffb, venue-games, prof-02]
dependency_graph:
  requires: ["59-01", "59-02"]
  provides: ["PROF-02 fully satisfied", "VENUE_GAME_KEYS 4-entry confirmed"]
  affects: ["verify_game_to_base_config runtime behavior on all pods"]
tech_stack:
  added: []
  patterns: ["Pod hardware inspection via Tailscale SSH", "RCAGENT_SELF_RESTART deploy pattern"]
key_files:
  modified:
    - crates/rc-agent/src/ffb_controller.rs
decisions:
  - "4th game key is ASSETTO_CORSA_EVO (uppercase-underscore style) — confirmed from Pod 8 GameToBaseConfig.json hardware inspection"
  - "AC Rally (ASSETTO_CORSA_RALLY) not added — not an active venue game at Racing Point"
metrics:
  duration_secs: 284
  completed_date: "2026-03-24T13:29:45Z"
  tasks_completed: 1
  files_modified: 1
requirements_satisfied: [PROF-02]
---

# Phase 59 Plan 03: Add 4th VENUE_GAME_KEY (Gap Closure) Summary

**One-liner:** Added confirmed 4th venue game key `ASSETTO_CORSA_EVO` to `VENUE_GAME_KEYS` from Pod 8 hardware inspection, rebuilt rc-agent, deployed to Pod 8 canary — PROF-02 fully satisfied.

## What Was Built

VENUE_GAME_KEYS in `crates/rc-agent/src/ffb_controller.rs` updated from 3 to 4 confirmed entries. The 4th game key `ASSETTO_CORSA_EVO` was identified by reading `GameToBaseConfig.json` directly from Pod 8 hardware via Tailscale SSH, confirming the exact key string ConspitLink 2.0 uses.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Inspect Pod 8 GameToBaseConfig.json and add 4th game key | 5ae28d8b | crates/rc-agent/src/ffb_controller.rs |

## Verification Results

- `VENUE_GAME_KEYS` contains 4 entries: `"Assetto Corsa"`, `"F1 25"`, `"Assetto Corsa Competizione"`, `"ASSETTO_CORSA_EVO"`
- No TBD or Phase 61 comments remain in the constant
- `cargo test -p rc-agent-crate ffb_controller` — **37 passed, 0 failed**
- Pod 8 health endpoint: `{"build_id":"5ae28d8b","status":"ok","uptime_secs":7}`
- Startup log: `phase=self_heal no_repairs_needed` — confirms all 4 keys found in GameToBaseConfig.json

## Pod 8 Hardware Inspection Results

GameToBaseConfig.json on Pod 8 (`C:\Program Files (x86)\Conspit Link 2.0\JsonConfigure\GameToBaseConfig.json`) contains these keys with `C:\...\Presets\` style paths (actively configured venue games):
- `"Assetto Corsa"` — already in VENUE_GAME_KEYS
- `"Assetto Corsa Competizione"` — already in VENUE_GAME_KEYS
- `"F1 25"` — already in VENUE_GAME_KEYS
- `"ASSETTO_CORSA_EVO"` — **4th confirmed key (uppercase-underscore style)**
- `"ASSETTO_CORSA_RALLY"` — present but NOT an active venue game at Racing Point

## Decisions Made

1. **Key format**: `ASSETTO_CORSA_EVO` uses the uppercase-underscore style (ConspitLink 2.0 newer game format). This differs from the friendly-name style used for older games. The exact string must match what ConspitLink writes — confirmed from hardware.

2. **AC Rally excluded**: `ASSETTO_CORSA_RALLY` exists in the config but is not an active venue game. Only AC, F1 25, ACC, and AC EVO are venue games. Adding non-venue keys would cause false "missing entry" insertions.

## Deploy Sequence Used

1. `touch crates/rc-agent/build.rs` + `cargo build --release --bin rc-agent`
2. `cp target/release/rc-agent.exe deploy-staging/`
3. HTTP server on :9998 already running
4. `POST http://192.168.31.91:8090/exec` with `curl.exe -o C:\RacingPoint\rc-agent-new.exe http://192.168.31.27:9998/rc-agent.exe`
5. `POST http://192.168.31.91:8090/exec` with `RCAGENT_SELF_RESTART` sentinel
6. rc-agent didn't auto-restart — used Tailscale SSH: `schtasks /Run /TN StartRCAgent`
7. Health check confirmed build_id `5ae28d8b`, uptime 7s

## Deviations from Plan

### Auto-fixed Issues

None — plan executed as written.

### Notes

- Self-restart via `RCAGENT_SELF_RESTART` sentinel timed out (connection reset). Recovered via Tailscale SSH `schtasks /Run /TN StartRCAgent` as specified in the fallback plan. This is an existing known issue with the self-restart path, not a regression.
- The startup log `phase=self_heal no_repairs_needed` is the positive confirmation that ASSETTO_CORSA_EVO was found in GameToBaseConfig.json (no key was missing, no insertion needed).

## Self-Check: PASSED

- `crates/rc-agent/src/ffb_controller.rs` — FOUND
- Commit `5ae28d8b` — FOUND in git log
- `59-03-SUMMARY.md` — FOUND
- Pod 8 health `build_id: 5ae28d8b` — VERIFIED
