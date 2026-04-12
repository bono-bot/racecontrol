# Phase 371: Lap Recording — Execution Plan

## Recommendation: Use EXISTING rc-agent adapters (NOT VMS plugin)

All 4 sim adapters are already fully implemented:
- `assetto_corsa.rs` (971 lines) — reads `acpmf_physics/graphics/static` shared memory
- `f1_25.rs` (1,041 lines) — binds UDP port 20777, parses all 5 packet types
- `iracing.rs` (988 lines) — reads `Local\IRSDKMemMapFileName` with dynamic variable lookup
- `lmu.rs` (1,033 lines) — reads `$rFactor2SMMP_Scoring$` and `$rFactor2SMMP_Telemetry$`

Server-side persistence is also wired: `ws/mod.rs` line 892 receives `AgentMessage::LapCompleted`, calls `resolve_driver_for_pod()`, then `persist_lap()` which does `INSERT INTO laps`.

## Why Zero Laps After 43 Days — Root Causes

1. **Adapter swap not wired on LaunchGame** — before `ADAPTER-SWAP-01` (2026-04-12), rc-agent was sim-locked to the adapter chosen at startup. If the game was launched after rc-agent started, the adapter never connected to the running game's telemetry.

2. **Connect timing** — shared memory does not exist until the game creates it. If `adapter.connect()` is called before the game starts, it fails silently with no retry.

3. **`persist_lap()` possibly rejects empty session_id** — if no billing session is active, the lap may be dropped.

## 10 Tasks (Priority Order)

### BLOCKING (must complete for any laps)

**Task 1: Verify adapter rebuild on LaunchGame**
- File: `crates/rc-agent/src/ws_handler.rs` (the ADAPTER-SWAP code from 2026-04-12)
- Verify: when `LaunchGame` is received, the sim adapter is rebuilt for the correct game type
- Test: launch AC from kiosk, check rc-agent logs for "adapter rebuilt for AssettCorsa"
- If broken: fix the adapter swap to rebuild on every LaunchGame, not just at startup

**Task 2: Add connect retry loop**
- File: `crates/rc-agent/src/event_loop.rs`
- Current: `adapter.connect()` called once. If shared memory doesn't exist yet, connection fails permanently.
- Fix: after `LaunchGame`, retry `adapter.connect()` every 2 seconds for up to 60 seconds (game needs time to create shared memory)
- ~20 lines of change

**Task 3: Verify server lap persistence**
- File: `crates/racecontrol/src/ws/mod.rs` line ~892
- Test: send a fake `LapCompleted` WS message → verify row appears in `laps` table
- Check: does `persist_lap()` reject laps when no billing session is active? If yes, add a fallback that records the lap anyway (with `session_id: NULL`)
- ~10 lines of change

### IMPORTANT (fix data quality)

**Task 4: Driver attribution**
- `resolve_driver_for_pod()` must return the correct driver_id for the active session
- Test: verify the lap row has the right driver_id, not NULL or 0

**Task 5: Lap validity**
- AC: check `lapInvalid` flag from graphics shared memory (VMS plugin uses `ac.getCarState(id, acsys.CS.LapInvalidated)`)
- F1 25: check `m_resultStatus` in lap data packet
- Mark invalid laps as `is_valid: false` in the DB (still record them, just flagged)

**Task 6: Sector times**
- AC: extract from `lastSectorTime` in graphics SHM
- F1 25: extract from `m_sector1TimeInMS`, `m_sector2TimeInMS` in lap data packet
- iRacing: extract from session YAML `SplitTimeInfo`
- LMU: extract from `mCurSector` transitions in scoring SHM

### LEADERBOARD (P0 requirement LAPS-05)

**Task 7: Real-time leaderboard push**
- After `persist_lap()`, broadcast `DashboardEvent::LapCompleted` via WS
- PWA leaderboard page subscribes to this event for live updates
- Target: lap appears on leaderboard within 10 seconds of completion

### TELEMETRY (P0 requirement LAPS-06)

**Task 8: Telemetry snapshot recording**
- All 4 adapters already produce telemetry frames
- Verify: telemetry frames are being sent via WS as `AgentMessage::Telemetry`
- Server: write snapshots to `telemetry_samples` table (existing)

### VALIDATION

**Task 9: E2E test per game**
- AC: launch → drive 1 lap → `SELECT * FROM laps WHERE game_type='ac'` has a row
- F1 25: launch → drive 1 lap → `SELECT * FROM laps WHERE game_type='f1_25'` has a row
- iRacing: launch → drive 1 lap → verify
- LMU: launch → drive 1 lap → verify

**Task 10: CSV fallback verification**
- Verify `csv_lap_fallback.rs` writes to `C:\RacingPoint\laps-offline.csv` when WS is down
- Verify laps are synced back when WS reconnects (TODO from VMS audit findings)

## File-by-File Summary

| File | Change | Lines |
|------|--------|-------|
| `event_loop.rs` | Add connect retry loop (2s interval, 60s timeout) | +20 |
| `ws_handler.rs` | Verify ADAPTER-SWAP on LaunchGame | +0 (verify only) |
| `ws/mod.rs` | Allow laps without billing session | +10 |
| `sims/assetto_corsa.rs` | Add lapInvalid flag to LapCompleted | +5 |
| `sims/f1_25.rs` | Add m_resultStatus check | +5 |
| `sims/iracing.rs` | Add sector time extraction | +15 |
| `sims/lmu.rs` | Add sector time extraction | +15 |
| Total | | ~70 new lines |

## Critical Insight

The adapters are COMPLETE. The problem is not telemetry reading — it's the WIRING between launch, adapter connection, and persistence. ~70 lines of fixes in the plumbing, not thousands of lines of new code.

## VMS Plugin — Not Needed But Useful Reference

The VMS AC Python plugin (`VMS Connect.py`) provides richer data via AC's native API (driver names, pit status, leaderboard positions). This is useful for FUTURE enhancements (Phase 377 customer experience) but NOT needed for basic lap recording. The existing shared memory adapters provide lap times, telemetry, and validity — sufficient for P0.

Consider installing the VMS plugin later as a Phase 377 enhancement for richer session data.
