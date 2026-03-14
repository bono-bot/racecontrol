# Project Research Summary

**Project:** RaceControl v3.0 — Leaderboards, Telemetry & Competitive
**Domain:** Sim racing competitive platform — hotlap events, championships, telemetry visualization, driver rating
**Researched:** 2026-03-14
**Confidence:** HIGH

## Executive Summary

RaceControl v3.0 is an extension of an existing venue management system (Rust/Axum + SQLite + Next.js PWA) that adds a public competitive layer: hotlap events, championship standings, driver profiles, and telemetry visualization. Research across competitor platforms (racecentres.com, Multitap, LapLegends, simracing.gp) and direct codebase inspection confirms the core insight: the leaderboard is the product, everything else is infrastructure to make laps worth submitting. The single highest-ROI feature for driving repeat visits is the automated "you've been beaten" notification — proven at comparable venues. The recommended approach is to build the leaderboard core (circuit records, driver profiles, hotlap events) as the v3.0 foundation, then layer telemetry visualization and championships in subsequent sub-phases once the engagement loop is validated.

The stack requires zero new Rust crates and only one new npm package (date-fns 4.1.0). All competitive features are achievable with the existing recharts, SQLite window functions, and Axum routing infrastructure already in production. The cloud sync architecture already handles the venue-to-cloud push pattern; v3.0 extends it with 5 new tables and targeted telemetry sync (event laps only). The most critical architectural decision — keeping competitive data venue-authoritative and one-directional to the cloud — must be enforced from day one to prevent sync corruption.

Four critical risks require immediate attention before any feature work: telemetry_samples has no index (full table scan will make visualization unusable within weeks), the laps table lacks leaderboard indexes (performance degrades as lap count grows), the cloud sync has a known driver ID mismatch problem that will corrupt competitive data when lap sync is added, and the WAL checkpoint is not tuned (read latency degrades under concurrent load). All four must be resolved in the first phase before any competitive features are built on top.

---

## Key Findings

### Recommended Stack

The v3.0 stack adds zero new Rust crates and one npm package to an already-working system. recharts 3.8.0 (already installed) handles lap comparison via ComposedChart with two Line components and syncId. The 2D track map uses the Canvas API via useRef — 50 lines of vanilla code that outperforms D3 (80KB) at 5,400 sample points on mobile. SQLite window functions (PERCENT_RANK, ROW_NUMBER) are available via the bundled SQLite 3.25.0+ and handle all ranking queries natively. A custom percentile-based driver rating in Rust is the correct algorithm choice — Elo/Glicko require head-to-head match outcomes and produce meaningless ratings for time-trial data.

**Core technologies:**
- Rust/Axum 0.8: backend — extend with championship.rs, driver_rating.rs, hotlap_events.rs modules; zero new crates
- sqlx 0.8 + SQLite (WAL): storage — add 5 new tables + indexes; window functions available for ranking
- Next.js App Router + React 19: PWA — 6 new pages + 6 new components; recharts already installed
- recharts 3.8.0: charting — lap comparison via ComposedChart + syncId; no new charting library needed
- Canvas API (browser built-in): 2D track map from pos_x/pos_z telemetry; zero bundle cost
- date-fns 4.1.0: the only new npm dependency — event timelines and relative timestamps; 13KB tree-shakeable ESM
- cloud_sync.rs (existing): extend collect_push_payload() with 5 new tables + event-only telemetry sync

### Expected Features

**Must have (table stakes) — launch with v3.0:**
- Public leaderboard by track with car filter — the core product; racecentres.com has had this since launch
- Circuit records (best per car per track) — immediately populated from existing laps data, zero events needed
- Vehicle records (best per track per car) — same query infrastructure as circuit records
- Driver profile page — lap history, PBs, stats cards, accessible by name search, no login required
- Hotlap events — staff creation UI, public event leaderboard, car class support (A/B/C/D)
- 107% rule enforcement — display filter within event board; toggle to show all times
- Gold/Silver/Bronze badges — staff sets reference time on event creation, auto-calculated per entry
- Group event results with F1 scoring — 25/18/15/12/10/8/6/4/2/1 points
- Automated "beaten" notification — email when track record is broken; reuses existing send_email.js

