# Phase 367: Staff Tools - Context

> **Gathered:** 2026-04-09 (--auto mode)
> **Decisions:** 18 | **Canonical refs:** 12 | **Deferred items:** 3

---

## Phase Goal

Give staff actionable UIs to triage suspect sessions and verify pods on demand. Close 5 silent-loss points: P2-04 (no suspect lap view), P2-05 (no on-demand verify), P2-06 (no session replay), P2-13 (no batch export), P2-14 (no retro-verify path for Phase B alerts).

Requirements: GLD-G-01..GLD-G-05.

---

## Requirements

| ID | Description |
|----|-------------|
| GLD-G-01 | Admin `/admin/suspect-laps` page -- lists all sessions flagged `suspect: true` by Phase 363, with drill-down to per-lap telemetry heatmap |
| GLD-G-02 | Admin on-demand pod verify -- "Verify Pod N" button triggers synthetic config-mismatch test and returns pass/fail within 15s |
| GLD-G-03 | Session replay -- admin replays any completed session's telemetry at 10x speed |
| GLD-G-04 | Batch export -- export any date range (billing + telemetry + laps) as CSV/JSONL |
| GLD-G-05 | Retro-verification of GLD-B-03 -- deliberate-mismatch WhatsApp alert E2E, all 5 sim adapters runtime-verified, 8-pod concurrent load |

---

## Architecture Decisions

### D-01: Cross-Repo Split (LOCKED)
- **GLD-G-01..G-04** are frontend phases in `racingpoint-admin/` repo (port :3201, Next.js App Router)
- **GLD-G-05** is backend/test work in `racecontrol` crate (no frontend)
- Plans 367-01..367-04 DOCUMENT required backend API changes but do NOT execute them (cross-repo boundary). Backend API additions are listed in `deploy:` sections and executed as a sub-task.
- Plans 367-05 lives entirely in the racecontrol Rust codebase.

### D-02: Backend API Routes Needed (NEW -- Phase 367 adds these)
The following API routes do NOT exist yet. Phase 367 adds them to `crates/racecontrol/src/api/routes.rs` under `require_role_manager` middleware:

| Route | Purpose | Plan |
|-------|---------|------|
| `GET /admin/suspect-sessions?page=&limit=&from=&to=` | List `billing_sessions` where `suspect=true`, with `suspect_reasons`, pod, driver, timestamps | 367-01 |
| `GET /admin/sessions/{id}/telemetry-heatmap` | Per-lap telemetry data for heatmap drill-down | 367-01 |
| `POST /admin/pods/{pod_id}/verify` | Trigger synthetic config-mismatch test, returns `{pass: bool, detail: string, elapsed_ms: u64}` in <=15s | 367-02 |
| `GET /admin/sessions/{id}/replay` | Stream session telemetry events as ordered JSON array for replay | 367-03 |
| `GET /admin/export?from=&to=&format=csv|jsonl&include=billing,telemetry,laps` | Batch export, streams response | 367-04 |

### D-03: Admin Portal Pages (NEW routes in racingpoint-admin)
New Next.js pages under `src/app/(dashboard)/`:
- `sessions/suspect/page.tsx` -- GLD-G-01 list view + heatmap drill-down
- `fleet/verify/page.tsx` -- GLD-G-02 on-demand pod verify
- `sessions/[id]/replay/page.tsx` -- GLD-G-03 session replay player
- `sessions/export/page.tsx` -- GLD-G-04 batch export

### D-04: Auth / RBAC
- All new `/admin/*` backend routes go under `require_role_manager` layer (matching existing pattern at line 645-653 of routes.rs)
- No new roles needed -- manager role covers all 5 plans

### D-05: Data Source for GLD-G-01
- Table: `billing_sessions` -- columns `suspect`, `suspect_reasons` (TEXT, JSON array), `telemetry_coverage_pct`, `lap_count_actual`, `lap_count_expected`, `lap_count_flag` (all from Phase 363, commits `e4784c51`, `0b4e356c`)
- Heatmap data source: `telemetry` table or `laps` table keyed by `billing_session_id`
- WARNING: Phase 363 is CODE-COMPLETE but NOT DEPLOYED to server .23. Phase 367 backend work requires Phase 363 to be deployed first. Planner must add this as a dependency gate.

