# Roadmap: RaceControl

## Completed Milestones

<details>
<summary>v1.0 RaceControl HUD & Safety — 5 phases, 15 plans (Shipped 2026-03-13)</summary>

See [milestones/v1.0-ROADMAP.md](milestones/v1.0-ROADMAP.md) for full phase details and plan breakdown.

Phases: State Wiring & Config Hardening → Watchdog Hardening → WebSocket Resilience → Deployment Pipeline Hardening → Blanking Screen Protocol

</details>

<details>
<summary>v2.0 Kiosk URL Reliability — 6 phases, 12 plans (Shipped 2026-03-14)</summary>

Phases: Diagnosis → Server-Side Pinning → Pod Lock Screen Hardening → Edge Browser Hardening → Staff Dashboard Controls → Customer Experience Polish

</details>

## Current Milestone

### v3.0 Leaderboards, Telemetry & Competitive (Phases 12–15)

**Milestone Goal:** Give customers a public competitive platform — leaderboards, telemetry analysis, group event results, and championships — accessible from their phones via the cloud PWA.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 12: Data Foundation** - Schema migrations, indexes, WAL tuning, and cloud ID resolution — the safe ground every competitive feature builds on (completed 2026-03-14)
- [ ] **Phase 13: Leaderboard Core** - Public leaderboards, circuit/vehicle records, driver profiles, lap validity hardening, and "beaten" notifications — immediate customer value from existing data
- [ ] **Phase 14: Events and Championships** - Hotlap events with 107% rule and badges, group event F1 scoring, multi-round championships, and cloud sync for all competitive tables
- [ ] **Phase 15: Telemetry and Driver Rating** - Speed trace + lap comparison, inputs trace, 2D track map, and percentile-based driver skill classes

## Phase Details

### Phase 12: Data Foundation
**Goal**: The database is correctly indexed, WAL-tuned, and extended with all v3.0 tables — every competitive feature that follows builds on a safe, performant foundation with zero risk of silent data corruption or query performance collapse
**Depends on**: Phase 11 (v2.0 complete)
**Requirements**: DATA-01, DATA-02, DATA-03, DATA-04, DATA-05, DATA-06
**Success Criteria** (what must be TRUE):
  1. A leaderboard query for a specific track returns results in under 50ms with 10,000 laps in the database — verified by query EXPLAIN showing index usage
  2. A telemetry fetch for any lap returns in under 100ms with 500,000 telemetry_samples rows — verified by EXPLAIN showing idx_telemetry_lap_offset used
  3. All six new competitive tables (hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, driver_ratings) exist in the schema and accept valid inserts
  4. A lap sync operation completes without UUID mismatch error — the cloud_driver_id column resolves before any lap is written to competitive tables
  5. The laps table has a car_class column and new laps are automatically assigned a class on completion
**Plans**: 2 plans

Plans:
- [x] 12-01-PLAN.md — Schema infrastructure: WAL tuning, covering indexes, cloud_driver_id, 6 competitive tables (DATA-01 through DATA-05)
- [x] 12-02-PLAN.md — car_class column on laps + persist_lap auto-population (DATA-06)

### Phase 13: Leaderboard Core
**Goal**: Customers can browse public leaderboards, circuit records, vehicle records, and driver profiles from the cloud PWA using existing lap data — and receive an automated email when their track record is broken — all without any login
**Depends on**: Phase 12
**Requirements**: LB-01, LB-02, LB-03, LB-04, LB-05, LB-06, DRV-01, DRV-02, DRV-03, DRV-04, NTF-01, NTF-02, PUB-01, PUB-02
**Success Criteria** (what must be TRUE):
  1. User on a phone can open app.racingpoint.cloud, navigate to any track's leaderboard, filter by car and sim type, and see the fastest valid laps sorted by time — without logging in
  2. User can navigate to circuit records and see the all-time fastest lap per vehicle per circuit, and to vehicle records and see the fastest laps per circuit for a specific vehicle
  3. User can search for a driver by name, open their public profile via a shareable URL, and see their stats, personal bests, and full lap history with sector times
  4. When a track record is beaten, the previous record holder receives an email with the track name, car, old time, new time, new holder name, and a link to the leaderboard
  5. Invalid laps are hidden by default on all leaderboards; user can toggle to show invalid laps
**Plans**: 5 plans

