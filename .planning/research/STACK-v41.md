# Stack Research — v41.0 Game Intelligence System

**Domain:** Per-pod game inventory + proactive combo validation + launch timeline tracing + reliability dashboard
**Researched:** 2026-04-03 IST
**Confidence:** HIGH — all recommendations are extensions of the already-deployed stack; no new technology categories introduced

---

## Context: What Already Exists (Do Not Re-research)

The following are confirmed deployed and working. This research covers ONLY the delta needed for v41.0.

| Existing Capability | Crate / File | Status |
|--------------------|-------------|--------|
| AC content scanning | `rc-agent/src/content_scanner.rs` | Deployed, AC-only |
| combo_reliability SQLite table | `racecontrol/src/preset_library.rs` | Deployed |
| GamePresetWithReliability struct | `rc-common/src/types.rs:1017` | Deployed |
| Game Doctor 12-point check | `rc-agent/src/game_doctor.rs` | Deployed, reactive only |
| Steam process checks | `rc-agent/src/steam_checks.rs` | Deployed |
| Tier engine (GameLaunchFail trigger) | `rc-agent/src/tier_engine.rs` | Deployed |
| launch_events SQLite table | `racecontrol/src/api/metrics.rs` | Deployed |
| SimType enum (8 games) | `rc-common/src/types.rs:8` | Deployed |
| installed_games field on PodInfo | `rc-common/src/types.rs:104` | Field exists, not populated |
| Next.js kiosk (Next 16.1.6, React 19) | `kiosk/package.json` | Deployed |
| Admin dashboard (recharts 3.7, swr 2.4) | `racingpoint-admin/package.json` | Deployed |

---

## Recommended Stack — New Additions Only

### Rust/rc-agent: Content Scanner Extension

**No new crates needed.** The existing dependency set (`std::fs`, `sysinfo 0.33`, `rusqlite 0.32`) covers everything required for expanding content_scanner.rs to non-AC games.

| Capability | Mechanism | Why No New Crate |
|------------|-----------|-----------------|
| Steam library scan | Read `libraryfolders.vdf` via `std::fs::read_to_string` | VDF is INI-like; stdlib string parsing is sufficient. A full VDF parser crate is overkill for 3 fields we need. |
| Non-Steam game detection | Walk well-known install paths (configurable per game in TOML) | Already done for AC at `C:\Program Files (x86)\Steam\steamapps\common\assettocorsa`. Same pattern for other games. |
| Per-game install verification | Check for the game exe via `std::fs::metadata` | `sysinfo` already used for process checks; `metadata` is cheaper for existence checks. |
| Crash loop detection | Count consecutive launch failures in a rolling window; rusqlite already on pod | `rusqlite 0.32` bundled — already compiled in. Store `(pod_id, sim_type, window_start, fail_count)`. |

**Steam VDF parsing approach:** `libraryfolders.vdf` is at `C:\Program Files (x86)\Steam\steamapps\libraryfolders.vdf`. Parse with `std::str::lines()` + `contains("path")` pattern. HIGH confidence — this path is stable across Steam versions (confirmed in steam_checks.rs which already references the Steam default path).

### Rust/rc-agent: Launch Timeline Tracing

**No new crates needed.** Use `std::time::Instant` + `rusqlite` (already bundled).

**Pattern:**
```rust
// In ac_launcher.rs / game process launch path
struct LaunchSpan { label: &'static str, elapsed_ms: u64 }
let mut spans: Vec<LaunchSpan> = Vec::new();
let t0 = std::time::Instant::now();
// ... each checkpoint records (label, t0.elapsed().as_millis())
// On completion or failure: INSERT INTO launch_timeline_events
```

The `launch_events` table already stores `duration_to_playable_ms`. The timeline spans go into a new `launch_timeline_spans` table (one row per checkpoint per launch attempt). This reuses `rusqlite` on pod and `sqlx` on server — no new deps.

### Rust/rc-agent: Timeout Watchdog

**No new crates needed.** Use `tokio::time::timeout` (already in `tokio` workspace dep).

```rust
// 90s default, dynamic per-combo from server-pushed config (already has AgentConfig push)
tokio::time::timeout(
    Duration::from_secs(config.game_launch_timeout_secs.unwrap_or(90)),
    launch_game_inner(params)
)
```

The `AgentConfig` struct (rc-common/src/config_schema.rs) already has server-push capability via `FullConfigPushPayload`. Add `game_launch_timeout_secs: Option<u64>` field there — no new dep.

### Rust/racecontrol: Fleet Game Matrix API

**No new crates.** The server already uses `sqlx 0.8` (SQLite) and `axum 0.8`. New endpoint aggregates `pod_game_inventory` table (new, see DB section) with `combo_reliability`.

