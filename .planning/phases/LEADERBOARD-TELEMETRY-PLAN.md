# Milestone: Leaderboard & Telemetry — Full Experience
## UP Phase 1: PLAN (v3 — MMA Round 1+2 converged)

**Version:** v28.0
**Goal:** Transform the existing leaderboard data layer (Phase 88, shipped) and telemetry infrastructure (Phases 82-87, shipped) into a complete user-facing experience — live telemetry storage, historical lap replay, leaderboard display kiosks, driver ratings, and real-time record-break notifications.

**Status:** PLAN v3 CONVERGED — MMA Round 1 (53 findings, 19 P1) + Round 2 (50 findings, 17 P1) applied. 2 net-new P1s in Round 2 fixed. Ready for execution.

**MMA Round 1:** 53 findings (19 P1, 26 P2, 8 P3) — Qwen3 + DeepSeek V3 + DeepSeek R1 + Gemini 2.5 Pro
**MMA Round 2:** 50 findings (17 P1, 25 P2, 8 P3) — same 4 models. 2 net-new P1s: incremental_vacuum call + dual telemetry endpoints.

---

## Current State (Ground Truth)

### What EXISTS and WORKS:
1. **Multi-game lap storage** — laps table with sim_type, track normalization (Phase 88)
2. **Leaderboard API endpoints** — public, staff, bot with sim_type filtering (Phase 88)
3. **Leaderboard frontend** — Web dashboard, PWA public + authenticated pages
4. **Time trials** — Full CRUD + public endpoint + PWA display
5. **Telemetry collection** — rc-agent collects UDP telemetry from all 6 games (AC, F1 25, iRacing, LMU, ACEvo, Forza)
6. **Telemetry display** — Live telemetry pages in kiosk + web dashboard (speed, gear, RPM, DRS, ERS)
7. **Telemetry DB table** — `telemetry_samples` table with indexes exists in main DB
8. **Telemetry API** — `GET /customer/telemetry` (live) + `GET /public/laps/{id}/telemetry` (historical)
9. **Cloud sync** — track_records, personal_bests in SYNC_TABLES
10. **3 leaderboard display machines** — desktop-e3dn32l (100.122.215.124), desktop-q1bbl73 (100.98.92.17), desktop-q2mcek4 (100.99.109.79)
11. **SQLite WAL mode** — already enabled on production DB
12. **WhatsApp alerting** — already exists via Evolution API on Bono VPS (do NOT build new)

### What's BROKEN or MISSING:
1. **NO telemetry INSERT** — server receives telemetry via WS but never writes to `telemetry_samples`. Zero historical data.
2. **NO driver ratings computation** — `driver_ratings` table exists but no logic fills it.
3. **NO leaderboard display mode** — The 3 physical displays have no dedicated auto-refreshing kiosk page.
4. **NO record-break notifications** — When a track record is beaten, no real-time push to connected clients.
5. **NO telemetry replay visualization** — Historical endpoint exists but no frontend chart component.
6. **F1 25 Python telemetry** on Pod 7 — Standalone, not integrated. Port 20777 conflict with ConspitLink.

---

## Phase Breakdown

### Phase 251: Telemetry Sample Persistence
**Goal:** When rc-agent sends TelemetryFrame via WebSocket, server persists samples to a **separate telemetry.db** file, linked to the active lap.

**MMA Round 1 fixes applied:**
- [F-02 ALL] **Separate SQLite DB** — `telemetry.db` via `ATTACH DATABASE` or dedicated SqlitePool to isolate 80 writes/sec from main billing/leaderboard DB
- [F-01 ALL] **Agent-side lap_id stamping** — rc-agent stamps each TelemetryFrame with `lap_id` from its local lap tracker, NOT server-side mutable state (eliminates race condition)
- [F-18 qwen3] **Per-lap buffer with flush-on-transition** — buffer flushes on lap_id change, session end, OR 1-second timeout (not just batch count)
- [F-03 deepseek-r1] **Partial batch flush** — 1-second periodic flush prevents data loss on short/interrupted laps
- [F-02 deepseek-v3] **Pre-lap frame discard** — discard frames with no lap_id (before first lap starts)
- [F-15 qwen3] **Feature flag gated** — enable via runtime feature flag (FF), Pod 8 canary first per standing rule

