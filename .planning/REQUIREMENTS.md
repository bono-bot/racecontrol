# Requirements: RaceControl v3.0 Leaderboards, Telemetry & Competitive

**Defined:** 2026-03-14
**Core Value:** Customers see their lap times, compete on leaderboards, and compare telemetry — driving repeat visits and social sharing from a publicly accessible cloud PWA.

## v1 Requirements

Requirements for milestone v3.0. Each maps to roadmap phases.

### Data Foundation

- [x] **DATA-01**: Database has composite covering indexes on laps table for leaderboard queries (track, car, valid, lap_time_ms)
- [x] **DATA-02**: Database has index on telemetry_samples (lap_id, offset_ms) for telemetry visualization
- [x] **DATA-03**: SQLite WAL checkpoint is tuned (wal_autocheckpoint=400, connection max_lifetime=300s) to prevent read latency growth
- [x] **DATA-04**: Venue drivers table has cloud_driver_id column that resolves UUID mismatch before lap sync
- [x] **DATA-05**: Database schema includes hotlap_events, hotlap_event_entries, championships, championship_rounds, championship_standings, and driver_ratings tables
- [x] **DATA-06**: Laps table has car_class column populated from car-to-class mapping on lap completion

### Leaderboards & Records

- [x] **LB-01**: User can view public leaderboard for any track, filtered by car and sim_type, sorted by fastest valid lap time
- [x] **LB-02**: User can view circuit records page showing the all-time fastest lap per vehicle per circuit
- [x] **LB-03**: User can view vehicle records page showing the fastest lap per circuit for a given vehicle
- [x] **LB-04**: Leaderboard endpoints require sim_type filter — AC and F1 25 laps are never mixed on the same board
- [x] **LB-05**: Lap validity is hardened with sanity range check and sector-sum consistency before accepting a lap as valid
- [x] **LB-06**: Only valid laps appear on leaderboards by default; user can toggle to show invalid laps

### Driver Profiles

- [ ] **DRV-01**: User can search for any driver by name and view their public profile page (no login required)
- [ ] **DRV-02**: Driver profile shows stats cards: total laps, total time, personal bests per track/car, class badge
- [ ] **DRV-03**: Driver profile shows full lap history with circuit, vehicle, date, time, S1/S2/S3 sector times
- [ ] **DRV-04**: Driver profile is accessible via shareable URL (e.g. /drivers/{id} or /drivers?name=X)

### Hotlap Events

- [ ] **EVT-01**: Staff can create a hotlap event with track, car class(es), start date, end date, description, and optional reference time
- [ ] **EVT-02**: Laps automatically enter the matching hotlap event when track, car class, and date range match
- [ ] **EVT-03**: User can view public event leaderboard showing position, driver, time, date, vehicle, and venue
- [ ] **EVT-04**: Event leaderboard displays car class tabs — one ranking per class within the event
- [ ] **EVT-05**: 107% rule is enforced — laps slower than 107% of the class leader are flagged as outside representative pace
- [ ] **EVT-06**: Gold/Silver/Bronze badges are auto-calculated from staff-set reference time (within 2%/5%/8%)
- [ ] **EVT-07**: User can browse all active and past hotlap events from an events listing page

### Group Events & Scoring

- [ ] **GRP-01**: When a multiplayer group session completes, race results are auto-scored using F1 points (25/18/15/12/10/8/6/4/2/1)
- [ ] **GRP-02**: User can view group event summary showing position, driver, qual points, race points, best laps, wins, total points
- [ ] **GRP-03**: User can view per-session breakdowns within a group event (qualification, race)
- [ ] **GRP-04**: Group event results include gap-to-leader timing for each driver

### Championships

- [ ] **CHP-01**: Staff can create a championship with name, description, and assign group events as rounds
- [ ] **CHP-02**: Championship standings are auto-calculated by summing F1 points across rounds
- [ ] **CHP-03**: User can view championship standings page with overall table and per-round breakdown
- [ ] **CHP-04**: Championship tiebreaker follows F1 rules: most wins, then most P2s, then most P3s, then earliest occurrence
- [ ] **CHP-05**: Event entries have result_status (finished/DNS/DNF/pending) for correct scoring of incomplete results

### Telemetry Visualization

- [ ] **TEL-01**: User can view speed trace chart for any lap (speed vs track distance/time with throttle/brake overlay)
- [ ] **TEL-02**: User can compare two laps side-by-side — speed trace with time delta channel showing where time was gained/lost
- [ ] **TEL-03**: Inputs trace shows throttle, brake, and steering angle plotted alongside the speed trace with linked cursor
- [ ] **TEL-04**: User can view 2D track map overlay showing racing line colored by speed (green=fast, red=braking)
- [ ] **TEL-05**: Telemetry comparison allows selecting personal best vs track record, or any two laps from the same track

### Driver Skill Rating

- [ ] **RAT-01**: Drivers are assigned a skill class (A/B/C/D) per track+car combination based on percentile ranking of their best valid lap
- [ ] **RAT-02**: Driver class badge is displayed on profile page and leaderboard entries
- [ ] **RAT-03**: Skill class recalculates automatically when new laps are recorded (requires minimum 5 drivers per track+car before classification)