**Should have (competitive advantage) — add in v3.x after validation:**
- Telemetry speed trace + time delta — highest-value "wow" feature; self-contained infrastructure
- Telemetry inputs trace — throttle/brake/steering alongside speed; shared infrastructure with speed trace
- Driver skill class (A/B/C/D) — percentile-based; wait until sufficient lap data accumulates
- Multi-round championships — depends on at least 2 scored group events to validate data model

**Defer to v4+:**
- 2D track map overlay — high complexity; requires coordinate projection and track bounds setup
- WhatsApp/email share card — high viral potential but requires server-side image generation (satori)
- Discord bot integration — needs a Discord server; effective community mechanic but not urgent
- Global multi-venue leaderboards — requires external API integration with closed VMS ecosystem

### Architecture Approach

The architecture follows a strict venue-authoritative one-way push pattern: all competitive data (events, scoring, championships, driver ratings) is computed on the venue rc-core and synced to the cloud replica, which serves as a read-only public endpoint. The cloud never writes back to venue competitive tables — mixing bidirectional sync would allow stale cloud data to corrupt venue scores. Telemetry sync is targeted (event laps only, bounded at ~20 laps per event) to avoid the 18 million rows per day that unrestricted sync would generate. Championship standings are materialized (written after each round closes, not computed live) to keep public GET requests O(1) per driver regardless of event count.

**Major components:**
1. `hotlap_events.rs` (new Rust module) — event CRUD, auto-entry on lap completion via check_hotlap_event_entry(), F1 scoring, badge assignment, 107% rule enforcement
2. `championships.rs` (new Rust module) — championship CRUD, recalculate_standings() aggregating points across events
3. `driver_rating.rs` (new Rust module) — percentile-based class computation, nightly scheduled recalculation via tokio::time::interval
4. `cloud_sync.rs` extensions — 5 new table queries in collect_push_payload(); targeted telemetry via event lap_id JOIN
5. 8 new public API routes — /public/events, /public/championships, /public/drivers, /public/compare-laps, /public/laps/{id}/track-position, plus leaderboard extensions
6. 6 new PWA pages — /events, /events/[id], /championships, /championships/[id], /drivers/[id], /telemetry/compare
7. 6 new PWA components — EventCard, ChampionshipStandings, DriverRatingBadge, LapComparisonChart, TrackMapOverlay, SectorBadge
8. 4 existing files to modify — db/mod.rs (5 new tables + indexes), lap_tracker.rs (event entry hook + car_class), cloud_sync.rs (new payload), routes.rs (register new handlers)

### Critical Pitfalls

1. **telemetry_samples has no index** — add `CREATE INDEX idx_telemetry_lap_offset ON telemetry_samples(lap_id, offset_ms)` in Phase 1 migration before any data accumulates; full table scan makes visualization unusable within weeks at 3,000 rows per lap
2. **Driver ID mismatch corrupts lap sync** — add `cloud_driver_id` column to venue drivers table; never sync a lap whose driver_id is unresolved; this is documented in MEMORY.md and becomes acute when lap sync is added for competitive tables
3. **WAL checkpoint starvation** — add `PRAGMA wal_autocheckpoint=400` and sqlx pool `max_lifetime(Duration::from_secs(300))`; sqlx's persistent connections block checkpoint completion, causing read latency to grow over uptime
4. **No leaderboard indexes on laps table** — add composite covering index `(track, car, valid, lap_time_ms)` in Phase 1 migration; without it, every leaderboard query is a full table scan that grows week-over-week
5. **Cross-game comparability is impossible** — never mix AC and F1 25 laps on the same leaderboard; sim_type must be a required filter in every leaderboard API endpoint; driver ratings must be tracked per sim_type, not globally
6. **Championship scoring has silent edge cases** — add `result_status` column (DNS/DNF/finished/pending) to event_entries before any event is created; write characterization tests for ties, DNS, round cancellation before implementing scoring logic

---

## Implications for Roadmap

