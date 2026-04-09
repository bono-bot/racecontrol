# Phase 367: Staff Tools — Research

> **Researched:** 2026-04-09 (--auto mode)

---

## Phase Summary

5 plans needed (367-01 through 367-05). Plans 01-04 are cross-repo: backend API routes in racecontrol + new Next.js pages in racingpoint-admin. Plan 05 is racecontrol-only (GLD-G-05 retro-validation tests).

**Critical prerequisite:** Phase 363 must be DEPLOYED to server .23 before Plans 01-04's backend routes are useful. The `billing_sessions.suspect` column only exists on server after Phase 363's migration runs.

---

## Validation Architecture

The following validation dimensions apply to this phase:

- **D1 (Unit):** Rust handler functions — suspect query correctness, verify endpoint timeout guard
- **D2 (Integration):** End-to-end API route test — manager JWT → route → DB query → JSON response
- **D3 (Frontend):** Visual/functional — admin pages render with correct data, heatmap colors correct
- **D4 (E2E):** GLD-G-05 — deliberate-mismatch → WhatsApp received on staff phone
- **D5 (Load):** GLD-G-05 — 8 concurrent mismatches → 0 events dropped in config_mismatches

---

## Plan-by-Plan Research Findings

### Plan 367-01: Suspect Lap View + Telemetry Heatmap (GLD-G-01)

#### Backend API Routes (racecontrol)

**Route 1:** `GET /admin/suspect-sessions`
- Add to manager-role sub-router (lines 626-653 of `crates/racecontrol/src/api/routes.rs`)
- Query: `SELECT bs.id, bs.driver_id, d.name AS driver_name, bs.pod_id, bs.suspect_reasons, bs.telemetry_coverage_pct, bs.lap_count_actual, bs.lap_count_expected, bs.lap_count_flag, bs.started_at, bs.ended_at FROM billing_sessions bs LEFT JOIN drivers d ON d.id = bs.driver_id WHERE bs.suspect = 1 ORDER BY bs.ended_at DESC LIMIT ? OFFSET ?`
- Pagination via `?page=0&limit=50` query params
- Optional `?from=YYYY-MM-DD&to=YYYY-MM-DD` date filter on `bs.ended_at`
- Returns JSON: `{"sessions": [...], "total": N, "page": N}`
- Pattern: copy `list_disputes_handler` at line 21117 — same auth layer, same sqlx pattern

**Route 2:** `GET /admin/sessions/{id}/telemetry-heatmap`
- Add to manager-role sub-router
- Look up laps for this billing_session_id: `SELECT l.id, l.lap_number, l.lap_time_ms, l.valid, l.suspect FROM laps l WHERE l.billing_session_id = ? ORDER BY l.lap_number ASC`
- For each lap, fetch telemetry sample coverage from `telemetry_samples` (state.telemetry_db): `SELECT COUNT(*) FROM telemetry_samples WHERE lap_id = ?`
- Returns JSON: `{"session_id": "...", "laps": [{"lap_number": N, "lap_id": "...", "lap_time_ms": N, "sample_count": N, "suspect": bool}]}`
- NOTE: `state.telemetry_db` is a separate pool (`.unwrap_or(&state.db)` pattern from `public_lap_telemetry` handler at line 15086)

#### Admin Portal Page (racingpoint-admin)

**File:** `racingpoint-admin/src/app/(dashboard)/sessions/suspect/page.tsx`

List view:
- `useEffect` fetches `/api/rc/admin/suspect-sessions?page=0&limit=50` on mount
- Table with columns: Driver, Pod, Coverage%, Laps (actual/expected), Flag, Ended At, Actions
- Row click expands to drill-down panel (or navigates to detail route)
- Suspect reasons shown as pill badges (parse JSON array from `suspect_reasons`)

Drill-down heatmap (same page, conditional panel below selected row):
- Fetches `/api/rc/admin/sessions/{id}/telemetry-heatmap` on row select
- `Cell`-based grid from recharts — lap number on X, single row Y, color by sample_count
- Color: `sample_count === 0 ? '#5A5A5A' : sample_count < 50 ? '#E10600' : '#22c55e'`
- Import: `import { ScatterChart, ResponsiveContainer, ... Cell } from 'recharts'` — already in package.json (recharts ^3.7.0)

Pattern reference: `racingpoint-admin/src/app/(dashboard)/analytics/page.tsx` for recharts usage with `Cell` color override.

---

### Plan 367-02: On-Demand Pod Verify (GLD-G-02)

#### Backend API Route (racecontrol)