### Notifications

- [ ] **NTF-01**: When a track record is beaten, the previous record holder receives an automated email via send_email.js
- [ ] **NTF-02**: Notification email includes track, car, old time, new time, and new record holder name with a link to the leaderboard

### Cloud Sync

- [ ] **SYNC-01**: Cloud sync extends to push hotlap_events, event_entries, championships, standings, and driver_ratings to app.racingpoint.cloud
- [ ] **SYNC-02**: Telemetry sync is targeted — only event-entered lap telemetry is synced (bounded volume, not all laps)
- [ ] **SYNC-03**: Competitive data sync is venue-authoritative one-way push — cloud never writes back to venue competitive tables

### Public Access

- [ ] **PUB-01**: All leaderboard, records, events, championships, and driver profile pages are accessible without login
- [ ] **PUB-02**: PWA pages are mobile-first with responsive tables/cards (minimum 14px for times, 16px for positions)

## v2 Requirements

Deferred to future milestone. Tracked but not in current roadmap.

### Social & Sharing

- **SHARE-01**: Auto-generated OG image share card (position, time, track, car, branding) for WhatsApp/iMessage
- **SHARE-02**: Discord bot posts leaderboard updates to venue Discord server

### Advanced Visualization

- **VIZ-01**: Mini-map showing car position on track in real-time during live sessions (spectator mode)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Real-time WebSocket leaderboard push | At 8 pods, updates every ~15s — polling every 30s is visually identical. Adds complexity for zero benefit at venue scale. |
| Login/account system for browsing | racecentres.com is fully public. Adding login adds friction that kills organic sharing. PIN-linked driver IDs are sufficient. |
| Social feed / comments / likes | Social moderation is a full-time job. Venue scale doesn't generate enough content for a feed. |
| Video replay / session recording | Storage and processing requirements incompatible with venue hardware. Telemetry visualization serves the same coaching use case. |
| Cross-car performance normalization | No calibrated lap time model exists per car. Keep leaderboards within same car or car class. |
| Global multi-venue leaderboards | Requires proprietary API integration with closed racecentres.com VMS ecosystem. |
| Elo/Glicko rating system | Designed for head-to-head matchups, not time-trials. Percentile-based class is correct for venue context. |
| Venue kiosk changes | v3.0 targets cloud PWA only. Kiosk spectator display is a future milestone. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| DATA-01 | Phase 12 | Complete |
| DATA-02 | Phase 12 | Complete |
| DATA-03 | Phase 12 | Complete |
| DATA-04 | Phase 12 | Complete |
| DATA-05 | Phase 12 | Complete |
| DATA-06 | Phase 12 | Complete |
| LB-01 | Phase 13 | Complete |
| LB-02 | Phase 13 | Complete |
| LB-03 | Phase 13 | Complete |
| LB-04 | Phase 13 | Complete |
| LB-05 | Phase 13 | Complete |
| LB-06 | Phase 13 | Complete |
| DRV-01 | Phase 13 | Pending |
| DRV-02 | Phase 13 | Pending |
| DRV-03 | Phase 13 | Pending |
| DRV-04 | Phase 13 | Pending |
| NTF-01 | Phase 13 | Pending |
| NTF-02 | Phase 13 | Pending |
| PUB-01 | Phase 13 | Pending |
| PUB-02 | Phase 13 | Pending |
| EVT-01 | Phase 14 | Pending |
| EVT-02 | Phase 14 | Pending |
| EVT-03 | Phase 14 | Pending |
| EVT-04 | Phase 14 | Pending |
| EVT-05 | Phase 14 | Pending |
| EVT-06 | Phase 14 | Pending |
| EVT-07 | Phase 14 | Pending |
| GRP-01 | Phase 14 | Pending |
| GRP-02 | Phase 14 | Pending |
| GRP-03 | Phase 14 | Pending |
| GRP-04 | Phase 14 | Pending |
| CHP-01 | Phase 14 | Pending |
| CHP-02 | Phase 14 | Pending |
| CHP-03 | Phase 14 | Pending |
| CHP-04 | Phase 14 | Pending |
| CHP-05 | Phase 14 | Pending |
| SYNC-01 | Phase 14 | Pending |
| SYNC-02 | Phase 14 | Pending |
| SYNC-03 | Phase 14 | Pending |
| TEL-01 | Phase 15 | Pending |
| TEL-02 | Phase 15 | Pending |
| TEL-03 | Phase 15 | Pending |
| TEL-04 | Phase 15 | Pending |
| TEL-05 | Phase 15 | Pending |
| RAT-01 | Phase 15 | Pending |
| RAT-02 | Phase 15 | Pending |
| RAT-03 | Phase 15 | Pending |

**Coverage:**
- v1 requirements: 47 total (note: requirements doc self-count of 42 is off; 47 are listed above)
- Mapped to phases: 47
- Unmapped: 0

---
*Requirements defined: 2026-03-14*
*Last updated: 2026-03-14 after roadmap created (traceability populated)*