**Changes:**
- `crates/rc-common/src/types.rs` — Add `lap_id: Option<String>` to TelemetryFrame
- `crates/rc-agent/src/event_loop.rs` — Stamp TelemetryFrame with current lap_id from local LapTracker state
- `crates/racecontrol/src/telemetry_store.rs` — **New module**: dedicated telemetry.db connection pool, batch writer with per-pod/per-lap buffers
- `crates/racecontrol/src/ws/mod.rs` — On `AgentMessage::Telemetry(frame)`, route to telemetry_store if feature flag `telemetry_persistence` is enabled
- **Sampling rate control** — Store 1 sample per 100ms (10Hz), discard intermediate frames via server-side timestamp comparison
- **Dedicated writer task** — Single `tokio::spawn` task consuming from `mpsc::channel`, batching across all pods, flushing every 1 second OR when buffer hits 50 samples
- **Disk budget** — ~600 bytes/sample × 10Hz × 60min = ~36MB/hour/pod. TOML config: `telemetry_retention_days = 30`
- **Nightly cleanup** — Batched DELETE (1000 rows per iteration with 100ms sleep between) to avoid long DB locks. Schedule at venue off-hours via server timer. Use `PRAGMA auto_vacuum = INCREMENTAL` on telemetry.db, then call `PRAGMA incremental_vacuum;` AFTER each batch of DELETEs to actually reclaim disk space (MMA R2 fix: incremental auto_vacuum only marks pages, explicit call needed to return space to filesystem).

**Requirements:**
| ID | Description |
|----|-------------|
| TEL-01 | Telemetry frames from rc-agent are persisted to telemetry.db with correct lap_id (stamped by agent) |
| TEL-02 | Sampling rate capped at 10Hz per pod to control disk usage |
| TEL-03 | Samples buffered per-pod/per-lap, flushed on: lap change, session end, 1-second timeout, OR 50 samples |
| TEL-04 | Retention policy: configurable days in TOML, batched nightly purge (1000 rows/iteration) |
| TEL-05 | GET /public/laps/{id}/telemetry returns stored samples for PUBLIC session laps only |
| TEL-05b | GET /api/v1/me/laps/{id}/telemetry (authenticated) returns driver's own private lap telemetry |
| TEL-06 | Separate telemetry.db file — does NOT use main racecontrol.db |
| TEL-07 | Feature flag gated: `telemetry_persistence` flag controls whether samples are stored |
| TEL-08 | Pre-lap frames (no lap_id) are discarded, not stored |
| TEL-09 | Pod 8 canary: enable feature flag on Pod 8 first, monitor 24h before fleet-wide |

**Success Criteria:** Enable flag on Pod 8 → drive a lap in AC → `SELECT COUNT(*) FROM telemetry_samples WHERE lap_id = '<that_lap>'` on telemetry.db returns 300-600 rows. Main racecontrol.db write latency unchanged.

---

### Phase 252: Historical Telemetry Visualization
**Goal:** Add telemetry replay charts to the leaderboard pages so customers can see speed/throttle/brake traces for any recorded lap.

**MMA Round 1 fixes applied:**
- [F-07 gemini-2.5] **Server-side downsampling** — `?resolution=` param with LTTB algorithm for mobile/long laps
- [F-05 deepseek-v3] **Rate limiting** — Per-IP rate limit on public telemetry endpoint (10 req/min)
- [F-10 qwen3] **Access control** — Only return telemetry for public sessions or requesting driver's own laps