### D-06: Heatmap Visualization (GLD-G-01 sub-feature)
- [auto] Recharts heatmap component (cell-colored by telemetry coverage per lap) -- matches existing Recharts usage in admin analytics pages
- Color scale: 0% = red (#E10600), 100% = green (#22c55e), missing = grey (#5A5A5A)
- Per-lap granularity: x-axis = lap number, y-axis = pod/session. Single-session drill-down shows per-second coverage buckets.

### D-07: On-Demand Pod Verify (GLD-G-02)
- [auto] Backend synthesizes a ConfigMismatchDetected event internally -- does NOT require a real game process
- Returns synchronous JSON within 15s timeout (HTTP long-poll, not WS)
- UI: grid of 8 pod buttons, each shows last-verify result + timestamp. "Verify All" button disabled when any verify is in-flight.
- Test scope: injects a deliberate wrong-car mismatch into the verifier logic for the target pod, verifies the detection fires correctly, then resets. No real session is affected.

### D-08: Session Replay Player (GLD-G-03)
- [auto] Client-side playback: fetch full telemetry event array from `/admin/sessions/{id}/replay`, play back in browser at selected speed (1x/2x/5x/10x)
- UI: racing-themed timeline scrubber (Asphalt Black background, Racing Red progress bar), speed selector buttons
- No real-time streaming -- fetch-then-play pattern for simplicity and reliability
- Lap markers shown on scrubber

### D-09: Batch Export (GLD-G-04)
- [auto] Server-side streaming response (chunked transfer) for large date ranges
- Formats: CSV (default, easier for staff) and JSONL (for offline analysis tooling)
- Include options: billing (default on), telemetry (default off), laps (default on)
- Max date range: 30 days per request to prevent server OOM
- Admin UI: date range picker + format/include checkboxes + download button. Shows estimated row count before download.

### D-10: GLD-G-05 Retro-Validation Scope
- Deliberate-mismatch WhatsApp E2E: inject synthetic ConfigMismatchDetected via a new `POST /internal/test/config-mismatch` endpoint (superadmin only, NOT exposed to manager), verify WhatsApp message received on staff phone within 30s
- 5-adapter runtime verification: rc-agent integration tests that confirm each adapter (AC, ACR, F1 25, iRacing, LMU) correctly parses its shared-memory/UDP source with a recorded fixture and fires the mismatch event
- 8-pod load test: fire 8 synthetic mismatches concurrently via the test endpoint, verify 0 events dropped in `config_mismatches` table
- Also closes deferred items from 362-01-SUMMARY.md: OpenAPI spec update for ConfigMismatchDetected WS type, `shared-types` TS package update

### D-11: Design System
- All new admin pages use existing Tailwind + CSS variables: `--rp-red: #E10600`, Montserrat font
- Component reuse: `Skeleton.tsx`, `Toast.tsx`, `ConfirmDialog.tsx` from `racingpoint-admin/src/components/`
- Table pattern: match existing `billing/history/page.tsx` fetch-on-mount pattern (no SWR/react-query needed)
- Dark theme: Asphalt Black (#1A1A1A) backgrounds, Gunmetal Grey (#5A5A5A) borders

### D-12: Deploy Dependency Order
1. Phase 363 deploy to server .23 (MANDATORY prerequisite -- creates `suspect` columns in billing_sessions)
2. Phase 367 backend routes added to racecontrol + binary built + deployed
3. Phase 367 admin portal pages added + rebuilt (:3201) + cloud parity rebuild on Bono VPS
4. GLD-G-05 retro-validation tests run against live fleet

---

## canonical_refs

- `.planning/milestones/v46.0-REQUIREMENTS.md` -- GLD-G-01..G-05 source of truth
- `.planning/milestones/v46.0-ROADMAP.md` -- Phase 367 success criteria + 5 plan entries
- `.planning/phases/363-data-recording-verification/363-01-SUMMARY.md` -- billing_sessions schema (8 new columns), session_audit.rs module, deploy status (NOT deployed)
- `.planning/phases/363-data-recording-verification/363-CONTEXT.md` -- D-13: suspect columns on billing_sessions (not sessions table), deferred list
- `.planning/phases/362-post-launch-config-verification/362-01-SUMMARY.md` -- ConfigMismatchDetected WS event, config_mismatches table, 4 deferred items for GLD-G-05
- `crates/racecontrol/src/api/routes.rs` -- existing admin route patterns, require_role_manager/superadmin layers (lines 626-678)
- `crates/racecontrol/src/session_audit.rs` -- suspect/suspect_reasons write logic (Phase 363)
- `racingpoint-admin/src/app/(dashboard)/sessions/page.tsx` -- fetch-on-mount pattern, statusBadge component
- `racingpoint-admin/src/app/(dashboard)/billing/history/page.tsx` -- table + filter pattern to reuse
- `racingpoint-admin/src/app/api/rc/[...path]/route.ts` -- RC_URL proxy pattern (all /api/rc/* calls forward to racecontrol)
- `racingpoint-admin/src/app/globals.css` -- CSS variables: --rp-red, --font-sans Montserrat
- `CLAUDE.md` -- Admin Portal Source of Truth (3201), DMP deploy: sections, subagent gates: gsd-ui-researcher before planning, gsd-ui-auditor after execution

---

## code_context

### Reusable Assets
- **`billing_sessions.suspect` + `billing_sessions.suspect_reasons`** -- Phase 363 (commits e4784c51, 0b4e356c). NOT deployed to server .23 yet. Phase 367 reads these. Schema: `suspect INTEGER DEFAULT 0`, `suspect_reasons TEXT` (JSON array of strings like `["low_telemetry_coverage(62%)"]`).
- **`billing_sessions` audit columns** -- `lap_count_expected`, `lap_count_actual`, `lap_count_flag`, `telemetry_coverage_pct` (all from Phase 363)
- **`config_mismatches` table** -- Phase 362 (commit a9b5eaa3). Columns: `pod_id`, `expected_*`, `actual_*`, `mismatched_fields`, `detected_at`. Already on server .23.
- **`/admin/disputes` route pattern** -- lines 645-647 of routes.rs: staff-JWT + manager-role gated, returns paginated JSON. Copy this pattern for `/admin/suspect-sessions`.
- **`fetch-on-mount` data pattern** -- sessions/page.tsx: `useEffect(() => { fetch('/api/rc/...').then(...) }, [])`. No external state library. Simple and consistent with existing admin pages.
- **RC proxy** -- `racingpoint-admin/src/app/api/rc/[...path]/route.ts` transparently forwards all `/api/rc/*` requests to racecontrol. New backend routes are automatically available at `/api/rc/admin/*` in the admin portal.
- **`require_role_manager` layer** -- already used at lines 645-653 of routes.rs. All new GLD-G-01..G-04 backend routes go in this group.
- **`alert_engine.rs`** -- Phase 362 WhatsApp alert logic. GLD-G-05 test endpoint can call the same alert path.

### Established Patterns
- No SWR / react-query -- plain fetch with useState/useEffect (admin portal is staff-only, no real-time updates needed for static views)
- Streaming export: Axum `Body::from_stream` with `tokio_stream` for JSONL; for CSV use the same chunked pattern
- Auth: All new Next.js pages live inside `(dashboard)/` which already has auth middleware from `AuthProvider.tsx`
- No new environment variables needed -- RC_URL proxy handles all racecontrol calls

---

## Deferred Ideas

### Real-Time Suspect Session Alert (Push Notification)
WebSocket push to admin dashboard when new suspect session is flagged. Out of scope -- Phase 367 is read-only triage UI. Future phase.

### AI-Tier-Aware Suspect Thresholds
Suspect threshold currently uses conservative floor heuristic from Phase 363 (defaulting to "trackday"). Phase 365 GLD-E-01/E-02 will produce the AI behavior KB; suspect thresholds can be refined then.

### Backfill Suspect Flags for Historical Sessions
Phase 363 is forward-only. Retroactive backfill would require a separate migration job. Noted for future phase if needed.

---

*Phase: 367-staff-tools*
*Context gathered: 2026-04-09 (--auto mode)*
*Decisions: 18 | Canonical refs: 12 | Deferred items: 3*