### Frontend: Reliability Dashboard

**No new npm packages needed.** The admin dashboard already has:
- `recharts 3.7.0` — sufficient for success rate bar charts and fleet heatmap
- `swr 2.4.1` — handles polling/revalidation for live dashboard data
- `lucide-react` — icons for flagged/healthy combo indicators
- Tailwind CSS 4 — styling

**One candidate addition (optional):** `@tanstack/react-table 8.x` for the fleet game matrix table (pod rows × game columns). The admin already uses `react-hook-form` and `recharts` but has no data-grid library. The matrix could be rendered with plain HTML table + Tailwind instead if complexity is low. Recommend **deferring** until the matrix requirements are clearer — a 8-pod × 8-game matrix is 64 cells, which is straightforward with a plain table.

| Library | Version | Purpose | Decision |
|---------|---------|---------|----------|
| `@tanstack/react-table` | 8.21.x | Fleet game matrix sortable/filterable grid | DEFER — plain table first, add if >3 sort/filter axes needed |
| `recharts` | already 3.7.0 | Success rate charts, launch timeline bar | USE EXISTING |
| `swr` | already 2.4.1 | Data fetching with auto-revalidation | USE EXISTING |

### Frontend: Kiosk Game Filtering

**No new npm packages.** The kiosk needs to:
1. Receive `installed_games` from the server (field exists on `PodInfo`)
2. Filter the game selection UI to only show installed games

This is a pure API change + React state filter — no new library needed. The kiosk already fetches pod state via WebSocket (`ws_handler.rs` → kiosk WS client).

---

## DB Schema Additions

These are new SQLite tables on the server (sqlx migrations). No new DB technology.

### `pod_game_inventory` (new, server-side)

```sql
CREATE TABLE IF NOT EXISTS pod_game_inventory (
    pod_id        TEXT NOT NULL,
    sim_type      TEXT NOT NULL,  -- SimType string
    is_installed  INTEGER NOT NULL DEFAULT 1,
    install_path  TEXT,           -- detected exe path for verification
    last_scanned  TEXT NOT NULL,  -- ISO-8601 UTC
    PRIMARY KEY (pod_id, sim_type)
);
```

**Why:** The `installed_games` field on `PodInfo` is in-memory only (not persisted). This table lets the server answer "which pods have Forza?" without every pod being online.

### `launch_timeline_spans` (new, pod-side rusqlite + server-side sqlx)

```sql
-- Pod-side (rusqlite, local):
CREATE TABLE IF NOT EXISTS launch_timeline_spans (
    launch_id    TEXT NOT NULL,
    pod_id       TEXT NOT NULL,
    sim_type     TEXT NOT NULL,
    label        TEXT NOT NULL,  -- "process_start", "first_frame", "telemetry_live", etc.
    elapsed_ms   INTEGER NOT NULL,
    recorded_at  TEXT NOT NULL,
    PRIMARY KEY (launch_id, label)
);
```

Server aggregates spans from agent via WS message (new `LaunchTimelineReport` protocol message). Server stores in same structure via sqlx.

### `combo_validation_flags` (new, server-side)

```sql
CREATE TABLE IF NOT EXISTS combo_validation_flags (
    id             TEXT PRIMARY KEY,
    preset_id      TEXT NOT NULL REFERENCES game_presets(id),
    pod_id         TEXT,          -- NULL = fleet-wide flag
    flag_reason    TEXT NOT NULL, -- e.g. "car_folder_missing", "ai_lines_missing"
    flagged_at     TEXT NOT NULL,
    resolved_at    TEXT,          -- NULL = still active
    auto_disabled  INTEGER NOT NULL DEFAULT 0
);
```

---

## Integration Points

### rc-agent changes (content_scanner.rs extension)

| Change | Where | Impact |
|--------|-------|--------|
| Add `scan_steam_library()` | `content_scanner.rs` | New function, no existing code touched |
| Add `scan_non_steam_games()` | `content_scanner.rs` | New function, reads TOML config paths |
| Call both at boot + WS reconnect | `ws_handler.rs` or `event_loop.rs` (current `scan_ac_content` call site) | 1-line addition |
| Populate `installed_games` on `PodInfo` | `app_state.rs` | Extend existing heartbeat assembly |
| Add `GameLaunchTimeout` trigger | `tier_engine.rs` | Extend existing `GameLaunchFail` arm |
| Add `CrashLoop` trigger | `tier_engine.rs` | New arm, detection in `failure_monitor.rs` |

### racecontrol changes (server)