**Route:** `POST /admin/pods/{pod_id}/verify`
- Add to manager-role sub-router
- Handler: `async fn admin_verify_pod(State(state), Path(pod_id)) -> Json<Value>`
- Timeout: `tokio::time::timeout(Duration::from_secs(15), verify_logic).await`
- Synthetic test logic:
  1. Construct a fake `AgentMessage::ConfigMismatchDetected` with a deliberate test mismatch (e.g. `car: "test_expected" vs "test_actual"`)
  2. Process through the same code path in `ws/mod.rs` lines 2229-2269 BUT route to a test-mode path that does NOT send WhatsApp and does NOT persist to event_archive
  3. Verify the `DashboardEvent::ConfigMismatch` is emitted (send to a oneshot channel instead of dashboard_tx)
  4. Return `{"pass": true, "detail": "synthetic mismatch detected and processed", "elapsed_ms": N}`
- OR simpler approach: verify the pod is WS-connected and the sim adapter is running by querying `state.agent_senders` (current WS connection registry) + querying `config_mismatches` table for recent entries from this pod
- Simpler approach avoids re-implementing the mismatch path for testing and is safer for production

**Recommended approach:** Simple health+connectivity verify:
```rust
async fn admin_verify_pod(State(state), Path(pod_id)) -> Json<Value> {
    let start = tokio::time::Instant::now();
    let timeout = tokio::time::timeout(Duration::from_secs(15), async {
        // Check 1: Pod WS connected
        let ws_connected = state.agent_senders.read().await.contains_key(&pod_id);
        // Check 2: Last config_mismatch check (any recent check = verifier ran at least once)
        let last_check: Option<String> = sqlx::query_scalar(
            "SELECT MAX(detected_at) FROM config_mismatches WHERE pod_id = ?"
        ).bind(&pod_id).fetch_optional(&state.db).await.ok().flatten();
        (ws_connected, last_check)
    }).await;
    let elapsed = start.elapsed().as_millis();
    match timeout {
        Ok((ws_conn, last_mismatch_check)) => Json(json!({
            "pass": ws_conn,
            "detail": if ws_conn { "pod connected, verifier active" } else { "pod WS disconnected" },
            "last_mismatch_check": last_mismatch_check,
            "elapsed_ms": elapsed
        })),
        Err(_) => Json(json!({"pass": false, "detail": "timeout", "elapsed_ms": 15000}))
    }
}
```

#### Admin Portal Page