**Changes:**
- `crates/racecontrol/src/api/routes.rs` — Split telemetry access:
  - `GET /public/laps/{id}/telemetry` — Only serves telemetry for laps in public sessions (time trials, events). Returns 404 for private laps.
  - `GET /api/v1/me/laps/{id}/telemetry` — Authenticated endpoint, returns telemetry only for requesting driver's own laps (JWT required)
  - Both endpoints: Add `?resolution=100ms|500ms|1s` query param. Use LTTB algorithm (not simple decimation — MMA R2 fix) for downsampling.
  - Rate limit: 10 requests/minute per IP on public endpoint via tower middleware
- `web/src/components/TelemetryChart.tsx` — New component using lightweight canvas charting (recharts or chart.js)
- Charts: Speed trace, Throttle+Brake overlay, Gear shifts — all vs time offset
- `web/src/app/leaderboards/page.tsx` — Click any lap time → expand to show telemetry chart
- `pwa/src/app/leaderboard/public/page.tsx` — Same expandable telemetry for public leaderboard

**Requirements:**
| ID | Description |
|----|-------------|
| VIZ-01 | Speed trace chart renders for any lap with telemetry data |
| VIZ-02 | Throttle/brake overlay chart (dual Y-axis or stacked) |
| VIZ-03 | Gear indicator along time axis |
| VIZ-04 | Charts responsive on mobile (PWA) and desktop (web) |
| VIZ-05 | Loading state + "No telemetry data" fallback for laps without samples |
| VIZ-06 | Server-side downsampling via ?resolution= param |
| VIZ-07 | Public telemetry rate-limited to 10 req/min per IP |

**Success Criteria:** Click a lap on /leaderboards → see smooth speed/throttle/brake charts load in <2s. Mobile gets downsampled data (<100KB payload).

---

### Phase 253: Driver Rating System
**Goal:** Compute driver skill ratings from lap data — consistency, speed relative to track records, improvement trend.

**MMA Round 1 fixes applied:**
- [F-05/F-06 ALL] **Edge case guards** — division by zero, insufficient laps, first-ever track
- [F-06 qwen3] **Async computation** — rating computed via tokio::spawn, never blocks persist_lap()
- [F-03 gemini-2.5] **Staff endpoint auth** — rating-history behind staff JWT
- [F-09 gemini-2.5] **Denormalized rating** — store composite_rating + rating_class in personal_bests for fast JOIN-free leaderboard queries
- [F-06 gemini-2.5] **Backfill migration** — one-time job to compute ratings for all existing drivers
- [F-16 qwen3 + F-09 deepseek-v3] **Cloud sync** — Add driver_ratings to SYNC_TABLES

**Changes:**
- `crates/racecontrol/src/driver_rating.rs` — New module with rating computation
- **Rating algorithm (with edge case guards):**
  - `pace_score` (0-100): `100.0 * (1.0 - (best_lap - track_record).max(0) / track_record)`, clamped 0-100. **If no track record exists: pace_score = 50 (neutral)**
  - `consistency_score` (0-100): `100.0 * (1.0 - std_dev / mean)`, clamped 0-100. **If <3 laps: consistency_score = 50 (neutral)**. **If mean == 0: consistency_score = 0**
  - `experience_score` (0-100): `100.0 * log10(total_laps + 1) / log10(1001)` (1001 laps = 100 score). **Never divides by zero (log10(1) = 0, log10(1001) > 0)**
  - `composite_rating` = 0.5 × pace + 0.3 × consistency + 0.2 × experience
  - `rating_class`: Unrated (< 3 laps), Rookie (0-30), Amateur (31-50), Club (51-70), Pro (71-85), Elite (86-100)
- **Trigger:** `tokio::spawn` after persist_lap() returns. Uses mpsc channel to rating worker task for sequential per-driver processing
- **Per sim_type ratings** — A driver can be Pro in AC but Rookie in F1 25
- **API:** `GET /api/v1/public/drivers/{id}/rating` (public), `GET /api/v1/drivers/{id}/rating-history` (staff JWT)
- **Leaderboard integration:** Denormalize `composite_rating` and `rating_class` into leaderboard response JSON (read from driver_ratings table via simple JOIN, cached)
- **Backfill:** On first startup with new code, detect empty driver_ratings table and queue backfill job for all drivers with 3+ laps
- **Cloud sync:** Add driver_ratings to SYNC_TABLES (venue-authoritative)