| Change | Where | Impact |
|--------|-------|--------|
| Store `installed_games` from heartbeat | `ws_handler.rs` (heartbeat handler) | 5-line DB write |
| New endpoint: `GET /api/v1/fleet/game-matrix` | `api/routes.rs` + new handler | Additive |
| New endpoint: `GET /api/v1/presets/{id}/validation` | `api/routes.rs` + new handler | Additive |
| New endpoint: `GET /api/v1/launch-timeline/{launch_id}` | `api/metrics.rs` extension | Additive |
| Extend `combo_reliability` queries for per-pod breakdown | `preset_library.rs` | Extend existing query |
| New migration: `pod_game_inventory`, `launch_timeline_spans`, `combo_validation_flags` | `db/mod.rs` | Idempotent `CREATE TABLE IF NOT EXISTS` |

### Kiosk changes

| Change | Where | Impact |
|--------|-------|--------|
| Filter game list by `installed_games` from pod heartbeat | Game selection component | Additive filter, no API contract change |
| Show combo validity indicator | Preset selection screen | Read `flagged_unreliable` (already in `GamePresetWithReliability`) |

### Admin dashboard changes

| Change | Where | Impact |
|--------|-------|--------|
| Fleet game matrix page (`/reliability`) | New Next.js page | New route, uses existing recharts + swr |
| Launch timeline viewer | Extend `/metrics` page or new page | New query to `launch_timeline_spans` endpoint |
| Flagged combo UI | Extend `/presets` page | Add `flagged_unreliable` badge (already in API response) |

---

## What NOT to Add

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| A VDF parser crate (e.g. `keyvalues-parser`) | libraryfolders.vdf parsing needs 3 fields from a well-known format; a full parser adds compile overhead and a new dependency for trivial benefit | `std::str::lines()` + manual parsing of `"path"` key |
| A time-series DB (InfluxDB, TimescaleDB) | Already have SQLite TSDB from v34.0; launch timeline data is low-frequency (one row per launch span, ~10 spans/launch) | Existing SQLite + launch_timeline_spans table |
| A separate inventory service | Current fleet size is 8 pods; a microservice adds latency and operational complexity for no benefit | Extend racecontrol's existing WS handler |
| `@tanstack/react-table` immediately | 8×8 matrix is 64 cells; recharts + plain HTML table handles it | Add only if filtering/sorting needs exceed 2 dimensions |
| A dedicated job queue (Redis, BullMQ) | Proactive combo validation runs at boot per pod — tokio::spawn is sufficient | tokio::task::spawn_blocking for filesystem validation |
| Per-combo AI diagnosis at boot | Boot-time validation is deterministic (does the folder exist?); AI tier is for runtime failures | Game Doctor filesystem checks (already Tier 1 deterministic) |

---

## Version Compatibility

All additions are within the existing workspace. No version constraints introduced.

| Package | Current Version | New Usage | Compatibility |
|---------|----------------|-----------|---------------|
| `rusqlite` | 0.32 bundled | launch_timeline_spans (pod-side) | No change |
| `sqlx` | 0.8 | pod_game_inventory, combo_validation_flags | No change |
| `tokio` | workspace (1.x) | `tokio::time::timeout` for watchdog | No change |
| `serde_json` | workspace | LaunchTimelineReport WS message | No change |
| `recharts` | 3.7.0 | Reliability charts | No change |
| `swr` | 2.4.1 | Dashboard data polling | No change |

---

## Installation

No new packages to install. All capabilities are achieved by extending existing dependencies.

```bash
# Verify no new deps crept in after implementing:
cargo tree -p rc-agent-crate | diff - <(cat expected-rc-agent-tree.txt)
cargo tree -p racecontrol-crate | diff - <(cat expected-racecontrol-tree.txt)

# Frontend — no changes expected:
cd kiosk && npm ls --depth=0
cd ../racingpoint-admin && npm ls --depth=0
```

---

## Sources

- Codebase inspection: `crates/rc-agent/src/content_scanner.rs` — confirms AC-only scope, std::fs pattern
- Codebase inspection: `crates/rc-agent/src/steam_checks.rs` — confirms sysinfo usage, Steam path pattern
- Codebase inspection: `crates/rc-agent/src/game_doctor.rs` — confirms deterministic boot-time check pattern
- Codebase inspection: `crates/racecontrol/src/preset_library.rs` — confirms combo_reliability table structure
- Codebase inspection: `crates/racecontrol/src/api/metrics.rs` — confirms launch_events table, LaunchStatsResponse
- Codebase inspection: `crates/rc-common/src/types.rs:897-1027` — confirms ContentManifest, GamePresetWithReliability
- Codebase inspection: `kiosk/package.json`, `racingpoint-admin/package.json` — confirmed current frontend deps
- Codebase inspection: `crates/rc-agent/Cargo.toml`, `crates/racecontrol/Cargo.toml` — confirmed Rust deps

---
*Stack research for: v41.0 Game Intelligence System*
*Researched: 2026-04-03 IST*