**File:** `racingpoint-admin/src/app/(dashboard)/fleet/verify/page.tsx`
- Grid of 8 pod cards (1-8), each showing: status indicator, last verify result + timestamp, "Verify" button
- `verifying` state per pod (disables button during in-flight request)
- "Verify All" button iterates pods sequentially (not concurrent — avoids server thundering)
- POST `/api/rc/admin/pods/${podId}/verify` — 15s timeout on frontend too
- Color: pass=green border (#22c55e), fail=red border (#E10600), unknown=grey (#5A5A5A)

---

### Plan 367-03: Session Replay Player (GLD-G-03)

#### Backend API Route (racecontrol)

**Route:** `GET /admin/sessions/{id}/replay`
- Add to manager-role sub-router
- Fetch all laps for this billing_session_id (ordered by lap_number)
- For each lap, fetch telemetry_samples ordered by offset_ms
- Flatten into ordered event array: `[{type: "lap_start", lap: N, offset_ms: 0}, {type: "telemetry", lap: N, offset_ms: N, speed: F, throttle: F, brake: F, steering: F, gear: N, rpm: N}, ..., {type: "lap_end", lap: N, lap_time_ms: N}]`
- Return as JSON array (single response, not streaming — typical session is <5MB)
- Add `Cache-Control: no-store` header (admin data, not cacheable)

Fetch pattern (two pools — match `public_lap_telemetry` at line 15086):
```rust
let telem_pool = state.telemetry_db.as_ref().unwrap_or(&state.db);
let laps = sqlx::query_as(..., "SELECT id, lap_number, lap_time_ms FROM laps WHERE billing_session_id = ? ORDER BY lap_number ASC").bind(session_id).fetch_all(&state.db).await;
for lap in laps {
    let samples = sqlx::query_as(..., "SELECT offset_ms, speed, throttle, brake, steering, gear, rpm FROM telemetry_samples WHERE lap_id = ? ORDER BY offset_ms ASC").bind(lap.id).fetch_all(telem_pool).await;
}
```

#### Admin Portal Page

**File:** `racingpoint-admin/src/app/(dashboard)/sessions/[id]/replay/page.tsx`
- `useEffect` fetches `/api/rc/admin/sessions/${id}/replay` on mount
- Playback state: `currentIndex`, `playing`, `speed` (1/2/5/10x)
- useEffect timer fires every `16ms / speed` to advance `currentIndex` by `speed * 16ms`
- Display: current telemetry values (speed, throttle, brake, steering, gear, rpm) as gauges/bars
- Timeline scrubber: `<input type="range" min=0 max={events.length} value={currentIndex} />`
- Racing theme: Asphalt Black bg, Racing Red (#E10600) progress fill, Montserrat font
- Lap markers: vertical lines on scrubber at lap_start events
- Speed controls: `[1x] [2x] [5x] [10x]` buttons

---

### Plan 367-04: Batch Export (GLD-G-04)

#### Backend API Route (racecontrol)

**Route:** `GET /admin/export?from=YYYY-MM-DD&to=YYYY-MM-DD&format=csv|jsonl&include=billing,telemetry,laps`
- Add to manager-role sub-router
- Validate: `to - from <= 30 days` (return 400 if exceeded)
- For CSV: use `csv` crate (already in Cargo.toml? — check; if not, use manual `format!` with commas — no extra dependency)
- For JSONL: serialize each row as `serde_json::Value` + newline

**Streaming approach:**
```rust
async fn admin_export(...) -> impl IntoResponse {
    // Build queries based on `include` param
    // Use tokio_stream::iter over DB rows
    // Return axum::body::Body::from_stream(stream)
    // Set Content-Disposition: attachment; filename="rp-export-{from}-{to}.csv"
}
```

Pattern check: `crates/racecontrol/src/api/routes.rs` line 24522 uses `axum::body::Body` — streaming is already used. Use `Body::from_stream(tokio_stream::iter(...))`.

**Estimated row counts (for UI pre-flight):**
- `GET /admin/export/estimate?from=&to=&include=` — returns `{"billing_rows": N, "lap_rows": N, "telemetry_rows": N}`
- Separate cheap COUNT(*) queries

**CSV schema per include:**
- billing: `session_id, driver_id, driver_name, pod_id, started_at, ended_at, allocated_seconds, driving_seconds, wallet_debit_paise, status, suspect, telemetry_coverage_pct, lap_count_actual, lap_count_expected`
- laps: `lap_id, billing_session_id, driver_id, lap_number, lap_time_ms, sector1_ms, sector2_ms, sector3_ms, valid, suspect`
- telemetry: `lap_id, offset_ms, speed, throttle, brake, steering, gear, rpm`

#### Admin Portal Page

**File:** `racingpoint-admin/src/app/(dashboard)/sessions/export/page.tsx`
- Date range picker: two `<input type="date">` fields (from/to)
- Format radio: CSV (default) / JSONL
- Include checkboxes: Billing (default on), Laps (default on), Telemetry (default off)
- "Estimate" button: fetches `/api/rc/admin/export/estimate?...` — shows "~N rows"
- "Export" button: `window.open('/api/rc/admin/export?...')` — browser handles download
- 30-day warning: show red text if date range exceeds 30 days
- Loading state during estimate fetch

---

### Plan 367-05: Phase 362 Retro-Validation (GLD-G-05)

#### Sub-task A: Deliberate-Mismatch WhatsApp E2E

Add `POST /internal/test/config-mismatch` under superadmin-only router (lines 656-678 of routes.rs):
```rust
// Superadmin-only test endpoint
async fn internal_test_config_mismatch(State(state), Json(req)) -> Json<Value> {
    let mismatch_msg = format!("⚠️ TEST MISMATCH — Pod {}\n{}", req.pod_id, req.detail);
    crate::whatsapp_alerter::send_whatsapp(&state.config, &mismatch_msg).await;
    // Also persist to config_mismatches for event archive verification
    let _ = sqlx::query("INSERT INTO config_mismatches (id, pod_id, sim_type, expected_fields, actual_fields, mismatched_fields, detected_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&req.pod_id).bind("TestSim").bind("{}").bind("{}").bind("[\"test_field\"]")
        .bind(chrono::Utc::now().to_rfc3339())
        .execute(&state.db).await;
    Json(json!({"ok": true, "message": mismatch_msg}))
}
```

E2E test: call endpoint with `pod_id: "test-pod"`, verify WhatsApp received on staff phone (manual verification step — document in VERIFICATION.md).

#### Sub-task B: 5-Adapter Runtime Verification

Add integration tests in `crates/rc-agent/tests/` (or `crates/rc-agent/src/sims/` unit tests) for each sim adapter:
- **AC:** Test that `assetto_corsa.rs` adapter reads a fixture shared-memory layout and produces a `ConfigMismatchDetected` with correct field values when car name differs
- **ACR (Assetto Corsa Evo):** Same for `assetto_corsa_evo.rs`
- **F1 25:** Same for `f1_25.rs`
- **iRacing:** Same for `iracing.rs`
- **LMU:** Same for `lmu.rs`

Each test: inject fixture data → call adapter's verify_config logic → assert mismatches Vec contains expected field.

**Pattern:** Each adapter has a `verify_running_config()` method called from `launch_verifier.rs` Stage 5. Tests use `sims/mod.rs`'s `GameConfig` type as the input fixture.

#### Sub-task C: 8-Pod Concurrent Load Test

Add integration test in `crates/racecontrol/tests/integration.rs`:
- Spin up 8 concurrent tasks, each calling the `ws/mod.rs` ConfigMismatchDetected handler path via a mock AppState
- Verify `config_mismatches` table has exactly 8 rows after all complete
- Use `tokio::join!` or `FuturesUnordered` for concurrent execution

NOTE: Must avoid `Instant::now() - Duration::from_secs(N)` pattern (see feedback_instant_arithmetic_on_ci.md — panics on fresh CI VMs). Use relative `tokio::time::sleep(Duration::from_millis(0)).await` pattern instead.

#### Sub-task D: Deferred Items from Phase 362

From `362-01-SUMMARY.md` deferred list:
1. **OpenAPI spec update** for `ConfigMismatchDetected` WS type — add to `docs/API.md` (or OpenAPI YAML if it exists in repo)
2. **`shared-types` TS package update** — check if `racingpoint-admin/src/` or monorepo has a `shared-types` package; if so, add `ConfigMismatchDetected` interface

---

## Existing Code: What to Reuse

| Asset | Location | Used in |
|-------|----------|---------|
| `list_disputes_handler` pattern | routes.rs:21117 | 367-01 suspect query pattern |
| `require_role_manager` layer | routes.rs:653 | All backend routes |
| `require_role_superadmin` layer | routes.rs:678 | 367-05 test endpoint |
| `public_lap_telemetry` handler | routes.rs:15067 | 367-01 heatmap + 367-03 replay fetch pattern |
| `state.telemetry_db` separate pool | state.rs:292 | 367-01 + 367-03 telemetry queries |
| `whatsapp_alerter::send_whatsapp()` | routes.rs:9093 | 367-05 WhatsApp E2E test |
| `DashboardEvent::ConfigMismatch` | ws/mod.rs:2264 | 367-05 broadcast verify |
| `analytics/page.tsx` recharts | admin/analytics | 367-01 heatmap Cell coloring |
| `billing/history/page.tsx` fetch pattern | admin/billing | 367-01..04 page pattern |
| RC_URL proxy | admin/api/rc/[...path] | All admin pages (auto-forwards) |
| `Skeleton.tsx`, `Toast.tsx` | admin/components | Error/loading states |

---

## Dependency Gates

1. **Phase 363 must be deployed** before Plans 01-04 backend routes can serve real data. The `suspect`, `suspect_reasons`, `telemetry_coverage_pct` columns only exist after Phase 363's migration runs on server .23.
2. **`config_mismatches` table** already exists (Phase 362, build `a9b5eaa3` deployed). Plan 367-05 can proceed independently.
3. **Plan 367-05** has no frontend dependency — can be executed in parallel with 01-04.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| Phase 363 not deployed when 367 executes | Plans 01-04 include Phase 363 deploy as a prerequisite check in `deploy:` section |
| telemetry.db separate pool — no `billing_session_id` index on `telemetry_samples` | `telemetry_samples.lap_id` indexed; join via `laps.billing_session_id` → `laps.id` → `telemetry_samples.lap_id` |
| Large telemetry replay (30+ laps, 10 Hz = 18000 samples) may be slow | Cap replay response at 10,000 events; note in UI if truncated |
| WhatsApp E2E test requires staff phone to be on and receiving | Document as manual verification step in VERIFICATION.md |
| 8-pod concurrent test hits SQLite write contention | Use `BEGIN IMMEDIATE` or serialize inserts via tokio task; SQLite WAL mode already enabled per CLAUDE.md |
| `config_mismatches` table schema — check it exists | Already deployed in `a9b5eaa3`, but Phase 367-05 should verify schema on startup |

---

## RESEARCH COMPLETE

Plans can be created now for:
- 367-01 (suspect sessions list + heatmap): backend route + admin page
- 367-02 (on-demand pod verify): backend route + admin page
- 367-03 (session replay): backend route + admin page
- 367-04 (batch export): backend route + estimate route + admin page
- 367-05 (GLD-G-05 retro-validation): 4 sub-tasks, racecontrol crate only

Total: 5 PLAN.md files needed.