Based on the combined research, the dependency graph is clear: schema and indexes must come before any feature; leaderboard data exposure before events; events before championships; telemetry core before track map. The natural phase split is four phases.

### Phase 1: Data Foundation
**Rationale:** All competitive features are built on top of a correctly indexed, sync-safe schema. Building features on a broken foundation means retrofitting fixes through 5+ interconnected modules. This phase has no customer-visible UI deliverables but is the highest-risk phase if skipped or rushed — four confirmed production failures originate here.
**Delivers:** Production-safe database schema with all v3.0 tables, correct indexes, WAL tuning, and cloud driver ID resolution — the foundation every subsequent phase depends on.
**Addresses:** Prerequisite for all competitive features; makes leaderboard queries fast from day one; makes telemetry visualization feasible
**Avoids:** telemetry_samples full table scan (Pitfall 1), laps table leaderboard scan (Pitfall 3), WAL checkpoint starvation (Pitfall 6), cloud sync ID mismatch (Pitfall 7)
**Key deliverables:** idx_telemetry_lap_offset index, 3 laps covering indexes, WAL pragma tuning, cloud_driver_id column, 5 new competitive tables (hotlap_events, hotlap_event_entries, championships, championship_standings, driver_ratings), ALTER TABLE for car_class on laps, all new table indexes

### Phase 2: Leaderboard Core (Public Data Surface)
**Rationale:** Circuit records and driver profiles are independent of events — they query existing lap data with no new business logic. Shipping this first gives immediate customer-visible value from day one and validates the public PWA architecture before event complexity is added. The "beaten" notification reuses existing email infrastructure and belongs here because it hooks into track_records which already exists.
**Delivers:** Public leaderboard with car filter, circuit records, vehicle records, driver profile pages, automated "beaten" email notification — all populated from existing data with no events needed.
**Implements:** 4 new public API routes (/public/leaderboard extensions, /public/circuit-records, /public/vehicle-records/{car}, /public/drivers), leaderboard/public PWA page enhancements, driver profile page
**Avoids:** Cross-game comparability (Pitfall 5) — sim_type filter enforced on all endpoints; leaderboard cache staleness (Pitfall 11) — ISR revalidate:30 + on-demand revalidation; lap validity gaming (Pitfall 4) — suspect flag and sector sum consistency check
**Key deliverables:** sim_type required on all leaderboard endpoints, lap validity hardening (suspect column), ISR caching strategy with revalidateTag, driver profile page with PBs and stats cards, send_email.js notification hook on track_records update

### Phase 3: Hotlap Events and Championships
**Rationale:** Events depend on Phase 1 schema and Phase 2 leaderboard patterns. Building events after leaderboards means the event leaderboard page is a natural extension of the circuit records page. Championships depend on scored events, so they belong in the same phase but must come after event scoring is validated with at least one round.
**Delivers:** Staff event creation UI, public event leaderboard with 107% rule and gold/silver/bronze badges, group event results with F1 scoring, multi-round championship standings.
**Addresses:** All 9 P1 must-have features from FEATURES.md; the full engagement loop (event -> leaderboard -> beaten notification -> return visit)
**Avoids:** Championship scoring edge cases (Pitfall 9) — result_status column + characterization tests before first event; competitive data sync corruption — venue-authoritative one-way push only (never add competitive tables to SYNC_TABLES bidirectional list)
**Key deliverables:** hotlap_events.rs module (check_hotlap_event_entry, finalize_event_scoring), championships.rs module (recalculate_standings), staff event CRUD endpoints + score trigger, cloud_sync.rs extended with 5 new table payloads + targeted telemetry sync, /events and /championships PWA pages with EventCard and ChampionshipStandings components

