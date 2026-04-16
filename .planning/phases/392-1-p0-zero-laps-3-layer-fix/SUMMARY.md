# Phase 392-1: P0 Zero Laps 3-Layer Fix — SUMMARY

**Status:** PARTIAL — Layer 1 deployed, Layers 2+3 deferred, apps preset fix found separately
**Milestone:** v49.0 Unified Operations
**Layer 1 Deployed:** 2026-04-16

## What Was Done

### Layer 1: Per-minute minimum duration floor (DEPLOYED)
- Added 3-minute (180s) minimum floor for per-minute billing: `33717300`
- Fixed None duration rejection for open-ended per-minute sessions: `3c854979`
- Server .23 fully current at `3c854979` = git HEAD

### Separate finding: Apps preset missing RaceControl plugin
- Root cause of zero laps: `write_apps_preset()` in `ac_launcher.rs` excluded `[RACECONTROL]` app
- AC never loaded the RaceControl Python plugin → no `rcpmf_telemetry` shared memory → no laps
- Fix committed: `d4b6247d` — adds `[RACECONTROL] ACTIVE=1 VISIBLE=0` to preset
- NOT DEPLOYED to pods (requires rc-agent rebuild)

### Layer 2: Kiosk UX warning — DEFERRED
- Needs per-track x per-car reference lap data (not yet located)

### Layer 3: Server grace window — DEFERRED
- Research completed (`RESEARCH-layer3-grace-window.md`)
- Needs reference lap data + integration test + MMA audit

## Commits

| Hash | Description |
|------|-------------|
| `33717300` | fix(392.1): add 3-minute minimum floor for per-minute billing |
| `3c854979` | fix(392.1): allow None duration for open-ended per-minute billing |
| `d4b6247d` | fix(rc-agent): add RaceControl plugin to AC apps-default.ini preset |

## What Remains

1. Pod fleet rebuild with `d4b6247d` (apps preset fix) — ALL 8 pods + POS
2. Live E2E verification: staff runs session → lap appears in `laps` table
3. Layer 2 + Layer 3 in future phases
4. FK pragma verification (d24b17f7) on both server + cloud

---

*Summary written 2026-04-16 to close phase gap artifact.*