Plans:
- [x] 13-01-PLAN.md — Suspect column + lap validity hardening in persist_lap (LB-05)
- [ ] 13-02-PLAN.md — Leaderboard sim_type filter + circuit/vehicle records endpoints (LB-01, LB-02, LB-03, LB-04, LB-06)
- [ ] 13-03-PLAN.md — Track record "beaten" email notification (NTF-01, NTF-02)
- [ ] 13-04-PLAN.md — Public driver search and profile endpoints (DRV-01, DRV-02, DRV-03, DRV-04)
- [ ] 13-05-PLAN.md — PWA pages: leaderboard, records, driver search, driver profile (PUB-01, PUB-02)

### Phase 14: Events and Championships
**Goal**: Staff can run structured hotlap events and multi-round championships — customers see ranked event leaderboards with 107% rule, gold/silver/bronze badges, F1-scored group results, and cumulative championship standings — all synced to the cloud PWA
**Depends on**: Phase 13
**Requirements**: EVT-01, EVT-02, EVT-03, EVT-04, EVT-05, EVT-06, EVT-07, GRP-01, GRP-02, GRP-03, GRP-04, CHP-01, CHP-02, CHP-03, CHP-04, CHP-05, SYNC-01, SYNC-02, SYNC-03
**Success Criteria** (what must be TRUE):
  1. Staff can create a hotlap event (track, car class, date range, reference time, description) and see it appear on the public events listing page immediately
  2. When a customer drives a lap matching an active event's track, car class, and date range, their lap automatically appears on the event leaderboard without any manual entry
  3. User can open an event leaderboard and see per-class tabs, gold/silver/bronze badges, and laps outside 107% of the class leader flagged
  4. When a multiplayer group session completes, user can view the results page showing position, driver, gap-to-leader, best laps, qual points, race points (F1 25/18/15...), and total
  5. Staff can create a championship, assign group event rounds to it, and users can view the standings table with per-round point breakdowns and F1 tiebreaker ordering
  6. All event, championship, and standings data is visible on app.racingpoint.cloud and reflects venue data within 60 seconds of a change
**Plans**: TBD

### Phase 15: Telemetry and Driver Rating
**Goal**: Customers can compare lap telemetry — speed trace, time delta, throttle/brake/steering, and 2D track map — and see their skill class badge (A/B/C/D) on their profile and leaderboard entries, based on percentile ranking among all drivers
**Depends on**: Phase 14
**Requirements**: TEL-01, TEL-02, TEL-03, TEL-04, TEL-05, RAT-01, RAT-02, RAT-03
**Success Criteria** (what must be TRUE):
  1. User can open any lap's telemetry page and see a speed trace chart with throttle and brake overlaid, plotted against track distance
  2. User can select two laps from the same track and see them compared side-by-side with a time delta channel showing where time was gained or lost, with a linked cursor across all traces
  3. User can view a 2D track map for any lap showing the racing line colored by speed (green fast, red braking zones)
  4. Every driver profile and leaderboard entry shows a class badge (A/B/C/D) that reflects the driver's percentile rank among all drivers on that track and car combination — updating automatically when new laps are recorded
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 12 → 13 → 14 → 15

Note: Phase 14 depends on Phase 13 (event leaderboards extend circuit records patterns and require the public PWA architecture to be validated first). Phase 15 depends on Phase 14 because telemetry comparison derives its value from comparing against event leaders — that context does not exist until events have run.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. State Wiring & Config Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 2. Watchdog Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 3. WebSocket Resilience | v1.0 | 3/3 | Complete | 2026-03-13 |
| 4. Deployment Pipeline Hardening | v1.0 | 3/3 | Complete | 2026-03-13 |
| 5. Blanking Screen Protocol | v1.0 | 3/3 | Complete | 2026-03-13 |
| 6. Diagnosis | v2.0 | 2/2 | Complete | 2026-03-13 |
| 7. Server-Side Pinning | v2.0 | 2/2 | Complete | 2026-03-14 |
| 8. Pod Lock Screen Hardening | v2.0 | 3/3 | Complete | 2026-03-14 |
| 9. Edge Browser Hardening | v2.0 | 1/1 | Complete | 2026-03-14 |
| 10. Staff Dashboard Controls | v2.0 | 2/2 | Complete | 2026-03-14 |
| 11. Customer Experience Polish | v2.0 | 2/2 | Complete | 2026-03-14 |
| 12. Data Foundation | v3.0 | 2/2 | Complete | 2026-03-14 |
| 13. Leaderboard Core | 4/5 | In Progress|  | - |
| 14. Events and Championships | v3.0 | 0/? | Not started | - |
| 15. Telemetry and Driver Rating | v3.0 | 0/? | Not started | - |