### Phase 4: Telemetry Visualization and Driver Rating
**Rationale:** Telemetry comparison is self-contained — it shares only telemetry_samples infrastructure with the rest of the system. It can be built cleanly after the leaderboard establishes which laps are worth comparing. Driver rating requires sufficient lap data to produce meaningful percentiles and belongs after events generate structured competitive data. The 2D track map is deferred beyond this phase due to coordinate projection complexity.
**Delivers:** Speed trace + time delta lap comparison viewer, inputs trace (throttle/brake/steering), driver skill class (A/B/C/D percentile), class badge on driver profiles.
**Implements:** /public/compare-laps endpoint (server-side telemetry merge), LapComparisonChart component (recharts dual-trace), driver_rating.rs with nightly tokio recalculation, DriverRatingBadge component
**Avoids:** Driver rating algorithm failure (Pitfall 8) — percentile method confirmed correct, not Elo; telemetry endpoint size limit (security) — LIMIT 2000 per lap; track map coordinate issues (Pitfall 10) — track map deferred to v4 where pre-computation setup can be done properly
**Key deliverables:** compare-laps server-side merge endpoint, driver_rating.rs percentile computation with PERCENT_RANK() SQL, Uday sign-off on rating formula and class thresholds before implementation begins

### Phase Ordering Rationale

- Phase 1 must be first because all competitive features are broken without correct indexes and schema. telemetry visualization fails within weeks without the missing index. The ID mismatch corrupts the cloud leaderboard silently. No exceptions.
- Phase 2 before Phase 3 because circuit records ship immediately from existing data (no event dependency) and validate the public PWA architecture pattern before the more complex event logic is layered on top. The "beaten" notification also validates the email integration before events add their own notifications.
- Phase 3 before Phase 4 because telemetry comparison derives its value from "compare my lap to the event leader" — that context does not exist until events have run. Driver rating is also more meaningful after event data provides structured competitive reference points rather than only raw lap data.
- Championships are in Phase 3 (not Phase 4) because they are a direct extension of event scoring — the data model is identical, only the aggregation layer is new. Separating them would create a gap between events and standings that is confusing to customers.

### Research Flags

Phases needing deeper investigation before planning:
- **Phase 3 (Championship scoring rules):** Tiebreaker rules, DNS/DNF handling, round cancellation policy are product decisions, not engineering decisions. Need Uday sign-off before the scoring module is written. Write characterization tests first to make edge cases concrete.
- **Phase 4 (Driver rating thresholds):** The algorithm (percentile-based classes) is settled. The specific class boundaries and class_points accumulation rates are product decisions documented as needing sign-off. Flag this before Phase 4 planning begins.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Schema + Indexes):** SQLite index and pragma patterns are well-documented. Direct codebase inspection confirms exactly what is missing. All fixes are specific and verifiable. No unknowns.
- **Phase 2 (Public Leaderboard):** recharts, Next.js ISR, and public API patterns are all established in the existing codebase. The "beaten" email reuses existing send_email.js. No new patterns to discover.
- **Phase 3 (Events scoring logic):** F1 points array is a constant. Auto-entry logic is a conditional. 107% rule is arithmetic. The edge cases are the research finding — handle them with characterization tests, not further research.
- **Phase 4 (Telemetry comparison):** Server-side telemetry merge pattern is fully specified in ARCHITECTURE.md. recharts ComposedChart + syncId is already used in TelemetryChart.tsx. Canvas API track map is 50 lines of documented code.

---

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Verified directly against pwa/package.json, Cargo.toml, npm registry, and GitHub. Zero new Rust crates confirmed. Only new npm dep is date-fns 4.1.0. recharts ComposedChart + syncId is a documented pattern already used in TelemetryChart.tsx. |
| Features | HIGH | Competitor platforms analyzed directly (racecentres.com, Multitap, LapLegends, simracing.gp). Feature priority matrix is well-grounded. The "beaten" notification as #1 repeat-visit driver is confirmed by Multitap NXT LVL Gaming case study. |
| Architecture | HIGH | Based on direct codebase inspection of 1,791-line db/mod.rs, 276-endpoint routes.rs, cloud_sync.rs, and all PWA pages. Existing system inventory is exact, not estimated. All architectural claims are based on code that exists in the repository. |
| Pitfalls | HIGH | All critical pitfalls verified against actual code (telemetry_samples no index confirmed; laps no composite index confirmed; WAL checkpoint not tuned confirmed; ID mismatch documented in MEMORY.md). Cross-referenced with SQLite official docs, F1 25 UDP spec, and racing Elo research. |