**Requirements:**
| ID | Description |
|----|-------------|
| RAT-01 | Driver rating computed from pace, consistency, experience after each lap |
| RAT-02 | Rating classes assigned (Unrated, Rookie through Elite) with edge case guards |
| RAT-03 | Ratings scoped per sim_type |
| RAT-04 | Public API returns current rating for any driver |
| RAT-05 | Staff-only rating-history endpoint behind JWT auth |
| RAT-06 | Leaderboard entries include driver rating badge (denormalized for perf) |
| RAT-07 | Async computation — never blocks persist_lap() |
| RAT-08 | Backfill migration for existing drivers on first deploy |
| RAT-09 | driver_ratings in SYNC_TABLES for cloud sync |

**Success Criteria:** After 5+ laps, driver profile shows a rating class. Leaderboard entries display rating badges. Backfill completes within 60s for existing data.

---

### Phase 254: Real-Time Leaderboard Updates
**Goal:** When a track record or personal best is broken, push update to all connected WebSocket clients instantly.

**MMA Round 1 fixes applied:**
- [F-13 qwen3] **Debounced broadcast** — max 1 leaderboard update per second per track via tokio throttle
- [F-19 qwen3] **Transaction-based record detection** — SELECT + conditional INSERT in transaction, not UPSERT result
- [F-07 qwen3] **Polling fallback** — leaderboard responses include `last_updated` timestamp for short-polling clients
- [F-08 gemini-2.5] **WhatsApp removed** — out of scope, existing WhatsApp alerting system can be wired later separately

**Changes:**
- `crates/racecontrol/src/lap_tracker.rs` — After track_records UPSERT, check if new best (transaction-based: SELECT old → compare → INSERT/UPDATE). If new record, send `LeaderboardUpdate` to dashboard event channel
- `crates/racecontrol/src/ws/mod.rs` — Add `DashboardEvent::LeaderboardUpdate { track, car, sim_type, driver_name, lap_time_ms, record_type }` variant
- **Debounce:** Server-side throttle: max 1 broadcast per second per (track, sim_type) combo. Buffer latest update, send on tick.
- **Frontend:** Web + PWA leaderboard pages subscribe to WS and auto-update affected entries. Polling fallback every 30s if WS disconnected.
- **Record-break animation:** Flash/highlight the new record entry for 5 seconds with CSS animation
- **last_updated field:** All leaderboard API responses include `last_updated: ISO8601` so polling clients can detect changes

**Requirements:**
| ID | Description |
|----|-------------|
| RT-01 | Track record breaks broadcast to all connected dashboard clients via WS |
| RT-02 | Broadcasts debounced to max 1/sec per (track, sim_type) |
| RT-03 | Frontend auto-updates leaderboard table without full page refresh |
| RT-04 | Record-break visual highlight animation on leaderboard |
| RT-05 | Polling fallback: last_updated timestamp in all leaderboard API responses |
| RT-06 | Record detection uses transaction (SELECT old → compare), not UPSERT side-effect |

**Success Criteria:** Set a new track record → leaderboard page updates within 2 seconds without manual refresh. No broadcast storms during busy sessions.

---

### Phase 255: Leaderboard Display Kiosk
**Goal:** Dedicated auto-rotating leaderboard display for the 3 physical display machines.

**MMA Round 1 fixes applied:**
- [F-08 qwen3] **Offline fallback** — Cache last-known-good state, show "Last updated X min ago" when WS disconnected >30s
- [F-10 deepseek-r1] **Panel load timeout** — 5s timeout per panel, skip to next on failure
- [F-06 deepseek-v3] **Display heartbeat** — POST /api/v1/kiosk/ping every 30s from display, admin dashboard shows display status
- [F-12 qwen3] **Input lockdown** — Disable keyboard/mouse input via Windows Group Policy or AutoHotkey script on display machines

