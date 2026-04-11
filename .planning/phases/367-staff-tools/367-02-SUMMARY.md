---
plan: 367-02
phase: 367
title: On-Demand Pod Verify Endpoint + Admin UI
status: complete
completed_date: "2026-04-11"
duration_minutes: 25
tasks_completed: 2
tasks_total: 2
files_created: 1
files_modified: 2
commits:
  - repo: racecontrol
    hash: b36e8a7b
    message: "fix(367): remove duplicate handler definitions from stash merge (includes 367-02 backend handler)"
  - repo: racingpoint-admin
    hash: b011f35
    message: "feat(367-02): add Pod Verify admin page at /fleet/verify (GLD-G-02)"
key_decisions:
  - "Used content_drift_events table instead of plan's config_mismatches (which does not exist in schema)"
  - "Used pods table last_seen column instead of plan's pod_status table (which does not exist in schema)"
  - "Lock guard dropped before .await via tight { } scope block (CLAUDE.md lock-across-await rule)"
requirements_closed:
  - GLD-G-02
tags: [fleet, admin, verification, staff-tools, rust, nextjs]
---

# Phase 367 Plan 02: On-Demand Pod Verify Endpoint + Admin UI Summary

**One-liner:** POST /admin/pods/{pod_id}/verify with 15s timeout + 8-pod React grid at /fleet/verify using brand colors and Verify All control.

## What Was Built

### Task 01: Backend Route (racecontrol)

Added `POST /api/v1/admin/pods/{pod_id}/verify` to the manager-role sub-router in `crates/racecontrol/src/api/routes.rs`.

Handler `admin_verify_pod_handler`:
- 15-second `tokio::time::timeout` wrapping all DB queries
- Check 1: WS connectivity via `agent_senders.read()` — lock acquired, bool extracted, guard dropped before any `.await`
- Check 2: Last content drift event timestamp from `content_drift_events` table (per pod)
- Check 3: `last_seen` from `pods` table
- Returns JSON: `{ pod_id, pass, detail, last_mismatch_check, last_seen, elapsed_ms }`
- On timeout: `{ pod_id, pass: false, detail: "Timeout after 15s", elapsed_ms: 15000 }`

### Task 02: Admin Portal Page (racingpoint-admin)

Created `src/app/(dashboard)/fleet/verify/page.tsx`:
- 8-pod grid (1/2/4 column responsive layout)
- `StatusDot` component: grey=idle, yellow pulsing=in-flight, green=pass, red=fail
- Per-pod Verify button with disabled state while verifying
- Verify All button disabled while any pod verification is in-flight (`anyVerifying` flag)
- Shows elapsed_ms, PASS/FAIL label, detail text, last_mismatch_check timestamp (IST locale)
- Brand colors: `#E10600` red, `#222` card, `#333` border, Montserrat font
- Added "Pod Verify" nav item to AdminLayout fleet section between Content Drift and Metrics

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Adaptation] Replaced non-existent tables with actual schema tables**
- **Found during:** Task 01 implementation
- **Issue:** Plan referenced `config_mismatches` table (Check 2) and `pod_status` table (Check 3) — neither exists in the DB schema
- **Fix:** Used `content_drift_events` table for last verification activity timestamp, and `pods` table `last_seen` column for pod activity signal
- **Files modified:** `crates/racecontrol/src/api/routes.rs`

## Known Stubs

None — the page fetches live data from the backend endpoint. The endpoint returns real WS connectivity and DB state. No mock/hardcoded data.

## Verification

- `cargo check --bin racecontrol` — Finished dev profile, 0 errors, 20+1 pre-existing warnings only
- Route uniqueness check — `/admin/pods/{pod_id}/verify` not in duplicate list (only pre-existing duplicates)
- Lock-across-await confirmed clean: `senders.contains_key(&pod_id)` inside `{ }` block, guard dropped before `.await`
- Acceptance criteria:
  - `grep "admin/pods/{pod_id}/verify" routes.rs` returns `.route(` line at 672 ✅
  - `grep "admin_verify_pod_handler" routes.rs` returns route (672) + `async fn` (24873) ✅
  - `grep "admin/pods" page.tsx` returns fetch URL at line 29 ✅
  - `grep "Verify All" page.tsx` returns button label at line 73 ✅
  - `grep "verifyingAll|anyVerifying" page.tsx` shows disabled-when-in-flight logic ✅

## Self-Check: PASSED

- `crates/racecontrol/src/api/routes.rs` — modified, in git HEAD b36e8a7b ✅
- `racingpoint-admin/src/app/(dashboard)/fleet/verify/page.tsx` — created, in git b011f35 ✅
- `racingpoint-admin/src/components/AdminLayout.tsx` — modified, in git b011f35 ✅