**Overall confidence:** HIGH

### Gaps to Address

- **Driver rating thresholds (Phase 4):** Algorithm approach is settled (percentile-based classes), but specific class boundaries (A = top 10%? top 15%?) and class_points accumulation rates are product decisions. ARCHITECTURE.md documents this as needing Uday sign-off. Flag before Phase 4 planning begins.
- **Championship edge cases (Phase 3):** Tiebreaker sequence, DNS/DNF scoring, and round cancellation behavior are not defined. PITFALLS.md identifies all the cases. Write characterization tests (currently failing) before implementing scoring to make the decisions concrete.
- **107% rule with zero entries (Phase 3):** When a hotlap event has no entries yet, there is no reference time for the 107% check. Decision is to skip the check until at least one valid entry exists. Test this edge case explicitly.
- **Car class assignment for historical laps (Phase 3):** Laps recorded before the car_class column is added will have NULL car_class and will not auto-qualify for events. This is the correct behavior (confirmed in ARCHITECTURE.md), but staff need to understand historical laps do not appear in new events. Document in operational notes.
- **Public leaderboard under viral traffic (Phase 2):** At current venue scale the ISR revalidate:30 strategy is sufficient. If a share mechanic causes a traffic spike, add a 30-second in-memory cache on the public leaderboard endpoint proactively rather than reactively. ARCHITECTURE.md flags this as an open question.

---

## Sources

### Primary (HIGH confidence — direct codebase inspection)
- `crates/rc-core/src/db/mod.rs` — 1,791 lines; 40+ tables; confirmed missing indexes; WAL mode without checkpoint tuning
- `crates/rc-core/src/api/routes.rs` — 276 endpoints; /public/* pattern; existing stub routes confirmed
- `crates/rc-core/src/cloud_sync.rs` — push/pull logic; SYNC_TABLES constant; ID mismatch handling
- `crates/rc-core/src/lap_tracker.rs` — valid flag trusted directly from game UDP; track_records update not atomic
- `pwa/src/components/TelemetryChart.tsx` — recharts AreaChart + LineChart + syncId pattern (existing)
- `pwa/src/app/` — all existing PWA pages and scaffolds confirmed

### Primary (HIGH confidence — official documentation)
- SQLite WAL documentation (sqlite.org/wal.html) — checkpoint starvation confirmed with direct quote
- SQLite window functions (sqlite.org/windowfunctions.html) — PERCENT_RANK available from SQLite 3.25.0 (2018)
- F1 25 UDP Specification (EA Forums) — LapHistoryData bit flags; 20Hz recommendation; 60Hz causes packet loss
- Next.js ISR caching (nextjs.org) — on-demand revalidation with revalidateTag confirmed
- recharts 3.8.0 (GitHub releases) — React 19 peer dependency; ComposedChart + syncId documented
- SQLite query optimizer (sqlite.org/optoverview.html) — covering index behavior for GROUP BY + ORDER BY confirmed

### Secondary (MEDIUM confidence — web research)
- racecentres.com / r2r.racecentres.com — direct platform analysis; feature set confirmed: Hotlap Events, Circuit Records, Driver Data, Championships
- Multitap (multitap.space) + NXT LVL Gaming case study — "beaten" email as #1 repeat-visit driver confirmed
- simracing.gp SGP Ranking tutorial — Elo wrong for sim racing; percentile approach confirmed as correct for venue context
- LapLegends (laplegends.net/drivers) — driver profile feature set confirmed: laps, fastest laps, PBs, tracks, cars
- Sim Racing Telemetry (simracingtelemetry.com) — speed trace, lap comparison, TDiff, 2D track map patterns confirmed
- Elo generalization for racing (de Gruyter, Journal of Quantitative Analysis in Sports, 2024) — standard Elo inappropriate for time-only competitions confirmed
- SQLite performance tuning (phiresky.github.io) — WAL + busy_timeout + connection pool configuration confirmed effective
- Sim Racing Alliance championship rules (simracingalliance.com) — DNS/DNF edge case handling confirmed

---
*Research completed: 2026-03-14*
*Ready for roadmap: yes*