**Changes:**
- `web/src/app/leaderboard-display/page.tsx` — New full-screen page designed for wall-mounted displays
- **Auto-rotation:** Cycle through: All-time records → Top drivers → Current time trial → Per-track records (rotate every 15 seconds)
- **Offline resilience:** Cache leaderboard data in sessionStorage. If WS disconnects for >30s, show cached data with "Last updated: X ago" overlay
- **Panel timeout:** Each panel has 5s load timeout. On timeout, skip to next panel (log warning)
- **Visual design:** Large text, high contrast, Racing Point brand (red #E10600 on black #1A1A1A), Montserrat + Enthocentric fonts
- **Real-time:** Subscribe to WS for LeaderboardUpdate events, show record-break animation
- **Kiosk mode:** No scrollbars, no header/footer, auto-fullscreen via URL param `?kiosk=true`
- **Configuration:** URL params: `?sim_type=f125&rotation_speed=15&kiosk=true`
- **Heartbeat:** `POST /api/v1/kiosk/ping` every 30s with `{display_id, uptime_s}` — server stores in `kiosk_heartbeats` table, admin shows display status
- **Deployment:** Edge `--app` mode on the 3 desktop-* machines with bat file for auto-start

**Requirements:**
| ID | Description |
|----|-------------|
| DSP-01 | Full-screen leaderboard display page with auto-rotation |
| DSP-02 | 4 rotation panels: all-time records, top drivers, time trial, per-track |
| DSP-03 | Real-time update on record breaks (WS subscription) |
| DSP-04 | Configurable via URL params (sim_type, rotation speed, kiosk mode) |
| DSP-05 | Racing Point brand styling (colors, fonts) |
| DSP-06 | Offline fallback: cached data + staleness indicator when WS disconnected |
| DSP-07 | Display heartbeat: 30s ping to server, admin dashboard shows status |
| DSP-08 | Panel load timeout (5s) with skip-to-next on failure |
| DSP-09 | Deploy bat files for 3 display machines (Edge --app + auto-start) |

**Success Criteria:** Open `/leaderboard-display?kiosk=true` → auto-cycles through panels, updates live on record break. Display loses network → shows cached data with staleness warning.

---

## Dependency Graph

```
Phase 251 (Telemetry Persistence)  ──→  Phase 252 (Telemetry Viz)
     │                                        │
     ├── (parallel) ──→ Phase 253 (Ratings)   │
     │                                        │
     └── (parallel) ──→ Phase 254 (RT Updates) ──→ Phase 255 (Display Kiosk)
```

**Execution order:** 251 + 253 (parallel, Wave 1) → 252 + 254 (parallel, Wave 2) → 255 (Wave 3)

---

## Cross-System Impact (UP 1.3)

| System | Impact | Action |
|--------|--------|--------|
| racecontrol (server) | New telemetry_store module, rating module, WS events, endpoints | Rebuild + server deploy |
| rc-agent (pods) | Add lap_id to TelemetryFrame | Rebuild + fleet deploy (Pod 8 canary first) |
| rc-common (lib) | TelemetryFrame struct change (new field) | Lib rebuild (dependency of both) |
| Web dashboard | New TelemetryChart, LeaderboardDisplay components | Rebuild + deploy |
| PWA | Telemetry chart on leaderboard | Rebuild + deploy (VPS) |
| Cloud (Bono VPS) | driver_ratings in SYNC_TABLES. telemetry.db NOT synced (too large) | Cloud binary rebuild |
| 3 Display machines | Edge --app bat files | New bat files + Group Policy |

---

## Risk Assessment (UP 1.6) — Updated with MMA findings

| Risk | Severity | Mitigation |
|------|----------|------------|
| ~~SQLite 80 writes/sec bottleneck~~ | ~~CRITICAL~~ MITIGATED | Separate telemetry.db isolates write load from main DB |
| ~~Race condition in lap_id assignment~~ | ~~CRITICAL~~ MITIGATED | Agent stamps frames with lap_id, server does not guess |
| ~~Rating formula division by zero~~ | ~~HIGH~~ MITIGATED | Edge case guards: neutral scores for missing data |
| ~~Partial batch data loss~~ | ~~HIGH~~ MITIGATED | 1-second timeout flush + flush on lap/session transition |
| Telemetry disk usage explosion | MEDIUM | 10Hz cap + 30-day retention + TOML config + incremental vacuum |
| Rating computation slowing persist_lap() | LOW | Async worker via mpsc channel, never blocks |
| WS broadcast storm on busy night | LOW | 1/sec debounce per (track, sim_type) |
| Display machines offline | LOW | Offline fallback with cached data + staleness indicator |
| F1 25 port 20777 conflict | DEFERRED | rc-agent already handles this — ConspitLink and telemetry listener coexist on separate bindings |

---

## MMA Round 1 Disposition

### P1s (19) — All addressed in plan v2
| ID | Source | Status | Fix Location |
|----|--------|--------|--------------|
| F-01 qwen3 | Lap linkage race | FIXED | TEL-01: agent-side lap_id |
| F-02 qwen3 | SQLite bottleneck | FIXED | TEL-06: separate telemetry.db |
| F-03 qwen3 | PII in telemetry | ACCEPTED | Telemetry only stores numeric values (speed, throttle, brake, gear, rpm, steering) — no PII fields |
| F-04 qwen3 | Clock drift | MITIGATED | Agent stamps local monotonic offset; server uses frame's offset directly |
| F-13 qwen3 | Broadcast storm | FIXED | RT-02: 1/sec debounce |
| F-18 qwen3 | Cross-lap batch | FIXED | TEL-03: per-lap buffer with transition flush |
| F-01 dv3 | Write bottleneck | FIXED | TEL-06: separate telemetry.db |
| F-02 dv3 | Pre-lap frames | FIXED | TEL-08: discard frames without lap_id |
| F-05 dv3 | Public API scraping | FIXED | VIZ-07: rate limiting |
| F-09 dv3 | Ratings not synced | FIXED | RAT-09: SYNC_TABLES |
| F-01 dr1 | Session state missing | FIXED | TEL-01: agent stamps lap_id, no server state needed |
| F-02 dr1 | SQLite bottleneck | FIXED | TEL-06: separate telemetry.db |
| F-03 dr1 | Partial batch loss | FIXED | TEL-03: 1-second flush timeout |
| F-06 dr1 | Rating div by zero | FIXED | RAT-02: edge case guards |
| F-09 dr1 | Broadcast storm | FIXED | RT-02: debounce |
| F-11 dr1 | F1 25 port conflict | NOTED | Already handled in rc-agent — deferred, not a plan bug |
| F-01 gem | Lap linkage race | FIXED | TEL-01: agent-side lap_id |
| F-02 gem | SQLite total load | FIXED | TEL-06: separate telemetry.db |
| F-03 gem | Staff endpoint auth | FIXED | RAT-05: staff JWT |

### P2s (26) — Key ones addressed, others tracked
- Edge case guards in rating: ADDRESSED (RAT-02)
- Async rating computation: ADDRESSED (RAT-07)
- Offline fallback for displays: ADDRESSED (DSP-06)
- Display heartbeat monitoring: ADDRESSED (DSP-07)
- Telemetry downsampling: ADDRESSED (VIZ-06)
- Backfill migration: ADDRESSED (RAT-08)
- Nightly cleanup batching: ADDRESSED (TEL-04)
- Pod 8 canary deployment: ADDRESSED (TEL-09)
- F1 25 Python integration: DEFERRED (out of scope — existing rc-agent F1 25 adapter handles telemetry)
- GDPR telemetry compliance: NOTED (telemetry is numeric only — speed/throttle/brake/gear, no PII)
- Lap comparison mode: DEFERRED to v28.1
- Telemetry data export: DEFERRED to v28.1
