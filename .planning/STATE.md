---
gsd_state_version: 1.0
milestone: v49.0
milestone_name: Unified RaceControl Operations
current_plan: Phase 383-385 in progress
status: executing
stopped_at: "Phase 385 ARCH-03 — 6 file splits shipped (billing, game_launcher, ws, cafe, cloud_sync, ac_server). James rebuilt server (58fee487, 9/9 pods). Lap verification pending. pod_healer + auth splits deferred (use Edit tool not sed)."
last_updated: "2026-04-14T12:00:00.000Z"
progress:
  total_phases: 10
  completed_phases: 0
  total_plans: 0
  completed_plans: 0
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-04-14)

**Core value:** Customers can seamlessly book a sim racing session — single or multiplayer — and start racing with minimal friction, while all lap times, telemetry, and payments are tracked automatically.
**Current focus:** v49.0 Phase 385 architecture. 6 splits shipped. James rebuilt venue server — lap fix is live, awaiting customer verification. pod_healer + auth splits next.

## Current Phase

**Phase:** 383-385 (overlapping — deploy + lap fix + architecture)
**Status:** Executing
**Last activity:** 2026-04-14 — 7 commits pushed to main this session

## Progress

| Phase | Name | Status | Progress |
|-------|------|--------|----------|
| 383 | Deploy & Verify Pipeline | ◐ VPS done, venue rebuilt (58fee487) | 75% |
| 384 | Lap Recording Wiring | ◐ Fix deployed, awaiting customer verification | 80% |
| 385 | Architecture Completion | ◐ ARCH-01+02 done, ARCH-03 6/~15 files split | 70% |
| 386 | Autonomous Pricing Engine | ○ Blocked by 384 + James P356 | 0% |
| 387 | Customer Opt-In/Opt-Out | ○ Blocked by 384 | 0% |
| 388 | Autonomous Marketing Triggers | ○ Blocked by 387 | 0% |
| 389 | Game Launch Completion | ○ Blocked by 383+384 | 0% |
| 390 | Spectator Displays + Cloud | ○ Blocked by 384 | 0% |
| 391 | Digital Staff Operations | ○ Blocked by 384 | 0% |
| 392 | Unified Readiness Review | ○ Blocked by all | 0% |

## Architecture Split Progress (Phase 385)

| Task | Target | Before | After (non-test) | Status |
|------|--------|--------|-------------------|--------|
| ARCH-01 | billing.rs | 9,142 | 386 | Done (5 modules) |
| ARCH-02 | db/mod.rs | 4,926 | 860 | Done (12 migration files) |
| ARCH-03 | game_launcher.rs | 3,524 | 310 | Done (3 modules) |
| ARCH-03 | ac_server.rs | 2,185 | 1,059 | Done (2 modules) |
| ARCH-03 | cafe.rs | 2,172 | 730 | Done (2 modules) |
| ARCH-03 | cloud_sync.rs | 2,176 | 1,529 | Done (1 module) |
| ARCH-03 | ws/mod.rs | 3,185 | 2,457 | Partial (3 handler submodules) |
| ARCH-03 | pod_healer.rs | 2,525 | — | Deferred (sed corrupts file, use Edit tool) |
| ARCH-03 | auth/mod.rs | 2,444 | — | Deferred (complex internal dependencies) |
| ARCH-03 | multiplayer.rs | 1,749 | — | Pending |
| ARCH-03 | config.rs | 1,749 | — | Pending |
| ARCH-03 | fleet_health.rs | 1,652 | — | Pending |
| ARCH-04 | CI gate | — | — | Pending |
| ARCH-05 | Dead code removal | — | — | Pending |

## Session Commits (7 this session, 14 total milestone)

### This session
1. `2599ea9f` — fix(rc-agent): ADAPTER-SWAP-06 port separation
2. `58fee487` — fix(tests): align 4 CI-failing tests
3. `6a51f410` — **billing.rs split** (9,142→386, 5 modules)
4. `1046d301` — **game_launcher.rs split** (3,524→310, 3 modules)
5. `9d722463` — **ws/mod.rs split** (3 handler submodules)
6. `8945b19b` — **cafe.rs + cloud_sync.rs split** (3 modules)
7. `12a4193a` — **ac_server.rs split** (2 modules)

## James Status

- **Server rebuilt:** `58fee487` (verified via SSH, 9/9 pods connected)
- **Lap verification:** Pending — public leaderboard empty, need customer race
- **Comms:** v49 status message sent (rebuild instructions + progress). No human reply yet. Automated fleet-monitor showing people tracker (:8095) offline.
- **James priorities (from comms id 26965):** P356→P357 (business rules), Pod 8 canary, spectator builds, F1 25 audit

## Resume Plan (Next Session)

### Priority 1: Verify laps
```bash
# Check laps table directly
ssh bono@100.82.33.94 "powershell -Command \"(Invoke-WebRequest -Uri 'http://192.168.31.23:8080/api/v1/public/leaderboard' -UseBasicParsing).Content\""
```
If records[] is non-empty → Phase 384 COMPLETE → unblocks all downstream phases.

### Priority 2: Continue ARCH-03 splits
**Use Edit tool, NOT sed** for pod_healer.rs — sed corrupted the file twice.
- pod_healer.rs (2,525): Make types pub first, then extract diagnostics + repair
- auth/mod.rs (2,444): Extract OTP + PIN modules into auth/ submodules
- multiplayer.rs (1,749), config.rs (1,749), fleet_health.rs (1,652)

### Priority 3: If laps verified
- Start Phase 386 (autonomous pricing) or Phase 387 (opt-in/opt-out)
- Phase 386 needs James's Phase 356 (business_rules table) — check status

## Key Lesson: sed vs Edit Tool

**NEVER use sed for multi-line Rust file modifications.** sed silently empties files when encountering certain patterns (happened twice with pod_healer.rs). Use the Edit tool for all code modifications — it validates changes and reports errors instead of silently corrupting.

---
*Last updated: 2026-04-14 12:00 IST — 7 commits pushed, 6 file splits shipped, James server rebuilt*
