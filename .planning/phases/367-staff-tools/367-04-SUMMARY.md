---
plan: 367-04
title: Batch Export Endpoint + Admin UI
phase: 367
subsystem: [racecontrol-api, racingpoint-admin]
tags: [export, csv, jsonl, batch, staff-tools, gld-g-04]
dependency_graph:
  requires: [367-01]
  provides: [GLD-G-04]
  affects: [racecontrol-api, racingpoint-admin]
tech_stack:
  added: []
  patterns: [axum-streaming-response, next-client-component, date-range-validation]
key_files:
  created:
    - racingpoint-admin/src/app/(dashboard)/sessions/export/page.tsx
  modified:
    - crates/racecontrol/src/api/routes.rs
decisions:
  - Backend handlers committed in prior session (9b6e94f3) — Task 01 was pre-complete; Task 02 (admin page) executed here
  - 30-day cap enforced on both client (daysDiff > 30 guard) and server (num_days() > 30 → 400)
  - CSV uses semicolon-replacement for driver names containing commas
  - Telemetry estimate uses first-lap sample count × lap count (avoids N+1 on estimate path)
  - window.open() used for export download (triggers browser file save dialog without AJAX blob overhead)
metrics:
  duration_minutes: 12
  completed_date: "2026-04-11"
  tasks_completed: 2
  tasks_total: 2
  files_created: 1
  files_modified: 0
---

# Phase 367 Plan 04: Batch Export Endpoint + Admin UI Summary

## One-liner

`GET /admin/export` + `GET /admin/export/estimate` with 30-day cap, CSV/JSONL format, billing/laps/telemetry include-flags, and a React admin page with date picker, row estimate, and download trigger.

## Tasks Completed

| Task | Description | Commit | Repo |
|------|-------------|--------|------|
| 01 | Backend: `/admin/export/estimate` + `/admin/export` handlers | `9b6e94f3` | racecontrol |
| 02 | Admin UI: `/sessions/export` page with date range, format, includes, estimate, download | `8e434de` | racingpoint-admin |

## What Was Built

### Task 01 — Backend (pre-existing commit `9b6e94f3`)

Both routes registered in manager-role sub-router:
- `GET /api/v1/admin/export/estimate?from&to&include` — COUNT(*) only, returns `{billing_rows, lap_rows, telemetry_rows, total_rows}`
- `GET /api/v1/admin/export?from&to&format&include` — streams CSV or JSONL response with `Content-Disposition: attachment`

Guards:
- 30-day range limit enforced server-side (`chrono::NaiveDate` diff > 30 → HTTP 400)
- Telemetry uses separate `telem_pool` (falls back to `state.db` if no dedicated telemetry DB)
- CSV driver names: commas replaced with semicolons to preserve column integrity

### Task 02 — Admin Page (`8e434de`)

File: `racingpoint-admin/src/app/(dashboard)/sessions/export/page.tsx`

- Date range picker defaulting to last 30 days → today
- Client-side 30-day validation with inline error message and button disable
- Format radio: CSV (recommended for Excel) / JSONL (for tooling)
- Include checkboxes: Billing Sessions, Lap Data, Telemetry Samples (off by default — large)
- "Estimate Rows" button: fetches `/api/rc/admin/export/estimate`, shows per-type counts + total
- Large export warning shown when `total_rows > 100_000`
- "Export" button: `window.open()` to `/api/rc/admin/export?...` — triggers browser file download
- Racing Point brand colors (`#E10600`, `#222`, `#333`, Montserrat font)

## Verification Results

- `grep -n "admin/export" crates/racecontrol/src/api/routes.rs` → both `.route()` lines at 676-677
- `grep -n "admin_export_handler\|admin_export_estimate_handler" crates/racecontrol/src/api/routes.rs` → defs at 25029, 25105
- `grep -n "30 days\|num_days" crates/racecontrol/src/api/routes.rs` → 30-day guard confirmed
- `cargo build --release --bin racecontrol 2>&1 | grep -i "^error"` → empty (clean build)
- Admin page: `rangeExceeds`, `daysDiff`, `window.open`, both `/api/rc/admin/export` URLs — all present
- No duplicate routes for `/admin/export` or `/admin/export/estimate`

## Deviations from Plan

### Pre-completed Task 01

**Found during:** Initial read of routes.rs
**Issue:** Backend handlers (`admin_export_estimate_handler`, `admin_export_handler`) and both `.route()` registrations were already present in commit `9b6e94f3` from a prior parallel session.
**Action:** Verified implementation matches plan spec exactly (30-day guard, CSV/JSONL, billing/laps/telemetry sections, Content-Disposition header). No changes needed. Proceeded directly to Task 02.

## Known Stubs

None. The export page wires directly to live backend endpoints — no mock data, no placeholders.

## Self-Check: PASSED

- `racingpoint-admin/src/app/(dashboard)/sessions/export/page.tsx` — FOUND
- racecontrol commit `9b6e94f3` — FOUND (`git log --oneline --all | grep 367-04`)
- racingpoint-admin commit `8e434de` — FOUND (`git log --oneline -1` in racingpoint-admin)
